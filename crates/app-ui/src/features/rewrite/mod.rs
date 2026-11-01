//! Rewrite: select text in any app, press the shortcut, and a Liquid Glass window next to the pointer shows the text
//! fixed, made more formal, made shorter or put into English, written by the model that Jarvis is set to ask. Return puts
//! the result in the clipboard, or, when the person chose to replace, pastes it over the selection.
//!
//! Turning the feature on is the consent to send the selected text to the service of that model (or to the model on this
//! Mac when Jarvis is set to one), over https; nothing else is sent and nothing is kept. It is off until turned on.

use super::{
    AttachEnv, jarvis,
    smart::{Problem, read_selection},
    translate::material_of,
};
use crate::{config::parse_hotkey, history::History, i18n, settings::model::Fault, trace};
use core_jarvis::{Cancel, Failure, Message};
use macos_sys::{
    MainThreadMarker, app,
    hotkey::Hotkey,
    notch::Material,
    pasteboard::{Pasteboard, Write},
    permissions,
    status_item::{Entry, Item},
    synth,
    translate_ui::TranslatePopup,
};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::{
    cell::{Cell, RefCell},
    rc::{Rc, Weak},
};

pub mod settings;
mod text;

pub const DEFAULT_SHORTCUT: &str = "ctrl+opt+cmd+r";
/// The most characters sent at a time.
pub const MAX_CHARS: usize = 4000;
/// How long an answer may take.
const ANSWER_SECONDS: u64 = 60;
/// How long after the paste the person's clipboard is put back.
const RESTORE_SECONDS: f64 = 0.6;
/// Time for the window to give the keyboard back to the app it came from.
const FOCUS_SECONDS: f64 = 0.2;
const SAMPLE: &str =
    "i has went to the market yesterday and buyed some apple, but they was too expensive";

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    pub enabled: bool,
    pub shortcut: String,
    /// `fix`, `formal`, `short` or `english`.
    pub mode: String,
    /// Return pastes the result over the selection instead of copying it.
    pub replace: bool,
    /// The glass of the window: `glass`, `clear`, `ink` or `accent`.
    pub material: String,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            enabled: false,
            shortcut: DEFAULT_SHORTCUT.into(),
            mode: "fix".into(),
            replace: false,
            material: "glass".into(),
        }
    }
}

impl Config {
    /// A config file can say anything; whatever is unusable is replaced by the default.
    pub fn sanitized(mut self) -> Self {
        let defaults = Self::default();
        if !self.shortcut.is_empty() && parse_hotkey(&self.shortcut).is_err() {
            self.shortcut = defaults.shortcut;
        }
        if !text::MODES.contains(&self.mode.as_str()) {
            self.mode = defaults.mode;
        }
        if material_of(&self.material).is_none() {
            self.material = defaults.material;
        }
        self
    }
}

thread_local! {
    static FEATURE: RefCell<Weak<Feature>> = const { RefCell::new(Weak::new()) };
}

fn with_feature(f: impl FnOnce(&Feature)) {
    FEATURE.with(|slot| {
        if let Some(feature) = slot.borrow().upgrade() {
            f(&feature);
        }
    });
}

pub struct Feature {
    mtm: MainThreadMarker,
    popup: TranslatePopup,
    config: RefCell<Config>,
    hotkey: RefCell<Option<Hotkey>>,
    history: Option<Rc<History>>,
    jarvis: Option<Weak<jarvis::Feature>>,
    /// Counts the texts shown, so that a late answer to an earlier one is dropped.
    generation: Cell<u64>,
    /// Stops the question that is on its way when the window closes or another text comes.
    cancel: RefCell<Cancel>,
    me: Weak<Feature>,
}

/// Starts the feature, with the Jarvis whose model it asks; the global shortcut is registered when it is on.
pub fn attach(env: &AttachEnv, jarvis: Option<&Rc<jarvis::Feature>>) -> Option<Rc<Feature>> {
    let config = env.config.rewrite.clone().sanitized();
    let history = env.clipboard.map(|c| Rc::clone(&c.history));
    let feature = Rc::new_cyclic(|me| Feature {
        mtm: env.mtm,
        popup: TranslatePopup::new(env.mtm, |chosen| with_feature(|f| f.chosen(&chosen))),
        config: RefCell::new(config.clone()),
        hotkey: RefCell::new(None),
        history,
        jarvis: jarvis.map(Rc::downgrade),
        generation: Cell::new(0),
        cancel: RefCell::default(),
        me: me.clone(),
    });
    FEATURE.with(|slot| *slot.borrow_mut() = Rc::downgrade(&feature));
    if let Err(fault) = feature.configure(&config) {
        eprintln!("rewrite: cannot register its shortcut ({})", fault.detail);
    }
    Some(feature)
}

