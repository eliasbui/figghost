//! A mirror and a teleprompter for the camera at the top of the screen. The mirror shows the camera, mirrored, in a window
//! you can drag, under the notch; the teleprompter scrolls the note you wrote last in a strip just under the menu bar, as
//! near to the lens as it can be, and can be left out of screenshots and recordings. Each has a shortcut; while the
//! teleprompter runs, Control, Option and Command with Space, Up, Down and Left pause, speed up, slow down and restart it.
//! The camera is opened only while the mirror shows, and what it sees is drawn on the screen and kept nowhere.

use super::AttachEnv;
use crate::{config::parse_hotkey, settings::model::Fault, trace};
use macos_sys::{
    MainThreadMarker, NSPoint, NSRect, NSSize,
    hotkey::Hotkey,
    permissions,
    record_ui::{self, CameraBubble},
    teleprompter::{Strip, Teleprompter, pointer_screen},
};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::{
    cell::RefCell,
    path::PathBuf,
    rc::{Rc, Weak},
};

pub mod logic;
pub mod settings;

pub const DEFAULT_MIRROR: &str = "ctrl+opt+cmd+g";
pub const DEFAULT_PROMPTER: &str = "ctrl+opt+cmd+y";
/// The keys that drive the teleprompter while it runs.
const PROMPTER_KEYS: [(&str, &str); 4] = [
    ("space", "pause"),
    ("up", "faster"),
    ("down", "slower"),
    ("left", "restart"),
];
const MIRROR_GAP: f64 = 6.0;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    pub enabled: bool,
    pub mirror_shortcut: String,
    pub prompter_shortcut: String,
    /// 200, 280 or 360 points.
    pub mirror_size: u32,
    /// The letters of the teleprompter: 28, 36, 48 or 60 points.
    pub text_size: u32,
    /// 1 to 6.
    pub speed: u32,
    /// The teleprompter is left out of screenshots and recordings.
    pub hide_in_capture: bool,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            enabled: false,
            mirror_shortcut: DEFAULT_MIRROR.into(),
            prompter_shortcut: DEFAULT_PROMPTER.into(),
            mirror_size: 280,
            text_size: 36,
            speed: 3,
            hide_in_capture: true,
        }
    }
}

impl Config {
    /// A config file can say anything; whatever is not offered is the usual.
    pub fn sanitized(mut self) -> Self {
        let defaults = Self::default();
        for (shortcut, usual) in [
            (&mut self.mirror_shortcut, defaults.mirror_shortcut),
            (&mut self.prompter_shortcut, defaults.prompter_shortcut),
        ] {
            if !shortcut.is_empty() && parse_hotkey(shortcut).is_err() {
                *shortcut = usual;
            }
        }
        if !logic::MIRROR_SIZES.contains(&self.mirror_size) {
            self.mirror_size = defaults.mirror_size;
        }
        if !logic::TEXT_SIZES.contains(&self.text_size) {
            self.text_size = defaults.text_size;
        }
        if !(1..=6).contains(&self.speed) {
            self.speed = defaults.speed;
        }
        self
    }
}

pub struct Feature {
    mtm: MainThreadMarker,
    home: PathBuf,
    config: RefCell<Config>,
    hotkeys: RefCell<Vec<Hotkey>>,
    mirror: RefCell<Option<CameraBubble>>,
    prompter: Teleprompter,
    /// Held only while the teleprompter runs.
    prompter_keys: RefCell<Vec<Hotkey>>,
    me: Weak<Feature>,
}

pub fn attach(env: &AttachEnv) -> Option<Rc<Feature>> {
    let config = env.config.mirror.clone().sanitized();
    let feature = Rc::new_cyclic(|me| Feature {
        mtm: env.mtm,
        home: env.home.to_owned(),
        config: RefCell::new(config.clone()),
        hotkeys: RefCell::default(),
        mirror: RefCell::default(),
        prompter: Teleprompter::new(env.mtm),
        prompter_keys: RefCell::default(),
        me: me.clone(),
    });
    if let Err(fault) = feature.configure(&config) {
        eprintln!("mirror: cannot register its shortcuts ({})", fault.detail);
    }
    Some(feature)
}

impl Feature {
    pub fn configure(&self, config: &Config) -> Result<(), Fault> {
        *self.config.borrow_mut() = config.clone();
        self.hotkeys.borrow_mut().clear();
        if !config.enabled {
            self.close_all();
            return Ok(());
        }
        self.register_hotkeys(config)
    }

    fn register_hotkeys(&self, config: &Config) -> Result<(), Fault> {
        let mut keys = Vec::new();
        for (shortcut, mirror) in [
            (&config.mirror_shortcut, true),
            (&config.prompter_shortcut, false),
        ] {
            if shortcut.is_empty() {
                continue;
            }
            let spec = parse_hotkey(shortcut).map_err(|e| Fault::new("invalid_shortcut", e))?;
            let weak = self.me.clone();
            let key = Hotkey::register(self.mtm, spec.key_code, spec.modifiers, move || {
                if let Some(f) = weak.upgrade() {
                    if mirror {
                        f.toggle_mirror()
                    } else {
                        f.toggle_prompter()
                    }
                }
            })
            .map_err(|status| Fault::new("taken", format!("mirror (status {status})")))?;
            keys.push(key);
        }
        *self.hotkeys.borrow_mut() = keys;
        Ok(())
    }

    pub fn suspend_hotkeys(&self) {
        self.hotkeys.borrow_mut().clear();
    }

    pub fn resume_hotkeys(&self, config: &Config) {
        if config.enabled
            && let Err(fault) = self.register_hotkeys(config)
        {
            eprintln!(
                "mirror: cannot register its shortcuts again ({})",
                fault.detail
            );
        }
    }

