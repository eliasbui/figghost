//! Vietnamese typing with no interface. The marks are put by the Telex and VNI engine of Gõ Nhanh
//! (`gonhanh-core`, BSD 3-Clause); this crate decides what becomes of each key press, the way Gõ Nhanh's own macOS
//! app does: a key either goes on to the app as it is, or is held back while the end of the word is deleted and
//! typed again with its marks. `plan` says how each app wants that typing posted.
//!
//! Key codes are the macOS virtual key codes of an ANSI keyboard, which is what the engine reads.

use gonhanh_core::engine::{Action as EngineAction, Engine};

mod plan;

pub use plan::{Delays, Delivery, Plan, plan_for};

/// The virtual key codes the rules name.
pub mod key {
    pub const RETURN: u16 = 36;
    pub const TAB: u16 = 48;
    pub const SPACE: u16 = 49;
    pub const DELETE: u16 = 51;
    pub const ESCAPE: u16 = 53;
    pub const ENTER: u16 = 76;
    pub const HOME: u16 = 115;
    pub const PAGE_UP: u16 = 116;
    pub const FORWARD_DELETE: u16 = 117;
    pub const END: u16 = 119;
    pub const PAGE_DOWN: u16 = 121;
    pub const LEFT: u16 = 123;
    pub const RIGHT: u16 = 124;
    pub const DOWN: u16 = 125;
    pub const UP: u16 = 126;
    pub const EQUAL: u16 = 24;
    pub const MINUS: u16 = 27;
    pub const RIGHT_BRACKET: u16 = 30;
    pub const LEFT_BRACKET: u16 = 33;
    pub const QUOTE: u16 = 39;
    pub const SEMICOLON: u16 = 41;
    pub const BACKSLASH: u16 = 42;
    pub const COMMA: u16 = 43;
    pub const SLASH: u16 = 44;
    pub const PERIOD: u16 = 47;
    pub const GRAVE: u16 = 50;
    /// 1 to 9, then 0.
    pub const DIGITS: [u16; 10] = [18, 19, 20, 21, 23, 22, 26, 28, 25, 29];
}

/// A key press as the keyboard reports it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Key {
    pub code: u16,
    pub shift: bool,
    pub caps_lock: bool,
    pub control: bool,
    pub option: bool,
    pub command: bool,
}

/// A key with the modifiers that must be held with it, such as the shortcut that switches between Vietnamese
/// and English. Caps Lock does not count.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Chord {
    pub code: u16,
    pub shift: bool,
    pub control: bool,
    pub option: bool,
    pub command: bool,
}

impl Chord {
    pub fn matches(&self, key: &Key) -> bool {
        key.code == self.code
            && key.shift == self.shift
            && key.control == self.control
            && key.option == self.option
            && key.command == self.command
    }
}

/// The two ways of typing the marks.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Method {
    /// Letters: `aa` is â, `s` is the acute accent.
    #[default]
    Telex,
    /// Digits: `a6` is â, `1` is the acute accent.
    Vni,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Options {
    pub method: Method,
    /// Puts the mark where the 1997 rules do (`hoà`, `khoẻ`) rather than the older way (`hòa`, `khỏe`).
    pub modern_tone: bool,
    /// Gives back an English word the marks would have spoilt (`text`, `with`) when the word ends.
    pub english_restore: bool,
}

impl Default for Options {
    fn default() -> Self {
        Self {
            method: Method::Telex,
            modern_tone: true,
            english_restore: true,
        }
    }
}

/// What becomes of the key itself after a replacement.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Then {
    /// It is held back: the replacement stands for it.
    Drop,
    /// It goes on to the app after the replacement.
    Pass,
    /// It is held back and posted again after the replacement, so that it lands after the text.
    Retype,
}

/// What to do with one key press.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Verdict {
    /// The key goes on to the app as it is.
    Pass,
    /// Delete `delete` characters before the cursor and type `text`.
    Replace {
        delete: usize,
        text: String,
        then: Then,
    },
}

/// What the engine answered for one key.
struct Answer {
    delete: usize,
    text: String,
    consumed: bool,
}

/// The engine and the rules around it, for one stream of key presses.
pub struct Typist {
    engine: Engine,
}

const NAVIGATION: [u16; 8] = [
    key::LEFT,
    key::RIGHT,
    key::UP,
    key::DOWN,
    key::HOME,
    key::END,
    key::PAGE_UP,
    key::PAGE_DOWN,
];

