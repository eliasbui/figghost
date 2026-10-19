//! The drag wheel: while files are dragged from Finder (or any app that drags file URLs), tapping Shift, Command
//! or Control opens a wheel of slots around the pointer, and dropping the files on a slot runs it.
//!
//! This folder holds what each wheel offers and what each slot does (`catalog`, pure and tested against the real
//! tools), the words (`text`), the settings, and the wiring. The wheel itself, which watches the drag and draws
//! the slots, is `macos_sys::wheel_ui`. Here the two meet: the wheel asks what to show for a kind of key and a
//! set of files, and tells which slot the files were dropped on.

use super::{AttachEnv, capture};
use crate::{
    i18n,
    settings::model::Fault,
    shelf::{self, ShelfFeature},
    shelf_actions,
};
use macos_sys::{
    MainThreadMarker, app, capture_ui, file_ops,
    pasteboard::Write,
    status_item::Entry,
    vision,
    wheel_ui::{self, DragWheel, Keys, Modifier, Slot, Trigger, Wheel},
    workspace,
};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::{cell::RefCell, path::PathBuf, rc::Rc};

pub mod catalog;
pub mod settings;
mod text;

#[cfg(test)]
mod real;

#[cfg(test)]
mod guard {
    /// A folder under the system temp folder that is removed when it goes out of scope, so also when an assertion
    /// fails and the test stops early.
    pub struct TempDir(std::path::PathBuf);

    impl TempDir {
        pub fn new(tag: &str) -> Self {
            static N: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
            let dir = std::env::temp_dir().join(format!(
                "{tag}-{}-{}",
                std::process::id(),
                N.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
            ));
            std::fs::create_dir_all(&dir).unwrap();
            Self(dir)
        }
    }

    impl std::ops::Deref for TempDir {
        type Target = std::path::Path;
        fn deref(&self) -> &std::path::Path {
            &self.0
        }
    }

    impl AsRef<std::path::Path> for TempDir {
        fn as_ref(&self) -> &std::path::Path {
            &self.0
        }
    }

    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }
}

#[cfg(test)]
use guard::TempDir;

use catalog::{MainAction, Plan};

/// The keys a wheel can be bound to: a tap of one of the four modifier keys, alone, opens it, and so does a press
/// of any other key a shortcut can name ("f19", "a", "space"), held by its virtual key code.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Key {
    Shift,
    Command,
    Control,
    Option,
    Other(u32),
}

/// The virtual key code of Escape, which ends the drag and so can never open a wheel.
const ESCAPE: u32 = 53;

impl Key {
    /// The four modifier keys.
    pub const ALL: [Key; 4] = [Key::Shift, Key::Command, Key::Control, Key::Option];

    /// The name of a modifier key; empty for another key, whose name stays as it was written
    /// (`Config::key_name_of`).
    pub const fn as_str(self) -> &'static str {
        match self {
            Key::Shift => "shift",
            Key::Command => "command",
            Key::Control => "control",
            Key::Option => "option",
            Key::Other(_) => "",
        }
    }

    pub fn parse(name: &str) -> Option<Self> {
        Self::ALL
            .into_iter()
            .find(|k| k.as_str() == name)
            .or_else(|| {
                crate::config::key_code(name)
                    .filter(|&code| code != ESCAPE)
                    .map(Key::Other)
            })
    }
}

/// The key a wheel has until the user picks another: Shift converts, Command opens the tools, Control the actions.
const fn default_key(kind: catalog::Kind) -> Key {
    match kind {
        catalog::Kind::Formats => Key::Shift,
        catalog::Kind::Tools => Key::Command,
        catalog::Kind::Actions => Key::Control,
    }
}

/// The settings of one wheel.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct WheelConfig {
    /// A wheel that is off never opens; its key is free for another wheel.
    pub enabled: bool,
    /// "shift", "command", "control" or "option". Anything else, and an empty name, means the wheel's own key.
    pub key: String,
    /// The slots that are not shown. Slots added in a later version are not here, so they show by default.
    pub hidden: Vec<String>,
}

