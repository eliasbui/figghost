//! The Displays tab of the notch: a card for each display with a slider for its brightness and one for its contrast
//! when it answers over DDC/CI, and a menu of its colour profiles. With more than one display it has a second view,
//! the arrangement: the displays drawn where they are in the desktop, to be dragged into place as in System
//! Settings, the menu bar to be dragged onto the display that should be the main one, and a menu of what the chosen
//! display is used as. Everything on it is plain data from the app, which does the talking to the displays; nothing
//! here knows a language. A slider reports every move while it is dragged.

use crate::{
    button::{Look, TouchButton},
    ddc::Control,
    notch::Page,
    style::{self, Tone, Weight, white},
    widgets::{ActionTarget, Fit, draw_page_note, draw_text, text_attributes},
};
use objc2::{
    DefinedClass, MainThreadMarker, MainThreadOnly, define_class, msg_send, rc::Retained, sel,
};
use objc2_app_kit::{
    NSAccessibility, NSBezelStyle, NSControlSize, NSEvent, NSFont, NSLineBreakMode, NSPopUpButton,
    NSSlider, NSTextAlignment, NSTextField, NSView,
};
use objc2_foundation::{NSPoint, NSRect, NSSize, NSString};
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
};

/// The size of the area the tab fills below the header, without the row of its two views.
pub const PAGE_SIZE: NSSize = NSSize::new(580.0, 200.0);
/// The row of the two views at the top of the page, while the arrangement is offered.
pub const MODES_HEIGHT: f64 = 38.0;
/// The canvas the displays are arranged on.
pub const ARRANGE_SIZE: NSSize = NSSize::new(330.0, 176.0);
/// The cards in a row, which is all the page has room for.
pub const MAX_SCREENS: usize = 3;

const MARGIN: f64 = 18.0;
const GAP: f64 = 10.0;
const PAD: f64 = 14.0;
const RADIUS: f64 = 14.0;
/// Below the top of a card: the name, then the rows of controls, one under the other.
const NAME_TOP: f64 = 12.0;
const FIRST_ROW: f64 = 48.0;
const ROW_STEP: f64 = 34.0;
/// From the middle of the last row to the bottom of the card.
const LAST_ROW_BOTTOM: f64 = 30.0;
const ICON: f64 = 22.0;
const PERCENT: f64 = 40.0;
const POPUP_HEIGHT: f64 = 20.0;
const CHIP_HEIGHT: f64 = 28.0;
/// A display drawn on the canvas, and the menu bar on top of the main one.
const SCREEN_RADIUS: f64 = 5.0;
const BAR_HEIGHT: f64 = 4.0;
/// How near the menu bar a press has to be to take it.
const BAR_GRAB: f64 = 9.0;
/// A press that moves less than this is a click.
const CLICK_SLOP: f64 = 3.0;

/// One display: its name and where its settings stand, 0 to 1, and its colour profiles. A setting the display
/// does not have is `None` and has no slider; a display with no profiles has no menu.
#[derive(Debug, Clone, PartialEq)]
pub struct Screen {
    pub name: String,
    pub brightness: Option<f64>,
    pub contrast: Option<f64>,
    /// The names of the profiles it can take.
    pub profiles: Vec<String>,
    /// The one in use, by its place in `profiles`.
    pub profile: Option<usize>,
}

/// A row of a card.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Part {
    Level(Control),
    Profile,
}

/// The rows a display has, in order, with no gap for the ones it lacks.
fn parts(screen: &Screen) -> Vec<Part> {
    let mut parts = Vec::new();
    if screen.brightness.is_some() {
        parts.push(Part::Level(Control::Brightness));
    }
    if screen.contrast.is_some() {
        parts.push(Part::Level(Control::Contrast));
    }
    if !screen.profiles.is_empty() {
        parts.push(Part::Profile);
    }
    parts
}

/// A display on the canvas of the arrangement.
#[derive(Debug, Clone, PartialEq)]
pub struct Arranged {
    pub name: String,
    /// Where it is drawn, in the canvas's points with the origin at its bottom-left.
    pub frame: NSRect,
    /// The main display carries the menu bar.
    pub main: bool,
    /// How many other displays show its picture.
    pub mirrored_by: usize,
}

/// The second view of the tab: the canvas, and the chosen display's settings beside it.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Arrangement {
    pub screens: Vec<Arranged>,
    /// The display whose settings show beside the canvas, by its place in `screens`.
    pub chosen: usize,
    /// Its name and what it is: its size, built in or not.
    pub title: String,
    pub detail: String,
    /// The name of the menu, and its choices: main display, extended display, a mirror of each other display.
    pub use_label: String,
    pub uses: Vec<String>,
    pub using: Option<usize>,
    /// How to arrange, at the bottom of the column.
    pub hint: String,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct DisplaysDisplay {
    pub screens: Vec<Screen>,
    /// Words that replace the cards: the tab is off, or no display answers.
    pub note: Option<(String, String)>,
    /// The names of the two views of the tab and the one shown; `None` shows the cards alone.
    pub modes: Option<([String; 2], usize)>,
    /// What the second view shows.
    pub arrangement: Option<Arrangement>,
}

impl DisplaysDisplay {
    fn arranging(&self) -> bool {
        self.arrangement.is_some() && self.modes.as_ref().is_some_and(|(_, shown)| *shown == 1)
    }

    fn height(&self) -> f64 {
        PAGE_SIZE.height
            + if self.modes.is_some() {
                MODES_HEIGHT
            } else {
                0.0
            }
    }
}

