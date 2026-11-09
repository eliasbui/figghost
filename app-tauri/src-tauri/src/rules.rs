//! The rules of the settings that do not need an engine: the kind of value a setting takes, how far it may go, the words it
//! may be, how a shortcut is written and when two of them are the same. They are the rules of
//! crates/app-ui/src/settings/model.rs and crates/app-ui/src/config.rs, written again here because those files belong to
//! the AppKit host of the current app and cannot be linked into this one; a rule that changes there changes here too.
use crate::store::Fault;
use core_window::Layout;
use serde_json::{json, Value};

/// The longest bundle identifier the page may send.
const MAX_ID_LEN: usize = 200;

// -- paths into the state

/// A dotted path as a JSON Pointer: "mouse.apps.0.id" is "/mouse/apps/0/id". Keys are lowercase letters, digits and
/// underscores, so nothing needs escaping.
fn pointer(path: &str) -> String {
    format!("/{}", path.replace('.', "/"))
}

pub fn get<'a>(root: &'a Value, path: &str) -> Option<&'a Value> {
    root.pointer(&pointer(path))
}

/// Replaces the value at a path that exists, and says whether it did.
pub fn put(root: &mut Value, path: &str, value: Value) -> bool {
    match root.pointer_mut(&pointer(path)) {
        Some(slot) => {
            *slot = value;
            true
        }
        None => false,
    }
}

/// A path the page may write: two sections or more of lowercase letters, digits and underscores, none of them only digits
/// (which would reach into a list), and never the facts the host fills in.
pub fn valid_path(path: &str) -> Result<(), Fault> {
    let word = |part: &&str| {
        !part.is_empty()
            && part
                .bytes()
                .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'_')
            && !part.bytes().all(|b| b.is_ascii_digit())
    };
    let parts: Vec<&str> = path.split('.').collect();
    if parts.len() >= 2 && parts.iter().all(word) && !matches!(parts[0], "env" | "app") {
        Ok(())
    } else {
        Err(Fault::new("unknown_setting", path))
    }
}

/// The lists that have messages of their own (apps, snippets, wires, ...): a plain `set` of them is refused.
pub const OWN_MESSAGES: [&str; 9] = [
    "mouse.apps",
    "clipboard.ignored",
    "tiling.layouts",
    "snippets.items",
    "network.wires",
    "network.layout",
    "network.known_links",
    "network.metered.marked",
    "network.metered.unmarked",
];

// -- the value of a setting

fn kind(v: &Value) -> u8 {
    match v {
        Value::Null => 0,
        Value::Bool(_) => 1,
        Value::Number(_) => 2,
        Value::String(_) => 3,
        Value::Array(_) => 4,
        Value::Object(_) => 5,
    }
}

/// A new value may take the place of the old one when it is of the same kind (or the old one says nothing yet).
fn compatible(old: &Value, new: &Value) -> bool {
    old.is_null() || kind(old) == kind(new)
}

/// Number settings: the least, the most and whether the value is made whole. A value outside the range is brought in.
fn limits(path: &str) -> Option<(f64, f64, bool)> {
    Some(match path {
        "mouse.step_px" => (1.0, 400.0, false),
        "mouse.speed" => (0.1, 10.0, false),
        "mouse.duration_ms" => (50.0, 2000.0, false),
        "mouse.boost" => (1.5, 20.0, false),
        "clipboard.history_limit" => (10.0, 100_000.0, true),
        "notch.hover_delay_ms" => (0.0, 1000.0, true),
        "agents.wait" => (5.0, 100.0, true),
        _ => return None,
    })
}

/// The sizes of the notch: 0 means automatic, any other value is brought into the range. The largest value accepted before
/// that, then the least and the most.
fn sizes(path: &str) -> Option<(f64, u64, u64)> {
    Some(match path {
        "notch.width" => (1000.0, 480, 1000),
        "notch.height" => (900.0, 260, 900),
        "notch.rest_width" => (400.0, 120, 400),
        "notch.rest_height" => (48.0, 20, 48),
        _ => return None,
    })
}

