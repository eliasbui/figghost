//! This feature's part of the settings model: the JSON the page renders and the changes it may make.

use super::{Config, SECONDS};
use crate::settings::model::Fault;
use serde_json::{Value, json};

/// The state under the key "outlook". `web/features/outlook.fixture.json` must have exactly the same keys.
pub fn state(config: &Config, runtime: &Value) -> Value {
    let config = config.clone().sanitized();
    json!({
        "enabled": config.enabled,
        "notch": config.notch,
        "seconds": config.seconds,
        "preview": config.preview,
        "drop": config.drop,
        "events": config.events,
        "access": runtime.get("access").and_then(Value::as_bool),
        "count": runtime.get("count").and_then(Value::as_u64).unwrap_or(0),
        "installed": runtime.get("installed").and_then(Value::as_bool),
    })
}

/// A change from the page, which is not trusted.
pub fn set(config: &mut Config, path: &str, value: &Value) -> Option<Result<(), Fault>> {
    let bad = || Fault::new("invalid_value", path);
    let flag = match path {
        "enabled" => &mut config.enabled,
        "notch" => &mut config.notch,
        "preview" => &mut config.preview,
        "drop" => &mut config.drop,
        "events" => &mut config.events,
        "seconds" => {
            return Some(
                value
                    .as_u64()
                    .and_then(|n| u32::try_from(n).ok())
                    .filter(|n| SECONDS.contains(n))
                    .map(|n| config.seconds = n)
                    .ok_or_else(bad),
            );
        }
        _ => return None,
    };
    Some(value.as_bool().map(|on| *flag = on).ok_or_else(bad))
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
            serde_json::from_str(include_str!("../../../web/features/outlook.fixture.json"))
                .unwrap();
        assert_eq!(
            keys(&fixture),
            keys(&state(&Config::default(), &Value::Null))
        );
    }

    #[test]
    fn only_what_is_offered_is_taken() {
        let mut config = Config::default();
        for path in ["enabled", "notch", "preview", "drop", "events"] {
            assert!(
                set(&mut config, path, &json!("yes")).unwrap().is_err(),
                "{path}"
            );
            assert!(
                set(&mut config, path, &json!(true)).unwrap().is_ok(),
                "{path}"
            );
        }
        assert!(config.drop);
        assert!(set(&mut config, "seconds", &json!(8)).unwrap().is_ok());
        assert!(set(&mut config, "seconds", &json!(7)).unwrap().is_err());
        assert_eq!(config.seconds, 8, "a refused value changes nothing");
        assert!(set(&mut config, "nonsense", &json!(1)).is_none());
        let s = state(
            &config,
            &json!({ "access": true, "count": 3, "installed": true }),
        );
        assert_eq!(
            (s["count"].clone(), s["installed"].clone()),
            (json!(3), json!(true))
        );
    }
}
