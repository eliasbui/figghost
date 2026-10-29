//! This feature's part of the settings model: the JSON the page renders and the changes it may make.
//! Pure data with no AppKit, so every rule is covered by plain unit tests. What the page can do to the pins and the
//! groups is done by actions (`launcher_remove:<group>:<n>`, ...), which `Feature::action` takes, and by
//! `launcher_add_app:<group>`, `launcher_add_folder:<group>` and `launcher_add_file:<group>`, which ask the person
//! with the system's panel (`Feature::choose`) before the config is touched (`Feature::pin`).

use super::{
    Config, DockEdge,
    logic::{self, MAX_GROUPS, MAX_ITEMS},
};
use crate::settings::model::Fault;
use serde_json::{Value, json};

/// The state under the key "launcher". `runtime` is `Feature::runtime()`, or null when the feature is not running.
/// `web/features/launcher.fixture.json` must have exactly the same keys; a unit test checks it.
pub fn state(config: &Config, runtime: &Value) -> Value {
    let config = config.clone().sanitized();
    let groups: Vec<Value> = config
        .groups
        .iter()
        .map(|g| {
            json!({
                "name": g.name,
                "items": g.items.iter().map(|i| json!({ "kind": i.kind.code(), "name": i.name })).collect::<Vec<_>>(),
                "full": g.items.len() >= MAX_ITEMS,
            })
        })
        .collect();
    json!({
        "enabled": config.enabled,
        "groups": groups,
        "groups_full": config.groups.len() >= MAX_GROUPS,
        "dock": config.dock,
        "dock_edge": edge_code(config.dock_edge),
        "dock_display": config.dock_display,
        "dock_group": config.dock_group,
        "dock_autohide": config.dock_autohide,
        "screens": runtime.get("screens").cloned().unwrap_or_else(|| json!([])),
        "listing": runtime.get("listing").and_then(Value::as_bool).unwrap_or(false),
        "shortcuts": runtime.get("shortcuts").cloned().unwrap_or_else(|| json!([])),
        "problem": runtime.get("problem").and_then(Value::as_str).unwrap_or_default(),
    })
}

fn edge_code(edge: DockEdge) -> &'static str {
    match edge {
        DockEdge::Left => "left",
        DockEdge::Right => "right",
        DockEdge::Bottom => "bottom",
    }
}

/// A change from the page. `path` is relative to the feature. Values must be validated here; the page is not
/// trusted. The pins and the groups are changed by actions, not set.
pub fn set(config: &mut Config, path: &str, value: &Value) -> Option<Result<(), Fault>> {
    let wrong = || Fault::new("invalid_value", path);
    let switch = |slot: &mut bool| value.as_bool().map(|on| *slot = on).ok_or_else(wrong);
    Some(match path {
        "enabled" => switch(&mut config.enabled),
        "dock" => switch(&mut config.dock),
        "dock_autohide" => switch(&mut config.dock_autohide),
        "dock_edge" => match value.as_str() {
            Some("left") => Some(DockEdge::Left),
            Some("right") => Some(DockEdge::Right),
            Some("bottom") => Some(DockEdge::Bottom),
            _ => None,
        }
        .map(|edge| config.dock_edge = edge)
        .ok_or_else(wrong),
        "dock_display" => match value.as_str() {
            Some(name) if name.chars().count() <= 100 => {
                config.dock_display = name.to_owned();
                *config = config.clone().sanitized();
                Ok(())
            }
            _ => Err(wrong()),
        },
        "dock_group" => {
            let groups = config.clone().sanitized().groups.len();
            value
                .as_u64()
                .filter(|group| (*group as usize) < groups)
                .map(|group| config.dock_group = group as usize)
                .ok_or_else(wrong)
        }
        _ => return None,
    })
}

/// Makes the fault that tells why a pin or a group could not be added.
pub fn refusal(error: logic::AddError) -> Fault {
    Fault::new(
        match error {
            logic::AddError::Full => "launcher_full",
            logic::AddError::Already => "launcher_already",
            logic::AddError::Unusable => "launcher_unusable",
            logic::AddError::Groups => "launcher_groups_full",
        },
        "launcher",
    )
}

#[cfg(test)]
mod tests {
    use super::super::logic::{Group, Item, Kind};
    use super::*;

    fn keys(value: &Value) -> Vec<String> {
        let mut keys: Vec<String> = value.as_object().unwrap().keys().cloned().collect();
        keys.sort();
        keys
    }

