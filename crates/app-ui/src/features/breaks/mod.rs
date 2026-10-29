//! Breaks for the eyes: after the time chosen at the Mac (20 minutes by default), a short break to look into the
//! distance (20 seconds by default), announced on the notch a few seconds before, shown on a screen that covers every
//! display with a card to skip it or have it five minutes later, or only on the notch. It is off until the person
//! turns it on.
//!
//! Only time at the Mac counts: keyboard and pointer still for five minutes, or for a whole break if that is longer,
//! start the clock again. A meeting (a camera or a microphone in use) or a full-screen app in front holds a break back
//! until it is over, each as the person chooses. Small reminders to blink and to sit up can come between breaks.
//! Nothing is kept on disk and no permission is needed: whether a camera or a microphone is in use, and which app is
//! full screen, are public, and the stillness is read from the clock of the input system.

use super::{AttachEnv, islands};
use crate::{
    config::parse_hotkey,
    i18n::{self, Language},
    menu::symbols,
    settings::model::Fault,
    trace,
};
use logic::{Clock, Event, Rules};
use macos_sys::{
    MainThreadMarker, app,
    break_ui::{BreakDisplay, BreakScreen},
    camera,
    hotkey::Hotkey,
    notch::{self, Activity, Detail, Ear, Notch},
    status_item::{Entry, Item},
    style::Tone,
    usage, volume, workspace,
};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::{
    cell::{Cell, RefCell},
    rc::{Rc, Weak},
    time::Instant,
};

pub mod logic;
pub mod settings;
mod text;

const SLOT: &str = "breaks";
const PRIORITY: u8 = 26;
const SHOW_SECONDS: f64 = 4.0;
/// How often the clock is looked at while waiting for a break, and while one runs.
const SLOW_SECONDS: f64 = 5.0;
const FAST_SECONDS: f64 = 1.0;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    pub enabled: bool,
    /// Minutes at the Mac between two breaks, one of `logic::WORK_MINUTES`.
    pub work_minutes: u32,
    /// Seconds a break lasts, one of `logic::BREAK_SECONDS`.
    pub break_seconds: u32,
    /// Cover every display during a break; off shows it on the notch only.
    pub cover: bool,
    pub sound: bool,
    /// Minutes between reminders to blink and to sit up, one of `logic::NUDGE_MINUTES`; 0 is never.
    pub blink_minutes: u32,
    pub posture_minutes: u32,
    /// Hold a break back while a camera or a microphone is in use.
    pub hold_meetings: bool,
    /// Hold a break back while the app in front is full screen.
    pub hold_full_screen: bool,
    /// Starts a break now, or ends the one that runs.
    pub shortcut: String,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            enabled: false,
            work_minutes: logic::WORK_MINUTES[0],
            break_seconds: logic::BREAK_SECONDS[0],
            cover: true,
            sound: true,
            blink_minutes: 0,
            posture_minutes: 0,
            hold_meetings: true,
            hold_full_screen: true,
            shortcut: String::new(),
        }
    }
}

impl Config {
    /// A config file can say anything; whatever is not offered is read as the default.
    pub fn sanitized(mut self) -> Self {
        self.work_minutes = logic::work_minutes(self.work_minutes);
        self.break_seconds = logic::break_seconds(self.break_seconds);
        self.blink_minutes = logic::nudge_minutes(self.blink_minutes);
        self.posture_minutes = logic::nudge_minutes(self.posture_minutes);
        if !self.shortcut.is_empty() && parse_hotkey(&self.shortcut).is_err() {
            self.shortcut.clear();
        }
        self
    }

    fn rules(&self) -> Rules {
        Rules::of(
            self.work_minutes,
            self.break_seconds,
            self.blink_minutes,
            self.posture_minutes,
        )
    }
}