/// The names of the sliders and the menu, for VoiceOver, in the user's language.
#[derive(Debug, Clone, Default)]
pub struct DisplaysTexts {
    pub brightness: String,
    pub contrast: String,
    pub profile: String,
}

/// Told that a slider moved: which display (its place in `screens`), which setting, and where to, 0 to 1.
pub type OnChange = Rc<dyn Fn(usize, Control, f64)>;
/// Told that a profile was chosen: which display, and which profile by its place in the display's `profiles`.
pub type OnProfile = Rc<dyn Fn(usize, usize)>;

/// What the arrangement tells the app, each display by its place in `Arrangement::screens`.
#[derive(Clone)]
pub struct ArrangeCallbacks {
    /// One of the two views was chosen.
    pub on_mode: Rc<dyn Fn(usize)>,
    /// A display was clicked.
    pub on_choose: Rc<dyn Fn(usize)>,
    /// A display was dragged and let go with this frame on the canvas.
    pub on_move: Rc<dyn Fn(usize, NSRect)>,
    /// The menu bar was dragged onto this display.
    pub on_main: Rc<dyn Fn(usize)>,
    /// A choice in the menu of the chosen display.
    pub on_use: Rc<dyn Fn(usize)>,
}

pub struct CanvasIvars {
    display: RefCell<DisplaysDisplay>,
}

define_class!(
    // SAFETY: NSView allows subclassing; the ivars are only touched on the main thread and need no Drop.
    #[unsafe(super(NSView))]
    #[thread_kind = MainThreadOnly]
    #[name = "FigDisplaysCanvas"]
    #[ivars = CanvasIvars]
    pub struct DisplaysCanvas;

    impl DisplaysCanvas {
        #[unsafe(method(drawRect:))]
        fn draw_rect(&self, _dirty: NSRect) {
            draw(self.bounds(), &self.ivars().display.borrow());
        }
    }
);

impl DisplaysCanvas {
    fn new(mtm: MainThreadMarker, frame: NSRect) -> Retained<Self> {
        let this = mtm.alloc::<Self>().set_ivars(CanvasIvars {
            display: RefCell::default(),
        });
        // SAFETY: designated initializer with valid arguments on the main thread.
        unsafe { msg_send![super(this), initWithFrame: frame] }
    }
}

/// The part of the page below the row of the two views.
fn body(bounds: NSRect, modes: bool) -> NSRect {
    let top = if modes { MODES_HEIGHT } else { 0.0 };
    NSRect::new(
        bounds.origin,
        NSSize::new(bounds.size.width, bounds.size.height - top),
    )
}

/// How tall a card is with this many rows; every card is as tall as the one with the most.
fn card_height(rows: usize) -> f64 {
    FIRST_ROW + rows.max(1).saturating_sub(1) as f64 * ROW_STEP + LAST_ROW_BOTTOM
}

/// Where the cards go: side by side across the page, as many as there are displays, as tall as `rows` needs.
fn cards(bounds: NSRect, count: usize, rows: usize) -> Vec<NSRect> {
    let n = count.clamp(1, MAX_SCREENS) as f64;
    let width = (bounds.size.width - 2.0 * MARGIN - (n - 1.0) * GAP) / n;
    let height = card_height(rows);
    // The page is as tall as the note it may show; the cards sit in its middle.
    let y = bounds.origin.y + (bounds.size.height - height) / 2.0;
    (0..count.min(MAX_SCREENS))
        .map(|i| {
            NSRect::new(
                NSPoint::new(MARGIN + i as f64 * (width + GAP), y),
                NSSize::new(width, height),
            )
        })
        .collect()
}

/// The cards of a display list, each with its display.
fn laid_out(bounds: NSRect, screens: &[Screen]) -> Vec<(NSRect, &Screen)> {
    let rows = screens
        .iter()
        .take(MAX_SCREENS)
        .map(|s| parts(s).len())
        .max()
        .unwrap_or(0);
    cards(bounds, screens.len(), rows)
        .into_iter()
        .zip(screens)
        .collect()
}

/// The middle of a row of controls in a card, from the bottom of the page.
fn row_middle(card: NSRect, row: usize) -> f64 {
    card.origin.y + card.size.height - FIRST_ROW - row as f64 * ROW_STEP
}

fn symbol_of(part: Part) -> &'static str {
    match part {
        Part::Level(Control::Brightness) => "sun.max.fill",
        Part::Level(Control::Contrast) => "circle.lefthalf.filled",
        Part::Profile => "paintpalette.fill",
    }
}

/// Where the canvas of the arrangement sits in the body of the page.
fn desk_frame(body: NSRect) -> NSRect {
    NSRect::new(
        NSPoint::new(
            MARGIN,
            body.origin.y + (body.size.height - ARRANGE_SIZE.height) / 2.0,
        ),
        ARRANGE_SIZE,
    )
}

/// The column beside the canvas: its left edge and width.
fn column(bounds: NSRect) -> (f64, f64) {
    let left = MARGIN + ARRANGE_SIZE.width + 16.0;
    (left, bounds.size.width - MARGIN - left)
}

