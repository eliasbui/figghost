//! The Launcher tab of the notch: apps, folders and Shortcuts the person pinned, one press away. It is off until the
//! person turns it on, and has nothing pinned until they pin something.
//!
//! An app is pinned by its bundle identifier and opened by asking the system where it is now, a folder by its path
//! and opened in the Finder, a Shortcut by its name and run with the system's `shortcuts` command, which is given
//! the name as an argument and never through a shell. The tab runs nothing by itself: a tile does something only
//! when it is pressed.

use super::{AttachEnv, Ran, run_bounded};
use crate::{
    i18n::{self, Language},
    settings::model::Fault,
    trace,
};
use logic::{Item, Kind};
use macos_sys::{
    MainThreadMarker, app,
    launcher_ui::{LauncherDisplay, LauncherView, PAGE_SIZE, Tile, TileIcon},
    notch::Notch,
    style::Tone,
    workspace,
};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::{
    cell::{Cell, RefCell},
    path::PathBuf,
    process::{Command, Stdio},
    rc::{Rc, Weak},
};

pub mod logic;
pub mod settings;
mod text;

const SHORTCUTS: &str = "/usr/bin/shortcuts";
/// How long the system is given to list the Shortcuts before it is given up on: its service can be slow to start.
const LIST_SECONDS: u64 = 8;

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    pub enabled: bool,
    /// What is pinned, in the order of the tiles. A pin that cannot be opened is dropped when the file is read.
    #[serde(deserialize_with = "lenient_items")]
    pub items: Vec<Item>,
}

fn lenient_items<'de, D: serde::Deserializer<'de>>(deserializer: D) -> Result<Vec<Item>, D::Error> {
    let value = Value::deserialize(deserializer)?;
    Ok(match value {
        Value::Array(values) => logic::items_of(values),
        _ => Vec::new(),
    })
}

impl Config {
    pub fn sanitized(mut self) -> Self {
        self.items = logic::tidy_items(self.items);
        self
    }
}

#[derive(Default)]
struct Listing {
    busy: bool,
    names: Vec<String>,
    /// "none" when `shortcuts` is missing or printed nothing usable.
    problem: Option<&'static str>,
}

thread_local! {
    /// The list of Shortcuts arrives from another thread and hops to the main one, which finds the feature here.
    static FEATURE: RefCell<Weak<Feature>> = const { RefCell::new(Weak::new()) };
}

fn with_feature(f: impl FnOnce(&Feature)) {
    FEATURE.with(|slot| {
        if let Some(feature) = slot.borrow().upgrade() {
            f(&feature);
        }
    });
}

pub struct Feature {
    mtm: MainThreadMarker,
    notch: Weak<Notch>,
    view: Rc<LauncherView>,
    config: RefCell<Config>,
    listing: RefCell<Listing>,
    generation: Cell<u64>,
}

/// Adds the tab. `None` without a notch, which is where the tab lives.
pub fn attach(env: &AttachEnv) -> Option<Rc<Feature>> {
    let notch = env.notch?;
    let feature = Rc::new_cyclic(|me: &Weak<Feature>| {
        let open = me.clone();
        let view = LauncherView::new(
            env.mtm,
            notch.page_area(PAGE_SIZE.width, PAGE_SIZE.height),
            move |index| {
                if let Some(f) = open.upgrade() {
                    f.open(index);
                }
            },
        );
        Feature {
            mtm: env.mtm,
            notch: Rc::downgrade(notch),
            view,
            config: RefCell::new(env.config.launcher.clone().sanitized()),
            listing: RefCell::default(),
            generation: Cell::new(0),
        }
    });
    FEATURE.with(|slot| *slot.borrow_mut() = Rc::downgrade(&feature));
    let shown = Rc::downgrade(&feature);
    notch.add_page(feature.view.page(
        i18n::current().launcher_tab().to_owned(),
        move || {
            if let Some(f) = shown.upgrade() {
                f.render();
            }
        },
        || {},
    ));
    Some(feature)
}

impl Feature {
    pub fn configure(&self, config: &Config) -> Result<(), Fault> {
        *self.config.borrow_mut() = config.clone().sanitized();
        self.render();
        Ok(())
    }

    /// Facts only the running app knows, for the settings page: the Shortcuts that can be pinned.
    pub fn runtime(&self) -> Value {
        let listing = self.listing.borrow();
        json!({
            "listing": listing.busy,
            "shortcuts": listing.names,
            "problem": listing.problem.unwrap_or_default(),
        })
    }

