//! This feature's part of the settings model: the JSON the page renders and the changes it may make.
//! Pure data with no AppKit, so every rule is covered by plain unit tests. The list is changed by `set` with the
//! paths `add`, `update` and `remove`, since an item carries a text that a button's name cannot.

use super::{Config, Item, Refusal, check, lf};
use crate::settings::model::Fault;
use core_typing::{MAX_ABBR, MAX_SNIPPETS, MAX_TEXT};
use serde_json::{Value, json};

/// The abbreviation of another item that is typed first and takes the place of this one, if there is one: an
/// expansion happens at the last key of an abbreviation, so one that holds another before its own last key is
/// replaced before it can be finished.
fn shadowed_by<'a>(items: &'a [Item], item: &Item) -> Option<&'a str> {
    // Abbreviations are ASCII once they are checked, so cutting off the last byte cuts off the last character.
    let head = item.abbr.get(..item.abbr.len().saturating_sub(1))?;
    items
        .iter()
        .find(|other| other.abbr != item.abbr && head.contains(&other.abbr))
        .map(|other| other.abbr.as_str())
}

/// The state under the key "snippets". `runtime` is `Feature::runtime()`, or null when the feature is not running.
/// `web/features/snippets.fixture.json` must have exactly the same keys; a unit test checks it.
pub fn state(config: &Config, runtime: &Value) -> Value {
    let config = config.clone().sanitized();
    let flag = |key: &str| runtime.get(key).and_then(Value::as_bool).unwrap_or(false);
    json!({
        "enabled": config.enabled,
        "items": config.items.iter().map(|i| json!({
            "abbr": i.abbr,
            "text": i.text,
            "shadowed_by": shadowed_by(&config.items, i).unwrap_or_default(),
        })).collect::<Vec<_>>(),
        "max_items": MAX_SNIPPETS,
        "max_abbr": MAX_ABBR,
        "max_text": MAX_TEXT,
        "running": flag("running"),
        "accessibility": flag("accessibility"),
        "other": runtime.get("other").cloned().unwrap_or(Value::Null),
    })
}

fn text_field(value: &Value, name: &str) -> Result<String, Fault> {
    value
        .get(name)
        .and_then(Value::as_str)
        .map(str::to_owned)
        .ok_or_else(|| Fault::new("invalid_value", format!("expected {name} as text")))
}

/// A change from the page. `path` is relative to the feature. Values must be validated here; the page is not
/// trusted. `None` when the path is not one of this feature's.
pub fn set(config: &mut Config, path: &str, value: &Value) -> Option<Result<(), Fault>> {
    Some(match path {
        "enabled" => value
            .as_bool()
            .map(|on| config.enabled = on)
            .ok_or_else(|| Fault::new("invalid_value", "expected true or false")),
        "add" => add(config, value),
        "update" => update(config, value),
        "remove" => value
            .as_str()
            .map(|abbr| config.items.retain(|i| i.abbr != abbr))
            .ok_or_else(|| Fault::new("invalid_value", "expected an abbreviation")),
        _ => return None,
    })
}

fn add(config: &mut Config, value: &Value) -> Result<(), Fault> {
    let abbr = text_field(value, "abbr")?;
    let text = lf(&text_field(value, "text")?);
    check(&abbr, &text).map_err(Refusal::fault)?;
    if config.items.iter().any(|i| i.abbr == abbr) {
        return Err(Refusal::Duplicate.fault());
    }
    if config.items.len() >= MAX_SNIPPETS {
        return Err(Refusal::Full.fault());
    }
    config.items.push(Item { abbr, text });
    Ok(())
}

