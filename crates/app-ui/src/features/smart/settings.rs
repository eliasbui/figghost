//! Smart Select's part of the settings model: the JSON the page renders and the changes it may make.

use super::Config;
use crate::settings::model::Fault;
use core_smart::{currency, translate};
use macos_sys::timezone;
use serde_json::{Value, json};

/// How many currencies and zones are shown at most.
pub const MAX_LISTED: usize = 6;

fn invalid(detail: &str) -> Fault {
    Fault::new("invalid_value", detail)
}

fn flag(value: &Value) -> Result<bool, Fault> {
    value
        .as_bool()
        .ok_or_else(|| invalid("expected true or false"))
}

/// A list of currency codes from the page: capital letters, no repeats, at least one and few enough to read.
fn currencies(value: &Value) -> Result<Vec<String>, Fault> {
    let mut codes: Vec<String> = Vec::new();
    for item in value.as_array().ok_or_else(|| invalid("expected a list"))? {
        let code = item
            .as_str()
            .map(|c| c.trim().to_uppercase())
            .filter(|c| currency::is_code(c))
            .ok_or_else(|| invalid("a currency is three letters, like USD"))?;
        if !codes.contains(&code) {
            codes.push(code);
        }
    }
    if codes.is_empty() || codes.len() > MAX_LISTED {
        return Err(invalid("between one and six currencies"));
    }
    Ok(codes)
}

/// A list of time zones from the page: "UTC" or names the system knows, such as "Asia/Tokyo".
fn zones(value: &Value) -> Result<Vec<String>, Fault> {
    let mut ids: Vec<String> = Vec::new();
    for item in value.as_array().ok_or_else(|| invalid("expected a list"))? {
        let id = item
            .as_str()
            .map(str::trim)
            .filter(|id| id.len() <= 40 && *id != "local" && timezone::exists(id))
            .ok_or_else(|| invalid("a time zone is a name like Asia/Tokyo or UTC"))?;
        if !ids.iter().any(|known| known == id) {
            ids.push(id.to_owned());
        }
    }
    if ids.len() > MAX_LISTED {
        return Err(invalid("at most six time zones"));
    }
    Ok(ids)
}

/// The state under the key "smart". `runtime` carries `accessibility` and `rates_updated`.
pub fn state(config: &Config, runtime: &Value) -> Value {
    let languages: Vec<Value> = translate::LANGUAGES
        .iter()
        .map(|l| json!({ "code": l.code, "name": l.name }))
        .collect();
    json!({
        "enabled": config.enabled,
        "shortcut": config.shortcut,
        "internet": config.internet,
        "translate_to": config.translate_to,
        "currencies": config.currencies,
        "zones": config.zones,
        "languages": languages,
        "accessibility": runtime.get("accessibility").and_then(Value::as_bool).unwrap_or(false),
        "rates_updated": runtime.get("rates_updated").and_then(Value::as_i64).unwrap_or(0),
    })
}

/// A change from the page. `path` is relative to the feature.
pub fn set(config: &mut Config, path: &str, value: &Value) -> Option<Result<(), Fault>> {
    Some(match path {
        "enabled" => flag(value).map(|on| config.enabled = on),
        "internet" => flag(value).map(|on| config.internet = on),
        "translate_to" => match value.as_str() {
            Some(code) if code == "auto" || translate::is_supported(code) => {
                config.translate_to = code.to_owned();
                Ok(())
            }
            _ => Err(invalid("an unknown language")),
        },
        "currencies" => currencies(value).map(|list| config.currencies = list),
        "zones" => zones(value).map(|list| config.zones = list),
        _ => return None,
    })
}

pub fn shortcuts(config: &Config) -> Vec<(String, String)> {
    if config.shortcut.is_empty() {
        Vec::new()
    } else {
        vec![("smart".to_owned(), config.shortcut.clone())]
    }
}

