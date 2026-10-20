//! The Day progress tab of the notch: how much of the day has gone, one line of time from the morning to bedtime with the
//! events of today laid along it and the reminders marked on it, and what comes next. Everything on it is plain data from
//! the app, so nothing here reads a calendar or knows a language.

use crate::{
    notch::Page,
    style::{self, Weight, white},
    widgets::{Fit, draw_page_note, draw_text, text_attributes},
};
use objc2::{DefinedClass, MainThreadMarker, MainThreadOnly, define_class, msg_send, rc::Retained};
use objc2_app_kit::{NSBezierPath, NSColor, NSStringDrawing, NSView};
use objc2_foundation::{NSPoint, NSRect, NSSize, NSString};
use std::{cell::RefCell, rc::Rc};

/// The size of the area the tab fills below the header.
pub const PAGE_SIZE: NSSize = NSSize::new(580.0, 250.0);
/// How many rows of events the timeline has room for.
pub const LANES: usize = 3;

const MARGIN: f64 = 24.0;
const TRACK_HEIGHT: f64 = 12.0;
const LANE_HEIGHT: f64 = 7.0;
const LANE_GAP: f64 = 3.0;
const ROW_HEIGHT: f64 = 22.0;
/// How many things of the day are listed under the line.
pub const AGENDA: usize = 3;

/// An event laid along the line of the day. `start` and `end` are shares of the line, 0 to 1.
#[derive(Debug, Clone)]
pub struct Block {
    pub start: f64,
    pub end: f64,
    pub lane: usize,
    /// 0xRRGGBB, the colour of its calendar.
    pub color: u32,
    /// It is on now.
    pub running: bool,
}

/// One line of the list of what is left of the day.
#[derive(Debug, Clone)]
pub struct Row {
    pub color: u32,
    pub title: String,
    /// "now", "in 40 min", "2:00 PM"
    pub when: String,
    pub running: bool,
}

/// A reminder marked on the line at its time.
#[derive(Debug, Clone)]
pub struct Mark {
    pub at: f64,
    pub color: u32,
}

#[derive(Debug, Clone, Default)]
pub struct DayDisplay {
    /// "63%"
    pub percent: String,
    /// "of the day has gone"
    pub caption: String,
    /// "9 h 36 min left"
    pub left: String,
    /// Where now is on the line, 0 to 1.
    pub now: f64,
    /// A label under the line at a share of it: ("7 AM", 0.0).
    pub ticks: Vec<(String, f64)>,
    pub blocks: Vec<Block>,
    pub marks: Vec<Mark>,
    /// What is on now and what comes next.
    pub agenda: Vec<Row>,
    /// What the list says when there is nothing left: "Nothing more today".
    pub empty: String,
    /// "5 events, 3 reminders left".
    pub summary: String,
    /// Words that replace everything: the tab is off, or the calendar is not allowed.
    pub note: Option<(String, String)>,
}

fn attrs(
    size: f64,
    weight: Weight,
    alpha: f64,
    centred: bool,
) -> Retained<objc2_foundation::NSMutableDictionary<NSString, objc2::runtime::AnyObject>> {
    text_attributes(
        &style::font(size, weight),
        &white(alpha),
        Fit::Tail,
        centred,
    )
}

fn rgb(color: u32) -> Retained<NSColor> {
    let channel = |shift: u32| f64::from((color >> shift) & 0xff) / 255.0;
    NSColor::colorWithSRGBRed_green_blue_alpha(channel(16), channel(8), channel(0), 1.0)
}