impl Default for WheelConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            key: String::new(),
            hidden: Vec::new(),
        }
    }
}

impl WheelConfig {
    fn with_key(key: Key) -> Self {
        Self {
            key: key.as_str().to_owned(),
            ..Self::default()
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    /// Off until switched on: a drag of files does nothing else unless this is true.
    pub enabled: bool,
    pub formats: WheelConfig,
    pub tools: WheelConfig,
    pub actions: WheelConfig,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            enabled: false,
            formats: WheelConfig::with_key(default_key(catalog::Kind::Formats)),
            tools: WheelConfig::with_key(default_key(catalog::Kind::Tools)),
            actions: WheelConfig::with_key(default_key(catalog::Kind::Actions)),
        }
    }
}

/// The key each wheel answers to after the settings are cleaned; `None` for a wheel that is off.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WheelKeys {
    pub formats: Option<Key>,
    pub tools: Option<Key>,
    pub actions: Option<Key>,
}

impl WheelKeys {
    pub fn of(&self, kind: catalog::Kind) -> Option<Key> {
        match kind {
            catalog::Kind::Formats => self.formats,
            catalog::Kind::Tools => self.tools,
            catalog::Kind::Actions => self.actions,
        }
    }
}

impl Config {
    pub fn wheel(&self, kind: catalog::Kind) -> &WheelConfig {
        match kind {
            catalog::Kind::Formats => &self.formats,
            catalog::Kind::Tools => &self.tools,
            catalog::Kind::Actions => &self.actions,
        }
    }

    /// The key of a wheel as written, or its own key when the name is not one a wheel can have.
    pub fn key_of(&self, kind: catalog::Kind) -> Key {
        Key::parse(&self.wheel(kind).key).unwrap_or(default_key(kind))
    }

    /// The name of the key of a wheel, as the page shows it: the one written, or the wheel's own.
    pub fn key_name_of(&self, kind: catalog::Kind) -> String {
        let written = &self.wheel(kind).key;
        if Key::parse(written).is_some() {
            written.clone()
        } else {
            default_key(kind).as_str().to_owned()
        }
    }

    /// The slots of a wheel that are hidden: only ids the wheel has, each once, in the order of the page.
    pub fn hidden_ids(&self, kind: catalog::Kind) -> Vec<&'static str> {
        let wanted = &self.wheel(kind).hidden;
        let mut out: Vec<&'static str> = Vec::new();
        for (_, id) in catalog::all_ids(kind) {
            if wanted.iter().any(|w| w == id) && !out.contains(&id) {
                out.push(id);
            }
        }
        out
    }

    /// Whether slot `id` of a wheel may be offered and run: its wheel is on and has a key, and the slot is
    /// not hidden.
    pub fn shows(&self, kind: catalog::Kind, id: &str) -> bool {
        keys_from(self).of(kind).is_some() && !self.wheel(kind).hidden.iter().any(|h| h == id)
    }
}

/// The key of each wheel that is on. When two wheels that are on name the same key, the later one in the order
/// formats, tools, actions is treated as off, so that a hand-edited file can never bind a key twice.
pub fn keys_from(config: &Config) -> WheelKeys {
    let mut taken: Vec<Key> = Vec::new();
    let mut pick = |kind: catalog::Kind| {
        if !config.wheel(kind).enabled {
            return None;
        }
        let key = config.key_of(kind);
        if taken.contains(&key) {
            return None;
        }
        taken.push(key);
        Some(key)
    };
    WheelKeys {
        formats: pick(catalog::Kind::Formats),
        tools: pick(catalog::Kind::Tools),
        actions: pick(catalog::Kind::Actions),
    }
}

