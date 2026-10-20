//! The Screen Time tab of the notch: how long each app was in front today and over the last week, by app and by the
//! group an app says it belongs to. It is off until the person turns it on.
//!
//! Every five seconds, while the feature is on and the keyboard or pointer was touched within the time chosen, the app in
//! front is given those seconds. What is kept is the name and bundle identifier of the app and the seconds, for the last thirty
//! days, in a file of the data directory. No window title, no text, no screen and no keystroke is looked at, and nothing
//! leaves the Mac. No permission is needed: the app in front is public, and how long since the last input is read from the
//! clock of the input system, not from the events.

use super::{AttachEnv, MAX_FILE, read_small, write_atomically};
use crate::{
    i18n::{self, Language},
    settings::model::Fault,
    trace,
};
use core_calendar::format::day_index;
use logic::{IDLE_MINUTES, SAMPLE_SECONDS, Tally};
use macos_sys::{
    app, eventkit,
    screentime_ui::{
        Icon, MAX_ROWS, PAGE_SIZE, Row, ScreenTimeCallbacks, ScreenTimeDisplay, ScreenTimeView,
    },
    usage, workspace,
};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::{
    cell::{Cell, RefCell},
    collections::HashMap,
    fs, io,
    path::PathBuf,
    rc::{Rc, Weak},
    time::Instant,
};

pub mod logic;
pub mod settings;
mod text;

const FILE: &str = "screentime.json";
/// The tally is written this many looks apart, a minute.
const SAVE_EVERY: u32 = 12;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    pub enabled: bool,
    /// How long the keyboard and pointer may be still before time stops counting, one of `IDLE_MINUTES`.
    pub idle_minutes: u32,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            enabled: false,
            idle_minutes: 2,
        }
    }
}

impl Config {
    /// A config file can say anything; a time that is not offered is read as the default.
    pub fn sanitized(mut self) -> Self {
        if !IDLE_MINUTES.contains(&self.idle_minutes) {
            self.idle_minutes = Self::default().idle_minutes;
        }
        self
    }
}

pub struct Feature {
    view: Rc<ScreenTimeView>,
    config: RefCell<Config>,
    tally: RefCell<Tally>,
    file: PathBuf,
    own_id: Option<String>,
    sampler: RefCell<Option<app::Timer>>,
    last_look: Cell<Option<Instant>>,
    looks_since_save: Cell<u32>,
    dirty: Cell<bool>,
    showing: Cell<bool>,
    /// 0 is today, 1 the last seven days.
    range: Cell<usize>,
    /// 0 is by app, 1 by group.
    group: Cell<usize>,
    /// The group of each app already asked about, so that an app's file is read once.
    groups: RefCell<HashMap<String, String>>,
    me: Weak<Feature>,
}

/// Adds the tab. `None` without a notch, which is where the tab lives.
pub fn attach(env: &AttachEnv) -> Option<Rc<Feature>> {
    let notch = env.notch?;
    let config = env.config.screentime.clone().sanitized();
    let feature = Rc::new_cyclic(|me: &Weak<Feature>| {
        let (range, group) = (me.clone(), me.clone());
        let view = ScreenTimeView::new(
            env.mtm,
            notch.page_area(PAGE_SIZE.width, PAGE_SIZE.height),
            ScreenTimeCallbacks {
                on_range: Box::new(move |i| {
                    if let Some(f) = range.upgrade() {
                        f.range.set(i.min(1));
                        f.render();
                    }
                }),
                on_group: Box::new(move |i| {
                    if let Some(f) = group.upgrade() {
                        f.group.set(i.min(1));
                        f.render();
                    }
                }),
            },
        );
        Feature {
            view,
            config: RefCell::new(config),
            tally: RefCell::default(),
            file: env.home.join(FILE),
            own_id: workspace::own_bundle_id(),
            sampler: RefCell::default(),
            last_look: Cell::new(None),
            looks_since_save: Cell::new(0),
            dirty: Cell::new(false),
            showing: Cell::new(false),
            range: Cell::new(0),
            group: Cell::new(0),
            groups: RefCell::default(),
            me: me.clone(),
        }
    });
    let (shown, hidden) = (Rc::downgrade(&feature), Rc::downgrade(&feature));
    notch.add_page(feature.view.page(
        i18n::current().screentime_tab().to_owned(),
        move || {
            if let Some(f) = shown.upgrade() {
                f.showing.set(true);
                f.render();
            }
        },
        move || {
            if let Some(f) = hidden.upgrade() {
                f.showing.set(false);
                // Closing the tab is a quiet moment to write what has been counted since the last time.
                f.save();
            }
        },
    ));
    if feature.config.borrow().enabled {
        feature.start();
    }
    Some(feature)
}

