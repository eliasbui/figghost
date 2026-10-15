//! Hiding menu bar icons.
//!
//! The app adds two status items to the menu bar, an arrow and a divider to its left. While the icons are hidden
//! the divider is made very long, which pushes every icon to its left out of view; showing them makes it short
//! again. The person chooses which icons hide by dragging them (Command-drag) to the left of the divider.
//!
//! What this needs from the system is not fixed. macOS caps the length of a status item at 10 000 points, and on
//! macOS 27 an item longer than the room the status area has is dropped from the layout instead of laid out, so
//! it pushes nothing; that room depends on the display (the notch, or the app menus) and changes as the front app
//! changes. Hence `controller`: it measures where the divider's window ends up, searches for the longest length
//! that is laid out, checks again after every hide and whenever the screens or the front app change, and leaves
//! the icons showing, with a note on the settings page, when no length works. `logic` holds the rules, `platform`
//! the AppKit side. Turning the feature off, quitting or crashing removes both items, which gives every icon
//! back; nothing is left changed in other apps.

use super::AttachEnv;
use crate::settings::model::Fault;
use logic::{Rules, SEPARATOR_WIDTH};
use macos_sys::{
    MainThreadMarker, app,
    menubar::Hider,
    status_item::{Entry, Item},
    workspace,
};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::{
    cell::{Cell, RefCell},
    rc::{Rc, Weak},
};

pub mod controller;
pub mod logic;
mod platform;
pub mod settings;
mod text;

use controller::{Controller, Event, Status};

/// The longest delay the settings accept, an hour.
pub const MAX_AUTO_SECONDS: u32 = 3600;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    pub enabled: bool,
    /// Hide the icons again this long after they were shown; 0 never does.
    pub auto_seconds: u32,
    /// Show the icons only when the arrow is clicked. Off, resting the pointer on the arrow shows them too.
    pub click_only: bool,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            enabled: false,
            auto_seconds: 10,
            click_only: true,
        }
    }
}

impl Config {
    /// The rules as they will be used: a hand-edited delay beyond an hour is cut to an hour.
    pub fn rules(&self) -> Rules {
        Rules {
            auto_seconds: self.auto_seconds.min(MAX_AUTO_SECONDS),
            click_only: self.click_only,
        }
    }
}

type Live = Controller<platform::AppKitBar>;

struct Running {
    controller: Rc<Live>,
    /// Another app coming to the front can change how much room the status icons have.
    _front: workspace::Observer,
}

/// The running feature. Created by `attach`, kept alive by the app context.
pub struct Feature {
    mtm: MainThreadMarker,
    running: RefCell<Option<Running>>,
    /// Set while the start at launch is still waiting; the settings page taking over cancels it.
    pending_start: Cell<bool>,
}

/// Starts the feature. Nothing is created while it is off.
pub fn attach(env: &AttachEnv) -> Option<Rc<Feature>> {
    let feature = Rc::new(Feature {
        mtm: env.mtm,
        running: RefCell::new(None),
        pending_start: Cell::new(env.config.menubar.enabled),
    });
    if env.config.menubar.enabled {
        // The "Fig" item is made after the features. Items the system places newest-first, so ours wait a moment
        // to land to the left of it: Fig must never be on the hidden side.
        let (weak, config) = (Rc::downgrade(&feature), env.config.menubar.clone());
        app::after(START_DELAY_SECONDS, move || {
            if let Some(feature) = weak.upgrade()
                && feature.pending_start.replace(false)
                && let Err(fault) = feature.configure(&config)
            {
                eprintln!("menubar: {}", fault.detail);
            }
        });
    }
    Some(feature)
}

const START_DELAY_SECONDS: f64 = 0.5;

