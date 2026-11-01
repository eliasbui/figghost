//! This feature's part of the settings model: the JSON the page renders and the changes it may make.
//! Pure data with no AppKit, so every rule is covered by plain unit tests.

use super::{
    CANVAS_EDGE, Config, HISTORY_SECONDS, KnownLink, MAX_ALLOWED, MAX_KNOWN_LINKS, MAX_MERGE, MAX_PLACES, MAX_QUOTA_MB, Wire,
    sanitized_names, valid_text,
};
use super::dns;
use core_netcontrol::policy::Period;
use crate::settings::model::Fault;
use core_netcontrol::rules::{
    self, MAX_KBPS, MAX_RULES, Priority, Target, priority_for_new_wire, valid_app,
};
use serde_json::{Value, json};

/// The state under the key "network". `runtime` is `Feature::runtime()`, or null when the feature is not running.
/// `web/features/network.fixture.json` must have exactly the same keys; a unit test checks it.
pub fn state(config: &Config, runtime: &Value) -> Value {
    let config = config.clone().sanitized();
    let list = |key: &str| {
        runtime
            .get(key)
            .filter(|v| v.is_array())
            .cloned()
            .unwrap_or_else(|| json!([]))
    };
    let number = |key: &str| runtime[key].as_u64().unwrap_or(0);
    json!({
        "enabled": config.enabled,
        "control": config.control,
        "history": config.history,
        "history_seconds": config.history_seconds,
        "history_view": runtime.get("history").filter(|v| v.is_object()).cloned().unwrap_or_else(|| json!({
            "empty": true, "today": [], "week": [], "month": [], "daily": [],
        })),
        "port": runtime["port"].as_u64().unwrap_or(u64::from(config.port)),
        "wires": config.wires.iter().map(|w| json!({
            "app": w.app, "target": w.target, "priority": w.priority, "down_kbps": w.down_kbps, "up_kbps": w.up_kbps,
            "quota_mb": w.quota_mb, "quota_period": w.quota_period,
        })).collect::<Vec<_>>(),
        "watch": config.watch,
        "dns": config.dns,
        "dns_nextdns": config.dns_nextdns,
        "dns_custom": config.dns_custom,
        "anomaly": config.anomaly,
        "anomaly_ignore": config.anomaly_ignore,
        "metered": {
            "enabled": config.metered.enabled, "auto": config.metered.auto, "marked": config.metered.marked,
            "unmarked": config.metered.unmarked, "allow": config.metered.allow, "cap_mb": config.metered.cap_mb,
            "period": config.metered.period,
        },
        "policy": runtime.get("policy").filter(|v| v.is_object()).cloned().unwrap_or_else(|| json!({
            "metered_now": false, "link": "", "cap_used": 0, "cap_level": "ok", "refusing": false, "quotas": [],
        })),
        "merge": config.merge,
        "known_links": config.links.iter().map(|l| json!({ "name": l.name, "label": l.label })).collect::<Vec<_>>(),
        "layout": config.layout,
        "known_apps": list("known_apps"),
        "status": runtime["status"].as_str().unwrap_or("off"),
        "saved": runtime["saved"].as_bool().unwrap_or(false),
        "running": runtime["running"].as_bool().unwrap_or(false),
        "paused": runtime["paused"].as_bool().unwrap_or(false),
        "links": list("links"),
        "apps": list("apps"),
        "total_down": number("total_down"),
        "total_up": number("total_up"),
        "tether": runtime.get("tether").filter(|v| v.is_object()).cloned().unwrap_or_else(|| json!({
            "gnirehtet": false, "adb": false, "running": false, "phones": [], "unauthorized": 0,
            "meter": { "down": 0, "up": 0, "received": 0, "sent": 0 }, "test": { "state": "idle" },
        })),
    })
}

fn app_of(value: &Value) -> Option<String> {
    value["app"]
        .as_str()
        .map(str::trim)
        .filter(|a| valid_app(a))
        .map(str::to_owned)
}

