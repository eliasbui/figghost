//! The focus page: length chips, a large countdown, a progress bar and round start / pause, minus and plus
//! buttons.

use crate::{
    button::{Look, TouchButton},
    labels,
    notch::Page,
    panel::Key,
    style::{self, Tone, Weight, white},
    widgets::{PageRoot, ProgressBar},
};
use objc2::{MainThreadMarker, rc::Retained};
use objc2_app_kit::{
    NSAutoresizingMaskOptions, NSColor, NSLineBreakMode, NSTextAlignment, NSTextField,
};
use objc2_foundation::{NSPoint, NSRect, NSSize, NSString};
use std::rc::Rc;

const MARGIN: f64 = 24.0;
const CHIP_HEIGHT: f64 = 28.0;
const CHIP_GAP: f64 = 8.0;
const SIDE_BUTTON: f64 = 44.0;
const MAIN_BUTTON: f64 = 62.0;
const BUTTON_GAP: f64 = 26.0;
/// How far the minus and plus buttons move the length.
pub const ADJUST_MINUTES: i32 = 5;

/// What the buttons do; the app owns the timer.
pub struct FocusCallbacks {
    pub on_toggle: Box<dyn Fn()>,
    pub on_reset: Box<dyn Fn()>,
    pub on_preset: Box<dyn Fn(u32)>,
    /// Minus and plus, in minutes.
    pub on_adjust: Box<dyn Fn(i32)>,
}

/// What the page shows.
pub struct FocusDisplay<'a> {
    pub clock: &'a str,
    /// Share of the time that has passed, 0 to 1.
    pub progress: f64,
    pub running: bool,
    pub paused: bool,
    pub finished: bool,
    /// The chosen length, which highlights its preset.
    pub minutes: u32,
}

pub struct FocusView {
    pub root: Retained<PageRoot>,
    clock: Retained<NSTextField>,
    status: Retained<NSTextField>,
    bar: Retained<ProgressBar>,
    toggle: Retained<TouchButton>,
    minus: Retained<TouchButton>,
    plus: Retained<TouchButton>,
    reset: Retained<TouchButton>,
    presets: Vec<(u32, Retained<TouchButton>)>,
    callbacks: Rc<FocusCallbacks>,
}

fn label(
    mtm: MainThreadMarker,
    font: Retained<objc2_app_kit::NSFont>,
    color: &NSColor,
    frame: NSRect,
) -> Retained<NSTextField> {
    let label = NSTextField::labelWithString(&NSString::from_str(""), mtm);
    label.setFont(Some(&font));
    label.setTextColor(Some(color));
    label.setAlignment(NSTextAlignment::Center);
    label.setLineBreakMode(NSLineBreakMode::ByClipping);
    label.setFrame(frame);
    label.setAutoresizingMask(NSAutoresizingMaskOptions::ViewWidthSizable);
    label
}

fn round(
    mtm: MainThreadMarker,
    centre: NSPoint,
    side: f64,
    filled: bool,
    symbol: &str,
) -> Retained<TouchButton> {
    let button = TouchButton::new(
        mtm,
        NSRect::new(
            NSPoint::new(centre.x - side / 2.0, centre.y - side / 2.0),
            NSSize::new(side, side),
        ),
        Look::Round { filled },
    );
    button.set_symbol(symbol);
    button
}

