//! This feature's part of the settings model: the JSON the page renders and the changes it may make.
//! Pure data with no AppKit, so every rule is covered by plain unit tests.

use super::{Config, MAX_APPS, Method, is_bundle_id};
use crate::settings::model::Fault;
use serde_json::{Value, json};

/// The state under the key "typing". `runtime` is `Feature::runtime()`, or null when the feature is not running.
/// `web/features/typing.fixture.json` must have exactly the same keys; a unit test checks it.
pub fn state(config: &Config, runtime: &Value) -> Value {
    let flag = |key: &str| runtime.get(key).and_then(Value::as_bool).unwrap_or(false);
    let apps = runtime.get("apps").cloned().unwrap_or_else(|| {
        config
            .apps
            .iter()
            .map(|id| json!({ "id": id, "name": id }))
            .collect()
    });
    json!({
        "enabled": config.enabled,
        "method": config.method.as_str(),
        "modern_tone": config.modern_tone,
        "english_restore": config.english_restore,
        "notch_badge": config.notch_badge,
        "tray_badge": config.tray_badge,
        "flag": config.flag,
        "waving_flag": config.waving_flag,
        "shortcut": config.shortcut,
        "apps": apps,
        "max_apps": MAX_APPS,
        "running": flag("running"),
        "vietnamese": flag("vietnamese"),
        "accessibility": flag("accessibility"),
        "other": runtime.get("other").cloned().unwrap_or(Value::Null),
    })
}

fn invalid(what: &'static str) -> Fault {
    Fault::new("invalid_value", what)
}

/// A change from the page. `path` is relative to the feature: "enabled", not "typing.enabled". `None` when the
/// path is not one of this feature's. Values must be validated here; the page is not trusted.
pub fn set(config: &mut Config, path: &str, value: &Value) -> Option<Result<(), Fault>> {
    let flag = |target: &mut bool| {
        value
            .as_bool()
            .map(|on| *target = on)
            .ok_or_else(|| invalid("expected true or false"))
    };
    Some(match path {
        "enabled" => flag(&mut config.enabled),
        "modern_tone" => flag(&mut config.modern_tone),
        "english_restore" => flag(&mut config.english_restore),
        "notch_badge" => flag(&mut config.notch_badge),
        "tray_badge" => flag(&mut config.tray_badge),
        "flag" => flag(&mut config.flag),
        "waving_flag" => flag(&mut config.waving_flag),
        "method" => value
            .as_str()
            .and_then(Method::parse)
            .map(|m| config.method = m)
            .ok_or_else(|| invalid("expected telex or vni")),
        _ => return None,
    })
}

/// The buttons of the list of apps: `typing_app_add:<bundle id>` and `typing_app_remove:<bundle id>`. `None` when
/// the name is not one of them.
pub fn apply_action(config: &mut Config, name: &str) -> Option<Result<(), Fault>> {
    let (verb, id) = name.strip_prefix("typing_app_")?.split_once(':')?;
    if !matches!(verb, "add" | "remove") {
        return None;
    }
    if !is_bundle_id(id) {
        return Some(Err(invalid("expected a bundle identifier")));
    }
    if verb == "remove" {
        config.apps.retain(|a| a != id);
        return Some(Ok(()));
    }
    if config.apps.iter().any(|a| a == id) {
        return Some(Ok(()));
    }
    if config.apps.len() >= MAX_APPS {
        return Some(Err(invalid("the list is full")));
    }
    config.apps.push(id.to_owned());
    Some(Ok(()))
}

/// The shortcut that switches between Vietnamese and English, under the target "typing".
pub fn shortcuts(config: &Config) -> Vec<(String, String)> {
    if config.shortcut.is_empty() {
        return Vec::new();
    }
    vec![("typing".to_owned(), config.shortcut.clone())]
}