fn today() -> i64 {
    day_index(eventkit::now(), &eventkit::utc_offset)
}

impl Feature {
    pub fn configure(&self, config: &Config) -> Result<(), Fault> {
        let config = config.clone().sanitized();
        let (was, now) = (self.config.borrow().enabled, config.enabled);
        *self.config.borrow_mut() = config;
        match (was, now) {
            (false, true) => self.start(),
            (true, false) => self.stop(),
            _ => {}
        }
        self.render();
        Ok(())
    }

    /// Facts only the running app knows, for the settings page.
    pub fn runtime(&self) -> Value {
        let tally = self.tally.borrow();
        let today = today();
        json!({
            "days": tally.days_with_data(),
            "today": tally.total(today, today),
        })
    }

    /// The buttons of the settings page. `None` when the name is not this feature's.
    pub fn action(&self, name: &str) -> Option<Result<(), Fault>> {
        if name != "screentime_clear" {
            return None;
        }
        self.tally.borrow_mut().clear();
        self.groups.borrow_mut().clear();
        self.dirty.set(false);
        if let Err(e) = fs::remove_file(&self.file)
            && e.kind() != io::ErrorKind::NotFound
        {
            eprintln!("screentime: cannot remove the file: {e}");
        }
        self.render();
        Some(Ok(()))
    }

    /// Reads what was kept and starts looking at the app in front.
    fn start(&self) {
        let mut tally = read_small(&self.file, MAX_FILE)
            .map(|text| Tally::from_json(&text))
            .unwrap_or_default();
        tally.prune(today());
        *self.tally.borrow_mut() = tally;
        self.last_look.set(Some(Instant::now()));
        let weak = self.me.clone();
        *self.sampler.borrow_mut() = Some(app::every(SAMPLE_SECONDS as f64, move || {
            if let Some(f) = weak.upgrade() {
                f.look();
            }
        }));
        trace::log("screentime: counting");
    }

    /// Stops looking, writes what is not written, and lets the tally go from memory.
    fn stop(&self) {
        self.sampler.borrow_mut().take();
        self.last_look.set(None);
        self.save();
        self.tally.borrow_mut().clear();
        self.groups.borrow_mut().clear();
        trace::log("screentime: stopped");
    }

    /// One look: gives the seconds since the last to the app in front, if someone is there.
    fn look(&self) {
        let now = Instant::now();
        let elapsed = self
            .last_look
            .replace(Some(now))
            .map_or(0.0, |before| now.duration_since(before).as_secs_f64());
        let seconds = logic::step(elapsed);
        let idle_minutes = self.config.borrow().idle_minutes;
        if seconds > 0
            && logic::is_active(usage::idle_seconds(), idle_minutes)
            && let Some(front) = workspace::frontmost_app()
            && !front.bundle_id.is_empty()
            && self.own_id.as_deref() != Some(front.bundle_id.as_str())
        {
            self.tally
                .borrow_mut()
                .add(today(), &front.bundle_id, &front.name, seconds);
            self.dirty.set(true);
            trace::log("screentime: counted a look");
        }
        let looks = self.looks_since_save.get() + 1;
        if looks >= SAVE_EVERY {
            self.looks_since_save.set(0);
            self.save();
        } else {
            self.looks_since_save.set(looks);
        }
        if self.showing.get() {
            self.render();
        }
    }

    /// Writes the tally whole or not at all, if it changed.
    fn save(&self) {
        if !self.dirty.replace(false) {
            return;
        }
        let mut tally = self.tally.borrow_mut();
        tally.prune(today());
        if let Err(e) = write_atomically(&self.file, &tally.to_json()) {
            eprintln!("screentime: cannot write the tally: {e}");
        }
    }

    /// The group an app is in, asked of its file once.
    fn group_of(&self, id: &str) -> String {
        if let Some(known) = self.groups.borrow().get(id) {
            return known.clone();
        }
        let group = logic::group_of(usage::app_category(id).as_deref());
        self.groups
            .borrow_mut()
            .insert(id.to_owned(), group.clone());
        group
    }