/// Keys that end a word: space, punctuation, arrows, and the shifted digits, which type symbols.
fn is_break(code: u16, shift: bool) -> bool {
    matches!(
        code,
        key::SPACE
            | key::TAB
            | key::RETURN
            | key::ENTER
            | key::ESCAPE
            | key::LEFT
            | key::RIGHT
            | key::UP
            | key::DOWN
            | key::PERIOD
            | key::COMMA
            | key::SLASH
            | key::SEMICOLON
            | key::QUOTE
            | key::LEFT_BRACKET
            | key::RIGHT_BRACKET
            | key::BACKSLASH
            | key::MINUS
            | key::EQUAL
            | key::GRAVE
    ) || (shift && key::DIGITS.contains(&code))
}

impl Typist {
    pub fn new(options: Options) -> Self {
        let mut typist = Self {
            engine: Engine::new(),
        };
        typist.set_options(options);
        typist
    }

    pub fn set_options(&mut self, options: Options) {
        self.engine.set_method(match options.method {
            Method::Telex => 0,
            Method::Vni => 1,
        });
        self.engine.set_modern_tone(options.modern_tone);
        self.engine
            .set_english_auto_restore(options.english_restore);
        self.engine.clear_all();
    }

    /// The cursor or the focus moved: the word being typed is no longer the one before the cursor.
    pub fn forget(&mut self) {
        self.engine.clear_all();
    }

    /// Control went down. A word is not continued across it, as in EVKey.
    pub fn control_down(&mut self) {
        self.engine.clear();
    }

    fn ask(&mut self, code: u16, caps: bool, bypass: bool, shift: bool) -> Option<Answer> {
        let result = self.engine.on_key_ext(code, caps, bypass, shift);
        (result.action == EngineAction::Send as u8).then(|| Answer {
            delete: usize::from(result.backspace),
            text: result.chars[..usize::from(result.count)]
                .iter()
                .filter_map(|&c| char::from_u32(c))
                .collect(),
            consumed: result.key_consumed(),
        })
    }

    pub fn key(&mut self, key: Key) -> Verdict {
        let caps = key.shift || key.caps_lock;
        let bypass = key.command || key.control;
        // Return goes to the engine first: it may give back an English word, which must land before the line break.
        if key.code == key::RETURN || key.code == key::ENTER {
            return match self.ask(key.code, caps, bypass, key.shift) {
                Some(a) if a.delete > 0 || !a.text.is_empty() => Verdict::Replace {
                    delete: a.delete,
                    text: a.text,
                    then: if a.consumed { Then::Drop } else { Then::Retype },
                },
                _ => Verdict::Pass,
            };
        }
        // Moving or selecting by word, by line or by page leaves the word behind.
        if NAVIGATION.contains(&key.code) && (key.command || key.option || key.shift) {
            self.engine.clear();
            return Verdict::Pass;
        }
        // Command shortcuts: a new tab, a paste, an undo. Whatever was being typed is over.
        if key.command && !key.control && !key.option {
            self.engine.clear_all();
            return Verdict::Pass;
        }
        // Option+Delete deletes a whole word, which the engine cannot follow.
        if key.code == key::DELETE && key.option {
            self.engine.clear();
            return Verdict::Pass;
        }
        if key.code == key::DELETE && !bypass {
            return match self.ask(key::DELETE, caps, false, key.shift) {
                Some(a) => Verdict::Replace {
                    delete: a.delete,
                    text: a.text,
                    then: Then::Drop,
                },
                None => Verdict::Pass,
            };
        }
        // Option with a key types a symbol (Option+V is √): no marks, and the word ends there.
        if key.option && !bypass {
            self.ask(key.code, caps, true, key.shift);
            return Verdict::Pass;
        }
        let Some(a) = self.ask(key.code, caps, bypass, key.shift) else {
            return Verdict::Pass;
        };
        // Space is part of a restored word's text; punctuation is not, so it follows the replacement.
        let breaks = is_break(key.code, key.shift) && key.code != key::SPACE && !a.consumed;
        let then = if !breaks {
            Then::Drop
        } else if a.delete > 0 && !a.text.is_empty() {
            Then::Retype
        } else {
            Then::Pass
        };
        Verdict::Replace {
            delete: a.delete,
            text: a.text,
            then,
        }
    }
}

