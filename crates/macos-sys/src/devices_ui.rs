//! The Devices tab of the notch: the Bluetooth devices that are connected, each in a card with a symbol, its name and
//! the charge it reports as a gauge. Everything on it is plain data from the app, so nothing here reads Bluetooth or
//! knows a language.

use crate::{
    notch::Page,
    style::{self, Tone, Weight, white},
    widgets::{Fit, draw_page_note, draw_text, text_attributes},
};
use objc2::{DefinedClass, MainThreadMarker, MainThreadOnly, define_class, msg_send, rc::Retained};
use objc2_app_kit::{NSStringDrawing, NSView};
use objc2_foundation::{NSMutableDictionary, NSPoint, NSRect, NSSize, NSString};
use std::{cell::RefCell, rc::Rc};

/// The size of the area the tab fills below the header.
pub const PAGE_SIZE: NSSize = NSSize::new(580.0, 236.0);
/// Cards in a row, and how many the page has room for.
pub const COLUMNS: usize = 3;
pub const MAX_CARDS: usize = 6;

const MARGIN: f64 = 24.0;
const GAP: f64 = 10.0;
const TOP: f64 = 16.0;
const CARD_HEIGHT: f64 = 92.0;
/// The room kept at the bottom for the footer.
const FOOTER_ROOM: f64 = 34.0;
const INSET: f64 = 14.0;

