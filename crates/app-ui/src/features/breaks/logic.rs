//! When a break is due, from the time spent at the Mac: the clock that counts it, held back while a meeting or a
//! full-screen app is in front, reset by a pause as long as a break, and the small reminders to blink and to sit up.
//! Nothing here reads the system or draws.

/// The time between two breaks that can be chosen, in minutes; the first is the default.
pub const WORK_MINUTES: [u32; 4] = [20, 30, 45, 60];
/// How long a break lasts, in seconds; the first is the default, twenty seconds of the 20-20-20 rule.
pub const BREAK_SECONDS: [u32; 3] = [20, 60, 300];
/// How often a reminder to blink or to sit up comes, in minutes; 0, the default, is never.
pub const NUDGE_MINUTES: [u32; 4] = [0, 10, 20, 30];
/// How long before a break it is announced, in seconds.
pub const WARN_SECONDS: f64 = 10.0;
/// How much later a break comes when it is put off, in seconds.
pub const LATER_SECONDS: f64 = 300.0;
/// Keyboard and pointer still for this long, or for a whole break if that is longer, is someone who went away: the
/// clock starts again. Shorter stillness still counts, since reading or watching keeps the eyes on the screen.
pub const AWAY_SECONDS: f64 = 300.0;
/// The most one step may count: a Mac that slept, or a timer that ran late, does not count the gap.
const MAX_STEP: f64 = 30.0;

fn offered(value: u32, choices: &[u32]) -> u32 {
    if choices.contains(&value) {
        value
    } else {
        choices[0]
    }
}

pub fn work_minutes(minutes: u32) -> u32 {
    offered(minutes, &WORK_MINUTES)
}

pub fn break_seconds(seconds: u32) -> u32 {
    offered(seconds, &BREAK_SECONDS)
}

pub fn nudge_minutes(minutes: u32) -> u32 {
    offered(minutes, &NUDGE_MINUTES)
}

/// The lengths the clock works with, in seconds; a reminder of 0 is off.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Rules {
    pub work: f64,
    pub rest: f64,
    pub blink: f64,
    pub posture: f64,
}

impl Rules {
    pub fn of(
        work_minutes: u32,
        break_seconds: u32,
        blink_minutes: u32,
        posture_minutes: u32,
    ) -> Self {
        let minutes = |m: u32| f64::from(m) * 60.0;
        Self {
            work: minutes(self::work_minutes(work_minutes)),
            rest: f64::from(self::break_seconds(break_seconds)),
            blink: minutes(nudge_minutes(blink_minutes)),
            posture: minutes(nudge_minutes(posture_minutes)),
        }
    }
}

/// What a step brings, one at a time and the most pressing first.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Event {
    /// A break comes in `WARN_SECONDS`.
    Warn,
    Start,
    End,
    Blink,
    Posture,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct Clock {
    worked: f64,
    warned: bool,
    /// Seconds left of the break that runs.
    left: Option<f64>,
    blink: f64,
    posture: f64,
}

impl Clock {
    pub fn left(&self) -> Option<f64> {
        self.left
    }

    pub fn breaking(&self) -> bool {
        self.left.is_some()
    }

    /// Seconds at the Mac before the next break.
    pub fn until_break(&self, rules: &Rules) -> f64 {
        (rules.work - self.worked).max(0.0)
    }

    /// Counts `dt` seconds. `idle` is how long the keyboard and pointer have been still. `held` is asked only when
    /// something would happen, and says whether a meeting or a full-screen app holds it back: the time goes on
    /// counting, and the break comes as soon as nothing holds it.
    pub fn step(
        &mut self,
        rules: &Rules,
        dt: f64,
        idle: f64,
        held: impl FnOnce() -> bool,
    ) -> Option<Event> {
        let dt = if dt.is_finite() {
            dt.clamp(0.0, MAX_STEP)
        } else {
            0.0
        };
        if let Some(left) = self.left {
            let left = left - dt;
            if left <= 0.0 {
                self.skip();
                return Some(Event::End);
            }
            self.left = Some(left);
            return None;
        }
        if idle >= rules.rest.max(AWAY_SECONDS) {
            *self = Self::default();
            return None;
        }
        self.worked += dt;
        self.blink += dt;
        self.posture += dt;
        let due = self.worked >= rules.work;
        let warn = !self.warned && self.worked >= rules.work - WARN_SECONDS;
        let blink = rules.blink > 0.0 && self.blink >= rules.blink;
        let posture = rules.posture > 0.0 && self.posture >= rules.posture;
        if !(due || warn || blink || posture) || held() {
            return None;
        }
        if due {
            self.start(rules);
            Some(Event::Start)
        } else if warn {
            self.warned = true;
            Some(Event::Warn)
        } else if blink {
            self.blink = 0.0;
            Some(Event::Blink)
        } else {
            self.posture = 0.0;
            Some(Event::Posture)
        }
    }

    /// A break now, whatever the clock says. The reminders start over, since a break does what they ask.
    pub fn start(&mut self, rules: &Rules) {
        self.left = Some(rules.rest);
        self.blink = 0.0;
        self.posture = 0.0;
    }

    /// Ends a break, or the wait for one, and counts from nothing.
    pub fn skip(&mut self) {
        self.left = None;
        self.worked = 0.0;
        self.warned = false;
    }

    /// Ends a break, or the wait for one, and has it come `LATER_SECONDS` from now.
    pub fn later(&mut self, rules: &Rules) {
        self.skip();
        self.worked = (rules.work - LATER_SECONDS).max(0.0);
    }
}

