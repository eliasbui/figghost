//! This feature's part of the settings model: the JSON the page renders and the changes it may make.
//! Pure data with no AppKit, so every rule is covered by plain unit tests.

use super::Config;
use crate::settings::model::Fault;
use serde_json::{Value, json};

/// The state under the key "displays". `web/features/displays.fixture.json` must have exactly the same keys; a unit
/// test checks it.
pub fn state(config: &Config) -> Value {
    json!({
        "enabled": config.enabled,
        "keys": config.keys,
        "profiles": config.profiles,
        "arrange": config.arrange,
    })
}

/// A change from the page. `path` is relative to the feature. Values must be validated here; the page is not trusted.
pub fn set(config: &mut Config, path: &str, value: &Value) -> Option<Result<(), Fault>> {
    let slot = match path {
        "enabled" => &mut config.enabled,
        "keys" => &mut config.keys,
        "profiles" => &mut config.profiles,
        "arrange" => &mut config.arrange,
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

    fn keys(value: &Value) -> Vec<String> {
        let mut keys: Vec<String> = value.as_object().unwrap().keys().cloned().collect();
        keys.sort();
        keys
    }

    #[test]
    fn the_fixture_has_exactly_the_keys_of_the_state() {
        let fixture: Value =
            serde_json::from_str(include_str!("../../../web/features/displays.fixture.json"))
                .unwrap();
        assert_eq!(keys(&fixture), keys(&state(&Config::default())));
    }

    #[test]
    fn the_switches_take_only_true_or_false() {
        let mut config = Config::default();
        for path in ["enabled", "keys", "profiles", "arrange"] {
            for bad in [json!("yes"), json!(1), json!(null)] {
                assert!(
                    set(&mut config, path, &bad).unwrap().is_err(),
                    "{path} {bad}"
                );
            }
            assert!(set(&mut config, path, &json!(true)).unwrap().is_ok());
        }
        assert!(config.enabled && config.keys && config.profiles && config.arrange);
        assert!(set(&mut config, "nonsense", &json!(true)).is_none());
    }
}
