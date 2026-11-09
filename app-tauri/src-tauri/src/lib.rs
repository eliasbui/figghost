//! Figghost on Tauri 2: the backend of the settings window. The page (src/) speaks to it with one command and one event, the
//! way the page of the current app speaks to its AppKit host: it sends a message (`fig_send`) and is told the state
//! (`fig:recv`). What a setting is, and what it does to the machine, stays on this side.
mod actions;
mod glass;
mod messages;
mod native;
mod rules;
mod store;
mod windows;

use glass::Glass;
use messages::Message;
use serde_json::{json, Value};
use std::{
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicBool, Ordering},
        Mutex, MutexGuard,
    },
    thread,
    time::Duration,
};
use store::{Effect, Fault, Store};
use tauri::{
    image::Image,
    menu::{Menu, MenuItem, PredefinedMenuItem},
    tray::TrayIconBuilder,
    AppHandle, Emitter, Manager,
};

/// The event the page listens to (src/bridge/tauri.ts); the command it calls is `fig_send`.
const EVENT: &str = "fig:recv";
/// The id of the menu bar item.
const TRAY: &str = "main";
/// Set by a restart so that the new process shows the settings again, as `--show-settings` does in the current app.
pub const SHOW_AGAIN: &str = "FIGGHOST_TAURI_SHOW_SETTINGS";
/// How often an open window is told the state again, to follow the permissions and the facts around the settings.
const REFRESH: Duration = Duration::from_secs(2);

/// A lock that a panic elsewhere has not made unusable: what these guard is plain data.
fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

/// The state of the app that the commands share.
pub struct Backend {
    store: Mutex<Store>,
    glass: Mutex<Glass>,
    /// The last state sent, so that a window is told only what changed.
    last: Mutex<String>,
    /// A page asked for before the window's script was ready to take it.
    pending_page: Mutex<Option<String>>,
    refreshing: AtomicBool,
    dir: PathBuf,
    bundled: bool,
    system_language: String,
}

impl Backend {
    fn new(dir: PathBuf) -> Self {
        let (store, note) = Store::open(&dir);
        if let Some(note) = note {
            eprintln!("settings: {note}");
        }
        Self {
            store: Mutex::new(store),
            glass: Mutex::new(Glass::None),
            last: Mutex::new(String::new()),
            pending_page: Mutex::new(None),
            refreshing: AtomicBool::new(false),
            dir,
            bundled: native::bundled(),
            system_language: native::system_language(),
        }
    }

    pub fn dir(&self) -> &Path {
        &self.dir
    }

    pub fn set_glass(&self, glass: Glass) {
        *lock(&self.glass) = glass;
    }

    pub fn theme(&self) -> String {
        lock(&self.store).theme().to_owned()
    }

