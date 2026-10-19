//! The calendar page of the notch: a meeting that is about to start with its Join button, today's and
//! tomorrow's events with the colour of their calendar, reminders that are due with a button to complete
//! them, and a note when there is nothing to show. Everything on it is plain data from the app.

use crate::{
    button::{Look, TouchButton},
    notch::Page,
    panel::Key,
    style::{self, Tone, Weight, white},
    widgets::{Fit, PageRoot, draw_text, text_attributes},
};
use objc2::{DefinedClass, MainThreadMarker, MainThreadOnly, define_class, msg_send, rc::Retained};
use objc2_app_kit::{NSColor, NSFont, NSStringDrawing, NSView};
use objc2_foundation::{NSObjectProtocol, NSPoint, NSRect, NSSize, NSString};
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
};

const MARGIN: f64 = 18.0;
const FOOTER: f64 = 24.0;
const TOP: f64 = 6.0;
const GAP: f64 = 8.0;
const HEADING_HEIGHT: f64 = 24.0;
const EVENT_HEIGHT: f64 = 42.0;
const REMINDER_HEIGHT: f64 = 34.0;
const ALERT_HEIGHT: f64 = 122.0;
const MORE_HEIGHT: f64 = 18.0;
const BUTTON_HEIGHT: f64 = 32.0;
const CARD_PAD: f64 = 14.0;
const TIME_WIDTH: f64 = 68.0;

/// A meeting that is about to start.
#[derive(Debug, Clone)]
pub struct AlertData {
    pub key: String,
    /// "Starting soon", drawn in capitals.
    pub heading: String,
    /// "in 5 min" or "now".
    pub when: String,
    pub title: String,
    /// The time and place.
    pub detail: String,
    /// "Join Zoom"; `None` when the event has no meeting link.
    pub join: Option<String>,
    pub dismiss: String,
}

#[derive(Debug, Clone)]
pub struct EventData {
    pub key: String,
    /// 0xRRGGBB.
    pub color: u32,
    pub time: String,
    /// The end time under the start time; empty for an all-day event.
    pub until: String,
    pub title: String,
    pub detail: String,
    /// "in 5 min", "now".
    pub tag: String,
    pub tag_tone: Option<Tone>,
    pub join: Option<String>,
}

#[derive(Debug, Clone)]
pub struct DayData {
    pub heading: String,
    pub date: String,
    pub events: Vec<EventData>,
}

#[derive(Debug, Clone)]
pub struct ReminderData {
    pub key: String,
    pub color: u32,
    pub title: String,
    pub when: String,
    pub overdue: bool,
    pub done: String,
}

#[derive(Debug, Clone)]
pub struct NoteData {
    pub symbol: &'static str,
    pub title: String,
    pub hint: String,
    /// A button under the note, such as "Allow access".
    pub action: Option<String>,
}

#[derive(Debug, Clone, Default)]
pub struct CalendarDisplay {
    pub alerts: Vec<AlertData>,
    /// "2 more alerts", or empty.
    pub more_alerts: String,
    pub days: Vec<DayData>,
    pub reminders_heading: String,
    pub reminders: Vec<ReminderData>,
    /// "+{n} more", where `{n}` stands for how many rows did not fit.
    pub more: String,
    /// Replaces everything else when there is nothing to list.
    pub note: Option<NoteData>,
}

pub struct CalendarCallbacks {
    pub on_join: Box<dyn Fn(&str)>,
    pub on_dismiss: Box<dyn Fn(&str)>,
    pub on_complete: Box<dyn Fn(&str)>,
    /// The button of the note.
    pub on_action: Box<dyn Fn()>,
}

fn srgb(rgb: u32, alpha: f64) -> Retained<NSColor> {
    let part = |shift: u32| f64::from((rgb >> shift) & 0xff) / 255.0;
    NSColor::colorWithSRGBRed_green_blue_alpha(part(16), part(8), part(0), alpha)
}

fn attrs(
    size: f64,
    weight: Weight,
    color: &NSColor,
    fit: Fit,
) -> Retained<objc2_foundation::NSMutableDictionary<NSString, objc2::runtime::AnyObject>> {
    text_attributes(&style::font(size, weight), color, fit, false)
}

fn text_width(text: &str, font: &NSFont) -> f64 {
    let attrs = text_attributes(font, &white(1.0), Fit::Clip, false);
    // SAFETY: the attributes dictionary holds valid attribute objects.
    unsafe { NSString::from_str(text).sizeWithAttributes(Some(&attrs)) }
        .width
        .ceil()
}

