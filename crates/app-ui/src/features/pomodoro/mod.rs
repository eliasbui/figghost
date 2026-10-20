//! The Pomodoro tab of the notch: work sessions and breaks one after the other, a ring that shows how far a stage has
//! got, a notice and a sound when it ends, the countdown in the closed notch while it runs, and a count of the sessions
//! done today, the days in a row and the week. It is off until the person turns it on.
//!
//! It is a tab of its own beside Focus, which stays the plain countdown it was. The minutes are counted by the same
//! timer as Focus (wall-clock time, so that a stage keeps going while the Mac sleeps), and only run while a stage is
//! running, so an idle app does no work for it. What is kept is the number of sessions and minutes of each of the last
//! thirty days, in the file `pomodoro.json` of the data directory; nothing leaves the Mac.

use super::{AttachEnv, write_atomically};
use crate::{
    config::parse_hotkey,
    focus_timer::{Phase, Snapshot, clock_text},
    history::now_ms,
    i18n::{self, Language},
    menu::symbols,
    settings::model::Fault,
    trace,
};
use core_calendar::format::day_index;
use logic::{Cycle, Machine, Plan, Stage, Stats, bar_height};
use macos_sys::{
    MainThreadMarker, app, eventkit,
    hotkey::Hotkey,
    notch::{Activity, Ear, Notch},
    pomodoro_ui::{Hue, Labels, PAGE_SIZE, PomodoroCallbacks, PomodoroDisplay, PomodoroView},
    status_item::{Entry, Item},
    style::Tone,
    workspace,
};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::{
    cell::{Cell, RefCell},
    fs, io,
    path::PathBuf,
    rc::{Rc, Weak},
};

pub mod logic;
pub mod settings;
mod text;

const FILE: &str = "pomodoro.json";
const SLOT: &str = "pomodoro";
/// Above the music that may be playing, below a keep-awake: a timer that was started on purpose is worth a glance.
const PRIORITY: u8 = 21;
const TICK_SECONDS: f64 = 0.25;
/// The notch that a finished stage opened goes away again after this long, unless the pointer is on it.
const PEEK_SECONDS: f64 = 8.0;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    pub enabled: bool,
    pub work: u32,
    pub short_break: u32,
    pub long_break: u32,
    /// How many work sessions come before a long break.
    pub rounds: u32,
    /// The next stage starts by itself when one ends, instead of waiting to be started.
    pub auto_start: bool,
    pub sound: bool,
    /// The notch opens on this tab when a stage ends.
    pub open_on_end: bool,
    /// The countdown shows in the closed notch while a stage runs.
    pub closed_notch: bool,
    /// Starts and pauses it from any app; none by default.
    pub shortcut: String,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            enabled: false,
            work: 25,
            short_break: 5,
            long_break: 15,
            rounds: 4,
            auto_start: false,
            sound: true,
            open_on_end: true,
            closed_notch: true,
            shortcut: String::new(),
        }
    }
}

impl Config {
    /// A config file can say anything: a length that is not offered is the usual one, a shortcut that cannot be
    /// parsed is none.
    pub fn sanitized(mut self) -> Self {
        let held = self.plan().held();
        self.work = held.work;
        self.short_break = held.short_break;
        self.long_break = held.long_break;
        self.rounds = held.rounds;
        if !self.shortcut.is_empty() && parse_hotkey(&self.shortcut).is_err() {
            self.shortcut.clear();
        }
        self
    }

    fn plan(&self) -> Plan {
        Plan {
            work: self.work,
            short_break: self.short_break,
            long_break: self.long_break,
            rounds: self.rounds,
        }
    }
}

