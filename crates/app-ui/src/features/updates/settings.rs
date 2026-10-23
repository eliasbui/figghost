//! This feature's part of the settings model: the JSON the page renders and the changes it may make.

use super::Config;
use crate::settings::model::Fault;
use serde_json::{Value, json};

/// The state under the key "updates". `web/features/updates.fixture.json` must have exactly the same keys.
pub fn state(config: &Config, runtime: &Value) -> Value {
    json!({
        "enabled": config.enabled,
        "daily": config.daily,
        "count": runtime.get("count").and_then(Value::as_u64).unwrap_or(0),
        "busy": runtime.get("busy").and_then(Value::as_bool).unwrap_or(false),
    })
}

/// A change from the page, which is not trusted.
pub fn set(config: &mut Config, path: &str, value: &Value) -> Option<Result<(), Fault>> {
    let slot = match path {
        "enabled" => &mut config.enabled,
        "daily" => &mut config.daily,
        _ => return None,
    };
    Some(
        value
            .as_bool()
            .map(|on| *slot = on)
            .ok_or_else(|| Fault::new("invalid_value", path)),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_fixture_has_exactly_the_keys_of_the_state() {
        let fixture: Value =
            serde_json::from_str(include_str!("../../../web/features/updates.fixture.json"))
                .unwrap();
        let keys = |v: &Value| {
            let mut k: Vec<String> = v.as_object().unwrap().keys().cloned().collect();
            k.sort();
            k
        };
        assert_eq!(
            keys(&fixture),
            keys(&state(&Config::default(), &Value::Null))
        );
    }

    #[test]
    fn only_switches_are_taken() {
        let mut config = Config::default();
        assert!(set(&mut config, "daily", &json!(true)).unwrap().is_ok());
        assert!(config.daily);
        assert!(set(&mut config, "enabled", &json!("yes")).unwrap().is_err());
        assert!(set(&mut config, "other", &json!(true)).is_none());
    }
}