fn rect(x: f64, y: f64, w: f64, h: f64) -> NSRect {
    NSRect::new(NSPoint::new(x, y), NSSize::new(w, h))
}

#[derive(Debug)]
enum Item {
    Heading {
        y: f64,
        text: String,
        date: String,
    },
    Alert {
        y: f64,
        data: AlertData,
    },
    Event {
        y: f64,
        data: EventData,
        join_width: f64,
    },
    Reminder {
        y: f64,
        data: ReminderData,
    },
    Line {
        y: f64,
        text: String,
    },
    Note(NoteData),
}

pub struct BodyIvars {
    items: RefCell<Vec<Item>>,
}

define_class!(
    // SAFETY: NSView allows subclassing; the ivars are only touched on the main thread and need no Drop.
    #[unsafe(super(NSView))]
    #[thread_kind = MainThreadOnly]
    #[name = "FigCalendarBody"]
    #[ivars = BodyIvars]
    struct Body;

    impl Body {
        #[unsafe(method(isFlipped))]
        fn is_flipped(&self) -> bool {
            true
        }

        #[unsafe(method(drawRect:))]
        fn draw_rect(&self, _dirty: NSRect) {
            let bounds = self.bounds();
            for item in self.ivars().items.borrow().iter() {
                draw_item(item, bounds.size.width, bounds.size.height);
            }
        }
    }

    unsafe impl NSObjectProtocol for Body {}
);

