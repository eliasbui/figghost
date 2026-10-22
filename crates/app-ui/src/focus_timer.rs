//! The countdown behind the Pomodoro tab: a countdown that can be started, paused, resumed and reset.
//!
//! Time comes in as a wall-clock timestamp in milliseconds, so the machine is plain data that tests can
//! drive, and a countdown keeps running while the Mac sleeps.

pub const DEFAULT_MINUTES: u32 = 25;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Phase {
    /// Set to a length, not started.
    Ready,
    Running,
    Paused,
    /// The time ran out and nobody has acknowledged it yet.
    Finished,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Snapshot {
    pub phase: Phase,
    pub total_secs: u64,
    pub remaining_secs: u64,
}

#[derive(Debug, Clone, Copy)]
enum State {
    Ready,
    Running { ends_at: i64 },
    Paused { remaining_ms: i64 },
    Finished,
}

#[derive(Debug, Clone, Copy)]
pub struct FocusTimer {
    total_ms: i64,
    state: State,
}

impl Default for FocusTimer {
    fn default() -> Self {
        Self::new(DEFAULT_MINUTES)
    }
}

impl FocusTimer {
    pub fn new(minutes: u32) -> Self {
        Self {
            total_ms: i64::from(minutes.clamp(1, 24 * 60)) * 60_000,
            state: State::Ready,
        }
    }

    /// Start, pause or resume; after the time ran out it starts again from the full length.
    pub fn toggle(&mut self, now: i64) {
        self.state = match self.state {
            State::Ready | State::Finished => State::Running {
                ends_at: now + self.total_ms,
            },
            State::Running { ends_at } => State::Paused {
                remaining_ms: (ends_at - now).max(0),
            },
            State::Paused { remaining_ms } => State::Running {
                ends_at: now + remaining_ms,
            },
        };
    }

    pub fn is_running(&self) -> bool {
        matches!(self.state, State::Running { .. })
    }

    /// Moves a countdown whose time is up to `Finished`; true exactly once, at that moment.
    pub fn tick(&mut self, now: i64) -> bool {
        if let State::Running { ends_at } = self.state
            && now >= ends_at
        {
            self.state = State::Finished;
            return true;
        }
        false
    }

    pub fn snapshot(&self, now: i64) -> Snapshot {
        let (phase, remaining_ms) = match self.state {
            State::Ready => (Phase::Ready, self.total_ms),
            State::Running { ends_at } => (Phase::Running, (ends_at - now).clamp(0, self.total_ms)),
            State::Paused { remaining_ms } => (Phase::Paused, remaining_ms),
            State::Finished => (Phase::Finished, 0),
        };
        Snapshot {
            phase,
            total_secs: (self.total_ms / 1000) as u64,
            // Rounded up, so the display reads 0:01 until the very last moment and never 0:00 early.
            remaining_secs: ((remaining_ms + 999) / 1000) as u64,
        }
    }
}

/// `25:00`, or `1:05:00` from an hour up.
pub fn clock_text(secs: u64) -> String {
    let (h, m, s) = (secs / 3600, secs / 60 % 60, secs % 60);
    if h > 0 {
        format!("{h}:{m:02}:{s:02}")
    } else {
        format!("{m:02}:{s:02}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const MIN: i64 = 60_000;

    #[test]
    fn a_new_timer_is_ready_at_the_default_length() {
        let t = FocusTimer::default();
        let s = t.snapshot(0);
        assert_eq!(
            (s.phase, s.total_secs, s.remaining_secs),
            (Phase::Ready, 25 * 60, 25 * 60)
        );
        assert!(!t.is_running());
    }

    #[test]
    fn it_counts_down_and_finishes_exactly_once() {
        let mut t = FocusTimer::new(1);
        t.toggle(1_000);
        assert!(t.is_running());
        assert_eq!(t.snapshot(1_000).remaining_secs, 60);
        assert_eq!(t.snapshot(1_000 + 30_000).remaining_secs, 30);
        assert!(!t.tick(1_000 + MIN - 1));
        assert_eq!(
            t.snapshot(1_000 + MIN - 1).remaining_secs,
            1,
            "never shows 0:00 before the end"
        );
        assert!(t.tick(1_000 + MIN));
        assert!(!t.tick(1_000 + MIN + 5), "the end is reported once");
        let s = t.snapshot(1_000 + MIN + 5);
        assert_eq!((s.phase, s.remaining_secs), (Phase::Finished, 0));
    }

    #[test]
    fn pausing_stops_the_clock_and_resuming_continues_from_there() {
        let mut t = FocusTimer::new(10);
        t.toggle(0);
        t.toggle(4 * MIN);
        let s = t.snapshot(9 * MIN);
        assert_eq!((s.phase, s.remaining_secs), (Phase::Paused, 6 * 60));
        assert!(!t.tick(20 * MIN), "a paused timer never finishes");
        t.toggle(10 * MIN);
        assert_eq!(t.snapshot(10 * MIN + 2 * MIN).remaining_secs, 4 * 60);
        assert!(t.tick(10 * MIN + 6 * MIN));
    }

    #[test]
    fn toggling_after_the_end_starts_a_fresh_countdown() {
        let mut t = FocusTimer::new(1);
        t.toggle(0);
        assert!(t.tick(MIN));
        t.toggle(5 * MIN);
        assert_eq!(t.snapshot(5 * MIN).remaining_secs, 60);
        assert!(t.is_running());
    }

    #[test]
    fn lengths_are_kept_within_sensible_bounds() {
        assert_eq!(FocusTimer::new(0).snapshot(0).total_secs, 60);
        assert_eq!(FocusTimer::new(100_000).snapshot(0).total_secs, 24 * 3600);
    }

    #[test]
    fn a_clock_that_jumps_back_does_not_overflow_the_display() {
        let mut t = FocusTimer::new(1);
        t.toggle(10 * MIN);
        // The wall clock was set back by an hour: never more than the full length remains.
        assert_eq!(t.snapshot(0).remaining_secs, 60);
    }

    #[test]
    fn clock_text_formats_minutes_and_hours() {
        assert_eq!(clock_text(0), "00:00");
        assert_eq!(clock_text(59), "00:59");
        assert_eq!(clock_text(25 * 60), "25:00");
        assert_eq!(clock_text(3600), "1:00:00");
        assert_eq!(clock_text(3600 + 5 * 60 + 9), "1:05:09");
    }
}