    fn render(&self) {
        let language = i18n::current();
        let enabled = self.config.borrow().enabled;
        if !enabled {
            self.view.show(ScreenTimeDisplay {
                note: Some((
                    language.screentime_off_title().to_owned(),
                    language.screentime_off_hint().to_owned(),
                )),
                ..ScreenTimeDisplay::default()
            });
            return;
        }
        let tally = self.tally.borrow();
        let (range, group) = (self.range.get(), self.group.get());
        let day = today();
        let from = if range == 0 { day } else { day - 6 };
        self.view.show(display(
            language,
            &tally,
            (from, day),
            (range, group),
            |id| self.group_of(id),
        ));
    }
}

/// What the tab shows for a tally, a range of days and a way of grouping. Pure, so that the tests drive it.
fn display(
    language: Language,
    tally: &Tally,
    (from, to): (i64, i64),
    (range, group): (usize, usize),
    group_of: impl Fn(&str) -> String,
) -> ScreenTimeDisplay {
    let total = tally.total(from, to);
    let listed: Vec<(String, u64)> = if group == 0 {
        tally.by_app(from, to)
    } else {
        tally.by_group(from, to, group_of)
    };
    let longest = listed.first().map_or(1, |(_, s)| (*s).max(1));
    let rows = listed
        .into_iter()
        .take(MAX_ROWS)
        .map(|(key, seconds)| Row {
            icon: if group == 0 {
                workspace::app_path(&key).map_or(Icon::None, Icon::App)
            } else {
                Icon::Symbol(text::group_symbol(&key))
            },
            title: if group == 0 {
                tally.name_of(&key).to_owned()
            } else {
                language.screentime_group(&key).to_owned()
            },
            value: language.screentime_duration(seconds),
            fraction: seconds as f64 / longest as f64,
        })
        .collect();
    ScreenTimeDisplay {
        caption: if range == 0 {
            language.screentime_today().to_owned()
        } else {
            language.screentime_week_caption().to_owned()
        },
        total: language.screentime_duration(total),
        rows,
        empty: language.screentime_empty().to_owned(),
        ranges: [
            language.screentime_today().to_owned(),
            language.screentime_week().to_owned(),
        ],
        groups: [
            language.screentime_apps().to_owned(),
            language.screentime_groups().to_owned(),
        ],
        range,
        group,
        note: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const LANGUAGES: [Language; 2] = [Language::English, Language::Vietnamese];

    fn tally() -> Tally {
        let mut t = Tally::default();
        t.add(100, "com.apple.Safari", "Safari", 3_600);
        t.add(100, "com.apple.Terminal", "Terminal", 5_400);
        t.add(99, "com.apple.Safari", "Safari", 600);
        t.add(93, "com.apple.mail", "Mail", 9_000);
        t
    }

    #[test]
    fn the_feature_is_off_by_default_and_old_files_load() {
        let config: Config = serde_json::from_str("{}").unwrap();
        assert_eq!(config, Config::default());
        assert!(!config.enabled && config.idle_minutes == 2);
        let partial: Config =
            serde_json::from_str(r#"{ "enabled": true, "future_key": 1 }"#).unwrap();
        assert!(partial.enabled);
    }

    #[test]
    fn an_idle_time_that_is_not_offered_is_read_as_the_default() {
        for minutes in IDLE_MINUTES {
            let config = Config {
                idle_minutes: minutes,
                ..Config::default()
            };
            assert_eq!(config.sanitized().idle_minutes, minutes);
        }
        for minutes in [0, 3, 10, u32::MAX] {
            let config = Config {
                idle_minutes: minutes,
                ..Config::default()
            };
            assert_eq!(config.sanitized().idle_minutes, 2, "{minutes}");
        }
    }

    #[test]
    fn today_lists_the_apps_of_today_longest_first_with_bars_against_the_longest() {
        let shown = display(Language::English, &tally(), (100, 100), (0, 0), |_| {
            String::new()
        });
        assert_eq!(shown.total, "2 h 30 min");
        assert_eq!(shown.caption, "Today");
        let titles: Vec<&str> = shown.rows.iter().map(|r| r.title.as_str()).collect();
        assert_eq!(titles, ["Terminal", "Safari"]);
        assert_eq!(shown.rows[0].value, "1 h 30 min");
        assert!((shown.rows[0].fraction - 1.0).abs() < 1e-9);
        assert!((shown.rows[1].fraction - 2.0 / 3.0).abs() < 1e-9);
        assert_eq!((shown.range, shown.group), (0, 0));
    }

    #[test]
    fn the_week_adds_the_last_seven_days_and_not_the_eighth() {
        let shown = display(Language::English, &tally(), (94, 100), (1, 0), |_| {
            String::new()
        });
        assert_eq!(shown.caption, "Last 7 days");
        assert_eq!(
            shown.total, "2 h 40 min",
            "day 93 is eight days back and is left out"
        );
        assert_eq!(shown.rows.len(), 2);
        assert_eq!(shown.rows[1].value, "1 h 10 min");
    }

    #[test]
    fn groups_add_up_the_apps_that_say_they_are_alike_and_name_them_in_the_language() {
        let by = |id: &str| {
            if id.contains("Safari") || id.contains("Terminal") {
                "utilities".to_owned()
            } else {
                String::new()
            }
        };
        let shown = display(Language::Vietnamese, &tally(), (99, 100), (1, 1), by);
        assert_eq!(shown.rows.len(), 1);
        assert_eq!(shown.rows[0].title, "Tiện ích");
        assert_eq!(shown.rows[0].value, "2 giờ 40 phút");
        assert!(matches!(
            shown.rows[0].icon,
            Icon::Symbol("wrench.and.screwdriver")
        ));
        let other = display(Language::English, &tally(), (93, 93), (1, 1), |_| {
            String::new()
        });
        assert_eq!(other.rows[0].title, "Other");
        assert!(matches!(
            other.rows[0].icon,
            Icon::Symbol("questionmark.circle")
        ));
    }

    #[test]
    fn a_day_with_nothing_shows_no_rows_and_says_what_it_counts() {
        for language in LANGUAGES {
            let shown = display(language, &Tally::default(), (1, 1), (0, 0), |_| {
                String::new()
            });
            assert!(shown.rows.is_empty());
            assert_eq!(shown.total, language.screentime_duration(0));
            assert!(!shown.empty.is_empty());
        }
    }

    #[test]
    fn at_most_six_rows_are_listed() {
        let mut t = Tally::default();
        for i in 0..20 {
            t.add(1, &format!("com.example.app{i:02}"), "x", 60 + i);
        }
        let shown = display(Language::English, &t, (1, 1), (0, 0), |_| String::new());
        assert_eq!(shown.rows.len(), MAX_ROWS);
    }

    #[test]
    fn a_length_of_time_is_said_whole_in_both_languages() {
        assert_eq!(Language::English.screentime_duration(0), "0 min");
        assert_eq!(Language::English.screentime_duration(20), "<1 min");
        assert_eq!(Language::English.screentime_duration(59 * 60), "59 min");
        assert_eq!(Language::English.screentime_duration(13_320), "3 h 42 min");
        assert_eq!(Language::Vietnamese.screentime_duration(20), "<1 phút");
        assert_eq!(
            Language::Vietnamese.screentime_duration(3_600),
            "1 giờ 0 phút"
        );
    }

    #[test]
    fn every_group_has_a_name_in_each_language_and_a_glyph_the_system_knows() {
        for language in LANGUAGES {
            for code in [
                "business",
                "developer-tools",
                "games",
                "productivity",
                "utilities",
                "nonsense",
                "",
            ] {
                assert!(!language.screentime_group(code).is_empty());
            }
            assert_ne!(
                language.screentime_group("music"),
                language.screentime_group("nonsense")
            );
        }
        for symbol in text::all_group_symbols() {
            assert!(
                macos_sys::widgets::symbol_png(symbol, 16.0, 1.0).is_some(),
                "{symbol} is not a symbol of this system"
            );
        }
    }

    #[test]
    fn the_tally_is_written_whole_and_leaves_no_half_written_file() {
        let dir = std::env::temp_dir().join(format!("figghost-screentime-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        let file = dir.join(FILE);
        let t = tally();
        write_atomically(&file, &t.to_json()).unwrap();
        assert_eq!(Tally::from_json(&fs::read_to_string(&file).unwrap()), t);
        let mut names: Vec<String> = fs::read_dir(&dir)
            .unwrap()
            .flatten()
            .map(|e| e.file_name().to_string_lossy().into_owned())
            .collect();
        names.sort();
        assert_eq!(names, [FILE]);
        fs::remove_dir_all(&dir).unwrap();
    }
}