pub struct Feature {
    mtm: MainThreadMarker,
    notch: Option<Weak<Notch>>,
    config: RefCell<Config>,
    clock: RefCell<Clock>,
    last: Cell<Option<Instant>>,
    ticker: RefCell<Option<app::Timer>>,
    /// Whether the ticker runs at the pace of a break.
    fast: Cell<bool>,
    /// Made the first time a break covers the screens.
    screen: RefCell<Option<Rc<BreakScreen>>>,
    hotkey: RefCell<Option<Hotkey>>,
    me: Weak<Feature>,
}

/// Starts the feature. It works without a notch, only without the words on it.
pub fn attach(env: &AttachEnv) -> Option<Rc<Feature>> {
    let config = env.config.breaks.clone().sanitized();
    let feature = Rc::new_cyclic(|me| Feature {
        mtm: env.mtm,
        notch: env.notch.map(Rc::downgrade),
        config: RefCell::new(config.clone()),
        clock: RefCell::default(),
        last: Cell::new(None),
        ticker: RefCell::default(),
        fast: Cell::new(false),
        screen: RefCell::default(),
        hotkey: RefCell::default(),
        me: me.clone(),
    });
    if config.enabled {
        feature.start();
        feature.register_hotkey(&config);
    }
    Some(feature)
}

impl Feature {
    pub fn configure(&self, config: &Config) -> Result<(), Fault> {
        let config = config.clone().sanitized();
        let was = self.config.borrow().enabled;
        *self.config.borrow_mut() = config.clone();
        self.hotkey.borrow_mut().take();
        match (was, config.enabled) {
            (false, true) => self.start(),
            (true, false) => self.stop(),
            _ => {}
        }
        if config.enabled {
            self.register_hotkey(&config);
        }
        Ok(())
    }

    fn register_hotkey(&self, config: &Config) {
        if config.shortcut.is_empty() {
            return;
        }
        let Ok(spec) = parse_hotkey(&config.shortcut) else {
            return;
        };
        let weak = self.me.clone();
        match Hotkey::register(self.mtm, spec.key_code, spec.modifiers, move || {
            if let Some(f) = weak.upgrade() {
                f.toggle();
            }
        }) {
            Ok(hotkey) => *self.hotkey.borrow_mut() = Some(hotkey),
            Err(status) => eprintln!("breaks: cannot register its shortcut (status {status})"),
        }
    }

    pub fn suspend_hotkeys(&self) {
        self.hotkey.borrow_mut().take();
    }

    pub fn resume_hotkeys(&self, config: &Config) {
        if config.enabled {
            self.register_hotkey(&config.clone().sanitized());
        }
    }

    fn start(&self) {
        *self.clock.borrow_mut() = Clock::default();
        self.last.set(Some(Instant::now()));
        self.pace(false);
        trace::log("breaks: counting");
    }

    fn stop(&self) {
        self.ticker.borrow_mut().take();
        self.last.set(None);
        *self.clock.borrow_mut() = Clock::default();
        self.cover(false);
        self.silence();
        trace::log("breaks: stopped");
    }

    /// Looks at the clock every second while a break runs, and every few seconds while waiting for one.
    fn pace(&self, fast: bool) {
        if self.ticker.borrow().is_some() && self.fast.get() == fast {
            return;
        }
        self.fast.set(fast);
        let weak = self.me.clone();
        *self.ticker.borrow_mut() = Some(app::every(
            if fast { FAST_SECONDS } else { SLOW_SECONDS },
            move || {
                if let Some(f) = weak.upgrade() {
                    f.tick();
                }
            },
        ));
    }

    fn tick(&self) {
        let now = Instant::now();
        let dt = self
            .last
            .replace(Some(now))
            .map_or(0.0, |before| now.duration_since(before).as_secs_f64());
        let config = self.config.borrow().clone();
        let event =
            self.clock
                .borrow_mut()
                .step(&config.rules(), dt, usage::idle_seconds(), || {
                    self.held(&config)
                });
        match event {
            Some(Event::Warn) => self.notice("eye", self.language().breaks_soon()),
            Some(Event::Start) => self.begin(),
            Some(Event::End) => {
                self.finish();
                self.notice("checkmark.circle", self.language().breaks_done());
            }
            Some(Event::Blink) => self.notice("eye", self.language().breaks_blink()),
            Some(Event::Posture) => {
                self.notice("figure.stand", self.language().breaks_posture());
            }
            None => {}
        }
        if self.clock.borrow().breaking() {
            self.paint();
        }
    }

