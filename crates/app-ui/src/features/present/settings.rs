//! This feature's part of the settings model: the JSON the page renders and the changes it may make.
//! Pure data with no AppKit, so every rule is covered by plain unit tests.

use super::{Action, Config, FPS, Kind, SPOTLIGHTS, ZOOM_OUTS, ZOOMS};
use crate::{config::parse_hotkey, settings::model::Fault};
use serde_json::{Map, Value, json};

/// The state under the key "present". `runtime` is `Feature::runtime()`, or null when the feature is not running.
/// `web/features/present.fixture.json` must have exactly the same keys; a unit test checks it.
pub fn state(config: &Config, runtime: &Value) -> Value {
    let config = config.clone().sanitized();
    let shortcuts: Map<String, Value> = Action::ALL
        .into_iter()
        .map(|a| (a.key().to_owned(), json!(config.shortcut(a))))
        .collect();
    let fact = |key: &str| runtime.get(key).and_then(Value::as_bool).unwrap_or(false);
    json!({
        "enabled": config.enabled,
        "source": config.source,
        "cursor": config.cursor,
        "border": config.border,
        "fps": config.fps,
        "auto_zoom": config.auto_zoom,
        "zoom": config.zoom,
        "zoom_out": config.zoom_out,
        "ring": config.ring,
        "clicks": config.clicks,
        "spotlight": config.spotlight,
        "closed_notch": config.closed_notch,
        "shortcuts": shortcuts,
        "screen_access": fact("screen_access"),
        "presenting": fact("presenting"),
    })
}

/// A change from the page. `path` is relative to the feature. Values must be validated here; the page is not
/// trusted.
pub fn set(config: &mut Config, path: &str, value: &Value) -> Option<Result<(), Fault>> {
    let bad = || Fault::new("invalid_value", path);
    let switch = |slot: &mut bool| value.as_bool().map(|on| *slot = on).ok_or_else(bad);
    let number = |slot: &mut u32, list: &[u32]| {
        value
            .as_u64()
            .and_then(|n| u32::try_from(n).ok())
            .filter(|n| list.contains(n))
            .map(|n| *slot = n)
            .ok_or_else(bad)
    };
    Some(match path {
        "enabled" => switch(&mut config.enabled),
        "cursor" => switch(&mut config.cursor),
        "border" => switch(&mut config.border),
        "auto_zoom" => switch(&mut config.auto_zoom),
        "ring" => switch(&mut config.ring),
        "clicks" => switch(&mut config.clicks),
        "closed_notch" => switch(&mut config.closed_notch),
        "fps" => number(&mut config.fps, &FPS),
        "zoom_out" => number(&mut config.zoom_out, &ZOOM_OUTS),
        "spotlight" => number(&mut config.spotlight, &SPOTLIGHTS),
        "zoom" => value
            .as_f64()
            .filter(|z| ZOOMS.iter().any(|offered| (offered - z).abs() < 1e-9))
            .map(|z| config.zoom = z)
            .ok_or_else(bad),
        "source" => value
            .as_str()
            .filter(|s| Kind::ALL.iter().any(|k| k.key() == *s))
            .map(|s| config.source = s.to_owned())
            .ok_or_else(bad),
        _ => return None,
    })
}

/// This feature's shortcuts as (target, shortcut), the ones turned off left out.
pub fn shortcuts(config: &Config) -> Vec<(String, String)> {
    Action::ALL
        .into_iter()
        .map(|a| (format!("present:{}", a.key()), config.shortcut(a)))
        .filter(|(_, shortcut)| !shortcut.is_empty())
        .collect()
}

