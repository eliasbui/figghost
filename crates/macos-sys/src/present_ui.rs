//! The Present tab of the notch: what to present (a display, an area, a window or an app) and where (another display,
//! or a window of its own), and a button that starts. While a presentation runs, it shows how long it has run, what
//! is shown where, a button that stops, the switches of the automatic zoom, the spotlight, the ring around the
//! pointer, the frozen picture and the black or white screen, the zoom by steps, and the keys for all of it.
//! Everything on it is plain data from the app; nothing here keeps time or knows a language.

use crate::{
    button::{Look, TouchButton},
    notch::Page,
    style::{self, Tone, Weight, white},
    widgets::{Fit, draw_page_note, draw_text, text_attributes},
};
use objc2::{DefinedClass, MainThreadMarker, MainThreadOnly, define_class, msg_send, rc::Retained};
use objc2_app_kit::NSView;
use objc2_foundation::{NSPoint, NSRect, NSSize};
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
};

/// The size of the area the tab fills below the header.
pub const PAGE_SIZE: NSSize = NSSize::new(580.0, 232.0);

const MARGIN: f64 = 18.0;
const CHIP: f64 = 28.0;
const CHIP_GAP: f64 = 6.0;
const BUTTON: NSSize = NSSize::new(176.0, 36.0);
const STOP: NSSize = NSSize::new(104.0, 32.0);

/// A choice on the tab: its words and its SF Symbol.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Choice {
    pub title: String,
    pub symbol: &'static str,
}

/// While a presentation runs.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Live {
    /// "Presenting", or what holds the picture: frozen, black, white.
    pub title: String,
    /// Green while the audience sees the picture live, orange while it does not.
    pub tone: Option<Tone>,
    pub clock: String,
    /// What is shown, and where.
    pub what: String,
    pub stop: String,
    /// The switches, each on or off, in two rows: the zoom and the pointer, then what covers the picture.
    pub toggles: Vec<(Choice, bool)>,
    /// How many of the switches go on the first row.
    pub first_row: usize,
    /// The zoom now, as "2x", and the names of the buttons that step it out and in.
    pub zoom: String,
    pub zoom_names: [String; 2],
    /// The keys, in a line at the bottom.
    pub keys: String,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct PresentDisplay {
    /// Words that replace everything: the tab is off.
    pub note: Option<(String, String)>,
    /// What can be presented, the one chosen, and the words over them.
    pub sources: Vec<Choice>,
    pub source: usize,
    pub source_label: String,
    /// Where it can be presented, the one chosen, and the words over them.
    pub stages: Vec<Choice>,
    pub stage: Option<usize>,
    pub stage_label: String,
    pub start: String,
    /// Beside the button: how it goes, or why it cannot start yet.
    pub hint: String,
    pub live: Option<Live>,
}

pub struct PresentCallbacks {
    pub on_source: Box<dyn Fn(usize)>,
    pub on_stage: Box<dyn Fn(usize)>,
    pub on_start: Box<dyn Fn()>,
    pub on_stop: Box<dyn Fn()>,
    /// A switch, by its place in `Live::toggles`.
    pub on_toggle: Box<dyn Fn(usize)>,
    /// A step of the zoom: in, or out.
    pub on_zoom: Box<dyn Fn(bool)>,
}

pub struct CanvasIvars {
    display: RefCell<PresentDisplay>,
}

define_class!(
    // SAFETY: NSView allows subclassing; the ivars are only touched on the main thread and need no Drop.
    #[unsafe(super(NSView))]
    #[thread_kind = MainThreadOnly]
    #[name = "FigPresentCanvas"]
    #[ivars = CanvasIvars]
    struct PresentCanvas;

    impl PresentCanvas {
        #[unsafe(method(drawRect:))]
        fn draw_rect(&self, _dirty: NSRect) {
            draw(self.bounds().size, &self.ivars().display.borrow());
        }
    }
);

impl PresentCanvas {
    fn new(mtm: MainThreadMarker, frame: NSRect) -> Retained<Self> {
        let this = mtm.alloc::<Self>().set_ivars(CanvasIvars {
            display: RefCell::default(),
        });
        // SAFETY: designated initializer with valid arguments on the main thread.
        unsafe { msg_send![super(this), initWithFrame: frame] }
    }
}

