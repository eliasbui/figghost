//! This feature's part of the settings model: the JSON the page renders and the changes it may make.
//! Pure data with no AppKit, so every rule is covered by plain unit tests.

use super::{Config, MAX_EXCLUDED, valid_id};
use crate::settings::model::Fault;
use core_calendar::schedule::{MAX_LEAD, MIN_LEAD};
use serde_json::{Value, json};

/// The calendars of `runtime[key]` for the page, each marked with whether it is included.
fn listing(config: &Config, runtime: &Value, key: &str) -> Vec<Value> {
    runtime[key]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|c| {
            let id = c["id"].as_str().filter(|id| valid_id(id))?;
            Some(json!({
                "id": id,
                "title": c["title"].as_str().unwrap_or_default(),
                "source": c["source"].as_str().unwrap_or_default(),
                "color": c["color"].as_str().unwrap_or("#8e8e93"),
                "included": !config.excluded.iter().any(|e| e == id),
            }))
        })
        .collect()
}

/// The state under the key "calendar". `runtime` is `Feature::runtime()`, or null when the feature is not running.
/// `web/features/calendar.fixture.json` must have exactly the same keys; a unit test checks it.
pub fn state(config: &Config, runtime: &Value) -> Value {
    let config = config.clone().sanitized();
    let access = |key: &str| runtime[key].as_str().unwrap_or("unknown").to_owned();
    json!({
        "enabled": config.enabled,
        "lead_minutes": config.lead_minutes,
        "sound": config.sound,
        "reminders": config.reminders,
        "all_day": config.all_day,
        "events_access": access("events_access"),
        "reminders_access": access("reminders_access"),
        "can_prompt": runtime["can_prompt"].as_bool().unwrap_or(false),
        "calendars": listing(&config, runtime, "calendars"),
        "lists": listing(&config, runtime, "lists"),
    })
}

fn flag(value: &Value) -> Result<bool, Fault> {
    value
        .as_bool()
        .ok_or_else(|| Fault::new("invalid_value", "expected true or false"))
}

/// A change from the page. `path` is relative to the feature: "enabled", not "calendar.enabled". `None` when the
/// path is not one of this feature's. Values must be validated here; the page is not trusted.
pub fn set(config: &mut Config, path: &str, value: &Value) -> Option<Result<(), Fault>> {
    let result = match path {
        "enabled" => flag(value).map(|on| config.enabled = on),
        "sound" => flag(value).map(|on| config.sound = on),
        "reminders" => flag(value).map(|on| config.reminders = on),
        "all_day" => flag(value).map(|on| config.all_day = on),
        "lead_minutes" => value
            .as_f64()
            .filter(|n| n.is_finite())
            .map(|n| {
                config.lead_minutes =
                    n.round().clamp(f64::from(MIN_LEAD), f64::from(MAX_LEAD)) as u32
            })
            .ok_or_else(|| Fault::new("invalid_value", "expected a number of minutes")),
        "include" => include(config, value),
        _ => return None,
    };
    Some(result)
}

/// `{ "id": "<calendar identifier>", "on": true }`: a calendar is in or out. The list holds the ones that
/// are out, so a calendar that appears later is included.
fn include(config: &mut Config, value: &Value) -> Result<(), Fault> {
    let id = value["id"]
        .as_str()
        .filter(|id| valid_id(id))
        .ok_or_else(|| Fault::new("invalid_value", "not a calendar identifier"))?;
    let on = flag(&value["on"])?;
    config.excluded.retain(|e| e != id);
    if !on {
        if config.excluded.len() >= MAX_EXCLUDED {
            return Err(Fault::new("invalid_value", "too many calendars"));
        }
        config.excluded.push(id.to_owned());
    }
    Ok(())
}

/// This feature has no global shortcut.
pub fn shortcuts(_config: &Config) -> Vec<(String, String)> {
    Vec::new()
}