fn draw_item(item: &Item, width: f64, height: f64) {
    match item {
        Item::Heading { y, text, date } => {
            let font = style::font(10.5, Weight::Semibold);
            let label = text.to_uppercase();
            let label_width = text_width(&label, &font);
            draw_text(
                &label,
                rect(2.0, y + 6.0, label_width + 2.0, 14.0),
                &attrs(
                    10.5,
                    Weight::Semibold,
                    &white(style::TEXT_TERTIARY),
                    Fit::Clip,
                ),
            );
            draw_text(
                date,
                rect(
                    label_width + 12.0,
                    y + 6.0,
                    width - label_width - 14.0,
                    14.0,
                ),
                &attrs(
                    10.5,
                    Weight::Regular,
                    &white(style::TEXT_TERTIARY),
                    Fit::Tail,
                ),
            );
        }
        Item::Alert { y, data } => {
            let card = rect(0.0, *y, width, ALERT_HEIGHT);
            style::card(card, style::CARD_RADIUS);
            let edge = Tone::Orange.color().colorWithAlphaComponent(0.5);
            style::stroke_rounded(card, style::CARD_RADIUS, &edge, 1.2);
            let inner = width - 2.0 * CARD_PAD;
            let when_font = style::font(11.0, Weight::Semibold);
            let when_width = text_width(&data.when, &when_font);
            draw_text(
                &data.heading.to_uppercase(),
                rect(CARD_PAD, y + 14.0, inner - when_width - 8.0, 14.0),
                &attrs(10.5, Weight::Semibold, &Tone::Orange.color(), Fit::Tail),
            );
            draw_text(
                &data.when,
                rect(
                    width - CARD_PAD - when_width,
                    y + 14.0,
                    when_width + 2.0,
                    14.0,
                ),
                &attrs(11.0, Weight::Semibold, &Tone::Orange.color(), Fit::Clip),
            );
            draw_text(
                &data.title,
                rect(CARD_PAD, y + 31.0, inner, 20.0),
                &attrs(15.0, Weight::Semibold, &white(style::TEXT), Fit::Tail),
            );
            draw_text(
                &data.detail,
                rect(CARD_PAD, y + 53.0, inner, 15.0),
                &attrs(
                    11.5,
                    Weight::Regular,
                    &white(style::TEXT_SECONDARY),
                    Fit::Tail,
                ),
            );
        }
        Item::Event {
            y,
            data,
            join_width,
        } => {
            let row = rect(0.0, y + 2.0, width, EVENT_HEIGHT - 4.0);
            style::card(row, 12.0);
            style::fill_capsule(
                rect(12.0, y + 9.0, 4.0, EVENT_HEIGHT - 18.0),
                &srgb(data.color, 1.0),
            );
            let timed = !data.until.is_empty();
            let time_y = if timed { y + 7.0 } else { y + 13.0 };
            draw_text(
                &data.time,
                rect(24.0, time_y, TIME_WIDTH, 16.0),
                &text_attributes(
                    &style::digits(12.5, Weight::Semibold),
                    &white(style::TEXT),
                    Fit::Clip,
                    false,
                ),
            );
            if timed {
                draw_text(
                    &data.until,
                    rect(24.0, y + 23.0, TIME_WIDTH, 13.0),
                    &text_attributes(
                        &style::digits(10.5, Weight::Regular),
                        &white(style::TEXT_TERTIARY),
                        Fit::Clip,
                        false,
                    ),
                );
            }
            let right = width
                - 10.0
                - if *join_width > 0.0 {
                    join_width + 8.0
                } else {
                    0.0
                };
            let tag_font = style::font(11.0, Weight::Semibold);
            let tag_width = if data.tag.is_empty() {
                0.0
            } else {
                text_width(&data.tag, &tag_font) + 2.0
            };
            if tag_width > 0.0 {
                let color = data
                    .tag_tone
                    .map_or_else(|| white(style::TEXT_SECONDARY), Tone::color);
                draw_text(
                    &data.tag,
                    rect(right - tag_width, y + 13.0, tag_width, 15.0),
                    &text_attributes(&tag_font, &color, Fit::Clip, false),
                );
            }
            let text_x = 24.0 + TIME_WIDTH + 4.0;
            let text_width = (right - tag_width - 10.0 - text_x).max(40.0);
            let title_y = if data.detail.is_empty() {
                y + 12.0
            } else {
                y + 6.0
            };
            draw_text(
                &data.title,
                rect(text_x, title_y, text_width, 17.0),
                &attrs(13.0, Weight::Medium, &white(style::TEXT), Fit::Tail),
            );
            if !data.detail.is_empty() {
                draw_text(
                    &data.detail,
                    rect(text_x, y + 23.0, text_width, 14.0),
                    &attrs(
                        11.0,
                        Weight::Regular,
                        &white(style::TEXT_SECONDARY),
                        Fit::Tail,
                    ),
                );
            }
        }
        Item::Reminder { y, data } => {
            style::card(rect(0.0, y + 2.0, width, REMINDER_HEIGHT - 4.0), 10.0);
            style::fill_circle(rect(10.0, y + 12.0, 10.0, 10.0), &srgb(data.color, 1.0));
            let font = style::font(11.5, Weight::Regular);
            let when_width = text_width(&data.when, &font) + 2.0;
            let tone = if data.overdue {
                Tone::Red.color()
            } else {
                white(style::TEXT_SECONDARY)
            };
            // The circle button sits at the right edge of the row, so the date stays clear of it.
            let when_x = width - 44.0 - when_width;
            draw_text(
                &data.when,
                rect(when_x, y + 10.0, when_width, 15.0),
                &text_attributes(&font, &tone, Fit::Clip, false),
            );
            draw_text(
                &data.title,
                rect(30.0, y + 9.0, (when_x - 40.0).max(40.0), 17.0),
                &attrs(13.0, Weight::Regular, &white(style::TEXT), Fit::Tail),
            );
        }
        Item::Line { y, text } => {
            draw_text(
                text,
                rect(2.0, *y, width - 4.0, MORE_HEIGHT),
                &attrs(
                    11.0,
                    Weight::Regular,
                    &white(style::TEXT_TERTIARY),
                    Fit::Tail,
                ),
            );
        }
        Item::Note(note) => {
            let middle = height / 2.0 - if note.action.is_some() { 16.0 } else { 0.0 };
            style::draw_symbol(
                note.symbol,
                NSPoint::new(width / 2.0, middle - 40.0),
                34.0,
                &white(0.34),
            );
            draw_text(
                &note.title,
                rect(20.0, middle - 8.0, width - 40.0, 20.0),
                &text_attributes(
                    &style::font(14.0, Weight::Semibold),
                    &white(0.8),
                    Fit::Clip,
                    true,
                ),
            );
            draw_text(
                &note.hint,
                rect(40.0, middle + 16.0, width - 80.0, 48.0),
                &text_attributes(
                    &style::font(12.0, Weight::Regular),
                    &white(style::TEXT_SECONDARY),
                    Fit::Wrap,
                    true,
                ),
            );
        }
    }
}

pub struct CalendarView {
    pub root: Retained<PageRoot>,
    frame: NSRect,
    body: RefCell<Option<Retained<Body>>>,
    first_alert: RefCell<Option<(String, bool)>>,
    shown: Cell<bool>,
    callbacks: Rc<CalendarCallbacks>,
}

/// A row to lay out: how tall it is and whether it is the heading that must stay with the row after it.
struct Unit {
    height: f64,
    heading: bool,
}

impl CalendarView {
    pub fn new(
        mtm: MainThreadMarker,
        frame: NSRect,
        hint: &str,
        callbacks: CalendarCallbacks,
    ) -> Rc<Self> {
        Rc::new(Self {
            root: PageRoot::new(mtm, frame, hint),
            frame,
            body: RefCell::new(None),
            first_alert: RefCell::new(None),
            shown: Cell::new(false),
            callbacks: Rc::new(callbacks),
        })
    }

