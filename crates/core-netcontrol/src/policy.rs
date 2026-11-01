//! When an app is refused and when a connection is metered: pure decisions from what the history counted, so that every
//! rule is a unit test. The proxy only obeys the answer (`Control::set_policy`).

use crate::history::{self, History};
use std::net::Ipv4Addr;

/// How long a quota or a cap lasts before it counts from nothing again.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Period {
    Day,
    /// From the first of the calendar month.
    Month,
}

impl Period {
    pub fn id(self) -> &'static str {
        match self {
            Period::Day => "day",
            Period::Month => "month",
        }
    }

    pub fn from_id(id: &str) -> Option<Period> {
        match id {
            "day" => Some(Period::Day),
            "month" => Some(Period::Month),
            _ => None,
        }
    }

    /// The first and last day of the period that `today` is in.
    pub fn range(self, today: i64) -> (i64, i64) {
        match self {
            Period::Day => (today, today),
            Period::Month => (history::first_of_month(today), today),
        }
    }

    /// A number that is the same for every day of one period and differs from the next: what a notice is remembered by.
    pub fn key(self, today: i64) -> i64 {
        self.range(today).0
    }
}

/// How far a quota has been used: `Ok`, `Warn` from 80 %, `Over` from 100 %. A quota of nothing is never reached.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Level {
    Ok,
    Warn,
    Over,
}

impl Level {
    pub fn of(used: u64, quota: u64) -> Level {
        if quota == 0 {
            Level::Ok
        } else if used >= quota {
            Level::Over
        } else if u128::from(used) * 5 >= u128::from(quota) * 4 {
            Level::Warn
        } else {
            Level::Ok
        }
    }
}

/// What one app may move in a period.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Quota {
    pub app: String,
    pub bytes: u64,
    pub period: Period,
}

/// How far each quota is used, in the order of `quotas`.
pub fn quota_levels(history: &History, quotas: &[Quota], today: i64) -> Vec<(u64, Level)> {
    quotas
        .iter()
        .map(|q| {
            let (from, to) = q.period.range(today);
            let used = history.bytes(from, to, Some(&q.app), None);
            (used, Level::of(used, q.bytes))
        })
        .collect()
}

/// The apps whose quota has run out.
pub fn over_quota(history: &History, quotas: &[Quota], today: i64) -> Vec<String> {
    quota_levels(history, quotas, today)
        .into_iter()
        .zip(quotas)
        .filter(|((_, level), _)| *level == Level::Over)
        .map(|(_, q)| q.app.clone())
        .collect()
}

/// A connection that looks like a phone's: by its name in System Settings (`iPhone USB`), or by the address a phone
/// gives when it shares its data (an iPhone's hotspot is 172.20.10.0/28, an Android phone's is mostly 192.168.43.0/24).
/// A guess, which the person can correct by marking connections by hand.
pub fn looks_like_phone(label: &str, ip: Ipv4Addr) -> bool {
    let label = label.to_lowercase();
    let o = ip.octets();
    ["iphone", "ipad", "android"]
        .iter()
        .any(|word| label.contains(word))
        || (o[0] == 172 && o[1] == 20 && o[2] == 10 && o[3] < 16)
        || (o[0] == 192 && o[1] == 168 && o[2] == 43)
}

/// Whether a connection is metered: marked by hand, or taken for a phone's when `auto` is on. `unmarked` are the
/// connections the person said are not metered, which beat the guess.
pub fn is_metered(
    name: &str,
    label: &str,
    ip: Ipv4Addr,
    marked: &[String],
    unmarked: &[String],
    auto: bool,
) -> bool {
    if unmarked.iter().any(|n| n == name) {
        return false;
    }
    marked.iter().any(|n| n == name) || (auto && looks_like_phone(label, ip))
}

#[cfg(test)]
mod tests {
    use super::*;

    const MB: u64 = 1_000_000;

    #[test]
    fn a_quota_is_ok_then_warns_at_four_fifths_then_is_over() {
        assert_eq!(Level::of(0, 100), Level::Ok);
        assert_eq!(Level::of(79, 100), Level::Ok);
        assert_eq!(Level::of(80, 100), Level::Warn);
        assert_eq!(Level::of(99, 100), Level::Warn);
        assert_eq!(Level::of(100, 100), Level::Over);
        assert_eq!(
            Level::of(u64::MAX, 0),
            Level::Ok,
            "no quota is never reached"
        );
        assert_eq!(
            Level::of(u64::MAX, u64::MAX),
            Level::Over,
            "nothing overflows"
        );
        assert!(Level::Over > Level::Warn && Level::Warn > Level::Ok);
    }

