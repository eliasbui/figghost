//! Translate: select text in any app, press the shortcut, and a Liquid Glass window next to the pointer
//! shimmers over the text while it is translated, then shows the translation. Return or a click copies it.
//!
//! Turning the feature on is the consent to send the selected text to the translation service (MyMemory),
//! which is the only thing it sends. The service takes `core_smart::translate::MAX_CHARS` characters at a time.

use super::{
    AttachEnv,
    smart::{Problem, TranslationProblem, read_selection},
};
use crate::{
    config::parse_hotkey,
    history::History,
    i18n::{self, Language},
    settings::model::Fault,
    trace,
};
use core_smart::{net, translate};
use macos_sys::{
    MainThreadMarker, app,
    hotkey::Hotkey,
    pasteboard::{Pasteboard, Write},
    permissions,
    status_item::{Entry, Item},
    notch::Material,
    translate_ui::TranslatePopup,
};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::{
    cell::{Cell, RefCell},
    rc::{Rc, Weak},
};

pub mod settings;

const DEFAULT_SHORTCUT: &str = "ctrl+opt+cmd+t";
/// How long a translation may take.
const FETCH_SECONDS: u64 = 10;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    pub enabled: bool,
    pub shortcut: String,
    /// `auto` is the language of the app when there is a translation into it, else English.
    pub translate_to: String,
    /// The glass of the window: `glass`, `clear`, `ink` or `accent`.
    pub material: String,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            enabled: false,
            shortcut: DEFAULT_SHORTCUT.into(),
            translate_to: "auto".into(),
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
        if self.translate_to != "auto" && !translate::is_supported(&self.translate_to) {
            self.translate_to = defaults.translate_to;
        }
        if material_of(&self.material).is_none() {
            self.material = defaults.material;
        }
        self
    }

    /// The code of the language a translation goes into.
    pub fn target_language(&self, app_language: Language) -> &str {
        if self.translate_to != "auto" {
            &self.translate_to
        } else if translate::is_supported(app_language.code()) {
            app_language.code()
        } else {
            "en"
        }
    }
}

pub(crate) fn material_of(name: &str) -> Option<Material> {
    Some(match name {
        "glass" => Material::Glass,
        "clear" => Material::ClearGlass,
        "ink" => Material::Ink,
        "accent" => Material::Accent,
        _ => return None,
    })
}

fn language_name(code: &str) -> &'static str {
    translate::LANGUAGES
        .iter()
        .find(|l| l.code == code)
        .map_or("?", |l| l.name)
}

fn menu_title(language: Language) -> &'static str {
    match language {
        Language::English => "Translate",
        Language::Vietnamese => "D\u{1ecb}ch",
    }
}

fn needs_accessibility(language: Language) -> &'static str {
    match language {
        Language::English => {
            "Translate needs Accessibility access to read the selected text. Turn it on in System Settings, Privacy and Security."
        }
        Language::Vietnamese => {
            "D\u{1ecb}ch c\u{1ea7}n quy\u{1ec1}n Tr\u{1ee3} n\u{103}ng \u{111}\u{1ec3} \u{111}\u{1ecd}c \u{111}o\u{1ea1}n v\u{103}n b\u{1ea3}n \u{111}ang ch\u{1ecd}n. H\u{e3}y b\u{1ead}t trong C\u{e0}i \u{111}\u{1eb7}t h\u{1ec7} th\u{1ed1}ng, Quy\u{1ec1}n ri\u{ea}ng t\u{1b0} v\u{e0} B\u{1ea3}o m\u{1ead}t."
        }
    }
}

/// What the window says for a selection that cannot be read.
fn problem_text(problem: &Problem, language: Language) -> &'static str {
    match problem {
        Problem::Accessibility => needs_accessibility(language),
        Problem::NotRestorable => language.smart_clipboard_not_restorable(),
        Problem::Nothing => language.smart_nothing_selected(),
    }
}

thread_local! {
    static FEATURE: RefCell<Weak<Feature>> = const { RefCell::new(Weak::new()) };
}

/// Runs `f` on the feature, if it is still there, from a callback of another thread that hopped to the main one.
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
    /// Counts the selections shown, so that a late translation of an earlier one is dropped.
    generation: Cell<u64>,
    me: Weak<Feature>,
}