    /// The buttons of the settings page. `None` when the name is not this feature's. A change to the pins is made on
    /// `config`, which the caller saves, and on this feature's own copy.
    pub fn action(&self, name: &str, config: &mut Config) -> Option<Result<(), Fault>> {
        let language = i18n::current();
        let result = match name {
            "launcher_add_app" => self.pin_app(config, language),
            "launcher_add_folder" => self.pin_folder(config, language),
            "launcher_shortcuts" => {
                self.list_shortcuts();
                return Some(Ok(()));
            }
            other => {
                let (verb, number) = other.split_once(':')?;
                let index: usize = number.parse().ok()?;
                match verb {
                    "launcher_add_shortcut" => self.pin_shortcut(config, index),
                    "launcher_remove" => ok_if(logic::remove(&mut config.items, index)),
                    "launcher_up" => ok_if(logic::shift(&mut config.items, index, true)),
                    "launcher_down" => ok_if(logic::shift(&mut config.items, index, false)),
                    _ => return None,
                }
            }
        };
        if result.is_ok() {
            *self.config.borrow_mut() = config.clone().sanitized();
            self.render();
        }
        Some(result)
    }

    fn pin_app(&self, config: &mut Config, language: Language) -> Result<(), Fault> {
        let Some(app) = workspace::pick_app(self.mtm, language.pick_app_prompt()) else {
            return Ok(());
        };
        let item = Item {
            kind: Kind::App,
            target: app.bundle_id,
            name: app.name,
        };
        logic::add(&mut config.items, item).map_err(settings::refusal)
    }

    fn pin_folder(&self, config: &mut Config, language: Language) -> Result<(), Fault> {
        let Some(path) = workspace::pick_folder(self.mtm, language.launcher_pick_folder()) else {
            return Ok(());
        };
        let item = Item {
            kind: Kind::Folder,
            target: path.to_string_lossy().into_owned(),
            name: String::new(),
        };
        logic::add(&mut config.items, item).map_err(settings::refusal)
    }

    /// Pins the n-th Shortcut of the list the page was last shown.
    fn pin_shortcut(&self, config: &mut Config, index: usize) -> Result<(), Fault> {
        let name = self.listing.borrow().names.get(index).cloned();
        let Some(name) = name else {
            return Err(Fault::new("invalid_value", "a Shortcut from the list"));
        };
        let item = Item {
            kind: Kind::Shortcut,
            target: name,
            name: String::new(),
        };
        logic::add(&mut config.items, item).map_err(settings::refusal)
    }

    /// Asks the system which Shortcuts there are. It takes a moment, so it is done off the main thread.
    fn list_shortcuts(&self) {
        let generation = self.generation.get() + 1;
        self.generation.set(generation);
        *self.listing.borrow_mut() = Listing {
            busy: true,
            ..Listing::default()
        };
        trace::log("launcher: listing the Shortcuts");
        std::thread::spawn(move || {
            let listed = run_bounded(SHORTCUTS, &["list"], LIST_SECONDS)
                .map(|text| logic::shortcut_names(&text));
            app::on_main(move || with_feature(|f| f.listed(generation, listed)));
        });
    }

    fn listed(&self, generation: u64, listed: Result<Vec<String>, Ran>) {
        if generation != self.generation.get() {
            return;
        }
        let (names, problem) = match listed {
            Ok(names) if names.is_empty() => (names, Some("none")),
            Ok(names) => (names, None),
            Err(Ran::Slow) => (Vec::new(), Some("slow")),
            Err(Ran::Failed) => (Vec::new(), Some("none")),
        };
        trace::log(&format!(
            "launcher: {} Shortcuts{}",
            names.len(),
            problem.map_or(String::new(), |p| format!(" ({p})"))
        ));
        *self.listing.borrow_mut() = Listing {
            busy: false,
            problem,
            names,
        };
    }

    /// Draws the tiles, or says why there are none.
    fn render(&self) {
        let config = self.config.borrow();
        let language = i18n::current();
        if !config.enabled {
            self.view.show(&note(
                language.launcher_off_title(),
                language.launcher_off_hint(),
            ));
            return;
        }
        if config.items.is_empty() {
            self.view.show(&note(
                language.launcher_empty_title(),
                language.launcher_empty_hint(),
            ));
            return;
        }
        trace::log(&format!("launcher: showing {} tiles", config.items.len()));
        let tiles = config.items.iter().map(tile_of).collect();
        self.view.show(&LauncherDisplay { tiles, note: None });
    }

    /// A tile was pressed: open what it stands for, and put the notch away, as a launcher does.
    fn open(&self, index: usize) {
        let item = self.config.borrow().items.get(index).cloned();
        let Some(item) = item else {
            return;
        };
        trace::log(&format!("launcher: opening {}", item.kind.code()));
        if let Err(e) = launch(&item) {
            eprintln!("launcher: cannot open {}: {e}", item.name);
            return;
        }
        if let Some(notch) = self.notch.upgrade() {
            notch.close();
        }
    }
}

fn ok_if(done: bool) -> Result<(), Fault> {
    if done {
        Ok(())
    } else {
        Err(Fault::new("invalid_value", "a pin of the list"))
    }
}

fn note(title: &str, hint: &str) -> LauncherDisplay {
    LauncherDisplay {
        tiles: Vec::new(),
        note: Some((title.to_owned(), hint.to_owned())),
    }
}

