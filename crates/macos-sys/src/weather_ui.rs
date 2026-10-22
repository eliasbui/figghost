//! The Weather tab of the notch: the weather now, the next hours and the next days of one place, with a control to
//! flip to the next city when there are several. Everything on it is plain data from the app, so nothing here reads
//! the network or knows a language.

use crate::{
    button::{Look, TouchButton},
    notch::Page,
    panel::Key,
    style::{self, Weight, white},
    widgets::{Fit, draw_page_note, draw_text, text_attributes},
};
use objc2::{DefinedClass, MainThreadMarker, MainThreadOnly, define_class, msg_send, rc::Retained};
use objc2_app_kit::{NSColor, NSStringDrawing, NSView};
use objc2_foundation::{NSPoint, NSRect, NSSize, NSString};
use std::{cell::RefCell, rc::Rc};

/// The size of the area the tab fills below the header.
pub const PAGE_SIZE: NSSize = NSSize::new(580.0, 250.0);

const MARGIN: f64 = 18.0;
const GAP: f64 = 10.0;
const NOW_WIDTH: f64 = 196.0;
const RADIUS: f64 = 14.0;
const HOURS_HEIGHT: f64 = 84.0;
/// The flip control at the left of the line over the cards: two round buttons around "2/3".
const FLIP_BUTTON: f64 = 22.0;
const FLIP_LABEL: f64 = 34.0;
const FLIP_WIDTH: f64 = FLIP_BUTTON * 2.0 + FLIP_LABEL + 8.0;
/// The middle of the line over the cards, from the top.
const HEADER_MIDDLE: f64 = 18.0;
const NOTE_BUTTON_HEIGHT: f64 = 32.0;

/// The colour a weather glyph is drawn in: the sky says what it is, and white would say nothing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Hue {
    Sun,
    Moon,
    Cloud,
    Rain,
    Snow,
    Storm,
    Fog,
}

impl Hue {
    fn color(self) -> Retained<NSColor> {
        let (r, g, b) = match self {
            Hue::Sun => (1.0, 0.80, 0.18),
            Hue::Moon => (0.78, 0.82, 1.0),
            Hue::Cloud => (0.86, 0.88, 0.92),
            Hue::Rain => (0.35, 0.65, 1.0),
            Hue::Snow => (0.80, 0.93, 1.0),
            Hue::Storm => (1.0, 0.72, 0.20),
            Hue::Fog => (0.66, 0.68, 0.72),
        };
        NSColor::colorWithSRGBRed_green_blue_alpha(r, g, b, 1.0)
    }
}

/// A glyph: an SF Symbol and the colour to draw it in.
#[derive(Debug, Clone, Copy)]
pub struct Glyph {
    pub symbol: &'static str,
    pub hue: Hue,
}

/// The weather now.
#[derive(Debug, Clone)]
pub struct Current {
    pub glyph: Glyph,
    /// "31°"
    pub temp: String,
    /// "Partly cloudy"
    pub condition: String,
    /// Up to five facts under it, such as ("Feels like", "35°").
    pub facts: Vec<(String, String)>,
}

#[derive(Debug, Clone)]
pub struct Hour {
    pub label: String,
    pub glyph: Glyph,
    pub temp: String,
    /// The chance of rain, such as "40%", or empty when it is small.
    pub rain: String,
}

#[derive(Debug, Clone)]
pub struct Day {
    pub label: String,
    pub glyph: Glyph,
    pub high: String,
    pub low: String,
}

/// Which of several cities shows, for the flip control.
#[derive(Debug, Clone)]
pub struct Pager {
    /// From 1.
    pub position: usize,
    pub count: usize,
    /// What the buttons are called for assistive tools: "Previous city", "Next city".
    pub previous: String,
    pub next: String,
}