    fn language(&self) -> &'static str {
        lock(&self.store).language(&self.system_language)
    }

    fn flag(&self, path: &str) -> bool {
        rules::get(lock(&self.store).state(), path)
            .and_then(Value::as_bool)
            .unwrap_or(false)
    }

    /// The state with the facts around the settings put in: the version, the permissions, the language the app runs in.
    fn snapshot(&self) -> Value {
        let (mut state, language) = {
            let store = lock(&self.store);
            (store.state().clone(), store.language(&self.system_language))
        };
        let glass = *lock(&self.glass) == Glass::Liquid;
        if let Some(sections) = state.as_object_mut() {
            sections.insert(
                "app".to_owned(),
                // What the app uses is the engines' to measure; the page says it is waiting for the first reading.
                json!({ "version": env!("CARGO_PKG_VERSION"), "bundled": self.bundled, "config_dir": self.dir.to_string_lossy(), "usage": null }),
            );
            sections.insert(
                "env".to_owned(),
                json!({
                    "accessibility": native::accessibility_trusted(),
                    // The login item and the display preferences wait for the host that reads them from the system.
                    "login": "unavailable",
                    "language": language,
                    "reduce_transparency": false,
                    "reduce_motion": false,
                    "increase_contrast": false,
                    "glass": glass,
                }),
            );
        }
        state
    }

    fn emit(&self, app: &AppHandle, message: Value) {
        if let Err(error) = app.emit(EVENT, message) {
            eprintln!("settings: cannot tell the windows ({error})");
        }
    }

    /// Tells the windows the state; `force` tells it even when it has not changed since the last time.
    pub fn push(&self, app: &AppHandle, force: bool) {
        let state = self.snapshot();
        let text = state.to_string();
        {
            let mut last = lock(&self.last);
            if !force && *last == text {
                return;
            }
            *last = text;
        }
        self.emit(app, json!({ "t": "state", "state": state }));
    }

    /// Tells the open windows the state every few seconds, until none is open.
    pub fn keep_refreshing(&self, app: &AppHandle) {
        if self.refreshing.swap(true, Ordering::SeqCst) {
            return;
        }
        let handle = app.clone();
        thread::spawn(move || loop {
            thread::sleep(REFRESH);
            let backend = handle.state::<Backend>();
            if handle.webview_windows().is_empty() {
                backend.refreshing.store(false, Ordering::SeqCst);
                // A window opened between the check and the store found the flag still set and started nothing: take over.
                if handle.webview_windows().is_empty()
                    || backend.refreshing.swap(true, Ordering::SeqCst)
                {
                    break;
                }
                continue;
            }
            backend.push(&handle, false);
        });
    }

    fn fault(&self, app: &AppHandle, fault: &Fault) {
        eprintln!("settings: refused: {} ({})", fault.code, fault.detail);
        self.emit(app, fault.to_json());
    }

    /// Opens the settings on a page.
    pub fn show_page(&self, app: &AppHandle, id: &str) {
        let was_open = app.get_webview_window(windows::SETTINGS).is_some();
        *lock(&self.pending_page) = Some(id.to_owned());
        windows::open(app, windows::SETTINGS);
        // A window that was open takes the page at once; a new one takes it when it says it is ready.
        if was_open {
            self.go_pending(app);
        }
    }

    fn go_pending(&self, app: &AppHandle) {
        let page = lock(&self.pending_page).take();
        if let Some(page) = page {
            self.emit(app, json!({ "t": "go", "page": page }));
        }
    }

    /// Applies what the page sent.
    pub fn handle(&self, app: &AppHandle, message: Message) {
        match &message {
            Message::Ready => {
                self.push(app, true);
                self.emit(app, json!({ "t": "update", "state": "idle" }));
                self.go_pending(app);
            }
            Message::JsError { message, line } => {
                eprintln!("settings page: {message} (line {line})")
            }
            Message::Page { .. } | Message::Recording { .. } => {}
            Message::Action { name } if name != "reset_tiling" => {
                match actions::run(app, self, name) {
                    Ok(true) => {}
                    Ok(false) => self.fault(app, &Fault::new("unsupported", name.as_str())),
                    Err(error) => self.fault(app, &Fault::new("action_failed", error)),
                }
                self.push(app, true);
            }
            // Choosing an app from the disk and listing the open ones need AppKit; they come with the native adapter.
            Message::PickApp { .. } => self.fault(app, &Fault::new("unsupported", "pick_app")),
            Message::Running { purpose } => self.emit(
                app,
                json!({ "t": "running", "purpose": purpose.as_str(), "apps": [] }),
            ),
            Message::Icons { .. } => self.emit(app, json!({ "t": "icons", "icons": {} })),
            edit => self.edit(app, edit),
        }
    }

    /// A change of a setting: kept and saved, or refused and reported; either way the page is told the state.
    fn edit(&self, app: &AppHandle, message: &Message) {
        let result = {
            let mut store = lock(&self.store);
            store.handle(message).map(|effect| {
                if let Err(error) = store.save() {
                    eprintln!("settings: cannot save: {error}");
                }
                effect
            })
        };
        match result {
            Ok(Effect::Theme) => windows::apply_theme(app),
            Ok(Effect::MenuIcon(on)) => set_menu_icon(app, on),
            Ok(Effect::LoginItem(_)) => self.fault(
                app,
                &Fault::new("login_failed", "not available in this version"),
            ),
            Ok(Effect::Language | Effect::None) => {}
            Err(fault) => self.fault(app, &fault),
        }
        self.push(app, true);
    }
}

/// What the page calls. The message is read here, so that one the app does not know is a line in the log and not a crash.
#[tauri::command]
fn fig_send(app: AppHandle, backend: tauri::State<'_, Backend>, message: Value) {
    match serde_json::from_value::<Message>(message) {
        Ok(message) => backend.handle(&app, message),
        Err(error) => eprintln!("settings: unreadable message ({error})"),
    }
}