pub fn set_shortcut(
    _config: &mut Config,
    _target: &str,
    _value: &str,
) -> Option<Result<(), Fault>> {
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_switches_are_validated() {
        let mut config = Config::default();
        assert!(set(&mut config, "enabled", &json!(true)).unwrap().is_ok() && config.enabled);
        for path in ["enabled", "sound", "reminders", "all_day"] {
            assert!(
                set(&mut config, path, &json!("yes")).unwrap().is_err(),
                "{path}"
            );
            assert!(
                set(&mut config, path, &json!(null)).unwrap().is_err(),
                "{path}"
            );
        }
        assert!(set(&mut config, "sound", &json!(false)).unwrap().is_ok() && !config.sound);
        assert!(set(&mut config, "nonsense", &json!(1)).is_none());
    }

    #[test]
    fn the_lead_time_is_a_number_of_minutes_inside_its_range() {
        let mut config = Config::default();
        assert!(
            set(&mut config, "lead_minutes", &json!(10))
                .unwrap()
                .is_ok()
        );
        assert_eq!(config.lead_minutes, 10);
        set(&mut config, "lead_minutes", &json!(0))
            .unwrap()
            .unwrap();
        assert_eq!(config.lead_minutes, 1);
        set(&mut config, "lead_minutes", &json!(100_000))
            .unwrap()
            .unwrap();
        assert_eq!(config.lead_minutes, 60);
        set(&mut config, "lead_minutes", &json!(-5))
            .unwrap()
            .unwrap();
        assert_eq!(config.lead_minutes, 1);
        set(&mut config, "lead_minutes", &json!(7.6))
            .unwrap()
            .unwrap();
        assert_eq!(config.lead_minutes, 8);
        assert!(
            set(&mut config, "lead_minutes", &json!("5"))
                .unwrap()
                .is_err()
        );
        assert!(
            set(&mut config, "lead_minutes", &json!(null))
                .unwrap()
                .is_err()
        );
        assert_eq!(config.lead_minutes, 8);
    }

    #[test]
    fn a_calendar_is_taken_out_and_put_back_by_its_identifier() {
        let mut config = Config::default();
        let out = |id: &str, on: bool| json!({ "id": id, "on": on });
        set(&mut config, "include", &out("A-1", false))
            .unwrap()
            .unwrap();
        set(&mut config, "include", &out("A-1", false))
            .unwrap()
            .unwrap();
        set(&mut config, "include", &out("B-2", false))
            .unwrap()
            .unwrap();
        assert_eq!(config.excluded, ["A-1", "B-2"]);
        set(&mut config, "include", &out("A-1", true))
            .unwrap()
            .unwrap();
        assert_eq!(config.excluded, ["B-2"]);
    }

    #[test]
    fn hostile_calendar_identifiers_are_refused() {
        let mut config = Config::default();
        for bad in [
            json!({ "id": "", "on": false }),
            json!({ "id": "x".repeat(300), "on": false }),
            json!({ "id": "a\nb", "on": false }),
            json!({ "id": 5, "on": false }),
            json!({ "id": "ok", "on": "no" }),
            json!("ok"),
            json!(null),
        ] {
            assert!(set(&mut config, "include", &bad).unwrap().is_err(), "{bad}");
        }
        assert!(config.excluded.is_empty());
        for i in 0..MAX_EXCLUDED {
            set(
                &mut config,
                "include",
                &json!({ "id": format!("id-{i}"), "on": false }),
            )
            .unwrap()
            .unwrap();
        }
        assert!(
            set(
                &mut config,
                "include",
                &json!({ "id": "one-more", "on": false })
            )
            .unwrap()
            .is_err()
        );
        assert_eq!(config.excluded.len(), MAX_EXCLUDED);
    }

    #[test]
    fn the_state_marks_which_calendars_are_included() {
        let config = Config {
            excluded: vec!["B".into(), "gone".into()],
            ..Config::default()
        };
        let runtime = json!({
            "events_access": "granted",
            "reminders_access": "denied",
            "can_prompt": true,
            "calendars": [
                { "id": "A", "title": "Work", "source": "iCloud", "color": "#3478f6" },
                { "id": "B", "title": "Home", "source": "iCloud", "color": "#30d158" },
                { "id": "bad\nid", "title": "Hostile", "source": "", "color": "#000000" },
            ],
            "lists": [],
        });
        let s = state(&config, &runtime);
        assert_eq!(s["events_access"], "granted");
        assert_eq!(s["reminders_access"], "denied");
        assert_eq!(s["calendars"].as_array().unwrap().len(), 2);
        assert_eq!(s["calendars"][0]["included"], true);
        assert_eq!(s["calendars"][1]["included"], false);
    }

    #[test]
    fn without_a_running_feature_the_state_is_still_complete() {
        let s = state(&Config::default(), &Value::Null);
        assert_eq!(s["enabled"], false);
        assert_eq!(s["lead_minutes"], 5);
        assert_eq!(s["events_access"], "unknown");
        assert_eq!(s["can_prompt"], false);
        assert!(s["calendars"].as_array().unwrap().is_empty());
    }

    #[test]
    fn a_hostile_config_file_is_clamped_in_the_state() {
        let config = Config {
            lead_minutes: 4_000_000,
            ..Config::default()
        };
        assert_eq!(state(&config, &Value::Null)["lead_minutes"], 60);
    }
}
