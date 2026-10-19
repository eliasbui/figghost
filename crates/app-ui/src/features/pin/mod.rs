//! Always on top: a shortcut, or the switch in the menu bar panel, keeps the window in front above the others, with
//! a red border round it. macOS lets a program raise only its own windows, so while another window covers the
//! pinned one, a picture of it, taken again many times a second, shows over the cover, and a click on the picture
//! brings the real window forward. One window is pinned at a time; the shortcut lets go of it.
//!
//! Needs Accessibility, to find the window in front and to bring it forward, and Screen Recording, for the
//! pictures, which are kept in memory only.

use super::AttachEnv;
use crate::{config::parse_hotkey, i18n, settings::model::Fault, trace};
use core_window::{
    Rect,
    pin::{self, State},
};
use macos_sys::{
    MainThreadMarker,
    app::{self, Timer},
    capture_ui,
    hotkey::Hotkey,
    permissions,
    pin_ui::{self, PinPanel, PinShots},
    screen_capture,
    status_item::{Entry, Item},
    window::{Frame, Window},
    workspace,
};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::{
    cell::{Cell, RefCell},
    process,
    rc::{Rc, Weak},
    time::Duration,
};

pub mod settings;
mod text;

pub const DEFAULT_SHORTCUT: &str = "ctrl+opt+cmd+p";
/// How often the pinned window is looked at, in seconds: the longest a cover can stay uncovered by its picture.
const LOOK_SECONDS: f64 = 0.05;
/// The least time between two pictures while the window is covered, and while it is not. A picture is kept ready
/// then, so that the one that first shows over a cover is a recent one.
const COVERED_EVERY: Duration = Duration::from_millis(60);
const READY_EVERY: Duration = Duration::from_secs(1);
/// How long a call into the pinned app may take, so that an app that hangs cannot hang this one.
const AX_SECONDS: f32 = 0.3;
/// How many looks in a row (one second) must find the window missing from the system's list before it counts as
/// closed: a locked screen or a change of user can empty the list for a moment.
const GONE_LOOKS: u32 = 20;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    pub enabled: bool,
    /// Empty leaves the feature to the menu bar panel.
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
    /// A config file can say anything; whatever is unusable is replaced by the default.
    pub fn sanitized(mut self) -> Self {
        if !self.shortcut.is_empty() && parse_hotkey(&self.shortcut).is_err() {
            self.shortcut = DEFAULT_SHORTCUT.into();
        }
        self
    }
}

fn own_pid() -> i32 {
    i32::try_from(process::id()).unwrap_or(-1)
}

fn rect(f: Frame) -> Rect {
    Rect::new(f.x, f.y, f.w, f.h)
}

fn frame(r: Rect) -> Frame {
    Frame {
        x: r.x,
        y: r.y,
        w: r.w,
        h: r.h,
    }
}

fn listed(w: pin_ui::Listed) -> pin::Listed {
    pin::Listed {
        id: w.id,
        pid: w.pid,
        layer: w.layer,
        frame: rect(w.frame),
        alpha: w.alpha,
    }
}

/// Looks at the pinned window many times a second and moves the border and the picture to match.
struct Follow {
    mtm: MainThreadMarker,
    id: u32,
    panel: PinPanel,
    /// Ready once ScreenCaptureKit has found the window.
    shots: RefCell<Option<PinShots>>,
    /// The size the pictures are made for.
    size: Cell<(f64, f64)>,
    /// The looks in a row that found the window missing.
    missing: Cell<u32>,
}