#[derive(Debug, Clone, Default)]
pub struct WeatherDisplay {
    pub place: String,
    /// "Updated 3 minutes ago", or what is being done.
    pub status: String,
    pub current: Option<Current>,
    pub hours: Vec<Hour>,
    pub days: Vec<Day>,
    /// Words that replace everything: the tab is off, there is no city, no connection.
    pub note: Option<(String, String)>,
    /// A button under the note, such as "Open Settings".
    pub action: Option<String>,
    /// Shown when there is more than one city, over the note too, so that a city that cannot be shown can be flipped past.
    pub pager: Option<Pager>,
    /// Where the data comes from.
    pub credit: String,
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

fn digits(
    size: f64,
    weight: Weight,
    alpha: f64,
    centred: bool,
) -> Retained<objc2_foundation::NSMutableDictionary<NSString, objc2::runtime::AnyObject>> {
    text_attributes(
        &style::digits(size, weight),
        &white(alpha),
        Fit::Tail,
        centred,
    )
}

/// Text with its top edge `top` points below the top of a view `view_height` tall whose origin is at the bottom.
fn put(
    text: &str,
    (x, top, width, height): (f64, f64, f64, f64),
    attributes: &objc2_foundation::NSMutableDictionary<NSString, objc2::runtime::AnyObject>,
    view_height: f64,
) {
    draw_text(
        text,
        NSRect::new(
            NSPoint::new(x, view_height - top - height),
            NSSize::new(width, height),
        ),
        attributes,
    );
}

fn glyph(g: Glyph, centre_x: f64, centre_top: f64, size: f64, view_height: f64) {
    style::draw_symbol(
        g.symbol,
        NSPoint::new(centre_x, view_height - centre_top),
        size,
        &g.hue.color(),
    );
}

fn width_of(
    text: &str,
    attributes: &objc2_foundation::NSMutableDictionary<NSString, objc2::runtime::AnyObject>,
) -> f64 {
    // SAFETY: the attributes dictionary holds valid attribute objects.
    unsafe { NSString::from_str(text).sizeWithAttributes(Some(attributes)) }.width
}

fn draw_now(current: &Current, card: NSRect, h: f64) {
    style::card(card, RADIUS);
    let x = card.origin.x + 16.0;
    let top = h - card.origin.y - card.size.height;
    let inner = card.size.width - 32.0;
    glyph(current.glyph, x + 18.0, top + 36.0, 34.0, h);
    put(
        &current.temp,
        (x + 48.0, top + 14.0, inner - 48.0, 52.0),
        &digits(44.0, Weight::Light, style::TEXT, false),
        h,
    );
    put(
        &current.condition,
        (x, top + 70.0, inner, 16.0),
        &attrs(12.5, Weight::Medium, style::TEXT, false),
        h,
    );
    for (i, (name, value)) in current.facts.iter().take(5).enumerate() {
        let row = top + 96.0 + i as f64 * 18.0;
        put(
            name,
            (x, row, inner * 0.6, 14.0),
            &attrs(10.5, Weight::Regular, style::TEXT_SECONDARY, false),
            h,
        );
        let value_attrs = digits(10.5, Weight::Medium, style::TEXT, false);
        let value_w = width_of(value, &value_attrs).ceil() + 2.0;
        put(
            value,
            (x + inner - value_w, row, value_w, 14.0),
            &value_attrs,
            h,
        );
    }
}

fn draw_hours(hours: &[Hour], card: NSRect, h: f64) {
    style::card(card, RADIUS);
    if hours.is_empty() {
        return;
    }
    let top = h - card.origin.y - card.size.height;
    let column = (card.size.width - 12.0) / hours.len() as f64;
    for (i, hour) in hours.iter().enumerate() {
        let left = card.origin.x + 6.0 + i as f64 * column;
        let centre = left + column / 2.0;
        put(
            &hour.label,
            (left, top + 8.0, column, 13.0),
            &attrs(10.0, Weight::Medium, style::TEXT_SECONDARY, true),
            h,
        );
        glyph(hour.glyph, centre, top + 36.0, 17.0, h);
        put(
            &hour.temp,
            (left, top + 48.0, column, 15.0),
            &digits(12.5, Weight::Medium, style::TEXT, true),
            h,
        );
        if !hour.rain.is_empty() {
            put(
                &hour.rain,
                (left, top + 65.0, column, 12.0),
                &text_attributes(
                    &style::digits(9.5, Weight::Medium),
                    &Hue::Rain.color(),
                    Fit::Tail,
                    true,
                ),
                h,
            );
        }
    }
}

fn draw_days(days: &[Day], card: NSRect, h: f64) {
    style::card(card, RADIUS);
    if days.is_empty() {
        return;
    }
    let top = h - card.origin.y - card.size.height;
    let column = (card.size.width - 12.0) / days.len() as f64;
    for (i, day) in days.iter().enumerate() {
        let left = card.origin.x + 6.0 + i as f64 * column;
        let centre = left + column / 2.0;
        put(
            &day.label,
            (left, top + 8.0, column, 13.0),
            &attrs(10.0, Weight::Medium, style::TEXT_SECONDARY, true),
            h,
        );
        glyph(day.glyph, centre, top + 36.0, 17.0, h);
        put(
            &day.high,
            (left, top + 50.0, column, 15.0),
            &digits(12.5, Weight::Medium, style::TEXT, true),
            h,
        );
        put(
            &day.low,
            (left, top + 66.0, column, 14.0),
            &digits(11.5, Weight::Regular, style::TEXT_TERTIARY, true),
            h,
        );
    }
}

fn draw(bounds: NSRect, display: &WeatherDisplay) {
    let (w, h) = (bounds.size.width, bounds.size.height);
    let mut place_x = MARGIN + 2.0;
    if let Some(pager) = &display.pager {
        put(
            &format!("{}/{}", pager.position, pager.count),
            (MARGIN + FLIP_BUTTON, HEADER_MIDDLE - 7.0, FLIP_LABEL, 14.0),
            &digits(10.5, Weight::Medium, style::TEXT_SECONDARY, true),
            h,
        );
        place_x = MARGIN + FLIP_WIDTH;
    }
    // The line over the cards: the place at the left, how fresh it is and where it comes from at the right.
    put(
        &display.place,
        (place_x, 8.0, w * 0.5 - (place_x - MARGIN - 2.0), 20.0),
        &attrs(15.0, Weight::Semibold, style::TEXT, false),
        h,
    );
    if let Some((title, hint)) = &display.note {
        draw_page_note("cloud.sun.fill", title, hint, bounds.size);
        return;
    }
    let status = if display.credit.is_empty() {
        display.status.clone()
    } else if display.status.is_empty() {
        display.credit.clone()
    } else {
        format!("{}  ·  {}", display.status, display.credit)
    };
    let status_attrs = attrs(10.0, Weight::Regular, style::TEXT_TERTIARY, false);
    let right = w - MARGIN - 2.0;
    let left = (right - width_of(&status, &status_attrs).ceil() - 2.0).max(w * 0.45);
    put(&status, (left, 11.0, right - left, 14.0), &status_attrs, h);
    let cards_top = 36.0;
    let cards_h = h - cards_top - 8.0;
    if let Some(current) = &display.current {
        let card = NSRect::new(
            NSPoint::new(MARGIN, h - cards_top - cards_h),
            NSSize::new(NOW_WIDTH, cards_h),
        );
        draw_now(current, card, h);
    }
    let right_x = MARGIN + NOW_WIDTH + GAP;
    let right_w = w - right_x - MARGIN;
    let hours = NSRect::new(
        NSPoint::new(right_x, h - cards_top - HOURS_HEIGHT),
        NSSize::new(right_w, HOURS_HEIGHT),
    );
    draw_hours(&display.hours, hours, h);
    let days_h = cards_h - HOURS_HEIGHT - GAP;
    let days = NSRect::new(
        NSPoint::new(right_x, h - cards_top - cards_h),
        NSSize::new(right_w, days_h),
    );
    draw_days(&display.days, days, h);
}

pub struct CanvasIvars {
    display: RefCell<WeatherDisplay>,
}

define_class!(
    // SAFETY: NSView allows subclassing; the ivars are only touched on the main thread and need no Drop.
    #[unsafe(super(NSView))]
    #[thread_kind = MainThreadOnly]
    #[name = "FigWeatherCanvas"]
    #[ivars = CanvasIvars]
    pub struct WeatherCanvas;

    impl WeatherCanvas {
        #[unsafe(method(drawRect:))]
        fn draw_rect(&self, _dirty: NSRect) {
            draw(self.bounds(), &self.ivars().display.borrow());
        }
    }
);

impl WeatherCanvas {
    fn new(mtm: MainThreadMarker, frame: NSRect) -> Retained<Self> {
        let this = mtm.alloc::<Self>().set_ivars(CanvasIvars {
            display: RefCell::default(),
        });
        // SAFETY: designated initializer with valid arguments on the main thread.
        unsafe { msg_send![super(this), initWithFrame: frame] }
    }
}

/// What the controls of the tab do.
pub struct WeatherCallbacks {
    /// -1 for the previous city, 1 for the next.
    pub on_flip: Box<dyn Fn(isize)>,
    /// The button under the note.
    pub on_action: Box<dyn Fn()>,
}

pub struct WeatherView {
    canvas: Retained<WeatherCanvas>,
    previous: Retained<TouchButton>,
    next: Retained<TouchButton>,
    action: Retained<TouchButton>,
    callbacks: Rc<WeatherCallbacks>,
}

impl WeatherView {
    /// `frame` is `Notch::page_area(PAGE_SIZE.width, PAGE_SIZE.height)`.
    pub fn new(mtm: MainThreadMarker, frame: NSRect, callbacks: WeatherCallbacks) -> Rc<Self> {
        let canvas = WeatherCanvas::new(mtm, frame);
        let callbacks = Rc::new(callbacks);
        let h = frame.size.height;
        let round = |x: f64, symbol: &str, step: isize| {
            let button = TouchButton::new(
                mtm,
                NSRect::new(
                    NSPoint::new(x, h - HEADER_MIDDLE - FLIP_BUTTON / 2.0),
                    NSSize::new(FLIP_BUTTON, FLIP_BUTTON),
                ),
                Look::Round { filled: false },
            );
            button.set_symbol(symbol);
            let shared = Rc::clone(&callbacks);
            button.set_on_click(move || (shared.on_flip)(step));
            button.setHidden(true);
            canvas.addSubview(&button);
            button
        };
        let previous = round(MARGIN, "chevron.left", -1);
        let next = round(MARGIN + FLIP_BUTTON + FLIP_LABEL, "chevron.right", 1);
        let action = TouchButton::new(
            mtm,
            NSRect::new(NSPoint::ZERO, NSSize::new(10.0, NOTE_BUTTON_HEIGHT)),
            Look::Rect,
        );
        action.set_selected(true);
        let shared = Rc::clone(&callbacks);
        action.set_on_click(move || (shared.on_action)());
        action.setHidden(true);
        canvas.addSubview(&action);
        Rc::new(Self {
            canvas,
            previous,
            next,
            action,
            callbacks,
        })
    }

