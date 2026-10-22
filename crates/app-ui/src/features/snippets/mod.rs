//! Snippets: type an abbreviation in any app and it is replaced by a longer text. Off by default.
//!
//! The keys are read by the same tap as Vietnamese typing (`macos_sys::typing`), because two taps would each see the
//! keys the other holds back and neither could know what is on the screen. So the keyboard is owned by the typing
//! feature, which keeps it running while either Vietnamese typing or snippets is on; this folder holds the list, its
//! rules, and the page's part of the settings model.

use super::typing;
use crate::settings::model::Fault;
use core_typing::{MAX_SNIPPETS, MAX_TEXT, Snippet, check_abbr};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::rc::Rc;

pub mod settings;

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Item {
    /// What is typed. Typeable on a US keyboard, without spaces, at most 32 characters.
    pub abbr: String,
    /// What replaces it, up to 20000 characters; line breaks are kept.
    pub text: String,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    pub enabled: bool,
    /// In the order of the list on the page. An entry of the wrong shape is dropped when the file is read, not the
    /// whole file.
    #[serde(deserialize_with = "lenient_items")]
    pub items: Vec<Item>,
}

fn lenient_items<'de, D: serde::Deserializer<'de>>(deserializer: D) -> Result<Vec<Item>, D::Error> {
    let value = Value::deserialize(deserializer)?;
    Ok(value
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|item| serde_json::from_value(item.clone()).ok())
        .collect())
}

/// Why an item is refused.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Refusal {
    Empty,
    TooLong,
    Spaces,
    Untypeable,
    NoText,
    TextTooLong,
    Duplicate,
    Full,
}

impl Refusal {
    /// The code the page words in the person's language.
    pub const fn code(self) -> &'static str {
        match self {
            Refusal::Empty => "snippets_empty",
            Refusal::TooLong => "snippets_long",
            Refusal::Spaces => "snippets_space",
            Refusal::Untypeable => "snippets_untypeable",
            Refusal::NoText => "snippets_no_text",
            Refusal::TextTooLong => "snippets_text_long",
            Refusal::Duplicate => "snippets_duplicate",
            Refusal::Full => "snippets_full",
        }
    }

    pub fn fault(self) -> Fault {
        Fault::new(self.code(), "snippets")
    }
}

/// Line breaks as the pasteboard and the apps take them.
fn lf(text: &str) -> String {
    text.replace("\r\n", "\n").replace('\r', "\n")
}

/// Checks one item on its own.
pub fn check(abbr: &str, text: &str) -> Result<(), Refusal> {
    check_abbr(abbr).map_err(|e| match e {
        core_typing::AbbrError::Empty => Refusal::Empty,
        core_typing::AbbrError::TooLong => Refusal::TooLong,
        core_typing::AbbrError::Spaces => Refusal::Spaces,
        core_typing::AbbrError::Untypeable => Refusal::Untypeable,
    })?;
    if text.is_empty() {
        return Err(Refusal::NoText);
    }
    if text.chars().count() > MAX_TEXT {
        return Err(Refusal::TextTooLong);
    }
    Ok(())
}

impl Config {
    /// A config file can say anything; an item that cannot be used is left out, and of two with the same abbreviation
    /// the first stays.
    pub fn sanitized(mut self) -> Self {
        let mut kept: Vec<Item> = Vec::new();
        for item in self.items {
            let text = lf(&item.text);
            if check(&item.abbr, &text).is_ok()
                && kept.len() < MAX_SNIPPETS
                && !kept.iter().any(|k| k.abbr == item.abbr)
            {
                kept.push(Item {
                    abbr: item.abbr,
                    text,
                });
            }
        }
        self.items = kept;
        self
    }

    /// What the keyboard side needs; nothing while the feature is off.
    pub fn list(&self) -> Vec<Snippet> {
        if !self.enabled {
            return Vec::new();
        }
        self.items
            .iter()
            .map(|i| Snippet {
                abbr: i.abbr.clone(),
                text: i.text.clone(),
            })
            .collect()
    }
}

/// The running feature: the page's connection to the keyboard that the typing feature owns.
pub struct Feature {
    typing: Rc<typing::Feature>,
}

pub fn attach(typing: Option<&Rc<typing::Feature>>) -> Option<Rc<Feature>> {
    Some(Rc::new(Feature {
        typing: Rc::clone(typing?),
    }))
}

