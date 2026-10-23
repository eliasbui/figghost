//! This feature's part of the settings model: the JSON the page renders and the changes it may make.
//! Pure data with no AppKit, so every rule is covered by plain unit tests.

use super::{Config, valid_url};
use crate::settings::model::Fault;
use serde_json::{Value, json};

/// The state under the key "video". `web/features/video.fixture.json` must have exactly the same keys; a unit test
/// checks it.
pub fn state(config: &Config) -> Value {
    let config = config.clone().sanitized();
    json!({
        "enabled": config.enabled,
        "url": config.url,
    })
}

/// A change from the page. `path` is relative to the feature. Values must be validated here; the page is not trusted.
pub fn set(config: &mut Config, path: &str, value: &Value) -> Option<Result<(), Fault>> {
    let bad = || Fault::new("invalid_value", path);
    Some(match path {
        "enabled" => value
            .as_bool()
            .map(|on| config.enabled = on)
            .ok_or_else(bad),
        "url" => value
            .as_str()
            .map(str::trim)
            .filter(|url| url.is_empty() || valid_url(url))
            .map(|url| config.url = url.to_owned())
            .ok_or_else(bad),
        _ => return None,
    })
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
        let fixture: Value =
            serde_json::from_str(include_str!("../../../web/features/video.fixture.json")).unwrap();
        assert_eq!(keys(&fixture), keys(&state(&Config::default())));
    }

    #[test]
    fn the_tab_starts_off_with_no_address() {
        let s = state(&Config::default());
        assert_eq!(s["enabled"], json!(false));
        assert_eq!(s["url"], json!(""));
    }

    #[test]
    fn an_address_is_kept_trimmed_and_one_that_cannot_play_is_refused() {
        let mut config = Config::default();
        let url = json!("  https://example.com/live.m3u8 ");
        assert!(matches!(set(&mut config, "url", &url), Some(Ok(()))));
        assert_eq!(config.url, "https://example.com/live.m3u8");
        for bad in [
            json!("rtsp://camera/stream"),
            json!("javascript:alert(1)"),
            json!("https://a b"),
            json!(3),
        ] {
            assert!(
                matches!(set(&mut config, "url", &bad), Some(Err(_))),
                "{bad}"
            );
        }
        assert_eq!(
            config.url, "https://example.com/live.m3u8",
            "a refusal changes nothing"
        );
        assert!(matches!(set(&mut config, "url", &json!("")), Some(Ok(()))));
        assert_eq!(config.url, "", "empty clears it");
    }

    #[test]
    fn the_switch_takes_a_boolean_only_and_unknown_paths_are_not_ours() {
        let mut config = Config::default();
        assert!(matches!(
            set(&mut config, "enabled", &json!(true)),
            Some(Ok(()))
        ));
        assert!(config.enabled);
        assert!(matches!(
            set(&mut config, "enabled", &json!("yes")),
            Some(Err(_))
        ));
        assert!(set(&mut config, "nonsense", &json!(1)).is_none());
    }
}