impl FocusView {
    pub fn new(
        mtm: MainThreadMarker,
        frame: NSRect,
        presets: &[u32],
        callbacks: FocusCallbacks,
    ) -> Rc<Self> {
        let texts = labels::get();
        let root = PageRoot::new(mtm, frame, &texts.focus_hint);
        let width = frame.size.width;
        let top = frame.size.height;
        let callbacks = Rc::new(callbacks);

        // Length chips, centred.
        let mut chips = Vec::new();
        for minutes in presets.iter().copied() {
            let chip = TouchButton::new(mtm, NSRect::ZERO, Look::Chip);
            chip.set_title(&format!("{minutes} {}", texts.focus_minutes));
            let shared = Rc::clone(&callbacks);
            chip.set_on_click(move || (shared.on_preset)(minutes));
            chips.push((minutes, chip));
        }
        let widths: Vec<f64> = chips.iter().map(|(_, c)| c.fit_width()).collect();
        let total = widths.iter().sum::<f64>() + CHIP_GAP * (chips.len() as f64 - 1.0).max(0.0);
        let mut x = (width - total) / 2.0;
        for ((_, chip), w) in chips.iter().zip(&widths) {
            chip.setFrame(NSRect::new(
                NSPoint::new(x, top - 26.0 - CHIP_HEIGHT),
                NSSize::new(*w, CHIP_HEIGHT),
            ));
            root.addSubview(chip);
            x += w + CHIP_GAP;
        }

        let big = style::digits(78.0, Weight::Light);
        let row = |y: f64, height: f64| {
            NSRect::new(
                NSPoint::new(MARGIN, y),
                NSSize::new(width - 2.0 * MARGIN, height),
            )
        };
        let clock = label(mtm, big, &white(style::TEXT), row(top - 150.0, 92.0));
        let status = label(
            mtm,
            style::font(13.0, Weight::Medium),
            &white(style::TEXT_SECONDARY),
            row(top - 178.0, 20.0),
        );
        root.addSubview(&clock);
        root.addSubview(&status);

        let bar = ProgressBar::new(
            mtm,
            NSRect::new(
                NSPoint::new(MARGIN + 56.0, top - 206.0),
                NSSize::new(width - 2.0 * (MARGIN + 56.0), 6.0),
            ),
        );
        root.addSubview(&bar);

        let middle = NSPoint::new(width / 2.0, top - 270.0);
        let toggle = round(mtm, middle, MAIN_BUTTON, true, "play.fill");
        let side = MAIN_BUTTON / 2.0 + BUTTON_GAP + SIDE_BUTTON / 2.0;
        let minus = round(
            mtm,
            NSPoint::new(middle.x - side, middle.y),
            SIDE_BUTTON,
            false,
            "minus",
        );
        let plus = round(
            mtm,
            NSPoint::new(middle.x + side, middle.y),
            SIDE_BUTTON,
            false,
            "plus",
        );
        minus.set_title(&format!("-{ADJUST_MINUTES} {}", texts.focus_minutes));
        plus.set_title(&format!("+{ADJUST_MINUTES} {}", texts.focus_minutes));
        let shared = Rc::clone(&callbacks);
        toggle.set_on_click(move || (shared.on_toggle)());
        let shared = Rc::clone(&callbacks);
        minus.set_on_click(move || (shared.on_adjust)(-ADJUST_MINUTES));
        let shared = Rc::clone(&callbacks);
        plus.set_on_click(move || (shared.on_adjust)(ADJUST_MINUTES));
        for button in [&toggle, &minus, &plus] {
            root.addSubview(button);
        }

        let reset = TouchButton::new(mtm, NSRect::ZERO, Look::Chip);
        reset.set_symbol("arrow.counterclockwise");
        reset.set_title(&texts.focus_reset);
        let reset_width = reset.fit_width();
        reset.setFrame(NSRect::new(
            NSPoint::new((width - reset_width) / 2.0, top - 350.0),
            NSSize::new(reset_width, CHIP_HEIGHT),
        ));
        let shared = Rc::clone(&callbacks);
        reset.set_on_click(move || (shared.on_reset)());
        root.addSubview(&reset);

        Rc::new(Self {
            root,
            clock,
            status,
            bar,
            toggle,
            minus,
            plus,
            reset,
            presets: chips,
            callbacks,
        })
    }

    pub fn show(&self, display: &FocusDisplay<'_>) {
        let texts = labels::get();
        self.clock
            .setStringValue(&NSString::from_str(display.clock));
        let (status, color, tone) = if display.finished {
            (&texts.focus_done, Tone::Green.color(), Tone::Green)
        } else if display.running {
            (&texts.focus_running, Tone::Orange.color(), Tone::Orange)
        } else if display.paused {
            (
                &texts.focus_paused,
                white(style::TEXT_SECONDARY),
                Tone::Gray,
            )
        } else {
            (&texts.focus_ready, white(style::TEXT_SECONDARY), Tone::Gray)
        };
        self.status.setStringValue(&NSString::from_str(status));
        self.status.setTextColor(Some(&color));
        self.bar.set(display.progress.clamp(0.0, 1.0), tone);
        let (symbol, title) = if display.running {
            ("pause.fill", &texts.focus_pause)
        } else if display.paused {
            ("play.fill", &texts.focus_resume)
        } else {
            ("play.fill", &texts.focus_start)
        };
        self.toggle.set_symbol(symbol);
        self.toggle.set_title(title);
        // The length can only change while nothing is counting.
        let adjustable = !display.running;
        self.minus.set_enabled(adjustable);
        self.plus.set_enabled(adjustable);
        self.reset
            .set_enabled(display.running || display.paused || display.finished);
        for (minutes, chip) in &self.presets {
            chip.set_selected(*minutes == display.minutes);
            chip.set_enabled(adjustable);
        }
    }

    /// Returns true when the key was consumed.
    pub fn handle_key(&self, key: Key) -> bool {
        match key {
            Key::Enter => (self.callbacks.on_toggle)(),
            Key::Left => (self.callbacks.on_adjust)(-ADJUST_MINUTES),
            Key::Right => (self.callbacks.on_adjust)(ADJUST_MINUTES),
            _ => return false,
        }
        true
    }

    /// The notch tab for this view.
    pub fn page(
        self: &Rc<Self>,
        on_show: impl Fn() + 'static,
        on_hide: impl Fn() + 'static,
    ) -> Page {
        let keys = Rc::clone(self);
        Page {
            title: labels::get().focus_tab.clone(),
            symbol: "timer",
            root: Retained::into_super(Retained::clone(&self.root)),
            on_show: Box::new(on_show),
            on_hide: Box::new(on_hide),
            on_key: Box::new(move |key| keys.handle_key(key)),
            accepts_files: false,
        }
    }
}
