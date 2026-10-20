//! This feature's part of the settings model: the JSON the page renders and the changes it may make.
//! Pure data with no AppKit, so every rule is covered by plain unit tests. What the page can do beyond switching the
//! tab on is done by actions (`launcher_add_app`, `launcher_remove:<n>`, ...), which `Feature::action` takes.

use super::{
    Config,
    logic::{self, MAX_ITEMS},
};
use crate::settings::model::Fault;
use serde_json::{Value, json};

/// The state under the key "launcher". `runtime` is `Feature::runtime()`, or null when the feature is not running.
/// `web/features/launcher.fixture.json` must have exactly the same keys; a unit test checks it.
pub fn state(config: &Config, runtime: &Value) -> Value {
    let config = config.clone().sanitized();
    json!({
        "enabled": config.enabled,
        "items": config.items.iter().map(|i| json!({ "kind": i.kind.code(), "name": i.name })).collect::<Vec<_>>(),
        "full": config.items.len() >= MAX_ITEMS,
        "listing": runtime.get("listing").and_then(Value::as_bool).unwrap_or(false),
        "shortcuts": runtime.get("shortcuts").cloned().unwrap_or_else(|| json!([])),
        "problem": runtime.get("problem").and_then(Value::as_str).unwrap_or_default(),
    })
}

/// A change from the page. `path` is relative to the feature. Values must be validated here; the page is not
/// trusted. The pins are changed by actions, not set.
pub fn set(config: &mut Config, path: &str, value: &Value) -> Option<Result<(), Fault>> {
    Some(match path {
        "enabled" => value
            .as_bool()
            .map(|on| config.enabled = on)
            .ok_or_else(|| Fault::new("invalid_value", path)),
        _ => return None,
    })
}

/// Makes the fault that tells why a pin could not be added.
pub fn refusal(error: logic::AddError) -> Fault {
    Fault::new(
        match error {
            logic::AddError::Full => "launcher_full",
            logic::AddError::Already => "launcher_already",
            logic::AddError::Unusable => "launcher_unusable",
        },
        "launcher",
    )
}

#[cfg(test)]
mod tests {
    use super::super::logic::{Item, Kind};
    use super::*;

    fn keys(value: &Value) -> Vec<String> {
        let mut keys: Vec<String> = value.as_object().unwrap().keys().cloned().collect();
        keys.sort();
        keys
    }

    #[test]
    fn the_fixture_has_exactly_the_keys_of_the_state() {
        let fixture: Value =
            serde_json::from_str(include_str!("../../../web/features/launcher.fixture.json"))
                .unwrap();
        let config = Config {
            items: vec![Item {
                kind: Kind::App,
                target: "com.apple.Notes".into(),
                name: "Notes".into(),
            }],
            ..Config::default()
        };
        let state = state(&config, &Value::Null);
        assert_eq!(keys(&fixture), keys(&state));
        assert_eq!(keys(&fixture["items"][0]), keys(&state["items"][0]));
    }

    #[test]
    fn the_state_starts_off_and_empty_and_says_what_the_running_feature_knows() {
        let s = state(&Config::default(), &Value::Null);
        assert_eq!(s["enabled"], json!(false));
        assert_eq!(s["items"], json!([]));
        assert_eq!(s["shortcuts"], json!([]));
        let runtime =
            json!({ "listing": true, "shortcuts": ["Morning routine"], "problem": "none" });
        let s = state(&Config::default(), &runtime);
        assert_eq!(s["listing"], json!(true));
        assert_eq!(s["shortcuts"][0], json!("Morning routine"));
        assert_eq!(s["problem"], json!("none"));
        let full = Config {
            items: (0..MAX_ITEMS)
                .map(|i| Item {
                    kind: Kind::App,
                    target: format!("com.example.a{i}"),
                    name: String::new(),
                })
                .collect(),
            ..Config::default()
        };
        assert_eq!(state(&full, &Value::Null)["full"], json!(true));
    }

    #[test]
    fn the_switch_is_validated_and_the_pins_are_not_set_from_the_page() {
        let mut config = Config::default();
        assert!(set(&mut config, "enabled", &json!("yes")).unwrap().is_err());
        assert!(set(&mut config, "enabled", &json!(true)).unwrap().is_ok() && config.enabled);
        assert!(set(&mut config, "items", &json!([])).is_none());
        assert!(set(&mut config, "nonsense", &json!(1)).is_none());
    }
}
