//! Dragging a window onto the notch to snap it into a layout: halves, thirds, quarters, a wide window with a narrow
//! one, or a layout a person made from a grid.
//!
//! While a window is dragged by its title bar and the pointer reaches the top edge of a screen, the layouts to
//! choose from hang from the notch, each a small screen with its cells. The pointer on one shows its cells over the
//! screen; going down onto the screen shrinks the row to a chip with the name of the layout and lights the cell under
//! the pointer, which is where the window will land. Releasing the button in a cell puts the window there on that
//! screen; releasing anywhere else changes nothing. Each screen can offer its own layouts.
//!
//! How it works. A global monitor sees the left button of every app (mouse events need no permission). At the
//! press, Accessibility finds the window under the pointer; the press counts only if it is on the title bar
//! strip of that window and not on a control. During the drag the window must actually follow the pointer
//! (Accessibility is asked until it has moved), which also rules out selecting text or dragging a scroll bar.
//! The pure state machine in `core-window::snap` decides what shows; `controller` turns its answers into calls,
//! and `platform` is the AppKit side. Without Accessibility access nothing is looked at.

use super::AttachEnv;
use crate::settings::model::Fault;
use core_window::templates::{self, Custom};
use macos_sys::{
    MainThreadMarker, notch::Notch, snap_ui::MouseMonitor, status_item::Entry, workspace,
};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::{
    cell::{Cell, RefCell},
    collections::BTreeMap,
    rc::Rc,
};

pub mod controller;
mod platform;
pub mod settings;
mod text;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    pub enabled: bool,
    /// The layouts every screen offers that has no list of its own, by id: built-in ones and the ones made here. An
    /// id that names nothing is ignored; see `templates::choose`.
    pub templates: Vec<String>,
    /// The layouts a person made from a grid.
    pub custom: Vec<CustomLayout>,
    /// The list of one screen, by the name of the screen, in place of `templates`.
    pub screens: BTreeMap<String, Vec<String>>,
}

/// A layout made from a grid, as it is kept in the config file: see `templates::Custom`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct CustomLayout {
    pub id: String,
    pub name: String,
    pub cols: u8,
    pub rows: u8,
    /// Each cell as `[column, row, column, row]` of its first part and of the one after its last.
    pub cells: Vec<[u8; 4]>,
}

impl CustomLayout {
    pub fn to_core(&self) -> Custom {
        Custom {
            id: self.id.clone(),
            name: self.name.clone(),
            cols: self.cols,
            rows: self.rows,
            cells: self.cells.clone(),
        }
    }
}

impl Default for Config {
    fn default() -> Self {
        Self {
            enabled: true,
            templates: templates::DEFAULT.iter().map(|id| (*id).to_owned()).collect(),
            custom: Vec::new(),
            screens: BTreeMap::new(),
        }
    }
}

impl Config {
    /// What the screens offer: the layouts of each list that can be made, and the default ones where none can.
    pub fn choice(&self) -> controller::Choice {
        let custom: Vec<Custom> = self.custom.iter().map(CustomLayout::to_core).collect();
        controller::Choice {
            default: templates::choose(&self.templates, &custom),
            screens: self
                .screens
                .iter()
                .map(|(name, ids)| (name.clone(), templates::choose(ids, &custom)))
                .collect(),
        }
    }
}

type Live = controller::Controller<platform::AppKit>;

/// The running feature. Created by `attach`, kept alive by the app context.
pub struct Feature {
    mtm: MainThreadMarker,
    controller: Rc<Live>,
    /// Present only while the feature is on: nothing watches the pointer otherwise.
    monitor: RefCell<Option<MouseMonitor>>,
    /// macOS was asked for Accessibility once in this run; it is not asked again until the next.
    asked: Cell<bool>,
}

/// Starts the feature. It does not need the notch, which it only holds shut while the zones show.
pub fn attach(env: &AttachEnv) -> Option<Rc<Feature>> {
    let notch: Option<Rc<Notch>> = env.notch.cloned();
    let feature = Rc::new(Feature {
        mtm: env.mtm,
        controller: Rc::new(controller::Controller::new(
            platform::AppKit::new(env.mtm, notch),
            env.config.snap.choice(),
        )),
        monitor: RefCell::new(None),
        asked: Cell::new(false),
    });
    if let Err(fault) = feature.configure(&env.config.snap) {
        eprintln!("snap: {}", fault.detail);
    }
    Some(feature)
}

