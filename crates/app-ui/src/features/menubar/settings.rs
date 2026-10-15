//! This feature's part of the settings model: the JSON the page renders and the changes it may make.
//! Pure data with no AppKit, so every rule is covered by plain unit tests.

use super::{Config, MAX_AUTO_SECONDS};
use crate::settings::model::Fault;
use serde_json::{Value, json};

/// The state under the key "menubar". `runtime` is `Feature::runtime()`, or null when the feature is not running.
/// `web/features/menubar.fixture.json` must have exactly the same keys; a unit test checks it.
pub fn state(config: &Config, runtime: &Value) -> Value {
    json!({
        "enabled": config.enabled,
        "auto_seconds": config.rules().auto_seconds,
        "click_only": config.click_only,
        "status": runtime["status"].as_str().unwrap_or("off"),
    })
}

fn flag(value: &Value) -> Result<bool, Fault> {
    value
        .as_bool()
        .ok_or_else(|| Fault::new("invalid_value", "expected true or false"))
}

/// A change from the page. `path` is relative to the feature: "enabled", not "menubar.enabled". `None` when the
/// path is not one of this feature's. Values must be validated here; the page is not trusted.
pub fn set(config: &mut Config, path: &str, value: &Value) -> Option<Result<(), Fault>> {
    match path {
        "enabled" => Some(flag(value).map(|on| config.enabled = on)),
        "click_only" => Some(flag(value).map(|on| config.click_only = on)),
        "auto_seconds" => Some(
            value
                .as_u64()
                .filter(|seconds| *seconds <= u64::from(MAX_AUTO_SECONDS))
                .map(|seconds| config.auto_seconds = seconds as u32)
                .ok_or_else(|| {
                    Fault::new(
                        "invalid_value",
                        format!("expected 0 to {MAX_AUTO_SECONDS} seconds"),
                    )
                }),
        ),
        _ => None,
    }
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

    #[test]
    fn the_switches_are_validated() {
        let mut config = Config::default();
        assert!(set(&mut config, "enabled", &json!(true)).unwrap().is_ok() && config.enabled);
        assert!(set(&mut config, "enabled", &json!("yes")).unwrap().is_err());
        assert!(
            set(&mut config, "click_only", &json!(false))
                .unwrap()
                .is_ok()
                && !config.click_only
        );
        assert!(set(&mut config, "click_only", &json!(1)).unwrap().is_err());
        assert!(set(&mut config, "nonsense", &json!(1)).is_none());
    }

    #[test]
    fn the_delay_is_a_whole_number_of_seconds_up_to_an_hour() {
        let mut config = Config::default();
        for ok in [0, 5, 3600] {
            assert!(
                set(&mut config, "auto_seconds", &json!(ok))
                    .unwrap()
                    .is_ok()
            );
            assert_eq!(config.auto_seconds, ok);
        }
        for bad in [
            json!(3601),
            json!(-1),
            json!(2.5),
            json!("10"),
            json!(null),
            json!(1e12),
        ] {
            let before = config.clone();
            assert!(
                set(&mut config, "auto_seconds", &bad).unwrap().is_err(),
                "{bad}"
            );
            assert_eq!(config, before, "{bad}");
        }
    }

    #[test]
    fn the_state_carries_what_the_running_feature_reports() {
        let config = Config::default();
        assert_eq!(state(&config, &Value::Null)["status"], "off");
        assert_eq!(
            state(&config, &json!({ "status": "hiding" }))["status"],
            "hiding"
        );
        assert_eq!(state(&config, &json!({ "status": 7 }))["status"], "off");
    }

    #[test]
    fn the_sample_data_has_the_keys_of_the_state() {
        let sample: Value =
            serde_json::from_str(include_str!("../../../web/features/menubar.fixture.json"))
                .unwrap();
        let real = state(&Config::default(), &json!({ "status": "showing" }));
        let names = |v: &Value| {
            let mut k: Vec<String> = v.as_object().unwrap().keys().cloned().collect();
            k.sort();
            k
        };
        assert_eq!(names(&sample), names(&real));
    }
}
