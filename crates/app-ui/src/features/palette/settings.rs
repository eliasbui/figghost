//! The command palette's part of the settings model: the JSON the page renders and the changes it may make.

use super::Config;
use crate::{config::parse_hotkey, settings::model::Fault};
use serde_json::{Value, json};

/// The state under the key "palette". `web/features/palette.fixture.json` must have exactly the same keys.
pub fn state(config: &Config) -> Value {
    let config = config.clone().sanitized();
    json!({ "enabled": config.enabled, "shortcut": config.shortcut })
}

pub fn set(config: &mut Config, path: &str, value: &Value) -> Option<Result<(), Fault>> {
    Some(match path {
        "enabled" => value
            .as_bool()
            .map(|on| config.enabled = on)
            .ok_or_else(|| Fault::new("invalid_value", path)),
        _ => return None,
    })
}

pub fn shortcuts(config: &Config) -> Vec<(String, String)> {
    if config.shortcut.is_empty() {
        Vec::new()
    } else {
        vec![("palette".to_owned(), config.shortcut.clone())]
    }
}

pub fn set_shortcut(config: &mut Config, target: &str, value: &str) -> Option<Result<(), Fault>> {
    if target != "palette" {
        return None;
    }
    let value = value.trim().to_owned();
    if !value.is_empty() && parse_hotkey(&value).is_err() {
        return Some(Err(Fault::new("invalid_shortcut", target)));
    }
    config.shortcut = value;
    Some(Ok(()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_fixture_has_exactly_the_keys_of_the_state() {
        let fixture: Value =
            serde_json::from_str(include_str!("../../../web/features/palette.fixture.json"))
                .unwrap();
        let keys = |v: &Value| v.as_object().unwrap().keys().cloned().collect::<Vec<_>>();
        assert_eq!(keys(&fixture), keys(&state(&Config::default())));
    }

    #[test]
    fn the_switch_and_the_shortcut_are_validated() {
        let mut c = Config::default();
        assert!(!c.enabled);
        assert!(set(&mut c, "enabled", &json!(1)).unwrap().is_err());
        assert!(set(&mut c, "enabled", &json!(true)).unwrap().is_ok() && c.enabled);
        assert!(set(&mut c, "other", &json!(true)).is_none());
        assert!(
            set_shortcut(&mut c, "palette", "cmd+shift+k")
                .unwrap()
                .is_ok()
        );
        assert!(
            set_shortcut(&mut c, "palette", "nonsense")
                .unwrap()
                .is_err()
        );
        assert_eq!(c.shortcut, "cmd+shift+k");
        assert!(set_shortcut(&mut c, "palette", "").unwrap().is_ok() && shortcuts(&c).is_empty());
        assert!(set_shortcut(&mut c, "translate", "cmd+k").is_none());
    }
}
