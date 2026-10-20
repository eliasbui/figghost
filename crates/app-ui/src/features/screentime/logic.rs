//! How much time went to which app: the tally that is kept, how it is read back by app, by category and by day, how it
//! is written to and read from a file, and how a length of time is said. Nothing here reads the system or draws.

use serde_json::{Value, json};
use std::collections::BTreeMap;

/// How often the app in front is looked at, in seconds.
pub const SAMPLE_SECONDS: u64 = 5;
/// The most a look may count for, in seconds: a Mac that was asleep or a timer that ran late does not count the gap.
pub const MAX_STEP_SECONDS: u64 = 15;
/// How many days are kept.
pub const KEEP_DAYS: i64 = 30;
/// The choices of how long the keyboard and pointer may be still before time stops counting, in minutes.
pub const IDLE_MINUTES: [u32; 3] = [1, 2, 5];
/// What a file may hold: more apps than this in a day, or more seconds than a day has, is not believed.
const MAX_APPS_PER_DAY: usize = 400;
const MAX_DAY_SECONDS: u64 = 86_400;
const MAX_NAME: usize = 80;

/// Whether the time since the last input says someone is there.
pub fn is_active(idle_seconds: f64, idle_minutes: u32) -> bool {
    idle_seconds < f64::from(idle_minutes) * 60.0
}

/// The seconds a look counts for, from the time since the last one: what it was, and no more than a step.
pub fn step(elapsed: f64) -> u64 {
    if !elapsed.is_finite() || elapsed <= 0.0 {
        return 0;
    }
    (elapsed.round() as u64).min(MAX_STEP_SECONDS)
}

/// The seconds of each app for each day.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct Tally {
    /// Day number (days since 1970-01-01, local) to bundle identifier to seconds.
    days: BTreeMap<i64, BTreeMap<String, u64>>,
    /// Bundle identifier to the name it was last seen with.
    names: BTreeMap<String, String>,
}

fn clean(text: &str, max: usize) -> String {
    let kept: String = text.chars().filter(|c| !c.is_control()).collect();
    kept.trim().chars().take(max).collect()
}

impl Tally {
    #[cfg(test)]
    pub fn is_empty(&self) -> bool {
        self.days.values().all(BTreeMap::is_empty)
    }

    /// How many days have anything in them.
    pub fn days_with_data(&self) -> usize {
        self.days.values().filter(|d| !d.is_empty()).count()
    }

    /// Counts `seconds` for an app on a day.
    pub fn add(&mut self, day: i64, id: &str, name: &str, seconds: u64) {
        if seconds == 0 || id.is_empty() {
            return;
        }
        let apps = self.days.entry(day).or_default();
        if !apps.contains_key(id) && apps.len() >= MAX_APPS_PER_DAY {
            return;
        }
        let spent = apps.entry(id.to_owned()).or_default();
        *spent = (*spent + seconds).min(MAX_DAY_SECONDS);
        let name = clean(name, MAX_NAME);
        if !name.is_empty() {
            self.names.insert(id.to_owned(), name);
        }
    }

    /// Forgets the days older than the ones that are kept, and the names of apps that have nothing left.
    pub fn prune(&mut self, today: i64) {
        self.days
            .retain(|day, _| *day > today - KEEP_DAYS && *day <= today);
        let used: Vec<&String> = self.days.values().flat_map(BTreeMap::keys).collect();
        self.names.retain(|id, _| used.contains(&id));
    }

    pub fn clear(&mut self) {
        *self = Self::default();
    }

    /// What the app was called when it was last seen; the identifier when it never had a name.
    pub fn name_of<'a>(&'a self, id: &'a str) -> &'a str {
        self.names.get(id).map_or(id, String::as_str)
    }

    fn in_range(&self, from: i64, to: i64) -> impl Iterator<Item = (&String, &u64)> {
        self.days.range(from..=to).flat_map(|(_, apps)| apps.iter())
    }

    /// The seconds of all the apps between two days, both included.
    pub fn total(&self, from: i64, to: i64) -> u64 {
        self.in_range(from, to).map(|(_, s)| *s).sum()
    }

    /// The apps between two days with their seconds, the most first.
    pub fn by_app(&self, from: i64, to: i64) -> Vec<(String, u64)> {
        let mut sum: BTreeMap<&String, u64> = BTreeMap::new();
        for (id, seconds) in self.in_range(from, to) {
            *sum.entry(id).or_default() += seconds;
        }
        sorted(sum.into_iter().map(|(id, s)| (id.clone(), s)).collect())
    }

    /// The same, with apps put into the group `group_of` says, the most first.
    pub fn by_group(
        &self,
        from: i64,
        to: i64,
        group_of: impl Fn(&str) -> String,
    ) -> Vec<(String, u64)> {
        let mut sum: BTreeMap<String, u64> = BTreeMap::new();
        for (id, seconds) in self.in_range(from, to) {
            *sum.entry(group_of(id)).or_default() += seconds;
        }
        sorted(sum.into_iter().collect())
    }

