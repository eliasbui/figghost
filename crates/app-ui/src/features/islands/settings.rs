//! This feature's part of the settings model: the JSON the page renders and the changes it may make.
//! Pure data with no AppKit, so every rule is covered by plain unit tests.

use super::{Config, FULL_RANGE, LOW_RANGE, SECONDS_RANGE};
use crate::settings::model::Fault;
use serde_json::{Value, json};

/// The state under the key "islands". `runtime` is `Feature::runtime()`, or null when the feature is not running.
/// `web/features/islands.fixture.json` must have exactly the same keys; a unit test checks it.
pub fn state(config: &Config, runtime: &Value) -> Value {
    json!({
        "enabled": config.enabled,
        "caps_lock": config.caps_lock,
        "power": config.power,
        "drives": config.drives,
        "downloads": config.downloads,
        "privacy": config.privacy,
        "full_at": config.full_at,
        "low_at": config.low_at,
        "seconds": config.seconds,
        "has_battery": runtime.get("has_battery").and_then(Value::as_bool).unwrap_or(false),
    })
}

/// A change from the page. `path` is relative to the feature. Values must be validated here; the page is not
/// trusted.
pub fn set(config: &mut Config, path: &str, value: &Value) -> Option<Result<(), Fault>> {
    let bad = || Fault::new("invalid_value", path);
    let switch = |slot: &mut bool| value.as_bool().map(|on| *slot = on).ok_or_else(bad);
    Some(match path {
        "enabled" => switch(&mut config.enabled),
        "caps_lock" => switch(&mut config.caps_lock),
        "power" => switch(&mut config.power),
        "drives" => switch(&mut config.drives),
        "downloads" => switch(&mut config.downloads),
        "privacy" => switch(&mut config.privacy),
        "full_at" => whole(value, FULL_RANGE)
            .map(|n| config.full_at = n)
            .ok_or_else(bad),
        "low_at" => whole(value, LOW_RANGE)
            .map(|n| config.low_at = n)
            .ok_or_else(bad),
        "seconds" => fraction(value, SECONDS_RANGE)
            .map(|n| config.seconds = n)
            .ok_or_else(bad),
        _ => return None,
    })
}

/// A whole number from the page held to its range; `None` when it is not a finite number.
fn whole(value: &Value, (low, high): (u8, u8)) -> Option<u8> {
    value
        .as_f64()
        .filter(|n| n.is_finite())
        .map(|n| n.round().clamp(f64::from(low), f64::from(high)) as u8)
}

fn fraction(value: &Value, (low, high): (f64, f64)) -> Option<f64> {
    value
        .as_f64()
        .filter(|n| n.is_finite())
        .map(|n| n.clamp(low, high))
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
            serde_json::from_str(include_str!("../../../web/features/islands.fixture.json"))
                .unwrap();
        assert_eq!(
            keys(&fixture),
            keys(&state(&Config::default(), &Value::Null))
        );
    }

    #[test]
    fn the_feature_is_off_until_asked_and_every_notice_is_on_by_default() {
        let config = Config::default();
        assert!(!config.enabled);
        assert!(
            config.caps_lock && config.power && config.drives && config.downloads && config.privacy
        );
        assert_eq!(state(&config, &Value::Null)["has_battery"], json!(false));
    }

    #[test]
    fn switches_take_booleans_and_nothing_else() {
        let mut config = Config::default();
        assert!(
            set(&mut config, "caps_lock", &json!(false))
                .unwrap()
                .is_ok()
        );
        assert!(!config.caps_lock);
        assert!(
            set(&mut config, "caps_lock", &json!("yes"))
                .unwrap()
                .is_err()
        );
        assert!(set(&mut config, "nonsense", &json!(true)).is_none());
    }

    #[test]
    fn levels_are_held_to_their_ranges() {
        let mut config = Config::default();
        set(&mut config, "full_at", &json!(10)).unwrap().unwrap();
        assert_eq!(config.full_at, FULL_RANGE.0);
        set(&mut config, "full_at", &json!(999)).unwrap().unwrap();
        assert_eq!(config.full_at, FULL_RANGE.1);
        set(&mut config, "low_at", &json!(2)).unwrap().unwrap();
        assert_eq!(config.low_at, LOW_RANGE.0);
        set(&mut config, "seconds", &json!(1000)).unwrap().unwrap();
        assert!((config.seconds - SECONDS_RANGE.1).abs() < f64::EPSILON);
        assert!(set(&mut config, "seconds", &json!(null)).unwrap().is_err());
    }
}