/// The settings' keys in the form the wheel takes.
fn wheel_keys(keys: WheelKeys) -> Keys {
    let trigger = |key: Option<Key>| {
        key.map(|k| match k {
            Key::Shift => Trigger::Modifier(Modifier::Shift),
            Key::Command => Trigger::Modifier(Modifier::Command),
            Key::Control => Trigger::Modifier(Modifier::Control),
            Key::Option => Trigger::Modifier(Modifier::Option),
            // Every name a shortcut knows has a code below 128.
            Key::Other(code) => Trigger::Key(code as u16),
        })
    };
    Keys {
        formats: trigger(keys.formats),
        tools: trigger(keys.tools),
        actions: trigger(keys.actions),
    }
}

/// The running feature. Created by `attach`, kept alive by the app context.
pub struct Feature {
    mtm: MainThreadMarker,
    /// `None` without a notch: the Float slot is then not offered.
    shelf: Option<Rc<ShelfFeature>>,
    /// Opens an image for Annotate.
    capture: Option<Rc<capture::Feature>>,
    /// The watcher of file drags, alive exactly while the feature is switched on.
    wheel: RefCell<Option<DragWheel>>,
    /// The settings as last applied. The wheel reads them at each drag, so a change needs no restart.
    config: Rc<RefCell<Config>>,
}

/// Starts the feature with the saved settings.
pub fn attach(env: &AttachEnv, capture: Option<&Rc<capture::Feature>>) -> Option<Rc<Feature>> {
    let feature = Rc::new(Feature {
        mtm: env.mtm,
        shelf: env.shelf.cloned(),
        capture: capture.cloned(),
        wheel: RefCell::new(None),
        config: Rc::new(RefCell::new(Config::default())),
    });
    if let Err(fault) = feature.configure(&env.config.wheel) {
        eprintln!("wheel: {}", fault.detail);
    }
    Some(feature)
}

impl Feature {
    /// Applies the settings after a change. An error is shown on the settings page and the change is undone.
    pub fn configure(&self, config: &Config) -> Result<(), Fault> {
        *self.config.borrow_mut() = config.clone();
        if let Some(wheel) = self.wheel.borrow().as_ref() {
            wheel.set_keys(wheel_keys(keys_from(config)));
        }
        if config.enabled && self.wheel.borrow().is_none() {
            let wheel = self.start().ok_or_else(|| {
                Fault::new("wheel_failed", "the system refused the mouse monitor")
            })?;
            *self.wheel.borrow_mut() = Some(wheel);
        } else if !config.enabled {
            // Dropping the watcher takes its monitor and its panel away.
            self.wheel.borrow_mut().take();
        }
        Ok(())
    }

    /// Starts watching file drags. The closures hold what they need and not the feature, which owns the wheel.
    fn start(&self) -> Option<DragWheel> {
        let has_shelf = self.shelf.is_some();
        let (mtm, shelf, capture) = (self.mtm, self.shelf.clone(), self.capture.clone());
        let (for_offer, for_run) = (Rc::clone(&self.config), Rc::clone(&self.config));
        DragWheel::start(
            self.mtm,
            wheel_keys(keys_from(&self.config.borrow())),
            move |kind, paths| offer(kind, paths, has_shelf, &for_offer.borrow()),
            move |_, id, paths| {
                run(
                    mtm,
                    shelf.as_deref(),
                    capture.as_deref(),
                    &for_run.borrow(),
                    id,
                    paths,
                )
            },
        )
    }

    /// Releases the global shortcuts while the settings page records a new one. There are none.
    pub fn suspend_hotkeys(&self) {}

    /// Registers the global shortcuts again from the config. There are none.
    pub fn resume_hotkeys(&self, _config: &Config) {}

    /// Facts only the running app knows, merged into this feature's part of the page state.
    pub fn runtime(&self) -> Value {
        json!({})
    }

    /// A button of the settings page. `None` when the name is not this feature's.
    pub fn action(&self, _name: &str, _config: &mut Config) -> Option<Result<(), Fault>> {
        None
    }

