//! The colour picker: a loupe that follows the pointer over a magnified, gridded view of the pixels around
//! it, with the colour under the centre in the chosen format. Tab changes the format, a click copies the
//! value, Escape ends it.
//!
//! A transparent panel per display catches the pointer and the keys, because a click must not reach the app
//! below. Both kinds of panel are left out of the pixels that are read: the read is of everything below the
//! catching panel. Nothing runs while the picker is off: moves of the pointer, not a timer, trigger a read,
//! and at most about thirty are made a second.

pub mod sample;

use crate::{
    app,
    panel::FigPanel,
    style::{self, Weight, fill_capsule, fill_circle, fill_rounded, stroke_rounded},
    widgets::{Fit, draw_text, text_attributes},
};
use objc2::runtime::{AnyObject, ProtocolObject};
use objc2::{
    AnyThread, DefinedClass, MainThreadMarker, MainThreadOnly, define_class, msg_send, rc::Retained,
};
use objc2_app_kit::{
    NSBezierPath, NSColor, NSEvent, NSFontAttributeName, NSForegroundColorAttributeName,
    NSLineBreakMode, NSMutableParagraphStyle, NSParagraphStyleAttributeName, NSScreen,
    NSTextAlignment, NSTrackingArea, NSTrackingAreaOptions, NSView,
};
use objc2_foundation::{NSMutableDictionary, NSPoint, NSRect, NSSize, NSString};
use sample::Grid;
use std::{
    cell::{Cell, RefCell},
    rc::{Rc, Weak},
    time::{Duration, Instant},
};

/// Pixels read around the pointer on each side of the middle one.
const CELLS: usize = 11;
const CELL: f64 = 12.0;
const DIAMETER: f64 = CELLS as f64 * CELL;
const WIDTH: f64 = 232.0;
const CARD_TOP: f64 = DIAMETER + 12.0;
const CARD_HEIGHT: f64 = 78.0;
const HEIGHT: f64 = CARD_TOP + CARD_HEIGHT + 6.0;
/// Where the loupe sits relative to the pointer, clear of the pixels that are read.
const GAP: f64 = 22.0;
/// Between two reads, which keeps them to about thirty a second.
const MIN_INTERVAL: Duration = Duration::from_millis(33);
/// Almost invisible, but not clear: the window server lets clicks through a window that is fully clear.
const CATCH_ALPHA: f64 = 0.01;

/// The text the loupe shows, in the language of the app.
#[derive(Debug, Clone)]
pub struct Strings {
    /// The keys, in a short line: "Tab format   Click copy   Esc exit".
    pub hint: String,
    /// Shown instead of a colour while Screen Recording is not allowed.
    pub no_access: String,
    /// Names of the formats in the order Tab goes through them.
    pub formats: Vec<String>,
}

pub struct Callbacks {
    /// The text for a colour in the format with this index.
    pub render: Box<dyn Fn([u8; 3], usize) -> String>,
    /// Called with the text of the colour that was clicked, once, before the picker ends.
    pub on_copy: Box<dyn Fn(String)>,
}

/// Where the pixels, the permission and the display frames come from. `system` is the real thing; the
/// checks swap in a synthetic one so nothing depends on the screen.
pub struct Source {
    pub access: Box<dyn Fn() -> bool>,
    /// The pixels around a point (global, bottom left origin) below the window with this number.
    pub grab: Box<dyn Fn(NSPoint, usize, u32) -> Option<Grid>>,
    /// The frames of the displays, in the same coordinates.
    pub screens: Box<dyn Fn() -> Vec<NSRect>>,
}

impl Source {
    pub fn system(mtm: MainThreadMarker) -> Self {
        Self {
            access: Box::new(sample::screen_access),
            grab: Box::new(move |point, cells, below| {
                let scale = NSScreen::screens(mtm)
                    .iter()
                    .find(|s| contains(s.frame(), point))
                    .map_or(1.0, |s| s.backingScaleFactor());
                sample::grab(point, cells, below, scale)
            }),
            screens: Box::new(move || NSScreen::screens(mtm).iter().map(|s| s.frame()).collect()),
        }
    }
}

fn contains(frame: NSRect, p: NSPoint) -> bool {
    p.x >= frame.origin.x
        && p.x < frame.origin.x + frame.size.width
        && p.y >= frame.origin.y
        && p.y < frame.origin.y + frame.size.height
}

