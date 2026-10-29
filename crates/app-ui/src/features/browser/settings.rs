//! This feature's part of the settings model: the JSON the page renders and the changes it may make.
//! Pure data with no AppKit, so every rule is covered by plain unit tests.

use super::{Config, logic};
use crate::settings::model::Fault;
use serde_json::{Value, json};

/// The state under the key "browser". `runtime` is `Feature::runtime()`, or null when the feature is not running.
/// `web/features/browser.fixture.json` must have exactly the same keys; a unit test checks it.
pub fn state(config: &Config, runtime: &Value) -> Value {
    let config = config.clone().sanitized();
    json!({
        "enabled": config.enabled,
        "block": config.block,
        "sites": config.sites.join("\n"),
        "download": config.download,
        "browsers": runtime.get("browsers").cloned().unwrap_or_else(|| json!([])),
        "connected": runtime.get("connected").and_then(Value::as_bool).unwrap_or(false),
        "blocking": runtime.get("blocking").and_then(Value::as_bool).unwrap_or(false),
        "folder": runtime.get("folder").and_then(Value::as_str).unwrap_or_default(),
        "extension_id": logic::EXTENSION_ID,
    })
}

/// A change from the page. `path` is relative to the feature. Values must be validated here; the page is not
/// trusted.
pub fn set(config: &mut Config, path: &str, value: &Value) -> Option<Result<(), Fault>> {
    let bad = || Fault::new("invalid_value", path);
    let switch = |slot: &mut bool| value.as_bool().map(|on| *slot = on).ok_or_else(bad);
    Some(match path {
        "enabled" => switch(&mut config.enabled),
        "block" => switch(&mut config.block),
        "download" => switch(&mut config.download),
        // The list as typed: whatever is not a site is dropped, and the page shows what was kept.
        "sites" => value
            .as_str()
            .filter(|text| text.len() <= 64 * 1024)
            .map(|text| config.sites = logic::sites(text))
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
            serde_json::from_str(include_str!("../../../web/features/browser.fixture.json"))
                .unwrap();
        assert_eq!(
            keys(&fixture),
            keys(&state(&Config::default(), &Value::Null))
        );
    }

    #[test]
    fn the_state_starts_off_and_shows_the_list_one_site_a_line() {
        let s = state(&Config::default(), &Value::Null);
        assert_eq!(s["enabled"], json!(false));
        assert_eq!(s["browsers"], json!([]));
        assert!(
            s["sites"]
                .as_str()
                .unwrap()
                .starts_with("youtube.com\nfacebook.com")
        );
        assert_eq!(s["extension_id"], json!(logic::EXTENSION_ID));
    }

    #[test]
    fn switches_and_the_list_are_validated() {
        let mut config = Config::default();
        for path in ["enabled", "block", "download"] {
            assert!(
                set(&mut config, path, &json!(1)).unwrap().is_err(),
                "{path}"
            );
            assert!(
                set(&mut config, path, &json!(true)).unwrap().is_ok(),
                "{path}"
            );
        }
        set(
            &mut config,
            "sites",
            &json!("vnexpress.net\nhttps://www.Zing.vn/a\nnot a site"),
        )
        .unwrap()
        .unwrap();
        assert_eq!(config.sites, ["vnexpress.net", "zing.vn"]);
        assert!(
            set(&mut config, "sites", &json!(["x.com"]))
                .unwrap()
                .is_err()
        );
        assert!(
            set(&mut config, "sites", &json!("x".repeat(70_000)))
                .unwrap()
                .is_err()
        );
        set(&mut config, "sites", &json!("")).unwrap().unwrap();
        assert!(config.sites.is_empty(), "an empty list blocks nothing");
        assert!(set(&mut config, "nonsense", &json!(1)).is_none());
    }
}
