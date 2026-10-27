//! The Present tab of the notch: shows a display, an area of one, one window or the windows of one app to an audience,
//! full screen on another display or in a window of its own that a video call can share, the way a projector or a
//! shared screen is used in a talk. The presenter chooses on the tab what and where, and starts; the area, window or
//! display is picked on the frozen screen as for a screenshot. While it runs, a click zooms the audience's picture in
//! where it lands and the view follows the pointer until the quiet takes it back out, and keys zoom by steps, light a
//! spotlight around the pointer, ring the pointer, hold the picture still and put up a black or white screen, as the
//! slide shows of PowerPoint and Keynote and the live zoom of ZoomIt do. The time runs in the closed notch. It is off
//! until the person turns it on, and its keys other than the one that starts work only while a presentation runs.
//!
//! This folder belongs to one task. Shared code reaches it only through `features/mod.rs`.

pub mod settings;
mod text;

use super::AttachEnv;
use crate::{
    config::parse_hotkey,
    focus_timer::clock_text,
    i18n::{self, Language},
    menu::symbols,
    settings::model::Fault,
    trace,
};
use core_capture::{Point, camera::Camera};
use macos_sys::{
    MainThreadMarker, NSPoint, NSRect, app,
    capture_overlay::{self, Mode, Overlay, Picked},
    capture_ui, display_layout,
    hotkey::Hotkey,
    notch::{Activity, Ear, Notch},
    present_ui::{Choice, Live, PAGE_SIZE, PresentCallbacks, PresentDisplay, PresentView},
    presenter::{self, Cover, Ended, Look, Options, Presentation, Source, Stage},
    screen_capture::{self, Frozen},
    snap_ui::{Mouse, MouseMonitor},
    status_item::{Entry, Item},
    style::Tone,
    workspace,
};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::{
    cell::{Cell, RefCell},
    collections::BTreeMap,
    rc::{Rc, Weak},
    time::Instant,
};

const SLOT: &str = "present";
/// Above the music and a running Pomodoro: an audience is watching.
const PRIORITY: u8 = 24;
const TICK_SECONDS: f64 = 1.0 / 60.0;

/// The zooms a click may go to, the seconds of quiet before it goes back out (0 keeps it), the sizes of the
/// spotlight in percent of the stage's height, and the frames a second.
pub const ZOOMS: [f64; 5] = [1.5, 2.0, 2.5, 3.0, 4.0];
pub const ZOOM_OUTS: [u32; 5] = [0, 2, 3, 5, 8];
pub const SPOTLIGHTS: [u32; 3] = [12, 18, 26];
pub const FPS: [u32; 2] = [30, 60];

/// What is presented.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Display,
    Area,
    Window,
    App,
}

impl Kind {
    pub const ALL: [Kind; 4] = [Kind::Display, Kind::Area, Kind::Window, Kind::App];

    /// Name in the config file.
    pub const fn key(self) -> &'static str {
        match self {
            Kind::Display => "display",
            Kind::Area => "area",
            Kind::Window => "window",
            Kind::App => "app",
        }
    }

    fn from_key(key: &str) -> Kind {
        Kind::ALL
            .into_iter()
            .find(|k| k.key() == key)
            .unwrap_or(Kind::Display)
    }

    const fn symbol(self) -> &'static str {
        match self {
            Kind::Display => "display",
            Kind::Area => "rectangle.dashed",
            Kind::Window => "macwindow",
            Kind::App => "app",
        }
    }
}

/// What a key does.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Action {
    Start,
    Black,
    White,
    Freeze,
    ZoomIn,
    ZoomOut,
    ZoomReset,
    Auto,
    Spotlight,
    Ring,
}

impl Action {
    pub const ALL: [Action; 10] = [
        Action::Start,
        Action::Black,
        Action::White,
        Action::Freeze,
        Action::ZoomIn,
        Action::ZoomOut,
        Action::ZoomReset,
        Action::Auto,
        Action::Spotlight,
        Action::Ring,
    ];

    /// Name in the config file and in the shortcut targets ("present:black").
    pub const fn key(self) -> &'static str {
        match self {
            Action::Start => "start",
            Action::Black => "black",
            Action::White => "white",
            Action::Freeze => "freeze",
            Action::ZoomIn => "zoom_in",
            Action::ZoomOut => "zoom_out",
            Action::ZoomReset => "zoom_reset",
            Action::Auto => "auto",
            Action::Spotlight => "spotlight",
            Action::Ring => "ring",
        }
    }

    pub fn from_key(key: &str) -> Option<Action> {
        Action::ALL.into_iter().find(|a| a.key() == key)
    }

    /// Control and Option with a letter, as the slide shows' own letters (B black, W white, L laser) where they are
    /// free; none of them is a shortcut of macOS or of another part of the app.
    pub const fn default_shortcut(self) -> &'static str {
        match self {
            Action::Start => "ctrl+opt+p",
            Action::Black => "ctrl+opt+b",
            Action::White => "ctrl+opt+w",
            Action::Freeze => "ctrl+opt+h",
            Action::ZoomIn => "ctrl+opt+z",
            Action::ZoomOut => "ctrl+opt+x",
            Action::ZoomReset => "ctrl+opt+0",
            Action::Auto => "ctrl+opt+a",
            Action::Spotlight => "ctrl+opt+l",
            Action::Ring => "ctrl+opt+m",
        }
    }
}