    pub fn to_json(&self) -> String {
        let days: serde_json::Map<String, Value> = self
            .days
            .iter()
            .filter(|(_, apps)| !apps.is_empty())
            .map(|(day, apps)| (day.to_string(), json!(apps)))
            .collect();
        json!({ "days": days, "names": self.names }).to_string()
    }

    /// A tally from a file. Anything that is not of the right shape or not believable is left out, and the rest stays.
    pub fn from_json(text: &str) -> Self {
        let mut tally = Self::default();
        let Ok(root) = serde_json::from_str::<Value>(text) else {
            return tally;
        };
        if let Some(days) = root["days"].as_object() {
            for (day, apps) in days {
                let (Ok(day), Some(apps)) = (day.parse::<i64>(), apps.as_object()) else {
                    continue;
                };
                for (id, seconds) in apps.iter().take(MAX_APPS_PER_DAY) {
                    if let Some(seconds) = seconds.as_u64().filter(|s| *s > 0 && !id.is_empty()) {
                        let id = clean(id, 200);
                        if !id.is_empty() {
                            tally
                                .days
                                .entry(day)
                                .or_default()
                                .insert(id, seconds.min(MAX_DAY_SECONDS));
                        }
                    }
                }
            }
        }
        if let Some(names) = root["names"].as_object() {
            for (id, name) in names {
                if let Some(name) = name.as_str() {
                    let name = clean(name, MAX_NAME);
                    if !name.is_empty() {
                        tally.names.insert(clean(id, 200), name);
                    }
                }
            }
        }
        tally
    }
}

fn sorted(mut list: Vec<(String, u64)>) -> Vec<(String, u64)> {
    list.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
    list
}

/// A length of time as whole minutes and hours: (hours, minutes). Under a minute is none of either.
pub fn hours_minutes(seconds: u64) -> (u64, u64) {
    let minutes = seconds / 60;
    (minutes / 60, minutes % 60)
}

