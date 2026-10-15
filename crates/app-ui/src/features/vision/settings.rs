//! This feature's part of the settings model: the JSON the page renders and the changes it may make.
//! Pure data with no AppKit, so every rule is covered by plain unit tests.

use super::{Config, colour::Format};
use crate::settings::model::Fault;
use serde_json::{Value, json};

/// The state under the key "vision". `runtime` is `Feature::runtime()`, or null when the feature is not running.
/// `web/features/vision.fixture.json` must have exactly the same keys; a unit test checks it.
pub fn state(config: &Config, runtime: &Value) -> Value {
    let fact = |key: &str| runtime.get(key).and_then(Value::as_bool).unwrap_or(false);
    json!({
        "enabled": config.enabled,
        "shortcut": config.shortcut,
        "format": config.format().id(),
        "screen_access": fact("screen_access"),
        "text_recognition": fact("text_recognition"),
        "background_removal": fact("background_removal"),
    })
}

/// A change from the page. `path` is relative to the feature: "enabled", not "vision.enabled". `None` when the
/// path is not one of this feature's. Values must be validated here; the page is not trusted.
pub fn set(config: &mut Config, path: &str, value: &Value) -> Option<Result<(), Fault>> {
    let invalid = || Fault::new("invalid_value", path);
    match path {
        "enabled" => Some(
            value
                .as_bool()
                .map(|on| config.enabled = on)
                .ok_or_else(invalid),
        ),
        "format" => Some(
            value
                .as_str()
                .and_then(Format::from_id)
                .map(|f| config.format = f.id().to_owned())
                .ok_or_else(invalid),
        ),
        _ => None,
    }
}

/// This feature's shortcuts as (target, shortcut). A target is "vision" or "vision:<what>", which is what the
/// page's shortcut recorder sends. Shortcuts are checked against all the others before `set_shortcut` runs.
pub fn shortcuts(config: &Config) -> Vec<(String, String)> {
    if config.shortcut.is_empty() {
        return Vec::new();
    }
    vec![("vision".into(), config.shortcut.clone())]
}

/// Stores a shortcut that was already parsed and checked for conflicts; an empty value removes it.
pub fn set_shortcut(config: &mut Config, target: &str, value: &str) -> Option<Result<(), Fault>> {
    (target == "vision").then(|| {
        config.shortcut = value.to_owned();
        Ok(())
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::features::vision::DEFAULT_SHORTCUT;

    fn keys(value: &Value) -> Vec<String> {
        let mut keys: Vec<String> = value.as_object().unwrap().keys().cloned().collect();
        keys.sort();
        keys
    }

    #[test]
    fn the_fixture_has_exactly_the_keys_of_the_state() {
        let fixture: Value =
            serde_json::from_str(include_str!("../../../web/features/vision.fixture.json"))
                .unwrap();
        assert_eq!(
            keys(&fixture),
            keys(&state(&Config::default(), &Value::Null))
        );
    }

    #[test]
    fn the_feature_is_off_with_the_documented_shortcut_until_asked() {
        let config = Config::default();
        assert!(!config.enabled);
        assert_eq!(config.shortcut, DEFAULT_SHORTCUT);
        assert!(crate::config::parse_hotkey(DEFAULT_SHORTCUT).is_ok());
        let s = state(&config, &Value::Null);
        assert_eq!(
            (s["enabled"].clone(), s["format"].clone()),
            (json!(false), json!("hex"))
        );
        assert_eq!(
            s["screen_access"],
            json!(false),
            "unknown until the app says"
        );
    }

    #[test]
    fn runtime_facts_reach_the_page() {
        let runtime =
            json!({ "screen_access": true, "text_recognition": true, "background_removal": false });
        let s = state(&Config::default(), &runtime);
        assert_eq!(
            (
                s["screen_access"].clone(),
                s["text_recognition"].clone(),
                s["background_removal"].clone()
            ),
            (json!(true), json!(true), json!(false))
        );
        // A value of the wrong type is not trusted.
        let s = state(&Config::default(), &json!({ "screen_access": "yes" }));
        assert_eq!(s["screen_access"], json!(false));
    }

    #[test]
    fn the_switch_and_the_format_are_validated() {
        let mut config = Config::default();
        assert!(set(&mut config, "enabled", &json!(true)).unwrap().is_ok() && config.enabled);
        assert!(set(&mut config, "enabled", &json!("yes")).unwrap().is_err());
        assert!(set(&mut config, "format", &json!("hsl")).unwrap().is_ok());
        assert_eq!(config.format, "hsl");
        for bad in [
            json!("cmyk"),
            json!("HEX"),
            json!(3),
            json!(null),
            json!(["hex"]),
        ] {
            assert!(set(&mut config, "format", &bad).unwrap().is_err(), "{bad}");
        }
        assert_eq!(
            config.format, "hsl",
            "a refused change leaves the value alone"
        );
        assert!(set(&mut config, "nonsense", &json!(1)).is_none());
        assert!(
            set(&mut config, "shortcut", &json!("cmd+x")).is_none(),
            "shortcuts go through set_shortcut"
        );
    }

    #[test]
    fn a_format_the_config_file_should_not_hold_shows_as_hex() {
        let config: Config = serde_json::from_str(r#"{"format":"rm -rf","enabled":true}"#).unwrap();
        assert_eq!(state(&config, &Value::Null)["format"], json!("hex"));
        assert_eq!(
            config.shortcut, DEFAULT_SHORTCUT,
            "missing keys take their defaults"
        );
        let config: Config = serde_json::from_str("{}").unwrap();
        assert_eq!(config, Config::default());
    }

    #[test]
    fn the_shortcut_can_change_and_be_removed() {
        let mut config = Config::default();
        assert_eq!(
            shortcuts(&config),
            [("vision".to_owned(), DEFAULT_SHORTCUT.to_owned())]
        );
        assert!(
            set_shortcut(&mut config, "vision", "cmd+shift+9")
                .unwrap()
                .is_ok()
        );
        assert_eq!(config.shortcut, "cmd+shift+9");
        assert!(set_shortcut(&mut config, "vision", "").unwrap().is_ok());
        assert!(
            shortcuts(&config).is_empty(),
            "a removed shortcut is not offered for conflicts"
        );
        assert!(set_shortcut(&mut config, "vision:other", "cmd+x").is_none());
        assert!(set_shortcut(&mut config, "capture", "cmd+x").is_none());
    }
}