/// A line of text with its top `top` points below the top of a page `height` tall.
fn line(
    text: &str,
    (x, top, width, tall): (f64, f64, f64, f64),
    size: f64,
    weight: Weight,
    alpha: f64,
    fit: Fit,
    height: f64,
) {
    draw_text(
        text,
        NSRect::new(
            NSPoint::new(x, height - top - tall),
            NSSize::new(width, tall),
        ),
        &text_attributes(&style::font(size, weight), &white(alpha), fit, false),
    );
}

fn draw(size: NSSize, display: &PresentDisplay) {
    if let Some((title, hint)) = &display.note {
        draw_page_note("rectangle.on.rectangle", title, hint, size);
        return;
    }
    let (w, h) = (size.width, size.height);
    match &display.live {
        Some(live) => {
            let tone = live.tone.unwrap_or(Tone::Green);
            style::fill_circle(
                NSRect::new(
                    NSPoint::new(MARGIN, h - 24.0 - 5.0),
                    NSSize::new(10.0, 10.0),
                ),
                &tone.color(),
            );
            line(
                &live.title,
                (MARGIN + 18.0, 14.0, 250.0, 20.0),
                15.0,
                Weight::Semibold,
                style::TEXT,
                Fit::Tail,
                h,
            );
            draw_text(
                &live.clock,
                NSRect::new(
                    NSPoint::new(w - MARGIN - STOP.width - 12.0 - 90.0, h - 14.0 - 20.0),
                    NSSize::new(90.0, 20.0),
                ),
                &text_attributes(
                    &style::digits(15.0, Weight::Semibold),
                    &white(style::TEXT),
                    Fit::Clip,
                    false,
                ),
            );
            line(
                &live.what,
                (
                    MARGIN + 18.0,
                    38.0,
                    w - 2.0 * MARGIN - STOP.width - 30.0,
                    16.0,
                ),
                11.5,
                Weight::Regular,
                style::TEXT_SECONDARY,
                Fit::Tail,
                h,
            );
            line(
                &live.zoom,
                (w - MARGIN - 28.0 - 6.0 - 46.0, 76.0, 46.0, 16.0),
                12.0,
                Weight::Semibold,
                style::TEXT,
                Fit::Clip,
                h,
            );
            line(
                &live.keys,
                (MARGIN, 150.0, w - 2.0 * MARGIN, 64.0),
                10.5,
                Weight::Regular,
                style::TEXT_TERTIARY,
                Fit::Words,
                h,
            );
        }
        None => {
            line(
                &display.source_label,
                (MARGIN, 14.0, w - 2.0 * MARGIN, 14.0),
                11.0,
                Weight::Medium,
                style::TEXT_TERTIARY,
                Fit::Tail,
                h,
            );
            line(
                &display.stage_label,
                (MARGIN, 76.0, w - 2.0 * MARGIN, 14.0),
                11.0,
                Weight::Medium,
                style::TEXT_TERTIARY,
                Fit::Tail,
                h,
            );
            line(
                &display.hint,
                (MARGIN, 140.0, w - 3.0 * MARGIN - BUTTON.width, 78.0),
                11.0,
                Weight::Regular,
                style::TEXT_SECONDARY,
                Fit::Words,
                h,
            );
        }
    }
}

type OnPick = Rc<dyn Fn(usize)>;
/// Choices to show, with which of them are lit; `None` hides the row.
type Row<'a> = Option<(&'a [Choice], &'a dyn Fn(usize) -> bool)>;

/// A row of chips, made again only when the number of choices changes.
struct Chips {
    buttons: RefCell<Vec<Retained<TouchButton>>>,
    on_pick: OnPick,
}

impl Chips {
    fn new(on_pick: OnPick) -> Self {
        Self {
            buttons: RefCell::default(),
            on_pick,
        }
    }