/// Seconds as a clock: `0:18`, `4:52`.
pub fn clock(seconds: f64) -> String {
    let s = seconds.max(0.0).ceil() as u64;
    format!("{}:{:02}", s / 60, s % 60)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rules() -> Rules {
        Rules::of(20, 20, 0, 0)
    }

    /// Steps of five seconds with someone at the keyboard and nothing holding the break back, until an event comes.
    fn run(clock: &mut Clock, rules: &Rules, seconds: f64) -> Vec<(f64, Event)> {
        let mut events = Vec::new();
        let mut t = 0.0;
        while t < seconds {
            t += 5.0;
            if let Some(e) = clock.step(rules, 5.0, 1.0, || false) {
                events.push((t, e));
            }
        }
        events
    }

    #[test]
    fn a_length_that_is_not_offered_is_the_default() {
        assert_eq!(work_minutes(45), 45);
        assert_eq!(work_minutes(7), 20);
        assert_eq!(break_seconds(300), 300);
        assert_eq!(break_seconds(u32::MAX), 20);
        assert_eq!(nudge_minutes(10), 10);
        assert_eq!(nudge_minutes(15), 0, "a reminder not offered is off");
        let r = Rules::of(30, 60, 10, 0);
        assert_eq!(
            (r.work, r.rest, r.blink, r.posture),
            (1800.0, 60.0, 600.0, 0.0)
        );
    }

    #[test]
    fn twenty_minutes_at_the_mac_bring_a_warning_then_a_break_that_ends_by_itself() {
        let rules = rules();
        let mut clock = Clock::default();
        let events = run(&mut clock, &rules, 1200.0);
        assert_eq!(events, [(1190.0, Event::Warn), (1200.0, Event::Start)]);
        assert!(clock.breaking());
        assert_eq!(clock.left(), Some(20.0));
        let mut ended = None;
        for second in 1..=25 {
            if let Some(e) = clock.step(&rules, 1.0, 0.0, || unreachable!("a break is not held")) {
                ended = Some((second, e));
                break;
            }
        }
        assert_eq!(ended, Some((20, Event::End)));
        assert!(!clock.breaking());
        assert_eq!(
            clock.until_break(&rules),
            1200.0,
            "it counts from nothing again"
        );
    }

    #[test]
    fn a_meeting_holds_the_break_back_and_it_comes_as_soon_as_the_meeting_ends() {
        let rules = rules();
        let mut clock = Clock::default();
        for _ in 0..300 {
            assert_eq!(clock.step(&rules, 5.0, 1.0, || true), None);
        }
        assert!(!clock.breaking());
        assert_eq!(clock.until_break(&rules), 0.0, "the time went on counting");
        assert_eq!(clock.step(&rules, 5.0, 1.0, || false), Some(Event::Start));
    }

    #[test]
    fn whether_something_holds_it_back_is_asked_only_when_something_would_happen() {
        let rules = rules();
        let mut clock = Clock::default();
        let mut asked = 0;
        for _ in 0..100 {
            clock.step(&rules, 5.0, 1.0, || {
                asked += 1;
                false
            });
        }
        assert_eq!(asked, 0, "nothing is due in the first 500 seconds");
    }

    #[test]
    fn going_away_starts_the_clock_again_and_shorter_stillness_still_counts() {
        let rules = rules();
        let mut clock = Clock::default();
        run(&mut clock, &rules, 600.0);
        clock.step(&rules, 5.0, 200.0, || false);
        assert_eq!(
            clock.until_break(&rules),
            595.0,
            "reading without touching still counts"
        );
        clock.step(&rules, 5.0, AWAY_SECONDS, || false);
        assert_eq!(clock.until_break(&rules), 1200.0);
        let long = Rules::of(20, 300, 0, 0);
        let mut clock = Clock::default();
        run(&mut clock, &long, 600.0);
        clock.step(&long, 5.0, 299.0, || false);
        assert!(clock.until_break(&long) < 1200.0);
    }

    #[test]
    fn a_step_never_counts_more_than_a_moment() {
        let rules = rules();
        let mut clock = Clock::default();
        for dt in [3600.0, f64::NAN, f64::INFINITY, -50.0] {
            clock.step(&rules, dt, 1.0, || false);
        }
        assert_eq!(clock.until_break(&rules), 1200.0 - MAX_STEP);
    }

    #[test]
    fn skipping_counts_from_nothing_and_later_brings_it_back_in_five_minutes() {
        let rules = rules();
        let mut clock = Clock::default();
        clock.start(&rules);
        clock.later(&rules);
        assert!(!clock.breaking());
        assert_eq!(clock.until_break(&rules), LATER_SECONDS);
        let events = run(&mut clock, &rules, LATER_SECONDS);
        assert_eq!(events.last().map(|e| e.1), Some(Event::Start));
        clock.skip();
        assert_eq!(clock.until_break(&rules), 1200.0);
    }

    #[test]
    fn the_reminders_come_on_their_own_time_and_a_break_starts_them_over() {
        let rules = Rules::of(30, 20, 10, 20);
        let mut clock = Clock::default();
        let events = run(&mut clock, &rules, 1800.0);
        let kinds: Vec<Event> = events.iter().map(|e| e.1).collect();
        assert_eq!(
            kinds,
            [
                Event::Blink,
                Event::Blink,
                Event::Posture,
                Event::Warn,
                Event::Start
            ],
            "the break at 30 minutes takes the place of the blink due then"
        );
        assert_eq!(events[0].0, 600.0);
        assert_eq!(events[2].0, 1205.0, "two at once come one after the other");
    }

    #[test]
    fn the_time_left_is_a_clock_that_rounds_up() {
        assert_eq!(clock(18.0), "0:18");
        assert_eq!(clock(17.2), "0:18");
        assert_eq!(clock(292.0), "4:52");
        assert_eq!(clock(-3.0), "0:00");
    }
}
