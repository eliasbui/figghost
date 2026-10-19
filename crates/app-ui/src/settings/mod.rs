//! The settings window: a web page on a glass window, and the bridge that applies its changes live.

pub mod model;
mod page;

use crate::{context::AppContext, i18n, trace};
use core_agents::Agent;
use macos_sys::{
    app,
    login_item::{self, LoginItem},
    pasteboard::{Pasteboard, Write},
    permissions,
    webview::{WebWindow, WindowSpec},
    workspace,
};
use model::{Effect, Env, Fault, Message, Purpose, apply, hotkey_spec, state};
use serde_json::{Value, json};
use std::{
    cell::RefCell,
    collections::HashMap,
    process::Command,
    rc::{Rc, Weak},
};

const WINDOW_WIDTH: f64 = 1000.0;
const WINDOW_HEIGHT: f64 = 700.0;
const MIN_WIDTH: f64 = 820.0;
const MIN_HEIGHT: f64 = 560.0;
/// How often an open window re-reads the state, to follow the menu, the permissions and the clipboard count.
const REFRESH_SECONDS: f64 = 2.0;
const ACCESSIBILITY_PANE: &str =
    "x-apple.systempreferences:com.apple.preference.security?Privacy_Accessibility";
/// Icons are sent as PNG data URLs; this many apps at most per request.
const MAX_ICONS: usize = 60;
const ICON_PIXELS: f64 = 48.0;

type ShowIcon = Box<dyn Fn(bool)>;

thread_local! {
    static FEATURE: RefCell<Weak<SettingsFeature>> = const { RefCell::new(Weak::new()) };
}

/// Runs `f` on the window's feature, if it is still there, from a callback of another thread that hopped to
/// the main one.
fn with_feature(f: impl FnOnce(&SettingsFeature)) {
    FEATURE.with(|slot| {
        if let Some(feature) = slot.borrow().upgrade() {
            f(&feature);
        }
    });
}

pub struct SettingsFeature {
    ctx: Rc<AppContext>,
    me: Weak<SettingsFeature>,
    window: RefCell<Option<Rc<WebWindow>>>,
    refresh: RefCell<Option<app::Timer>>,
    names: RefCell<HashMap<String, String>>,
    icons: RefCell<HashMap<String, String>>,
    last_state: RefCell<String>,
    /// Shows or hides the menu bar icon; set once the menu exists.
    menu_icon: RefCell<Option<ShowIcon>>,
}

impl SettingsFeature {
    pub fn new(ctx: Rc<AppContext>) -> Rc<Self> {
        let feature = Rc::new_cyclic(|me| Self {
            ctx,
            me: me.clone(),
            window: RefCell::default(),
            refresh: RefCell::default(),
            names: RefCell::default(),
            icons: RefCell::default(),
            last_state: RefCell::default(),
            menu_icon: RefCell::default(),
        });
        FEATURE.with(|slot| *slot.borrow_mut() = Rc::downgrade(&feature));
        // The About page follows the update, which goes on whether or not this window is open.
        feature
            .ctx
            .update
            .on_status(|status| with_feature(|this| this.post(status)));
        feature
    }

    /// How a change of the "menu bar icon" setting reaches the menu bar.
    pub fn on_menu_icon(&self, show: impl Fn(bool) + 'static) {
        *self.menu_icon.borrow_mut() = Some(Box::new(show));
    }

    fn notch(&self) -> Option<Rc<macos_sys::notch::Notch>> {
        self.ctx.clipboard.as_ref().and_then(|c| c.notch.clone())
    }

    /// The window follows the theme setting: the system's appearance, or always light or dark.
    fn apply_theme(&self) {
        let dark = match self.ctx.config.borrow().theme.as_str() {
            "light" => Some(false),
            "dark" => Some(true),
            _ => None,
        };
        if let Some(window) = self.window.borrow().as_ref() {
            window.set_dark(dark);
        }
    }

