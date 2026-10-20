//! This feature's part of the settings model: the JSON the page renders and the changes it may make.
//! Pure data with no AppKit, so every rule is covered by plain unit tests.

use super::{Config, MINUTES};
use crate::settings::model::Fault;
use serde_json::{Value, json};

/// The state under the key "prs". `runtime` is `Feature::runtime()`, or null when the feature is not running.
/// `web/features/prs.fixture.json` must have exactly the same keys; a unit test checks it.
pub fn state(config: &Config, runtime: &Value) -> Value {
    let config = config.clone().sanitized();
    let text = |key: &str| {
        runtime
            .get(key)
            .and_then(Value::as_str)
            .unwrap_or("unknown")
            .to_owned()
    };
    json!({
        "enabled": config.enabled,
        "github": config.github,
        "gitlab": config.gitlab,
        "drafts": config.drafts,
        "minutes": config.minutes,
        "github_tool": text("github_tool"),
        "gitlab_tool": text("gitlab_tool"),
        "checking": runtime.get("checking").and_then(Value::as_bool).unwrap_or(false),
        "count": runtime.get("count").and_then(Value::as_u64).unwrap_or(0),
    })
}

/// A change from the page. `path` is relative to the feature. Values must be validated here; the page is not
/// trusted.
pub fn set(config: &mut Config, path: &str, value: &Value) -> Option<Result<(), Fault>> {
    let bad = || Fault::new("invalid_value", path);
    let switch = |slot: &mut bool| value.as_bool().map(|on| *slot = on).ok_or_else(bad);
    Some(match path {
        "enabled" => switch(&mut config.enabled),
        "github" => switch(&mut config.github),
        "gitlab" => switch(&mut config.gitlab),
        "drafts" => switch(&mut config.drafts),
        "minutes" => value
            .as_u64()
            .and_then(|n| u32::try_from(n).ok())
            .filter(|n| MINUTES.contains(n))
            .map(|n| config.minutes = n)
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
            serde_json::from_str(include_str!("../../../web/features/prs.fixture.json")).unwrap();
        assert_eq!(
            keys(&fixture),
            keys(&state(&Config::default(), &Value::Null))
        );
    }

    #[test]
    fn the_state_starts_off_and_says_what_the_running_feature_knows() {
        let s = state(&Config::default(), &Value::Null);
        assert_eq!(s["enabled"], json!(false));
        assert_eq!(
            (
                s["github"].clone(),
                s["gitlab"].clone(),
                s["drafts"].clone(),
                s["minutes"].clone()
            ),
            (json!(true), json!(true), json!(false), json!(10))
        );
        assert_eq!(
            (
                s["github_tool"].clone(),
                s["checking"].clone(),
                s["count"].clone()
            ),
            (json!("unknown"), json!(false), json!(0))
        );
        let runtime = json!({ "github_tool": "ready", "gitlab_tool": "signed_out", "checking": true, "count": 3 });
        let s = state(&Config::default(), &runtime);
        assert_eq!(
            (
                s["github_tool"].clone(),
                s["gitlab_tool"].clone(),
                s["checking"].clone(),
                s["count"].clone()
            ),
            (json!("ready"), json!("signed_out"), json!(true), json!(3))
        );
        let hostile = Config {
            minutes: 4_000_000,
            ..Config::default()
        };
        assert_eq!(state(&hostile, &Value::Null)["minutes"], json!(10));
    }

    #[test]
    fn the_switches_and_the_interval_are_validated() {
        let mut config = Config::default();
        for path in ["enabled", "github", "gitlab", "drafts"] {
            assert!(
                set(&mut config, path, &json!("yes")).unwrap().is_err(),
                "{path}"
            );
            assert!(
                set(&mut config, path, &json!(null)).unwrap().is_err(),
                "{path}"
            );
        }
        assert!(set(&mut config, "enabled", &json!(true)).unwrap().is_ok() && config.enabled);
        assert!(set(&mut config, "github", &json!(false)).unwrap().is_ok() && !config.github);
        for minutes in MINUTES {
            assert!(
                set(&mut config, "minutes", &json!(minutes))
                    .unwrap()
                    .is_ok()
            );
            assert_eq!(config.minutes, minutes);
        }
        for bad in [
            json!(0),
            json!(7),
            json!(-1),
            json!(2.5),
            json!("10"),
            json!(null),
            json!(1e12),
        ] {
            assert!(set(&mut config, "minutes", &bad).unwrap().is_err(), "{bad}");
        }
        assert_eq!(
            config.minutes,
            *MINUTES.last().unwrap(),
            "a refused value changes nothing"
        );
        assert!(set(&mut config, "nonsense", &json!(1)).is_none());
    }
}