fn draw(bounds: NSRect, display: &DisplaysDisplay) {
    let body = body(bounds, display.modes.is_some());
    if display.arranging() {
        if let Some(arrangement) = &display.arrangement {
            draw_column(bounds, body, arrangement);
        }
        return;
    }
    if let Some((title, hint)) = &display.note {
        draw_page_note("display", title, hint, body.size);
        return;
    }
    let attrs = text_attributes(
        &style::font(13.0, Weight::Semibold),
        &white(style::TEXT),
        Fit::Clip,
        false,
    );
    for (card, screen) in laid_out(body, &display.screens) {
        style::card(card, RADIUS);
        let top = card.origin.y + card.size.height;
        draw_text(
            &screen.name,
            NSRect::new(
                NSPoint::new(card.origin.x + PAD, top - NAME_TOP - 17.0),
                NSSize::new(card.size.width - 2.0 * PAD, 17.0),
            ),
            &attrs,
        );
        for (row, part) in parts(screen).into_iter().enumerate() {
            style::draw_symbol(
                symbol_of(part),
                NSPoint::new(
                    card.origin.x + PAD + ICON / 2.0 - 2.0,
                    row_middle(card, row),
                ),
                13.0,
                &white(style::TEXT_SECONDARY),
            );
        }
    }
}

/// The chosen display's name, what it is, the name of its menu and the hint, beside the canvas.
fn draw_column(bounds: NSRect, body: NSRect, arrangement: &Arrangement) {
    let (left, width) = column(bounds);
    let top = body.origin.y + body.size.height;
    let line =
        |text: &str, y: f64, height: f64, size: f64, weight: Weight, alpha: f64, fit: Fit| {
            draw_text(
                text,
                NSRect::new(NSPoint::new(left, y), NSSize::new(width, height)),
                &text_attributes(&style::font(size, weight), &white(alpha), fit, false),
            );
        };
    line(
        &arrangement.title,
        top - 14.0 - 17.0,
        17.0,
        13.0,
        Weight::Semibold,
        style::TEXT,
        Fit::Tail,
    );
    line(
        &arrangement.detail,
        top - 36.0 - 30.0,
        30.0,
        11.0,
        Weight::Regular,
        style::TEXT_SECONDARY,
        Fit::Words,
    );
    if !arrangement.uses.is_empty() {
        line(
            &arrangement.use_label,
            top - 78.0 - 14.0,
            14.0,
            11.0,
            Weight::Medium,
            style::TEXT_TERTIARY,
            Fit::Tail,
        );
    }
    line(
        &arrangement.hint,
        body.origin.y + 10.0,
        60.0,
        10.5,
        Weight::Regular,
        style::TEXT_TERTIARY,
        Fit::Words,
    );
}

fn percent_text(fraction: f64) -> String {
    format!("{:.0}%", (fraction * 100.0).clamp(0.0, 100.0))
}

// ---------------------------------------------------------------- the canvas of the arrangement

/// A press on the canvas being held: on a display, or on the menu bar of the main one.
#[derive(Debug, Clone, Copy, PartialEq)]
struct Drag {
    index: usize,
    from: NSPoint,
    at: NSPoint,
    bar: bool,
}

pub struct DeskIvars {
    screens: RefCell<Vec<Arranged>>,
    chosen: Cell<usize>,
    drag: Cell<Option<Drag>>,
    callbacks: ArrangeCallbacks,
}

define_class!(
    // SAFETY: NSView allows subclassing; the ivars are only touched on the main thread and need no Drop.
    #[unsafe(super(NSView))]
    #[thread_kind = MainThreadOnly]
    #[name = "FigDisplaysDesk"]
    #[ivars = DeskIvars]
    struct DisplaysDesk;

    impl DisplaysDesk {
        #[unsafe(method(acceptsFirstMouse:))]
        fn accepts_first_mouse(&self, _event: Option<&NSEvent>) -> bool {
            true
        }

        #[unsafe(method(drawRect:))]
        fn draw_rect(&self, _dirty: NSRect) {
            self.paint();
        }

        #[unsafe(method(mouseDown:))]
        fn mouse_down(&self, event: &NSEvent) {
            let at = self.local(event);
            let screens = self.ivars().screens.borrow();
            let grabbed = hit(&screens, at);
            drop(screens);
            self.ivars().drag.set(grabbed.map(|(index, bar)| Drag {
                index,
                from: at,
                at,
                bar,
            }));
            self.setNeedsDisplay(true);
        }

        #[unsafe(method(mouseDragged:))]
        fn mouse_dragged(&self, event: &NSEvent) {
            if let Some(mut drag) = self.ivars().drag.get() {
                drag.at = self.local(event);
                self.ivars().drag.set(Some(drag));
                self.setNeedsDisplay(true);
            }
        }

        #[unsafe(method(mouseUp:))]
        fn mouse_up(&self, event: &NSEvent) {
            let Some(mut drag) = self.ivars().drag.take() else {
                return;
            };
            drag.at = self.local(event);
            self.setNeedsDisplay(true);
            self.release(drag);
        }
    }
);

/// What a press at `at` takes: a display, and whether it is the menu bar of the main one. The top display wins.
fn hit(screens: &[Arranged], at: NSPoint) -> Option<(usize, bool)> {
    let inside = |r: NSRect, p: NSPoint| {
        p.x >= r.origin.x
            && p.x <= r.origin.x + r.size.width
            && p.y >= r.origin.y
            && p.y <= r.origin.y + r.size.height
    };
    if let Some(main) = screens.iter().position(|s| s.main) {
        let f = screens[main].frame;
        let bar = NSRect::new(
            NSPoint::new(f.origin.x, f.origin.y + f.size.height - BAR_GRAB),
            NSSize::new(f.size.width, BAR_GRAB + 2.0),
        );
        if inside(bar, at) && screens.len() > 1 {
            return Some((main, true));
        }
    }
    screens
        .iter()
        .rposition(|s| inside(s.frame, at))
        .map(|i| (i, false))
}