pub struct Feature {
    mtm: MainThreadMarker,
    notch: Weak<Notch>,
    /// The number of this tab in the notch.
    page: usize,
    view: Rc<PomodoroView>,
    config: RefCell<Config>,
    /// Where in the rhythm, the countdown of the stage, and the statistics.
    machine: RefCell<Machine>,
    file: PathBuf,
    ticker: RefCell<Option<app::Timer>>,
    hotkey: RefCell<Option<Hotkey>>,
    /// What is on the closed notch, so that it is drawn again only when it changes.
    on_notch: RefCell<String>,
    showing: Cell<bool>,
    /// The notch was closed when a finished stage opened it, so it may be closed again.
    opened_by_us: Cell<bool>,
    me: Weak<Feature>,
}

/// Adds the tab. `None` without a notch, which is where the tab lives.
pub fn attach(env: &AttachEnv) -> Option<Rc<Feature>> {
    let notch = env.notch?;
    let config = env.config.pomodoro.clone().sanitized();
    let feature = Rc::new_cyclic(|me: &Weak<Feature>| {
        let (toggle, skip, reset) = (me.clone(), me.clone(), me.clone());
        let view = PomodoroView::new(
            env.mtm,
            notch.page_area(PAGE_SIZE.width, PAGE_SIZE.height),
            PomodoroCallbacks {
                on_toggle: Box::new(move || {
                    if let Some(f) = toggle.upgrade() {
                        f.toggle();
                    }
                }),
                on_skip: Box::new(move || {
                    if let Some(f) = skip.upgrade() {
                        f.skip();
                    }
                }),
                on_reset: Box::new(move || {
                    if let Some(f) = reset.upgrade() {
                        f.reset();
                    }
                }),
            },
        );
        Feature {
            mtm: env.mtm,
            notch: Rc::downgrade(notch),
            page: notch.page_count(),
            view,
            machine: RefCell::new(Machine::new(config.plan())),
            config: RefCell::new(config),
            file: env.home.join(FILE),
            ticker: RefCell::default(),
            hotkey: RefCell::default(),
            on_notch: RefCell::default(),
            showing: Cell::new(false),
            opened_by_us: Cell::new(false),
            me: me.clone(),
        }
    });
    let (shown, hidden) = (Rc::downgrade(&feature), Rc::downgrade(&feature));
    notch.add_page(feature.view.page(
        i18n::current().pomodoro_tab().to_owned(),
        move || {
            if let Some(f) = shown.upgrade() {
                f.showing.set(true);
                f.render();
            }
        },
        move || {
            if let Some(f) = hidden.upgrade() {
                f.showing.set(false);
            }
        },
    ));
    if feature.config.borrow().enabled {
        feature.load_stats();
        let config = feature.config.borrow().clone();
        if let Err(fault) = feature.register_hotkey(&config) {
            eprintln!("pomodoro: cannot register its shortcut ({})", fault.detail);
        }
    }
    Some(feature)
}

fn today() -> i64 {
    day_index(eventkit::now(), &eventkit::utc_offset)
}

impl Feature {
    /// Applies the settings: new lengths for what has not begun, the shortcut, and everything let go when it is off.
    pub fn configure(&self, config: &Config) -> Result<(), Fault> {
        let config = config.clone().sanitized();
        let was = self.config.replace(config.clone());
        self.hotkey.borrow_mut().take();
        if !config.enabled {
            self.shut();
            self.render();
            return Ok(());
        }
        if !was.enabled {
            self.load_stats();
        }
        self.machine.borrow_mut().set_plan(config.plan(), now_ms());
        self.paint();
        self.register_hotkey(&config)
    }

