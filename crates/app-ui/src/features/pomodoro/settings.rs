//! This feature's part of the settings model: the JSON the page renders and the changes it may make.
//! Pure data with no AppKit, so every rule is covered by plain unit tests.

use super::{
    Config,
    logic::{LONG_BREAK_MINUTES, ROUNDS, SHORT_BREAK_MINUTES, WORK_MINUTES},
};
use crate::{config::parse_hotkey, settings::model::Fault};
use serde_json::{Value, json};

/// The state under the key "pomodoro". `runtime` is `Feature::runtime()`, or null when the feature is not running.
/// `web/features/pomodoro.fixture.json` must have exactly the same keys; a unit test checks it.
pub fn state(config: &Config, runtime: &Value) -> Value {
    let config = config.clone().sanitized();
    let number = |key: &str| runtime.get(key).and_then(Value::as_u64).unwrap_or(0);
    json!({
        "enabled": config.enabled,
        "work": config.work,
        "short_break": config.short_break,
        "long_break": config.long_break,
        "rounds": config.rounds,
        "auto_start": config.auto_start,
        "sound": config.sound,
        "open_on_end": config.open_on_end,
        "closed_notch": config.closed_notch,
        "shortcuts": { "toggle": config.shortcut },
        "today": number("today"),
        "streak": number("streak"),
        "days": number("days"),
    })
}

/// A change from the page. `path` is relative to the feature. Values must be validated here; the page is not
/// trusted.
pub fn set(config: &mut Config, path: &str, value: &Value) -> Option<Result<(), Fault>> {
    let bad = || Fault::new("invalid_value", path);
    let switch = |slot: &mut bool| value.as_bool().map(|on| *slot = on).ok_or_else(bad);
    let choice = |slot: &mut u32, list: &[u32]| {
        value
            .as_u64()
            .and_then(|n| u32::try_from(n).ok())
            .filter(|n| list.contains(n))
            .map(|n| *slot = n)
            .ok_or_else(bad)
    };
    Some(match path {
        "enabled" => switch(&mut config.enabled),
        "auto_start" => switch(&mut config.auto_start),
        "sound" => switch(&mut config.sound),
        "open_on_end" => switch(&mut config.open_on_end),
        "closed_notch" => switch(&mut config.closed_notch),
        "work" => choice(&mut config.work, &WORK_MINUTES),
        "short_break" => choice(&mut config.short_break, &SHORT_BREAK_MINUTES),
        "long_break" => choice(&mut config.long_break, &LONG_BREAK_MINUTES),
        "rounds" => choice(&mut config.rounds, &ROUNDS),
        _ => return None,
    })
}

/// This feature's shortcuts as (target, shortcut).
pub fn shortcuts(config: &Config) -> Vec<(String, String)> {
    if config.shortcut.is_empty() {
        Vec::new()
    } else {
        vec![("pomodoro:toggle".to_owned(), config.shortcut.clone())]
    }
}

