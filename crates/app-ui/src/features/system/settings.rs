//! This feature's part of the settings model: the JSON the page renders and the changes it may make.
//! Pure data with no AppKit, so every rule is covered by plain unit tests.

use super::{Config, SECONDS};
use crate::settings::model::Fault;
use serde_json::{Value, json};

/// The state under the key "system". `web/features/system.fixture.json` must have exactly the same keys; a unit
/// test checks it.
pub fn state(config: &Config) -> Value {
    let config = config.clone().sanitized();
    json!({
        "enabled": config.enabled,
        "seconds": config.seconds,
        "processor": config.processor,
        "memory": config.memory,
        "network": config.network,
        "details": config.details,
        "bar_processor": config.bar_processor,
        "bar_memory": config.bar_memory,
        "bar_network": config.bar_network,
    })
}

/// A change from the page. `path` is relative to the feature. Values must be validated here; the page is not
/// trusted.
pub fn set(config: &mut Config, path: &str, value: &Value) -> Option<Result<(), Fault>> {
    let bad = || Fault::new("invalid_value", path);
    let switch = |slot: &mut bool| value.as_bool().map(|on| *slot = on).ok_or_else(bad);
    Some(match path {
        "enabled" => switch(&mut config.enabled),
        "processor" => switch(&mut config.processor),
        "memory" => switch(&mut config.memory),
        "network" => switch(&mut config.network),
        "details" => switch(&mut config.details),
        "bar_processor" => switch(&mut config.bar_processor),
        "bar_memory" => switch(&mut config.bar_memory),
        "bar_network" => switch(&mut config.bar_network),
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
        let fixture: Value =
            serde_json::from_str(include_str!("../../../web/features/system.fixture.json"))
                .unwrap();
        assert_eq!(keys(&fixture), keys(&state(&Config::default())));
    }

    #[test]
    fn the_state_starts_off_and_holds_a_hostile_interval_to_a_choice() {
        let s = state(&Config::default());
        assert_eq!(s["enabled"], json!(false));
        assert_eq!(s["seconds"], json!(1));
        let hostile = Config {
            seconds: 4_000_000,
            ..Config::default()
        };
        assert_eq!(state(&hostile)["seconds"], json!(1));
    }

    #[test]
    fn the_switches_are_validated() {
        let mut config = Config::default();
        for path in [
            "enabled",
            "processor",
            "memory",
            "network",
            "details",
            "bar_processor",
            "bar_memory",
            "bar_network",
        ] {
            assert!(
                set(&mut config, path, &json!("yes")).unwrap().is_err(),
                "{path}"
            );
            assert!(
                set(&mut config, path, &json!(null)).unwrap().is_err(),
                "{path}"
            );
            assert!(
                set(&mut config, path, &json!(false)).unwrap().is_ok(),
                "{path}"
            );
        }
        assert!(
            !config.enabled
                && !config.processor
                && !config.memory
                && !config.network
                && !config.details
        );
        assert!(
            set(&mut config, "bar_memory", &json!(true))
                .unwrap()
                .is_ok()
                && config.bar_memory
        );
        assert!(set(&mut config, "nonsense", &json!(1)).is_none());
    }

    #[test]
    fn only_an_offered_interval_is_taken() {
        let mut config = Config::default();
        for seconds in SECONDS {
            assert!(
                set(&mut config, "seconds", &json!(seconds))
                    .unwrap()
                    .is_ok()
            );
            assert_eq!(config.seconds, seconds);
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
            assert!(set(&mut config, "seconds", &bad).unwrap().is_err(), "{bad}");
        }
        assert_eq!(config.seconds, 5, "a refused value changes nothing");
    }
}