/// A frame moved by a drag, kept on the canvas.
fn dragged(frame: NSRect, drag: &Drag, bounds: NSRect) -> NSRect {
    let x = (frame.origin.x + drag.at.x - drag.from.x)
        .clamp(0.0, (bounds.size.width - frame.size.width).max(0.0));
    let y = (frame.origin.y + drag.at.y - drag.from.y)
        .clamp(0.0, (bounds.size.height - frame.size.height).max(0.0));
    NSRect::new(NSPoint::new(x, y), frame.size)
}

impl DisplaysDesk {
    fn new(mtm: MainThreadMarker, frame: NSRect, callbacks: ArrangeCallbacks) -> Retained<Self> {
        let this = mtm.alloc::<Self>().set_ivars(DeskIvars {
            screens: RefCell::default(),
            chosen: Cell::new(0),
            drag: Cell::new(None),
            callbacks,
        });
        // SAFETY: designated initializer with valid arguments on the main thread.
        unsafe { msg_send![super(this), initWithFrame: frame] }
    }

    fn local(&self, event: &NSEvent) -> NSPoint {
        self.convertPoint_fromView(event.locationInWindow(), None)
    }

    fn show(&self, screens: Vec<Arranged>, chosen: usize) {
        *self.ivars().screens.borrow_mut() = screens;
        self.ivars().chosen.set(chosen);
        self.setNeedsDisplay(true);
    }

    /// A press let go: a click chooses, a drag of a display moves it, a drag of the menu bar makes the display it is
    /// let go on the main one.
    fn release(&self, drag: Drag) {
        let ivars = self.ivars();
        let moved = (drag.at.x - drag.from.x).hypot(drag.at.y - drag.from.y) >= CLICK_SLOP;
        let screens = ivars.screens.borrow().clone();
        if drag.bar {
            if !moved {
                (ivars.callbacks.on_choose)(drag.index);
                return;
            }
            if let Some((target, _)) = hit(&screens, drag.at).filter(|(i, _)| *i != drag.index) {
                (ivars.callbacks.on_main)(target);
            }
            return;
        }
        if !moved {
            (ivars.callbacks.on_choose)(drag.index);
            return;
        }
        if let Some(screen) = screens.get(drag.index) {
            let frame = dragged(screen.frame, &drag, self.bounds());
            (ivars.callbacks.on_move)(drag.index, frame);
        }
    }

    fn paint(&self) {
        let ivars = self.ivars();
        let bounds = self.bounds();
        style::well(bounds, 14.0);
        let screens = ivars.screens.borrow();
        let drag = ivars.drag.get();
        let lifted = drag.filter(|d| !d.bar).map(|d| d.index);
        for (index, screen) in screens.iter().enumerate() {
            if Some(index) != lifted {
                let target = drag
                    .is_some_and(|d| d.bar && hit(&screens, d.at).is_some_and(|(i, _)| i == index));
                paint_screen(
                    screen,
                    screen.frame,
                    index == ivars.chosen.get() || target,
                    !drag.is_some_and(|d| d.bar),
                );
            }
        }
        match drag {
            Some(d) if d.bar => {
                // The menu bar follows the pointer, as wide as it is on its display.
                if let Some(main) = screens.get(d.index) {
                    let width = main.frame.size.width - 6.0;
                    let bar = NSRect::new(
                        NSPoint::new(d.at.x - width / 2.0, d.at.y - BAR_HEIGHT / 2.0),
                        NSSize::new(width, BAR_HEIGHT),
                    );
                    style::fill_rounded(bar, BAR_HEIGHT / 2.0, &white(0.92));
                }
            }
            Some(d) => {
                if let Some(screen) = screens.get(d.index) {
                    paint_screen(screen, dragged(screen.frame, &d, bounds), true, true);
                }
            }
            None => {}
        }
    }
}

/// One display: a stack when others mirror it, lit when chosen, its name in the middle, and the menu bar on the main
/// one unless the bar is being dragged away.
fn paint_screen(screen: &Arranged, frame: NSRect, lit: bool, bar: bool) {
    if screen.mirrored_by > 0 {
        let behind = NSRect::new(
            NSPoint::new(frame.origin.x + 4.0, frame.origin.y - 4.0),
            frame.size,
        );
        style::fill_rounded(behind, SCREEN_RADIUS, &white(0.10));
        style::stroke_rounded(behind, SCREEN_RADIUS, &white(0.22), 1.0);
    }
    if lit {
        style::card_selected(frame, SCREEN_RADIUS, &Tone::Blue);
    } else {
        style::fill_rounded(frame, SCREEN_RADIUS, &white(0.13));
        style::stroke_rounded(frame, SCREEN_RADIUS, &white(0.26), 1.0);
    }
    if screen.main && bar {
        let strip = NSRect::new(
            NSPoint::new(
                frame.origin.x + 3.0,
                frame.origin.y + frame.size.height - 3.0 - BAR_HEIGHT,
            ),
            NSSize::new((frame.size.width - 6.0).max(0.0), BAR_HEIGHT),
        );
        style::fill_rounded(strip, BAR_HEIGHT / 2.0, &white(0.92));
    }
    let size = if frame.size.height < 34.0 { 9.5 } else { 11.0 };
    draw_text(
        &screen.name,
        NSRect::new(
            NSPoint::new(
                frame.origin.x + 4.0,
                frame.origin.y + (frame.size.height - size * 1.3) / 2.0,
            ),
            NSSize::new((frame.size.width - 8.0).max(0.0), size * 1.3),
        ),
        &text_attributes(
            &style::font(size, Weight::Medium),
            &white(if lit {
                style::TEXT
            } else {
                style::TEXT_SECONDARY
            }),
            Fit::Tail,
            true,
        ),
    );
}

