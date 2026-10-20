//! The Pomodoro tab of the notch: a ring that fills as a stage passes with the clock in it, the dots of the round, the
//! buttons, and how many sessions were done today, the streak and a week of bars. Everything on it is plain data from the
//! app, so nothing here keeps time or knows a language.

use crate::{
    button::{Look, TouchButton},
    notch::Page,
    style::{self, Tone, Weight, white},
    widgets::{Fit, draw_page_note, draw_text, text_attributes},
};
use objc2::{DefinedClass, MainThreadMarker, MainThreadOnly, define_class, msg_send, rc::Retained};
use objc2_app_kit::{NSBezierPath, NSLineCapStyle, NSView};
use objc2_foundation::{NSPoint, NSRect, NSSize, NSString};
use std::{cell::RefCell, rc::Rc};

/// The size of the area the tab fills below the header.
pub const PAGE_SIZE: NSSize = NSSize::new(580.0, 268.0);

const MARGIN: f64 = 18.0;
const RING: f64 = 176.0;
const RING_WIDTH: f64 = 10.0;
const COLUMN: f64 = 270.0;
const CARD_HEIGHT: f64 = 100.0;
const RADIUS: f64 = 14.0;

/// Which stage the ring is of: it sets the colour.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum Hue {
    #[default]
    Work,
    ShortBreak,
    LongBreak,
}

impl Hue {
    fn tone(self) -> Tone {
        match self {
            Hue::Work => Tone::Orange,
            Hue::ShortBreak => Tone::Green,
            Hue::LongBreak => Tone::Blue,
        }
    }
}

/// The words on the buttons, which name them for a person who cannot see them.
#[derive(Debug, Clone, Default)]
pub struct Labels {
    pub start: String,
    pub pause: String,
    pub resume: String,
    pub skip: String,
    pub reset: String,
}

#[derive(Debug, Clone, Default)]
pub struct PomodoroDisplay {
    pub hue: Hue,
    /// "Focus", "Short break", "Long break"
    pub stage: String,
    /// "24:31"
    pub clock: String,
    /// What is happening: "Running", "Paused", "Ready".
    pub status: String,
    /// How much of the stage has passed, 0 to 1.
    pub progress: f64,
    pub running: bool,
    pub paused: bool,
    /// "Session 2 of 4"
    pub round: String,
    /// The sessions done in the round, and how many a round has.
    pub dots: (usize, usize),
    /// The dot after the done ones is the session that is on.
    pub dot_active: bool,
    /// "TODAY" and what was done: "3 sessions, 75 min".
    pub today_caption: String,
    pub today: String,
    pub streak_caption: String,
    pub streak: String,
    /// The sessions of the last seven days against the busiest, 0 to 1, the oldest first, with their initials.
    pub week: [f64; 7],
    pub week_labels: [String; 7],
    pub labels: Labels,
    /// Words that replace everything: the tab is off.
    pub note: Option<(String, String)>,
}

pub struct PomodoroCallbacks {
    pub on_toggle: Box<dyn Fn()>,
    pub on_skip: Box<dyn Fn()>,
    pub on_reset: Box<dyn Fn()>,
}

type Attributes =
    Retained<objc2_foundation::NSMutableDictionary<NSString, objc2::runtime::AnyObject>>;

fn attrs(size: f64, weight: Weight, alpha: f64, centred: bool) -> Attributes {
    text_attributes(
        &style::font(size, weight),
        &white(alpha),
        Fit::Tail,
        centred,
    )
}

fn digits(size: f64, weight: Weight, alpha: f64, centred: bool) -> Attributes {
    text_attributes(
        &style::digits(size, weight),
        &white(alpha),
        Fit::Tail,
        centred,
    )
}

/// Text with its top edge `top` points below the top of a view `height` tall whose origin is at the bottom.
fn put(
    text: &str,
    (x, top, width, line): (f64, f64, f64, f64),
    attributes: &Attributes,
    height: f64,
) {
    draw_text(
        text,
        NSRect::new(
            NSPoint::new(x, height - top - line),
            NSSize::new(width, line),
        ),
        attributes,
    );
}

pub struct CanvasIvars {
    display: RefCell<PomodoroDisplay>,
}

define_class!(
    // SAFETY: NSView allows subclassing; the ivars are only touched on the main thread and need no Drop.
    #[unsafe(super(NSView))]
    #[thread_kind = MainThreadOnly]
    #[name = "FigPomodoroCanvas"]
    #[ivars = CanvasIvars]
    pub struct PomodoroCanvas;

    impl PomodoroCanvas {
        #[unsafe(method(drawRect:))]
        fn draw_rect(&self, _dirty: NSRect) {
            paint(self.bounds(), &self.ivars().display.borrow());
        }
    }
);

