//! The settings model: the JSON state the page renders, and the changes it may ask for, each validated
//! before it touches the config. Pure data with no AppKit, so every rule is covered by plain unit tests.

use crate::{
    config::{Config, HotkeySpec, parse_hotkey},
    i18n::Language,
};
use core_window::Layout;
use macos_sys::{login_item::LoginItem, workspace::DisplayOptions};
use serde::Deserialize;
use serde_json::{Value, json};
use std::collections::HashMap;

/// Longest bundle identifier accepted from the page.
const MAX_ID_LEN: usize = 200;

/// What the page can send, as JSON with a `t` field naming the kind.
#[derive(Debug, Deserialize, PartialEq)]
#[serde(tag = "t", rename_all = "snake_case")]
pub enum Message {
    Ready,
    Set {
        path: String,
        value: Value,
    },
    AppRule {
        id: String,
        field: RuleField,
        value: Option<bool>,
    },
    AppAdd {
        id: String,
    },
    AppRemove {
        id: String,
    },
    IgnoredAdd {
        id: String,
    },
    IgnoredRemove {
        id: String,
    },
    Hotkey {
        target: String,
        value: String,
    },
    /// The page is capturing a key combination: global shortcuts must not fire meanwhile.
    Recording {
        on: bool,
    },
    Action {
        name: String,
    },
    PickApp {
        purpose: Purpose,
    },
    /// The apps that are open now, to offer in the "add app" menu.
    Running {
        purpose: Purpose,
    },
    Icons {
        ids: Vec<String>,
    },
    /// The shape of one scroll glide, for the graph.
    Curve {
        duration_ms: f64,
    },
    Page {
        id: String,
    },
    JsError {
        message: String,
        #[serde(default)]
        line: u32,
    },
}

#[derive(Debug, Deserialize, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum RuleField {
    Smooth,
    Reverse,
}

/// Which list an app picked in the page is destined for.
#[derive(Debug, Deserialize, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Purpose {
    Scroll,
    Ignore,
}

/// What the app has to do after a change was written to the config.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Effect {
    None,
    Mouse,
    Language,
    LoginItem(bool),
    VolumeHud(bool),
    History,
    Tiling,
    Notch,
    /// `clipboard` or `tiling:<layout>`.
    Hotkey(String),
}

/// A refused change. `code` is stable so the page can word it in the user's language.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Fault {
    pub code: &'static str,
    pub detail: String,
}

impl Fault {
    pub fn new(code: &'static str, detail: impl Into<String>) -> Self {
        Self {
            code,
            detail: detail.into(),
        }
    }

    pub fn to_json(&self) -> Value {
        json!({ "t": "fault", "code": self.code, "detail": self.detail })
    }
}

type Outcome = Result<Effect, Fault>;

/// Runtime facts the config file cannot know.
pub struct Env {
    pub version: &'static str,
    pub bundled: bool,
    pub config_dir: String,
    pub accessibility: bool,
    pub login: LoginItem,
    pub language: Language,
    pub history_count: usize,
    pub shelf_count: usize,
    /// Display names by bundle identifier, for the apps the lists mention.
    pub names: HashMap<String, String>,
    pub display: DisplayOptions,
}

fn login_name(login: LoginItem) -> &'static str {
    match login {
        LoginItem::Enabled => "enabled",
        LoginItem::Disabled => "disabled",
        LoginItem::NeedsApproval => "needs_approval",
        LoginItem::Unavailable => "unavailable",
    }
}

fn app_entry(id: &str, names: &HashMap<String, String>) -> Value {
    json!({ "id": id, "name": names.get(id).map_or(id, String::as_str) })
}

