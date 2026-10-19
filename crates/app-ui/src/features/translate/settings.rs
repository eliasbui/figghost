//! Translate's part of the settings model: the JSON the page renders and the changes it may make.

use super::Config;
use crate::settings::model::Fault;
use core_smart::translate;
use serde_json::{Value, json};

fn invalid(detail: &str) -> Fault {
    Fault::new("invalid_value", detail)
}

/// The state under the key "translate". `runtime` carries `accessibility`.
pub fn state(config: &Config, runtime: &Value) -> Value {
    let languages: Vec<Value> = translate::LANGUAGES
        .iter()
        .map(|l| json!({ "code": l.code, "name": l.name }))
        .collect();
    json!({
        "enabled": config.enabled,
        "shortcut": config.shortcut,
        "translate_to": config.translate_to,
        "material": config.material,
        "languages": languages,
        "accessibility": runtime.get("accessibility").and_then(Value::as_bool).unwrap_or(false),
    })
}

/// A change from the page. `path` is relative to the feature.
pub fn set(config: &mut Config, path: &str, value: &Value) -> Option<Result<(), Fault>> {
    Some(match path {
        "enabled" => value
            .as_bool()
            .map(|on| config.enabled = on)
            .ok_or_else(|| invalid("expected true or false")),
        "translate_to" => match value.as_str() {
            Some(code) if code == "auto" || translate::is_supported(code) => {
                config.translate_to = code.to_owned();
                Ok(())
            }
            _ => Err(invalid("an unknown language")),
        },
        "material" => match value.as_str() {
            Some(name) if super::material_of(name).is_some() => {
                config.material = name.to_owned();
                Ok(())
            }
            _ => Err(invalid("an unknown material")),
        },
        _ => return None,
    })
}

pub fn shortcuts(config: &Config) -> Vec<(String, String)> {
    if config.shortcut.is_empty() {
        Vec::new()
    } else {
        vec![("translate".to_owned(), config.shortcut.clone())]
    }
}

pub fn set_shortcut(config: &mut Config, target: &str, value: &str) -> Option<Result<(), Fault>> {
    if target != "translate" {
        return None;
    }
    // Without a shortcut the feature can only be reached from the menu; that is allowed.
    config.shortcut = value.trim().to_owned();
    Some(Ok(()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_target_language_is_auto_or_a_known_one() {
        let mut config = Config::default();
        assert!(
            set(&mut config, "translate_to", &json!("ja"))
                .unwrap()
                .is_ok()
        );
        assert_eq!(config.translate_to, "ja");
        for bad in [json!("klingon"), json!(""), json!(3), json!(null)] {
            assert!(
                set(&mut config, "translate_to", &bad).unwrap().is_err(),
                "{bad}"
            );
        }
        assert_eq!(
            config.translate_to, "ja",
            "a refused change leaves the setting alone"
        );
        assert!(
            set(&mut config, "translate_to", &json!("auto"))
                .unwrap()
                .is_ok()
        );
    }

    #[test]
    fn the_switch_and_the_shortcut_are_validated() {
        let mut config = Config::default();
        assert!(set(&mut config, "enabled", &json!(true)).unwrap().is_ok() && config.enabled);
        assert!(set(&mut config, "enabled", &json!("yes")).unwrap().is_err());
        assert!(set(&mut config, "nonsense", &json!(1)).is_none());
        assert!(set_shortcut(&mut config, "other", "ctrl+opt+x").is_none());
        assert!(set_shortcut(&mut config, "translate", "").unwrap().is_ok());
        assert!(
            shortcuts(&config).is_empty(),
            "a removed shortcut is not listed"
        );
    }

    #[test]
    fn a_config_file_cannot_hold_an_unusable_value() {
        let config = Config {
            shortcut: "nonsense".into(),
            translate_to: "klingon".into(),
            material: "wood".into(),
            ..Config::default()
        }
        .sanitized();
        assert_eq!(config, Config::default());
    }
}
