//! Clipboard extras: Copy Stack, pinned boards and sensitive data protection.
//!
//! Copy Stack: a double press of a bare modifier key starts a stack. While it runs, every copy is queued and a
//! small pill shows how many wait; each Command+V puts the oldest queued item on the pasteboard just before the
//! app reads it and lets the key through, and the stack ends when the queue is empty or at another double
//! press. Pinned boards and sensitive data are properties of the history store (`core-clipboard`); this
//! folder holds their settings and the wiring. See `machine` for the rules and `run` for the connections.

use super::AttachEnv;
use crate::{clipboard::Clipboard, history::History, settings::model::Fault};
use core_clipboard::{Error, Sensitivity};
use macos_sys::{
    MainThreadMarker, app, permissions,
    stack::{
        input::{ModifierWatch, Observed, PasteTap},
        pill::Pill,
    },
    status_item::{Entry, Item},
};
use serde::{Deserialize, Deserializer, Serialize};
use serde_json::{Value, json};
use std::{any::Any, cell::RefCell, rc::Rc};

pub mod machine;
mod run;
pub mod settings;
mod strings;

use machine::Modifier;

/// How long a masked item stays: the shortest and the longest the settings offer.
pub const EXPIRE_MINUTES: std::ops::RangeInclusive<u32> = 1..=1440;
const DEFAULT_EXPIRE_MINUTES: u32 = 10;

/// What happens to a copy that looks like a secret.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Protect {
    /// Nothing: it is stored like any other copy.
    #[default]
    Off,
    /// Stored hidden, and deleted after a while.
    Mask,
    /// Not stored at all.
    Never,
}

impl Protect {
    pub const ALL: [Protect; 3] = [Protect::Off, Protect::Mask, Protect::Never];

    pub const fn as_str(self) -> &'static str {
        match self {
            Protect::Off => "off",
            Protect::Mask => "mask",
            Protect::Never => "never",
        }
    }

    pub fn parse(text: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|p| p.as_str() == text)
    }
}

// A value of the wrong type in a hand-edited file falls back to the default instead of making the whole
// config unreadable.
fn lenient_bool<'de, D: Deserializer<'de>>(d: D) -> Result<bool, D::Error> {
    Ok(Value::deserialize(d)?.as_bool().unwrap_or(false))
}

fn lenient_modifier<'de, D: Deserializer<'de>>(d: D) -> Result<Modifier, D::Error> {
    let value = Value::deserialize(d)?;
    Ok(value.as_str().and_then(Modifier::parse).unwrap_or_default())
}

fn lenient_protect<'de, D: Deserializer<'de>>(d: D) -> Result<Protect, D::Error> {
    let value = Value::deserialize(d)?;
    Ok(value.as_str().and_then(Protect::parse).unwrap_or_default())
}

fn lenient_minutes<'de, D: Deserializer<'de>>(d: D) -> Result<u32, D::Error> {
    let value = Value::deserialize(d)?;
    Ok(value
        .as_f64()
        .filter(|n| n.is_finite())
        .map_or(DEFAULT_EXPIRE_MINUTES, clamp_minutes))
}

/// Any number of minutes, brought into the range the settings allow.
pub fn clamp_minutes(minutes: f64) -> u32 {
    (minutes.round() as i64).clamp(
        i64::from(*EXPIRE_MINUTES.start()),
        i64::from(*EXPIRE_MINUTES.end()),
    ) as u32
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    /// Copy Stack. Off by default.
    #[serde(deserialize_with = "lenient_bool")]
    pub enabled: bool,
    /// The key whose double press starts and ends a stack.
    #[serde(deserialize_with = "lenient_modifier")]
    pub modifier: Modifier,
    #[serde(deserialize_with = "lenient_protect")]
    pub sensitive: Protect,
    /// Minutes before a masked item is deleted.
    #[serde(deserialize_with = "lenient_minutes")]
    pub expire_minutes: u32,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            enabled: false,
            modifier: Modifier::default(),
            sensitive: Protect::Off,
            expire_minutes: DEFAULT_EXPIRE_MINUTES,
        }
    }
}

