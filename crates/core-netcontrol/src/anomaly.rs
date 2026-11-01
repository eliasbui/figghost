//! A sudden upload: an app that sent far more in the last few minutes than it usually does in an hour. Pure, from what
//! the history counted and what was sent lately.

use crate::history::History;

/// What is looked at: the last five minutes.
pub const WINDOW_SECONDS: u64 = 300;
/// An upload under this much is never called sudden, however quiet the app usually is.
pub const MIN_BYTES: u64 = 50_000_000;
/// How many times its usual hour an upload must be.
pub const FACTOR: u64 = 10;
/// How many days before today are looked at for what is usual.
pub const DAYS: i64 = 7;

/// What the app usually sends in an hour: the middle one of the days before today on which it sent anything, over 24.
/// `None` for an app with no such day, which is not judged: nothing is known of it yet.
pub fn usual_hour(history: &History, app: &str, today: i64) -> Option<u64> {
    let mut days = history.sent_by_day(today - DAYS, today - 1, app);
    if days.is_empty() {
        return None;
    }
    days.sort_unstable();
    Some(days[days.len() / 2] / 24)
}

/// The most an app may send in the window before it is called sudden.
pub fn limit(usual_hour: u64) -> u64 {
    MIN_BYTES.max(usual_hour.saturating_mul(FACTOR))
}

/// Whether `recent` bytes sent in the window are sudden for an app.
pub fn is_sudden(history: &History, app: &str, recent: u64, today: i64) -> bool {
    usual_hour(history, app, today).is_some_and(|usual| recent > limit(usual))
}

#[cfg(test)]
mod tests {
    use super::*;

    const MB: u64 = 1_000_000;

    fn week(app: &str, per_day: u64) -> History {
        let mut h = History::default();
        for day in 93..100 {
            h.add(day, app, "en0", 1, per_day);
        }
        h
    }

    #[test]
    fn what_is_usual_is_the_middle_day_over_twenty_four_hours() {
        let mut h = History::default();
        h.add(95, "Backup", "en0", 0, 2400 * MB);
        h.add(96, "Backup", "en0", 0, 480 * MB);
        h.add(97, "Backup", "en0", 0, 240 * MB);
        assert_eq!(
            usual_hour(&h, "backup", 100),
            Some(480 * MB / 24),
            "the middle of 240, 480 and 2400"
        );
        assert_eq!(
            usual_hour(&h, "Mail", 100),
            None,
            "an app with no day behind it is not judged"
        );
        h.add(100, "Backup", "en0", 0, 9999 * MB);
        assert_eq!(
            usual_hour(&h, "Backup", 100),
            Some(20 * MB),
            "today is not part of what is usual"
        );
        assert_eq!(
            usual_hour(&h, "Backup", 200),
            None,
            "days long ago are not either"
        );
    }

    #[test]
    fn a_quiet_app_is_called_sudden_only_over_the_floor_and_a_busy_one_over_ten_times_its_hour() {
        let quiet = week("Notes", 24 * MB / 10);
        assert!(!is_sudden(&quiet, "Notes", 49 * MB, 100), "under the floor");
        assert!(is_sudden(&quiet, "Notes", 51 * MB, 100));
        let busy = week("Backup", 24 * 20 * MB);
        assert_eq!(limit(usual_hour(&busy, "Backup", 100).unwrap()), 200 * MB);
        assert!(
            !is_sudden(&busy, "Backup", 200 * MB, 100),
            "an app that always sends a lot is not suddenly sending"
        );
        assert!(is_sudden(&busy, "Backup", 201 * MB, 100));
        assert!(
            !is_sudden(&busy, "Mail", 900 * MB, 100),
            "an unknown app is not judged"
        );
        assert_eq!(limit(u64::MAX), u64::MAX, "nothing overflows");
    }
}