    /// Shows `items` from the left at `top` points below the top of the page, the chosen ones lit; `None` hides them.
    fn show(&self, mtm: MainThreadMarker, canvas: &NSView, items: Row, top: f64) {
        let mut buttons = self.buttons.borrow_mut();
        let Some((items, chosen)) = items else {
            for button in buttons.iter() {
                button.setHidden(true);
            }
            return;
        };
        while buttons.len() > items.len() {
            if let Some(button) = buttons.pop() {
                button.removeFromSuperview();
            }
        }
        while buttons.len() < items.len() {
            let index = buttons.len();
            let button = TouchButton::new(mtm, NSRect::ZERO, Look::Chip);
            let on_pick = Rc::clone(&self.on_pick);
            button.set_on_click(move || on_pick(index));
            canvas.addSubview(&button);
            buttons.push(button);
        }
        let height = canvas.frame().size.height;
        let mut left = MARGIN;
        for (index, (button, item)) in buttons.iter().zip(items).enumerate() {
            button.set_symbol(item.symbol);
            button.set_title(&item.title);
            button.set_selected(chosen(index));
            button.setHidden(false);
            let width = button.fit_width();
            button.setFrame(NSRect::new(
                NSPoint::new(left, height - top - CHIP),
                NSSize::new(width, CHIP),
            ));
            left += width + CHIP_GAP;
        }
    }
}

pub struct PresentView {
    mtm: MainThreadMarker,
    canvas: Retained<PresentCanvas>,
    sources: Chips,
    stages: Chips,
    first: Chips,
    second: Chips,
    start: Retained<TouchButton>,
    stop: Retained<TouchButton>,
    zoom: [Retained<TouchButton>; 2],
    /// How many switches the first row has, for the second row's buttons to number on from it.
    split: Rc<Cell<usize>>,
}

impl PresentView {
    /// `frame` is `Notch::page_area(PAGE_SIZE.width, PAGE_SIZE.height)`.
    pub fn new(mtm: MainThreadMarker, frame: NSRect, callbacks: PresentCallbacks) -> Rc<Self> {
        let canvas = PresentCanvas::new(mtm, frame);
        let callbacks = Rc::new(callbacks);
        let button = |look: Look, symbol: &str| {
            let button = TouchButton::new(mtm, NSRect::ZERO, look);
            button.set_symbol(symbol);
            button.setHidden(true);
            canvas.addSubview(&button);
            button
        };
        let start = button(Look::Rect, "play.fill");
        start.set_selected(true);
        let stop = button(Look::Rect, "stop.fill");
        stop.set_tone(Some(Tone::Red));
        let zoom = [
            button(Look::Round { filled: false }, "minus"),
            button(Look::Round { filled: false }, "plus"),
        ];
        let pick = |f: fn(&PresentCallbacks, usize)| -> OnPick {
            let callbacks = Rc::clone(&callbacks);
            Rc::new(move |index| f(&callbacks, index))
        };
        let toggles = pick(|c, i| (c.on_toggle)(i));
        let split = Rc::new(Cell::new(0usize));
        let (counted, on_second) = (Rc::clone(&split), Rc::clone(&toggles));
        let view = Rc::new(Self {
            mtm,
            sources: Chips::new(pick(|c, i| (c.on_source)(i))),
            stages: Chips::new(pick(|c, i| (c.on_stage)(i))),
            first: Chips::new(Rc::clone(&toggles)),
            second: Chips::new(Rc::new(move |i| on_second(counted.get() + i))),
            start,
            stop,
            zoom,
            split,
            canvas,
        });
        let c = Rc::clone(&callbacks);
        view.start.set_on_click(move || (c.on_start)());
        let c = Rc::clone(&callbacks);
        view.stop.set_on_click(move || (c.on_stop)());
        for (i, button) in view.zoom.iter().enumerate() {
            let c = Rc::clone(&callbacks);
            button.set_on_click(move || (c.on_zoom)(i == 1));
        }
        view
    }

