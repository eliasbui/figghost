//! Dragging a window onto the notch to snap it to halves and thirds.
//!
//! While a window is dragged by its title bar and the pointer reaches the top edge of a screen, a row of drop
//! zones hangs from the notch. Releasing the button on a zone puts the window in that layout on that screen;
//! releasing anywhere else changes nothing.
//!
//! How it works. A global monitor sees the left button of every app (mouse events need no permission). At the
//! press, Accessibility finds the window under the pointer; the press counts only if it is on the title bar
//! strip of that window and not on a control. During the drag the window must actually follow the pointer
//! (Accessibility is asked until it has moved), which also rules out selecting text or dragging a scroll bar.
//! The pure state machine in `core-window::snap` decides what shows; `controller` turns its answers into calls,
//! and `platform` is the AppKit side. Without Accessibility access nothing is looked at.

use super::AttachEnv;
use crate::settings::model::Fault;
use core_window::snap;
use macos_sys::{MainThreadMarker, notch::Notch, snap_ui::MouseMonitor, status_item::Entry};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::{cell::RefCell, rc::Rc};

pub mod controller;
mod platform;
pub mod settings;
mod text;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    pub enabled: bool,
    /// Layout names, as in `tiling.shortcuts`. Unknown and repeated names are ignored and the row is capped; see
    /// `snap::choose`.
    pub zones: Vec<String>,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            enabled: false,
            zones: snap::DEFAULT_ZONES
                .iter()
                .map(|l| l.key().to_owned())
                .collect(),
        }
    }
}

impl Config {
    pub fn layouts(&self) -> Vec<core_window::Layout> {
        snap::choose(&self.zones)
    }
}

type Live = controller::Controller<platform::AppKit>;

/// The running feature. Created by `attach`, kept alive by the app context.
pub struct Feature {
    mtm: MainThreadMarker,
    controller: Rc<Live>,
    /// Present only while the feature is on: nothing watches the pointer otherwise.
    monitor: RefCell<Option<MouseMonitor>>,
}

/// Starts the feature. It does not need the notch, which it only holds shut while the zones show.
pub fn attach(env: &AttachEnv) -> Option<Rc<Feature>> {
    let notch: Option<Rc<Notch>> = env.notch.cloned();
    let feature = Rc::new(Feature {
        mtm: env.mtm,
        controller: Rc::new(controller::Controller::new(
            platform::AppKit::new(env.mtm, notch),
            env.config.snap.layouts(),
        )),
        monitor: RefCell::new(None),
    });
    if let Err(fault) = feature.configure(&env.config.snap) {
        eprintln!("snap: {}", fault.detail);
    }
    Some(feature)
}

impl Feature {
    /// Applies the settings after a change. An error is shown on the settings page and the change is undone.
    pub fn configure(&self, config: &Config) -> Result<(), Fault> {
        self.controller.set_zones(config.layouts());
        if !config.enabled {
            self.monitor.borrow_mut().take();
            self.controller.cancel();
            return Ok(());
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
        json!({ "accessibility": macos_sys::permissions::accessibility_trusted(false) })
    }

    /// A button of the settings page. `None` when the name is not this feature's.
    pub fn action(&self, _name: &str, _config: &mut Config) -> Option<Result<(), Fault>> {
        None
    }

    /// Entries for the menu bar item. The feature has none: the "Fig" menu stays as it is.
    pub fn menu_entries(_this: &Rc<Self>) -> Vec<Entry> {
        Vec::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn it_is_off_by_default_with_the_six_zones_of_the_task() {
        let config = Config::default();
        assert!(!config.enabled);
        assert_eq!(
            config.zones,
            [
                "left",
                "right",
                "first_third",
                "center_third",
                "last_third",
                "maximize"
            ]
        );
        assert_eq!(config.layouts().len(), 6);
    }

    #[test]
    fn a_config_file_missing_the_zones_gets_the_default_row() {
        let config: Config = serde_json::from_str(r#"{ "enabled": true }"#).unwrap();
        assert!(config.enabled);
        assert_eq!(config.layouts(), snap::DEFAULT_ZONES.to_vec());
    }

    #[test]
    fn hostile_zone_names_in_the_file_are_cleaned_when_used() {
        let config: Config = serde_json::from_str(
            r#"{ "enabled": true, "zones": ["maximize", "left", "left", "../../etc", "", "center"] }"#,
        )
        .unwrap();
        assert_eq!(
            config.layouts(),
            vec![core_window::Layout::Left, core_window::Layout::Maximize]
        );
        let empty: Config = serde_json::from_str(r#"{ "zones": [] }"#).unwrap();
        assert_eq!(empty.layouts(), snap::DEFAULT_ZONES.to_vec());
    }
}
