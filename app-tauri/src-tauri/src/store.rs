//! The settings of the window as the page sees them.
//!
//! The state starts from the sample state of the current page (src/data/state.sample.json, written by
//! tools/extract-state.mjs) and keeps what the person changed on top of it, as a list of paths and values in `settings.json`
//! in the data folder of this app. That is a stand-in: what a setting does to the machine is the work of the engines, which
//! are not wired in yet, and the sample holds numbers and names that only an engine can know (the connections of Network, the
//! accounts of the agents, the calendars). What is here is what does not need an engine: the rules of `rules.rs`, the
//! shortcuts, and the lists the page edits one entry at a time (apps, snippets, the tabs of the notch, the wires of Network).
use crate::messages::{Message, RuleField};
use crate::rules::{self, get, put, slots, OWN_MESSAGES};
use serde_json::{json, Map, Value};
use std::{
    fs, io,
    path::{Path, PathBuf},
};

const SAMPLE: &str = include_str!("../../src/data/state.sample.json");
const FILE: &str = "settings.json";
const VERSION: u64 = 1;
const TILING: &str = "tiling.layouts";
/// The most entries of a list the page can make grow without bound.
const MOST_ENTRIES: usize = 2000;

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

/// What the app has to do after a change was kept.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Effect {
    None,
    /// The light or dark appearance of the windows changed.
    Theme,
    Language,
    /// Start the app at login, or stop.
    LoginItem(bool),
    /// Show or hide the menu bar icon.
    MenuIcon(bool),
}

pub struct Store {
    dir: PathBuf,
    state: Value,
    overrides: Map<String, Value>,
}

/// Moves a file that cannot be used out of the way, so that a later save cannot destroy what the person edited by hand.
fn set_aside(path: &Path, why: &str) -> String {
    let backup = path.with_extension("json.bad");
    match fs::rename(path, &backup) {
        Ok(()) => format!(
            "{} cannot be used ({why}); it was moved to {}",
            path.display(),
            backup.display()
        ),
        Err(moved) => format!(
            "{} cannot be used ({why}) and could not be moved aside ({moved})",
            path.display()
        ),
    }
}

fn read_overrides(dir: &Path) -> (Map<String, Value>, Option<String>) {
    let path = dir.join(FILE);
    let text = match fs::read_to_string(&path) {
        Ok(text) => text,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return (Map::new(), None),
        Err(error) => return (Map::new(), Some(set_aside(&path, &error.to_string()))),
    };
    match serde_json::from_str::<Value>(&text) {
        Ok(doc) => (
            doc.get("overrides")
                .and_then(Value::as_object)
                .cloned()
                .unwrap_or_default(),
            None,
        ),
        Err(error) => (Map::new(), Some(set_aside(&path, &error.to_string()))),
    }
}

fn text_of<'a>(value: &'a Value, name: &str) -> Option<&'a str> {
    value.get(name)?.as_str()
}

impl Store {
    /// Reads what was kept in `dir`. The second answer says what went wrong, if something did, for the log.
    pub fn open(dir: &Path) -> (Self, Option<String>) {
        let (saved, note) = read_overrides(dir);
        let mut state: Value = serde_json::from_str(SAMPLE).expect("the sample state is JSON");
        let mut overrides = Map::new();
        for (path, value) in &saved {
            if path == TILING {
                continue;
            }
            // What the file holds goes through the same rules as what the page sends, so that a hand-edited file cannot
            // put a value in the state that the page could not.
            if let Ok(value) = rules::normalize(&state, path, value) {
                if put(&mut state, path, value.clone()) {
                    overrides.insert(path.clone(), value);
                }
            }
        }
        let layouts = rules::tiling_layouts(saved.get(TILING));
        if layouts != rules::tiling_layouts(None) {
            overrides.insert(TILING.to_owned(), layouts.clone());
        }
        put(&mut state, TILING, layouts);
        (
            Self {
                dir: dir.to_owned(),
                state,
                overrides,
            },
            note,
        )
    }

    /// The state, without the facts around it (`env` and `app`), which the host fills in.
    pub fn state(&self) -> &Value {
        &self.state
    }

