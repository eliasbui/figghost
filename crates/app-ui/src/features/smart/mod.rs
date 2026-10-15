//! Smart Select: reads the text that is selected in the app in front and says what it is: a length, a weight,
//! an amount of money, a time, or words that can be translated. The answers appear next to the pointer; Return
//! copies the highlighted one.
//!
//! Money and translation need the internet and are off until the person turns it on: the exchange rates are a
//! download that sends nothing, a translation sends the selected text to the translation service.

use super::AttachEnv;
use crate::{
    config::parse_hotkey,
    history::{History, now_ms},
    i18n::{self, Language},
    settings::model::Fault,
    trace,
};
use core_smart::{
    Analysis, Answer, Context, Kind, civil_date,
    currency::{self, Rates},
    net,
    number::NumberStyle,
    translate,
    units::Dim,
};
use macos_sys::{
    MainThreadMarker, app,
    hotkey::Hotkey,
    pasteboard::{Pasteboard, Write},
    permissions,
    selection::{self, CopyError},
    smart_ui::{Body, Content, SmartPopup, SmartRow},
    status_item::{Entry, Item},
    style::Tone,
    timezone,
};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::{
    cell::{Cell, RefCell},
    fs,
    io::Write as _,
    os::unix::fs::OpenOptionsExt,
    path::{Path, PathBuf},
    rc::{Rc, Weak},
    time::Duration,
};
use text::TranslationProblem;

pub mod settings;
mod text;

const DEFAULT_SHORTCUT: &str = "ctrl+opt+s";
const RATES_FILE: &str = "rates.json";
/// How long a download or a translation may take.
const FETCH_SECONDS: u64 = 10;
/// The exchange rates are asked for at most this often, whether the last try worked or not.
const RATES_RETRY_SECONDS: i64 = 3600;
/// How long to wait for an app to answer Command+C.
const COPY_WAIT: Duration = Duration::from_millis(350);
/// What is shown of a selection that has no answer, in the header.
const EXCERPT_CHARS: usize = 36;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    pub enabled: bool,
    pub shortcut: String,
    /// Exchange rates and translation. Off by default: translation sends the selected text away.
    pub internet: bool,
    /// `auto` is the language of the app when there is a translation into it, else English.
    pub translate_to: String,
    pub currencies: Vec<String>,
    pub zones: Vec<String>,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            enabled: false,
            shortcut: DEFAULT_SHORTCUT.into(),
            internet: false,
            translate_to: "auto".into(),
            currencies: ["USD", "EUR", "VND"].map(String::from).to_vec(),
            zones: ["UTC", "America/New_York", "Europe/London", "Asia/Tokyo"]
                .map(String::from)
                .to_vec(),
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
        let mut currencies: Vec<String> = Vec::new();
        for code in &self.currencies {
            let code = code.trim().to_uppercase();
            if currency::is_code(&code) && !currencies.contains(&code) {
                currencies.push(code);
            }
        }
        currencies.truncate(settings::MAX_LISTED);
        self.currencies = if currencies.is_empty() {
            defaults.currencies
        } else {
            currencies
        };
        let mut zones: Vec<String> = Vec::new();
        for id in &self.zones {
            let id = id.trim();
            if id != "local"
                && id.len() <= 40
                && timezone::exists(id)
                && !zones.iter().any(|z| z == id)
            {
                zones.push(id.to_owned());
            }
        }
        zones.truncate(settings::MAX_LISTED);
        self.zones = zones;
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

/// What is known when a selection is shown.
struct Situation<'a> {
    config: &'a Config,
    rates: Option<&'a Rates>,
    language: Language,
    now: i64,
    /// Exchange rates are being downloaded right now.
    fetching: bool,
    /// Unix seconds of the last try to download them, 0 for never.
    last_attempt: i64,
}

/// Work that follows showing the answer.
#[derive(Debug, PartialEq)]
enum Followup {
    None,
    Rates,
    Translate(String),
}

