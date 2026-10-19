//! How much of the usage limits is left. Codex records it in its session logs; Claude Code hands it to the
//! status line command, which is where `cli::run_statusline` picks it up.

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::{
    fs,
    io::{self, BufRead, BufReader, Read, Seek, SeekFrom, Write},
    path::{Path, PathBuf},
    process::{Child, Command, Stdio},
    sync::mpsc,
    thread,
    time::{Duration, Instant},
};

/// Where the status line program leaves what Claude Code told it, inside the app's data directory.
pub const CLAUDE_USAGE_FILE: &str = "claude-usage.json";

const TAIL_BYTES: u64 = 512 * 1024;
const FILES_TO_TRY: usize = 4;
/// The Codex app server answers in about a second; this is how long a silent one is waited for.
pub const LIVE_TIMEOUT: Duration = Duration::from_secs(15);

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Limit {
    /// 0 to 100.
    pub used_percent: f64,
    /// How long the window is, when known.
    pub window_minutes: Option<u64>,
    /// When the window starts over, in seconds since 1970.
    pub resets_at: Option<i64>,
}

impl Limit {
    /// Seconds until the window starts over; negative once it has.
    pub fn resets_in(&self, now_s: i64) -> Option<i64> {
        self.resets_at.map(|at| at - now_s)
    }

    /// Whether the window has started over since this was measured, which makes the percentage stale.
    pub fn has_reset(&self, now_s: i64) -> bool {
        self.resets_in(now_s).is_some_and(|left| left <= 0)
    }
}

#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct Limits {
    /// The short window: five hours for Claude Code, whatever Codex reports first.
    pub primary: Option<Limit>,
    /// The long one, a week for Claude Code.
    pub secondary: Option<Limit>,
    /// When the numbers were read, in milliseconds since 1970.
    pub observed_ms: i64,
    #[serde(default)]
    pub plan: Option<String>,
}