impl Follow {
    /// One look. Returns false once the window is gone.
    fn look(&self) -> bool {
        let windows: Vec<pin::Listed> = pin_ui::on_screen_windows()
            .into_iter()
            .map(listed)
            .collect();
        let exists = windows.iter().any(|w| w.id == self.id) || pin_ui::window_exists(self.id);
        let state = pin::state(&windows, exists, self.id, own_pid());
        self.missing.set(if state == State::Gone {
            self.missing.get() + 1
        } else {
            0
        });
        let (at, covered) = match state {
            State::Gone => {
                self.panel.hide();
                return self.missing.get() < GONE_LOOKS;
            }
            // Minimised, its app hidden, or on another desktop: nothing to show until it is back.
            State::Away => {
                self.panel.hide();
                return true;
            }
            State::Clear(at) => (at, false),
            State::Covered(at) => (at, true),
        };
        if let Some(shots) = &*self.shots.borrow() {
            let size = (at.w, at.h);
            if self.size.replace(size) != size {
                shots.resize(size, pin_ui::scale_at(self.mtm, frame(at)));
            }
            let every = if covered { COVERED_EVERY } else { READY_EVERY };
            shots.capture(&self.panel.copy_layer(), every);
        }
        self.panel.show(frame(at), covered, true);
        true
    }
}

/// The window that is pinned and what keeps it on top.
struct Pinned {
    /// The name the settings page shows.
    name: String,
    _follow: Rc<Follow>,
    _timer: Timer,
}

pub struct Feature {
    mtm: MainThreadMarker,
    config: RefCell<Config>,
    hotkey: RefCell<Option<Hotkey>>,
    pinned: RefCell<Option<Pinned>>,
    me: Weak<Feature>,
}

/// Starts the feature; the global shortcut is registered when it is switched on.
pub fn attach(env: &AttachEnv) -> Option<Rc<Feature>> {
    let config = env.config.pin.clone().sanitized();
    let feature = Rc::new_cyclic(|me| Feature {
        mtm: env.mtm,
        config: RefCell::new(config.clone()),
        hotkey: RefCell::new(None),
        pinned: RefCell::new(None),
        me: me.clone(),
    });
    if let Err(fault) = feature.configure(&config) {
        eprintln!("pin: cannot register its shortcut ({})", fault.detail);
    }
    Some(feature)
}

/// "App - window title", as far as either is known.
fn describe(window: &Window) -> String {
    let app = window
        .pid()
        .and_then(workspace::running_app)
        .map(|a| a.name)
        .filter(|n| !n.is_empty());
    let title = window.title().filter(|t| !t.is_empty());
    match (app, title) {
        (Some(app), Some(title)) => format!("{app} - {title}"),
        (Some(one), None) | (None, Some(one)) => one,
        (None, None) => i18n::current().pin_unnamed().to_owned(),
    }
}