/// What the audience sees, for the words at the top of the tab and the colour of the closed notch.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Shown {
    Live,
    Held,
    Black,
    White,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    pub enabled: bool,
    /// What the tab offers first: `display`, `area`, `window` or `app`.
    pub source: String,
    /// The pointer is in the audience's picture.
    pub cursor: bool,
    /// A green frame around the area or display shown, on the presenter's screen only.
    pub border: bool,
    pub fps: u32,
    /// A click zooms in where it lands, the view follows the pointer, and the quiet takes it back out.
    pub auto_zoom: bool,
    /// The zoom a click goes to.
    pub zoom: f64,
    /// Seconds without a click before the zoom goes back out; 0 keeps it.
    pub zoom_out: u32,
    /// A ring around the pointer, from the start.
    pub ring: bool,
    /// A ripple where the mouse is clicked.
    pub clicks: bool,
    /// The spotlight's circle, in percent of the stage's height.
    pub spotlight: u32,
    /// The time runs in the closed notch.
    pub closed_notch: bool,
    /// The key of each action (`start`, `black`, `white`, `freeze`, `zoom_in`, `zoom_out`, `zoom_reset`, `auto`,
    /// `spotlight`, `ring`); empty turns one off, a missing one has its default.
    pub shortcuts: BTreeMap<String, String>,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            enabled: false,
            source: Kind::Display.key().into(),
            cursor: true,
            border: true,
            fps: 60,
            auto_zoom: true,
            zoom: 2.0,
            zoom_out: 3,
            ring: false,
            clicks: true,
            spotlight: 18,
            closed_notch: true,
            shortcuts: BTreeMap::new(),
        }
    }
}

impl Config {
    /// A config file can say anything: what is not offered is the default, a key that cannot be read is dropped.
    pub fn sanitized(mut self) -> Self {
        let defaults = Config::default();
        if Kind::ALL.iter().all(|k| k.key() != self.source) {
            self.source = defaults.source;
        }
        if !FPS.contains(&self.fps) {
            self.fps = defaults.fps;
        }
        if !ZOOMS.iter().any(|z| (*z - self.zoom).abs() < 1e-9) {
            self.zoom = defaults.zoom;
        }
        if !ZOOM_OUTS.contains(&self.zoom_out) {
            self.zoom_out = defaults.zoom_out;
        }
        if !SPOTLIGHTS.contains(&self.spotlight) {
            self.spotlight = defaults.spotlight;
        }
        self.shortcuts.retain(|action, shortcut| {
            Action::from_key(action).is_some()
                && (shortcut.trim().is_empty() || parse_hotkey(shortcut).is_ok())
        });
        self
    }

    /// The key of an action: the person's, else the default. Empty when turned off.
    pub fn shortcut(&self, action: Action) -> String {
        self.shortcuts.get(action.key()).map_or_else(
            || action.default_shortcut().to_owned(),
            |s| s.trim().to_owned(),
        )
    }

    fn hold(&self) -> Option<f64> {
        (self.zoom_out > 0).then_some(f64::from(self.zoom_out))
    }
}

/// A shortcut as macOS writes it in a menu: "ctrl+opt+p" is "⌃⌥P".
pub fn keys_text(shortcut: &str) -> String {
    let parts: Vec<String> = shortcut
        .split('+')
        .map(|p| p.trim().to_lowercase())
        .filter(|p| !p.is_empty())
        .collect();
    let mut text = String::new();
    for (name, glyph) in [("ctrl", "⌃"), ("opt", "⌥"), ("shift", "⇧"), ("cmd", "⌘")] {
        if parts.iter().any(|p| p == name) {
            text.push_str(glyph);
        }
    }
    for part in &parts {
        let key = match part.as_str() {
            "ctrl" | "opt" | "shift" | "cmd" => continue,
            "left" => "←".to_owned(),
            "right" => "→".to_owned(),
            "up" => "↑".to_owned(),
            "down" => "↓".to_owned(),
            "return" => "↩".to_owned(),
            "delete" => "⌫".to_owned(),
            "esc" => "⎋".to_owned(),
            "tab" => "⇥".to_owned(),
            "space" => "Space".to_owned(),
            other => other.to_uppercase(),
        };
        text.push_str(&key);
    }
    text
}

/// Where the audience looks, as the tab offers it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Where {
    Display(u32),
    Window,
}

/// A presentation that runs, with what goes with it.
struct Session {
    presentation: Presentation,
    camera: Camera,
    started: Instant,
    last: Instant,
    ring: bool,
    spotlight: bool,
    frozen: bool,
    cover: Cover,
    /// What is shown where, for the tab.
    what: String,
    /// The time the tab and the closed notch show, drawn again only when it changes.
    clock: String,
    /// Which start this is, so that the end of an older one is not taken for it.
    generation: u64,
    keys: Vec<Hotkey>,
    _clicks: Option<MouseMonitor>,
    _ticker: app::Timer,
}

