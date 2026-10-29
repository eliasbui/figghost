//! This feature's part of the settings model: the JSON the page renders and the changes it may make.
//! Pure data with no AppKit, so every rule is covered by plain unit tests.

use super::{Config, logic::Quality};
use crate::settings::model::Fault;
use serde_json::{Value, json};
use std::path::Path;

/// The state under the key "download". `runtime` is `Feature::runtime()`, or null when the feature is not running.
/// `web/features/download.fixture.json` must have exactly the same keys; a unit test checks it.
pub fn state(config: &Config, runtime: &Value) -> Value {
    let config = config.clone().sanitized();
    let text = |key: &str| {
        runtime
            .get(key)
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_owned()
    };
    json!({
        "enabled": config.enabled,
        "folder": config.folder,
        "quality": config.quality,
        "playlist": config.playlist,
        "net_merge": config.net_merge,
        "folder_path": text("folder_path"),
        "ytdlp": text("ytdlp"),
        "ytdlp_version": text("ytdlp_version"),
        "ffmpeg": text("ffmpeg"),
        "jobs": runtime.get("jobs").filter(|j| j.is_array()).cloned().unwrap_or_else(|| json!([])),
        "picked": runtime
            .get("picked")
            .filter(|p| p.is_object())
            .cloned()
            .unwrap_or_else(|| json!({ "seq": 0, "path": "" })),
        "net_links": runtime.get("net_links").filter(|l| l.is_array()).cloned().unwrap_or_else(|| json!([])),
        "net_paused": runtime.get("net_paused").and_then(Value::as_bool).unwrap_or(false),
    })
}

/// A change from the page. `path` is relative to the feature. Values must be validated here; the page is not
/// trusted.
pub fn set(config: &mut Config, path: &str, value: &Value) -> Option<Result<(), Fault>> {
    let bad = || Fault::new("invalid_value", path);
    let switch = |slot: &mut bool| value.as_bool().map(|on| *slot = on).ok_or_else(bad);
    Some(match path {
        "enabled" => switch(&mut config.enabled),
        "playlist" => switch(&mut config.playlist),
        "net_merge" => switch(&mut config.net_merge),
        "quality" => value
            .as_str()
            .and_then(Quality::parse)
            .map(|q| config.quality = q.key().to_owned())
            .ok_or_else(bad),
        "folder" => value
            .as_str()
            .map(str::trim)
            .filter(|f| f.is_empty() || (Path::new(f).is_absolute() && Path::new(f).is_dir()))
            .map(|f| config.folder = f.to_owned())
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
            serde_json::from_str(include_str!("../../../web/features/download.fixture.json"))
                .unwrap();
        assert_eq!(
            keys(&fixture),
            keys(&state(&Config::default(), &Value::Null))
        );
    }

    #[test]
    fn the_state_starts_off_and_says_what_the_running_feature_knows() {
        let s = state(&Config::default(), &Value::Null);
        assert_eq!(s["enabled"], json!(false));
        assert_eq!(s["folder"], json!(""));
        assert_eq!(s["quality"], json!("best"));
        assert_eq!(s["playlist"], json!(false));
        assert_eq!(s["ytdlp"], json!(""));
        assert_eq!(s["jobs"], json!([]));
        assert_eq!(s["picked"], json!({ "seq": 0, "path": "" }));
        assert_eq!(s["net_merge"], json!(false));
        assert_eq!(
            (s["net_links"].clone(), s["net_paused"].clone()),
            (json!([]), json!(false))
        );
        let runtime = json!({
            "ytdlp": "/opt/homebrew/bin/yt-dlp",
            "ytdlp_version": "2026.08.19",
            "ffmpeg": "",
            "folder_path": "/Users/x/Downloads",
            "jobs": [{ "id": 1 }],
            "picked": { "seq": 2, "path": "/Users/x/Movies" },
            "net_links": [{ "name": "en0" }],
            "net_paused": true,
        });
        let s = state(&Config::default(), &runtime);
        assert_eq!(s["net_links"], json!([{ "name": "en0" }]));
        assert_eq!(s["net_paused"], json!(true));
        assert_eq!(s["ytdlp_version"], json!("2026.08.19"));
        assert_eq!(s["folder_path"], json!("/Users/x/Downloads"));
        assert_eq!(s["jobs"], json!([{ "id": 1 }]));
        assert_eq!(s["picked"]["seq"], json!(2));
        let odd = json!({ "ytdlp": 5, "jobs": "none", "picked": [], "net_links": {}, "net_paused": "yes" });
        let s = state(&Config::default(), &odd);
        assert_eq!(
            (
                s["ytdlp"].clone(),
                s["jobs"].clone(),
                s["picked"]["seq"].clone(),
                s["net_links"].clone(),
                s["net_paused"].clone()
            ),
            (json!(""), json!([]), json!(0), json!([]), json!(false)),
            "a runtime of the wrong shape says nothing"
        );
        let hostile = Config {
            quality: "8k".into(),
            folder: "relative".into(),
            ..Config::default()
        };
        let s = state(&hostile, &Value::Null);
        assert_eq!(
            (s["quality"].clone(), s["folder"].clone()),
            (json!("best"), json!(""))
        );
    }

    #[test]
    fn the_switches_the_quality_and_the_folder_are_validated() {
        let mut config = Config::default();
        for path in ["enabled", "playlist", "net_merge"] {
            for bad in [json!("yes"), json!(null), json!(1)] {
                assert!(
                    set(&mut config, path, &bad).unwrap().is_err(),
                    "{path} {bad}"
                );
            }
        }
        assert!(set(&mut config, "enabled", &json!(true)).unwrap().is_ok() && config.enabled);
        assert!(set(&mut config, "playlist", &json!(true)).unwrap().is_ok() && config.playlist);
        assert!(set(&mut config, "net_merge", &json!(true)).unwrap().is_ok() && config.net_merge);
        for q in Quality::ALL {
            assert!(
                set(&mut config, "quality", &json!(q.key()))
                    .unwrap()
                    .is_ok()
            );
            assert_eq!(config.quality, q.key());
        }
        for bad in [
            json!("4k"),
            json!(""),
            json!(720),
            json!(null),
            json!("Best"),
        ] {
            assert!(set(&mut config, "quality", &bad).unwrap().is_err(), "{bad}");
        }
        assert_eq!(config.quality, "audio", "a refused value changes nothing");

        let here = std::env::temp_dir();
        let here = here.to_str().unwrap();
        assert!(set(&mut config, "folder", &json!(here)).unwrap().is_ok());
        assert_eq!(config.folder, here);
        assert!(set(&mut config, "folder", &json!("")).unwrap().is_ok());
        assert_eq!(config.folder, "", "empty goes back to Downloads");
        for bad in [
            json!("relative/dir"),
            json!("~/Movies"),
            json!("/no/such/folder/anywhere"),
            json!(5),
            json!(null),
        ] {
            assert!(set(&mut config, "folder", &bad).unwrap().is_err(), "{bad}");
        }
        assert_eq!(config.folder, "");
        assert!(set(&mut config, "nonsense", &json!(1)).is_none());
    }
}
