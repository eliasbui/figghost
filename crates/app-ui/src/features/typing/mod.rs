//! Vietnamese typing: Telex or VNI in every app, with the engine of Gõ Nhanh. Off by default.
//!
//! The keyboard side (`macos_sys::typing`) holds each key press that ends in a mark and types the word again with it;
//! this folder holds the settings, the shortcut that switches between Vietnamese and English (seen on the keyboard's
//! thread, so that the system's own use of the same keys never gets it first), the entry in the menu bar panel and
//! the notice on the notch. Another Vietnamese input method at work would put every mark twice, so while one runs
//! every key is left alone and the settings page says why.

use super::{AttachEnv, islands};
use crate::{config::parse_hotkey, i18n, menu::symbols, settings::model::Fault};
use core_typing::{Chord, Options};
use macos_sys::{
    app,
    hotkey::modifier,
    notch::{Activity, Detail, Ear, Notch},
    permissions,
    status_item::{Entry, Item},
    style::Tone,
    typing::{self, Keyboard},
    workspace,
};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::{
    cell::{Cell, RefCell},
    collections::HashMap,
    rc::{Rc, Weak},
};

pub mod settings;
mod text;

pub const DEFAULT_SHORTCUT: &str = "ctrl+space";
/// The most apps the list of apps where typing is left alone may hold.
pub const MAX_APPS: usize = 100;
/// The longest bundle identifier the list takes.
const MAX_ID_LEN: usize = 200;
/// How often the keyboard is tried again while Accessibility is missing.
const RETRY_SECONDS: f64 = 3.0;
const SLOT: &str = "typing";
const PRIORITY: u8 = 21;
const SHOW_SECONDS: f64 = 1.5;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Method {
    #[default]
    Telex,
    Vni,
}

impl Method {
    pub const ALL: [Method; 2] = [Method::Telex, Method::Vni];

    pub const fn as_str(self) -> &'static str {
        match self {
            Method::Telex => "telex",
            Method::Vni => "vni",
        }
    }

    pub fn parse(text: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|m| m.as_str() == text)
    }

    /// Its name as people write it.
    pub const fn label(self) -> &'static str {
        match self {
            Method::Telex => "Telex",
            Method::Vni => "VNI",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    /// Vietnamese typing. Off by default.
    pub enabled: bool,
    pub method: Method,
    /// `hoà`, `khoẻ` rather than `hòa`, `khỏe`.
    pub modern_tone: bool,
    /// Gives back an English word the marks would have spoilt, when the word ends.
    pub english_restore: bool,
    /// Switches between Vietnamese and English; empty for none.
    pub shortcut: String,
    /// Bundle identifiers of the apps where every key is left alone.
    pub apps: Vec<String>,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            enabled: false,
            method: Method::Telex,
            modern_tone: true,
            english_restore: true,
            shortcut: DEFAULT_SHORTCUT.into(),
            apps: Vec::new(),
        }
    }
}

/// A bundle identifier as the list takes it: letters, digits, dots, hyphens and underscores, and not too long.
pub fn is_bundle_id(id: &str) -> bool {
    !id.is_empty()
        && id.len() <= MAX_ID_LEN
        && id
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | '_'))
}

/// The key and modifiers of a shortcut such as "ctrl+space"; `None` for none or for one that does not parse.
pub fn chord_of(shortcut: &str) -> Option<Chord> {
    let spec = parse_hotkey(shortcut).ok()?;
    Some(Chord {
        code: u16::try_from(spec.key_code).ok()?,
        shift: spec.modifiers & modifier::SHIFT != 0,
        control: spec.modifiers & modifier::CONTROL != 0,
        option: spec.modifiers & modifier::OPTION != 0,
        command: spec.modifiers & modifier::COMMAND != 0,
    })
}

impl Config {
    /// A config file can say anything; whatever is unusable is replaced by the default or left out.
    pub fn sanitized(mut self) -> Self {
        if !self.shortcut.is_empty() && parse_hotkey(&self.shortcut).is_err() {
            self.shortcut = DEFAULT_SHORTCUT.into();
        }
        let mut kept: Vec<String> = Vec::new();
        for id in self.apps {
            if is_bundle_id(&id) && !kept.contains(&id) && kept.len() < MAX_APPS {
                kept.push(id);
            }
        }
        self.apps = kept;
        self
    }

