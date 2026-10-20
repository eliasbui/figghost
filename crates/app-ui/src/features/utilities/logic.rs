//! The small rules of the utilities: how long a keep-awake can be, and how time is written on an island.

/// The lengths offered for keeping the Mac awake, in minutes. 0 is "until I stop".
pub const AWAKE_MINUTES: [u32; 6] = [15, 30, 60, 120, 240, 0];
/// The lengths offered for snoozing the notch, in minutes. 0 is "until I wake it".
pub const SNOOZE_MINUTES: [u32; 6] = [5, 15, 30, 60, 120, 0];
/// The lengths of the keyboard lock the settings offer, in seconds.
pub const LOCK_SECONDS: (u32, u32) = (15, 300);

/// Held to the lengths that are offered; anything else is "until I stop".
pub fn awake_minutes(minutes: u32) -> u32 {
    if AWAKE_MINUTES.contains(&minutes) {
        minutes
    } else {
        0
    }
}

/// Held to the lengths that are offered; anything else is half an hour.
pub fn snooze_minutes(minutes: u32) -> u32 {
    if SNOOZE_MINUTES.contains(&minutes) {
        minutes
    } else {
        30
    }
}

/// Seconds as a clock: `0:42`, `4:05`, `1:02:09`.
pub fn clock(seconds: u64) -> String {
    let (h, m, s) = (seconds / 3600, seconds % 3600 / 60, seconds % 60);
    if h > 0 {
        format!("{h}:{m:02}:{s:02}")
    } else {
        format!("{m}:{s:02}")
    }
}

/// What is left, as it is said in a menu: `45 s`, `12 min`, `1 h 05 min`. A minute that has begun counts.
pub fn left(seconds: u64) -> String {
    if seconds < 60 {
        return format!("{seconds} s");
    }
    let minutes = seconds.div_ceil(60);
    if minutes < 60 {
        format!("{minutes} min")
    } else {
        format!("{} h {:02} min", minutes / 60, minutes % 60)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_length_that_is_not_offered_means_until_stopped() {
        assert_eq!(awake_minutes(60), 60);
        assert_eq!(awake_minutes(0), 0);
        assert_eq!(awake_minutes(37), 0);
        assert_eq!(awake_minutes(100_000), 0);
    }

    #[test]
    fn a_snooze_that_is_not_offered_is_half_an_hour() {
        for minutes in SNOOZE_MINUTES {
            assert_eq!(snooze_minutes(minutes), minutes);
        }
        assert_eq!(snooze_minutes(7), 30);
        assert_eq!(snooze_minutes(u32::MAX), 30);
    }

    #[test]
    fn the_clock_has_minutes_and_seconds_and_hours_when_needed() {
        assert_eq!(clock(0), "0:00");
        assert_eq!(clock(42), "0:42");
        assert_eq!(clock(245), "4:05");
        assert_eq!(clock(3729), "1:02:09");
    }

    #[test]
    fn what_is_left_rounds_up_to_the_minute_that_has_begun() {
        assert_eq!(left(45), "45 s");
        assert_eq!(left(60), "1 min");
        assert_eq!(left(61), "2 min");
        assert_eq!(left(3600), "1 h 00 min");
        assert_eq!(left(3900), "1 h 05 min");
    }
}