    pub fn show(&self, display: WeatherDisplay) {
        let pager = display.pager.as_ref();
        self.previous.setHidden(pager.is_none());
        self.next.setHidden(pager.is_none());
        if let Some(pager) = pager {
            self.previous.set_title(&pager.previous);
            self.next.set_title(&pager.next);
        }
        let label = display.action.as_ref().filter(|_| display.note.is_some());
        self.action.setHidden(label.is_none());
        if let Some(label) = label {
            self.action.set_title(label);
            let size = self.canvas.bounds().size;
            let w = (self.action.fit_width() + 24.0).max(140.0);
            // Under the hint of the note, which ends 182 points below the top.
            self.action.setFrame(NSRect::new(
                NSPoint::new(
                    (size.width - w) / 2.0,
                    size.height - 190.0 - NOTE_BUTTON_HEIGHT,
                ),
                NSSize::new(w, NOTE_BUTTON_HEIGHT),
            ));
        }
        *self.canvas.ivars().display.borrow_mut() = display;
        self.canvas.setNeedsDisplay(true);
    }

    /// Left and Right flip through the cities when there are several. Returns true when the key was consumed.
    pub fn handle_key(&self, key: Key) -> bool {
        if self.canvas.ivars().display.borrow().pager.is_none() {
            return false;
        }
        match key {
            Key::Left => (self.callbacks.on_flip)(-1),
            Key::Right => (self.callbacks.on_flip)(1),
            _ => return false,
        }
        true
    }

    /// The notch tab for this view.
    pub fn page(
        self: &Rc<Self>,
        title: String,
        on_show: impl Fn() + 'static,
        on_hide: impl Fn() + 'static,
    ) -> Page {
        let keys = Rc::clone(self);
        Page {
            id: "weather",
            title,
            symbol: "cloud.sun.fill",
            root: Retained::into_super(Retained::clone(&self.canvas)),
            on_show: Box::new(on_show),
            on_hide: Box::new(on_hide),
            on_key: Box::new(move |key| keys.handle_key(key)),
            accepts_files: false,
            size: None,
        }
    }
}