/// Names of the other Vietnamese input methods for the Mac, folded as `fold` does. With two at work every mark is
/// put twice.
const OTHER_INPUT_METHODS: [&str; 7] = [
    "gonhanh",
    "gõnhanh",
    "evkey",
    "openkey",
    "gotiengviet",
    "unikey",
    "org.gonhanh.gonhanh",
];

fn fold(name: &str) -> String {
    name.chars()
        .filter(|c| !c.is_whitespace())
        .flat_map(char::to_lowercase)
        .collect()
}

/// Whether an app, by its name or bundle identifier, is another Vietnamese input method.
pub fn is_other_input_method(name: &str) -> bool {
    OTHER_INPUT_METHODS.contains(&fold(name).as_str())
}

/// What the person chose, as the keyboard side needs it.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Settings {
    pub options: Options,
    /// The shortcut that switches between Vietnamese and English; `None` for none, and while the settings page
    /// records a shortcut.
    pub toggle: Option<Chord>,
    /// Bundle identifiers of the apps where typing stays as it is.
    pub excluded: Vec<String>,
}

/// What the keyboard reports, of what matters here.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Event {
    Key(Key),
    /// Control went down on its own.
    ControlDown,
    /// A mouse button went down: the cursor may be anywhere now.
    Click,
}

/// The element that has the keyboard focus, as far as it could be told.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Focus<'a> {
    /// Bundle identifier of its app.
    pub app: Option<&'a str>,
    /// Its Accessibility role, such as `AXTextField`.
    pub role: Option<&'a str>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Action {
    Pass,
    /// The shortcut was pressed: hold it back and switch between Vietnamese and English.
    Toggle,
    /// Hold the key back and post the replacement the way `plan` says.
    Type {
        plan: Plan,
        delete: usize,
        text: String,
        then: Then,
    },
}

fn is_discord(app: &str) -> bool {
    app.to_lowercase().contains("discord")
}

/// One keyboard's worth of typing: the typist, and the choices that decide when it may act.
pub struct Session {
    typist: Typist,
    settings: Settings,
}

impl Session {
    pub fn new(settings: Settings) -> Self {
        Self {
            typist: Typist::new(settings.options),
            settings,
        }
    }

    pub fn configure(&mut self, settings: Settings) {
        if settings.options != self.settings.options {
            self.typist.set_options(settings.options);
        }
        self.settings = settings;
    }

    /// The focus moved to another app.
    pub fn forget(&mut self) {
        self.typist.forget();
    }