impl Session {
    fn shown(&self) -> Shown {
        match (self.cover, self.frozen) {
            (Cover::Black, _) => Shown::Black,
            (Cover::White, _) => Shown::White,
            (Cover::None, true) => Shown::Held,
            (Cover::None, false) => Shown::Live,
        }
    }

    fn clock(&self) -> String {
        clock_text(self.started.elapsed().as_secs())
    }
}

pub struct Feature {
    mtm: MainThreadMarker,
    notch: Weak<Notch>,
    /// The number of this tab in the notch.
    page: usize,
    view: Rc<PresentView>,
    config: RefCell<Config>,
    kind: Cell<Kind>,
    stage: Cell<Option<Where>>,
    /// The places the tab offers, in the order of its chips, with their names.
    stages: RefCell<Vec<(Where, String)>>,
    session: RefCell<Option<Session>>,
    starting: Cell<bool>,
    generation: Cell<u64>,
    overlay: RefCell<Option<Rc<Overlay>>>,
    hotkey: RefCell<Option<Hotkey>>,
    showing: Cell<bool>,
    me: Weak<Feature>,
}

/// Adds the tab. `None` without a notch, which is where the tab lives.
pub fn attach(env: &AttachEnv) -> Option<Rc<Feature>> {
    let notch = env.notch?;
    let config = env.config.present.clone().sanitized();
    let feature = Rc::new_cyclic(|me: &Weak<Feature>| {
        let with = |act: fn(&Feature, usize)| -> Box<dyn Fn(usize)> {
            let me = me.clone();
            Box::new(move |index| {
                if let Some(f) = me.upgrade() {
                    act(&f, index);
                }
            })
        };
        let (start, stop, zoom) = (me.clone(), me.clone(), me.clone());
        let view = PresentView::new(
            env.mtm,
            notch.page_area(PAGE_SIZE.width, PAGE_SIZE.height),
            PresentCallbacks {
                on_source: with(Feature::choose_source),
                on_stage: with(Feature::choose_stage),
                on_start: Box::new(move || {
                    if let Some(f) = start.upgrade() {
                        f.start();
                    }
                }),
                on_stop: Box::new(move || {
                    if let Some(f) = stop.upgrade() {
                        f.stop(None);
                    }
                }),
                on_toggle: with(Feature::switch),
                on_zoom: Box::new(move |inwards| {
                    if let Some(f) = zoom.upgrade() {
                        f.act(if inwards {
                            Action::ZoomIn
                        } else {
                            Action::ZoomOut
                        });
                    }
                }),
            },
        );
        Feature {
            mtm: env.mtm,
            notch: Rc::downgrade(notch),
            page: notch.page_count(),
            view,
            kind: Cell::new(Kind::from_key(&config.source)),
            config: RefCell::new(config),
            stage: Cell::new(None),
            stages: RefCell::default(),
            session: RefCell::default(),
            starting: Cell::new(false),
            generation: Cell::new(0),
            overlay: RefCell::default(),
            hotkey: RefCell::default(),
            showing: Cell::new(false),
            me: me.clone(),
        }
    });
    let (shown, hidden) = (Rc::downgrade(&feature), Rc::downgrade(&feature));
    notch.add_page(feature.view.page(
        i18n::current().present_tab().to_owned(),
        move || {
            if let Some(f) = shown.upgrade() {
                f.showing.set(true);
                f.read_stages();
                f.render();
            }
        },
        move || {
            if let Some(f) = hidden.upgrade() {
                f.showing.set(false);
            }
        },
    ));
    if feature.config.borrow().enabled {
        let config = feature.config.borrow().clone();
        if let Err(fault) = feature.register_start(&config) {
            eprintln!("present: cannot register its shortcut ({})", fault.detail);
        }
    }
    Some(feature)
}

impl Feature {
    /// Applies the settings: the key that starts, the camera of a presentation that runs, and everything let go when
    /// the feature is turned off.
    pub fn configure(&self, config: &Config) -> Result<(), Fault> {
        let config = config.clone().sanitized();
        let was = self.config.replace(config.clone());
        self.hotkey.borrow_mut().take();
        if !config.enabled {
            self.stop(None);
            self.overlay.borrow_mut().take();
            self.render();
            return Ok(());
        }
        if config.source != was.source {
            self.kind.set(Kind::from_key(&config.source));
        }
        if let Some(session) = self.session.borrow_mut().as_mut() {
            session.camera.set_level(config.zoom);
            session.camera.set_hold(config.hold());
            // Let go first: the system refuses a key this app already holds.
            session.keys.clear();
            session.keys = self.register_keys(&config);
        }
        self.render();
        self.register_start(&config)
    }

    fn register_start(&self, config: &Config) -> Result<(), Fault> {
        let shortcut = config.shortcut(Action::Start);
        if shortcut.is_empty() {
            return Ok(());
        }
        let spec = parse_hotkey(&shortcut).map_err(|e| Fault::new("invalid_shortcut", e))?;
        let weak = self.me.clone();
        let hotkey = Hotkey::register(self.mtm, spec.key_code, spec.modifiers, move || {
            if let Some(f) = weak.upgrade() {
                f.toggle();
            }
        })
        .map_err(|status| Fault::new("taken", format!("present (status {status})")))?;
        *self.hotkey.borrow_mut() = Some(hotkey);
        Ok(())
    }