/// What a tile shows for a pin. An app that is not installed any more shows a dashed app, so that the tile can still
/// be seen and taken away.
fn tile_of(item: &Item) -> Tile {
    let icon = match item.kind {
        Kind::App => match workspace::app_path(&item.target) {
            Some(path) => TileIcon::File(path),
            None => TileIcon::Symbol("questionmark.app.dashed", Tone::Gray),
        },
        Kind::Folder => TileIcon::File(PathBuf::from(&item.target)),
        Kind::Shortcut => TileIcon::Symbol("bolt.fill", Tone::Orange),
    };
    Tile {
        title: item.name.clone(),
        icon,
    }
}

/// Opens what a pin stands for.
fn launch(item: &Item) -> Result<(), String> {
    match item.kind {
        Kind::App => {
            let path = workspace::app_path(&item.target).ok_or("the app is not installed")?;
            workspace::open_path(&path);
            Ok(())
        }
        Kind::Folder => {
            let path = PathBuf::from(&item.target);
            if !path.is_dir() {
                return Err("the folder is not there".into());
            }
            workspace::open_path(&path);
            Ok(())
        }
        Kind::Shortcut => {
            let mut child = Command::new(SHORTCUTS)
                .args(["run", &item.target])
                .stdin(Stdio::null())
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .spawn()
                .map_err(|e| e.to_string())?;
            // It runs as long as the Shortcut does; something has to collect it when it is done.
            std::thread::spawn(move || {
                let _ = child.wait();
            });
            Ok(())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn the_feature_is_off_by_default_with_nothing_pinned_and_old_files_load() {
        let config: Config = serde_json::from_str("{}").unwrap();
        assert_eq!(config, Config::default());
        assert!(!config.enabled && config.items.is_empty());
        let partial: Config =
            serde_json::from_str(r#"{ "enabled": true, "future_key": 1 }"#).unwrap();
        assert!(partial.enabled);
    }

    #[test]
    fn a_hostile_file_keeps_the_pins_that_are_good_and_never_becomes_unreadable() {
        let config: Config = serde_json::from_value(json!({
            "enabled": true,
            "items": [
                { "kind": "app", "target": "com.apple.Notes", "name": "Notes" },
                { "kind": "app", "target": "not a bundle id" },
                { "kind": "shortcut", "target": "-h" },
                { "kind": "nonsense", "target": "x" },
                "string",
                { "kind": "folder", "target": "/Users/me/Documents" },
            ]
        }))
        .unwrap();
        assert_eq!(config.items.len(), 2);
        for wrong in [
            r#"{ "items": "x" }"#,
            r#"{ "items": 5 }"#,
            r#"{ "items": null }"#,
            r#"{ "items": {} }"#,
        ] {
            let config: Config = serde_json::from_str(wrong).expect(wrong);
            assert!(config.items.is_empty(), "{wrong}");
        }
    }

    #[test]
    fn a_tile_is_made_for_each_kind_of_pin() {
        let app = tile_of(&Item {
            kind: Kind::App,
            target: "com.apple.finder".into(),
            name: "Finder".into(),
        });
        assert_eq!(app.title, "Finder");
        assert!(
            matches!(app.icon, TileIcon::File(_)),
            "an installed app shows its own icon"
        );
        let missing = tile_of(&Item {
            kind: Kind::App,
            target: "com.example.NotInstalled".into(),
            name: "Gone".into(),
        });
        assert!(matches!(
            missing.icon,
            TileIcon::Symbol("questionmark.app.dashed", _)
        ));
        let folder = tile_of(&Item {
            kind: Kind::Folder,
            target: "/Users".into(),
            name: "Users".into(),
        });
        assert!(matches!(folder.icon, TileIcon::File(_)));
        let shortcut = tile_of(&Item {
            kind: Kind::Shortcut,
            target: "Morning".into(),
            name: "Morning".into(),
        });
        assert!(matches!(shortcut.icon, TileIcon::Symbol("bolt.fill", _)));
    }

    #[test]
    fn what_is_gone_is_not_opened() {
        let folder = Item {
            kind: Kind::Folder,
            target: "/this/folder/does/not/exist".into(),
            name: "x".into(),
        };
        assert!(launch(&folder).is_err());
        let app = Item {
            kind: Kind::App,
            target: "com.example.NotInstalled".into(),
            name: "x".into(),
        };
        assert!(launch(&app).is_err());
    }

    #[test]
    fn the_words_are_there_in_both_languages() {
        for language in [Language::English, Language::Vietnamese] {
            for text in [
                language.launcher_tab(),
                language.launcher_off_title(),
                language.launcher_off_hint(),
                language.launcher_empty_title(),
                language.launcher_empty_hint(),
                language.launcher_pick_folder(),
            ] {
                assert!(!text.is_empty());
            }
        }
    }
}