const THEME: &[&str] = &["auto", "light", "dark"];
const LANGUAGE: &[&str] = &["auto", "en", "vi"];
const MATERIAL: &[&str] = &["glass", "clear", "ink", "accent", "black"];
const HUD: &[&str] = &[
    "notch",
    "glass",
    "corner",
    "bar",
    "island",
    "side",
    "tile",
    "ring",
    "equalizer",
    "wide",
];
const HUD_POSITION: &[&str] = &[
    "auto",
    "top-left",
    "top",
    "top-right",
    "left",
    "center",
    "right",
    "bottom-left",
    "bottom",
    "bottom-right",
];
const MOTION: &[&str] = &["spring", "smooth", "none"];
const REST: &[&str] = &["notch", "hidden"];
const CUTOUT: &[&str] = &["auto", "yes", "no"];
const WAVEFORM: &[&str] = &["live", "animated"];

/// The settings that take one of a few words.
fn words(path: &str) -> Option<&'static [&'static str]> {
    match path {
        "general.theme" => Some(THEME),
        "general.language" => Some(LANGUAGE),
        "notch.material" => Some(MATERIAL),
        "notch.hud" => Some(HUD),
        "notch.hud_position" => Some(HUD_POSITION),
        "notch.motion" => Some(MOTION),
        "notch.rest" => Some(REST),
        "notch.cutout" => Some(CUTOUT),
        "notch.waveform" => Some(WAVEFORM),
        _ => None,
    }
}

/// The screen setting: a fixed word, or `screen:<name>`.
fn screen_choice(text: &str) -> bool {
    ["auto", "main", "pointer"].contains(&text)
        || text
            .strip_prefix("screen:")
            .is_some_and(|name| !name.is_empty())
}

/// The ids of the tabs of the notch that the state lists.
fn tab_ids(state: &Value) -> Vec<&str> {
    get(state, "notch.pages")
        .and_then(Value::as_array)
        .map(|tabs| tabs.iter().filter_map(|tab| tab["id"].as_str()).collect())
        .unwrap_or_default()
}

/// The value a setting takes when the page sends `value` for it, or why it cannot.
pub fn normalize(state: &Value, path: &str, value: &Value) -> Result<Value, Fault> {
    let current = get(state, path).ok_or_else(|| Fault::new("unknown_setting", path))?;
    let invalid = || Fault::new("invalid_value", path);
    if !compatible(current, value) {
        return Err(invalid());
    }
    let number = || value.as_f64().filter(|n| n.is_finite()).ok_or_else(invalid);
    if let Some((low, high, whole)) = limits(path) {
        let n = number()?.clamp(low, high);
        return Ok(if whole {
            json!(n.round() as u64)
        } else {
            json!(n)
        });
    }
    if let Some((top, least, most)) = sizes(path) {
        let n = number()?.clamp(0.0, top).round() as u64;
        return Ok(json!(if n == 0 { 0 } else { n.clamp(least, most) }));
    }
    let allowed = match path {
        "notch.open_page" => value
            .as_str()
            .is_some_and(|w| w == "last" || w == "activity" || tab_ids(state).contains(&w)),
        "notch.display" => value.as_str().is_some_and(screen_choice),
        _ => match words(path) {
            Some(list) => value.as_str().is_some_and(|w| list.contains(&w)),
            None => true,
        },
    };
    if allowed {
        Ok(value.clone())
    } else {
        Err(invalid())
    }
}

/// A bundle identifier, as the lists of apps keep it.
pub fn check_id(id: &str) -> Result<(), Fault> {
    let ok = !id.is_empty()
        && id.len() <= MAX_ID_LEN
        && id
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | '_'));
    if ok {
        Ok(())
    } else {
        Err(Fault::new(
            "invalid_app_id",
            id.chars().take(40).collect::<String>(),
        ))
    }
}

// -- shortcuts

pub const COMMAND: u8 = 1;
const SHIFT: u8 = 2;
const OPTION: u8 = 4;
const CONTROL: u8 = 8;

/// A shortcut as the system reads it: which modifiers, which key, whatever order and names it was written with.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Chord {
    pub modifiers: u8,
    key: String,
}

/// The name a key has in a shortcut: a letter, a digit, one of nine named keys, or F1 to F20.
fn key_name(name: &str) -> Option<String> {
    let name = match name {
        "enter" => "return",
        "escape" => "esc",
        "backspace" => "delete",
        other => other,
    };
    let single = name.len() == 1
        && name
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit());
    let named = [
        "space", "return", "tab", "esc", "delete", "left", "right", "down", "up",
    ]
    .contains(&name);
    let function = name
        .strip_prefix('f')
        .is_some_and(|n| (1..=20).any(|i| n == i.to_string()));
    (single || named || function).then(|| name.to_owned())
}