    pub fn show(&self, display: PresentDisplay) {
        let canvas: &NSView = &self.canvas;
        let (w, h) = (PAGE_SIZE.width, self.canvas.frame().size.height);
        let off = display.note.is_some();
        let live = display.live.as_ref().filter(|_| !off);
        let idle = !off && live.is_none();
        let source = display.source;
        self.sources.show(
            self.mtm,
            canvas,
            idle.then_some((&display.sources[..], &|i| i == source)),
            32.0,
        );
        let stage = display.stage;
        self.stages.show(
            self.mtm,
            canvas,
            idle.then_some((&display.stages[..], &|i| Some(i) == stage)),
            94.0,
        );
        self.start.setHidden(!idle);
        self.start.set_title(&display.start);
        self.start.set_enabled(display.stage.is_some());
        self.start.setFrame(NSRect::new(
            NSPoint::new(w - MARGIN - BUTTON.width, 18.0),
            BUTTON,
        ));
        self.stop.setHidden(live.is_none());
        for button in &self.zoom {
            button.setHidden(live.is_none());
        }
        match live {
            Some(live) => {
                let split = live.first_row.min(live.toggles.len());
                self.split.set(split);
                let (first, second): (Vec<Choice>, Vec<Choice>) = (
                    live.toggles[..split]
                        .iter()
                        .map(|(c, _)| c.clone())
                        .collect(),
                    live.toggles[split..]
                        .iter()
                        .map(|(c, _)| c.clone())
                        .collect(),
                );
                let on: Vec<bool> = live.toggles.iter().map(|(_, on)| *on).collect();
                self.first
                    .show(self.mtm, canvas, Some((&first, &|i| on[i])), 70.0);
                self.second
                    .show(self.mtm, canvas, Some((&second, &|i| on[split + i])), 106.0);
                self.stop.set_title(&live.stop);
                self.stop.setFrame(NSRect::new(
                    NSPoint::new(w - MARGIN - STOP.width, h - 14.0 - STOP.height),
                    STOP,
                ));
                for (i, button) in self.zoom.iter().enumerate() {
                    button.set_title(&live.zoom_names[i]);
                    let x = if i == 0 {
                        w - MARGIN - 28.0 - 6.0 - 46.0 - 6.0 - 28.0
                    } else {
                        w - MARGIN - 28.0
                    };
                    button.setFrame(NSRect::new(
                        NSPoint::new(x, h - 70.0 - CHIP),
                        NSSize::new(28.0, 28.0),
                    ));
                }
            }
            None => {
                self.first.show(self.mtm, canvas, None, 0.0);
                self.second.show(self.mtm, canvas, None, 0.0);
            }
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
            id: "present",
            title,
            symbol: "rectangle.on.rectangle",
            root: Retained::into_super(Retained::clone(&self.canvas)),
            on_show: Box::new(on_show),
            on_hide: Box::new(on_hide),
            on_key: Box::new(|_| false),
            accepts_files: false,
            size: None,
        }
    }

    /// Presses a button of the tab as a person would, for the checks: "start", "stop", "source N", "stage N",
    /// "toggle N", "zoom in", "zoom out". Returns whether there was such a button to press.
    pub fn press(&self, what: &str) -> bool {
        let (kind, n) = what.split_once(' ').unwrap_or((what, ""));
        let n: usize = n.parse().unwrap_or(0);
        let pick = |chips: &Chips, n: usize| {
            chips
                .buttons
                .borrow()
                .get(n)
                .filter(|b| !b.isHidden())
                .cloned()
        };
        let button = match kind {
            "start" => Some(self.start.clone()).filter(|b| !b.isHidden()),
            "stop" => Some(self.stop.clone()).filter(|b| !b.isHidden()),
            "source" => pick(&self.sources, n),
            "stage" => pick(&self.stages, n),
            "toggle" if n < self.split.get() => pick(&self.first, n),
            "toggle" => pick(&self.second, n - self.split.get()),
            "zoom" => Some(self.zoom[usize::from(what.ends_with(" in"))].clone())
                .filter(|b| !b.isHidden()),
            _ => None,
        };
        // SAFETY: accessibilityPerformPress takes no arguments and returns whether it pressed.
        button.is_some_and(|b| unsafe { msg_send![&*b, accessibilityPerformPress] })
    }

    /// The page drawn into a PNG, buttons included, for the pictures of the documentation and the checks.
    pub fn snapshot_png(&self) -> Option<Vec<u8>> {
        crate::widgets::view_png(&self.canvas)
    }
}