/// What an app declares as its category, brought to the name of a group: `public.app-category.developer-tools` is
/// `developer-tools`, and a sub-category such as `games.action-games` is its main one, `games`. An app that declares
/// nothing is in no group, which is the empty text.
pub fn group_of(category: Option<&str>) -> String {
    let Some(text) = category else {
        return String::new();
    };
    let rest = text.strip_prefix("public.app-category.").unwrap_or(text);
    let main = rest.split('.').next().unwrap_or(rest);
    let clean: String = main
        .chars()
        .filter(|c| c.is_ascii_alphanumeric() || *c == '-')
        .take(40)
        .collect();
    clean.to_ascii_lowercase()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn someone_is_there_until_the_keyboard_and_pointer_have_been_still_for_the_chosen_time() {
        assert!(is_active(0.0, 2));
        assert!(is_active(119.9, 2));
        assert!(!is_active(120.0, 2));
        assert!(!is_active(5000.0, 5));
        assert!(is_active(299.0, 5));
        assert!(!is_active(60.0, 1));
    }

    #[test]
    fn a_look_counts_for_the_time_since_the_last_but_never_more_than_a_step() {
        assert_eq!(step(5.0), 5);
        assert_eq!(step(5.4), 5);
        assert_eq!(step(0.2), 0);
        assert_eq!(
            step(600.0),
            MAX_STEP_SECONDS,
            "a Mac that slept does not count the gap"
        );
        assert_eq!(step(-3.0), 0);
        assert_eq!(step(f64::NAN), 0);
        assert_eq!(step(f64::INFINITY), 0);
    }

    fn sample() -> Tally {
        let mut t = Tally::default();
        t.add(100, "com.apple.Safari", "Safari", 600);
        t.add(100, "com.apple.Terminal", "Terminal", 1800);
        t.add(101, "com.apple.Safari", "Safari", 900);
        t.add(101, "com.apple.mail", "Mail", 300);
        t.add(95, "com.apple.mail", "Mail", 3600);
        t
    }

    #[test]
    fn the_time_of_each_app_is_added_up_over_a_range_of_days() {
        let t = sample();
        assert_eq!(t.total(100, 100), 2400);
        assert_eq!(t.total(100, 101), 3600);
        assert_eq!(t.total(90, 101), 7200);
        assert_eq!(t.total(200, 300), 0);
        assert_eq!(
            t.by_app(100, 101),
            [
                ("com.apple.Terminal".to_owned(), 1800),
                ("com.apple.Safari".to_owned(), 1500),
                ("com.apple.mail".to_owned(), 300),
            ]
        );
        assert_eq!(t.name_of("com.apple.mail"), "Mail");
        assert_eq!(t.name_of("com.example.unseen"), "com.example.unseen");
    }

    #[test]
    fn equal_times_are_listed_in_a_steady_order() {
        let mut t = Tally::default();
        for id in ["c.app", "a.app", "b.app"] {
            t.add(1, id, id, 60);
        }
        let ids: Vec<String> = t.by_app(1, 1).into_iter().map(|(id, _)| id).collect();
        assert_eq!(ids, ["a.app", "b.app", "c.app"]);
    }

    #[test]
    fn apps_are_put_into_groups_and_the_groups_added_up() {
        let t = sample();
        let by = t.by_group(100, 101, |id| {
            if id.ends_with("Safari") || id.ends_with("mail") {
                "productivity".into()
            } else {
                "utilities".into()
            }
        });
        assert_eq!(
            by,
            [
                ("productivity".to_owned(), 1800),
                ("utilities".to_owned(), 1800)
            ]
        );
    }

    #[test]
    fn a_day_cannot_hold_more_than_a_day_and_nothing_counts_without_an_app_or_a_time() {
        let mut t = Tally::default();
        t.add(1, "a.app", "A", 80_000);
        t.add(1, "a.app", "A", 80_000);
        assert_eq!(t.total(1, 1), MAX_DAY_SECONDS);
        t.add(1, "", "Nameless", 60);
        t.add(1, "b.app", "B", 0);
        assert_eq!(t.by_app(1, 1).len(), 1);
    }

    #[test]
    fn a_day_with_too_many_apps_takes_no_more_new_ones() {
        let mut t = Tally::default();
        for i in 0..(MAX_APPS_PER_DAY + 50) {
            t.add(1, &format!("com.example.app{i}"), "x", 60);
        }
        assert_eq!(t.by_app(1, 1).len(), MAX_APPS_PER_DAY);
        t.add(1, "com.example.app0", "x", 60);
        assert_eq!(
            t.by_app(1, 1)
                .iter()
                .find(|(id, _)| id == "com.example.app0")
                .unwrap()
                .1,
            120,
            "an app already there still counts"
        );
    }

    #[test]
    fn old_days_and_the_names_of_apps_with_nothing_left_are_forgotten() {
        let mut t = Tally::default();
        t.add(10, "old.app", "Old", 60);
        t.add(100, "new.app", "New", 60);
        t.prune(100);
        assert_eq!(t.total(0, 200), 60);
        assert_eq!(t.name_of("old.app"), "old.app");
        assert_eq!(t.name_of("new.app"), "New");
        t.add(120, "future.app", "Future", 60);
        t.prune(100);
        assert_eq!(
            t.by_app(0, 500).len(),
            1,
            "a day that has not come yet is not believed either"
        );
        let mut kept = Tally::default();
        kept.add(100 - KEEP_DAYS + 1, "edge.app", "Edge", 60);
        kept.add(100 - KEEP_DAYS, "gone.app", "Gone", 60);
        kept.prune(100);
        assert_eq!(kept.by_app(0, 500).len(), 1);
    }

    #[test]
    fn a_tally_is_written_and_read_back_the_same() {
        let t = sample();
        assert_eq!(Tally::from_json(&t.to_json()), t);
        assert!(Tally::from_json(&Tally::default().to_json()).is_empty());
    }

    #[test]
    fn a_file_that_is_wrong_in_places_keeps_what_is_right() {
        let text = r#"{
            "days": {
                "100": { "a.app": 600, "bad": "x", "neg": -5, "zero": 0, "huge": 99999999, "": 60 },
                "notaday": { "b.app": 60 },
                "101": "not an object",
                "102": { "c.app": 30 }
            },
            "names": { "a.app": "  A  ", "b.app": 5, "c.app": "" }
        }"#;
        let t = Tally::from_json(text);
        assert_eq!(
            t.by_app(0, 500),
            [
                ("huge".to_owned(), MAX_DAY_SECONDS),
                ("a.app".to_owned(), 600),
                ("c.app".to_owned(), 30)
            ]
        );
        assert_eq!(t.name_of("a.app"), "A");
        assert_eq!(t.name_of("b.app"), "b.app");
        for broken in ["", "not json", "[]", "null", r#"{"days":5}"#] {
            assert!(Tally::from_json(broken).is_empty(), "{broken}");
        }
    }

    #[test]
    fn a_length_of_time_is_whole_hours_and_minutes() {
        assert_eq!(hours_minutes(0), (0, 0));
        assert_eq!(hours_minutes(59), (0, 0));
        assert_eq!(hours_minutes(60), (0, 1));
        assert_eq!(hours_minutes(3_599), (0, 59));
        assert_eq!(hours_minutes(3_600), (1, 0));
        assert_eq!(hours_minutes(13_320), (3, 42));
    }

    #[test]
    fn a_category_is_brought_to_the_name_of_a_group() {
        assert_eq!(
            group_of(Some("public.app-category.developer-tools")),
            "developer-tools"
        );
        assert_eq!(
            group_of(Some("public.app-category.games.action-games")),
            "games"
        );
        assert_eq!(
            group_of(Some("public.app-category.Productivity")),
            "productivity"
        );
        assert_eq!(group_of(Some("com.vendor.custom")), "com");
        assert_eq!(group_of(Some("")), "");
        assert_eq!(group_of(None), "");
        assert_eq!(group_of(Some("public.app-category.we ird;\n")), "weird");
    }
}
