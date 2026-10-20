//! The Devices tab of the notch: the Bluetooth devices that are connected, each with the charge it reports, so that a
//! mouse that is about to die or a pair of earbuds that is flat is seen before it is needed. It is off until the person
//! turns it on.
//!
//! The devices are asked of the system's own `system_profiler`, run with fixed arguments and no shell, off the main
//! thread and stopped after a few seconds; what it prints is read as data. It is asked when the tab opens and then every
//! half minute while the tab shows, and never otherwise. No permission is needed and nothing is written to disk. A device
//! shows a charge only when it reports one to the system, which many keyboards, mice and speakers do not.

use super::{AttachEnv, Ran, run_bounded};
use crate::{
    history::{ago, now_ms},
    i18n::{self, Language},
    settings::model::Fault,
    trace,
};
use logic::{Gadget, Kind, LOW_AT, parse};
use macos_sys::{
    app,
    devices_ui::{Card, DevicesDisplay, DevicesView, MAX_CARDS, PAGE_SIZE, Reading},
};
use serde::{Deserialize, Serialize};
use std::{
    cell::{Cell, RefCell},
    rc::{Rc, Weak},
};

pub mod logic;
pub mod settings;
mod text;

const PROFILER: &str = "/usr/sbin/system_profiler";
const COMMAND_SECONDS: u64 = 15;
/// While the tab shows, the devices are asked for again this often, in seconds.
const TICK_SECONDS: f64 = 30.0;
/// Opening the tab again within this many milliseconds of a look does not ask again.
const FRESH_MS: i64 = 20_000;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    pub enabled: bool,
    /// The charge at and under which a device is called low, one of `LOW_AT`.
    pub low_at: u8,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            enabled: false,
            low_at: 20,
        }
    }
}

impl Config {
    /// A config file can say anything; a level that is not offered is read as the usual one.
    pub fn sanitized(mut self) -> Self {
        if !LOW_AT.contains(&self.low_at) {
            self.low_at = Self::default().low_at;
        }
        self
    }
}

/// Why the devices could not be read.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Problem {
    Slow,
    Failed,
    Unreadable,
}

/// What is known of the devices.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Known {
    /// Nothing yet: the first look is on its way.
    Nothing,
    Read {
        gadgets: Vec<Gadget>,
        /// Milliseconds since 1970.
        at: i64,
    },
    Problem(Problem),
}

thread_local! {
    /// The answer arrives from another thread and hops to the main one, which finds the feature here.
    static FEATURE: RefCell<Weak<Feature>> = const { RefCell::new(Weak::new()) };
}

pub struct Feature {
    view: Rc<DevicesView>,
    config: RefCell<Config>,
    known: RefCell<Known>,
    busy: Cell<bool>,
    /// Bumped by every look and when the feature is switched off; an answer that finds another number is not wanted.
    generation: Cell<u64>,
    ticker: RefCell<Option<app::Timer>>,
    showing: Cell<bool>,
    me: Weak<Feature>,
}

/// Adds the tab. `None` without a notch, which is where the tab lives.
pub fn attach(env: &AttachEnv) -> Option<Rc<Feature>> {
    let notch = env.notch?;
    let view = DevicesView::new(env.mtm, notch.page_area(PAGE_SIZE.width, PAGE_SIZE.height));
    let feature = Rc::new_cyclic(|me| Feature {
        view: Rc::clone(&view),
        config: RefCell::new(env.config.devices.clone().sanitized()),
        known: RefCell::new(Known::Nothing),
        busy: Cell::new(false),
        generation: Cell::new(0),
        ticker: RefCell::default(),
        showing: Cell::new(false),
        me: me.clone(),
    });
    FEATURE.with(|slot| *slot.borrow_mut() = Rc::downgrade(&feature));
    let (shown, hidden) = (Rc::downgrade(&feature), Rc::downgrade(&feature));
    notch.add_page(view.page(
        i18n::current().devices_tab().to_owned(),
        move || {
            if let Some(f) = shown.upgrade() {
                f.showing.set(true);
                f.begin();
            }
        },
        move || {
            if let Some(f) = hidden.upgrade() {
                f.showing.set(false);
                f.ticker.borrow_mut().take();
                trace::log("devices: closed");
            }
        },
    ));
    Some(feature)
}