/// One charge a device reports.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Reading {
    /// Which part it is for ("L", "R", "Case"); empty for the device as a whole.
    pub label: String,
    /// 0 to 100.
    pub percent: u8,
    pub low: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Card {
    pub symbol: &'static str,
    pub name: String,
    pub readings: Vec<Reading>,
    /// The words under the gauge: "Low", or that the device says nothing about its charge.
    pub note: String,
    pub low: bool,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DevicesDisplay {
    pub cards: Vec<Card>,
    /// "Updated just now", or how many more devices there are than the page shows.
    pub footer: String,
    /// Words that replace everything: nothing is connected, or the devices could not be read.
    pub note: Option<(String, String)>,
}

type Attributes = NSMutableDictionary<NSString, objc2::runtime::AnyObject>;

fn attrs(size: f64, weight: Weight, alpha: f64, fit: Fit) -> Retained<Attributes> {
    text_attributes(&style::font(size, weight), &white(alpha), fit, false)
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

fn width_of(text: &str, attributes: &Attributes) -> f64 {
    // SAFETY: the attributes dictionary holds valid attribute objects.
    unsafe { NSString::from_str(text).sizeWithAttributes(Some(attributes)) }.width
}

fn gauge(x: f64, top: f64, width: f64, percent: u8, low: bool, height: f64) {
    let bar = NSRect::new(NSPoint::new(x, height - top - 6.0), NSSize::new(width, 6.0));
    style::fill_capsule(bar, &white(0.12));
    let share = f64::from(percent.min(100)) / 100.0;
    // Even an empty one shows a sliver, so that the gauge is seen to be there.
    let fill = NSRect::new(bar.origin, NSSize::new((width * share).max(6.0), 6.0));
    let tone = if low { Tone::Orange } else { Tone::Green };
    style::fill_capsule(fill, &tone.color());
}

fn paint_card(card: &Card, x: f64, top: f64, width: f64, height: f64) {
    let rect = NSRect::new(
        NSPoint::new(x, height - top - CARD_HEIGHT),
        NSSize::new(width, CARD_HEIGHT),
    );
    style::fill_rounded(rect, 14.0, &white(0.07));
    let warning = Tone::Orange.color();
    if card.low {
        style::stroke_rounded(rect, 14.0, &warning, 1.2);
    }
    let tone = if card.low {
        warning.clone()
    } else {
        white(style::TEXT)
    };
    style::draw_symbol(
        card.symbol,
        NSPoint::new(x + INSET + 10.0, height - top - 25.0),
        18.0,
        &tone,
    );
    let inner = width - 2.0 * INSET;
    // A low device says so in words beside its symbol, so that the colour is not the only sign.
    if card.low {
        let word = text_attributes(
            &style::font(10.5, Weight::Semibold),
            &warning,
            Fit::Tail,
            false,
        );
        put(
            &card.note,
            (x + INSET + 30.0, top + 18.0, inner - 30.0 - 60.0, 14.0),
            &word,
            height,
        );
    }
    // The charge of the device as a whole, big, at the right; a pair of earbuds with a case has none.
    if let Some(r) = card.readings.iter().find(|r| r.label.is_empty()) {
        let text = format!("{}%", r.percent);
        let big = text_attributes(
            &style::digits(20.0, Weight::Semibold),
            &white(style::TEXT),
            Fit::Clip,
            false,
        );
        let at = width_of(&text, &big).ceil() + 2.0;
        put(
            &text,
            (x + width - INSET - at, top + 12.0, at, 26.0),
            &big,
            height,
        );
    }
    put(
        &card.name,
        (x + INSET, top + 42.0, inner, 16.0),
        &attrs(12.0, Weight::Medium, style::TEXT, Fit::Tail),
        height,
    );
    match card.readings.as_slice() {
        [] => {
            style::fill_capsule(
                NSRect::new(
                    NSPoint::new(x + INSET, height - top - 68.0 - 6.0),
                    NSSize::new(inner, 6.0),
                ),
                &white(0.06),
            );
            put(
                &card.note,
                (x + INSET, top + 76.0, inner, 13.0),
                &attrs(10.5, Weight::Regular, style::TEXT_TERTIARY, Fit::Tail),
                height,
            );
        }
        [only] if only.label.is_empty() => {
            gauge(x + INSET, top + 68.0, inner, only.percent, only.low, height);
        }
        parts => {
            // Earbuds: a small gauge for each part with its label and figure above it.
            let n = parts.len().min(3) as f64;
            let each = (inner - 4.0 * (n - 1.0)) / n;
            let small = attrs(9.0, Weight::Medium, style::TEXT_SECONDARY, Fit::Clip);
            for (i, r) in parts.iter().take(3).enumerate() {
                let px = x + INSET + i as f64 * (each + 4.0);
                let label = if r.label.is_empty() {
                    format!("{}%", r.percent)
                } else {
                    format!("{} {}%", r.label, r.percent)
                };
                put(&label, (px, top + 60.0, each, 12.0), &small, height);
                gauge(px, top + 74.0, each, r.percent, r.low, height);
            }
        }
    }
}

fn paint(bounds: NSRect, d: &DevicesDisplay) {
    let size = bounds.size;
    if let Some((title, hint)) = &d.note {
        draw_page_note("headphones", title, hint, size);
        return;
    }
    let width = (size.width - 2.0 * MARGIN - GAP * (COLUMNS - 1) as f64) / COLUMNS as f64;
    let shown = d.cards.len().min(MAX_CARDS);
    let rows = shown.div_ceil(COLUMNS);
    // The rows are centred in the room above the footer, and a last row with fewer cards is centred too.
    let used = rows as f64 * CARD_HEIGHT + rows.saturating_sub(1) as f64 * GAP;
    let first = TOP + ((size.height - TOP - FOOTER_ROOM - used) / 2.0).max(0.0);
    for (i, card) in d.cards.iter().take(MAX_CARDS).enumerate() {
        let (column, row) = (i % COLUMNS, i / COLUMNS);
        let in_row = if row + 1 == rows {
            shown - row * COLUMNS
        } else {
            COLUMNS
        };
        let indent = (COLUMNS - in_row) as f64 * (width + GAP) / 2.0;
        paint_card(
            card,
            MARGIN + indent + column as f64 * (width + GAP),
            first + row as f64 * (CARD_HEIGHT + GAP),
            width,
            size.height,
        );
    }
    put(
        &d.footer,
        (MARGIN, size.height - 24.0, size.width - 2.0 * MARGIN, 14.0),
        &attrs(10.5, Weight::Regular, style::TEXT_TERTIARY, Fit::Tail),
        size.height,
    );
}

pub struct CanvasIvars {
    display: RefCell<DevicesDisplay>,
}

define_class!(
    // SAFETY: NSView allows subclassing; the ivars are only touched on the main thread and need no Drop.
    #[unsafe(super(NSView))]
    #[thread_kind = MainThreadOnly]
    #[name = "FigDevicesCanvas"]
    #[ivars = CanvasIvars]
    pub struct DevicesCanvas;

    impl DevicesCanvas {
        #[unsafe(method(drawRect:))]
        fn draw_rect(&self, _dirty: NSRect) {
            paint(self.bounds(), &self.ivars().display.borrow());
        }
    }
);

impl DevicesCanvas {
    fn new(mtm: MainThreadMarker, frame: NSRect) -> Retained<Self> {
        let this = mtm.alloc::<Self>().set_ivars(CanvasIvars {
            display: RefCell::default(),
        });
        // SAFETY: designated initializer with valid arguments on the main thread.
        unsafe { msg_send![super(this), initWithFrame: frame] }
    }
}

pub struct DevicesView {
    canvas: Retained<DevicesCanvas>,
}

impl DevicesView {
    /// `frame` is `Notch::page_area(PAGE_SIZE.width, PAGE_SIZE.height)`.
    pub fn new(mtm: MainThreadMarker, frame: NSRect) -> Rc<Self> {
        Rc::new(Self {
            canvas: DevicesCanvas::new(mtm, frame),
        })
    }

    pub fn show(&self, display: DevicesDisplay) {
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
            id: "devices",
            title,
            symbol: "headphones",
            root: Retained::into_super(Retained::clone(&self.canvas)),
            on_show: Box::new(on_show),
            on_hide: Box::new(on_hide),
            on_key: Box::new(|_| false),
            accepts_files: false,
            size: None,
        }
    }
}