/// Where the loupe's bottom left corner goes: to the lower right of the pointer, moved to the other side
/// when it would leave the display, and always inside it.
pub fn place(pointer: NSPoint, size: NSSize, screen: NSRect) -> NSPoint {
    let (left, bottom) = (screen.origin.x, screen.origin.y);
    let (right, top) = (left + screen.size.width, bottom + screen.size.height);
    let x = if pointer.x + GAP + size.width <= right {
        pointer.x + GAP
    } else {
        pointer.x - GAP - size.width
    };
    let hanging = pointer.y + 14.0 - size.height;
    let y = if hanging >= bottom {
        hanging
    } else {
        pointer.y - 14.0
    };
    NSPoint::new(
        x.clamp(left, (right - size.width).max(left)),
        y.clamp(bottom, (top - size.height).max(bottom)),
    )
}

enum Input {
    Moved(NSPoint),
    Click,
    Cancel,
    Format(isize),
}

type OnInput = Rc<dyn Fn(Input)>;

pub struct CatcherIvars {
    on_input: OnInput,
}

define_class!(
    // SAFETY: NSView allows subclassing; the ivars are only touched on the main thread and need no Drop.
    #[unsafe(super(NSView))]
    #[thread_kind = MainThreadOnly]
    #[name = "FigPickerCatcher"]
    #[ivars = CatcherIvars]
    struct Catcher;

    impl Catcher {
        #[unsafe(method(acceptsFirstResponder))]
        fn accepts_first_responder(&self) -> bool {
            true
        }

        #[unsafe(method(acceptsFirstMouse:))]
        fn accepts_first_mouse(&self, _event: Option<&NSEvent>) -> bool {
            true
        }

        #[unsafe(method(isOpaque))]
        fn is_opaque(&self) -> bool {
            false
        }

        #[unsafe(method(drawRect:))]
        fn draw_rect(&self, dirty: NSRect) {
            style::white(CATCH_ALPHA).setFill();
            NSBezierPath::fillRect(dirty);
        }

        #[unsafe(method(mouseMoved:))]
        fn mouse_moved(&self, event: &NSEvent) {
            self.moved(event);
        }

        #[unsafe(method(mouseDragged:))]
        fn mouse_dragged(&self, event: &NSEvent) {
            self.moved(event);
        }

        #[unsafe(method(mouseDown:))]
        fn mouse_down(&self, _event: &NSEvent) {
            (self.ivars().on_input)(Input::Click);
        }

        // Any other button ends it, like Escape: only the main button picks.
        #[unsafe(method(rightMouseDown:))]
        fn right_mouse_down(&self, _event: &NSEvent) {
            (self.ivars().on_input)(Input::Cancel);
        }

        #[unsafe(method(otherMouseDown:))]
        fn other_mouse_down(&self, _event: &NSEvent) {
            (self.ivars().on_input)(Input::Cancel);
        }

        #[unsafe(method(keyDown:))]
        fn key_down(&self, event: &NSEvent) {
            use objc2_app_kit::NSEventModifierFlags as Flags;
            let on_input = &self.ivars().on_input;
            match event.keyCode() {
                48 => {
                    let back = event.modifierFlags().contains(Flags::Shift);
                    on_input(Input::Format(if back { -1 } else { 1 }));
                }
                53 => on_input(Input::Cancel),
                36 | 76 => on_input(Input::Click),
                _ => {}
            }
        }
    }
);

impl Catcher {
    fn new(mtm: MainThreadMarker, frame: NSRect, on_input: OnInput) -> Retained<Self> {
        let this = mtm.alloc::<Self>().set_ivars(CatcherIvars { on_input });
        // SAFETY: designated initializer with valid arguments on the main thread.
        let this: Retained<Self> = unsafe { msg_send![super(this), initWithFrame: frame] };
        // Moves arrive while the app is inactive, which a non-activating panel always is.
        // SAFETY: valid options and owner; InVisibleRect ignores the (empty) rect.
        let area = unsafe {
            NSTrackingArea::initWithRect_options_owner_userInfo(
                NSTrackingArea::alloc(),
                NSRect::ZERO,
                NSTrackingAreaOptions::MouseMoved
                    | NSTrackingAreaOptions::ActiveAlways
                    | NSTrackingAreaOptions::InVisibleRect,
                Some(&this),
                None,
            )
        };
        this.addTrackingArea(&area);
        this
    }