    /// What the keyboard side needs.
    pub fn keyboard(&self) -> core_typing::Settings {
        core_typing::Settings {
            options: Options {
                method: match self.method {
                    Method::Telex => core_typing::Method::Telex,
                    Method::Vni => core_typing::Method::Vni,
                },
                modern_tone: self.modern_tone,
                english_restore: self.english_restore,
            },
            toggle: chord_of(&self.shortcut),
            excluded: self.apps.clone(),
        }
    }
}

thread_local! {
    /// The running feature, for the shortcut: it is seen on the keyboard's thread and handled on this one.
    static LIVE: RefCell<Weak<Feature>> = const { RefCell::new(Weak::new()) };
}

/// Runs on the keyboard's thread, inside the input path: everything else happens on the main thread.
fn on_toggle(vietnamese: bool) {
    app::on_main(move || {
        LIVE.with(|live| {
            if let Some(feature) = live.borrow().upgrade() {
                feature.announce(vietnamese);
            }
        });
    });
}

/// The running feature. Created by `attach`, kept alive by the app context.
pub struct Feature {
    notch: Option<Weak<Notch>>,
    config: RefCell<Config>,
    keyboard: RefCell<Option<Keyboard>>,
    retry: RefCell<Option<app::Timer>>,
    front: RefCell<Option<workspace::Observer>>,
    /// The settings page is recording a shortcut.
    recording: Cell<bool>,
    /// The name of another Vietnamese input method that is running.
    other: RefCell<Option<String>>,
    names: RefCell<HashMap<String, String>>,
    me: Weak<Feature>,
}

/// Starts the feature; it runs only while switched on.
pub fn attach(env: &AttachEnv) -> Option<Rc<Feature>> {
    let feature = Rc::new_cyclic(|me| Feature {
        notch: env.notch.map(Rc::downgrade),
        config: RefCell::new(Config::default()),
        keyboard: RefCell::default(),
        retry: RefCell::default(),
        front: RefCell::default(),
        recording: Cell::new(false),
        other: RefCell::default(),
        names: RefCell::default(),
        me: me.clone(),
    });
    LIVE.with(|live| *live.borrow_mut() = Rc::downgrade(&feature));
    feature.apply(&env.config.typing.clone().sanitized());
    Some(feature)
}

impl Feature {
    fn apply(&self, config: &Config) {
        *self.config.borrow_mut() = config.clone();
        if !config.enabled {
            self.stop();
            return;
        }
        *self.other.borrow_mut() = typing::other_input_method();
        if let Some(keyboard) = &*self.keyboard.borrow() {
            keyboard.configure(config.keyboard());
            keyboard.set_paused(self.paused());
            return;
        }
        self.start();
    }

    fn paused(&self) -> bool {
        self.recording.get() || self.other.borrow().is_some()
    }

    fn start(&self) {
        if self.keyboard.borrow().is_some() || !self.config.borrow().enabled {
            return;
        }
        let settings = self.config.borrow().keyboard();
        let frontmost = workspace::frontmost_app().map(|a| a.bundle_id);
        match Keyboard::start(settings, true, frontmost, on_toggle) {
            Ok(keyboard) => {
                keyboard.set_paused(self.paused());
                *self.keyboard.borrow_mut() = Some(keyboard);
                self.retry.borrow_mut().take();
                self.watch_front();
            }
            Err(e) => {
                if self.retry.borrow().is_some() {
                    return;
                }
                eprintln!(
                    "typing: no keyboard tap yet ({e:?}); trying again until Accessibility is allowed"
                );
                let me = self.me.clone();
                *self.retry.borrow_mut() = Some(app::every(RETRY_SECONDS, move || {
                    if let Some(feature) = me.upgrade() {
                        feature.start();
                    }
                }));
            }
        }
    }

    fn stop(&self) {
        self.retry.borrow_mut().take();
        self.front.borrow_mut().take();
        self.keyboard.borrow_mut().take();
    }