    /// The keys that act on a presentation, while one runs. A key another app holds is left out.
    fn register_keys(&self, config: &Config) -> Vec<Hotkey> {
        Action::ALL
            .into_iter()
            .filter(|a| *a != Action::Start)
            .filter_map(|action| {
                let spec = parse_hotkey(&config.shortcut(action)).ok()?;
                let weak = self.me.clone();
                Hotkey::register(self.mtm, spec.key_code, spec.modifiers, move || {
                    if let Some(f) = weak.upgrade() {
                        f.act(action);
                    }
                })
                .inspect_err(|status| {
                    eprintln!("present: the key of {} is taken ({status})", action.key())
                })
                .ok()
            })
            .collect()
    }

    pub fn suspend_hotkeys(&self) {
        self.hotkey.borrow_mut().take();
        if let Some(session) = self.session.borrow_mut().as_mut() {
            session.keys.clear();
        }
    }

    pub fn resume_hotkeys(&self, config: &Config) {
        if !config.enabled {
            return;
        }
        if let Err(fault) = self.register_start(config) {
            eprintln!(
                "present: cannot register its shortcut again ({})",
                fault.detail
            );
        }
        if let Some(session) = self.session.borrow_mut().as_mut() {
            session.keys = self.register_keys(config);
        }
    }

    /// Facts only the running app knows, for the settings page.
    pub fn runtime(&self) -> Value {
        json!({
            "screen_access": screen_capture::screen_access(false),
            "presenting": self.session.borrow().is_some(),
        })
    }

    /// The buttons of the settings page. `None` when the name is not this feature's.
    pub fn action(&self, name: &str) -> Option<Result<(), Fault>> {
        match name {
            "present_screen_access" => {
                screen_capture::screen_access(true);
                Some(Ok(()))
            }
            _ => None,
        }
    }

    /// The entry of the menu bar panel: starts with what the tab chose, or stops.
    pub fn menu_entries(this: &Rc<Self>) -> Vec<Entry> {
        let (act, title, shown) = (this.me.clone(), this.me.clone(), this.me.clone());
        vec![Entry::Item(
            Item::new(i18n::current().present_menu_start(), move || {
                if let Some(f) = act.upgrade() {
                    f.toggle();
                }
            })
            .symbol(symbols::PRESENT)
            .titled(move || {
                let language = i18n::current();
                let clock = title
                    .upgrade()
                    .and_then(|f| f.session.borrow().as_ref().map(Session::clock));
                match clock {
                    Some(clock) => language.present_menu_stop(&clock),
                    None => language.present_menu_start().to_owned(),
                }
            })
            .visible_when(move || shown.upgrade().is_some_and(|f| f.config.borrow().enabled)),
        )]
    }

    // -- the tab --------------------------------------------------------------------------------------------------

    fn choose_source(&self, index: usize) {
        if let Some(kind) = Kind::ALL.get(index) {
            self.kind.set(*kind);
            self.render();
        }
    }

    fn choose_stage(&self, index: usize) {
        if let Some((place, _)) = self.stages.borrow().get(index) {
            self.stage.set(Some(*place));
        }
        self.render();
    }

    /// The displays to present on, and a window of its own; the chosen one stays while it is there, and at first it
    /// is a display that is not the main one, as a projector is, or the window when there is only one display.
    fn read_stages(&self) {
        let layout = display_layout::displays(self.mtm);
        let language = i18n::current();
        let mut stages: Vec<(Where, String)> = layout
            .iter()
            .enumerate()
            .filter(|(_, d)| d.mirror_of.is_none())
            .map(|(i, d)| {
                let name = if d.name.is_empty() {
                    language.displays_unnamed(i + 1)
                } else {
                    d.name.clone()
                };
                (Where::Display(d.id), name)
            })
            .collect();
        stages.push((Where::Window, language.present_own_window().to_owned()));
        let kept = self
            .stage
            .get()
            .filter(|s| stages.iter().any(|(w, _)| w == s));
        let first = layout
            .iter()
            .find(|d| !d.main && d.mirror_of.is_none())
            .map_or(Where::Window, |d| Where::Display(d.id));
        self.stage.set(Some(kept.unwrap_or(first)));
        *self.stages.borrow_mut() = stages;
    }

    fn name_of(&self, place: Where) -> String {
        self.stages
            .borrow()
            .iter()
            .find(|(w, _)| *w == place)
            .map(|(_, n)| n.clone())
            .unwrap_or_default()
    }