fn set_menu_icon(app: &AppHandle, on: bool) {
    if let Some(tray) = app.tray_by_id(TRAY) {
        let _ = tray.set_visible(on);
    }
}

/// The palette, or the page that turns it on when it is off.
fn open_palette(app: &AppHandle) {
    let backend = app.state::<Backend>();
    if backend.flag("palette.enabled") {
        windows::toggle(app, windows::PALETTE);
    } else {
        backend.show_page(app, "palette");
    }
}

/// The menu bar item, a ghost with a menu: the settings, the palette and quitting. The words are those of the current app.
fn build_tray(app: &tauri::App, language: &str, visible: bool) -> tauri::Result<()> {
    let (settings_text, palette_text, quit_text) = if language == "vi" {
        (
            "C\u{e0}i \u{111}\u{1eb7}t...",
            "B\u{1ea3}ng l\u{1ec7}nh",
            "Tho\u{e1}t",
        )
    } else {
        ("Settings...", "Command palette", "Quit")
    };
    let settings = MenuItem::with_id(app, "settings", settings_text, true, None::<&str>)?;
    let palette = MenuItem::with_id(app, "palette", palette_text, true, None::<&str>)?;
    let separator = PredefinedMenuItem::separator(app)?;
    let quit = MenuItem::with_id(app, "quit", quit_text, true, None::<&str>)?;
    let menu = Menu::with_items(app, &[&settings, &palette, &separator, &quit])?;
    let mut tray = TrayIconBuilder::with_id(TRAY)
        .menu(&menu)
        .tooltip("Figghost")
        .icon_as_template(true)
        .on_menu_event(|app, event| match event.id.as_ref() {
            "settings" => windows::open(app, windows::SETTINGS),
            "palette" => open_palette(app),
            "quit" => app.exit(0),
            _ => {}
        });
    // The ghost (tools/make-tray-icon.mjs) is a template image, which the menu bar paints for a light or a dark bar; the
    // app's own icon is the way out if the file cannot be read.
    match Image::from_bytes(include_bytes!("../icons/tray.png")) {
        Ok(ghost) => tray = tray.icon(ghost),
        Err(error) => {
            eprintln!("tray: the icon cannot be read ({error})");
            if let Some(icon) = app.default_window_icon() {
                tray = tray.icon(icon.clone()).icon_as_template(false);
            }
        }
    }
    let tray = tray.build(app)?;
    if !visible {
        tray.set_visible(false)?;
    }
    Ok(())
}

pub fn run() {
    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![fig_send])
        .setup(|app| {
            // FIGGHOST_TAURI_HOME names another folder for the data, for a second profile or a test.
            let dir = match std::env::var_os("FIGGHOST_TAURI_HOME") {
                Some(home) => PathBuf::from(home),
                None => app.path().app_data_dir()?,
            };
            let backend = Backend::new(dir);
            let again = std::env::var_os(SHOW_AGAIN).is_some();
            std::env::remove_var(SHOW_AGAIN);
            let (language, show, icon) = (
                backend.language(),
                again || backend.flag("general.show_on_launch"),
                backend.flag("general.menu_icon"),
            );
            app.manage(backend);
            // Until a window is open the app is in the menu bar alone.
            windows::policy(app.handle(), false);
            build_tray(app, language, icon)?;
            if show {
                windows::open(app.handle(), windows::SETTINGS);
            }
            Ok(())
        })
        .build(tauri::generate_context!())
        .expect("error while building the app")
        .run(|app, event| {
            match event {
                // The last window closing does not end the app: the menu bar item stays. A quit has a code.
                tauri::RunEvent::ExitRequested { code, api, .. } => {
                    if code.is_none() {
                        api.prevent_exit();
                    }
                }
                // The Dock icon or Finder opened the app again.
                #[cfg(target_os = "macos")]
                tauri::RunEvent::Reopen {
                    has_visible_windows,
                    ..
                } => {
                    if !has_visible_windows {
                        windows::open(app, windows::SETTINGS);
                    }
                }
                _ => {}
            }
            #[cfg(not(target_os = "macos"))]
            let _ = app;
        });
}
