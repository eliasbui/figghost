//! Screen capture: a region, a window or a whole display from global shortcuts and the menu bar, the
//! picture on the clipboard and in a folder, optionally in the editor, and text read from a region.
//!
//! The steps of a capture are in `flow` and run against a small `Backend`, so that they are tested with a
//! fake that returns in-memory pictures. `system` is the real one, on top of `macos_sys::capture`.

use super::AttachEnv;
use crate::{config::parse_hotkey, i18n, settings::model::Fault};
use core_capture::naming::{default_folder, validate_folder};
use macos_sys::{
    MainThreadMarker,
    capture::{self, folder, grab},
    hotkey::Hotkey,
    status_item::{Entry, Item},
};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::{
    cell::{Cell, RefCell},
    collections::BTreeMap,
    path::PathBuf,
    rc::{Rc, Weak},
};

pub mod flow;
pub mod settings;
mod strings;
mod system;

/// What a shortcut or a menu entry captures.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    Region,
    Window,
    Screen,
    /// The last region again, without the overlay.
    Repeat,
    /// A region, read as text that goes to the clipboard.
    Text,
}

impl Mode {
    pub const ALL: [Mode; 5] = [
        Mode::Region,
        Mode::Window,
        Mode::Screen,
        Mode::Repeat,
        Mode::Text,
    ];

    /// The name in the config file, in the page's shortcut targets ("capture:region") and in its state.
    pub const fn key(self) -> &'static str {
        match self {
            Mode::Region => "region",
            Mode::Window => "window",
            Mode::Screen => "screen",
            Mode::Repeat => "repeat",
            Mode::Text => "text",
        }
    }

    pub fn from_key(key: &str) -> Option<Mode> {
        Mode::ALL.into_iter().find(|m| m.key() == key)
    }

    /// Control, Option and Command with 3 to 7: next to the system's own Command+Shift+3 to 5, not on them.
    pub const fn default_shortcut(self) -> &'static str {
        match self {
            Mode::Screen => "ctrl+opt+cmd+3",
            Mode::Region => "ctrl+opt+cmd+4",
            Mode::Window => "ctrl+opt+cmd+5",
            Mode::Repeat => "ctrl+opt+cmd+6",
            Mode::Text => "ctrl+opt+cmd+7",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    pub enabled: bool,
    /// Put the picture on the clipboard.
    pub copy: bool,
    /// Save the picture to a file in `folder`.
    pub save: bool,
    /// Where pictures are saved; empty means Pictures/Figghost.
    pub folder: String,
    /// Open the editor on the picture.
    pub edit: bool,
    /// Keep the window's shadow in a window capture.
    pub window_shadow: bool,
    /// Shortcuts that differ from the defaults; an empty one means the mode has no shortcut.
    pub shortcuts: BTreeMap<String, String>,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            enabled: false,
            copy: true,
            save: true,
            folder: String::new(),
            edit: false,
            window_shadow: true,
            shortcuts: BTreeMap::new(),
        }
    }
}

/// The home folder that Pictures is looked up in. A private instance (`FIGGHOST_HOME`) keeps its pictures
/// inside its own folder, so that a test never writes into the real Pictures.
fn pictures_home() -> PathBuf {
    std::env::var_os("FIGGHOST_HOME")
        .or_else(|| std::env::var_os("HOME"))
        .map(PathBuf::from)
        .unwrap_or_default()
}

impl Config {
    pub fn shortcut(&self, mode: Mode) -> String {
        self.shortcuts
            .get(mode.key())
            .cloned()
            .unwrap_or_else(|| mode.default_shortcut().to_owned())
    }

    /// The folder pictures are saved in. A value from a hand-edited config file that is not a plain absolute
    /// path falls back to the default instead of being used.
    pub fn save_dir(&self) -> PathBuf {
        if self.folder.trim().is_empty() {
            return default_folder(&pictures_home());
        }
        validate_folder(&self.folder).unwrap_or_else(|reason| {
            eprintln!("capture: the folder is not usable ({reason}); using the default");
            default_folder(&pictures_home())
        })
    }
}

/// The running feature. Created by `attach`, kept alive by the app context.
pub struct Feature {
    me: Weak<Feature>,
    mtm: MainThreadMarker,
    flow: Rc<flow::Flow<system::System>>,
    /// A copy of the config, kept up to date by `configure` and the actions, for the shortcuts to use.
    config: RefCell<Config>,
    enabled: Cell<bool>,
    hotkeys: RefCell<Vec<Hotkey>>,
}

/// Starts the feature. It needs no notch, so it always starts; nothing runs until a shortcut is pressed.
pub fn attach(env: &AttachEnv) -> Option<Rc<Feature>> {
    let feature = Rc::new_cyclic(|me| Feature {
        me: me.clone(),
        mtm: env.mtm,
        flow: Rc::new(flow::Flow::new(system::System::new(env.mtm))),
        config: RefCell::new(env.config.capture.clone()),
        enabled: Cell::new(false),
        hotkeys: RefCell::default(),
    });
    if let Err(fault) = feature.configure(&env.config.capture) {
        eprintln!("capture: {}: {}", fault.code, fault.detail);
    }
    Some(feature)
}

impl Feature {
    /// Applies the settings after a change. An error is shown on the settings page and the change is undone.
    pub fn configure(&self, config: &Config) -> Result<(), Fault> {
        *self.config.borrow_mut() = config.clone();
        self.enabled.set(config.enabled);
        self.register(config)
    }