// ---------------------------------------------------------------- the controls

/// A slider and the figure beside it, which its action reads once both are made.
type Controls = (Retained<NSSlider>, Retained<NSTextField>);

/// The controls of one setting of one display.
struct Row {
    slider: Retained<NSSlider>,
    percent: Retained<NSTextField>,
    _target: Retained<ActionTarget>,
}

/// A menu: the profiles of a display, or what the chosen display is used as.
struct Menu {
    popup: Retained<NSPopUpButton>,
    _target: Retained<ActionTarget>,
}

/// The controls of one display: its brightness and contrast rows, either of which it may not have, and its menu.
#[derive(Default)]
struct CardControls {
    levels: [Option<Row>; 2],
    menu: Option<Menu>,
}

fn level_place(control: Control) -> usize {
    match control {
        Control::Brightness => 0,
        Control::Contrast => 1,
    }
}

/// A small capsule menu with these items, the one in use chosen, reporting a choice by its place.
fn popup_menu(
    mtm: MainThreadMarker,
    frame: NSRect,
    items: &[String],
    chosen: Option<usize>,
    label: &str,
    on_pick: impl Fn(usize) + 'static,
) -> Menu {
    let slot: Rc<RefCell<Option<Retained<NSPopUpButton>>>> = Rc::default();
    let reader = Rc::clone(&slot);
    let target = ActionTarget::new(mtm, move || {
        if let Some(popup) = reader.borrow().as_ref()
            && let Ok(choice) = usize::try_from(popup.indexOfSelectedItem())
        {
            on_pick(choice);
        }
    });
    let popup = NSPopUpButton::initWithFrame_pullsDown(mtm.alloc(), frame, false);
    popup.setControlSize(NSControlSize::Small);
    // The badge bezel is a small capsule, as the menus of the sound mixer are.
    popup.setBezelStyle(NSBezelStyle::Badge);
    popup.setFont(Some(&NSFont::systemFontOfSize(11.0)));
    // Through the menu, so that two items of the same name are both listed.
    if let Some(menu) = popup.menu() {
        for name in items {
            // SAFETY: an item with no action, which the pop-up button's own action covers.
            unsafe {
                menu.addItemWithTitle_action_keyEquivalent(
                    &NSString::from_str(name),
                    None,
                    &NSString::new(),
                );
            }
        }
    }
    popup.selectItemAtIndex(chosen.map_or(-1, |p| p as isize));
    let target_object: &objc2::runtime::AnyObject = &target;
    // SAFETY: `fire:` takes the sender as its only argument; the menu keeps the target alive.
    unsafe {
        popup.setTarget(Some(target_object));
        popup.setAction(Some(sel!(fire:)));
    }
    popup.setAccessibilityLabel(Some(&NSString::from_str(label)));
    *slot.borrow_mut() = Some(popup.clone());
    Menu {
        popup,
        _target: target,
    }
}

/// What the controls were made for: the controls are made again only when this changes.
type Shape = (bool, bool, Vec<(String, Vec<Part>, Vec<String>)>);

fn shape(d: &DisplaysDisplay) -> Shape {
    let cards = if d.note.is_some() || d.arranging() {
        Vec::new()
    } else {
        d.screens
            .iter()
            .take(MAX_SCREENS)
            .map(|s| (s.name.clone(), parts(s), s.profiles.clone()))
            .collect()
    };
    (d.modes.is_some(), d.arranging(), cards)
}

pub struct DisplaysView {
    mtm: MainThreadMarker,
    canvas: Retained<DisplaysCanvas>,
    texts: DisplaysTexts,
    on_change: OnChange,
    on_profile: OnProfile,
    rows: RefCell<Vec<CardControls>>,
    modes: [Retained<TouchButton>; 2],
    desk: Retained<DisplaysDesk>,
    /// What the chosen display is used as, with the choices it was made with.
    uses: RefCell<Option<(Vec<String>, Menu)>>,
    callbacks: ArrangeCallbacks,
    built: RefCell<Option<Shape>>,
}

impl DisplaysView {
    /// `frame` is `Notch::page_area(PAGE_SIZE.width, PAGE_SIZE.height)`.
    pub fn new(
        mtm: MainThreadMarker,
        frame: NSRect,
        texts: DisplaysTexts,
        on_change: OnChange,
        on_profile: OnProfile,
        callbacks: ArrangeCallbacks,
    ) -> Rc<Self> {
        let canvas = DisplaysCanvas::new(mtm, frame);
        let chip = |index: usize| {
            let button = TouchButton::new(mtm, NSRect::ZERO, Look::Chip);
            let on_mode = Rc::clone(&callbacks.on_mode);
            button.set_on_click(move || on_mode(index));
            button.setHidden(true);
            canvas.addSubview(&button);
            button
        };
        let modes = [chip(0), chip(1)];
        let desk = DisplaysDesk::new(
            mtm,
            NSRect::new(NSPoint::ZERO, ARRANGE_SIZE),
            callbacks.clone(),
        );
        desk.setHidden(true);
        canvas.addSubview(&desk);
        Rc::new(Self {
            mtm,
            canvas,
            texts,
            on_change,
            on_profile,
            rows: RefCell::default(),
            modes,
            desk,
            uses: RefCell::default(),
            callbacks,
            built: RefCell::default(),
        })
    }

