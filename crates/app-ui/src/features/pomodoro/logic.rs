//! The rhythm of a Pomodoro: which stage comes after which, how many work sessions make a round, and what is kept of the
//! days that went by (how many sessions, how many minutes, and how many days in a row). Nothing here reads a clock or
//! draws, so the tests drive it with days and minutes of their own.

use crate::focus_timer::{FocusTimer, Phase, Snapshot};
use serde_json::{Value, json};
use std::collections::BTreeMap;

/// The lengths the settings offer, in minutes.
pub const WORK_MINUTES: [u32; 7] = [15, 20, 25, 30, 45, 50, 60];
pub const SHORT_BREAK_MINUTES: [u32; 4] = [3, 5, 10, 15];
pub const LONG_BREAK_MINUTES: [u32; 4] = [10, 15, 20, 30];
/// How many work sessions come before a long break.
pub const ROUNDS: [u32; 5] = [2, 3, 4, 5, 6];
/// How many days of statistics are kept.
pub const KEEP_DAYS: i64 = 30;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Stage {
    Work,
    ShortBreak,
    LongBreak,
}

/// The lengths and the number of sessions in a round.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Plan {
    pub work: u32,
    pub short_break: u32,
    pub long_break: u32,
    pub rounds: u32,
}

impl Plan {
    /// Each value held to what the settings offer; one that is not offered is the usual one.
    pub fn held(self) -> Self {
        let pick = |value: u32, list: &[u32], usual: u32| {
            if list.contains(&value) { value } else { usual }
        };
        Self {
            work: pick(self.work, &WORK_MINUTES, 25),
            short_break: pick(self.short_break, &SHORT_BREAK_MINUTES, 5),
            long_break: pick(self.long_break, &LONG_BREAK_MINUTES, 15),
            rounds: pick(self.rounds, &ROUNDS, 4),
        }
    }
}

/// Where in the rhythm the person is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Cycle {
    plan: Plan,
    stage: Stage,
    /// The work sessions finished in this round: 0 up to `rounds`. It is `rounds` during the long break that follows.
    done: u32,
}

impl Cycle {
    pub fn new(plan: Plan) -> Self {
        Self {
            plan: plan.held(),
            stage: Stage::Work,
            done: 0,
        }
    }

    pub fn stage(&self) -> Stage {
        self.stage
    }

    /// The length of the stage that is on, in minutes.
    pub fn minutes(&self) -> u32 {
        match self.stage {
            Stage::Work => self.plan.work,
            Stage::ShortBreak => self.plan.short_break,
            Stage::LongBreak => self.plan.long_break,
        }
    }

    /// The sessions finished in this round and how many a round has, for the dots and for "session 2 of 4".
    pub fn progress(&self) -> (u32, u32) {
        (self.done, self.plan.rounds)
    }

    /// The number of the work session that is on or comes next: 1 up to `rounds`.
    pub fn session(&self) -> u32 {
        (self.done + 1).min(self.plan.rounds)
    }

    /// The stage that is on has been lived to its end. A work session counts; after the last of a round comes the
    /// long break, otherwise a short one; a break is followed by work, and a long break starts a new round.
    pub fn finish(&mut self) {
        match self.stage {
            Stage::Work => {
                self.done = (self.done + 1).min(self.plan.rounds);
                self.stage = if self.done >= self.plan.rounds {
                    Stage::LongBreak
                } else {
                    Stage::ShortBreak
                };
            }
            Stage::ShortBreak => self.stage = Stage::Work,
            Stage::LongBreak => {
                self.done = 0;
                self.stage = Stage::Work;
            }
        }
    }

    /// Moves to the next stage without counting the work session that was on: it was not lived to its end.
    pub fn skip(&mut self) {
        match self.stage {
            // The session is not counted, but the last one of a round still gives the long break that ends it.
            Stage::Work => {
                self.stage = if self.done + 1 >= self.plan.rounds {
                    Stage::LongBreak
                } else {
                    Stage::ShortBreak
                };
            }
            Stage::ShortBreak => self.stage = Stage::Work,
            Stage::LongBreak => {
                self.done = 0;
                self.stage = Stage::Work;
            }
        }
    }