/// Reads "cmd+shift+c". The error is the code of the fault the page words: a shortcut needs a modifier, one key, and no
/// more than one key.
pub fn parse_shortcut(text: &str) -> Result<Chord, &'static str> {
    let mut modifiers: u8 = 0;
    let mut key: Option<String> = None;
    for part in text.split('+').map(|p| p.trim().to_lowercase()) {
        match part.as_str() {
            "cmd" | "command" => modifiers |= COMMAND,
            "shift" => modifiers |= SHIFT,
            "opt" | "option" | "alt" => modifiers |= OPTION,
            "ctrl" | "control" => modifiers |= CONTROL,
            other => {
                let name = key_name(other).ok_or("invalid_shortcut")?;
                if key.replace(name).is_some() {
                    return Err("invalid_shortcut");
                }
            }
        }
    }
    let key = key.ok_or("invalid_shortcut")?;
    if modifiers == 0 {
        return Err("needs_modifier");
    }
    Ok(Chord { modifiers, key })
}

/// A place where the state keeps a shortcut: the name the page calls it by, where it is, and what it holds.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Slot {
    pub target: String,
    pub path: String,
    pub value: String,
}

impl Slot {
    fn new(target: impl Into<String>, path: impl Into<String>, value: &str) -> Self {
        Self {
            target: target.into(),
            path: path.into(),
            value: value.to_owned(),
        }
    }
}

/// Every shortcut in the state. "clipboard" is clipboard.hotkey; "tiling:left" is a layout of the tiling list; "<feature>" is
/// <feature>.shortcut and "<feature>:<what>" is <feature>.shortcuts.<what>, except that the one shortcut of Breaks is
/// "breaks:now". The page reads them the same way (src/lib/shortcuts.ts).
pub fn slots(state: &Value) -> Vec<Slot> {
    let mut found = Vec::new();
    let Some(sections) = state.as_object() else {
        return found;
    };
    let text = |v: Option<&Value>| v.and_then(Value::as_str).unwrap_or_default().to_owned();
    for (name, section) in sections {
        match name.as_str() {
            "env" | "app" => {}
            "clipboard" => found.push(Slot::new(
                "clipboard",
                "clipboard.hotkey",
                &text(section.get("hotkey")),
            )),
            "tiling" => {
                for (i, layout) in section
                    .get("layouts")
                    .and_then(Value::as_array)
                    .into_iter()
                    .flatten()
                    .enumerate()
                {
                    found.push(Slot::new(
                        format!("tiling:{}", text(layout.get("key"))),
                        format!("tiling.layouts.{i}.shortcut"),
                        &text(layout.get("shortcut")),
                    ));
                }
            }
            _ => {
                if let Some(one) = section.get("shortcut").and_then(Value::as_str) {
                    let target = if name == "breaks" {
                        "breaks:now".to_owned()
                    } else {
                        name.clone()
                    };
                    found.push(Slot::new(target, format!("{name}.shortcut"), one));
                }
                for (what, v) in section
                    .get("shortcuts")
                    .and_then(Value::as_object)
                    .into_iter()
                    .flatten()
                {
                    found.push(Slot::new(
                        format!("{name}:{what}"),
                        format!("{name}.shortcuts.{what}"),
                        v.as_str().unwrap_or_default(),
                    ));
                }
            }
        }
    }
    found
}

// -- tiling

fn saved_shortcut<'a>(saved: Option<&'a Value>, key: &str) -> Option<&'a str> {
    let layout = saved?.as_array()?.iter().find(|l| l["key"] == key)?;
    layout["shortcut"]
        .as_str()
        .filter(|s| s.is_empty() || parse_shortcut(s).is_ok())
}

/// The layouts of window tiling with their shortcuts, as the page lists them. The layouts and their default shortcuts are
/// those of `core-window`, the same crate the current app places windows with; `saved` has what the person changed.
pub fn tiling_layouts(saved: Option<&Value>) -> Value {
    Value::Array(
        Layout::ALL
            .into_iter()
            .map(|layout| {
                let shortcut = saved_shortcut(saved, layout.key()).unwrap_or(layout.default_shortcut());
                json!({ "key": layout.key(), "shortcut": shortcut, "default": layout.default_shortcut() })
            })
            .collect(),
    )
}

// -- snippets