    fn moved(&self, event: &NSEvent) {
        if let Some(window) = self.window() {
            let at = window.convertPointToScreen(event.locationInWindow());
            (self.ivars().on_input)(Input::Moved(at));
        }
    }
}

/// What the loupe draws.
#[derive(Default)]
struct Look {
    grid: Option<Grid>,
    text: String,
    format: String,
    /// The message that replaces the colour when nothing can be read.
    blocked: Option<String>,
    hint: String,
}

pub struct LoupeIvars {
    look: RefCell<Look>,
}

define_class!(
    // SAFETY: NSView allows subclassing; the ivars are only touched on the main thread and need no Drop.
    #[unsafe(super(NSView))]
    #[thread_kind = MainThreadOnly]
    #[name = "FigPickerLoupe"]
    #[ivars = LoupeIvars]
    struct Loupe;

    impl Loupe {
        #[unsafe(method(isFlipped))]
        fn is_flipped(&self) -> bool {
            true
        }

        #[unsafe(method(isOpaque))]
        fn is_opaque(&self) -> bool {
            false
        }

        #[unsafe(method(drawRect:))]
        fn draw_rect(&self, _dirty: NSRect) {
            paint(&self.ivars().look.borrow());
        }
    }
);

impl Loupe {
    fn new(mtm: MainThreadMarker) -> Retained<Self> {
        let this = mtm.alloc::<Self>().set_ivars(LoupeIvars {
            look: RefCell::new(Look::default()),
        });
        let frame = NSRect::new(NSPoint::new(0.0, 0.0), NSSize::new(WIDTH, HEIGHT));
        // SAFETY: designated initializer with valid arguments on the main thread.
        unsafe { msg_send![super(this), initWithFrame: frame] }
    }

    fn show(&self, look: Look) {
        *self.ivars().look.borrow_mut() = look;
        self.setNeedsDisplay(true);
    }
}

fn rgb_colour([r, g, b]: [u8; 3]) -> Retained<NSColor> {
    NSColor::colorWithSRGBRed_green_blue_alpha(
        f64::from(r) / 255.0,
        f64::from(g) / 255.0,
        f64::from(b) / 255.0,
        1.0,
    )
}

fn rect(x: f64, y: f64, w: f64, h: f64) -> NSRect {
    NSRect::new(NSPoint::new(x, y), NSSize::new(w, h))
}

fn paint(look: &Look) {
    let ring = rect((WIDTH - DIAMETER) / 2.0, 4.0, DIAMETER, DIAMETER);
    match &look.grid {
        Some(grid) => paint_pixels(grid, ring),
        None => {
            fill_circle(ring, &style::black(0.82));
            style::draw_symbol(
                "eye.slash",
                NSPoint::new(
                    ring.origin.x + DIAMETER / 2.0,
                    ring.origin.y + DIAMETER / 2.0,
                ),
                34.0,
                &style::white(style::TEXT_SECONDARY),
            );
        }
    }
    stroke_rounded(
        rect(
            ring.origin.x - 1.0,
            ring.origin.y - 1.0,
            DIAMETER + 2.0,
            DIAMETER + 2.0,
        ),
        DIAMETER / 2.0 + 1.0,
        &style::black(0.45),
        1.0,
    );
    stroke_rounded(ring, DIAMETER / 2.0, &style::white(0.94), 3.0);

    let card = rect(6.0, CARD_TOP, WIDTH - 12.0, CARD_HEIGHT);
    fill_rounded(card, style::BUTTON_RADIUS + 2.0, &style::black(0.88));
    stroke_rounded(card, style::BUTTON_RADIUS + 2.0, &style::white(0.14), 1.0);

    let secondary = style::white(style::TEXT_SECONDARY);
    let hint = text_attributes(
        &style::font(10.5, Weight::Regular),
        &secondary,
        Fit::Tail,
        true,
    );
    let (x, y, w) = (card.origin.x, card.origin.y, card.size.width);
    draw_text(&look.hint, rect(x + 8.0, y + 57.0, w - 16.0, 14.0), &hint);

    if let Some(message) = &look.blocked {
        draw_text(
            message,
            rect(x + 12.0, y + 9.0, w - 24.0, 44.0),
            &message_attributes(),
        );
        return;
    }
    let swatch = rect(x + 12.0, y + 10.0, 26.0, 26.0);
    if let Some(grid) = &look.grid {
        fill_circle(swatch, &rgb_colour(grid.centre()));
    }
    stroke_rounded(swatch, 13.0, &style::white(0.6), 1.5);
    let value = text_attributes(
        &style::digits(14.0, Weight::Semibold),
        &style::white(style::TEXT),
        Fit::Tail,
        false,
    );
    draw_text(
        &look.text,
        rect(x + 48.0, y + 9.0, w - 48.0 - 10.0, 18.0),
        &value,
    );
    let chip = rect(x + 48.0, y + 30.0, 38.0, 17.0);
    fill_capsule(chip, &style::white(style::CONTROL_HOVER));
    let label = text_attributes(
        &style::font(10.0, Weight::Semibold),
        &style::white(style::TEXT),
        Fit::Clip,
        true,
    );
    draw_text(
        &look.format,
        rect(chip.origin.x, chip.origin.y + 2.0, chip.size.width, 13.0),
        &label,
    );
}

