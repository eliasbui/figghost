//! This feature's part of the settings model: the JSON the page renders and the changes it may make.
//! Pure data with no AppKit, so every rule is covered by plain unit tests.

use super::{
    Config, Key,
    catalog::{self, Kind},
};
use crate::settings::model::Fault;
use serde_json::{Value, json};

const WHEELS: [(&str, Kind); 3] = [
    ("formats", Kind::Formats),
    ("tools", Kind::Tools),
    ("actions", Kind::Actions),
];

/// The state under the key "wheel": the switch of the feature, and for each wheel its switch, its key and the
/// slots hidden, as they will be used (an unknown key name shows as the wheel's own key). `runtime` is
/// `Feature::runtime()`, or null when the feature is not running. `web/features/wheel.fixture.json` must have
/// exactly the same keys; a unit test checks it.
pub fn state(config: &Config, _runtime: &Value) -> Value {
    let wheel = |kind: Kind| {
        json!({
            "enabled": config.wheel(kind).enabled,
            "key": config.key_of(kind).as_str(),
            "hidden": config.hidden_ids(kind),
        })
    };
    json!({
        "enabled": config.enabled,
        "formats": wheel(Kind::Formats),
        "tools": wheel(Kind::Tools),
        "actions": wheel(Kind::Actions),
    })
}

fn invalid(what: &'static str) -> Fault {
    Fault::new("invalid_value", what)
}

/// Whether another wheel that is on uses `key`.
fn taken_by_other(config: &Config, kind: Kind, key: Key) -> bool {
    Kind::ALL
        .into_iter()
        .any(|other| other != kind && config.wheel(other).enabled && config.key_of(other) == key)
}

fn taken(key: Key) -> Fault {
    Fault::new(
        "wheel_key_taken",
        format!("{} is used by another wheel", key.as_str()),
    )
}

/// A change from the page. `path` is relative to the feature: "enabled" or "tools.key", not "wheel.tools.key".
/// `None` when the path is not one of this feature's. Values must be validated here; the page is not trusted,
/// and a refused change changes nothing.
pub fn set(config: &mut Config, path: &str, value: &Value) -> Option<Result<(), Fault>> {
    if path == "enabled" {
        return Some(
            value
                .as_bool()
                .map(|on| config.enabled = on)
                .ok_or_else(|| invalid("expected true or false")),
        );
    }
    let (name, field) = path.split_once('.')?;
    let kind = WHEELS.iter().find(|(n, _)| *n == name)?.1;
    Some(match field {
        "enabled" => set_enabled(config, kind, value),
        "key" => set_key(config, kind, value),
        "hidden" => set_hidden(config, kind, value),
        _ => return None,
    })
}

fn set_enabled(config: &mut Config, kind: Kind, value: &Value) -> Result<(), Fault> {
    let on = value
        .as_bool()
        .ok_or_else(|| invalid("expected true or false"))?;
    let key = config.key_of(kind);
    if on && taken_by_other(config, kind, key) {
        return Err(taken(key));
    }
    set_wheel(config, kind, |w| w.enabled = on);
    Ok(())
}

fn set_key(config: &mut Config, kind: Kind, value: &Value) -> Result<(), Fault> {
    let key = value
        .as_str()
        .and_then(Key::parse)
        .ok_or_else(|| invalid("expected shift, command, control or option"))?;
    // A wheel that is off may take any key: the conflict is checked when it is turned on.
    if config.wheel(kind).enabled && taken_by_other(config, kind, key) {
        return Err(taken(key));
    }
    set_wheel(config, kind, |w| w.key = key.as_str().to_owned());
    Ok(())
}

/// Every id must be one the wheel has, none twice. The page builds the list from checkboxes, so anything else is
/// a bug or a hostile message, and is refused whole.
fn set_hidden(config: &mut Config, kind: Kind, value: &Value) -> Result<(), Fault> {
    let bad = || invalid("expected a list of slot names");
    let ids: Vec<&str> = value
        .as_array()
        .ok_or_else(bad)?
        .iter()
        .map(|v| v.as_str().ok_or_else(bad))
        .collect::<Result<_, _>>()?;
    let known = catalog::all_ids(kind);
    let mut unique = ids.clone();
    unique.sort_unstable();
    unique.dedup();
    if unique.len() != ids.len() || ids.iter().any(|id| !known.iter().any(|(_, k)| k == id)) {
        return Err(Fault::new(
            "invalid_value",
            format!("{} slot names", ids.len()),
        ));
    }
    // Stored in the order of the page, whatever order they came in.
    let mut ordered: Vec<String> = Vec::new();
    for (_, id) in known {
        if ids.contains(&id) && !ordered.iter().any(|o| o == id) {
            ordered.push(id.to_owned());
        }
    }
    set_wheel(config, kind, |w| w.hidden = ordered);
    Ok(())
}

