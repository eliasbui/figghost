//! The cursor's part of the settings model: the JSON the page renders and the changes it may make.

use super::{Config, SIZE_RANGE, SPEED_RANGE, TIPS, is_theme};
use crate::{features::mascot::Character, settings::model::Fault};
use serde_json::{Map, Value, json};
use std::{path::Path, sync::OnceLock};

/// The first frame of each character for the page, made once: they are part of the program.
fn sheets() -> &'static Value {
    static SHEETS: OnceLock<Value> = OnceLock::new();
    SHEETS.get_or_init(|| {
        let all: Map<String, Value> = Character::ALL
            .into_iter()
            .map(|c| {
                let url = format!("data:image/png;base64,{}", crate::settings::base64(c.png()));
                (c.as_str().to_owned(), Value::String(url))
            })
            .collect();
        Value::Object(all)
    })
}

fn invalid(detail: &str) -> Fault {
    Fault::new("invalid_value", detail)
}

/// The state under the key "cursor". `runtime` is `Feature::runtime()`, or null when the feature is not
/// running. `web/features/cursor.fixture.json` must have exactly the same keys; a unit test checks it.
pub fn state(config: &Config, runtime: &Value) -> Value {
    let file = Path::new(&config.custom_path)
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    let mut themes = vec!["neon".to_owned()];
    themes.extend(Character::ALL.map(|c| c.as_str().to_owned()));
    themes.push("custom".to_owned());
    json!({
        "enabled": config.enabled,
        "theme": config.theme,
        "themes": themes,
        "size": config.size,
        "speed": config.speed,
        "tip": config.tip,
        "custom_name": file,
        "sheets": sheets(),
        "unreadable": runtime.get("unreadable").and_then(Value::as_bool).unwrap_or(false),
    })
}

fn number(value: &Value, range: (f64, f64)) -> Result<f64, Fault> {
    value
        .as_f64()
        .filter(|v| v.is_finite() && (range.0..=range.1).contains(v))
        .ok_or_else(|| invalid("a number out of range"))
}

/// A change from the page. `path` is relative to the feature. `custom_path` is not among them: a file is
/// chosen with the button, never typed in by the page.
pub fn set(config: &mut Config, path: &str, value: &Value) -> Option<Result<(), Fault>> {
    Some(match path {
        "enabled" => value
            .as_bool()
            .map(|on| config.enabled = on)
            .ok_or_else(|| invalid("expected true or false")),
        "theme" => match value.as_str() {
            Some(theme) if is_theme(theme) => {
                config.theme = theme.to_owned();
                Ok(())
            }
            _ => Err(invalid("an unknown theme")),
        },
        "size" => number(value, SIZE_RANGE).map(|v| config.size = v),
        "speed" => number(value, SPEED_RANGE).map(|v| config.speed = v),
        "tip" => match value.as_str() {
            Some(tip) if TIPS.contains(&tip) => {
                config.tip = tip.to_owned();
                Ok(())
            }
            _ => Err(invalid("expected top or centre")),
        },
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_fixture_has_the_keys_of_the_state() {
        let fixture: Value =
            serde_json::from_str(include_str!("../../../web/features/cursor.fixture.json"))
                .unwrap();
        let real = state(&Config::default(), &Value::Null);
        let keys = |v: &Value| {
            let mut k: Vec<String> = v.as_object().unwrap().keys().cloned().collect();
            k.sort();
            k
        };
        assert_eq!(keys(&fixture), keys(&real));
        assert_eq!(
            real["themes"].as_array().unwrap().len(),
            Character::ALL.len() + 2
        );
    }

    #[test]
    fn the_page_changes_are_checked() {
        let mut config = Config::default();
        assert!(set(&mut config, "enabled", &json!(true)).unwrap().is_ok() && config.enabled);
        assert!(set(&mut config, "enabled", &json!("yes")).unwrap().is_err());
        assert!(set(&mut config, "theme", &json!("ghost")).unwrap().is_ok());
        assert_eq!(config.theme, "ghost");
        for bad in [json!("klingon"), json!(""), json!(3), json!(null)] {
            assert!(set(&mut config, "theme", &bad).unwrap().is_err(), "{bad}");
        }
        assert_eq!(
            config.theme, "ghost",
            "a refused change leaves the setting alone"
        );
        assert!(set(&mut config, "size", &json!(48)).unwrap().is_ok());
        assert_eq!(config.size, 48.0);
        for bad in [json!(2), json!(500), json!("big"), json!(null)] {
            assert!(set(&mut config, "size", &bad).unwrap().is_err(), "{bad}");
        }
        assert!(set(&mut config, "speed", &json!(2.5)).unwrap().is_ok());
        assert!(set(&mut config, "speed", &json!(0)).unwrap().is_err());
        assert!(set(&mut config, "tip", &json!("centre")).unwrap().is_ok());
        assert!(set(&mut config, "tip", &json!("left")).unwrap().is_err());
        assert!(
            set(&mut config, "custom_path", &json!("/etc/passwd")).is_none(),
            "a file cannot be named by the page"
        );
        assert_eq!(config.custom_path, "");
    }
}