    /// Opens the window, or brings it to the front. While it is open the app is a regular one, with a
    /// Dock icon and a menu bar.
    pub fn show(&self) {
        let mtm = self.ctx.mtm;
        let existing = self.window.borrow().clone();
        if let Some(window) = existing {
            window.show(mtm);
            return;
        }
        app::set_regular(mtm, true);
        let (on_message, on_close) = (self.me.clone(), self.me.clone());
        let window = WebWindow::new(
            mtm,
            &WindowSpec {
                title: "Figghost",
                width: WINDOW_WIDTH,
                height: WINDOW_HEIGHT,
                min_width: MIN_WIDTH,
                min_height: MIN_HEIGHT,
            },
            &page::html(),
            move |text| {
                if let Some(this) = on_message.upgrade() {
                    this.handle(&text);
                }
            },
            move || {
                if let Some(this) = on_close.upgrade() {
                    this.closed();
                }
            },
        );
        *self.window.borrow_mut() = Some(Rc::clone(&window));
        self.apply_theme();
        self.last_state.borrow_mut().clear();
        let weak = self.me.clone();
        *self.refresh.borrow_mut() = Some(app::every(REFRESH_SECONDS, move || {
            if let Some(this) = weak.upgrade() {
                this.push_state(false);
            }
        }));
        window.show(mtm);
    }

    /// Pushes the current state to an open window, for changes made elsewhere (the menu).
    pub fn refresh(&self) {
        self.push_state(false);
    }

    fn closed(&self) {
        self.refresh.borrow_mut().take();
        // The page may have been capturing a shortcut when it went away.
        self.recording(false);
        let weak = self.me.clone();
        // The window is still inside its own delegate call, so it is dropped on the next turn of the run loop.
        app::after(0.0, move || {
            let Some(this) = weak.upgrade() else {
                return;
            };
            let reopened = this
                .window
                .borrow()
                .as_ref()
                .is_some_and(|w| w.is_visible());
            if !reopened {
                this.window.borrow_mut().take();
                app::set_regular(this.ctx.mtm, false);
            }
        });
    }

    fn post(&self, message: &Value) {
        if let Some(window) = self.window.borrow().as_ref() {
            window.host.post(&message.to_string());
        }
    }

    fn fault(&self, fault: &Fault) {
        trace::log(&format!(
            "settings: refused: {} ({})",
            fault.code, fault.detail
        ));
        self.post(&fault.to_json());
    }

    fn name_of(&self, id: &str) -> String {
        self.names
            .borrow_mut()
            .entry(id.to_owned())
            .or_insert_with(|| workspace::app_name(id).unwrap_or_else(|| id.to_owned()))
            .clone()
    }

    fn env(&self) -> Env {
        let (history_count, shelf_count) = (
            self.ctx.clipboard.as_ref().map_or(0, |c| c.history.count()),
            self.ctx.shelf.as_ref().map_or(0, |s| s.count()),
        );
        let config = self.ctx.config.borrow();
        let ids = config.mouse.apps.keys().chain(&config.ignored_apps);
        let names = ids.map(|id| (id.clone(), self.name_of(id))).collect();
        let login = login_item::status();
        let notch = self.notch();
        Env {
            version: env!("CARGO_PKG_VERSION"),
            bundled: login != LoginItem::Unavailable,
            config_dir: self.ctx.home.to_string_lossy().into_owned(),
            exe: std::env::current_exe()
                .map(|p| p.to_string_lossy().into_owned())
                .unwrap_or_default(),
            accessibility: permissions::accessibility_trusted(false),
            login,
            language: i18n::current(),
            history_count,
            shelf_count,
            names,
            display: workspace::display_options(),
            agents: self
                .ctx
                .agents
                .as_ref()
                .map(|agents| agents.status())
                .unwrap_or_default(),
            notch_pages: notch.as_ref().map_or_else(Vec::new, |n| {
                n.page_ids()
                    .into_iter()
                    .map(|(id, title)| (id.to_owned(), title))
                    .collect()
            }),
            notch_hardware: notch.as_ref().is_some_and(|n| n.on_hardware_notch()),
            screens: workspace::screen_names(self.ctx.mtm),
            runtime: self.ctx.features.runtime(),
        }
    }

