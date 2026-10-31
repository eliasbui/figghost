//! What goes out and what comes back, as plain text and JSON, so it can be tested without a connection.

use serde_json::{Value, json};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    /// `POST /v1/messages` with `x-api-key`.
    Anthropic,
    /// `POST /v1/chat/completions` with a bearer key (none for a model on this machine).
    OpenAi,
    /// The Responses format of the backend Codex uses, with the sign-in of a ChatGPT plan.
    Codex,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Role {
    User,
    Assistant,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Message {
    pub role: Role,
    pub text: String,
}

impl Message {
    pub fn user(text: impl Into<String>) -> Self {
        Self { role: Role::User, text: text.into() }
    }

    pub fn assistant(text: impl Into<String>) -> Self {
        Self { role: Role::Assistant, text: text.into() }
    }
}

/// The most an answer may be: Jarvis answers briefly, and this bounds what a mistake can cost.
pub const MAX_TOKENS: u32 = 1024;

pub fn body(kind: Kind, model: &str, system: &str, messages: &[Message]) -> String {
    if kind == Kind::Codex {
        let input: Vec<Value> = messages
            .iter()
            .map(|m| {
                let (role, part) = match m.role {
                    Role::User => ("user", "input_text"),
                    Role::Assistant => ("assistant", "output_text"),
                };
                json!({ "type": "message", "role": role, "content": [{ "type": part, "text": m.text }] })
            })
            .collect();
        return json!({
            "model": model,
            "instructions": system,
            "input": input,
            "tools": [],
            "tool_choice": "auto",
            "parallel_tool_calls": false,
            "store": false,
            "stream": true,
        })
        .to_string();
    }
    let turns = |extra: Option<Value>| -> Vec<Value> {
        extra
            .into_iter()
            .chain(messages.iter().map(|m| {
                let role = match m.role {
                    Role::User => "user",
                    Role::Assistant => "assistant",
                };
                json!({ "role": role, "content": m.text })
            }))
            .collect()
    };
    match kind {
        Kind::Anthropic => json!({
            "model": model,
            "max_tokens": MAX_TOKENS,
            "stream": true,
            "system": system,
            "messages": turns(None),
        }),
        Kind::OpenAi => json!({
            "model": model,
            "max_tokens": MAX_TOKENS,
            "stream": true,
            "messages": turns(Some(json!({ "role": "system", "content": system }))),
        }),
        Kind::Codex => unreachable!("answered above"),
    }
    .to_string()
}

/// The headers a request needs, the key included. `account` is the ChatGPT account of a Codex sign-in, whose access
/// token goes in `key`.
pub fn headers(kind: Kind, key: &str, account: &str) -> Vec<(&'static str, String)> {
    let mut out = vec![("content-type", "application/json".to_owned())];
    match kind {
        Kind::Anthropic => {
            out.push(("x-api-key", key.to_owned()));
            out.push(("anthropic-version", "2023-06-01".to_owned()));
        }
        Kind::OpenAi if !key.is_empty() => out.push(("authorization", format!("Bearer {key}"))),
        Kind::OpenAi => {}
        Kind::Codex => {
            out.push(("authorization", format!("Bearer {key}")));
            out.push(("chatgpt-account-id", account.to_owned()));
            out.push(("accept", "text/event-stream".to_owned()));
            out.push(("openai-beta", "responses=experimental".to_owned()));
            out.push(("originator", "figghost".to_owned()));
        }
    }
    out
}

/// One line of the stream, read.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Event {
    Text(String),
    Done,
    Error(String),
    /// A comment, an event name, a keep-alive, or a part with no words in it.
    Nothing,
}