/// The whole state the page renders.
pub fn state(config: &Config, env: &Env) -> Value {
    let mut rules: Vec<(&String, &core_mouse::AppRule)> = config.mouse.apps.iter().collect();
    let name_of = |id: &str| env.names.get(id).cloned().unwrap_or_else(|| id.to_owned());
    rules.sort_by_key(|(id, _)| name_of(id).to_lowercase());
    let apps: Vec<Value> = rules
        .into_iter()
        .map(|(id, rule)| {
            json!({ "id": id, "name": name_of(id), "smooth": rule.smooth, "reverse": rule.reverse })
        })
        .collect();

    let mut ignored: Vec<&String> = config.ignored_apps.iter().collect();
    ignored.sort_by_key(|id| name_of(id).to_lowercase());
    let ignored: Vec<Value> = ignored
        .into_iter()
        .map(|id| app_entry(id, &env.names))
        .collect();

    let layouts: Vec<Value> = Layout::ALL
        .into_iter()
        .map(|layout| {
            let shortcut = config
                .tiling
                .shortcuts
                .get(layout.key())
                .map_or(layout.default_shortcut(), |s| s.trim());
            json!({ "key": layout.key(), "shortcut": shortcut, "default": layout.default_shortcut() })
        })
        .collect();

    let m = &config.mouse;
    json!({
        "app": {
            "version": env.version,
            "bundled": env.bundled,
            "config_dir": env.config_dir,
        },
        "env": {
            "accessibility": env.accessibility,
            "login": login_name(env.login),
            "language": env.language.code(),
            "reduce_transparency": env.display.reduce_transparency,
            "reduce_motion": env.display.reduce_motion,
            "increase_contrast": env.display.increase_contrast,
        },
        "general": {
            "language": config.language,
            "show_on_launch": config.show_settings_on_launch,
        },
        "mouse": {
            "smooth": m.smooth,
            "reverse": m.reverse,
            "step_px": m.step_px,
            "speed": m.speed,
            "duration_ms": m.duration_ms,
            "shift_horizontal": m.shift_horizontal,
            "option_boost": m.option_boost,
            "boost": m.boost,
            "command_zoom": m.command_zoom,
            "apps": apps,
        },
        "clipboard": {
            "hotkey": config.hotkey,
            "auto_paste": config.auto_paste,
            "paste_plain": config.paste_plain,
            "history_limit": config.history_limit,
            "count": env.history_count,
            "ignored": ignored,
        },
        "notch": {
            "enabled": config.notch.enabled,
            "volume_hud": config.volume_hud,
            "shelf_count": env.shelf_count,
        },
        "tiling": {
            "enabled": config.tiling.enabled,
            "layouts": layouts,
        },
    })
}

fn flag(value: &Value) -> Result<bool, Fault> {
    value
        .as_bool()
        .ok_or_else(|| Fault::new("invalid_value", "expected true or false"))
}

fn number(value: &Value, low: f64, high: f64) -> Result<f64, Fault> {
    let n = value
        .as_f64()
        .filter(|n| n.is_finite())
        .ok_or_else(|| Fault::new("invalid_value", "expected a number"))?;
    Ok(n.clamp(low, high))
}

fn check_id(id: &str) -> Result<(), Fault> {
    let ok = !id.is_empty()
        && id.len() <= MAX_ID_LEN
        && id
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | '_'));
    if ok {
        Ok(())
    } else {
        Err(Fault::new(
            "invalid_app_id",
            id.chars().take(40).collect::<String>(),
        ))
    }
}

/// Applies one change to the config. Messages that are not config edits (`Ready`, `Action`, ...) are the
/// caller's business and give `Effect::None`.
pub fn apply(config: &mut Config, message: &Message) -> Outcome {
    match message {
        Message::Set { path, value } => set(config, path, value),
        Message::AppRule { id, field, value } => {
            check_id(id)?;
            let rule = config.mouse.apps.entry(id.clone()).or_default();
            match field {
                RuleField::Smooth => rule.smooth = *value,
                RuleField::Reverse => rule.reverse = *value,
            }
            // A rule that says nothing is just clutter in the file.
            if rule.smooth.is_none() && rule.reverse.is_none() {
                config.mouse.apps.remove(id);
            }
            Ok(Effect::Mouse)
        }
        Message::AppAdd { id } => {
            check_id(id)?;
            // The new entry does the opposite of the global switch, which is the point of listing the app:
            // with smoothing on it is blocked, with smoothing off it is allowed.
            let smooth = !config.mouse.smooth;
            config.mouse.apps.entry(id.clone()).or_default().smooth = Some(smooth);
            Ok(Effect::Mouse)
        }
        Message::AppRemove { id } => {
            config.mouse.apps.remove(id);
            Ok(Effect::Mouse)
        }
        Message::IgnoredAdd { id } => {
            check_id(id)?;
            if !config.ignored_apps.contains(id) {
                config.ignored_apps.push(id.clone());
            }
            Ok(Effect::History)
        }
        Message::IgnoredRemove { id } => {
            config.ignored_apps.retain(|app| app != id);
            Ok(Effect::History)
        }
        Message::Hotkey { target, value } => set_hotkey(config, target, value),
        Message::Action { name } if name == "reset_tiling" => {
            config.tiling.shortcuts.clear();
            Ok(Effect::Tiling)
        }
        _ => Ok(Effect::None),
    }
}

