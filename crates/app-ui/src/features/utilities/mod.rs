//! Small tools for the Mac itself: keep it awake, mute the microphone for every app at once, lock the keyboard
//! while it is wiped, switch the dark appearance, hide the icons on the desktop, and copy the link of the tab in front
//! of a browser. Each has a shortcut, most have a place in the menu bar panel, and each says what it did on the notch.
//!
//! Keeping awake is a power assertion and holds only while the app runs. Muting the microphone is done on the
//! input device itself, so no app has a button to find. The keyboard lock swallows keys in an event tap, which
//! needs Accessibility, and always has three ways out: Command and Escape, the length chosen, and quitting. The
//! appearance and the link go through AppleScript (`macos_sys::desktop`, `macos_sys::browser`), off the main thread,
//! and macOS asks once whether Figghost may control System Events or that browser.

use super::{AttachEnv, islands};
use crate::{
    config::parse_hotkey,
    i18n::{self, Language},
    menu::symbols,
    settings::model::Fault,
};
use macos_sys::{
    MainThreadMarker, app,
    awake::Awake,
    browser, desktop,
    hotkey::Hotkey,
    keylock::KeyLock,
    notch::{Activity, Detail, Ear, Notch},
    pasteboard::{Pasteboard, Write},
    permissions,
    status_item::{Entry, Item},
    style::Tone,
    volume, workspace,
};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::{
    cell::{Cell, RefCell},
    rc::{Rc, Weak},
    thread,
    time::{Duration, Instant},
};

pub mod logic;
pub mod settings;
mod text;

const SLOT_AWAKE: &str = "util-awake";
const SLOT_MIC: &str = "util-mic";
const SLOT_LOCK: &str = "util-lock";
const SLOT_SNOOZE: &str = "util-snooze";
const SLOT_LOOK: &str = "util-look";
const SLOT_LINK: &str = "util-link";
const PRIORITY_MIC: u8 = 28;
const PRIORITY_AWAKE: u8 = 22;
const PRIORITY_LOCK: u8 = 29;
const PRIORITY_SNOOZE: u8 = 23;
const PRIORITY_LOOK: u8 = 24;
const PRIORITY_LINK: u8 = 27;
/// A person asked for the link, so the first ask may wait for them to answer macOS.
const LINK_TIMEOUT: Duration = Duration::from_secs(30);
const SHOW_SECONDS: f64 = 3.0;

pub const DEFAULT_MIC_SHORTCUT: &str = "ctrl+opt+cmd+m";
pub const DEFAULT_LOCK_SHORTCUT: &str = "ctrl+opt+cmd+k";
pub const DEFAULT_LINK_SHORTCUT: &str = "ctrl+opt+cmd+l";

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    pub enabled: bool,
    /// Keep the screen lit too, not only the Mac awake.
    pub awake_display: bool,
    /// How long a keep-awake from the shortcut lasts; 0 is until it is stopped.
    pub awake_minutes: u32,
    /// How long the keyboard stays locked at most.
    pub lock_seconds: u32,
    /// How long a snooze of the notch lasts; 0 is until it is woken.
    pub snooze_minutes: u32,
    pub awake_shortcut: String,
    pub mic_shortcut: String,
    pub lock_shortcut: String,
    pub snooze_shortcut: String,
    pub dark_shortcut: String,
    pub desktop_shortcut: String,
    pub link_shortcut: String,
    /// How the link of a tab is written: one of `logic::LINK_FORMATS`.
    pub link_format: String,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            enabled: false,
            awake_display: true,
            awake_minutes: 60,
            lock_seconds: 60,
            snooze_minutes: 30,
            awake_shortcut: String::new(),
            mic_shortcut: DEFAULT_MIC_SHORTCUT.into(),
            lock_shortcut: DEFAULT_LOCK_SHORTCUT.into(),
            snooze_shortcut: String::new(),
            dark_shortcut: String::new(),
            desktop_shortcut: String::new(),
            link_shortcut: DEFAULT_LINK_SHORTCUT.into(),
            link_format: logic::LINK_FORMATS[0].into(),
        }
    }
}