fn set_wheel(config: &mut Config, kind: Kind, change: impl FnOnce(&mut super::WheelConfig)) {
    change(match kind {
        Kind::Formats => &mut config.formats,
        Kind::Tools => &mut config.tools,
        Kind::Actions => &mut config.actions,
    });
}

/// This feature's shortcuts as (target, shortcut). It has none: the keys that open the wheels are chosen on
/// the page and are single modifier taps, not shortcuts.
pub fn shortcuts(_config: &Config) -> Vec<(String, String)> {
    Vec::new()
}

/// Stores a shortcut that was already parsed and checked for conflicts. It has none.
pub fn set_shortcut(
    _config: &mut Config,
    _target: &str,
    _value: &str,
) -> Option<Result<(), Fault>> {
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run(config: &mut Config, path: &str, value: Value) -> Result<(), Fault> {
        set(config, path, &value).unwrap_or_else(|| panic!("{path} is not a setting"))
    }

    #[test]
    fn the_feature_switch_is_validated() {
        let mut config = Config::default();
        assert!(!config.enabled, "the wheel is off until it is switched on");
        assert!(run(&mut config, "enabled", json!(true)).is_ok() && config.enabled);
        for bad in [json!("yes"), json!(1), json!(null), json!([true])] {
            assert!(run(&mut config, "enabled", bad.clone()).is_err(), "{bad}");
            assert!(config.enabled, "a refused value changes nothing");
        }
        assert!(set(&mut config, "nonsense", &json!(1)).is_none());
        assert!(set(&mut config, "tools.nonsense", &json!(1)).is_none());
        assert!(set(&mut config, "bogus.enabled", &json!(true)).is_none());
        assert!(set(&mut config, "tools.enabled.more", &json!(true)).is_none());
    }

    #[test]
    fn a_wheel_is_switched_and_given_a_key() {
        let mut config = Config::default();
        assert!(run(&mut config, "tools.enabled", json!(false)).is_ok());
        assert!(!config.tools.enabled);
        // With the tools off, their key is free for the formats.
        assert!(run(&mut config, "formats.key", json!("command")).is_ok());
        assert_eq!(config.formats.key, "command");
        for bad in [
            json!("Shift"),
            json!("fn"),
            json!(""),
            json!(3),
            json!(null),
        ] {
            let before = config.clone();
            assert_eq!(
                run(&mut config, "formats.key", bad.clone())
                    .unwrap_err()
                    .code,
                "invalid_value"
            );
            assert_eq!(config, before, "{bad}");
        }
        assert_eq!(
            run(&mut config, "tools.enabled", json!("yes"))
                .unwrap_err()
                .code,
            "invalid_value"
        );
    }

    #[test]
    fn a_key_that_another_wheel_uses_is_refused_whole() {
        let mut config = Config::default();
        let before = config.clone();
        // Formats on shift, tools on command, actions on control: none of the others is free.
        for (path, key) in [
            ("formats.key", "command"),
            ("tools.key", "control"),
            ("actions.key", "shift"),
        ] {
            let fault = run(&mut config, path, json!(key)).unwrap_err();
            assert_eq!(fault.code, "wheel_key_taken", "{path}");
            assert_eq!(config, before, "{path}");
        }
        // The key a wheel already has is not taken from itself.
        assert!(run(&mut config, "formats.key", json!("shift")).is_ok());
        // Option is free.
        assert!(run(&mut config, "formats.key", json!("option")).is_ok());
        // And now shift is free for the tools, which are on.
        assert!(run(&mut config, "tools.key", json!("shift")).is_ok());
    }

    #[test]
    fn turning_a_wheel_on_while_its_key_is_taken_is_refused() {
        let mut config = Config::default();
        config.tools.enabled = false;
        config.formats.key = "command".into();
        let before = config.clone();
        let fault = run(&mut config, "tools.enabled", json!(true)).unwrap_err();
        assert_eq!(fault.code, "wheel_key_taken");
        assert_eq!(config, before);
        // Turning off is always allowed, and so is turning on once the key is free.
        assert!(run(&mut config, "formats.enabled", json!(false)).is_ok());
        assert!(run(&mut config, "tools.enabled", json!(true)).is_ok());
        assert!(config.tools.enabled);
    }

    #[test]
    fn hidden_slots_are_checked_against_the_wheels_own_slots() {
        let mut config = Config::default();
        assert!(run(&mut config, "tools.hidden", json!(["quarter", "half"])).is_ok());
        assert_eq!(
            config.tools.hidden,
            ["half", "quarter"],
            "in the order of the page"
        );
        assert!(
            run(&mut config, "tools.hidden", json!([])).is_ok() && config.tools.hidden.is_empty()
        );
        // Every slot of every wheel can be hidden at once.
        for (name, kind) in WHEELS {
            let all: Vec<&str> = catalog::all_ids(kind).iter().map(|(_, id)| *id).collect();
            let mut unique = all.clone();
            unique.sort_unstable();
            unique.dedup();
            assert!(
                run(&mut config, &format!("{name}.hidden"), json!(unique)).is_ok(),
                "{name}"
            );
        }
        let before = config.clone();
        for (path, bad) in [
            ("tools.hidden", json!("half")),
            ("tools.hidden", json!(null)),
            ("tools.hidden", json!([1])),
            ("tools.hidden", json!(["half", "half"])),
            ("tools.hidden", json!(["bogus"])),
            ("tools.hidden", json!(["to_png"])),
            ("tools.hidden", json!(["../../etc/passwd"])),
            ("actions.hidden", json!(["compress"])),
            ("formats.hidden", json!(["airdrop"])),
        ] {
            assert!(run(&mut config, path, bad.clone()).is_err(), "{path} {bad}");
            assert_eq!(config, before, "{path} {bad}");
        }
    }

    #[test]
    fn the_state_shows_what_will_be_used() {
        let mut config = Config {
            enabled: true,
            ..Config::default()
        };
        config.formats.key = "bogus".into();
        config.tools.hidden = ["quarter", "nonsense", "half", "half"]
            .map(String::from)
            .to_vec();
        config.actions.enabled = false;
        let shown = state(&config, &Value::Null);
        assert_eq!(shown["enabled"], true);
        assert_eq!(
            shown["formats"]["key"], "shift",
            "an unknown name shows as the wheel's own key"
        );
        assert_eq!(shown["tools"]["hidden"], json!(["half", "quarter"]));
        assert_eq!(shown["actions"]["enabled"], false);
        assert_eq!(shown["actions"]["key"], "control");
        assert!(shown.get("accessibility").is_none());
    }

    #[test]
    fn the_sample_data_has_the_keys_of_the_state() {
        let sample: Value =
            serde_json::from_str(include_str!("../../../web/features/wheel.fixture.json")).unwrap();
        let real = state(&Config::default(), &Value::Null);
        let names = |v: &Value| {
            let mut k: Vec<String> = v.as_object().unwrap().keys().cloned().collect();
            k.sort();
            k
        };
        assert_eq!(names(&sample), names(&real));
        for wheel in ["formats", "tools", "actions"] {
            assert_eq!(names(&sample[wheel]), names(&real[wheel]), "{wheel}");
        }
    }

    /// The page lists the slots itself; this keeps its list and the catalog's the same.
    #[test]
    fn the_page_lists_exactly_the_slots_of_the_catalog() {
        let page = include_str!("../../../web/features/wheel.js");
        let mut total = 0;
        for (_, kind) in WHEELS {
            for (group, id) in catalog::all_ids(kind) {
                assert!(
                    page.contains(&format!("'{group}:{id}'")),
                    "{group}:{id} is not on the page"
                );
                total += 1;
            }
        }
        let on_page: usize = ["image", "video", "audio", "pdf", "document", "any"]
            .iter()
            .map(|g| page.matches(&format!("'{g}:")).count())
            .sum();
        assert_eq!(
            on_page, total,
            "the page lists a slot the catalog does not have"
        );
    }
}