    fn watch_front(&self) {
        let me = self.me.clone();
        *self.front.borrow_mut() = Some(workspace::observe_frontmost(move |front| {
            let Some(feature) = me.upgrade() else {
                return;
            };
            *feature.other.borrow_mut() = typing::other_input_method();
            if let Some(keyboard) = &*feature.keyboard.borrow() {
                keyboard.app_changed(Some(front.bundle_id));
                keyboard.set_paused(feature.paused());
            }
        }));
    }

    /// Applies the settings after a change.
    pub fn configure(&self, config: &Config) -> Result<(), Fault> {
        self.apply(&config.clone().sanitized());
        Ok(())
    }

    /// While the settings page records a shortcut, every key reaches it as it is, the switch's own included.
    pub fn suspend_hotkeys(&self) {
        self.recording.set(true);
        if let Some(keyboard) = &*self.keyboard.borrow() {
            keyboard.set_paused(true);
        }
    }

    pub fn resume_hotkeys(&self, _config: &Config) {
        self.recording.set(false);
        if let Some(keyboard) = &*self.keyboard.borrow() {
            keyboard.set_paused(self.paused());
        }
    }

    pub fn is_running(&self) -> bool {
        self.keyboard.borrow().is_some()
    }

    pub fn is_vietnamese(&self) -> bool {
        self.keyboard
            .borrow()
            .as_ref()
            .is_some_and(Keyboard::is_vietnamese)
    }

    /// Switches between Vietnamese and English, as the shortcut does.
    pub fn toggle(&self) {
        let now = {
            let keyboard = self.keyboard.borrow();
            let Some(keyboard) = keyboard.as_ref() else {
                return;
            };
            let now = !keyboard.is_vietnamese();
            keyboard.set_vietnamese(now);
            now
        };
        self.announce(now);
    }

    /// Says on the notch which language typing is in now.
    fn announce(&self, vietnamese: bool) {
        let Some(notch) = self.notch.as_ref().and_then(Weak::upgrade) else {
            return;
        };
        let language = i18n::current();
        let tone = if vietnamese { Tone::Green } else { Tone::Gray };
        let method = self.config.borrow().method.label();
        islands::show_for(
            &notch,
            SLOT,
            Activity {
                left: Ear::Symbol {
                    name: "keyboard",
                    tone,
                    pulse: false,
                },
                right: Ear::Text {
                    text: if vietnamese { "VI" } else { "EN" }.into(),
                    tone,
                },
                detail: Some(Detail {
                    title: language.typing_now(vietnamese).to_owned(),
                    subtitle: language.typing_now_detail(vietnamese, method),
                }),
                page: None,
                priority: PRIORITY,
            },
            Some(SHOW_SECONDS),
        );
    }

    fn name_of(&self, id: &str) -> String {
        if let Some(name) = self.names.borrow().get(id) {
            return name.clone();
        }
        let name = workspace::app_name(id).unwrap_or_else(|| id.to_owned());
        self.names.borrow_mut().insert(id.to_owned(), name.clone());
        name
    }

    /// Facts only the running app knows: whether the keyboard runs, the language, the permission, another input
    /// method, and the names of the listed apps.
    pub fn runtime(&self) -> Value {
        let apps: Vec<Value> = self
            .config
            .borrow()
            .apps
            .iter()
            .map(|id| json!({ "id": id, "name": self.name_of(id) }))
            .collect();
        json!({
            "running": self.is_running(),
            "vietnamese": self.is_vietnamese(),
            "accessibility": permissions::accessibility_trusted(false),
            "other": *self.other.borrow(),
            "apps": apps,
        })
    }

    /// The buttons of the page: the switch, and adding or removing an app of the list (the bundle identifier is
    /// part of the name, as an action carries nothing else).
    pub fn action(&self, name: &str, config: &mut Config) -> Option<Result<(), Fault>> {
        if name == "typing_toggle" {
            self.toggle();
            return Some(Ok(()));
        }
        let result = settings::apply_action(config, name)?;
        if result.is_ok() {
            self.apply(&config.clone().sanitized());
        }
        Some(result)
    }