impl Config {
    /// What the history store is told to do with a secret.
    pub fn policy(&self) -> Sensitivity {
        match self.sensitive {
            Protect::Off => Sensitivity::Keep,
            Protect::Mask => Sensitivity::Mask {
                minutes: clamp_minutes(f64::from(self.expire_minutes)),
            },
            Protect::Never => Sensitivity::Skip,
        }
    }
}

/// The running feature. Created by `attach`, kept alive by the app context.
pub struct Feature {
    history: Rc<History>,
    clipboard: Rc<Clipboard>,
    run: Rc<run::Run>,
}

/// The stack's connections to the keyboard, the pasteboard and the screen.
struct MacPorts {
    mtm: MainThreadMarker,
    history: Rc<History>,
    clipboard: Rc<Clipboard>,
    pill: RefCell<Option<Pill>>,
}

impl run::Ports for MacPorts {
    fn trusted(&self) -> bool {
        permissions::accessibility_trusted(false)
    }

    fn watch_keys(&self, on_input: Box<dyn Fn(Observed, f64)>) -> Option<Box<dyn Any>> {
        ModifierWatch::start(on_input).map(|w| Box::new(w) as Box<dyn Any>)
    }

    fn watch_paste(&self, on_paste: Box<dyn Fn()>) -> Option<Box<dyn Any>> {
        PasteTap::start(on_paste).map(|t| Box::new(t) as Box<dyn Any>)
    }

    fn listen_copies(&self, hook: Option<Box<dyn Fn(i64)>>) {
        self.history.set_on_recorded(hook);
    }

    fn stage(&self, id: i64) -> bool {
        self.history.stage(id)
    }

    fn fast_poll(&self, fast: bool) {
        self.clipboard.set_fast_poll(fast);
    }

    fn show(&self, queued: usize) {
        let mut slot = self.pill.borrow_mut();
        let pill = slot.get_or_insert_with(|| Pill::new(self.mtm));
        pill.set_text(&crate::i18n::current().stack_pill(queued));
        pill.show();
    }

    fn hide(&self) {
        if let Some(pill) = &*self.pill.borrow() {
            pill.hide();
        }
    }

    fn defer(&self, f: Box<dyn FnOnce()>) {
        app::after(0.0, f);
    }
}

/// Starts the feature; `None` when there is no clipboard history to work on.
pub fn attach(env: &AttachEnv) -> Option<Rc<Feature>> {
    let clipboard = Rc::clone(env.clipboard?);
    let ports = MacPorts {
        mtm: env.mtm,
        history: Rc::clone(&clipboard.history),
        clipboard: Rc::clone(&clipboard),
        pill: RefCell::new(None),
    };
    let feature = Rc::new(Feature {
        history: Rc::clone(&clipboard.history),
        run: run::Run::new(Rc::new(ports)),
        clipboard,
    });
    feature.apply(&env.config.stack);
    Some(feature)
}

impl Feature {
    fn apply(&self, config: &Config) {
        self.history.set_sensitivity(config.policy());
        self.run.configure(config.enabled, config.modifier);
    }

    /// Applies the settings after a change. An error is shown on the settings page and the change is undone.
    pub fn configure(&self, config: &Config) -> Result<(), Fault> {
        self.apply(config);
        Ok(())
    }

    /// There are no global shortcuts to release.
    pub fn suspend_hotkeys(&self) {}

    pub fn resume_hotkeys(&self, _config: &Config) {}

    /// Facts only the running app knows: the boards, the permission and the state of the stack.
    pub fn runtime(&self) -> Value {
        let boards: Vec<Value> = self
            .history
            .boards()
            .into_iter()
            .map(|b| json!({ "id": b.id, "name": b.name, "items": b.items }))
            .collect();
        json!({
            "boards": boards,
            "accessibility": permissions::accessibility_trusted(false),
            "active": self.run.is_active(),
            "queued": self.run.queued(),
        })
    }