fn set(config: &mut Config, path: &str, value: &Value) -> Outcome {
    let m = &mut config.mouse;
    match path {
        "general.language" => {
            let text = value.as_str().unwrap_or_default();
            if text != "auto" && !Language::ALL.iter().any(|l| l.code() == text) {
                return Err(Fault::new("invalid_value", text));
            }
            config.language = text.to_owned();
            Ok(Effect::Language)
        }
        "general.show_on_launch" => {
            config.show_settings_on_launch = flag(value)?;
            Ok(Effect::None)
        }
        "launch_at_login" => Ok(Effect::LoginItem(flag(value)?)),
        "mouse.smooth" => {
            m.smooth = flag(value)?;
            Ok(Effect::Mouse)
        }
        "mouse.reverse" => {
            m.reverse = flag(value)?;
            Ok(Effect::Mouse)
        }
        "mouse.shift_horizontal" => {
            m.shift_horizontal = flag(value)?;
            Ok(Effect::Mouse)
        }
        "mouse.option_boost" => {
            m.option_boost = flag(value)?;
            Ok(Effect::Mouse)
        }
        "mouse.command_zoom" => {
            m.command_zoom = flag(value)?;
            Ok(Effect::Mouse)
        }
        "mouse.step_px" => {
            m.step_px = number(value, 1.0, 400.0)?;
            Ok(Effect::Mouse)
        }
        "mouse.speed" => {
            m.speed = number(value, 0.1, 10.0)?;
            Ok(Effect::Mouse)
        }
        "mouse.duration_ms" => {
            m.duration_ms = number(value, 50.0, 2000.0)?;
            Ok(Effect::Mouse)
        }
        "mouse.boost" => {
            m.boost = number(value, 1.5, 20.0)?;
            Ok(Effect::Mouse)
        }
        "clipboard.auto_paste" => {
            config.auto_paste = flag(value)?;
            Ok(Effect::History)
        }
        "clipboard.paste_plain" => {
            config.paste_plain = flag(value)?;
            Ok(Effect::History)
        }
        "clipboard.history_limit" => {
            config.history_limit = number(value, 10.0, 100_000.0)?.round() as usize;
            Ok(Effect::History)
        }
        "notch.volume_hud" => {
            config.volume_hud = flag(value)?;
            Ok(Effect::VolumeHud(config.volume_hud))
        }
        "notch.enabled" => {
            config.notch.enabled = flag(value)?;
            Ok(Effect::Notch)
        }
        "tiling.enabled" => {
            config.tiling.enabled = flag(value)?;
            Ok(Effect::Tiling)
        }
        other => Err(Fault::new("unknown_setting", other)),
    }
}

/// The shortcut a target currently has, if any.
fn shortcut_of(config: &Config, target: &str) -> Option<String> {
    if target == "clipboard" {
        return Some(config.hotkey.clone());
    }
    let layout = Layout::ALL
        .into_iter()
        .find(|l| format!("tiling:{}", l.key()) == target)?;
    let text = config
        .tiling
        .shortcuts
        .get(layout.key())
        .map_or(layout.default_shortcut(), |s| s.trim());
    (!text.is_empty()).then(|| text.to_owned())
}

fn all_targets() -> Vec<String> {
    let mut targets = vec!["clipboard".to_owned()];
    targets.extend(
        Layout::ALL
            .into_iter()
            .map(|l| format!("tiling:{}", l.key())),
    );
    targets
}