impl Feature {
    /// Applies the settings: registers or releases the shortcut.
    pub fn configure(&self, config: &Config) -> Result<(), Fault> {
        *self.config.borrow_mut() = config.clone();
        self.popup
            .set_material(material_of(&config.material).unwrap_or(Material::Glass));
        self.hotkey.borrow_mut().take();
        if !config.enabled {
            self.stop();
            self.popup.hide();
            return Ok(());
        }
        self.register_hotkey(config)
    }

    fn register_hotkey(&self, config: &Config) -> Result<(), Fault> {
        if config.shortcut.is_empty() {
            return Ok(());
        }
        let spec = parse_hotkey(&config.shortcut).map_err(|e| Fault::new("invalid_shortcut", e))?;
        let weak = self.me.clone();
        let hotkey = Hotkey::register(self.mtm, spec.key_code, spec.modifiers, move || {
            if let Some(feature) = weak.upgrade() {
                feature.trigger();
            }
        })
        .map_err(|status| Fault::new("taken", format!("rewrite (status {status})")))?;
        *self.hotkey.borrow_mut() = Some(hotkey);
        Ok(())
    }

    pub fn suspend_hotkeys(&self) {
        self.hotkey.borrow_mut().take();
    }

    pub fn resume_hotkeys(&self, config: &Config) {
        if config.enabled
            && let Err(fault) = self.register_hotkey(config)
        {
            eprintln!(
                "rewrite: cannot register its shortcut again ({})",
                fault.detail
            );
        }
    }

    pub fn runtime(&self) -> Value {
        let ready = self
            .jarvis
            .as_ref()
            .and_then(Weak::upgrade)
            .map(|j| j.lend().map(|_| ()));
        json!({
            "accessibility": permissions::accessibility_trusted(false),
            "model": match ready {
                Some(Ok(())) => json!({ "ready": true, "note": "" }),
                Some(Err(why)) => json!({ "ready": false, "note": text::needs_jarvis(i18n::current(), &why) }),
                None => json!({ "ready": false, "note": "" }),
            },
        })
    }

    /// The "Try it" button of the settings page.
    pub fn action(&self, name: &str) -> Option<Result<(), Fault>> {
        if name != "rewrite_try" {
            return None;
        }
        self.start(SAMPLE.to_owned());
        Some(Ok(()))
    }

    pub fn menu_entries(this: &Rc<Self>) -> Vec<Entry> {
        let (open, visible) = (Rc::downgrade(this), Rc::downgrade(this));
        let item = Item::new(text::menu_title(i18n::current()), move || {
            // The menu has to be gone, and the app that was in front has to have its focus back, before its
            // selection can be read.
            let weak = open.clone();
            app::after(0.25, move || {
                if let Some(feature) = weak.upgrade() {
                    feature.trigger();
                }
            });
        })
        .symbol(crate::menu::symbols::TRANSLATE)
        .visible_when(move || visible.upgrade().is_some_and(|f| f.config.borrow().enabled));
        vec![Entry::Item(item)]
    }

    /// The shortcut, the menu and the settings all come here: closes the window if it is open, otherwise reads the
    /// selection and starts on it.
    pub fn trigger(&self) {
        if self.popup.is_visible() {
            self.stop();
            self.popup.hide();
            return;
        }
        let language = i18n::current();
        match read_selection(self.history.as_ref()) {
            Ok(selected) => self.start(selected),
            Err(problem) => {
                self.generation.set(self.generation.get() + 1);
                let message = match problem {
                    Problem::Accessibility => language.smart_needs_accessibility().to_owned(),
                    Problem::NotRestorable => language.smart_clipboard_not_restorable().to_owned(),
                    Problem::Nothing => language.smart_nothing_selected().to_owned(),
                };
                self.popup.show_message(
                    text::menu_title(language),
                    &message,
                    language.smart_footer_message(),
                );
            }
        }
    }

    /// Stops the question on its way, if any.
    fn stop(&self) {
        self.cancel
            .borrow()
            .store(true, std::sync::atomic::Ordering::SeqCst);
        self.generation.set(self.generation.get() + 1);
    }