impl Feature {
    pub fn configure(&self, config: &Config) -> Result<(), Fault> {
        let config = config.clone().sanitized();
        let was = self.config.replace(config.clone());
        if was.enabled && !config.enabled {
            // Off keeps nothing, and an answer that is still on its way is for a feature that is off.
            self.generation.set(self.generation.get() + 1);
            self.busy.set(false);
            *self.known.borrow_mut() = Known::Nothing;
            self.ticker.borrow_mut().take();
        }
        if !was.enabled && config.enabled && self.showing.get() {
            self.begin();
        }
        self.render();
        Ok(())
    }

    /// The tab opened: show what is known, look unless it was looked at a moment ago, and look again while it shows.
    fn begin(&self) {
        self.ticker.borrow_mut().take();
        if !self.config.borrow().enabled {
            return;
        }
        self.render();
        let fresh =
            matches!(&*self.known.borrow(), Known::Read { at, .. } if now_ms() - at < FRESH_MS);
        if !fresh {
            self.look();
        }
        let weak = self.me.clone();
        *self.ticker.borrow_mut() = Some(app::every(TICK_SECONDS, move || {
            if let Some(f) = weak.upgrade() {
                f.look();
            }
        }));
    }

    /// Asks the system. It can take seconds, so it is done off the main thread.
    fn look(&self) {
        if !self.config.borrow().enabled || self.busy.get() {
            return;
        }
        let generation = self.generation.get() + 1;
        self.generation.set(generation);
        self.busy.set(true);
        trace::log("devices: reading");
        std::thread::spawn(move || {
            let outcome =
                match run_bounded(PROFILER, &["SPBluetoothDataType", "-json"], COMMAND_SECONDS) {
                    Ok(text) => parse(&text).map_err(|_| Problem::Unreadable),
                    Err(Ran::Slow) => Err(Problem::Slow),
                    Err(Ran::Failed) => Err(Problem::Failed),
                };
            app::on_main(move || {
                FEATURE.with(|slot| {
                    if let Some(f) = slot.borrow().upgrade() {
                        f.arrived(generation, outcome);
                    }
                });
            });
        });
    }

    fn arrived(&self, generation: u64, outcome: Result<Vec<Gadget>, Problem>) {
        if generation != self.generation.get() {
            return;
        }
        self.busy.set(false);
        match &outcome {
            Ok(gadgets) => trace::log(&format!(
                "devices: {} connected, {} with a charge",
                gadgets.len(),
                gadgets.iter().filter(|g| g.reports_a_charge()).count()
            )),
            Err(problem) => trace::log(&format!("devices: could not read ({problem:?})")),
        }
        let old = self.known.replace(Known::Nothing);
        *self.known.borrow_mut() = learn(old, outcome, now_ms());
        self.render();
    }

    fn render(&self) {
        if !self.showing.get() || !self.config.borrow().enabled {
            return;
        }
        self.view.show(display(
            i18n::current(),
            &self.config.borrow(),
            &self.known.borrow(),
            now_ms(),
        ));
    }
}

/// What is known once an answer has arrived. A failure never takes away what was read before: the time it was read at
/// says how old it is.
fn learn(known: Known, outcome: Result<Vec<Gadget>, Problem>, now: i64) -> Known {
    match (outcome, known) {
        (Ok(gadgets), _) => Known::Read { gadgets, at: now },
        (Err(_), old @ Known::Read { .. }) => old,
        (Err(problem), _) => Known::Problem(problem),
    }
}

fn symbol_of(kind: Kind) -> &'static str {
    match kind {
        Kind::Headphones => "headphones",
        Kind::Speaker => "hifispeaker.fill",
        Kind::Keyboard => "keyboard",
        Kind::Mouse => "computermouse.fill",
        Kind::Trackpad => "hand.point.up.left.fill",
        Kind::Controller => "gamecontroller.fill",
        Kind::Phone => "iphone",
        Kind::Other => "dot.radiowaves.left.and.right",
    }
}

fn card(language: Language, gadget: &Gadget, low_at: u8) -> Card {
    let low = |percent: u8| percent <= low_at;
    let mut readings = Vec::new();
    for (label, percent, counts) in [
        (String::new(), gadget.main, true),
        ("L".to_owned(), gadget.left, true),
        ("R".to_owned(), gadget.right, true),
        (language.devices_case().to_owned(), gadget.case, false),
    ] {
        if let Some(percent) = percent {
            readings.push(Reading {
                label,
                percent,
                low: counts && low(percent),
            });
        }
    }
    let is_low = gadget.lowest().is_some_and(low);
    Card {
        symbol: symbol_of(gadget.kind),
        name: gadget.name.clone(),
        note: if is_low {
            language.devices_low()
        } else if !gadget.reports_a_charge() {
            language.devices_silent()
        } else {
            ""
        }
        .to_owned(),
        readings,
        low: is_low,
    }
}

