//! Vietnamese typing: Telex or VNI in every app, with the engine of Gõ Nhanh. Off by default.
//!
//! The keyboard side (`macos_sys::typing`) holds each key press that ends in a mark and types the word again with it;
//! this folder holds the settings, the shortcut that switches between Vietnamese and English (seen on the keyboard's
//! thread, so that the system's own use of the same keys never gets it first), the entry in the menu bar panel and
//! the notice on the notch. Another Vietnamese input method at work would put every mark twice, so while one runs
//! every key is left alone and the settings page says why.
//!
//! The same keyboard also serves snippets (`features::snippets`), which is why it can run with Vietnamese typing off:
//! it then starts in English, has no shortcut, and shows no badge or menu entry.

use super::{AttachEnv, islands};
use crate::{config::parse_hotkey, i18n, menu::symbols, settings::model::Fault};
use core_typing::{Chord, Options};
use macos_sys::{
    MainThreadMarker, app,
    hotkey::modifier,
    notch::{Activity, Detail, Ear, Notch},
    permissions,
    status_item::{Entry, Item, TextItem},
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
    time::Instant,
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
/// The language shown all the time, under everything else that is live.
const BADGE_SLOT: &str = "typing_language";
const BADGE_PRIORITY: u8 = 1;
const PRIORITY: u8 = 21;
const SHOW_SECONDS: f64 = 1.5;
/// The flag of Vietnam waving, for the notice of switching to Vietnamese (docs/typing/build.py): a row of square
/// frames that loops.
const WAVING: &[u8] = include_bytes!("../../../assets/typing/vietnam.png");
const WAVING_COUNT: usize = 8;
const WAVING_FPS: f64 = 8.0;
/// The language as a flag (docs/typing/build.py): Vietnam, then the United States for English.
const FLAGS: &[u8] = include_bytes!("../../../assets/typing/flags.png");

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
    /// The language on the closed notch all the time, not only for a moment when it changes.
    pub notch_badge: bool,
    /// An item of its own in the menu bar with the language, that switches it when clicked.
    pub tray_badge: bool,
    /// The language as a flag rather than VI or EN, on the notch and in the menu bar.
    pub flag: bool,
    /// The notice of switching to Vietnamese shows the flag of Vietnam waving.
    pub waving_flag: bool,
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
            notch_badge: true,
            tray_badge: true,
            flag: false,
            waving_flag: true,
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
            snippets: Vec::new(),
        }
    }
}

type ChordSink = Box<dyn Fn(Option<String>)>;

thread_local! {
    /// The running feature, for the shortcut: it is seen on the keyboard's thread and handled on this one.
    static LIVE: RefCell<Weak<Feature>> = const { RefCell::new(Weak::new()) };
    /// Where a shortcut taken while the settings page records one goes: to the page.
    static CHORD_SINK: RefCell<Option<ChordSink>> = const { RefCell::new(None) };
}

/// Tells `sink` the shortcut ("ctrl+space"; `None` for a key shortcuts have no name for) the keyboard took while
/// the settings page was recording one. The system gives some shortcuts to itself before any window sees them, so
/// the page would never hear of them.
pub fn on_chord(sink: impl Fn(Option<String>) + 'static) {
    CHORD_SINK.with(|slot| *slot.borrow_mut() = Some(Box::new(sink)));
}

/// A key pressed with modifiers as a shortcut is written: "ctrl+opt+shift+cmd+space".
fn chord_text(code: u16, flags: u64) -> Option<String> {
    let key = typing::key_of(code, flags);
    let name = crate::config::key_name(u32::from(code))?;
    let mut parts: Vec<&str> = Vec::new();
    for (held, word) in [
        (key.control, "ctrl"),
        (key.option, "opt"),
        (key.shift, "shift"),
        (key.command, "cmd"),
    ] {
        if held {
            parts.push(word);
        }
    }
    parts.push(name);
    Some(parts.join("+"))
}

/// Runs on the keyboard's thread, inside the input path.
fn on_chord_taken(code: u16, flags: u64) {
    app::on_main(move || {
        let text = chord_text(code, flags);
        CHORD_SINK.with(|slot| {
            if let Some(sink) = &*slot.borrow() {
                sink(text);
            }
        });
    });
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

fn word(vietnamese: bool) -> &'static str {
    if vietnamese { "VI" } else { "EN" }
}

fn flag_frame(vietnamese: bool) -> usize {
    if vietnamese { 0 } else { 1 }
}

/// The running feature. Created by `attach`, kept alive by the app context.
pub struct Feature {
    mtm: MainThreadMarker,
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
    /// The abbreviations the keyboard replaces; empty while snippets are off.
    snippets: RefCell<Vec<core_typing::Snippet>>,
    /// The item of the menu bar that shows the language.
    tray: RefCell<Option<TextItem>>,
    /// Counts the notices, so that the frames of the waving flag stop when a newer notice replaces it.
    notices: Cell<u64>,
    me: Weak<Feature>,
}