impl PomodoroCanvas {
    fn new(mtm: MainThreadMarker, frame: NSRect) -> Retained<Self> {
        let this = mtm.alloc::<Self>().set_ivars(CanvasIvars {
            display: RefCell::default(),
        });
        // SAFETY: designated initializer with valid arguments on the main thread.
        unsafe { msg_send![super(this), initWithFrame: frame] }
    }
}

/// Where the middle of the ring is.
fn ring_centre(size: NSSize) -> NSPoint {
    NSPoint::new(MARGIN + 8.0 + RING / 2.0, size.height / 2.0)
}

fn paint(bounds: NSRect, d: &PomodoroDisplay) {
    let size = bounds.size;
    if let Some((title, hint)) = &d.note {
        draw_page_note("timer", title, hint, size);
        return;
    }
    let h = size.height;
    let centre = ring_centre(size);
    let radius = RING / 2.0 - RING_WIDTH / 2.0;
    let tone = d.hue.tone();

    // The ring: a track all the way round, and over it the part of the stage that has passed, from the top clockwise.
    let track = NSBezierPath::bezierPathWithOvalInRect(NSRect::new(
        NSPoint::new(centre.x - radius, centre.y - radius),
        NSSize::new(2.0 * radius, 2.0 * radius),
    ));
    track.setLineWidth(RING_WIDTH);
    white(0.10).setStroke();
    track.stroke();
    let progress = d.progress.clamp(0.0, 1.0);
    if progress > 0.0 {
        let arc = NSBezierPath::bezierPath();
        arc.appendBezierPathWithArcWithCenter_radius_startAngle_endAngle_clockwise(
            centre,
            radius,
            90.0,
            90.0 - 360.0 * progress.min(0.9999),
            true,
        );
        arc.setLineWidth(RING_WIDTH);
        arc.setLineCapStyle(NSLineCapStyle::Round);
        tone.color().setStroke();
        arc.stroke();
    }
    // In the ring: the stage, the clock and what is going on.
    let inner = RING - 2.0 * RING_WIDTH - 16.0;
    let left = centre.x - inner / 2.0;
    let top_of_ring = h - (centre.y + RING / 2.0);
    put(
        &d.stage.to_uppercase(),
        (left, top_of_ring + 54.0, inner, 12.0),
        &attrs(9.5, Weight::Semibold, style::TEXT_TERTIARY, true),
        h,
    );
    put(
        &d.clock,
        (left, top_of_ring + 68.0, inner, 46.0),
        &digits(40.0, Weight::Light, style::TEXT, true),
        h,
    );
    put(
        &d.status,
        (left, top_of_ring + 118.0, inner, 14.0),
        &attrs(11.0, Weight::Medium, style::TEXT_SECONDARY, true),
        h,
    );

    // The right column: the round, then (under the buttons the view adds) the statistics.
    put(
        &d.round,
        (COLUMN, 22.0, size.width - COLUMN - MARGIN, 16.0),
        &attrs(13.0, Weight::Semibold, style::TEXT, false),
        h,
    );
    let (done, total) = d.dots;
    for i in 0..total {
        let rect = NSRect::new(
            NSPoint::new(COLUMN + i as f64 * 22.0, h - 46.0 - 14.0),
            NSSize::new(14.0, 14.0),
        );
        if i < done {
            style::fill_circle(rect, &tone.color());
        } else if i == done && d.dot_active {
            style::fill_circle(rect, &tone.color().colorWithAlphaComponent(0.45));
        } else {
            style::fill_circle(rect, &white(0.14));
        }
    }

    // The statistics card.
    let card = NSRect::new(
        NSPoint::new(COLUMN, 14.0),
        NSSize::new(size.width - COLUMN - MARGIN, CARD_HEIGHT),
    );
    style::card(card, RADIUS);
    let top = h - card.origin.y - card.size.height;
    let x = card.origin.x + 16.0;
    for (i, (caption, value)) in [(&d.today_caption, &d.today), (&d.streak_caption, &d.streak)]
        .into_iter()
        .enumerate()
    {
        let row = top + 14.0 + i as f64 * 40.0;
        put(
            &caption.to_uppercase(),
            (x, row, 130.0, 12.0),
            &attrs(9.5, Weight::Semibold, style::TEXT_TERTIARY, false),
            h,
        );
        put(
            value,
            (x, row + 14.0, 130.0, 18.0),
            &digits(13.0, Weight::Medium, style::TEXT, false),
            h,
        );
    }
    // A week of bars: seven columns, the oldest at the left, with the initial of the day under each.
    let chart_left = card.origin.x + 150.0;
    let chart_width = card.size.width - 150.0 - 16.0;
    let column = chart_width / 7.0;
    let floor = card.origin.y + 26.0;
    let tallest = CARD_HEIGHT - 26.0 - 18.0;
    for i in 0..7 {
        let cx = chart_left + (i as f64 + 0.5) * column;
        let bar = NSRect::new(
            NSPoint::new(cx - 5.0, floor),
            NSSize::new(10.0, (tallest * d.week[i].clamp(0.0, 1.0)).max(3.0)),
        );
        let lit = d.week[i] > 0.0;
        let fill = if lit { tone.color() } else { white(0.12) };
        style::fill_rounded(bar, 3.0, &fill);
        put(
            &d.week_labels[i],
            (cx - column / 2.0, top + CARD_HEIGHT - 20.0, column, 12.0),
            &attrs(9.0, Weight::Medium, style::TEXT_TERTIARY, true),
            h,
        );
    }
}

