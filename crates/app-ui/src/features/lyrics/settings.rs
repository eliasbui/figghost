//! This feature's part of the settings model: the JSON the page renders and the changes it may make.
//! Pure data with no AppKit, so every rule is covered by plain unit tests.

use super::Config;
use crate::settings::model::Fault;
use serde_json::{Value, json};

/// The state under the key "lyrics". `web/features/lyrics.fixture.json` must have exactly the same keys.
pub fn state(config: &Config, _runtime: &Value) -> Value {
    json!({ "enabled": config.enabled, "fetch": config.fetch })
}

fn flag(value: &Value) -> Result<bool, Fault> {
    value
        .as_bool()
        .ok_or_else(|| Fault::new("invalid_value", "expected true or false"))
}

/// A change from the page. `path` is relative to the feature: "enabled", not "lyrics.enabled".
pub fn set(config: &mut Config, path: &str, value: &Value) -> Option<Result<(), Fault>> {
    let field = match path {
        "enabled" => &mut config.enabled,
        "fetch" => &mut config.fetch,
        _ => return None,
    };
    Some(flag(value).map(|on| *field = on))
}

/// The feature has no global shortcut.
pub fn shortcuts(_config: &Config) -> Vec<(String, String)> {
    Vec::new()
}

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
    fn both_switches_start_off_and_are_validated() {
        let mut config = Config::default();
        assert_eq!(
            state(&config, &Value::Null),
            json!({"enabled": false, "fetch": false})
        );
        assert!(set(&mut config, "enabled", &json!(true)).unwrap().is_ok() && config.enabled);
        assert!(!config.fetch);
        assert!(set(&mut config, "fetch", &json!(true)).unwrap().is_ok() && config.fetch);
        for bad in [json!("yes"), json!(1), json!(null), json!([true])] {
            assert!(set(&mut config, "fetch", &bad).unwrap().is_err());
            assert!(set(&mut config, "enabled", &bad).unwrap().is_err());
        }
        assert!(config.enabled && config.fetch);
        assert!(set(&mut config, "nonsense", &json!(1)).is_none());
    }

    #[test]
    fn a_partial_config_keeps_the_defaults_for_the_rest() {
        let config: Config = serde_json::from_str(r#"{"enabled": true}"#).unwrap();
        assert!(config.enabled && !config.fetch);
        assert!(serde_json::from_str::<Config>(r#"{"fetch": "yes"}"#).is_err());
        assert_eq!(
            serde_json::from_str::<Config>("{}").unwrap(),
            Config::default()
        );
    }
}