    /// Replaces what the page shows.
    pub fn show(&self, mtm: MainThreadMarker, display: &CalendarDisplay) {
        if let Some(old) = self.body.borrow_mut().take() {
            old.removeFromSuperview();
        }
        self.shown.set(true);
        let width = self.frame.size.width - 2.0 * MARGIN;
        let height = self.frame.size.height - FOOTER - TOP;
        let body = {
            let this = mtm.alloc::<Body>().set_ivars(BodyIvars {
                items: RefCell::new(Vec::new()),
            });
            // SAFETY: designated initializer with valid arguments on the main thread.
            let body: Retained<Body> = unsafe {
                msg_send![super(this), initWithFrame: rect(MARGIN, FOOTER, width, height)]
            };
            body
        };
        let mut items = Vec::new();
        *self.first_alert.borrow_mut() = display
            .alerts
            .first()
            .map(|a| (a.key.clone(), a.join.is_some()));

        if let Some(note) = &display.note {
            self.lay_note(mtm, &body, note, width, height, &mut items);
        } else {
            self.lay_list(mtm, &body, display, width, height, &mut items);
        }
        *body.ivars().items.borrow_mut() = items;
        self.root.addSubview(&body);
        *self.body.borrow_mut() = Some(body);
    }

    fn lay_note(
        &self,
        mtm: MainThreadMarker,
        body: &Body,
        note: &NoteData,
        width: f64,
        height: f64,
        items: &mut Vec<Item>,
    ) {
        if let Some(label) = &note.action {
            let button = TouchButton::new(mtm, rect(0.0, 0.0, 10.0, BUTTON_HEIGHT), Look::Rect);
            button.set_title(label);
            button.set_selected(true);
            let w = (button.fit_width() + 24.0).max(140.0);
            let middle = height / 2.0 - 16.0;
            button.setFrame(rect((width - w) / 2.0, middle + 70.0, w, BUTTON_HEIGHT));
            let shared = Rc::clone(&self.callbacks);
            button.set_on_click(move || (shared.on_action)());
            body.addSubview(&button);
        }
        items.push(Item::Note(note.clone()));
    }

    fn lay_list(
        &self,
        mtm: MainThreadMarker,
        body: &Body,
        display: &CalendarDisplay,
        width: f64,
        height: f64,
        items: &mut Vec<Item>,
    ) {
        let mut y = 0.0;
        if let Some(alert) = display.alerts.first() {
            items.push(Item::Alert {
                y,
                data: alert.clone(),
            });
            let top = y + ALERT_HEIGHT - CARD_PAD - BUTTON_HEIGHT;
            let inner = width - 2.0 * CARD_PAD;
            let half = (inner - GAP) / 2.0;
            let dismiss_width = if alert.join.is_some() { half } else { inner };
            let dismiss = TouchButton::new(
                mtm,
                rect(CARD_PAD, top, dismiss_width, BUTTON_HEIGHT),
                Look::Rect,
            );
            dismiss.set_symbol("xmark");
            dismiss.set_title(&alert.dismiss);
            dismiss.set_selected(alert.join.is_none());
            let (shared, key) = (Rc::clone(&self.callbacks), alert.key.clone());
            dismiss.set_on_click(move || (shared.on_dismiss)(&key));
            body.addSubview(&dismiss);
            if let Some(label) = &alert.join {
                let join = TouchButton::new(
                    mtm,
                    rect(CARD_PAD + half + GAP, top, half, BUTTON_HEIGHT),
                    Look::Rect,
                );
                join.set_symbol("video.fill");
                join.set_title(label);
                join.set_selected(true);
                let (shared, key) = (Rc::clone(&self.callbacks), alert.key.clone());
                join.set_on_click(move || (shared.on_join)(&key));
                body.addSubview(&join);
            }
            y += ALERT_HEIGHT + GAP;
            if !display.more_alerts.is_empty() {
                items.push(Item::Line {
                    y: y - 4.0,
                    text: display.more_alerts.clone(),
                });
                y += MORE_HEIGHT;
            }
        }

        let mut units = Vec::new();
        for day in &display.days {
            units.push(Unit {
                height: HEADING_HEIGHT,
                heading: true,
            });
            units.extend(day.events.iter().map(|_| Unit {
                height: EVENT_HEIGHT,
                heading: false,
            }));
        }
        if !display.reminders.is_empty() {
            units.push(Unit {
                height: HEADING_HEIGHT,
                heading: true,
            });
            units.extend(display.reminders.iter().map(|_| Unit {
                height: REMINDER_HEIGHT,
                heading: false,
            }));
        }
        let total: f64 = units.iter().map(|u| u.height).sum();
        let limit = if y + total <= height {
            height
        } else {
            height - MORE_HEIGHT
        };
        // How many units fit; a heading that would end up alone at the bottom does not count.
        let mut fit = 0;
        let mut used = y;
        for (i, unit) in units.iter().enumerate() {
            let next = units
                .get(i + 1)
                .map_or(0.0, |u| if unit.heading { u.height } else { 0.0 });
            if used + unit.height + next > limit {
                break;
            }
            used += unit.height;
            fit = i + 1;
        }
        let mut index = 0;
        let mut next = || {
            index += 1;
            index <= fit
        };
        for day in &display.days {
            let heading_shown = next();
            if heading_shown {
                items.push(Item::Heading {
                    y,
                    text: day.heading.clone(),
                    date: day.date.clone(),
                });
                y += HEADING_HEIGHT;
            }
            for event in &day.events {
                if !next() {
                    continue;
                }
                let join_width = self.add_join(mtm, body, event, y, width);
                items.push(Item::Event {
                    y,
                    data: event.clone(),
                    join_width,
                });
                y += EVENT_HEIGHT;
            }
        }
        if !display.reminders.is_empty() && next() {
            items.push(Item::Heading {
                y,
                text: display.reminders_heading.clone(),
                date: String::new(),
            });
            y += HEADING_HEIGHT;
            for reminder in &display.reminders {
                if !next() {
                    continue;
                }
                self.add_check(mtm, body, reminder, y, width);
                items.push(Item::Reminder {
                    y,
                    data: reminder.clone(),
                });
                y += REMINDER_HEIGHT;
            }
        }
        let rows_hidden = units.iter().skip(fit).filter(|u| !u.heading).count();
        if rows_hidden > 0 {
            items.push(Item::Line {
                y: y + 2.0,
                text: display.more.replace("{n}", &rows_hidden.to_string()),
            });
        }
    }

