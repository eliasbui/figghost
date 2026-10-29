//! Dictation: with the caret in a text box of any app, a shortcut brings a microphone to the middle of the screen; the
//! person speaks, sees the words as they are understood, and they are put into the text box. Off until the person
//! turns it on.
//!
//! The words are recognized by macOS's own models, one for each language: English and Vietnamese. With `on_device`
//! (the default) the audio never leaves the Mac. The words go in with Command+V, the person's clipboard saved first
//! and put back half a second later; without the Accessibility access that Command+V needs, or with no text box that
//! has the keyboard, they are left on the clipboard and the panel says so. Nothing is kept on disk.

use super::AttachEnv;
use crate::{
    config::parse_hotkey,
    history::when_focus_returns,
    i18n::{self, Language},
    settings::model::Fault,
    trace,
};
use macos_sys::{
    MainThreadMarker, app,
    ax::Element,
    dictation_ui::{DictationDisplay, DictationPanel, Stage},
    hotkey::Hotkey,
    panel::Key,
    pasteboard::{Pasteboard, Write},
    permissions,
    speech::{self, Heard, Listening},
    synth, workspace,
};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::{
    cell::RefCell,
    rc::{Rc, Weak},
    time::Instant,
};

pub mod settings;
mod text;

/// The silences after which the panel puts the words in by itself; 0 is never.
pub const SILENCE_SECONDS: [u32; 4] = [0, 2, 3, 5];
pub const SPEECH_PANE: &str =
    "x-apple.systempreferences:com.apple.preference.security?Privacy_SpeechRecognition";
/// How long the panel waits for nothing at all before it says nothing was heard, and the longest it listens.
const QUIET_SECONDS: f64 = 10.0;
// ponytail: one session is capped at three minutes; long dictation in parts, if anyone needs it.
const LONGEST_SECONDS: f64 = 180.0;
/// How long the last words may take once the microphone is off.
const LAST_WORDS_SECONDS: f64 = 1.5;
const NOTE_SECONDS: f64 = 3.5;
const LOOK_SECONDS: f64 = 0.2;
/// How long the app is given to read the paste before the person's clipboard is put back.
const RESTORE_SECONDS: f64 = 0.6;

/// The languages there are models for.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Spoken {
    /// The language of the app.
    #[default]
    Auto,
    Vi,
    En,
}

impl Spoken {
    /// `vi` or `en`: `Auto` follows the language of the app.
    pub fn code(self, app: Language) -> &'static str {
        match self {
            Spoken::Vi => "vi",
            Spoken::En => "en",
            Spoken::Auto if app == Language::Vietnamese => "vi",
            Spoken::Auto => "en",
        }
    }
}

/// The locale of the model for `vi` or `en`.
pub fn locale(code: &str) -> &'static str {
    if code == "vi" { "vi-VN" } else { "en-US" }
}

/// The other language, for the arrow keys.
fn other(code: &str) -> &'static str {
    if code == "vi" { "en" } else { "vi" }
}

/// The words of two stretches of speech, one after the other.
pub fn joined(before: &str, after: &str) -> String {
    match (before.trim(), after.trim()) {
        ("", b) => b.to_owned(),
        (a, "") => a.to_owned(),
        (a, b) => format!("{a} {b}"),
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    pub enabled: bool,
    /// Brings the microphone, and puts the words in when it is pressed again.
    pub shortcut: String,
    pub language: Spoken,
    /// The audio never leaves the Mac; a language this Mac cannot recognize by itself is then refused.
    pub on_device: bool,
    /// Seconds of silence after words after which they are put in, one of `SILENCE_SECONDS`.
    pub silence_seconds: u32,
    /// A soft sound when the microphone opens and when the words go in.
    pub sound: bool,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            enabled: false,
            shortcut: "ctrl+opt+v".into(),
            language: Spoken::Auto,
            on_device: true,
            silence_seconds: 3,
            sound: true,
        }
    }
}

impl Config {
    /// A config file can say anything; whatever is not offered is read as the default.
    pub fn sanitized(mut self) -> Self {
        if !SILENCE_SECONDS.contains(&self.silence_seconds) {
            self.silence_seconds = Self::default().silence_seconds;
        }
        if !self.shortcut.is_empty() && parse_hotkey(&self.shortcut).is_err() {
            self.shortcut = Self::default().shortcut;
        }
        self
    }
}