/// Starts the feature; it runs only while switched on.
pub fn attach(env: &AttachEnv) -> Option<Rc<Feature>> {
    let feature = Rc::new_cyclic(|me| Feature {
        mtm: env.mtm,
        notch: env.notch.map(Rc::downgrade),
        config: RefCell::new(Config::default()),
        keyboard: RefCell::default(),
        retry: RefCell::default(),
        front: RefCell::default(),
        recording: Cell::new(false),
        other: RefCell::default(),
        names: RefCell::default(),
        snippets: RefCell::new(env.config.snippets.clone().sanitized().list()),
        tray: RefCell::default(),
        notices: Cell::new(0),
        me: me.clone(),
    });
    LIVE.with(|live| *live.borrow_mut() = Rc::downgrade(&feature));
    feature.apply(&env.config.typing.clone().sanitized());
    Some(feature)
}

impl Feature {
    fn apply(&self, config: &Config) {
        let was_on = self.config.borrow().enabled;
        *self.config.borrow_mut() = config.clone();
        if !self.wanted() {
            self.stop();
            return;
        }
        *self.other.borrow_mut() = if config.enabled {
            typing::other_input_method()
        } else {
            None
        };
        if let Some(keyboard) = &*self.keyboard.borrow() {
            keyboard.configure(self.settings());
            // Typing starts in Vietnamese each time it is switched on, and is English while only snippets run.
            if config.enabled != was_on {
                keyboard.set_vietnamese(config.enabled);
            }
            keyboard.set_paused(self.paused());
        } else {
            self.start();
        }
        self.refresh_badges();
    }

    /// The keyboard runs while Vietnamese typing or snippets need it.
    fn wanted(&self) -> bool {
        self.config.borrow().enabled || !self.snippets.borrow().is_empty()
    }

    /// What the keyboard side needs: the shortcut that switches the language only while Vietnamese typing is on.
    fn settings(&self) -> core_typing::Settings {
        let config = self.config.borrow();
        let mut settings = config.keyboard();
        if !config.enabled {
            settings.toggle = None;
        }
        settings.snippets = self.snippets.borrow().clone();
        settings
    }

    /// Takes a new list of snippets (empty for none), starting or stopping the keyboard as needed.
    pub fn set_snippets(&self, snippets: Vec<core_typing::Snippet>) {
        *self.snippets.borrow_mut() = snippets;
        let config = self.config.borrow().clone();
        self.apply(&config);
    }

    /// The name of another Vietnamese input method that is running, while Vietnamese typing is on.
    pub fn other(&self) -> Option<String> {
        self.other.borrow().clone()
    }

    fn paused(&self) -> bool {
        self.recording.get() || self.other.borrow().is_some()
    }