    /// `vietnamese` is false while typing is switched to English.
    pub fn handle(&mut self, event: Event, focus: Focus<'_>, vietnamese: bool) -> Action {
        let key = match event {
            Event::Key(key) => key,
            Event::ControlDown => {
                self.typist.control_down();
                return Action::Pass;
            }
            Event::Click => {
                self.typist.forget();
                return Action::Pass;
            }
        };
        if self
            .settings
            .toggle
            .is_some_and(|chord| chord.matches(&key))
        {
            self.typist.forget();
            return Action::Toggle;
        }
        let excluded = focus
            .app
            .is_some_and(|app| self.settings.excluded.iter().any(|e| e == app));
        let plan = plan_for(focus.app, focus.role);
        // Discord sends the message on Return, before a restored word could reach it.
        let discord_return = (key.code == key::RETURN || key.code == key::ENTER)
            && focus.app.is_some_and(is_discord);
        if !vietnamese || excluded || plan.delivery == Delivery::Passthrough || discord_return {
            self.typist.forget();
            return Action::Pass;
        }
        match self.typist.key(key) {
            Verdict::Pass => Action::Pass,
            Verdict::Replace { delete, text, then } => Action::Type {
                plan,
                delete,
                text,
                then,
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The key that types `c` on an ANSI keyboard, with Shift where it needs it. Backspace is `\u{8}`, Return `\n`.
    fn press(c: char) -> Key {
        const LOWER: &str = "asdfhgzxcv\u{0}bqweryt123465=97-80]ou[ip\u{0}lj'k;\\,/nm.\u{0}\u{0}`";
        const UPPER: &str = "ASDFHGZXCV\u{0}BQWERYT!@#$^%+(&_*)}OU{IP\u{0}LJ\"K:|<?NM>\u{0}\u{0}~";
        let code = match c {
            ' ' => key::SPACE,
            '\u{8}' => key::DELETE,
            '\n' => key::RETURN,
            _ => LOWER
                .chars()
                .position(|l| l == c)
                .or_else(|| UPPER.chars().position(|u| u == c))
                .unwrap_or_else(|| panic!("no key types {c:?}")) as u16,
        };
        Key {
            code,
            shift: UPPER.contains(c),
            ..Key::default()
        }
    }

    /// A text field that takes the keys and the replacements the way an app does.
    #[derive(Default)]
    struct Field(Vec<char>);

    impl Field {
        fn take(&mut self, typed: char) {
            match typed {
                '\u{8}' => {
                    self.0.pop();
                }
                c => self.0.push(c),
            }
        }

        fn apply(&mut self, typed: char, verdict: Verdict) {
            match verdict {
                Verdict::Pass => self.take(typed),
                Verdict::Replace { delete, text, then } => {
                    for _ in 0..delete {
                        self.0.pop();
                    }
                    self.0.extend(text.chars());
                    if then != Then::Drop {
                        self.take(typed);
                    }
                }
            }
        }

        fn text(&self) -> String {
            self.0.iter().collect()
        }
    }

    fn typed(typist: &mut Typist, keys: &str) -> String {
        let mut field = Field::default();
        for c in keys.chars() {
            let verdict = typist.key(press(c));
            field.apply(c, verdict);
        }
        field.text()
    }

    fn telex() -> Typist {
        Typist::new(Options::default())
    }

    #[test]
    fn telex_puts_the_marks() {
        let mut typist = telex();
        for (keys, text) in [
            ("Tieengs Vieetj ", "Tiếng Việt "),
            ("xin chaof ", "xin chào "),
            ("ddeens ", "đến "),
            ("nguwowif ", "người "),
            ("Ddaay laf booj gox ", "Đây là bộ gõ "),
            ("bootj ", "bột "),
        ] {
            typist.forget();
            assert_eq!(typed(&mut typist, keys), text, "{keys:?}");
        }
    }

    #[test]
    fn vni_puts_the_marks_with_digits_and_shifted_digits_stay_symbols() {
        let mut typist = Typist::new(Options {
            method: Method::Vni,
            ..Options::default()
        });
        assert_eq!(typed(&mut typist, "Vie65t Nam "), "Việt Nam ");
        typist.forget();
        assert_eq!(typed(&mut typist, "hoa2 "), "hoà ");
        typist.forget();
        assert_eq!(
            typed(&mut typist, "a@b#"),
            "a@b#",
            "Shift+2 is @, not the grave accent"
        );
    }

    #[test]
    fn the_place_of_the_mark_follows_the_choice() {
        let mut modern = telex();
        assert_eq!(typed(&mut modern, "hoaf khoer "), "hoà khoẻ ");
        let mut old = Typist::new(Options {
            modern_tone: false,
            ..Options::default()
        });
        assert_eq!(typed(&mut old, "hoaf khoer "), "hòa khỏe ");
    }

    #[test]
    fn english_words_come_back_at_the_end_of_the_word() {
        let mut typist = telex();
        for (keys, text) in [
            ("text ", "text "),
            ("with ", "with "),
            ("expect, ", "expect, "),
            ("perfect.", "perfect."),
            ("text\n", "text\n"),
        ] {
            typist.forget();
            assert_eq!(typed(&mut typist, keys), text, "{keys:?}");
        }
        let mut kept = Typist::new(Options {
            english_restore: false,
            ..Options::default()
        });
        assert_eq!(
            typed(&mut kept, "text "),
            "tẽt ",
            "without the choice the marks stay"
        );
    }

    #[test]
    fn a_restored_word_lands_before_the_punctuation_that_ended_it() {
        let mut typist = telex();
        // "ưith" is given back only when the word ends; some words ("text") come back at their last letter.
        assert_eq!(typed(&mut typist, "with"), "ưith");
        let verdict = typist.key(press(','));
        let Verdict::Replace { text, then, .. } = verdict else {
            panic!("the comma ends a word that is given back: {verdict:?}");
        };
        assert_eq!(text, "with");
        assert_eq!(then, Then::Retype, "held back and typed after the word");
    }

    #[test]
    fn caps_lock_types_capitals_with_their_marks() {
        let mut typist = telex();
        let mut field = Field::default();
        for c in "vieetj ".chars() {
            let key = Key {
                caps_lock: true,
                ..press(c)
            };
            let verdict = typist.key(key);
            field.apply(c.to_ascii_uppercase(), verdict);
        }
        assert_eq!(field.text(), "VIỆT ");
    }

    #[test]
    fn backspace_edits_the_word_being_typed() {
        let mut typist = telex();
        // The last letter goes; the marks of what is left stay.
        assert_eq!(typed(&mut typist, "vieetj\u{8}"), "việ");
        typist.forget();
        assert_eq!(typed(&mut typist, "chaof\u{8}\u{8}"), "ch");
    }

    #[test]
    fn a_command_shortcut_ends_the_word() {
        let mut typist = telex();
        let mut field = Field::default();
        for c in "vie".chars() {
            let verdict = typist.key(press(c));
            field.apply(c, verdict);
        }
        let paste = Key {
            command: true,
            ..press('v')
        };
        assert_eq!(typist.key(paste), Verdict::Pass);
        for c in "es".chars() {
            let verdict = typist.key(press(c));
            field.apply(c, verdict);
        }
        // Without the shortcut "viees" would make "viế"; after it the e starts a new word.
        assert_eq!(field.text(), "vieé");
    }

    #[test]
    fn moving_by_word_and_control_end_the_word_too() {
        for interrupt in ["select", "control"] {
            let mut typist = telex();
            let mut field = Field::default();
            for c in "vie".chars() {
                let verdict = typist.key(press(c));
                field.apply(c, verdict);
            }
            if interrupt == "select" {
                let select = Key {
                    code: key::LEFT,
                    shift: true,
                    ..Key::default()
                };
                assert_eq!(typist.key(select), Verdict::Pass);
            } else {
                typist.control_down();
            }
            for c in "ej".chars() {
                let verdict = typist.key(press(c));
                field.apply(c, verdict);
            }
            assert_eq!(field.text(), "vieẹ", "{interrupt}");
        }
    }

    #[test]
    fn option_types_a_symbol_and_the_word_stops_there() {
        let mut typist = telex();
        let mut field = Field::default();
        field.apply('a', typist.key(press('a')));
        let sharp_s = Key {
            option: true,
            ..press('s')
        };
        let verdict = typist.key(sharp_s);
        assert_eq!(verdict, Verdict::Pass, "Option+S types ß");
        field.apply('ß', verdict);
        field.apply('s', typist.key(press('s')));
        assert_eq!(
            field.text(),
            "aßs",
            "no acute accent reaches back over the symbol"
        );
    }

    #[test]
    fn the_shortcut_chord_matches_only_its_own_modifiers() {
        let chord = Chord {
            code: key::SPACE,
            control: true,
            ..Chord::default()
        };
        let space = Key {
            code: key::SPACE,
            ..Key::default()
        };
        assert!(chord.matches(&Key {
            control: true,
            ..space
        }));
        assert!(
            chord.matches(&Key {
                control: true,
                caps_lock: true,
                ..space
            }),
            "Caps Lock does not count"
        );
        assert!(!chord.matches(&space));
        assert!(!chord.matches(&Key {
            control: true,
            shift: true,
            ..space
        }));
        assert!(!chord.matches(&Key {
            command: true,
            ..space
        }));
        assert!(!chord.matches(&Key {
            code: key::TAB,
            control: true,
            ..Key::default()
        }));
    }

    #[test]
    fn other_input_methods_are_known_by_name_or_bundle_identifier() {
        for name in [
            "GoNhanh",
            "Gõ Nhanh",
            "EVKey",
            "OpenKey",
            "GoTiengViet",
            "UniKey",
            "org.gonhanh.GoNhanh",
        ] {
            assert!(is_other_input_method(name), "{name}");
        }
        for name in ["Figghost", "OpenKeychain", "Keychain Access", "Safari", ""] {
            assert!(!is_other_input_method(name), "{name}");
        }
    }

    fn session() -> Session {
        Session::new(Settings {
            options: Options::default(),
            toggle: Some(Chord {
                code: key::SPACE,
                control: true,
                ..Chord::default()
            }),
            excluded: vec!["com.apple.Terminal".into()],
        })
    }

    const NOTES: Focus<'static> = Focus {
        app: Some("com.apple.Notes"),
        role: Some("AXTextArea"),
    };

    /// Types `keys` through a session into `field`, the way an app takes them.
    fn type_into(session: &mut Session, field: &mut Field, focus: Focus<'_>, keys: &str) {
        for c in keys.chars() {
            match session.handle(Event::Key(press(c)), focus, true) {
                Action::Pass => field.take(c),
                Action::Toggle => panic!("{c:?} is not the shortcut"),
                Action::Type {
                    delete, text, then, ..
                } => field.apply(c, Verdict::Replace { delete, text, then }),
            }
        }
    }

    /// The text a new field holds after `keys`.
    fn through(session: &mut Session, focus: Focus<'_>, vietnamese: bool, keys: &str) -> String {
        let mut field = Field::default();
        for c in keys.chars() {
            match session.handle(Event::Key(press(c)), focus, vietnamese) {
                Action::Pass => field.take(c),
                Action::Toggle => panic!("{c:?} is not the shortcut"),
                Action::Type {
                    delete, text, then, ..
                } => field.apply(c, Verdict::Replace { delete, text, then }),
            }
        }
        field.text()
    }

    #[test]
    fn a_session_types_vietnamese_with_the_plan_of_the_app() {
        let mut session = session();
        assert_eq!(through(&mut session, NOTES, true, "Vieejt "), "Việt ");
        session.forget();
        through(&mut session, NOTES, true, "vie");
        let Action::Type { plan, .. } = session.handle(
            Event::Key(press('e')),
            Focus {
                app: Some("com.google.Chrome"),
                role: Some("AXTextField"),
            },
            true,
        ) else {
            panic!("the second e makes ê");
        };
        assert_eq!(plan.delivery, Delivery::EmptyCharPrefix);
    }

    #[test]
    fn english_an_excluded_app_and_another_device_leave_every_key_alone() {
        let mut session = session();
        assert_eq!(through(&mut session, NOTES, false, "Vieejt "), "Vieejt ");
        let terminal = Focus {
            app: Some("com.apple.Terminal"),
            role: Some("AXTextArea"),
        };
        assert_eq!(through(&mut session, terminal, true, "Vieejt "), "Vieejt ");
        let mirroring = Focus {
            app: Some("com.apple.ScreenContinuity"),
            role: None,
        };
        assert_eq!(through(&mut session, mirroring, true, "Vieejt "), "Vieejt ");
    }

    #[test]
    fn the_shortcut_switches_in_both_languages_and_can_be_put_aside() {
        let mut session = session();
        let chord = Key {
            code: key::SPACE,
            control: true,
            ..Key::default()
        };
        for vietnamese in [true, false] {
            assert_eq!(
                session.handle(Event::Key(chord), NOTES, vietnamese),
                Action::Toggle
            );
        }
        // While the settings page records a shortcut there is none, and the key goes on to the page.
        let mut settings = session.settings.clone();
        settings.toggle = None;
        session.configure(settings);
        assert_eq!(session.handle(Event::Key(chord), NOTES, true), Action::Pass);
    }

    #[test]
    fn a_click_forgets_the_word() {
        let mut session = session();
        let mut field = Field::default();
        type_into(&mut session, &mut field, NOTES, "vie");
        assert_eq!(session.handle(Event::Click, NOTES, true), Action::Pass);
        type_into(&mut session, &mut field, NOTES, "ej");
        assert_eq!(
            field.text(),
            "vieẹ",
            "the e after the click starts a new word"
        );
    }

    #[test]
    fn return_in_discord_is_never_held_back() {
        let mut session = session();
        let discord = Focus {
            app: Some("com.hnc.Discord"),
            role: Some("AXTextArea"),
        };
        through(&mut session, discord, true, "text");
        assert_eq!(
            session.handle(Event::Key(press('\n')), discord, true),
            Action::Pass
        );
    }

    #[test]
    fn new_options_take_effect_and_unchanged_ones_keep_the_word() {
        let mut session = session();
        let mut field = Field::default();
        type_into(&mut session, &mut field, NOTES, "vie");
        // The same options again (only the excluded apps changed): the word goes on.
        let mut settings = session.settings.clone();
        settings.excluded.push("com.example".into());
        session.configure(settings.clone());
        type_into(&mut session, &mut field, NOTES, "ej");
        assert_eq!(field.text(), "việ");
        settings.options.method = Method::Vni;
        session.configure(settings);
        type_into(&mut session, &mut field, NOTES, " a6");
        assert_eq!(field.text(), "việ â");
    }
}