impl Config {
    /// A config file can say anything; whatever is unusable is replaced by the default.
    pub fn sanitized(mut self) -> Self {
        let defaults = Self::default();
        self.awake_minutes = logic::awake_minutes(self.awake_minutes);
        self.snooze_minutes = logic::snooze_minutes(self.snooze_minutes);
        self.lock_seconds = self
            .lock_seconds
            .clamp(logic::LOCK_SECONDS.0, logic::LOCK_SECONDS.1);
        self.link_format = logic::link_format(&self.link_format).into();
        for (value, default) in [
            (&mut self.awake_shortcut, defaults.awake_shortcut),
            (&mut self.mic_shortcut, defaults.mic_shortcut),
            (&mut self.lock_shortcut, defaults.lock_shortcut),
            (&mut self.snooze_shortcut, defaults.snooze_shortcut),
            (&mut self.dark_shortcut, defaults.dark_shortcut),
            (&mut self.desktop_shortcut, defaults.desktop_shortcut),
            (&mut self.link_shortcut, defaults.link_shortcut),
        ] {
            if !value.is_empty() && parse_hotkey(value).is_err() {
                *value = default;
            }
        }
        self
    }
}

/// What a shortcut does.
type Action = fn(&Feature);

/// A keep-awake that is running: the assertion lives as long as this does.
struct Session {
    _hold: Awake,
    minutes: u32,
    ends: Option<Instant>,
    serial: u64,
}

struct Locked {
    lock: KeyLock,
    ends: Instant,
}

/// A snooze of the notch that is running.
struct Snoozing {
    minutes: u32,
    ends: Option<Instant>,
    serial: u64,
}

pub struct Feature {
    mtm: MainThreadMarker,
    notch: Option<Weak<Notch>>,
    config: RefCell<Config>,
    awake: RefCell<Option<Session>>,
    lock: RefCell<Option<Locked>>,
    snooze: RefCell<Option<Snoozing>>,
    ticker: RefCell<Option<app::Timer>>,
    hotkeys: RefCell<Vec<Hotkey>>,
    serial: Cell<u64>,
    me: Weak<Feature>,
}

thread_local! {
    /// What the appearance, the desktop and a browser answered arrives from another thread and hops to the main one,
    /// which finds the feature here.
    static FEATURE: RefCell<Weak<Feature>> = const { RefCell::new(Weak::new()) };
}

fn with_feature(f: impl FnOnce(&Feature)) {
    FEATURE.with(|slot| {
        if let Some(feature) = slot.borrow().upgrade() {
            f(&feature);
        }
    });
}

/// Starts the feature. It works without a notch, only without the words on it.
pub fn attach(env: &AttachEnv) -> Option<Rc<Feature>> {
    let config = env.config.utilities.clone().sanitized();
    let feature = Rc::new_cyclic(|me| Feature {
        mtm: env.mtm,
        notch: env.notch.map(Rc::downgrade),
        config: RefCell::new(config.clone()),
        awake: RefCell::default(),
        lock: RefCell::default(),
        snooze: RefCell::default(),
        ticker: RefCell::default(),
        hotkeys: RefCell::default(),
        serial: Cell::new(0),
        me: me.clone(),
    });
    if config.enabled
        && let Err(fault) = feature.register_hotkeys(&config)
    {
        eprintln!(
            "utilities: cannot register its shortcuts ({})",
            fault.detail
        );
    }
    FEATURE.with(|slot| *slot.borrow_mut() = Rc::downgrade(&feature));
    Some(feature)
}

impl Feature {
    /// Applies the settings: registers the shortcuts, and lets everything go when it is switched off.
    pub fn configure(&self, config: &Config) -> Result<(), Fault> {
        let config = config.clone().sanitized();
        *self.config.borrow_mut() = config.clone();
        self.hotkeys.borrow_mut().clear();
        if !config.enabled {
            self.stop_awake(false);
            self.unlock(false);
            self.stop_snooze(false);
            return Ok(());
        }
        self.register_hotkeys(&config)
    }

