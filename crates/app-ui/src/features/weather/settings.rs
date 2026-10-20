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
        "place": config.place.as_ref().map(|p| p.label()),
        "searching": runtime.get("searching").and_then(Value::as_bool).unwrap_or(false),
        "found": runtime.get("found").cloned().unwrap_or_else(|| json!([])),
        "problem": runtime.get("problem").and_then(Value::as_str).unwrap_or_default(),
    })
}

/// A change from the page. `path` is relative to the feature. Values must be validated here; the page is not
/// trusted. The place is not set here: it is picked from a search, which only the running feature knows.
pub fn set(config: &mut Config, path: &str, value: &Value) -> Option<Result<(), Fault>> {
    let bad = || Fault::new("invalid_value", path);
    Some(match path {
        "enabled" => value
            .as_bool()
            .map(|on| config.enabled = on)
            .ok_or_else(bad),
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
        let found = &fixture["found"][0];
        assert_eq!(keys(found), ["label"]);
    }

    #[test]
    fn the_state_starts_off_with_no_place_and_says_what_the_running_feature_knows() {
        let s = state(&Config::default(), &Value::Null);
        assert_eq!(s["enabled"], json!(false));
        assert_eq!(s["place"], Value::Null);
        assert_eq!(s["units"], json!("metric"));
        assert_eq!(s["found"], json!([]));
        let config = Config {
            place: Some(Place {
                name: "Hanoi".into(),
                region: "Hanoi".into(),
                country: "Vietnam".into(),
                latitude: 21.0,
                longitude: 105.8,
            }),
            ..Config::default()
        };
        let runtime = json!({ "searching": true, "found": [{ "label": "Hue, Vietnam" }], "problem": "offline" });
        let s = state(&config, &runtime);
        assert_eq!(s["place"], json!("Hanoi, Vietnam"));
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
            set(&mut config, "place", &json!("Hue")).is_none(),
            "a place is picked from a search, never set"
        );
        assert!(set(&mut config, "nonsense", &json!(1)).is_none());
    }
}
