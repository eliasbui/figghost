//! The command palette: one shortcut opens a small window with a field to type in; what is typed picks a command, such as a
//! tab of the notch, a page of the settings, a feature to turn on or off, the agent that waits, a speed test of a connection
//! or a reminder written in a few words. The list is made by the page from what the app says it has (see
//! `app-tauri/src/pages/palette`); this side keeps the shortcut, and the address `figghost://...` which asks for a few of the
//! same things (`url.rs`).

use super::AttachEnv;
use crate::{config::parse_hotkey, settings::model::Fault};
use macos_sys::{MainThreadMarker, hotkey::Hotkey};
use serde::{Deserialize, Serialize};
use std::{
    cell::RefCell,
    rc::{Rc, Weak},
};

pub mod settings;
pub mod url;

pub const DEFAULT_SHORTCUT: &str = "ctrl+opt+cmd+c";

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    pub enabled: bool,
    pub shortcut: String,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            enabled: false,
            shortcut: DEFAULT_SHORTCUT.into(),
        }
    }
}

impl Config {
    pub fn sanitized(mut self) -> Self {
        if !self.shortcut.is_empty() && parse_hotkey(&self.shortcut).is_err() {
            self.shortcut = DEFAULT_SHORTCUT.into();
        }
        self
    }
}

pub struct Feature {
    mtm: MainThreadMarker,
    config: RefCell<Config>,
    hotkey: RefCell<Option<Hotkey>>,
    me: Weak<Feature>,
}

pub fn attach(env: &AttachEnv) -> Option<Rc<Feature>> {
    let config = env.config.palette.clone().sanitized();
    let feature = Rc::new_cyclic(|me| Feature {
        mtm: env.mtm,
        config: RefCell::new(config.clone()),
        hotkey: RefCell::new(None),
        me: me.clone(),
    });
    if let Err(fault) = feature.configure(&config) {
        eprintln!("palette: cannot register its shortcut ({})", fault.detail);
    }
    Some(feature)
}

impl Feature {
    pub fn configure(&self, config: &Config) -> Result<(), Fault> {
        *self.config.borrow_mut() = config.clone();
        self.hotkey.borrow_mut().take();
        if !config.enabled {
            return Ok(());
        }
        self.register_hotkey(config)
    }

    fn register_hotkey(&self, config: &Config) -> Result<(), Fault> {
        if config.shortcut.is_empty() {
            return Ok(());
        }
        let spec = parse_hotkey(&config.shortcut).map_err(|e| Fault::new("invalid_shortcut", e))?;
        let weak = self.me.clone();
        let hotkey = Hotkey::register(self.mtm, spec.key_code, spec.modifiers, move || {
            if weak.upgrade().is_some() {
                crate::settings::open_palette();
            }
        })
        .map_err(|status| Fault::new("taken", format!("palette (status {status})")))?;
        *self.hotkey.borrow_mut() = Some(hotkey);
        Ok(())
    }

    pub fn suspend_hotkeys(&self) {
        self.hotkey.borrow_mut().take();
    }

    pub fn resume_hotkeys(&self, config: &Config) {
        if config.enabled
            && let Err(fault) = self.register_hotkey(config)
        {
            eprintln!(
                "palette: cannot register its shortcut again ({})",
                fault.detail
            );
        }
    }

    pub fn enabled(&self) -> bool {
        self.config.borrow().enabled
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn it_is_off_by_default_and_a_hostile_shortcut_is_the_usual_one() {
        let config = Config::default();
        assert!(!config.enabled && config.shortcut == DEFAULT_SHORTCUT);
        assert_eq!(
            Config {
                shortcut: "nonsense".into(),
                ..Config::default()
            }
            .sanitized()
            .shortcut,
            DEFAULT_SHORTCUT
        );
        assert!(
            Config {
                shortcut: String::new(),
                ..Config::default()
            }
            .sanitized()
            .shortcut
            .is_empty()
        );
    }
}
