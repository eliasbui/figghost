//! The Day progress tab of the notch: how much of the day has gone, one line from the morning to bedtime with the
//! events of today laid along it and the reminders marked on it, what is on now and what comes next. It is off until the
//! person turns it on.
//!
//! It only reads: the same calendar and reminders the Calendar tab reads, with the calendars that tab leaves out left out
//! here too, and it never asks for access itself (the Calendar page has the button for that). The line is read again while
//! the tab shows and not otherwise, so an idle app does no work for it.

use super::AttachEnv;
use crate::{
    i18n::{self, Language},
    settings::model::Fault,
    trace,
};
use core_calendar::{
    format::{self, Lang},
    model::{Event, Reminder},
};
use logic::{Window, agenda, events_left, hours_minutes, marks, place, tick_hours};
use macos_sys::{
    app,
    dayprogress_ui::{Block, DayDisplay, DayView, Mark, PAGE_SIZE, Row},
    eventkit::{self, Access, Kind, Store},
};
use serde::{Deserialize, Serialize};
use std::{
    cell::{Cell, RefCell},
    rc::{Rc, Weak},
};

pub mod logic;
pub mod settings;
mod text;

/// While the tab shows, the line is read again this often, in seconds.
const TICK_SECONDS: f64 = 30.0;
/// An event starting within this many seconds is said as "in 40 min"; later ones as the time they begin.
const SOON_SECONDS: i64 = 2 * 3600;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    pub enabled: bool,
    /// The hour the line begins at, one of `logic::WAKE`.
    pub wake: u32,
    /// The hour it ends at, one of `logic::BED`; 24 is midnight.
    pub bed: u32,
    /// Reminders are marked on the line and counted.
    pub reminders: bool,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            enabled: false,
            wake: 7,
            bed: 23,
            reminders: true,
        }
    }
}

impl Config {
    /// A config file can say anything: hours that are not offered are the usual day, 7 to 23.
    pub fn sanitized(mut self) -> Self {
        if !logic::WAKE.contains(&self.wake) || !logic::BED.contains(&self.bed) {
            let usual = Self::default();
            (self.wake, self.bed) = (usual.wake, usual.bed);
        }
        self
    }
}

pub struct Feature {
    view: Rc<DayView>,
    config: RefCell<Config>,
    /// Present once the calendar is allowed and the tab has been shown.
    store: RefCell<Option<Store>>,
    /// The calendars the Calendar tab leaves out, which this one leaves out too.
    excluded: RefCell<Vec<String>>,
    events: RefCell<Vec<Event>>,
    reminders: RefCell<Vec<Reminder>>,
    ticker: RefCell<Option<app::Timer>>,
    showing: Cell<bool>,
    /// Bumped by every look; a list of reminders that arrives for another number is old.
    generation: Cell<u64>,
    me: Weak<Feature>,
}

/// Adds the tab. `None` without a notch, which is where the tab lives.
pub fn attach(env: &AttachEnv) -> Option<Rc<Feature>> {
    let notch = env.notch?;
    let view = DayView::new(env.mtm, notch.page_area(PAGE_SIZE.width, PAGE_SIZE.height));
    let feature = Rc::new_cyclic(|me| Feature {
        view: Rc::clone(&view),
        config: RefCell::new(env.config.dayprogress.clone().sanitized()),
        store: RefCell::default(),
        excluded: RefCell::new(env.config.calendar.excluded.clone()),
        events: RefCell::default(),
        reminders: RefCell::default(),
        ticker: RefCell::default(),
        showing: Cell::new(false),
        generation: Cell::new(0),
        me: me.clone(),
    });
    let (shown, hidden) = (Rc::downgrade(&feature), Rc::downgrade(&feature));
    notch.add_page(view.page(
        i18n::current().dayprogress_tab().to_owned(),
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
                trace::log("dayprogress: closed");
            }
        },
    ));
    Some(feature)
}

fn lang(language: Language) -> Lang {
    match language {
        Language::English => Lang::En,
        Language::Vietnamese => Lang::Vi,
    }
}

impl Feature {
    pub fn configure(&self, config: &Config) -> Result<(), Fault> {
        *self.config.borrow_mut() = config.clone().sanitized();
        if !config.enabled {
            // Off keeps nothing: no connection to the calendar and no copy of its events.
            self.store.borrow_mut().take();
            self.events.borrow_mut().clear();
            self.reminders.borrow_mut().clear();
        }
        if self.showing.get() {
            self.begin();
        }
        Ok(())
    }

    /// The Calendar tab's list of calendars that are left out changed.
    pub fn set_excluded(&self, excluded: &[String]) {
        *self.excluded.borrow_mut() = excluded.to_vec();
        if self.showing.get() {
            self.refresh();
        }
    }

