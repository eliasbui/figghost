//! This feature's part of the settings model: the JSON the page renders and the changes it may make.
//! Pure data with no AppKit, so every rule is covered by plain unit tests.

use super::{
    Config,
    logic::{BED, WAKE},
};
use crate::settings::model::Fault;
use serde_json::{Value, json};

/// The state under the key "dayprogress". `web/features/dayprogress.fixture.json` must have exactly the same keys; a unit
/// test checks it.
pub fn state(config: &Config) -> Value {
    let config = config.clone().sanitized();
    json!({
        "enabled": config.enabled,
        "wake": config.wake,
        "bed": config.bed,
        "reminders": config.reminders,
    })
}

/// A change from the page. `path` is relative to the feature. Values must be validated here; the page is not trusted.
pub fn set(config: &mut Config, path: &str, value: &Value) -> Option<Result<(), Fault>> {
    let bad = || Fault::new("invalid_value", path);
    let switch = |slot: &mut bool| value.as_bool().map(|on| *slot = on).ok_or_else(bad);
    let hour = |slot: &mut u32, list: &[u32]| {
        value
            .as_u64()
            .and_then(|n| u32::try_from(n).ok())
            .filter(|n| list.contains(n))
            .map(|n| *slot = n)
            .ok_or_else(bad)
    };
    Some(match path {
        "enabled" => switch(&mut config.enabled),
        "reminders" => switch(&mut config.reminders),
        "wake" => hour(&mut config.wake, &WAKE),
        "bed" => hour(&mut config.bed, &BED),
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
            "../../../web/features/dayprogress.fixture.json"
        ))
        .unwrap();
        assert_eq!(keys(&fixture), keys(&state(&Config::default())));
    }

    #[test]
    fn the_state_starts_off_with_the_usual_day_and_holds_a_hostile_file_to_the_choices() {
        let s = state(&Config::default());
        assert_eq!(
            (
                s["enabled"].clone(),
                s["wake"].clone(),
                s["bed"].clone(),
                s["reminders"].clone()
            ),
            (json!(false), json!(7), json!(23), json!(true))
        );
        let hostile = Config {
            wake: 3,
            bed: 99,
            ..Config::default()
        };
        let s = state(&hostile);
        assert_eq!((s["wake"].clone(), s["bed"].clone()), (json!(7), json!(23)));
    }

    #[test]
    fn the_switches_and_the_hours_are_validated() {
        let mut config = Config::default();
        for path in ["enabled", "reminders"] {
            assert!(
                set(&mut config, path, &json!("yes")).unwrap().is_err(),
                "{path}"
            );
            assert!(
                set(&mut config, path, &json!(null)).unwrap().is_err(),
                "{path}"
            );
        }
        assert!(set(&mut config, "enabled", &json!(true)).unwrap().is_ok() && config.enabled);
        for (path, list) in [("wake", &WAKE[..]), ("bed", &BED[..])] {
            for &hour in list {
                assert!(
                    set(&mut config, path, &json!(hour)).unwrap().is_ok(),
                    "{path} {hour}"
                );
            }
            for bad in [
                json!(0),
                json!(25),
                json!(-1),
                json!(7.5),
                json!("7"),
                json!(null),
                json!(1e12),
            ] {
                assert!(
                    set(&mut config, path, &bad).unwrap().is_err(),
                    "{path} {bad}"
                );
            }
        }
        assert_eq!(
            (config.wake, config.bed),
            (10, 24),
            "a refused value changes nothing"
        );
        assert!(set(&mut config, "nonsense", &json!(1)).is_none());
    }
}
