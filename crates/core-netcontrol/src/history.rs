//! What each app moved over the days, by interface: added up from the steps of `monitor`, kept in a small text file and
//! read back by app, by interface and by day. Pure data with no clock: the caller says which day it is.

use crate::{monitor::Usage, rules};
use std::collections::BTreeMap;

/// How many days are kept.
pub const KEEP_DAYS: i64 = 90;
/// What a file may hold: more apps than this in a day, or more bytes than this in a record, is not believed.
const MAX_APPS_PER_DAY: usize = 400;
const MAX_BYTES: u64 = 1 << 50;
const HEADER: &str = "figghost-network-history 1";

/// Bytes in and out.
type Pair = (u64, u64);

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct History {
    /// Day number (days since 1970-01-01, local) to app to interface to bytes.
    days: BTreeMap<i64, BTreeMap<String, BTreeMap<String, Pair>>>,
}

/// What one app moved over a range of days.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Total {
    pub app: String,
    pub down: u64,
    pub up: u64,
    /// By interface, the most first.
    pub interfaces: Vec<(String, u64, u64)>,
}

/// One line of an export.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Row {
    pub date: String,
    pub app: String,
    pub interface: String,
    pub down: u64,
    pub up: u64,
}

impl History {
    pub fn is_empty(&self) -> bool {
        self.days.values().all(BTreeMap::is_empty)
    }

    /// Counts bytes for an app on an interface on a day. An app that is not a name, and a day's 401st app, count for
    /// nothing; bytes never go below zero because nothing is ever taken away.
    pub fn add(&mut self, day: i64, app: &str, interface: &str, down: u64, up: u64) {
        if (down == 0 && up == 0) || !rules::valid_app(app) {
            return;
        }
        let apps = self.days.entry(day).or_default();
        if !apps.contains_key(app) && apps.len() >= MAX_APPS_PER_DAY {
            return;
        }
        let interface = if rules::valid_interface(interface) {
            interface
        } else {
            "?"
        };
        let slot = apps
            .entry(app.to_owned())
            .or_default()
            .entry(interface.to_owned())
            .or_default();
        slot.0 = slot.0.saturating_add(down).min(MAX_BYTES);
        slot.1 = slot.1.saturating_add(up).min(MAX_BYTES);
    }

    /// Adds what the apps moved in one step of the monitor.
    pub fn add_step(&mut self, day: i64, usage: &[Usage]) {
        for u in usage {
            for (interface, down, up) in &u.delta {
                self.add(day, &u.name, interface, *down, *up);
            }
        }
    }

    /// Forgets the days older than the ones that are kept, and days that have not come yet.
    pub fn prune(&mut self, today: i64) {
        self.days
            .retain(|day, apps| *day > today - KEEP_DAYS && *day <= today && !apps.is_empty());
    }

    pub fn clear(&mut self) {
        self.days.clear();
    }

    /// The apps between two days, both included, with what they moved, the most first.
    pub fn totals(&self, from: i64, to: i64) -> Vec<Total> {
        let mut by_app: BTreeMap<&str, BTreeMap<&str, Pair>> = BTreeMap::new();
        for (_, apps) in self.days.range(from..=to) {
            for (app, interfaces) in apps {
                let slot = by_app.entry(app).or_default();
                for (interface, (down, up)) in interfaces {
                    let pair = slot.entry(interface).or_default();
                    pair.0 += down;
                    pair.1 += up;
                }
            }
        }
        let mut out: Vec<Total> = by_app
            .into_iter()
            .map(|(app, interfaces)| {
                let mut interfaces: Vec<(String, u64, u64)> = interfaces
                    .into_iter()
                    .map(|(i, (d, u))| (i.to_owned(), d, u))
                    .collect();
                interfaces.sort_by(|a, b| (b.1 + b.2).cmp(&(a.1 + a.2)).then(a.0.cmp(&b.0)));
                Total {
                    app: app.to_owned(),
                    down: interfaces.iter().map(|i| i.1).sum(),
                    up: interfaces.iter().map(|i| i.2).sum(),
                    interfaces,
                }
            })
            .collect();
        out.sort_by(|a, b| {
            (b.down + b.up)
                .cmp(&(a.down + a.up))
                .then(a.app.cmp(&b.app))
        });
        out
    }

