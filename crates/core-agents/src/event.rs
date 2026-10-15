//! What an agent tells us through a hook, reduced to the few fields the notch shows.
//! The hook program does this reduction before anything leaves its process: a tool call can carry a whole file.

use serde::{Deserialize, Serialize};
use serde_json::Value;

const DETAIL_CHARS: usize = 600;
const MESSAGE_CHARS: usize = 300;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Agent {
    Claude,
    Codex,
}

impl Agent {
    pub const ALL: [Agent; 2] = [Agent::Claude, Agent::Codex];

    pub fn id(self) -> &'static str {
        match self {
            Agent::Claude => "claude",
            Agent::Codex => "codex",
        }
    }

    pub fn from_id(id: &str) -> Option<Agent> {
        Agent::ALL.into_iter().find(|agent| agent.id() == id)
    }

    pub fn name(self) -> &'static str {
        match self {
            Agent::Claude => "Claude Code",
            Agent::Codex => "Codex",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EventKind {
    SessionStart,
    PromptSubmit,
    /// A command or edit is about to ask the user; the hook may answer for them.
    PermissionRequest,
    Notification,
    /// A tool finished, which also means a pending question was answered, wherever it was answered.
    ToolDone,
    Stop,
    StopFailure,
    SessionEnd,
    Interrupt,
    Other,
}

impl EventKind {
    pub fn from_hook_name(name: &str) -> EventKind {
        match name {
            "SessionStart" => EventKind::SessionStart,
            "UserPromptSubmit" => EventKind::PromptSubmit,
            "PermissionRequest" => EventKind::PermissionRequest,
            "Notification" => EventKind::Notification,
            "PostToolUse" => EventKind::ToolDone,
            "Stop" => EventKind::Stop,
            "StopFailure" => EventKind::StopFailure,
            "SessionEnd" => EventKind::SessionEnd,
            "Interrupt" => EventKind::Interrupt,
            _ => EventKind::Other,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HookEvent {
    pub agent: Agent,
    pub kind: EventKind,
    pub session_id: String,
    pub cwd: String,
    /// The tool a permission request is about ("Bash", "Edit", ...); for a notification, its type.
    #[serde(default)]
    pub tool: Option<String>,
    /// The command, file or address the tool works on.
    #[serde(default)]
    pub detail: Option<String>,
    /// The text of a notification.
    #[serde(default)]
    pub message: Option<String>,
    pub at_ms: i64,
}

/// At most `max` characters, with "..." when something was cut.
pub fn clip(text: &str, max: usize) -> String {
    let text = text.trim();
    let mut out: String = text.chars().take(max).collect();
    if text.chars().nth(max).is_some() {
        out.push_str("...");
    }
    out
}

fn summarize_input(input: Option<&Value>) -> Option<String> {
    let input = input?;
    if let Some(text) = input.as_str() {
        return Some(clip(text, DETAIL_CHARS)).filter(|t| !t.is_empty());
    }
    let object = input.as_object()?;
    for key in [
        "command",
        "cmd",
        "file_path",
        "notebook_path",
        "path",
        "url",
        "pattern",
        "query",
        "description",
    ] {
        match object.get(key) {
            Some(Value::String(text)) if !text.trim().is_empty() => {
                return Some(clip(text, DETAIL_CHARS));
            }
            Some(Value::Array(words)) => {
                let words: Vec<&str> = words.iter().filter_map(Value::as_str).collect();
                if !words.is_empty() {
                    return Some(clip(&words.join(" "), DETAIL_CHARS));
                }
            }
            _ => {}
        }
    }
    Some(clip(&input.to_string(), DETAIL_CHARS))
}

/// Turns the JSON an agent writes to a hook's standard input into an event. `None` for anything that is not
/// a hook payload, so a surprise from a newer agent version never gets in the way.
pub fn parse_event(agent: Agent, raw: &Value, now_ms: i64) -> Option<HookEvent> {
    let name = raw.get("hook_event_name")?.as_str()?;
    let text = |key: &str| {
        raw.get(key)
            .and_then(Value::as_str)
            .filter(|t| !t.is_empty())
            .map(str::to_owned)
    };
    let kind = EventKind::from_hook_name(name);
    let tool = if kind == EventKind::Notification {
        text("notification_type")
    } else {
        text("tool_name")
    };
    Some(HookEvent {
        agent,
        kind,
        session_id: text("session_id").unwrap_or_else(|| "unknown".to_owned()),
        cwd: text("cwd").unwrap_or_default(),
        detail: if kind == EventKind::PermissionRequest {
            summarize_input(raw.get("tool_input"))
        } else {
            None
        },
        tool,
        message: if kind == EventKind::Notification {
            text("message").map(|m| clip(&m, MESSAGE_CHARS))
        } else {
            None
        },
        at_ms: now_ms,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn a_claude_permission_request_keeps_the_command_and_drops_the_rest() {
        let raw = json!({
            "session_id": "abc",
            "transcript_path": "/x/y.jsonl",
            "cwd": "/Users/me/app",
            "hook_event_name": "PermissionRequest",
            "tool_name": "Bash",
            "tool_input": { "command": "rm -rf build", "description": "Clean" }
        });
        let event = parse_event(Agent::Claude, &raw, 7).unwrap();
        assert_eq!(event.kind, EventKind::PermissionRequest);
        assert_eq!(event.tool.as_deref(), Some("Bash"));
        assert_eq!(event.detail.as_deref(), Some("rm -rf build"));
        assert_eq!(
            (event.session_id.as_str(), event.cwd.as_str(), event.at_ms),
            ("abc", "/Users/me/app", 7)
        );
    }

    #[test]
    fn a_command_given_as_words_is_joined() {
        let raw = json!({
            "session_id": "s", "cwd": "/w", "hook_event_name": "PermissionRequest",
            "tool_name": "shell", "tool_input": { "command": ["bash", "-lc", "ls -la"] }
        });
        let event = parse_event(Agent::Codex, &raw, 0).unwrap();
        assert_eq!(event.detail.as_deref(), Some("bash -lc ls -la"));
    }

    #[test]
    fn files_are_shown_by_path_never_by_content() {
        let raw = json!({
            "session_id": "s", "cwd": "/w", "hook_event_name": "PermissionRequest",
            "tool_name": "Write",
            "tool_input": { "file_path": "/w/a.txt", "content": "x".repeat(10_000) }
        });
        let event = parse_event(Agent::Claude, &raw, 0).unwrap();
        assert_eq!(event.detail.as_deref(), Some("/w/a.txt"));
        let notebook = json!({
            "session_id": "s", "cwd": "/w", "hook_event_name": "PermissionRequest",
            "tool_name": "NotebookEdit",
            "tool_input": { "notebook_path": "/w/n.ipynb", "new_source": "secret = 1" }
        });
        let event = parse_event(Agent::Claude, &notebook, 0).unwrap();
        assert_eq!(event.detail.as_deref(), Some("/w/n.ipynb"));
    }

    #[test]
    fn an_unknown_tool_input_is_summarised_and_bounded() {
        let raw = json!({
            "session_id": "s", "cwd": "/w", "hook_event_name": "PermissionRequest",
            "tool_name": "mcp__x", "tool_input": { "blob": "y".repeat(5_000) }
        });
        let detail = parse_event(Agent::Claude, &raw, 0).unwrap().detail.unwrap();
        assert!(detail.len() <= DETAIL_CHARS + 3 && detail.ends_with("..."));
    }

    #[test]
    fn only_notifications_keep_their_message_and_only_requests_keep_their_detail() {
        let note = json!({ "hook_event_name": "Notification", "session_id": "s", "cwd": "/w", "message": "Claude needs you" });
        assert_eq!(
            parse_event(Agent::Claude, &note, 0)
                .unwrap()
                .message
                .as_deref(),
            Some("Claude needs you")
        );
        let stop = json!({ "hook_event_name": "Stop", "session_id": "s", "cwd": "/w", "message": "secret summary", "tool_input": { "command": "ls" } });
        let event = parse_event(Agent::Claude, &stop, 0).unwrap();
        assert_eq!((event.message, event.detail), (None, None));
    }

    #[test]
    fn payloads_that_are_not_hooks_are_ignored() {
        assert!(parse_event(Agent::Claude, &json!({"hello": 1}), 0).is_none());
        assert!(parse_event(Agent::Claude, &json!("text"), 0).is_none());
        assert!(parse_event(Agent::Claude, &json!({"hook_event_name": 4}), 0).is_none());
    }

    #[test]
    fn an_unknown_event_name_is_kept_as_other() {
        let raw = json!({ "hook_event_name": "PostCompact", "session_id": "s", "cwd": "/w" });
        assert_eq!(
            parse_event(Agent::Codex, &raw, 0).unwrap().kind,
            EventKind::Other
        );
    }

    #[test]
    fn clip_counts_characters_not_bytes() {
        assert_eq!(clip("  abc  ", 10), "abc");
        assert_eq!(clip("đường dẫn", 4), "đườn...");
        assert_eq!(clip("abcd", 4), "abcd");
    }

    #[test]
    fn agent_ids_round_trip() {
        for agent in Agent::ALL {
            assert_eq!(Agent::from_id(agent.id()), Some(agent));
        }
        assert_eq!(Agent::from_id("gemini"), None);
    }
}