    fn register_hotkeys(&self, config: &Config) -> Result<(), Fault> {
        let mut hotkeys = Vec::new();
        let wanted: [(&str, Action); 7] = [
            (&config.awake_shortcut, Feature::toggle_awake),
            (&config.mic_shortcut, Feature::toggle_mic),
            (&config.lock_shortcut, Feature::toggle_lock),
            (&config.snooze_shortcut, Feature::toggle_snooze),
            (&config.dark_shortcut, Feature::toggle_dark),
            (&config.desktop_shortcut, Feature::toggle_desktop),
            (&config.link_shortcut, Feature::copy_link),
        ];
        for (shortcut, action) in wanted {
            if shortcut.is_empty() {
                continue;
            }
            let spec = parse_hotkey(shortcut).map_err(|e| Fault::new("invalid_shortcut", e))?;
            let weak = self.me.clone();
            let hotkey = Hotkey::register(self.mtm, spec.key_code, spec.modifiers, move || {
                if let Some(feature) = weak.upgrade() {
                    action(&feature);
                }
            })
            .map_err(|status| Fault::new("taken", format!("utilities (status {status})")))?;
            hotkeys.push(hotkey);
        }
        *self.hotkeys.borrow_mut() = hotkeys;
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
                "utilities: cannot register its shortcuts again ({})",
                fault.detail
            );
        }
    }

    fn language(&self) -> Language {
        i18n::current()
    }

    fn say(&self, slot: &'static str, activity: Activity, seconds: Option<f64>) {
        if let Some(notch) = self.notch.as_ref().and_then(Weak::upgrade) {
            islands::show_for(&notch, slot, activity, seconds);
        }
    }

    fn silence(&self, slot: &'static str) {
        if let Some(notch) = self.notch.as_ref().and_then(Weak::upgrade) {
            islands::clear_slot(&notch, slot);
        }
    }

    // -- keep awake -------------------------------------------------------------------------------------------

    pub fn is_awake(&self) -> bool {
        self.awake.borrow().is_some()
    }

    /// Seconds left of a keep-awake that has an end; `None` when there is none or it never ends.
    fn awake_left(&self) -> Option<u64> {
        let session = self.awake.borrow();
        let ends = session.as_ref()?.ends?;
        Some(ends.saturating_duration_since(Instant::now()).as_secs())
    }

    pub fn toggle_awake(&self) {
        if self.is_awake() {
            self.stop_awake(true);
        } else {
            let minutes = self.config.borrow().awake_minutes;
            self.start_awake(minutes);
        }
    }

    pub fn start_awake(&self, minutes: u32) {
        let display = self.config.borrow().awake_display;
        let Some(hold) = Awake::start(display, "Figghost keeps this Mac awake") else {
            eprintln!("utilities: the system refused to keep this Mac awake");
            return;
        };
        let serial = self.serial.get() + 1;
        self.serial.set(serial);
        let ends =
            (minutes > 0).then(|| Instant::now() + Duration::from_secs(u64::from(minutes) * 60));
        *self.awake.borrow_mut() = Some(Session {
            _hold: hold,
            minutes,
            ends,
            serial,
        });
        if minutes > 0 {
            let me = self.me.clone();
            app::after(f64::from(minutes) * 60.0 + 1.0, move || {
                if let Some(feature) = me.upgrade()
                    && feature
                        .awake
                        .borrow()
                        .as_ref()
                        .is_some_and(|s| s.serial == serial)
                {
                    feature.stop_awake(true);
                }
            });
        }
        let language = self.language();
        self.say(
            SLOT_AWAKE,
            Activity {
                left: Ear::Symbol {
                    name: "cup.and.saucer.fill",
                    tone: Tone::Orange,
                    pulse: false,
                },
                right: Ear::Text {
                    text: short_length(minutes),
                    tone: Tone::Orange,
                },
                detail: Some(Detail {
                    title: language.util_awake_on().to_owned(),
                    subtitle: language.util_awake_for(minutes),
                }),
                page: None,
                priority: PRIORITY_AWAKE,
            },
            Some(SHOW_SECONDS),
        );
    }

    /// `say` is false when the feature is being switched off, which has no need of a notice.
    pub fn stop_awake(&self, say: bool) {
        if self.awake.borrow_mut().take().is_none() {
            return;
        }
        if say {
            let language = self.language();
            self.say(
                SLOT_AWAKE,
                Activity {
                    left: Ear::Symbol {
                        name: "cup.and.saucer",
                        tone: Tone::Gray,
                        pulse: false,
                    },
                    right: Ear::Empty,
                    detail: Some(Detail {
                        title: language.util_awake_off().to_owned(),
                        subtitle: String::new(),
                    }),
                    page: None,
                    priority: PRIORITY_AWAKE,
                },
                Some(SHOW_SECONDS),
            );
        } else {
            self.silence(SLOT_AWAKE);
        }
    }

    // -- microphone -------------------------------------------------------------------------------------------

    pub fn toggle_mic(&self) {
        let language = self.language();
        let done = volume::microphone_muted().and_then(|muted| {
            let want = !muted;
            volume::set_microphone_muted(want).then_some(want)
        });
        let (symbol, tone, title, hint) = match done {
            Some(true) => (
                "mic.slash.fill",
                Tone::Red,
                language.util_mic_muted(),
                language.util_mic_hint(),
            ),
            Some(false) => ("mic.fill", Tone::Green, language.util_mic_live(), ""),
            None => ("mic.slash", Tone::Gray, language.util_mic_cannot(), ""),
        };
        self.say(
            SLOT_MIC,
            Activity {
                left: Ear::Symbol {
                    name: symbol,
                    tone,
                    pulse: false,
                },
                right: Ear::Empty,
                detail: Some(Detail {
                    title: title.to_owned(),
                    subtitle: hint.to_owned(),
                }),
                page: None,
                priority: PRIORITY_MIC,
            },
            Some(SHOW_SECONDS),
        );
    }

    // -- keyboard lock ----------------------------------------------------------------------------------------

    pub fn is_locked(&self) -> bool {
        self.lock.borrow().is_some()
    }

    pub fn toggle_lock(&self) {
        if self.is_locked() {
            self.unlock(true);
            return;
        }
        let language = self.language();
        let seconds = u64::from(self.config.borrow().lock_seconds);
        let lock = if permissions::accessibility_trusted(false) {
            KeyLock::start(Duration::from_secs(seconds)).ok()
        } else {
            None
        };
        let Some(lock) = lock else {
            self.say(
                SLOT_LOCK,
                Activity {
                    left: Ear::Symbol {
                        name: "hand.raised.fill",
                        tone: Tone::Orange,
                        pulse: false,
                    },
                    right: Ear::Empty,
                    detail: Some(Detail {
                        title: language.util_lock_needs_access().to_owned(),
                        subtitle: String::new(),
                    }),
                    page: None,
                    priority: PRIORITY_LOCK,
                },
                Some(SHOW_SECONDS),
            );
            return;
        };
        *self.lock.borrow_mut() = Some(Locked {
            lock,
            ends: Instant::now() + Duration::from_secs(seconds),
        });
        self.paint_lock();
        let me = self.me.clone();
        *self.ticker.borrow_mut() = Some(app::every(1.0, move || {
            if let Some(feature) = me.upgrade() {
                feature.tick_lock();
            }
        }));
    }

    fn tick_lock(&self) {
        let done = self
            .lock
            .borrow()
            .as_ref()
            .is_none_or(|l| l.lock.was_released() || Instant::now() >= l.ends);
        if done {
            self.unlock(true);
        } else {
            self.paint_lock();
        }
    }

    /// The countdown, drawn again each second while the keyboard is locked.
    fn paint_lock(&self) {
        let Some(left) = self
            .lock
            .borrow()
            .as_ref()
            .map(|l| l.ends.saturating_duration_since(Instant::now()).as_secs())
        else {
            return;
        };
        let language = self.language();
        self.say(
            SLOT_LOCK,
            Activity {
                left: Ear::Symbol {
                    name: "lock.fill",
                    tone: Tone::Orange,
                    pulse: true,
                },
                right: Ear::Text {
                    text: logic::clock(left),
                    tone: Tone::Orange,
                },
                detail: Some(Detail {
                    title: language.util_lock_title().to_owned(),
                    subtitle: language.util_lock_hint().to_owned(),
                }),
                page: None,
                priority: PRIORITY_LOCK,
            },
            None,
        );
    }

    /// `say` is false when the feature is being switched off.
    pub fn unlock(&self, say: bool) {
        self.ticker.borrow_mut().take();
        if self.lock.borrow_mut().take().is_none() {
            return;
        }
        if say {
            let language = self.language();
            self.say(
                SLOT_LOCK,
                Activity {
                    left: Ear::Symbol {
                        name: "lock.open.fill",
                        tone: Tone::Green,
                        pulse: false,
                    },
                    right: Ear::Empty,
                    detail: Some(Detail {
                        title: language.util_lock_done().to_owned(),
                        subtitle: String::new(),
                    }),
                    page: None,
                    priority: PRIORITY_LOCK,
                },
                Some(SHOW_SECONDS),
            );
        } else {
            self.silence(SLOT_LOCK);
        }
    }

    // -- snooze -----------------------------------------------------------------------------------------------

    pub fn is_snoozed(&self) -> bool {
        self.snooze.borrow().is_some()
    }

    /// Seconds left of a snooze that has an end; `None` when there is none or it never ends.
    fn snooze_left(&self) -> Option<u64> {
        let session = self.snooze.borrow();
        let ends = session.as_ref()?.ends?;
        Some(ends.saturating_duration_since(Instant::now()).as_secs())
    }

    pub fn toggle_snooze(&self) {
        if self.is_snoozed() {
            self.stop_snooze(true);
        } else {
            let minutes = self.config.borrow().snooze_minutes;
            self.start_snooze(minutes);
        }
    }

    /// The notch stops opening when the pointer reaches it, for `minutes` (0 is until it is woken). It still opens
    /// for what asks for the person, such as a coding agent that waits for an answer.
    pub fn start_snooze(&self, minutes: u32) {
        let Some(notch) = self.notch.as_ref().and_then(Weak::upgrade) else {
            return;
        };
        let serial = self.serial.get() + 1;
        self.serial.set(serial);
        let ends =
            (minutes > 0).then(|| Instant::now() + Duration::from_secs(u64::from(minutes) * 60));
        *self.snooze.borrow_mut() = Some(Snoozing {
            minutes,
            ends,
            serial,
        });
        notch.set_snoozed(true);
        if minutes > 0 {
            let me = self.me.clone();
            app::after(f64::from(minutes) * 60.0 + 1.0, move || {
                if let Some(feature) = me.upgrade()
                    && feature
                        .snooze
                        .borrow()
                        .as_ref()
                        .is_some_and(|s| s.serial == serial)
                {
                    feature.stop_snooze(true);
                }
            });
        }
        let language = self.language();
        self.say(
            SLOT_SNOOZE,
            Activity {
                left: Ear::Symbol {
                    name: "moon.zzz.fill",
                    tone: Tone::Purple,
                    pulse: false,
                },
                right: Ear::Text {
                    text: short_length(minutes),
                    tone: Tone::Purple,
                },
                detail: Some(Detail {
                    title: language.util_snooze_on().to_owned(),
                    subtitle: language.util_snooze_for(minutes),
                }),
                page: None,
                priority: PRIORITY_SNOOZE,
            },
            Some(SHOW_SECONDS),
        );
    }

    /// `say` is false when the feature is being switched off, which has no need of a notice.
    pub fn stop_snooze(&self, say: bool) {
        if self.snooze.borrow_mut().take().is_none() {
            return;
        }
        if let Some(notch) = self.notch.as_ref().and_then(Weak::upgrade) {
            notch.set_snoozed(false);
        }
        if say {
            let language = self.language();
            self.say(
                SLOT_SNOOZE,
                Activity {
                    left: Ear::Symbol {
                        name: "moon.zzz",
                        tone: Tone::Gray,
                        pulse: false,
                    },
                    right: Ear::Empty,
                    detail: Some(Detail {
                        title: language.util_snooze_off().to_owned(),
                        subtitle: String::new(),
                    }),
                    page: None,
                    priority: PRIORITY_SNOOZE,
                },
                Some(SHOW_SECONDS),
            );
        } else {
            self.silence(SLOT_SNOOZE);
        }
    }

    // -- appearance and desktop -------------------------------------------------------------------------------

    /// A short notice with a symbol, a title and a line under it.
    fn tell(
        &self,
        slot: &'static str,
        priority: u8,
        (symbol, tone): (&'static str, Tone),
        title: &str,
        hint: &str,
    ) {
        self.say(
            slot,
            Activity {
                left: Ear::Symbol {
                    name: symbol,
                    tone,
                    pulse: false,
                },
                right: Ear::Empty,
                detail: Some(Detail {
                    title: title.to_owned(),
                    subtitle: hint.to_owned(),
                }),
                page: None,
                priority,
            },
            Some(SHOW_SECONDS),
        );
    }

    /// Switches the Mac between its dark and its light appearance.
    pub fn toggle_dark(&self) {
        let want = !desktop::dark();
        thread::spawn(move || {
            let done = desktop::set_dark(want);
            app::on_main(move || with_feature(|f| f.looked(done, want, true)));
        });
    }

    /// Hides the icons on the desktop, or shows them again. The Finder starts again to read it.
    pub fn toggle_desktop(&self) {
        let want = !desktop::icons_shown();
        thread::spawn(move || {
            let done = desktop::show_icons(want);
            app::on_main(move || with_feature(|f| f.looked(done, want, false)));
        });
    }

    /// Says on the notch what the appearance or the desktop now is, or that it could not be changed.
    fn looked(&self, done: bool, on: bool, appearance: bool) {
        let language = self.language();
        let (symbol, title) = match (done, appearance, on) {
            (false, true, _) => {
                return self.tell(
                    SLOT_LOOK,
                    PRIORITY_LOOK,
                    ("circle.lefthalf.filled", Tone::Gray),
                    language.util_cannot(),
                    language.util_automation_hint(),
                );
            }
            (false, false, _) => {
                return self.tell(
                    SLOT_LOOK,
                    PRIORITY_LOOK,
                    ("desktopcomputer", Tone::Gray),
                    language.util_cannot(),
                    "",
                );
            }
            (true, true, true) => ("moon.fill", language.util_dark_on()),
            (true, true, false) => ("sun.max.fill", language.util_dark_off()),
            (true, false, true) => ("desktopcomputer", language.util_desktop_shown()),
            (true, false, false) => ("eye.slash", language.util_desktop_hidden()),
        };
        self.tell(SLOT_LOOK, PRIORITY_LOOK, (symbol, Tone::Blue), title, "");
    }

    // -- the link of a tab ------------------------------------------------------------------------------------

    /// Copies the link of the tab in front of the browser in front, written as the settings say.
    pub fn copy_link(&self) {
        let language = self.language();
        let Some(front) = workspace::frontmost_app().filter(|a| browser::is_browser(&a.bundle_id))
        else {
            return self.tell(
                SLOT_LINK,
                PRIORITY_LINK,
                ("link", Tone::Gray),
                language.util_link_not_browser(),
                language.util_link_browsers(),
            );
        };
        let format = self.config.borrow().link_format.clone();
        thread::spawn(move || {
            let tab = browser::front_tab(&front.bundle_id, LINK_TIMEOUT);
            app::on_main(move || {
                with_feature(|f| {
                    let language = f.language();
                    let Some(tab) = tab else {
                        return f.tell(
                            SLOT_LINK,
                            PRIORITY_LINK,
                            ("link", Tone::Orange),
                            language.util_link_failed(),
                            language.util_automation_hint(),
                        );
                    };
                    let text = logic::link_text(&format, &tab.title, &tab.url);
                    Pasteboard::general().write(Write::Text(&text));
                    let shown = if tab.title.is_empty() {
                        &tab.url
                    } else {
                        &tab.title
                    };
                    f.tell(
                        SLOT_LINK,
                        PRIORITY_LINK,
                        ("link", Tone::Green),
                        language.util_link_copied(),
                        shown,
                    );
                });
            });
        });
    }

    // -- the settings page and the menu -----------------------------------------------------------------------

    pub fn runtime(&self) -> Value {
        json!({
            "awake": self.is_awake(),
            "awake_left": self.awake_left(),
            "locked": self.is_locked(),
            "snoozed": self.is_snoozed(),
            "snooze_left": self.snooze_left(),
            "mic_muted": volume::microphone_muted(),
            "accessibility": permissions::accessibility_trusted(false),
            "dark": desktop::dark(),
            "desktop_icons": desktop::icons_shown(),
        })
    }

    /// The buttons of the settings page.
    pub fn action(&self, name: &str, _config: &mut Config) -> Option<Result<(), Fault>> {
        match name {
            "utilities_awake" => self.toggle_awake(),
            "utilities_mic" => self.toggle_mic(),
            "utilities_lock" => self.toggle_lock(),
            "utilities_snooze" => self.toggle_snooze(),
            "utilities_dark" => self.toggle_dark(),
            "utilities_desktop" => self.toggle_desktop(),
            "utilities_automation" => permissions::open_pane(permissions::AUTOMATION_PANE),
            _ => return None,
        }
        Some(Ok(()))
    }

    /// The three tools of the menu bar panel: a submenu with the lengths of keeping awake, the microphone switch
    /// and the keyboard lock.
    pub fn menu_entries(this: &Rc<Self>) -> Vec<Entry> {
        let on = |me: &Weak<Feature>| me.upgrade().is_some_and(|f| f.config.borrow().enabled);
        let language = || i18n::current();
        let mut lengths: Vec<Entry> = logic::AWAKE_MINUTES
            .into_iter()
            .map(|minutes| {
                let (act, mark, shown) = (this.me.clone(), this.me.clone(), this.me.clone());
                Entry::Item(
                    Item::new(&language().util_awake_choice(minutes), move || {
                        if let Some(feature) = act.upgrade() {
                            feature.stop_awake(false);
                            feature.start_awake(minutes);
                        }
                    })
                    .checked(move || {
                        mark.upgrade().is_some_and(|f| {
                            f.awake
                                .borrow()
                                .as_ref()
                                .is_some_and(|s| s.minutes == minutes)
                        })
                    })
                    .visible_when(move || on(&shown))
                    .menu_only(),
                )
            })
            .collect();
        let (act, title, shown) = (this.me.clone(), this.me.clone(), this.me.clone());
        lengths.push(Entry::Separator);
        lengths.push(Entry::Item(
            Item::new(language().util_awake_stop_open(), move || {
                if let Some(feature) = act.upgrade() {
                    feature.stop_awake(true);
                }
            })
            .titled(move || {
                let language = i18n::current();
                match title.upgrade().and_then(|f| f.awake_left()) {
                    Some(left) => language.util_awake_stop(&logic::left(left)),
                    None => language.util_awake_stop_open().to_owned(),
                }
            })
            .visible_when(move || shown.upgrade().is_some_and(|f| f.is_awake()))
            .menu_only(),
        ));
        let mut snooze_lengths: Vec<Entry> = logic::SNOOZE_MINUTES
            .into_iter()
            .map(|minutes| {
                let (act, mark, shown) = (this.me.clone(), this.me.clone(), this.me.clone());
                Entry::Item(
                    Item::new(&language().util_snooze_choice(minutes), move || {
                        if let Some(feature) = act.upgrade() {
                            feature.stop_snooze(false);
                            feature.start_snooze(minutes);
                        }
                    })
                    .checked(move || {
                        mark.upgrade().is_some_and(|f| {
                            f.snooze
                                .borrow()
                                .as_ref()
                                .is_some_and(|s| s.minutes == minutes)
                        })
                    })
                    .visible_when(move || on(&shown))
                    .menu_only(),
                )
            })
            .collect();
        let (wake_act, wake_title, wake_shown) =
            (this.me.clone(), this.me.clone(), this.me.clone());
        snooze_lengths.push(Entry::Separator);
        snooze_lengths.push(Entry::Item(
            Item::new(language().util_snooze_wake_open(), move || {
                if let Some(feature) = wake_act.upgrade() {
                    feature.stop_snooze(true);
                }
            })
            .titled(move || {
                let language = i18n::current();
                match wake_title.upgrade().and_then(|f| f.snooze_left()) {
                    Some(left) => language.util_snooze_wake(&logic::left(left)),
                    None => language.util_snooze_wake_open().to_owned(),
                }
            })
            .visible_when(move || wake_shown.upgrade().is_some_and(|f| f.is_snoozed()))
            .menu_only(),
        ));
        let (mic_act, mic_mark, mic_shown) = (this.me.clone(), this.me.clone(), this.me.clone());
        let (lock_act, lock_title, lock_shown) =
            (this.me.clone(), this.me.clone(), this.me.clone());
        let (dark_act, dark_shown) = (this.me.clone(), this.me.clone());
        let (desk_act, desk_shown) = (this.me.clone(), this.me.clone());
        // The panel under the icon shows one switch for each of the two instead of their lists of lengths: it starts
        // the length chosen on the page, or stops what runs. The lists stay in the menu.
        let panel_toggle = |title: &str,
                            symbol: &'static str,
                            toggle: fn(&Feature),
                            running: fn(&Feature) -> bool| {
            let (act, mark, shown) = (this.me.clone(), this.me.clone(), this.me.clone());
            Entry::Item(
                Item::new(title, move || {
                    if let Some(feature) = act.upgrade() {
                        toggle(&feature);
                    }
                })
                .symbol(symbol)
                .checked(move || mark.upgrade().is_some_and(|f| running(&f)))
                .visible_when(move || on(&shown))
                .panel_only(),
            )
        };
        let awake_switch = panel_toggle(
            language().util_awake_menu(),
            symbols::AWAKE,
            Feature::toggle_awake,
            Feature::is_awake,
        );
        let snooze_switch = panel_toggle(
            language().util_snooze_menu(),
            symbols::SNOOZE,
            Feature::toggle_snooze,
            Feature::is_snoozed,
        );
        vec![
            awake_switch,
            snooze_switch,
            Entry::Submenu {
                title: language().util_awake_menu().to_owned(),
                symbol: Some(symbols::AWAKE),
                entries: lengths,
            },
            Entry::Submenu {
                title: language().util_snooze_menu().to_owned(),
                symbol: Some(symbols::SNOOZE),
                entries: snooze_lengths,
            },
            Entry::Item(
                Item::new(language().util_mic_menu(), move || {
                    if let Some(feature) = mic_act.upgrade() {
                        feature.toggle_mic();
                    }
                })
                .symbol(symbols::MIC_MUTE)
                .checked(move || {
                    mic_mark.upgrade().is_some() && volume::microphone_muted() == Some(true)
                })
                .visible_when(move || on(&mic_shown)),
            ),
            Entry::Item(
                Item::new(language().util_lock_menu(), move || {
                    if let Some(feature) = lock_act.upgrade() {
                        feature.toggle_lock();
                    }
                })
                .symbol(symbols::KEYBOARD)
                .titled(move || {
                    let language = i18n::current();
                    if lock_title.upgrade().is_some_and(|f| f.is_locked()) {
                        language.util_lock_unlock_menu().to_owned()
                    } else {
                        language.util_lock_menu().to_owned()
                    }
                })
                .visible_when(move || on(&lock_shown)),
            ),
            Entry::Item(
                Item::new(language().util_dark_menu(), move || {
                    if let Some(feature) = dark_act.upgrade() {
                        feature.toggle_dark();
                    }
                })
                .symbol(symbols::DARK)
                .checked(desktop::dark)
                .visible_when(move || on(&dark_shown)),
            ),
            Entry::Item(
                Item::new(language().util_desktop_menu(), move || {
                    if let Some(feature) = desk_act.upgrade() {
                        feature.toggle_desktop();
                    }
                })
                .symbol(symbols::DESKTOP)
                .checked(|| !desktop::icons_shown())
                .visible_when(move || on(&desk_shown)),
            ),
        ]
    }
}