    #[test]
    fn a_period_runs_from_its_beginning_to_today() {
        let nov_1 = history::days_from_civil(2025, 11, 1);
        let nov_15 = nov_1 + 14;
        assert_eq!(Period::Day.range(nov_15), (nov_15, nov_15));
        assert_eq!(Period::Month.range(nov_15), (nov_1, nov_15));
        assert_eq!(Period::Month.key(nov_15), Period::Month.key(nov_1));
        assert_ne!(
            Period::Month.key(nov_1),
            Period::Month.key(nov_1 - 1),
            "the last of October is another month"
        );
        assert_ne!(Period::Day.key(nov_15), Period::Day.key(nov_15 + 1));
        assert_eq!(
            (
                Period::from_id("day"),
                Period::from_id("month"),
                Period::from_id("week")
            ),
            (Some(Period::Day), Some(Period::Month), None)
        );
        assert_eq!(Period::Month.id(), "month");
    }

    #[test]
    fn only_an_app_over_its_quota_in_this_period_is_refused() {
        let nov_1 = history::days_from_civil(2025, 11, 1);
        let mut h = History::default();
        h.add(nov_1 - 2, "Steam", "en0", 900 * MB, 0);
        h.add(nov_1 + 3, "Steam", "en0", 60 * MB, 0);
        h.add(nov_1 + 3, "Dropbox", "en0", 70 * MB, 5 * MB);
        let quotas = [
            Quota {
                app: "steam".into(),
                bytes: 100 * MB,
                period: Period::Month,
            },
            Quota {
                app: "Dropbox".into(),
                bytes: 100 * MB,
                period: Period::Day,
            },
            Quota {
                app: "Mail".into(),
                bytes: 1,
                period: Period::Day,
            },
        ];
        let today = nov_1 + 3;
        let levels = quota_levels(&h, &quotas, today);
        assert_eq!(
            levels[0],
            (60 * MB, Level::Ok),
            "last month's 900 MB do not count"
        );
        assert_eq!(levels[1], (75 * MB, Level::Ok));
        assert_eq!(
            levels[2],
            (0, Level::Ok),
            "an app with no traffic uses nothing"
        );
        assert!(over_quota(&h, &quotas, today).is_empty());
        h.add(today, "Steam", "en7", 45 * MB, 0);
        assert_eq!(over_quota(&h, &quotas, today), ["steam"]);
        assert_eq!(
            over_quota(&h, &quotas, today + 1),
            ["steam"],
            "the month goes on; the day of Dropbox starts again"
        );
        assert!(
            over_quota(&h, &quotas, nov_1 + 40).is_empty(),
            "a month later nothing was counted"
        );
    }

    #[test]
    fn a_phone_is_known_by_its_name_or_the_address_it_gives() {
        let ip = |a, b, c, d| Ipv4Addr::new(a, b, c, d);
        assert!(looks_like_phone("iPhone USB", ip(169, 254, 1, 2)));
        assert!(looks_like_phone("Android Tethering", ip(10, 0, 0, 5)));
        assert!(
            looks_like_phone("Wi-Fi", ip(172, 20, 10, 2)),
            "an iPhone's hotspot"
        );
        assert!(
            looks_like_phone("Wi-Fi", ip(192, 168, 43, 17)),
            "an Android phone's hotspot"
        );
        assert!(
            !looks_like_phone("Wi-Fi", ip(172, 20, 10, 16)),
            "outside the iPhone's small range"
        );
        assert!(!looks_like_phone("Wi-Fi", ip(192, 168, 1, 20)));
        assert!(!looks_like_phone("USB 10/100/1000 LAN", ip(10, 0, 0, 5)));
    }

    #[test]
    fn the_person_has_the_last_word_on_whether_a_connection_is_metered() {
        let phone = Ipv4Addr::new(172, 20, 10, 2);
        let home = Ipv4Addr::new(192, 168, 1, 2);
        let none: [String; 0] = [];
        let en0 = ["en0".to_owned()];
        assert!(
            is_metered("en0", "Wi-Fi", phone, &none, &none, true),
            "guessed"
        );
        assert!(
            !is_metered("en0", "Wi-Fi", phone, &none, &none, false),
            "the guess is off"
        );
        assert!(
            !is_metered("en0", "Wi-Fi", phone, &none, &en0, true),
            "marked as not metered"
        );
        assert!(
            is_metered("en0", "Wi-Fi", home, &en0, &none, false),
            "marked as metered"
        );
        assert!(!is_metered("en1", "Wi-Fi", home, &en0, &none, true));
    }
}
