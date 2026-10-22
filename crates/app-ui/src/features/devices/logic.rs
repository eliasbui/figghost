//! What the Bluetooth devices that are connected say about themselves, as `system_profiler SPBluetoothDataType -json`
//! prints it: their names, what kind each is and the charge each one reports. Nothing here runs a command or draws.

use serde_json::Value;

/// How many devices are read; a longer list is not a Mac's.
pub const MAX_DEVICES: usize = 12;
const MAX_NAME_CHARS: usize = 60;
/// The levels of charge at and under which a device is called low, as the settings page offers them.
pub const LOW_AT: [u8; 4] = [10, 15, 20, 30];

/// What a device is, in the order the cards are laid out.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Kind {
    Headphones,
    Speaker,
    Keyboard,
    Mouse,
    Trackpad,
    Controller,
    Phone,
    Other,
}

/// A connected device. Each charge is a percentage the device itself reports, if it does.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Gadget {
    pub name: String,
    pub kind: Kind,
    /// The device as a whole.
    pub main: Option<u8>,
    /// A pair of earbuds, each bud and the case.
    pub left: Option<u8>,
    pub right: Option<u8>,
    pub case: Option<u8>,
}

impl Gadget {
    /// The lowest charge among the parts that run out in use: the device, and each earbud. A case is not counted.
    pub fn lowest(&self) -> Option<u8> {
        [self.main, self.left, self.right]
            .into_iter()
            .flatten()
            .min()
    }

    pub fn reports_a_charge(&self) -> bool {
        [self.main, self.left, self.right, self.case]
            .iter()
            .any(Option::is_some)
    }
}

/// What was printed is not what was asked for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Unreadable;

/// How long the charge a device has stopped reporting is still shown, in milliseconds.
pub const KEEP_MS: i64 = 2 * 3600 * 1000;
/// How many devices are remembered; a longer list is not a Mac's.
const MAX_MEMORY: usize = 50;

/// A device as shown: what it reports now, or if it reports nothing, what it last reported.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Seen {
    pub gadget: Gadget,
    /// When the charge shown was reported, if it is not one from the look just made.
    pub since: Option<i64>,
}

/// What a device last said about its charge. Some say it only now and then: a headset may report it when it connects
/// and not again for an hour, and a card that came and went between "100%" and "does not report" would read as broken.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Memory {
    name: String,
    kind: Kind,
    charge: [Option<u8>; 4],
    at: i64,
}

impl Gadget {
    fn charge(&self) -> [Option<u8>; 4] {
        [self.main, self.left, self.right, self.case]
    }
}

/// The devices of a look with what they said before filled in for those that now say nothing, for no longer than
/// `KEEP_MS`. A device that reports anything is shown as it reports and remembered as that; `memory` is brought up to date
/// and loses what is too old.
pub fn with_memory(gadgets: Vec<Gadget>, memory: &mut Vec<Memory>, now: i64) -> Vec<Seen> {
    memory.retain(|m| now - m.at <= KEEP_MS);
    let seen = gadgets
        .into_iter()
        .map(|gadget| {
            let at = memory
                .iter()
                .position(|m| m.name == gadget.name && m.kind == gadget.kind);
            if gadget.reports_a_charge() {
                let fresh = Memory {
                    name: gadget.name.clone(),
                    kind: gadget.kind,
                    charge: gadget.charge(),
                    at: now,
                };
                match at {
                    Some(i) => memory[i] = fresh,
                    None => memory.push(fresh),
                }
                return Seen {
                    gadget,
                    since: None,
                };
            }
            match at.map(|i| &memory[i]) {
                Some(old) => {
                    let [main, left, right, case] = old.charge;
                    Seen {
                        gadget: Gadget {
                            main,
                            left,
                            right,
                            case,
                            ..gadget
                        },
                        since: Some(old.at),
                    }
                }
                None => Seen {
                    gadget,
                    since: None,
                },
            }
        })
        .collect();
    // The oldest go first when there are too many.
    memory.sort_by_key(|m| std::cmp::Reverse(m.at));
    memory.truncate(MAX_MEMORY);
    seen
}

