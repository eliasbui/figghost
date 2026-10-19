//! This feature's part of the settings model: the JSON the page renders and the changes it may make.
//! Pure data with no AppKit, so every rule is covered by plain unit tests.

use super::{Config, CustomLayout};
use crate::settings::model::Fault;
use core_window::templates::{self, BUILT_IN, MAX_CUSTOM};
use serde_json::{Value, json};
use std::collections::BTreeMap;

/// The most screens that can have a list of their own, and the longest name of one.
const MAX_SCREENS: usize = 16;
const MAX_SCREEN_NAME: usize = 100;
/// Every built-in layout and every one a person can make.
const MAX_TEMPLATES: usize = BUILT_IN.len() + MAX_CUSTOM;

/// The ids a list asks for that can be offered, none twice, in the order the row shows them: the built-in ones as
/// the catalogue has them, then the ones a person made as they were made.
fn canonical<S: AsRef<str>>(ids: &[S], custom: &[CustomLayout]) -> Vec<String> {
    let wanted = |id: &str| ids.iter().any(|w| w.as_ref() == id);
    let mine = custom
        .iter()
        .filter(|c| c.to_core().check().is_ok())
        .map(|c| c.id.as_str());
    let mut shown: Vec<String> = Vec::new();
    for id in BUILT_IN.iter().copied().chain(mine) {
        if wanted(id) && !shown.iter().any(|s| s == id) {
            shown.push(id.to_owned());
        }
    }
    shown
}

/// The state under the key "snap". `runtime` is `Feature::runtime()`, or null when the feature is not running.
/// `web/features/snap.fixture.json` must have exactly the same keys; a unit test checks it.
pub fn state(config: &Config, runtime: &Value) -> Value {
    let valid: Vec<&CustomLayout> = config
        .custom
        .iter()
        .filter(|c| c.to_core().check().is_ok())
        .collect();
    // A list with nothing usable in it is not one: the screen is offered the common list.
    let screens: BTreeMap<&str, Vec<String>> = config
        .screens
        .iter()
        .map(|(name, ids)| (name.as_str(), canonical(ids, &config.custom)))
        .filter(|(_, ids)| !ids.is_empty())
        .collect();
    let mut common = canonical(&config.templates, &config.custom);
    if common.is_empty() {
        common = templates::DEFAULT
            .iter()
            .map(|id| (*id).to_owned())
            .collect();
    }
    let built_in: Vec<Value> = BUILT_IN
        .iter()
        .filter_map(|id| templates::built_in(id))
        .map(|t| {
            let cells: Vec<[f64; 4]> = t.cells.iter().map(|c| [c.x0, c.y0, c.x1, c.y1]).collect();
            json!({ "id": t.id, "cells": cells })
        })
        .collect();
    json!({
        "enabled": config.enabled,
        "templates": common,
        "custom": valid,
        "screens": screens,
        "built_in": built_in,
        "displays": runtime["displays"].as_array().cloned().unwrap_or_default(),
        "accessibility": runtime["accessibility"].as_bool().unwrap_or(false),
    })
}

/// A change from the page. `path` is relative to the feature: "enabled", not "snap.enabled". `None` when the
/// path is not one of this feature's. Values must be validated here; the page is not trusted.
pub fn set(config: &mut Config, path: &str, value: &Value) -> Option<Result<(), Fault>> {
    match path {
        "enabled" => Some(
            value
                .as_bool()
                .map(|on| config.enabled = on)
                .ok_or_else(|| Fault::new("invalid_value", "expected true or false")),
        ),
        "templates" => Some(set_templates(config, value)),
        "screens" => Some(set_screens(config, value)),
        "custom" => Some(set_custom(config, value)),
        _ => None,
    }
}

