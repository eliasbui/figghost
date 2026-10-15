//! Features that live in a folder of their own, so that several can be built at the same time without touching
//! the same files. Shared code (the settings model and window, the menu, the composition root) reaches them
//! only through this file. A feature owns everything under `features/<name>/`, its page, strings and styles
//! under `web/features/<name>.*`, and its part of the macOS layer in `macos-sys`.

use crate::{
    clipboard::Clipboard, config::Config, media::MediaFeature, settings::model::Fault,
    tiling::Tiling,
};
use macos_sys::{MainThreadMarker, notch::Notch, status_item::Entry};
use serde_json::{Map, Value};
use std::{collections::HashMap, path::Path, rc::Rc};

pub mod calendar;
pub mod capture;
pub mod lyrics;
pub mod menubar;
pub mod smart;
pub mod snap;
pub mod stack;
pub mod vision;

/// What a feature may need when it starts. Each feature reads the fields it needs.
#[allow(dead_code)]
pub struct AttachEnv<'a> {
    pub mtm: MainThreadMarker,
    /// `None` when the notch cannot be shown on this machine.
    pub notch: Option<&'a Rc<Notch>>,
    /// The data directory.
    pub home: &'a Path,
    pub config: &'a Config,
    pub clipboard: Option<&'a Rc<Clipboard>>,
    pub tiling: &'a Rc<Tiling>,
    /// The media tab, `None` without a notch.
    pub media: Option<&'a Rc<MediaFeature>>,
}

/// The running features, kept alive by the app context.
#[derive(Default)]
pub struct Features {
    pub smart: Option<Rc<smart::Feature>>,
    pub capture: Option<Rc<capture::Feature>>,
    pub vision: Option<Rc<vision::Feature>>,
    pub lyrics: Option<Rc<lyrics::Feature>>,
    pub calendar: Option<Rc<calendar::Feature>>,
    pub snap: Option<Rc<snap::Feature>>,
    pub menubar: Option<Rc<menubar::Feature>>,
    pub stack: Option<Rc<stack::Feature>>,
}

impl Features {
    pub fn attach(env: &AttachEnv) -> Self {
        Self {
            smart: smart::attach(env),
            capture: capture::attach(env),
            vision: vision::attach(env),
            lyrics: lyrics::attach(env),
            calendar: calendar::attach(env),
            snap: snap::attach(env),
            menubar: menubar::attach(env),
            stack: stack::attach(env),
        }
    }