/// A charge: "85%" or a bare number, from 0 to 100. Anything else is no charge.
fn percent(value: &Value) -> Option<u8> {
    let number = match value {
        Value::String(text) => {
            let digits = text.trim().strip_suffix('%').unwrap_or(text.trim()).trim();
            if digits.is_empty() || digits.len() > 3 || !digits.bytes().all(|b| b.is_ascii_digit())
            {
                return None;
            }
            digits.parse::<u64>().ok()?
        }
        Value::Number(number) => number.as_u64()?,
        _ => return None,
    };
    u8::try_from(number).ok().filter(|n| *n <= 100)
}

/// A name as one clean line: no control characters, runs of spaces made one, cut on a character boundary.
fn clean_name(name: &str) -> String {
    let single: String = name
        .chars()
        .map(|c| {
            if crate::features::is_hidden_char(c) {
                ' '
            } else {
                c
            }
        })
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ");
    single.chars().take(MAX_NAME_CHARS).collect()
}

fn kind_of(minor: &str, name: &str) -> Kind {
    let (minor, name) = (minor.to_lowercase(), name.to_lowercase());
    let says = |word: &str| minor.contains(word) || name.contains(word);
    if says("headset") || says("headphone") || says("airpods") || says("earbuds") || says("buds") {
        Kind::Headphones
    } else if says("speaker") || says("soundbar") {
        Kind::Speaker
    } else if says("keyboard") {
        Kind::Keyboard
    } else if says("trackpad") {
        Kind::Trackpad
    } else if says("mouse") {
        Kind::Mouse
    } else if says("gamepad") || says("controller") || says("joystick") {
        Kind::Controller
    } else if says("phone") {
        Kind::Phone
    } else {
        Kind::Other
    }
}

fn gadget(name: &str, props: &Value) -> Option<Gadget> {
    let name = clean_name(name);
    let props = props.as_object()?;
    if name.is_empty() {
        return None;
    }
    let get = |key: &str| props.get(key).and_then(percent);
    let minor = props
        .get("device_minorType")
        .and_then(Value::as_str)
        .unwrap_or_default();
    Some(Gadget {
        kind: kind_of(minor, &name),
        main: get("device_batteryLevelMain"),
        left: get("device_batteryLevelLeft"),
        right: get("device_batteryLevelRight"),
        case: get("device_batteryLevelCase"),
        name,
    })
}

/// The devices that are connected, by kind and then by name. A device that is only paired is left out, whatever it once
/// reported. An answer that is not what the command prints is an error; one with no device connected is an empty list.
pub fn parse(text: &str) -> Result<Vec<Gadget>, Unreadable> {
    let root: Value = serde_json::from_str(text).map_err(|_| Unreadable)?;
    let sections = root
        .get("SPBluetoothDataType")
        .and_then(Value::as_array)
        .ok_or(Unreadable)?;
    let mut found: Vec<Gadget> = sections
        .iter()
        .filter_map(|section| section.get("device_connected")?.as_array())
        .flatten()
        .filter_map(Value::as_object)
        .flatten()
        .filter_map(|(name, props)| gadget(name, props))
        .collect();
    found.sort_by_key(|g| (g.kind, g.name.to_lowercase()));
    found.truncate(MAX_DEVICES);
    Ok(found)
}

/// A charge the power system has for an accessory, as `pmset -g accps -xml` prints it. Many devices that tell
/// `system_profiler` nothing, a mouse or a headset, tell this, and it is what the menu bar shows.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Accessory {
    pub name: String,
    pub kind: Kind,
    pub charge: u8,
}

/// The text of a plist element, with the five entities and numeric ones undone.
fn unescape(text: &str) -> String {
    let mut out = String::new();
    let mut rest = text;
    while let Some(at) = rest.find('&') {
        out.push_str(&rest[..at]);
        rest = &rest[at..];
        let Some(end) = rest.find(';').filter(|e| *e <= 10) else {
            break;
        };
        let decoded = match &rest[1..end] {
            "amp" => Some('&'),
            "lt" => Some('<'),
            "gt" => Some('>'),
            "quot" => Some('"'),
            "apos" => Some('\''),
            code => code
                .strip_prefix("#x")
                .map(|hex| u32::from_str_radix(hex, 16).ok())
                .unwrap_or_else(|| code.strip_prefix('#').and_then(|d| d.parse().ok()))
                .and_then(char::from_u32),
        };
        match decoded {
            Some(c) => {
                out.push(c);
                rest = &rest[end + 1..];
            }
            None => break,
        }
    }
    out.push_str(rest);
    out
}

