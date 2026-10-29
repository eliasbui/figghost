//! This feature's part of the settings model: the JSON the page renders and the changes it may make.
//! Pure data with no AppKit, so every rule is covered by plain unit tests.

use super::{Config, logic};
use crate::{config::parse_hotkey, settings::model::Fault};
use serde_json::{Value, json};

/// The state under the key "utilities". `runtime` is `Feature::runtime()`, or null when the feature is not
/// running. `web/features/utilities.fixture.json` must have exactly the same keys; a unit test checks it.
pub fn state(config: &Config, runtime: &Value) -> Value {
    let fact = |key: &str| runtime.get(key).and_then(Value::as_bool).unwrap_or(false);
    json!({
        "enabled": config.enabled,
        "awake_display": config.awake_display,
        "awake_minutes": config.awake_minutes,
        "lock_seconds": config.lock_seconds,
        "snooze_minutes": config.snooze_minutes,
        "shortcuts": {
            "awake": config.awake_shortcut,
            "mic": config.mic_shortcut,
            "lock": config.lock_shortcut,
            "snooze": config.snooze_shortcut,
            "dark": config.dark_shortcut,
            "desktop": config.desktop_shortcut,
            "link": config.link_shortcut,
        },
        "link_format": config.link_format,
        "awake": fact("awake"),
        "awake_left": runtime.get("awake_left").and_then(Value::as_u64),
        "locked": fact("locked"),
        "snoozed": fact("snoozed"),
        "snooze_left": runtime.get("snooze_left").and_then(Value::as_u64),
        "mic_muted": runtime.get("mic_muted").and_then(Value::as_bool),
        "accessibility": fact("accessibility"),
        "dark": fact("dark"),
        "desktop_icons": runtime.get("desktop_icons").and_then(Value::as_bool).unwrap_or(true),
    })
}

/// A change from the page. `path` is relative to the feature. Values must be validated here; the page is not
/// trusted.
pub fn set(config: &mut Config, path: &str, value: &Value) -> Option<Result<(), Fault>> {
    let bad = || Fault::new("invalid_value", path);
    Some(match path {
        "enabled" => value
            .as_bool()
            .map(|on| config.enabled = on)
            .ok_or_else(bad),
        "awake_display" => value
            .as_bool()
            .map(|on| config.awake_display = on)
            .ok_or_else(bad),
        "awake_minutes" => value
            .as_u64()
            .and_then(|m| u32::try_from(m).ok())
            .map(|m| config.awake_minutes = logic::awake_minutes(m))
            .ok_or_else(bad),
        "snooze_minutes" => value
            .as_u64()
            .and_then(|m| u32::try_from(m).ok())
            .map(|m| config.snooze_minutes = logic::snooze_minutes(m))
            .ok_or_else(bad),
        "link_format" => value
            .as_str()
            .filter(|f| logic::LINK_FORMATS.contains(f))
            .map(|f| config.link_format = f.to_owned())
            .ok_or_else(bad),
        "lock_seconds" => value
            .as_f64()
            .filter(|n| n.is_finite())
            .map(|n| {
                config.lock_seconds = (n.round() as i64)
                    .clamp(logic::LOCK_SECONDS.0.into(), logic::LOCK_SECONDS.1.into())
                    as u32;
            })
            .ok_or_else(bad),
        _ => return None,
    })
}

/// This feature's shortcuts as (target, shortcut).
pub fn shortcuts(config: &Config) -> Vec<(String, String)> {
    [
        ("utilities:awake", &config.awake_shortcut),
        ("utilities:mic", &config.mic_shortcut),
        ("utilities:lock", &config.lock_shortcut),
        ("utilities:snooze", &config.snooze_shortcut),
        ("utilities:dark", &config.dark_shortcut),
        ("utilities:desktop", &config.desktop_shortcut),
        ("utilities:link", &config.link_shortcut),
    ]
    .into_iter()
    .filter(|(_, shortcut)| !shortcut.is_empty())
    .map(|(target, shortcut)| (target.to_owned(), shortcut.clone()))
    .collect()
}