/// The length of a keep-awake as the few characters that fit beside the notch.
fn short_length(minutes: u32) -> String {
    match minutes {
        0 => "On".into(),
        m if m % 60 == 0 => format!("{}h", m / 60),
        m => format!("{m}m"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_config_file_cannot_hold_an_unusable_length_or_shortcut() {
        let config = Config {
            awake_minutes: 37,
            snooze_minutes: 99,
            snooze_shortcut: "nonsense".into(),
            lock_seconds: 2,
            mic_shortcut: "nonsense".into(),
            lock_shortcut: "cmd+k".into(),
            ..Config::default()
        }
        .sanitized();
        assert_eq!(config.awake_minutes, 60);
        assert_eq!(config.snooze_minutes, 30);
        assert!(config.snooze_shortcut.is_empty());
        assert_eq!(config.lock_seconds, logic::LOCK_SECONDS.0);
        assert_eq!(config.mic_shortcut, DEFAULT_MIC_SHORTCUT);
        assert_eq!(config.lock_shortcut, "cmd+k");
        assert!(
            Config::default().awake_shortcut.is_empty(),
            "no shortcut is allowed"
        );
    }

    #[test]
    fn the_default_shortcuts_are_usable() {
        assert!(parse_hotkey(DEFAULT_MIC_SHORTCUT).is_ok());
        assert!(parse_hotkey(DEFAULT_LOCK_SHORTCUT).is_ok());
        assert!(parse_hotkey(DEFAULT_LINK_SHORTCUT).is_ok());
    }

    #[test]
    fn a_config_file_cannot_hold_a_link_format_that_is_not_offered() {
        let config = Config {
            link_format: "html".into(),
            dark_shortcut: "nonsense".into(),
            ..Config::default()
        }
        .sanitized();
        assert_eq!(config.link_format, "markdown");
        assert!(config.dark_shortcut.is_empty());
        let old: Config = serde_json::from_str(r#"{ "enabled": true }"#).unwrap();
        assert_eq!(
            old.link_shortcut, DEFAULT_LINK_SHORTCUT,
            "an older file gets the new tools"
        );
    }

    #[test]
    fn a_length_is_written_in_few_characters() {
        assert_eq!(short_length(0), "On");
        assert_eq!(short_length(15), "15m");
        assert_eq!(short_length(120), "2h");
    }
}