    /// Back to the first work session of a new round.
    pub fn restart(&mut self) {
        self.stage = Stage::Work;
        self.done = 0;
    }

    /// New lengths from the settings. Where the person is stays where it can: the sessions done never exceed a round.
    pub fn set_plan(&mut self, plan: Plan) {
        self.plan = plan.held();
        self.done = self.done.min(self.plan.rounds);
        // Before the long break a round is never full: its last session is the one that brings the break.
        if self.stage != Stage::LongBreak && self.done >= self.plan.rounds {
            self.done = self.plan.rounds - 1;
        }
    }
}

/// What was done on each day.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct Stats {
    /// Day number (days since 1970-01-01, local) to (sessions, minutes).
    days: BTreeMap<i64, (u32, u32)>,
}

/// More than this in a day is not believed: a session is at least a minute and a day has 1440.
const MAX_SESSIONS_PER_DAY: u32 = 1_440;
const MAX_MINUTES_PER_DAY: u32 = 1_440;

impl Stats {
    #[cfg(test)]
    pub fn is_empty(&self) -> bool {
        self.days.is_empty()
    }

    /// A work session of `minutes` was lived to its end on `day`.
    pub fn record(&mut self, day: i64, minutes: u32) {
        let entry = self.days.entry(day).or_default();
        entry.0 = (entry.0 + 1).min(MAX_SESSIONS_PER_DAY);
        entry.1 = (entry.1 + minutes).min(MAX_MINUTES_PER_DAY);
    }

    /// Forgets the days older than the ones that are kept, and the days that have not come.
    pub fn prune(&mut self, today: i64) {
        self.days
            .retain(|day, _| *day > today - KEEP_DAYS && *day <= today);
    }

    pub fn clear(&mut self) {
        self.days.clear();
    }

    /// (sessions, minutes) on a day.
    pub fn on(&self, day: i64) -> (u32, u32) {
        self.days.get(&day).copied().unwrap_or_default()
    }

    /// How many days in a row, counting back from today, had at least one session. A today with none yet does not
    /// break a run that was alive yesterday.
    pub fn streak(&self, today: i64) -> u32 {
        let start = if self.on(today).0 > 0 {
            today
        } else {
            today - 1
        };
        let mut count = 0;
        let mut day = start;
        while self.on(day).0 > 0 {
            count += 1;
            day -= 1;
        }
        count
    }

    /// The sessions of each of the last `days` days ending at `today`, the oldest first.
    pub fn sessions_per_day(&self, today: i64, days: usize) -> Vec<u32> {
        (0..days as i64)
            .rev()
            .map(|back| self.on(today - back).0)
            .collect()
    }

    pub fn days_with_data(&self) -> usize {
        self.days.len()
    }

    pub fn to_json(&self) -> String {
        let days: serde_json::Map<String, Value> = self
            .days
            .iter()
            .map(|(day, (sessions, minutes))| (day.to_string(), json!([sessions, minutes])))
            .collect();
        json!({ "days": days }).to_string()
    }

    /// Statistics from a file. Anything of the wrong shape or not believable is left out, and the rest stays.
    pub fn from_json(text: &str) -> Self {
        let mut stats = Self::default();
        let Ok(root) = serde_json::from_str::<Value>(text) else {
            return stats;
        };
        let Some(days) = root["days"].as_object() else {
            return stats;
        };
        for (day, entry) in days {
            let (Ok(day), Some(pair)) = (day.parse::<i64>(), entry.as_array()) else {
                continue;
            };
            let number = |i: usize| {
                pair.get(i)
                    .and_then(Value::as_u64)
                    .and_then(|n| u32::try_from(n).ok())
            };
            if let (Some(sessions), Some(minutes)) = (number(0), number(1))
                && sessions > 0
                && pair.len() == 2
            {
                stats.days.insert(
                    day,
                    (
                        sessions.min(MAX_SESSIONS_PER_DAY),
                        minutes.min(MAX_MINUTES_PER_DAY),
                    ),
                );
            }
        }
        stats
    }
}

/// A stage that ran to its end.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Ended {
    pub stage: Stage,
    /// It was a work session, which is now counted in the statistics.
    pub counted: bool,
}