    fn render(&self) {
        if !self.showing.get() {
            return;
        }
        let language = i18n::current();
        let config = self.config.borrow();
        if !config.enabled {
            self.view.show(PresentDisplay {
                note: Some((
                    language.present_off_title().to_owned(),
                    language.present_off_hint().to_owned(),
                )),
                ..PresentDisplay::default()
            });
            return;
        }
        let session = self.session.borrow();
        let stages = self.stages.borrow();
        let display = match session.as_ref() {
            Some(session) => PresentDisplay {
                live: Some(live(language, &config, session)),
                ..PresentDisplay::default()
            },
            None => PresentDisplay {
                sources: Kind::ALL
                    .into_iter()
                    .map(|k| Choice {
                        title: language.present_kind(k).to_owned(),
                        symbol: k.symbol(),
                    })
                    .collect(),
                source: Kind::ALL
                    .iter()
                    .position(|k| *k == self.kind.get())
                    .unwrap_or(0),
                source_label: language.present_source_label().to_owned(),
                stages: stages
                    .iter()
                    .map(|(w, name)| Choice {
                        title: name.clone(),
                        symbol: match w {
                            Where::Display(_) => "display",
                            Where::Window => "macwindow.on.rectangle",
                        },
                    })
                    .collect(),
                stage: self
                    .stage
                    .get()
                    .and_then(|s| stages.iter().position(|(w, _)| *w == s)),
                stage_label: language.present_stage_label().to_owned(),
                start: language.present_start().to_owned(),
                hint: language
                    .present_hint(self.kind.get(), &keys_text(&config.shortcut(Action::Start))),
                ..PresentDisplay::default()
            },
        };
        drop((session, stages, config));
        self.view.show(display);
    }

    // -- starting -------------------------------------------------------------------------------------------------

    /// Starts with what the tab chose, or stops the one that runs.
    pub fn toggle(&self) {
        if self.session.borrow().is_some() {
            self.stop(None);
        } else {
            self.start();
        }
    }

    fn tell(&self, text: &str) {
        trace::log(&format!("present: {text}"));
        capture_ui::toast(self.mtm, text);
    }

    fn start(&self) {
        if !self.config.borrow().enabled || self.starting.get() || self.session.borrow().is_some() {
            return;
        }
        let language = i18n::current();
        if !screen_capture::screen_access(false) {
            screen_capture::screen_access(true);
            self.tell(language.present_needs_access());
            return;
        }
        self.read_stages();
        let stage = match self.stage.get() {
            Some(Where::Display(id)) => Stage::Display(id),
            _ => Stage::Window,
        };
        let kind = self.kind.get();
        if kind == Kind::Display {
            let others: Vec<u32> = self
                .stages
                .borrow()
                .iter()
                .filter_map(|(w, _)| match w {
                    Where::Display(id) if Stage::Display(*id) != stage => Some(*id),
                    _ => None,
                })
                .collect();
            match others.as_slice() {
                [] => return self.tell(language.present_nothing_else()),
                [only] => return self.begin(Source::Display(*only), Some(*only), stage),
                _ => {}
            }
        }
        let mode = match kind {
            Kind::Display => Mode::Display,
            Kind::Area => Mode::AreaOnly,
            Kind::Window | Kind::App => Mode::Window,
        };
        self.starting.set(true);
        if let Some(notch) = self.notch.upgrade() {
            notch.close();
        }
        let me = self.me.clone();
        screen_capture::freeze(move |result| {
            let Some(f) = me.upgrade() else {
                return;
            };
            match result {
                Ok(frozen) => f.pick(kind, mode, stage, Rc::new(frozen)),
                Err(e) => {
                    f.starting.set(false);
                    f.tell(&i18n::current().present_failed(&e));
                }
            }
        });
    }

    /// The overlay picks what to show on the frozen screen.
    fn pick(&self, kind: Kind, mode: Mode, stage: Stage, frozen: Rc<Frozen>) {
        let me = self.me.clone();
        let held = Rc::clone(&frozen);
        let overlay = capture_overlay::pick(self.mtm, &frozen, mode, "", move |picked| {
            let Some(f) = me.upgrade() else {
                return;
            };
            f.overlay.borrow_mut().take();
            f.starting.set(false);
            let display = |index: usize| held.displays.get(index).map(|d| d.id);
            let (source, on) = match picked {
                Picked::Cancelled => return,
                Picked::Display(index) => match display(index) {
                    Some(id) => (Source::Display(id), Some(id)),
                    None => return,
                },
                Picked::Area {
                    display: index,
                    rect,
                } => match display(index) {
                    Some(id) => (Source::Area { display: id, rect }, Some(id)),
                    None => return,
                },
                Picked::Window(id) => {
                    let on = held
                        .windows
                        .iter()
                        .find(|w| w.id == id)
                        .and_then(|w| display_under(&held, w.frame))
                        .and_then(display);
                    let source = if kind == Kind::App {
                        Source::App(id)
                    } else {
                        Source::Window(id)
                    };
                    (source, on)
                }
            };
            f.begin(source, on, stage);
        });
        *self.overlay.borrow_mut() = Some(overlay);
    }