/// Centred text that breaks between words, which the shared `Fit` has no mode for.
fn message_attributes() -> Retained<NSMutableDictionary<NSString, AnyObject>> {
    let style = NSMutableParagraphStyle::new();
    style.setLineBreakMode(NSLineBreakMode::ByWordWrapping);
    style.setAlignment(NSTextAlignment::Center);
    let dict = NSMutableDictionary::<NSString, AnyObject>::new();
    let font = style::font(11.0, Weight::Medium);
    let ink = style::white(style::TEXT);
    // SAFETY: extern statics are constants; the values are valid attribute objects for those keys.
    unsafe {
        dict.setObject_forKey(&*font, ProtocolObject::from_ref(NSFontAttributeName));
        dict.setObject_forKey(
            &*ink,
            ProtocolObject::from_ref(NSForegroundColorAttributeName),
        );
        dict.setObject_forKey(
            &*style,
            ProtocolObject::from_ref(NSParagraphStyleAttributeName),
        );
    }
    dict
}

fn paint_pixels(grid: &Grid, ring: NSRect) {
    objc2_app_kit::NSGraphicsContext::saveGraphicsState_class();
    NSBezierPath::bezierPathWithOvalInRect(ring).addClip();
    for y in 0..grid.cells {
        for x in 0..grid.cells {
            rgb_colour(grid.at(x, y)).setFill();
            // A hair larger than the cell, so no seam of the background shows between cells.
            NSBezierPath::fillRect(rect(
                ring.origin.x + x as f64 * CELL,
                ring.origin.y + y as f64 * CELL,
                CELL + 0.5,
                CELL + 0.5,
            ));
        }
    }
    let lines = NSBezierPath::bezierPath();
    for i in 0..=grid.cells {
        let offset = i as f64 * CELL;
        lines.moveToPoint(NSPoint::new(ring.origin.x + offset, ring.origin.y));
        lines.lineToPoint(NSPoint::new(
            ring.origin.x + offset,
            ring.origin.y + DIAMETER,
        ));
        lines.moveToPoint(NSPoint::new(ring.origin.x, ring.origin.y + offset));
        lines.lineToPoint(NSPoint::new(
            ring.origin.x + DIAMETER,
            ring.origin.y + offset,
        ));
    }
    style::black(0.22).setStroke();
    lines.setLineWidth(0.5);
    lines.stroke();
    // The middle pixel is the one that is picked: dark outside and light inside, readable on any colour.
    let mid = (grid.cells / 2) as f64 * CELL;
    let centre = rect(ring.origin.x + mid, ring.origin.y + mid, CELL, CELL);
    stroke_rounded(centre, 1.0, &style::black(0.9), 2.5);
    stroke_rounded(
        rect(
            centre.origin.x + 1.0,
            centre.origin.y + 1.0,
            CELL - 2.0,
            CELL - 2.0,
        ),
        0.5,
        &style::white(0.95),
        1.0,
    );
    objc2_app_kit::NSGraphicsContext::restoreGraphicsState_class();
}

struct Session {
    overlays: Vec<Retained<FigPanel>>,
    loupe_panel: Retained<FigPanel>,
}

struct Inner {
    mtm: MainThreadMarker,
    me: Weak<Inner>,
    strings: Strings,
    callbacks: Callbacks,
    source: Source,
    loupe: Retained<Loupe>,
    session: RefCell<Option<Session>>,
    format: Cell<usize>,
    pointer: Cell<NSPoint>,
    grid: RefCell<Option<Grid>>,
    blocked: Cell<bool>,
    last_read: Cell<Option<Instant>>,
    read_scheduled: Cell<bool>,
}