    /// Shows the displays. The controls are made again only when the displays, their settings, their profiles or the
    /// view change; otherwise they are moved, so that a slider in the hand is not taken away from under it.
    pub fn show(&self, display: DisplaysDisplay) {
        let height = display.height();
        if (self.canvas.frame().size.height - height).abs() > 0.5 {
            // The shape of the notch follows the page's frame.
            self.canvas
                .setFrameSize(NSSize::new(PAGE_SIZE.width, height));
            self.built.borrow_mut().take();
        }
        let wanted = shape(&display);
        if self.built.borrow().as_ref() != Some(&wanted) {
            self.build(&display);
            *self.built.borrow_mut() = Some(wanted);
        }
        for (controls, screen) in self.rows.borrow().iter().zip(&display.screens) {
            for (row, level) in controls
                .levels
                .iter()
                .zip([screen.brightness, screen.contrast])
            {
                if let (Some(row), Some(level)) = (row, level) {
                    row.slider.setDoubleValue(level);
                    row.percent
                        .setStringValue(&NSString::from_str(&percent_text(level)));
                }
            }
            if let Some(menu) = &controls.menu {
                menu.popup
                    .selectItemAtIndex(screen.profile.map_or(-1, |p| p as isize));
            }
        }
        self.show_modes(&display);
        self.show_arrangement(&display);
        *self.canvas.ivars().display.borrow_mut() = display;
        self.canvas.setNeedsDisplay(true);
    }

    /// The two chips at the top left, while the arrangement is offered.
    fn show_modes(&self, display: &DisplaysDisplay) {
        let top = self.canvas.frame().size.height - 10.0 - CHIP_HEIGHT;
        let mut left = MARGIN;
        for (i, chip) in self.modes.iter().enumerate() {
            match &display.modes {
                Some((names, shown)) => {
                    chip.set_title(&names[i]);
                    chip.set_selected(i == *shown);
                    chip.setHidden(false);
                    let width = chip.fit_width();
                    chip.setFrame(NSRect::new(
                        NSPoint::new(left, top),
                        NSSize::new(width, CHIP_HEIGHT),
                    ));
                    left += width + 6.0;
                }
                _ => chip.setHidden(true),
            }
        }
    }

    /// The canvas and the menu of the chosen display, while the arrangement is shown.
    fn show_arrangement(&self, display: &DisplaysDisplay) {
        let arrangement = display.arrangement.as_ref().filter(|_| display.arranging());
        self.desk.setHidden(arrangement.is_none());
        let Some(arrangement) = arrangement else {
            if let Some((_, menu)) = self.uses.borrow_mut().take() {
                menu.popup.removeFromSuperview();
            }
            return;
        };
        let bounds = self.canvas.bounds();
        let body = body(bounds, true);
        self.desk.setFrame(desk_frame(body));
        self.desk
            .show(arrangement.screens.clone(), arrangement.chosen);
        let mut uses = self.uses.borrow_mut();
        if uses.as_ref().map(|(items, _)| items) != Some(&arrangement.uses) {
            if let Some((_, menu)) = uses.take() {
                menu.popup.removeFromSuperview();
            }
            if !arrangement.uses.is_empty() {
                let (left, width) = column(bounds);
                let top = body.origin.y + body.size.height;
                let on_use = Rc::clone(&self.callbacks.on_use);
                let menu = popup_menu(
                    self.mtm,
                    NSRect::new(
                        NSPoint::new(left - 2.0, top - 98.0 - POPUP_HEIGHT),
                        NSSize::new(width + 2.0, POPUP_HEIGHT),
                    ),
                    &arrangement.uses,
                    arrangement.using,
                    &format!("{}, {}", arrangement.use_label, arrangement.title),
                    move |choice| on_use(choice),
                );
                self.canvas.addSubview(&menu.popup);
                *uses = Some((arrangement.uses.clone(), menu));
            }
        }
        if let Some((_, menu)) = uses.as_ref() {
            menu.popup
                .selectItemAtIndex(arrangement.using.map_or(-1, |u| u as isize));
            menu.popup
                .setAccessibilityLabel(Some(&NSString::from_str(&format!(
                    "{}, {}",
                    arrangement.use_label, arrangement.title
                ))));
        }
    }

    fn build(&self, display: &DisplaysDisplay) {
        for controls in self.rows.borrow_mut().drain(..) {
            for row in controls.levels.into_iter().flatten() {
                row.slider.removeFromSuperview();
                row.percent.removeFromSuperview();
            }
            if let Some(menu) = controls.menu {
                menu.popup.removeFromSuperview();
            }
        }
        if display.note.is_some() || display.arranging() {
            return;
        }
        let body = body(self.canvas.bounds(), display.modes.is_some());
        let mut built = Vec::new();
        for (index, (card, screen)) in laid_out(body, &display.screens).into_iter().enumerate() {
            let mut controls = CardControls::default();
            for (row, part) in parts(screen).into_iter().enumerate() {
                match part {
                    Part::Level(control) => {
                        let name = match control {
                            Control::Brightness => &self.texts.brightness,
                            Control::Contrast => &self.texts.contrast,
                        };
                        controls.levels[level_place(control)] =
                            Some(self.row(card, row, index, control, &screen.name, name));
                    }
                    Part::Profile => controls.menu = Some(self.menu(card, row, index, screen)),
                }
            }
            built.push(controls);
        }
        *self.rows.borrow_mut() = built;
    }