    /// Registers a global hotkey per mode, as many as can be; the first that is taken is reported.
    fn register(&self, config: &Config) -> Result<(), Fault> {
        let mut keys = self.hotkeys.borrow_mut();
        keys.clear();
        if !config.enabled {
            return Ok(());
        }
        let mut taken = None;
        for mode in Mode::ALL {
            let shortcut = config.shortcut(mode);
            if shortcut.is_empty() {
                continue;
            }
            let spec = match parse_hotkey(&shortcut) {
                Ok(spec) => spec,
                Err(reason) => {
                    eprintln!("capture: shortcut for {}: {reason}", mode.key());
                    continue;
                }
            };
            let me = self.me.clone();
            let registered = Hotkey::register(self.mtm, spec.key_code, spec.modifiers, move || {
                if let Some(feature) = me.upgrade() {
                    feature.run(mode);
                }
            });
            match registered {
                Ok(key) => keys.push(key),
                Err(status) => {
                    taken.get_or_insert_with(|| {
                        Fault::new("taken", format!("capture:{} (status {status})", mode.key()))
                    });
                }
            }
        }
        taken.map_or(Ok(()), Err)
    }

    fn run(&self, mode: Mode) {
        if self.enabled.get() {
            let config = self.config.borrow().clone();
            self.flow.run(mode, &config);
        }
    }

    /// Releases the global shortcuts while the settings page records a new one.
    pub fn suspend_hotkeys(&self) {
        self.hotkeys.borrow_mut().clear();
    }

    /// Registers the global shortcuts again from the config.
    pub fn resume_hotkeys(&self, config: &Config) {
        if let Err(fault) = self.register(config) {
            eprintln!("capture: cannot register again ({})", fault.detail);
        }
    }

    /// Facts only the running app knows: the Screen Recording permission and whether this system can capture.
    pub fn runtime(&self) -> Value {
        json!({ "permission": capture::permission_granted(), "available": grab::available() })
    }

    /// A button of the settings page. `None` when the name is not this feature's.
    pub fn action(&self, name: &str, config: &mut Config) -> Option<Result<(), Fault>> {
        match name {
            "capture_choose_folder" => Some(self.choose_folder(config)),
            "capture_permission" => {
                if !capture::request_permission() {
                    capture::open_permission_settings();
                }
                Some(Ok(()))
            }
            _ => None,
        }
    }

    fn choose_folder(&self, config: &mut Config) -> Result<(), Fault> {
        let text = i18n::current();
        let Some(chosen) = folder::choose(
            self.mtm,
            text.capture_folder_prompt(),
            text.capture_folder_button(),
            &config.save_dir(),
        ) else {
            return Ok(());
        };
        let clean = validate_folder(&chosen.to_string_lossy())
            .map_err(|reason| Fault::new("folder_invalid", reason))?;
        config.folder = clean.to_string_lossy().into_owned();
        self.config.borrow_mut().folder.clone_from(&config.folder);
        Ok(())
    }

    /// Entries for the menu bar item, between "Open clipboard" and the login item. They show only while the
    /// feature is on.
    pub fn menu_entries(this: &Rc<Self>) -> Vec<Entry> {
        let text = i18n::current();
        let entry = |title: &str, mode: Mode| {
            let (run, shown) = (Rc::clone(this), Rc::clone(this));
            Entry::Item(
                Item::new(title, move || run.run(mode)).visible_when(move || shown.enabled.get()),
            )
        };
        vec![
            entry(text.capture_menu_region(), Mode::Region),
            entry(text.capture_menu_window(), Mode::Window),
            entry(text.capture_menu_screen(), Mode::Screen),
            entry(text.capture_menu_text(), Mode::Text),
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_defaults_are_off_and_keep_the_picture_in_two_places() {
        let config = Config::default();
        assert!(!config.enabled && config.copy && config.save && !config.edit);
        assert!(config.window_shadow);
    }

    #[test]
    fn the_default_shortcuts_parse_and_differ_from_the_systems() {
        let mut seen = Vec::new();
        for mode in Mode::ALL {
            let spec = parse_hotkey(mode.default_shortcut()).expect("a valid shortcut");
            assert!(!seen.contains(&spec), "{} repeats a shortcut", mode.key());
            seen.push(spec);
            // The system's own are Command+Shift plus 3 to 5; ours always carry Control and Option as well.
            assert!(mode.default_shortcut().starts_with("ctrl+opt+cmd+"));
        }
    }

    #[test]
    fn a_shortcut_comes_from_the_config_or_the_default() {
        let mut config = Config::default();
        assert_eq!(config.shortcut(Mode::Region), "ctrl+opt+cmd+4");
        config.shortcuts.insert("region".into(), "f13".into());
        assert_eq!(config.shortcut(Mode::Region), "f13");
        config.shortcuts.insert("text".into(), String::new());
        assert_eq!(config.shortcut(Mode::Text), "");
    }

    #[test]
    fn modes_are_found_by_their_key() {
        for mode in Mode::ALL {
            assert_eq!(Mode::from_key(mode.key()), Some(mode));
        }
        assert_eq!(Mode::from_key("nonsense"), None);
    }

    #[test]
    fn an_old_or_partial_config_file_loads_with_defaults() {
        let config: Config = serde_json::from_str(r#"{"enabled": true}"#).unwrap();
        assert!(config.enabled && config.copy && config.save);
        let empty: Config = serde_json::from_str("{}").unwrap();
        assert_eq!(empty, Config::default());
    }

    #[test]
    fn a_bad_folder_in_the_file_falls_back_to_the_default() {
        let config = Config {
            folder: "../../etc".into(),
            ..Config::default()
        };
        assert_eq!(config.save_dir(), Config::default().save_dir());
        let good = Config {
            folder: "/Users/me/Shots/".into(),
            ..Config::default()
        };
        assert_eq!(good.save_dir(), PathBuf::from("/Users/me/Shots"));
    }
}