impl Feature {
    fn start(&self, config: &Config) {
        let language = crate::i18n::current();
        let mtm = self.mtm;
        let controller = Rc::new_cyclic(|me: &Weak<Live>| {
            let event = |me: &Weak<Live>, event: Event| {
                let me = me.clone();
                move || {
                    if let Some(controller) = me.upgrade() {
                        controller.event(event);
                    }
                }
            };
            let hider = Hider::new(
                mtm,
                macos_sys::menubar::Labels {
                    show: language.menubar_show().to_owned(),
                    hide: language.menubar_hide().to_owned(),
                },
                SEPARATOR_WIDTH,
                event(me, Event::Click),
                event(me, Event::Hover),
                event(me, Event::Screens),
            );
            Controller::new(
                platform::AppKitBar::new(mtm, hider),
                config.rules(),
                me.clone(),
            )
        });
        let observed = Rc::downgrade(&controller);
        let front = workspace::observe_frontmost(move |_| {
            if let Some(controller) = observed.upgrade() {
                controller.event(Event::AppSwitched);
            }
        });
        *self.running.borrow_mut() = Some(Running {
            controller,
            _front: front,
        });
    }

    /// Applies the settings after a change. An error is shown on the settings page and the change is undone.
    pub fn configure(&self, config: &Config) -> Result<(), Fault> {
        self.pending_start.set(false);
        if !config.enabled {
            // Dropping the controller drops the two status items, which gives every icon back.
            self.running.borrow_mut().take();
            return Ok(());
        }
        let running = self.running.borrow();
        match running.as_ref() {
            Some(running) => running.controller.configure(config.rules()),
            None => {
                drop(running);
                self.start(config);
            }
        }
        Ok(())
    }

    /// Releases the global shortcuts while the settings page records a new one. There are none.
    pub fn suspend_hotkeys(&self) {}

    /// Registers the global shortcuts again from the config. There are none.
    pub fn resume_hotkeys(&self, _config: &Config) {}

    /// Facts only the running app knows, merged into this feature's part of the page state.
    pub fn runtime(&self) -> Value {
        let status = match self
            .running
            .borrow()
            .as_ref()
            .map(|r| r.controller.status())
        {
            None => "off",
            Some(Status::Showing) => "showing",
            Some(Status::Hiding) => "hiding",
            Some(Status::Measuring) => "measuring",
            Some(Status::Unavailable) => "unavailable",
        };
        json!({ "status": status })
    }

    /// A button of the settings page. `None` when the name is not this feature's.
    pub fn action(&self, name: &str, _config: &mut Config) -> Option<Result<(), Fault>> {
        (name == "menubar_toggle").then(|| {
            self.toggle();
            Ok(())
        })
    }

    fn toggle(&self) {
        if let Some(running) = self.running.borrow().as_ref() {
            running.controller.event(Event::Click);
        }
    }

    fn collapsed(&self) -> bool {
        self.running
            .borrow()
            .as_ref()
            .is_some_and(|r| r.controller.is_collapsed())
    }

    fn running(&self) -> bool {
        self.running.borrow().is_some()
    }

    /// One entry in the "Fig" menu, only while the feature is on: a way to show the icons again even when the
    /// arrow itself has been dragged to the hidden side.
    pub fn menu_entries(this: &Rc<Self>) -> Vec<Entry> {
        let (toggle, title, visible) = (Rc::clone(this), Rc::clone(this), Rc::clone(this));
        vec![Entry::Item(
            Item::new("", move || toggle.toggle())
                .titled(move || {
                    let language = crate::i18n::current();
                    if title.collapsed() {
                        language.menubar_show().to_owned()
                    } else {
                        language.menubar_hide().to_owned()
                    }
                })
                .visible_when(move || visible.running()),
        )]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn it_is_off_by_default_and_hides_after_ten_seconds_on_a_click_only() {
        let config = Config::default();
        assert!(!config.enabled);
        assert_eq!(
            config.rules(),
            Rules {
                auto_seconds: 10,
                click_only: true
            }
        );
    }

    #[test]
    fn a_config_file_with_only_the_switch_gets_the_defaults() {
        let config: Config = serde_json::from_str(r#"{ "enabled": true }"#).unwrap();
        assert!(config.enabled && config.auto_seconds == 10 && config.click_only);
    }

    #[test]
    fn a_hand_edited_delay_is_cut_to_an_hour() {
        let config: Config = serde_json::from_str(r#"{ "auto_seconds": 4000000000 }"#).unwrap();
        assert_eq!(config.rules().auto_seconds, MAX_AUTO_SECONDS);
        // A negative number is not a delay; the file is read with the default instead of failing.
        assert!(serde_json::from_str::<Config>(r#"{ "auto_seconds": -5 }"#).is_err());
    }
}