/// Line breaks as the pasteboard and the apps take them.
pub fn lf(text: &str) -> String {
    text.replace("\r\n", "\n").replace('\r', "\n")
}

/// The abbreviation of another snippet that is typed first and takes the place of this one, or "": an expansion happens at the
/// last key of an abbreviation, so one that holds another before its own last key is replaced before it can be finished.
pub fn shadowed_by<'a>(items: &'a [(String, String)], abbr: &str) -> &'a str {
    // Abbreviations are ASCII once they are checked, so cutting off the last byte cuts off the last character.
    let Some(head) = abbr.get(..abbr.len().saturating_sub(1)) else {
        return "";
    };
    items
        .iter()
        .map(|(other, _)| other.as_str())
        .find(|other| *other != abbr && head.contains(*other))
        .unwrap_or("")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn state() -> Value {
        json!({
            "mouse": { "smooth": true, "step_px": 36, "apps": [] },
            "notch": { "width": 0, "material": "glass", "display": "auto", "open_page": "activity", "pages": [{ "id": "clipboard" }, { "id": "media" }] },
            "general": { "theme": "auto" },
            "clipboard": { "hotkey": "cmd+shift+c" },
            "breaks": { "shortcut": "" },
            "keys": { "shortcuts": { "pin": "opt+p" } },
            "tiling": { "layouts": [{ "key": "left", "shortcut": "ctrl+opt+left", "default": "ctrl+opt+left" }] },
            "env": { "login": "disabled" },
        })
    }

    fn code(result: Result<Value, Fault>) -> &'static str {
        result.err().map_or("", |f| f.code)
    }

    #[test]
    fn a_number_is_brought_into_its_range_and_must_be_a_number() {
        let s = state();
        assert_eq!(
            normalize(&s, "mouse.step_px", &json!(5000)),
            Ok(json!(400.0))
        );
        assert_eq!(normalize(&s, "mouse.step_px", &json!(0.2)), Ok(json!(1.0)));
        assert_eq!(
            code(normalize(&s, "mouse.step_px", &json!("wide"))),
            "invalid_value"
        );
        assert_eq!(
            code(normalize(&s, "mouse.step_px", &json!(true))),
            "invalid_value"
        );
        assert_eq!(
            code(normalize(&s, "mouse.nothing", &json!(1))),
            "unknown_setting"
        );
    }

    #[test]
    fn a_size_of_the_notch_is_automatic_or_inside_its_range() {
        let s = state();
        assert_eq!(normalize(&s, "notch.width", &json!(0)), Ok(json!(0)));
        assert_eq!(normalize(&s, "notch.width", &json!(100)), Ok(json!(480)));
        assert_eq!(normalize(&s, "notch.width", &json!(700.4)), Ok(json!(700)));
        assert_eq!(normalize(&s, "notch.width", &json!(99999)), Ok(json!(1000)));
    }

    #[test]
    fn a_setting_of_words_takes_only_its_words() {
        let s = state();
        assert!(normalize(&s, "general.theme", &json!("dark")).is_ok());
        assert_eq!(
            code(normalize(&s, "general.theme", &json!("purple"))),
            "invalid_value"
        );
        assert!(normalize(&s, "notch.material", &json!("clear")).is_ok());
        assert_eq!(
            code(normalize(&s, "notch.material", &json!("plaid"))),
            "invalid_value"
        );
        assert_eq!(
            code(normalize(&s, "notch.hud", &json!("notch"))),
            "unknown_setting"
        );
        assert!(normalize(&s, "notch.display", &json!("screen:Sidecar")).is_ok());
        assert_eq!(
            code(normalize(&s, "notch.display", &json!("screen:"))),
            "invalid_value"
        );
        assert!(normalize(&s, "notch.open_page", &json!("media")).is_ok());
        assert!(normalize(&s, "notch.open_page", &json!("last")).is_ok());
        assert_eq!(
            code(normalize(&s, "notch.open_page", &json!("weather"))),
            "invalid_value"
        );
    }

    #[test]
    fn the_page_cannot_write_the_facts_of_the_host_or_reach_into_a_list() {
        for bad in [
            "env.login",
            "app.version",
            "mouse",
            "mouse..apps",
            "mouse.apps.0",
            "Mouse.smooth",
            "mouse.step-px",
        ] {
            assert_eq!(
                valid_path(bad).err().map(|f| f.code),
                Some("unknown_setting"),
                "{bad}"
            );
        }
        assert!(valid_path("mouse.step_px").is_ok());
        assert!(valid_path("wheel.files.hidden").is_ok());
    }

    #[test]
    fn a_shortcut_needs_a_modifier_and_one_key_and_is_read_whatever_its_order() {
        assert_eq!(
            parse_shortcut("cmd+shift+c"),
            parse_shortcut(" Shift + Command + C ")
        );
        assert_eq!(parse_shortcut("opt+enter"), parse_shortcut("alt+return"));
        assert_eq!(
            parse_shortcut("ctrl+opt+f20").map(|c| c.modifiers),
            Ok(CONTROL | OPTION)
        );
        assert_eq!(parse_shortcut("c"), Err("needs_modifier"));
        assert_eq!(parse_shortcut("f13"), Err("needs_modifier"));
        assert_eq!(parse_shortcut("cmd"), Err("invalid_shortcut"));
        assert_eq!(parse_shortcut("cmd+a+b"), Err("invalid_shortcut"));
        assert_eq!(parse_shortcut("cmd+f21"), Err("invalid_shortcut"));
        assert_eq!(parse_shortcut("cmd+f01"), Err("invalid_shortcut"));
        assert_eq!(parse_shortcut("hyper+c"), Err("invalid_shortcut"));
        assert_eq!(parse_shortcut(""), Err("invalid_shortcut"));
        assert_ne!(parse_shortcut("cmd+c"), parse_shortcut("cmd+shift+c"));
    }

    #[test]
    fn the_shortcuts_of_the_state_are_found_by_the_names_the_page_uses() {
        let found: Vec<(String, String, String)> = slots(&state())
            .into_iter()
            .map(|s| (s.target, s.path, s.value))
            .collect();
        let has = |t: &str, p: &str| {
            found
                .iter()
                .any(|(target, path, _)| target == t && path == p)
        };
        assert!(has("clipboard", "clipboard.hotkey"));
        assert!(has("breaks:now", "breaks.shortcut"));
        assert!(has("keys:pin", "keys.shortcuts.pin"));
        assert!(has("tiling:left", "tiling.layouts.0.shortcut"));
        assert!(!found.iter().any(|(target, _, _)| target.starts_with("env")));
    }

    #[test]
    fn the_layouts_and_their_defaults_are_those_of_the_tiling_crate() {
        let saved = json!([{ "key": "right", "shortcut": "", "default": "x" }, { "key": "top", "shortcut": "nonsense" }, { "key": "left", "shortcut": "cmd+9" }]);
        let layouts = tiling_layouts(Some(&saved));
        let list = layouts.as_array().unwrap();
        assert_eq!(list.len(), Layout::ALL.len());
        assert_eq!(list[0]["key"], "left");
        assert_eq!(list[0]["shortcut"], "cmd+9");
        assert_eq!(
            list[1]["shortcut"], "",
            "a shortcut the person removed stays removed"
        );
        assert_eq!(list[1]["default"], "ctrl+opt+right");
        assert_eq!(
            list[2]["shortcut"], "ctrl+opt+up",
            "a shortcut that cannot be read goes back to the default"
        );
        assert_eq!(tiling_layouts(None)[0]["shortcut"], "ctrl+opt+left");
    }

    #[test]
    fn a_snippet_that_holds_another_before_its_last_key_says_which() {
        let items: Vec<(String, String)> = [";s", ";sig", "sig", ";x", "ab", ";abc"]
            .iter()
            .map(|a| ((*a).to_owned(), String::new()))
            .collect();
        assert_eq!(shadowed_by(&items, ";s"), "");
        assert_eq!(shadowed_by(&items, ";sig"), ";s");
        assert_eq!(shadowed_by(&items, "sig"), "");
        assert_eq!(shadowed_by(&items, ";abc"), "ab");
        assert_eq!(lf("a\r\nb\rc"), "a\nb\nc");
    }

    #[test]
    fn a_bundle_identifier_is_letters_digits_and_a_few_marks() {
        assert!(check_id("com.apple.dt.Xcode").is_ok());
        assert_eq!(
            check_id("../etc").err().map(|f| f.code),
            Some("invalid_app_id")
        );
        assert_eq!(check_id("").err().map(|f| f.code), Some("invalid_app_id"));
        assert_eq!(
            check_id(&"a".repeat(201)).err().map(|f| f.code),
            Some("invalid_app_id")
        );
    }
}
