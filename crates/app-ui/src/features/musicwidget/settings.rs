//! The widget's part of the settings model: the JSON the page renders and the changes it may make.

use super::{Config, LAYERS, MATERIALS, STYLES, clamp_size};
use crate::settings::model::Fault;
use serde_json::{Value, json};

fn invalid(detail: &str) -> Fault {
    Fault::new("invalid_value", detail)
}

/// The state under the key "musicwidget". `runtime` is `Feature::runtime()`, or null when the feature is not
/// running; the size it carries is the one on screen, which the user may have changed by dragging a corner.
/// `web/features/musicwidget.fixture.json` must have exactly the same keys; a unit test checks it.
pub fn state(config: &Config, runtime: &Value) -> Value {
    let shown = |key: &str, set: f64| runtime.get(key).and_then(Value::as_f64).unwrap_or(set);
    json!({
        "enabled": config.enabled,
        "style": config.style,
        "material": config.material,
        "layer": config.layer,
        "ambient": config.ambient,
        "hide_idle": config.hide_idle,
        "opacity": config.opacity,
        "width": shown("width", config.width),
        "height": shown("height", config.height),
    })
}

fn word(value: &Value, words: &[&str]) -> Result<String, Fault> {
    match value.as_str() {
        Some(w) if words.contains(&w) => Ok(w.to_owned()),
        _ => Err(invalid("an unknown choice")),
    }
}

fn flag(value: &Value) -> Result<bool, Fault> {
    value
        .as_bool()
        .ok_or_else(|| invalid("expected true or false"))
}

/// A change from the page. `path` is relative to the feature.
pub fn set(config: &mut Config, path: &str, value: &Value) -> Option<Result<(), Fault>> {
    Some(match path {
        "enabled" => flag(value).map(|on| config.enabled = on),
        "ambient" => flag(value).map(|on| config.ambient = on),
        "hide_idle" => flag(value).map(|on| config.hide_idle = on),
        "style" => word(value, STYLES).map(|w| {
            // A new style starts at its own size rather than at the last one's.
            config.style = w;
            config.width = 0.0;
            config.height = 0.0;
        }),
        "material" => word(value, MATERIALS).map(|w| config.material = w),
        "layer" => word(value, LAYERS).map(|w| config.layer = w),
        "opacity" => value
            .as_f64()
            .filter(|v| (0.3..=1.0).contains(v))
            .map(|v| config.opacity = v)
            .ok_or_else(|| invalid("a number from 0.3 to 1")),
        "width" => size(value, 0).map(|v| config.width = v),
        "height" => size(value, 1).map(|v| config.height = v),
        _ => return None,
    })
}

fn size(value: &Value, axis: usize) -> Result<f64, Fault> {
    let v = value
        .as_f64()
        .filter(|v| v.is_finite())
        .ok_or_else(|| invalid("a number"))?;
    if v == 0.0 {
        return Ok(0.0);
    }
    let clamped = clamp_size(v, axis);
    if (clamped - v).abs() > f64::EPSILON {
        return Err(invalid("a size out of range"));
    }
    Ok(v)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_fixture_has_the_keys_of_the_state() {
        let fixture: Value = serde_json::from_str(include_str!(
            "../../../web/features/musicwidget.fixture.json"
        ))
        .unwrap();
        let real = state(&Config::default(), &Value::Null);
        let keys = |v: &Value| {
            let mut k: Vec<String> = v.as_object().unwrap().keys().cloned().collect();
            k.sort();
            k
        };
        assert_eq!(keys(&fixture), keys(&real));
    }

    #[test]
    fn the_page_changes_are_checked() {
        let mut config = Config::default();
        assert!(set(&mut config, "enabled", &json!(true)).unwrap().is_ok() && config.enabled);
        assert!(set(&mut config, "enabled", &json!("yes")).unwrap().is_err());
        assert!(set(&mut config, "style", &json!("vinyl")).unwrap().is_ok());
        assert_eq!(config.style, "vinyl");
        assert!(set(&mut config, "style", &json!("plaid")).unwrap().is_err());
        assert!(
            set(&mut config, "material", &json!("clear"))
                .unwrap()
                .is_ok()
        );
        assert!(set(&mut config, "layer", &json!("always")).unwrap().is_ok());
        assert!(set(&mut config, "layer", &json!("lock")).unwrap().is_err());
        assert!(set(&mut config, "opacity", &json!(0.6)).unwrap().is_ok());
        for bad in [json!(0.1), json!(2), json!("x"), json!(null)] {
            assert!(set(&mut config, "opacity", &bad).unwrap().is_err(), "{bad}");
        }
        assert!(set(&mut config, "width", &json!(400)).unwrap().is_ok());
        assert_eq!(config.width, 400.0);
        assert!(set(&mut config, "width", &json!(0)).unwrap().is_ok());
        for bad in [json!(10), json!(5000), json!("wide")] {
            assert!(set(&mut config, "width", &bad).unwrap().is_err(), "{bad}");
        }
        config.width = 400.0;
        assert!(set(&mut config, "style", &json!("bar")).unwrap().is_ok());
        assert_eq!(config.width, 0.0, "a new style starts at its own size");
        assert!(set(&mut config, "nonsense", &json!(1)).is_none());
    }

    #[test]
    fn the_size_on_screen_shows_in_the_state() {
        let state = state(
            &Config::default(),
            &json!({ "width": 410.0, "height": 150.0 }),
        );
        assert_eq!(
            (state["width"].as_f64(), state["height"].as_f64()),
            (Some(410.0), Some(150.0))
        );
    }
}
