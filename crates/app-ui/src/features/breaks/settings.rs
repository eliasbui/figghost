//! This feature's part of the settings model: the JSON the page renders and the changes it may make.
//! Pure data with no AppKit, so every rule is covered by plain unit tests.

use super::{Config, logic};
use crate::{config::parse_hotkey, settings::model::Fault};
use serde_json::{Value, json};

/// The state under the key "breaks". `runtime` is `Feature::runtime()`, or null when the feature is not running.
/// `web/features/breaks.fixture.json` must have exactly the same keys; a unit test checks it.
pub fn state(config: &Config, runtime: &Value) -> Value {
    let config = config.clone().sanitized();
    json!({
        "enabled": config.enabled,
        "work_minutes": config.work_minutes,
        "break_seconds": config.break_seconds,
        "cover": config.cover,
        "sound": config.sound,
        "blink_minutes": config.blink_minutes,
        "posture_minutes": config.posture_minutes,
        "hold_meetings": config.hold_meetings,
        "hold_full_screen": config.hold_full_screen,
        "shortcut": config.shortcut,
        "until": runtime.get("until").and_then(Value::as_u64),
        "breaking": runtime.get("breaking").and_then(Value::as_bool).unwrap_or(false),
    })
}

/// A change from the page. `path` is relative to the feature. Values must be validated here; the page is not
/// trusted.
pub fn set(config: &mut Config, path: &str, value: &Value) -> Option<Result<(), Fault>> {
    let bad = || Fault::new("invalid_value", path);
    let switch = |slot: &mut bool| value.as_bool().map(|on| *slot = on).ok_or_else(bad);
    let choice = |slot: &mut u32, choices: &[u32]| {
        value
            .as_u64()
            .and_then(|n| u32::try_from(n).ok())
            .filter(|n| choices.contains(n))
            .map(|n| *slot = n)
            .ok_or_else(bad)
    };
    Some(match path {
        "enabled" => switch(&mut config.enabled),
        "cover" => switch(&mut config.cover),
        "sound" => switch(&mut config.sound),
        "hold_meetings" => switch(&mut config.hold_meetings),
        "hold_full_screen" => switch(&mut config.hold_full_screen),
        "work_minutes" => choice(&mut config.work_minutes, &logic::WORK_MINUTES),
        "break_seconds" => choice(&mut config.break_seconds, &logic::BREAK_SECONDS),
        "blink_minutes" => choice(&mut config.blink_minutes, &logic::NUDGE_MINUTES),
        "posture_minutes" => choice(&mut config.posture_minutes, &logic::NUDGE_MINUTES),
        _ => return None,
    })
}

/// This feature's shortcut as (target, shortcut): it starts a break, or ends the one that runs.
pub fn shortcuts(config: &Config) -> Vec<(String, String)> {
    if config.shortcut.is_empty() {
        Vec::new()
    } else {
        vec![("breaks:now".to_owned(), config.shortcut.clone())]
    }
}

/// Stores a shortcut that was already parsed and checked for conflicts; an empty value removes it.
pub fn set_shortcut(config: &mut Config, target: &str, value: &str) -> Option<Result<(), Fault>> {
    let value = value.trim().to_owned();
    if !value.is_empty() && parse_hotkey(&value).is_err() {
        return Some(Err(Fault::new("invalid_shortcut", target)));
    }
    match target.strip_prefix("breaks:")? {
        "now" => config.shortcut = value,
        _ => return None,
    }
    Some(Ok(()))
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
            serde_json::from_str(include_str!("../../../web/features/breaks.fixture.json"))
                .unwrap();
        assert_eq!(
            keys(&fixture),
            keys(&state(&Config::default(), &Value::Null))
        );
    }

    #[test]
    fn the_state_starts_off_and_says_what_the_running_feature_knows() {
        let s = state(&Config::default(), &Value::Null);
        assert_eq!(s["enabled"], json!(false));
        assert_eq!(
            s["until"],
            Value::Null,
            "nothing is known until the app says"
        );
        let s = state(
            &Config::default(),
            &json!({ "until": 300, "breaking": true }),
        );
        assert_eq!(
            (s["until"].clone(), s["breaking"].clone()),
            (json!(300), json!(true))
        );
        let hostile = Config {
            work_minutes: 7,
            break_seconds: 0,
            blink_minutes: 99,
            ..Config::default()
        };
        let s = state(&hostile, &Value::Null);
        assert_eq!(
            (
                s["work_minutes"].clone(),
                s["break_seconds"].clone(),
                s["blink_minutes"].clone()
            ),
            (json!(20), json!(20), json!(0))
        );
    }

    #[test]
    fn switches_and_lengths_are_validated() {
        let mut config = Config::default();
        for path in [
            "enabled",
            "cover",
            "sound",
            "hold_meetings",
            "hold_full_screen",
        ] {
            assert!(
                set(&mut config, path, &json!("yes")).unwrap().is_err(),
                "{path}"
            );
            assert!(
                set(&mut config, path, &json!(true)).unwrap().is_ok(),
                "{path}"
            );
        }
        assert!(
            set(&mut config, "work_minutes", &json!(45))
                .unwrap()
                .is_ok()
        );
        assert_eq!(config.work_minutes, 45);
        for bad in [json!(25), json!(-20), json!("20"), json!(null), json!(1e12)] {
            assert!(
                set(&mut config, "work_minutes", &bad).unwrap().is_err(),
                "{bad}"
            );
        }
        assert_eq!(config.work_minutes, 45, "a refused value changes nothing");
        assert!(
            set(&mut config, "break_seconds", &json!(300))
                .unwrap()
                .is_ok()
        );
        assert!(
            set(&mut config, "break_seconds", &json!(30))
                .unwrap()
                .is_err()
        );
        assert!(
            set(&mut config, "blink_minutes", &json!(10))
                .unwrap()
                .is_ok()
        );
        assert!(
            set(&mut config, "posture_minutes", &json!(0))
                .unwrap()
                .is_ok()
        );
        assert!(
            set(&mut config, "posture_minutes", &json!(15))
                .unwrap()
                .is_err()
        );
        assert!(set(&mut config, "nonsense", &json!(1)).is_none());
    }

    #[test]
    fn the_shortcut_is_stored_removed_and_refused_when_it_is_nonsense() {
        let mut config = Config::default();
        assert!(
            shortcuts(&config).is_empty(),
            "no shortcut until one is chosen"
        );
        assert!(
            set_shortcut(&mut config, "breaks:now", "ctrl+opt+cmd+e")
                .unwrap()
                .is_ok()
        );
        assert_eq!(
            shortcuts(&config),
            [("breaks:now".to_owned(), "ctrl+opt+cmd+e".to_owned())]
        );
        assert!(
            set_shortcut(&mut config, "breaks:now", "nonsense")
                .unwrap()
                .is_err()
        );
        assert!(set_shortcut(&mut config, "breaks:now", "").unwrap().is_ok());
        assert!(shortcuts(&config).is_empty());
        assert!(set_shortcut(&mut config, "breaks:other", "ctrl+opt+cmd+e").is_none());
        assert!(set_shortcut(&mut config, "pomodoro:toggle", "ctrl+opt+cmd+e").is_none());
    }
}