/// Stores a shortcut that was already parsed and checked for conflicts; an empty value turns it off.
pub fn set_shortcut(config: &mut Config, target: &str, value: &str) -> Option<Result<(), Fault>> {
    let action = Action::from_key(target.strip_prefix("present:")?)?;
    let value = value.trim().to_owned();
    if !value.is_empty() && parse_hotkey(&value).is_err() {
        return Some(Err(Fault::new("invalid_shortcut", target)));
    }
    config.shortcuts.insert(action.key().to_owned(), value);
    Some(Ok(()))
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
            serde_json::from_str(include_str!("../../../web/features/present.fixture.json"))
                .unwrap();
        let state = state(&Config::default(), &Value::Null);
        assert_eq!(keys(&fixture), keys(&state));
        assert_eq!(keys(&fixture["shortcuts"]), keys(&state["shortcuts"]));
    }

    #[test]
    fn the_state_has_the_defaults_and_what_the_running_feature_knows() {
        let s = state(&Config::default(), &Value::Null);
        assert_eq!(s["enabled"], json!(false));
        assert_eq!(
            (s["zoom"].clone(), s["zoom_out"].clone()),
            (json!(2.0), json!(3))
        );
        assert_eq!(s["shortcuts"]["start"], json!("ctrl+opt+p"));
        assert_eq!(s["screen_access"], json!(false));
        let s = state(
            &Config::default(),
            &json!({ "screen_access": true, "presenting": true }),
        );
        assert_eq!(
            (s["screen_access"].clone(), s["presenting"].clone()),
            (json!(true), json!(true))
        );
    }

    #[test]
    fn the_switches_and_choices_take_only_what_is_offered() {
        let mut config = Config::default();
        for path in [
            "enabled",
            "cursor",
            "border",
            "auto_zoom",
            "ring",
            "clicks",
            "closed_notch",
        ] {
            assert!(
                set(&mut config, path, &json!("yes")).unwrap().is_err(),
                "{path}"
            );
            assert!(
                set(&mut config, path, &json!(true)).unwrap().is_ok(),
                "{path}"
            );
        }
        assert!(set(&mut config, "fps", &json!(30)).unwrap().is_ok());
        assert!(set(&mut config, "fps", &json!(240)).unwrap().is_err());
        assert!(set(&mut config, "zoom", &json!(2.5)).unwrap().is_ok());
        assert!(set(&mut config, "zoom", &json!(2.6)).unwrap().is_err());
        assert!(set(&mut config, "zoom", &json!("2")).unwrap().is_err());
        assert!(set(&mut config, "zoom_out", &json!(0)).unwrap().is_ok());
        assert!(set(&mut config, "zoom_out", &json!(-1)).unwrap().is_err());
        assert!(set(&mut config, "spotlight", &json!(26)).unwrap().is_ok());
        assert!(set(&mut config, "spotlight", &json!(50)).unwrap().is_err());
        assert!(set(&mut config, "source", &json!("app")).unwrap().is_ok());
        assert!(
            set(&mut config, "source", &json!("everything"))
                .unwrap()
                .is_err()
        );
        assert_eq!(
            (
                config.fps,
                config.zoom,
                config.zoom_out,
                config.spotlight,
                config.source.as_str()
            ),
            (30, 2.5, 0, 26, "app"),
            "a refused value changes nothing"
        );
        assert!(set(&mut config, "nonsense", &json!(1)).is_none());
    }

    #[test]
    fn a_shortcut_is_stored_turned_off_and_refused_when_it_is_nonsense() {
        let mut config = Config::default();
        assert_eq!(
            shortcuts(&config).len(),
            Action::ALL.len(),
            "every action has a key at first"
        );
        assert!(
            set_shortcut(&mut config, "present:black", "ctrl+opt+cmd+b")
                .unwrap()
                .is_ok()
        );
        assert_eq!(config.shortcut(Action::Black), "ctrl+opt+cmd+b");
        assert!(
            set_shortcut(&mut config, "present:black", "nonsense")
                .unwrap()
                .is_err()
        );
        assert_eq!(config.shortcut(Action::Black), "ctrl+opt+cmd+b");
        assert!(
            set_shortcut(&mut config, "present:white", "")
                .unwrap()
                .is_ok()
        );
        assert!(
            !shortcuts(&config).iter().any(|(t, _)| t == "present:white"),
            "turned off"
        );
        assert!(set_shortcut(&mut config, "present:launch", "ctrl+opt+q").is_none());
        assert!(set_shortcut(&mut config, "pin", "ctrl+opt+q").is_none());
    }
}