/// One time the microphone is open, from the shortcut to the words going in.
struct Session {
    listening: Option<Listening>,
    /// `vi` or `en`.
    spoken: &'static str,
    /// The words heard in another language before the arrows changed it.
    kept: String,
    /// The words of this language so far.
    words: String,
    started: Instant,
    changed: Instant,
    /// The microphone is off and the last words are awaited.
    finishing: bool,
    /// The app in front when the shortcut was pressed, which gets the keyboard back.
    target: Option<i32>,
    /// Whether a text box of that app had the keyboard.
    in_text: bool,
}

impl Session {
    fn text(&self) -> String {
        joined(&self.kept, &self.words)
    }
}

pub struct Feature {
    mtm: MainThreadMarker,
    me: Weak<Feature>,
    config: RefCell<Config>,
    hotkey: RefCell<Option<Hotkey>>,
    /// Made the first time the microphone opens.
    panel: RefCell<Option<Rc<DictationPanel>>>,
    session: RefCell<Option<Session>>,
    ticker: RefCell<Option<app::Timer>>,
    /// Bumped by every note, so an older timer never hides a newer panel.
    shown: std::cell::Cell<u64>,
}

pub fn attach(env: &AttachEnv) -> Option<Rc<Feature>> {
    let config = env.config.dictation.clone().sanitized();
    let feature = Rc::new_cyclic(|me| Feature {
        mtm: env.mtm,
        me: me.clone(),
        config: RefCell::new(config.clone()),
        hotkey: RefCell::default(),
        panel: RefCell::default(),
        session: RefCell::default(),
        ticker: RefCell::default(),
        shown: std::cell::Cell::new(0),
    });
    feature.register_hotkey(&config);
    // For the checks, which cannot press a global shortcut: the same as pressing it once, a moment after the start.
    if config.enabled && std::env::var_os("FIGGHOST_DICTATION_START").is_some() {
        let weak = Rc::downgrade(&feature);
        app::after(1.0, move || {
            if let Some(f) = weak.upgrade() {
                f.toggle();
            }
        });
    }
    Some(feature)
}

/// Whether the element that has the keyboard takes text where a caret is. Asks the app in front, which needs the
/// Accessibility access; without it the answer is no.
fn caret_in_text() -> bool {
    // The timeout is set on the field alone: set on the system-wide element it would be every element's.
    Element::system_wide()
        .and_then(|system| system.attribute("AXFocusedUIElement"))
        .is_some_and(|field| {
            field.set_timeout(0.3);
            field.range("AXSelectedTextRange").is_some()
        })
}

impl Feature {
    pub fn configure(&self, config: &Config) -> Result<(), Fault> {
        let config = config.clone().sanitized();
        *self.config.borrow_mut() = config.clone();
        self.hotkey.borrow_mut().take();
        if !config.enabled {
            self.cancel();
        }
        self.register_hotkey(&config);
        Ok(())
    }

    fn register_hotkey(&self, config: &Config) {
        if !config.enabled || config.shortcut.is_empty() {
            return;
        }
        let Ok(spec) = parse_hotkey(&config.shortcut) else {
            return;
        };
        let weak = self.me.clone();
        match Hotkey::register(self.mtm, spec.key_code, spec.modifiers, move || {
            if let Some(f) = weak.upgrade() {
                f.toggle();
            }
        }) {
            Ok(hotkey) => *self.hotkey.borrow_mut() = Some(hotkey),
            Err(status) => eprintln!("dictation: cannot register its shortcut (status {status})"),
        }
    }

    pub fn suspend_hotkeys(&self) {
        self.hotkey.borrow_mut().take();
    }

    pub fn resume_hotkeys(&self, config: &Config) {
        self.register_hotkey(&config.clone().sanitized());
    }

    /// Facts only the running app knows, for the settings page: the access it has, and what this Mac can do for
    /// each language.
    pub fn runtime(&self) -> Value {
        let model = |code: &str| {
            let m = speech::model(locale(code));
            json!({ "available": m.available, "on_device": m.on_device })
        };
        json!({
            "listening": self.session.borrow().is_some(),
            "speech": match speech::speech_access() {
                Some(true) => "allowed",
                Some(false) => "denied",
                None => "ask",
            },
            "microphone": permissions::microphone_trusted(),
            "accessibility": permissions::accessibility_trusted(false),
            "models": { "vi": model("vi"), "en": model("en") },
        })
    }

    /// The buttons of the settings page: ask for the access, or open where it is given.
    pub fn action(&self, name: &str) -> Option<Result<(), Fault>> {
        match name {
            "dictation_allow" => self.ask_access(),
            "dictation_open_speech" => permissions::open_pane(SPEECH_PANE),
            "dictation_open_microphone" => permissions::open_pane(permissions::MICROPHONE_PANE),
            "dictation_open_accessibility" => {
                permissions::open_pane(permissions::ACCESSIBILITY_PANE)
            }
            _ => return None,
        }
        Some(Ok(()))
    }