/// The rhythm, the countdown of the stage that is on and what has been done, moved by time that comes in as
/// milliseconds, so that a test can live a whole round in a moment.
#[derive(Debug, Clone)]
pub struct Machine {
    cycle: Cycle,
    timer: FocusTimer,
    pub stats: Stats,
}

impl Machine {
    pub fn new(plan: Plan) -> Self {
        let cycle = Cycle::new(plan);
        Self {
            timer: FocusTimer::new(cycle.minutes()),
            cycle,
            stats: Stats::default(),
        }
    }

    pub fn cycle(&self) -> Cycle {
        self.cycle
    }

    pub fn snapshot(&self, now: i64) -> Snapshot {
        self.timer.snapshot(now)
    }

    pub fn is_running(&self) -> bool {
        self.timer.is_running()
    }

    /// Starts the stage, pauses it while it runs, resumes it while it is paused.
    pub fn toggle(&mut self, now: i64) {
        self.timer.toggle(now);
    }

    /// Moves time on. When the stage that runs has reached its end, a work session is counted on `today`, the next stage
    /// is set (and started when `auto_start`), and what ended is given back; otherwise nothing.
    pub fn tick(&mut self, now: i64, today: i64, auto_start: bool) -> Option<Ended> {
        if !self.timer.tick(now) {
            return None;
        }
        let (stage, minutes) = (self.cycle.stage(), self.cycle.minutes());
        let counted = stage == Stage::Work;
        if counted {
            self.stats.record(today, minutes);
            self.stats.prune(today);
        }
        self.cycle.finish();
        self.begin(now, auto_start);
        Some(Ended { stage, counted })
    }

    /// Goes to the next stage without counting the one that was on.
    pub fn skip(&mut self, now: i64, auto_start: bool) {
        self.cycle.skip();
        self.begin(now, auto_start);
    }

    /// Back to the first session of a new round, not started.
    pub fn reset(&mut self) {
        self.cycle.restart();
        self.timer = FocusTimer::new(self.cycle.minutes());
    }

    /// New lengths. A stage that has not begun takes its new length; one that has begun keeps the one it began with.
    pub fn set_plan(&mut self, plan: Plan, now: i64) {
        self.cycle.set_plan(plan);
        if self.snapshot(now).phase == Phase::Ready {
            self.timer = FocusTimer::new(self.cycle.minutes());
        }
    }

    /// The timer of the stage that is on, from its start; running at once when stages start by themselves.
    fn begin(&mut self, now: i64, auto_start: bool) {
        self.timer = FocusTimer::new(self.cycle.minutes());
        if auto_start {
            self.timer.toggle(now);
        }
    }
}

