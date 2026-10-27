//! This feature's part of the settings model: the JSON the page renders and the changes it may make.
//! Pure data with no AppKit, so every rule is covered by plain unit tests.

use super::{Config, MODES, SECONDS};
use crate::settings::model::Fault;
use serde_json::{Value, json};

/// The state under the key "notifications". `runtime` is `Feature::runtime()`, or null when the feature is not running.
/// `web/features/notifications.fixture.json` must have exactly the same keys; a unit test checks it.
pub fn state(config: &Config, runtime: &Value) -> Value {
    let config = config.clone().sanitized();
    json!({
        "enabled": config.enabled,
        "mode": config.mode,
        "seconds": config.seconds,
        "preview": config.preview,
        // Null until the feature is on: the store is not touched before.
        "access": runtime.get("access").and_then(Value::as_bool),
        "count": runtime.get("count").and_then(Value::as_u64).unwrap_or(0),
    })
}

/// A change from the page. `path` is relative to the feature. Values must be validated here; the page is not trusted.
pub fn set(config: &mut Config, path: &str, value: &Value) -> Option<Result<(), Fault>> {
    let bad = || Fault::new("invalid_value", path);
    Some(match path {
        "enabled" => value
            .as_bool()
            .map(|on| config.enabled = on)
            .ok_or_else(bad),
        "preview" => value
            .as_bool()
            .map(|on| config.preview = on)
            .ok_or_else(bad),
        "mode" => value
            .as_str()
            .filter(|m| MODES.contains(m))
            .map(|m| config.mode = m.to_owned())
            .ok_or_else(bad),
        "seconds" => value
            .as_u64()
            .and_then(|n| u32::try_from(n).ok())
            .filter(|n| SECONDS.contains(n))
            .map(|n| config.seconds = n)
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
            "../../../web/features/notifications.fixture.json"
        ))
        .unwrap();
        assert_eq!(
            keys(&fixture),
            keys(&state(&Config::default(), &Value::Null))
        );
    }

    #[test]
    fn the_state_says_what_the_running_feature_knows() {
        let s = state(&Config::default(), &Value::Null);
        assert_eq!(
            (s["access"].clone(), s["count"].clone()),
            (Value::Null, json!(0)),
            "not known before the store was tried"
        );
        let s = state(&Config::default(), &json!({ "access": true, "count": 7 }));
        assert_eq!(
            (s["access"].clone(), s["count"].clone()),
            (json!(true), json!(7))
        );
        assert_eq!(s["mode"], json!("notch"));
    }

    #[test]
    fn only_what_is_offered_is_taken() {
        let mut config = Config::default();
        for path in ["enabled", "preview"] {
            assert!(
                set(&mut config, path, &json!("yes")).unwrap().is_err(),
                "{path}"
            );
            assert!(
                set(&mut config, path, &json!(true)).unwrap().is_ok(),
                "{path}"
            );
        }
        assert!(set(&mut config, "mode", &json!("system")).unwrap().is_ok());
        assert!(set(&mut config, "mode", &json!("both")).unwrap().is_err());
        assert!(set(&mut config, "seconds", &json!(8)).unwrap().is_ok());
        assert!(set(&mut config, "seconds", &json!(9)).unwrap().is_err());
        assert!(set(&mut config, "seconds", &json!(-3)).unwrap().is_err());
        assert_eq!(
            (config.mode.as_str(), config.seconds),
            ("system", 8),
            "a refused value changes nothing"
        );
        assert!(set(&mut config, "nonsense", &json!(1)).is_none());
    }
}