    pub fn runtime(&self) -> Value {
        json!({
            "camera": permissions::camera_trusted(),
            "has_note": logic::newest_note(&self.home.join("notes")).is_some(),
            "running": { "mirror": self.mirror.borrow().is_some(), "prompter": self.prompter.is_visible() },
        })
    }

    pub fn action(&self, name: &str) -> Option<Result<(), Fault>> {
        match name {
            "mirror_toggle" => self.toggle_mirror(),
            "mirror_prompter" => self.toggle_prompter(),
            "open_camera_settings" => permissions::open_pane(permissions::CAMERA_PANE),
            _ => return None,
        }
        Some(Ok(()))
    }

    fn close_all(&self) {
        self.close_mirror();
        self.stop_prompter();
    }

    // -- the mirror ---------------------------------------------------------------------------------------------------

    fn close_mirror(&self) {
        if let Some(mirror) = self.mirror.borrow_mut().take() {
            mirror.close();
        }
    }

    fn toggle_mirror(&self) {
        if self.mirror.borrow().is_some() {
            self.close_mirror();
            return;
        }
        let weak = self.me.clone();
        record_ui::camera_access(move |granted| {
            let Some(f) = weak.upgrade() else { return };
            if !granted {
                trace::log("mirror: no access to the camera");
                f.say_no_camera();
                return;
            }
            f.open_mirror();
        });
    }

    fn open_mirror(&self) {
        let Some(visible) = pointer_screen(self.mtm) else {
            return;
        };
        let size = f64::from(self.config.borrow().mirror_size);
        // Under the notch, in the middle of the top of the screen, where the eyes already are.
        let frame = NSRect::new(
            NSPoint::new(
                visible.origin.x + (visible.size.width - size) / 2.0,
                visible.origin.y + visible.size.height - size - MIRROR_GAP,
            ),
            NSSize::new(size, size),
        );
        match CameraBubble::show_in(self.mtm, frame, false) {
            Some(mirror) => {
                trace::log("mirror: shown");
                *self.mirror.borrow_mut() = Some(mirror);
            }
            None => self.say_no_camera(),
        }
    }

    /// The settings page shows whether the camera may be used; a person who has not been asked yet is asked now.
    fn say_no_camera(&self) {
        if !permissions::camera_trusted() {
            permissions::camera_request();
        }
    }

    // -- the teleprompter -----------------------------------------------------------------------------------------------

    fn stop_prompter(&self) {
        self.prompter.hide();
        self.prompter_keys.borrow_mut().clear();
    }

    fn toggle_prompter(&self) {
        if self.prompter.is_visible() {
            self.stop_prompter();
            return;
        }
        let Some(text) = logic::newest_note(&self.home.join("notes")) else {
            trace::log("mirror: no note to read");
            return;
        };
        let Some(visible) = pointer_screen(self.mtm) else {
            return;
        };
        let config = self.config.borrow().clone();
        let size = f64::from(config.text_size);
        let height = (size * 4.6).clamp(180.0, 360.0);
        let strip = Strip {
            text: &text,
            size,
            width: 900.0,
            height,
            speed: logic::speed_of(config.speed),
            hidden: config.hide_in_capture,
        };
        self.prompter.show(visible, &strip);
        trace::log("mirror: the teleprompter runs");
        self.hold_prompter_keys();
    }

    /// While the strip shows, Control, Option and Command with a key drive it; they are let go with it.
    fn hold_prompter_keys(&self) {
        let mut keys = Vec::new();
        for (name, verb) in PROMPTER_KEYS {
            let Ok(spec) = parse_hotkey(&format!("ctrl+opt+cmd+{name}")) else {
                continue;
            };
            let weak = self.me.clone();
            let registered = Hotkey::register(self.mtm, spec.key_code, spec.modifiers, move || {
                if let Some(f) = weak.upgrade() {
                    f.drive(verb);
                }
            });
            match registered {
                Ok(key) => keys.push(key),
                Err(status) => eprintln!("mirror: the key for {verb} is taken (status {status})"),
            }
        }
        *self.prompter_keys.borrow_mut() = keys;
    }

    fn drive(&self, verb: &str) {
        match verb {
            "pause" => {
                self.prompter.toggle_pause();
            }
            "faster" => self.prompter.set_speed(self.prompter.speed() * 1.25),
            "slower" => self.prompter.set_speed(self.prompter.speed() / 1.25),
            "restart" => self.prompter.restart(),
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn it_is_off_by_default_and_a_config_file_can_say_anything() {
        let c = Config::default();
        assert!(!c.enabled && c.hide_in_capture && c.speed == 3);
        let odd = Config {
            mirror_shortcut: "nonsense".into(),
            prompter_shortcut: "x".into(),
            mirror_size: 5,
            text_size: 1,
            speed: 99,
            ..Config::default()
        }
        .sanitized();
        assert_eq!(
            (
                odd.mirror_shortcut.as_str(),
                odd.prompter_shortcut.as_str(),
                odd.mirror_size,
                odd.text_size,
                odd.speed
            ),
            (DEFAULT_MIRROR, DEFAULT_PROMPTER, 280, 36, 3)
        );
        let none = Config {
            mirror_shortcut: String::new(),
            ..Config::default()
        }
        .sanitized();
        assert!(none.mirror_shortcut.is_empty(), "no shortcut is a choice");
    }

    #[test]
    fn the_keys_that_drive_the_teleprompter_are_shortcuts_that_can_be_parsed() {
        for (name, _) in PROMPTER_KEYS {
            assert!(
                parse_hotkey(&format!("ctrl+opt+cmd+{name}")).is_ok(),
                "{name}"
            );
        }
    }
}