    /// Facts only the running app knows, by feature id, for the settings state.
    pub fn runtime(&self) -> HashMap<&'static str, Value> {
        let mut out = HashMap::new();
        if let Some(f) = &self.smart {
            out.insert("smart", f.runtime());
        }
        if let Some(f) = &self.capture {
            out.insert("capture", f.runtime());
        }
        if let Some(f) = &self.vision {
            out.insert("vision", f.runtime());
        }
        if let Some(f) = &self.lyrics {
            out.insert("lyrics", f.runtime());
        }
        if let Some(f) = &self.calendar {
            out.insert("calendar", f.runtime());
        }
        if let Some(f) = &self.snap {
            out.insert("snap", f.runtime());
        }
        if let Some(f) = &self.menubar {
            out.insert("menubar", f.runtime());
        }
        if let Some(f) = &self.stack {
            out.insert("stack", f.runtime());
        }
        out
    }

    /// Applies the settings of one feature after a change.
    pub fn configure(&self, id: &str, config: &Config) -> Result<(), Fault> {
        match id {
            "smart" => self
                .smart
                .as_ref()
                .map_or(Ok(()), |f| f.configure(&config.smart)),
            "capture" => self
                .capture
                .as_ref()
                .map_or(Ok(()), |f| f.configure(&config.capture)),
            "vision" => self
                .vision
                .as_ref()
                .map_or(Ok(()), |f| f.configure(&config.vision)),
            "lyrics" => self
                .lyrics
                .as_ref()
                .map_or(Ok(()), |f| f.configure(&config.lyrics)),
            "calendar" => self
                .calendar
                .as_ref()
                .map_or(Ok(()), |f| f.configure(&config.calendar)),
            "snap" => self
                .snap
                .as_ref()
                .map_or(Ok(()), |f| f.configure(&config.snap)),
            "menubar" => self
                .menubar
                .as_ref()
                .map_or(Ok(()), |f| f.configure(&config.menubar)),
            "stack" => self
                .stack
                .as_ref()
                .map_or(Ok(()), |f| f.configure(&config.stack)),
            _ => Ok(()),
        }
    }

    /// While the settings page records a shortcut, no global shortcut may fire.
    pub fn suspend_hotkeys(&self) {
        if let Some(f) = &self.smart {
            f.suspend_hotkeys();
        }
        if let Some(f) = &self.capture {
            f.suspend_hotkeys();
        }
        if let Some(f) = &self.vision {
            f.suspend_hotkeys();
        }
        if let Some(f) = &self.lyrics {
            f.suspend_hotkeys();
        }
        if let Some(f) = &self.calendar {
            f.suspend_hotkeys();
        }
        if let Some(f) = &self.snap {
            f.suspend_hotkeys();
        }
        if let Some(f) = &self.menubar {
            f.suspend_hotkeys();
        }
        if let Some(f) = &self.stack {
            f.suspend_hotkeys();
        }
    }

    pub fn resume_hotkeys(&self, config: &Config) {
        if let Some(f) = &self.smart {
            f.resume_hotkeys(&config.smart);
        }
        if let Some(f) = &self.capture {
            f.resume_hotkeys(&config.capture);
        }
        if let Some(f) = &self.vision {
            f.resume_hotkeys(&config.vision);
        }
        if let Some(f) = &self.lyrics {
            f.resume_hotkeys(&config.lyrics);
        }
        if let Some(f) = &self.calendar {
            f.resume_hotkeys(&config.calendar);
        }
        if let Some(f) = &self.snap {
            f.resume_hotkeys(&config.snap);
        }
        if let Some(f) = &self.menubar {
            f.resume_hotkeys(&config.menubar);
        }
        if let Some(f) = &self.stack {
            f.resume_hotkeys(&config.stack);
        }
    }

    /// A button of the settings page; `None` when no feature knows the name.
    pub fn action(&self, name: &str, config: &mut Config) -> Option<Result<(), Fault>> {
        if let Some(f) = &self.smart
            && let Some(result) = f.action(name, &mut config.smart)
        {
            return Some(result);
        }
        if let Some(f) = &self.capture
            && let Some(result) = f.action(name, &mut config.capture)
        {
            return Some(result);
        }
        if let Some(f) = &self.vision
            && let Some(result) = f.action(name, &mut config.vision)
        {
            return Some(result);
        }
        if let Some(f) = &self.lyrics
            && let Some(result) = f.action(name, &mut config.lyrics)
        {
            return Some(result);
        }
        if let Some(f) = &self.calendar
            && let Some(result) = f.action(name, &mut config.calendar)
        {
            return Some(result);
        }
        if let Some(f) = &self.snap
            && let Some(result) = f.action(name, &mut config.snap)
        {
            return Some(result);
        }
        if let Some(f) = &self.menubar
            && let Some(result) = f.action(name, &mut config.menubar)
        {
            return Some(result);
        }
        if let Some(f) = &self.stack
            && let Some(result) = f.action(name, &mut config.stack)
        {
            return Some(result);
        }
        None
    }

    /// Entries for the menu bar item.
    pub fn menu_entries(&self) -> Vec<Entry> {
        let mut entries = Vec::new();
        if let Some(f) = &self.smart {
            entries.extend(smart::Feature::menu_entries(f));
        }
        if let Some(f) = &self.capture {
            entries.extend(capture::Feature::menu_entries(f));
        }
        if let Some(f) = &self.vision {
            entries.extend(vision::Feature::menu_entries(f));
        }
        if let Some(f) = &self.lyrics {
            entries.extend(lyrics::Feature::menu_entries(f));
        }
        if let Some(f) = &self.calendar {
            entries.extend(calendar::Feature::menu_entries(f));
        }
        if let Some(f) = &self.snap {
            entries.extend(snap::Feature::menu_entries(f));
        }
        if let Some(f) = &self.menubar {
            entries.extend(menubar::Feature::menu_entries(f));
        }
        if let Some(f) = &self.stack {
            entries.extend(stack::Feature::menu_entries(f));
        }
        entries
    }
}

/// Adds each feature's part to the state of the settings page, under its id.
pub fn state(
    config: &Config,
    runtime: &HashMap<&'static str, Value>,
    root: &mut Map<String, Value>,
) {
    let none = Value::Null;
    root.insert(
        "smart".into(),
        smart::settings::state(&config.smart, runtime.get("smart").unwrap_or(&none)),
    );
    root.insert(
        "capture".into(),
        capture::settings::state(&config.capture, runtime.get("capture").unwrap_or(&none)),
    );
    root.insert(
        "vision".into(),
        vision::settings::state(&config.vision, runtime.get("vision").unwrap_or(&none)),
    );
    root.insert(
        "lyrics".into(),
        lyrics::settings::state(&config.lyrics, runtime.get("lyrics").unwrap_or(&none)),
    );
    root.insert(
        "calendar".into(),
        calendar::settings::state(&config.calendar, runtime.get("calendar").unwrap_or(&none)),
    );
    root.insert(
        "snap".into(),
        snap::settings::state(&config.snap, runtime.get("snap").unwrap_or(&none)),
    );
    root.insert(
        "menubar".into(),
        menubar::settings::state(&config.menubar, runtime.get("menubar").unwrap_or(&none)),
    );
    root.insert(
        "stack".into(),
        stack::settings::state(&config.stack, runtime.get("stack").unwrap_or(&none)),
    );
}