/// Each `<dict>` of what `pmset` printed, as its keys with the text of the value after each.
fn plist_dicts(text: &str) -> Vec<Vec<(String, String)>> {
    let element = |s: &str, tag: &str| -> Option<(String, usize)> {
        let open = format!("<{tag}>");
        let close = format!("</{tag}>");
        let rest = s.strip_prefix(&open)?;
        let end = rest.find(&close)?;
        Some((unescape(&rest[..end]), open.len() + end + close.len()))
    };
    let mut dicts = Vec::new();
    for block in text.split("<dict>").skip(1) {
        let body = block.split("</dict>").next().unwrap_or_default();
        let mut pairs = Vec::new();
        let mut rest = body.trim_start();
        while let Some((key, used)) = element(rest, "key") {
            rest = rest[used..].trim_start();
            let value = ["string", "integer"]
                .iter()
                .find_map(|tag| element(rest, tag));
            if let Some((value, used)) = value {
                rest = rest[used..].trim_start();
                pairs.push((key, value));
            } else if let Some(next) = rest.find("<key>") {
                rest = &rest[next..];
            } else {
                break;
            }
        }
        dicts.push(pairs);
    }
    dicts
}

/// The accessories `pmset -g accps -xml` reports a charge for. What cannot be read is left out.
pub fn parse_accessories(text: &str) -> Vec<Accessory> {
    plist_dicts(text)
        .into_iter()
        .filter_map(|pairs| {
            let get = |key: &str| pairs.iter().find(|(k, _)| k == key).map(|(_, v)| v.trim());
            let current: u64 = get("Current Capacity")?.parse().ok()?;
            let max: u64 = get("Max Capacity")
                .and_then(|m| m.parse().ok())
                .unwrap_or(100);
            let charge = u8::try_from(current.checked_mul(100)? / max.max(1))
                .ok()
                .filter(|c| *c <= 100)?;
            let name = clean_name(get("Name").unwrap_or_default());
            Some(Accessory {
                kind: kind_of(get("Accessory Category").unwrap_or_default(), &name),
                name,
                charge,
            })
        })
        .take(MAX_DEVICES)
        .collect()
}

