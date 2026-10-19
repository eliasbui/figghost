//! Putting the hooks into the files the agents read, without disturbing anything else in them. These files
//! belong to the user and other tools write to them too, so entries are only ever added, updated or removed
//! when they carry our marker, and the file keeps its key order and permissions.

use crate::event::Agent;
use serde_json::{Value, json};
use std::{
    fmt, fs, io,
    path::{Path, PathBuf},
};

/// Every command we add contains this word, which is how we find our entries again.
pub const HOOK_MARKER: &str = "--figghost-hook";
pub const STATUS_MARKER: &str = "--figghost-statusline";
const ORIGINAL_STATUS_LINE: &str = "_figghostOriginalStatusLine";

/// A permission request may wait for a person; everything else must finish in an instant.
pub const REQUEST_TIMEOUT_SECONDS: u32 = 120;
pub const QUICK_TIMEOUT_SECONDS: u32 = 10;

#[derive(Debug, Clone, Copy)]
pub struct Subscription {
    pub event: &'static str,
    pub matcher: Option<&'static str>,
    pub timeout: u32,
}

const fn quick(event: &'static str, matcher: Option<&'static str>) -> Subscription {
    Subscription {
        event,
        matcher,
        timeout: QUICK_TIMEOUT_SECONDS,
    }
}

const REQUEST: Subscription = Subscription {
    event: "PermissionRequest",
    matcher: Some("*"),
    timeout: REQUEST_TIMEOUT_SECONDS,
};

const CLAUDE: &[Subscription] = &[
    quick("SessionStart", None),
    quick("UserPromptSubmit", None),
    REQUEST,
    quick("PostToolUse", Some("*")),
    quick("Notification", Some("*")),
    quick("Stop", None),
    quick("StopFailure", None),
    quick("SessionEnd", None),
];

const CODEX: &[Subscription] = &[
    quick("SessionStart", None),
    quick("UserPromptSubmit", None),
    REQUEST,
    quick("PostToolUse", Some("*")),
    quick("Stop", None),
    quick("Interrupt", None),
    quick("SessionEnd", None),
];

pub fn subscriptions(agent: Agent) -> &'static [Subscription] {
    match agent {
        Agent::Claude => CLAUDE,
        Agent::Codex => CODEX,
    }
}

#[derive(Debug)]
pub enum InstallError {
    Io(io::Error),
    Json(serde_json::Error),
    /// The file is valid JSON but not shaped the way the agent documents.
    Shape(String),
}

impl fmt::Display for InstallError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            InstallError::Io(e) => write!(f, "{e}"),
            InstallError::Json(e) => write!(f, "the file is not valid JSON: {e}"),
            InstallError::Shape(what) => write!(f, "{what}"),
        }
    }
}

impl std::error::Error for InstallError {}