    /// Starts the presentation, unless what is shown is on the display the audience looks at: the stage would cover
    /// it.
    fn begin(&self, source: Source, on: Option<u32>, stage: Stage) {
        let language = i18n::current();
        if let (Stage::Display(target), Some(on)) = (stage, on)
            && target == on
        {
            return self.tell(language.present_same_display());
        }
        let config = self.config.borrow().clone();
        let options = Options {
            fps: config.fps,
            cursor: config.cursor,
            border: config.border,
            title: language.present_window_title().to_owned(),
        };
        let generation = self.generation.get() + 1;
        self.generation.set(generation);
        self.starting.set(true);
        let (me, gone) = (self.me.clone(), self.me.clone());
        trace::log(&format!("present: starting {source:?} on {stage:?}"));
        Presentation::start(
            self.mtm,
            source,
            stage,
            options,
            move |result| {
                if let Some(f) = me.upgrade() {
                    f.started(result, source, generation);
                }
            },
            move |ended| {
                if let Some(f) = gone.upgrade() {
                    f.ended(ended, generation);
                }
            },
        );
    }

    fn started(&self, result: Result<Presentation, String>, source: Source, generation: u64) {
        self.starting.set(false);
        let presentation = match result {
            Ok(p) if self.generation.get() == generation && self.config.borrow().enabled => p,
            Ok(_) => return,
            Err(e) => return self.tell(&i18n::current().present_failed(&e)),
        };
        trace::log("present: running");
        let config = self.config.borrow().clone();
        let what = self.what(&presentation, source);
        let ticker = {
            let me = self.me.clone();
            app::every(TICK_SECONDS, move || {
                if let Some(f) = me.upgrade() {
                    f.tick();
                }
            })
        };
        let clicks = {
            let me = self.me.clone();
            MouseMonitor::start(self.mtm, move |mouse| {
                if let (Mouse::Down(_), Some(f)) = (mouse, me.upgrade()) {
                    f.clicked();
                }
            })
        };
        let now = Instant::now();
        *self.session.borrow_mut() = Some(Session {
            presentation,
            camera: Camera::new(config.zoom, config.hold(), config.auto_zoom),
            started: now,
            last: now,
            ring: config.ring,
            spotlight: false,
            frozen: false,
            cover: Cover::None,
            what,
            clock: String::new(),
            generation,
            keys: self.register_keys(&config),
            _clicks: clicks,
            _ticker: ticker,
        });
        self.paint();
    }

    /// What is shown where, in words.
    fn what(&self, presentation: &Presentation, source: Source) -> String {
        let language = i18n::current();
        self.read_stages();
        let shown = match source {
            Source::Display(id) => self.name_of(Where::Display(id)),
            Source::Area { display, .. } => {
                language.present_area_of(&self.name_of(Where::Display(display)))
            }
            Source::Window(_) | Source::App(_) => presentation.subject().to_owned(),
        };
        let place = match presentation.stage() {
            Stage::Display(id) => Where::Display(id),
            Stage::Window => Where::Window,
        };
        language.present_what(&shown, &self.name_of(place))
    }

    fn ended(&self, ended: Ended, generation: u64) {
        let ours = self
            .session
            .borrow()
            .as_ref()
            .is_some_and(|s| s.generation == generation);
        if ours {
            self.stop(ended.map(|why| i18n::current().present_ended(&why)));
        }
    }

    /// Ends the presentation, saying why when it was not the presenter's choice.
    pub fn stop(&self, why: Option<String>) {
        let session = self.session.borrow_mut().take();
        let Some(session) = session else {
            return;
        };
        session.presentation.stop();
        // Dropped outside the borrow: its timer, keys and the watch on clicks go with it.
        drop(session);
        trace::log("present: stopped");
        if let Some(why) = why {
            self.tell(&why);
        }
        self.paint();
    }

    // -- while it runs --------------------------------------------------------------------------------------------

    /// Where the pointer is in the picture, while it is in it.
    fn pointer(session: &Session) -> Option<Point> {
        let frame = session.presentation.picture_frame()?;
        unit_of(frame, screen_capture::pointer())
    }

    fn tick(&self) {
        let mut stop = false;
        let clock_changed = {
            let mut session = self.session.borrow_mut();
            let Some(s) = session.as_mut() else {
                return;
            };
            let now = Instant::now();
            let seconds = now.duration_since(s.last).as_secs_f64();
            s.last = now;
            let pointer = Self::pointer(s);
            if let Some(p) = pointer {
                s.camera.pointer(p);
            }
            s.camera.advance(seconds);
            if workspace::display_options().reduce_motion {
                s.camera.settle();
            }
            let view = s.camera.view();
            let spotlight = f64::from(self.config.borrow().spotlight) / 100.0;
            s.presentation.show(Look {
                view: (view.x, view.y, view.w, view.h),
                pointer: pointer.map(|p| (p.x, p.y)),
                ring: s.ring,
                spotlight: s.spotlight.then_some(spotlight),
                cover: s.cover,
            });
            let clock = s.clock();
            if clock != s.clock {
                s.clock = clock;
                // Once a second: the display the audience looks at may have been unplugged.
                if let Stage::Display(id) = s.presentation.stage() {
                    stop = presenter::screen_frame(self.mtm, id).is_none();
                }
                true
            } else {
                false
            }
        };
        if stop {
            // After the tick: its timer goes with the presentation.
            let me = self.me.clone();
            app::after(0.0, move || {
                if let Some(f) = me.upgrade() {
                    f.stop(Some(i18n::current().present_display_gone().to_owned()));
                }
            });
        } else if clock_changed {
            self.paint();
        }
    }