/// A line of server-sent events, or a plain JSON body such as the error a server answers instead of a stream.
pub fn read_line(kind: Kind, line: &str) -> Event {
    let line = line.trim();
    let data = match line.strip_prefix("data:") {
        Some(data) => data.trim(),
        None if line.starts_with('{') => line,
        None => return Event::Nothing,
    };
    if data == "[DONE]" {
        return Event::Done;
    }
    let Ok(value) = serde_json::from_str::<Value>(data) else {
        return Event::Nothing;
    };
    if let Some(error) = value.get("error") {
        let message = error
            .get("message")
            .and_then(Value::as_str)
            .or_else(|| error.as_str())
            .unwrap_or("the service answered with an error");
        return Event::Error(message.to_owned());
    }
    match kind {
        Kind::Codex => match value.get("type").and_then(Value::as_str) {
            Some("response.output_text.delta") => value
                .get("delta")
                .and_then(Value::as_str)
                .filter(|t| !t.is_empty())
                .map_or(Event::Nothing, |t| Event::Text(t.to_owned())),
            Some("response.completed") => Event::Done,
            Some("response.failed") => Event::Error(
                value
                    .pointer("/response/error/message")
                    .and_then(Value::as_str)
                    .unwrap_or("the service could not answer")
                    .to_owned(),
            ),
            Some("response.incomplete") => Event::Error("the answer was cut short".into()),
            Some("error") => Event::Error(
                value
                    .get("message")
                    .or_else(|| value.pointer("/error/message"))
                    .and_then(Value::as_str)
                    .unwrap_or("the service answered with an error")
                    .to_owned(),
            ),
            _ => Event::Nothing,
        },
        Kind::Anthropic => match value.get("type").and_then(Value::as_str) {
            Some("content_block_delta") => value
                .pointer("/delta/text")
                .and_then(Value::as_str)
                .filter(|t| !t.is_empty())
                .map_or(Event::Nothing, |t| Event::Text(t.to_owned())),
            Some("message_stop") => Event::Done,
            _ => Event::Nothing,
        },
        Kind::OpenAi => value
            .pointer("/choices/0/delta/content")
            .and_then(Value::as_str)
            .filter(|t| !t.is_empty())
            .map_or(Event::Nothing, |t| Event::Text(t.to_owned())),
    }
}

