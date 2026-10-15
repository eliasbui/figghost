#![forbid(unsafe_code)]
//! Composition root: starts each feature and keeps its handles alive.

mod audio;
mod clipboard;
mod config;
mod context;
mod history;
mod i18n;
mod media;
mod menu;
mod mouse;
mod settings;
mod shelf;
mod tiling;
mod trace;
mod volume_hud;

use audio::AudioFeature;
use clipboard::Clipboard;
use config::{Config, home_dir};
use context::AppContext;
use macos_sys::{MainThreadMarker, app, status_item::StatusMenu};
use media::MediaFeature;
use mouse::Mouse;
use settings::SettingsFeature;
use shelf::ShelfFeature;
use std::{cell::RefCell, path::PathBuf, rc::Rc, sync::Arc};
use tiling::Tiling;
use volume_hud::VolumeHud;

fn main() {
    let mtm = MainThreadMarker::new().expect("main() must run on the main thread");
    let home = home_dir();
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
    media: Option<MediaFeature>,
    audio: AudioFeature,
    tiling: Rc<Tiling>,
    volume_hud: Rc<VolumeHud>,
    settings: Rc<SettingsFeature>,
    status: StatusMenu,
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

impl Shell {
    fn build(mtm: MainThreadMarker, home: PathBuf, config: Config) -> Shell {
        // Only meaningful while the launch event is being handled.
        let at_login = app::launched_at_login();
        let args: Vec<String> = std::env::args().skip(1).collect();
        let show_settings = opens_settings(&config, &args, at_login);

        let mouse = Mouse::start(config.mouse.clone());
        let clipboard = Clipboard::start(mtm, &home, &config).map(Rc::new);
        let tiling = Rc::new(Tiling::new(mtm));
        for failure in tiling.configure(&config.tiling) {
            eprintln!("tiling: {}", failure.note);
        }
        let notch = clipboard.as_ref().and_then(|c| c.notch.clone());
        if let Some(notch) = &notch {
            notch.set_enabled(config.notch.enabled);
        }
        let shelf = notch
            .as_ref()
            .map(|notch| Rc::new(ShelfFeature::attach(mtm, notch, &home)));
        let media = notch.as_ref().map(|notch| MediaFeature::attach(mtm, notch));
        let config = Rc::new(RefCell::new(config));
        let audio = AudioFeature::start(mtm, &config, home.clone(), notch.as_deref());
        let volume_hud = Rc::new(VolumeHud::new(mtm, audio.engine.soft_volume()));
        if config.borrow().volume_hud {
            volume_hud.enable_when_possible();
        }
        {
            let hud = Rc::clone(&volume_hud);
            audio.engine.set_on_soft(move |on| hud.set_soft_keys(on));
        }

        let ctx = Rc::new(AppContext {
            mtm,
            home,
            config: Rc::clone(&config),
            control: Arc::clone(&mouse.control),
            clipboard: clipboard.clone(),
            shelf: shelf.clone(),
            volume_hud: Rc::clone(&volume_hud),
            tiling: Rc::clone(&tiling),
        });
        let settings = SettingsFeature::new(Rc::clone(&ctx));
        let on_settings = Rc::clone(&settings);
        app::set_settings_handler(move || on_settings.show());
        let on_reopen = Rc::clone(&settings);
        let reopen = app::on_reopen(mtm, move || on_reopen.show());
        let status = menu::build(&ctx, &settings);
        if show_settings {
            settings.show();
        }
        Shell {
            mouse,
            clipboard,
            shelf,
            media,
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
}
