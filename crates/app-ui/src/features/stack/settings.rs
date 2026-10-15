//! This feature's part of the settings model: the JSON the page renders and the changes it may make.
//! Pure data with no AppKit, so every rule is covered by plain unit tests.

use super::{Config, Protect, clamp_minutes, machine::Modifier};
use crate::settings::model::Fault;
use core_clipboard::{MAX_BOARD_NAME_CHARS, MAX_BOARDS};
use serde_json::{Value, json};

/// The state under the key "stack". `runtime` is `Feature::runtime()`, or null when the feature is not running.
/// `web/features/stack.fixture.json` must have exactly the same keys; a unit test checks it.
pub fn state(config: &Config, runtime: &Value) -> Value {
    json!({
        "enabled": config.enabled,
        "modifier": config.modifier.as_str(),
        "sensitive": config.sensitive.as_str(),
        "expire_minutes": config.expire_minutes,
        "boards": runtime.get("boards").cloned().unwrap_or_else(|| json!([])),
        "max_boards": MAX_BOARDS,
        "max_name": MAX_BOARD_NAME_CHARS,
        "accessibility": runtime.get("accessibility").and_then(Value::as_bool).unwrap_or(false),
        "active": runtime.get("active").and_then(Value::as_bool).unwrap_or(false),
        "queued": runtime.get("queued").and_then(Value::as_u64).unwrap_or(0),
    })
}

fn invalid(what: &'static str) -> Fault {
    Fault::new("invalid_value", what)
}

/// A change from the page. `path` is relative to the feature: "enabled", not "stack.enabled". `None` when the
/// path is not one of this feature's. Values must be validated here; the page is not trusted.
pub fn set(config: &mut Config, path: &str, value: &Value) -> Option<Result<(), Fault>> {
    Some(match path {
        "enabled" => value
            .as_bool()
            .map(|on| config.enabled = on)
            .ok_or_else(|| invalid("expected true or false")),
        "modifier" => value
            .as_str()
            .and_then(Modifier::parse)
            .map(|m| config.modifier = m)
            .ok_or_else(|| invalid("expected option, command, control or shift")),
        "sensitive" => value
            .as_str()
            .and_then(Protect::parse)
            .map(|p| config.sensitive = p)
            .ok_or_else(|| invalid("expected off, mask or never")),
        // Any number is accepted and brought into range, so a slider at its end never fails.
        "expire_minutes" => value
            .as_f64()
            .filter(|n| n.is_finite())
            .map(|n| config.expire_minutes = clamp_minutes(n))
            .ok_or_else(|| invalid("expected a number of minutes")),
        _ => return None,
    })
}

/// A button of the boards card. An action carries only a name, so the arguments are part of it:
/// `stack_board_add:<name>`, `stack_board_rename:<id>:<name>`, `stack_board_delete:<id>`. A name may contain
/// colons, because the id comes first.
#[derive(Debug, PartialEq, Eq)]
pub enum BoardAction<'a> {
    Add(&'a str),
    Rename(i64, &'a str),
    Delete(i64),
}

/// `None` when the name is not one of this feature's actions.
pub fn parse_action(name: &str) -> Option<Result<BoardAction<'_>, Fault>> {
    let (verb, rest) = name.strip_prefix("stack_board_")?.split_once(':')?;
    let id = |text: &str| {
        text.parse::<i64>()
            .map_err(|_| invalid("expected a board id"))
    };
    Some(match verb {
        "add" => Ok(BoardAction::Add(rest)),
        "rename" => {
            let (given, board) = rest.split_once(':')?;
            id(given).map(|id| BoardAction::Rename(id, board))
        }
        "delete" => id(rest).map(BoardAction::Delete),
        _ => return None,
    })
}

/// This feature has no global shortcuts: the double press of a modifier key is not one.
pub fn shortcuts(_config: &Config) -> Vec<(String, String)> {
    Vec::new()
}

