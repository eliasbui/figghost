//! This feature's part of the settings model: the JSON the page renders and the changes it may make.
//! Pure data with no AppKit, so every rule is covered by plain unit tests.

use super::Config;
use crate::settings::model::Fault;
use core_window::snap::{self, MAX_ZONES};
use serde_json::{Value, json};

/// The state under the key "snap". `runtime` is `Feature::runtime()`, or null when the feature is not running.
/// `web/features/snap.fixture.json` must have exactly the same keys; a unit test checks it.
pub fn state(config: &Config, runtime: &Value) -> Value {
    let zones: Vec<&str> = config.layouts().iter().map(|l| l.key()).collect();
    json!({
        "enabled": config.enabled,
        "zones": zones,
        "accessibility": runtime["accessibility"].as_bool().unwrap_or(false),
    })
}

/// A change from the page. `path` is relative to the feature: "enabled", not "snap.enabled". `None` when the
/// path is not one of this feature's. Values must be validated here; the page is not trusted.
pub fn set(config: &mut Config, path: &str, value: &Value) -> Option<Result<(), Fault>> {
    match path {
        "enabled" => Some(
            value
                .as_bool()
                .map(|on| config.enabled = on)
                .ok_or_else(|| Fault::new("invalid_value", "expected true or false")),
        ),
        "zones" => Some(set_zones(config, value)),
        _ => None,
    }
}

/// Every name must be one that can be offered, none twice, and there must be between one and `MAX_ZONES`. The page
/// builds the list from checkboxes, so anything else is a bug or a hostile message, and is refused whole.
fn set_zones(config: &mut Config, value: &Value) -> Result<(), Fault> {
    let invalid = || Fault::new("invalid_value", "expected a list of layout names");
    let names: Vec<&str> = value
        .as_array()
        .ok_or_else(invalid)?
        .iter()
        .map(|v| v.as_str().ok_or_else(invalid))
        .collect::<Result<_, _>>()?;
    let chosen = snap::choose(&names);
    let known = names
        .iter()
        .all(|n| snap::layout_from_key(n).is_some_and(|l| snap::OFFERED.contains(&l)));
    let mut unique = names.clone();
    unique.sort_unstable();
    unique.dedup();
    if names.is_empty() || names.len() > MAX_ZONES || !known || unique.len() != names.len() {
        return Err(Fault::new("snap_zones", format!("{} names", names.len())));
    }
    config.zones = chosen.iter().map(|l| l.key().to_owned()).collect();
    Ok(())
}

/// This feature's shortcuts as (target, shortcut). It has none.
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

    fn keys(config: &Config) -> Vec<&str> {
        config.zones.iter().map(String::as_str).collect()
    }

    #[test]
    fn the_switch_is_validated() {
        let mut config = Config::default();
        assert!(set(&mut config, "enabled", &json!(true)).unwrap().is_ok() && config.enabled);
        assert!(set(&mut config, "enabled", &json!("yes")).unwrap().is_err());
        assert!(set(&mut config, "nonsense", &json!(1)).is_none());
    }

    #[test]
    fn zones_are_stored_in_the_order_of_the_row() {
        let mut config = Config::default();
        let result = set(&mut config, "zones", &json!(["maximize", "right", "left"]));
        assert!(result.unwrap().is_ok());
        assert_eq!(keys(&config), ["left", "right", "maximize"]);
    }

    #[test]
    fn bad_zone_lists_are_refused_and_change_nothing() {
        let before = Config::default();
        let mut config = before.clone();
        let too_many: Vec<&str> = snap::OFFERED.iter().map(|l| l.key()).collect();
        for bad in [
            json!([]),
            json!("left"),
            json!(null),
            json!([1, 2]),
            json!(["left", "left"]),
            json!(["left", "bogus"]),
            json!(["center"]),
            json!(["../../etc/passwd"]),
            json!(too_many),
        ] {
            let result = set(&mut config, "zones", &bad).unwrap();
            assert!(result.is_err(), "{bad}");
            assert_eq!(config, before, "{bad}");
        }
    }

    #[test]
    fn the_state_shows_what_will_be_used_and_the_permission() {
        let config = Config {
            zones: vec!["bogus".into()],
            ..Config::default()
        };
        let shown = state(&config, &json!({ "accessibility": true }));
        assert_eq!(shown["zones"].as_array().unwrap().len(), 6);
        assert_eq!(shown["accessibility"], true);
        assert_eq!(state(&config, &Value::Null)["accessibility"], false);
    }

    #[test]
    fn the_sample_data_has_the_keys_of_the_state() {
        let sample: Value =
            serde_json::from_str(include_str!("../../../web/features/snap.fixture.json")).unwrap();
        let real = state(&Config::default(), &json!({ "accessibility": true }));
        let names = |v: &Value| {
            let mut k: Vec<String> = v.as_object().unwrap().keys().cloned().collect();
            k.sort();
            k
        };
        assert_eq!(names(&sample), names(&real));
    }
}
