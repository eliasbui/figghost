//! This feature's part of the settings model: the JSON the page renders and the changes it may make.
//! Pure data with no AppKit, so every rule is covered by plain unit tests.

use super::{Config, Mode};
use crate::settings::model::Fault;
use core_capture::naming::{tilde, validate_folder};
use serde_json::{Map, Value, json};

/// The state under the key "capture". `runtime` is `Feature::runtime()`, or null when the feature is not running.
/// `web/features/capture.fixture.json` must have exactly the same keys; a unit test checks it.
pub fn state(config: &Config, runtime: &Value) -> Value {
    let flag = |key: &str| runtime.get(key).and_then(Value::as_bool);
    let shortcuts: Map<String, Value> = Mode::ALL
        .into_iter()
        .map(|mode| (mode.key().to_owned(), json!(config.shortcut(mode))))
        .collect();
    let home = std::env::var_os("HOME")
        .map(std::path::PathBuf::from)
        .unwrap_or_default();
    json!({
        "enabled": config.enabled,
        "copy": config.copy,
        "save": config.save,
        "edit": config.edit,
        "window_shadow": config.window_shadow,
        "folder": tilde(&config.save_dir(), &home),
        "custom_folder": !config.folder.trim().is_empty(),
        "shortcuts": shortcuts,
        "permission": flag("permission").unwrap_or(false),
        "available": flag("available").unwrap_or(true),
    })
}

/// A change from the page. `path` is relative to the feature: "enabled", not "capture.enabled". `None` when the
/// path is not one of this feature's. Values must be validated here; the page is not trusted.
pub fn set(config: &mut Config, path: &str, value: &Value) -> Option<Result<(), Fault>> {
    let flag = |slot: &mut bool| {
        value
            .as_bool()
            .map(|on| *slot = on)
            .ok_or_else(|| Fault::new("invalid_value", "expected true or false"))
    };
    Some(match path {
        "enabled" => flag(&mut config.enabled),
        "copy" => flag(&mut config.copy),
        "save" => flag(&mut config.save),
        "edit" => flag(&mut config.edit),
        "window_shadow" => flag(&mut config.window_shadow),
        // An empty folder goes back to the default one.
        "folder" => match value.as_str() {
            Some(text) if text.trim().is_empty() => {
                config.folder.clear();
                Ok(())
            }
            Some(text) => validate_folder(text)
                .map(|clean| config.folder = clean.to_string_lossy().into_owned())
                .map_err(|reason| Fault::new("folder_invalid", reason)),
            None => Err(Fault::new("invalid_value", "expected a folder path")),
        },
        _ => return None,
    })
}

/// This feature's shortcuts as (target, shortcut). A target is "capture" or "capture:<what>", which is what the
/// page's shortcut recorder sends. Shortcuts are checked against all the others before `set_shortcut` runs.
pub fn shortcuts(config: &Config) -> Vec<(String, String)> {
    Mode::ALL
        .into_iter()
        .map(|mode| (format!("capture:{}", mode.key()), config.shortcut(mode)))
        .filter(|(_, shortcut)| !shortcut.is_empty())
        .collect()
}