impl Feature {
    /// Applies the settings: registers or releases the shortcut, and lets go of the window when turned off.
    pub fn configure(&self, config: &Config) -> Result<(), Fault> {
        *self.config.borrow_mut() = config.clone();
        self.hotkey.borrow_mut().take();
        if !config.enabled {
            self.release();
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
            if let Some(feature) = weak.upgrade() {
                feature.toggle();
            }
        })
        .map_err(|status| Fault::new("taken", format!("pin (status {status})")))?;
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
            eprintln!("pin: cannot register its shortcut again ({})", fault.detail);
        }
    }

    pub fn runtime(&self) -> Value {
        json!({
            "accessibility": permissions::accessibility_trusted(false),
            "screen_access": screen_capture::screen_access(false),
            "pinned": self.pinned.borrow().as_ref().map(|p| p.name.clone()),
        })
    }

    /// The buttons of the settings page.
    pub fn action(&self, name: &str, _config: &mut Config) -> Option<Result<(), Fault>> {
        match name {
            "pin_release" => self.release(),
            "pin_screen_access" => {
                // The system's own dialog appears the first time only; after that the list in System Settings is
                // the only place to switch it on.
                if !screen_capture::screen_access(true) {
                    workspace::open_url(screen_capture::SCREEN_RECORDING_PANE);
                }
            }
            _ => return None,
        }
        Some(Ok(()))
    }

    /// The switch of the menu bar panel: on while a window is pinned.
    pub fn menu_entries(this: &Rc<Self>) -> Vec<Entry> {
        let (toggle, pinned, shown) = (
            Rc::downgrade(this),
            Rc::downgrade(this),
            Rc::downgrade(this),
        );
        let item = Item::new(i18n::current().pin_menu(), move || {
            if let Some(feature) = toggle.upgrade() {
                feature.toggle();
            }
        })
        .symbol(crate::menu::symbols::PIN)
        .checked(move || {
            pinned
                .upgrade()
                .is_some_and(|f| f.pinned.borrow().is_some())
        })
        .visible_when(move || shown.upgrade().is_some_and(|f| f.config.borrow().enabled));
        vec![Entry::Item(item)]
    }

    /// The shortcut and the switch both come here: lets go of the pinned window, or pins the one in front.
    pub fn toggle(&self) {
        if self.pinned.borrow().is_some() {
            self.release();
        } else {
            self.pin_front();
        }
    }

    fn release(&self) {
        let old = self.pinned.borrow_mut().take();
        if old.is_some() {
            trace::log("pin: let go");
        }
    }

    fn tell(&self, text: &str) {
        capture_ui::toast(self.mtm, text);
    }

    fn pin_front(&self) {
        let language = i18n::current();
        if !permissions::accessibility_trusted(false) {
            self.tell(language.pin_needs_accessibility());
            return;
        }
        if !screen_capture::capture_supported() {
            self.tell(language.pin_needs_newer_macos());
            return;
        }
        if !screen_capture::screen_access(false) {
            screen_capture::screen_access(true);
            self.tell(language.pin_needs_screen_recording());
            return;
        }
        let Some(window) = Window::focused() else {
            self.tell(language.pin_no_window());
            return;
        };
        window.set_timeout(AX_SECONDS);
        if window.pid() == Some(own_pid()) {
            self.tell(language.pin_own_window());
            return;
        }
        if window.is_full_screen() {
            self.tell(language.pin_full_screen());
            return;
        }
        let (Some(id), Some(at)) = (window.id(), window.frame()) else {
            self.tell(language.pin_cannot());
            return;
        };
        trace::log(&format!("pin: window {id} at {at:?}"));

        let name = describe(&window);
        let follow = Rc::new(Follow {
            mtm: self.mtm,
            id,
            // The window comes forward as it does when a person clicks it, and its picture goes.
            panel: PinPanel::new(self.mtm, move || {
                window.raise();
            }),
            shots: RefCell::new(None),
            size: Cell::new((at.w, at.h)),
            missing: Cell::new(0),
        });
        let (weak_follow, weak_me) = (Rc::downgrade(&follow), self.me.clone());
        PinShots::prepare(id, move |prepared| {
            // Let go of before the answer came, or the feature was turned off: nothing is waiting for it.
            let Some(follow) = weak_follow.upgrade() else {
                return;
            };
            match prepared {
                Ok(shots) => *follow.shots.borrow_mut() = Some(shots),
                Err(why) => {
                    trace::log(&format!("pin: no pictures ({why})"));
                    if let Some(feature) = weak_me.upgrade() {
                        feature.release();
                        feature.tell(i18n::current().pin_cannot());
                    }
                }
            }
        });
        follow.look();
        let timer = {
            let (follow, weak) = (Rc::clone(&follow), self.me.clone());
            app::every(LOOK_SECONDS, move || {
                if !follow.look() {
                    trace::log("pin: the window is gone");
                    // A timer cannot be dropped from inside its own callback.
                    let weak = weak.clone();
                    app::after(0.0, move || {
                        if let Some(feature) = weak.upgrade() {
                            feature.release();
                        }
                    });
                }
            })
        };
        *self.pinned.borrow_mut() = Some(Pinned {
            name,
            _follow: follow,
            _timer: timer,
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_config_file_cannot_hold_an_unusable_shortcut() {
        let config = Config {
            shortcut: "nonsense".into(),
            ..Config::default()
        }
        .sanitized();
        assert_eq!(config, Config::default());
        let removed = Config {
            shortcut: String::new(),
            ..Config::default()
        };
        assert_eq!(
            removed.clone().sanitized(),
            removed,
            "no shortcut is allowed"
        );
    }

    #[test]
    fn a_config_without_the_feature_leaves_it_off() {
        let config: Config = serde_json::from_str("{}").unwrap();
        assert_eq!(config, Config::default());
        assert!(!config.enabled);
    }
}