    /// The menu of a display's colour profiles, with the one in use chosen.
    fn menu(&self, card: NSRect, row: usize, index: usize, screen: &Screen) -> Menu {
        let middle = row_middle(card, row);
        let left = card.origin.x + PAD + ICON;
        let right = card.origin.x + card.size.width - PAD;
        let on_profile = Rc::clone(&self.on_profile);
        let menu = popup_menu(
            self.mtm,
            NSRect::new(
                NSPoint::new(left - 2.0, middle - POPUP_HEIGHT / 2.0),
                NSSize::new(right - left + 2.0, POPUP_HEIGHT),
            ),
            &screen.profiles,
            screen.profile,
            &format!("{}, {}", self.texts.profile, screen.name),
            move |choice| on_profile(index, choice),
        );
        self.canvas.addSubview(&menu.popup);
        menu
    }

    fn row(
        &self,
        card: NSRect,
        row: usize,
        index: usize,
        control: Control,
        screen: &str,
        name: &str,
    ) -> Row {
        let mtm = self.mtm;
        let middle = row_middle(card, row);
        let left = card.origin.x + PAD + ICON;
        let right = card.origin.x + card.size.width - PAD;
        let percent = NSTextField::labelWithString(&NSString::new(), mtm);
        percent.setFont(Some(&NSFont::monospacedDigitSystemFontOfSize_weight(
            11.0,
            // SAFETY: extern static weight constant.
            unsafe { objc2_app_kit::NSFontWeightMedium },
        )));
        percent.setTextColor(Some(&white(style::TEXT_SECONDARY)));
        percent.setAlignment(NSTextAlignment::Right);
        percent.setLineBreakMode(NSLineBreakMode::ByClipping);
        percent.setFrame(NSRect::new(
            NSPoint::new(right - PERCENT, middle - 8.0),
            NSSize::new(PERCENT, 16.0),
        ));
        let slot: Rc<RefCell<Option<Controls>>> = Rc::default();
        let (reader, on_change) = (Rc::clone(&slot), Rc::clone(&self.on_change));
        let target = ActionTarget::new(mtm, move || {
            if let Some((slider, percent)) = reader.borrow().as_ref() {
                let value = slider.doubleValue();
                percent.setStringValue(&NSString::from_str(&percent_text(value)));
                on_change(index, control, value);
            }
        });
        let target_object: &objc2::runtime::AnyObject = &target;
        // SAFETY: `fire:` takes the sender as its only argument; the row keeps the target alive.
        let slider = unsafe {
            NSSlider::sliderWithValue_minValue_maxValue_target_action(
                0.5,
                0.0,
                1.0,
                Some(target_object),
                Some(sel!(fire:)),
                mtm,
            )
        };
        slider.setControlSize(NSControlSize::Small);
        slider.setContinuous(true);
        slider.setTrackFillColor(Some(&white(style::TEXT)));
        slider.setFrame(NSRect::new(
            NSPoint::new(left, middle - 9.0),
            NSSize::new(right - PERCENT - 6.0 - left, 18.0),
        ));
        slider.setAccessibilityLabel(Some(&NSString::from_str(&format!("{name}, {screen}"))));
        *slot.borrow_mut() = Some((slider.clone(), percent.clone()));
        self.canvas.addSubview(&slider);
        self.canvas.addSubview(&percent);
        Row {
            slider,
            percent,
            _target: target,
        }
    }

    /// The notch tab for this view.
    pub fn page(
        self: &Rc<Self>,
        title: String,
        on_show: impl Fn() + 'static,
        on_hide: impl Fn() + 'static,
    ) -> Page {
        Page {
            id: "displays",
            title,
            symbol: "display",
            root: Retained::into_super(Retained::clone(&self.canvas)),
            on_show: Box::new(on_show),
            on_hide: Box::new(on_hide),
            on_key: Box::new(|_| false),
            accepts_files: false,
            size: None,
        }
    }

    /// The page drawn into a PNG, sliders included, for the pictures of the documentation and the checks.
    pub fn snapshot_png(&self) -> Option<Vec<u8>> {
        crate::widgets::view_png(&self.canvas)
    }

    /// Chooses a profile in a display's menu as a person would, for the checks: it reports the choice as a click does.
    pub fn choose(&self, index: usize, choice: usize) -> bool {
        let rows = self.rows.borrow();
        let Some(Some(menu)) = rows.get(index).map(|r| r.menu.as_ref()) else {
            return false;
        };
        fire_menu(menu, choice)
    }

    /// Chooses what the chosen display of the arrangement is used as, for the checks.
    pub fn choose_use(&self, choice: usize) -> bool {
        self.uses
            .borrow()
            .as_ref()
            .is_some_and(|(_, menu)| fire_menu(menu, choice))
    }

    /// Lets go of a display of the arrangement at a frame of the canvas, as a drag does, for the checks.
    pub fn drop_screen(&self, index: usize, frame: NSRect) {
        (self.callbacks.on_move)(index, frame);
    }