    fn notes() -> Item {
        Item {
            kind: Kind::App,
            target: "com.apple.Notes".into(),
            name: "Notes".into(),
        }
    }

    #[test]
    fn the_fixture_has_exactly_the_keys_of_the_state() {
        let fixture: Value =
            serde_json::from_str(include_str!("../../../web/features/launcher.fixture.json"))
                .unwrap();
        let config = Config {
            groups: vec![Group {
                name: String::new(),
                items: vec![notes()],
            }],
            ..Config::default()
        };
        let state = state(&config, &Value::Null);
        assert_eq!(keys(&fixture), keys(&state));
        assert_eq!(keys(&fixture["groups"][0]), keys(&state["groups"][0]));
        assert_eq!(
            keys(&fixture["groups"][0]["items"][0]),
            keys(&state["groups"][0]["items"][0])
        );
    }

    #[test]
    fn the_state_starts_off_and_empty_and_says_what_the_running_feature_knows() {
        let s = state(&Config::default(), &Value::Null);
        assert_eq!(s["enabled"], json!(false));
        assert_eq!(s["dock"], json!(false));
        assert_eq!(s["dock_edge"], json!("left"));
        assert_eq!(s["dock_autohide"], json!(true));
        assert_eq!(
            s["groups"].as_array().unwrap().len(),
            1,
            "there is always a group to pin to"
        );
        assert_eq!(s["groups"][0]["items"], json!([]));
        assert_eq!(s["shortcuts"], json!([]));
        assert_eq!(s["screens"], json!([]));
        let runtime = json!({ "listing": true, "shortcuts": ["Morning routine"], "problem": "none", "screens": ["Studio Display"] });
        let s = state(&Config::default(), &runtime);
        assert_eq!(s["listing"], json!(true));
        assert_eq!(s["shortcuts"][0], json!("Morning routine"));
        assert_eq!(s["problem"], json!("none"));
        assert_eq!(s["screens"][0], json!("Studio Display"));
        let full = Config {
            groups: (0..MAX_GROUPS)
                .map(|_| Group {
                    name: String::new(),
                    items: (0..MAX_ITEMS)
                        .map(|i| Item {
                            kind: Kind::App,
                            target: format!("com.example.a{i}"),
                            name: String::new(),
                        })
                        .collect(),
                })
                .collect(),
            ..Config::default()
        };
        let s = state(&full, &Value::Null);
        assert_eq!(s["groups"][0]["full"], json!(true));
        assert_eq!(s["groups_full"], json!(true));
    }

    #[test]
    fn the_switches_and_the_dock_are_validated_and_the_pins_are_not_set_from_the_page() {
        let mut config = Config::default();
        assert!(set(&mut config, "enabled", &json!("yes")).unwrap().is_err());
        assert!(set(&mut config, "enabled", &json!(true)).unwrap().is_ok() && config.enabled);
        assert!(set(&mut config, "dock", &json!(true)).unwrap().is_ok() && config.dock);
        assert!(
            set(&mut config, "dock_autohide", &json!(false))
                .unwrap()
                .is_ok()
                && !config.dock_autohide
        );
        assert!(
            set(&mut config, "dock_edge", &json!("bottom"))
                .unwrap()
                .is_ok()
        );
        assert_eq!(config.dock_edge, DockEdge::Bottom);
        assert!(
            set(&mut config, "dock_edge", &json!("top"))
                .unwrap()
                .is_err(),
            "the top is the notch's"
        );
        assert!(
            set(&mut config, "dock_display", &json!("Studio Display"))
                .unwrap()
                .is_ok()
        );
        assert_eq!(config.dock_display, "Studio Display");
        assert!(
            set(&mut config, "dock_display", &json!("x".repeat(101)))
                .unwrap()
                .is_err()
        );
        assert!(set(&mut config, "dock_group", &json!(0)).unwrap().is_ok());
        assert!(
            set(&mut config, "dock_group", &json!(1)).unwrap().is_err(),
            "there is one group"
        );
        assert!(set(&mut config, "dock_group", &json!(-1)).unwrap().is_err());
        assert!(set(&mut config, "groups", &json!([])).is_none());
        assert!(set(&mut config, "items", &json!([])).is_none());
        assert!(set(&mut config, "nonsense", &json!(1)).is_none());
    }
}