fn days_from_civil(year: i64, month: i64, day: i64) -> i64 {
    let y = if month <= 2 { year - 1 } else { year };
    let era = if y >= 0 { y } else { y - 399 } / 400;
    let yoe = y - era * 400;
    let doy = (153 * (if month > 2 { month - 3 } else { month + 9 }) + 2) / 5 + day - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

/// "2026-10-09T03:15:49.500Z" or with a numeric offset, as milliseconds since 1970.
pub fn parse_rfc3339_ms(text: &str) -> Option<i64> {
    // The numbers are read into narrow types so that a corrupt date is refused instead of overflowing the sums
    // below, which these functions meet in files that anyone can edit.
    let number = |field: &str| field.parse::<u16>().ok().map(i64::from);
    let (date, rest) = text.split_once('T')?;
    let mut parts = date.split('-');
    let year = number(parts.next()?)?;
    let month = number(parts.next()?)?;
    let day = number(parts.next()?)?;
    let (time, offset_s) = if let Some(time) = rest.strip_suffix('Z') {
        (time, 0)
    } else {
        let at = rest.rfind(['+', '-'])?;
        let (time, offset) = rest.split_at(at);
        let sign = if offset.starts_with('-') { -1 } else { 1 };
        let (hours, minutes) = offset[1..].split_once(':')?;
        let seconds = number(hours)? * 3600 + number(minutes)? * 60;
        (time, sign * seconds)
    };
    let (clock, fraction) = time.split_once('.').unwrap_or((time, ""));
    let mut clock = clock.split(':');
    let hour = number(clock.next()?)?;
    let minute = number(clock.next()?)?;
    let second = number(clock.next()?)?;
    let digits: String = fraction.chars().take(3).collect();
    let millis = if digits.is_empty() {
        0
    } else {
        digits.parse::<i64>().ok()? * 10_i64.pow(3 - digits.len() as u32)
    };
    let seconds = days_from_civil(year, month, day) * 86_400 + hour * 3600 + minute * 60 + second;
    Some((seconds - offset_s) * 1000 + millis)
}

fn codex_limit(value: &Value, observed_s: i64) -> Option<Limit> {
    let object = value.as_object()?;
    let used = object.get("used_percent")?.as_f64()?;
    let resets_at = object.get("resets_at").and_then(Value::as_i64).or_else(|| {
        object
            .get("resets_in_seconds")
            .and_then(Value::as_i64)
            .map(|left| observed_s + left)
    });
    Some(Limit {
        used_percent: used.clamp(0.0, 100.0),
        window_minutes: object.get("window_minutes").and_then(Value::as_u64),
        resets_at,
    })
}

/// The limits in one line of a Codex session log, if it is a token count that carries them.
pub fn codex_limits_from_line(line: &str) -> Option<Limits> {
    if !line.contains("rate_limits") {
        return None;
    }
    let value: Value = serde_json::from_str(line).ok()?;
    if value.get("type")?.as_str()? != "event_msg" {
        return None;
    }
    let payload = value.get("payload")?;
    if payload.get("type")?.as_str()? != "token_count" {
        return None;
    }
    let limits = payload.get("rate_limits")?;
    let observed_ms = value
        .get("timestamp")
        .and_then(Value::as_str)
        .and_then(parse_rfc3339_ms)
        .unwrap_or(0);
    let primary = limits
        .get("primary")
        .and_then(|v| codex_limit(v, observed_ms / 1000));
    let secondary = limits
        .get("secondary")
        .and_then(|v| codex_limit(v, observed_ms / 1000));
    if primary.is_none() && secondary.is_none() {
        return None;
    }
    Some(Limits {
        primary,
        secondary,
        observed_ms,
        plan: limits
            .get("plan_type")
            .and_then(Value::as_str)
            .map(str::to_owned),
    })
}

/// The newest session logs, newest first. Folders are named year/month/day and files start with their
/// time, so sorting names is sorting by age and nothing has to be opened to find the recent ones.
pub(crate) fn newest_rollouts(dir: &Path, depth: usize, want: usize, found: &mut Vec<PathBuf>) {
    let Ok(read) = fs::read_dir(dir) else {
        return;
    };
    let mut entries: Vec<_> = read.flatten().collect();
    entries.sort_by_key(|e| std::cmp::Reverse(e.file_name()));
    for entry in entries {
        if found.len() >= want {
            return;
        }
        let path = entry.path();
        if depth < 3 {
            if path.is_dir() {
                newest_rollouts(&path, depth + 1, want, found);
            }
        } else {
            let name = entry.file_name();
            let name = name.to_string_lossy();
            if name.starts_with("rollout-") && name.ends_with(".jsonl") {
                found.push(path);
            }
        }
    }
}

fn tail_lines(path: &Path) -> io::Result<Vec<String>> {
    let mut file = fs::File::open(path)?;
    let len = file.metadata()?.len();
    let start = len.saturating_sub(TAIL_BYTES);
    file.seek(SeekFrom::Start(start))?;
    let mut bytes = Vec::new();
    file.read_to_end(&mut bytes)?;
    let text = String::from_utf8_lossy(&bytes);
    let mut lines: Vec<String> = text.lines().map(str::to_owned).collect();
    if start > 0 && !lines.is_empty() {
        // The cut usually lands inside a line.
        lines.remove(0);
    }
    Ok(lines)
}

/// The most recent limits Codex wrote down, looking only at the end of the newest logs.
pub fn codex_limits(sessions_dir: &Path) -> Option<Limits> {
    let mut files = Vec::new();
    newest_rollouts(sessions_dir, 0, FILES_TO_TRY, &mut files);
    for file in files {
        let Ok(lines) = tail_lines(&file) else {
            continue;
        };
        if let Some(limits) = lines.iter().rev().find_map(|l| codex_limits_from_line(l)) {
            return Some(limits);
        }
    }
    None
}

fn claude_window(root: &Value, key: &str, minutes: u64) -> Option<Limit> {
    let object = root.get(key)?;
    let used = object.get("used_percentage")?.as_f64()?;
    let resets_at = match object.get("resets_at") {
        Some(Value::Number(n)) => n.as_f64().map(|s| s as i64),
        Some(Value::String(text)) => parse_rfc3339_ms(text).map(|ms| ms / 1000),
        _ => None,
    };
    Some(Limit {
        used_percent: used.clamp(0.0, 100.0),
        window_minutes: Some(minutes),
        resets_at,
    })
}

/// The limits in the JSON Claude Code feeds its status line, which carries them for subscribers.
pub fn claude_limits_from_status(status: &Value, now_ms: i64) -> Option<Limits> {
    let rates = status.get("rate_limits")?;
    let primary = claude_window(rates, "five_hour", 300);
    let secondary = claude_window(rates, "seven_day", 10_080);
    if primary.is_none() && secondary.is_none() {
        return None;
    }
    Some(Limits {
        primary,
        secondary,
        observed_ms: now_ms,
        plan: None,
    })
}

pub fn write_limits(path: &Path, limits: &Limits) -> io::Result<()> {
    let text = serde_json::to_string(limits).map_err(io::Error::other)?;
    let mut name = path.as_os_str().to_owned();
    name.push(format!(".{}.tmp", std::process::id()));
    let temp = PathBuf::from(name);
    fs::write(&temp, text)?;
    fs::rename(&temp, path).inspect_err(|_| {
        let _ = fs::remove_file(&temp);
    })
}

pub fn read_limits(path: &Path) -> Option<Limits> {
    serde_json::from_str(&fs::read_to_string(path).ok()?).ok()
}

/// The limits in the answer of the Codex app server to `account/rateLimits/read`, which are true as of `now_ms`.
pub fn codex_limits_from_rpc(result: &Value, now_ms: i64) -> Option<Limits> {
    let snapshot = result.get("rateLimits")?;
    let window = |key: &str| {
        let object = snapshot.get(key)?.as_object()?;
        Some(Limit {
            used_percent: object.get("usedPercent")?.as_f64()?.clamp(0.0, 100.0),
            window_minutes: object.get("windowDurationMins").and_then(Value::as_u64),
            resets_at: object.get("resetsAt").and_then(Value::as_i64),
        })
    };
    let (primary, secondary) = (window("primary"), window("secondary"));
    if primary.is_none() && secondary.is_none() {
        return None;
    }
    Some(Limits {
        primary,
        secondary,
        observed_ms: now_ms,
        plan: snapshot
            .get("planType")
            .and_then(Value::as_str)
            .map(str::to_owned),
    })
}

/// Where the Codex program is. An app started from the Dock does not get a terminal's PATH, so the places it is
/// usually installed in are tried.
// shortcut: fixed places, add a setting for the path if someone installs it elsewhere.
pub fn find_codex(user_home: &Path) -> Option<PathBuf> {
    // The user's own folders first, so a home given on purpose (the private instance of a test) is the one used.
    [
        user_home.join(".local/bin/codex"),
        user_home.join(".bun/bin/codex"),
        PathBuf::from("/opt/homebrew/bin/codex"),
        PathBuf::from("/usr/local/bin/codex"),
    ]
    .into_iter()
    .find(|path| path.is_file())
}

/// Asks the Codex app server (JSON-RPC lines over stdio) for the limits as it has them now, which is newer than
/// anything in the session logs. Codex reads its own login for this, so nothing secret passes through here. It
/// runs with the given home, which is what keeps a private instance away from the real account. The server is
/// stopped afterwards, whatever happened.
pub fn codex_limits_live(
    codex: &Path,
    user_home: &Path,
    now_ms: i64,
    timeout: Duration,
) -> Result<Limits, String> {
    talk(&mut server_command(codex, user_home), now_ms, timeout)
}

fn server_command(codex: &Path, user_home: &Path) -> Command {
    let dir = codex.parent().unwrap_or(Path::new(""));
    let mut command = Command::new(codex);
    command
        .arg("app-server")
        .env("HOME", user_home)
        .env("CODEX_HOME", user_home.join(".codex"))
        // A program installed through npm starts with `env node`.
        .env(
            "PATH",
            format!(
                "{}:/opt/homebrew/bin:/usr/local/bin:/usr/bin:/bin",
                dir.display()
            ),
        );
    command
}

fn talk(command: &mut Command, now_ms: i64, timeout: Duration) -> Result<Limits, String> {
    let mut child = command
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|e| e.to_string())?;
    let answer = ask_for_limits(&mut child, now_ms, timeout);
    let _ = child.kill();
    let _ = child.wait();
    answer
}

