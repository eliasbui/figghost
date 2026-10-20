//! This feature's part of the settings model: the JSON the page renders and the changes it may make.
//! Pure data with no AppKit, so every rule is covered by plain unit tests.

use super::{Config, logic::LOW_AT};
use crate::settings::model::Fault;
use serde_json::{Value, json};

/// The state under the key "devices". `web/features/devices.fixture.json` must have exactly the same keys; a unit test
/// checks it.
pub fn state(config: &Config) -> Value {
    let config = config.clone().sanitized();
    json!({
        "enabled": config.enabled,
        "low_at": config.low_at,
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
        "low_at" => value
            .as_u64()
            .and_then(|n| u8::try_from(n).ok())
            .filter(|n| LOW_AT.contains(n))
            .map(|n| config.low_at = n)
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
            serde_json::from_str(include_str!("../../../web/features/devices.fixture.json"))
                .unwrap();
        assert_eq!(keys(&fixture), keys(&state(&Config::default())));
    }

    #[test]
    fn the_state_starts_off_and_holds_a_hostile_level_to_a_choice() {
        let s = state(&Config::default());
        assert_eq!(
            (s["enabled"].clone(), s["low_at"].clone()),
            (json!(false), json!(20))
        );
        let hostile = Config {
            low_at: 99,
            ..Config::default()
        };
        assert_eq!(state(&hostile)["low_at"], json!(20));
    }

    #[test]
    fn the_switch_and_the_level_are_validated() {
        let mut config = Config::default();
        for bad in [json!("yes"), json!(null), json!(1)] {
            assert!(set(&mut config, "enabled", &bad).unwrap().is_err(), "{bad}");
        }
        assert!(set(&mut config, "enabled", &json!(true)).unwrap().is_ok() && config.enabled);
        for &level in &LOW_AT {
            assert!(
                set(&mut config, "low_at", &json!(level)).unwrap().is_ok(),
                "{level}"
            );
        }
        for bad in [
            json!(0),
            json!(25),
            json!(256),
            json!(-1),
            json!(10.5),
            json!("10"),
            json!(null),
            json!(1e12),
        ] {
            assert!(set(&mut config, "low_at", &bad).unwrap().is_err(), "{bad}");
        }
        assert_eq!(config.low_at, 30, "a refused value changes nothing");
        assert!(set(&mut config, "nonsense", &json!(1)).is_none());
    }
}
