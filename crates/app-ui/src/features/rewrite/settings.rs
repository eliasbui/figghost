//! Rewrite's part of the settings model: the JSON the page renders and the changes it may make.

use super::{Config, text};
use crate::{config::parse_hotkey, settings::model::Fault};
use serde_json::{Value, json};

/// The state under the key "rewrite". `runtime` carries `accessibility` and what the model of Jarvis lacks.
/// `web/features/rewrite.fixture.json` must have exactly the same keys; a unit test checks it.
pub fn state(config: &Config, runtime: &Value) -> Value {
    let config = config.clone().sanitized();
    json!({
        "enabled": config.enabled,
        "shortcut": config.shortcut,
        "mode": config.mode,
        "replace": config.replace,
        "material": config.material,
        "accessibility": runtime.get("accessibility").and_then(Value::as_bool).unwrap_or(false),
        "model": runtime.get("model").filter(|v| v.is_object()).cloned().unwrap_or_else(|| json!({ "ready": false, "note": "" })),
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
        "replace" => value
            .as_bool()
            .map(|on| config.replace = on)
            .ok_or_else(bad),
        "mode" => value
            .as_str()
            .filter(|m| text::MODES.contains(m))
            .map(|m| config.mode = m.to_owned())
            .ok_or_else(bad),
        "material" => value
            .as_str()
            .filter(|m| super::material_of(m).is_some())
            .map(|m| config.material = m.to_owned())
            .ok_or_else(bad),
        _ => return None,
    })
}

pub fn shortcuts(config: &Config) -> Vec<(String, String)> {
    if config.shortcut.is_empty() {
        Vec::new()
    } else {
        vec![("rewrite".to_owned(), config.shortcut.clone())]
    }
}

pub fn set_shortcut(config: &mut Config, target: &str, value: &str) -> Option<Result<(), Fault>> {
    if target != "rewrite" {
        return None;
    }
    let value = value.trim().to_owned();
    if !value.is_empty() && parse_hotkey(&value).is_err() {
        return Some(Err(Fault::new("invalid_shortcut", target)));
    }
    // Without a shortcut the feature can still be reached from the menu.
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
            serde_json::from_str(include_str!("../../../web/features/rewrite.fixture.json"))
                .unwrap();
        let state = state(&Config::default(), &Value::Null);
        assert_eq!(keys(&fixture), keys(&state));
        assert_eq!(keys(&fixture["model"]), keys(&state["model"]));
    }

    #[test]
    fn only_what_is_offered_is_taken() {
        let mut config = Config::default();
        assert!(set(&mut config, "enabled", &json!("yes")).unwrap().is_err());
        assert!(set(&mut config, "enabled", &json!(true)).unwrap().is_ok() && config.enabled);
        assert!(set(&mut config, "replace", &json!(true)).unwrap().is_ok() && config.replace);
        for mode in text::MODES {
            assert!(
                set(&mut config, "mode", &json!(mode)).unwrap().is_ok(),
                "{mode}"
            );
        }
        assert!(set(&mut config, "mode", &json!("rude")).unwrap().is_err());
        assert_eq!(config.mode, "english", "a refused mode changes nothing");
        assert!(
            set(&mut config, "material", &json!("clear"))
                .unwrap()
                .is_ok()
        );
        assert!(
            set(&mut config, "material", &json!("mud"))
                .unwrap()
                .is_err()
        );
        assert!(set(&mut config, "nonsense", &json!(1)).is_none());
    }

    #[test]
    fn the_shortcut_is_checked_and_may_be_taken_away() {
        let mut config = Config::default();
        assert_eq!(shortcuts(&config).len(), 1);
        assert!(
            set_shortcut(&mut config, "rewrite", "cmd+shift+r")
                .unwrap()
                .is_ok()
        );
        assert_eq!(config.shortcut, "cmd+shift+r");
        assert!(
            set_shortcut(&mut config, "rewrite", "nonsense")
                .unwrap()
                .is_err()
        );
        assert_eq!(config.shortcut, "cmd+shift+r");
        assert!(
            set_shortcut(&mut config, "rewrite", "").unwrap().is_ok()
                && shortcuts(&config).is_empty()
        );
        assert!(set_shortcut(&mut config, "translate", "cmd+r").is_none());
    }

    #[test]
    fn what_the_model_lacks_is_told_to_the_page() {
        let s = state(
            &Config::default(),
            &json!({ "accessibility": true, "model": { "ready": false, "note": "No key." } }),
        );
        assert_eq!(
            (s["accessibility"].clone(), s["model"]["note"].clone()),
            (json!(true), json!("No key."))
        );
    }
}
