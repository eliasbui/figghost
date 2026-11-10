//! The mirror and teleprompter's part of the settings model.

use super::{Config, logic};
use crate::{config::parse_hotkey, settings::model::Fault};
use serde_json::{Value, json};

/// The state under the key "mirror". `runtime` carries `camera`, whether the camera may be used.
/// The `mirror` part of `app-tauri/src/data/state.sample.json` must have exactly the same keys; a unit test checks it.
pub fn state(config: &Config, runtime: &Value) -> Value {
    let config = config.clone().sanitized();
    json!({
        "enabled": config.enabled,
        "mirror_size": config.mirror_size,
        "text_size": config.text_size,
        "speed": config.speed,
        "hide_in_capture": config.hide_in_capture,
        "shortcuts": { "mirror": config.mirror_shortcut, "prompter": config.prompter_shortcut },
        "camera": runtime.get("camera").and_then(Value::as_bool).unwrap_or(false),
        "has_note": runtime.get("has_note").and_then(Value::as_bool).unwrap_or(false),
        "running": runtime.get("running").cloned().unwrap_or_else(|| json!({ "mirror": false, "prompter": false })),
    })
}

pub fn set(config: &mut Config, path: &str, value: &Value) -> Option<Result<(), Fault>> {
    let bad = || Fault::new("invalid_value", path);
    let pick = |list: &[u32], slot: &mut u32| {
        value
            .as_u64()
            .and_then(|n| u32::try_from(n).ok())
            .filter(|n| list.contains(n))
            .map(|n| *slot = n)
            .ok_or_else(bad)
    };
    Some(match path {
        "enabled" => value
            .as_bool()
            .map(|on| config.enabled = on)
            .ok_or_else(bad),
        "hide_in_capture" => value
            .as_bool()
            .map(|on| config.hide_in_capture = on)
            .ok_or_else(bad),
        "mirror_size" => pick(&logic::MIRROR_SIZES, &mut config.mirror_size),
        "text_size" => pick(&logic::TEXT_SIZES, &mut config.text_size),
        "speed" => pick(&[1, 2, 3, 4, 5, 6], &mut config.speed),
        _ => return None,
    })
}

pub fn shortcuts(config: &Config) -> Vec<(String, String)> {
    [
        ("mirror:mirror", &config.mirror_shortcut),
        ("mirror:prompter", &config.prompter_shortcut),
    ]
    .into_iter()
    .filter(|(_, s)| !s.is_empty())
    .map(|(t, s)| (t.to_owned(), s.clone()))
    .collect()
}

pub fn set_shortcut(config: &mut Config, target: &str, value: &str) -> Option<Result<(), Fault>> {
    let which = target.strip_prefix("mirror:")?;
    let value = value.trim().to_owned();
    if !value.is_empty() && parse_hotkey(&value).is_err() {
        return Some(Err(Fault::new("invalid_shortcut", target)));
    }
    match which {
        "mirror" => config.mirror_shortcut = value,
        "prompter" => config.prompter_shortcut = value,
        _ => return None,
    }
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
        let fixture: Value = crate::settings::sample("mirror");
        let state = state(&Config::default(), &Value::Null);
        assert_eq!(keys(&fixture), keys(&state));
        assert_eq!(keys(&fixture["shortcuts"]), keys(&state["shortcuts"]));
        assert_eq!(keys(&fixture["running"]), keys(&state["running"]));
    }

    #[test]
    fn only_what_is_offered_is_taken() {
        let mut c = Config::default();
        assert!(set(&mut c, "mirror_size", &json!(280)).unwrap().is_ok());
        assert!(set(&mut c, "mirror_size", &json!(281)).unwrap().is_err());
        assert!(set(&mut c, "text_size", &json!(60)).unwrap().is_ok() && c.text_size == 60);
        assert!(set(&mut c, "text_size", &json!(10)).unwrap().is_err());
        assert!(set(&mut c, "speed", &json!(6)).unwrap().is_ok() && c.speed == 6);
        for bad in [json!(0), json!(7), json!("2"), json!(-1)] {
            assert!(set(&mut c, "speed", &bad).unwrap().is_err(), "{bad}");
        }
        assert!(
            set(&mut c, "hide_in_capture", &json!(false))
                .unwrap()
                .is_ok()
                && !c.hide_in_capture
        );
        assert!(set(&mut c, "enabled", &json!("on")).unwrap().is_err());
        assert!(set(&mut c, "other", &json!(1)).is_none());
    }

    #[test]
    fn the_two_shortcuts_are_checked_and_may_be_taken_away() {
        let mut c = Config::default();
        assert_eq!(shortcuts(&c).len(), 2);
        assert!(
            set_shortcut(&mut c, "mirror:prompter", "cmd+shift+y")
                .unwrap()
                .is_ok()
        );
        assert_eq!(c.prompter_shortcut, "cmd+shift+y");
        assert!(
            set_shortcut(&mut c, "mirror:mirror", "nonsense")
                .unwrap()
                .is_err()
        );
        assert!(
            set_shortcut(&mut c, "mirror:mirror", "").unwrap().is_ok() && shortcuts(&c).len() == 1
        );
        assert!(set_shortcut(&mut c, "mirror:other", "cmd+k").is_none());
        assert!(set_shortcut(&mut c, "translate", "cmd+k").is_none());
    }
}