/// Replaces the item whose abbreviation is `from` where it stands in the list.
fn update(config: &mut Config, value: &Value) -> Result<(), Fault> {
    let from = text_field(value, "from")?;
    let abbr = text_field(value, "abbr")?;
    let text = lf(&text_field(value, "text")?);
    check(&abbr, &text).map_err(Refusal::fault)?;
    let Some(at) = config.items.iter().position(|i| i.abbr == from) else {
        return Err(Fault::new("invalid_value", "no such abbreviation"));
    };
    if config
        .items
        .iter()
        .enumerate()
        .any(|(i, other)| i != at && other.abbr == abbr)
    {
        return Err(Refusal::Duplicate.fault());
    }
    config.items[at] = Item { abbr, text };
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn keys(value: &Value) -> Vec<String> {
        let mut keys: Vec<String> = value.as_object().unwrap().keys().cloned().collect();
        keys.sort();
        keys
    }

    fn item(abbr: &str, text: &str) -> Item {
        Item {
            abbr: abbr.into(),
            text: text.into(),
        }
    }

    fn code(result: Option<Result<(), Fault>>) -> Option<&'static str> {
        result.unwrap().err().map(|f| f.code)
    }

    #[test]
    fn the_fixture_has_exactly_the_keys_of_the_state() {
        let fixture: Value =
            serde_json::from_str(include_str!("../../../web/features/snippets.fixture.json"))
                .unwrap();
        let config = Config {
            items: vec![item(";a", "A")],
            ..Config::default()
        };
        let state = state(&config, &Value::Null);
        assert_eq!(keys(&fixture), keys(&state));
        assert_eq!(keys(&fixture["items"][0]), keys(&state["items"][0]));
    }

    #[test]
    fn the_state_reports_the_config_and_what_is_running() {
        let config = Config {
            enabled: true,
            items: vec![item(";sig", "Best,\nAn")],
        };
        let runtime = json!({ "running": true, "accessibility": true, "other": "EVKey" });
        let on = state(&config, &runtime);
        assert_eq!(on["enabled"], true);
        assert_eq!(on["items"][0]["abbr"], ";sig");
        assert_eq!(on["items"][0]["text"], "Best,\nAn");
        assert_eq!(on["running"], true);
        assert_eq!(on["accessibility"], true);
        assert_eq!(on["other"], "EVKey");
        assert_eq!(on["max_items"], MAX_SNIPPETS);
        assert_eq!(on["max_abbr"], MAX_ABBR);
        assert_eq!(on["max_text"], MAX_TEXT);
        let idle = state(&config, &Value::Null);
        assert_eq!(idle["running"], false);
        assert_eq!(idle["other"], Value::Null);
    }

    #[test]
    fn an_abbreviation_that_cannot_be_finished_says_which_one_gets_in_its_way() {
        let items = vec![
            item(";s", "short"),
            item(";sig", "long"),
            item("sig", "tail"),
            item(";x", "other"),
            item("ab", "ab"),
            item(";abc", "holds ab"),
        ];
        let config = Config {
            enabled: true,
            items,
        };
        let state = state(&config, &Value::Null);
        let by = |i: usize| {
            state["items"][i]["shadowed_by"]
                .as_str()
                .unwrap()
                .to_owned()
        };
        assert_eq!(by(0), "", "nothing before its own last key");
        assert_eq!(by(1), ";s", ";s is finished at the second key");
        assert_eq!(
            by(2),
            "",
            "`sig` ends where `;sig` does: the longer wins, neither is in the way"
        );
        assert_eq!(by(3), "");
        assert_eq!(by(4), "");
        assert_eq!(by(5), "ab", "typed inside it");
    }

    #[test]
    fn every_change_is_validated() {
        let mut config = Config::default();
        assert_eq!(code(set(&mut config, "enabled", &json!(true))), None);
        assert!(config.enabled);
        for bad in [json!("yes"), json!(1), json!(null)] {
            assert_eq!(
                code(set(&mut config, "enabled", &bad)),
                Some("invalid_value")
            );
        }
        assert!(config.enabled, "a refused change changes nothing");
        assert!(set(&mut config, "nonsense", &json!(1)).is_none());
        assert!(set(&mut config, "items", &json!([])).is_none());
    }

    #[test]
    fn an_item_is_added_once_and_refused_for_a_reason() {
        let mut config = Config::default();
        let add = |config: &mut Config, abbr: &str, text: &str| {
            code(set(config, "add", &json!({ "abbr": abbr, "text": text })))
        };
        assert_eq!(add(&mut config, ";sig", "Best,\r\nAn"), None);
        assert_eq!(config.items, [item(";sig", "Best,\nAn")]);
        assert_eq!(
            add(&mut config, ";sig", "again"),
            Some("snippets_duplicate")
        );
        assert_eq!(add(&mut config, "", "x"), Some("snippets_empty"));
        assert_eq!(
            add(&mut config, &"x".repeat(MAX_ABBR + 1), "x"),
            Some("snippets_long")
        );
        assert_eq!(add(&mut config, ";a b", "x"), Some("snippets_space"));
        assert_eq!(add(&mut config, ";a\nb", "x"), Some("snippets_space"));
        assert_eq!(add(&mut config, ";đ", "x"), Some("snippets_untypeable"));
        assert_eq!(add(&mut config, ";ok", ""), Some("snippets_no_text"));
        assert_eq!(
            add(&mut config, ";ok", &"y".repeat(MAX_TEXT + 1)),
            Some("snippets_text_long")
        );
        assert_eq!(add(&mut config, ";ok", &"y".repeat(MAX_TEXT)), None);
        assert_eq!(config.items.len(), 2, "refused items are not added");
        for bad in [
            json!({ "abbr": ";a" }),
            json!({ "text": "x" }),
            json!("x"),
            json!(null),
        ] {
            assert_eq!(code(set(&mut config, "add", &bad)), Some("invalid_value"));
        }
        config.items = (0..MAX_SNIPPETS)
            .map(|i| item(&format!(";k{i}"), "t"))
            .collect();
        assert_eq!(add(&mut config, ";more", "x"), Some("snippets_full"));
    }

    #[test]
    fn an_item_is_edited_where_it_stands_and_removed_by_its_abbreviation() {
        let mut config = Config {
            enabled: true,
            items: vec![item(";a", "A"), item(";b", "B"), item(";c", "C")],
        };
        let update = |config: &mut Config, from: &str, abbr: &str, text: &str| {
            code(set(
                config,
                "update",
                &json!({ "from": from, "abbr": abbr, "text": text }),
            ))
        };
        assert_eq!(update(&mut config, ";b", ";bb", "new"), None);
        assert_eq!(
            config.items[1],
            item(";bb", "new"),
            "same place in the list"
        );
        assert_eq!(update(&mut config, ";bb", ";bb", "only the text"), None);
        assert_eq!(config.items[1].text, "only the text");
        assert_eq!(
            update(&mut config, ";bb", ";c", "x"),
            Some("snippets_duplicate")
        );
        assert_eq!(
            update(&mut config, ";bb", "a b", "x"),
            Some("snippets_space")
        );
        assert_eq!(
            update(&mut config, ";bb", ";bb", ""),
            Some("snippets_no_text")
        );
        assert_eq!(update(&mut config, ";zz", ";y", "x"), Some("invalid_value"));
        assert_eq!(
            config.items[1],
            item(";bb", "only the text"),
            "refused edits change nothing"
        );
        assert_eq!(code(set(&mut config, "remove", &json!(";a"))), None);
        assert_eq!(code(set(&mut config, "remove", &json!(";absent"))), None);
        assert_eq!(
            config
                .items
                .iter()
                .map(|i| i.abbr.as_str())
                .collect::<Vec<_>>(),
            [";bb", ";c"]
        );
        assert_eq!(
            code(set(&mut config, "remove", &json!(1))),
            Some("invalid_value")
        );
        assert!(config.enabled);
    }
}
