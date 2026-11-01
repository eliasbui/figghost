//! The Commands tab's part of the settings model: the JSON the page renders and the changes it may make.

use super::{
    Config,
    logic::{self, Item},
};
use crate::settings::model::Fault;
use serde_json::{Value, json};

/// The state under the key "commands". `web/features/commands.fixture.json` must have exactly the same keys.
pub fn state(config: &Config, runtime: &Value) -> Value {
    let config = config.clone().sanitized();
    json!({
        "enabled": config.enabled,
        "items": config.items.iter().map(|i| json!({ "name": i.name, "command": i.command })).collect::<Vec<_>>(),
        "max": logic::MAX_ITEMS,
        "running": runtime.get("running").and_then(Value::as_bool).unwrap_or(false),
    })
}

pub fn set(config: &mut Config, path: &str, value: &Value) -> Option<Result<(), Fault>> {
    let bad = || Fault::new("invalid_value", path);
    Some(match path {
        "enabled" => value
            .as_bool()
            .map(|on| config.enabled = on)
            .ok_or_else(bad),
        // The whole list as the page has it. Every item has to be good: a list with a half-written one is not taken, so
        // that the list is never cut behind the person's back.
        "items" => value
            .as_array()
            .filter(|list| list.len() <= logic::MAX_ITEMS)
            .and_then(|list| {
                list.iter()
                    .map(|v| {
                        Item {
                            name: v["name"].as_str()?.to_owned(),
                            command: v["command"].as_str()?.to_owned(),
                        }
                        .valid()
                    })
                    .collect::<Option<Vec<Item>>>()
            })
            .map(|items| config.items = items)
            .ok_or_else(bad),
        _ => return None,
    })
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
            serde_json::from_str(include_str!("../../../web/features/commands.fixture.json"))
                .unwrap();
        assert_eq!(
            keys(&fixture),
            keys(&state(&Config::default(), &Value::Null))
        );
        assert_eq!(keys(&fixture["items"][0]), ["command", "name"]);
    }

    #[test]
    fn the_list_is_taken_whole_or_not_at_all() {
        let mut c = Config::default();
        let good = json!([{ "name": " Build ", "command": "cargo build" }, { "name": "Test", "command": "cargo test" }]);
        assert!(set(&mut c, "items", &good).unwrap().is_ok());
        assert_eq!((c.items.len(), c.items[0].name.as_str()), (2, "Build"));
        for bad in [
            json!([{ "name": "Ok", "command": "ls" }, { "name": "", "command": "ls" }]),
            json!([{ "name": "x", "command": "a\nb" }]),
            json!([{ "name": "x" }]),
            json!("ls"),
            json!([5]),
            json!(
                (0..9)
                    .map(|i| json!({ "name": format!("c{i}"), "command": "ls" }))
                    .collect::<Vec<_>>()
            ),
        ] {
            assert!(set(&mut c, "items", &bad).unwrap().is_err(), "{bad}");
        }
        assert_eq!(
            c.items.len(),
            2,
            "a list that was refused leaves the one before"
        );
        assert!(set(&mut c, "items", &json!([])).unwrap().is_ok() && c.items.is_empty());
        assert!(set(&mut c, "enabled", &json!(1)).unwrap().is_err());
        assert!(set(&mut c, "other", &json!(1)).is_none());
        assert_eq!(
            state(&c, &json!({ "running": true }))["running"],
            json!(true)
        );
    }
}