    /// Asks for what is not decided yet: Speech Recognition, then the microphone. A program that is not in its
    /// bundle has no words for the dialogs and is not asked.
    fn ask_access(&self) {
        if speech::speech_access().is_none() {
            speech::speech_request(|allowed| {
                trace::log(&format!("dictation: speech access {allowed}"))
            });
        }
        if !permissions::microphone_trusted() && speech::explains("NSMicrophoneUsageDescription") {
            permissions::microphone_request();
        }
    }

    fn language(&self) -> Language {
        i18n::current()
    }

    fn panel(&self) -> Rc<DictationPanel> {
        let mut slot = self.panel.borrow_mut();
        let panel = slot.get_or_insert_with(|| {
            let weak = self.me.clone();
            DictationPanel::new(self.mtm, move |key| {
                weak.upgrade().is_some_and(|f| f.key(key))
            })
        });
        Rc::clone(panel)
    }

    /// The shortcut: the microphone opens, or, open, the words go in.
    pub fn toggle(&self) {
        if self.session.borrow().is_some() {
            self.finish();
        } else {
            self.begin();
        }
    }

    fn key(&self, key: Key) -> bool {
        if self.session.borrow().is_none() {
            return false;
        }
        match key {
            Key::Enter => self.finish(),
            Key::Escape => self.cancel(),
            Key::Left | Key::Right => self.switch_language(),
            _ => return false,
        }
        true
    }

    fn begin(&self) {
        if !self.config.borrow().enabled {
            return;
        }
        let language = self.language();
        match speech::speech_access() {
            Some(true) => {}
            Some(false) => {
                return self.note("mic.slash.fill", language.dictation_no_speech_access());
            }
            None => {
                self.ask_access();
                return self.note("mic.fill", language.dictation_asking());
            }
        }
        if !permissions::microphone_trusted() {
            self.ask_access();
            return self.note("mic.slash.fill", language.dictation_no_microphone());
        }
        let target = workspace::frontmost_app().map(|a| a.pid);
        let in_text = permissions::accessibility_trusted(false) && caret_in_text();
        let spoken = self.config.borrow().language.code(language);
        let now = Instant::now();
        *self.session.borrow_mut() = Some(Session {
            listening: None,
            spoken,
            kept: String::new(),
            words: String::new(),
            started: now,
            changed: now,
            finishing: false,
            target,
            in_text,
        });
        if !self.listen(spoken) {
            return;
        }
        trace::log(&format!(
            "dictation: listening in {spoken}, text box {in_text}"
        ));
        if self.config.borrow().sound {
            workspace::play_sound("Tink");
        }
        let weak = self.me.clone();
        *self.ticker.borrow_mut() = Some(app::every(LOOK_SECONDS, move || {
            if let Some(f) = weak.upgrade() {
                f.look();
            }
        }));
    }

    /// Opens the microphone for `spoken` in the session; false, with the session ended and the reason shown, when
    /// it cannot.
    fn listen(&self, spoken: &'static str) -> bool {
        let on_device = self.config.borrow().on_device;
        let weak = self.me.clone();
        let started = Listening::start(locale(spoken), on_device, move |heard| {
            if let Some(f) = weak.upgrade() {
                f.heard(heard);
            }
        });
        let listening = match started {
            Ok(listening) => listening,
            Err(reason) => {
                eprintln!("dictation: cannot listen: {reason}");
                self.end();
                let language = self.language();
                let says = if on_device && !speech::model(locale(spoken)).on_device {
                    language.dictation_not_on_device()
                } else {
                    language.dictation_cannot_start()
                };
                self.note("mic.slash.fill", says);
                return false;
            }
        };
        if let Some(session) = self.session.borrow_mut().as_mut() {
            session.listening = Some(listening);
            session.spoken = spoken;
            session.changed = Instant::now();
        }
        self.paint(true);
        true
    }

    fn heard(&self, heard: Heard) {
        match heard {
            Heard::Level(level) => {
                if let Some(panel) = self.panel.borrow().as_ref() {
                    panel.level(level);
                }
            }
            Heard::Words { text, last } => {
                let finishing = {
                    let mut slot = self.session.borrow_mut();
                    let Some(session) = slot.as_mut() else {
                        return;
                    };
                    if session.words != text {
                        session.words = text;
                        session.changed = Instant::now();
                    }
                    session.finishing
                };
                if last && finishing {
                    self.deliver();
                } else {
                    self.paint(!finishing);
                }
            }
            Heard::Failed(reason) => {
                let (any, finishing) = match self.session.borrow().as_ref() {
                    Some(s) => (!s.text().is_empty(), s.finishing),
                    None => return,
                };
                trace::log(&format!("dictation: the recognizer stopped: {reason}"));
                if any || finishing {
                    self.deliver();
                } else {
                    self.end();
                    self.note("mic.slash.fill", self.language().dictation_nothing());
                }
            }
        }
    }