    /// The bytes (in and out) between two days, both included, of one app or of all, on some interfaces or on all.
    pub fn bytes(
        &self,
        from: i64,
        to: i64,
        app: Option<&str>,
        interfaces: Option<&[String]>,
    ) -> u64 {
        let mut sum = 0u64;
        for (_, apps) in self.days.range(from..=to) {
            for (name, by_interface) in apps {
                if app.is_some_and(|a| !a.eq_ignore_ascii_case(name)) {
                    continue;
                }
                for (interface, (down, up)) in by_interface {
                    if interfaces.is_none_or(|list| list.contains(interface)) {
                        sum = sum.saturating_add(down + up);
                    }
                }
            }
        }
        sum
    }

    /// What one app sent on each day it had anything counted, between two days, both included, the oldest first.
    pub fn sent_by_day(&self, from: i64, to: i64, app: &str) -> Vec<u64> {
        self.days
            .range(from..=to)
            .filter_map(|(_, apps)| {
                let (_, by_interface) = apps
                    .iter()
                    .find(|(name, _)| name.eq_ignore_ascii_case(app))?;
                Some(by_interface.values().map(|(_, up)| up).sum())
            })
            .collect()
    }

    /// What all the apps moved on each day from `from` to `to`, a pair for every day, quiet ones included.
    pub fn per_day(&self, from: i64, to: i64) -> Vec<(i64, u64, u64)> {
        (from..=to)
            .map(|day| {
                let (mut down, mut up) = (0, 0);
                for interfaces in self.days.get(&day).into_iter().flat_map(BTreeMap::values) {
                    for (d, u) in interfaces.values() {
                        down += d;
                        up += u;
                    }
                }
                (day, down, up)
            })
            .collect()
    }

    /// The lines of an export between two days: one for each day, app and interface, or one for each month when
    /// `monthly`, the oldest first.
    pub fn rows(&self, from: i64, to: i64, monthly: bool) -> Vec<Row> {
        let mut merged: BTreeMap<(String, &str, &str), Pair> = BTreeMap::new();
        for (day, apps) in self.days.range(from..=to) {
            let (y, m, d) = civil(*day);
            let date = if monthly {
                format!("{y:04}-{m:02}")
            } else {
                format!("{y:04}-{m:02}-{d:02}")
            };
            for (app, interfaces) in apps {
                for (interface, (down, up)) in interfaces {
                    let pair = merged.entry((date.clone(), app, interface)).or_default();
                    pair.0 += down;
                    pair.1 += up;
                }
            }
        }
        merged
            .into_iter()
            .map(|((date, app, interface), (down, up))| Row {
                date,
                app: app.to_owned(),
                interface: interface.to_owned(),
                down,
                up,
            })
            .collect()
    }

    /// The text of the history file: a header, then a line for each day, app and interface, with tabs between.
    pub fn write(&self) -> String {
        let mut out = format!("{HEADER}\n");
        for (day, apps) in &self.days {
            for (app, interfaces) in apps {
                for (interface, (down, up)) in interfaces {
                    out.push_str(&format!("{day}\t{app}\t{interface}\t{down}\t{up}\n"));
                }
            }
        }
        out
    }

    /// The history a file holds. A line that is not of the right shape is left out and the rest is kept.
    pub fn read(text: &str) -> History {
        let mut history = History::default();
        let mut lines = text.lines();
        if lines.next() != Some(HEADER) {
            return history;
        }
        for line in lines {
            let fields: Vec<&str> = line.split('\t').collect();
            let [day, app, interface, down, up] = fields[..] else {
                continue;
            };
            let (Ok(day), Ok(down), Ok(up)) =
                (day.parse::<i64>(), down.parse::<u64>(), up.parse::<u64>())
            else {
                continue;
            };
            if down > MAX_BYTES || up > MAX_BYTES || !(0..200_000).contains(&day) {
                continue;
            }
            history.add(day, app, interface, down, up);
        }
        history
    }
}

/// A field of a CSV line: in quotes when it holds a comma, a quote or a line break, and never starting with a character
/// a spreadsheet would take for a formula.
fn csv_field(text: &str) -> String {
    let text = if text.starts_with(['=', '+', '-', '@']) {
        format!("'{text}")
    } else {
        text.to_owned()
    };
    if text.contains([',', '"', '\n', '\r']) {
        format!("\"{}\"", text.replace('"', "\"\""))
    } else {
        text
    }
}

/// The rows as CSV: `date,app,interface,bytes_in,bytes_out`.
pub fn csv(rows: &[Row]) -> String {
    let mut out = String::from("date,app,interface,bytes_in,bytes_out\n");
    for r in rows {
        out.push_str(&format!(
            "{},{},{},{},{}\n",
            r.date,
            csv_field(&r.app),
            r.interface,
            r.down,
            r.up
        ));
    }
    out
}