/// Stores a shortcut that was already parsed and checked for conflicts; an empty value removes it.
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
    fn the_fixture_has_the_keys_of_the_state() {
        let fixture: Value =
            serde_json::from_str(include_str!("../../../web/features/stack.fixture.json")).unwrap();
        let keys = |v: &Value| {
            let mut keys: Vec<String> = v.as_object().unwrap().keys().cloned().collect();
            keys.sort();
            keys
        };
        assert_eq!(
            keys(&fixture),
            keys(&state(&Config::default(), &Value::Null))
        );
    }

    #[test]
    fn the_state_reports_the_config_and_what_is_running() {
        let config = Config {
            enabled: true,
            modifier: Modifier::Command,
            sensitive: Protect::Mask,
            expire_minutes: 30,
        };
        let runtime = json!({
            "boards": [{ "id": 3, "name": "Work", "items": 2 }],
            "accessibility": true,
            "active": true,
            "queued": 4,
        });
        let state = state(&config, &runtime);
        assert_eq!(state["enabled"], true);
        assert_eq!(state["modifier"], "command");
        assert_eq!(state["sensitive"], "mask");
        assert_eq!(state["expire_minutes"], 30);
        assert_eq!(state["boards"][0]["name"], "Work");
        assert_eq!(state["max_boards"], MAX_BOARDS);
        assert_eq!(state["max_name"], MAX_BOARD_NAME_CHARS);
        assert_eq!(state["accessibility"], true);
        assert_eq!(state["active"], true);
        assert_eq!(state["queued"], 4);
        // Not running: the page still gets a complete state.
        let idle = super::state(&Config::default(), &Value::Null);
        assert_eq!(idle["boards"], json!([]));
        assert_eq!(idle["accessibility"], false);
        assert_eq!(idle["queued"], 0);
    }

    #[test]
    fn the_switch_is_validated() {
        let mut config = Config::default();
        assert!(set(&mut config, "enabled", &json!(true)).unwrap().is_ok() && config.enabled);
        assert!(set(&mut config, "enabled", &json!("yes")).unwrap().is_err());
        assert!(set(&mut config, "enabled", &json!(1)).unwrap().is_err());
        assert!(config.enabled, "a refused change changes nothing");
        assert!(set(&mut config, "nonsense", &json!(1)).is_none());
    }

    #[test]
    fn the_modifier_and_the_choice_accept_only_their_words() {
        let mut config = Config::default();
        for m in Modifier::ALL {
            assert!(
                set(&mut config, "modifier", &json!(m.as_str()))
                    .unwrap()
                    .is_ok()
            );
            assert_eq!(config.modifier, m);
        }
        for bad in [
            json!("alt"),
            json!("Option"),
            json!(3),
            json!(null),
            json!(["option"]),
        ] {
            assert!(
                set(&mut config, "modifier", &bad).unwrap().is_err(),
                "{bad}"
            );
        }
        assert_eq!(config.modifier, Modifier::Shift, "the last good one stays");
        for p in Protect::ALL {
            assert!(
                set(&mut config, "sensitive", &json!(p.as_str()))
                    .unwrap()
                    .is_ok()
            );
            assert_eq!(config.sensitive, p);
        }
        for bad in [json!("on"), json!(true), json!(""), json!(null)] {
            assert!(
                set(&mut config, "sensitive", &bad).unwrap().is_err(),
                "{bad}"
            );
        }
        assert_eq!(config.sensitive, Protect::Never);
    }

    #[test]
    fn minutes_are_clamped_and_text_is_refused() {
        let mut config = Config::default();
        for (given, expected) in [
            (json!(30), 30),
            (json!(0), 1),
            (json!(-4), 1),
            (json!(100_000), 1440),
            (json!(7.6), 8),
            (json!(1440), 1440),
        ] {
            assert!(
                set(&mut config, "expire_minutes", &given).unwrap().is_ok(),
                "{given}"
            );
            assert_eq!(config.expire_minutes, expected, "{given}");
        }
        for bad in [json!("10"), json!(null), json!(true), json!([10])] {
            assert!(
                set(&mut config, "expire_minutes", &bad).unwrap().is_err(),
                "{bad}"
            );
        }
        assert_eq!(config.expire_minutes, 1440);
    }

    #[test]
    fn board_actions_carry_their_arguments_in_the_name() {
        use BoardAction::*;
        let parse = |name| parse_action(name).map(|r| r.map_err(|f| f.code));
        assert_eq!(parse("stack_board_add:Work"), Some(Ok(Add("Work"))));
        assert_eq!(
            parse("stack_board_add:a:b c"),
            Some(Ok(Add("a:b c"))),
            "a name may hold colons"
        );
        assert_eq!(
            parse("stack_board_add:"),
            Some(Ok(Add(""))),
            "an empty name is the store's to refuse"
        );
        assert_eq!(
            parse("stack_board_rename:7:New: name"),
            Some(Ok(Rename(7, "New: name")))
        );
        assert_eq!(parse("stack_board_rename:7:"), Some(Ok(Rename(7, ""))));
        assert_eq!(parse("stack_board_delete:12"), Some(Ok(Delete(12))));
        // A bad id is refused with a message; a shape that is not ours is not answered at all.
        for bad in [
            "stack_board_rename:x:Name",
            "stack_board_delete:",
            "stack_board_delete:1.5",
            "stack_board_delete:99999999999999999999",
        ] {
            assert_eq!(parse(bad), Some(Err("invalid_value")), "{bad}");
        }
        for other in [
            "stack_board_add",
            "stack_board_rename:7",
            "stack_board_zap:1",
            "stack_end",
            "open_accessibility",
            "",
            "board_add:x",
        ] {
            assert!(parse(other).is_none(), "{other}");
        }
    }

    #[test]
    fn there_are_no_global_shortcuts() {
        let mut config = Config::default();
        assert!(shortcuts(&config).is_empty());
        assert!(set_shortcut(&mut config, "stack", "cmd+k").is_none());
    }
}
