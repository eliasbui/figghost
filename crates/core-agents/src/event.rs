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
    /// Reported by the extension Figghost installs into pi, not by hooks: pi has no hook file and no usage limits.
    Pi,
    /// Antigravity's command line tool, `agy`, through the hooks of `~/.gemini/config/hooks.json`.
    Agy,
}

impl Agent {
    pub const ALL: [Agent; 4] = [Agent::Claude, Agent::Codex, Agent::Pi, Agent::Agy];

    pub fn id(self) -> &'static str {
        match self {
            Agent::Claude => "claude",
            Agent::Codex => "codex",
            Agent::Pi => "pi",
            Agent::Agy => "agy",
        }
    }

    pub fn from_id(id: &str) -> Option<Agent> {
        Agent::ALL.into_iter().find(|agent| agent.id() == id)
    }

    pub fn name(self) -> &'static str {
        match self {
            Agent::Claude => "Claude Code",
            Agent::Codex => "Codex",
            Agent::Pi => "Pi",
            Agent::Agy => "Antigravity",
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
    /// A question with choices that the agent put to the user (Claude Code's AskUserQuestion, pi's ask_user_question).
    Question,
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
    /// The questions of a `Question` event.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub questions: Vec<Question>,
    /// Where the session runs, when its hook knew.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub terminal: Option<Terminal>,
    pub at_ms: i64,
}

/// Where a session runs, as far as its hook can tell from its environment: enough to bring that terminal to the front.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Terminal {
    /// The handle Orca gave the terminal (`term_...`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub orca: Option<String>,
    /// `TERM_PROGRAM`: `Apple_Terminal`, `iTerm.app`, `ghostty`, `vscode`...
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub program: Option<String>,
}

impl Terminal {
    /// What a hook says of its terminal, from an environment: `None` when it says nothing usable.
    pub fn from_env(env: impl Fn(&str) -> Option<String>) -> Option<Terminal> {
        Terminal {
            orca: env("ORCA_TERMINAL_HANDLE"),
            program: env("TERM_PROGRAM"),
        }
        .sanitized()
    }

    /// Only what can be passed on safely is kept: a handle of Orca's shape, a program that is a plain word. The socket
    /// takes lines from anything of this user, and the handle ends up as an argument of a command.
    pub fn sanitized(self) -> Option<Terminal> {
        let orca = self.orca.filter(|h| {
            h.strip_prefix("term_")
                .is_some_and(|rest| !rest.is_empty() && rest.len() <= 64 && rest.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-'))
        });
        let program = self.program.filter(|p| {
            !p.is_empty() && p.len() <= 40 && p.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'.' || b == b'_' || b == b'-')
        });
        (orca.is_some() || program.is_some()).then_some(Terminal { orca, program })
    }

    /// The bundle identifier of the terminal program the person runs, when it is one that is known.
    pub fn bundle(&self) -> Option<&'static str> {
        Some(match self.program.as_deref()? {
            "Apple_Terminal" => "com.apple.Terminal",
            "iTerm.app" => "com.googlecode.iterm2",
            "ghostty" => "com.mitchellh.ghostty",
            "WezTerm" => "com.github.wez.wezterm",
            "vscode" => "com.microsoft.VSCode",
            "Hyper" => "co.zeit.hyper",
            "kitty" => "net.kovidgoyal.kitty",
            "Alacritty" => "org.alacritty",
            "WarpTerminal" => "dev.warp.Warp-Stable",
            _ => return None,
        })
    }

    /// The command that brings the terminal of Orca to the front: the arguments of `orca`, none of them from a shell.
    pub fn orca_switch_args(&self) -> Option<Vec<String>> {
        let handle = self.orca.as_ref()?;
        Some(vec!["terminal".into(), "switch".into(), "--terminal".into(), handle.clone()])
    }
}

/// One question of AskUserQuestion with the labels of its choices.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Question {
    pub header: String,
    pub question: String,
    pub options: Vec<String>,
    #[serde(default)]
    pub multi: bool,
}

const QUESTIONS_MAX: usize = 4;
const OPTIONS_MAX: usize = 4;