    /// A click anywhere: in the picture, it zooms in there and ripples.
    fn clicked(&self) {
        let ripple = self.config.borrow().clicks;
        let mut session = self.session.borrow_mut();
        let Some(s) = session.as_mut() else {
            return;
        };
        let Some(at) = Self::pointer(s) else {
            return;
        };
        s.camera.click(at);
        if ripple && s.cover == Cover::None && !s.frozen {
            s.presentation.ripple((at.x, at.y));
        }
    }

    /// A switch of the tab: the automatic zoom, the spotlight, the ring, the held picture, black, white.
    fn switch(&self, index: usize) {
        let action = [
            Action::Auto,
            Action::Spotlight,
            Action::Ring,
            Action::Freeze,
            Action::Black,
            Action::White,
        ];
        if let Some(action) = action.get(index) {
            self.act(*action);
        }
    }

    /// What a key or a button of a running presentation does.
    fn act(&self, action: Action) {
        {
            let mut session = self.session.borrow_mut();
            let Some(s) = session.as_mut() else {
                return;
            };
            let at = Self::pointer(s).unwrap_or(Point::new(0.5, 0.5));
            match action {
                Action::Start => {}
                Action::Black => s.cover = toggled(s.cover, Cover::Black),
                Action::White => s.cover = toggled(s.cover, Cover::White),
                Action::Freeze => {
                    s.frozen = !s.frozen;
                    s.presentation.freeze(s.frozen);
                }
                Action::ZoomIn => s.camera.step(true, at),
                Action::ZoomOut => s.camera.step(false, at),
                Action::ZoomReset => s.camera.reset(),
                Action::Auto => {
                    let auto = !s.camera.is_auto();
                    s.camera.set_auto(auto);
                }
                Action::Spotlight => s.spotlight = !s.spotlight,
                Action::Ring => s.ring = !s.ring,
            }
        }
        if action == Action::Start {
            return self.stop(None);
        }
        self.paint();
    }

    /// The tab and the closed notch.
    fn paint(&self) {
        self.render();
        self.update_notch();
    }

    fn update_notch(&self) {
        let Some(notch) = self.notch.upgrade() else {
            return;
        };
        let wanted = self.config.borrow().closed_notch;
        let mut session = self.session.borrow_mut();
        let Some(s) = session.as_mut().filter(|_| wanted) else {
            notch.set_activity(SLOT, None);
            return;
        };
        let clock = s.clock();
        let tone = if s.shown() == Shown::Live {
            Tone::Green
        } else {
            Tone::Orange
        };
        notch.set_activity(
            SLOT,
            Some(Activity {
                left: Ear::Symbol {
                    name: "rectangle.on.rectangle",
                    tone,
                    pulse: false,
                },
                right: Ear::Text { text: clock, tone },
                detail: None,
                page: Some(self.page),
                priority: PRIORITY,
            }),
        );
    }
}

fn toggled(now: Cover, wanted: Cover) -> Cover {
    if now == wanted { Cover::None } else { wanted }
}

/// A point of the screens as a fraction of a frame from its bottom-left, while it is inside.
fn unit_of(frame: NSRect, p: NSPoint) -> Option<Point> {
    if frame.size.width <= 0.0 || frame.size.height <= 0.0 {
        return None;
    }
    let x = (p.x - frame.origin.x) / frame.size.width;
    let y = (p.y - frame.origin.y) / frame.size.height;
    ((0.0..=1.0).contains(&x) && (0.0..=1.0).contains(&y)).then_some(Point::new(x, y))
}

/// The display a window is mostly on, by its place in the frozen displays.
fn display_under(frozen: &Frozen, frame: NSRect) -> Option<usize> {
    let middle = NSPoint::new(
        frame.origin.x + frame.size.width / 2.0,
        frame.origin.y + frame.size.height / 2.0,
    );
    frozen
        .displays
        .iter()
        .position(|d| unit_of(d.frame, middle).is_some())
}

/// The running presentation on the tab: what the audience sees, the time, the switches and the keys.
fn live(language: Language, config: &Config, session: &Session) -> Live {
    let shown = session.shown();
    let toggle = |action: Action, symbol: &'static str, on: bool| {
        (
            Choice {
                title: language.present_action(action).to_owned(),
                symbol,
            },
            on,
        )
    };
    let keys: Vec<String> = [
        Action::ZoomIn,
        Action::ZoomOut,
        Action::ZoomReset,
        Action::Auto,
        Action::Spotlight,
        Action::Ring,
        Action::Freeze,
        Action::Black,
        Action::White,
        Action::Start,
    ]
    .into_iter()
    .filter_map(|action| {
        let shortcut = config.shortcut(action);
        (!shortcut.is_empty()).then(|| {
            format!(
                "{} {}",
                keys_text(&shortcut),
                language.present_action(action).to_lowercase()
            )
        })
    })
    .collect();
    let [zoom_out, zoom_in] = language.present_zoom_names();
    Live {
        title: language.present_live_title(shown).to_owned(),
        tone: Some(if shown == Shown::Live {
            Tone::Green
        } else {
            Tone::Orange
        }),
        clock: session.clock(),
        what: session.what.clone(),
        stop: language.present_stop().to_owned(),
        toggles: vec![
            toggle(
                Action::Auto,
                "plus.magnifyingglass",
                session.camera.is_auto(),
            ),
            toggle(Action::Spotlight, "flashlight.on.fill", session.spotlight),
            toggle(Action::Ring, "cursorarrow.rays", session.ring),
            toggle(Action::Freeze, "pause.fill", session.frozen),
            toggle(
                Action::Black,
                "rectangle.fill",
                session.cover == Cover::Black,
            ),
            toggle(Action::White, "rectangle", session.cover == Cover::White),
        ],
        first_row: 3,
        zoom: zoom_text(session.camera.zoom()),
        zoom_names: [zoom_out.to_owned(), zoom_in.to_owned()],
        keys: keys.join(", "),
    }
}