/// Gives each device that reports no charge of its own the one the power system has for it: the accessory of the same
/// name, or, when one accessory with no name of a device is left and one device that could be it has no charge, that one.
/// A headset can be listed with no name at all.
pub fn add_charges(gadgets: &mut [Gadget], accessories: &[Accessory]) {
    let mut left: Vec<&Accessory> = Vec::new();
    for accessory in accessories {
        let same = gadgets.iter_mut().find(|g| {
            !g.reports_a_charge()
                && !accessory.name.is_empty()
                && g.name.to_lowercase() == accessory.name.to_lowercase()
        });
        match same {
            Some(g) => g.main = Some(accessory.charge),
            None => left.push(accessory),
        }
    }
    let [only] = left[..] else { return };
    let named = |g: &Gadget| {
        accessories
            .iter()
            .any(|a| a.name.to_lowercase() == g.name.to_lowercase())
    };
    let mut could = gadgets.iter_mut().filter(|g| {
        !g.reports_a_charge() && !named(g) && (only.kind == Kind::Other || g.kind == only.kind)
    });
    if let (Some(g), None) = (could.next(), could.next()) {
        if only.name.is_empty() || only.kind != Kind::Other {
            g.main = Some(only.charge);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn entry(category: &str, name: &str, current: u32) -> String {
        format!(
            "<dict><key>Accessory Category</key><string>{category}</string><key>Current Capacity</key>\
             <integer>{current}</integer><key>Max Capacity</key><integer>100</integer><key>Name</key>\
             <string>{name}</string><key>Type</key><string>Accessory Source</string></dict>"
        )
    }

    #[test]
    fn the_power_system_gives_a_charge_to_a_device_that_reports_none_itself() {
        let text = format!(
            "<?xml version=\"1.0\"?><plist version=\"1.0\">{}</plist><plist>{}</plist>",
            entry("Mouse", "BT5.0 Mouse", 68),
            "<dict><key>Current Capacity</key><integer>89</integer><key>Name</key><string></string></dict>"
        );
        let accessories = parse_accessories(&text);
        assert_eq!(
            accessories,
            [
                Accessory {
                    name: "BT5.0 Mouse".into(),
                    kind: Kind::Mouse,
                    charge: 68
                },
                Accessory {
                    name: String::new(),
                    kind: Kind::Other,
                    charge: 89
                },
            ]
        );
        let quiet = |name: &str, kind| Gadget {
            name: name.into(),
            kind,
            main: None,
            left: None,
            right: None,
            case: None,
        };
        let mut gadgets = vec![
            quiet("Buds Core của Bui", Kind::Headphones),
            quiet("BT5.0 Mouse", Kind::Mouse),
        ];
        add_charges(&mut gadgets, &accessories);
        assert_eq!((gadgets[0].main, gadgets[1].main), (Some(89), Some(68)));

        let mut two = vec![quiet("A", Kind::Headphones), quiet("B", Kind::Headphones)];
        add_charges(&mut two, &accessories[1..]);
        assert!(
            two.iter().all(|g| g.main.is_none()),
            "a charge with no name is not guessed between two"
        );

        let mut reported = vec![Gadget {
            main: Some(40),
            ..quiet("BT5.0 Mouse", Kind::Mouse)
        }];
        add_charges(&mut reported, &accessories);
        assert_eq!(
            reported[0].main,
            Some(40),
            "what a device reports itself is kept"
        );

        let mut other = vec![quiet("Keys", Kind::Keyboard)];
        add_charges(
            &mut other,
            &parse_accessories(&entry("Mouse", "Some Mouse", 50)),
        );
        assert_eq!(
            other[0].main, None,
            "a named accessory of another kind is not given away"
        );

        assert_eq!(
            parse_accessories(&entry("Mouse", "A &amp; B", 7))[0].name,
            "A & B"
        );
        for bad in [
            "",
            "nonsense",
            &entry("Mouse", "X", 101),
            "<dict><key>Name</key><string>X</string></dict>",
        ] {
            assert!(parse_accessories(bad).is_empty(), "{bad}");
        }
    }

    /// A real answer of this Mac, with the names and addresses replaced; the devices that are only paired were given a
    /// charge, to be sure it is not read.
    const REAL: &str = include_str!("testdata/profiler.json");

    fn names(found: &[Gadget]) -> Vec<&str> {
        found.iter().map(|g| g.name.as_str()).collect()
    }

    #[test]
    fn a_real_answer_gives_the_devices_that_are_connected_and_no_other() {
        let found = parse(REAL).unwrap();
        assert_eq!(names(&found), ["Sample Headset 2", "Sample Mouse 1"]);
        let (headset, mouse) = (&found[0], &found[1]);
        assert_eq!((headset.kind, headset.main), (Kind::Headphones, Some(100)));
        assert_eq!(
            (mouse.kind, mouse.main),
            (Kind::Mouse, None),
            "a mouse that says nothing about its charge has none"
        );
        assert!(headset.reports_a_charge() && !mouse.reports_a_charge());
    }

    #[test]
    fn a_charge_is_a_whole_percentage_from_text_or_a_number() {
        for (value, want) in [
            (json!("85%"), Some(85)),
            (json!(" 7% "), Some(7)),
            (json!("100"), Some(100)),
            (json!("0%"), Some(0)),
            (json!("50 %"), Some(50)),
            (json!(64), Some(64)),
            (json!("101%"), None),
            (json!("-5%"), None),
            (json!("5%%"), None),
            (json!("1e2%"), None),
            (json!("12.5%"), None),
            (json!("５０%"), None),
            (json!("9999%"), None),
            (json!(""), None),
            (json!("%"), None),
            (json!("abc"), None),
            (json!(101), None),
            (json!(-1), None),
            (json!(7.5), None),
            (json!(null), None),
            (json!(true), None),
            (json!([85]), None),
        ] {
            assert_eq!(percent(&value), want, "{value}");
        }
    }

    #[test]
    fn earbuds_report_each_bud_and_the_case_and_the_case_does_not_make_them_low() {
        let text = json!({ "SPBluetoothDataType": [{ "device_connected": [
            { "Sample Earbuds": {
                "device_batteryLevelLeft": "82%", "device_batteryLevelRight": "76%", "device_batteryLevelCase": "4%",
                "device_minorType": "Headphones" } }
        ]}]})
        .to_string();
        let found = parse(&text).unwrap();
        let buds = &found[0];
        assert_eq!(
            (buds.main, buds.left, buds.right, buds.case),
            (None, Some(82), Some(76), Some(4))
        );
        assert_eq!(buds.lowest(), Some(76), "the case is not counted");
        assert!(buds.reports_a_charge());
        let only_case = Gadget {
            left: None,
            right: None,
            case: Some(3),
            ..buds.clone()
        };
        assert_eq!(only_case.lowest(), None);
        assert!(only_case.reports_a_charge());
    }

    #[test]
    fn what_a_device_is_comes_from_its_type_and_then_from_its_name() {
        for (minor, name, want) in [
            ("Headset", "X", Kind::Headphones),
            ("Headphones", "X", Kind::Headphones),
            ("", "Someone's AirPods Pro", Kind::Headphones),
            ("Speaker", "X", Kind::Speaker),
            ("Keyboard", "X", Kind::Keyboard),
            ("", "Magic Keyboard", Kind::Keyboard),
            ("Mouse", "X", Kind::Mouse),
            ("", "BT5.0 Mouse", Kind::Mouse),
            ("", "Magic Trackpad", Kind::Trackpad),
            ("Gamepad", "X", Kind::Controller),
            ("", "Wireless Controller", Kind::Controller),
            ("Phone", "X", Kind::Phone),
            ("", "Thing", Kind::Other),
        ] {
            assert_eq!(kind_of(minor, name), want, "{minor} {name}");
        }
    }

    #[test]
    fn a_name_is_one_clean_line_that_is_cut_on_a_character() {
        assert_eq!(clean_name("  Sam's \t\n Mouse  "), "Sam's Mouse");
        assert_eq!(clean_name("a\u{7}b\u{1b}[31mc"), "a b [31mc");
        assert_eq!(clean_name("   "), "");
        assert_eq!(
            clean_name("Pay\u{202E}fdp.exe"),
            "Pay fdp.exe",
            "a character that changes the direction of text is not kept"
        );
        let long = "ệ".repeat(100);
        assert_eq!(clean_name(&long).chars().count(), MAX_NAME_CHARS);
        let text = json!({ "SPBluetoothDataType": [{ "device_connected": [
            { " ": { "device_minorType": "Mouse" } }, { "Real": { "device_minorType": "Mouse" } }
        ]}]})
        .to_string();
        assert_eq!(
            names(&parse(&text).unwrap()),
            ["Real"],
            "a device with no name is left out"
        );
    }

    #[test]
    fn the_cards_are_ordered_by_kind_then_name_and_no_more_than_the_limit_are_read() {
        let device = |name: &str, minor: &str| json!({ name: { "device_minorType": minor } });
        let text = json!({ "SPBluetoothDataType": [{ "device_connected": [
            device("Zed Mouse", "Mouse"), device("b Keys", "Keyboard"), device("Alpha Mouse", "Mouse"),
            device("a Keys", "Keyboard"), device("Cans", "Headset"), device("Radio", "")
        ]}]})
        .to_string();
        assert_eq!(
            names(&parse(&text).unwrap()),
            [
                "Cans",
                "a Keys",
                "b Keys",
                "Alpha Mouse",
                "Zed Mouse",
                "Radio"
            ]
        );
        let many: Vec<Value> = (0..40)
            .map(|i| device(&format!("Mouse {i:02}"), "Mouse"))
            .collect();
        let text = json!({ "SPBluetoothDataType": [{ "device_connected": many }] }).to_string();
        assert_eq!(parse(&text).unwrap().len(), MAX_DEVICES);
    }

    fn quiet(name: &str, kind: Kind) -> Gadget {
        Gadget {
            name: name.into(),
            kind,
            main: None,
            left: None,
            right: None,
            case: None,
        }
    }

    fn loud(name: &str, kind: Kind, main: u8) -> Gadget {
        Gadget {
            main: Some(main),
            ..quiet(name, kind)
        }
    }

    #[test]
    fn a_charge_a_device_has_stopped_reporting_is_shown_for_a_while_with_the_time_it_was_reported()
    {
        let mut memory = Vec::new();
        let first = with_memory(
            vec![
                loud("Cans", Kind::Headphones, 100),
                quiet("Mouse", Kind::Mouse),
            ],
            &mut memory,
            1_000,
        );
        assert_eq!(first[0].since, None, "what a device reports now is fresh");
        assert_eq!(
            first[1],
            Seen {
                gadget: quiet("Mouse", Kind::Mouse),
                since: None
            },
            "a device that never reported has nothing to show"
        );
        let later = with_memory(
            vec![quiet("Cans", Kind::Headphones), quiet("Mouse", Kind::Mouse)],
            &mut memory,
            1_000 + 10 * 60_000,
        );
        assert_eq!(later[0].gadget.main, Some(100), "it keeps what it said");
        assert_eq!(later[0].since, Some(1_000), "and says when it said it");
        assert_eq!(later[1].gadget.main, None);
        let new = with_memory(
            vec![loud("Cans", Kind::Headphones, 80)],
            &mut memory,
            2_000_000,
        );
        assert_eq!(
            (new[0].gadget.main, new[0].since),
            (Some(80), None),
            "a new report replaces the old one"
        );
        let after = with_memory(
            vec![quiet("Cans", Kind::Headphones)],
            &mut memory,
            2_000_000 + 60_000,
        );
        assert_eq!(
            (after[0].gadget.main, after[0].since),
            (Some(80), Some(2_000_000))
        );
    }

    #[test]
    fn a_charge_that_is_too_old_is_forgotten_and_a_different_device_does_not_inherit_one() {
        let mut memory = Vec::new();
        with_memory(vec![loud("Cans", Kind::Headphones, 60)], &mut memory, 0);
        let edge = with_memory(vec![quiet("Cans", Kind::Headphones)], &mut memory, KEEP_MS);
        assert_eq!(
            edge[0].gadget.main,
            Some(60),
            "right at the limit it is still shown"
        );
        let past = with_memory(
            vec![quiet("Cans", Kind::Headphones)],
            &mut memory,
            KEEP_MS + 1,
        );
        assert_eq!(
            (past[0].gadget.main, past[0].since),
            (None, None),
            "past it, it is forgotten"
        );
        assert!(memory.is_empty(), "and no longer kept");
        let mut memory = Vec::new();
        with_memory(vec![loud("Cans", Kind::Headphones, 60)], &mut memory, 0);
        let other = with_memory(
            vec![
                quiet("Cans", Kind::Speaker),
                quiet("Other", Kind::Headphones),
            ],
            &mut memory,
            1,
        );
        assert!(
            other
                .iter()
                .all(|s| s.gadget.main.is_none() && s.since.is_none()),
            "another name or kind has nothing"
        );
    }

    #[test]
    fn a_pair_of_earbuds_is_remembered_as_a_whole_and_the_memory_is_bounded() {
        let mut memory = Vec::new();
        let buds = Gadget {
            left: Some(82),
            right: Some(76),
            case: Some(54),
            ..quiet("Buds", Kind::Headphones)
        };
        with_memory(vec![buds.clone()], &mut memory, 5);
        let seen = with_memory(vec![quiet("Buds", Kind::Headphones)], &mut memory, 6);
        assert_eq!(seen[0].gadget, buds);
        assert_eq!(seen[0].since, Some(5));
        let many: Vec<Gadget> = (0..80)
            .map(|i| loud(&format!("Device {i}"), Kind::Other, 50))
            .collect();
        with_memory(many, &mut memory, 7);
        assert!(memory.len() <= MAX_MEMORY);
    }

    #[test]
    fn an_answer_that_is_not_what_was_asked_for_is_unreadable_and_odd_shapes_are_skipped() {
        for text in [
            "",
            "not json",
            "[]",
            "{}",
            r#"{"SPBluetoothDataType": 5}"#,
            r#"{"SPBluetoothDataType": {}}"#,
        ] {
            assert_eq!(parse(text), Err(Unreadable), "{text:?}");
        }
        for text in [
            r#"{"SPBluetoothDataType": []}"#,
            r#"{"SPBluetoothDataType": [{}]}"#,
            r#"{"SPBluetoothDataType": [5, null, "x"]}"#,
            r#"{"SPBluetoothDataType": [{"device_connected": 5}]}"#,
            r#"{"SPBluetoothDataType": [{"device_connected": [5, null, [], "x"]}]}"#,
            r#"{"SPBluetoothDataType": [{"device_connected": [{"Name": 5}]}]}"#,
        ] {
            assert_eq!(parse(text), Ok(Vec::new()), "{text}");
        }
    }
}