/// The questions of an AskUserQuestion input; empty when the input has none a person could answer.
fn parse_questions(input: Option<&Value>) -> Vec<Question> {
    let list = input
        .and_then(|i| i.get("questions"))
        .and_then(Value::as_array);
    list.into_iter()
        .flatten()
        .take(QUESTIONS_MAX)
        .filter_map(|q| {
            let text = |key: &str| q.get(key).and_then(Value::as_str).unwrap_or_default();
            let options: Vec<String> = q
                .get("options")?
                .as_array()?
                .iter()
                .take(OPTIONS_MAX)
                .filter_map(|o| o.get("label")?.as_str().map(|l| clip(l, 80)))
                .collect();
            (!text("question").is_empty() && !options.is_empty()).then(|| Question {
                header: clip(text("header"), 30),
                question: clip(text("question"), MESSAGE_CHARS),
                options,
                multi: q
                    .get("multiSelect")
                    .and_then(Value::as_bool)
                    .unwrap_or(false),
            })
        })
        .collect()
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

/// What `agy` writes to a hook, which does not name its event and spells its keys in camelCase, in the shape the
/// other agents use: `event` is the name its hook command was given. `None` for an event that is not followed.
pub fn agy_payload(event: &str, raw: &Value) -> Option<Value> {
    let name = match event {
        "SessionStart" => "SessionStart",
        // Before each call of the model: the agent is at work.
        "PreInvocation" => "UserPromptSubmit",
        "PostToolUse" => "PostToolUse",
        "Stop" => match raw.get("terminationReason").and_then(Value::as_str) {
            Some("ERROR" | "MAX_TOKEN_BUDGET_EXCEEDED") => "StopFailure",
            Some("USER_CANCELED") => "Interrupt",
            _ => "Stop",
        },
        _ => return None,
    };
    let cwd = raw
        .get("workspacePaths")
        .and_then(Value::as_array)
        .and_then(|paths| paths.first())
        .cloned()
        .unwrap_or(Value::Null);
    Some(serde_json::json!({
        "hook_event_name": name,
        "session_id": raw.get("conversationId").cloned().unwrap_or(Value::Null),
        "cwd": cwd,
        "tool_name": raw.pointer("/toolCall/name").cloned().unwrap_or(Value::Null),
    }))
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
    let mut kind = EventKind::from_hook_name(name);
    let mut questions = Vec::new();
    // The question has its own card; the permission prompt that may follow it is the terminal's to ask.
    if raw.get("tool_name").and_then(Value::as_str) == Some("AskUserQuestion") {
        kind = match (name, kind) {
            ("PreToolUse", _) => {
                questions = parse_questions(raw.get("tool_input"));
                if questions.is_empty() {
                    EventKind::Other
                } else {
                    EventKind::Question
                }
            }
            (_, EventKind::PermissionRequest) => EventKind::Other,
            (_, kind) => kind,
        };
    }
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
            questions.first().map(|q| q.question.clone())
        },
        questions,
        tool,
        message: if kind == EventKind::Notification {
            text("message").map(|m| clip(&m, MESSAGE_CHARS))
        } else {
            None
        },
        terminal: None,
        at_ms: now_ms,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn a_terminal_is_read_from_the_environment_and_only_safe_things_are_kept() {
        let env = |pairs: &'static [(&'static str, &'static str)]| {
            move |key: &str| pairs.iter().find(|(k, _)| *k == key).map(|(_, v)| (*v).to_owned())
        };
        let t = Terminal::from_env(env(&[("ORCA_TERMINAL_HANDLE", "term_15e6ccf3-2e42-453a-b15e-6c8a28b26987"), ("TERM_PROGRAM", "Orca")])).unwrap();
        assert_eq!(t.orca.as_deref(), Some("term_15e6ccf3-2e42-453a-b15e-6c8a28b26987"));
        assert_eq!(
            t.orca_switch_args().unwrap(),
            ["terminal", "switch", "--terminal", "term_15e6ccf3-2e42-453a-b15e-6c8a28b26987"]
        );
        assert_eq!(t.bundle(), None, "Orca is not one of the terminal programs known by name");
        assert!(Terminal::from_env(env(&[])).is_none(), "an environment that says nothing");
        for bad in ["--help", "term_", "term_a b", "term_;rm", "TERM_x", "term_\u{7}", "x"] {
            let t = Terminal { orca: Some(bad.to_owned()), program: None };
            assert!(t.sanitized().is_none(), "{bad:?}");
        }
        let long = Terminal { orca: Some(format!("term_{}", "a".repeat(65))), program: Some("p".repeat(41)) };
        assert!(long.sanitized().is_none());
        let kept = Terminal { orca: Some("--x".into()), program: Some("iTerm.app".into()) }.sanitized().unwrap();
        assert_eq!((kept.orca.clone(), kept.bundle()), (None, Some("com.googlecode.iterm2")));
        assert!(Terminal { orca: Some("term_a".into()), program: None }.bundle().is_none());
    }

    #[test]
    fn an_event_without_a_terminal_is_what_old_hooks_send_and_still_reads() {
        let old = r#"{"agent":"claude","kind":"stop","session_id":"s","cwd":"/w","at_ms":1}"#;
        let event: HookEvent = serde_json::from_str(old).unwrap();
        assert!(event.terminal.is_none());
        assert!(!serde_json::to_string(&event).unwrap().contains("terminal"));
        let with = r#"{"agent":"pi","kind":"stop","session_id":"s","cwd":"/w","at_ms":1,"terminal":{"orca":"term_a1","program":"ghostty"}}"#;
        let event: HookEvent = serde_json::from_str(with).unwrap();
        assert_eq!(event.terminal.unwrap().orca.as_deref(), Some("term_a1"));
    }

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
    fn ask_user_question_becomes_a_question_and_its_permission_prompt_is_left_alone() {
        let input = json!({ "questions": [
            { "question": "Which?", "header": "Pick", "multiSelect": true,
              "options": [ { "label": "A", "description": "x" }, { "label": "B", "description": "y" } ] }
        ] });
        let raw = json!({ "hook_event_name": "PreToolUse", "session_id": "s", "cwd": "/w",
            "tool_name": "AskUserQuestion", "tool_input": input });
        let event = parse_event(Agent::Claude, &raw, 0).unwrap();
        assert_eq!(event.kind, EventKind::Question);
        assert_eq!(event.questions[0].options, ["A", "B"]);
        assert!(event.questions[0].multi);
        let prompt = json!({ "hook_event_name": "PermissionRequest", "session_id": "s", "cwd": "/w",
            "tool_name": "AskUserQuestion", "tool_input": input });
        assert_eq!(
            parse_event(Agent::Claude, &prompt, 0).unwrap().kind,
            EventKind::Other
        );
        let empty = json!({ "hook_event_name": "PreToolUse", "tool_name": "AskUserQuestion", "tool_input": {} });
        assert_eq!(
            parse_event(Agent::Claude, &empty, 0).unwrap().kind,
            EventKind::Other
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
    fn agy_events_take_the_shape_of_the_others() {
        let raw = json!({ "conversationId": "c1", "workspacePaths": ["/w", "/x"],
            "toolCall": { "name": "run_command", "args": { "CommandLine": "npm test" } }, "stepIdx": 3 });
        let event = parse_event(Agent::Agy, &agy_payload("PostToolUse", &raw).unwrap(), 5).unwrap();
        assert_eq!(event.kind, EventKind::ToolDone);
        assert_eq!(
            (event.session_id.as_str(), event.cwd.as_str()),
            ("c1", "/w")
        );
        assert_eq!(
            event.detail, None,
            "a command is never carried out of the hook"
        );
        let working = agy_payload("PreInvocation", &json!({ "conversationId": "c1" })).unwrap();
        assert_eq!(
            parse_event(Agent::Agy, &working, 0).unwrap().kind,
            EventKind::PromptSubmit
        );
        for (reason, kind) in [
            ("NO_TOOL_CALL", EventKind::Stop),
            ("ERROR", EventKind::StopFailure),
            ("USER_CANCELED", EventKind::Interrupt),
        ] {
            let stop = agy_payload(
                "Stop",
                &json!({ "conversationId": "c1", "terminationReason": reason }),
            )
            .unwrap();
            assert_eq!(
                parse_event(Agent::Agy, &stop, 0).unwrap().kind,
                kind,
                "{reason}"
            );
        }
        assert!(
            agy_payload("PreToolUse", &raw).is_none(),
            "tool calls are not gated"
        );
    }

    #[test]
    fn agent_ids_round_trip() {
        for agent in Agent::ALL {
            assert_eq!(Agent::from_id(agent.id()), Some(agent));
        }
        assert_eq!(Agent::from_id("gemini"), None);
    }
}