pub struct PomodoroView {
    canvas: Retained<PomodoroCanvas>,
    toggle: Retained<TouchButton>,
    skip: Retained<TouchButton>,
    reset: Retained<TouchButton>,
}

impl PomodoroView {
    /// `frame` is `Notch::page_area(PAGE_SIZE.width, PAGE_SIZE.height)`.
    pub fn new(mtm: MainThreadMarker, frame: NSRect, callbacks: PomodoroCallbacks) -> Rc<Self> {
        let canvas = PomodoroCanvas::new(mtm, frame);
        let h = frame.size.height;
        let callbacks = Rc::new(callbacks);
        let round = |x: f64, side: f64, filled: bool, symbol: &str| {
            let button = TouchButton::new(
                mtm,
                NSRect::new(NSPoint::new(x, h - 76.0 - side), NSSize::new(side, side)),
                Look::Round { filled },
            );
            button.set_symbol(symbol);
            canvas.addSubview(&button);
            button
        };
        let toggle = round(COLUMN, 54.0, true, "play.fill");
        let skip = round(COLUMN + 54.0 + 16.0, 42.0, false, "forward.end.fill");
        let reset = round(
            COLUMN + 54.0 + 16.0 + 42.0 + 12.0,
            42.0,
            false,
            "arrow.counterclockwise",
        );
        // The row of buttons is centred on the toggle, which is the tallest.
        for button in [&skip, &reset] {
            let mut f = button.frame();
            f.origin.y = h - 76.0 - 54.0 + (54.0 - f.size.height) / 2.0;
            button.setFrame(f);
        }
        let shared = Rc::clone(&callbacks);
        toggle.set_on_click(move || (shared.on_toggle)());
        let shared = Rc::clone(&callbacks);
        skip.set_on_click(move || (shared.on_skip)());
        let shared = Rc::clone(&callbacks);
        reset.set_on_click(move || (shared.on_reset)());
        Rc::new(Self {
            canvas,
            toggle,
            skip,
            reset,
        })
    }

    /// Draws what the tab is now to show.
    pub fn show(&self, display: PomodoroDisplay) {
        let off = display.note.is_some();
        let labels = display.labels.clone();
        let (symbol, title) = if display.running {
            ("pause.fill", &labels.pause)
        } else if display.paused {
            ("play.fill", &labels.resume)
        } else {
            ("play.fill", &labels.start)
        };
        self.toggle.set_symbol(symbol);
        self.toggle.set_title(title);
        self.skip.set_title(&labels.skip);
        self.reset.set_title(&labels.reset);
        self.reset.set_enabled(display.running || display.paused);
        for button in [&self.toggle, &self.skip, &self.reset] {
            button.setHidden(off);
        }
        *self.canvas.ivars().display.borrow_mut() = display;
        self.canvas.setNeedsDisplay(true);
    }

    /// The notch tab for this view.
    pub fn page(
        self: &Rc<Self>,
        title: String,
        on_show: impl Fn() + 'static,
        on_hide: impl Fn() + 'static,
    ) -> Page {
        Page {
            id: "pomodoro",
            title,
            symbol: "timer",
            root: Retained::into_super(Retained::clone(&self.canvas)),
            on_show: Box::new(on_show),
            on_hide: Box::new(on_hide),
            on_key: Box::new(|_| false),
            accepts_files: false,
            size: None,
        }
    }
}