fn ask_for_limits(child: &mut Child, now_ms: i64, timeout: Duration) -> Result<Limits, String> {
    let mut stdin = child.stdin.take().ok_or("no input pipe")?;
    let stdout = child.stdout.take().ok_or("no output pipe")?;
    let (lines, answers) = mpsc::channel();
    // The reader ends when the server is killed and its pipe closes.
    thread::spawn(move || {
        for line in BufReader::new(stdout).lines().map_while(Result::ok) {
            if lines.send(line).is_err() {
                break;
            }
        }
    });
    let mut send = |text: &str| {
        writeln!(stdin, "{text}")
            .and_then(|()| stdin.flush())
            .map_err(|e| e.to_string())
    };
    let hello = json!({
        "id": 1, "method": "initialize",
        "params": { "clientInfo": { "name": "figghost", "title": "Figghost", "version": env!("CARGO_PKG_VERSION") } }
    });
    send(&hello.to_string())?;
    let deadline = Instant::now() + timeout;
    loop {
        let left = deadline.saturating_duration_since(Instant::now());
        let line = answers.recv_timeout(left).map_err(|_| "no answer")?;
        // Anything that is not an answer to one of our two requests (notifications, stray output) is skipped.
        let Ok(message) = serde_json::from_str::<Value>(&line) else {
            continue;
        };
        if message.get("method").is_some() {
            continue;
        }
        let id = message.get("id").and_then(Value::as_i64);
        if !matches!(id, Some(1 | 2)) {
            continue;
        }
        if let Some(error) = message.get("error") {
            let text = error.get("message").and_then(Value::as_str);
            return Err(text.unwrap_or("the server refused").to_owned());
        }
        if id == Some(1) {
            // The handshake is over: say so, then ask.
            send(r#"{"method":"initialized"}"#)?;
            send(r#"{"id":2,"method":"account/rateLimits/read"}"#)?;
            continue;
        }
        return message
            .get("result")
            .and_then(|result| codex_limits_from_rpc(result, now_ms))
            .ok_or_else(|| "the answer holds no limits".to_owned());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A line shaped like the ones Codex writes (checked against a real log).
    fn token_line(timestamp: &str, primary: f64, secondary: Option<f64>) -> String {
        json!({
            "timestamp": timestamp, "ordinal": 9, "type": "event_msg",
            "payload": {
                "type": "token_count",
                "info": { "total_token_usage": {} },
                "rate_limits": {
                    "limit_id": "codex",
                    "primary": { "used_percent": primary, "window_minutes": 300, "resets_at": 1_792_739_920 },
                    "secondary": secondary.map(|p| json!({ "used_percent": p, "window_minutes": 10_080, "resets_in_seconds": 3600 })),
                    "plan_type": "plus"
                }
            }
        })
        .to_string()
    }

    #[test]
    fn dates_with_and_without_offsets_parse_to_the_same_instant() {
        assert_eq!(
            parse_rfc3339_ms("2026-10-09T03:15:49Z"),
            Some(1_791_515_749_000)
        );
        assert_eq!(
            parse_rfc3339_ms("2026-10-09T03:15:49.500Z"),
            Some(1_791_515_749_500)
        );
        assert_eq!(
            parse_rfc3339_ms("2026-10-09T10:15:49+07:00"),
            Some(1_791_515_749_000)
        );
        assert_eq!(
            parse_rfc3339_ms("2026-10-08T20:15:49-07:00"),
            Some(1_791_515_749_000)
        );
        assert_eq!(
            parse_rfc3339_ms("2000-03-01T00:00:00Z"),
            Some(951_868_800_000)
        );
        assert_eq!(
            parse_rfc3339_ms("2024-02-29T23:59:59Z"),
            Some(1_709_251_199_000)
        );
        assert_eq!(parse_rfc3339_ms("1970-01-01T00:00:01.5Z"), Some(1_500));
        assert_eq!(parse_rfc3339_ms("not a date"), None);
    }

    #[test]
    fn numbers_too_big_to_be_a_date_are_refused_instead_of_overflowing() {
        assert_eq!(parse_rfc3339_ms("99999999999999-01-01T00:00:00Z"), None);
        assert_eq!(parse_rfc3339_ms("2026-10-15T99999999999:00:00Z"), None);
        assert_eq!(
            parse_rfc3339_ms("2026-10-15T07:00:00+99999999999999:00"),
            None
        );
    }

    #[test]
    fn a_codex_line_gives_both_windows_with_their_reset_times() {
        let line = token_line("2026-10-09T03:15:49.500Z", 54.0, Some(6.0));
        let limits = codex_limits_from_line(&line).unwrap();
        let primary = limits.primary.unwrap();
        assert_eq!(
            (
                primary.used_percent,
                primary.window_minutes,
                primary.resets_at
            ),
            (54.0, Some(300), Some(1_792_739_920))
        );
        let secondary = limits.secondary.unwrap();
        // A relative reset time is anchored at the moment of the measurement.
        assert_eq!(secondary.resets_at, Some(1_791_515_749 + 3600));
        assert_eq!(limits.plan.as_deref(), Some("plus"));
        assert_eq!(limits.observed_ms, 1_791_515_749_500);
    }

    #[test]
    fn a_missing_long_window_is_fine_and_other_lines_are_skipped() {
        let limits =
            codex_limits_from_line(&token_line("2026-10-09T03:15:49Z", 10.0, None)).unwrap();
        assert!(limits.secondary.is_none());
        assert!(
            codex_limits_from_line(r#"{"type":"event_msg","payload":{"type":"agent_message"}}"#)
                .is_none()
        );
        assert!(codex_limits_from_line("garbage rate_limits").is_none());
        let empty = json!({"type":"event_msg","payload":{"type":"token_count","rate_limits":{"primary":null,"secondary":null}}});
        assert!(codex_limits_from_line(&empty.to_string()).is_none());
    }

    #[test]
    fn percentages_are_kept_inside_0_to_100() {
        let limits =
            codex_limits_from_line(&token_line("2026-10-09T03:15:49Z", 140.0, Some(-3.0))).unwrap();
        assert_eq!(limits.primary.unwrap().used_percent, 100.0);
        assert_eq!(limits.secondary.unwrap().used_percent, 0.0);
    }

    #[test]
    fn the_newest_log_with_limits_wins_and_old_lines_inside_it_lose() {
        let root = std::env::temp_dir().join(format!("core-agents-usage-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        let older = root.join("2026/10/08");
        let newer = root.join("2026/10/09");
        fs::create_dir_all(&older).unwrap();
        fs::create_dir_all(&newer).unwrap();
        fs::write(
            older.join("rollout-2026-10-08T09-00-00-aaa.jsonl"),
            token_line("2026-10-08T09:00:00Z", 11.0, None),
        )
        .unwrap();
        let newer_lines = [
            token_line("2026-10-09T09:00:00Z", 20.0, None),
            r#"{"type":"event_msg","payload":{"type":"agent_message"}}"#.to_owned(),
            token_line("2026-10-09T09:30:00Z", 25.0, None),
            r#"{"type":"event_msg","payload":{"type":"agent_message"}}"#.to_owned(),
        ];
        fs::write(
            newer.join("rollout-2026-10-09T08-00-00-bbb.jsonl"),
            newer_lines.join("\n"),
        )
        .unwrap();
        // A newer log that has no token count yet must not hide the older one's numbers.
        fs::write(
            newer.join("rollout-2026-10-09T09-45-00-ccc.jsonl"),
            r#"{"type":"session_meta"}"#,
        )
        .unwrap();
        let limits = codex_limits(&root).unwrap();
        assert_eq!(limits.primary.unwrap().used_percent, 25.0);
        assert!(codex_limits(&root.join("missing")).is_none());
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn only_the_end_of_a_huge_log_is_read() {
        let root =
            std::env::temp_dir().join(format!("core-agents-usage-big-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        let day = root.join("2026/10/09");
        fs::create_dir_all(&day).unwrap();
        let mut text = token_line("2026-10-09T01:00:00Z", 1.0, None);
        text.push('\n');
        // More than the tail we read, so the first line is out of reach.
        text.push_str(&format!("{}\n", "x".repeat(TAIL_BYTES as usize + 10)));
        text.push_str(&token_line("2026-10-09T02:00:00Z", 2.0, None));
        fs::write(day.join("rollout-2026-10-09T00-00-00-big.jsonl"), text).unwrap();
        assert_eq!(
            codex_limits(&root).unwrap().primary.unwrap().used_percent,
            2.0
        );
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn claude_status_input_gives_the_two_windows() {
        let status = json!({
            "session_id": "s",
            "rate_limits": {
                "five_hour": { "used_percentage": 23.5, "resets_at": 1_792_000_000 },
                "seven_day": { "used_percentage": 41, "resets_at": 1_792_500_000 }
            }
        });
        let limits = claude_limits_from_status(&status, 99).unwrap();
        let five = limits.primary.unwrap();
        assert_eq!(
            (five.used_percent, five.window_minutes, five.resets_at),
            (23.5, Some(300), Some(1_792_000_000))
        );
        assert_eq!(limits.secondary.unwrap().window_minutes, Some(10_080));
        assert_eq!(limits.observed_ms, 99);
        assert!(claude_limits_from_status(&json!({"session_id": "s"}), 0).is_none());
        assert!(claude_limits_from_status(&json!({"rate_limits": {}}), 0).is_none());
    }

    #[test]
    fn a_window_that_has_started_over_is_marked_stale() {
        let limit = Limit {
            used_percent: 90.0,
            window_minutes: Some(300),
            resets_at: Some(1_000),
        };
        assert!(!limit.has_reset(999));
        assert!(limit.has_reset(1_000));
        assert_eq!(limit.resets_in(400), Some(600));
        assert_eq!(
            Limit {
                resets_at: None,
                ..limit
            }
            .resets_in(5),
            None
        );
    }

    #[test]
    fn saved_limits_read_back_the_same() {
        let dir =
            std::env::temp_dir().join(format!("core-agents-usage-file-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        let path = dir.join(CLAUDE_USAGE_FILE);
        let limits = claude_limits_from_status(
            &json!({"rate_limits": {"five_hour": {"used_percentage": 5, "resets_at": 7}}}),
            42,
        )
        .unwrap();
        write_limits(&path, &limits).unwrap();
        assert_eq!(read_limits(&path), Some(limits));
        assert_eq!(read_limits(&dir.join("nope")), None);
        let _ = fs::remove_dir_all(&dir);
    }

    /// The shape of a real answer to `account/rateLimits/read` (checked against a live server).
    fn rpc_result(primary: f64, secondary: Option<f64>) -> Value {
        json!({
            "rateLimits": {
                "limitId": "codex",
                "primary": { "usedPercent": primary, "windowDurationMins": 300, "resetsAt": 1_792_739_920 },
                "secondary": secondary.map(|p| json!({ "usedPercent": p, "windowDurationMins": 10_080, "resetsAt": 1_793_000_000 })),
                "planType": "plus"
            }
        })
    }

    #[test]
    fn a_server_answer_gives_both_windows_as_of_the_time_it_was_asked() {
        let limits = codex_limits_from_rpc(&rpc_result(54.0, Some(6.0)), 77).unwrap();
        let primary = limits.primary.unwrap();
        assert_eq!(
            (
                primary.used_percent,
                primary.window_minutes,
                primary.resets_at
            ),
            (54.0, Some(300), Some(1_792_739_920))
        );
        assert_eq!(limits.secondary.unwrap().used_percent, 6.0);
        assert_eq!(
            (limits.observed_ms, limits.plan.as_deref()),
            (77, Some("plus"))
        );
        // One window is enough, a percentage outside 0 to 100 is kept inside, and an answer without any is none.
        let one = codex_limits_from_rpc(&rpc_result(140.0, None), 0).unwrap();
        assert_eq!(one.primary.unwrap().used_percent, 100.0);
        assert!(one.secondary.is_none());
        assert!(codex_limits_from_rpc(&json!({ "rateLimits": { "primary": null } }), 0).is_none());
        assert!(codex_limits_from_rpc(&json!({}), 0).is_none());
    }

    #[test]
    fn a_program_in_the_given_home_is_found_before_the_system_ones() {
        let home = std::env::temp_dir().join(format!("core-agents-find-{}", std::process::id()));
        let _ = fs::remove_dir_all(&home);
        fs::create_dir_all(home.join(".local/bin")).unwrap();
        fs::write(home.join(".local/bin/codex"), "").unwrap();
        assert_eq!(find_codex(&home), Some(home.join(".local/bin/codex")));
        let _ = fs::remove_dir_all(&home);
    }

    #[test]
    fn the_server_runs_with_the_home_it_was_given() {
        let command = server_command(Path::new("/opt/bin/codex"), Path::new("/private/home"));
        let env = |name: &str| {
            command
                .get_envs()
                .find(|(key, _)| *key == name)
                .and_then(|(_, value)| value)
                .map(|value| value.to_string_lossy().into_owned())
        };
        assert_eq!(command.get_args().collect::<Vec<_>>(), ["app-server"]);
        assert_eq!(env("HOME").as_deref(), Some("/private/home"));
        assert_eq!(env("CODEX_HOME").as_deref(), Some("/private/home/.codex"));
        assert!(env("PATH").unwrap().starts_with("/opt/bin:"));
    }

    /// Runs a shell script as the server, which answers the two requests the way the real one does.
    fn talk_to(script: &str, timeout: Duration) -> Result<Limits, String> {
        talk(Command::new("/bin/sh").args(["-c", script]), 5_000, timeout)
    }

    const HELLO: &str = r#"*'"initialize"'*) echo '{"id":1,"result":{}}' ;;"#;

    #[test]
    fn the_limits_come_out_of_the_answers_among_other_lines() {
        let script = format!(
            r#"while read line; do case "$line" in {HELLO}
            *rateLimits*) echo '{{"method":"account/rateLimits/updated","params":{{}}}}'; echo 'stray output';
              echo '{{"id":2,"result":{{"rateLimits":{{"primary":{{"usedPercent":42,"windowDurationMins":300}},"planType":"plus"}}}}}}' ;;
            esac; done"#
        );
        let limits = talk_to(&script, Duration::from_secs(10)).unwrap();
        assert_eq!(limits.primary.unwrap().used_percent, 42.0);
        assert_eq!(
            (limits.observed_ms, limits.plan.as_deref()),
            (5_000, Some("plus"))
        );
    }

    #[test]
    fn a_refusal_a_silent_server_and_a_missing_program_are_errors_and_nothing_is_left_running() {
        let refusal = format!(
            r#"while read line; do case "$line" in {HELLO}
            *rateLimits*) echo '{{"id":2,"error":{{"code":-32600,"message":"not signed in"}}}}' ;;
            esac; done"#
        );
        assert_eq!(
            talk_to(&refusal, Duration::from_secs(10)),
            Err("not signed in".to_owned())
        );
        // `exec` makes the shell the sleeper itself, so killing the server leaves nothing behind.
        let started = Instant::now();
        assert!(talk_to("exec sleep 30", Duration::from_millis(300)).is_err());
        assert!(started.elapsed() < Duration::from_secs(10));
        // A server that ends before it answers is an error at once, not a wait for the timeout.
        let started = Instant::now();
        assert!(talk_to("exit 3", Duration::from_secs(10)).is_err());
        assert!(started.elapsed() < Duration::from_secs(5));
        assert!(
            codex_limits_live(
                Path::new("/nonexistent/codex"),
                Path::new("/tmp"),
                0,
                LIVE_TIMEOUT
            )
            .is_err()
        );
    }
}