    /// Entries for the menu bar item. The feature has none: the menu stays as it is.
    pub fn menu_entries(_this: &Rc<Self>) -> Vec<Entry> {
        Vec::new()
    }
}

fn catalog_kind(kind: wheel_ui::Kind) -> catalog::Kind {
    match kind {
        wheel_ui::Kind::Formats => catalog::Kind::Formats,
        wheel_ui::Kind::Tools => catalog::Kind::Tools,
        wheel_ui::Kind::Actions => catalog::Kind::Actions,
    }
}

/// What the wheel shows for a key and some files: the slots the catalog offers, with the caption of the hub.
/// `None` when nothing applies, so that the wheel does not open: the wheel is off, or every slot that fits is
/// hidden.
fn offer(
    kind: wheel_ui::Kind,
    paths: &[PathBuf],
    has_shelf: bool,
    config: &Config,
) -> Option<Wheel> {
    let language = i18n::current();
    let kind = catalog_kind(kind);
    let mut offers = catalog::offers_with(kind, paths, language, &|k, id| config.shows(k, id));
    if !has_shelf {
        // Floating needs the shelf, which lives in the notch.
        offers.retain(|offer| offer.id != "float");
    }
    if offers.is_empty() {
        return None;
    }
    Some(Wheel {
        slots: offers
            .into_iter()
            .map(|offer| Slot {
                id: offer.id.to_owned(),
                title: offer.title,
                symbol: offer.symbol.map(str::to_owned),
            })
            .collect(),
        caption: language.wheel_caption(kind, paths.len()),
    })
}

/// Shows a notice from any thread.
fn toast(text: String) {
    app::on_main(move || {
        if let Some(mtm) = MainThreadMarker::new() {
            capture_ui::toast(mtm, &text);
        }
    });
}

/// What dropping the files on slot `id` does.
fn run(
    mtm: MainThreadMarker,
    shelf: Option<&ShelfFeature>,
    capture: Option<&capture::Feature>,
    config: &Config,
    id: &str,
    paths: Vec<PathBuf>,
) {
    // The drop is not trusted: an id that was not on offer for these files, or that the settings hide, is
    // nothing.
    let Some(plan) = catalog::plan_with(id, &paths, &|k, i| config.shows(k, i)) else {
        return;
    };
    match plan {
        Plan::Jobs(jobs) => {
            // The tools can take minutes; every file is tried, and the notice says how many were made.
            std::thread::spawn(move || {
                let mut made = 0;
                let mut failure = None;
                for job in &jobs {
                    match shelf_actions::run_published(job) {
                        Ok(_) => made += 1,
                        Err(e) => {
                            eprintln!("wheel: {e}");
                            failure.get_or_insert(e);
                        }
                    }
                }
                let language = i18n::current();
                toast(match failure {
                    None => language.wheel_made(made),
                    Some(e) if made == 0 => language.wheel_failed(&e),
                    Some(e) => language.wheel_partial(made, &e),
                });
            });
        }
        Plan::Main(action) => main_action(mtm, shelf, capture, action),
    }
}

