//! This feature's part of the settings model: the JSON the page renders and the changes it may make.
//! Pure data with no AppKit, so every rule is covered by plain unit tests.

use super::Config;
use crate::settings::model::Fault;
use serde_json::{Value, json};

/// The state under the key "weather". `runtime` is `Feature::runtime()`, or null when the feature is not running.
/// `web/features/weather.fixture.json` must have exactly the same keys; a unit test checks it.
pub fn state(config: &Config, runtime: &Value) -> Value {
    let config = config.clone().sanitized();
    json!({
        "enabled": config.enabled,
        "units": config.units,
        "sound": config.sound,
        "places": config.places.iter().map(|p| json!({ "label": p.label() })).collect::<Vec<_>>(),
        "full": config.places.len() >= super::logic::MAX_CITIES,
        "searching": runtime.get("searching").and_then(Value::as_bool).unwrap_or(false),
        "found": runtime.get("found").cloned().unwrap_or_else(|| json!([])),
        "problem": runtime.get("problem").and_then(Value::as_str).unwrap_or_default(),
    })
}

/// A change from the page. `path` is relative to the feature. Values must be validated here; the page is not
/// trusted. The cities are not set here: they are added from a search, which only the running feature knows.
pub fn set(config: &mut Config, path: &str, value: &Value) -> Option<Result<(), Fault>> {
    let bad = || Fault::new("invalid_value", path);
    Some(match path {
        "enabled" => value
            .as_bool()
            .map(|on| config.enabled = on)
            .ok_or_else(bad),
        "sound" => value.as_bool().map(|on| config.sound = on).ok_or_else(bad),
        "units" => value
            .as_str()
            .filter(|u| super::UNITS.contains(u))
            .map(|u| config.units = u.to_owned())
            .ok_or_else(bad),
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::super::logic::Place;
    use super::*;

    fn keys(value: &Value) -> Vec<String> {
        let mut keys: Vec<String> = value.as_object().unwrap().keys().cloned().collect();
        keys.sort();
        keys
    }

    #[test]
    fn the_fixture_has_exactly_the_keys_of_the_state() {
        let fixture: Value =
            serde_json::from_str(include_str!("../../../web/features/weather.fixture.json"))
                .unwrap();
        assert_eq!(
            keys(&fixture),
            keys(&state(&Config::default(), &Value::Null))
        );
        assert_eq!(keys(&fixture["found"][0]), ["label"]);
        assert_eq!(keys(&fixture["places"][0]), ["label"]);
    }

    #[test]
    fn the_state_starts_off_with_no_place_and_says_what_the_running_feature_knows() {
        let s = state(&Config::default(), &Value::Null);
        assert_eq!(s["enabled"], json!(false));
        assert_eq!(s["places"], json!([]));
        assert_eq!(s["full"], json!(false));
        assert_eq!(s["units"], json!("metric"));
        assert_eq!(s["found"], json!([]));
        let city = |name: &str, latitude: f64| Place {
            name: name.into(),
            region: name.into(),
            country: "Vietnam".into(),
            latitude,
            longitude: 105.8,
        };
        let config = Config {
            places: vec![city("Hanoi", 21.0), city("Hue", 16.4)],
            ..Config::default()
        };
        let runtime = json!({ "searching": true, "found": [{ "label": "Hue, Vietnam" }], "problem": "offline" });
        let s = state(&config, &runtime);
        assert_eq!(
            s["places"],
            json!([{ "label": "Hanoi, Vietnam" }, { "label": "Hue, Vietnam" }])
        );
        let full = Config {
            places: (0..20).map(|i| city("C", f64::from(i))).collect(),
            ..Config::default()
        };
        assert_eq!(state(&full, &Value::Null)["full"], json!(true));
        assert_eq!(s["searching"], json!(true));
        assert_eq!(s["found"][0]["label"], json!("Hue, Vietnam"));
        assert_eq!(s["problem"], json!("offline"));
    }

    #[test]
    fn the_switch_and_the_units_are_validated() {
        let mut config = Config::default();
        assert!(set(&mut config, "enabled", &json!("yes")).unwrap().is_err());
        assert!(set(&mut config, "enabled", &json!(true)).unwrap().is_ok() && config.enabled);
        assert!(
            set(&mut config, "units", &json!("imperial"))
                .unwrap()
                .is_ok()
        );
        assert_eq!(config.units, "imperial");
        for bad in [json!("kelvin"), json!(1), json!(null), json!(["metric"])] {
            assert!(set(&mut config, "units", &bad).unwrap().is_err(), "{bad}");
        }
        assert_eq!(config.units, "imperial", "a refused value changes nothing");
        assert!(
            set(&mut config, "places", &json!(["Hue"])).is_none(),
            "a city is added from a search, never set"
        );
        assert!(set(&mut config, "nonsense", &json!(1)).is_none());
    }
}
