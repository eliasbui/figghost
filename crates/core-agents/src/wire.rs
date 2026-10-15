//! The short conversation between a hook program and the app, one JSON line each way over a local socket.

use crate::event::HookEvent;
use serde::{Deserialize, Serialize};
use serde_json::json;

/// A request line longer than this is refused; the hook program has already reduced the event to a few hundred bytes.
pub const MAX_LINE: usize = 64 * 1024;

/// The answer to a permission request.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "decision", rename_all = "snake_case")]
pub enum Reply {
    Allow,
    Deny {
        message: String,
    },
    /// No answer: the agent asks the user itself, as if no hook existed.
    Pass,
}

pub fn encode_request(event: &HookEvent) -> String {
    let mut line = serde_json::to_string(event).expect("an event is always valid JSON");
    line.push('\n');
    line
}

pub fn decode_request(line: &str) -> Result<HookEvent, serde_json::Error> {
    serde_json::from_str(line.trim())
}

pub fn encode_reply(reply: &Reply) -> String {
    let mut line = serde_json::to_string(reply).expect("a reply is always valid JSON");
    line.push('\n');
    line
}

pub fn decode_reply(line: &str) -> Result<Reply, serde_json::Error> {
    serde_json::from_str(line.trim())
}

/// What the hook program prints for the agent, `None` meaning print nothing. Both Claude Code and Codex read
/// the same shape for a permission request.
pub fn hook_stdout(reply: &Reply) -> Option<String> {
    let decision = match reply {
        Reply::Allow => json!({ "behavior": "allow" }),
        Reply::Deny { message } => json!({ "behavior": "deny", "message": message }),
        Reply::Pass => return None,
    };
    Some(
        json!({ "hookSpecificOutput": { "hookEventName": "PermissionRequest", "decision": decision } })
            .to_string(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::event::{Agent, EventKind};
    use serde_json::Value;

    fn event() -> HookEvent {
        HookEvent {
            agent: Agent::Claude,
            kind: EventKind::PermissionRequest,
            session_id: "s".into(),
            cwd: "/w".into(),
            tool: Some("Bash".into()),
            detail: Some("ls".into()),
            message: None,
            at_ms: 5,
        }
    }

    #[test]
    fn a_request_survives_the_trip_as_one_line() {
        let line = encode_request(&event());
        assert_eq!(line.matches('\n').count(), 1);
        assert_eq!(decode_request(&line).unwrap(), event());
    }

    #[test]
    fn replies_survive_the_trip() {
        for reply in [
            Reply::Allow,
            Reply::Deny {
                message: "no".into(),
            },
            Reply::Pass,
        ] {
            assert_eq!(decode_reply(&encode_reply(&reply)).unwrap(), reply);
        }
        assert!(decode_reply("not json").is_err());
    }

    #[test]
    fn the_agent_gets_the_documented_decision_shape() {
        let allow: Value = serde_json::from_str(&hook_stdout(&Reply::Allow).unwrap()).unwrap();
        assert_eq!(
            allow["hookSpecificOutput"]["hookEventName"],
            "PermissionRequest"
        );
        assert_eq!(allow["hookSpecificOutput"]["decision"]["behavior"], "allow");
        let deny: Value = serde_json::from_str(
            &hook_stdout(&Reply::Deny {
                message: "Denied from the notch".into(),
            })
            .unwrap(),
        )
        .unwrap();
        assert_eq!(deny["hookSpecificOutput"]["decision"]["behavior"], "deny");
        assert_eq!(
            deny["hookSpecificOutput"]["decision"]["message"],
            "Denied from the notch"
        );
        assert_eq!(hook_stdout(&Reply::Pass), None);
    }
}
