//! This feature's part of the settings model: the JSON the page renders and the changes it may make.
//! Pure data with no AppKit, so every rule is covered by plain unit tests.

use super::Config;
use crate::settings::model::Fault;
use serde_json::{Value, json};

/// The state under the key "pin". `runtime` is `Feature::runtime()`, or null when the feature is not running.
/// `web/features/pin.fixture.json` must have exactly the same keys; a unit test checks it.
pub fn state(config: &Config, runtime: &Value) -> Value {
    let fact = |key: &str| runtime.get(key).and_then(Value::as_bool).unwrap_or(false);
    json!({
        "enabled": config.enabled,
        "shortcut": config.shortcut,
        "accessibility": fact("accessibility"),
        "screen_access": fact("screen_access"),
        "pinned": runtime.get("pinned").and_then(Value::as_str),
    })
}

/// A change from the page. `path` is relative to the feature. Values must be validated here; the page is not
/// trusted.
pub fn set(config: &mut Config, path: &str, value: &Value) -> Option<Result<(), Fault>> {
    match path {
        "enabled" => Some(
            value
                .as_bool()
                .map(|on| config.enabled = on)
                .ok_or_else(|| Fault::new("invalid_value", path)),
        ),
        _ => None,
    }
}

/// This feature's shortcuts as (target, shortcut).
pub fn shortcuts(config: &Config) -> Vec<(String, String)> {
    if config.shortcut.is_empty() {
        return Vec::new();
    }
    vec![("pin".into(), config.shortcut.clone())]
}

/// Stores a shortcut that was already parsed and checked for conflicts; an empty value removes it, and the
/// feature can then only be reached from the menu bar panel.
pub fn set_shortcut(config: &mut Config, target: &str, value: &str) -> Option<Result<(), Fault>> {
    (target == "pin").then(|| {
        config.shortcut = value.trim().to_owned();
        Ok(())
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::features::pin::DEFAULT_SHORTCUT;

    fn keys(value: &Value) -> Vec<String> {
        let mut keys: Vec<String> = value.as_object().unwrap().keys().cloned().collect();
        keys.sort();
        keys
    }

    #[test]
    fn the_fixture_has_exactly_the_keys_of_the_state() {
        let fixture: Value =
            serde_json::from_str(include_str!("../../../web/features/pin.fixture.json")).unwrap();
        assert_eq!(
            keys(&fixture),
            keys(&state(&Config::default(), &Value::Null))
        );
    }

    #[test]
    fn the_feature_is_off_with_a_usable_shortcut_until_asked() {
        let config = Config::default();
        assert!(!config.enabled);
        assert_eq!(config.shortcut, DEFAULT_SHORTCUT);
        assert!(crate::config::parse_hotkey(DEFAULT_SHORTCUT).is_ok());
        let s = state(&config, &Value::Null);
        assert_eq!(
            (
                s["accessibility"].clone(),
                s["screen_access"].clone(),
                s["pinned"].clone()
            ),
            (json!(false), json!(false), Value::Null),
            "nothing is known until the app says"
        );
    }

    #[test]
    fn runtime_facts_reach_the_page_and_a_wrong_type_is_not_trusted() {
        let runtime =
            json!({ "accessibility": true, "screen_access": true, "pinned": "Notes - Todo" });
        let s = state(&Config::default(), &runtime);
        assert_eq!(
            (
                s["accessibility"].clone(),
                s["screen_access"].clone(),
                s["pinned"].clone()
            ),
            (json!(true), json!(true), json!("Notes - Todo"))
        );
        let s = state(
            &Config::default(),
            &json!({ "accessibility": "yes", "pinned": 3 }),
        );
        assert_eq!(
            (s["accessibility"].clone(), s["pinned"].clone()),
            (json!(false), Value::Null)
        );
    }

    #[test]
    fn the_switch_is_validated_and_the_shortcut_can_be_removed() {
        let mut config = Config::default();
        assert!(set(&mut config, "enabled", &json!(true)).unwrap().is_ok() && config.enabled);
        for bad in [json!("yes"), json!(1), json!(null)] {
            assert!(set(&mut config, "enabled", &bad).unwrap().is_err(), "{bad}");
        }
        assert!(config.enabled, "a refused change leaves the setting alone");
        assert!(set(&mut config, "nonsense", &json!(true)).is_none());
        assert!(set_shortcut(&mut config, "other", "ctrl+opt+x").is_none());
        assert!(set_shortcut(&mut config, "pin", " ").unwrap().is_ok());
        assert!(
            shortcuts(&config).is_empty(),
            "a removed shortcut is not listed"
        );
        assert!(
            set_shortcut(&mut config, "pin", "ctrl+opt+x")
                .unwrap()
                .is_ok()
        );
        assert_eq!(
            shortcuts(&config),
            vec![("pin".to_owned(), "ctrl+opt+x".to_owned())]
        );
    }
}