    /// Sends the state to the page; `force` sends it even when it has not changed since the last time.
    fn push_state(&self, force: bool) {
        if self.window.borrow().is_none() {
            return;
        }
        let json = state(&self.ctx.config.borrow(), &self.env());
        let text = json.to_string();
        if !force && *self.last_state.borrow() == text {
            return;
        }
        *self.last_state.borrow_mut() = text;
        self.post(&json!({ "t": "state", "state": json }));
    }

    fn handle(&self, text: &str) {
        let message: Message = match serde_json::from_str(text) {
            Ok(message) => message,
            Err(e) => {
                let head: String = text.chars().take(80).collect();
                eprintln!("settings: unreadable message ({e}): {head}");
                return;
            }
        };
        match &message {
            Message::Ready => {
                self.push_state(true);
                self.post(&self.ctx.update.status());
            }
            Message::JsError { message, line } => {
                eprintln!("settings page: {message} (line {line})");
            }
            Message::Page { .. } => {}
            Message::Recording { on } => self.recording(*on),
            Message::Action { name } if name != "reset_tiling" => self.action(name),
            Message::PickApp { purpose } => self.pick_app(*purpose),
            Message::Running { purpose } => self.running_apps(*purpose),
            Message::Curve { duration_ms } => self.post(&model::curve(*duration_ms)),
            Message::Icons { ids } => self.icons(ids),
            edit => self.edit(edit),
        }
    }

    /// Applies a change to the config and to the running app; a change the app cannot carry out (a
    /// shortcut another app owns, missing permission) is rolled back and reported to the page.
    fn edit(&self, message: &Message) {
        let before = self.ctx.config.borrow().clone();
        let effect = apply(&mut self.ctx.config.borrow_mut(), message);
        let rollback = effect.clone().ok();
        let result = effect.and_then(|effect| self.perform(&effect).map(|()| effect));
        match result {
            Ok(_) => {
                self.ctx.save_config();
                // Menu entries read the config when the menu opens; nothing else needs telling.
            }
            Err(fault) => {
                *self.ctx.config.borrow_mut() = before;
                // Put the previous shortcuts and settings back in force.
                if let Message::Hotkey { target, .. } = message {
                    let _ = self.perform(&Effect::Hotkey(target.clone()));
                }
                if let Some(Effect::Feature(id)) = rollback {
                    let _ = self.perform(&Effect::Feature(id));
                }
                self.fault(&fault);
            }
        }
        self.push_state(true);
    }

    fn perform(&self, effect: &Effect) -> Result<(), Fault> {
        let config = self.ctx.config.borrow();
        match effect {
            Effect::None | Effect::Language => Ok(()),
            Effect::Mouse => {
                let settings = config.mouse.clone().sanitized();
                self.ctx.control.edit(|live| *live = settings);
                Ok(())
            }
            Effect::LoginItem(on) => {
                login_item::set_enabled(*on).map_err(|e| Fault::new("login_failed", e))
            }
            Effect::VolumeHud(on) => self.ctx.volume_hud.set_enabled(*on).map_err(|e| {
                eprintln!("volume hud: cannot start ({e:?}); Accessibility access is needed");
                permissions::accessibility_trusted(true);
                Fault::new("accessibility_needed", "volume_hud")
            }),
            Effect::Hud => {
                self.ctx.volume_hud.set_style(config.notch.hud_style());
                Ok(())
            }
            Effect::History => {
                if let Some(clipboard) = &self.ctx.clipboard {
                    clipboard.history.configure(
                        config.history_limit,
                        config.ignored_apps.clone(),
                        config.auto_paste,
                        config.paste_plain,
                    );
                }
                Ok(())
            }
            Effect::Agents => {
                if let Some(agents) = &self.ctx.agents {
                    agents.configure(&config.agents);
                }
                Ok(())
            }
            Effect::Notch => {
                if let Some(notch) = self.notch() {
                    notch.configure(&config.notch.style());
                    notch.set_enabled(config.notch.enabled);
                }
                if let Some(agents) = &self.ctx.agents {
                    agents.configure_notch(&config.notch);
                }
                if let Some(live) = &self.ctx.live_media {
                    live.configure(&config.notch);
                }
                Ok(())
            }
            Effect::MenuIcon(show) => {
                if let Some(set) = self.menu_icon.borrow().as_ref() {
                    set(*show);
                }
                Ok(())
            }
            Effect::Theme => {
                drop(config);
                self.apply_theme();
                Ok(())
            }
            Effect::Tiling => {
                for failure in self.ctx.tiling.configure(&config.tiling) {
                    eprintln!("tiling: {}", failure.note);
                }
                Ok(())
            }
            Effect::Feature(id) => self.ctx.features.configure(id, &config),
            Effect::Hotkey(target) => self.register_hotkey(&config, target),
        }
    }