/// Why a service refused, from the plain JSON it answered with: its own `error.message`, `message` or `detail`.
pub fn refusal(body: &str) -> Option<String> {
    let value: Value = serde_json::from_str(body.trim()).ok()?;
    let error = value.get("error");
    error
        .and_then(|e| e.get("message").or(Some(e)))
        .and_then(Value::as_str)
        .or_else(|| value.get("message").and_then(Value::as_str))
        .or_else(|| value.get("detail").and_then(Value::as_str))
        .map(str::to_owned)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn anthropic_puts_the_system_apart_and_openai_first() {
        let turns = [Message::user("hi"), Message::assistant("hello"), Message::user("and?")];
        let a: Value = serde_json::from_str(&body(Kind::Anthropic, "m", "be brief", &turns)).unwrap();
        assert_eq!(a["system"], "be brief");
        assert_eq!(a["messages"].as_array().unwrap().len(), 3);
        assert_eq!(a["stream"], true);
        let o: Value = serde_json::from_str(&body(Kind::OpenAi, "m", "be brief", &turns)).unwrap();
        assert!(o.get("system").is_none());
        assert_eq!(o["messages"][0], json!({ "role": "system", "content": "be brief" }));
        assert_eq!(o["messages"][3]["content"], "and?");
    }

    #[test]
    fn the_key_goes_where_each_service_wants_it() {
        let a = headers(Kind::Anthropic, "k", "");
        assert!(a.contains(&("x-api-key", "k".to_owned())));
        assert!(a.iter().any(|(n, _)| *n == "anthropic-version"));
        assert!(headers(Kind::OpenAi, "k", "").contains(&("authorization", "Bearer k".to_owned())));
        assert!(!headers(Kind::OpenAi, "", "").iter().any(|(n, _)| *n == "authorization"));
    }

    #[test]
    fn anthropic_lines_are_read() {
        let k = Kind::Anthropic;
        assert_eq!(read_line(k, "event: content_block_delta"), Event::Nothing);
        assert_eq!(
            read_line(k, r#"data: {"type":"content_block_delta","delta":{"type":"text_delta","text":"Xin chào"}}"#),
            Event::Text("Xin chào".into())
        );
        assert_eq!(read_line(k, r#"data: {"type":"message_start"}"#), Event::Nothing);
        assert_eq!(read_line(k, r#"data: {"type":"message_stop"}"#), Event::Done);
        assert_eq!(
            read_line(k, r#"{"type":"error","error":{"type":"authentication_error","message":"invalid x-api-key"}}"#),
            Event::Error("invalid x-api-key".into())
        );
    }

    #[test]
    fn a_codex_request_is_in_the_responses_format_with_the_account_in_a_header() {
        let turns = [Message::user("hi"), Message::assistant("hello"), Message::user("and?")];
        let b: Value = serde_json::from_str(&body(Kind::Codex, "m", "be brief", &turns)).unwrap();
        assert_eq!(b["instructions"], "be brief");
        assert_eq!(b["store"], false);
        assert_eq!(b["stream"], true);
        assert_eq!(b["input"][0]["content"][0], json!({ "type": "input_text", "text": "hi" }));
        assert_eq!(b["input"][1]["content"][0]["type"], "output_text", "what Jarvis said is output");
        assert!(b.get("messages").is_none() && b.get("max_tokens").is_none());
        let h = headers(Kind::Codex, "tok", "acct-1");
        assert!(h.contains(&("authorization", "Bearer tok".to_owned())));
        assert!(h.contains(&("chatgpt-account-id", "acct-1".to_owned())));
        assert!(h.contains(&("originator", "figghost".to_owned())), "it says who it is");
    }

    #[test]
    fn codex_lines_are_read() {
        let k = Kind::Codex;
        assert_eq!(read_line(k, "event: response.output_text.delta"), Event::Nothing);
        assert_eq!(
            read_line(k, r#"data: {"type":"response.output_text.delta","delta":"Xin chào"}"#),
            Event::Text("Xin chào".into())
        );
        assert_eq!(read_line(k, r#"data: {"type":"response.created"}"#), Event::Nothing);
        assert_eq!(read_line(k, r#"data: {"type":"response.completed","response":{}}"#), Event::Done);
        assert_eq!(
            read_line(k, r#"data: {"type":"response.failed","response":{"error":{"message":"usage limit"}}}"#),
            Event::Error("usage limit".into())
        );
        assert_eq!(
            read_line(k, r#"{"detail":"Unauthorized"}"#),
            Event::Nothing,
            "a bare detail is the caller's to read from the status"
        );
    }

    #[test]
    fn a_plain_refusal_is_read_in_the_services_words() {
        assert_eq!(refusal(r#"{"detail":"Unauthorized"}"#).as_deref(), Some("Unauthorized"));
        assert_eq!(refusal(r#"{"error":{"message":"bad model"}}"#).as_deref(), Some("bad model"));
        assert_eq!(refusal(r#"{"message":"slow down"}"#).as_deref(), Some("slow down"));
        assert_eq!(refusal(""), None);
        assert_eq!(refusal("<html>"), None);
    }

    #[test]
    fn openai_lines_are_read() {
        let k = Kind::OpenAi;
        assert_eq!(
            read_line(k, r#"data: {"choices":[{"delta":{"content":"Hi"}}]}"#),
            Event::Text("Hi".into())
        );
        assert_eq!(read_line(k, r#"data: {"choices":[{"delta":{"role":"assistant"}}]}"#), Event::Nothing);
        assert_eq!(read_line(k, "data: [DONE]"), Event::Done);
        assert_eq!(read_line(k, ": keep-alive"), Event::Nothing);
        assert_eq!(read_line(k, r#"{"error":"model not found"}"#), Event::Error("model not found".into()));
    }
}