/// The height of a bar for `sessions` against the busiest day of the week, 0 to 1; a day with none is nothing.
pub fn bar_height(sessions: u32, busiest: u32) -> f64 {
    if sessions == 0 || busiest == 0 {
        0.0
    } else {
        (f64::from(sessions) / f64::from(busiest)).clamp(0.0, 1.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn plan() -> Plan {
        Plan {
            work: 25,
            short_break: 5,
            long_break: 15,
            rounds: 4,
        }
    }

    #[test]
    fn a_plan_keeps_what_is_offered_and_replaces_what_is_not() {
        assert_eq!(plan().held(), plan());
        let odd = Plan {
            work: 7,
            short_break: 0,
            long_break: 999,
            rounds: 1,
        };
        assert_eq!(odd.held(), plan(), "each unusual length is the usual one");
        let mixed = Plan { work: 50, ..odd };
        assert_eq!(mixed.held().work, 50);
        for list in [
            &WORK_MINUTES[..],
            &SHORT_BREAK_MINUTES[..],
            &LONG_BREAK_MINUTES[..],
            &ROUNDS[..],
        ] {
            assert!(
                list.windows(2).all(|w| w[0] < w[1]),
                "the choices are in order"
            );
        }
    }

    #[test]
    fn work_and_short_breaks_alternate_and_the_last_work_of_a_round_is_followed_by_a_long_break() {
        let mut cycle = Cycle::new(plan());
        let mut seen = Vec::new();
        for _ in 0..8 {
            seen.push((cycle.stage(), cycle.minutes()));
            cycle.finish();
        }
        assert_eq!(
            seen,
            [
                (Stage::Work, 25),
                (Stage::ShortBreak, 5),
                (Stage::Work, 25),
                (Stage::ShortBreak, 5),
                (Stage::Work, 25),
                (Stage::ShortBreak, 5),
                (Stage::Work, 25),
                (Stage::LongBreak, 15),
            ]
        );
        assert_eq!(
            (cycle.stage(), cycle.progress()),
            (Stage::Work, (0, 4)),
            "a long break ends the round"
        );
    }

    #[test]
    fn the_dots_count_the_sessions_done_in_the_round() {
        let mut cycle = Cycle::new(plan());
        assert_eq!((cycle.progress(), cycle.session()), ((0, 4), 1));
        cycle.finish();
        assert_eq!(
            (cycle.progress(), cycle.session()),
            ((1, 4), 2),
            "the break after the first"
        );
        cycle.finish();
        assert_eq!(
            (cycle.progress(), cycle.session()),
            ((1, 4), 2),
            "the second work session is on"
        );
        for _ in 0..4 {
            cycle.finish();
        }
        assert_eq!(cycle.stage(), Stage::Work);
        assert_eq!(cycle.progress(), (3, 4));
        cycle.finish();
        assert_eq!(
            (cycle.stage(), cycle.progress(), cycle.session()),
            (Stage::LongBreak, (4, 4), 4)
        );
    }

    #[test]
    fn skipping_does_not_count_a_session_but_still_moves_on() {
        let mut cycle = Cycle::new(plan());
        cycle.skip();
        assert_eq!(
            (cycle.stage(), cycle.progress()),
            (Stage::ShortBreak, (0, 4))
        );
        cycle.skip();
        assert_eq!((cycle.stage(), cycle.progress()), (Stage::Work, (0, 4)));
        // Skipping the last work session of a round still gives the long break that ends the round.
        for _ in 0..3 {
            cycle.finish();
            cycle.finish();
        }
        assert_eq!(cycle.progress(), (3, 4));
        cycle.skip();
        assert_eq!(
            (cycle.stage(), cycle.progress()),
            (Stage::LongBreak, (3, 4)),
            "the session skipped is not a dot"
        );
        cycle.skip();
        assert_eq!((cycle.stage(), cycle.progress()), (Stage::Work, (0, 4)));
    }

    #[test]
    fn a_new_round_starts_with_the_first_work_session() {
        let mut cycle = Cycle::new(plan());
        cycle.finish();
        cycle.finish();
        cycle.finish();
        cycle.restart();
        assert_eq!((cycle.stage(), cycle.progress()), (Stage::Work, (0, 4)));
    }

    #[test]
    fn new_lengths_keep_the_place_as_far_as_it_still_exists() {
        let mut cycle = Cycle::new(plan());
        for _ in 0..6 {
            cycle.finish();
        }
        assert_eq!((cycle.stage(), cycle.progress()), (Stage::Work, (3, 4)));
        cycle.set_plan(Plan {
            rounds: 2,
            ..plan()
        });
        assert_eq!(
            cycle.progress(),
            (1, 2),
            "three done of a round of two is the last of that round"
        );
        cycle.set_plan(Plan { work: 50, ..plan() });
        assert_eq!(cycle.minutes(), 50);
        let mut long = Cycle::new(plan());
        for _ in 0..7 {
            long.finish();
        }
        assert_eq!(long.stage(), Stage::LongBreak);
        long.set_plan(Plan {
            rounds: 6,
            ..plan()
        });
        assert_eq!(
            (long.stage(), long.progress()),
            (Stage::LongBreak, (4, 6)),
            "a long break stays a long break"
        );
    }

    #[test]
    fn sessions_and_minutes_are_added_up_by_day() {
        let mut stats = Stats::default();
        stats.record(100, 25);
        stats.record(100, 25);
        stats.record(101, 50);
        assert_eq!(
            (stats.on(100), stats.on(101), stats.on(99)),
            ((2, 50), (1, 50), (0, 0))
        );
        assert_eq!(stats.sessions_per_day(101, 3), [0, 2, 1]);
        assert_eq!(stats.sessions_per_day(101, 0), Vec::<u32>::new());
        assert_eq!(stats.days_with_data(), 2);
    }

    #[test]
    fn a_streak_counts_the_days_in_a_row_and_a_quiet_today_does_not_break_it() {
        let mut stats = Stats::default();
        assert_eq!(stats.streak(100), 0);
        for day in [97, 98, 99] {
            stats.record(day, 25);
        }
        assert_eq!(stats.streak(99), 3);
        assert_eq!(
            stats.streak(100),
            3,
            "today has none yet, and yesterday's run is still alive"
        );
        assert_eq!(
            stats.streak(101),
            0,
            "a day with none, after a day with none, has broken it"
        );
        stats.record(100, 25);
        assert_eq!(stats.streak(100), 4);
        stats.record(95, 25);
        assert_eq!(stats.streak(100), 4, "a gap ends the run where it is");
    }

    #[test]
    fn old_days_and_days_that_have_not_come_are_forgotten() {
        let mut stats = Stats::default();
        stats.record(10, 25);
        stats.record(100 - KEEP_DAYS + 1, 25);
        stats.record(100 - KEEP_DAYS, 25);
        stats.record(100, 25);
        stats.record(120, 25);
        stats.prune(100);
        assert_eq!(stats.days_with_data(), 2);
        assert_eq!(stats.on(100 - KEEP_DAYS + 1).0, 1);
        assert_eq!(stats.on(120).0, 0);
    }

    #[test]
    fn a_day_holds_no_more_than_a_day_can() {
        let mut stats = Stats::default();
        for _ in 0..3_000 {
            stats.record(1, 60);
        }
        assert_eq!(stats.on(1), (MAX_SESSIONS_PER_DAY, MAX_MINUTES_PER_DAY));
    }

    #[test]
    fn statistics_are_written_and_read_back_the_same() {
        let mut stats = Stats::default();
        stats.record(100, 25);
        stats.record(101, 50);
        stats.record(101, 25);
        assert_eq!(Stats::from_json(&stats.to_json()), stats);
        assert!(Stats::from_json(&Stats::default().to_json()).is_empty());
    }

    #[test]
    fn a_file_that_is_wrong_in_places_keeps_what_is_right() {
        let text = r#"{ "days": {
            "100": [2, 50], "101": [0, 0], "102": [1], "103": [1, 2, 3], "104": "x", "x": [1, 25],
            "105": [-1, 25], "106": [99999999, 99999999], "107": [1.5, 25]
        } }"#;
        let stats = Stats::from_json(text);
        assert_eq!(stats.on(100), (2, 50));
        assert_eq!(stats.on(106), (MAX_SESSIONS_PER_DAY, MAX_MINUTES_PER_DAY));
        assert_eq!(stats.days_with_data(), 2, "{stats:?}");
        for broken in [
            "",
            "not json",
            "[]",
            "null",
            r#"{"days":5}"#,
            r#"{"days":[]}"#,
        ] {
            assert!(Stats::from_json(broken).is_empty(), "{broken}");
        }
    }

    const MIN: i64 = 60_000;

    #[test]
    fn a_whole_round_is_lived_and_counted() {
        let mut m = Machine::new(plan());
        let mut now = 1_000_000;
        let mut ended = Vec::new();
        for _ in 0..8 {
            assert!(!m.is_running());
            m.toggle(now);
            assert!(m.is_running());
            let length = i64::from(m.cycle().minutes()) * MIN;
            assert_eq!(
                m.tick(now + length - 1, 100, false),
                None,
                "not a moment early"
            );
            now += length;
            ended.push(m.tick(now, 100, false).expect("it ends exactly on time"));
        }
        let stages: Vec<Stage> = ended.iter().map(|e| e.stage).collect();
        assert_eq!(
            stages,
            [
                Stage::Work,
                Stage::ShortBreak,
                Stage::Work,
                Stage::ShortBreak,
                Stage::Work,
                Stage::ShortBreak,
                Stage::Work,
                Stage::LongBreak,
            ]
        );
        assert_eq!(
            ended.iter().filter(|e| e.counted).count(),
            4,
            "only the work sessions count"
        );
        assert_eq!(m.stats.on(100), (4, 100));
        assert_eq!(m.cycle().progress(), (0, 4), "a long break ends the round");
        assert_eq!(
            m.snapshot(now).phase,
            Phase::Ready,
            "the next stage waits to be started"
        );
    }

    #[test]
    fn with_auto_start_the_next_stage_runs_at_once() {
        let mut m = Machine::new(plan());
        let t0 = 5_000_000;
        m.toggle(t0);
        assert!(m.tick(t0 + 25 * MIN, 7, true).is_some());
        assert!(m.is_running(), "the break began by itself");
        assert_eq!(m.cycle().stage(), Stage::ShortBreak);
        assert_eq!(m.snapshot(t0 + 25 * MIN).remaining_secs, 5 * 60);
        assert!(m.tick(t0 + 30 * MIN, 7, true).is_some());
        assert_eq!((m.cycle().stage(), m.is_running()), (Stage::Work, true));
    }

    #[test]
    fn a_pause_keeps_the_time_left_and_the_clock_does_not_run_meanwhile() {
        let mut m = Machine::new(plan());
        let t0 = 0;
        m.toggle(t0);
        m.toggle(t0 + 10 * MIN);
        assert_eq!(m.snapshot(t0 + 10 * MIN).phase, Phase::Paused);
        assert_eq!(m.snapshot(t0 + 500 * MIN).remaining_secs, 15 * 60);
        assert_eq!(
            m.tick(t0 + 500 * MIN, 1, false),
            None,
            "a paused stage does not end"
        );
        m.toggle(t0 + 100 * MIN);
        assert_eq!(m.tick(t0 + 114 * MIN, 1, false), None);
        assert!(m.tick(t0 + 115 * MIN, 1, false).is_some());
    }

    #[test]
    fn a_stage_that_ends_while_the_mac_sleeps_ends_once() {
        let mut m = Machine::new(plan());
        m.toggle(0);
        let woke = 10 * 60 * MIN;
        let first = m.tick(woke, 1, false);
        assert_eq!(
            first,
            Some(Ended {
                stage: Stage::Work,
                counted: true
            })
        );
        assert_eq!(
            m.tick(woke + 1, 1, false),
            None,
            "the break waits to be started"
        );
        assert_eq!(m.stats.on(1).0, 1);
    }

    #[test]
    fn skipping_counts_nothing_and_resetting_starts_a_new_round() {
        let mut m = Machine::new(plan());
        m.toggle(0);
        m.skip(10 * MIN, false);
        assert_eq!(
            (m.cycle().stage(), m.is_running()),
            (Stage::ShortBreak, false)
        );
        assert!(m.stats.is_empty());
        m.skip(11 * MIN, true);
        assert_eq!(
            (m.cycle().stage(), m.is_running()),
            (Stage::Work, true),
            "auto start applies to a skip too"
        );
        m.reset();
        assert_eq!(
            (m.cycle().stage(), m.cycle().progress(), m.is_running()),
            (Stage::Work, (0, 4), false)
        );
    }

    #[test]
    fn new_lengths_reach_a_stage_that_has_not_begun_and_leave_one_that_has() {
        let mut m = Machine::new(plan());
        m.set_plan(Plan { work: 50, ..plan() }, 0);
        assert_eq!(m.snapshot(0).total_secs, 50 * 60);
        m.toggle(0);
        m.set_plan(Plan { work: 15, ..plan() }, MIN);
        assert_eq!(
            m.snapshot(MIN).total_secs,
            50 * 60,
            "the running stage keeps its length"
        );
        assert_eq!(
            m.cycle().minutes(),
            15,
            "the next one will be the new length"
        );
    }

    #[test]
    fn a_bar_is_as_tall_as_the_day_is_busy_against_the_busiest() {
        assert_eq!(bar_height(0, 5), 0.0);
        assert_eq!(bar_height(5, 5), 1.0);
        assert!((bar_height(2, 4) - 0.5).abs() < 1e-9);
        assert_eq!(bar_height(3, 0), 0.0, "no busiest day, no bars");
        assert_eq!(bar_height(9, 3), 1.0, "never taller than the box");
    }
}