fn set_hotkey(config: &mut Config, target: &str, value: &str) -> Outcome {
    let tiling_layout = target
        .strip_prefix("tiling:")
        .and_then(|key| Layout::ALL.into_iter().find(|l| l.key() == key));
    if target != "clipboard" && tiling_layout.is_none() {
        return Err(Fault::new("unknown_setting", target));
    }
    let value = value.trim();
    if value.is_empty() {
        // Only a layout can go without a shortcut; the clipboard popup would become unreachable.
        let Some(layout) = tiling_layout else {
            return Err(Fault::new("shortcut_required", target));
        };
        config
            .tiling
            .shortcuts
            .insert(layout.key().into(), String::new());
        return Ok(Effect::Hotkey(target.to_owned()));
    }
    let spec = parse_hotkey(value).map_err(|e| {
        let code = if e.contains("modifier") {
            "needs_modifier"
        } else {
            "invalid_shortcut"
        };
        Fault::new(code, e)
    })?;
    for other in all_targets().into_iter().filter(|t| t != target) {
        let taken = shortcut_of(config, &other).and_then(|s| parse_hotkey(&s).ok());
        if taken == Some(spec) {
            return Err(Fault::new("conflict", other));
        }
    }
    match tiling_layout {
        Some(layout) if layout.default_shortcut() == value => {
            // Back to the default: leave no override behind.
            config.tiling.shortcuts.remove(layout.key());
        }
        Some(layout) => {
            config
                .tiling
                .shortcuts
                .insert(layout.key().into(), value.to_owned());
        }
        None => config.hotkey = value.to_owned(),
    }
    Ok(Effect::Hotkey(target.to_owned()))
}

/// The glide of one wheel push, as `[milliseconds, share covered, relative speed]` triples, computed by the
/// same code that smooths the real scrolling.
pub fn curve(duration_ms: f64) -> Value {
    let duration = if duration_ms.is_finite() {
        duration_ms.clamp(50.0, 2000.0)
    } else {
        360.0
    };
    let points: Vec<Value> = core_mouse::glide_profile(duration, 64)
        .into_iter()
        .map(|p| json!([p.t_ms, p.covered, p.speed]))
        .collect();
    json!({ "t": "curve", "duration_ms": duration, "points": points })
}