/// "2x", "1.5x": the zoom in a few characters.
fn zoom_text(zoom: f64) -> String {
    let rounded = (zoom * 10.0).round() / 10.0;
    if (rounded - rounded.round()).abs() < 1e-9 {
        format!("{rounded:.0}x")
    } else {
        format!("{rounded:.1}x")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_feature_is_off_by_default_and_a_hostile_file_is_tamed() {
        let config: Config = serde_json::from_str("{}").unwrap();
        assert_eq!(config, Config::default());
        assert!(!config.enabled && config.auto_zoom && config.cursor);
        let hostile: Config = serde_json::from_str(
            r#"{ "source": "everything", "fps": 1000, "zoom": 9.5, "zoom_out": 77, "spotlight": 3,
                 "shortcuts": { "black": "nonsense", "white": "", "launch": "ctrl+opt+q" } }"#,
        )
        .unwrap();
        let tamed = hostile.sanitized();
        assert_eq!(tamed.source, "display");
        assert_eq!(
            (tamed.fps, tamed.zoom, tamed.zoom_out, tamed.spotlight),
            (60, 2.0, 3, 18)
        );
        assert_eq!(
            tamed.shortcut(Action::Black),
            "ctrl+opt+b",
            "a key that cannot be read has its default"
        );
        assert_eq!(
            tamed.shortcut(Action::White),
            "",
            "an empty key turns it off"
        );
        assert!(!tamed.shortcuts.contains_key("launch"));
    }

    #[test]
    fn every_action_has_a_different_default_key_that_parses() {
        let mut seen = Vec::new();
        for action in Action::ALL {
            let spec = parse_hotkey(action.default_shortcut()).unwrap();
            assert!(!seen.contains(&spec), "{} repeats a key", action.key());
            seen.push(spec);
            assert_eq!(Action::from_key(action.key()), Some(action));
        }
        assert_eq!(Action::from_key("nonsense"), None);
    }

    #[test]
    fn keys_are_written_as_menus_write_them() {
        assert_eq!(keys_text("ctrl+opt+p"), "⌃⌥P");
        assert_eq!(keys_text("cmd+shift+opt+ctrl+left"), "⌃⌥⇧⌘←");
        assert_eq!(keys_text("opt+0"), "⌥0");
        assert_eq!(keys_text(""), "");
    }

    #[test]
    fn the_pointer_is_placed_in_the_picture_only_while_inside() {
        let frame = NSRect::new(
            NSPoint::new(100.0, 50.0),
            macos_sys::NSSize::new(200.0, 100.0),
        );
        assert_eq!(
            unit_of(frame, NSPoint::new(200.0, 100.0)),
            Some(Point::new(0.5, 0.5))
        );
        assert_eq!(
            unit_of(frame, NSPoint::new(100.0, 150.0)),
            Some(Point::new(0.0, 1.0))
        );
        assert_eq!(unit_of(frame, NSPoint::new(99.0, 100.0)), None);
        let empty = NSRect::new(NSPoint::new(0.0, 0.0), macos_sys::NSSize::new(0.0, 10.0));
        assert_eq!(unit_of(empty, NSPoint::new(0.0, 0.0)), None);
    }

    #[test]
    fn black_and_white_take_turns_and_a_second_press_takes_them_away() {
        assert_eq!(toggled(Cover::None, Cover::Black), Cover::Black);
        assert_eq!(toggled(Cover::Black, Cover::White), Cover::White);
        assert_eq!(toggled(Cover::White, Cover::White), Cover::None);
    }

    #[test]
    fn the_zoom_is_written_short() {
        assert_eq!(zoom_text(2.0), "2x");
        assert_eq!(zoom_text(1.4999), "1.5x");
        assert_eq!(zoom_text(1.0), "1x");
    }

    #[test]
    fn the_hint_names_the_key_in_both_languages() {
        for language in [Language::English, Language::Vietnamese] {
            for kind in Kind::ALL {
                let hint = language.present_hint(kind, "⌃⌥P");
                assert!(hint.contains("⌃⌥P"), "{hint}");
                assert!(!language.present_hint(kind, "").contains("⌃"));
            }
        }
        assert_ne!(
            Language::English.present_hint(Kind::Area, ""),
            Language::Vietnamese.present_hint(Kind::Area, "")
        );
    }
}