    /// The tab opened: read the day, and keep reading it while the tab shows.
    fn begin(&self) {
        self.ticker.borrow_mut().take();
        self.refresh();
        if !self.config.borrow().enabled {
            return;
        }
        let weak = self.me.clone();
        *self.ticker.borrow_mut() = Some(app::every(TICK_SECONDS, move || {
            if let Some(f) = weak.upgrade() {
                f.refresh();
            }
        }));
    }

    /// Reads the events of today and asks for the reminders, then draws.
    fn refresh(&self) {
        let config = self.config.borrow().clone();
        if !config.enabled {
            trace::log("dayprogress: off");
        } else if eventkit::access(Kind::Events) != Access::Granted {
            trace::log("dayprogress: no access");
        } else {
            self.read(&config);
        }
        self.render();
    }

    fn read(&self, config: &Config) {
        if self.store.borrow().is_none() {
            *self.store.borrow_mut() = Some(Store::new());
        }
        let store = self.store.borrow();
        let Some(store) = store.as_ref() else {
            return;
        };
        let (midnight, next) = day_bounds(eventkit::now());
        let excluded = self.excluded.borrow().clone();
        let events = store.events(midnight, next, &excluded);
        trace::log(&format!("dayprogress: read {} events", events.len()));
        *self.events.borrow_mut() = events;
        if config.reminders && eventkit::access(Kind::Reminders) == Access::Granted {
            let generation = self.generation.get() + 1;
            self.generation.set(generation);
            let weak = self.me.clone();
            // Everything not done that is due before tomorrow, which includes what is overdue.
            store.reminders(next, &excluded, move |list| {
                if let Some(f) = weak.upgrade()
                    && f.generation.get() == generation
                {
                    *f.reminders.borrow_mut() = list;
                    f.render();
                }
            });
        } else {
            self.generation.set(self.generation.get() + 1);
            self.reminders.borrow_mut().clear();
        }
    }

    fn render(&self) {
        if !self.showing.get() {
            return;
        }
        let config = self.config.borrow();
        let granted = eventkit::access(Kind::Events) == Access::Granted;
        self.view.show(display(
            i18n::current(),
            &config,
            granted,
            &self.events.borrow(),
            &self.reminders.borrow(),
            eventkit::now(),
        ));
    }
}

/// The instants the local day of `now` begins and ends.
fn day_bounds(now: i64) -> (i64, i64) {
    let today = format::day_index(now, &eventkit::utc_offset);
    (
        format::day_start(today, &eventkit::utc_offset),
        format::day_start(today + 1, &eventkit::utc_offset),
    )
}

fn note(title: &str, hint: &str) -> DayDisplay {
    DayDisplay {
        note: Some((title.to_owned(), hint.to_owned())),
        ..DayDisplay::default()
    }
}

/// What the tab shows for the day it is. Pure, so that the tests drive it with a day and a clock of their own.
fn display(
    language: Language,
    config: &Config,
    granted: bool,
    events: &[Event],
    reminders: &[Reminder],
    now: i64,
) -> DayDisplay {
    if !config.enabled {
        return note(
            language.dayprogress_off_title(),
            language.dayprogress_off_hint(),
        );
    }
    if !granted {
        return note(
            language.dayprogress_access_title(),
            language.dayprogress_access_hint(),
        );
    }
    display_for(language, config, events, reminders, now, day_bounds(now))
}