/// The slots that cannot run on a background thread.
fn main_action(
    mtm: MainThreadMarker,
    shelf: Option<&ShelfFeature>,
    capture: Option<&capture::Feature>,
    action: MainAction,
) {
    let language = i18n::current();
    let fail = |detail: &str| capture_ui::toast(mtm, &language.wheel_failed(detail));
    match action {
        MainAction::CopyPath(text) => {
            let count = text.lines().count();
            shelf::pasteboard().write(Write::Text(&text));
            capture_ui::toast(mtm, &language.wheel_copied_paths(count));
        }
        MainAction::CopyFiles(paths) => {
            if file_ops::copy_files(&paths) {
                capture_ui::toast(mtm, &language.wheel_copied_files(paths.len()));
            } else {
                fail("the clipboard refused the files");
            }
        }
        MainAction::AirDrop(paths) => {
            if let Err(e) = file_ops::airdrop(mtm, &paths) {
                fail(&e);
            }
        }
        MainAction::Terminal(folder) => {
            if let Err(e) = file_ops::open_terminal(&folder) {
                fail(&e);
            }
        }
        MainAction::Reveal(paths) => {
            for path in &paths {
                workspace::reveal(path);
            }
        }
        MainAction::Float(paths) => match shelf {
            Some(shelf) => {
                let added = shelf.add_files(&paths);
                capture_ui::toast(mtm, &language.wheel_floated(added));
            }
            None => capture_ui::toast(mtm, language.wheel_no_shelf()),
        },
        MainAction::Annotate(path) => match capture {
            Some(capture) => {
                if let Err(e) = capture.annotate(&path) {
                    fail(&e);
                }
            }
            None => fail("the capture feature is not running"),
        },
        MainAction::CopyText(path) => shelf::start_text_copy(&path.to_string_lossy()),
        MainAction::RemoveBackground(path) => {
            if !vision::background_removal_available() {
                fail("this needs macOS 14");
                return;
            }
            std::thread::spawn(move || match shelf_actions::cut_out(&path) {
                Ok(_) => toast(i18n::current().wheel_made(1)),
                Err(e) => {
                    eprintln!("wheel: {e}");
                    toast(i18n::current().wheel_failed(&e));
                }
            });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn it_is_off_by_default_with_every_wheel_on_and_nothing_hidden() {
        let config = Config::default();
        assert!(!config.enabled);
        for (kind, key) in [
            (catalog::Kind::Formats, "shift"),
            (catalog::Kind::Tools, "command"),
            (catalog::Kind::Actions, "control"),
        ] {
            let wheel = config.wheel(kind);
            assert!(wheel.enabled && wheel.hidden.is_empty(), "{kind:?}");
            assert_eq!(wheel.key, key);
        }
        assert_eq!(
            keys_from(&config),
            WheelKeys {
                formats: Some(Key::Shift),
                tools: Some(Key::Command),
                actions: Some(Key::Control)
            }
        );
    }

    #[test]
    fn an_older_file_with_only_the_switch_keeps_the_default_keys() {
        let config: Config = serde_json::from_str(r#"{ "enabled": true }"#).unwrap();
        assert!(config.enabled);
        assert_eq!(
            config,
            Config {
                enabled: true,
                ..Config::default()
            }
        );
        // A wheel written without its key gets its own key, not another wheel's.
        let config: Config =
            serde_json::from_str(r#"{ "tools": { "enabled": false }, "actions": {} }"#).unwrap();
        assert!(!config.tools.enabled);
        assert_eq!(config.key_of(catalog::Kind::Tools), Key::Command);
        assert_eq!(config.key_of(catalog::Kind::Actions), Key::Control);
    }

    #[test]
    fn an_unknown_key_name_falls_back_to_the_wheels_own_key() {
        let config: Config = serde_json::from_str(
            r#"{ "formats": { "key": "Hyper" }, "tools": { "key": "SHIFT" } }"#,
        )
        .unwrap();
        assert_eq!(config.key_of(catalog::Kind::Tools), Key::Command);
        assert_eq!(config.key_of(catalog::Kind::Formats), Key::Shift);
        let config: Config = serde_json::from_str(r#"{ "formats": { "key": "" } }"#).unwrap();
        assert_eq!(config.key_of(catalog::Kind::Formats), Key::Shift);
        let config: Config = serde_json::from_str(r#"{ "actions": { "key": "option" } }"#).unwrap();
        assert_eq!(config.key_of(catalog::Kind::Actions), Key::Option);
    }

    #[test]
    fn unknown_and_repeated_hidden_ids_are_dropped_when_used() {
        let mut config = Config::default();
        config.tools.hidden = ["quarter", "bogus", "half", "half", "../x", "to_png"]
            .map(String::from)
            .to_vec();
        assert_eq!(config.hidden_ids(catalog::Kind::Tools), ["half", "quarter"]);
        assert!(!config.shows(catalog::Kind::Tools, "half"));
        assert!(config.shows(catalog::Kind::Tools, "compress"));
        // A slot added in a later version is not in the file, so it shows.
        assert!(config.shows(catalog::Kind::Tools, "something_new"));
    }

    #[test]
    fn two_wheels_on_one_key_leave_the_later_one_off() {
        let mut config = Config::default();
        config.tools.key = "shift".into();
        let keys = keys_from(&config);
        assert_eq!((keys.formats, keys.tools), (Some(Key::Shift), None));
        assert_eq!(keys.actions, Some(Key::Control));
        assert!(!config.shows(catalog::Kind::Tools, "half"));
        // A wheel that is off frees its key.
        config.formats.enabled = false;
        let keys = keys_from(&config);
        assert_eq!((keys.formats, keys.tools), (None, Some(Key::Shift)));
        // All three on option: only the first.
        let mut all = Config::default();
        for kind in [&mut all.formats, &mut all.tools, &mut all.actions] {
            kind.key = "option".into();
        }
        assert_eq!(
            keys_from(&all),
            WheelKeys {
                formats: Some(Key::Option),
                tools: None,
                actions: None
            }
        );
    }

    #[test]
    fn a_wheel_that_is_off_or_a_hidden_slot_is_not_offered_to_the_wheel_and_not_run() {
        let dir = TempDir::new("figghost-wheel-cfg");
        let file = dir.join("note.txt");
        std::fs::write(&file, "x").unwrap();
        let paths = vec![file];
        let mut config = Config::default();
        config.actions.hidden = vec!["airdrop".into(), "terminal".into()];
        let wheel = offer(wheel_ui::Kind::Actions, &paths, true, &config).unwrap();
        let ids: Vec<&str> = wheel.slots.iter().map(|s| s.id.as_str()).collect();
        assert_eq!(ids, ["copy_path", "copy", "reveal", "float"]);
        config.actions.enabled = false;
        assert_eq!(offer(wheel_ui::Kind::Actions, &paths, true, &config), None);
        assert!(
            catalog::plan_with("copy", &paths, &|k, i| config.shows(k, i)).is_none(),
            "a wheel that is off runs nothing"
        );
    }

    #[test]
    fn a_config_file_missing_the_setting_gets_its_default() {
        let config: Config = serde_json::from_str("{}").unwrap();
        assert_eq!(config, Config::default());
    }

    #[test]
    fn the_wheel_gets_the_catalogs_slots_and_a_caption() {
        let dir = TempDir::new("figghost-wheel-offer");
        let file = dir.join("note.txt");
        std::fs::write(&file, "x").unwrap();
        let paths = vec![file];
        let wheel = offer(wheel_ui::Kind::Actions, &paths, true, &Config::default())
            .expect("actions apply to any file");
        let ids: Vec<&str> = wheel.slots.iter().map(|s| s.id.as_str()).collect();
        assert_eq!(
            ids,
            [
                "copy_path",
                "copy",
                "airdrop",
                "terminal",
                "reveal",
                "float"
            ]
        );
        assert!(wheel.slots.iter().all(|s| !s.title.is_empty()));
        assert!(wheel.caption.contains('1'));
        // Without a notch there is no shelf to float to.
        let without = offer(wheel_ui::Kind::Actions, &paths, false, &Config::default()).unwrap();
        assert!(without.slots.iter().all(|s| s.id != "float"));
        // A text file has no tools: the wheel does not open.
        assert_eq!(
            offer(wheel_ui::Kind::Tools, &paths, true, &Config::default()),
            None
        );
        assert_eq!(
            offer(wheel_ui::Kind::Actions, &[], true, &Config::default()),
            None
        );
    }
}