    /// Whether a meeting or a full-screen app holds a break back, as the person chose.
    fn held(&self, config: &Config) -> bool {
        let meeting = config.hold_meetings && (camera::in_use() || volume::microphone_in_use());
        let held = meeting || (config.hold_full_screen && notch::front_is_full_screen(self.mtm));
        if held {
            trace::log("breaks: held back");
        }
        held
    }

    fn language(&self) -> Language {
        i18n::current()
    }

    /// A break starts: the sound, the screens or the notch, and the clock looked at every second.
    fn begin(&self) {
        trace::log("breaks: a break starts");
        if self.config.borrow().sound {
            workspace::play_sound("Glass");
        }
        self.cover(self.config.borrow().cover);
        self.paint();
        self.pace(true);
    }

    /// A break is over, however it ended.
    fn finish(&self) {
        trace::log("breaks: a break ends");
        self.cover(false);
        self.silence();
        if self.config.borrow().sound {
            workspace::play_sound("Ping");
        }
        self.pace(false);
    }

    /// Starts a break now, or ends the one that runs: the shortcut, the menu and the page.
    pub fn toggle(&self) {
        if !self.config.borrow().enabled {
            return;
        }
        if self.clock.borrow().breaking() {
            self.clock.borrow_mut().skip();
            self.finish();
        } else {
            let rules = self.config.borrow().rules();
            self.clock.borrow_mut().start(&rules);
            self.begin();
        }
    }

    fn skip(&self) {
        self.clock.borrow_mut().skip();
        self.finish();
    }

    fn later(&self) {
        let rules = self.config.borrow().rules();
        self.clock.borrow_mut().later(&rules);
        self.finish();
        self.notice("clock", self.language().breaks_put_off());
    }

    /// Puts the screens of a break up or takes them down.
    fn cover(&self, on: bool) {
        if !on {
            if let Some(screen) = &*self.screen.borrow() {
                screen.hide();
            }
            return;
        }
        if self.screen.borrow().is_none() {
            let (skip, later) = (self.me.clone(), self.me.clone());
            *self.screen.borrow_mut() = Some(BreakScreen::new(
                self.mtm,
                move || {
                    if let Some(f) = skip.upgrade() {
                        f.skip();
                    }
                },
                move || {
                    if let Some(f) = later.upgrade() {
                        f.later();
                    }
                },
            ));
        }
    }

    /// Draws the time left on the screens and on the notch.
    fn paint(&self) {
        let Some(left) = self.clock.borrow().left() else {
            return;
        };
        let language = self.language();
        let time = logic::clock(left);
        if let Some(screen) = &*self.screen.borrow()
            && self.config.borrow().cover
        {
            screen.show(&BreakDisplay {
                title: language.breaks_title().to_owned(),
                hint: language.breaks_hint().to_owned(),
                left: time.clone(),
                skip: language.breaks_skip().to_owned(),
                later: language.breaks_later().to_owned(),
            });
        }
        self.say(
            Activity {
                left: Ear::Symbol {
                    name: "eye",
                    tone: Tone::Green,
                    pulse: false,
                },
                right: Ear::Text {
                    text: time,
                    tone: Tone::Green,
                },
                detail: Some(Detail {
                    title: language.breaks_title().to_owned(),
                    subtitle: language.breaks_hint().to_owned(),
                }),
                page: None,
                priority: PRIORITY,
            },
            None,
        );
    }

