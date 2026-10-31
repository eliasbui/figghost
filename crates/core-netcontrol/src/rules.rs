//! What each app is wired to on the diagram, and what the wire allows: pure data, so every rule is a unit test.

/// Where the connections of an app leave the computer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Target {
    /// The way the system would send them without Figghost.
    System,
    /// Spread over every link that works, a connection on each in turn (net-merge).
    Merge,
    /// One link only, by its interface name (`en0`). While it is not there, the system's way is taken.
    Link(String),
}

impl Target {
    pub fn id(&self) -> String {
        match self {
            Target::System => "system".into(),
            Target::Merge => "merge".into(),
            Target::Link(name) => format!("link:{name}"),
        }
    }

    /// The target of an id the page or the file gave; `None` for one that is not of that shape.
    pub fn from_id(id: &str) -> Option<Target> {
        match id {
            "system" => Some(Target::System),
            "merge" => Some(Target::Merge),
            _ => id
                .strip_prefix("link:")
                .filter(|name| valid_interface(name))
                .map(|name| Target::Link(name.to_owned())),
        }
    }
}

/// An interface name as the system gives them: letters then digits, short (`en0`, `en12`, `bridge100`).
pub fn valid_interface(name: &str) -> bool {
    let letters = name.bytes().take_while(u8::is_ascii_lowercase).count();
    (2..=16).contains(&name.len())
        && letters >= 2
        && name.len() > letters
        && name.bytes().skip(letters).all(|b| b.is_ascii_digit())
}

/// How much an app counts when a link is full. An app wired later to the same place starts below the ones before it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Priority {
    High,
    Normal,
    Low,
}

impl Priority {
    pub fn id(self) -> &'static str {
        match self {
            Priority::High => "high",
            Priority::Normal => "normal",
            Priority::Low => "low",
        }
    }

    pub fn from_id(id: &str) -> Option<Priority> {
        match id {
            "high" => Some(Priority::High),
            "normal" => Some(Priority::Normal),
            "low" => Some(Priority::Low),
            _ => None,
        }
    }

    pub fn index(self) -> usize {
        self as usize
    }
}

/// The wire of one app.
#[derive(Debug, Clone, PartialEq)]
pub struct Rule {
    /// The app, by the name of its bundle without `.app` (`Microsoft Edge`), or of its program when it has none.
    pub app: String,
    pub target: Target,
    pub priority: Priority,
    /// The most it may receive, in kilobytes a second; 0 is no limit.
    pub down_kbps: u32,
    /// The most it may send, in kilobytes a second; 0 is no limit.
    pub up_kbps: u32,
}

/// The most rules kept, and the highest limit a page may set (about 8 Gbit/s).
pub const MAX_RULES: usize = 64;
pub const MAX_KBPS: u32 = 1_000_000;

/// Whether an app name may stand in a rule: what a bundle or a program is called, without control characters.
pub fn valid_app(name: &str) -> bool {
    !name.trim().is_empty()
        && name.chars().count() <= 80
        && !name.chars().any(|c| c.is_control() || c == '/')
}

/// The rules as they apply: the app is the key, the first rule for an app wins, and at most `MAX_RULES` are kept.
pub fn sanitized(rules: Vec<Rule>) -> Vec<Rule> {
    let mut out: Vec<Rule> = Vec::new();
    for mut rule in rules {
        rule.app = rule.app.trim().to_owned();
        if !valid_app(&rule.app) || out.iter().any(|r| r.app.eq_ignore_ascii_case(&rule.app)) {
            continue;
        }
        rule.down_kbps = rule.down_kbps.min(MAX_KBPS);
        rule.up_kbps = rule.up_kbps.min(MAX_KBPS);
        out.push(rule);
        if out.len() == MAX_RULES {
            break;
        }
    }
    out
}

/// The rule of an app; without one an app takes the system's way, at normal priority, with no limit.
pub fn rule_for<'a>(rules: &'a [Rule], app: &str) -> Option<&'a Rule> {
    rules.iter().find(|r| r.app.eq_ignore_ascii_case(app))
}

/// The priority a new wire to `target` gets: the first app there is the main one, every app wired after it comes below.
pub fn priority_for_new_wire(rules: &[Rule], target: &Target) -> Priority {
    if rules.iter().any(|r| &r.target == target) {
        Priority::Low
    } else {
        Priority::High
    }
}