pub struct Picker {
    inner: Rc<Inner>,
}

impl Picker {
    pub fn new(
        mtm: MainThreadMarker,
        strings: Strings,
        callbacks: Callbacks,
        source: Source,
    ) -> Self {
        Self {
            inner: Rc::new_cyclic(|me| Inner {
                mtm,
                me: me.clone(),
                strings,
                callbacks,
                source,
                loupe: Loupe::new(mtm),
                session: RefCell::new(None),
                format: Cell::new(0),
                pointer: Cell::new(NSPoint::new(0.0, 0.0)),
                grid: RefCell::new(None),
                blocked: Cell::new(false),
                last_read: Cell::new(None),
                read_scheduled: Cell::new(false),
            }),
        }
    }

    pub fn is_active(&self) -> bool {
        self.inner.session.borrow().is_some()
    }

    /// The index of the format being shown.
    pub fn format_index(&self) -> usize {
        self.inner.format.get()
    }

    /// Starts picking in the format with this index, with the loupe at `pointer`. Does nothing when it is
    /// already running.
    pub fn start(&self, format: usize, pointer: NSPoint) {
        Inner::start(&self.inner, format, pointer);
    }

    /// Starts picking where the pointer is now.
    pub fn start_at_pointer(&self, format: usize) {
        self.start(format, NSEvent::mouseLocation());
    }

    /// Ends it without copying anything.
    pub fn cancel(&self) {
        self.inner.end();
    }

    /// The loupe as it looks now, as PNG, without showing anything.
    pub fn loupe_png(&self) -> Option<Vec<u8>> {
        let view = &self.inner.loupe;
        let bounds = view.bounds();
        let rep = view.bitmapImageRepForCachingDisplayInRect(bounds)?;
        view.cacheDisplayInRect_toBitmapImageRep(bounds, &rep);
        // SAFETY: an empty properties dictionary is valid for PNG output.
        unsafe {
            rep.representationUsingType_properties(
                objc2_app_kit::NSBitmapImageFileType::PNG,
                &objc2_foundation::NSDictionary::new(),
            )
        }
        .map(|data| data.to_vec())
    }

    /// The panel that catches input on the display at `index`, for checks that send it events.
    pub fn catcher_window(&self, index: usize) -> Option<Retained<objc2_app_kit::NSPanel>> {
        let session = self.inner.session.borrow();
        let panel = session.as_ref()?.overlays.get(index)?;
        Some(Retained::into_super(panel.clone()))
    }

    pub fn loupe_frame(&self) -> Option<NSRect> {
        self.inner
            .session
            .borrow()
            .as_ref()
            .map(|s| s.loupe_panel.frame())
    }
}

impl Inner {
    fn start(this: &Rc<Self>, format: usize, pointer: NSPoint) {
        if this.session.borrow().is_some() {
            return;
        }
        let frames = (this.source.screens)();
        if frames.is_empty() {
            return;
        }
        this.format
            .set(format.min(this.strings.formats.len().saturating_sub(1)));
        this.pointer.set(pointer);
        *this.grid.borrow_mut() = None;
        this.last_read.set(None);
        this.blocked.set(!(this.source.access)());

        let weak = Rc::downgrade(this);
        let on_input: OnInput = Rc::new(move |input| {
            if let Some(this) = weak.upgrade() {
                this.handle(input);
            }
        });
        let overlays: Vec<Retained<FigPanel>> = frames
            .iter()
            .map(|frame| {
                let panel = FigPanel::new(this.mtm, *frame);
                let catcher = Catcher::new(
                    this.mtm,
                    rect(0.0, 0.0, frame.size.width, frame.size.height),
                    Rc::clone(&on_input),
                );
                panel.setContentView(Some(&catcher));
                panel.setAcceptsMouseMovedEvents(true);
                panel.makeFirstResponder(Some(&catcher));
                panel.orderFrontRegardless();
                panel
            })
            .collect();
        let loupe_panel = FigPanel::new(this.mtm, rect(0.0, 0.0, WIDTH, HEIGHT));
        loupe_panel.setContentView(Some(&this.loupe));
        // Clicks must reach the catcher underneath.
        loupe_panel.setIgnoresMouseEvents(true);
        *this.session.borrow_mut() = Some(Session {
            overlays,
            loupe_panel,
        });
        this.refresh_look();
        this.move_loupe(pointer);
        if let Some(session) = this.session.borrow().as_ref() {
            session.loupe_panel.orderFrontRegardless();
        }
        this.key_panel_at(pointer);
        this.read_at(pointer);
    }

