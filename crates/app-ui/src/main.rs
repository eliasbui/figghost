#![forbid(unsafe_code)]
//! Composition root: starts each feature and keeps its handles alive.

mod agents;
mod audio;
mod bluetooth;
mod clipboard;
mod config;
mod context;
mod features;
mod focus;
mod focus_timer;
mod history;
mod i18n;
mod live_media;
mod mcp;
mod media;
mod menu;
mod mouse;
mod settings;
mod shelf;
mod shelf_actions;
mod tiling;
mod trace;
mod update;
mod volume_hud;

use agents::AgentsFeature;
use audio::AudioFeature;
use bluetooth::Bluetooth;
use clipboard::Clipboard;
use config::{Config, home_dir};
use context::AppContext;
use features::{AttachEnv, Features};
use focus::FocusFeature;
use live_media::LiveMedia;
use macos_sys::{MainThreadMarker, app, hello, status_item::StatusMenu};
use media::MediaFeature;
use mouse::Mouse;
use settings::SettingsFeature;
use shelf::ShelfFeature;
use std::{cell::RefCell, path::PathBuf, rc::Rc, sync::Arc};
use tiling::Tiling;
use update::UpdateFeature;
use volume_hud::VolumeHud;

fn main() {
    let home = home_dir();
    // The coding agents start the app this way for each hook event and for the status line: no window, no
    // AppKit, and nothing that can get in their way.
    let args: Vec<String> = std::env::args().collect();
    if let Some(at) = args.iter().position(|a| a == "--figghost-hook") {
        std::process::exit(agents::run_hook_cli(
            &home,
            args.get(at + 1).map(String::as_str),
        ));
    }
    if args.iter().any(|a| a == "--figghost-statusline") {
        std::process::exit(agents::run_statusline_cli(&home));
    }
    // An AI assistant starts the app this way to talk to it over standard input and output: no window,
    // no AppKit, and nothing on stdout but the protocol.
    if std::env::args().any(|a| a == "--mcp") {
        std::process::exit(mcp::run(&home));
    }
    let mtm = MainThreadMarker::new().expect("main() must run on the main thread");
    let (config, note) = Config::load(&home);
    if let Some(note) = note {
        eprintln!("config: {note}");
    }
    // The language is fixed before any view or menu is built.
    let language =
        i18n::Language::resolve(&config.language, &macos_sys::locale::preferred_languages());
    i18n::set(language);
    macos_sys::labels::set(language.labels());
    app::run_accessory(mtm, move || Shell::build(mtm, home, config));
}

/// Everything that must stay alive while the app runs.
#[allow(dead_code)]
struct Shell {
    mouse: Mouse,
    clipboard: Option<Rc<Clipboard>>,
    shelf: Option<Rc<ShelfFeature>>,
    media: Option<Rc<MediaFeature>>,
    live_media: Option<Rc<LiveMedia>>,
    bluetooth: Option<Rc<Bluetooth>>,
    focus: Option<FocusFeature>,
    agents: Option<Rc<AgentsFeature>>,
    audio: AudioFeature,
    tiling: Rc<Tiling>,
    volume_hud: Rc<VolumeHud>,
    settings: Rc<SettingsFeature>,
    status: Rc<StatusMenu>,
    reopen: app::ReopenHandler,
}

/// Whether the settings window should open now: always when asked for, never at login or when told to
/// stay in the background, and otherwise as the config says.
fn opens_settings(config: &Config, args: &[String], at_login: bool) -> bool {
    if args.iter().any(|a| a == "--show-settings") {
        return true;
    }
    config.show_settings_on_launch && !at_login && !args.iter().any(|a| a == "--background")
}

/// The ghost of the hello (`macos_sys::hello`), its eyes open and closed, drawn with agy to match the app icon;
/// `docs/images/ghost-hello.gif` shows the moment.
const HELLO_OPEN: &[u8] = include_bytes!("../assets/hello/ghost-open.png");
const HELLO_BLINK: &[u8] = include_bytes!("../assets/hello/ghost-blink.png");
/// The notch is on the screen a moment after the start; the hello waits for it.
const HELLO_DELAY: f64 = 0.5;

/// Whether the ghost says hello now: each time the app is opened by hand, never at login or when told to stay in
/// the background, and only while the notch it comes out of is on.
fn greets(config: &Config, args: &[String], at_login: bool) -> bool {
    config.launch_effect
        && config.notch.enabled
        && !at_login
        && !args.iter().any(|a| a == "--background")
}

