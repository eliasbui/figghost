//! Snippets: an abbreviation typed in any app is taken back and the longer text put in its place.
//!
//! The abbreviation is recognised by the keys that were pressed, not by what is on the screen. With Vietnamese typing
//! on, the keys of `;dd` leave `;đ` there, two characters for three keys, and what must be deleted is what those keys
//! put on the screen. So the matcher follows, for every key, how the screen changed under it (the verdict the typist
//! gave), and an expansion deletes exactly the characters that the keys of the abbreviation left, no more.

use crate::{Key, Then, Verdict, key};
use std::collections::VecDeque;

/// The most characters an abbreviation may have.
pub const MAX_ABBR: usize = 32;
/// The most characters a text may have.
pub const MAX_TEXT: usize = 20_000;
/// The most snippets there may be.
pub const MAX_SNIPPETS: usize = 500;
/// A text of one line up to this long is typed; any other is pasted, since only a paste breaks lines without pressing
/// Return, and a long text typed key by key holds the keyboard back for seconds.
const TYPED_LIMIT: usize = 100;

/// The characters of the keys of an ANSI keyboard, plain and with Shift, by virtual key code. The engine of Vietnamese
/// typing reads the keys by their place on the keyboard too, so this is the same promise: a US layout.
const PLAIN: &str = "asdfhgzxcv\0bqweryt123465=97-80]ou[ip\0lj'k;\\,/nm.\0\0`";
const SHIFTED: &str = "ASDFHGZXCV\0BQWERYT!@#$^%+(&_*)}OU{IP\0LJ\"K:|<?NM>\0\0~";

fn char_at(table: &str, code: u16) -> Option<char> {
    table.chars().nth(usize::from(code)).filter(|c| *c != '\0')
}

/// The character a key types, if it types one that an abbreviation may hold. Caps Lock capitalises letters only.
pub fn char_of(key: &Key) -> Option<char> {
    let plain = char_at(PLAIN, key.code)?;
    let shifted = if plain.is_ascii_lowercase() {
        key.shift != key.caps_lock
    } else {
        key.shift
    };
    if shifted {
        char_at(SHIFTED, key.code)
    } else {
        Some(plain)
    }
}