fn message(header: String, text: &str, language: Language) -> Content {
    Content {
        header,
        body: Body::Text {
            text: text.to_owned(),
            muted: true,
        },
        footer: language.smart_footer_message().to_owned(),
    }
}

fn excerpt(text: &str) -> String {
    let line: String = text.split_whitespace().collect::<Vec<_>>().join(" ");
    if line.chars().count() > EXCERPT_CHARS {
        let cut: String = line.chars().take(EXCERPT_CHARS).collect();
        format!("{cut}...")
    } else {
        line
    }
}

fn language_name(code: &str) -> &'static str {
    translate::LANGUAGES
        .iter()
        .find(|l| l.code == code)
        .map_or("?", |l| l.name)
}

fn unit_symbol(dim: Dim) -> &'static str {
    match dim {
        Dim::Length => "ruler",
        Dim::Mass => "scalemass",
        Dim::Volume => "drop",
        Dim::Area => "square.dashed",
        Dim::Speed => "speedometer",
        Dim::Temperature => "thermometer.medium",
        Dim::Data => "externaldrive",
    }
}

fn row(answer: &Answer, language: Language) -> SmartRow {
    let (symbol, tone, detail) = match answer.kind {
        Kind::Unit(dim) => (
            unit_symbol(dim),
            Tone::Blue,
            language.smart_dim(dim).to_owned(),
        ),
        Kind::Money => ("dollarsign.circle", Tone::Green, answer.note.clone()),
        Kind::Time => {
            let zone = if answer.note == "local" {
                language.smart_local_time().to_owned()
            } else {
                answer.note.clone()
            };
            let detail = if answer.day == 0 {
                zone
            } else {
                format!("{zone}, {}", language.smart_day(answer.day))
            };
            ("clock", Tone::Orange, detail)
        }
    };
    SmartRow {
        symbol,
        tone,
        title: answer.text.clone(),
        detail,
    }
}

/// What to show for a selection, and what to do next. No windows and no network here.
fn content_for(text: &str, situation: &Situation) -> (Content, Followup) {
    let Situation {
        config,
        rates,
        language,
        now,
        fetching,
        last_attempt,
    } = *situation;
    let style = if language == Language::Vietnamese {
        NumberStyle::VIETNAMESE
    } else {
        NumberStyle::ENGLISH
    };
    let mut zones = vec!["local".to_owned()];
    zones.extend(config.zones.iter().cloned());
    let context = Context {
        style,
        rates,
        currencies: &config.currencies,
        zones: &zones,
        now,
        offset: &timezone::offset_minutes,
    };
    match core_smart::analyze(text, &context) {
        Analysis::Found { input, answers } if answers.is_empty() => (
            message(input, language.smart_nothing_to_show(), language),
            Followup::None,
        ),
        Analysis::Found { input, answers } => {
            let money = answers.iter().any(|a| a.kind == Kind::Money);
            let footer = match rates.filter(|_| money) {
                Some(rates) => {
                    let (_, month, day) = civil_date(rates.updated);
                    language.smart_footer_rates(&language.smart_short_date(month, day))
                }
                None => language.smart_footer().to_owned(),
            };
            // Rates that are out of date are refreshed quietly while the old ones are shown.
            let stale = money
                && config.internet
                && !fetching
                && rates.is_some_and(|r| r.is_due(now))
                && now - last_attempt >= RATES_RETRY_SECONDS;
            let content = Content {
                header: input,
                body: Body::Rows(answers.iter().map(|a| row(a, language)).collect()),
                footer,
            };
            (
                content,
                if stale {
                    Followup::Rates
                } else {
                    Followup::None
                },
            )
        }
        Analysis::NoRates { input } => {
            if !config.internet {
                (
                    message(input, language.smart_internet_for_rates(), language),
                    Followup::None,
                )
            } else if fetching {
                (
                    message(input, language.smart_fetching_rates(), language),
                    Followup::None,
                )
            } else if now - last_attempt >= RATES_RETRY_SECONDS {
                (
                    message(input, language.smart_fetching_rates(), language),
                    Followup::Rates,
                )
            } else {
                (
                    message(input, language.smart_rates_unavailable(), language),
                    Followup::None,
                )
            }
        }
        Analysis::Text => plain_text(text, config, language),
    }
}

