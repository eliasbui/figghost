//! This feature's part of the settings model: the JSON the page renders and the changes it may make.
//! Pure data with no AppKit, so every rule is covered by plain unit tests.

use super::{Config, Provider, Spoken, codex_status, forget_key, has_key, store_key, valid_model};
use crate::{config::parse_hotkey, settings::model::Fault};
use serde_json::{Value, json};

/// The state under the key "jarvis". `runtime` is `Feature::runtime()`, or null when the feature is not running.
/// `web/features/jarvis.fixture.json` must have exactly the same keys; a unit test checks it.
pub fn state(config: &Config, runtime: &Value) -> Value {
    let config = config.clone().sanitized();
    json!({
        "enabled": config.enabled,
        "shortcut": config.shortcut,
        "provider": config.provider.id(),
        "model": config.model,
        "default_model": config.provider.default_model(),
        "needs_key": config.provider.needs_key(),
        "key": has_key(config.provider),
        // Only the provider that has one is looked at, so that a sign-in file is never read for nothing.
        "signin": if config.provider == Provider::Codex { codex_status() } else { "" },
        "voice": config.voice,
        "language": match config.language {
            Spoken::Auto => "auto",
            Spoken::Vi => "vi",
            Spoken::En => "en",
        },
        "on_device": config.on_device,
        "speak": config.speak,
        "sound": config.sound,
        "speech": runtime["speech"].as_str().unwrap_or("ask"),
        "microphone": runtime["microphone"].as_bool().unwrap_or(false),
    })
}

/// A change from the page. `path` is relative to the feature. Values must be validated here; the page is not
/// trusted. The key never goes into the config file: it goes to the Keychain.
pub fn set(config: &mut Config, path: &str, value: &Value) -> Option<Result<(), Fault>> {
    let bad = || Fault::new("invalid_value", path);
    Some(match path {
        "enabled" => value.as_bool().map(|on| config.enabled = on).ok_or_else(bad),
        "voice" => value.as_bool().map(|on| config.voice = on).ok_or_else(bad),
        "on_device" => value.as_bool().map(|on| config.on_device = on).ok_or_else(bad),
        "speak" => value.as_bool().map(|on| config.speak = on).ok_or_else(bad),
        "sound" => value.as_bool().map(|on| config.sound = on).ok_or_else(bad),
        "language" => match value.as_str() {
            Some("auto") => Some(Spoken::Auto),
            Some("vi") => Some(Spoken::Vi),
            Some("en") => Some(Spoken::En),
            _ => None,
        }
        .map(|spoken| config.language = spoken)
        .ok_or_else(bad),
        "provider" => value
            .as_str()
            .and_then(Provider::from_id)
            .map(|p| config.provider = p)
            .ok_or_else(bad),
        "model" => value
            .as_str()
            .map(str::trim)
            .filter(|m| valid_model(m))
            .map(|m| config.model = m.to_owned())
            .ok_or_else(bad),
        "key" => match value.as_str().map(str::trim) {
            Some("") => forget_key(config.provider).map_err(|_| Fault::new("keychain", path)),
            Some(key) if key.len() <= 400 && key.chars().all(|c| c.is_ascii_graphic()) => {
                store_key(config.provider, key).map_err(|_| Fault::new("keychain", path))
            }
            _ => Err(bad()),
        },
        _ => return None,
    })
}

/// This feature's shortcut as (target, shortcut): it opens the panel and closes it.
pub fn shortcuts(config: &Config) -> Vec<(String, String)> {
    if config.shortcut.is_empty() {
        Vec::new()
    } else {
        vec![("jarvis".to_owned(), config.shortcut.clone())]
    }
}

