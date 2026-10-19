//! This feature's part of the settings model: the JSON the page renders and the changes it may make.
//! Pure data with no AppKit, so every rule is covered by plain unit tests.

use super::{ACTIONS, Config, Kind};
use crate::settings::model::Fault;
use serde_json::{Map, Value, json};

/// The state under the key "capture". `runtime` is `Feature::runtime()`, or null when the feature is not running.
/// `web/features/capture.fixture.json` must have exactly the same keys; a unit test checks it.
pub fn state(config: &Config, runtime: &Value) -> Value {
    let mut shortcuts: Map<String, Value> = Kind::ALL
        .into_iter()
        .map(|kind| {
            let shortcut = config
                .shortcuts
                .get(kind.key())
                .map_or(kind.default_shortcut(), String::as_str);
            (kind.key().to_owned(), Value::from(shortcut.trim()))
        })
        .collect();
    for action in ACTIONS {
        let shortcut = config.shortcuts.get(action).map_or("", String::as_str);
        shortcuts.insert(action.to_owned(), Value::from(shortcut.trim()));
    }
    json!({
        "enabled": config.enabled,
        "folder_path": runtime["folder_path"].as_str().unwrap_or(""),
        "save": config.save,
        "copy": config.copy,
        "quick_panel": config.quick_panel,
        "quick_seconds": config.quick_seconds,
        "timer_seconds": config.timer_seconds,
        "shortcuts": shortcuts,
        "record": {
            "system_audio": config.record.system_audio,
            "microphone": config.record.microphone,
            "camera": config.record.camera,
            "clicks": config.record.clicks,
            "keys": config.record.keys,
        },
        "screen_access": runtime["screen_access"].as_bool().unwrap_or(false),
        "recording_supported": runtime["recording_supported"].as_bool().unwrap_or(false),
    })
}

fn whole_seconds(value: &Value, min: f64, max: f64) -> Result<f64, Fault> {
    value
        .as_f64()
        .filter(|v| v.is_finite())
        .map(|v| v.round().clamp(min, max))
        .ok_or_else(|| Fault::new("invalid_value", "expected a number of seconds"))
}

/// A change from the page. `path` is relative to the feature: "enabled", not "capture.enabled". `None` when the
/// path is not one of this feature's. Values must be validated here; the page is not trusted.
pub fn set(config: &mut Config, path: &str, value: &Value) -> Option<Result<(), Fault>> {
    let switch = match path {
        "quick_seconds" => {
            return Some(whole_seconds(value, 0.0, 600.0).map(|v| config.quick_seconds = v));
        }
        "timer_seconds" => {
            return Some(whole_seconds(value, 1.0, 60.0).map(|v| config.timer_seconds = v as u32));
        }
        "enabled" => &mut config.enabled,
        "save" => &mut config.save,
        "copy" => &mut config.copy,
        "quick_panel" => &mut config.quick_panel,
        "record.system_audio" => &mut config.record.system_audio,
        "record.microphone" => &mut config.record.microphone,
        "record.camera" => &mut config.record.camera,
        "record.clicks" => &mut config.record.clicks,
        "record.keys" => &mut config.record.keys,
        _ => return None,
    };
    Some(
        value
            .as_bool()
            .map(|on| *switch = on)
            .ok_or_else(|| Fault::new("invalid_value", "expected true or false")),
    )
}

/// This feature's shortcuts as (target, shortcut). A target is "capture" or "capture:<what>", which is what the
/// page's shortcut recorder sends. Shortcuts are checked against all the others before `set_shortcut` runs.
pub fn shortcuts(config: &Config) -> Vec<(String, String)> {
    let kinds = config
        .bindings()
        .into_iter()
        .map(|(kind, shortcut)| (format!("capture:{}", kind.key()), shortcut));
    let actions = config
        .action_bindings()
        .into_iter()
        .map(|(action, shortcut)| (format!("capture:{action}"), shortcut));
    kinds.chain(actions).collect()
}

/// Stores a shortcut that was already parsed and checked for conflicts; an empty value removes it, and the
/// built-in one stays off.
pub fn set_shortcut(config: &mut Config, target: &str, value: &str) -> Option<Result<(), Fault>> {
    let what = target.strip_prefix("capture:")?;
    let key = Kind::ALL
        .into_iter()
        .map(Kind::key)
        .chain(ACTIONS)
        .find(|k| *k == what)?;
    config.shortcuts.insert(key.to_owned(), value.trim().to_owned());
    Some(Ok(()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn switches_and_numbers_are_validated() {
        let mut config = Config::default();
        assert!(
            set(&mut config, "record.keys", &json!(true))
                .unwrap()
                .is_ok()
        );
        assert!(config.record.keys);
        assert!(set(&mut config, "save", &json!("yes")).unwrap().is_err());
        assert!(
            set(&mut config, "timer_seconds", &json!(999))
                .unwrap()
                .is_ok()
        );
        assert_eq!(config.timer_seconds, 60);
        assert!(set(&mut config, "quick_seconds", &json!(f64::NAN)).is_some());
        assert!(set(&mut config, "nonsense", &json!(1)).is_none());
    }

    #[test]
    fn shortcuts_show_the_built_in_ones_until_changed_or_removed() {
        let mut config = Config::default();
        assert_eq!(
            state(&config, &Value::Null)["shortcuts"]["area"],
            "cmd+shift+2"
        );
        assert!(shortcuts(&config).contains(&("capture:area".into(), "cmd+shift+2".into())));
        assert!(
            set_shortcut(&mut config, "capture:area", "")
                .unwrap()
                .is_ok()
        );
        assert!(shortcuts(&config).iter().all(|(t, _)| t != "capture:area"));
        assert!(
            set_shortcut(&mut config, "capture:record", "cmd+shift+8")
                .unwrap()
                .is_ok()
        );
        assert_eq!(
            state(&config, &Value::Null)["shortcuts"]["record"],
            "cmd+shift+8"
        );
        assert!(set_shortcut(&mut config, "capture:nothing", "cmd+k").is_none());
        assert!(shortcuts(&config).iter().all(|(t, _)| t != "capture:again"), "an action is off until it has a key");
        assert!(set_shortcut(&mut config, "capture:again", "cmd+shift+9").unwrap().is_ok());
        assert!(set_shortcut(&mut config, "capture:folder", "cmd+shift+0").unwrap().is_ok());
        assert!(shortcuts(&config).contains(&("capture:again".into(), "cmd+shift+9".into())));
        assert_eq!(state(&config, &Value::Null)["shortcuts"]["folder"], "cmd+shift+0");
        assert!(set_shortcut(&mut config, "tiling:left", "cmd+k").is_none());
    }

    #[test]
    fn the_sample_data_has_the_same_keys_as_the_state() {
        fn keys(value: &Value, prefix: &str, out: &mut Vec<String>) {
            if let Value::Object(map) = value {
                for (key, inner) in map {
                    let path = format!("{prefix}.{key}");
                    keys(inner, &path, out);
                    out.push(path);
                }
            }
        }
        let sample: Value =
            serde_json::from_str(include_str!("../../../web/features/capture.fixture.json"))
                .unwrap();
        let (mut a, mut b) = (Vec::new(), Vec::new());
        keys(&sample, "", &mut a);
        keys(&state(&Config::default(), &Value::Null), "", &mut b);
        a.sort();
        b.sort();
        assert_eq!(a, b);
    }
}