/// Stores a shortcut that was already parsed and checked for conflicts; an empty value removes it.
pub fn set_shortcut(config: &mut Config, target: &str, value: &str) -> Option<Result<(), Fault>> {
    let value = value.trim().to_owned();
    if !value.is_empty() && parse_hotkey(&value).is_err() {
        return Some(Err(Fault::new("invalid_shortcut", target)));
    }
    match target.strip_prefix("pomodoro:")? {
        "toggle" => config.shortcut = value,
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
            serde_json::from_str(include_str!("../../../web/features/pomodoro.fixture.json"))
                .unwrap();
        let state = state(&Config::default(), &Value::Null);
        assert_eq!(keys(&fixture), keys(&state));
        assert_eq!(keys(&fixture["shortcuts"]), keys(&state["shortcuts"]));
    }

    #[test]
    fn the_state_starts_off_with_the_usual_lengths_and_says_what_the_running_feature_knows() {
        let s = state(&Config::default(), &Value::Null);
        assert_eq!(s["enabled"], json!(false));
        assert_eq!(
            (
                s["work"].clone(),
                s["short_break"].clone(),
                s["long_break"].clone(),
                s["rounds"].clone()
            ),
            (json!(25), json!(5), json!(15), json!(4))
        );
        assert_eq!(
            (
                s["auto_start"].clone(),
                s["sound"].clone(),
                s["open_on_end"].clone(),
                s["closed_notch"].clone()
            ),
            (json!(false), json!(true), json!(true), json!(true))
        );
        let s = state(
            &Config::default(),
            &json!({ "today": 3, "streak": 5, "days": 12 }),
        );
        assert_eq!(
            (s["today"].clone(), s["streak"].clone(), s["days"].clone()),
            (json!(3), json!(5), json!(12))
        );
        let hostile = Config {
            work: 7,
            rounds: 99,
            shortcut: "nonsense".into(),
            ..Config::default()
        };
        let s = state(&hostile, &Value::Null);
        assert_eq!(
            (
                s["work"].clone(),
                s["rounds"].clone(),
                s["shortcuts"]["toggle"].clone()
            ),
            (json!(25), json!(4), json!(""))
        );
    }

    #[test]
    fn the_switches_are_validated() {
        let mut config = Config::default();
        for path in [
            "enabled",
            "auto_start",
            "sound",
            "open_on_end",
            "closed_notch",
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
                set(&mut config, path, &json!(true)).unwrap().is_ok(),
                "{path}"
            );
        }
        assert!(
            config.enabled
                && config.auto_start
                && config.sound
                && config.open_on_end
                && config.closed_notch
        );
        assert!(set(&mut config, "nonsense", &json!(1)).is_none());
    }

    #[test]
    fn a_length_is_taken_only_from_what_is_offered() {
        let mut config = Config::default();
        for (path, list) in [
            ("work", &WORK_MINUTES[..]),
            ("short_break", &SHORT_BREAK_MINUTES[..]),
            ("long_break", &LONG_BREAK_MINUTES[..]),
            ("rounds", &ROUNDS[..]),
        ] {
            for &offered in list {
                assert!(
                    set(&mut config, path, &json!(offered)).unwrap().is_ok(),
                    "{path} {offered}"
                );
            }
            let kept = *list.last().unwrap();
            for bad in [
                json!(0),
                json!(-1),
                json!(7_777),
                json!(2.5),
                json!("25"),
                json!(null),
                json!(1e12),
            ] {
                assert!(
                    set(&mut config, path, &bad).unwrap().is_err(),
                    "{path} {bad}"
                );
            }
            let _ = kept;
        }
        assert_eq!(
            (
                config.work,
                config.short_break,
                config.long_break,
                config.rounds
            ),
            (60, 15, 30, 6),
            "a refused value changes nothing"
        );
    }

    #[test]
    fn the_shortcut_is_stored_removed_and_refused_when_it_is_nonsense() {
        let mut config = Config::default();
        assert!(shortcuts(&config).is_empty(), "none by default");
        assert!(
            set_shortcut(&mut config, "pomodoro:toggle", "ctrl+opt+cmd+p")
                .unwrap()
                .is_ok()
        );
        assert_eq!(
            shortcuts(&config),
            [("pomodoro:toggle".to_owned(), "ctrl+opt+cmd+p".to_owned())]
        );
        assert!(
            set_shortcut(&mut config, "pomodoro:toggle", "nonsense")
                .unwrap()
                .is_err()
        );
        assert_eq!(config.shortcut, "ctrl+opt+cmd+p");
        assert!(
            set_shortcut(&mut config, "pomodoro:toggle", "")
                .unwrap()
                .is_ok()
                && config.shortcut.is_empty()
        );
        assert!(set_shortcut(&mut config, "pomodoro:other", "ctrl+opt+cmd+x").is_none());
        assert!(set_shortcut(&mut config, "pin", "ctrl+opt+cmd+x").is_none());
    }
}