fn is_typeable(c: char) -> bool {
    PLAIN.contains(c) && c != '\0' || SHIFTED.contains(c) && c != '\0'
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AbbrError {
    Empty,
    TooLong,
    /// A space, a line break or another control character.
    Spaces,
    /// A character no key of a US keyboard types, so that the abbreviation could never be matched.
    Untypeable,
}

/// Whether `abbr` is something that can be typed and matched.
pub fn check_abbr(abbr: &str) -> Result<(), AbbrError> {
    if abbr.is_empty() {
        return Err(AbbrError::Empty);
    }
    if abbr.chars().count() > MAX_ABBR {
        return Err(AbbrError::TooLong);
    }
    if abbr.chars().any(|c| c.is_whitespace() || c.is_control()) {
        return Err(AbbrError::Spaces);
    }
    if !abbr.chars().all(is_typeable) {
        return Err(AbbrError::Untypeable);
    }
    Ok(())
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Snippet {
    pub abbr: String,
    pub text: String,
}

/// What to do when the last key of an abbreviation is pressed: hold that key back, delete `delete` characters before
/// the cursor and put `text` there.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Expansion {
    pub delete: usize,
    pub text: String,
    /// Whether the text is pasted instead of typed.
    pub paste: bool,
}

/// One key pressed since the buffer began, and the length of the screen (counted from where the buffer began, so it
/// may be negative) before it, the lowest it was brought to while the key took effect, and after it.
struct Typed {
    ch: char,
    before: isize,
    low: isize,
    after: isize,
}

/// The keys typed lately and what they did to the screen, matched against the abbreviations.
#[derive(Default)]
pub struct Matcher {
    /// Longest abbreviation first, so that `;sig` wins over `sig` when both end the keys.
    snippets: Vec<Snippet>,
    keys: VecDeque<Typed>,
    len: isize,
    longest: usize,
}

impl Matcher {
    pub fn set(&mut self, mut snippets: Vec<Snippet>) {
        snippets.sort_by_key(|s| std::cmp::Reverse(s.abbr.chars().count()));
        self.longest = snippets.first().map_or(0, |s| s.abbr.chars().count());
        self.snippets = snippets;
        self.reset();
    }

    /// The cursor or the focus moved, or something else happened that the keys cannot follow.
    pub fn reset(&mut self) {
        self.keys.clear();
        self.len = 0;
    }

    /// Follows one key press. `verdict` is what the typist decided for it: what will reach the screen if it is
    /// let through. Returns the expansion when this key completes an abbreviation; the caller then drops the key
    /// and the verdict, and forgets the typist's word.
    pub fn key(&mut self, key: &Key, verdict: &Verdict) -> Option<Expansion> {
        if self.snippets.is_empty() {
            return None;
        }
        if key.command || key.control || key.option {
            self.reset();
            return None;
        }
        if key.code == key::DELETE {
            self.backspace(verdict);
            return None;
        }
        let Some(ch) = char_of(key) else {
            // Space, Return, Tab, an arrow, Escape: no abbreviation holds them, and the cursor may have moved.
            self.reset();
            return None;
        };
        let (delete, landed) = match verdict {
            Verdict::Pass => (0, 1),
            Verdict::Replace { delete, text, then } => (
                *delete,
                text.chars().count() + usize::from(*then != Then::Drop),
            ),
        };
        let before = self.len;
        let low = before - delete as isize;
        self.keys.push_back(Typed {
            ch,
            before,
            low,
            after: low + landed as isize,
        });
        if let Some(found) = self.find() {
            self.reset();
            return Some(found);
        }
        self.len = low + landed as isize;
        while self.keys.len() > self.longest {
            self.keys.pop_front();
        }
        None
    }

    /// Backspace takes back the last key if that key only added a character; anything the typist did to the word
    /// (a mark taken off) is more than the keys can be taken back by, so the buffer starts again.
    fn backspace(&mut self, verdict: &Verdict) {
        let plain = match verdict {
            Verdict::Pass => true,
            Verdict::Replace {
                delete: 1,
                text,
                then: Then::Drop,
            } => text.is_empty(),
            Verdict::Replace { .. } => false,
        };
        match self.keys.back() {
            Some(last) if plain && last.after == last.before + 1 && last.after == self.len => {
                self.len = last.before;
                self.keys.pop_back();
            }
            _ => self.reset(),
        }
    }

    fn find(&self) -> Option<Expansion> {
        let n = self.keys.len();
        for snippet in &self.snippets {
            let size = snippet.abbr.chars().count();
            if size > n {
                continue;
            }
            let start = n - size;
            let window = self.keys.range(start..);
            if !window.clone().map(|k| k.ch).eq(snippet.abbr.chars()) {
                continue;
            }
            let first = self.keys[start].before;
            // A key that took characters from before the abbreviation changed what was typed earlier: deleting
            // by count would eat it, and nothing here can put it back.
            if window.take(size - 1).any(|k| k.low < first) {
                continue;
            }
            let Ok(delete) = usize::try_from(self.keys[n - 1].before - first) else {
                continue;
            };
            return Some(Expansion {
                delete,
                text: snippet.text.clone(),
                paste: snippet.text.contains('\n') || snippet.text.chars().count() > TYPED_LIMIT,
            });
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_tables_line_up_with_the_key_codes() {
        let key = |code| Key {
            code,
            ..Key::default()
        };
        assert_eq!(char_of(&key(key::SEMICOLON)), Some(';'));
        assert_eq!(char_of(&key(key::QUOTE)), Some('\''));
        assert_eq!(char_of(&key(key::GRAVE)), Some('`'));
        assert_eq!(char_of(&key(key::DIGITS[0])), Some('1'));
        assert_eq!(char_of(&key(key::DIGITS[9])), Some('0'));
        assert_eq!(char_of(&key(0)), Some('a'));
        assert_eq!(char_of(&key(key::SPACE)), None);
        assert_eq!(char_of(&key(key::RETURN)), None);
        assert_eq!(char_of(&key(key::LEFT)), None);
        let shift = Key {
            shift: true,
            ..key(key::SEMICOLON)
        };
        assert_eq!(char_of(&shift), Some(':'));
        let caps = Key {
            caps_lock: true,
            ..key(0)
        };
        assert_eq!(char_of(&caps), Some('A'));
        assert_eq!(
            char_of(&Key {
                caps_lock: true,
                shift: true,
                ..key(0)
            }),
            Some('a')
        );
        assert_eq!(
            char_of(&Key {
                caps_lock: true,
                ..key(key::SEMICOLON)
            }),
            Some(';'),
            "Caps Lock does not shift a symbol"
        );
    }

    #[test]
    fn an_abbreviation_is_checked() {
        assert_eq!(check_abbr(";sig"), Ok(()));
        assert_eq!(check_abbr("Addr_1"), Ok(()));
        assert_eq!(check_abbr(""), Err(AbbrError::Empty));
        assert_eq!(check_abbr(&"a".repeat(MAX_ABBR)), Ok(()));
        assert_eq!(
            check_abbr(&"a".repeat(MAX_ABBR + 1)),
            Err(AbbrError::TooLong)
        );
        for bad in [";a b", " ;a", ";a\t", ";a\n", ";a\u{7}", ";a\u{a0}"] {
            assert_eq!(check_abbr(bad), Err(AbbrError::Spaces), "{bad:?}");
        }
        for bad in ["\u{111}", ";\u{e9}", "\u{2192}x", "\u{1F600}"] {
            assert_eq!(check_abbr(bad), Err(AbbrError::Untypeable), "{bad:?}");
        }
    }

    /// The key that types `c` on an ANSI keyboard. Backspace is `\u{8}`.
    fn press(c: char) -> Key {
        if c == '\u{8}' || c == ' ' {
            return Key {
                code: if c == ' ' { key::SPACE } else { key::DELETE },
                ..Key::default()
            };
        }
        let code = (0..=50u16)
            .find(|&code| char_at(PLAIN, code) == Some(c) || char_at(SHIFTED, code) == Some(c))
            .unwrap_or_else(|| panic!("no key types {c:?}"));
        Key {
            code,
            shift: char_at(SHIFTED, code) == Some(c) && char_at(PLAIN, code) != Some(c),
            ..Key::default()
        }
    }

    fn matcher(pairs: &[(&str, &str)]) -> Matcher {
        let mut matcher = Matcher::default();
        matcher.set(
            pairs
                .iter()
                .map(|(abbr, text)| Snippet {
                    abbr: (*abbr).into(),
                    text: (*text).into(),
                })
                .collect(),
        );
        matcher
    }

    /// Keys with no typist: every one lands as it is.
    fn plain(matcher: &mut Matcher, keys: &str) -> Option<Expansion> {
        keys.chars()
            .fold(None, |_, c| matcher.key(&press(c), &Verdict::Pass))
    }

    fn expansion(delete: usize, text: &str, paste: bool) -> Option<Expansion> {
        Some(Expansion {
            delete,
            text: text.into(),
            paste,
        })
    }

    #[test]
    fn the_last_key_of_an_abbreviation_expands_it() {
        let mut m = matcher(&[(";sig", "Best,\nAn")]);
        assert_eq!(plain(&mut m, ";si"), None);
        assert_eq!(
            plain(&mut m, "g"),
            expansion(3, "Best,\nAn", true),
            "the held key is not on the screen, so three are deleted"
        );
        // The buffer is empty after an expansion: the text typed in is not matched again.
        assert_eq!(plain(&mut m, "sig"), None);
        assert_eq!(plain(&mut m, ";sig"), expansion(3, "Best,\nAn", true));
        // Anywhere in a stream of keys, not only at the start of a word.
        assert_eq!(plain(&mut m, "hello;sig"), expansion(3, "Best,\nAn", true));
    }

    #[test]
    fn a_short_single_line_text_is_typed_and_any_other_is_pasted() {
        let long = "x".repeat(TYPED_LIMIT + 1);
        let mut m = matcher(&[(";a", "me@example.com"), (";b", &long), (";c", "one\ntwo")]);
        assert_eq!(plain(&mut m, ";a"), expansion(1, "me@example.com", false));
        assert_eq!(plain(&mut m, ";b"), expansion(1, &long, true));
        assert_eq!(plain(&mut m, ";c"), expansion(1, "one\ntwo", true));
    }

    #[test]
    fn what_is_not_an_abbreviation_resets_the_buffer() {
        let mut m = matcher(&[(";sig", "x")]);
        let reset = |m: &mut Matcher, key: Key| {
            assert_eq!(plain(m, ";si"), None);
            assert_eq!(m.key(&key, &Verdict::Pass), None);
            assert_eq!(plain(m, "g"), None, "{key:?}");
        };
        for code in [
            key::LEFT,
            key::RIGHT,
            key::UP,
            key::DOWN,
            key::SPACE,
            key::RETURN,
            key::TAB,
            key::ESCAPE,
            key::HOME,
        ] {
            reset(
                &mut m,
                Key {
                    code,
                    ..Key::default()
                },
            );
        }
        for modified in [
            Key {
                command: true,
                ..press('v')
            },
            Key {
                control: true,
                ..press('a')
            },
            Key {
                option: true,
                ..press('s')
            },
        ] {
            reset(&mut m, modified);
        }
        assert_eq!(plain(&mut m, ";si"), None);
        m.reset();
        assert_eq!(plain(&mut m, "g"), None, "a click, a new app");
        assert_eq!(plain(&mut m, ";sig"), expansion(3, "x", false));
    }

    #[test]
    fn backspace_takes_back_the_last_key() {
        let mut m = matcher(&[(";sig", "x")]);
        assert_eq!(plain(&mut m, ";sa\u{8}ig"), expansion(3, "x", false));
        assert_eq!(plain(&mut m, ";sa\u{8}\u{8}sig"), expansion(3, "x", false));
        // Past what was typed, the buffer is empty and the abbreviation cannot be completed.
        assert_eq!(plain(&mut m, ";s\u{8}\u{8}\u{8}ig"), None);
        // A backspace that did more than take one character away starts the buffer again.
        let mut m = matcher(&[(";sig", "x")]);
        assert_eq!(plain(&mut m, ";si"), None);
        let marks = Verdict::Replace {
            delete: 2,
            text: "í".into(),
            then: Then::Drop,
        };
        assert_eq!(m.key(&press('\u{8}'), &marks), None);
        assert_eq!(plain(&mut m, "g"), None);
    }

    #[test]
    fn the_longer_abbreviation_wins_over_one_that_is_its_tail() {
        let mut m = matcher(&[("sig", "short"), (";sig", "long")]);
        assert_eq!(plain(&mut m, ";sig"), expansion(3, "long", false));
        assert_eq!(plain(&mut m, "sig"), expansion(2, "short", false));
        assert_eq!(plain(&mut m, "asig"), expansion(2, "short", false));
        // Whichever order they were listed in.
        let mut m = matcher(&[(";sig", "long"), ("sig", "short")]);
        assert_eq!(plain(&mut m, ";sig"), expansion(3, "long", false));
        // The shorter one that is the head of the longer fires first: the longer can never be typed in full.
        let mut m = matcher(&[(";s", "short"), (";sig", "long")]);
        assert_eq!(plain(&mut m, ";s"), expansion(1, "short", false));
    }

    #[test]
    fn abbreviations_are_matched_by_the_characters_typed_with_their_case() {
        let mut m = matcher(&[(";Sig", "x"), (";a:", "y"), ("é", "z")]);
        assert_eq!(plain(&mut m, ";sig"), None);
        assert_eq!(plain(&mut m, ";Sig"), expansion(3, "x", false));
        assert_eq!(plain(&mut m, ";a:"), expansion(2, "y", false));
        // Nothing types it, so it never matches and never breaks the others.
        assert_eq!(plain(&mut m, "e"), None);
    }

    #[test]
    fn nothing_happens_without_snippets() {
        let mut m = matcher(&[]);
        assert_eq!(plain(&mut m, ";sig"), None);
        assert!(m.keys.is_empty());
    }

    /// A screen the typist writes to, so that the matcher is tried on what a person would see.
    #[derive(Default)]
    struct Screen(Vec<char>);

    impl Screen {
        fn text(&self) -> String {
            self.0.iter().collect()
        }

        /// Types `keys` through a typist and the matcher, the way the tap does: the typist decides, the matcher
        /// may replace its decision, and the screen shows the result.
        fn type_keys(&mut self, typist: &mut crate::Typist, matcher: &mut Matcher, keys: &str) {
            for c in keys.chars() {
                let key = press(c);
                let verdict = typist.key(key);
                if let Some(e) = matcher.key(&key, &verdict) {
                    typist.forget();
                    for _ in 0..e.delete {
                        self.0.pop();
                    }
                    self.0.extend(e.text.chars());
                    continue;
                }
                match verdict {
                    Verdict::Pass if c == '\u{8}' => {
                        self.0.pop();
                    }
                    Verdict::Pass => self.0.push(c),
                    Verdict::Replace { delete, text, then } => {
                        for _ in 0..delete {
                            self.0.pop();
                        }
                        self.0.extend(text.chars());
                        if then != Then::Drop {
                            self.0.push(c);
                        }
                    }
                }
            }
        }
    }

    fn telex() -> crate::Typist {
        crate::Typist::new(crate::Options::default())
    }

    #[test]
    fn telex_marks_do_not_make_the_deletion_eat_what_stands_before() {
        // Typed `;dd`, the screen shows `;đ`: two characters for the three keys.
        let mut typist = telex();
        let mut m = matcher(&[(";dd", "do not disturb")]);
        let mut screen = Screen::default();
        screen.type_keys(&mut typist, &mut m, "xin ");
        // Space is not a word break the matcher needs: it resets, and the abbreviation starts after it.
        screen.type_keys(&mut typist, &mut m, ";d");
        assert_eq!(screen.text(), "xin ;d");
        screen.type_keys(&mut typist, &mut m, "d");
        assert_eq!(
            screen.text(),
            "xin do not disturb",
            "nothing before it was eaten"
        );
    }

    #[test]
    fn telex_with_a_letter_right_before_the_abbreviation() {
        // The letters before are one word with the abbreviation's keys: no break between them.
        let mut typist = telex();
        let mut m = matcher(&[("dd", "X")]);
        let mut screen = Screen::default();
        screen.type_keys(&mut typist, &mut m, "an");
        screen.type_keys(&mut typist, &mut m, "dd");
        assert_eq!(
            screen.text(),
            "anX",
            "the first d is deleted, the n and a stand"
        );
    }

    #[test]
    fn a_key_that_rewrites_what_came_before_the_abbreviation_is_not_an_expansion() {
        // The first `s` puts an acute accent on the `o` typed before the abbreviation. Deleting the abbreviation by
        // count would delete that `ó` too, and nothing could give the `o` back; so it is not an expansion.
        let mut without = Screen::default();
        without.type_keys(&mut telex(), &mut matcher(&[]), "toss");
        let mut with = Screen::default();
        with.type_keys(&mut telex(), &mut matcher(&[("ss", "SNIPPET")]), "toss");
        assert_eq!(with.text(), without.text());
        assert!(!with.text().contains("SNIPPET"));
        // The key that completes an abbreviation is held back and its own rewrite never happens: `o` stays.
        let mut last = Screen::default();
        last.type_keys(&mut telex(), &mut matcher(&[("s", "S")]), "tos");
        assert_eq!(last.text(), "toS");
    }

    #[test]
    fn telex_marks_inside_the_abbreviation_and_the_text_after() {
        let mut typist = telex();
        let mut m = matcher(&[(";viet", "Việt Nam"), (";ee", "ê")]);
        let mut screen = Screen::default();
        screen.type_keys(&mut typist, &mut m, "a ;viet");
        assert_eq!(screen.text(), "a Việt Nam");
        // The text put in is not read by the typist: the next word starts clean.
        screen.type_keys(&mut typist, &mut m, " ;ee");
        assert_eq!(screen.text(), "a Việt Nam ê");
        // Keys that make a mark inside the abbreviation: `ee` is `ê`, one character for two keys.
        screen.type_keys(&mut typist, &mut m, " b;ee");
        assert_eq!(screen.text(), "a Việt Nam ê bê");
    }

    #[test]
    fn telex_backspace_inside_the_abbreviation() {
        let mut typist = telex();
        let mut m = matcher(&[(";dd", "T")]);
        let mut screen = Screen::default();
        // `x` is taken back; the abbreviation is typed again and the text stands.
        screen.type_keys(&mut typist, &mut m, "k;dx\u{8}d");
        assert_eq!(screen.text(), "kT");
    }

    #[test]
    fn telex_keys_that_make_marks_inside_a_longer_abbreviation() {
        let mut typist = telex();
        let mut m = matcher(&[(";dd", "Do not disturb"), (";addr", "12 Le Loi")]);
        let mut screen = Screen::default();
        // `dd` is `đ` and `r` puts a hook on the vowel: the keys of `;addr` rewrite the screen twice.
        screen.type_keys(&mut typist, &mut m, "Vieetj ;addr");
        assert_eq!(screen.text(), "Việt 12 Le Loi");
        screen.type_keys(&mut typist, &mut m, " ;dd Vieetj ");
        assert_eq!(screen.text(), "Việt 12 Le Loi Do not disturb Việt ");
    }

    #[test]
    fn telex_backspace_takes_back_a_plain_key_but_not_a_mark() {
        let mut m = matcher(&[(";sig", "SIG")]);
        let mut screen = Screen::default();
        screen.type_keys(&mut telex(), &mut m, ";sg\u{8}ig");
        assert_eq!(
            screen.text(),
            "SIG",
            "the key taken back is not part of the abbreviation"
        );
        let mut screen = Screen::default();
        screen.type_keys(&mut telex(), &mut m, "x;sg\u{8}\u{8}sig");
        assert_eq!(screen.text(), "xSIG");
        let mut screen = Screen::default();
        screen.type_keys(&mut telex(), &mut m, ";sgg\u{8}ig");
        assert_eq!(screen.text(), ";sgig", "`;sgig` is not `;sig`");
        // A backspace over a key that made a mark did more than take one key back: the buffer starts again, and an
        // abbreviation typed around it is not expanded.
        let mut m = matcher(&[(";addr", "12 Le Loi")]);
        let mut screen = Screen::default();
        screen.type_keys(&mut telex(), &mut m, ";add\u{8}r");
        assert!(!screen.text().contains("12 Le Loi"), "{:?}", screen.text());
    }
}