    /// Moves the slider of a setting as a person would, for the checks: it reports the move as a drag does.
    pub fn press(&self, index: usize, control: Control, value: f64) -> bool {
        let rows = self.rows.borrow();
        let Some(Some(row)) = rows
            .get(index)
            .map(|r| r.levels[level_place(control)].as_ref())
        else {
            return false;
        };
        row.slider.setDoubleValue(value);
        // SAFETY: the target's action takes the sender, which may be nil.
        let _: () = unsafe { msg_send![&*row._target, fire: Option::<&NSView>::None] };
        true
    }
}

fn fire_menu(menu: &Menu, choice: usize) -> bool {
    menu.popup.selectItemAtIndex(choice as isize);
    // SAFETY: the target's action takes the sender, which may be nil.
    let _: () = unsafe { msg_send![&*menu._target, fire: Option::<&NSView>::None] };
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_cards_share_the_width_and_stay_on_the_page() {
        let bounds = NSRect::new(NSPoint::ZERO, PAGE_SIZE);
        for count in 1..=MAX_SCREENS {
            for rows in 1..=3 {
                let cards = cards(bounds, count, rows);
                assert_eq!(cards.len(), count);
                let last = cards.last().unwrap();
                assert!(
                    (last.origin.x + last.size.width - (PAGE_SIZE.width - MARGIN)).abs() < 1e-9
                );
                assert!(cards.iter().all(|c| c.origin.y >= 0.0), "{rows} rows fit");
            }
        }
        assert_eq!(
            cards(bounds, 5, 2).len(),
            MAX_SCREENS,
            "the rest have no room"
        );
        assert!(cards(bounds, 0, 2).is_empty());
        assert_eq!(
            card_height(2),
            112.0,
            "two rows keep the height the cards had"
        );
        // Below the row of the two views, the cards keep the room they had.
        let tall = NSRect::new(
            NSPoint::ZERO,
            NSSize::new(PAGE_SIZE.width, PAGE_SIZE.height + MODES_HEIGHT),
        );
        assert_eq!(body(tall, true).size, PAGE_SIZE);
        let below = cards(body(tall, true), 2, 3);
        assert!(
            below
                .iter()
                .all(|c| c.origin.y + c.size.height <= PAGE_SIZE.height)
        );
    }

    #[test]
    fn the_rows_follow_one_another_without_gaps() {
        let screen = |brightness: bool, contrast: bool, profiles: usize| Screen {
            name: "A".into(),
            brightness: brightness.then_some(0.5),
            contrast: contrast.then_some(0.5),
            profiles: vec!["P".into(); profiles],
            profile: None,
        };
        assert_eq!(
            parts(&screen(true, true, 2)),
            [
                Part::Level(Control::Brightness),
                Part::Level(Control::Contrast),
                Part::Profile
            ]
        );
        assert_eq!(
            parts(&screen(true, false, 2)),
            [Part::Level(Control::Brightness), Part::Profile]
        );
        assert_eq!(
            parts(&screen(false, false, 1)),
            [Part::Profile],
            "a display that does not answer DDC/CI"
        );
        assert!(parts(&screen(false, false, 0)).is_empty());
    }

    #[test]
    fn percentages_are_whole_and_held_to_the_range() {
        assert_eq!(percent_text(0.756), "76%");
        assert_eq!(percent_text(2.0), "100%");
        assert_eq!(percent_text(-0.5), "0%");
    }

    fn arranged(x: f64, y: f64, w: f64, h: f64, main: bool) -> Arranged {
        Arranged {
            name: "A".into(),
            frame: NSRect::new(NSPoint::new(x, y), NSSize::new(w, h)),
            main,
            mirrored_by: 0,
        }
    }

    #[test]
    fn a_press_takes_the_menu_bar_of_the_main_display_or_the_display_on_top() {
        let screens = [
            arranged(10.0, 10.0, 100.0, 60.0, true),
            arranged(110.0, 20.0, 120.0, 70.0, false),
        ];
        assert_eq!(
            hit(&screens, NSPoint::new(50.0, 67.0)),
            Some((0, true)),
            "the strip at the top"
        );
        assert_eq!(hit(&screens, NSPoint::new(50.0, 30.0)), Some((0, false)));
        assert_eq!(hit(&screens, NSPoint::new(150.0, 40.0)), Some((1, false)));
        assert_eq!(hit(&screens, NSPoint::new(300.0, 40.0)), None);
        assert_eq!(
            hit(&screens[..1], NSPoint::new(50.0, 67.0)),
            Some((0, false)),
            "alone, the menu bar has nowhere to go"
        );
    }

    #[test]
    fn a_dragged_display_stays_on_the_canvas() {
        let bounds = NSRect::new(NSPoint::ZERO, ARRANGE_SIZE);
        let frame = NSRect::new(NSPoint::new(20.0, 20.0), NSSize::new(80.0, 50.0));
        let far = Drag {
            index: 0,
            from: NSPoint::new(30.0, 30.0),
            at: NSPoint::new(900.0, -400.0),
            bar: false,
        };
        let moved = dragged(frame, &far, bounds);
        assert_eq!(moved.origin, NSPoint::new(ARRANGE_SIZE.width - 80.0, 0.0));
        let near = Drag {
            at: NSPoint::new(40.0, 35.0),
            ..far
        };
        assert_eq!(
            dragged(frame, &near, bounds).origin,
            NSPoint::new(30.0, 25.0)
        );
    }
}