    fn start(&self, selected: String) {
        self.stop();
        let generation = self.generation.get();
        let language = i18n::current();
        let config = self.config.borrow().clone();
        let header = text::header(language, &config.mode);
        let footer = text::footer(language, config.replace);
        let selected = selected.trim().to_owned();
        if selected.chars().count() > MAX_CHARS {
            self.popup.show_message(
                &header,
                &text::too_long(language, MAX_CHARS),
                language.smart_footer_message(),
            );
            return;
        }
        if selected.is_empty() {
            self.popup.show_message(
                &header,
                language.smart_nothing_selected(),
                language.smart_footer_message(),
            );
            return;
        }
        let lent = match self
            .jarvis
            .as_ref()
            .and_then(Weak::upgrade)
            .map(|j| j.lend())
        {
            Some(Ok(lent)) => lent,
            Some(Err(why)) => {
                self.popup.show_message(
                    &header,
                    &text::needs_jarvis(language, &why),
                    language.smart_footer_message(),
                );
                return;
            }
            None => {
                self.popup.show_message(
                    &header,
                    &text::needs_jarvis(language, ""),
                    language.smart_footer_message(),
                );
                return;
            }
        };
        trace::log(&format!(
            "rewrite: {} characters, {}",
            selected.chars().count(),
            config.mode
        ));
        self.popup
            .show_working(&header, &selected, text::working(language));
        let cancel = Cancel::default();
        *self.cancel.borrow_mut() = cancel.clone();
        let system = text::system(&config.mode);
        std::thread::spawn(move || {
            let result = lent.ready().and_then(|source| {
                core_jarvis::stream(
                    &source,
                    &system,
                    &[Message::user(selected)],
                    ANSWER_SECONDS,
                    &cancel,
                    &mut |_| {},
                )
            });
            app::on_main(move || with_feature(|f| f.arrived(generation, result, footer)));
        });
    }

    fn arrived(&self, generation: u64, result: Result<String, Failure>, footer: &str) {
        if generation != self.generation.get() || !self.popup.is_visible() {
            return;
        }
        let language = i18n::current();
        match result {
            Ok(answer) => {
                let edited = text::clean(&answer);
                if edited.is_empty() {
                    self.popup.fail(
                        &text::header(language, &self.config.borrow().mode),
                        &text::failed(language, &Failure::Empty),
                        language.smart_footer_message(),
                    );
                } else {
                    self.popup.finish(&edited, footer);
                }
            }
            Err(Failure::Cancelled) => {}
            Err(failure) => self.popup.fail(
                &text::header(language, &self.config.borrow().mode),
                &text::failed(language, &failure),
                language.smart_footer_message(),
            ),
        }
    }

    /// Return or a click on the result: it is copied, or pasted over the selection when the person chose that.
    fn chosen(&self, edited: &str) {
        let replace = self.config.borrow().replace;
        let board = |text: &str| match &self.history {
            Some(history) => history.pasteboard().write(Write::Text(text)),
            None => Pasteboard::general().write(Write::Text(text)),
        };
        if !replace {
            board(edited);
            return;
        }
        // Without Accessibility there is nowhere to paste: the result is left in the clipboard instead.
        if !permissions::accessibility_trusted(false) {
            board(edited);
            return;
        }
        let edited = edited.to_owned();
        app::after(FOCUS_SECONDS, move || paste(&edited));
    }
}

/// Pastes `text` into the app that has the keyboard, keeping the person's clipboard: it is saved, the text goes on it
/// marked as passing (the clipboard history keeps no copy), Command+V is posted, and it is put back unless something was
/// copied meanwhile. A clipboard that cannot be saved in full is not risked: the text is left on it instead.
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
    fn it_is_off_by_default_and_a_config_file_can_say_anything() {
        let config = Config::default();
        assert!(
            !config.enabled
                && !config.replace
                && config.mode == "fix"
                && config.shortcut == DEFAULT_SHORTCUT
        );
        let odd = Config {
            shortcut: "nonsense".into(),
            mode: "rude".into(),
            material: "mud".into(),
            ..Config::default()
        }
        .sanitized();
        assert_eq!(
            (
                odd.shortcut.as_str(),
                odd.mode.as_str(),
                odd.material.as_str()
            ),
            (DEFAULT_SHORTCUT, "fix", "glass")
        );
        let none = Config {
            shortcut: String::new(),
            ..Config::default()
        }
        .sanitized();
        assert!(none.shortcut.is_empty(), "no shortcut is a choice");
        let parsed: Config = serde_json::from_str(r#"{ "enabled": true }"#).unwrap();
        assert!(parsed.enabled && parsed.mode == "fix");
    }
}
