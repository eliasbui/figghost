//! This feature's part of the settings model: the JSON the page renders and the changes it may make.
//! Pure data with no AppKit, so every rule is covered by plain unit tests.

use super::{Config, SILENCE_SECONDS, Spoken};
use crate::{config::parse_hotkey, settings::model::Fault};
use serde_json::{Value, json};

/// The state under the key "dictation". `runtime` is `Feature::runtime()`, or null when the feature is not running.
/// `web/features/dictation.fixture.json` must have exactly the same keys; a unit test checks it.
pub fn state(config: &Config, runtime: &Value) -> Value {
    let config = config.clone().sanitized();
    let model = |code: &str| {
        let m = &runtime["models"][code];
        json!({
            "available": m["available"].as_bool().unwrap_or(false),
            "on_device": m["on_device"].as_bool().unwrap_or(false),
        })
    };
    json!({
        "enabled": config.enabled,
        "shortcut": config.shortcut,
        "language": match config.language {
            Spoken::Auto => "auto",
            Spoken::Vi => "vi",
            Spoken::En => "en",
        },
        "on_device": config.on_device,
        "silence_seconds": config.silence_seconds,
        "sound": config.sound,
        "listening": runtime["listening"].as_bool().unwrap_or(false),
        "speech": runtime["speech"].as_str().unwrap_or("ask"),
        "microphone": runtime["microphone"].as_bool().unwrap_or(false),
        "accessibility": runtime["accessibility"].as_bool().unwrap_or(false),
        "models": { "vi": model("vi"), "en": model("en") },
    })
}

/// A change from the page. `path` is relative to the feature. Values must be validated here; the page is not
/// trusted.
pub fn set(config: &mut Config, path: &str, value: &Value) -> Option<Result<(), Fault>> {
    let bad = || Fault::new("invalid_value", path);
    let switch = |slot: &mut bool| value.as_bool().map(|on| *slot = on).ok_or_else(bad);
    Some(match path {
        "enabled" => switch(&mut config.enabled),
        "on_device" => switch(&mut config.on_device),
        "sound" => switch(&mut config.sound),
        "language" => match value.as_str() {
            Some("auto") => Some(Spoken::Auto),
            Some("vi") => Some(Spoken::Vi),
            Some("en") => Some(Spoken::En),
            _ => None,
        }
        .map(|spoken| config.language = spoken)
        .ok_or_else(bad),
        "silence_seconds" => value
            .as_u64()
            .and_then(|n| u32::try_from(n).ok())
            .filter(|n| SILENCE_SECONDS.contains(n))
            .map(|n| config.silence_seconds = n)
            .ok_or_else(bad),
        _ => return None,
    })
}

/// This feature's shortcut as (target, shortcut): it opens the microphone, and puts the words in.
pub fn shortcuts(config: &Config) -> Vec<(String, String)> {
    if config.shortcut.is_empty() {
        Vec::new()
    } else {
        vec![("dictation".to_owned(), config.shortcut.clone())]
    }
}

/// Stores a shortcut that was already parsed and checked for conflicts; an empty value removes it.
pub fn set_shortcut(config: &mut Config, target: &str, value: &str) -> Option<Result<(), Fault>> {
    let value = value.trim().to_owned();
    if !value.is_empty() && parse_hotkey(&value).is_err() {
        return Some(Err(Fault::new("invalid_shortcut", target)));
    }
    if target != "dictation" {
        return None;
    }
    config.shortcut = value;
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
            serde_json::from_str(include_str!("../../../web/features/dictation.fixture.json"))
                .unwrap();
        let state = state(&Config::default(), &Value::Null);
        assert_eq!(keys(&fixture), keys(&state));
        assert_eq!(keys(&fixture["models"]), keys(&state["models"]));
        assert_eq!(keys(&fixture["models"]["vi"]), keys(&state["models"]["vi"]));
    }

    #[test]
    fn the_state_starts_off_and_says_what_the_running_feature_knows() {
        let s = state(&Config::default(), &Value::Null);
        assert_eq!(s["enabled"], json!(false));
        assert_eq!(s["language"], json!("auto"));
        assert_eq!(s["speech"], json!("ask"));
        assert_eq!(s["models"]["vi"]["on_device"], json!(false));
        let runtime = json!({
            "listening": true, "speech": "allowed", "microphone": true, "accessibility": true,
            "models": { "vi": { "available": true, "on_device": true }, "en": { "available": true, "on_device": false } },
        });
        let s = state(&Config::default(), &runtime);
        assert_eq!(s["listening"], json!(true));
        assert_eq!(s["speech"], json!("allowed"));
        assert_eq!(s["microphone"], json!(true));
        assert_eq!(s["models"]["vi"]["on_device"], json!(true));
        assert_eq!(s["models"]["en"]["on_device"], json!(false));
    }

    #[test]
    fn the_choices_are_validated() {
        let mut config = Config::default();
        assert!(set(&mut config, "enabled", &json!("yes")).unwrap().is_err());
        assert!(set(&mut config, "enabled", &json!(true)).unwrap().is_ok() && config.enabled);
        assert!(
            set(&mut config, "on_device", &json!(false))
                .unwrap()
                .is_ok()
                && !config.on_device
        );
        assert!(set(&mut config, "language", &json!("vi")).unwrap().is_ok());
        assert_eq!(config.language, Spoken::Vi);
        assert!(
            set(&mut config, "language", &json!("fr")).unwrap().is_err(),
            "there is no French model here"
        );
        assert!(
            set(&mut config, "silence_seconds", &json!(5))
                .unwrap()
                .is_ok()
        );
        assert_eq!(config.silence_seconds, 5);
        assert!(
            set(&mut config, "silence_seconds", &json!(4))
                .unwrap()
                .is_err()
        );
        assert!(
            set(&mut config, "silence_seconds", &json!(-1))
                .unwrap()
                .is_err()
        );
        assert!(set(&mut config, "nonsense", &json!(1)).is_none());
    }

    #[test]
    fn the_shortcut_is_stored_removed_and_refused() {
        let mut config = Config::default();
        assert_eq!(
            shortcuts(&config),
            [("dictation".to_owned(), "ctrl+opt+v".to_owned())]
        );
        assert!(set_shortcut(&mut config, "dictation", "cmd+shift+d").unwrap().is_ok());
        assert_eq!(config.shortcut, "cmd+shift+d");
        assert!(set_shortcut(&mut config, "dictation", "nonsense").unwrap().is_err());
        assert!(set_shortcut(&mut config, "dictation", "").unwrap().is_ok());
        assert!(shortcuts(&config).is_empty());
        assert!(set_shortcut(&mut config, "dictation:other", "ctrl+opt+v").is_none());
    }

}
