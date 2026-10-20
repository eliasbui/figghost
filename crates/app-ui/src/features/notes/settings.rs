//! This feature's part of the settings model: the JSON the page renders and the changes it may make.
//! Pure data with no AppKit, so every rule is covered by plain unit tests.

use super::{Config, TEXT_SIZES};
use crate::settings::model::Fault;
use serde_json::{Value, json};

/// The state under the key "notes". `runtime` is `Feature::runtime()`, or null when the feature is not running.
/// `web/features/notes.fixture.json` must have exactly the same keys; a unit test checks it.
pub fn state(config: &Config, runtime: &Value) -> Value {
    let config = config.clone().sanitized();
    json!({
        "enabled": config.enabled,
        "text_size": config.text_size,
        "new_on_open": config.new_on_open,
        "count": runtime.get("count").and_then(Value::as_u64).unwrap_or(0),
        "folder": runtime.get("folder").and_then(Value::as_str).unwrap_or_default(),
    })
}

/// A change from the page. `path` is relative to the feature. Values must be validated here; the page is not
/// trusted.
pub fn set(config: &mut Config, path: &str, value: &Value) -> Option<Result<(), Fault>> {
    let bad = || Fault::new("invalid_value", path);
    let switch = |slot: &mut bool| value.as_bool().map(|on| *slot = on).ok_or_else(bad);
    Some(match path {
        "enabled" => switch(&mut config.enabled),
        "new_on_open" => switch(&mut config.new_on_open),
        "text_size" => value
            .as_u64()
            .and_then(|n| u32::try_from(n).ok())
            .filter(|n| TEXT_SIZES.contains(n))
            .map(|n| config.text_size = n)
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
        let fixture: Value =
            serde_json::from_str(include_str!("../../../web/features/notes.fixture.json")).unwrap();
        assert_eq!(
            keys(&fixture),
            keys(&state(&Config::default(), &Value::Null))
        );
    }

    #[test]
    fn the_state_starts_off_and_says_what_the_running_feature_knows() {
        let s = state(&Config::default(), &Value::Null);
        assert_eq!(s["enabled"], json!(false));
        assert_eq!(s["count"], json!(0));
        let running = json!({ "count": 12, "folder": "/Users/me/notes" });
        let s = state(&Config::default(), &running);
        assert_eq!(
            (s["count"].clone(), s["folder"].clone()),
            (json!(12), json!("/Users/me/notes"))
        );
        let hostile = Config {
            text_size: 4_000_000,
            ..Config::default()
        };
        assert_eq!(state(&hostile, &Value::Null)["text_size"], json!(14));
    }

    #[test]
    fn the_switches_are_validated() {
        let mut config = Config::default();
        for path in ["enabled", "new_on_open"] {
            assert!(
                set(&mut config, path, &json!("yes")).unwrap().is_err(),
                "{path}"
            );
            assert!(
                set(&mut config, path, &json!(null)).unwrap().is_err(),
                "{path}"
            );
            assert!(
                set(&mut config, path, &json!(true)).unwrap().is_ok(),
                "{path}"
            );
        }
        assert!(config.enabled && config.new_on_open);
        assert!(set(&mut config, "nonsense", &json!(1)).is_none());
    }

    #[test]
    fn only_an_offered_size_is_taken() {
        let mut config = Config::default();
        for size in TEXT_SIZES {
            assert!(set(&mut config, "text_size", &json!(size)).unwrap().is_ok());
            assert_eq!(config.text_size, size);
        }
        for bad in [
            json!(13),
            json!(0),
            json!(-14),
            json!(14.5),
            json!("14"),
            json!(null),
            json!(1e12),
        ] {
            assert!(
                set(&mut config, "text_size", &bad).unwrap().is_err(),
                "{bad}"
            );
        }
        assert_eq!(config.text_size, 16, "a refused value changes nothing");
    }
}