/// Stores a shortcut that was already parsed and checked for conflicts; an empty value removes it.
pub fn set_shortcut(config: &mut Config, target: &str, value: &str) -> Option<Result<(), Fault>> {
    let mode = target.strip_prefix("capture:").and_then(Mode::from_key);
    let Some(mode) = mode else {
        return Some(Err(Fault::new("unknown_setting", target)));
    };
    let value = value.trim();
    if value == mode.default_shortcut() {
        // Back to the default: leave no override behind.
        config.shortcuts.remove(mode.key());
    } else {
        config
            .shortcuts
            .insert(mode.key().to_owned(), value.to_owned());
    }
    Some(Ok(()))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn keys(value: &Value, prefix: &str, out: &mut Vec<String>) {
        if let Value::Object(map) = value {
            for (key, inner) in map {
                let path = format!("{prefix}.{key}");
                out.push(path.clone());
                keys(inner, &path, out);
            }
        }
    }

    #[test]
    fn the_switches_are_validated() {
        let mut config = Config::default();
        for path in ["enabled", "copy", "save", "edit", "window_shadow"] {
            assert!(
                set(&mut config, path, &json!("yes")).unwrap().is_err(),
                "{path}"
            );
            assert!(
                set(&mut config, path, &json!(1)).unwrap().is_err(),
                "{path}"
            );
        }
        assert!(set(&mut config, "enabled", &json!(true)).unwrap().is_ok() && config.enabled);
        assert!(set(&mut config, "copy", &json!(false)).unwrap().is_ok() && !config.copy);
        assert!(set(&mut config, "save", &json!(false)).unwrap().is_ok() && !config.save);
        assert!(set(&mut config, "edit", &json!(true)).unwrap().is_ok() && config.edit);
        assert!(
            set(&mut config, "window_shadow", &json!(false))
                .unwrap()
                .is_ok()
                && !config.window_shadow
        );
        assert!(set(&mut config, "nonsense", &json!(1)).is_none());
    }

    #[test]
    fn a_folder_from_the_page_is_checked() {
        let mut config = Config::default();
        assert!(
            set(&mut config, "folder", &json!("/Users/me/Shots/"))
                .unwrap()
                .is_ok()
        );
        assert_eq!(config.folder, "/Users/me/Shots");
        for bad in [
            "relative",
            "/Users/me/../../etc",
            "/System/Library",
            "",
            "   ",
        ] {
            let before = config.folder.clone();
            let result = set(&mut config, "folder", &json!(bad)).unwrap();
            if bad.trim().is_empty() {
                assert!(
                    result.is_ok() && config.folder.is_empty(),
                    "an empty folder resets it"
                );
                config.folder = before;
            } else {
                assert_eq!(result.unwrap_err().code, "folder_invalid", "{bad}");
                assert_eq!(config.folder, before, "{bad} changed the folder");
            }
        }
        for bad in [json!(5), json!(null), json!(["/a"]), json!({"a": 1})] {
            assert_eq!(
                set(&mut config, "folder", &bad).unwrap().unwrap_err().code,
                "invalid_value"
            );
        }
        assert_eq!(config.folder, "/Users/me/Shots");
    }

    #[test]
    fn the_state_has_every_switch_every_shortcut_and_the_permission() {
        let config = Config::default();
        let on = state(&config, &json!({"permission": true, "available": true}));
        assert_eq!(on["permission"], true);
        let off = state(&config, &Value::Null);
        assert_eq!(off["permission"], false, "unknown means not granted");
        assert_eq!(off["available"], true, "unknown means available");
        for mode in Mode::ALL {
            assert_eq!(on["shortcuts"][mode.key()], mode.default_shortcut());
        }
        assert_eq!(on["custom_folder"], false);
        assert!(
            on["folder"]
                .as_str()
                .unwrap()
                .ends_with("Pictures/Figghost")
        );
    }

    #[test]
    fn the_fixture_has_exactly_the_keys_of_the_state() {
        let fixture: Value =
            serde_json::from_str(include_str!("../../../web/features/capture.fixture.json"))
                .unwrap();
        let (mut a, mut b) = (Vec::new(), Vec::new());
        keys(&fixture, "", &mut a);
        keys(&state(&Config::default(), &Value::Null), "", &mut b);
        a.sort();
        b.sort();
        assert_eq!(
            a, b,
            "capture.fixture.json is out of date with settings::state"
        );
    }

    #[test]
    fn the_shortcuts_are_listed_under_their_targets_without_the_removed_ones() {
        let mut config = Config::default();
        let all = shortcuts(&config);
        assert_eq!(all.len(), 5);
        assert!(all.contains(&("capture:region".into(), "ctrl+opt+cmd+4".into())));
        config.shortcuts.insert("text".into(), String::new());
        assert_eq!(shortcuts(&config).len(), 4);
    }

    #[test]
    fn a_shortcut_is_stored_as_an_override_and_the_default_leaves_none() {
        let mut config = Config::default();
        set_shortcut(&mut config, "capture:region", "f13")
            .unwrap()
            .unwrap();
        assert_eq!(config.shortcut(Mode::Region), "f13");
        set_shortcut(&mut config, "capture:region", "ctrl+opt+cmd+4")
            .unwrap()
            .unwrap();
        assert!(config.shortcuts.is_empty());
        set_shortcut(&mut config, "capture:window", "")
            .unwrap()
            .unwrap();
        assert_eq!(config.shortcut(Mode::Window), "");
        assert_eq!(config.shortcuts.get("window").map(String::as_str), Some(""));
        assert_eq!(
            set_shortcut(&mut config, "capture:nonsense", "f1")
                .unwrap()
                .unwrap_err()
                .code,
            "unknown_setting"
        );
        assert_eq!(
            set_shortcut(&mut config, "capture", "f1")
                .unwrap()
                .unwrap_err()
                .code,
            "unknown_setting"
        );
    }
}
