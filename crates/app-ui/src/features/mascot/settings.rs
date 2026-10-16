//! This feature's part of the settings model: the JSON the page renders and the changes it may make.
//! Pure data with no AppKit, so every rule is covered by plain unit tests.

use super::{Character, Config, scene::Farewell};
use crate::settings::model::Fault;
use serde_json::{Map, Value, json};
use std::sync::OnceLock;

/// The pictures of the characters for the page, made once: they are part of the program.
fn sheets() -> &'static Value {
    static SHEETS: OnceLock<Value> = OnceLock::new();
    SHEETS.get_or_init(|| {
        let all: Map<String, Value> = Character::ALL
            .into_iter()
            .map(|c| {
                let url = format!("data:image/png;base64,{}", crate::settings::base64(c.png()));
                (c.as_str().to_owned(), Value::String(url))
            })
            .collect();
        Value::Object(all)
    })
}

/// The state under the key "mascot". `runtime` is `Feature::runtime()`, or null when the feature is not
/// running. `web/features/mascot.fixture.json` must have exactly the same keys; a unit test checks it.
pub fn state(config: &Config, runtime: &Value) -> Value {
    json!({
        "enabled": config.enabled,
        "character": config.character.as_str(),
        "farewell": config.farewell.as_str(),
        "characters": Character::ALL.map(Character::as_str),
        "sheets": sheets(),
        "accessibility": runtime.get("accessibility").and_then(Value::as_bool).unwrap_or(false),
    })
}

fn invalid(what: &'static str) -> Fault {
    Fault::new("invalid_value", what)
}

/// A change from the page. `path` is relative to the feature: "enabled", not "mascot.enabled". `None` when
/// the path is not one of this feature's. Values are checked here; the page is not trusted.
pub fn set(config: &mut Config, path: &str, value: &Value) -> Option<Result<(), Fault>> {
    Some(match path {
        "enabled" => value
            .as_bool()
            .map(|on| config.enabled = on)
            .ok_or_else(|| invalid("expected true or false")),
        "character" => value
            .as_str()
            .and_then(Character::parse)
            .map(|c| config.character = c)
            .ok_or_else(|| invalid("expected spider, frog, ufo, ghost, wizard or mario")),
        "farewell" => value
            .as_str()
            .and_then(Farewell::parse)
            .map(|f| config.farewell = f)
            .ok_or_else(|| invalid("expected guard or regret")),
        _ => return None,
    })
}

/// The feature has no global shortcut.
pub fn shortcuts(_config: &Config) -> Vec<(String, String)> {
    Vec::new()
}

pub fn set_shortcut(
    _config: &mut Config,
    _target: &str,
    _value: &str,
) -> Option<Result<(), Fault>> {
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_fixture_has_the_keys_of_the_state() {
        let fixture: Value =
            serde_json::from_str(include_str!("../../../web/features/mascot.fixture.json"))
                .unwrap();
        let keys = |v: &Value| {
            let mut keys: Vec<String> = v.as_object().unwrap().keys().cloned().collect();
            keys.sort();
            keys
        };
        assert_eq!(
            keys(&fixture),
            keys(&state(&Config::default(), &Value::Null))
        );
        for c in Character::ALL {
            assert!(
                fixture["sheets"].get(c.as_str()).is_some(),
                "the fixture draws {c:?}"
            );
        }
    }

    #[test]
    fn changes_are_checked_before_they_are_kept() {
        let mut config = Config::default();
        assert!(set(&mut config, "enabled", &json!(true)).unwrap().is_ok() && config.enabled);
        assert!(
            set(&mut config, "character", &json!("ufo"))
                .unwrap()
                .is_ok()
        );
        assert_eq!(config.character, Character::Ufo);
        assert!(
            set(&mut config, "farewell", &json!("regret"))
                .unwrap()
                .is_ok()
        );
        assert_eq!(config.farewell, Farewell::Regret);
        for (path, bad) in [
            ("enabled", json!("yes")),
            ("character", json!("dragon")),
            ("character", json!(3)),
            ("farewell", json!("shrug")),
            ("farewell", json!(null)),
        ] {
            assert!(
                set(&mut config, path, &bad).unwrap().is_err(),
                "{path} {bad}"
            );
        }
        assert_eq!(
            config.character,
            Character::Ufo,
            "a refused change keeps the old value"
        );
        assert!(set(&mut config, "nonsense", &json!(1)).is_none());
        let state = state(&config, &json!({ "accessibility": true }));
        assert_eq!(state["character"], "ufo");
        assert_eq!(state["farewell"], "regret");
        assert_eq!(state["accessibility"], true);
        assert_eq!(state["characters"].as_array().unwrap().len(), 6);
    }
}