/// A list of ids: each a string that names a built-in layout or one of the person's, none twice, and at least one.
/// The page builds the list from checkboxes, so anything else is a bug or a hostile message, and is refused whole.
fn ids_of(value: &Value, custom: &[CustomLayout]) -> Result<Vec<String>, Fault> {
    let invalid = || Fault::new("invalid_value", "expected a list of layout ids");
    let ids: Vec<&str> = value
        .as_array()
        .ok_or_else(invalid)?
        .iter()
        .map(|v| v.as_str().ok_or_else(invalid))
        .collect::<Result<_, _>>()?;
    let known = |id: &str| {
        BUILT_IN.contains(&id)
            || custom
                .iter()
                .any(|c| c.id == id && c.to_core().check().is_ok())
    };
    let mut unique = ids.clone();
    unique.sort_unstable();
    unique.dedup();
    if ids.is_empty()
        || ids.len() > MAX_TEMPLATES
        || unique.len() != ids.len()
        || !ids.iter().all(|id| known(id))
    {
        return Err(Fault::new("snap_templates", format!("{} ids", ids.len())));
    }
    Ok(canonical(&ids, custom))
}

fn set_templates(config: &mut Config, value: &Value) -> Result<(), Fault> {
    config.templates = ids_of(value, &config.custom)?;
    Ok(())
}

/// The lists of the screens that have one of their own, by name; a screen left out has the common list.
fn set_screens(config: &mut Config, value: &Value) -> Result<(), Fault> {
    let invalid = || {
        Fault::new(
            "invalid_value",
            "expected lists of layout ids by screen name",
        )
    };
    let object = value.as_object().ok_or_else(invalid)?;
    if object.len() > MAX_SCREENS {
        return Err(Fault::new(
            "snap_templates",
            format!("{} screens", object.len()),
        ));
    }
    let mut screens = BTreeMap::new();
    for (name, list) in object {
        if name.trim().is_empty()
            || name.chars().count() > MAX_SCREEN_NAME
            || name.chars().any(char::is_control)
        {
            return Err(invalid());
        }
        screens.insert(name.clone(), ids_of(list, &config.custom)?);
    }
    config.screens = screens;
    Ok(())
}

/// The layouts a person made, replacing the old ones. A layout that goes is dropped from every list that offered it,
/// and a list left with nothing is the default again.
fn set_custom(config: &mut Config, value: &Value) -> Result<(), Fault> {
    let refuse = |detail: String| Fault::new("snap_custom", detail);
    let list: Vec<CustomLayout> = serde_json::from_value(value.clone())
        .map_err(|e| Fault::new("invalid_value", format!("expected a list of layouts: {e}")))?;
    if list.len() > MAX_CUSTOM {
        return Err(refuse(format!("{} layouts", list.len())));
    }
    let mut seen: Vec<&str> = Vec::new();
    for layout in &list {
        layout
            .to_core()
            .check()
            .map_err(|why| refuse(format!("{}: {why}", layout.id)))?;
        if seen.contains(&layout.id.as_str()) {
            return Err(refuse(format!("{} twice", layout.id)));
        }
        seen.push(&layout.id);
    }
    config.custom = list;
    let keep = |ids: &mut Vec<String>, custom: &[CustomLayout]| {
        ids.retain(|id| BUILT_IN.contains(&id.as_str()) || custom.iter().any(|c| &c.id == id));
    };
    keep(&mut config.templates, &config.custom);
    if config.templates.is_empty() {
        config.templates = templates::DEFAULT
            .iter()
            .map(|id| (*id).to_owned())
            .collect();
    }
    let custom = config.custom.clone();
    for ids in config.screens.values_mut() {
        keep(ids, &custom);
    }
    config.screens.retain(|_, ids| !ids.is_empty());
    Ok(())
}

/// This feature's shortcuts as (target, shortcut). It has none.
pub fn shortcuts(_config: &Config) -> Vec<(String, String)> {
    Vec::new()
}