    fn register_hotkey(&self, config: &Config) -> Result<(), Fault> {
        if config.shortcut.is_empty() {
            return Ok(());
        }
        let spec = parse_hotkey(&config.shortcut).map_err(|e| Fault::new("invalid_shortcut", e))?;
        let weak = self.me.clone();
        let hotkey = Hotkey::register(self.mtm, spec.key_code, spec.modifiers, move || {
            if let Some(f) = weak.upgrade() {
                f.toggle();
            }
        })
        .map_err(|status| Fault::new("taken", format!("pomodoro (status {status})")))?;
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
                "pomodoro: cannot register its shortcut again ({})",
                fault.detail
            );
        }
    }

    /// Facts only the running app knows, for the settings page.
    pub fn runtime(&self) -> Value {
        let (machine, day) = (self.machine.borrow(), today());
        json!({
            "today": machine.stats.on(day).0,
            "streak": machine.stats.streak(day),
            "days": machine.stats.days_with_data(),
        })
    }

    /// The buttons of the settings page. `None` when the name is not this feature's.
    pub fn action(&self, name: &str) -> Option<Result<(), Fault>> {
        if name != "pomodoro_clear" {
            return None;
        }
        self.machine.borrow_mut().stats.clear();
        if let Err(e) = fs::remove_file(&self.file)
            && e.kind() != io::ErrorKind::NotFound
        {
            eprintln!("pomodoro: cannot remove the file: {e}");
        }
        self.render();
        Some(Ok(()))
    }

    /// The entry of the menu bar panel: starts, pauses or resumes.
    pub fn menu_entries(this: &Rc<Self>) -> Vec<Entry> {
        let (act, title, shown) = (this.me.clone(), this.me.clone(), this.me.clone());
        vec![Entry::Item(
            Item::new(i18n::current().pomodoro_menu_start(), move || {
                if let Some(f) = act.upgrade() {
                    f.toggle();
                }
            })
            .symbol(symbols::POMODORO)
            .titled(move || {
                let language = i18n::current();
                let Some(f) = title.upgrade() else {
                    return language.pomodoro_menu_start().to_owned();
                };
                let snapshot = f.snapshot();
                match snapshot.phase {
                    Phase::Running => {
                        language.pomodoro_menu_pause(&clock_text(snapshot.remaining_secs))
                    }
                    Phase::Paused => language.pomodoro_menu_resume().to_owned(),
                    _ => language.pomodoro_menu_start().to_owned(),
                }
            })
            .visible_when(move || shown.upgrade().is_some_and(|f| f.config.borrow().enabled)),
        )]
    }

    fn snapshot(&self) -> Snapshot {
        self.machine.borrow().snapshot(now_ms())
    }

    // -- the buttons -------------------------------------------------------------------------------------------

    /// Starts a stage, pauses one that runs, resumes one that is paused.
    pub fn toggle(&self) {
        if !self.config.borrow().enabled {
            return;
        }
        self.machine.borrow_mut().toggle(now_ms());
        self.sync();
    }

    /// Moves to the next stage without counting the one that was on, and starts it only when stages start by
    /// themselves.
    fn skip(&self) {
        let auto = self.config.borrow().auto_start;
        self.machine.borrow_mut().skip(now_ms(), auto);
        self.sync();
    }

    /// Back to the first session of a new round, not started.
    fn reset(&self) {
        self.machine.borrow_mut().reset();
        self.sync();
    }

    // -- time --------------------------------------------------------------------------------------------------

    /// Runs the ticker exactly while a stage runs, and draws what changed.
    fn sync(&self) {
        let running = self.machine.borrow().is_running();
        {
            let mut ticker = self.ticker.borrow_mut();
            match (running, ticker.is_some()) {
                (true, false) => {
                    trace::log("pomodoro: running");
                    let weak = self.me.clone();
                    *ticker = Some(app::every(TICK_SECONDS, move || {
                        if let Some(f) = weak.upgrade() {
                            f.tick();
                        }
                    }));
                }
                (false, true) => {
                    trace::log("pomodoro: stopped");
                    ticker.take();
                }
                _ => {}
            }
        }
        self.paint();
    }

    fn tick(&self) {
        let auto = self.config.borrow().auto_start;
        let ended = self.machine.borrow_mut().tick(now_ms(), today(), auto);
        match ended {
            Some(ended) => self.finished(ended),
            None => self.paint(),
        }
    }

    /// A stage ran to its end: a work session was counted (and is written down), and the person is told.
    fn finished(&self, ended: logic::Ended) {
        if ended.counted
            && let Err(e) = write_atomically(&self.file, &self.machine.borrow().stats.to_json())
        {
            eprintln!("pomodoro: cannot write the statistics: {e}");
        }
        trace::log(&format!("pomodoro: {:?} ended", ended.stage));
        self.sync();
        self.announce(ended.stage);
    }

    /// The sound and the open notch that tell a stage is over.
    fn announce(&self, was: Stage) {
        let config = self.config.borrow().clone();
        if config.sound {
            workspace::play_sound(if was == Stage::Work { "Glass" } else { "Ping" });
        }
        if config.open_on_end
            && let Some(notch) = self.notch.upgrade()
        {
            if !notch.is_expanded() {
                self.opened_by_us.set(true);
            }
            notch.open_page(self.page);
            let weak = self.me.clone();
            app::after(PEEK_SECONDS, move || {
                if let Some(f) = weak.upgrade() {
                    f.close_if_ours();
                }
            });
        }
    }

    /// Closes the notch if the end of a stage opened it and the pointer is elsewhere.
    fn close_if_ours(&self) {
        let Some(notch) = self.notch.upgrade() else {
            return;
        };
        if self.opened_by_us.get() && notch.is_expanded() && !notch.pointer_inside() {
            notch.close();
        }
        self.opened_by_us.set(false);
    }

    /// The feature is off: nothing runs, nothing shows on the notch, and the rhythm starts over.
    fn shut(&self) {
        self.ticker.borrow_mut().take();
        *self.machine.borrow_mut() = Machine::new(self.config.borrow().plan());
        self.clear_notch();
    }

    // -- what shows --------------------------------------------------------------------------------------------

    fn load_stats(&self) {
        let mut stats = fs::read_to_string(&self.file)
            .map(|text| Stats::from_json(&text))
            .unwrap_or_default();
        stats.prune(today());
        self.machine.borrow_mut().stats = stats;
    }

    fn paint(&self) {
        self.render();
        self.update_notch();
    }

    fn render(&self) {
        if !self.showing.get() {
            return;
        }
        let config = self.config.borrow();
        let machine = self.machine.borrow();
        self.view.show(display(
            i18n::current(),
            &config,
            &machine.cycle(),
            machine.snapshot(now_ms()),
            &machine.stats,
            today(),
        ));
    }

    fn clear_notch(&self) {
        self.on_notch.borrow_mut().clear();
        if let Some(notch) = self.notch.upgrade() {
            notch.set_activity(SLOT, None);
        }
    }

    /// The countdown in the closed notch, while a stage runs.
    fn update_notch(&self) {
        let snapshot = self.snapshot();
        let wanted = self.config.borrow().enabled
            && self.config.borrow().closed_notch
            && snapshot.phase == Phase::Running;
        if !wanted {
            if !self.on_notch.borrow().is_empty() {
                self.clear_notch();
            }
            return;
        }
        let stage = self.machine.borrow().cycle().stage();
        let clock = clock_text(snapshot.remaining_secs);
        let key = format!("{stage:?} {clock}");
        if *self.on_notch.borrow() == key {
            return;
        }
        *self.on_notch.borrow_mut() = key;
        let (symbol, tone) = match stage {
            Stage::Work => ("timer", Tone::Orange),
            Stage::ShortBreak => ("cup.and.saucer.fill", Tone::Green),
            Stage::LongBreak => ("cup.and.saucer.fill", Tone::Blue),
        };
        if let Some(notch) = self.notch.upgrade() {
            notch.set_activity(
                SLOT,
                Some(Activity {
                    left: Ear::Symbol {
                        name: symbol,
                        tone,
                        pulse: false,
                    },
                    right: Ear::Text { text: clock, tone },
                    detail: None,
                    page: Some(self.page),
                    priority: PRIORITY,
                }),
            );
        }
    }
}

