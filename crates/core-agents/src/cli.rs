//! The two small programs the agents run: the hook, which tells the app what is going on and may carry back
//! an answer, and the status line tap, which keeps what Claude Code says about its usage limits.
//! Both must never get in the agent's way: whatever goes wrong, they end quietly and the agent carries on
//! as if they did not exist.

use crate::{
    event::{Agent, EventKind, parse_event},
    usage::{self, CLAUDE_USAGE_FILE},
    wire,
};
use serde_json::Value;
use std::{
    fs,
    io::{BufRead, BufReader, Read, Write},
    os::unix::net::UnixStream,
    path::Path,
    process::{Command, Stdio},
    time::Duration,
};

/// Hook input can carry a whole file; more than this is not worth reading.
const MAX_STDIN: u64 = 8 * 1024 * 1024;
/// The app answers within its own wait, a few seconds at most; this only guards against an app that hangs.
const HARD_WAIT: Duration = Duration::from_secs(600);
/// Every other event is acknowledged at once. The hook has to wait for that: on macOS a connection closed
/// before the app has accepted it can lose what was written, so leaving early would drop events.
const ACK_WAIT: Duration = Duration::from_millis(1500);
/// Where the install step leaves the status line command that was there before ours.
pub const CHAIN_FILE: &str = "statusline-chain";

/// Reads the hook's input, tells the app, and for a permission request prints its answer. Always returns 0.
pub fn run_hook(
    agent: Agent,
    stdin: impl Read,
    stdout: &mut impl Write,
    socket: &Path,
    now_ms: i64,
) -> i32 {
    let mut raw = Vec::new();
    if stdin.take(MAX_STDIN).read_to_end(&mut raw).is_err() {
        return 0;
    }
    let Ok(value) = serde_json::from_slice::<Value>(&raw) else {
        return 0;
    };
    let Some(event) = parse_event(agent, &value, now_ms) else {
        return 0;
    };
    let Ok(mut stream) = UnixStream::connect(socket) else {
        // The app is not running: the agent asks the user itself.
        return 0;
    };
    if stream
        .write_all(wire::encode_request(&event).as_bytes())
        .is_err()
    {
        return 0;
    }
    let asks = matches!(event.kind, EventKind::PermissionRequest | EventKind::Question);
    let question = event.kind == EventKind::Question;
    let wait = if asks { HARD_WAIT } else { ACK_WAIT };
    if stream.set_read_timeout(Some(wait)).is_err() {
        return 0;
    }
    let mut line = String::new();
    if BufReader::new(&stream).read_line(&mut line).is_err() {
        return 0;
    }
    if asks
        && let Ok(reply) = wire::decode_reply(&line)
        && let Some(output) = if question {
            value
                .get("tool_input")
                .and_then(|input| wire::question_stdout(&reply, input))
        } else {
            wire::hook_stdout(&reply)
        }
    {
        let _ = writeln!(stdout, "{output}");
    }
    0
}

fn run_chain(command: &str, input: &[u8], stdout: &mut impl Write) -> i32 {
    let Ok(mut child) = Command::new("/bin/sh")
        .arg("-c")
        .arg(command)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
    else {
        return 0;
    };
    if let Some(mut pipe) = child.stdin.take() {
        // The program may not read everything, or anything.
        let _ = pipe.write_all(input);
    }
    match child.wait_with_output() {
        Ok(output) => {
            let _ = stdout.write_all(&output.stdout);
            output.status.code().unwrap_or(0)
        }
        Err(_) => 0,
    }
}