impl Shell {
    fn build(mtm: MainThreadMarker, home: PathBuf, config: Config) -> Shell {
        // Only meaningful while the launch event is being handled.
        let at_login = app::launched_at_login();
        let args: Vec<String> = std::env::args().skip(1).collect();
        let show_settings = opens_settings(&config, &args, at_login);
        let greet = greets(&config, &args, at_login);

        let mouse = Mouse::start(config.mouse.clone());
        let clipboard = Clipboard::start(mtm, &home, &config).map(Rc::new);
        let tiling = Rc::new(Tiling::new(mtm));
        for failure in tiling.configure(&config.tiling) {
            eprintln!("tiling: {}", failure.note);
        }
        let notch = clipboard.as_ref().and_then(|c| c.notch.clone());
        if let Some(notch) = &notch {
            notch.configure(&config.notch.style());
            notch.set_enabled(config.notch.enabled);
            if greet {
                // Out of the notch at rest: the cutout, the pill, or the top edge where the pill is hidden.
                let geometry = notch.geometry();
                let rest = geometry.rest_size(config.notch.style().rest);
                let resting = geometry.frame(rest.width, rest.height);
                app::after(HELLO_DELAY, move || {
                    hello::play(mtm, resting, HELLO_OPEN, HELLO_BLINK);
                });
            }
        }
        let shelf = notch
            .as_ref()
            .map(|notch| Rc::new(ShelfFeature::attach(mtm, notch, &home)));
        let media = notch.as_ref().map(|notch| MediaFeature::attach(mtm, notch));
        let live_media = notch
            .as_ref()
            .zip(media.as_ref())
            .map(|(notch, media)| LiveMedia::attach(notch, media, &config.notch));
        let bluetooth = notch.as_ref().map(Bluetooth::attach);
        let focus = notch.as_ref().map(|notch| FocusFeature::attach(mtm, notch));
        let agents = notch
            .as_ref()
            .map(|notch| AgentsFeature::attach(mtm, notch, &home, &config.agents));
        if let Some(agents) = &agents {
            agents.configure_notch(&config.notch);
        }
        let features = Features::attach(&AttachEnv {
            mtm,
            notch: notch.as_ref(),
            home: &home,
            config: &config,
            clipboard: clipboard.as_ref(),
            tiling: &tiling,
            media: media.as_ref(),
            shelf: shelf.as_ref(),
        });
        // The mixer saves what is changed on its notch page, so it shares the one config with the settings.
        let config = Rc::new(RefCell::new(config));
        let audio = AudioFeature::start(mtm, &config, home.clone(), notch.as_deref());
        let volume_hud = Rc::new(VolumeHud::new(
            mtm,
            audio.engine.soft_volume(),
            config.borrow().notch.hud_style(),
        ));
        if config.borrow().volume_hud {
            volume_hud.enable_when_possible();
        }
        {
            let hud = Rc::clone(&volume_hud);
            audio.engine.set_on_soft(move |on| hud.set_soft_keys(on));
        }

        let update = UpdateFeature::new(mtm, home.clone(), Rc::clone(&config));
        let ctx = Rc::new(AppContext {
            mtm,
            home,
            config: Rc::clone(&config),
            control: Arc::clone(&mouse.control),
            clipboard: clipboard.clone(),
            shelf: shelf.clone(),
            agents: agents.clone(),
            live_media: live_media.clone(),
            features,
            volume_hud: Rc::clone(&volume_hud),
            tiling: Rc::clone(&tiling),
            update: Rc::clone(&update),
        });
        let settings = SettingsFeature::new(Rc::clone(&ctx));
        let on_settings = Rc::clone(&settings);
        app::set_settings_handler(move || on_settings.show());
        let on_reopen = Rc::clone(&settings);
        let reopen = app::on_reopen(mtm, move || on_reopen.show());
        let status = Rc::new(menu::build(&ctx, &settings));
        // The three dots of the notch show the menu of the menu bar item.
        if let Some(notch) = ctx.clipboard.as_ref().and_then(|c| c.notch.as_ref()) {
            notch.set_orb_menu(status.menu());
        }
        status.set_visible(ctx.config.borrow().menu_icon);
        {
            let status = Rc::clone(&status);
            settings.on_menu_icon(move |show| status.set_visible(show));
        }
        if show_settings {
            settings.show();
        }
        update.start();
        Shell {
            mouse,
            clipboard,
            shelf,
            media,
            live_media,
            bluetooth,
            focus,
            agents,
            audio,
            tiling,
            volume_hud,
            settings,
            status,
            reopen,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(list: &[&str]) -> Vec<String> {
        list.iter().map(|s| (*s).to_owned()).collect()
    }

    #[test]
    fn settings_open_by_hand_but_not_at_login() {
        let config = Config::default();
        assert!(opens_settings(&config, &args(&[]), false));
        assert!(!opens_settings(&config, &args(&[]), true));
        assert!(!opens_settings(&config, &args(&["--background"]), false));
        // A restart from the settings window asks for the window explicitly, even at login.
        assert!(opens_settings(&config, &args(&["--show-settings"]), true));
        let quiet = Config {
            show_settings_on_launch: false,
            ..Config::default()
        };
        assert!(!opens_settings(&quiet, &args(&[]), false));
        assert!(opens_settings(&quiet, &args(&["--show-settings"]), false));
    }

    #[test]
    fn the_ghost_says_hello_only_when_opened_by_hand() {
        let config = Config::default();
        assert!(greets(&config, &args(&[]), false));
        // A restart from the settings window is opened by hand too.
        assert!(greets(&config, &args(&["--show-settings"]), false));
        assert!(!greets(&config, &args(&[]), true));
        assert!(!greets(&config, &args(&["--background"]), false));
        let mut off = Config::default();
        off.launch_effect = false;
        assert!(!greets(&off, &args(&[]), false));
        let mut no_notch = Config::default();
        no_notch.notch.enabled = false;
        assert!(!greets(&no_notch, &args(&[]), false));
    }
}