/// Starts the feature; the global shortcut is registered when it is switched on.
pub fn attach(env: &AttachEnv) -> Option<Rc<Feature>> {
    let config = env.config.translate.clone().sanitized();
    let history = env.clipboard.map(|c| Rc::clone(&c.history));
    let feature = Rc::new_cyclic(|me| {
        let board = history.clone();
        Feature {
            mtm: env.mtm,
            popup: TranslatePopup::new(env.mtm, move |chosen| {
                let general;
                let pasteboard = match &board {
                    Some(history) => history.pasteboard(),
                    None => {
                        general = Pasteboard::general();
                        &general
                    }
                };
                pasteboard.write(Write::Text(&chosen));
            }),
            config: RefCell::new(config.clone()),
            hotkey: RefCell::new(None),
            history,
            generation: Cell::new(0),
            me: me.clone(),
        }
    });
    FEATURE.with(|slot| *slot.borrow_mut() = Rc::downgrade(&feature));
    if let Err(fault) = feature.configure(&config) {
        eprintln!("translate: cannot register its shortcut ({})", fault.detail);
    }
    // For tests: translate `FIGGHOST_SMART_TEXT` as soon as the app is up, without a key or a menu.
    if config.enabled && std::env::var_os("FIGGHOST_TRANSLATE_OPEN").is_some() {
        let weak = Rc::downgrade(&feature);
        app::after(1.0, move || {
            if let Some(feature) = weak.upgrade() {
                feature.trigger();
            }
        });
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
        .map_err(|status| Fault::new("taken", format!("translate (status {status})")))?;
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
                "translate: cannot register its shortcut again ({})",
                fault.detail
            );
        }
    }

    pub fn runtime(&self) -> Value {
        json!({ "accessibility": permissions::accessibility_trusted(false) })
    }

    /// The "Try it" button of the settings page.
    pub fn action(&self, name: &str, _config: &mut Config) -> Option<Result<(), Fault>> {
        if name != "translate_try" {
            return None;
        }
        self.start("Hello, how are you today?".to_owned());
        Some(Ok(()))
    }

    pub fn menu_entries(this: &Rc<Self>) -> Vec<Entry> {
        let (open, visible) = (Rc::downgrade(this), Rc::downgrade(this));
        let item = Item::new(menu_title(i18n::current()), move || {
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

    /// The shortcut, the menu and the settings all come here: closes the window if it is open, otherwise reads
    /// the selection and starts translating it.
    pub fn trigger(&self) {
        if self.popup.is_visible() {
            self.popup.hide();
            return;
        }
        match read_selection(self.history.as_ref()) {
            Ok(text) => self.start(text),
            Err(problem) => {
                self.generation.set(self.generation.get() + 1);
                let language = i18n::current();
                self.popup.show_message(
                    menu_title(language),
                    problem_text(&problem, language),
                    language.smart_footer_message(),
                );
            }
        }
    }

    fn start(&self, text: String) {
        let generation = self.generation.get() + 1;
        self.generation.set(generation);
        trace::log(&format!("translate: {} characters", text.chars().count()));
        let language = i18n::current();
        let target = self.config.borrow().target_language(language).to_owned();
        let header = language.smart_translate_header(language_name(&target));
        let text = text.trim().to_owned();
        if !text.chars().any(char::is_alphabetic) {
            self.popup.show_message(
                &header,
                language.smart_nothing_recognised(),
                language.smart_footer_message(),
            );
            return;
        }
        let Some(query) = translate::query(&text, &target) else {
            let limit = language.smart_too_long(translate::MAX_CHARS);
            self.popup
                .show_message(&header, &limit, language.smart_footer_message());
            return;
        };
        self.popup
            .show_working(&header, &text, language.smart_translating());
        std::thread::spawn(move || {
            let result = match net::get(translate::SERVICE, &query, FETCH_SECONDS) {
                Ok(body) => translate::parse_response(&body).map_err(TranslationProblem::Service),
                Err(_) => Err(TranslationProblem::Offline),
            };
            app::on_main(move || with_feature(|feature| feature.arrived(generation, result)));
        });
    }

    fn arrived(&self, generation: u64, result: Result<String, TranslationProblem>) {
        if generation != self.generation.get() || !self.popup.is_visible() {
            return;
        }
        let language = i18n::current();
        match result {
            Ok(text) => self.popup.finish(&text, language.smart_footer()),
            Err(problem) => {
                let target = language_name(self.config.borrow().target_language(language));
                self.popup.fail(
                    &language.smart_translate_header(target),
                    &language.smart_translation_failed(&problem, target),
                    language.smart_footer_message(),
                );
            }
        }
    }
}