/// Keeps the usage limits in Claude Code's status line input, then runs the status line that was there
/// before ours with the same input, so what the user sees does not change.
pub fn run_statusline(stdin: impl Read, stdout: &mut impl Write, home: &Path, now_ms: i64) -> i32 {
    let mut raw = Vec::new();
    let _ = stdin.take(MAX_STDIN).read_to_end(&mut raw);
    if let Ok(value) = serde_json::from_slice::<Value>(&raw)
        && let Some(limits) = usage::claude_limits_from_status(&value, now_ms)
    {
        let _ = usage::write_limits(&home.join(CLAUDE_USAGE_FILE), &limits);
    }
    let chain = fs::read_to_string(home.join(CHAIN_FILE)).unwrap_or_default();
    let chain = chain.trim();
    if chain.is_empty() {
        return 0;
    }
    run_chain(chain, &raw, stdout)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::server::{Handler, Server, Ticket};
    use crate::wire::Reply;
    use serde_json::json;
    use std::{
        path::PathBuf,
        sync::{Arc, Mutex},
        thread,
        time::Instant,
    };

    fn scratch(name: &str) -> PathBuf {
        let dir =
            std::env::temp_dir().join(format!("core-agents-cli-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn request_json() -> Vec<u8> {
        json!({
            "session_id": "abc", "cwd": "/Users/me/shop", "hook_event_name": "PermissionRequest",
            "tool_name": "Bash", "tool_input": { "command": "rm -rf build" }
        })
        .to_string()
        .into_bytes()
    }

    fn answering(reply: Reply) -> (Arc<Handler>, Arc<Mutex<Vec<String>>>) {
        let seen: Arc<Mutex<Vec<String>>> = Arc::default();
        let log = Arc::clone(&seen);
        let handler: Arc<Handler> = Arc::new(move |event, ticket: Option<Ticket>| {
            log.lock().unwrap().push(event.detail.unwrap_or_default());
            if let Some(ticket) = ticket {
                let reply = reply.clone();
                thread::spawn(move || {
                    ticket.answer(reply);
                });
            }
        });
        (handler, seen)
    }

    #[test]
    fn an_approval_in_the_app_becomes_the_agents_documented_answer() {
        let dir = scratch("allow");
        let socket = dir.join("a.sock");
        let (handler, seen) = answering(Reply::Allow);
        let server = Server::start(&socket, 5, handler).unwrap();
        let mut out = Vec::new();
        assert_eq!(
            run_hook(Agent::Claude, &request_json()[..], &mut out, &socket, 1),
            0
        );
        let printed: Value = serde_json::from_slice(&out).unwrap();
        assert_eq!(
            printed["hookSpecificOutput"]["hookEventName"],
            "PermissionRequest"
        );
        assert_eq!(
            printed["hookSpecificOutput"]["decision"]["behavior"],
            "allow"
        );
        assert_eq!(seen.lock().unwrap().as_slice(), ["rm -rf build"]);
        drop(server);
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_denial_is_printed_with_its_reason() {
        let dir = scratch("deny");
        let socket = dir.join("a.sock");
        let (handler, _) = answering(Reply::Deny {
            message: "Denied from the notch".into(),
        });
        let server = Server::start(&socket, 5, handler).unwrap();
        let mut out = Vec::new();
        run_hook(Agent::Codex, &request_json()[..], &mut out, &socket, 1);
        let printed: Value = serde_json::from_slice(&out).unwrap();
        assert_eq!(
            printed["hookSpecificOutput"]["decision"]["behavior"],
            "deny"
        );
        assert_eq!(
            printed["hookSpecificOutput"]["decision"]["message"],
            "Denied from the notch"
        );
        drop(server);
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn no_app_running_means_silence_not_an_error() {
        let dir = scratch("noapp");
        let mut out = Vec::new();
        assert_eq!(
            run_hook(
                Agent::Claude,
                &request_json()[..],
                &mut out,
                &dir.join("none.sock"),
                1
            ),
            0
        );
        assert!(out.is_empty());
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn input_that_is_not_a_hook_payload_is_ignored() {
        let dir = scratch("junk");
        let socket = dir.join("a.sock");
        let (handler, seen) = answering(Reply::Allow);
        let server = Server::start(&socket, 5, handler).unwrap();
        for junk in [&b"not json"[..], b"{}", b"[1,2]", b""] {
            let mut out = Vec::new();
            assert_eq!(run_hook(Agent::Claude, junk, &mut out, &socket, 1), 0);
            assert!(out.is_empty());
        }
        assert!(seen.lock().unwrap().is_empty());
        drop(server);
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn an_event_that_is_not_a_question_returns_without_waiting() {
        let dir = scratch("stop");
        let socket = dir.join("a.sock");
        let (handler, seen) = answering(Reply::Allow);
        let server = Server::start(&socket, 5, handler).unwrap();
        let stop = json!({ "session_id": "s", "cwd": "/w", "hook_event_name": "Stop" }).to_string();
        let started = Instant::now();
        let mut out = Vec::new();
        run_hook(Agent::Claude, stop.as_bytes(), &mut out, &socket, 1);
        assert!(started.elapsed() < Duration::from_secs(1));
        assert!(out.is_empty());
        let deadline = Instant::now() + Duration::from_secs(2);
        while seen.lock().unwrap().is_empty() && Instant::now() < deadline {
            thread::sleep(Duration::from_millis(10));
        }
        assert_eq!(seen.lock().unwrap().len(), 1);
        drop(server);
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_huge_tool_input_is_reduced_before_it_leaves_the_hook() {
        let dir = scratch("huge");
        let socket = dir.join("a.sock");
        let (handler, seen) = answering(Reply::Allow);
        let server = Server::start(&socket, 5, handler).unwrap();
        let payload = json!({
            "session_id": "s", "cwd": "/w", "hook_event_name": "PermissionRequest",
            "tool_name": "Bash", "tool_input": { "command": "y".repeat(2_000_000) }
        })
        .to_string();
        let mut out = Vec::new();
        run_hook(Agent::Claude, payload.as_bytes(), &mut out, &socket, 1);
        assert!(
            !out.is_empty(),
            "the request still got through and was answered"
        );
        let seen = seen.lock().unwrap();
        assert!(seen[0].len() <= 700, "{}", seen[0].len());
        drop(server);
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn the_status_line_tap_keeps_the_limits_and_passes_the_input_on() {
        let dir = scratch("status");
        let input = json!({
            "session_id": "s",
            "rate_limits": { "five_hour": { "used_percentage": 12, "resets_at": 1_792_000_000 } }
        })
        .to_string();
        fs::write(dir.join(CHAIN_FILE), "printf 'before:'; cat").unwrap();
        let mut out = Vec::new();
        assert_eq!(run_statusline(input.as_bytes(), &mut out, &dir, 77), 0);
        assert_eq!(String::from_utf8(out).unwrap(), format!("before:{input}"));
        let limits = usage::read_limits(&dir.join(CLAUDE_USAGE_FILE)).unwrap();
        assert_eq!(limits.primary.unwrap().used_percent, 12.0);
        assert_eq!(limits.observed_ms, 77);
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn without_a_previous_status_line_the_tap_prints_nothing_and_input_without_limits_is_fine() {
        let dir = scratch("nochain");
        let mut out = Vec::new();
        assert_eq!(
            run_statusline(&b"{\"session_id\":\"s\"}"[..], &mut out, &dir, 1),
            0
        );
        assert!(out.is_empty());
        assert!(!dir.join(CLAUDE_USAGE_FILE).exists());
        // Garbage must not stop the previous status line from running.
        fs::write(dir.join(CHAIN_FILE), "echo still works").unwrap();
        let mut out = Vec::new();
        run_statusline(&b"garbage"[..], &mut out, &dir, 1);
        assert_eq!(String::from_utf8(out).unwrap().trim(), "still works");
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn the_exit_code_of_the_previous_status_line_is_kept() {
        let dir = scratch("exit");
        fs::write(dir.join(CHAIN_FILE), "exit 3").unwrap();
        let mut out = Vec::new();
        assert_eq!(run_statusline(&b"{}"[..], &mut out, &dir, 1), 3);
        let _ = fs::remove_dir_all(&dir);
    }
}
