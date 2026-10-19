//! Mole (github.com/tw93/Mole, GPL-3.0) in the settings window: the app runs the user's own `mo` program
//! for a health reading, a preview or a run of `mo clean` and `mo optimize`, and shows what it printed as
//! cards. Nothing of Mole is bundled or linked; without `mo` the page says how to install it.

use super::AttachEnv;
use crate::settings::model::Fault;
use macos_sys::workspace;
use serde_json::{Value, json};
use std::{
    path::PathBuf,
    process::{Command, Stdio},
    sync::{Arc, Mutex},
};

const SITE: &str = "https://github.com/tw93/Mole";
/// A GUI app does not inherit the shell's PATH, so the usual install places are tried in turn.
const PLACES: [&str; 3] = ["/opt/homebrew/bin/mo", "/usr/local/bin/mo", ".local/bin/mo"];
/// Lines of one report kept for the page; a long list is cut, not paged.
const MAX_LINES: usize = 300;

const SECTION: char = '\u{27A4}';
const ITEM: char = '\u{2192}';
const NOTE: [char; 3] = ['\u{25CE}', '\u{2299}', '\u{2713}'];

#[derive(Default)]
struct Inner {
    busy: Option<&'static str>,
    status: Value,
    report: Value,
    apps: Value,
    analyze: Value,
    history: Value,
}

pub struct Feature {
    bin: Option<PathBuf>,
    inner: Arc<Mutex<Inner>>,
}

pub fn attach(env: &AttachEnv) -> Option<Rc<Feature>> {
    let bin = PLACES
        .iter()
        .map(|p| {
            let path = PathBuf::from(p);
            if path.is_absolute() {
                path
            } else {
                env.home.join(p)
            }
        })
        .find(|p| p.is_file());
    Some(Rc::new(Feature {
        bin,
        inner: Arc::default(),
    }))
}

use std::rc::Rc;

impl Feature {
    pub fn runtime(&self) -> Value {
        let inner = self.inner.lock().unwrap_or_else(|e| e.into_inner());
        json!({
            "installed": self.bin.is_some(),
            "busy": inner.busy,
            "status": inner.status,
            "report": inner.report,
            "apps": inner.apps,
            "analyze": inner.analyze,
            "history": inner.history,
        })
    }

    /// A button of the page; `None` when the name is not this feature's. A name may carry one argument after
    /// a colon: the app to uninstall or the folder to analyze, checked here because the page is not trusted.
    pub fn action(&self, name: &str) -> Option<Result<(), Fault>> {
        let (verb, arg) = name.split_once(':').unwrap_or((name, ""));
        let plan = match verb {
            "mole_open_site" => {
                workspace::open_url(SITE);
                return Some(Ok(()));
            }
            "mole_terminal_installer" => {
                return Some(self.in_terminal("installer"));
            }
            "mole_status" => Plan::new(Kind::Status, &["status", "--json"]),
            "mole_apps" => Plan::new(Kind::Apps, &["uninstall", "--list"]),
            "mole_history" => Plan::new(Kind::History, &["history", "--json"]),
            "mole_analyze" => match self.folder(arg) {
                Ok(dir) => Plan::new(Kind::Analyze, &["analyze", "--json"]).arg(dir),
                Err(f) => return Some(Err(f)),
            },
            "mole_clean_preview" => Plan::new(Kind::Report("clean", true), &["clean", "--dry-run"]),
            "mole_clean_run" => Plan::new(Kind::Report("clean", false), &["clean"]),
            "mole_optimize_preview" => {
                Plan::new(Kind::Report("optimize", true), &["optimize", "--dry-run"])
            }
            "mole_optimize_run" => Plan::new(Kind::Report("optimize", false), &["optimize"]),
            "mole_purge_preview" => Plan::new(Kind::Report("purge", true), &["purge", "--dry-run"]),
            "mole_purge_run" => Plan::new(Kind::Report("purge", false), &["purge", "--yes"]),
            "mole_uninstall_preview" | "mole_uninstall_run" => {
                let dry = verb.ends_with("preview");
                match self.app(arg) {
                    Ok(app) => {
                        let mut plan = Plan::new(Kind::Report("uninstall", dry), &["uninstall"]);
                        if dry {
                            plan = plan.arg("--dry-run".into());
                        }
                        // `mo uninstall` asks once before it removes anything; the page already asked.
                        plan.arg(app).stdin("y\n")
                    }
                    Err(f) => return Some(Err(f)),
                }
            }
            _ => return None,
        };
        let Some(bin) = self.bin.clone() else {
            return Some(Err(Fault::new("mole_missing", "mo is not installed")));
        };
        let mut inner = self.inner.lock().unwrap_or_else(|e| e.into_inner());
        if inner.busy.is_some() {
            return Some(Ok(()));
        }
        inner.busy = Some(plan.kind.label());
        drop(inner);
        let shared = Arc::clone(&self.inner);
        // ponytail: no timeout; a hung `mo` keeps the page busy until the app restarts.
        std::thread::spawn(move || {
            let out = plan.run(&bin);
            let mut inner = shared.lock().unwrap_or_else(|e| e.into_inner());
            inner.busy = None;
            plan.kind.store(&mut inner, out);
        });
        Some(Ok(()))
    }