    fn handle(&self, input: Input) {
        if self.session.borrow().is_none() {
            return;
        }
        match input {
            Input::Moved(at) => self.moved(at),
            Input::Click => self.click(),
            Input::Cancel => self.end(),
            Input::Format(step) => {
                let n = self.strings.formats.len() as isize;
                if n > 0 {
                    self.format
                        .set((self.format.get() as isize + step).rem_euclid(n) as usize);
                    self.refresh_look();
                }
            }
        }
    }

    fn moved(&self, at: NSPoint) {
        self.pointer.set(at);
        self.move_loupe(at);
        self.key_panel_at(at);
        match self.last_read.get() {
            Some(last) if last.elapsed() < MIN_INTERVAL => {
                if !self.read_scheduled.replace(true) {
                    let wait = (MIN_INTERVAL - last.elapsed()).as_secs_f64();
                    let weak = self.me.clone();
                    app::after(wait, move || {
                        if let Some(this) = weak.upgrade() {
                            this.read_scheduled.set(false);
                            if this.session.borrow().is_some() {
                                this.read_at(this.pointer.get());
                            }
                        }
                    });
                }
            }
            _ => self.read_at(at),
        }
    }

    fn read_at(&self, at: NSPoint) {
        self.last_read.set(Some(Instant::now()));
        let below = self
            .session
            .borrow()
            .as_ref()
            .and_then(|s| s.overlays.first())
            .map_or(0, |p| u32::try_from(p.windowNumber()).unwrap_or(0));
        if !(self.source.access)() {
            self.blocked.set(true);
            *self.grid.borrow_mut() = None;
        } else {
            self.blocked.set(false);
            *self.grid.borrow_mut() = (self.source.grab)(at, CELLS, below);
        }
        self.refresh_look();
    }

    fn refresh_look(&self) {
        let grid = self.grid.borrow().clone();
        let format = self.format.get();
        let blocked = self.blocked.get().then(|| self.strings.no_access.clone());
        let text = grid
            .as_ref()
            .map(|g| (self.callbacks.render)(g.centre(), format))
            .unwrap_or_default();
        self.loupe.show(Look {
            grid,
            text,
            format: self
                .strings
                .formats
                .get(format)
                .cloned()
                .unwrap_or_default(),
            blocked,
            hint: self.strings.hint.clone(),
        });
    }

    fn move_loupe(&self, at: NSPoint) {
        let frames = (self.source.screens)();
        let Some(frame) = frames.iter().find(|f| contains(**f, at)).or(frames.first()) else {
            return;
        };
        if let Some(session) = self.session.borrow().as_ref() {
            let origin = place(at, NSSize::new(WIDTH, HEIGHT), *frame);
            session.loupe_panel.setFrameOrigin(origin);
        }
    }

    /// Keys go to the panel of the display the pointer is on.
    fn key_panel_at(&self, at: NSPoint) {
        let frames = (self.source.screens)();
        let session = self.session.borrow();
        let Some(session) = session.as_ref() else {
            return;
        };
        let Some(index) = frames.iter().position(|f| contains(*f, at)) else {
            return;
        };
        if let Some(panel) = session.overlays.get(index)
            && !panel.isKeyWindow()
        {
            panel.makeKeyWindow();
        }
    }

    fn click(&self) {
        let grid = self.grid.borrow().clone();
        // Nothing to copy without a reading; the click only ends it.
        if let Some(grid) = grid
            && !self.blocked.get()
        {
            (self.callbacks.on_copy)((self.callbacks.render)(grid.centre(), self.format.get()));
        }
        self.end();
    }

    fn end(&self) {
        let Some(session) = self.session.borrow_mut().take() else {
            return;
        };
        // Taking the panels off the screen is what hands the keyboard back to the app that had it: this app
        // was never made active.
        session.loupe_panel.orderOut(None);
        for panel in &session.overlays {
            panel.orderOut(None);
        }
        *self.grid.borrow_mut() = None;
    }
}