    fn notice(&self, symbol: &'static str, title: &str) {
        self.say(
            Activity {
                left: Ear::Symbol {
                    name: symbol,
                    tone: Tone::Green,
                    pulse: false,
                },
                right: Ear::Empty,
                detail: Some(Detail {
                    title: title.to_owned(),
                    subtitle: String::new(),
                }),
                page: None,
                priority: PRIORITY,
            },
            Some(SHOW_SECONDS),
        );
    }

    fn say(&self, activity: Activity, seconds: Option<f64>) {
        if let Some(notch) = self.notch.as_ref().and_then(Weak::upgrade) {
            islands::show_for(&notch, SLOT, activity, seconds);
        }
    }

    fn silence(&self) {
        if let Some(notch) = self.notch.as_ref().and_then(Weak::upgrade) {
            islands::clear_slot(&notch, SLOT);
        }
    }

    /// Facts only the running app knows, for the settings page.
    pub fn runtime(&self) -> Value {
        let config = self.config.borrow();
        let clock = self.clock.borrow();
        json!({
            "until": config.enabled.then(|| clock.until_break(&config.rules()).ceil() as u64),
            "breaking": clock.breaking(),
        })
    }

    /// The button of the settings page. `None` when the name is not this feature's.
    pub fn action(&self, name: &str) -> Option<Result<(), Fault>> {
        if name != "breaks_now" {
            return None;
        }
        self.toggle();
        Some(Ok(()))
    }

    /// One item for the menu bar panel and the menu: a break now, or the end of the one that runs.
    pub fn menu_entries(this: &Rc<Self>) -> Vec<Entry> {
        let (act, title, shown) = (this.me.clone(), this.me.clone(), this.me.clone());
        vec![Entry::Item(
            Item::new(i18n::current().breaks_menu_now(), move || {
                if let Some(f) = act.upgrade() {
                    f.toggle();
                }
            })
            .symbol(symbols::BREAK)
            .titled(move || {
                let language = i18n::current();
                if title.upgrade().is_some_and(|f| f.clock.borrow().breaking()) {
                    language.breaks_menu_end().to_owned()
                } else {
                    language.breaks_menu_now().to_owned()
                }
            })
            .visible_when(move || shown.upgrade().is_some_and(|f| f.config.borrow().enabled)),
        )]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_feature_is_off_by_default_and_a_file_cannot_hold_what_is_not_offered() {
        let config: Config = serde_json::from_str("{}").unwrap();
        assert_eq!(config, Config::default());
        assert!(!config.enabled && config.cover && config.hold_meetings);
        let hostile = Config {
            work_minutes: 1,
            break_seconds: 9999,
            posture_minutes: 7,
            shortcut: "nonsense".into(),
            ..Config::default()
        }
        .sanitized();
        assert_eq!(
            (
                hostile.work_minutes,
                hostile.break_seconds,
                hostile.posture_minutes
            ),
            (20, 20, 0)
        );
        assert!(hostile.shortcut.is_empty());
    }

    #[test]
    fn every_glyph_is_one_the_system_knows() {
        for symbol in ["eye", "checkmark.circle", "figure.stand", "clock"] {
            assert!(
                macos_sys::widgets::symbol_png(symbol, 16.0, 1.0).is_some(),
                "{symbol} is not a symbol of this system"
            );
        }
    }

    #[test]
    fn every_word_is_there_in_both_languages() {
        for language in [Language::English, Language::Vietnamese] {
            for words in [
                language.breaks_title(),
                language.breaks_hint(),
                language.breaks_skip(),
                language.breaks_later(),
                language.breaks_soon(),
                language.breaks_done(),
                language.breaks_put_off(),
                language.breaks_blink(),
                language.breaks_posture(),
                language.breaks_menu_now(),
                language.breaks_menu_end(),
            ] {
                assert!(!words.is_empty());
            }
        }
        assert_ne!(
            Language::English.breaks_title(),
            Language::Vietnamese.breaks_title()
        );
    }
}