    /// The language the app itself runs in: the setting, or the first language of the system that the page has words for.
    pub fn language(&self, system: &str) -> &'static str {
        match get(&self.state, "general.language").and_then(Value::as_str) {
            Some("en") => "en",
            Some("vi") => "vi",
            _ if system.starts_with("vi") => "vi",
            _ => "en",
        }
    }

    pub fn theme(&self) -> &str {
        get(&self.state, "general.theme")
            .and_then(Value::as_str)
            .unwrap_or("auto")
    }

    pub fn save(&self) -> io::Result<()> {
        #[cfg(unix)]
        {
            use std::os::unix::fs::DirBuilderExt;
            fs::DirBuilder::new()
                .recursive(true)
                .mode(0o700)
                .create(&self.dir)?;
        }
        #[cfg(not(unix))]
        fs::create_dir_all(&self.dir)?;
        let doc = json!({ "version": VERSION, "overrides": self.overrides });
        let tmp = self.dir.join(format!("{FILE}.tmp"));
        fs::write(
            &tmp,
            serde_json::to_vec_pretty(&doc).map_err(io::Error::other)?,
        )?;
        fs::rename(tmp, self.dir.join(FILE))
    }

    /// Keeps a value at a path that exists, in the state and in the file. The shortcuts of the tiling list are kept with the
    /// whole list.
    fn write(&mut self, path: &str, value: Value) {
        if !put(&mut self.state, path, value) {
            return;
        }
        let unit = if path.starts_with("tiling.layouts.") {
            TILING
        } else {
            path
        };
        match get(&self.state, unit) {
            // Back to the defaults of the layouts: leave no override behind.
            Some(now) if unit == TILING && *now == rules::tiling_layouts(None) => {
                self.overrides.remove(unit);
            }
            Some(now) => {
                self.overrides.insert(unit.to_owned(), now.clone());
            }
            None => {}
        }
    }

    /// The entries of a list that are objects; anything else a hand-edited file may hold is not an entry.
    fn list(&self, path: &str) -> Vec<Value> {
        get(&self.state, path)
            .and_then(Value::as_array)
            .map(|items| items.iter().filter(|v| v.is_object()).cloned().collect())
            .unwrap_or_default()
    }

    fn flag(&self, path: &str) -> bool {
        get(&self.state, path)
            .and_then(Value::as_bool)
            .unwrap_or(false)
    }

    fn number(&self, path: &str) -> usize {
        get(&self.state, path).and_then(Value::as_u64).unwrap_or(0) as usize
    }

    /// Applies what the page sent. A change the state cannot take is refused and leaves everything as it was.
    pub fn handle(&mut self, message: &Message) -> Result<Effect, Fault> {
        match message {
            Message::Set { path, value } => self.set(path, value),
            Message::AppRule { id, field, value } => self.app_rule(id, *field, *value),
            Message::AppAdd { id } => self.app_add(id),
            Message::AppRemove { id } => {
                self.forget("mouse.apps", id);
                Ok(Effect::None)
            }
            Message::IgnoredAdd { id } => {
                rules::check_id(id)?;
                let mut apps = self.list("clipboard.ignored");
                if !apps.iter().any(|a| a["id"] == id.as_str()) {
                    apps.push(json!({ "id": id, "name": id }));
                    self.write("clipboard.ignored", Value::Array(apps));
                }
                Ok(Effect::None)
            }
            Message::IgnoredRemove { id } => {
                self.forget("clipboard.ignored", id);
                Ok(Effect::None)
            }
            Message::Hotkey { target, value } => self.hotkey(target, value),
            Message::Action { name } if name == "reset_tiling" => {
                self.write(TILING, rules::tiling_layouts(None));
                Ok(Effect::None)
            }
            _ => Ok(Effect::None),
        }
    }

    fn forget(&mut self, path: &str, id: &str) {
        let mut apps = self.list(path);
        apps.retain(|a| a["id"] != id);
        self.write(path, Value::Array(apps));
    }

    fn app_rule(
        &mut self,
        id: &str,
        field: RuleField,
        value: Option<bool>,
    ) -> Result<Effect, Fault> {
        rules::check_id(id)?;
        let key = match field {
            RuleField::Smooth => "smooth",
            RuleField::Reverse => "reverse",
        };
        let mut apps = self.list("mouse.apps");
        let at = match apps.iter().position(|a| a["id"] == id) {
            Some(at) => at,
            None => {
                apps.push(json!({ "id": id, "name": id, "smooth": null, "reverse": null }));
                apps.len() - 1
            }
        };
        apps[at][key] = json!(value);
        // A rule that says nothing is just clutter in the file.
        apps.retain(|a| !(a["smooth"].is_null() && a["reverse"].is_null()));
        self.write("mouse.apps", Value::Array(apps));
        Ok(Effect::None)
    }

    fn app_add(&mut self, id: &str) -> Result<Effect, Fault> {
        rules::check_id(id)?;
        // The new entry does the opposite of the global switch, which is the point of listing the app: with smoothing on it
        // is blocked, with smoothing off it is allowed.
        let smooth = !self.flag("mouse.smooth");
        let mut apps = self.list("mouse.apps");
        match apps.iter().position(|a| a["id"] == id) {
            Some(at) => apps[at]["smooth"] = json!(smooth),
            None => apps.push(json!({ "id": id, "name": id, "smooth": smooth, "reverse": null })),
        }
        self.write("mouse.apps", Value::Array(apps));
        Ok(Effect::None)
    }

    fn set(&mut self, path: &str, value: &Value) -> Result<Effect, Fault> {
        match path {
            "launch_at_login" => {
                return value
                    .as_bool()
                    .map(Effect::LoginItem)
                    .ok_or_else(|| Fault::new("invalid_value", path))
            }
            "notch.pages" => return self.tabs(value),
            "snippets.add" | "snippets.update" | "snippets.remove" => {
                return self.snippets(path, value)
            }
            "calendar.include" => return self.include(value),
            "network.place"
            | "network.known_link"
            | "network.wire"
            | "network.unwire"
            | "network.rule"
            | "network.merge"
            | "network.metered.mark" => {
                return self.network(path, value);
            }
            // A secret waits for the Keychain; it is not kept in a file.
            "jarvis.key" => return Err(Fault::new("unsupported", path)),
            _ => {}
        }
        rules::valid_path(path)?;
        if OWN_MESSAGES.contains(&path) {
            return Err(Fault::new("unknown_setting", path));
        }
        let value = rules::normalize(&self.state, path, value)?;
        self.write(path, value);
        Ok(match path {
            "general.theme" => Effect::Theme,
            "general.language" => Effect::Language,
            "general.menu_icon" => Effect::MenuIcon(self.flag(path)),
            _ => Effect::None,
        })
    }

    /// The page keeps every tab in order and sends the ids of the ones that show: those come first, in that order, the others
    /// after them as they were. A notch with no tab could not be used.
    fn tabs(&mut self, value: &Value) -> Result<Effect, Fault> {
        let refused = || Fault::new("invalid_value", "expected a list of tab ids");
        let sent = value.as_array().ok_or_else(refused)?;
        let shown: Vec<&str> = sent.iter().filter_map(Value::as_str).collect();
        let tabs = self.list("notch.pages");
        let mut seen: Vec<&str> = Vec::new();
        for id in &shown {
            if seen.contains(id) || !tabs.iter().any(|t| t["id"] == *id) {
                return Err(refused());
            }
            seen.push(*id);
        }
        if shown.is_empty() || shown.len() != sent.len() {
            return Err(refused());
        }
        let mut next: Vec<Value> = Vec::new();
        for id in &shown {
            if let Some(tab) = tabs.iter().find(|t| t["id"] == *id) {
                let mut tab = tab.clone();
                tab["on"] = json!(true);
                next.push(tab);
            }
        }
        for tab in &tabs {
            if !shown.iter().any(|id| tab["id"] == *id) {
                let mut tab = tab.clone();
                tab["on"] = json!(false);
                next.push(tab);
            }
        }
        self.write("notch.pages", Value::Array(next));
        Ok(Effect::None)
    }

    fn hotkey(&mut self, target: &str, value: &str) -> Result<Effect, Fault> {
        let value = value.trim();
        let all = slots(&self.state);
        let Some(slot) = all.iter().find(|s| s.target == target) else {
            return Err(Fault::new("unknown_setting", target));
        };
        let panel_key = target.starts_with("keys:");
        if value.is_empty() {
            // The clipboard popup would become unreachable, and an action in the panels would lose its key.
            if panel_key || target == "clipboard" {
                return Err(Fault::new("shortcut_required", target));
            }
        } else {
            let chord = rules::parse_shortcut(value).map_err(|code| Fault::new(code, target))?;
            if let Some(other) = all.iter().find(|o| {
                o.target != target && rules::parse_shortcut(&o.value).ok().as_ref() == Some(&chord)
            }) {
                return Err(Fault::new("conflict", other.target.clone()));
            }
            // Command with a single key already means a tab, a pinned item or Delete in the panels.
            if panel_key && chord.modifiers == rules::COMMAND {
                return Err(Fault::new("reserved", target));
            }
        }
        let path = slot.path.clone();
        self.write(&path, json!(value));
        Ok(Effect::None)
    }

    /// The rules of a snippet, in the order of the current app: the abbreviation, then the text.
    fn check_snippet(&self, abbr: &str, text: &str) -> Result<(), Fault> {
        let count = |s: &str| s.chars().count();
        let code = if abbr.is_empty() {
            "snippets_empty"
        } else if count(abbr) > self.number("snippets.max_abbr") {
            "snippets_long"
        } else if abbr.chars().any(|c| c.is_whitespace() || c.is_control()) {
            "snippets_space"
        } else if !abbr.bytes().all(|b| (b'!'..=b'~').contains(&b)) {
            // Not a character a US keyboard types, so that the abbreviation could never be matched.
            "snippets_untypeable"
        } else if text.is_empty() {
            "snippets_no_text"
        } else if count(text) > self.number("snippets.max_text") {
            "snippets_text_long"
        } else {
            return Ok(());
        };
        Err(Fault::new(code, "snippets"))
    }

    fn snippets(&mut self, op: &str, value: &Value) -> Result<Effect, Fault> {
        let invalid = || Fault::new("invalid_value", op);
        let mut items: Vec<(String, String)> = self
            .list("snippets.items")
            .iter()
            .map(|i| {
                (
                    text_of(i, "abbr").unwrap_or_default().to_owned(),
                    text_of(i, "text").unwrap_or_default().to_owned(),
                )
            })
            .collect();
        match op {
            "snippets.add" => {
                let (abbr, text) = (
                    text_of(value, "abbr").ok_or_else(invalid)?,
                    rules::lf(text_of(value, "text").ok_or_else(invalid)?),
                );
                self.check_snippet(abbr, &text)?;
                if items.iter().any(|(a, _)| a == abbr) {
                    return Err(Fault::new("snippets_duplicate", "snippets"));
                }
                if items.len() >= self.number("snippets.max_items") {
                    return Err(Fault::new("snippets_full", "snippets"));
                }
                items.push((abbr.to_owned(), text));
            }
            "snippets.update" => {
                let (abbr, text, from) = (
                    text_of(value, "abbr").ok_or_else(invalid)?,
                    rules::lf(text_of(value, "text").ok_or_else(invalid)?),
                    text_of(value, "from").ok_or_else(invalid)?,
                );
                self.check_snippet(abbr, &text)?;
                let at = items
                    .iter()
                    .position(|(a, _)| a == from)
                    .ok_or_else(invalid)?;
                if items
                    .iter()
                    .enumerate()
                    .any(|(i, (a, _))| i != at && a == abbr)
                {
                    return Err(Fault::new("snippets_duplicate", "snippets"));
                }
                items[at] = (abbr.to_owned(), text);
            }
            _ => {
                let abbr = value.as_str().ok_or_else(invalid)?;
                items.retain(|(a, _)| a != abbr);
            }
        }
        let list: Vec<Value> = items.iter().map(|(abbr, text)| json!({ "abbr": abbr, "text": text, "shadowed_by": rules::shadowed_by(&items, abbr) })).collect();
        self.write("snippets.items", Value::Array(list));
        Ok(Effect::None)
    }

    /// A calendar or list chosen or left out, wherever it is listed.
    fn include(&mut self, value: &Value) -> Result<Effect, Fault> {
        let (Some(id), Some(on)) = (
            text_of(value, "id"),
            value.get("on").and_then(Value::as_bool),
        ) else {
            return Err(Fault::new("invalid_value", "calendar.include"));
        };
        for list in ["calendar.calendars", "calendar.lists"] {
            let mut items = self.list(list);
            for item in &mut items {
                if item["id"] == id {
                    item["included"] = json!(on);
                }
            }
            self.write(list, Value::Array(items));
        }
        Ok(Effect::None)
    }

    /// The diagram of Network: where its boxes are, the connections it has seen, the wires from apps, the circle of connections
    /// that work as one, and the marks of connections. The traffic these rules act on is the engine's; here they are kept.
    fn network(&mut self, op: &str, value: &Value) -> Result<Effect, Fault> {
        let invalid = || Fault::new("invalid_value", op);
        let whole = |name: &str, most: f64| {
            value
                .get(name)
                .and_then(Value::as_f64)
                .filter(|n| n.is_finite())
                .map_or(0, |n| n.clamp(0.0, most).round() as u64)
        };
        match op {
            "network.place" => {
                let (Some(id), Some(x), Some(y)) = (
                    text_of(value, "id"),
                    value.get("x").and_then(Value::as_f64),
                    value.get("y").and_then(Value::as_f64),
                ) else {
                    return Err(invalid());
                };
                if id.is_empty() || id.len() > 200 || !x.is_finite() || !y.is_finite() {
                    return Err(invalid());
                }
                let mut layout = get(&self.state, "network.layout")
                    .and_then(Value::as_object)
                    .cloned()
                    .unwrap_or_default();
                if layout.len() >= MOST_ENTRIES && !layout.contains_key(id) {
                    return Err(invalid());
                }
                layout.insert(id.to_owned(), json!([x.round() as i64, y.round() as i64]));
                self.write("network.layout", Value::Object(layout));
            }
            "network.known_link" => {
                let (Some(name), Some(label)) = (text_of(value, "name"), text_of(value, "label"))
                else {
                    return Err(invalid());
                };
                if name.is_empty() || name.len() > 64 || label.len() > 200 {
                    return Err(invalid());
                }
                let mut links = self.list("network.known_links");
                match links.iter().position(|l| l["name"] == name) {
                    Some(at) => links[at]["label"] = json!(label),
                    None if links.len() < MOST_ENTRIES => {
                        links.push(json!({ "name": name, "label": label }))
                    }
                    None => return Err(invalid()),
                }
                self.write("network.known_links", Value::Array(links));
            }
            "network.wire" | "network.rule" => {
                let (Some(app), Some(target)) = (text_of(value, "app"), text_of(value, "target"))
                else {
                    return Err(invalid());
                };
                let target_ok = matches!(target, "system" | "block" | "merge")
                    || target
                        .strip_prefix("link:")
                        .is_some_and(|name| !name.is_empty() && name.len() <= 64);
                if app.is_empty() || app.len() > 200 || !target_ok {
                    return Err(invalid());
                }
                let mut wires = self.list("network.wires");
                let existing = wires.iter().position(|w| {
                    w["app"]
                        .as_str()
                        .is_some_and(|a| a.eq_ignore_ascii_case(app))
                });
                let mut wire = existing
                    .map(|i| wires[i].clone())
                    .unwrap_or_else(|| json!({ "app": app, "target": target, "priority": "normal", "down_kbps": 0, "up_kbps": 0, "quota_mb": 0, "quota_period": "month" }));
                wire["target"] = json!(target);
                if op == "network.rule" {
                    let priority = text_of(value, "priority").unwrap_or("normal");
                    let period = text_of(value, "quota_period").unwrap_or("month");
                    if !["high", "normal", "low"].contains(&priority)
                        || !["day", "month"].contains(&period)
                    {
                        return Err(invalid());
                    }
                    wire["priority"] = json!(priority);
                    wire["quota_period"] = json!(period);
                    wire["down_kbps"] = json!(whole("down_kbps", 1_000_000.0));
                    wire["up_kbps"] = json!(whole("up_kbps", 1_000_000.0));
                    wire["quota_mb"] = json!(whole("quota_mb", 10_000_000.0));
                }
                match existing {
                    Some(i) => wires[i] = wire,
                    None if wires.len() < MOST_ENTRIES => wires.push(wire),
                    None => return Err(invalid()),
                }
                self.write("network.wires", Value::Array(wires));
            }
            "network.unwire" => {
                let app = value.as_str().ok_or_else(invalid)?;
                let mut wires = self.list("network.wires");
                wires.retain(|w| {
                    !w["app"]
                        .as_str()
                        .is_some_and(|a| a.eq_ignore_ascii_case(app))
                });
                self.write("network.wires", Value::Array(wires));
            }
            "network.merge" => {
                let names: Vec<&str> = value
                    .as_array()
                    .ok_or_else(invalid)?
                    .iter()
                    .filter_map(Value::as_str)
                    .collect();
                let mut kept: Vec<&str> = Vec::new();
                for name in &names {
                    if name.is_empty() || name.len() > 64 {
                        return Err(invalid());
                    }
                    if !kept.contains(name) {
                        kept.push(*name);
                    }
                }
                // A circle joins two connections at least, up to four; none splits it.
                if value.as_array().map(Vec::len) != Some(names.len())
                    || kept.len() == 1
                    || kept.len() > 4
                {
                    return Err(invalid());
                }
                self.write("network.merge", json!(kept));
            }
            _ => {
                let (Some(name), Some(state)) = (text_of(value, "name"), text_of(value, "state"))
                else {
                    return Err(invalid());
                };
                if name.is_empty()
                    || name.len() > 64
                    || !["auto", "metered", "not"].contains(&state)
                {
                    return Err(invalid());
                }
                let mut marked = self.list_of_text("network.metered.marked");
                let mut unmarked = self.list_of_text("network.metered.unmarked");
                marked.retain(|n| n != name);
                unmarked.retain(|n| n != name);
                match state {
                    "metered" => marked.push(name.to_owned()),
                    "not" => unmarked.push(name.to_owned()),
                    _ => {}
                }
                self.write("network.metered.marked", json!(marked));
                self.write("network.metered.unmarked", json!(unmarked));
            }
        }
        Ok(Effect::None)
    }

    fn list_of_text(&self, path: &str) -> Vec<String> {
        get(&self.state, path)
            .and_then(Value::as_array)
            .map(|items| {
                items
                    .iter()
                    .filter_map(|v| v.as_str().map(str::to_owned))
                    .collect()
            })
            .unwrap_or_default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scratch(name: &str) -> PathBuf {
        let dir =
            std::env::temp_dir().join(format!("figghost-tauri-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        dir
    }

    fn set(path: &str, value: Value) -> Message {
        Message::Set {
            path: path.into(),
            value,
        }
    }

    fn fault(code: &'static str, detail: &str) -> Result<Effect, Fault> {
        Err(Fault::new(code, detail))
    }

    #[test]
    fn a_setting_is_kept_in_range_and_in_the_file() {
        let dir = scratch("range");
        let (mut store, note) = Store::open(&dir);
        assert!(note.is_none());
        assert_eq!(
            store.handle(&set("mouse.step_px", json!("wide"))),
            fault("invalid_value", "mouse.step_px")
        );
        assert_eq!(
            store.handle(&set("mouse.nothing", json!(1))),
            fault("unknown_setting", "mouse.nothing")
        );
        assert_eq!(
            store.handle(&set("env.accessibility", json!(true))),
            fault("unknown_setting", "env.accessibility")
        );
        assert_eq!(
            store.handle(&set("mouse.apps", json!([]))),
            fault("unknown_setting", "mouse.apps")
        );
        store.handle(&set("mouse.step_px", json!(5000))).unwrap();
        assert_eq!(get(store.state(), "mouse.step_px"), Some(&json!(400.0)));
        store.save().unwrap();
        let (again, _) = Store::open(&dir);
        assert_eq!(get(again.state(), "mouse.step_px"), Some(&json!(400.0)));
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn the_theme_the_language_and_the_menu_icon_tell_what_to_do() {
        let (mut store, _) = Store::open(&scratch("words"));
        assert_eq!(
            store.handle(&set("general.theme", json!("dark"))),
            Ok(Effect::Theme)
        );
        assert_eq!(
            store.handle(&set("general.language", json!("vi"))),
            Ok(Effect::Language)
        );
        assert_eq!(
            store.handle(&set("general.menu_icon", json!(false))),
            Ok(Effect::MenuIcon(false))
        );
        assert!(store
            .handle(&set("general.theme", json!("purple")))
            .is_err());
        assert_eq!(store.theme(), "dark");
        assert_eq!(store.language("en-US"), "vi");
    }

    #[test]
    fn a_file_that_cannot_be_read_is_moved_aside_and_one_with_wrong_values_is_cleaned() {
        let dir = scratch("bad");
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join(FILE), "{ not json").unwrap();
        let (store, note) = Store::open(&dir);
        assert!(note.unwrap().contains("moved to"));
        assert!(dir.join("settings.json.bad").exists());
        assert_eq!(get(store.state(), "mouse.smooth"), Some(&json!(true)));
        let wrong = json!({ "version": 1, "overrides": { "mouse.step_px": "wide", "mouse.speed": 99, "tiling.layouts": [{ "key": "left", "shortcut": "cmd+9" }], "nothing.here": 1 } });
        fs::write(dir.join(FILE), wrong.to_string()).unwrap();
        let (store, note) = Store::open(&dir);
        assert!(note.is_none());
        assert_eq!(get(store.state(), "mouse.step_px"), Some(&json!(36)));
        assert_eq!(get(store.state(), "mouse.speed"), Some(&json!(10.0)));
        assert_eq!(
            get(store.state(), "tiling.layouts.0.shortcut"),
            Some(&json!("cmd+9"))
        );
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn an_app_added_to_the_rules_does_the_opposite_of_the_global_switch() {
        let (mut store, _) = Store::open(&scratch("apps"));
        store
            .handle(&Message::AppAdd {
                id: "com.example.App".into(),
            })
            .unwrap();
        let apps = store.list("mouse.apps");
        let added = apps.iter().find(|a| a["id"] == "com.example.App").unwrap();
        assert_eq!(added["smooth"], json!(!store.flag("mouse.smooth")));
        // A rule that says nothing goes.
        store
            .handle(&Message::AppRule {
                id: "com.example.App".into(),
                field: RuleField::Smooth,
                value: None,
            })
            .unwrap();
        assert!(!store
            .list("mouse.apps")
            .iter()
            .any(|a| a["id"] == "com.example.App"));
        assert_eq!(
            store.handle(&Message::AppAdd {
                id: "../etc".into()
            }),
            fault("invalid_app_id", "../etc")
        );
        store
            .handle(&Message::IgnoredAdd {
                id: "com.example.Vault".into(),
            })
            .unwrap();
        assert!(store
            .list("clipboard.ignored")
            .iter()
            .any(|a| a["id"] == "com.example.Vault"));
        store
            .handle(&Message::IgnoredRemove {
                id: "com.example.Vault".into(),
            })
            .unwrap();
        assert!(!store
            .list("clipboard.ignored")
            .iter()
            .any(|a| a["id"] == "com.example.Vault"));
    }

    #[test]
    fn a_shortcut_is_checked_the_way_the_current_app_checks_it() {
        let (mut store, _) = Store::open(&scratch("keys"));
        // The same keys in another order and other names are the same shortcut.
        assert_eq!(
            store.handle(&Message::Hotkey {
                target: "keys:pin".into(),
                value: "Shift+Command+C".into()
            }),
            fault("conflict", "clipboard")
        );
        assert_eq!(
            store.handle(&Message::Hotkey {
                target: "keys:pin".into(),
                value: "c".into()
            }),
            fault("needs_modifier", "keys:pin")
        );
        assert_eq!(
            store.handle(&Message::Hotkey {
                target: "keys:pin".into(),
                value: "cmd+".into()
            }),
            fault("invalid_shortcut", "keys:pin")
        );
        assert_eq!(
            store.handle(&Message::Hotkey {
                target: "keys:pin".into(),
                value: "cmd+9".into()
            }),
            fault("conflict", "tiling:grid_9")
        );
        assert_eq!(
            store.handle(&Message::Hotkey {
                target: "keys:pin".into(),
                value: "cmd+y".into()
            }),
            fault("reserved", "keys:pin")
        );
        assert_eq!(
            store.handle(&Message::Hotkey {
                target: "keys:pin".into(),
                value: String::new()
            }),
            fault("shortcut_required", "keys:pin")
        );
        assert_eq!(
            store.handle(&Message::Hotkey {
                target: "clipboard".into(),
                value: String::new()
            }),
            fault("shortcut_required", "clipboard")
        );
        assert_eq!(
            store.handle(&Message::Hotkey {
                target: "nothing".into(),
                value: "cmd+y".into()
            }),
            fault("unknown_setting", "nothing")
        );
        store
            .handle(&Message::Hotkey {
                target: "keys:pin".into(),
                value: "opt+9".into(),
            })
            .unwrap();
        assert_eq!(
            get(store.state(), "keys.shortcuts.pin"),
            Some(&json!("opt+9"))
        );
        // A shortcut can be set again to what it is, and a feature's can go.
        store
            .handle(&Message::Hotkey {
                target: "keys:pin".into(),
                value: "opt+9".into(),
            })
            .unwrap();
        store
            .handle(&Message::Hotkey {
                target: "smart".into(),
                value: String::new(),
            })
            .unwrap();
        assert_eq!(get(store.state(), "smart.shortcut"), Some(&json!("")));
        // The one shortcut of Breaks is named for what it does.
        store
            .handle(&Message::Hotkey {
                target: "breaks:now".into(),
                value: "ctrl+opt+cmd+e".into(),
            })
            .unwrap();
        assert_eq!(
            get(store.state(), "breaks.shortcut"),
            Some(&json!("ctrl+opt+cmd+e"))
        );
        assert_eq!(
            get(store.state(), "pomodoro.shortcuts.toggle"),
            Some(&json!(""))
        );
    }

    #[test]
    fn the_layouts_of_tiling_can_lose_their_shortcut_and_get_them_back() {
        let dir = scratch("tiling");
        let (mut store, _) = Store::open(&dir);
        store
            .handle(&Message::Hotkey {
                target: "tiling:left".into(),
                value: String::new(),
            })
            .unwrap();
        store
            .handle(&Message::Hotkey {
                target: "tiling:right".into(),
                value: "ctrl+opt+cmd+9".into(),
            })
            .unwrap();
        assert_eq!(
            get(store.state(), "tiling.layouts.0.shortcut"),
            Some(&json!(""))
        );
        assert_eq!(
            get(store.state(), "tiling.layouts.1.shortcut"),
            Some(&json!("ctrl+opt+cmd+9"))
        );
        store.save().unwrap();
        let (mut again, _) = Store::open(&dir);
        assert_eq!(
            get(again.state(), "tiling.layouts.1.shortcut"),
            Some(&json!("ctrl+opt+cmd+9"))
        );
        again
            .handle(&Message::Action {
                name: "reset_tiling".into(),
            })
            .unwrap();
        assert_eq!(
            get(again.state(), "tiling.layouts.0.shortcut"),
            Some(&json!("ctrl+opt+left"))
        );
        assert_eq!(
            get(again.state(), "tiling.layouts.1.shortcut"),
            Some(&json!("ctrl+opt+right"))
        );
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn the_sample_and_the_tiling_crate_agree_and_no_two_shortcuts_are_the_same() {
        let sample: Value = serde_json::from_str(SAMPLE).unwrap();
        let from_crate = rules::tiling_layouts(None);
        let names = |layouts: &Value| -> Vec<(String, String)> {
            layouts
                .as_array()
                .unwrap()
                .iter()
                .map(|l| {
                    (
                        l["key"].as_str().unwrap().to_owned(),
                        l["default"].as_str().unwrap().to_owned(),
                    )
                })
                .collect()
        };
        assert_eq!(
            names(&sample["tiling"]["layouts"]),
            names(&from_crate),
            "regenerate src/data/state.sample.json"
        );
        let (store, _) = Store::open(&scratch("same"));
        let chords: Vec<_> = slots(store.state())
            .into_iter()
            .filter(|s| !s.value.is_empty())
            .map(|s| (rules::parse_shortcut(&s.value), s.target))
            .collect();
        for (i, (chord, target)) in chords.iter().enumerate() {
            assert!(
                chord.is_ok(),
                "{target} holds a shortcut that cannot be read"
            );
            assert!(
                chords[i + 1..].iter().all(|(other, _)| other != chord),
                "{target} shares its shortcut"
            );
        }
    }

    #[test]
    fn the_tabs_that_show_come_first_in_the_order_sent() {
        let (mut store, _) = Store::open(&scratch("tabs"));
        store
            .handle(&set("notch.pages", json!(["media", "clipboard"])))
            .unwrap();
        let tabs = store.list("notch.pages");
        assert_eq!(tabs[0]["id"], "media");
        assert_eq!(tabs[1]["id"], "clipboard");
        assert_eq!(tabs[0]["on"], json!(true));
        assert!(tabs[2..].iter().all(|t| t["on"] == json!(false)));
        assert!(store.handle(&set("notch.pages", json!([]))).is_err());
        assert!(store
            .handle(&set("notch.pages", json!(["nothing"])))
            .is_err());
        assert!(store
            .handle(&set("notch.pages", json!(["media", "media"])))
            .is_err());
        assert!(store
            .handle(&set("notch.pages", json!(["media", 7])))
            .is_err());
    }

    #[test]
    fn snippets_follow_the_rules_of_the_current_app() {
        let (mut store, _) = Store::open(&scratch("snippets"));
        let add =
            |abbr: &str, text: &str| set("snippets.add", json!({ "abbr": abbr, "text": text }));
        assert_eq!(store.handle(&add(";new", "A new text")), Ok(Effect::None));
        assert_eq!(
            store.handle(&add(";new", "Again")),
            fault("snippets_duplicate", "snippets")
        );
        assert_eq!(
            store.handle(&add("a b", "x")),
            fault("snippets_space", "snippets")
        );
        assert_eq!(
            store.handle(&add("", "x")),
            fault("snippets_empty", "snippets")
        );
        assert_eq!(
            store.handle(&add("\u{111}", "x")),
            fault("snippets_untypeable", "snippets")
        );
        assert_eq!(
            store.handle(&add(";empty", "")),
            fault("snippets_no_text", "snippets")
        );
        assert_eq!(
            store.handle(&add(&"a".repeat(33), "x")),
            fault("snippets_long", "snippets")
        );
        store
            .handle(&set(
                "snippets.update",
                json!({ "abbr": ";newer", "text": "Changed\r\nline", "from": ";new" }),
            ))
            .unwrap();
        let items = store.list("snippets.items");
        assert!(items
            .iter()
            .any(|i| i["abbr"] == ";newer" && i["text"] == "Changed\nline"));
        // `;zz` is finished before `;zzy` is, so it gets in the way.
        store.handle(&add(";zz", "short")).unwrap();
        store.handle(&add(";zzy", "long")).unwrap();
        let by = |abbr: &str| {
            store
                .list("snippets.items")
                .iter()
                .find(|i| i["abbr"] == abbr)
                .unwrap()["shadowed_by"]
                .clone()
        };
        assert_eq!(by(";zzy"), json!(";zz"));
        assert_eq!(by(";zz"), json!(""));
        assert_eq!(by(";tyx"), json!(";ty"), "the sample already has one");
        store
            .handle(&set("snippets.remove", json!(";newer")))
            .unwrap();
        assert!(!store
            .list("snippets.items")
            .iter()
            .any(|i| i["abbr"] == ";newer"));
    }

    #[test]
    fn the_diagram_of_network_keeps_its_boxes_its_wires_and_its_circle() {
        let (mut store, _) = Store::open(&scratch("network"));
        store
            .handle(&set(
                "network.place",
                json!({ "id": "app:Safari", "x": 120.4, "y": 200 }),
            ))
            .unwrap();
        assert_eq!(
            get(store.state(), "network.layout").unwrap()["app:Safari"],
            json!([120, 200])
        );
        store
            .handle(&set(
                "network.wire",
                json!({ "app": "Safari", "target": "link:en0" }),
            ))
            .unwrap();
        store.handle(&set("network.rule", json!({ "app": "safari", "target": "block", "priority": "low", "down_kbps": 128, "up_kbps": 0, "quota_mb": 99999999, "quota_period": "day" }))).unwrap();
        let wires = store.list("network.wires");
        let safari: Vec<&Value> = wires.iter().filter(|w| w["app"] == "Safari").collect();
        assert_eq!(safari.len(), 1);
        assert_eq!(safari[0]["target"], "block");
        assert_eq!(safari[0]["down_kbps"], 128);
        assert_eq!(safari[0]["quota_mb"], 10_000_000);
        assert!(store
            .handle(&set(
                "network.rule",
                json!({ "app": "Safari", "target": "block", "priority": "urgent" })
            ))
            .is_err());
        assert!(store
            .handle(&set(
                "network.wire",
                json!({ "app": "Safari", "target": "nowhere" })
            ))
            .is_err());
        store
            .handle(&set("network.unwire", json!("safari")))
            .unwrap();
        assert!(!store
            .list("network.wires")
            .iter()
            .any(|w| w["app"] == "Safari"));
        store
            .handle(&set("network.merge", json!(["en0", "en7", "en0"])))
            .unwrap();
        assert_eq!(
            get(store.state(), "network.merge"),
            Some(&json!(["en0", "en7"]))
        );
        assert!(store.handle(&set("network.merge", json!(["en0"]))).is_err());
        store.handle(&set("network.merge", json!([]))).unwrap();
        store
            .handle(&set(
                "network.metered.mark",
                json!({ "name": "en0", "state": "metered" }),
            ))
            .unwrap();
        store
            .handle(&set(
                "network.metered.mark",
                json!({ "name": "en7", "state": "not" }),
            ))
            .unwrap();
        assert_eq!(
            get(store.state(), "network.metered.marked"),
            Some(&json!(["en0"]))
        );
        assert_eq!(
            get(store.state(), "network.metered.unmarked"),
            Some(&json!(["en7"]))
        );
        store
            .handle(&set(
                "network.metered.mark",
                json!({ "name": "en0", "state": "auto" }),
            ))
            .unwrap();
        assert_eq!(
            get(store.state(), "network.metered.marked"),
            Some(&json!([]))
        );
    }

    #[test]
    fn a_calendar_is_included_wherever_it_is_listed() {
        let (mut store, _) = Store::open(&scratch("calendar"));
        store.write(
            "calendar.calendars",
            json!([{ "id": "a", "included": true }, { "id": "b", "included": true }]),
        );
        store
            .handle(&set("calendar.include", json!({ "id": "b", "on": false })))
            .unwrap();
        let calendars = store.list("calendar.calendars");
        assert_eq!(calendars[0]["included"], json!(true));
        assert_eq!(calendars[1]["included"], json!(false));
        assert!(store
            .handle(&set("calendar.include", json!({ "id": "b" })))
            .is_err());
    }

    #[test]
    fn the_login_item_is_for_the_host_and_a_secret_is_not_kept_in_the_file() {
        let (mut store, _) = Store::open(&scratch("special"));
        assert_eq!(
            store.handle(&set("launch_at_login", json!(true))),
            Ok(Effect::LoginItem(true))
        );
        assert!(store.handle(&set("launch_at_login", json!("yes"))).is_err());
        assert_eq!(
            store.handle(&set("jarvis.key", json!("sk-secret"))),
            fault("unsupported", "jarvis.key")
        );
        assert!(!serde_json::to_string(&store.overrides)
            .unwrap()
            .contains("secret"));
    }
}