/// The most a second each priority may receive on a link that was seen to carry `peak` bytes a second, while the
/// priorities used `used` bytes a second each in the last second; `None` is no cap. An app is held back only by what
/// the apps above it use, so a link that is not full slows no one: the apps above get room for a little more than they
/// use, and those below keep a floor so that they never stop.
pub fn priority_caps(peak: f64, used: [f64; 3]) -> [Option<f64>; 3] {
    const ROOM: f64 = 1.25;
    const FLOOR: [f64; 3] = [1.0, 0.10, 0.05];
    // Below this nothing is measured well enough to hold anyone back (about 0.4 Mbit/s).
    if peak < 50_000.0 {
        return [None; 3];
    }
    let mut caps = [None; 3];
    let mut above = 0.0;
    for tier in 0..3 {
        if tier > 0 && above > 0.0 {
            caps[tier] = Some((peak - above * ROOM).max(peak * FLOOR[tier]));
        }
        above += used[tier];
    }
    caps
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rule(app: &str, target: Target) -> Rule {
        Rule {
            app: app.into(),
            target,
            priority: Priority::Normal,
            down_kbps: 0,
            up_kbps: 0,
        }
    }

    #[test]
    fn a_target_goes_to_its_id_and_back() {
        for t in [
            Target::System,
            Target::Merge,
            Target::Link("en0".into()),
            Target::Link("bridge100".into()),
        ] {
            assert_eq!(Target::from_id(&t.id()), Some(t));
        }
        for bad in [
            "",
            "link:",
            "link:EN0",
            "link:en",
            "link:0en",
            "link:en0;rm",
            "link:en0 ",
            "wifi",
        ] {
            assert_eq!(Target::from_id(bad), None, "{bad}");
        }
    }

    #[test]
    fn rules_are_one_for_each_app_within_the_limits() {
        let mut many = vec![
            Rule {
                down_kbps: u32::MAX,
                ..rule(" Safari ", Target::Merge)
            },
            rule("safari", Target::System),
            rule("", Target::System),
            rule("a/b", Target::System),
            rule("bad\u{7}", Target::System),
        ];
        let kept = sanitized(many.clone());
        assert_eq!(kept.len(), 1);
        assert_eq!(kept[0].app, "Safari");
        assert_eq!(
            kept[0].target,
            Target::Merge,
            "the first rule for an app wins"
        );
        assert_eq!(kept[0].down_kbps, MAX_KBPS);
        many.clear();
        for i in 0..100 {
            many.push(rule(&format!("app {i}"), Target::System));
        }
        assert_eq!(sanitized(many).len(), MAX_RULES);
        assert!(rule_for(&kept, "SAFARI").is_some() && rule_for(&kept, "Chrome").is_none());
    }

    #[test]
    fn the_first_wire_to_a_place_is_the_main_one_and_the_next_come_below() {
        let en0 = Target::Link("en0".into());
        assert_eq!(priority_for_new_wire(&[], &en0), Priority::High);
        let rules = [rule("Steam", en0.clone())];
        assert_eq!(priority_for_new_wire(&rules, &en0), Priority::Low);
        assert_eq!(
            priority_for_new_wire(&rules, &Target::Merge),
            Priority::High
        );
    }

    #[test]
    fn only_what_the_apps_above_use_holds_an_app_back() {
        let peak = 10_000_000.0;
        assert_eq!(
            priority_caps(peak, [0.0, 0.0, 9e6]),
            [None; 3],
            "alone on the link, a low app has it all"
        );
        let caps = priority_caps(peak, [2e6, 0.0, 8e6]);
        assert_eq!(caps[0], None, "the main app is never capped");
        assert_eq!(
            caps[2],
            Some(peak - 2e6 * 1.25),
            "the low app leaves the main one room to grow"
        );
        let caps = priority_caps(peak, [9e6, 1e6, 1e6]);
        assert_eq!(
            caps[1],
            Some(peak * 0.10),
            "a floor keeps the normal app going"
        );
        assert_eq!(caps[2], Some(peak * 0.05));
        assert_eq!(
            priority_caps(10_000.0, [5e3, 0.0, 5e3]),
            [None; 3],
            "a link not measured yet holds no one back"
        );
    }

    #[test]
    fn interface_names_are_what_the_system_gives() {
        assert!(valid_interface("en0") && valid_interface("en12") && valid_interface("bridge100"));
        assert!(!valid_interface("en") && !valid_interface("e0") && !valid_interface("en0a"));
    }
}