    /// The buttons of the boards card. The name of a board is part of the action, because an action carries
    /// no other argument: `stack_board_add:<name>`, `stack_board_rename:<id>:<name>`, `stack_board_delete:<id>`.
    pub fn action(&self, name: &str, _config: &mut Config) -> Option<Result<(), Fault>> {
        let action = match settings::parse_action(name)? {
            Ok(action) => action,
            Err(fault) => return Some(Err(fault)),
        };
        let outcome = match action {
            settings::BoardAction::Add(board) => self.history.add_board(board).map(drop),
            settings::BoardAction::Rename(id, board) => self.history.rename_board(id, board),
            settings::BoardAction::Delete(id) => self.history.delete_board(id),
        };
        Some(match outcome {
            Ok(()) => {
                self.clipboard.show_boards();
                Ok(())
            }
            Err(Error::Board(e)) => Err(Fault::new(e.code(), "")),
            Err(e) => Err(Fault::new("board_failed", e.to_string())),
        })
    }

    /// Entries for the menu bar item, between "Open clipboard" and the login item.
    pub fn menu_entries(this: &Rc<Self>) -> Vec<Entry> {
        let (end, title) = (Rc::clone(this), Rc::clone(this));
        vec![Entry::Item(
            Item::new(&crate::i18n::current().stack_end_menu(0), move || {
                end.run.finish();
            })
            .symbol(crate::menu::symbols::COPY_STACK)
            .titled(move || crate::i18n::current().stack_end_menu(title.run.queued()))
            .visible_when({
                let this = Rc::clone(this);
                move || this.run.is_active()
            }),
        )]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_are_off_and_do_nothing() {
        let config = Config::default();
        assert!(!config.enabled);
        assert_eq!(config.modifier, Modifier::Option);
        assert_eq!(config.sensitive, Protect::Off);
        assert_eq!(config.expire_minutes, 10);
        assert_eq!(config.policy(), Sensitivity::Keep);
        // A config file written by an older version has no such section at all.
        let loaded: Config = serde_json::from_str("{}").unwrap();
        assert_eq!(loaded, config);
    }

    #[test]
    fn the_policy_follows_the_choice() {
        let mut config = Config {
            sensitive: Protect::Mask,
            expire_minutes: 30,
            ..Config::default()
        };
        assert_eq!(config.policy(), Sensitivity::Mask { minutes: 30 });
        config.sensitive = Protect::Never;
        assert_eq!(config.policy(), Sensitivity::Skip);
    }

    #[test]
    fn a_hand_edited_file_never_makes_the_config_unreadable() {
        for text in [
            r#"{"enabled": "yes", "modifier": 7, "sensitive": null, "expire_minutes": "soon"}"#,
            r#"{"enabled": 1, "modifier": "hyper", "sensitive": "maybe", "expire_minutes": -5}"#,
            r#"{"modifier": ["option"], "sensitive": {"a": 1}, "expire_minutes": 1e99}"#,
        ] {
            let config: Config =
                serde_json::from_str(text).unwrap_or_else(|e| panic!("{text}: {e}"));
            assert!(!config.enabled, "{text}");
            assert_eq!(config.modifier, Modifier::Option, "{text}");
            assert_eq!(config.sensitive, Protect::Off, "{text}");
            assert!(EXPIRE_MINUTES.contains(&config.expire_minutes), "{text}");
        }
        let good: Config = serde_json::from_str(
            r#"{"enabled": true, "modifier": "shift", "sensitive": "mask", "expire_minutes": 45}"#,
        )
        .unwrap();
        assert!(good.enabled);
        assert_eq!(good.modifier, Modifier::Shift);
        assert_eq!(good.sensitive, Protect::Mask);
        assert_eq!(good.expire_minutes, 45);
    }

    #[test]
    fn minutes_are_kept_in_range() {
        assert_eq!(clamp_minutes(0.0), 1);
        assert_eq!(clamp_minutes(-3.0), 1);
        assert_eq!(clamp_minutes(7.4), 7);
        assert_eq!(clamp_minutes(1e12), 1440);
        let loaded: Config = serde_json::from_str(r#"{"expire_minutes": 99999}"#).unwrap();
        assert_eq!(loaded.expire_minutes, 1440);
    }

    #[test]
    fn the_config_round_trips() {
        let config = Config {
            enabled: true,
            modifier: Modifier::Control,
            sensitive: Protect::Never,
            expire_minutes: 5,
        };
        let text = serde_json::to_string(&config).unwrap();
        assert!(
            text.contains("\"control\"") && text.contains("\"never\""),
            "{text}"
        );
        assert_eq!(serde_json::from_str::<Config>(&text).unwrap(), config);
    }
}