fn note(title: &str, hint: &str) -> DevicesDisplay {
    DevicesDisplay {
        note: Some((title.to_owned(), hint.to_owned())),
        ..DevicesDisplay::default()
    }
}

/// What the tab shows for what is known. Pure, so that the tests drive it with devices and a clock of their own.
fn display(language: Language, config: &Config, known: &Known, now: i64) -> DevicesDisplay {
    match known {
        Known::Nothing => note(language.devices_looking_title(), ""),
        Known::Problem(_) => note(
            language.devices_problem_title(),
            language.devices_problem_hint(),
        ),
        Known::Read { gadgets, .. } if gadgets.is_empty() => {
            note(language.devices_none_title(), language.devices_none_hint())
        }
        Known::Read { gadgets, at } => DevicesDisplay {
            cards: gadgets
                .iter()
                .take(MAX_CARDS)
                .map(|g| card(language, g, config.low_at))
                .collect(),
            footer: language.devices_footer(
                &ago(language, now, *at),
                gadgets.len().saturating_sub(MAX_CARDS),
            ),
            note: None,
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const LANGUAGES: [Language; 2] = [Language::English, Language::Vietnamese];

    fn gadget(name: &str, kind: Kind, main: Option<u8>) -> Gadget {
        Gadget {
            name: name.into(),
            kind,
            main,
            left: None,
            right: None,
            case: None,
        }
    }

    fn read(gadgets: Vec<Gadget>) -> Known {
        Known::Read {
            gadgets,
            at: 1_000_000,
        }
    }

    #[test]
    fn the_feature_is_off_by_default_and_old_files_load() {
        let config: Config = serde_json::from_str("{}").unwrap();
        assert_eq!(config, Config::default());
        assert!(!config.enabled && config.low_at == 20);
        let partial: Config =
            serde_json::from_str(r#"{ "enabled": true, "future_key": 1 }"#).unwrap();
        assert!(partial.enabled && partial.low_at == 20);
    }

    #[test]
    fn a_level_that_is_not_offered_is_read_as_the_usual_one() {
        for low_at in [0, 5, 99, 255] {
            assert_eq!(
                Config {
                    low_at,
                    ..Config::default()
                }
                .sanitized()
                .low_at,
                20,
                "{low_at}"
            );
        }
        for &low_at in &LOW_AT {
            assert_eq!(
                Config {
                    low_at,
                    ..Config::default()
                }
                .sanitized()
                .low_at,
                low_at
            );
        }
    }

    #[test]
    fn a_card_shows_the_charge_a_device_reports_and_says_so_when_it_reports_none() {
        for language in LANGUAGES {
            let full = card(language, &gadget("Cans", Kind::Headphones, Some(100)), 20);
            assert_eq!(
                (full.low, full.note.as_str(), full.symbol),
                (false, "", "headphones")
            );
            assert_eq!(
                full.readings,
                [Reading {
                    label: String::new(),
                    percent: 100,
                    low: false
                }]
            );
            let silent = card(language, &gadget("Mouse", Kind::Mouse, None), 20);
            assert!(silent.readings.is_empty() && !silent.low);
            assert_eq!(silent.note, language.devices_silent());
        }
    }

    #[test]
    fn a_device_is_low_at_and_under_the_level_and_the_card_says_so_in_words() {
        for language in LANGUAGES {
            for (percent, low) in [(0, true), (19, true), (20, true), (21, false), (100, false)] {
                let card = card(language, &gadget("Mouse", Kind::Mouse, Some(percent)), 20);
                assert_eq!(card.low, low, "{percent}");
                assert_eq!(card.note == language.devices_low(), low, "{percent}");
                assert_eq!(card.readings[0].low, low, "{percent}");
            }
            let at_ten = card(language, &gadget("Mouse", Kind::Mouse, Some(15)), 10);
            assert!(!at_ten.low, "the level is the person's to choose");
        }
    }

    #[test]
    fn earbuds_show_each_bud_and_the_case_and_a_flat_case_alone_is_not_low() {
        let mut buds = gadget("Buds", Kind::Headphones, None);
        buds.left = Some(80);
        buds.right = Some(12);
        buds.case = Some(3);
        let card = card(Language::English, &buds, 20);
        let labels: Vec<&str> = card.readings.iter().map(|r| r.label.as_str()).collect();
        assert_eq!(labels, ["L", "R", "Case"]);
        assert!(card.low, "one bud is low");
        assert_eq!(
            card.readings.iter().map(|r| r.low).collect::<Vec<_>>(),
            [false, true, false],
            "the case is never marked"
        );
        buds.right = Some(70);
        let fine = super::card(Language::English, &buds, 20);
        assert!(
            !fine.low,
            "a case that is nearly flat does not make earbuds low"
        );
        assert_eq!(fine.note, "");
    }

    #[test]
    fn what_is_known_decides_what_the_tab_says_in_both_languages() {
        let now = 1_000_000 + 5_000;
        for language in LANGUAGES {
            let config = Config::default();
            let looking = display(language, &config, &Known::Nothing, now);
            assert_eq!(looking.note.unwrap().0, language.devices_looking_title());
            let broken = display(language, &config, &Known::Problem(Problem::Slow), now);
            assert_eq!(broken.note.unwrap().0, language.devices_problem_title());
            let none = display(language, &config, &read(Vec::new()), now);
            assert_eq!(none.note.unwrap().0, language.devices_none_title());
            let some = display(
                language,
                &config,
                &read(vec![gadget("Cans", Kind::Headphones, Some(50))]),
                now,
            );
            assert!(some.note.is_none() && some.cards.len() == 1);
            assert!(some.footer.contains(language.just_now()), "{}", some.footer);
        }
    }

    #[test]
    fn more_devices_than_the_page_has_room_for_are_counted_in_the_footer() {
        let many: Vec<Gadget> = (0..8)
            .map(|i| gadget(&format!("Mouse {i}"), Kind::Mouse, Some(50)))
            .collect();
        let shown = display(
            Language::English,
            &Config::default(),
            &read(many),
            1_000_000,
        );
        assert_eq!(shown.cards.len(), MAX_CARDS);
        assert_eq!(shown.footer, "Updated just now. 2 more not shown.");
        let vi = display(
            Language::Vietnamese,
            &Config::default(),
            &read(vec![gadget("A", Kind::Mouse, None); 7]),
            1_000_000,
        );
        assert!(
            vi.footer.ends_with("1 thiết bị nữa không hiện."),
            "{}",
            vi.footer
        );
    }

    #[test]
    fn a_later_failure_keeps_what_was_read_and_a_first_failure_is_the_news() {
        let old = read(vec![gadget("Cans", Kind::Headphones, Some(50))]);
        for problem in [Problem::Slow, Problem::Failed, Problem::Unreadable] {
            assert_eq!(learn(old.clone(), Err(problem), 9), old, "{problem:?}");
            assert_eq!(
                learn(Known::Nothing, Err(problem), 9),
                Known::Problem(problem)
            );
            assert_eq!(
                learn(Known::Problem(Problem::Slow), Err(problem), 9),
                Known::Problem(problem)
            );
        }
        let fresh = learn(old, Ok(Vec::new()), 9);
        assert_eq!(
            fresh,
            Known::Read {
                gadgets: Vec::new(),
                at: 9
            },
            "an answer replaces what was known, even with no device"
        );
        assert_eq!(
            learn(Known::Problem(Problem::Slow), Ok(Vec::new()), 9),
            Known::Read {
                gadgets: Vec::new(),
                at: 9
            },
            "and ends a problem"
        );
    }

    #[test]
    fn every_word_is_there_in_both_languages_and_every_symbol_is_one_the_system_knows() {
        for language in LANGUAGES {
            for text in [
                language.devices_tab(),
                language.devices_low(),
                language.devices_silent(),
                language.devices_case(),
                language.devices_none_title(),
                language.devices_none_hint(),
                language.devices_looking_title(),
                language.devices_problem_title(),
                language.devices_problem_hint(),
            ] {
                assert!(!text.is_empty());
            }
        }
        for kind in [
            Kind::Headphones,
            Kind::Speaker,
            Kind::Keyboard,
            Kind::Mouse,
            Kind::Trackpad,
            Kind::Controller,
            Kind::Phone,
            Kind::Other,
        ] {
            let symbol = symbol_of(kind);
            assert!(
                macos_sys::widgets::symbol_png(symbol, 16.0, 1.0).is_some(),
                "{symbol}"
            );
        }
    }
}