/// Stores a shortcut that was already parsed and checked for conflicts; an empty value removes it.
pub fn set_shortcut(config: &mut Config, target: &str, value: &str) -> Option<Result<(), Fault>> {
    if target != "typing" {
        return None;
    }
    config.shortcut = value.trim().to_owned();
    Some(Ok(()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_fixture_has_the_keys_of_the_state() {
        let fixture: Value =
            serde_json::from_str(include_str!("../../../web/features/typing.fixture.json"))
                .unwrap();
        let keys = |v: &Value| {
            let mut keys: Vec<String> = v.as_object().unwrap().keys().cloned().collect();
            keys.sort();
            keys
        };
        assert_eq!(
            keys(&fixture),
            keys(&state(&Config::default(), &Value::Null))
        );
    }

    #[test]
    fn the_state_reports_the_config_and_what_is_running() {
        let config = Config {
            enabled: true,
            method: Method::Vni,
            apps: vec!["com.apple.Terminal".into()],
            ..Config::default()
        };
        let runtime = json!({
            "running": true,
            "vietnamese": true,
            "accessibility": true,
            "other": "EVKey",
            "apps": [{ "id": "com.apple.Terminal", "name": "Terminal" }],
        });
        let state = state(&config, &runtime);
        assert_eq!(state["enabled"], true);
        assert_eq!(state["method"], "vni");
        assert_eq!(state["shortcut"], "ctrl+space");
        assert_eq!(state["apps"][0]["name"], "Terminal");
        assert_eq!(state["running"], true);
        assert_eq!(state["vietnamese"], true);
        assert_eq!(state["other"], "EVKey");
        // Not running: the page still gets a complete state, with the listed apps by their identifiers.
        let idle = super::state(&config, &Value::Null);
        assert_eq!(idle["running"], false);
        assert_eq!(idle["other"], Value::Null);
        assert_eq!(idle["apps"][0]["name"], "com.apple.Terminal");
    }

    #[test]
    fn every_change_is_validated() {
        let mut config = Config::default();
        for path in [
            "enabled",
            "modern_tone",
            "english_restore",
            "notch_badge",
            "tray_badge",
            "flag",
            "waving_flag",
        ] {
            assert!(
                set(&mut config, path, &json!(false)).unwrap().is_ok(),
                "{path}"
            );
            assert!(
                set(&mut config, path, &json!("yes")).unwrap().is_err(),
                "{path}"
            );
            assert!(
                set(&mut config, path, &json!(1)).unwrap().is_err(),
                "{path}"
            );
        }
        assert!(!config.enabled && !config.modern_tone && !config.english_restore);
        assert!(!config.notch_badge && !config.tray_badge);
        assert!(!config.flag && !config.waving_flag);
        assert!(set(&mut config, "method", &json!("vni")).unwrap().is_ok());
        assert_eq!(config.method, Method::Vni);
        for bad in [json!("VNI"), json!("qwerty"), json!(1), json!(null)] {
            assert!(set(&mut config, "method", &bad).unwrap().is_err(), "{bad}");
        }
        assert_eq!(
            config.method,
            Method::Vni,
            "a refused change changes nothing"
        );
        assert!(
            set(&mut config, "shortcut", &json!("ctrl+space")).is_none(),
            "shortcuts go through the recorder"
        );
        assert!(set(&mut config, "nonsense", &json!(1)).is_none());
    }

    #[test]
    fn apps_are_added_once_removed_and_checked() {
        let mut config = Config::default();
        let act = |config: &mut Config, name: &str| {
            apply_action(config, name).map(|r| r.map_err(|f| f.code))
        };
        assert_eq!(
            act(&mut config, "typing_app_add:com.apple.Terminal"),
            Some(Ok(()))
        );
        assert_eq!(
            act(&mut config, "typing_app_add:com.apple.Terminal"),
            Some(Ok(())),
            "listed once"
        );
        assert_eq!(config.apps, ["com.apple.Terminal"]);
        assert_eq!(
            act(&mut config, "typing_app_add:com apple"),
            Some(Err("invalid_value"))
        );
        assert_eq!(
            act(&mut config, "typing_app_add:"),
            Some(Err("invalid_value"))
        );
        assert_eq!(
            act(&mut config, "typing_app_remove:com.apple.Terminal"),
            Some(Ok(()))
        );
        assert!(config.apps.is_empty());
        assert_eq!(
            act(&mut config, "typing_app_remove:com.example.absent"),
            Some(Ok(()))
        );
        for other in [
            "typing_toggle",
            "typing_app_zap:com.x",
            "typing_app_add",
            "stack_board_add:x",
            "",
        ] {
            assert_eq!(act(&mut config, other), None, "{other}");
        }
        config.apps = (0..MAX_APPS)
            .map(|i| format!("com.example.app{i}"))
            .collect();
        assert_eq!(
            act(&mut config, "typing_app_add:com.example.more"),
            Some(Err("invalid_value")),
            "a full list"
        );
    }

    #[test]
    fn the_shortcut_is_under_its_own_target_and_may_be_removed() {
        let mut config = Config::default();
        assert_eq!(
            shortcuts(&config),
            vec![("typing".to_owned(), "ctrl+space".to_owned())]
        );
        assert!(
            set_shortcut(&mut config, "typing", "cmd+shift+space")
                .unwrap()
                .is_ok()
        );
        assert_eq!(config.shortcut, "cmd+shift+space");
        assert!(set_shortcut(&mut config, "typing", "").unwrap().is_ok());
        assert!(shortcuts(&config).is_empty());
        assert!(set_shortcut(&mut config, "typing:other", "ctrl+a").is_none());
    }
}