    /// An absolute folder, as the analyzer takes it; empty means the whole Mac.
    fn folder(&self, arg: &str) -> Result<String, Fault> {
        if arg.is_empty() {
            return Ok(String::new());
        }
        let path = std::path::Path::new(arg);
        if !path.is_absolute() || !path.is_dir() {
            return Err(Fault::new("invalid_value", "not a folder"));
        }
        Ok(arg.to_owned())
    }

    /// The name `mo uninstall` accepts, only if it is one of the apps it listed.
    fn app(&self, arg: &str) -> Result<String, Fault> {
        let inner = self.inner.lock().unwrap_or_else(|e| e.into_inner());
        inner
            .apps
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(|a| a["uninstall_name"].as_str())
            .find(|n| *n == arg && !n.starts_with('-'))
            .map(str::to_owned)
            .ok_or_else(|| Fault::new("invalid_value", "unknown app"))
    }

    /// `mo installer` and similar choose from a list in the terminal and have no plain mode.
    fn in_terminal(&self, command: &str) -> Result<(), Fault> {
        let Some(bin) = &self.bin else {
            return Err(Fault::new("mole_missing", "mo is not installed"));
        };
        let line = format!("'{}' {command}", bin.to_string_lossy().replace('\'', ""));
        let script = format!(
            "tell application \"Terminal\"\nactivate\ndo script \"{}\"\nend tell",
            line.replace('\\', "\\\\").replace('"', "\\\"")
        );
        Command::new("osascript")
            .args(["-e", &script])
            .spawn()
            .map(|_| ())
            .map_err(|e| Fault::new("mole_terminal", e.to_string()))
    }
}

#[derive(Clone, Copy)]
enum Kind {
    Status,
    Apps,
    History,
    Analyze,
    /// A task name and whether it is a preview.
    Report(&'static str, bool),
}

impl Kind {
    fn label(self) -> &'static str {
        match self {
            Kind::Status => "status",
            Kind::Apps => "apps",
            Kind::History => "history",
            Kind::Analyze => "analyze",
            Kind::Report(task, _) => task,
        }
    }