    /// Every fifth of a second while the microphone is open: a silence after words puts them in, a long silence
    /// with none gives up, and a session never lasts for ever.
    fn look(&self) {
        let (quiet, words, long, finishing) = match self.session.borrow().as_ref() {
            Some(s) => (
                s.changed.elapsed().as_secs_f64(),
                !s.text().is_empty(),
                s.started.elapsed().as_secs_f64() >= LONGEST_SECONDS,
                s.finishing,
            ),
            None => return,
        };
        if finishing {
            return;
        }
        let silence = f64::from(self.config.borrow().silence_seconds);
        if long || (words && silence > 0.0 && quiet >= silence) {
            self.finish();
        } else if !words && quiet >= QUIET_SECONDS {
            self.end();
            self.note("mic.slash.fill", self.language().dictation_nothing());
        }
    }

    /// The microphone goes off and the last words are awaited; pressed again, the words go in now.
    fn finish(&self) {
        let already = {
            let mut slot = self.session.borrow_mut();
            let Some(session) = slot.as_mut() else {
                return;
            };
            let already = session.finishing;
            session.finishing = true;
            if let Some(listening) = &session.listening {
                listening.finish();
            }
            already
        };
        if already {
            return self.deliver();
        }
        self.paint(false);
        let weak = self.me.clone();
        app::after(LAST_WORDS_SECONDS, move || {
            if let Some(f) = weak.upgrade()
                && f.session.borrow().as_ref().is_some_and(|s| s.finishing)
            {
                f.deliver();
            }
        });
    }

    /// The arrows: what was heard so far stays, and the rest is heard in the other language.
    fn switch_language(&self) {
        let next = {
            let mut slot = self.session.borrow_mut();
            let Some(session) = slot.as_mut() else {
                return;
            };
            if session.finishing {
                return;
            }
            session.kept = session.text();
            session.words.clear();
            // Dropped here, so that its last words do not come after the switch.
            session.listening = None;
            other(session.spoken)
        };
        trace::log(&format!("dictation: now listening in {next}"));
        self.listen(next);
    }

    /// Escape, or the feature turned off: nothing goes in.
    fn cancel(&self) {
        if self.session.borrow().is_some() {
            trace::log("dictation: cancelled");
        }
        self.end();
        if let Some(panel) = self.panel.borrow().as_ref() {
            panel.hide();
        }
    }

    /// Ends the session: the microphone off, the looks stopped. The panel stays for whoever shows a note on it.
    fn end(&self) -> Option<Session> {
        self.ticker.borrow_mut().take();
        self.session.borrow_mut().take()
    }

    /// The words go into the text box the person was in, or onto the clipboard.
    fn deliver(&self) {
        let Some(session) = self.end() else {
            return;
        };
        let text = session.text();
        if let Some(panel) = self.panel.borrow().as_ref() {
            panel.hide();
        }
        trace::log(&format!(
            "dictation: {} characters for {}",
            text.chars().count(),
            if session.in_text {
                "the text box"
            } else {
                "the clipboard"
            }
        ));
        if text.is_empty() {
            return;
        }
        if self.config.borrow().sound {
            workspace::play_sound("Pop");
        }
        if !session.in_text || !permissions::accessibility_trusted(false) {
            Pasteboard::general().write(Write::Text(&text));
            return self.note("doc.on.clipboard", self.language().dictation_copied());
        }
        let me = self.me.clone();
        when_focus_returns(session.target, move |ready| {
            let Some(f) = me.upgrade() else {
                return;
            };
            if ready {
                paste(&text);
            } else {
                Pasteboard::general().write(Write::Text(&text));
                f.note("doc.on.clipboard", f.language().dictation_copied());
            }
        });
    }