/// A change of a setting whose path starts with a feature id. `None` when no feature owns the path; the
/// success value is the id of the feature to configure.
pub fn set(config: &mut Config, path: &str, value: &Value) -> Option<Result<&'static str, Fault>> {
    let (head, rest) = path.split_once('.')?;
    match head {
        "smart" => {
            smart::settings::set(&mut config.smart, rest, value).map(|r| r.map(|()| "smart"))
        }
        "capture" => {
            capture::settings::set(&mut config.capture, rest, value).map(|r| r.map(|()| "capture"))
        }
        "vision" => {
            vision::settings::set(&mut config.vision, rest, value).map(|r| r.map(|()| "vision"))
        }
        "lyrics" => {
            lyrics::settings::set(&mut config.lyrics, rest, value).map(|r| r.map(|()| "lyrics"))
        }
        "calendar" => calendar::settings::set(&mut config.calendar, rest, value)
            .map(|r| r.map(|()| "calendar")),
        "snap" => snap::settings::set(&mut config.snap, rest, value).map(|r| r.map(|()| "snap")),
        "menubar" => {
            menubar::settings::set(&mut config.menubar, rest, value).map(|r| r.map(|()| "menubar"))
        }
        "stack" => {
            stack::settings::set(&mut config.stack, rest, value).map(|r| r.map(|()| "stack"))
        }
        _ => None,
    }
}

/// The id of the feature a shortcut target belongs to: "capture" or "capture:region" belong to `capture`.
pub fn shortcut_owner(target: &str) -> Option<&'static str> {
    let head = target.split(':').next()?;
    [
        "smart", "capture", "vision", "lyrics", "calendar", "snap", "menubar", "stack",
    ]
    .into_iter()
    .find(|id| *id == head)
}

/// Every shortcut of every feature as (target, shortcut).
pub fn shortcuts(config: &Config) -> Vec<(String, String)> {
    let mut all = Vec::new();
    all.extend(smart::settings::shortcuts(&config.smart));
    all.extend(capture::settings::shortcuts(&config.capture));
    all.extend(vision::settings::shortcuts(&config.vision));
    all.extend(lyrics::settings::shortcuts(&config.lyrics));
    all.extend(calendar::settings::shortcuts(&config.calendar));
    all.extend(snap::settings::shortcuts(&config.snap));
    all.extend(menubar::settings::shortcuts(&config.menubar));
    all.extend(stack::settings::shortcuts(&config.stack));
    all
}

/// Stores a shortcut for a target that `shortcut_owner` recognised.
pub fn set_shortcut(
    config: &mut Config,
    target: &str,
    value: &str,
) -> Option<Result<&'static str, Fault>> {
    match shortcut_owner(target)? {
        "smart" => smart::settings::set_shortcut(&mut config.smart, target, value)
            .map(|r| r.map(|()| "smart")),
        "capture" => capture::settings::set_shortcut(&mut config.capture, target, value)
            .map(|r| r.map(|()| "capture")),
        "vision" => vision::settings::set_shortcut(&mut config.vision, target, value)
            .map(|r| r.map(|()| "vision")),
        "lyrics" => lyrics::settings::set_shortcut(&mut config.lyrics, target, value)
            .map(|r| r.map(|()| "lyrics")),
        "calendar" => calendar::settings::set_shortcut(&mut config.calendar, target, value)
            .map(|r| r.map(|()| "calendar")),
        "snap" => snap::settings::set_shortcut(&mut config.snap, target, value)
            .map(|r| r.map(|()| "snap")),
        "menubar" => menubar::settings::set_shortcut(&mut config.menubar, target, value)
            .map(|r| r.map(|()| "menubar")),
        "stack" => stack::settings::set_shortcut(&mut config.stack, target, value)
            .map(|r| r.map(|()| "stack")),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::parse_hotkey;

    /// Every shortcut that is on by default, across the clipboard, the window layouts and the features, is a
    /// different key combination: two tasks must never claim the same one.
    #[test]
    fn default_shortcuts_never_clash() {
        let config = Config::default();
        let mut all: Vec<(String, String)> = vec![("clipboard".into(), config.hotkey.clone())];
        all.extend(
            config
                .tiling
                .bindings()
                .into_iter()
                .map(|(layout, shortcut)| (format!("tiling:{}", layout.key()), shortcut)),
        );
        all.extend(shortcuts(&config));
        let mut seen: Vec<(crate::config::HotkeySpec, String)> = Vec::new();
        for (target, shortcut) in all {
            let spec = parse_hotkey(&shortcut)
                .unwrap_or_else(|e| panic!("{target} has the shortcut {shortcut:?}: {e}"));
            if let Some((_, other)) = seen.iter().find(|(known, _)| *known == spec) {
                panic!("{target} and {other} both use {shortcut}");
            }
            seen.push((spec, target));
        }
    }
}