fn display_for(
    language: Language,
    config: &Config,
    events: &[Event],
    reminders: &[Reminder],
    now: i64,
    (midnight, next_midnight): (i64, i64),
) -> DayDisplay {
    let window = Window::new(midnight, config.wake, config.bed);
    let share = window.share(now);
    let clock = |t: i64| format::clock(lang(language), t, &eventkit::utc_offset);
    let blocks = place(events, window, now)
        .into_iter()
        .map(|p| Block {
            start: p.start,
            end: p.end,
            lane: p.lane,
            color: p.color,
            running: p.running,
        })
        .collect();
    let marks = if config.reminders {
        marks(reminders, window, midnight, next_midnight)
            .into_iter()
            .map(|(at, color)| Mark { at, color })
            .collect()
    } else {
        Vec::new()
    };
    let rows = agenda(events, now)
        .into_iter()
        .map(|e| {
            let running = e.start <= now;
            Row {
                color: e.color,
                title: e.title.clone(),
                when: if running {
                    language.dayprogress_now_until(&clock(e.end))
                } else if e.start - now <= SOON_SECONDS {
                    format::relative(lang(language), e.start, e.end, now)
                        .unwrap_or_else(|| clock(e.start))
                } else {
                    clock(e.start)
                },
                running,
            }
        })
        .collect();
    let ticks = tick_hours(window, config.wake, config.bed)
        .into_iter()
        .map(|(hour, at)| (language.dayprogress_hour(hour), at))
        .collect();
    let (hours, minutes) = hours_minutes(window.to - now.max(window.from));
    DayDisplay {
        percent: format!("{:.0}%", share * 100.0),
        caption: language.dayprogress_caption().to_owned(),
        left: language.dayprogress_left(hours, minutes),
        now: share,
        ticks,
        blocks,
        marks,
        agenda: rows,
        empty: language.dayprogress_nothing_more().to_owned(),
        summary: language.dayprogress_summary(
            events_left(events, now),
            if config.reminders { reminders.len() } else { 0 },
        ),
        note: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const LANGUAGES: [Language; 2] = [Language::English, Language::Vietnamese];
    /// A day of its own, a thousand days after the epoch, whatever zone the machine is in.
    const MIDNIGHT: i64 = 1_000_000 * 86_400;
    const NEXT: i64 = MIDNIGHT + 86_400;

    fn at(hour: f64) -> i64 {
        MIDNIGHT + (hour * 3600.0) as i64
    }

    fn on() -> Config {
        Config {
            enabled: true,
            ..Config::default()
        }
    }

    fn event(title: &str, from: f64, to: f64) -> Event {
        Event {
            id: title.into(),
            title: title.into(),
            start: at(from),
            end: at(to),
            all_day: false,
            calendar_id: "c".into(),
            calendar: "Work".into(),
            color: 0x3478f6,
            location: String::new(),
            notes: String::new(),
            url: String::new(),
        }
    }

    fn reminder(due: Option<f64>) -> Reminder {
        Reminder {
            id: "r".into(),
            title: "t".into(),
            due: due.map(at),
            date_only: false,
            calendar: "Reminders".into(),
            color: 0xffd60a,
        }
    }

    fn shown(
        language: Language,
        config: &Config,
        events: &[Event],
        reminders: &[Reminder],
        now: f64,
    ) -> DayDisplay {
        display_for(
            language,
            config,
            events,
            reminders,
            at(now),
            (MIDNIGHT, NEXT),
        )
    }

    #[test]
    fn the_feature_is_off_by_default_and_old_files_load() {
        let config: Config = serde_json::from_str("{}").unwrap();
        assert_eq!(config, Config::default());
        assert!(!config.enabled && config.reminders && (config.wake, config.bed) == (7, 23));
        let partial: Config =
            serde_json::from_str(r#"{ "enabled": true, "wake": 6, "future_key": 1 }"#).unwrap();
        assert!(partial.enabled && partial.wake == 6 && partial.bed == 23);
    }

    #[test]
    fn hours_that_are_not_offered_make_the_usual_day() {
        for (wake, bed) in [(3, 23), (7, 99), (0, 0), (u32::MAX, 24)] {
            let config = Config {
                wake,
                bed,
                ..Config::default()
            }
            .sanitized();
            assert_eq!((config.wake, config.bed), (7, 23), "{wake} {bed}");
        }
        let kept = Config {
            wake: 5,
            bed: 24,
            ..Config::default()
        }
        .sanitized();
        assert_eq!((kept.wake, kept.bed), (5, 24));
    }

    #[test]
    fn an_off_tab_and_a_calendar_that_is_not_allowed_say_so_in_both_languages() {
        for language in LANGUAGES {
            let off = display(language, &Config::default(), true, &[], &[], at(12.0));
            assert_eq!(off.note.unwrap().0, language.dayprogress_off_title());
            let denied = display(language, &on(), false, &[], &[], at(12.0));
            assert_eq!(denied.note.unwrap().0, language.dayprogress_access_title());
        }
    }

    #[test]
    fn the_figures_follow_the_clock_across_the_line() {
        let morning = shown(Language::English, &on(), &[], &[], 7.0);
        assert_eq!(
            (morning.percent.as_str(), morning.left.as_str()),
            ("0%", "16 h left")
        );
        let noon = shown(Language::English, &on(), &[], &[], 15.0);
        assert_eq!(
            (noon.percent.as_str(), noon.left.as_str()),
            ("50%", "8 h left")
        );
        assert!((noon.now - 0.5).abs() < 1e-9);
        let late = shown(Language::English, &on(), &[], &[], 22.5);
        assert_eq!(late.percent, "97%");
        assert_eq!(late.left, "30 min left");
        let night = shown(Language::English, &on(), &[], &[], 23.5);
        assert_eq!(
            (night.percent.as_str(), night.left.as_str()),
            ("100%", "Day done")
        );
        let early = shown(Language::Vietnamese, &on(), &[], &[], 4.0);
        assert_eq!(
            (early.percent.as_str(), early.left.as_str()),
            ("0%", "còn 16 giờ")
        );
    }

    #[test]
    fn what_is_on_and_what_comes_next_are_listed_with_when() {
        let events = [
            event("Standup", 9.0, 9.5),
            event("Focus time", 14.0, 16.0),
            event("Client call", 14.75, 15.5),
            event("Dinner", 19.0, 20.5),
        ];
        let d = shown(Language::English, &on(), &events, &[], 14.25);
        let titles: Vec<&str> = d.agenda.iter().map(|r| r.title.as_str()).collect();
        assert_eq!(titles, ["Focus time", "Client call", "Dinner"]);
        assert!(d.agenda[0].running && !d.agenda[1].running);
        assert!(
            d.agenda[0].when.starts_with("now, until "),
            "{}",
            d.agenda[0].when
        );
        assert_eq!(d.agenda[1].when, "in 30 min");
        assert!(
            d.agenda[2].when.contains("PM") || d.agenda[2].when.contains(':'),
            "a distant event gives its time: {}",
            d.agenda[2].when
        );
        assert_eq!(d.summary, "3 events to go");
        assert_eq!(d.blocks.len(), 4);
        assert!(d.blocks.iter().any(|b| b.running));
        let vi = shown(
            Language::Vietnamese,
            &on(),
            &events,
            &[reminder(Some(16.0))],
            14.25,
        );
        assert_eq!(vi.summary, "3 sự kiện nữa, 1 nhắc việc còn lại");
        assert_eq!(vi.agenda[1].when, "sau 30 phút");
    }

    #[test]
    fn reminders_are_marked_and_counted_only_when_they_are_wanted() {
        let list = [reminder(Some(10.0)), reminder(Some(16.0)), reminder(None)];
        let with = shown(Language::English, &on(), &[], &list, 12.0);
        assert_eq!(with.marks.len(), 2);
        assert_eq!(with.summary, "3 reminders left");
        let without = shown(
            Language::English,
            &Config {
                reminders: false,
                ..on()
            },
            &[],
            &list,
            12.0,
        );
        assert!(without.marks.is_empty());
        assert_eq!(without.summary, "", "nothing to say when nothing is left");
        let one = shown(
            Language::English,
            &on(),
            &[event("A", 13.0, 14.0)],
            &[reminder(Some(10.0))],
            12.0,
        );
        assert_eq!(one.summary, "1 event to go, 1 reminder left");
    }

    #[test]
    fn a_day_with_nothing_left_says_so() {
        for language in LANGUAGES {
            let d = shown(language, &on(), &[event("Early", 8.0, 9.0)], &[], 18.0);
            assert!(d.agenda.is_empty());
            assert_eq!(d.empty, language.dayprogress_nothing_more());
        }
    }

    #[test]
    fn the_hours_are_labelled_in_the_language() {
        let en = shown(Language::English, &on(), &[], &[], 12.0);
        assert_eq!(
            en.ticks.iter().map(|t| t.0.as_str()).collect::<Vec<_>>(),
            ["7 AM", "10 AM", "1 PM", "4 PM", "7 PM", "10 PM"]
        );
        let vi = shown(Language::Vietnamese, &on(), &[], &[], 12.0);
        assert_eq!(
            vi.ticks.iter().map(|t| t.0.as_str()).collect::<Vec<_>>(),
            ["7h", "10h", "13h", "16h", "19h", "22h"]
        );
        let late = shown(
            Language::English,
            &Config {
                wake: 6,
                bed: 24,
                ..on()
            },
            &[],
            &[],
            12.0,
        );
        assert_eq!(
            late.ticks.last().unwrap().0,
            "12 AM",
            "bedtime at midnight is said as 12 AM"
        );
    }

    #[test]
    fn every_word_is_there_in_both_languages_and_the_symbol_is_one_the_system_knows() {
        for language in LANGUAGES {
            for text in [
                language.dayprogress_tab(),
                language.dayprogress_caption(),
                language.dayprogress_nothing_more(),
                language.dayprogress_off_title(),
                language.dayprogress_off_hint(),
                language.dayprogress_access_title(),
                language.dayprogress_access_hint(),
            ] {
                assert!(!text.is_empty());
            }
            assert!(language.dayprogress_now_until("4 PM").contains("4 PM"));
            assert!(language.dayprogress_left(1, 5).contains('5'));
        }
        assert!(macos_sys::widgets::symbol_png("sun.horizon.fill", 16.0, 1.0).is_some());
    }
}