/// Stores a shortcut that was already parsed and checked for conflicts. It has none.
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

    fn mine(id: &str) -> Value {
        json!({ "id": id, "name": "Mine", "cols": 2, "rows": 1, "cells": [[0, 0, 1, 1], [1, 0, 2, 1]] })
    }

    fn with_one() -> Config {
        let mut config = Config::default();
        assert!(
            set(&mut config, "custom", &json!([mine("c1")]))
                .unwrap()
                .is_ok()
        );
        config
    }

    #[test]
    fn the_switch_is_validated() {
        let mut config = Config::default();
        assert!(set(&mut config, "enabled", &json!(true)).unwrap().is_ok() && config.enabled);
        assert!(set(&mut config, "enabled", &json!("yes")).unwrap().is_err());
        assert!(set(&mut config, "nonsense", &json!(1)).is_none());
    }

    #[test]
    fn layouts_are_stored_in_the_order_the_row_shows_them() {
        let mut config = with_one();
        let result = set(
            &mut config,
            "templates",
            &json!(["c1", "quarters", "full", "halves"]),
        );
        assert!(result.unwrap().is_ok());
        assert_eq!(config.templates, ["full", "halves", "quarters", "c1"]);
        let all: Vec<&str> = BUILT_IN.iter().copied().chain(["c1"]).collect();
        assert!(set(&mut config, "templates", &json!(all)).unwrap().is_ok());
        assert_eq!(config.templates, all, "every one fits");
    }

    #[test]
    fn bad_lists_are_refused_and_change_nothing() {
        let before = with_one();
        let mut config = before.clone();
        for bad in [
            json!([]),
            json!("halves"),
            json!(null),
            json!([1, 2]),
            json!(["halves", "halves"]),
            json!(["halves", "bogus"]),
            json!(["c2"]),
            json!(["center"]),
            json!(["../../etc/passwd"]),
        ] {
            for path in ["templates", "screens"] {
                let value = if path == "screens" {
                    json!({ "Display": bad })
                } else {
                    bad.clone()
                };
                let result = set(&mut config, path, &value).unwrap();
                assert!(result.is_err(), "{path}: {bad}");
                assert_eq!(config, before, "{path}: {bad}");
            }
        }
    }

    #[test]
    fn a_screen_can_have_a_list_of_its_own() {
        let mut config = with_one();
        let value = json!({ "DELL U2720Q": ["c1", "thirds"], "Built-in Retina Display": ["full"] });
        assert!(set(&mut config, "screens", &value).unwrap().is_ok());
        assert_eq!(config.screens["DELL U2720Q"], ["thirds", "c1"]);
        assert_eq!(config.screens["Built-in Retina Display"], ["full"]);
        // Sending none gives every screen the common list again.
        assert!(set(&mut config, "screens", &json!({})).unwrap().is_ok());
        assert!(config.screens.is_empty());
        for bad in [
            json!([]),
            json!({ "": ["full"] }),
            json!({ "   ": ["full"] }),
            json!({ "a\nb": ["full"] }),
            json!({ "x".repeat(MAX_SCREEN_NAME + 1): ["full"] }),
            json!({ "Display": [] }),
        ] {
            assert!(set(&mut config, "screens", &bad).unwrap().is_err(), "{bad}");
        }
        let many: serde_json::Map<String, Value> = (0..=MAX_SCREENS)
            .map(|i| (format!("Screen {i}"), json!(["full"])))
            .collect();
        assert!(
            set(&mut config, "screens", &Value::Object(many))
                .unwrap()
                .is_err()
        );
    }

    #[test]
    fn layouts_made_here_are_checked_and_replace_the_old_ones() {
        let mut config = Config::default();
        assert!(
            set(&mut config, "custom", &json!([mine("c1"), mine("c2")]))
                .unwrap()
                .is_ok()
        );
        assert_eq!(config.custom.len(), 2);
        let before = config.clone();
        let broken =
            json!({ "id": "c3", "name": "Gap", "cols": 2, "rows": 1, "cells": [[0, 0, 1, 1]] });
        for bad in [
            json!([mine("c1"), mine("c1")]),
            json!([broken]),
            json!([mine("left")]),
            json!([{ "id": "c4" }]),
            json!([{ "id": "c4", "name": "x", "cols": "2", "rows": 1, "cells": [] }]),
            json!("c1"),
            json!({}),
            Value::Array(
                (1..=MAX_CUSTOM + 1)
                    .map(|i| mine(&format!("c{i}")))
                    .collect(),
            ),
        ] {
            assert!(set(&mut config, "custom", &bad).unwrap().is_err(), "{bad}");
            assert_eq!(config, before, "{bad}");
        }
        let twelve = Value::Array((1..=MAX_CUSTOM).map(|i| mine(&format!("c{i}"))).collect());
        assert!(set(&mut config, "custom", &twelve).unwrap().is_ok());
    }

    #[test]
    fn deleting_a_layout_takes_it_out_of_every_list_that_offered_it() {
        let mut config = with_one();
        set(&mut config, "templates", &json!(["halves", "c1"]))
            .unwrap()
            .unwrap();
        set(
            &mut config,
            "screens",
            &json!({ "A": ["c1"], "B": ["c1", "full"] }),
        )
        .unwrap()
        .unwrap();
        assert!(set(&mut config, "custom", &json!([])).unwrap().is_ok());
        assert_eq!(config.templates, ["halves"]);
        assert_eq!(
            config.screens.keys().collect::<Vec<_>>(),
            ["B"],
            "a list left empty is gone"
        );
        assert_eq!(config.screens["B"], ["full"]);
        // The common list left empty is the default again.
        let mut config = with_one();
        set(&mut config, "templates", &json!(["c1"]))
            .unwrap()
            .unwrap();
        set(&mut config, "custom", &json!([])).unwrap().unwrap();
        assert_eq!(config.templates, Config::default().templates);
    }

    #[test]
    fn the_state_shows_what_will_be_offered_and_what_the_page_needs() {
        let mut config = with_one();
        config.templates = vec!["bogus".into(), "c1".into(), "halves".into(), "c9".into()];
        config.screens.insert("A".into(), vec!["nothing".into()]);
        let shown = state(
            &config,
            &json!({ "accessibility": true, "displays": ["A", "B"] }),
        );
        assert_eq!(shown["templates"], json!(["halves", "c1"]));
        assert!(
            shown["screens"].as_object().unwrap().is_empty(),
            "nothing usable on the list of a screen"
        );
        let empty = Config {
            templates: vec!["bogus".into()],
            ..Config::default()
        };
        assert_eq!(
            state(&empty, &Value::Null)["templates"],
            json!(templates::DEFAULT)
        );
        assert_eq!(shown["custom"][0]["id"], "c1");
        assert_eq!(shown["displays"], json!(["A", "B"]));
        assert_eq!(shown["accessibility"], true);
        let built_in = shown["built_in"].as_array().unwrap();
        assert_eq!(built_in.len(), BUILT_IN.len());
        assert_eq!(built_in[0]["id"], "full");
        assert_eq!(built_in[0]["cells"], json!([[0.0, 0.0, 1.0, 1.0]]));
        let none = state(&config, &Value::Null);
        assert_eq!(
            (none["accessibility"].clone(), none["displays"].clone()),
            (json!(false), json!([]))
        );
    }

    #[test]
    fn a_layout_that_does_not_check_out_is_not_shown() {
        let config = Config {
            custom: vec![
                serde_json::from_value(mine("c1")).unwrap(),
                serde_json::from_value(
                    json!({ "id": "c2", "name": "Bad", "cols": 9, "rows": 1, "cells": [] }),
                )
                .unwrap(),
            ],
            ..Config::default()
        };
        assert_eq!(
            state(&config, &Value::Null)["custom"]
                .as_array()
                .unwrap()
                .len(),
            1
        );
    }

    #[test]
    fn the_sample_data_has_the_keys_of_the_state() {
        let sample: Value =
            serde_json::from_str(include_str!("../../../web/features/snap.fixture.json")).unwrap();
        let real = state(&Config::default(), &json!({ "accessibility": true }));
        let names = |v: &Value| {
            let mut k: Vec<String> = v.as_object().unwrap().keys().cloned().collect();
            k.sort();
            k
        };
        assert_eq!(names(&sample), names(&real));
        assert_eq!(
            sample["built_in"], real["built_in"],
            "the drawings of the built-in layouts are the real ones"
        );
    }
}