    /// The entry of the menu bar panel: ticked while typing is in Vietnamese.
    pub fn menu_entries(this: &Rc<Self>) -> Vec<Entry> {
        let (act, mark, shown) = (this.me.clone(), this.me.clone(), this.me.clone());
        vec![Entry::Item(
            Item::new(i18n::current().typing_menu(), move || {
                if let Some(feature) = act.upgrade() {
                    feature.toggle();
                }
            })
            .symbol(symbols::TYPING)
            .checked(move || mark.upgrade().is_some_and(|f| f.is_vietnamese()))
            .visible_when(move || shown.upgrade().is_some_and(|f| f.is_running())),
        )]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_are_off_and_type_telex_the_modern_way() {
        let config = Config::default();
        assert!(!config.enabled);
        assert_eq!(config.method, Method::Telex);
        assert!(config.modern_tone && config.english_restore);
        assert_eq!(config.shortcut, DEFAULT_SHORTCUT);
        assert!(config.apps.is_empty());
        let loaded: Config = serde_json::from_str("{}").unwrap();
        assert_eq!(loaded, config, "a file from before this feature");
    }

    #[test]
    fn the_shortcut_becomes_the_chord_the_keyboard_looks_for() {
        assert_eq!(
            chord_of("ctrl+space"),
            Some(Chord {
                code: 49,
                control: true,
                ..Chord::default()
            })
        );
        assert_eq!(
            chord_of("cmd+shift+opt+v"),
            Some(Chord {
                code: 9,
                command: true,
                shift: true,
                option: true,
                control: false
            })
        );
        assert_eq!(chord_of(""), None, "no shortcut");
        assert_eq!(chord_of("space"), None, "a key alone is no shortcut");
        assert_eq!(chord_of("ctrl+nonsense"), None);
        let settings = Config::default().keyboard();
        assert_eq!(settings.toggle, chord_of(DEFAULT_SHORTCUT));
        assert_eq!(settings.options, Options::default());
    }

    #[test]
    fn a_hand_edited_file_is_brought_back_to_what_works() {
        let config = Config {
            shortcut: "hyper+q".into(),
            apps: vec![
                "com.apple.Terminal".into(),
                "com.apple.Terminal".into(),
                "not a bundle id".into(),
                String::new(),
                "x".repeat(MAX_ID_LEN + 1),
                "com.googlecode.iterm2".into(),
            ],
            ..Config::default()
        }
        .sanitized();
        assert_eq!(config.shortcut, DEFAULT_SHORTCUT);
        assert_eq!(config.apps, ["com.apple.Terminal", "com.googlecode.iterm2"]);
        let many = Config {
            apps: (0..MAX_APPS + 20)
                .map(|i| format!("com.example.app{i}"))
                .collect(),
            ..Config::default()
        }
        .sanitized();
        assert_eq!(many.apps.len(), MAX_APPS);
        let none = Config {
            shortcut: String::new(),
            ..Config::default()
        }
        .sanitized();
        assert_eq!(none.shortcut, "", "no shortcut is a choice");
        assert_eq!(none.keyboard().toggle, None);
    }

    #[test]
    fn a_wrong_value_in_the_file_does_not_lose_the_rest() {
        let loaded: Config = serde_json::from_str(
            r#"{"enabled": true, "method": "vni", "modern_tone": false, "apps": ["com.apple.Notes"]}"#,
        )
        .unwrap();
        assert!(loaded.enabled);
        assert_eq!(loaded.method, Method::Vni);
        assert!(!loaded.modern_tone);
        assert_eq!(loaded.apps, ["com.apple.Notes"]);
        assert!(
            serde_json::from_str::<Config>(r#"{"method": "qwerty"}"#).is_err(),
            "the section falls back key by key in config.rs"
        );
    }

    #[test]
    fn bundle_identifiers_are_what_the_list_takes() {
        for id in [
            "com.apple.Terminal",
            "com.todesktop.230313mzl4w4u92",
            "texstudio",
            "org.my_app.x-y",
        ] {
            assert!(is_bundle_id(id), "{id}");
        }
        for id in [
            "",
            "com.apple.Terminal\n",
            "com apple",
            "com/apple",
            "com.ap\u{202E}ple",
            "trường.app",
        ] {
            assert!(!is_bundle_id(id), "{id:?}");
        }
    }
}