    fn register_hotkey(&self, config: &crate::config::Config, target: &str) -> Result<(), Fault> {
        if target == "notch" {
            let Some(clipboard) = &self.ctx.clipboard else {
                return Ok(());
            };
            return clipboard
                .set_notch_hotkey(hotkey_spec(config, target))
                .map_err(|status| Fault::new("taken", format!("{target} (status {status})")));
        }
        if target == "clipboard" {
            let (Some(clipboard), Some(spec)) = (&self.ctx.clipboard, hotkey_spec(config, target))
            else {
                return Ok(());
            };
            return clipboard
                .set_hotkey(spec)
                .map_err(|status| Fault::new("taken", format!("{target} (status {status})")));
        }
        let failures = self.ctx.tiling.configure(&config.tiling);
        let layout = target.strip_prefix("tiling:").unwrap_or_default();
        for failure in &failures {
            eprintln!("tiling: {}", failure.note);
        }
        match failures.iter().find(|f| f.layout.key() == layout) {
            Some(failure) => Err(Fault::new("taken", failure.note.clone())),
            None => Ok(()),
        }
    }

    /// While the page captures a key combination, the global shortcuts are released so that pressing an
    /// existing one is recorded instead of triggering it.
    fn recording(&self, on: bool) {
        if on {
            if let Some(clipboard) = &self.ctx.clipboard {
                clipboard.suspend_hotkey();
            }
            self.ctx.tiling.suspend();
            self.ctx.features.suspend_hotkeys();
            return;
        }
        let config = self.ctx.config.borrow();
        if let (Some(clipboard), Some(spec)) =
            (&self.ctx.clipboard, hotkey_spec(&config, "clipboard"))
            && let Err(status) = clipboard.set_hotkey(spec)
        {
            eprintln!("hotkey: cannot register again (status {status})");
        }
        if let Some(clipboard) = &self.ctx.clipboard
            && let Err(status) = clipboard.set_notch_hotkey(hotkey_spec(&config, "notch"))
        {
            eprintln!("hotkey: cannot register the notch shortcut again (status {status})");
        }
        for failure in self.ctx.tiling.configure(&config.tiling) {
            eprintln!("tiling: {}", failure.note);
        }
        self.ctx.features.resume_hotkeys(&config);
    }

    fn action(&self, name: &str) {
        let mtm = self.ctx.mtm;
        match name {
            "open_accessibility" => {
                permissions::accessibility_trusted(true);
                workspace::open_url(ACCESSIBILITY_PANE);
            }
            "open_login_settings" => login_item::open_settings(),
            "check_updates" => self.ctx.update.check_now(),
            "install_update" => self.ctx.update.install_available(),
            "open_release" => self.ctx.update.open_release(),
            "open_config" => {
                let _ = std::fs::create_dir_all(&self.ctx.home);
                workspace::open_path(&self.ctx.home);
            }
            "copy_mcp_config" => {
                let exe = std::env::current_exe().map(|p| p.to_string_lossy().into_owned());
                if let Ok(exe) = exe {
                    Pasteboard::general().write(Write::Text(&mcp_config(&exe)));
                }
            }
            "clear_history" => {
                if let Some(clipboard) = &self.ctx.clipboard {
                    clipboard.history.clear_unpinned();
                }
            }
            "clear_shelf" => {
                if let Some(shelf) = &self.ctx.shelf {
                    shelf.clear();
                }
            }
            "agents_install_claude" => self.agent_hooks(Agent::Claude, true),
            "agents_remove_claude" => self.agent_hooks(Agent::Claude, false),
            "agents_install_codex" => self.agent_hooks(Agent::Codex, true),
            "agents_remove_codex" => self.agent_hooks(Agent::Codex, false),
            "agents_usage_on" => self.agent_usage(true),
            "agents_usage_off" => self.agent_usage(false),
            "restart" => relaunch(mtm, "--show-settings"),
            "quit" => app::quit(mtm),
            other => {
                let result = {
                    let mut config = self.ctx.config.borrow_mut();
                    self.ctx.features.action(other, &mut config)
                };
                match result {
                    Some(Ok(())) => self.ctx.save_config(),
                    Some(Err(fault)) => self.fault(&fault),
                    None => {
                        self.fault(&Fault::new("unknown_action", other));
                        return;
                    }
                }
            }
        }
        self.push_state(true);
    }