fn plain_text(text: &str, config: &Config, language: Language) -> (Content, Followup) {
    let header = excerpt(text);
    if !text.chars().any(char::is_alphabetic) {
        return (
            message(header, language.smart_nothing_recognised(), language),
            Followup::None,
        );
    }
    if !config.internet {
        return (
            message(header, language.smart_internet_for_translation(), language),
            Followup::None,
        );
    }
    if text.trim().chars().count() > translate::MAX_CHARS {
        let limit = language.smart_too_long(translate::MAX_CHARS);
        return (message(header, &limit, language), Followup::None);
    }
    let target = language_name(config.target_language(language));
    let content = message(
        language.smart_translate_header(target),
        language.smart_translating(),
        language,
    );
    (content, Followup::Translate(text.trim().to_owned()))
}

/// Why nothing could be read from the app in front.
enum Problem {
    Accessibility,
    NotRestorable,
    Nothing,
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

fn unix_now() -> i64 {
    now_ms() / 1000
}

/// Writes a file only its owner can read.
fn write_private(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    let mut file = fs::OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .mode(0o600)
        .open(path)?;
    file.write_all(bytes)
}

pub struct Feature {
    mtm: MainThreadMarker,
    popup: SmartPopup,
    config: RefCell<Config>,
    hotkey: RefCell<Option<Hotkey>>,
    history: Option<Rc<History>>,
    rates: RefCell<Option<Rates>>,
    rates_file: PathBuf,
    rates_attempt: Cell<i64>,
    fetching: Cell<bool>,
    /// The selection the open window is about, to redo the answer when the rates arrive.
    shown: RefCell<Option<String>>,
    /// Counts the times a selection was shown, so that a late translation of an earlier one is dropped.
    generation: Cell<u64>,
    me: Weak<Feature>,
}

/// Starts the feature; the global shortcut is registered when it is switched on.
pub fn attach(env: &AttachEnv) -> Option<Rc<Feature>> {
    let config = env.config.smart.clone().sanitized();
    let history = env.clipboard.map(|c| Rc::clone(&c.history));
    let rates_file = env.home.join(RATES_FILE);
    let rates = fs::read_to_string(&rates_file)
        .ok()
        .and_then(|body| Rates::parse(&body));
    let feature = Rc::new_cyclic(|me| {
        let board = history.clone();
        Feature {
            mtm: env.mtm,
            popup: SmartPopup::new(env.mtm, move |chosen| {
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
            rates: RefCell::new(rates),
            rates_file,
            rates_attempt: Cell::new(0),
            fetching: Cell::new(false),
            shown: RefCell::new(None),
            generation: Cell::new(0),
            me: me.clone(),
        }
    });
    FEATURE.with(|slot| *slot.borrow_mut() = Rc::downgrade(&feature));
    if let Err(fault) = feature.configure(&config) {
        eprintln!(
            "smart select: cannot register its shortcut ({})",
            fault.detail
        );
    }
    // For tests: answer for `FIGGHOST_SMART_TEXT` as soon as the app is up, without a key or a menu.
    if config.enabled && std::env::var_os("FIGGHOST_SMART_OPEN").is_some() {
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
        .map_err(|status| Fault::new("taken", format!("smart (status {status})")))?;
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
                "smart select: cannot register its shortcut again ({})",
                fault.detail
            );
        }
    }

    pub fn runtime(&self) -> Value {
        json!({
            "accessibility": permissions::accessibility_trusted(false),
            "rates_updated": self.rates.borrow().as_ref().map_or(0, |r| r.updated),
        })
    }

    /// The "Try it" button of the settings page.
    pub fn action(&self, name: &str, _config: &mut Config) -> Option<Result<(), Fault>> {
        if name != "smart_try" {
            return None;
        }
        self.show_text("5 km".to_owned());
        Some(Ok(()))
    }

    pub fn menu_entries(this: &Rc<Self>) -> Vec<Entry> {
        let (open, visible) = (Rc::downgrade(this), Rc::downgrade(this));
        let item = Item::new(i18n::current().smart_menu(), move || {
            // The menu has to be gone, and the app that was in front has to have its focus back, before its
            // selection can be read.
            let weak = open.clone();
            app::after(0.25, move || {
                if let Some(feature) = weak.upgrade() {
                    feature.trigger();
                }
            });
        })
        .visible_when(move || visible.upgrade().is_some_and(|f| f.config.borrow().enabled));
        vec![Entry::Item(item)]
    }

    /// The shortcut, the menu and the settings all come here: closes the window if it is open, otherwise reads
    /// the selection and answers.
    pub fn trigger(&self) {
        if self.popup.is_visible() {
            self.popup.hide();
            return;
        }
        match self.read_selection() {
            Ok(text) => self.show_text(text),
            Err(problem) => {
                self.generation.set(self.generation.get() + 1);
                *self.shown.borrow_mut() = None;
                let language = i18n::current();
                let text = match problem {
                    Problem::Accessibility => language.smart_needs_accessibility(),
                    Problem::NotRestorable => language.smart_clipboard_not_restorable(),
                    Problem::Nothing => language.smart_nothing_selected(),
                };
                self.popup.show(
                    self.mtm,
                    message(language.smart_menu().to_owned(), text, language),
                );
            }
        }
    }

    fn read_selection(&self) -> Result<String, Problem> {
        // For tests, which have no app with a selection to read.
        if let Ok(text) = std::env::var("FIGGHOST_SMART_TEXT") {
            return Ok(text);
        }
        if let Some(text) = selection::selected_text() {
            return Ok(text);
        }
        // The app does not offer its selection: copy it and put the clipboard back. That posts a key, which
        // needs the same permission.
        if !permissions::accessibility_trusted(false) {
            return Err(Problem::Accessibility);
        }
        let general;
        let board = match &self.history {
            Some(history) => history.pasteboard(),
            None => {
                general = Pasteboard::general();
                &general
            }
        };
        match selection::copy_selected_text(board, COPY_WAIT) {
            Ok(copied) => {
                // The copy and the restore were not the person's doing; the history must not record them.
                if let Some(history) = &self.history {
                    history.skip_change(copied.change_count);
                }
                Ok(copied.text)
            }
            Err(CopyError::NotRestorable) => Err(Problem::NotRestorable),
            Err(CopyError::Nothing) => Err(Problem::Nothing),
        }
    }

    fn show_text(&self, text: String) {
        self.generation.set(self.generation.get() + 1);
        trace::log(&format!(
            "smart select: {} characters",
            text.chars().count()
        ));
        *self.shown.borrow_mut() = Some(text.clone());
        self.present(&text, true);
    }

    fn present(&self, text: &str, first: bool) {
        let rates = self.rates.borrow();
        let (content, followup) = content_for(
            text,
            &Situation {
                config: &self.config.borrow(),
                rates: rates.as_ref(),
                language: i18n::current(),
                now: unix_now(),
                fetching: self.fetching.get(),
                last_attempt: self.rates_attempt.get(),
            },
        );
        drop(rates);
        if first {
            self.popup.show(self.mtm, content);
        } else {
            self.popup.update(content);
        }
        match followup {
            Followup::None => {}
            Followup::Rates => self.fetch_rates(),
            Followup::Translate(text) => self.translate(&text),
        }
    }

    fn fetch_rates(&self) {
        self.fetching.set(true);
        self.rates_attempt.set(unix_now());
        std::thread::spawn(|| {
            let result = net::get(currency::SERVICE, &[], FETCH_SECONDS)
                .ok()
                .and_then(|body| Rates::parse(&body).map(|rates| (rates, body)));
            app::on_main(move || with_feature(|feature| feature.rates_arrived(result)));
        });
    }

    fn rates_arrived(&self, result: Option<(Rates, String)>) {
        self.fetching.set(false);
        if let Some((rates, body)) = result {
            if let Err(e) = write_private(&self.rates_file, body.as_bytes()) {
                eprintln!("smart select: cannot keep the exchange rates: {e}");
            }
            *self.rates.borrow_mut() = Some(rates);
        }
        let shown = self.shown.borrow().clone();
        if let Some(text) = shown.filter(|_| self.popup.is_visible()) {
            self.present(&text, false);
        }
    }

    fn translate(&self, text: &str) {
        let generation = self.generation.get();
        let target = self
            .config
            .borrow()
            .target_language(i18n::current())
            .to_owned();
        let Some(query) = translate::query(text, &target) else {
            return;
        };
        std::thread::spawn(move || {
            let result = match net::get(translate::SERVICE, &query, FETCH_SECONDS) {
                Ok(body) => translate::parse_response(&body).map_err(TranslationProblem::Service),
                Err(_) => Err(TranslationProblem::Offline),
            };
            app::on_main(move || with_feature(|feature| feature.translated(generation, result)));
        });
    }

    fn translated(&self, generation: u64, result: Result<String, TranslationProblem>) {
        if generation != self.generation.get() || !self.popup.is_visible() {
            return;
        }
        let language = i18n::current();
        let target = language_name(self.config.borrow().target_language(language));
        let header = language.smart_translate_header(target);
        let content = match result {
            Ok(text) => Content {
                header,
                body: Body::Text { text, muted: false },
                footer: language.smart_footer().to_owned(),
            },
            Err(problem) => message(
                header,
                &language.smart_translation_failed(&problem, target),
                language,
            ),
        };
        self.popup.update(content);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 2026-10-15 03:00:00 UTC.
    const NOW: i64 = 1_792_022_400 + 3 * 3600;

    fn rates() -> Rates {
        let mut body = String::from(
            r#"{"result":"success","time_last_update_unix":1792022551,"time_next_update_unix":1792109261,"rates":{"USD":1,"EUR":0.894425,"VND":25890.2"#,
        );
        for i in 0..30 {
            body.push_str(&format!(
                r#","X{}{}":{}"#,
                (b'A' + i / 26) as char,
                (b'A' + i % 26) as char,
                i + 1
            ));
        }
        body.push_str("}}");
        Rates::parse(&body).expect("a plausible table")
    }

    struct Setup {
        config: Config,
        rates: Option<Rates>,
        language: Language,
        now: i64,
        fetching: bool,
        last_attempt: i64,
    }

    impl Setup {
        fn new() -> Self {
            Self {
                config: Config {
                    enabled: true,
                    ..Config::default()
                },
                rates: None,
                language: Language::English,
                now: NOW,
                fetching: false,
                last_attempt: 0,
            }
        }

        fn show(&self, text: &str) -> (Content, Followup) {
            content_for(
                text,
                &Situation {
                    config: &self.config,
                    rates: self.rates.as_ref(),
                    language: self.language,
                    now: self.now,
                    fetching: self.fetching,
                    last_attempt: self.last_attempt,
                },
            )
        }
    }

    fn rows(content: &Content) -> Vec<(&str, &str)> {
        match &content.body {
            Body::Rows(rows) => rows
                .iter()
                .map(|r| (r.title.as_str(), r.detail.as_str()))
                .collect(),
            Body::Text { .. } => panic!("expected rows, got {content:?}"),
        }
    }

    fn muted(content: &Content) -> &str {
        match &content.body {
            Body::Text { text, muted: true } => text,
            other => panic!("expected a message, got {other:?}"),
        }
    }

    #[test]
    fn a_length_in_each_language() {
        let mut setup = Setup::new();
        let (content, followup) = setup.show("5 km");
        assert_eq!(followup, Followup::None);
        assert_eq!(content.header, "5 km");
        assert_eq!(
            rows(&content),
            [("3.107 mi", "Length"), ("16,404 ft", "Length")]
        );
        setup.language = Language::Vietnamese;
        let (content, _) = setup.show("1,5 km");
        assert_eq!(content.header, "1,5 km");
        assert_eq!(rows(&content)[0], ("4.921 ft", "Chi\u{1ec1}u d\u{e0}i"));
        assert_eq!(content.footer, Language::Vietnamese.smart_footer());
    }

    #[test]
    fn money_with_rates_says_which_day_they_are_from() {
        let mut setup = Setup::new();
        setup.rates = Some(rates());
        let (content, followup) = setup.show("$100");
        assert_eq!(content.header, "100.00 USD");
        assert_eq!(
            rows(&content),
            [
                ("89.44 EUR", "1 USD = 0.8944 EUR"),
                ("2,589,020 VND", "1 USD = 25,890 VND")
            ]
        );
        assert_eq!(content.footer, "Rates of 15 Oct. Return copies, Esc closes");
        assert_eq!(followup, Followup::None, "the rates are not due yet");
    }

    #[test]
    fn old_rates_are_refreshed_quietly_when_the_internet_is_on() {
        let mut setup = Setup::new();
        setup.rates = Some(rates());
        setup.now = 1_792_200_000;
        assert_eq!(setup.show("$100").1, Followup::None, "the internet is off");
        setup.config.internet = true;
        assert_eq!(setup.show("$100").1, Followup::Rates);
        setup.last_attempt = setup.now - 60;
        assert_eq!(
            setup.show("$100").1,
            Followup::None,
            "not again within the hour"
        );
        setup.last_attempt = 0;
        setup.fetching = true;
        assert_eq!(setup.show("$100").1, Followup::None, "already on its way");
    }

    #[test]
    fn money_without_rates_follows_the_internet_setting() {
        let mut setup = Setup::new();
        let (content, followup) = setup.show("50 EUR");
        assert_eq!(content.header, "50.00 EUR");
        assert_eq!(
            muted(&content),
            Language::English.smart_internet_for_rates()
        );
        assert_eq!(followup, Followup::None);

        setup.config.internet = true;
        let (content, followup) = setup.show("50 EUR");
        assert_eq!(muted(&content), Language::English.smart_fetching_rates());
        assert_eq!(followup, Followup::Rates);

        setup.fetching = true;
        let (content, followup) = setup.show("50 EUR");
        assert_eq!(muted(&content), Language::English.smart_fetching_rates());
        assert_eq!(followup, Followup::None);

        setup.fetching = false;
        setup.last_attempt = NOW - 30;
        let (content, followup) = setup.show("50 EUR");
        assert_eq!(muted(&content), Language::English.smart_rates_unavailable());
        assert_eq!(followup, Followup::None);
    }

    #[test]
    fn a_time_comes_out_in_the_other_zones() {
        let setup = Setup::new();
        let (content, _) = setup.show("3pm PST");
        assert_eq!(content.header, "3:00 PM PST");
        let shown = rows(&content);
        assert!(
            shown
                .iter()
                .any(|(title, detail)| *title == "11:00 PM" && detail.starts_with("UTC")),
            "{shown:?}"
        );
        assert!(
            shown.iter().any(|(_, detail)| detail.contains("Tokyo")),
            "{shown:?}"
        );
    }

    #[test]
    fn nothing_to_show_in_is_said() {
        let mut setup = Setup::new();
        setup.config.zones.clear();
        // The reader's own zone is the only target and it equals the source.
        let (content, _) = setup.show("9:00");
        assert_eq!(muted(&content), Language::English.smart_nothing_to_show());
    }

    #[test]
    fn plain_words_are_translated_only_with_the_internet_on() {
        let mut setup = Setup::new();
        let (content, followup) = setup.show("good morning");
        assert_eq!(content.header, "good morning");
        assert_eq!(
            muted(&content),
            Language::English.smart_internet_for_translation()
        );
        assert_eq!(followup, Followup::None);

        setup.config.internet = true;
        setup.language = Language::Vietnamese;
        let (content, followup) = setup.show("  good morning ");
        assert_eq!(
            content.header,
            Language::Vietnamese.smart_translate_header("Ti\u{1ebf}ng Vi\u{1ec7}t")
        );
        assert_eq!(muted(&content), Language::Vietnamese.smart_translating());
        assert_eq!(followup, Followup::Translate("good morning".into()));

        setup.config.translate_to = "ja".into();
        let (content, _) = setup.show("good morning");
        assert!(
            content.header.contains("\u{65e5}\u{672c}\u{8a9e}"),
            "{}",
            content.header
        );
    }

    #[test]
    fn text_that_cannot_be_translated_says_why() {
        let mut setup = Setup::new();
        setup.config.internet = true;
        let (content, followup) = setup.show("12345");
        assert_eq!(
            muted(&content),
            Language::English.smart_nothing_recognised()
        );
        assert_eq!(followup, Followup::None);
        let long = "word ".repeat(100);
        let (content, followup) = setup.show(&long);
        assert_eq!(followup, Followup::None);
        assert!(muted(&content).contains("450"), "{}", muted(&content));
        assert!(
            content.header.ends_with("..."),
            "a long selection is shortened in the header"
        );
        assert!(content.header.chars().count() <= EXCERPT_CHARS + 3);
    }

    #[test]
    fn the_config_file_cannot_break_the_feature() {
        let broken = Config {
            enabled: true,
            shortcut: "nonsense".into(),
            internet: true,
            translate_to: "klingon".into(),
            currencies: vec![
                "usd".into(),
                "us".into(),
                "USD".into(),
                "x1x".into(),
                "vnd".into(),
            ],
            zones: vec![
                "Mars/Olympus".into(),
                "Asia/Tokyo".into(),
                "local".into(),
                "Asia/Tokyo".into(),
            ],
        }
        .sanitized();
        let defaults = Config::default();
        assert!(broken.enabled && broken.internet, "valid values are kept");
        assert_eq!(broken.shortcut, defaults.shortcut);
        assert_eq!(broken.translate_to, "auto");
        assert_eq!(broken.currencies, ["USD", "VND"]);
        assert_eq!(broken.zones, ["Asia/Tokyo"]);
        let empty = Config {
            currencies: vec![],
            shortcut: String::new(),
            ..Config::default()
        }
        .sanitized();
        assert_eq!(
            empty.currencies, defaults.currencies,
            "no currencies is replaced"
        );
        assert_eq!(
            empty.shortcut, "",
            "no shortcut is allowed: the menu still reaches it"
        );
    }

    #[test]
    fn the_target_language_follows_the_app_unless_chosen() {
        let mut config = Config::default();
        assert_eq!(config.target_language(Language::Vietnamese), "vi");
        assert_eq!(config.target_language(Language::English), "en");
        config.translate_to = "fr".into();
        assert_eq!(config.target_language(Language::Vietnamese), "fr");
    }

    #[test]
    fn an_old_config_without_the_feature_loads_with_its_defaults() {
        let config: crate::config::Config =
            serde_json::from_str(r#"{ "language": "vi" }"#).unwrap();
        assert_eq!(config.smart, Config::default());
        let partial: crate::config::Config =
            serde_json::from_str(r#"{ "smart": { "enabled": true, "currencies": ["JPY"] } }"#)
                .unwrap();
        assert!(partial.smart.enabled);
        assert_eq!(partial.smart.currencies, ["JPY"]);
        assert_eq!(partial.smart.shortcut, DEFAULT_SHORTCUT);
    }
}
