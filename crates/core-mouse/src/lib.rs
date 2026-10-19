#![forbid(unsafe_code)]
//! Mos engine: tells mouse wheels from trackpads, then reverses or smooths the wheel events.

mod settings;
mod smooth;

pub use macos_sys::synth::SELF_TAG;
pub use settings::{AppRule, Settings};
pub use smooth::{Frame, GlidePoint, Phase, Smoother, glide_profile};

use macos_sys::{
    event_tap::{EventTap, EventView, SCROLL_WHEEL, TapError, Verdict, field, mask},
    permissions, qos,
    synth::{ScrollPhase, post_scroll},
};
use std::{
    sync::{
        Arc, Condvar, Mutex, MutexGuard, PoisonError, RwLock, RwLockReadGuard,
        atomic::{AtomicBool, Ordering},
    },
    thread::{self, JoinHandle},
    time::{Duration, Instant},
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScrollSource {
    /// Finger on a trackpad or Magic Mouse surface, or the inertia that follows.
    Trackpad,
    /// Notched wheel: line-based, no phases.
    Mouse,
    /// Pixel-based without phases: high-resolution wheel drivers or another app's synthetic scrolling.
    PixelWheel,
    SelfGenerated,
}

/// The event fields every decision depends on, copied out of the CGEvent.
#[derive(Debug, Clone, Copy, Default)]
pub struct RawScroll {
    pub continuous: bool,
    pub phase: i64,
    pub momentum_phase: i64,
    pub user_data: i64,
    pub flags: u64,
    /// Line deltas, `[horizontal, vertical]`.
    pub lines: [i64; 2],
    /// Pixel deltas, `[horizontal, vertical]`.
    pub points: [f64; 2],
}

impl RawScroll {
    fn read(event: &EventView<'_>) -> Self {
        Self {
            continuous: event.int(field::SCROLL_IS_CONTINUOUS) != 0,
            phase: event.int(field::SCROLL_PHASE),
            momentum_phase: event.int(field::SCROLL_MOMENTUM_PHASE),
            user_data: event.int(field::SOURCE_USER_DATA),
            flags: event.flags(),
            lines: [
                event.int(field::SCROLL_DELTA_AXIS_2),
                event.int(field::SCROLL_DELTA_AXIS_1),
            ],
            points: [
                event.float(field::SCROLL_POINT_DELTA_AXIS_2),
                event.float(field::SCROLL_POINT_DELTA_AXIS_1),
            ],
        }
    }
}

pub fn classify(raw: &RawScroll) -> ScrollSource {
    if raw.user_data == SELF_TAG {
        ScrollSource::SelfGenerated
    } else if raw.phase != 0 || raw.momentum_phase != 0 {
        ScrollSource::Trackpad
    } else if raw.continuous {
        ScrollSource::PixelWheel
    } else {
        ScrollSource::Mouse
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Action {
    Pass,
    /// Negate the deltas in place and let the event through.
    Reverse,
    /// Swallow the event and animate this many pixels instead, `[horizontal, vertical]`.
    Smooth {
        dx: f64,
        dy: f64,
    },
    /// Swallow the event and press the zoom shortcut instead.
    Zoom {
        zoom_in: bool,
    },
}

const SHIFT: u64 = 0x0002_0000;
const CONTROL: u64 = 0x0004_0000;
const OPTION: u64 = 0x0008_0000;
const COMMAND: u64 = 0x0010_0000;
/// Shift, Control, Option, Command. A modifier changes what a wheel means (sideways, zoom), and macOS may
/// already have rewritten the deltas, so events with a modifier held are only touched for the three
/// combinations handled below: Shift alone, Option alone and Command alone.
const MODIFIER_FLAGS: u64 = SHIFT | CONTROL | OPTION | COMMAND;

pub fn decide(settings: &Settings, frontmost: &str, raw: &RawScroll) -> Action {
    let source = classify(raw);
    if matches!(source, ScrollSource::Trackpad | ScrollSource::SelfGenerated) {
        return Action::Pass;
    }
    let (smooth, reverse) = settings.effective(frontmost);
    let held = raw.flags & MODIFIER_FLAGS;

    if held == COMMAND && settings.command_zoom && !settings.is_blocked(frontmost) {
        // The physical direction: wheel away from you zooms in, whatever the scroll direction setting.
        let vertical = if raw.lines[1] != 0 {
            raw.lines[1] as f64
        } else {
            raw.points[1]
        };
        if vertical != 0.0 {
            return Action::Zoom {
                zoom_in: vertical > 0.0,
            };
        }
    }
    let sideways = held == SHIFT && settings.shift_horizontal;
    let boosted = held == OPTION && settings.option_boost;
    if !smooth || (held != 0 && !sideways && !boosted) {
        return if reverse {
            Action::Reverse
        } else {
            Action::Pass
        };
    }
    let sign = if reverse { -1.0 } else { 1.0 };
    let factor = if boosted { settings.boost } else { 1.0 };
    let axis = |i: usize| {
        let base = match source {
            ScrollSource::Mouse if raw.lines[i] != 0 => raw.lines[i] as f64 * settings.step_px,
            _ => raw.points[i],
        };
        sign * base * settings.speed * factor
    };
    let (mut dx, mut dy) = (axis(0), axis(1));
    if sideways {
        // Some wheels already report the sideways delta on the horizontal axis; adding keeps both.
        dx += dy;
        dy = 0.0;
    }
    if dx == 0.0 && dy == 0.0 {
        Action::Pass
    } else {
        Action::Smooth { dx, dy }
    }
}

fn negate(event: &EventView<'_>) {
    for f in [field::SCROLL_DELTA_AXIS_1, field::SCROLL_DELTA_AXIS_2] {
        event.set_int(f, -event.int(f));
    }
    for f in [
        field::SCROLL_FIXED_DELTA_AXIS_1,
        field::SCROLL_FIXED_DELTA_AXIS_2,
        field::SCROLL_POINT_DELTA_AXIS_1,
        field::SCROLL_POINT_DELTA_AXIS_2,
    ] {
        event.set_float(f, -event.float(f));
    }
}

/// State shared between the UI (which edits it) and the engine threads (which read it).
pub struct Control {
    pub settings: RwLock<Settings>,
    /// Bundle identifier of the frontmost app; updated from workspace notifications, never queried in the tap.
    pub frontmost: RwLock<String>,
}

impl Control {
    pub fn new(settings: Settings) -> Arc<Self> {
        Arc::new(Self {
            settings: RwLock::new(settings.sanitized()),
            frontmost: RwLock::new(String::new()),
        })
    }

    pub fn snapshot(&self) -> Settings {
        read(&self.settings).clone()
    }

    pub fn edit(&self, change: impl FnOnce(&mut Settings)) {
        change(
            &mut self
                .settings
                .write()
                .unwrap_or_else(PoisonError::into_inner),
        );
    }

    pub fn set_frontmost(&self, bundle_id: String) {
        *self
            .frontmost
            .write()
            .unwrap_or_else(PoisonError::into_inner) = bundle_id;
    }
}

fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(PoisonError::into_inner)
}

fn read<T>(l: &RwLock<T>) -> RwLockReadGuard<'_, T> {
    l.read().unwrap_or_else(PoisonError::into_inner)
}

struct Shared {
    control: Arc<Control>,
    smoother: Mutex<Smoother>,
    wake: Condvar,
    stop: AtomicBool,
}

impl Shared {
    /// Runs inside the system input path for every scroll event on the machine: no I/O, no blocking
    /// beyond the short smoother lock.
    fn on_scroll(&self, event: &EventView<'_>) -> Verdict {
        let raw = RawScroll::read(event);
        let (action, duration) = {
            let settings = read(&self.control.settings);
            let app = read(&self.control.frontmost);
            (decide(&settings, &app, &raw), settings.duration_ms)
        };
        match action {
            Action::Pass => Verdict::Pass,
            Action::Reverse => {
                negate(event);
                Verdict::Pass
            }
            Action::Smooth { dx, dy } => {
                {
                    let mut smoother = lock(&self.smoother);
                    smoother.set_duration(duration);
                    smoother.push(dx, dy);
                }
                self.wake.notify_one();
                Verdict::Drop
            }
            Action::Zoom { zoom_in } => {
                macos_sys::synth::post_zoom(zoom_in);
                Verdict::Drop
            }
        }
    }

    fn shutdown(&self) {
        self.stop.store(true, Ordering::Release);
        // Taking the lock first guarantees the pump is either checking `stop` or already waiting.
        let _guard = lock(&self.smoother);
        self.wake.notify_all();
    }

    /// Posts frames at the display rate while a glide is running; blocks without waking when idle.
    fn run_pump(&self, post: impl Fn(Frame)) {
        qos::interactive();
        let mut last = Instant::now();
        let mut deadline = last;
        let mut resumed = true;
        loop {
            let period = 1.0 / read(&self.control.settings).fps;
            let frame = {
                let mut smoother = lock(&self.smoother);
                while smoother.is_idle() {
                    if self.stop.load(Ordering::Acquire) {
                        return;
                    }
                    smoother = self
                        .wake
                        .wait(smoother)
                        .unwrap_or_else(PoisonError::into_inner);
                    resumed = true;
                    deadline = Instant::now();
                }
                let now = Instant::now();
                let dt = if resumed {
                    period
                } else {
                    now.duration_since(last).as_secs_f64().min(0.05)
                };
                resumed = false;
                last = now;
                smoother.tick(dt)
            };
            if let Some(frame) = frame {
                post(frame);
            }
            // Absolute deadlines: sleep overshoot is repaid on the next frame instead of lowering the rate.
            deadline += Duration::from_secs_f64(period);
            let now = Instant::now();
            match deadline.checked_duration_since(now) {
                Some(wait) => thread::sleep(wait),
                None => deadline = now,
            }
        }
    }
}

#[derive(Debug)]
pub enum Error {
    /// Accessibility access is missing; the caller decides whether to prompt.
    NotTrusted,
    Tap(TapError),
}

/// Owns the event tap and the frame pump; dropping it removes both.
pub struct Engine {
    shared: Arc<Shared>,
    tap: Option<EventTap>,
    pump: Option<JoinHandle<()>>,
}

impl Engine {
    pub fn start(control: Arc<Control>) -> Result<Self, Error> {
        Self::start_with(control, |f| {
            // Posted as a wheel, not a gesture. Browsers latch a gesture to the scroller it began over until it
            // ends (Chromium's MouseWheelPhaseHandler), and a glide that keeps being fed never ends, so a list the
            // pointer moves onto never scrolls. A phaseless wheel is re-targeted once the pointer moves 10 px, as
            // with Mos, which posts its frames the same way. The end frame carries no distance: nothing to post.
            if f.phase != Phase::Ended {
                post_scroll(f.dx, f.dy, ScrollPhase::None);
            }
        })
    }

    /// Like `start`, with the output sink replaced (used to observe the engine without scrolling anything).
    pub fn start_with(
        control: Arc<Control>,
        post: impl Fn(Frame) + Send + 'static,
    ) -> Result<Self, Error> {
        if !permissions::accessibility_trusted(false) {
            return Err(Error::NotTrusted);
        }
        let duration = read(&control.settings).duration_ms;
        let shared = Arc::new(Shared {
            control,
            smoother: Mutex::new(Smoother::new(duration)),
            wake: Condvar::new(),
            stop: AtomicBool::new(false),
        });
        let pump = {
            let shared = Arc::clone(&shared);
            thread::Builder::new()
                .name("fig-scroll-pump".into())
                .spawn(move || shared.run_pump(post))
                .map_err(|e| Error::Tap(TapError::Thread(e)))?
        };
        let tap_shared = Arc::clone(&shared);
        match EventTap::spawn(mask(SCROLL_WHEEL), move |_, event| {
            tap_shared.on_scroll(event)
        }) {
            Ok(tap) => Ok(Self {
                shared,
                tap: Some(tap),
                pump: Some(pump),
            }),
            Err(e) => {
                shared.shutdown();
                let _ = pump.join();
                Err(Error::Tap(e))
            }
        }
    }
}

impl Drop for Engine {
    fn drop(&mut self) {
        // Remove the tap first so no event is swallowed after the pump stops.
        drop(self.tap.take());
        self.shared.shutdown();
        if let Some(pump) = self.pump.take() {
            let _ = pump.join();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn wheel(lines: i64) -> RawScroll {
        RawScroll {
            lines: [0, lines],
            ..RawScroll::default()
        }
    }

    #[test]
    fn classifies_each_source() {
        assert_eq!(classify(&wheel(1)), ScrollSource::Mouse);
        let pixel = RawScroll {
            continuous: true,
            ..RawScroll::default()
        };
        assert_eq!(classify(&pixel), ScrollSource::PixelWheel);
        assert_eq!(
            classify(&RawScroll { phase: 2, ..pixel }),
            ScrollSource::Trackpad
        );
        // Inertia tail: the finger is gone but the momentum phase is still set.
        assert_eq!(
            classify(&RawScroll {
                momentum_phase: 2,
                ..pixel
            }),
            ScrollSource::Trackpad
        );
        assert_eq!(
            classify(&RawScroll {
                user_data: SELF_TAG,
                phase: 2,
                ..pixel
            }),
            ScrollSource::SelfGenerated
        );
    }

    #[test]
    fn mouse_wheel_becomes_pixels_and_trackpad_is_untouched() {
        let s = Settings::default();
        assert_eq!(
            decide(&s, "app", &wheel(2)),
            Action::Smooth { dx: 0.0, dy: 72.0 }
        );
        assert_eq!(
            decide(&s, "app", &wheel(-1)),
            Action::Smooth { dx: 0.0, dy: -36.0 }
        );
        let trackpad = RawScroll {
            continuous: true,
            phase: 2,
            points: [0.0, 9.0],
            ..wheel(1)
        };
        assert_eq!(decide(&s, "app", &trackpad), Action::Pass);
        let own = RawScroll {
            user_data: SELF_TAG,
            continuous: true,
            points: [0.0, 9.0],
            ..wheel(0)
        };
        assert_eq!(decide(&s, "app", &own), Action::Pass);
    }

    #[test]
    fn pixel_wheels_keep_their_own_distance_and_speed_scales_it() {
        let s = Settings {
            speed: 2.0,
            ..Settings::default()
        };
        let pixel = RawScroll {
            continuous: true,
            points: [3.0, -10.0],
            ..RawScroll::default()
        };
        assert_eq!(
            decide(&s, "app", &pixel),
            Action::Smooth { dx: 6.0, dy: -20.0 }
        );
    }

    #[test]
    fn reverse_flips_smoothed_and_unsmoothed_wheels_but_not_trackpads() {
        let reversed = Settings {
            reverse: true,
            ..Settings::default()
        };
        assert_eq!(
            decide(&reversed, "app", &wheel(1)),
            Action::Smooth { dx: 0.0, dy: -36.0 }
        );
        let plain = Settings {
            smooth: false,
            reverse: true,
            ..Settings::default()
        };
        assert_eq!(decide(&plain, "app", &wheel(1)), Action::Reverse);
        let trackpad = RawScroll {
            phase: 2,
            ..wheel(1)
        };
        assert_eq!(decide(&plain, "app", &trackpad), Action::Pass);
        let off = Settings {
            smooth: false,
            ..Settings::default()
        };
        assert_eq!(decide(&off, "app", &wheel(1)), Action::Pass);
    }

    #[test]
    fn held_modifiers_and_per_app_rules_bypass_smoothing() {
        let s = Settings::default();
        let with_ctrl = RawScroll {
            flags: CONTROL,
            ..wheel(1)
        };
        assert_eq!(decide(&s, "app", &with_ctrl), Action::Pass);
        let caps_lock = RawScroll {
            flags: 0x0001_0000,
            ..wheel(1)
        };
        assert!(matches!(
            decide(&s, "app", &caps_lock),
            Action::Smooth { .. }
        ));
        let mut s = Settings::default();
        s.apps.insert(
            "com.game".into(),
            AppRule {
                smooth: Some(false),
                reverse: None,
            },
        );
        assert_eq!(decide(&s, "com.game", &wheel(1)), Action::Pass);
        assert!(matches!(
            decide(&s, "com.other", &wheel(1)),
            Action::Smooth { .. }
        ));
    }

    fn held(flags: u64, lines: i64) -> RawScroll {
        RawScroll {
            flags,
            ..wheel(lines)
        }
    }

    #[test]
    fn shift_turns_the_wheel_sideways_and_keeps_it_smooth() {
        let s = Settings::default();
        assert_eq!(
            decide(&s, "app", &held(SHIFT, 2)),
            Action::Smooth { dx: 72.0, dy: 0.0 }
        );
        assert_eq!(
            decide(&s, "app", &held(SHIFT, -1)),
            Action::Smooth { dx: -36.0, dy: 0.0 }
        );
        // A wheel that already reports the sideways delta keeps it.
        let sideways = RawScroll {
            flags: SHIFT,
            lines: [1, 0],
            ..RawScroll::default()
        };
        assert_eq!(
            decide(&s, "app", &sideways),
            Action::Smooth { dx: 36.0, dy: 0.0 }
        );
        let off = Settings {
            shift_horizontal: false,
            ..Settings::default()
        };
        assert_eq!(decide(&off, "app", &held(SHIFT, 2)), Action::Pass);
        let reversed = Settings {
            reverse: true,
            ..Settings::default()
        };
        assert_eq!(
            decide(&reversed, "app", &held(SHIFT, 1)),
            Action::Smooth { dx: -36.0, dy: 0.0 }
        );
    }

    #[test]
    fn option_boosts_the_distance_only_when_it_is_the_only_modifier() {
        let s = Settings::default();
        assert_eq!(
            decide(&s, "app", &held(OPTION, 1)),
            Action::Smooth { dx: 0.0, dy: 180.0 }
        );
        let off = Settings {
            option_boost: false,
            ..Settings::default()
        };
        assert_eq!(decide(&off, "app", &held(OPTION, 1)), Action::Pass);
        assert_eq!(decide(&s, "app", &held(OPTION | SHIFT, 1)), Action::Pass);
        assert_eq!(decide(&s, "app", &held(OPTION | COMMAND, 1)), Action::Pass);
        let strong = Settings {
            boost: 10.0,
            speed: 2.0,
            ..Settings::default()
        };
        assert_eq!(
            decide(&strong, "app", &held(OPTION, 1)),
            Action::Smooth { dx: 0.0, dy: 720.0 }
        );
    }

    #[test]
    fn command_zoom_is_opt_in_and_follows_the_physical_direction() {
        let off = Settings::default();
        assert_eq!(decide(&off, "app", &held(COMMAND, 1)), Action::Pass);
        let on = Settings {
            command_zoom: true,
            ..Settings::default()
        };
        assert_eq!(
            decide(&on, "app", &held(COMMAND, 1)),
            Action::Zoom { zoom_in: true }
        );
        assert_eq!(
            decide(&on, "app", &held(COMMAND, -3)),
            Action::Zoom { zoom_in: false }
        );
        // The scroll direction setting does not flip the zoom direction.
        let reversed = Settings {
            reverse: true,
            ..on.clone()
        };
        assert_eq!(
            decide(&reversed, "app", &held(COMMAND, 1)),
            Action::Zoom { zoom_in: true }
        );
        // Zoom works with smoothing switched off, but not in an app that is blocked.
        let no_smooth = Settings {
            smooth: false,
            ..on.clone()
        };
        assert_eq!(
            decide(&no_smooth, "app", &held(COMMAND, 1)),
            Action::Zoom { zoom_in: true }
        );
        let mut blocked = on.clone();
        blocked.apps.insert(
            "com.game".into(),
            AppRule {
                smooth: Some(false),
                reverse: None,
            },
        );
        assert_eq!(
            decide(&blocked, "com.game", &held(COMMAND, 1)),
            Action::Pass
        );
        // Command with another modifier is a different shortcut, and a pixel wheel zooms too.
        assert_eq!(decide(&on, "app", &held(COMMAND | SHIFT, 1)), Action::Pass);
        let pixel = RawScroll {
            flags: COMMAND,
            continuous: true,
            points: [0.0, -4.0],
            ..RawScroll::default()
        };
        assert_eq!(decide(&on, "app", &pixel), Action::Zoom { zoom_in: false });
    }

    #[test]
    fn empty_scroll_events_are_left_alone() {
        assert_eq!(
            decide(&Settings::default(), "app", &RawScroll::default()),
            Action::Pass
        );
    }

    #[test]
    fn per_event_decision_is_far_below_the_one_millisecond_budget() {
        let mut s = Settings::default();
        for i in 0..50 {
            s.apps
                .insert(format!("com.example.app{i}"), AppRule::default());
        }
        let raw = wheel(3);
        let start = Instant::now();
        let mut smoothed = 0;
        for _ in 0..200_000 {
            if matches!(decide(&s, "com.example.app25", &raw), Action::Smooth { .. }) {
                smoothed += 1;
            }
        }
        let per_event = start.elapsed() / 200_000;
        assert_eq!(smoothed, 200_000);
        assert!(
            per_event < Duration::from_micros(20),
            "{per_event:?} per decision"
        );
    }
}
