//! Time zones from the system's own database, which knows daylight saving time.

use objc2_foundation::{NSDate, NSString, NSTimeZone};

fn zone(id: &str) -> Option<objc2::rc::Retained<NSTimeZone>> {
    match id {
        "local" => Some(NSTimeZone::localTimeZone()),
        "UTC" => Some(NSTimeZone::timeZoneForSecondsFromGMT(0)),
        other => NSTimeZone::timeZoneWithName(&NSString::from_str(other)),
    }
}

/// Minutes east of UTC that the zone is at a Unix time. "local" is the reader's own zone, anything else is an
/// IANA identifier such as "Asia/Tokyo".
pub fn offset_minutes(id: &str, at_unix: i64) -> Option<i32> {
    let date = NSDate::dateWithTimeIntervalSince1970(at_unix as f64);
    Some((zone(id)?.secondsFromGMTForDate(&date) / 60) as i32)
}

/// Whether the system knows this zone.
pub fn exists(id: &str) -> bool {
    zone(id).is_some()
}

#[cfg(test)]
mod tests {
    use super::*;

    // 2026-07-15 12:00 UTC and 2026-01-15 12:00 UTC.
    const SUMMER: i64 = 1_784_116_800;
    const WINTER: i64 = 1_768_478_400;

    #[test]
    fn fixed_and_named_zones() {
        assert_eq!(offset_minutes("UTC", SUMMER), Some(0));
        assert_eq!(offset_minutes("Asia/Tokyo", SUMMER), Some(540));
        assert_eq!(offset_minutes("Asia/Ho_Chi_Minh", WINTER), Some(420));
        assert_eq!(offset_minutes("Asia/Kolkata", WINTER), Some(330));
    }

    #[test]
    fn daylight_saving_time_follows_the_date() {
        assert_eq!(offset_minutes("America/New_York", SUMMER), Some(-240));
        assert_eq!(offset_minutes("America/New_York", WINTER), Some(-300));
        assert_eq!(offset_minutes("Europe/London", SUMMER), Some(60));
        assert_eq!(offset_minutes("Europe/London", WINTER), Some(0));
    }

    #[test]
    fn the_readers_own_zone_exists_and_nonsense_does_not() {
        assert!(offset_minutes("local", SUMMER).is_some());
        assert!(exists("local") && exists("Europe/Paris"));
        assert_eq!(offset_minutes("Mars/Olympus", SUMMER), None);
        assert!(!exists("") && !exists("not a zone"));
    }
}