/// Text with its top edge `top` points below the top of a view `height` tall whose origin is at the bottom.
fn put(
    text: &str,
    (x, top, width, line): (f64, f64, f64, f64),
    attributes: &objc2_foundation::NSMutableDictionary<NSString, objc2::runtime::AnyObject>,
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

fn width_of(
    text: &str,
    attributes: &objc2_foundation::NSMutableDictionary<NSString, objc2::runtime::AnyObject>,
) -> f64 {
    // SAFETY: the attributes dictionary holds valid attribute objects.
    unsafe { NSString::from_str(text).sizeWithAttributes(Some(attributes)) }.width
}

fn paint(bounds: NSRect, d: &DayDisplay) {
    let size = bounds.size;
    if let Some((title, hint)) = &d.note {
        draw_page_note("sun.horizon.fill", title, hint, size);
        return;
    }
    let (w, h) = (size.width, size.height);
    let line = w - 2.0 * MARGIN;

    // The figure and what it means, with the time that is left at the right.
    let big = text_attributes(
        &style::digits(40.0, Weight::Light),
        &white(style::TEXT),
        Fit::Tail,
        false,
    );
    put(&d.percent, (MARGIN, 14.0, 150.0, 48.0), &big, h);
    let percent_width = width_of(&d.percent, &big).ceil();
    put(
        &d.caption,
        (MARGIN + percent_width + 10.0, 40.0, 220.0, 16.0),
        &attrs(12.0, Weight::Regular, style::TEXT_SECONDARY, false),
        h,
    );
    let left_attrs = attrs(13.0, Weight::Medium, style::TEXT, false);
    let left_width = width_of(&d.left, &left_attrs).ceil() + 2.0;
    put(
        &d.left,
        (w - MARGIN - left_width, 34.0, left_width, 18.0),
        &left_attrs,
        h,
    );

    // The line of the day: a track, the part that has gone, and now.
    let track_top = 78.0;
    let track = NSRect::new(
        NSPoint::new(MARGIN, h - track_top - TRACK_HEIGHT),
        NSSize::new(line, TRACK_HEIGHT),
    );
    style::fill_capsule(track, &white(0.10));
    let now = d.now.clamp(0.0, 1.0);
    if now > 0.0 {
        let gone = NSRect::new(
            track.origin,
            NSSize::new((line * now).max(TRACK_HEIGHT), TRACK_HEIGHT),
        );
        style::fill_capsule(gone, &style::Tone::Blue.color());
    }
    // Reminders sit on the track as small bright dots.
    for mark in &d.marks {
        let x = MARGIN + line * mark.at.clamp(0.0, 1.0);
        style::fill_circle(
            NSRect::new(
                NSPoint::new(x - 4.0, track.origin.y + TRACK_HEIGHT / 2.0 - 4.0),
                NSSize::new(8.0, 8.0),
            ),
            &rgb(mark.color),
        );
    }
    // Hours under the track.
    let tick_attrs = attrs(9.5, Weight::Medium, style::TEXT_TERTIARY, true);
    for (label, at) in &d.ticks {
        let x = MARGIN + line * at.clamp(0.0, 1.0);
        put(
            label,
            (x - 30.0, track_top + TRACK_HEIGHT + 4.0, 60.0, 12.0),
            &tick_attrs,
            h,
        );
    }
    // The events, as thin bars below the hours in the colour of their calendar, the one on now bright.
    let lanes_top = track_top + TRACK_HEIGHT + 22.0;
    for block in &d.blocks {
        let (start, end) = (block.start.clamp(0.0, 1.0), block.end.clamp(0.0, 1.0));
        let x = MARGIN + line * start;
        let width = (line * (end - start)).max(LANE_HEIGHT);
        let top = lanes_top + block.lane.min(LANES - 1) as f64 * (LANE_HEIGHT + LANE_GAP);
        let color = rgb(block.color);
        let shade = if block.running { 1.0 } else { 0.6 };
        style::fill_capsule(
            NSRect::new(
                NSPoint::new(x, h - top - LANE_HEIGHT),
                NSSize::new(width, LANE_HEIGHT),
            ),
            &color.colorWithAlphaComponent(shade),
        );
    }
    // Now, as a line through the track and the bars.
    let now_x = MARGIN + line * now;
    white(0.9).setStroke();
    let marker = NSBezierPath::bezierPath();
    marker.moveToPoint(NSPoint::new(now_x, h - track_top + 5.0));
    marker.lineToPoint(NSPoint::new(
        now_x,
        h - lanes_top - LANES as f64 * (LANE_HEIGHT + LANE_GAP) + 2.0,
    ));
    marker.setLineWidth(1.5);
    marker.stroke();

    // Under it all: what is on and what comes next, one to a line, and how much is left to do.
    let list_top = lanes_top + LANES as f64 * (LANE_HEIGHT + LANE_GAP) + 10.0;
    if d.agenda.is_empty() {
        put(
            &d.empty,
            (MARGIN, list_top + 8.0, line, 16.0),
            &attrs(12.0, Weight::Regular, style::TEXT_SECONDARY, false),
            h,
        );
    }
    let when_attrs = attrs(11.0, Weight::Regular, style::TEXT_SECONDARY, false);
    for (i, row) in d.agenda.iter().take(AGENDA).enumerate() {
        let top = list_top + i as f64 * ROW_HEIGHT;
        style::fill_circle(
            NSRect::new(
                NSPoint::new(MARGIN, h - top - ROW_HEIGHT / 2.0 - 4.0),
                NSSize::new(8.0, 8.0),
            ),
            &rgb(row.color),
        );
        let when_width = width_of(&row.when, &when_attrs).ceil() + 2.0;
        put(
            &row.title,
            (
                MARGIN + 18.0,
                top + 3.0,
                line - 18.0 - when_width - 12.0,
                16.0,
            ),
            &attrs(
                12.0,
                if row.running {
                    Weight::Semibold
                } else {
                    Weight::Medium
                },
                style::TEXT,
                false,
            ),
            h,
        );
        put(
            &row.when,
            (w - MARGIN - when_width, top + 4.0, when_width, 14.0),
            &when_attrs,
            h,
        );
    }
    let summary = attrs(10.5, Weight::Regular, style::TEXT_TERTIARY, false);
    let summary_width = width_of(&d.summary, &summary).ceil() + 2.0;
    put(
        &d.summary,
        (w - MARGIN - summary_width, h - 22.0, summary_width, 14.0),
        &summary,
        h,
    );
}

pub struct CanvasIvars {
    display: RefCell<DayDisplay>,
}

define_class!(
    // SAFETY: NSView allows subclassing; the ivars are only touched on the main thread and need no Drop.
    #[unsafe(super(NSView))]
    #[thread_kind = MainThreadOnly]
    #[name = "FigDayCanvas"]
    #[ivars = CanvasIvars]
    pub struct DayCanvas;

    impl DayCanvas {
        #[unsafe(method(drawRect:))]
        fn draw_rect(&self, _dirty: NSRect) {
            paint(self.bounds(), &self.ivars().display.borrow());
        }
    }
);

impl DayCanvas {
    fn new(mtm: MainThreadMarker, frame: NSRect) -> Retained<Self> {
        let this = mtm.alloc::<Self>().set_ivars(CanvasIvars {
            display: RefCell::default(),
        });
        // SAFETY: designated initializer with valid arguments on the main thread.
        unsafe { msg_send![super(this), initWithFrame: frame] }
    }
}

pub struct DayView {
    canvas: Retained<DayCanvas>,
}

impl DayView {
    /// `frame` is `Notch::page_area(PAGE_SIZE.width, PAGE_SIZE.height)`.
    pub fn new(mtm: MainThreadMarker, frame: NSRect) -> Rc<Self> {
        Rc::new(Self {
            canvas: DayCanvas::new(mtm, frame),
        })
    }

    pub fn show(&self, display: DayDisplay) {
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
            id: "dayprogress",
            title,
            symbol: "sun.horizon.fill",
            root: Retained::into_super(Retained::clone(&self.canvas)),
            on_show: Box::new(on_show),
            on_hide: Box::new(on_hide),
            on_key: Box::new(|_| false),
            accepts_files: false,
            size: None,
        }
    }
}