    /// Adds our hooks to an agent's file, or takes them out. Installing also turns the listening on, since
    /// hooks nobody listens to would do nothing.
    fn agent_hooks(&self, agent: Agent, on: bool) {
        let Some(agents) = self.ctx.agents.clone() else {
            self.fault(&Fault::new("agents_failed", "the notch is off"));
            return;
        };
        let result = if on {
            agents.install(agent).map(|_| ())
        } else {
            agents.remove(agent).map(|_| ())
        };
        match result {
            Ok(()) => {
                if on && !self.ctx.config.borrow().agents.enabled {
                    self.ctx.config.borrow_mut().agents.enabled = true;
                    self.ctx.save_config();
                    agents.configure(&self.ctx.config.borrow().agents);
                }
            }
            Err(e) => self.fault(&Fault::new("agents_failed", e)),
        }
    }

    fn agent_usage(&self, on: bool) {
        let Some(agents) = self.ctx.agents.clone() else {
            self.fault(&Fault::new("agents_failed", "the notch is off"));
            return;
        };
        let result = if on {
            agents.install_usage_tap()
        } else {
            agents.remove_usage_tap()
        };
        if let Err(e) = result {
            self.fault(&Fault::new("agents_failed", e));
        }
    }

    fn pick_app(&self, purpose: Purpose) {
        let Some(picked) = workspace::pick_app(self.ctx.mtm, i18n::current().pick_app_prompt())
        else {
            return;
        };
        self.names
            .borrow_mut()
            .insert(picked.bundle_id.clone(), picked.name);
        let message = match purpose {
            Purpose::Scroll => Message::AppAdd {
                id: picked.bundle_id,
            },
            Purpose::Ignore => Message::IgnoredAdd {
                id: picked.bundle_id,
            },
        };
        self.edit(&message);
    }

    fn running_apps(&self, purpose: Purpose) {
        let mut apps = workspace::running_apps();
        apps.sort_by_key(|a| a.name.to_lowercase());
        apps.dedup_by(|a, b| a.bundle_id == b.bundle_id);
        let list: Vec<Value> = apps
            .into_iter()
            .map(|a| json!({ "id": a.bundle_id, "name": a.name }))
            .collect();
        let purpose = match purpose {
            Purpose::Scroll => "scroll",
            Purpose::Ignore => "ignore",
        };
        self.post(&json!({ "t": "running", "purpose": purpose, "apps": list }));
    }

    fn icons(&self, ids: &[String]) {
        let mut found = serde_json::Map::new();
        for id in ids.iter().take(MAX_ICONS) {
            let cached = self.icons.borrow().get(id).cloned();
            let url = cached.or_else(|| {
                let png = workspace::app_icon_png(id, ICON_PIXELS)?;
                let url = format!("data:image/png;base64,{}", base64(&png));
                self.icons.borrow_mut().insert(id.clone(), url.clone());
                Some(url)
            });
            if let Some(url) = url {
                found.insert(id.clone(), Value::String(url));
            }
        }
        self.post(&json!({ "t": "icons", "icons": found }));
    }
}