    fn store(self, inner: &mut Inner, out: std::io::Result<std::process::Output>) {
        let out = match out {
            Ok(out) => out,
            Err(e) => {
                if let Kind::Report(task, dry) = self {
                    inner.report =
                        json!({ "task": task, "dry": dry, "ok": false, "error": e.to_string() });
                }
                return;
            }
        };
        let json = || serde_json::from_slice::<Value>(&out.stdout).ok();
        match self {
            Kind::Status => inner.status = json().map(|v| health(&v)).unwrap_or(Value::Null),
            Kind::Apps => inner.apps = json().filter(Value::is_array).unwrap_or(Value::Null),
            Kind::History => inner.history = json().map(|v| sessions(&v)).unwrap_or(Value::Null),
            Kind::Analyze => inner.analyze = json().map(|v| entries(&v)).unwrap_or(Value::Null),
            Kind::Report(task, dry) => {
                let text = String::from_utf8_lossy(&out.stdout);
                inner.report = parse(task, dry, out.status.success(), &text);
            }
        }
    }
}

struct Plan {
    kind: Kind,
    args: Vec<String>,
    stdin: Option<&'static str>,
}

impl Plan {
    fn new(kind: Kind, args: &[&str]) -> Self {
        Self {
            kind,
            args: args.iter().map(|a| (*a).to_owned()).collect(),
            stdin: None,
        }
    }

    /// Appended after the fixed arguments; an empty one is left out.
    fn arg(mut self, arg: String) -> Self {
        if !arg.is_empty() {
            self.args.push(arg);
        }
        self
    }

    fn stdin(mut self, text: &'static str) -> Self {
        self.stdin = Some(text);
        self
    }

    fn run(&self, bin: &std::path::Path) -> std::io::Result<std::process::Output> {
        use std::io::Write;
        let mut child = Command::new(bin)
            .args(&self.args)
            .stdin(if self.stdin.is_some() {
                Stdio::piped()
            } else {
                Stdio::null()
            })
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()?;
        if let (Some(text), Some(mut pipe)) = (self.stdin, child.stdin.take()) {
            let _ = pipe.write_all(text.as_bytes());
        }
        child.wait_with_output()
    }
}

/// The state under the key "mole". `web/features/mole.fixture.json` must have exactly the same keys.
pub mod settings {
    use serde_json::{Value, json};

    pub fn state(runtime: &Value) -> Value {
        json!({
            "installed": runtime["installed"].as_bool().unwrap_or(false),
            "busy": runtime["busy"].as_str(),
            "status": if runtime["status"].is_object() { runtime["status"].clone() } else { Value::Null },
            "report": if runtime["report"].is_object() { runtime["report"].clone() } else { Value::Null },
            "apps": runtime["apps"].clone(),
            "analyze": runtime["analyze"].clone(),
            "history": runtime["history"].clone(),
        })
    }
}

/// The few numbers of `mo status --json` the page shows.
fn health(v: &Value) -> Value {
    let disk = &v["disks"][0];
    json!({
        "score": v["health_score"],
        "message": v["health_score_msg"],
        "uptime": v["uptime"],
        "model": v["hardware"]["model"],
        "chip": v["hardware"]["cpu_model"],
        "os": v["hardware"]["os_version"],
        "cpu": v["cpu"]["usage"],
        "memory": v["memory"]["used_percent"],
        "swap_used": v["memory"]["swap_used"],
        "swap_total": v["memory"]["swap_total"],
        "disk": disk["used_percent"],
        "disk_used": disk["used"],
        "disk_total": disk["total"],
    })
}

/// The folder or the Mac as `mo analyze --json` sizes it: the biggest entries first.
fn entries(v: &Value) -> Value {
    let mut list: Vec<Value> = v["entries"].as_array().cloned().unwrap_or_default();
    list.sort_by_key(|e| std::cmp::Reverse(e["size"].as_u64().unwrap_or(0)));
    list.truncate(40);
    let list: Vec<Value> = list
        .iter()
        .map(|e| json!({ "name": e["name"], "path": e["path"], "size": e["size"], "dir": e["is_dir"] }))
        .collect();
    json!({ "path": v["path"], "overview": v["overview"], "total": v["total_size"], "entries": list })
}