impl Feature {
    /// Applies the settings after a change. An error is shown on the settings page and the change is undone.
    pub fn configure(&self, config: &Config) -> Result<(), Fault> {
        self.controller.set_choice(config.choice());
        if !config.enabled {
            self.monitor.borrow_mut().take();
            self.controller.cancel();
            return Ok(());
        }
        // Without Accessibility a drag finds no window and nothing shows: ask macOS, which shows its own dialog
        // pointing to the setting.
        if !self.asked.replace(true) && !macos_sys::permissions::accessibility_trusted(false) {
            macos_sys::permissions::accessibility_trusted(true);
        }
        if self.monitor.borrow().is_some() {
            return Ok(());
        }
        let controller = Rc::clone(&self.controller);
        let monitor = MouseMonitor::start(self.mtm, move |mouse| controller.handle(mouse))
            .ok_or_else(|| Fault::new("snap_failed", "the system refused the mouse monitor"))?;
        *self.monitor.borrow_mut() = Some(monitor);
        Ok(())
    }

    /// Releases the global shortcuts while the settings page records a new one. There are none.
    pub fn suspend_hotkeys(&self) {}

    /// Registers the global shortcuts again from the config. There are none.
    pub fn resume_hotkeys(&self, _config: &Config) {}

    /// Facts only the running app knows, merged into this feature's part of the page state.
    pub fn runtime(&self) -> Value {
        json!({
            "accessibility": macos_sys::permissions::accessibility_trusted(false),
            "displays": workspace::screen_names(self.mtm),
        })
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

#[cfg(test)]
mod tests {
    use super::*;

    fn ids(list: &[core_window::templates::Template]) -> Vec<&str> {
        list.iter().map(|t| t.id.as_str()).collect()
    }

    #[test]
    fn it_is_on_by_default_with_the_layouts_most_often_wanted() {
        let config = Config::default();
        assert!(config.enabled);
        assert_eq!(
            config.templates,
            ["full", "halves", "thirds", "quarters", "rows", "main_left", "main_right"]
        );
        let choice = config.choice();
        assert_eq!(ids(&choice.default), config.templates);
        assert!(choice.screens.is_empty());
    }

    #[test]
    fn a_config_file_missing_a_setting_gets_its_default() {
        let config: Config = serde_json::from_str(r#"{ "enabled": false }"#).unwrap();
        assert!(!config.enabled, "a switch turned off stays off");
        assert_eq!(config.templates, Config::default().templates);
        let config: Config = serde_json::from_str(r#"{ "templates": ["halves"] }"#).unwrap();
        assert!(config.enabled);
        assert_eq!(ids(&config.choice().default), ["halves"]);
    }

    #[test]
    fn the_old_list_of_zones_in_a_file_is_ignored() {
        let config: Config = serde_json::from_str(r#"{ "enabled": true, "zones": ["left", "right"] }"#).unwrap();
        assert_eq!(config, Config::default());
    }

    #[test]
    fn hostile_ids_in_the_file_are_cleaned_when_used() {
        let config: Config = serde_json::from_str(
            r#"{ "templates": ["full", "halves", "halves", "../../etc", "", "center", "c1"],
                 "screens": { "DELL": ["bogus"], "Mac": ["quarters"] } }"#,
        )
        .unwrap();
        let choice = config.choice();
        assert_eq!(ids(&choice.default), ["full", "halves"]);
        let screen = |name: &str| ids(&choice.screens.iter().find(|(n, _)| n == name).unwrap().1);
        assert_eq!(screen("Mac"), ["quarters"]);
        assert_eq!(screen("DELL"), templates::DEFAULT, "nothing usable: the default ones");
        let empty: Config = serde_json::from_str(r#"{ "templates": [] }"#).unwrap();
        assert_eq!(ids(&empty.choice().default), templates::DEFAULT);
    }

    #[test]
    fn a_layout_made_here_is_offered_when_it_checks_out_and_ignored_when_it_does_not() {
        let config: Config = serde_json::from_str(
            r#"{ "templates": ["c1", "c2", "full"],
                 "custom": [
                   { "id": "c1", "name": "Mine", "cols": 2, "rows": 1, "cells": [[0, 0, 1, 1], [1, 0, 2, 1]] },
                   { "id": "c2", "name": "Gap", "cols": 2, "rows": 1, "cells": [[0, 0, 1, 1]] },
                   { "id": "c3" }
                 ] }"#,
        )
        .unwrap();
        let offered = config.choice().default;
        assert_eq!(ids(&offered), ["c1", "full"]);
        assert_eq!(offered[0].name.as_deref(), Some("Mine"));
        assert_eq!(offered[0].cells.len(), 2);
    }
}