/// A quota or a cap in megabytes: nothing (0) or up to ten terabytes.
fn megabytes(value: &Value) -> Option<u32> {
    match value {
        Value::Null => Some(0),
        v => v.as_u64().filter(|n| *n <= u64::from(MAX_QUOTA_MB)).map(|n| n as u32),
    }
}

fn names(value: &Value, valid: impl Fn(&str) -> bool) -> Option<Vec<String>> {
    value.as_array()?.iter().map(|v| v.as_str().filter(|n| valid(n.trim())).map(|n| n.trim().to_owned())).collect()
}

fn kbps(value: &Value) -> Option<u32> {
    match value {
        Value::Null => Some(0),
        v => v
            .as_u64()
            .filter(|n| *n <= u64::from(MAX_KBPS))
            .map(|n| n as u32),
    }
}

/// A change from the page. `path` is relative to the feature. Values must be validated here; the page is not trusted.
pub fn set(config: &mut Config, path: &str, value: &Value) -> Option<Result<(), Fault>> {
    let bad = || Fault::new("invalid_value", path);
    Some(
        match path {
            "enabled" => value
                .as_bool()
                .map(|on| config.enabled = on)
                .ok_or_else(bad),
            "control" => value
                .as_bool()
                .map(|on| config.control = on)
                .ok_or_else(bad),
            "dns" => value
                .as_str()
                .filter(|d| dns::PROVIDERS.iter().any(|p| p.key == *d) || matches!(*d, "nextdns" | "custom"))
                .map(|d| config.dns = d.to_owned())
                .ok_or_else(bad),
            // An empty text clears it; anything else must be of the right shape.
            "dns_nextdns" => value
                .as_str()
                .map(str::trim)
                .filter(|t| t.is_empty() || dns::valid_nextdns(t))
                .map(|t| config.dns_nextdns = t.to_owned())
                .ok_or_else(bad),
            "dns_custom" => value
                .as_str()
                .map(str::trim)
                .filter(|t| t.is_empty() || dns::valid_https_url(t))
                .map(|t| config.dns_custom = t.to_owned())
                .ok_or_else(bad),
            "watch" => value.as_bool().map(|on| config.watch = on).ok_or_else(bad),
            "anomaly" => value.as_bool().map(|on| config.anomaly = on).ok_or_else(bad),
            "anomaly_ignore" => names(value, rules::valid_app)
                .filter(|list| list.len() <= MAX_ALLOWED)
                .map(|list| config.anomaly_ignore = sanitized_names(&list))
                .ok_or_else(bad),
            "metered.enabled" => value.as_bool().map(|on| config.metered.enabled = on).ok_or_else(bad),
            "metered.auto" => value.as_bool().map(|on| config.metered.auto = on).ok_or_else(bad),
            // One connection said to be metered, not metered, or left to the guess.
            "metered.mark" => (|| {
                let name = value["name"].as_str().filter(|n| rules::valid_interface(n))?.to_owned();
                config.metered.marked.retain(|n| *n != name);
                config.metered.unmarked.retain(|n| *n != name);
                match value["state"].as_str()? {
                    "metered" => config.metered.marked.push(name),
                    "not" => config.metered.unmarked.push(name),
                    "auto" => {}
                    _ => return None,
                }
                config.metered = std::mem::take(&mut config.metered).sanitized();
                Some(())
            })()
            .ok_or_else(bad),
            "metered.allow" => names(value, rules::valid_app)
                .filter(|list| list.len() <= MAX_ALLOWED)
                .map(|list| {
                    config.metered.allow = list;
                    config.metered = std::mem::take(&mut config.metered).sanitized();
                })
                .ok_or_else(bad),
            "metered.cap_mb" => megabytes(value).map(|n| config.metered.cap_mb = n).ok_or_else(bad),
            "metered.period" => value
                .as_str()
                .and_then(Period::from_id)
                .map(|p| config.metered.period = p.id().into())
                .ok_or_else(bad),
            "history" => value
                .as_bool()
                .map(|on| config.history = on)
                .ok_or_else(bad),
            "history_seconds" => value
                .as_u64()
                .filter(|s| HISTORY_SECONDS.contains(s))
                .map(|s| config.history_seconds = s)
                .ok_or_else(bad),
            "port" => value
                .as_u64()
                .filter(|p| (1024..=65535).contains(p))
                .map(|p| config.port = p as u16)
                .ok_or_else(bad),
            // A wire dragged on the diagram: an app to a place. A new one comes below those already wired there.
            "wire" => (|| {
                let app = app_of(value)?;
                let target = Target::from_id(value["target"].as_str()?)?;
                let rules = config.rules();
                let count = config.wires.len();
                match config
                    .wires
                    .iter_mut()
                    .find(|w| w.app.eq_ignore_ascii_case(&app))
                {
                    Some(wire) => {
                        if wire.target != target.id() {
                            wire.priority = priority_for_new_wire(&rules, &target).id().into();
                        }
                        wire.target = target.id();
                    }
                    None if count < MAX_RULES => config.wires.push(Wire {
                        app,
                        priority: priority_for_new_wire(&rules, &target).id().into(),
                        target: target.id(),
                        ..Wire::default()
                    }),
                    None => return None,
                }
                Some(())
            })()
            .ok_or_else(bad),
            // The whole of one wire, from the card of an app.
            "rule" => (|| {
                let app = app_of(value)?;
                let wire = Wire {
                    app: app.clone(),
                    target: Target::from_id(value["target"].as_str()?)?.id(),
                    priority: Priority::from_id(value["priority"].as_str()?)?.id().into(),
                    down_kbps: kbps(&value["down_kbps"])?,
                    up_kbps: kbps(&value["up_kbps"])?,
                    quota_mb: megabytes(&value["quota_mb"])?,
                    quota_period: Period::from_id(value["quota_period"].as_str().unwrap_or("month"))?.id().into(),
                };
                let count = config.wires.len();
                match config
                    .wires
                    .iter_mut()
                    .find(|w| w.app.eq_ignore_ascii_case(&app))
                {
                    Some(old) => *old = wire,
                    None if count < MAX_RULES => config.wires.push(wire),
                    None => return None,
                }
                Some(())
            })()
            .ok_or_else(bad),
            // One link wired to another on the diagram: the links of the Combined circle. An empty list takes it away.
            "merge" => value
                .as_array()
                .and_then(|list| {
                    let names: Vec<String> = list.iter().map(|v| v.as_str().map(str::to_owned)).collect::<Option<_>>()?;
                    let ok = (names.is_empty() || (2..=MAX_MERGE).contains(&names.len()))
                        && names.iter().all(|n| rules::valid_interface(n));
                    ok.then(|| {
                        config.merge = names;
                        let cleaned = config.clone().sanitized().merge;
                        config.merge = cleaned;
                    })
                })
                .ok_or_else(bad),
            // Where a node was put on the canvas.
            "place" => (|| {
                let id = value["id"].as_str().filter(|id| valid_text(id, 90))?;
                let at = |k: &str| value[k].as_i64().map(|v| v.clamp(0, i64::from(CANVAS_EDGE)) as i32);
                let (x, y) = (at("x")?, at("y")?);
                if !config.layout.contains_key(id) && config.layout.len() >= MAX_PLACES {
                    return None;
                }
                config.layout.insert(id.to_owned(), [x, y]);
                Some(())
            })()
            .ok_or_else(bad),
            // A connection seen by the canvas, kept on it while it is away.
            "known_link" => (|| {
                let name = value["name"].as_str().filter(|n| rules::valid_interface(n))?;
                let label = value["label"].as_str().map(str::trim).filter(|l| valid_text(l, 64))?;
                let count = config.links.len();
                match config.links.iter_mut().find(|l| l.name == name) {
                    Some(l) => l.label = label.to_owned(),
                    None if count < MAX_KNOWN_LINKS => {
                        config.links.push(KnownLink { name: name.to_owned(), label: label.to_owned() })
                    }
                    None => return None,
                }
                Some(())
            })()
            .ok_or_else(bad),
            "unwire" => value
                .as_str()
                .map(|app| {
                    config
                        .wires
                        .retain(|w| !w.app.eq_ignore_ascii_case(app.trim()))
                })
                .ok_or_else(bad),
            _ => return None,
        }
        .map(|()| {
            config.wires = Wire::sanitized(&config.wires);
        }),
    )
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
            serde_json::from_str(include_str!("../../../web/features/network.fixture.json"))
                .unwrap();
        let state = state(&Config::default(), &Value::Null);
        assert_eq!(keys(&fixture), keys(&state));
        assert_eq!(keys(&fixture["tether"]), keys(&state["tether"]));
        assert_eq!(keys(&fixture["metered"]), keys(&state["metered"]));
        assert_eq!(keys(&fixture["policy"]), keys(&state["policy"]));
        assert_eq!(keys(&fixture["wires"][0]), keys(&json!({
            "app": "", "target": "", "priority": "", "down_kbps": 0, "up_kbps": 0, "quota_mb": 0, "quota_period": ""
        })));
    }

    #[test]
    fn a_quota_and_a_wire_to_block_are_kept_with_the_rule_and_survive_the_next_change() {
        let mut c = Config::default();
        let rule = json!({ "app": "Dropbox", "target": "block", "priority": "low", "down_kbps": 0, "up_kbps": 0, "quota_mb": 500, "quota_period": "day" });
        assert!(set(&mut c, "rule", &rule).unwrap().is_ok());
        assert_eq!((c.wires[0].target.as_str(), c.wires[0].quota_mb, c.wires[0].quota_period.as_str()), ("block", 500, "day"));
        assert!(set(&mut c, "wire", &json!({ "app": "Steam", "target": "merge" })).unwrap().is_ok());
        assert_eq!(c.wires[0].quota_mb, 500, "wiring another app leaves the quota of this one");
        assert!(set(&mut c, "unwire", &json!("steam")).unwrap().is_ok());
        assert_eq!(c.wires.len(), 1);
        for bad in [
            json!({ "app": "A", "target": "merge", "priority": "low", "down_kbps": 0, "up_kbps": 0, "quota_mb": -1, "quota_period": "day" }),
            json!({ "app": "A", "target": "merge", "priority": "low", "down_kbps": 0, "up_kbps": 0, "quota_mb": MAX_QUOTA_MB + 1, "quota_period": "day" }),
            json!({ "app": "A", "target": "merge", "priority": "low", "down_kbps": 0, "up_kbps": 0, "quota_mb": 5, "quota_period": "week" }),
        ] {
            assert!(set(&mut c, "rule", &bad).unwrap().is_err(), "{bad}");
        }
        let odd = Config { wires: vec![Wire { app: "X".into(), quota_mb: u32::MAX, quota_period: "year".into(), ..Wire::default() }], ..Config::default() }.sanitized();
        assert_eq!((odd.wires[0].quota_mb, odd.wires[0].quota_period.as_str()), (MAX_QUOTA_MB, "month"));
        assert_eq!(c.wires[0].quota().unwrap().bytes, 500_000_000);
    }

    #[test]
    fn the_choice_of_encrypted_dns_is_validated() {
        let mut c = Config::default();
        assert_eq!(c.dns, "cloudflare");
        assert!(set(&mut c, "dns", &json!("quad9")).unwrap().is_ok() && c.dns == "quad9");
        assert!(set(&mut c, "dns", &json!("evil")).unwrap().is_err());
        assert!(set(&mut c, "dns_nextdns", &json!(" a1b2c3 ")).unwrap().is_ok() && c.dns_nextdns == "a1b2c3");
        assert!(set(&mut c, "dns_nextdns", &json!("../x")).unwrap().is_err());
        assert!(set(&mut c, "dns_nextdns", &json!("")).unwrap().is_ok() && c.dns_nextdns.is_empty());
        assert!(set(&mut c, "dns_custom", &json!("https://dns.example.com/q")).unwrap().is_ok());
        assert!(set(&mut c, "dns_custom", &json!("http://dns.example.com/q")).unwrap().is_err());
        assert_eq!(c.dns_custom, "https://dns.example.com/q", "a refused change leaves it as it was");
        let odd = Config { dns: "x".into(), dns_nextdns: "no good".into(), dns_custom: "ftp://x".into(), ..Config::default() }.sanitized();
        assert_eq!((odd.dns.as_str(), odd.dns_nextdns.as_str(), odd.dns_custom.as_str()), ("cloudflare", "", ""));
    }

    #[test]
    fn the_upload_alert_and_the_apps_it_leaves_alone_are_validated() {
        let mut c = Config::default();
        assert!(!c.anomaly && c.anomaly_ignore.is_empty() && !c.watch);
        assert!(set(&mut c, "watch", &json!(1)).unwrap().is_err());
        assert!(set(&mut c, "watch", &json!(true)).unwrap().is_ok() && c.watch);
        assert!(set(&mut c, "anomaly", &json!(2)).unwrap().is_err());
        assert!(set(&mut c, "anomaly", &json!(true)).unwrap().is_ok() && c.anomaly);
        assert!(set(&mut c, "anomaly_ignore", &json!(["Backblaze", " backblaze", "Time Machine"])).unwrap().is_ok());
        assert_eq!(c.anomaly_ignore, ["Backblaze", "Time Machine"]);
        assert!(set(&mut c, "anomaly_ignore", &json!(["a/b"])).unwrap().is_err());
        assert_eq!(c.anomaly_ignore.len(), 2, "a refused change leaves the list as it was");
        let s = state(&c, &Value::Null);
        assert_eq!((s["anomaly"].clone(), s["anomaly_ignore"][1].clone()), (json!(true), json!("Time Machine")));
    }

    #[test]
    fn the_metered_settings_are_validated() {
        let mut c = Config::default();
        assert!(c.metered.auto && !c.metered.enabled);
        assert!(set(&mut c, "metered.enabled", &json!(true)).unwrap().is_ok() && c.metered.enabled);
        assert!(set(&mut c, "metered.auto", &json!("no")).unwrap().is_err());
        assert!(set(&mut c, "metered.mark", &json!({ "name": "en7", "state": "metered" })).unwrap().is_ok());
        assert!(set(&mut c, "metered.mark", &json!({ "name": "en1", "state": "not" })).unwrap().is_ok());
        assert_eq!((c.metered.marked.clone(), c.metered.unmarked.clone()), (vec!["en7".to_owned()], vec!["en1".to_owned()]));
        assert!(set(&mut c, "metered.mark", &json!({ "name": "en7", "state": "not" })).unwrap().is_ok());
        assert_eq!((c.metered.marked.len(), c.metered.unmarked.len()), (0, 2), "a connection is marked one way only");
        assert!(set(&mut c, "metered.mark", &json!({ "name": "en7", "state": "auto" })).unwrap().is_ok());
        assert_eq!(c.metered.unmarked, ["en1"]);
        for bad in [json!({ "name": "../x", "state": "auto" }), json!({ "name": "en0", "state": "maybe" })] {
            assert!(set(&mut c, "metered.mark", &bad).unwrap().is_err(), "{bad}");
        }
        assert!(set(&mut c, "metered.allow", &json!(["Mail", " Slack ", "mail"])).unwrap().is_ok());
        assert_eq!(c.metered.allow, ["Mail", "Slack"], "trimmed, and a name twice is one");
        assert!(set(&mut c, "metered.allow", &json!(["a/b"])).unwrap().is_err());
        assert!(set(&mut c, "metered.allow", &json!("Mail")).unwrap().is_err());
        assert_eq!(c.metered.allow.len(), 2, "a refused change leaves the list as it was");
        assert!(set(&mut c, "metered.cap_mb", &json!(2000)).unwrap().is_ok() && c.metered.cap_mb == 2000);
        assert!(set(&mut c, "metered.cap_mb", &json!(MAX_QUOTA_MB + 1)).unwrap().is_err());
        assert!(set(&mut c, "metered.period", &json!("day")).unwrap().is_ok() && c.metered.period == "day");
        assert!(set(&mut c, "metered.period", &json!("year")).unwrap().is_err());
        let s = state(&c, &json!({ "policy": { "metered_now": true, "link": "Wi-Fi", "cap_used": 5, "cap_level": "warn", "refusing": true, "quotas": [] } }));
        assert_eq!((s["metered"]["cap_mb"].clone(), s["policy"]["metered_now"].clone()), (json!(2000), json!(true)));
        assert_eq!(state(&c, &Value::Null)["policy"]["metered_now"], json!(false));
    }

    #[test]
    fn the_history_switch_and_its_pace_are_validated() {
        let mut c = Config::default();
        assert!(!c.history && c.history_seconds == 5);
        assert!(set(&mut c, "history", &json!("yes")).unwrap().is_err());
        assert!(set(&mut c, "history", &json!(true)).unwrap().is_ok() && c.history);
        assert!(set(&mut c, "history_seconds", &json!(3)).unwrap().is_err());
        assert!(set(&mut c, "history_seconds", &json!(10)).unwrap().is_ok() && c.history_seconds == 10);
        let odd = Config { history_seconds: 0, ..Config::default() }.sanitized();
        assert_eq!(odd.history_seconds, 5, "a pace that is not offered is read as the usual one");
        let s = state(&c, &json!({ "history": { "empty": false, "today": [{ "app": "Safari" }], "week": [], "month": [], "daily": [] } }));
        assert_eq!(s["history_view"]["today"][0]["app"], json!("Safari"));
        assert_eq!(state(&c, &Value::Null)["history_view"]["empty"], json!(true));
    }

    #[test]
    fn it_starts_off_with_nothing_wired() {
        let s = state(&Config::default(), &Value::Null);
        assert_eq!(s["enabled"], json!(false));
        assert_eq!(s["control"], json!(false));
        assert_eq!(s["status"], json!("off"));
        assert_eq!(s["wires"], json!([]));
        assert_eq!(s["apps"], json!([]));
        let s = state(
            &Config::default(),
            &json!({ "status": "on", "running": true, "apps": [{ "name": "Safari" }], "port": 50000 }),
        );
        assert_eq!(
            (s["status"].clone(), s["running"].clone(), s["port"].clone()),
            (json!("on"), json!(true), json!(50000))
        );
        assert_eq!(s["apps"][0]["name"], json!("Safari"));
    }

    #[test]
    fn the_switches_and_the_port_are_validated() {
        let mut c = Config::default();
        assert!(set(&mut c, "enabled", &json!(1)).unwrap().is_err());
        assert!(set(&mut c, "control", &json!(true)).unwrap().is_ok() && c.control);
        assert!(set(&mut c, "port", &json!(80)).unwrap().is_err());
        assert!(set(&mut c, "port", &json!(50000)).unwrap().is_ok() && c.port == 50000);
        assert!(set(&mut c, "nonsense", &json!(1)).is_none());
    }

    #[test]
    fn the_first_app_wired_to_a_place_is_the_main_one_and_the_next_come_below() {
        let mut c = Config::default();
        assert!(
            set(
                &mut c,
                "wire",
                &json!({ "app": "Steam", "target": "link:en0" })
            )
            .unwrap()
            .is_ok()
        );
        assert!(
            set(
                &mut c,
                "wire",
                &json!({ "app": "Safari", "target": "link:en0" })
            )
            .unwrap()
            .is_ok()
        );
        assert_eq!(c.wires[0].priority, "high");
        assert_eq!(
            c.wires[1].priority, "low",
            "wired after Steam to the same link"
        );
        assert!(
            set(
                &mut c,
                "wire",
                &json!({ "app": "Safari", "target": "merge" })
            )
            .unwrap()
            .is_ok()
        );
        assert_eq!(
            (c.wires[1].target.as_str(), c.wires[1].priority.as_str()),
            ("merge", "high"),
            "alone on the merge"
        );
        assert!(
            set(
                &mut c,
                "wire",
                &json!({ "app": "Safari", "target": "link:../etc" })
            )
            .unwrap()
            .is_err()
        );
        assert!(
            set(&mut c, "wire", &json!({ "app": "", "target": "merge" }))
                .unwrap()
                .is_err()
        );
        assert!(set(&mut c, "unwire", &json!("safari")).unwrap().is_ok());
        assert_eq!(c.wires.len(), 1);
    }

    #[test]
    fn a_whole_rule_is_validated() {
        let mut c = Config::default();
        let rule = json!({ "app": "Steam", "target": "merge", "priority": "low", "down_kbps": 2048, "up_kbps": null });
        assert!(set(&mut c, "rule", &rule).unwrap().is_ok());
        assert_eq!((c.wires[0].down_kbps, c.wires[0].up_kbps), (2048, 0));
        for bad in [
            json!({ "app": "Steam", "target": "merge", "priority": "top", "down_kbps": 0, "up_kbps": 0 }),
            json!({ "app": "Steam", "target": "merge", "priority": "low", "down_kbps": -1, "up_kbps": 0 }),
            json!({ "app": "Steam", "target": "merge", "priority": "low", "down_kbps": MAX_KBPS + 1, "up_kbps": 0 }),
        ] {
            assert!(set(&mut c, "rule", &bad).unwrap().is_err(), "{bad}");
        }
        assert_eq!(
            c.wires[0].priority, "low",
            "a refused change leaves the rule as it was"
        );
    }

    #[test]
    fn the_combined_circle_the_places_and_the_links_kept_are_validated() {
        let mut c = Config::default();
        assert!(set(&mut c, "merge", &json!(["en0", "en7"])).unwrap().is_ok());
        assert_eq!(c.merge, ["en0", "en7"]);
        assert!(set(&mut c, "merge", &json!(["en0"])).unwrap().is_err(), "one link is no circle");
        assert!(set(&mut c, "merge", &json!(["en0", "../x"])).unwrap().is_err());
        assert!(set(&mut c, "merge", &json!([])).unwrap().is_ok() && c.merge.is_empty());
        assert!(set(&mut c, "place", &json!({ "id": "app:Safari", "x": 120, "y": -40 })).unwrap().is_ok());
        assert_eq!(c.layout["app:Safari"], [120, 0]);
        assert!(set(&mut c, "place", &json!({ "id": "", "x": 1, "y": 1 })).unwrap().is_err());
        assert!(set(&mut c, "known_link", &json!({ "name": "en0", "label": "Wi-Fi" })).unwrap().is_ok());
        assert!(set(&mut c, "known_link", &json!({ "name": "en0", "label": "Wi-Fi 2" })).unwrap().is_ok());
        assert_eq!(c.links.len(), 1);
        assert!(set(&mut c, "known_link", &json!({ "name": "nope", "label": "x" })).unwrap().is_err());
        let s = state(&c, &json!({ "known_apps": [{ "name": "Safari", "bundle": "" }] }));
        assert_eq!(s["known_links"][0]["label"], json!("Wi-Fi 2"));
        assert_eq!(s["layout"]["app:Safari"], json!([120, 0]));
        assert_eq!(s["known_apps"][0]["name"], json!("Safari"));
    }

    #[test]
    fn no_more_than_the_limit_of_wires_are_kept() {
        let mut c = Config::default();
        for i in 0..MAX_RULES {
            assert!(
                set(
                    &mut c,
                    "wire",
                    &json!({ "app": format!("App {i}"), "target": "merge" })
                )
                .unwrap()
                .is_ok()
            );
        }
        assert!(
            set(
                &mut c,
                "wire",
                &json!({ "app": "One more", "target": "merge" })
            )
            .unwrap()
            .is_err()
        );
    }
}