/// What `mo history --json` kept of each session.
fn sessions(v: &Value) -> Value {
    let list: Vec<Value> = v["sessions"]
        .as_array()
        .into_iter()
        .flatten()
        .map(|s| json!({
            "command": s["command"], "at": s["started_at"], "items": s["items"], "size": s["size"],
            "failed": s["failed_tasks"],
        }))
        .collect();
    Value::Array(list)
}

/// Removes the colour and cursor codes `mo` prints even when its output is not a terminal.
fn strip_ansi(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut chars = text.chars().peekable();
    while let Some(c) = chars.next() {
        if c != '\u{1b}' {
            out.push(c);
        } else if chars.next_if_eq(&'[').is_some() {
            for n in chars.by_ref() {
                if n.is_ascii_alphabetic() {
                    break;
                }
            }
        }
    }
    out
}

/// Sections (`SECTION name`) with their lines, and the closing summary between the rules of `=`.
fn parse(task: &str, dry: bool, ok: bool, raw: &str) -> Value {
    let text = strip_ansi(raw);
    let mut sections: Vec<Value> = Vec::new();
    let mut summary: Vec<String> = Vec::new();
    let mut rules = 0;
    let mut lines = 0;
    for line in text.lines().map(str::trim).filter(|l| !l.is_empty()) {
        if line.starts_with("====") {
            rules += 1;
        } else if rules == 1 {
            summary.push(line.to_owned());
        } else if let Some(name) = line.strip_prefix(SECTION) {
            sections.push(json!({ "name": name.trim(), "lines": [] }));
        } else if lines < MAX_LINES && !line.contains("DRY RUN MODE") {
            let first = line.chars().next().unwrap_or(' ');
            let kind = if first == ITEM || first.is_ascii_digit() || line.contains("[DRY RUN]") {
                "item"
            } else if NOTE.contains(&first) {
                "note"
            } else {
                continue;
            };
            if sections.is_empty() {
                sections.push(json!({ "name": "", "lines": [] }));
            }
            let section = sections.last_mut().expect("a section was just made");
            let body = if first.is_ascii_digit() {
                line
            } else {
                line[first.len_utf8()..].trim()
            };
            section["lines"]
                .as_array_mut()
                .expect("lines is an array")
                .push(json!({ "kind": kind, "text": body }));
            lines += 1;
        }
    }
    json!({ "task": task, "dry": dry, "ok": ok, "sections": sections, "summary": summary })
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = "\u{1b}[1;36mClean Your Mac\u{1b}[0m\n\n\u{27A4} User essentials\n  \u{2192} User app cache \u{b7} 6 items, 340.8MB dry\n\u{27A4} Browsers\n  \u{2713} Nothing to clean\n\n======\nDry run complete - no changes made\nPotential space: 1.48GB | Items: 29\n======\n";

    #[test]
    fn a_report_keeps_sections_lines_and_summary() {
        let r = parse("clean", true, true, SAMPLE);
        let sections = r["sections"].as_array().unwrap();
        assert_eq!(sections.len(), 2);
        assert_eq!(sections[0]["name"], "User essentials");
        assert_eq!(sections[0]["lines"][0]["kind"], "item");
        assert_eq!(
            sections[0]["lines"][0]["text"],
            "User app cache \u{b7} 6 items, 340.8MB dry"
        );
        assert_eq!(sections[1]["lines"][0]["kind"], "note");
        assert_eq!(r["summary"][1], "Potential space: 1.48GB | Items: 29");
        assert!(!r.to_string().contains('\u{1b}'));
    }

    #[test]
    fn the_state_has_the_keys_of_the_fixture() {
        let fixture: Value =
            serde_json::from_str(include_str!("../../../web/features/mole.fixture.json")).unwrap();
        let state = settings::state(&Value::Null);
        let keys = |v: &Value| v.as_object().unwrap().keys().cloned().collect::<Vec<_>>();
        assert_eq!(keys(&state), keys(&fixture));
    }
}