    /// Shows the panel while the microphone is open or the last words are awaited.
    fn paint(&self, listening: bool) {
        let Some((text, spoken)) = self.session.borrow().as_ref().map(|s| (s.text(), s.spoken))
        else {
            return;
        };
        let language = self.language();
        self.shown.set(self.shown.get() + 1);
        self.panel().show(
            &DictationDisplay {
                stage: if listening {
                    Stage::Listening
                } else {
                    Stage::Finishing
                },
                symbol: if listening { "mic.fill" } else { "waveform" },
                text,
                placeholder: if listening {
                    language.dictation_listening()
                } else {
                    language.dictation_finishing()
                }
                .to_owned(),
                language: language.dictation_language(spoken).to_owned(),
                hint: language.dictation_hint().to_owned(),
            },
            true,
        );
    }

    /// Says something on the panel for a few seconds, without taking the keyboard.
    fn note(&self, symbol: &'static str, says: &str) {
        trace::log(&format!("dictation: a note with {symbol}"));
        let generation = self.shown.get() + 1;
        self.shown.set(generation);
        let panel = self.panel();
        panel.show(
            &DictationDisplay {
                stage: Stage::Note,
                symbol,
                text: says.to_owned(),
                ..DictationDisplay::default()
            },
            false,
        );
        let weak = self.me.clone();
        app::after(NOTE_SECONDS, move || {
            if let Some(f) = weak.upgrade()
                && f.shown.get() == generation
                && f.session.borrow().is_none()
            {
                panel.hide();
            }
        });
    }
}

/// Pastes `text` into the app that has the keyboard, keeping the person's clipboard: it is saved, the words go on it
/// marked as passing (the clipboard history keeps no copy), Command+V is posted, and it is put back unless something
/// was copied meanwhile. A clipboard that cannot be saved in full (a promised file that never came) is not risked:
/// the words are left on it instead.
fn paste(text: &str) {
    let board = Pasteboard::general();
    let Some(saved) = board.snapshot() else {
        board.write(Write::Text(text));
        return;
    };
    let count = board.write_transient(text);
    synth::post_paste();
    app::after(RESTORE_SECONDS, move || {
        let board = Pasteboard::general();
        if board.change_count() == count {
            board.restore(&saved);
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_feature_is_off_by_default_and_a_hostile_file_reads_as_the_defaults() {
        let config: Config = serde_json::from_str("{}").unwrap();
        assert_eq!(config, Config::default());
        assert!(!config.enabled && config.on_device && config.sound);
        assert_eq!(config.language, Spoken::Auto);
        let hostile = Config {
            silence_seconds: 7,
            shortcut: "nonsense".into(),
            ..Config::default()
        }
        .sanitized();
        assert_eq!(hostile.silence_seconds, 3);
        assert_eq!(hostile.shortcut, "ctrl+opt+v");
        let none = Config {
            shortcut: String::new(),
            ..Config::default()
        }
        .sanitized();
        assert!(none.shortcut.is_empty(), "no shortcut is a choice");
        assert!(
            serde_json::from_str::<Config>(r#"{ "language": "fr" }"#).is_err(),
            "an unknown language is not read"
        );
    }

    #[test]
    fn the_language_follows_the_app_unless_one_is_chosen_and_each_has_its_model() {
        assert_eq!(Spoken::Auto.code(Language::Vietnamese), "vi");
        assert_eq!(Spoken::Auto.code(Language::English), "en");
        assert_eq!(Spoken::En.code(Language::Vietnamese), "en");
        assert_eq!(Spoken::Vi.code(Language::English), "vi");
        assert_eq!(locale("vi"), "vi-VN");
        assert_eq!(locale("en"), "en-US");
        assert_eq!(other("vi"), "en");
        assert_eq!(other("en"), "vi");
    }

    #[test]
    fn words_heard_in_two_languages_are_joined_with_one_space() {
        assert_eq!(joined("", "xin chào"), "xin chào");
        assert_eq!(joined("xin chào ", ""), "xin chào");
        assert_eq!(joined(" xin chào", " hello world "), "xin chào hello world");
        assert_eq!(joined("  ", " "), "");
    }

    #[test]
    fn the_words_are_there_in_both_languages() {
        for language in [Language::English, Language::Vietnamese] {
            for text in [
                language.dictation_listening(),
                language.dictation_finishing(),
                language.dictation_hint(),
                language.dictation_copied(),
                language.dictation_nothing(),
                language.dictation_asking(),
                language.dictation_no_speech_access(),
                language.dictation_no_microphone(),
                language.dictation_not_on_device(),
                language.dictation_cannot_start(),
                language.dictation_language("vi"),
                language.dictation_language("en"),
            ] {
                assert!(!text.is_empty());
            }
        }
        assert_eq!(Language::Vietnamese.dictation_language("vi"), "Tiếng Việt");
        assert_eq!(Language::English.dictation_language("en"), "English");
    }
}