/// The parsed shortcut of a target, for registering it.
pub fn hotkey_spec(config: &Config, target: &str) -> Option<HotkeySpec> {
    shortcut_of(config, target).and_then(|s| parse_hotkey(&s).ok())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn env() -> Env {
        Env {
            version: "0.2.0",
            bundled: true,
            config_dir: "/tmp/figghost".into(),
            accessibility: true,
            login: LoginItem::Disabled,
            language: Language::English,
            history_count: 12,
            shelf_count: 3,
            names: HashMap::from([("com.apple.Safari".to_owned(), "Safari".to_owned())]),
            display: DisplayOptions::default(),
        }
    }

    fn msg(text: &str) -> Message {
        serde_json::from_str(text).unwrap()
    }

    fn set_value(config: &mut Config, path: &str, value: Value) -> Outcome {
        apply(
            config,
            &Message::Set {
                path: path.into(),
                value,
            },
        )
    }

    #[test]
    fn the_page_messages_parse() {
        assert_eq!(msg(r#"{"t":"ready"}"#), Message::Ready);
        assert_eq!(
            msg(r#"{"t":"set","path":"mouse.smooth","value":false}"#),
            Message::Set {
                path: "mouse.smooth".into(),
                value: json!(false)
            }
        );
        assert_eq!(
            msg(r#"{"t":"app_rule","id":"com.a.b","field":"smooth","value":null}"#),
            Message::AppRule {
                id: "com.a.b".into(),
                field: RuleField::Smooth,
                value: None
            }
        );
        assert_eq!(
            msg(r#"{"t":"running","purpose":"scroll"}"#),
            Message::Running {
                purpose: Purpose::Scroll
            }
        );
        assert_eq!(
            msg(r#"{"t":"curve","duration_ms":360}"#),
            Message::Curve { duration_ms: 360.0 }
        );
        assert_eq!(
            msg(r#"{"t":"pick_app","purpose":"ignore"}"#),
            Message::PickApp {
                purpose: Purpose::Ignore
            }
        );
        assert_eq!(
            msg(r#"{"t":"js_error","message":"x"}"#),
            Message::JsError {
                message: "x".into(),
                line: 0
            }
        );
        assert!(serde_json::from_str::<Message>(r#"{"t":"nonsense"}"#).is_err());
    }

    #[test]
    fn the_state_has_everything_the_page_reads() {
        let mut config = Config::default();
        config.mouse.apps.insert(
            "com.apple.Safari".into(),
            core_mouse::AppRule {
                smooth: Some(false),
                reverse: None,
            },
        );
        config.ignored_apps = vec!["com.example.vault".into()];
        let s = state(&config, &env());
        assert_eq!(s["app"]["version"], "0.2.0");
        assert_eq!(s["env"]["login"], "disabled");
        assert_eq!(s["general"]["language"], "auto");
        assert_eq!(s["mouse"]["step_px"], 36.0);
        assert_eq!(s["mouse"]["apps"][0]["name"], "Safari");
        assert_eq!(s["mouse"]["apps"][0]["smooth"], false);
        assert_eq!(s["mouse"]["apps"][0]["reverse"], Value::Null);
        // An app we cannot name is listed by its identifier.
        assert_eq!(s["clipboard"]["ignored"][0]["name"], "com.example.vault");
        assert_eq!(s["clipboard"]["hotkey"], "cmd+shift+c");
        assert_eq!(s["clipboard"]["count"], 12);
        assert_eq!(s["notch"]["shelf_count"], 3);
        assert_eq!(s["tiling"]["layouts"].as_array().unwrap().len(), 13);
        assert_eq!(s["tiling"]["layouts"][0]["key"], "left");
        assert_eq!(s["tiling"]["layouts"][0]["shortcut"], "ctrl+opt+left");
    }

    /// Every key path of a JSON document; an array stands for its first element.
    fn key_paths(value: &Value, prefix: &str, out: &mut Vec<String>) {
        match value {
            Value::Object(map) => {
                for (key, inner) in map {
                    let path = format!("{prefix}.{key}");
                    out.push(path.clone());
                    key_paths(inner, &path, out);
                }
            }
            Value::Array(items) => {
                if let Some(first) = items.first() {
                    key_paths(first, &format!("{prefix}[]"), out);
                }
            }
            _ => {}
        }
    }

    #[test]
    fn the_sample_state_used_for_previews_has_the_shape_of_the_real_one() {
        let sample: Value = serde_json::from_str(include_str!("../../web/fixture.json")).unwrap();
        let mut config = Config::default();
        config
            .mouse
            .apps
            .insert("com.a".into(), core_mouse::AppRule::default());
        config.ignored_apps = vec!["com.b".into()];
        let real = state(&config, &env());
        let (mut a, mut b) = (Vec::new(), Vec::new());
        key_paths(&sample, "", &mut a);
        key_paths(&real, "", &mut b);
        a.sort();
        b.sort();
        assert_eq!(
            a, b,
            "web/fixture.json is out of date with settings::model::state"
        );
    }

    #[test]
    fn app_lists_are_sorted_by_name_not_by_identifier() {
        let mut config = Config::default();
        for id in ["com.zeta.app", "com.alpha.app"] {
            config
                .mouse
                .apps
                .insert(id.into(), core_mouse::AppRule::default());
        }
        let mut e = env();
        e.names.insert("com.zeta.app".into(), "Alpha".into());
        e.names.insert("com.alpha.app".into(), "Zulu".into());
        let s = state(&config, &e);
        assert_eq!(s["mouse"]["apps"][0]["name"], "Alpha");
        assert_eq!(s["mouse"]["apps"][1]["name"], "Zulu");
    }

    #[test]
    fn numbers_are_clamped_and_bad_types_are_refused() {
        let mut config = Config::default();
        assert_eq!(
            set_value(&mut config, "mouse.step_px", json!(9999)),
            Ok(Effect::Mouse)
        );
        assert_eq!(config.mouse.step_px, 400.0);
        set_value(&mut config, "mouse.speed", json!(0.0)).unwrap();
        assert_eq!(config.mouse.speed, 0.1);
        set_value(&mut config, "mouse.duration_ms", json!(10)).unwrap();
        assert_eq!(config.mouse.duration_ms, 50.0);
        set_value(&mut config, "mouse.boost", json!(100)).unwrap();
        assert_eq!(config.mouse.boost, 20.0);
        set_value(&mut config, "clipboard.history_limit", json!(3)).unwrap();
        assert_eq!(config.history_limit, 10);
        let bad = set_value(&mut config, "mouse.smooth", json!("yes")).unwrap_err();
        assert_eq!(bad.code, "invalid_value");
        assert_eq!(
            set_value(&mut config, "mouse.step_px", json!(null))
                .unwrap_err()
                .code,
            "invalid_value"
        );
        assert_eq!(
            set_value(&mut config, "nope.nothing", json!(1))
                .unwrap_err()
                .code,
            "unknown_setting"
        );
    }

    #[test]
    fn each_setting_reports_what_must_be_done() {
        let mut config = Config::default();
        assert_eq!(
            set_value(&mut config, "mouse.reverse", json!(true)),
            Ok(Effect::Mouse)
        );
        assert!(config.mouse.reverse);
        assert_eq!(
            set_value(&mut config, "general.language", json!("vi")),
            Ok(Effect::Language)
        );
        assert_eq!(config.language, "vi");
        assert_eq!(
            set_value(&mut config, "general.language", json!("fr"))
                .unwrap_err()
                .code,
            "invalid_value"
        );
        assert_eq!(
            set_value(&mut config, "launch_at_login", json!(true)),
            Ok(Effect::LoginItem(true))
        );
        assert_eq!(
            set_value(&mut config, "notch.volume_hud", json!(true)),
            Ok(Effect::VolumeHud(true))
        );
        assert_eq!(
            set_value(&mut config, "notch.enabled", json!(false)),
            Ok(Effect::Notch)
        );
        assert!(!config.notch.enabled);
        assert_eq!(
            set_value(&mut config, "tiling.enabled", json!(false)),
            Ok(Effect::Tiling)
        );
        assert_eq!(
            set_value(&mut config, "clipboard.paste_plain", json!(true)),
            Ok(Effect::History)
        );
        assert_eq!(
            set_value(&mut config, "general.show_on_launch", json!(false)),
            Ok(Effect::None)
        );
        assert!(!config.show_settings_on_launch);
    }

    #[test]
    fn app_rules_are_added_edited_and_tidied() {
        let mut config = Config::default();
        // With smoothing on, a listed app is blocked; with it off, allowed.
        apply(
            &mut config,
            &Message::AppAdd {
                id: "com.game".into(),
            },
        )
        .unwrap();
        assert_eq!(config.mouse.apps["com.game"].smooth, Some(false));
        config.mouse.smooth = false;
        apply(
            &mut config,
            &Message::AppAdd {
                id: "com.browser".into(),
            },
        )
        .unwrap();
        assert_eq!(config.mouse.apps["com.browser"].smooth, Some(true));

        let rule = |field, value| Message::AppRule {
            id: "com.game".into(),
            field,
            value,
        };
        apply(&mut config, &rule(RuleField::Reverse, Some(true))).unwrap();
        assert_eq!(config.mouse.apps["com.game"].reverse, Some(true));
        apply(&mut config, &rule(RuleField::Smooth, None)).unwrap();
        assert!(config.mouse.apps.contains_key("com.game"));
        // Neither field set any more: the entry disappears.
        apply(&mut config, &rule(RuleField::Reverse, None)).unwrap();
        assert!(!config.mouse.apps.contains_key("com.game"));
        apply(
            &mut config,
            &Message::AppRemove {
                id: "com.browser".into(),
            },
        )
        .unwrap();
        assert!(config.mouse.apps.is_empty());
    }

    #[test]
    fn app_identifiers_from_the_page_are_checked() {
        let mut config = Config::default();
        for bad in ["", "has space", "semi;colon", &"a".repeat(300), "slash/x"] {
            let result = apply(&mut config, &Message::AppAdd { id: bad.into() });
            assert_eq!(result.unwrap_err().code, "invalid_app_id", "{bad:?}");
        }
        assert!(config.mouse.apps.is_empty());
        apply(
            &mut config,
            &Message::AppAdd {
                id: "com.apple.dt.Xcode".into(),
            },
        )
        .unwrap();
        apply(
            &mut config,
            &Message::AppAdd {
                id: "org.mozilla.firefox-dev_1".into(),
            },
        )
        .unwrap();
        assert_eq!(config.mouse.apps.len(), 2);
    }

    #[test]
    fn ignored_apps_do_not_duplicate() {
        let mut config = Config::default();
        let add = Message::IgnoredAdd {
            id: "com.vault".into(),
        };
        assert_eq!(apply(&mut config, &add), Ok(Effect::History));
        apply(&mut config, &add).unwrap();
        assert_eq!(config.ignored_apps, vec!["com.vault".to_owned()]);
        apply(
            &mut config,
            &Message::IgnoredRemove {
                id: "com.vault".into(),
            },
        )
        .unwrap();
        assert!(config.ignored_apps.is_empty());
    }

    fn hotkey(config: &mut Config, target: &str, value: &str) -> Outcome {
        apply(
            config,
            &Message::Hotkey {
                target: target.into(),
                value: value.into(),
            },
        )
    }

    #[test]
    fn the_clipboard_shortcut_can_change_but_not_vanish() {
        let mut config = Config::default();
        assert_eq!(
            hotkey(&mut config, "clipboard", "cmd+opt+v"),
            Ok(Effect::Hotkey("clipboard".into()))
        );
        assert_eq!(config.hotkey, "cmd+opt+v");
        assert_eq!(
            hotkey(&mut config, "clipboard", "").unwrap_err().code,
            "shortcut_required"
        );
        assert_eq!(
            hotkey(&mut config, "clipboard", "v").unwrap_err().code,
            "needs_modifier"
        );
        assert_eq!(
            hotkey(&mut config, "clipboard", "cmd+§").unwrap_err().code,
            "invalid_shortcut"
        );
        // Function keys are allowed on their own.
        hotkey(&mut config, "clipboard", "f13").unwrap();
        assert_eq!(config.hotkey, "f13");
    }

    #[test]
    fn shortcuts_cannot_be_used_twice() {
        let mut config = Config::default();
        let taken = hotkey(&mut config, "tiling:right", "ctrl+opt+left").unwrap_err();
        assert_eq!(
            (taken.code, taken.detail.as_str()),
            ("conflict", "tiling:left")
        );
        let taken = hotkey(&mut config, "clipboard", "ctrl+opt+up").unwrap_err();
        assert_eq!(
            (taken.code, taken.detail.as_str()),
            ("conflict", "tiling:top")
        );
        // Spelling does not matter, the key combination does.
        let taken = hotkey(&mut config, "tiling:bottom", "opt+control+left").unwrap_err();
        assert_eq!(taken.code, "conflict");
        // A shortcut that was cleared is free again.
        hotkey(&mut config, "tiling:left", "").unwrap();
        hotkey(&mut config, "tiling:right", "ctrl+opt+left").unwrap();
        // Asking for what it already has is not a conflict with itself.
        hotkey(&mut config, "tiling:right", "ctrl+opt+left").unwrap();
    }

    #[test]
    fn tiling_shortcuts_keep_the_config_minimal() {
        let mut config = Config::default();
        hotkey(&mut config, "tiling:left", "ctrl+opt+h").unwrap();
        assert_eq!(config.tiling.shortcuts["left"], "ctrl+opt+h");
        // Typing the default back removes the override.
        hotkey(&mut config, "tiling:left", "ctrl+opt+left").unwrap();
        assert!(config.tiling.shortcuts.is_empty());
        hotkey(&mut config, "tiling:center", "").unwrap();
        assert_eq!(config.tiling.shortcuts["center"], "");
        assert_eq!(hotkey_spec(&config, "tiling:center"), None);
        assert!(hotkey_spec(&config, "tiling:left").is_some());
        assert_eq!(
            apply(
                &mut config,
                &Message::Action {
                    name: "reset_tiling".into()
                }
            ),
            Ok(Effect::Tiling)
        );
        assert!(config.tiling.shortcuts.is_empty());
        assert_eq!(
            hotkey(&mut config, "tiling:diagonal", "ctrl+x")
                .unwrap_err()
                .code,
            "unknown_setting"
        );
    }

    #[test]
    fn the_curve_reply_has_the_shape_the_graph_draws() {
        let reply = curve(360.0);
        assert_eq!(reply["t"], "curve");
        assert_eq!(reply["duration_ms"], 360.0);
        let points = reply["points"].as_array().unwrap();
        assert_eq!(points.len(), 65);
        assert_eq!(points[0][0], 0.0);
        assert_eq!(points[0][1], 0.0);
        let last = points.last().unwrap();
        assert!(last[0].as_f64().unwrap() > 500.0 && last[1].as_f64().unwrap() > 0.97);
        // Nonsense is clamped instead of drawn.
        assert_eq!(curve(f64::NAN)["duration_ms"], 360.0);
        assert_eq!(curve(9.0)["duration_ms"], 50.0);
        assert_eq!(curve(1e9)["duration_ms"], 2000.0);
    }

    #[test]
    fn messages_that_are_not_edits_change_nothing() {
        let mut config = Config::default();
        let before = serde_json::to_string(&config).unwrap();
        for m in [
            Message::Ready,
            Message::Recording { on: true },
            Message::Page {
                id: "scroll".into(),
            },
            Message::Action {
                name: "quit".into(),
            },
        ] {
            assert_eq!(apply(&mut config, &m), Ok(Effect::None));
        }
        assert_eq!(serde_json::to_string(&config).unwrap(), before);
    }
}