/// Stores a shortcut that was already parsed and checked for conflicts; an empty value removes it.
pub fn set_shortcut(config: &mut Config, target: &str, value: &str) -> Option<Result<(), Fault>> {
    let value = value.trim().to_owned();
    if !value.is_empty() && parse_hotkey(&value).is_err() {
        return Some(Err(Fault::new("invalid_shortcut", target)));
    }
    if target != "jarvis" {
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
            serde_json::from_str(include_str!("../../../web/features/jarvis.fixture.json")).unwrap();
        assert_eq!(keys(&fixture), keys(&state(&Config::default(), &Value::Null)));
    }

    #[test]
    fn it_starts_off_with_a_provider_that_needs_a_key() {
        let s = state(&Config::default(), &Value::Null);
        assert_eq!(s["enabled"], json!(false));
        assert_eq!(s["provider"], json!("anthropic"));
        assert_eq!(s["needs_key"], json!(true));
        assert_eq!(s["signin"], json!(""), "no sign-in file is read for a provider that has none");
        assert_eq!(s["model"], json!(""));
        assert_ne!(s["default_model"], json!(""));
        assert_eq!(s["voice"], json!(true), "it listens and speaks unless told not to");
        assert_eq!(s["speak"], json!(true));
        assert_eq!(s["language"], json!("auto"));
        assert_eq!(s["speech"], json!("ask"));
        let runtime = json!({ "speech": "allowed", "microphone": true });
        let s = state(&Config::default(), &runtime);
        assert_eq!(s["speech"], json!("allowed"));
        assert_eq!(s["microphone"], json!(true));
    }

    #[test]
    fn the_voice_choices_are_validated() {
        let mut config = Config::default();
        for path in ["voice", "speak", "sound", "on_device"] {
            assert!(set(&mut config, path, &json!("yes")).unwrap().is_err(), "{path} is a switch");
            assert!(set(&mut config, path, &json!(false)).unwrap().is_ok(), "{path}");
        }
        assert!(!config.voice && !config.speak && !config.sound && !config.on_device);
        assert!(set(&mut config, "language", &json!("vi")).unwrap().is_ok());
        assert_eq!(config.language, Spoken::Vi);
        assert!(set(&mut config, "language", &json!("fr")).unwrap().is_err());
    }

    #[test]
    fn the_choices_are_validated() {
        let mut config = Config::default();
        assert!(set(&mut config, "enabled", &json!("yes")).unwrap().is_err());
        assert!(set(&mut config, "enabled", &json!(true)).unwrap().is_ok() && config.enabled);
        assert!(set(&mut config, "provider", &json!("local")).unwrap().is_ok());
        assert_eq!(config.provider, Provider::Local);
        assert!(set(&mut config, "provider", &json!("bing")).unwrap().is_err());
        assert!(set(&mut config, "model", &json!("  llama3.2:3b ")).unwrap().is_ok());
        assert_eq!(config.model, "llama3.2:3b");
        assert!(set(&mut config, "model", &json!("a model; rm -rf")).unwrap().is_err());
        assert!(set(&mut config, "model", &json!("")).unwrap().is_ok(), "empty means the provider's own");
        assert!(set(&mut config, "key", &json!("has a space")).unwrap().is_err());
        assert!(set(&mut config, "key", &json!(3)).unwrap().is_err());
        assert!(set(&mut config, "nonsense", &json!(1)).is_none());
    }

    #[test]
    fn the_key_goes_to_the_keychain_and_never_into_the_config() {
        let mut config = Config { provider: Provider::DeepSeek, ..Config::default() };
        let secret = "sk-test-0123456789";
        assert!(set(&mut config, "key", &json!(secret)).unwrap().is_ok());
        assert!(has_key(Provider::DeepSeek));
        assert!(!serde_json::to_string(&config).unwrap().contains(secret));
        assert!(!state(&config, &Value::Null).to_string().contains(secret), "the page only learns that there is one");
        assert!(set(&mut config, "key", &json!("")).unwrap().is_ok());
        assert!(!has_key(Provider::DeepSeek));
    }

    #[test]
    fn the_shortcut_is_stored_removed_and_refused() {
        let mut config = Config::default();
        assert_eq!(shortcuts(&config), [("jarvis".to_owned(), "ctrl+opt+space".to_owned())]);
        assert!(set_shortcut(&mut config, "jarvis", "cmd+shift+j").unwrap().is_ok());
        assert_eq!(config.shortcut, "cmd+shift+j");
        assert!(set_shortcut(&mut config, "jarvis", "nonsense").unwrap().is_err());
        assert!(set_shortcut(&mut config, "jarvis", "").unwrap().is_ok());
        assert!(shortcuts(&config).is_empty());
        assert!(set_shortcut(&mut config, "jarvis:other", "ctrl+opt+space").is_none());
    }
}