/// Stores a shortcut that was already parsed and checked for conflicts; an empty value removes it.
pub fn set_shortcut(config: &mut Config, target: &str, value: &str) -> Option<Result<(), Fault>> {
    let value = value.trim().to_owned();
    if !value.is_empty() && parse_hotkey(&value).is_err() {
        return Some(Err(Fault::new("invalid_shortcut", target)));
    }
    match target.strip_prefix("utilities:")? {
        "awake" => config.awake_shortcut = value,
        "mic" => config.mic_shortcut = value,
        "lock" => config.lock_shortcut = value,
        "snooze" => config.snooze_shortcut = value,
        "dark" => config.dark_shortcut = value,
        "desktop" => config.desktop_shortcut = value,
        "link" => config.link_shortcut = value,
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
            serde_json::from_str(include_str!("../../../web/features/utilities.fixture.json"))
                .unwrap();
        let state = state(&Config::default(), &Value::Null);
        assert_eq!(keys(&fixture), keys(&state));
        assert_eq!(keys(&fixture["shortcuts"]), keys(&state["shortcuts"]));
    }

    #[test]
    fn the_feature_is_off_until_asked() {
        let config = Config::default();
        assert!(!config.enabled);
        assert!(config.awake_display);
        let s = state(&config, &Value::Null);
        assert_eq!(s["awake"], json!(false));
        assert_eq!(
            s["mic_muted"],
            Value::Null,
            "nothing is known until the app says"
        );
    }

    #[test]
    fn lengths_are_held_to_what_is_offered() {
        let mut config = Config::default();
        set(&mut config, "awake_minutes", &json!(120))
            .unwrap()
            .unwrap();
        assert_eq!(config.awake_minutes, 120);
        set(&mut config, "awake_minutes", &json!(37))
            .unwrap()
            .unwrap();
        assert_eq!(
            config.awake_minutes, 60,
            "not offered: an hour, never for ever"
        );
        assert!(
            set(&mut config, "awake_minutes", &json!(-1))
                .unwrap()
                .is_err()
        );
        set(&mut config, "snooze_minutes", &json!(60))
            .unwrap()
            .unwrap();
        assert_eq!(config.snooze_minutes, 60);
        set(&mut config, "snooze_minutes", &json!(0))
            .unwrap()
            .unwrap();
        assert_eq!(config.snooze_minutes, 0, "until it is woken");
        set(&mut config, "snooze_minutes", &json!(37))
            .unwrap()
            .unwrap();
        assert_eq!(config.snooze_minutes, 30, "not offered: half an hour");
        assert!(
            set(&mut config, "snooze_minutes", &json!(-5))
                .unwrap()
                .is_err()
        );
        assert!(
            set(&mut config, "snooze_minutes", &json!("30"))
                .unwrap()
                .is_err()
        );
        set(&mut config, "lock_seconds", &json!(5))
            .unwrap()
            .unwrap();
        assert_eq!(config.lock_seconds, logic::LOCK_SECONDS.0);
        set(&mut config, "lock_seconds", &json!(99999))
            .unwrap()
            .unwrap();
        assert_eq!(config.lock_seconds, logic::LOCK_SECONDS.1);
        assert!(set(&mut config, "enabled", &json!("on")).unwrap().is_err());
        assert!(set(&mut config, "nonsense", &json!(1)).is_none());
        set(&mut config, "link_format", &json!("title_url"))
            .unwrap()
            .unwrap();
        assert_eq!(config.link_format, "title_url");
        assert!(
            set(&mut config, "link_format", &json!("html"))
                .unwrap()
                .is_err()
        );
        assert!(set(&mut config, "link_format", &json!(3)).unwrap().is_err());
        assert_eq!(
            config.link_format, "title_url",
            "a refused value changes nothing"
        );
    }

    #[test]
    fn a_shortcut_is_stored_removed_and_refused_when_it_is_nonsense() {
        let mut config = Config::default();
        assert!(
            set_shortcut(&mut config, "utilities:awake", "ctrl+opt+cmd+a")
                .unwrap()
                .is_ok()
        );
        assert_eq!(config.awake_shortcut, "ctrl+opt+cmd+a");
        assert!(
            shortcuts(&config)
                .iter()
                .any(|(t, _)| t == "utilities:awake")
        );
        assert!(
            set_shortcut(&mut config, "utilities:awake", "")
                .unwrap()
                .is_ok()
        );
        assert!(config.awake_shortcut.is_empty());
        assert!(
            set_shortcut(&mut config, "utilities:mic", "nonsense")
                .unwrap()
                .is_err()
        );
        assert!(
            set_shortcut(&mut config, "utilities:snooze", "ctrl+opt+cmd+z")
                .unwrap()
                .is_ok()
        );
        assert_eq!(config.snooze_shortcut, "ctrl+opt+cmd+z");
        assert!(
            shortcuts(&config)
                .iter()
                .any(|(t, _)| t == "utilities:snooze")
        );
        assert!(
            set_shortcut(&mut config, "utilities:link", "")
                .unwrap()
                .is_ok()
        );
        assert!(
            !shortcuts(&config)
                .iter()
                .any(|(t, _)| t == "utilities:link")
        );
        assert!(
            set_shortcut(&mut config, "utilities:dark", "ctrl+opt+cmd+d")
                .unwrap()
                .is_ok()
        );
        assert_eq!(config.dark_shortcut, "ctrl+opt+cmd+d");
        assert!(set_shortcut(&mut config, "utilities:other", "ctrl+opt+cmd+x").is_none());
        assert!(set_shortcut(&mut config, "pin", "ctrl+opt+cmd+x").is_none());
    }
}