impl From<io::Error> for InstallError {
    fn from(e: io::Error) -> Self {
        InstallError::Io(e)
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct Report {
    pub added: usize,
    pub updated: usize,
    pub unchanged: usize,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct Status {
    pub installed: usize,
    pub wanted: usize,
}

impl Status {
    pub fn complete(self) -> bool {
        self.installed == self.wanted
    }
}

/// The command an agent runs for each event. The path is quoted for the shell.
pub fn command_line(exe: &str, agent: Agent) -> String {
    format!("{} {HOOK_MARKER} {}", shell_quote(exe), agent.id())
}

pub fn statusline_command(exe: &str) -> String {
    format!("{} {STATUS_MARKER}", shell_quote(exe))
}

fn shell_quote(text: &str) -> String {
    format!("'{}'", text.replace('\'', "'\\''"))
}

fn is_ours(hook: &Value, marker: &str) -> bool {
    hook.get("command")
        .and_then(Value::as_str)
        .is_some_and(|c| c.contains(marker))
}

fn group(sub: &Subscription, command: &str) -> Value {
    let mut group = json!({});
    if let Some(matcher) = sub.matcher {
        group["matcher"] = json!(matcher);
    }
    group["hooks"] = json!([{ "type": "command", "command": command, "timeout": sub.timeout }]);
    group
}

fn hooks_object(doc: &mut Value) -> Result<&mut serde_json::Map<String, Value>, InstallError> {
    if doc.is_null() {
        *doc = json!({});
    }
    let root = doc
        .as_object_mut()
        .ok_or_else(|| InstallError::Shape("the file is not a JSON object".into()))?;
    root.entry("hooks")
        .or_insert_with(|| json!({}))
        .as_object_mut()
        .ok_or_else(|| InstallError::Shape("\"hooks\" is not an object".into()))
}

/// The entry of ours in an event's list, if there is one.
fn find_ours(list: &mut [Value]) -> Option<&mut Value> {
    list.iter_mut()
        .filter_map(|group| group.get_mut("hooks").and_then(Value::as_array_mut))
        .flat_map(|hooks| hooks.iter_mut())
        .find(|hook| is_ours(hook, HOOK_MARKER))
}

/// Adds our hook to every event we follow, or brings an existing entry of ours up to date. The document is
/// only meaningful when this returns `Ok`: on an error the caller must not write it.
pub fn install_hooks(doc: &mut Value, agent: Agent, exe: &str) -> Result<Report, InstallError> {
    let hooks = hooks_object(doc)?;
    let command = command_line(exe, agent);
    let mut report = Report::default();
    for sub in subscriptions(agent) {
        let list = hooks
            .entry(sub.event)
            .or_insert_with(|| json!([]))
            .as_array_mut()
            .ok_or_else(|| InstallError::Shape(format!("\"hooks.{}\" is not a list", sub.event)))?;
        match find_ours(list) {
            Some(entry) => {
                if entry["command"] == command && entry["timeout"] == sub.timeout {
                    report.unchanged += 1;
                } else {
                    entry["command"] = json!(command);
                    entry["timeout"] = json!(sub.timeout);
                    report.updated += 1;
                }
            }
            None => {
                list.push(group(sub, &command));
                report.added += 1;
            }
        }
    }
    Ok(report)
}

/// Removes every hook of ours and whatever it leaves empty; returns how many hooks went.
pub fn remove_hooks(doc: &mut Value) -> usize {
    let Some(hooks) = doc.get_mut("hooks").and_then(Value::as_object_mut) else {
        return 0;
    };
    let mut removed = 0;
    let mut emptied = Vec::new();
    for (event, list) in hooks.iter_mut() {
        let Some(list) = list.as_array_mut() else {
            continue;
        };
        let mut touched = false;
        list.retain_mut(|group| {
            let Some(inner) = group.get_mut("hooks").and_then(Value::as_array_mut) else {
                return true;
            };
            let before = inner.len();
            inner.retain(|hook| !is_ours(hook, HOOK_MARKER));
            let gone = before - inner.len();
            removed += gone;
            touched |= gone > 0;
            // A group is only dropped when our removal is what emptied it.
            !(gone > 0 && inner.is_empty())
        });
        if touched && list.is_empty() {
            emptied.push(event.clone());
        }
    }
    for event in emptied {
        hooks.shift_remove(&event);
    }
    if removed > 0
        && doc
            .get("hooks")
            .and_then(Value::as_object)
            .is_some_and(serde_json::Map::is_empty)
    {
        // Only a "hooks" key that we created ends up empty; one the user left empty stays as it was.
        if let Some(root) = doc.as_object_mut() {
            root.shift_remove("hooks");
        }
    }
    removed
}

pub fn status(doc: &Value, agent: Agent) -> Status {
    let wanted = subscriptions(agent);
    let installed = wanted
        .iter()
        .filter(|sub| {
            doc.get("hooks")
                .and_then(|h| h.get(sub.event))
                .and_then(Value::as_array)
                .is_some_and(|list| {
                    list.iter()
                        .filter_map(|g| g.get("hooks").and_then(Value::as_array))
                        .flatten()
                        .any(|hook| is_ours(hook, HOOK_MARKER))
                })
        })
        .count();
    Status {
        installed,
        wanted: wanted.len(),
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StatusLineChange {
    /// The command that was there before, which ours runs after it has read the input. Empty if none.
    pub chain: String,
    pub changed: bool,
}

/// Makes our program the status line, remembering the one that was there so it keeps working and can come back.
pub fn install_statusline(doc: &mut Value, exe: &str) -> Result<StatusLineChange, InstallError> {
    if doc.is_null() {
        *doc = json!({});
    }
    let root = doc
        .as_object_mut()
        .ok_or_else(|| InstallError::Shape("the file is not a JSON object".into()))?;
    let current = root.get("statusLine").cloned();
    if current.as_ref().is_some_and(|c| is_ours(c, STATUS_MARKER)) {
        // Ours already, but maybe at a path the app has moved away from (a build folder, then Applications): Claude
        // Code would run a program that is gone and the usage limits would stop at the last ones read. Only the
        // command changes; the line that was there before ours stays saved, and it is still the one to chain to.
        let wanted = statusline_command(exe);
        let chain = root
            .get(ORIGINAL_STATUS_LINE)
            .and_then(|c| c.get("command"))
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_owned();
        let line = root.get_mut("statusLine").and_then(Value::as_object_mut);
        let changed = line.is_some_and(|line| {
            let stale = line.get("command").and_then(Value::as_str) != Some(wanted.as_str());
            if stale {
                line.insert("command".to_owned(), Value::String(wanted));
            }
            stale
        });
        return Ok(StatusLineChange { chain, changed });
    }
    let chain = current
        .as_ref()
        .and_then(|c| c.get("command"))
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_owned();
    let padding = current
        .as_ref()
        .and_then(|c| c.get("padding"))
        .cloned()
        .unwrap_or(json!(0));
    root.insert(
        ORIGINAL_STATUS_LINE.to_owned(),
        current.unwrap_or(Value::Null),
    );
    root.insert(
        "statusLine".to_owned(),
        json!({ "type": "command", "command": statusline_command(exe), "padding": padding }),
    );
    Ok(StatusLineChange {
        chain,
        changed: true,
    })
}

/// Puts the previous status line back. Returns whether ours was there.
pub fn remove_statusline(doc: &mut Value) -> bool {
    let Some(root) = doc.as_object_mut() else {
        return false;
    };
    if !root
        .get("statusLine")
        .is_some_and(|c| is_ours(c, STATUS_MARKER))
    {
        return false;
    }
    match root.shift_remove(ORIGINAL_STATUS_LINE) {
        Some(previous) if !previous.is_null() => {
            root.insert("statusLine".to_owned(), previous);
        }
        _ => {
            root.shift_remove("statusLine");
        }
    }
    true
}

pub fn statusline_installed(doc: &Value) -> bool {
    doc.get("statusLine")
        .is_some_and(|c| is_ours(c, STATUS_MARKER))
}

/// Where each agent keeps the file the hooks go into, below the user's home directory.
pub struct Targets {
    pub claude_dir: PathBuf,
    pub claude_settings: PathBuf,
    pub codex_dir: PathBuf,
    pub codex_hooks: PathBuf,
}

pub fn targets(user_home: &Path) -> Targets {
    let claude_dir = user_home.join(".claude");
    let codex_dir = user_home.join(".codex");
    Targets {
        claude_settings: claude_dir.join("settings.json"),
        codex_hooks: codex_dir.join("hooks.json"),
        claude_dir,
        codex_dir,
    }
}

/// A missing file is an empty document; an unreadable one is an error and is never overwritten.
pub fn read_json(path: &Path) -> Result<Value, InstallError> {
    match fs::read_to_string(path) {
        Ok(text) if text.trim().is_empty() => Ok(json!({})),
        Ok(text) => serde_json::from_str(&text).map_err(InstallError::Json),
        Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(json!({})),
        Err(e) => Err(e.into()),
    }
}

/// Copies the file next to itself, once, before the first change we make to it.
pub fn backup_once(path: &Path) -> io::Result<()> {
    if !path.exists() {
        return Ok(());
    }
    let mut name = path.as_os_str().to_owned();
    name.push(".figghost-backup");
    let backup = PathBuf::from(name);
    if !backup.exists() {
        fs::copy(path, backup)?;
    }
    Ok(())
}

/// Replaces the file in one step and keeps its permissions (these files often hold secrets and are 0600).
pub fn write_json(path: &Path, doc: &Value) -> Result<(), InstallError> {
    let mut text = serde_json::to_string_pretty(doc).map_err(InstallError::Json)?;
    text.push('\n');
    let mut name = path.as_os_str().to_owned();
    name.push(format!(".figghost-{}.tmp", std::process::id()));
    let temp = PathBuf::from(name);
    fs::write(&temp, text)?;
    if let Ok(meta) = fs::metadata(path) {
        fs::set_permissions(&temp, meta.permissions())?;
    }
    fs::rename(&temp, path).inspect_err(|_| {
        let _ = fs::remove_file(&temp);
    })?;
    Ok(())
}

fn require_parent(path: &Path) -> Result<(), InstallError> {
    match path.parent() {
        Some(parent) if !parent.is_dir() => Err(InstallError::Io(io::Error::new(
            io::ErrorKind::NotFound,
            format!(
                "{} does not exist: is the agent installed?",
                parent.display()
            ),
        ))),
        _ => Ok(()),
    }
}

pub fn install_file(path: &Path, agent: Agent, exe: &str) -> Result<Report, InstallError> {
    require_parent(path)?;
    let mut doc = read_json(path)?;
    let report = install_hooks(&mut doc, agent, exe)?;
    if report.added + report.updated > 0 {
        backup_once(path)?;
        write_json(path, &doc)?;
    }
    Ok(report)
}

pub fn remove_file(path: &Path) -> Result<usize, InstallError> {
    if !path.exists() {
        return Ok(0);
    }
    let mut doc = read_json(path)?;
    let removed = remove_hooks(&mut doc);
    if removed > 0 {
        backup_once(path)?;
        write_json(path, &doc)?;
    }
    Ok(removed)
}

pub fn status_file(path: &Path, agent: Agent) -> Result<Status, InstallError> {
    Ok(status(&read_json(path)?, agent))
}

#[cfg(test)]
mod tests {
    use super::*;

    const EXE: &str = "/Applications/Figghost.app/Contents/MacOS/Figghost";

    /// The shapes found in real files: other tools' hooks with and without matchers, shell one-liners,
    /// a status line, and keys that have nothing to do with hooks.
    fn user_settings() -> Value {
        serde_json::from_str(
            r#"{
              "model": "opus",
              "permissions": { "allow": ["Bash(ls:*)"] },
              "statusLine": { "type": "command", "command": "'/Users/me/bin/status'", "padding": 2 },
              "hooks": {
                "Notification": [
                  { "matcher": "", "hooks": [ { "type": "command", "command": "if [ -n \"$X\" ]; then tool; fi", "timeout": 10 } ] },
                  { "matcher": "*", "hooks": [ { "type": "command", "command": "'/Users/me/other/hook'", "timeout": 45 } ] }
                ],
                "PermissionRequest": [
                  { "matcher": "*", "hooks": [ { "type": "command", "command": "'/Users/me/other/hook'", "timeout": 3600 } ] }
                ],
                "Stop": [ { "hooks": [ { "type": "command", "command": "other stop" } ] } ]
              },
              "theme": "dark"
            }"#,
        )
        .unwrap()
    }

    #[test]
    fn install_adds_next_to_the_hooks_of_other_tools_and_touches_nothing_else() {
        let mut doc = user_settings();
        let before = doc.clone();
        let report = install_hooks(&mut doc, Agent::Claude, EXE).unwrap();
        assert_eq!(
            report,
            Report {
                added: 8,
                updated: 0,
                unchanged: 0
            }
        );
        assert!(status(&doc, Agent::Claude).complete());
        // Everything that was there is still there, in the same place.
        for key in ["model", "permissions", "statusLine", "theme"] {
            assert_eq!(doc[key], before[key]);
        }
        let notification = doc["hooks"]["Notification"].as_array().unwrap();
        assert_eq!(
            &notification[..2],
            &before["hooks"]["Notification"].as_array().unwrap()[..]
        );
        assert_eq!(notification.len(), 3);
        assert_eq!(
            doc["hooks"]["PermissionRequest"][0],
            before["hooks"]["PermissionRequest"][0]
        );
        assert_eq!(doc["hooks"]["Stop"][0], before["hooks"]["Stop"][0]);
        let ours = &doc["hooks"]["PermissionRequest"][1]["hooks"][0];
        assert_eq!(ours["command"], format!("'{EXE}' --figghost-hook claude"));
        assert_eq!(ours["timeout"], REQUEST_TIMEOUT_SECONDS);
        assert_eq!(doc["hooks"]["PermissionRequest"][1]["matcher"], "*");
        assert!(doc["hooks"]["SessionStart"][0].get("matcher").is_none());
    }

    #[test]
    fn the_keys_keep_their_order() {
        let mut doc = user_settings();
        install_hooks(&mut doc, Agent::Claude, EXE).unwrap();
        let keys: Vec<&String> = doc.as_object().unwrap().keys().collect();
        assert_eq!(
            keys,
            ["model", "permissions", "statusLine", "hooks", "theme"]
        );
        let events: Vec<&String> = doc["hooks"].as_object().unwrap().keys().collect();
        assert_eq!(&events[..3], ["Notification", "PermissionRequest", "Stop"]);
    }

    #[test]
    fn installing_twice_changes_nothing() {
        let mut doc = user_settings();
        install_hooks(&mut doc, Agent::Claude, EXE).unwrap();
        let once = doc.clone();
        let report = install_hooks(&mut doc, Agent::Claude, EXE).unwrap();
        assert_eq!(
            report,
            Report {
                added: 0,
                updated: 0,
                unchanged: 8
            }
        );
        assert_eq!(doc, once);
    }

    #[test]
    fn a_moved_app_updates_its_entries_in_place() {
        let mut doc = user_settings();
        install_hooks(&mut doc, Agent::Claude, EXE).unwrap();
        let report = install_hooks(
            &mut doc,
            Agent::Claude,
            "/Users/me/My Apps/Figghost's.app/x",
        )
        .unwrap();
        assert_eq!(
            report,
            Report {
                added: 0,
                updated: 8,
                unchanged: 0
            }
        );
        let command = doc["hooks"]["Stop"][1]["hooks"][0]["command"]
            .as_str()
            .unwrap();
        assert_eq!(
            command,
            "'/Users/me/My Apps/Figghost'\\''s.app/x' --figghost-hook claude"
        );
        assert_eq!(
            doc["hooks"]["Stop"].as_array().unwrap().len(),
            2,
            "no duplicate"
        );
    }

    #[test]
    fn remove_gives_back_exactly_what_was_there() {
        let original = user_settings();
        let mut doc = original.clone();
        install_hooks(&mut doc, Agent::Claude, EXE).unwrap();
        assert_eq!(remove_hooks(&mut doc), 8);
        assert_eq!(doc, original);
        assert_eq!(status(&doc, Agent::Claude).installed, 0);
        assert_eq!(remove_hooks(&mut doc), 0);
    }

    #[test]
    fn remove_on_a_file_that_had_no_hooks_leaves_no_trace() {
        let original = json!({ "theme": "dark" });
        let mut doc = original.clone();
        install_hooks(&mut doc, Agent::Codex, EXE).unwrap();
        assert!(doc.get("hooks").is_some());
        remove_hooks(&mut doc);
        assert_eq!(doc, original);
    }

    #[test]
    fn remove_keeps_groups_that_were_already_empty() {
        let mut doc = json!({ "hooks": { "Stop": [ { "hooks": [] } ] } });
        let original = doc.clone();
        install_hooks(&mut doc, Agent::Claude, EXE).unwrap();
        remove_hooks(&mut doc);
        assert_eq!(doc, original);
    }

    #[test]
    fn codex_follows_its_own_events() {
        let mut doc = json!({});
        install_hooks(&mut doc, Agent::Codex, EXE).unwrap();
        let events: Vec<&String> = doc["hooks"].as_object().unwrap().keys().collect();
        assert!(events.contains(&&"Interrupt".to_owned()));
        assert!(!events.contains(&&"Notification".to_owned()));
        assert_eq!(
            doc["hooks"]["PermissionRequest"][0]["hooks"][0]["command"],
            format!("'{EXE}' --figghost-hook codex")
        );
        assert!(status(&doc, Agent::Codex).complete());
        assert!(!status(&doc, Agent::Claude).complete());
    }

    #[test]
    fn a_surprising_shape_is_an_error_and_not_a_guess() {
        let mut doc = json!({ "hooks": "nope" });
        assert!(matches!(
            install_hooks(&mut doc, Agent::Claude, EXE),
            Err(InstallError::Shape(_))
        ));
        let mut doc = json!({ "hooks": { "Stop": { "not": "a list" } } });
        assert!(matches!(
            install_hooks(&mut doc, Agent::Claude, EXE),
            Err(InstallError::Shape(_))
        ));
        let mut doc = json!([1, 2]);
        assert!(matches!(
            install_hooks(&mut doc, Agent::Claude, EXE),
            Err(InstallError::Shape(_))
        ));
    }

    #[test]
    fn the_status_line_is_wrapped_and_comes_back() {
        let original = user_settings();
        let mut doc = original.clone();
        let change = install_statusline(&mut doc, EXE).unwrap();
        assert_eq!(
            change,
            StatusLineChange {
                chain: "'/Users/me/bin/status'".into(),
                changed: true
            }
        );
        assert!(statusline_installed(&doc));
        assert_eq!(doc["statusLine"]["padding"], 2);
        // Doing it again changes nothing and must not forget the real original.
        let again = install_statusline(&mut doc, EXE).unwrap();
        assert!(!again.changed);
        assert!(remove_statusline(&mut doc));
        assert_eq!(doc, original);
        assert!(!remove_statusline(&mut doc));
    }

    #[test]
    fn a_status_line_left_at_an_old_path_follows_the_app() {
        // Installed from a build folder, then the app moved to Applications.
        let original = user_settings();
        let mut doc = original.clone();
        let built = "/Users/me/figghost/dist/Figghost.app/Contents/MacOS/Figghost";
        install_statusline(&mut doc, built).unwrap();
        let moved = install_statusline(&mut doc, EXE).unwrap();
        assert_eq!(
            moved,
            StatusLineChange {
                chain: "'/Users/me/bin/status'".into(),
                changed: true
            }
        );
        assert_eq!(doc["statusLine"]["command"], statusline_command(EXE));
        assert_eq!(doc["statusLine"]["padding"], 2);
        assert!(!install_statusline(&mut doc, EXE).unwrap().changed);
        // The user's own line is still the one that comes back.
        assert!(remove_statusline(&mut doc));
        assert_eq!(doc, original);
    }

    #[test]
    fn a_missing_status_line_is_removed_again_not_left_empty() {
        let original = json!({ "theme": "dark" });
        let mut doc = original.clone();
        let change = install_statusline(&mut doc, EXE).unwrap();
        assert_eq!(change.chain, "");
        assert!(remove_statusline(&mut doc));
        assert_eq!(doc, original);
    }

    #[test]
    fn files_are_backed_up_once_and_keep_their_permissions() {
        use std::os::unix::fs::PermissionsExt;
        let dir = std::env::temp_dir().join(format!("core-agents-install-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        let path = dir.join("settings.json");
        let original = serde_json::to_string_pretty(&user_settings()).unwrap();
        fs::write(&path, &original).unwrap();
        fs::set_permissions(&path, fs::Permissions::from_mode(0o600)).unwrap();

        let report = install_file(&path, Agent::Claude, EXE).unwrap();
        assert_eq!(report.added, 8);
        assert_eq!(
            fs::read_to_string(dir.join("settings.json.figghost-backup")).unwrap(),
            original
        );
        assert_eq!(
            fs::metadata(&path).unwrap().permissions().mode() & 0o777,
            0o600
        );
        assert!(status_file(&path, Agent::Claude).unwrap().complete());

        // A second change must not replace the first backup, which is the user's file as it was.
        install_file(&path, Agent::Claude, "/elsewhere/Figghost").unwrap();
        assert_eq!(
            fs::read_to_string(dir.join("settings.json.figghost-backup")).unwrap(),
            original
        );

        assert_eq!(remove_file(&path).unwrap(), 8);
        assert_eq!(read_json(&path).unwrap(), user_settings());
        let leftovers: Vec<_> = fs::read_dir(&dir)
            .unwrap()
            .flatten()
            .map(|e| e.file_name())
            .collect();
        assert_eq!(
            leftovers.len(),
            2,
            "only the file and its backup remain: {leftovers:?}"
        );
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_broken_file_is_reported_and_left_alone() {
        let dir = std::env::temp_dir().join(format!("core-agents-broken-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        let path = dir.join("settings.json");
        fs::write(&path, "{ this is not json").unwrap();
        assert!(matches!(
            install_file(&path, Agent::Claude, EXE),
            Err(InstallError::Json(_))
        ));
        assert_eq!(fs::read_to_string(&path).unwrap(), "{ this is not json");
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn an_agent_that_is_not_installed_gets_no_directory_made_for_it() {
        let path = std::env::temp_dir()
            .join(format!("core-agents-absent-{}", std::process::id()))
            .join(".codex")
            .join("hooks.json");
        let err = install_file(&path, Agent::Codex, EXE).unwrap_err();
        assert!(matches!(err, InstallError::Io(_)));
        assert!(!path.parent().unwrap().exists());
    }

    #[test]
    fn a_missing_file_inside_an_existing_directory_is_created() {
        let dir = std::env::temp_dir().join(format!("core-agents-new-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        let path = dir.join("hooks.json");
        install_file(&path, Agent::Codex, EXE).unwrap();
        assert!(status_file(&path, Agent::Codex).unwrap().complete());
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn the_command_runs_in_a_shell_even_when_the_path_has_spaces_and_quotes() {
        use std::os::unix::fs::PermissionsExt;
        let dir = std::env::temp_dir().join(format!("figghost it's here-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let exe = dir.join("Figghost");
        std::fs::write(&exe, "#!/bin/sh\nprintf '%s|%s' \"$1\" \"$2\"\n").unwrap();
        std::fs::set_permissions(&exe, std::fs::Permissions::from_mode(0o755)).unwrap();
        let exe = exe.to_string_lossy().into_owned();
        for (command, expected) in [
            (command_line(&exe, Agent::Codex), "--figghost-hook|codex"),
            (statusline_command(&exe), "--figghost-statusline|"),
        ] {
            let out = std::process::Command::new("/bin/sh")
                .arg("-c")
                .arg(&command)
                .output()
                .unwrap();
            assert_eq!(String::from_utf8_lossy(&out.stdout), expected, "{command}");
        }
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