/// What the tab shows for the state it is in. Pure, so that the tests drive it without a screen or a clock.
fn display(
    language: Language,
    config: &Config,
    cycle: &Cycle,
    snapshot: Snapshot,
    stats: &Stats,
    today: i64,
) -> PomodoroDisplay {
    if !config.enabled {
        return PomodoroDisplay {
            note: Some((
                language.pomodoro_off_title().to_owned(),
                language.pomodoro_off_hint().to_owned(),
            )),
            ..PomodoroDisplay::default()
        };
    }
    let stage = cycle.stage();
    let (done, rounds) = cycle.progress();
    let running = snapshot.phase == Phase::Running;
    let paused = snapshot.phase == Phase::Paused;
    let progress = if snapshot.total_secs == 0 || snapshot.phase == Phase::Ready {
        0.0
    } else {
        1.0 - snapshot.remaining_secs as f64 / snapshot.total_secs as f64
    };
    let (sessions, minutes) = stats.on(today);
    let per_day = stats.sessions_per_day(today, 7);
    let busiest = per_day.iter().copied().max().unwrap_or(0);
    let mut week = [0.0; 7];
    let mut week_labels: [String; 7] = Default::default();
    for (i, sessions) in per_day.iter().enumerate() {
        week[i] = bar_height(*sessions, busiest);
        let day = today - 6 + i as i64;
        week_labels[i] = language
            .pomodoro_weekday((day + 4).rem_euclid(7) as usize)
            .to_owned();
    }
    PomodoroDisplay {
        hue: match stage {
            Stage::Work => Hue::Work,
            Stage::ShortBreak => Hue::ShortBreak,
            Stage::LongBreak => Hue::LongBreak,
        },
        stage: language.pomodoro_stage(stage).to_owned(),
        clock: clock_text(snapshot.remaining_secs),
        status: if running {
            language.pomodoro_running()
        } else if paused {
            language.pomodoro_paused()
        } else {
            language.pomodoro_ready()
        }
        .to_owned(),
        progress,
        running,
        paused,
        round: language.pomodoro_session(cycle.session(), rounds),
        dots: (done as usize, rounds as usize),
        dot_active: stage == Stage::Work && (running || paused),
        today_caption: language.pomodoro_today().to_owned(),
        today: language.pomodoro_done(sessions, minutes),
        streak_caption: language.pomodoro_streak().to_owned(),
        streak: language.pomodoro_days(stats.streak(today)),
        week,
        week_labels,
        labels: Labels {
            start: language.pomodoro_start().to_owned(),
            pause: language.pomodoro_pause().to_owned(),
            resume: language.pomodoro_resume().to_owned(),
            skip: language.pomodoro_skip().to_owned(),
            reset: language.pomodoro_reset().to_owned(),
        },
        note: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const LANGUAGES: [Language; 2] = [Language::English, Language::Vietnamese];

    fn on() -> Config {
        Config {
            enabled: true,
            ..Config::default()
        }
    }

    fn snapshot(phase: Phase, remaining: u64, total: u64) -> Snapshot {
        Snapshot {
            phase,
            total_secs: total,
            remaining_secs: remaining,
        }
    }

    #[test]
    fn the_feature_is_off_by_default_and_old_files_load() {
        let config: Config = serde_json::from_str("{}").unwrap();
        assert_eq!(config, Config::default());
        assert!(!config.enabled && config.shortcut.is_empty());
        assert_eq!(
            (
                config.work,
                config.short_break,
                config.long_break,
                config.rounds
            ),
            (25, 5, 15, 4)
        );
        let partial: Config =
            serde_json::from_str(r#"{ "enabled": true, "work": 50, "future_key": 1 }"#).unwrap();
        assert!(partial.enabled && partial.work == 50 && partial.short_break == 5);
    }

    #[test]
    fn a_hostile_file_is_brought_into_range() {
        let config = Config {
            work: 7,
            short_break: 0,
            long_break: 999,
            rounds: 100,
            shortcut: "nonsense".into(),
            ..Config::default()
        }
        .sanitized();
        assert_eq!(config, Config::default());
        let kept = Config {
            shortcut: "cmd+shift+p".into(),
            work: 50,
            ..Config::default()
        }
        .sanitized();
        assert_eq!((kept.work, kept.shortcut.as_str()), (50, "cmd+shift+p"));
    }

    #[test]
    fn an_off_tab_says_so_in_both_languages() {
        for language in LANGUAGES {
            let shown = display(
                language,
                &Config::default(),
                &Cycle::new(Config::default().plan()),
                snapshot(Phase::Ready, 1500, 1500),
                &Stats::default(),
                100,
            );
            let (title, hint) = shown.note.expect("a notice");
            assert!(!title.is_empty() && !hint.is_empty());
        }
    }

    #[test]
    fn a_stage_that_has_not_begun_is_ready_with_an_empty_ring() {
        let cycle = Cycle::new(on().plan());
        let shown = display(
            Language::English,
            &on(),
            &cycle,
            snapshot(Phase::Ready, 1500, 1500),
            &Stats::default(),
            100,
        );
        assert_eq!(
            (
                shown.clock.as_str(),
                shown.stage.as_str(),
                shown.status.as_str()
            ),
            ("25:00", "Focus", "Ready")
        );
        assert_eq!(shown.progress, 0.0);
        assert!(!shown.running && !shown.paused && !shown.dot_active);
        assert_eq!(
            (shown.dots, shown.round.as_str()),
            ((0, 4), "Session 1 of 4")
        );
        assert_eq!(
            (shown.today.as_str(), shown.streak.as_str()),
            ("No sessions yet", "None yet")
        );
        assert_eq!(shown.hue, Hue::Work);
    }

    #[test]
    fn a_running_stage_fills_the_ring_and_lights_the_dot_of_the_session() {
        let cycle = Cycle::new(on().plan());
        let shown = display(
            Language::English,
            &on(),
            &cycle,
            snapshot(Phase::Running, 375, 1500),
            &Stats::default(),
            100,
        );
        assert!((shown.progress - 0.75).abs() < 1e-9);
        assert_eq!(
            (shown.clock.as_str(), shown.status.as_str()),
            ("06:15", "Running")
        );
        assert!(shown.running && shown.dot_active);
        let paused = display(
            Language::Vietnamese,
            &on(),
            &cycle,
            snapshot(Phase::Paused, 750, 1500),
            &Stats::default(),
            100,
        );
        assert_eq!(
            (paused.status.as_str(), paused.stage.as_str()),
            ("Tạm dừng", "Tập trung")
        );
        assert!(paused.paused && !paused.running && paused.dot_active);
    }

    #[test]
    fn a_break_is_green_or_blue_and_its_dot_is_not_lit_for_a_session() {
        let mut cycle = Cycle::new(on().plan());
        cycle.finish();
        let short = display(
            Language::English,
            &on(),
            &cycle,
            snapshot(Phase::Running, 100, 300),
            &Stats::default(),
            100,
        );
        assert_eq!(
            (short.hue, short.stage.as_str(), short.dots),
            (Hue::ShortBreak, "Short break", (1, 4))
        );
        assert!(!short.dot_active, "the work session is behind it");
        assert_eq!(short.round, "Session 2 of 4", "the one that comes next");
        for _ in 0..6 {
            cycle.finish();
        }
        let long = display(
            Language::English,
            &on(),
            &cycle,
            snapshot(Phase::Ready, 900, 900),
            &Stats::default(),
            100,
        );
        assert_eq!(
            (long.hue, long.stage.as_str(), long.dots),
            (Hue::LongBreak, "Long break", (4, 4))
        );
    }

    #[test]
    fn the_statistics_are_said_and_the_week_is_drawn_oldest_first_against_the_busiest_day() {
        let mut stats = Stats::default();
        // Day 100 is the 11th of April 1970, a Saturday; the week runs from day 94, a Sunday, to day 100.
        for _ in 0..2 {
            stats.record(100, 25);
        }
        stats.record(99, 25);
        stats.record(98, 25);
        stats.record(98, 25);
        stats.record(98, 25);
        stats.record(98, 25);
        let cycle = Cycle::new(on().plan());
        let shown = display(
            Language::English,
            &on(),
            &cycle,
            snapshot(Phase::Ready, 1500, 1500),
            &stats,
            100,
        );
        assert_eq!(shown.today, "2 sessions, 50 min");
        assert_eq!(shown.streak, "3 days");
        assert_eq!(shown.week, [0.0, 0.0, 0.0, 0.0, 1.0, 0.25, 0.5]);
        assert_eq!(shown.week_labels, ["S", "M", "T", "W", "T", "F", "S"]);
        let vi = display(
            Language::Vietnamese,
            &on(),
            &cycle,
            snapshot(Phase::Ready, 1500, 1500),
            &stats,
            100,
        );
        assert_eq!(vi.today, "2 phiên, 50 phút");
        assert_eq!(vi.week_labels, ["CN", "T2", "T3", "T4", "T5", "T6", "T7"]);
        let one = {
            let mut s = Stats::default();
            s.record(100, 25);
            display(
                Language::English,
                &on(),
                &cycle,
                snapshot(Phase::Ready, 1500, 1500),
                &s,
                100,
            )
        };
        assert_eq!(
            (one.today.as_str(), one.streak.as_str()),
            ("1 session, 25 min", "1 day")
        );
    }

    #[test]
    fn a_day_with_no_sessions_draws_no_bars() {
        let cycle = Cycle::new(on().plan());
        let shown = display(
            Language::English,
            &on(),
            &cycle,
            snapshot(Phase::Ready, 1500, 1500),
            &Stats::default(),
            100,
        );
        assert_eq!(shown.week, [0.0; 7]);
    }

    #[test]
    fn every_word_is_there_in_both_languages() {
        for language in LANGUAGES {
            for stage in [Stage::Work, Stage::ShortBreak, Stage::LongBreak] {
                assert!(!language.pomodoro_stage(stage).is_empty());
            }
            for text in [
                language.pomodoro_tab(),
                language.pomodoro_running(),
                language.pomodoro_paused(),
                language.pomodoro_ready(),
                language.pomodoro_today(),
                language.pomodoro_streak(),
                language.pomodoro_start(),
                language.pomodoro_pause(),
                language.pomodoro_resume(),
                language.pomodoro_skip(),
                language.pomodoro_reset(),
                language.pomodoro_off_title(),
                language.pomodoro_off_hint(),
                language.pomodoro_menu_start(),
                language.pomodoro_menu_resume(),
            ] {
                assert!(!text.is_empty());
            }
            assert!(language.pomodoro_menu_pause("12:30").contains("12:30"));
            for day in 0..7 {
                assert!(!language.pomodoro_weekday(day).is_empty());
            }
        }
        assert_ne!(
            Language::English.pomodoro_stage(Stage::Work),
            Language::Vietnamese.pomodoro_stage(Stage::Work)
        );
    }

    #[test]
    fn the_statistics_file_is_written_whole() {
        let dir = std::env::temp_dir().join(format!("figghost-pomodoro-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        let file = dir.join(FILE);
        let mut stats = Stats::default();
        stats.record(100, 25);
        write_atomically(&file, &stats.to_json()).unwrap();
        assert_eq!(Stats::from_json(&fs::read_to_string(&file).unwrap()), stats);
        let names: Vec<String> = fs::read_dir(&dir)
            .unwrap()
            .flatten()
            .map(|e| e.file_name().to_string_lossy().into_owned())
            .collect();
        assert_eq!(names, [FILE]);
        fs::remove_dir_all(&dir).unwrap();
    }
}