impl Feature {
    pub fn configure(&self, config: &Config) -> Result<(), Fault> {
        self.typing.set_snippets(config.clone().sanitized().list());
        Ok(())
    }

    /// Facts only the running app knows: whether the keyboard runs, the permission, another input method.
    pub fn runtime(&self) -> Value {
        json!({
            "running": self.typing.is_running(),
            "accessibility": macos_sys::permissions::accessibility_trusted(false),
            "other": self.typing.other(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn item(abbr: &str, text: &str) -> Item {
        Item {
            abbr: abbr.into(),
            text: text.into(),
        }
    }

    #[test]
    fn defaults_are_off_and_empty_and_an_old_file_reads() {
        let config = Config::default();
        assert!(!config.enabled && config.items.is_empty());
        let loaded: Config = serde_json::from_str("{}").unwrap();
        assert_eq!(loaded, config, "a file from before this feature");
    }

    #[test]
    fn a_hand_edited_file_is_brought_back_to_what_works() {
        let config = Config {
            enabled: true,
            items: vec![
                item(";sig", "Best,\r\nAn"),
                item(";sig", "a second with the same abbreviation"),
                item("", "no abbreviation"),
                item(&"x".repeat(33), "too long"),
                item("a b", "a space"),
                item("a\tb", "a tab"),
                item("a\u{7}", "a control character"),
                item("đ", "cannot be typed"),
                item(";empty", ""),
                item(";huge", &"y".repeat(MAX_TEXT + 1)),
                item(&"z".repeat(32), "exactly as long as it may be"),
                item(";cr", "one\rtwo"),
            ],
        }
        .sanitized();
        let abbrs: Vec<&str> = config.items.iter().map(|i| i.abbr.as_str()).collect();
        assert_eq!(abbrs, [";sig", &"z".repeat(32), ";cr"]);
        assert_eq!(
            config.items[0].text, "Best,\nAn",
            "the first one stays, with LF line breaks"
        );
        assert_eq!(config.items[2].text, "one\ntwo");
        let many = Config {
            items: (0..MAX_SNIPPETS + 20)
                .map(|i| item(&format!(";k{i}"), "text"))
                .collect(),
            ..Config::default()
        }
        .sanitized();
        assert_eq!(many.items.len(), MAX_SNIPPETS);
        let longest = Config {
            items: vec![item(";ok", &"y".repeat(MAX_TEXT))],
            ..Config::default()
        }
        .sanitized();
        assert_eq!(longest.items.len(), 1, "the longest text allowed stays");
    }

    #[test]
    fn a_wrong_value_in_the_file_does_not_lose_the_rest() {
        let loaded: Config = serde_json::from_str(
            r#"{"enabled": true, "items": [
                {"abbr": ";a", "text": "A"},
                {"abbr": 5, "text": "wrong type"},
                "not an object",
                {"abbr": ";b"},
                {"abbr": ";c", "text": "C", "extra": 1},
                null
            ]}"#,
        )
        .unwrap();
        assert!(loaded.enabled);
        let loaded = loaded.sanitized();
        let abbrs: Vec<&str> = loaded.items.iter().map(|i| i.abbr.as_str()).collect();
        assert_eq!(
            abbrs,
            [";a", ";c"],
            ";b has no text, the others are not items"
        );
        let broken: Config = serde_json::from_str(r#"{"items": "none"}"#).unwrap();
        assert!(broken.items.is_empty());
    }

    #[test]
    fn the_keyboard_gets_the_list_only_while_the_feature_is_on() {
        let mut config = Config {
            items: vec![item(";a", "A")],
            ..Config::default()
        };
        assert!(config.list().is_empty());
        config.enabled = true;
        assert_eq!(
            config.list(),
            [Snippet {
                abbr: ";a".into(),
                text: "A".into()
            }]
        );
    }

    #[test]
    fn every_refusal_has_its_own_code() {
        let all = [
            Refusal::Empty,
            Refusal::TooLong,
            Refusal::Spaces,
            Refusal::Untypeable,
            Refusal::NoText,
            Refusal::TextTooLong,
            Refusal::Duplicate,
            Refusal::Full,
        ];
        let mut codes: Vec<&str> = all.iter().map(|r| r.code()).collect();
        codes.sort_unstable();
        codes.dedup();
        assert_eq!(codes.len(), all.len());
        assert_eq!(check(";a", "A"), Ok(()));
        assert_eq!(check("", "A"), Err(Refusal::Empty));
        assert_eq!(check(";a", ""), Err(Refusal::NoText));
    }
}
