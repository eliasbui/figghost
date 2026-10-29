//! This feature's part of the settings model: the JSON the page renders and the changes it may make.
//! Pure data with no AppKit, so every rule is covered by plain unit tests.

use super::{Config, logic::IDLE_MINUTES};
use crate::settings::model::Fault;
use serde_json::{Value, json};

/// The state under the key "screentime". `runtime` is `Feature::runtime()`, or null when the feature is not running.
/// `web/features/screentime.fixture.json` must have exactly the same keys; a unit test checks it.
pub fn state(config: &Config, runtime: &Value) -> Value {
    let config = config.clone().sanitized();
    json!({
        "enabled": config.enabled,
        "idle_minutes": config.idle_minutes,
        "sites": config.sites,
        "days": runtime.get("days").and_then(Value::as_u64).unwrap_or(0),
        "today": runtime.get("today").and_then(Value::as_u64).unwrap_or(0),
    })
}

/// A change from the page. `path` is relative to the feature. Values must be validated here; the page is not
/// trusted.
pub fn set(config: &mut Config, path: &str, value: &Value) -> Option<Result<(), Fault>> {
    let bad = || Fault::new("invalid_value", path);
    Some(match path {
        "enabled" => value
            .as_bool()
            .map(|on| config.enabled = on)
            .ok_or_else(bad),
        "sites" => value.as_bool().map(|on| config.sites = on).ok_or_else(bad),
        "idle_minutes" => value
            .as_u64()
            .and_then(|n| u32::try_from(n).ok())
            .filter(|n| IDLE_MINUTES.contains(n))
            .map(|n| config.idle_minutes = n)
            .ok_or_else(bad),
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn keys(value: &Value) -> Vec<String> {
        let mut keys: Vec<String> = value.as_object().unwrap().keys().cloned().collect();
        keys.sort();
        keys
    }

    #[test]
    fn the_fixture_has_exactly_the_keys_of_the_state() {
        let fixture: Value = serde_json::from_str(include_str!(
            "../../../web/features/screentime.fixture.json"
        ))
        .unwrap();
        assert_eq!(
            keys(&fixture),
            keys(&state(&Config::default(), &Value::Null))
        );
    }

    #[test]
    fn the_state_starts_off_and_says_what_the_running_feature_knows() {
        let s = state(&Config::default(), &Value::Null);
        assert_eq!(
            (s["enabled"].clone(), s["days"].clone(), s["today"].clone()),
            (json!(false), json!(0), json!(0))
        );
        let s = state(&Config::default(), &json!({ "days": 12, "today": 3600 }));
        assert_eq!(
            (s["days"].clone(), s["today"].clone()),
            (json!(12), json!(3600))
        );
        let hostile = Config {
            idle_minutes: 4_000_000,
            ..Config::default()
        };
        assert_eq!(state(&hostile, &Value::Null)["idle_minutes"], json!(2));
    }

    #[test]
    fn the_switch_and_the_idle_time_are_validated() {
        let mut config = Config::default();
        assert!(set(&mut config, "enabled", &json!("yes")).unwrap().is_err());
        assert!(set(&mut config, "enabled", &json!(true)).unwrap().is_ok() && config.enabled);
        assert!(!config.sites, "sites are counted only when asked");
        assert!(set(&mut config, "sites", &json!(1)).unwrap().is_err());
        assert!(set(&mut config, "sites", &json!(true)).unwrap().is_ok() && config.sites);
        for minutes in IDLE_MINUTES {
            assert!(
                set(&mut config, "idle_minutes", &json!(minutes))
                    .unwrap()
                    .is_ok()
            );
            assert_eq!(config.idle_minutes, minutes);
        }
        for bad in [
            json!(3),
            json!(0),
            json!(-1),
            json!(2.5),
            json!("2"),
            json!(null),
            json!(1e12),
        ] {
            assert!(
                set(&mut config, "idle_minutes", &bad).unwrap().is_err(),
                "{bad}"
            );
        }
        assert_eq!(config.idle_minutes, 5, "a refused value changes nothing");
        assert!(set(&mut config, "nonsense", &json!(1)).is_none());
    }
}