/// Starts a new copy of the app and quits this one. The new copy waits for this one to exit, so the two
/// never hold the same global shortcuts. `flag` is how the new copy is told to start: `--show-settings` for
/// a restart asked for from the settings window, `--background` for one that should come back quietly.
pub(crate) fn relaunch(mtm: macos_sys::MainThreadMarker, flag: &str) {
    let Ok(exe) = std::env::current_exe() else {
        return;
    };
    let script = relaunch_script(&exe, flag);
    if let Err(e) = Command::new("/bin/sh").args(["-c", &script]).spawn() {
        eprintln!("restart: cannot start the new copy: {e}");
        return;
    }
    app::quit(mtm);
}

/// The shell command that starts the new copy once this one is gone: the app that `exe` is in, or the
/// program itself when it is not in one.
fn relaunch_script(exe: &std::path::Path, flag: &str) -> String {
    let quote =
        |path: &std::path::Path| format!("'{}'", path.to_string_lossy().replace('\'', "'\\''"));
    match exe
        .ancestors()
        .nth(3)
        .filter(|bundle| bundle.extension().is_some_and(|e| e == "app"))
    {
        Some(bundle) => format!("sleep 0.7; open -n {} --args {flag}", quote(bundle)),
        None => format!("sleep 0.7; exec {} {flag}", quote(exe)),
    }
}

/// The JSON an assistant's configuration needs to start the clipboard server.
pub fn mcp_config(exe: &str) -> String {
    serde_json::to_string_pretty(&json!({
        "mcpServers": { "figghost": { "command": exe, "args": ["--mcp"] } }
    }))
    .unwrap_or_default()
}

pub(crate) fn base64(bytes: &[u8]) -> String {
    const TABLE: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        let n = chunk
            .iter()
            .enumerate()
            .fold(0u32, |n, (i, &b)| n | u32::from(b) << (16 - 8 * i));
        for i in 0..4 {
            if i <= chunk.len() {
                out.push(char::from(TABLE[(n >> (18 - 6 * i) & 63) as usize]));
            } else {
                out.push('=');
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_assistant_configuration_names_the_program_and_the_flag() {
        let text = mcp_config("/Applications/Figghost.app/Contents/MacOS/Figghost");
        let parsed: Value = serde_json::from_str(&text).unwrap();
        let server = &parsed["mcpServers"]["figghost"];
        assert_eq!(
            server["command"],
            "/Applications/Figghost.app/Contents/MacOS/Figghost"
        );
        assert_eq!(server["args"][0], "--mcp");
        // Paths with quotes or backslashes stay valid JSON.
        assert!(serde_json::from_str::<Value>(&mcp_config("/odd \"path\\x")).is_ok());
    }

    #[test]
    fn a_restart_opens_the_app_with_the_flag_it_was_given() {
        use std::path::Path;
        let exe = Path::new("/Applications/Figghost.app/Contents/MacOS/Figghost");
        assert_eq!(
            relaunch_script(exe, "--background"),
            "sleep 0.7; open -n '/Applications/Figghost.app' --args --background"
        );
        assert_eq!(
            relaunch_script(exe, "--show-settings"),
            "sleep 0.7; open -n '/Applications/Figghost.app' --args --show-settings"
        );
        // Outside an app the program itself starts again.
        assert_eq!(
            relaunch_script(Path::new("/x/target/debug/app-ui"), "--show-settings"),
            "sleep 0.7; exec '/x/target/debug/app-ui' --show-settings"
        );
        // A quote in the path cannot end the quoting early.
        assert_eq!(
            relaunch_script(
                Path::new("/tmp/it's/Figghost.app/Contents/MacOS/Figghost"),
                "--background"
            ),
            "sleep 0.7; open -n '/tmp/it'\\''s/Figghost.app' --args --background"
        );
    }

    #[test]
    fn base64_matches_the_standard_vectors() {
        for (input, expected) in [
            ("", ""),
            ("f", "Zg=="),
            ("fo", "Zm8="),
            ("foo", "Zm9v"),
            ("foob", "Zm9vYg=="),
            ("fooba", "Zm9vYmE="),
            ("foobar", "Zm9vYmFy"),
        ] {
            assert_eq!(base64(input.as_bytes()), expected, "{input:?}");
        }
        assert_eq!(base64(&[0xff, 0xfe, 0xfd]), "//79");
    }
}