/// The day number of a date, the other way round from `civil`.
pub fn days_from_civil(y: i64, m: u32, d: u32) -> i64 {
    let y = y - i64::from(m <= 2);
    let era = y.div_euclid(400);
    let yoe = y.rem_euclid(400);
    let mp = i64::from(if m > 2 { m - 3 } else { m + 9 });
    let doy = (153 * mp + 2) / 5 + i64::from(d) - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

/// The day number of the first of the month that `day` is in.
pub fn first_of_month(day: i64) -> i64 {
    let (y, m, _) = civil(day);
    days_from_civil(y, m, 1)
}

/// Year, month and day of a day number counted from 1970-01-01.
pub fn civil(day: i64) -> (i64, u32, u32) {
    let z = day + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    (yoe + era * 400 + i64::from(m <= 2), m, d)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn usage(name: &str, delta: &[(&str, u64, u64)]) -> Usage {
        Usage {
            name: name.into(),
            delta: delta
                .iter()
                .map(|(i, d, u)| ((*i).into(), *d, *u))
                .collect(),
            ..Usage::default()
        }
    }

    #[test]
    fn steps_are_added_up_by_day_app_and_interface() {
        let mut h = History::default();
        h.add_step(
            100,
            &[usage("Safari", &[("en0", 1000, 100), ("en7", 50, 0)])],
        );
        h.add_step(
            100,
            &[
                usage("Safari", &[("en0", 500, 0)]),
                usage("Steam", &[("en0", 9000, 10)]),
            ],
        );
        h.add_step(101, &[usage("Safari", &[("en0", 1, 2)])]);
        let all = h.totals(100, 101);
        assert_eq!(all[0].app, "Steam", "the most first");
        let safari = &all[1];
        assert_eq!((safari.down, safari.up), (1551, 102));
        assert_eq!(safari.interfaces[0], ("en0".to_owned(), 1501, 102));
        let today = h.totals(101, 101);
        assert_eq!(today.len(), 1);
        assert_eq!((today[0].down, today[0].up), (1, 2));
    }

    #[test]
    fn nothing_that_moved_nothing_or_has_no_name_is_kept() {
        let mut h = History::default();
        h.add(1, "Safari", "en0", 0, 0);
        h.add(1, "", "en0", 5, 5);
        h.add(1, "a/b", "en0", 5, 5);
        h.add(1, "Safari", "../x", 5, 0);
        assert_eq!(h.totals(0, 2).len(), 1);
        assert_eq!(
            h.totals(0, 2)[0].interfaces[0].0,
            "?",
            "a name that is no interface is kept as unknown"
        );
        assert!(History::default().is_empty());
    }

    #[test]
    fn counts_never_overflow_and_a_day_takes_no_more_than_so_many_apps() {
        let mut h = History::default();
        h.add(1, "Big", "en0", u64::MAX, u64::MAX);
        h.add(1, "Big", "en0", u64::MAX, 1);
        assert_eq!(h.totals(1, 1)[0].down, MAX_BYTES);
        for i in 0..MAX_APPS_PER_DAY + 10 {
            h.add(2, &format!("App {i}"), "en0", 1, 0);
        }
        assert_eq!(h.totals(2, 2).len(), MAX_APPS_PER_DAY);
        h.add(2, "App 0", "en0", 1, 0);
        assert_eq!(
            h.totals(2, 2)
                .iter()
                .find(|t| t.app == "App 0")
                .unwrap()
                .down,
            2,
            "a known app still counts"
        );
    }

    #[test]
    fn old_days_and_days_to_come_are_forgotten() {
        let mut h = History::default();
        for day in [1, 50, 100, 200, 300] {
            h.add(day, "A", "en0", 1, 1);
        }
        h.prune(200);
        let days: Vec<i64> = h
            .per_day(0, 400)
            .into_iter()
            .filter(|(_, d, _)| *d > 0)
            .map(|d| d.0)
            .collect();
        assert_eq!(
            days,
            [200],
            "100 is exactly {KEEP_DAYS} days old or more, 300 is to come"
        );
        h.add(111, "A", "en0", 1, 1);
        h.prune(200);
        assert_eq!(h.per_day(111, 111)[0].1, 1, "the oldest day that is kept");
    }

    #[test]
    fn each_day_of_a_range_is_listed_even_when_it_was_quiet() {
        let mut h = History::default();
        h.add(10, "A", "en0", 5, 1);
        h.add(12, "A", "en0", 7, 2);
        assert_eq!(h.per_day(10, 12), [(10, 5, 1), (11, 0, 0), (12, 7, 2)]);
    }

    #[test]
    fn the_file_is_written_and_read_back_and_a_damaged_one_keeps_what_is_right() {
        let mut h = History::default();
        h.add(100, "Microsoft Edge", "en0", 10, 20);
        h.add(100, "Microsoft Edge", "en7", 1, 2);
        h.add(101, "Steam", "en0", 3, 4);
        assert_eq!(History::read(&h.write()), h);
        let broken = format!(
            "{HEADER}\n100\tSafari\ten0\t1\t2\nnonsense\n100\tSafari\ten0\tx\t2\n100\tA\ten0\t1\n-5\tB\ten0\t1\t1\n101\tSteam\ten0\t3\t4\n"
        );
        let kept = History::read(&broken);
        assert_eq!(kept.totals(0, 200).len(), 2);
        for text in [
            "",
            "not a history",
            "figghost-network-history 2\n100\tA\ten0\t1\t1\n",
        ] {
            assert!(History::read(text).is_empty(), "{text:?}");
        }
    }

    #[test]
    fn a_date_goes_to_its_day_number_and_back_and_a_month_begins_on_its_first() {
        for day in [-800, -1, 0, 59, 60, 11_016, 20_392, 20_393, 40_000] {
            let (y, m, d) = civil(day);
            assert_eq!(days_from_civil(y, m, d), day, "{y}-{m}-{d}");
        }
        assert_eq!(first_of_month(20_393), 20_393, "the first of November");
        assert_eq!(
            first_of_month(20_392),
            20_392 - 30,
            "the 31st of October is 30 days after the first"
        );
        assert_eq!(first_of_month(11_016), 11_016 - 28, "29 February 2000");
    }

    #[test]
    fn what_an_app_sent_is_listed_for_each_day_it_had_anything() {
        let mut h = History::default();
        h.add(10, "Safari", "en0", 5, 100);
        h.add(10, "Safari", "en7", 5, 50);
        h.add(12, "safari", "en0", 5, 7);
        h.add(11, "Steam", "en0", 5, 9);
        assert_eq!(h.sent_by_day(10, 12, "SAFARI"), [150, 7]);
        assert!(h.sent_by_day(10, 12, "Mail").is_empty());
        assert!(h.sent_by_day(13, 20, "Safari").is_empty());
    }

    #[test]
    fn bytes_are_added_up_for_an_app_and_for_some_interfaces() {
        let mut h = History::default();
        h.add(10, "Safari", "en0", 100, 10);
        h.add(10, "Safari", "en7", 1000, 0);
        h.add(11, "Steam", "en7", 5000, 5);
        let en7 = ["en7".to_owned()];
        assert_eq!(h.bytes(10, 11, None, None), 6115);
        assert_eq!(
            h.bytes(10, 11, Some("safari"), None),
            1110,
            "the case of the name does not matter"
        );
        assert_eq!(h.bytes(10, 11, None, Some(&en7)), 6005);
        assert_eq!(h.bytes(10, 10, Some("Safari"), Some(&en7)), 1000);
        assert_eq!(h.bytes(12, 20, None, None), 0);
    }

    #[test]
    fn days_are_named_as_the_calendar_names_them() {
        assert_eq!(civil(0), (1970, 1, 1));
        assert_eq!(civil(59), (1970, 3, 1));
        assert_eq!(civil(11_016), (2000, 2, 29));
        assert_eq!(civil(20_392), (2025, 10, 31));
        assert_eq!(civil(20_393), (2025, 11, 1));
        assert_eq!(civil(-1), (1969, 12, 31));
    }

    #[test]
    fn an_export_is_by_day_or_by_month_and_survives_a_comma_and_a_formula() {
        let mut h = History::default();
        // 2025-10-31 and 2025-11-01.
        h.add(20_392, "Safari", "en0", 10, 1);
        h.add(20_393, "Safari", "en0", 5, 1);
        h.add(20_393, "=cmd, \"x\"", "en0", 7, 0);
        let days = h.rows(20_000, 21_000, false);
        assert_eq!(days.len(), 3);
        assert_eq!(days[0].date, "2025-10-31");
        let months = h.rows(20_000, 21_000, true);
        assert_eq!(months.len(), 3, "two apps in November and one in October");
        assert!(months.iter().all(|r| r.date.len() == 7));
        let text = csv(&days);
        assert!(text.starts_with("date,app,interface,bytes_in,bytes_out\n"));
        assert!(text.contains("2025-10-31,Safari,en0,10,1\n"));
        assert!(text.contains("\"'=cmd, \"\"x\"\"\""), "{text}");
        assert!(h.rows(0, 10, false).is_empty());
    }
}