    /// The Join button of an event row, and its width (0 when the event has no link).
    fn add_join(
        &self,
        mtm: MainThreadMarker,
        body: &Body,
        event: &EventData,
        y: f64,
        width: f64,
    ) -> f64 {
        let Some(label) = &event.join else {
            return 0.0;
        };
        let button = TouchButton::new(mtm, rect(0.0, 0.0, 10.0, 26.0), Look::Rect);
        button.set_title(label);
        button.set_selected(true);
        let w = button.fit_width().max(56.0);
        button.setFrame(rect(
            width - 10.0 - w,
            y + (EVENT_HEIGHT - 26.0) / 2.0,
            w,
            26.0,
        ));
        let (shared, key) = (Rc::clone(&self.callbacks), event.key.clone());
        button.set_on_click(move || (shared.on_join)(&key));
        body.addSubview(&button);
        w
    }

    fn add_check(
        &self,
        mtm: MainThreadMarker,
        body: &Body,
        reminder: &ReminderData,
        y: f64,
        width: f64,
    ) {
        let button = TouchButton::new(
            mtm,
            rect(width - 36.0, y + (REMINDER_HEIGHT - 24.0) / 2.0, 24.0, 24.0),
            Look::Round { filled: false },
        );
        button.set_symbol("checkmark");
        button.set_title(&reminder.done);
        let (shared, key) = (Rc::clone(&self.callbacks), reminder.key.clone());
        button.set_on_click(move || (shared.on_complete)(&key));
        body.addSubview(&button);
    }

    /// Return joins the meeting in the alert; Option+Delete dismisses it.
    pub fn handle_key(&self, key: Key) -> bool {
        let first = self.first_alert.borrow().clone();
        let Some((id, joinable)) = first else {
            return false;
        };
        match key {
            Key::Enter if joinable => (self.callbacks.on_join)(&id),
            Key::Delete => (self.callbacks.on_dismiss)(&id),
            _ => return false,
        }
        true
    }

    /// The notch tab for this view.
    pub fn page(
        self: &Rc<Self>,
        title: &str,
        on_show: impl Fn() + 'static,
        on_hide: impl Fn() + 'static,
    ) -> Page {
        let keys = Rc::clone(self);
        Page {
            id: "calendar",
            title: title.to_owned(),
            symbol: "calendar",
            root: Retained::into_super(Retained::clone(&self.root)),
            on_show: Box::new(on_show),
            on_hide: Box::new(on_hide),
            on_key: Box::new(move |key| keys.handle_key(key)),
            accepts_files: false,
            size: None,
        }
    }
}