pub fn set_shortcut(config: &mut Config, target: &str, value: &str) -> Option<Result<(), Fault>> {
    if target != "smart" {
        return None;
    }
    Some(if value.trim().is_empty() {
        // Without a shortcut the feature could only be reached from the menu; that is allowed.
        config.shortcut.clear();
        Ok(())
    } else {
        config.shortcut = value.trim().to_owned();
        Ok(())
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_switches_are_validated() {
        let mut config = Config::default();
        assert!(set(&mut config, "enabled", &json!(true)).unwrap().is_ok() && config.enabled);
        assert!(set(&mut config, "internet", &json!(true)).unwrap().is_ok() && config.internet);
        assert!(
            set(&mut config, "internet", &json!("yes"))
                .unwrap()
                .is_err()
        );
        assert!(set(&mut config, "nonsense", &json!(1)).is_none());
    }

    #[test]
    fn the_target_language_is_auto_or_a_known_one() {
        let mut config = Config::default();
        assert!(
            set(&mut config, "translate_to", &json!("ja"))
                .unwrap()
                .is_ok()
        );
        assert_eq!(config.translate_to, "ja");
        assert!(
            set(&mut config, "translate_to", &json!("auto"))
                .unwrap()
                .is_ok()
        );
        for bad in [json!("klingon"), json!(""), json!(3), json!(null)] {
            assert!(
                set(&mut config, "translate_to", &bad).unwrap().is_err(),
                "{bad}"
            );
        }
        assert_eq!(
            config.translate_to, "auto",
            "a refused change leaves the setting alone"
        );
    }

    #[test]
    fn currencies_are_cleaned_and_bounded() {
        let mut config = Config::default();
        assert!(
            set(
                &mut config,
                "currencies",
                &json!(["usd", " eur ", "USD", "jpy"])
            )
            .unwrap()
            .is_ok()
        );
        assert_eq!(
            config.currencies,
            ["USD", "EUR", "JPY"],
            "uppercase, no repeats, in order"
        );
        for bad in [
            json!([]),
            json!(["US"]),
            json!(["USDD"]),
            json!(["U5D"]),
            json!([1]),
            json!("USD"),
            json!(["A", "B"]),
            json!(["USD", "EUR", "GBP", "JPY", "CNY", "VND", "KRW"]),
        ] {
            assert!(
                set(&mut config, "currencies", &bad).unwrap().is_err(),
                "{bad}"
            );
        }
        assert_eq!(config.currencies, ["USD", "EUR", "JPY"]);
    }

    #[test]
    fn zones_must_be_known_to_the_system() {
        let mut config = Config::default();
        assert!(
            set(
                &mut config,
                "zones",
                &json!(["Asia/Tokyo", "UTC", "Asia/Tokyo"])
            )
            .unwrap()
            .is_ok()
        );
        assert_eq!(config.zones, ["Asia/Tokyo", "UTC"]);
        assert!(
            set(&mut config, "zones", &json!([])).unwrap().is_ok(),
            "no extra zones is fine"
        );
        for bad in [
            json!(["Mars/Olympus"]),
            json!([""]),
            json!(["local"]),
            json!([5]),
            json!({}),
            json!([
                "Europe/London",
                "Europe/Paris",
                "Asia/Tokyo",
                "Asia/Seoul",
                "Asia/Dubai",
                "Africa/Cairo",
                "UTC"
            ]),
            json!(["../../etc/passwd"]),
        ] {
            assert!(set(&mut config, "zones", &bad).unwrap().is_err(), "{bad}");
        }
    }

    #[test]
    fn the_shortcut_belongs_to_the_feature() {
        let mut config = Config::default();
        assert_eq!(
            shortcuts(&config),
            [("smart".to_owned(), config.shortcut.clone())]
        );
        assert!(
            set_shortcut(&mut config, "smart", "ctrl+opt+t")
                .unwrap()
                .is_ok()
        );
        assert_eq!(config.shortcut, "ctrl+opt+t");
        assert!(set_shortcut(&mut config, "other", "ctrl+opt+x").is_none());
        assert!(set_shortcut(&mut config, "smart", "").unwrap().is_ok());
        assert!(
            shortcuts(&config).is_empty(),
            "a removed shortcut is not listed"
        );
    }

    #[test]
    fn the_state_carries_what_the_page_needs() {
        let config = Config::default();
        let state = state(
            &config,
            &json!({ "accessibility": true, "rates_updated": 1_792_022_551 }),
        );
        assert_eq!(state["accessibility"], true);
        assert_eq!(state["rates_updated"], 1_792_022_551);
        assert!(
            state["languages"]
                .as_array()
                .unwrap()
                .iter()
                .any(|l| l["code"] == "vi")
        );
        let without = super::state(&config, &Value::Null);
        assert_eq!(without["accessibility"], false);
        assert_eq!(without["rates_updated"], 0);
    }
}
