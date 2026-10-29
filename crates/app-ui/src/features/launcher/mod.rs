//! The Launcher: apps, folders, files, web addresses and Shortcuts the person pinned, one press away, in groups. It
//! shows in a tab of the notch, and in a dock at an edge of a screen when that is turned on too. Both are off until
//! the person turns them on, and nothing is pinned until they pin something.
//!
//! An app is pinned by its bundle identifier and opened by asking the system where it is now, a folder or a file by
//! its path and opened as the Finder would, a web address with the browser or mail app it belongs to, and a Shortcut
//! by its name, run with the system's `shortcuts` command, which is given the name as an argument and never through
//! a shell. Nothing runs by itself: a tile does something only when it is pressed, and a folder is read only when its
//! menu is asked for.

use super::{AttachEnv, Ran, run_bounded};
use crate::{
    i18n::{self},
    settings::model::Fault,
    trace,
};
use logic::{Group, Item, Kind};
use macos_sys::{
    MainThreadMarker, app,
    launcher_ui::{
        DockPanel, Edge, LauncherCallbacks, LauncherDisplay, LauncherView, MenuEntry, NSEvent,
        PAGE_SIZE, Tile, TileIcon,
    },
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
/// How many entries of a folder its menu lists; the Finder shows the rest.
const FOLDER_MENU: usize = 40;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum DockEdge {
    #[default]
    Left,
    Right,
    Bottom,
}

impl DockEdge {
    fn edge(self) -> Edge {
        match self {
            DockEdge::Left => Edge::Left,
            DockEdge::Right => Edge::Right,
            DockEdge::Bottom => Edge::Bottom,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    /// The tab of the notch.
    pub enabled: bool,
    /// What is pinned, by group, in the order of the tiles. A pin that cannot be opened is dropped when the file is
    /// read, and there is always one group.
    #[serde(deserialize_with = "logic::lenient_groups")]
    pub groups: Vec<Group>,
    /// The pins of a file written before there were groups, read as the first group.
    #[serde(deserialize_with = "logic::lenient_items", skip_serializing)]
    pub items: Vec<Item>,
    /// The dock at an edge of a screen.
    pub dock: bool,
    pub dock_edge: DockEdge,
    /// The name of the screen the dock is on; empty, or a screen that is not there, is the main one.
    pub dock_display: String,
    /// The group the dock shows first.
    pub dock_group: usize,
    /// The dock waits off the screen until the pointer reaches its edge.
    pub dock_autohide: bool,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            enabled: false,
            groups: Vec::new(),
            items: Vec::new(),
            dock: false,
            dock_edge: DockEdge::Left,
            dock_display: String::new(),
            dock_group: 0,
            dock_autohide: true,
        }
    }
}

impl Config {
    pub fn sanitized(mut self) -> Self {
        let mut groups = std::mem::take(&mut self.groups);
        let old = std::mem::take(&mut self.items);
        if !old.is_empty() {
            groups.insert(
                0,
                Group {
                    name: String::new(),
                    items: old,
                },
            );
        }
        self.groups = logic::tidy_groups(groups);
        self.dock_display = self
            .dock_display
            .chars()
            .filter(|c| !crate::features::is_hidden_char(*c))
            .take(100)
            .collect::<String>()
            .trim()
            .to_owned();
        self.dock_group = self.dock_group.min(self.groups.len() - 1);
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

/// Where a tile was pressed: in the tab of the notch, or in the dock.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Place {
    Tab,
    Dock,
}

/// What a line of a tile's menu does.
#[derive(Debug, Clone)]
enum Pick {
    Open(PathBuf),
    Reveal(PathBuf),
    Launch(Item),
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
    me: Weak<Feature>,
    notch: Weak<Notch>,
    /// The tab, `None` without a notch.
    view: Option<Rc<LauncherView>>,
    /// Made the first time the dock is turned on.
    dock: RefCell<Option<Rc<DockPanel>>>,
    config: RefCell<Config>,
    listing: RefCell<Listing>,
    generation: Cell<u64>,
    /// The group each place shows; the dock's starts at the one the settings chose.
    tab_group: Cell<usize>,
    dock_group: Cell<usize>,
}

fn callbacks(me: &Weak<Feature>, place: Place) -> LauncherCallbacks {
    let (open, menu, group) = (me.clone(), me.clone(), me.clone());
    LauncherCallbacks {
        on_open: Box::new(move |index| {
            if let Some(f) = open.upgrade() {
                f.open(place, index);
            }
        }),
        on_menu: Box::new(move |index, event| {
            if let Some(f) = menu.upgrade() {
                f.menu(place, index, event);
            }
        }),
        on_group: Box::new(move |index| {
            if let Some(f) = group.upgrade() {
                f.show_group(place, index);
            }
        }),
    }
}

/// Adds the tab when there is a notch, and the dock when it is turned on.
pub fn attach(env: &AttachEnv) -> Option<Rc<Feature>> {
    let config = env.config.launcher.clone().sanitized();
    let feature = Rc::new_cyclic(|me: &Weak<Feature>| Feature {
        mtm: env.mtm,
        me: me.clone(),
        notch: env.notch.map(Rc::downgrade).unwrap_or_default(),
        view: env.notch.map(|notch| {
            LauncherView::new(
                env.mtm,
                notch.page_area(PAGE_SIZE.width, PAGE_SIZE.height),
                callbacks(me, Place::Tab),
            )
        }),
        dock: RefCell::default(),
        dock_group: Cell::new(config.dock_group),
        config: RefCell::new(config),
        listing: RefCell::default(),
        generation: Cell::new(0),
        tab_group: Cell::new(0),
    });
    FEATURE.with(|slot| *slot.borrow_mut() = Rc::downgrade(&feature));
    if let (Some(notch), Some(view)) = (env.notch, &feature.view) {
        let shown = Rc::downgrade(&feature);
        notch.add_page(view.page(
            i18n::current().launcher_tab().to_owned(),
            move || {
                if let Some(f) = shown.upgrade() {
                    f.render_tab();
                }
            },
            || {},
        ));
    }
    feature.render_dock();
    Some(feature)
}

impl Feature {
    pub fn configure(&self, config: &Config) -> Result<(), Fault> {
        let config = config.clone().sanitized();
        self.dock_group.set(config.dock_group);
        *self.config.borrow_mut() = config;
        self.render();
        Ok(())
    }

    /// Facts only the running app knows, for the settings page: the Shortcuts that can be pinned and the screens
    /// the dock can be on.
    pub fn runtime(&self) -> Value {
        let listing = self.listing.borrow();
        json!({
            "listing": listing.busy,
            "shortcuts": listing.names,
            "problem": listing.problem.unwrap_or_default(),
            "screens": workspace::screen_names(self.mtm),
        })
    }

    /// The buttons of the settings page. `None` when the name is not this feature's. A change to the pins is made on
    /// `config`, which the caller saves, and on this feature's own copy. Each names its group, and a pin its number in
    /// the group: `launcher_remove:<group>:<n>`.
    pub fn action(&self, name: &str, config: &mut Config) -> Option<Result<(), Fault>> {
        let result = match name {
            "launcher_shortcuts" => {
                self.list_shortcuts();
                return Some(Ok(()));
            }
            "launcher_group_add" => logic::add_group(&mut config.groups).map_err(settings::refusal),
            other => {
                let (verb, rest) = other.split_once(':')?;
                let (group, rest) = rest.split_once(':').unwrap_or((rest, ""));
                let group: usize = group.parse().ok()?;
                let index = rest.parse::<usize>();
                let groups = &mut config.groups;
                match verb {
                    "launcher_add_shortcut" => self.pin_shortcut(groups, group, index.ok()?),
                    "launcher_add_url" => {
                        let item = Item {
                            kind: Kind::Url,
                            target: logic::url_of(rest),
                            name: String::new(),
                        };
                        logic::add(groups, group, item).map_err(settings::refusal)
                    }
                    "launcher_remove" => ok_if(groups.get_mut(group).is_some_and(|g| {
                        logic::remove(&mut g.items, index.ok().unwrap_or(usize::MAX))
                    })),
                    "launcher_up" | "launcher_down" => {
                        ok_if(groups.get_mut(group).is_some_and(|g| {
                            logic::shift(
                                &mut g.items,
                                index.ok().unwrap_or(usize::MAX),
                                verb == "launcher_up",
                            )
                        }))
                    }
                    "launcher_group_remove" => ok_if(logic::remove_group(groups, group)),
                    "launcher_group_rename" => ok_if(logic::rename_group(groups, group, rest)),
                    "launcher_group_up" | "launcher_group_down" => {
                        ok_if(logic::shift(groups, group, verb == "launcher_group_up"))
                    }
                    _ => return None,
                }
            }
        };
        if result.is_ok() {
            *config = config.clone().sanitized();
            *self.config.borrow_mut() = config.clone();
            self.render();
        }
        Some(result)
    }

    /// Asks the person for an app, a folder or a file (`launcher_add_app:<group>`, `launcher_add_folder:<group>`,
    /// `launcher_add_file:<group>`) with the system's panel. The panel runs a loop of its own while it is open, in
    /// which timers and shortcuts keep running and may read the config, so the caller must not hold the config while
    /// it is asked. `None` when the name is none of these, or the person cancelled.
    pub fn choose(&self, name: &str) -> Option<(usize, Item)> {
        let language = i18n::current();
        let (verb, group) = name.split_once(':')?;
        let group: usize = group.parse().ok()?;
        let path_item = |kind, path: PathBuf| Item {
            kind,
            target: path.to_string_lossy().into_owned(),
            name: String::new(),
        };
        let item = match verb {
            "launcher_add_app" => {
                let app = workspace::pick_app(self.mtm, language.pick_app_prompt())?;
                Item {
                    kind: Kind::App,
                    target: app.bundle_id,
                    name: app.name,
                }
            }
            "launcher_add_folder" => path_item(
                Kind::Folder,
                workspace::pick_folder(self.mtm, language.launcher_pick_folder())?,
            ),
            "launcher_add_file" => path_item(
                Kind::File,
                workspace::pick_file(self.mtm, language.launcher_pick_file())?,
            ),
            _ => return None,
        };
        Some((group, item))
    }

    /// Whether `name` is a button that `choose` answers.
    pub fn is_choice(name: &str) -> bool {
        [
            "launcher_add_app:",
            "launcher_add_folder:",
            "launcher_add_file:",
        ]
        .iter()
        .any(|prefix| name.starts_with(prefix))
    }

    /// Pins what `choose` gave, on `config`, which the caller saves, and on this feature's own copy.
    pub fn pin(&self, config: &mut Config, (group, item): (usize, Item)) -> Result<(), Fault> {
        logic::add(&mut config.groups, group, item).map_err(settings::refusal)?;
        *self.config.borrow_mut() = config.clone().sanitized();
        self.render();
        Ok(())
    }

    /// Pins the n-th Shortcut of the list the page was last shown.
    fn pin_shortcut(&self, groups: &mut [Group], group: usize, index: usize) -> Result<(), Fault> {
        let name = self.listing.borrow().names.get(index).cloned();
        let Some(name) = name else {
            return Err(Fault::new("invalid_value", "a Shortcut from the list"));
        };
        let item = Item {
            kind: Kind::Shortcut,
            target: name,
            name: String::new(),
        };
        logic::add(groups, group, item).map_err(settings::refusal)
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

    fn group_cell(&self, place: Place) -> &Cell<usize> {
        match place {
            Place::Tab => &self.tab_group,
            Place::Dock => &self.dock_group,
        }
    }

    /// The group a place shows, kept within the groups there are.
    fn group_of(&self, place: Place) -> usize {
        let last = self.config.borrow().groups.len().saturating_sub(1);
        let group = self.group_cell(place).get().min(last);
        self.group_cell(place).set(group);
        group
    }

    fn show_group(&self, place: Place, group: usize) {
        self.group_cell(place).set(group);
        match place {
            Place::Tab => self.render_tab(),
            Place::Dock => self.render_dock(),
        }
    }

    fn render(&self) {
        self.render_tab();
        self.render_dock();
    }

    /// What a place shows: the tiles of its group, or why there are none, with the strip of the groups when
    /// `strip`.
    fn display(&self, place: Place, strip: bool) -> LauncherDisplay {
        let group = self.group_of(place);
        let config = self.config.borrow();
        let language = i18n::current();
        let groups = if strip {
            config
                .groups
                .iter()
                .enumerate()
                .map(|(i, g)| group_name(g, i))
                .collect()
        } else {
            Vec::new()
        };
        let items = &config.groups[group].items;
        if items.is_empty() {
            return LauncherDisplay {
                note: Some((
                    language.launcher_empty_title().to_owned(),
                    language.launcher_empty_hint().to_owned(),
                )),
                groups,
                group,
                ..LauncherDisplay::default()
            };
        }
        LauncherDisplay {
            tiles: items.iter().map(tile_of).collect(),
            note: None,
            groups,
            group,
        }
    }

    /// Draws the tiles of the tab, or says why there are none.
    fn render_tab(&self) {
        let Some(view) = &self.view else {
            return;
        };
        if !self.config.borrow().enabled {
            let language = i18n::current();
            view.show(&note(
                language.launcher_off_title(),
                language.launcher_off_hint(),
            ));
            return;
        }
        let display = self.display(Place::Tab, true);
        if display.note.is_none() {
            trace::log(&format!("launcher: showing {} tiles", display.tiles.len()));
        }
        view.show(&display);
    }

    /// Puts the dock at its edge, or takes it away when it is off.
    fn render_dock(&self) {
        let (on, edge, screen, autohide) = {
            let config = self.config.borrow();
            (
                config.dock,
                config.dock_edge.edge(),
                config.dock_display.clone(),
                config.dock_autohide,
            )
        };
        if !on {
            if let Some(dock) = self.dock.borrow().as_ref() {
                dock.hide();
            }
            return;
        }
        let dock = Rc::clone(
            self.dock
                .borrow_mut()
                .get_or_insert_with(|| DockPanel::new(self.mtm, callbacks(&self.me, Place::Dock))),
        );
        // A side of the screen has no room for the names of the groups; a swipe moves between them there.
        let display = self.display(Place::Dock, edge == Edge::Bottom);
        trace::log(&format!(
            "launcher: the dock shows {} tiles at the {edge:?} edge{}",
            display.tiles.len(),
            if autohide {
                ", hidden until the pointer comes"
            } else {
                ""
            }
        ));
        dock.show(&display, edge, &screen, autohide);
    }

    /// A tile was pressed: open what it stands for, and put the notch away, as a launcher does.
    fn open(&self, place: Place, index: usize) {
        let group = self.group_of(place);
        let item = self.config.borrow().groups[group].items.get(index).cloned();
        if let Some(item) = item {
            self.run(place, Pick::Launch(item));
        }
    }

    fn run(&self, place: Place, pick: Pick) {
        let done = match &pick {
            Pick::Launch(item) => {
                trace::log(&format!("launcher: opening {}", item.kind.code()));
                launch(item).map_err(|e| format!("cannot open {}: {e}", item.name))
            }
            Pick::Open(path) => {
                workspace::open_path(path);
                Ok(())
            }
            Pick::Reveal(path) => {
                workspace::reveal(path);
                Ok(())
            }
        };
        if let Err(e) = done {
            eprintln!("launcher: {e}");
            return;
        }
        if place == Place::Tab
            && let Some(notch) = self.notch.upgrade()
        {
            notch.close();
        }
    }

    /// A tile was clicked with the right button: a folder lists what is in it, an app or a file offers to show it in
    /// the Finder. A web address and a Shortcut have nothing more to offer.
    fn menu(&self, place: Place, index: usize, event: &NSEvent) {
        let group = self.group_of(place);
        let item = self.config.borrow().groups[group].items.get(index).cloned();
        let Some(item) = item else {
            return;
        };
        let lines = menu_lines(&item);
        if lines.is_empty() {
            return;
        }
        let view = match place {
            Place::Tab => self.view.clone(),
            Place::Dock => self.dock.borrow().as_ref().map(|d| Rc::clone(d.view())),
        };
        let Some(view) = view else {
            return;
        };
        let (entries, picks): (Vec<MenuEntry>, Vec<Option<Pick>>) = lines.into_iter().unzip();
        let me = self.me.clone();
        view.pop_up(&entries, event, move |chosen| {
            if let (Some(f), Some(Some(pick))) = (me.upgrade(), picks.get(chosen)) {
                f.run(place, pick.clone());
            }
        });
    }
}

/// The lines of a tile's menu and what each does; none for what has no menu.
fn menu_lines(item: &Item) -> Vec<(MenuEntry, Option<Pick>)> {
    let language = i18n::current();
    let path = PathBuf::from(&item.target);
    match item.kind {
        Kind::Folder => {
            let mut lines = vec![
                (
                    MenuEntry::new(language.launcher_open_in_finder(), None),
                    Some(Pick::Open(path.clone())),
                ),
                (MenuEntry::separator(), None),
            ];
            if !path.is_dir() {
                lines.push((MenuEntry::note(language.launcher_folder_gone()), None));
                return lines;
            }
            let (entries, more) = logic::folder_entries(&path, FOLDER_MENU);
            if entries.is_empty() {
                lines.push((MenuEntry::note(language.launcher_folder_empty()), None));
            }
            for (name, _) in entries {
                let entry = path.join(&name);
                lines.push((
                    MenuEntry::new(&name, Some(entry.clone())),
                    Some(Pick::Open(entry)),
                ));
            }
            if more {
                lines.push((MenuEntry::separator(), None));
                lines.push((
                    MenuEntry::new(language.launcher_folder_more(), None),
                    Some(Pick::Open(path)),
                ));
            }
            lines
        }
        Kind::File | Kind::App => {
            let path = match item.kind {
                Kind::App => match workspace::app_path(&item.target) {
                    Some(path) => path,
                    None => return Vec::new(),
                },
                _ => path,
            };
            vec![
                (
                    MenuEntry::new(language.launcher_open(), None),
                    Some(Pick::Launch(item.clone())),
                ),
                (
                    MenuEntry::new(language.launcher_reveal(), None),
                    Some(Pick::Reveal(path)),
                ),
            ]
        }
        Kind::Url | Kind::Shortcut => Vec::new(),
    }
}

/// What the strip calls a group: its name, or its number when it has none.
fn group_name(group: &Group, index: usize) -> String {
    if group.name.is_empty() {
        i18n::current().launcher_group(index + 1)
    } else {
        group.name.clone()
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
        note: Some((title.to_owned(), hint.to_owned())),
        ..LauncherDisplay::default()
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
        Kind::Folder | Kind::File => TileIcon::File(PathBuf::from(&item.target)),
        Kind::Url if item.target.to_ascii_lowercase().starts_with("mailto:") => {
            TileIcon::Symbol("envelope.fill", Tone::Blue)
        }
        Kind::Url => TileIcon::Symbol("globe", Tone::Blue),
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
        Kind::Folder | Kind::File => {
            let path = PathBuf::from(&item.target);
            let there = if item.kind == Kind::Folder {
                path.is_dir()
            } else {
                path.exists()
            };
            if !there {
                return Err("it is not there".into());
            }
            workspace::open_path(&path);
            Ok(())
        }
        Kind::Url => {
            // The address was checked when it was read; it is checked again here, where it is opened.
            if item.clone().sanitized().is_none() {
                return Err("not a web or mail address".into());
            }
            workspace::open_url(&item.target);
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
    use crate::i18n::Language;
    use serde_json::json;

    #[test]
    fn the_feature_is_off_by_default_with_nothing_pinned_and_old_files_load() {
        let config: Config = serde_json::from_str("{}").unwrap();
        assert_eq!(config, Config::default());
        assert!(!config.enabled && !config.dock && config.dock_autohide);
        let config = config.sanitized();
        assert_eq!(config.groups.len(), 1);
        assert!(config.groups[0].items.is_empty());
        let partial: Config =
            serde_json::from_str(r#"{ "enabled": true, "future_key": 1 }"#).unwrap();
        assert!(partial.enabled);
    }

    #[test]
    fn the_pins_of_a_file_from_before_the_groups_become_the_first_group_and_are_written_as_groups()
    {
        let config: Config = serde_json::from_value(json!({
            "enabled": true,
            "items": [{ "kind": "app", "target": "com.apple.Notes", "name": "Notes" }],
            "groups": [{ "name": "Work", "items": [{ "kind": "folder", "target": "/Users" }] }],
        }))
        .unwrap();
        let config = config.sanitized();
        assert_eq!(config.groups.len(), 2);
        assert_eq!(config.groups[0].items[0].name, "Notes");
        assert_eq!(config.groups[1].name, "Work");
        let written = serde_json::to_value(&config).unwrap();
        assert!(written.get("items").is_none(), "{written}");
        assert_eq!(
            written["groups"][0]["items"][0]["target"],
            json!("com.apple.Notes")
        );
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
            ],
            "groups": [ 5, { "name": 3 }, { "name": "Web", "items": [{ "kind": "url", "target": "javascript:x" }] } ],
            "dock_display": "Studio\u{202E} Display",
            "dock_group": 99,
        }))
        .unwrap();
        let config = config.sanitized();
        assert_eq!(config.groups[0].items.len(), 2);
        assert_eq!(
            config.groups.len(),
            2,
            "the groups that are not objects of the right shape are left out"
        );
        assert!(config.groups[1].items.is_empty());
        assert_eq!(config.dock_display, "Studio Display");
        assert_eq!(
            config.dock_group, 1,
            "the dock's group is one that there is"
        );
        for wrong in [
            r#"{ "groups": "x" }"#,
            r#"{ "groups": 5 }"#,
            r#"{ "groups": null }"#,
            r#"{ "items": {} }"#,
        ] {
            let config: Config = serde_json::from_str(wrong).expect(wrong);
            let config = config.sanitized();
            assert!(
                config.groups.len() == 1 && config.groups[0].items.is_empty(),
                "{wrong}"
            );
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
        for kind in [Kind::Folder, Kind::File] {
            let tile = tile_of(&Item {
                kind,
                target: "/Users".into(),
                name: "Users".into(),
            });
            assert!(matches!(tile.icon, TileIcon::File(_)));
        }
        let web = |target: &str| {
            tile_of(&Item {
                kind: Kind::Url,
                target: target.into(),
                name: "x".into(),
            })
        };
        assert!(matches!(
            web("https://a.vn").icon,
            TileIcon::Symbol("globe", _)
        ));
        assert!(matches!(
            web("mailto:a@b.vn").icon,
            TileIcon::Symbol("envelope.fill", _)
        ));
        let shortcut = tile_of(&Item {
            kind: Kind::Shortcut,
            target: "Morning".into(),
            name: "Morning".into(),
        });
        assert!(matches!(shortcut.icon, TileIcon::Symbol("bolt.fill", _)));
    }

    #[test]
    fn what_is_gone_is_not_opened_and_a_bad_address_is_never_opened() {
        for (kind, target) in [
            (Kind::Folder, "/this/folder/does/not/exist"),
            (Kind::File, "/this/file/does/not/exist.txt"),
            (Kind::App, "com.example.NotInstalled"),
            (Kind::Url, "javascript:alert(1)"),
        ] {
            let item = Item {
                kind,
                target: target.into(),
                name: "x".into(),
            };
            assert!(launch(&item).is_err(), "{target}");
        }
    }

    #[test]
    fn a_folder_offers_what_is_in_it_and_a_web_address_has_no_menu() {
        let folder = Item {
            kind: Kind::Folder,
            target: "/".into(),
            name: "Root".into(),
        };
        let lines = menu_lines(&folder);
        assert!(
            matches!(lines[0].1, Some(Pick::Open(_))),
            "the first line opens the folder"
        );
        assert!(lines.iter().any(|(e, _)| e.title == "Users"));
        let gone = Item {
            kind: Kind::Folder,
            target: "/no/such/folder".into(),
            name: "x".into(),
        };
        assert!(
            menu_lines(&gone)
                .iter()
                .any(|(e, pick)| !e.enabled && pick.is_none() && !e.title.is_empty())
        );
        let web = Item {
            kind: Kind::Url,
            target: "https://a.vn".into(),
            name: "a.vn".into(),
        };
        assert!(menu_lines(&web).is_empty());
        let file = Item {
            kind: Kind::File,
            target: "/etc/hosts".into(),
            name: "hosts".into(),
        };
        assert!(matches!(menu_lines(&file)[1].1, Some(Pick::Reveal(_))));
    }

    #[test]
    fn the_buttons_that_ask_with_the_systems_panel_are_told_apart() {
        assert!(Feature::is_choice("launcher_add_app:0"));
        assert!(Feature::is_choice("launcher_add_file:3"));
        assert!(!Feature::is_choice("launcher_add_url:0:a.vn"));
        assert!(!Feature::is_choice("launcher_add_app"));
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
                language.launcher_pick_file(),
                language.launcher_open(),
                language.launcher_open_in_finder(),
                language.launcher_reveal(),
                language.launcher_folder_empty(),
                language.launcher_folder_gone(),
                language.launcher_folder_more(),
            ] {
                assert!(!text.is_empty());
            }
            assert!(language.launcher_group(2).contains('2'));
        }
    }
}