    fn start(&self) {
        if self.keyboard.borrow().is_some() || !self.wanted() {
            return;
        }
        let settings = self.settings();
        let vietnamese = self.config.borrow().enabled;
        let frontmost = workspace::frontmost_app().map(|a| a.bundle_id);
        match Keyboard::start(settings, vietnamese, frontmost, on_toggle, on_chord_taken) {
            Ok(keyboard) => {
                keyboard.set_paused(self.paused());
                keyboard.set_capture(self.recording.get());
                *self.keyboard.borrow_mut() = Some(keyboard);
                self.retry.borrow_mut().take();
                self.watch_front();
                self.refresh_badges();
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
        self.refresh_badges();
    }

    /// Puts the language on the notch and in the menu bar, or takes it off, as the settings and the keyboard say.
    fn refresh_badges(&self) {
        let (notch_badge, tray_badge, flag) = {
            let config = self.config.borrow();
            (config.notch_badge, config.tray_badge, config.flag)
        };
        let running = self.typing_on();
        let vietnamese = self.is_vietnamese();
        if let Some(notch) = self.notch.as_ref().and_then(Weak::upgrade) {
            let activity = (running && notch_badge).then(|| {
                let tone = if vietnamese { Tone::Green } else { Tone::Gray };
                Activity {
                    left: Ear::Symbol {
                        name: "keyboard",
                        tone,
                        pulse: false,
                    },
                    right: self.language_ear(vietnamese),
                    detail: None,
                    page: None,
                    priority: BADGE_PRIORITY,
                }
            });
            notch.set_activity(BADGE_SLOT, activity);
        }
        if !(running && tray_badge) {
            self.tray.borrow_mut().take();
            return;
        }
        let mut tray = self.tray.borrow_mut();
        let item = tray.get_or_insert_with(|| {
            let me = self.me.clone();
            TextItem::new(self.mtm, i18n::current().typing_menu(), "", move || {
                if let Some(feature) = me.upgrade() {
                    feature.toggle();
                }
            })
        });
        if flag {
            item.set_sprite(FLAGS, flag_frame(vietnamese));
        } else {
            item.set_text(word(vietnamese));
        }
    }

    /// The language on the right of the notch: VI or EN, or its flag when the settings ask for it.
    fn language_ear(&self, vietnamese: bool) -> Ear {
        if self.config.borrow().flag {
            return Ear::Sprite {
                sheet: FLAGS,
                frame: flag_frame(vietnamese),
            };
        }
        Ear::Text {
            text: word(vietnamese).into(),
            tone: if vietnamese { Tone::Green } else { Tone::Gray },
        }
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
            keyboard.set_capture(true);
        }
    }

    pub fn resume_hotkeys(&self, _config: &Config) {
        self.recording.set(false);
        if let Some(keyboard) = &*self.keyboard.borrow() {
            keyboard.set_capture(false);
            keyboard.set_paused(self.paused());
        }
    }

    pub fn is_running(&self) -> bool {
        self.keyboard.borrow().is_some()
    }

    /// Vietnamese typing itself is on and its keyboard runs; the keyboard may also run for snippets alone.
    fn typing_on(&self) -> bool {
        self.config.borrow().enabled && self.is_running()
    }

    pub fn is_vietnamese(&self) -> bool {
        self.keyboard
            .borrow()
            .as_ref()
            .is_some_and(Keyboard::is_vietnamese)
    }

    /// Switches between Vietnamese and English, as the shortcut does.
    pub fn toggle(&self) {
        if !self.typing_on() {
            return;
        }
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

    /// Says on the notch which language typing is in now; Vietnamese comes with the flag waving, when it is wanted.
    fn announce(&self, vietnamese: bool) {
        self.refresh_badges();
        let Some(notch) = self.notch.as_ref().and_then(Weak::upgrade) else {
            return;
        };
        let serial = self.notices.get() + 1;
        self.notices.set(serial);
        islands::show_for(&notch, SLOT, self.notice(vietnamese, 0), Some(SHOW_SECONDS));
        if !(vietnamese && self.config.borrow().waving_flag) {
            return;
        }
        // One timer for each frame, each checking that its notice is still the one shown: a repeating timer would
        // have to be dropped from inside its own call. The last frame is up before the notice is taken down.
        let started = Instant::now();
        let frames = (SHOW_SECONDS * WAVING_FPS) as usize;
        for frame in 1..frames {
            let (me, notch) = (self.me.clone(), Rc::downgrade(&notch));
            app::after(frame as f64 / WAVING_FPS, move || {
                let (Some(feature), Some(notch)) = (me.upgrade(), notch.upgrade()) else {
                    return;
                };
                if feature.notices.get() == serial
                    && started.elapsed().as_secs_f64() < SHOW_SECONDS - 0.5 / WAVING_FPS
                {
                    notch.set_activity(SLOT, Some(feature.notice(true, frame)));
                }
            });
        }
    }

    /// The notice of a change of language, with frame `frame` of the waving flag when it is wanted.
    fn notice(&self, vietnamese: bool, frame: usize) -> Activity {
        let language = i18n::current();
        let tone = if vietnamese { Tone::Green } else { Tone::Gray };
        let (method, waving) = {
            let config = self.config.borrow();
            (config.method.label(), config.waving_flag)
        };
        Activity {
            left: if vietnamese && waving {
                Ear::Sprite {
                    sheet: WAVING,
                    frame: frame % WAVING_COUNT,
                }
            } else {
                Ear::Symbol {
                    name: "keyboard",
                    tone,
                    pulse: false,
                }
            },
            right: self.language_ear(vietnamese),
            detail: Some(Detail {
                title: language.typing_now(vietnamese).to_owned(),
                subtitle: language.typing_now_detail(vietnamese, method),
            }),
            page: None,
            priority: PRIORITY,
        }
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
            .visible_when(move || shown.upgrade().is_some_and(|f| f.typing_on())),
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
        assert!(config.notch_badge && config.tray_badge);
        assert!(!config.flag && config.waving_flag);
        let loaded: Config = serde_json::from_str("{}").unwrap();
        assert_eq!(loaded, config, "a file from before this feature");
    }

    #[test]
    fn the_flags_are_rows_of_the_frames_the_code_shows() {
        // The size from the PNG header: width, then height, each four bytes after the signature and IHDR's head.
        let frames = |png: &[u8]| {
            let at = |i: usize| u32::from_be_bytes(png[i..i + 4].try_into().unwrap());
            assert_eq!(at(16) % at(20), 0, "square frames");
            (at(16) / at(20)) as usize
        };
        assert_eq!(frames(WAVING), WAVING_COUNT);
        assert_eq!(frames(FLAGS), 2, "Vietnamese, then English");
        assert!(flag_frame(true) < 2 && flag_frame(false) < 2);
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
    fn a_taken_key_is_written_as_a_shortcut() {
        assert_eq!(chord_text(49, 0x0004_0000).as_deref(), Some("ctrl+space"));
        assert_eq!(
            chord_text(9, 0x0002_0000 | 0x0008_0000 | 0x0010_0000).as_deref(),
            Some("opt+shift+cmd+v")
        );
        assert_eq!(
            chord_text(10, 0x0004_0000),
            None,
            "a key shortcuts cannot name"
        );
        // The recorder of the page writes the same, and the parser reads it back as the same chord.
        assert_eq!(
            chord_of(&chord_text(49, 0x0004_0000).unwrap()),
            chord_of("ctrl+space")
        );
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
