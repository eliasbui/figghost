//! The overlay on which a region or a window is chosen: a dimmed panel over every display, a crosshair, the
//! selection with a readout of its size, and the keys. The gestures are `core-capture`'s `Selector`; this
//! file turns events into its input and draws what it reports.
//!
//! The panels never activate the app, so the app that was in front stays in front; they take the keyboard
//! (for Return, Escape and Space) only while the overlay is up and give it back when they are ordered out.

use crate::{
    MainThreadMarker, app,
    panel::FigPanel,
    style::{self, Weight},
    widgets::{Fit, draw_text, text_attributes},
};
use core_capture::{
    Display, Pick, Point, Rect, WindowBox, display_index_at,
    select::{Event, Outcome, Selector, size_text},
    window_at,
};
use objc2::{DefinedClass, MainThreadOnly, define_class, msg_send, rc::Retained};
use objc2_app_kit::{
    NSBezierPath, NSColor, NSCompositingOperation, NSCursor, NSEvent, NSGraphicsContext,
    NSMainMenuWindowLevel, NSResponder, NSStringDrawing, NSTrackingArea, NSTrackingAreaOptions,
    NSView, NSWindowSharingType,
};
use objc2_foundation::{NSObjectProtocol, NSPoint, NSRect, NSSize, NSString};
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
};

/// How dark everything but the selection becomes.
const DIM: f64 = 0.40;
/// The selection itself is not quite clear: a window that is fully transparent under the pointer lets the
/// click through to whatever is below it.
const SELECTION_ALPHA: f64 = 0.02;
const READOUT_HEIGHT: f64 = 22.0;
const READOUT_GAP: f64 = 6.0;
const HINT_HEIGHT: f64 = 28.0;
/// Room around the selection that has to be redrawn when it changes: the border and the readout.
const DAMAGE_MARGIN: f64 = 44.0;
const KEY_ESCAPE: u16 = 53;
const KEY_RETURN: u16 = 36;
const KEY_ENTER: u16 = 76;
const KEY_SPACE: u16 = 49;

pub enum Mode {
    Region,
    /// The windows that can be chosen, from the front to the back.
    Window(Vec<WindowBox>),
}

/// What the overlay says, in the language of the app.
pub struct Strings {
    pub hint_region: String,
    pub hint_window: String,
}

struct Session {
    mode: Mode,
    displays: Vec<Display>,
    strings: Strings,
    selector: RefCell<Selector>,
    hover_window: Cell<Option<usize>>,
    pointer: Cell<Point>,
    /// The mouse went down in window mode, so that the release is a click.
    pressed: Cell<bool>,
    /// Set by `take_key`: only then does a click on another display's panel move the keyboard to it.
    keyboard: Cell<bool>,
    panels: RefCell<Vec<Retained<FigPanel>>>,
    views: RefCell<Vec<Retained<CaptureView>>>,
    done: RefCell<Option<Answer>>,
    finished: Cell<bool>,
}

type Answer = Box<dyn FnOnce(Pick)>;

/// An area of a display that has to be drawn again: (display index, rectangle in global points).
type Damage = Vec<(usize, Rect)>;

impl Session {
    fn pointer_display(&self) -> Option<usize> {
        display_index_at(&self.displays, self.pointer.get())
    }

    fn inflated(rect: Rect) -> Rect {
        Rect::new(
            rect.x - DAMAGE_MARGIN,
            rect.y - DAMAGE_MARGIN,
            rect.w + 2.0 * DAMAGE_MARGIN,
            rect.h + 2.0 * DAMAGE_MARGIN,
        )
    }

    /// Where the current state draws anything but the dim, so that a change redraws only that.
    fn damage(&self) -> Damage {
        let mut out = Vec::new();
        match &self.mode {
            Mode::Region => {
                if let Some((display, rect)) = self.selector.borrow().selection() {
                    out.push((display, Self::inflated(rect)));
                } else if let Some(display) = self.pointer_display() {
                    let frame = self.displays[display].frame;
                    let at = self.pointer.get();
                    out.push((display, Rect::new(at.x - 2.0, frame.y, 4.0, frame.h)));
                    out.push((display, Rect::new(frame.x, at.y - 2.0, frame.w, 4.0)));
                    out.push((display, self.hint_rect(frame)));
                }
            }
            Mode::Window(windows) => {
                if let Some(window) = self.hover_window.get().and_then(|i| windows.get(i)) {
                    for (i, display) in self.displays.iter().enumerate() {
                        if let Some(part) =
                            Self::inflated(window.frame).intersection(&display.frame)
                        {
                            out.push((i, part));
                        }
                    }
                }
                if let Some(display) = self.pointer_display() {
                    out.push((display, self.hint_rect(self.displays[display].frame)));
                }
            }
        }
        out
    }

    fn invalidate(&self, damage: &Damage) {
        let views = self.views.borrow();
        for (display, rect) in damage {
            let (Some(view), Some(display)) = (views.get(*display), self.displays.get(*display))
            else {
                continue;
            };
            view.setNeedsDisplayInRect(local(*rect, &display.frame));
        }
    }

    /// Changes the state with `change` and redraws what it touched, before and after.
    fn update(&self, change: impl FnOnce()) {
        let before = self.damage();
        change();
        self.invalidate(&before);
        self.invalidate(&self.damage());
    }

    fn hint(&self) -> &str {
        match self.mode {
            Mode::Region => &self.strings.hint_region,
            Mode::Window(_) => &self.strings.hint_window,
        }
    }

    /// The hint's pill: centred near the bottom of the display, wide enough for its text.
    fn hint_rect(&self, frame: Rect) -> Rect {
        let width = text_width(self.hint(), 12.0) + 28.0;
        Rect::new(
            frame.x + (frame.w - width) / 2.0,
            frame.y + 64.0,
            width,
            HINT_HEIGHT,
        )
    }

    fn feed(self: &Rc<Self>, event: Event) {
        if self.finished.get() {
            return;
        }
        let mut outcome = None;
        self.update(|| {
            if let Event::Move(p) | Event::Down(p) | Event::Drag(p) | Event::Up(p) = event {
                self.pointer.set(p);
            }
            outcome = self.selector.borrow_mut().handle(event);
        });
        match outcome {
            Some(Outcome::Selected { display, rect }) => self.finish(Pick::Region {
                display: self.displays[display],
                rect,
            }),
            Some(Outcome::Cancelled) => self.finish(Pick::Cancelled),
            None => {}
        }
    }

    fn pointer_moved(self: &Rc<Self>, p: Point) {
        match &self.mode {
            Mode::Region => self.feed(Event::Move(p)),
            Mode::Window(windows) => {
                let hit = window_at(windows, p);
                self.update(|| {
                    self.pointer.set(p);
                    self.hover_window.set(hit);
                });
            }
        }
    }

    fn pressed_at(self: &Rc<Self>, p: Point) {
        match self.mode {
            Mode::Region => self.feed(Event::Down(p)),
            Mode::Window(_) => {
                self.pointer_moved(p);
                self.pressed.set(true);
            }
        }
    }

    fn dragged_to(self: &Rc<Self>, p: Point) {
        match self.mode {
            Mode::Region => self.feed(Event::Drag(p)),
            Mode::Window(_) => self.pointer_moved(p),
        }
    }

    fn released_at(self: &Rc<Self>, p: Point) {
        match &self.mode {
            Mode::Region => self.feed(Event::Up(p)),
            Mode::Window(_) => {
                if self.pressed.replace(false) {
                    self.pointer_moved(p);
                    self.choose_window();
                }
            }
        }
    }

    fn choose_window(self: &Rc<Self>) {
        if let Mode::Window(windows) = &self.mode
            && let Some(window) = self.hover_window.get().and_then(|i| windows.get(i))
        {
            self.finish(Pick::Window(window.clone()));
        }
    }

    fn key(self: &Rc<Self>, code: u16, down: bool, repeat: bool) {
        match (code, down) {
            (KEY_ESCAPE, true) => self.feed_any(Event::Cancel),
            (KEY_RETURN | KEY_ENTER, true) if !repeat => match self.mode {
                Mode::Region => self.feed(Event::Confirm),
                Mode::Window(_) => self.choose_window(),
            },
            (KEY_SPACE, _) if !repeat => self.feed(Event::Space(down)),
            _ => {}
        }
    }

    /// Cancelling works in both modes, and in window mode there is no selector to ask.
    fn feed_any(self: &Rc<Self>, event: Event) {
        match (&self.mode, event) {
            (Mode::Window(_), Event::Cancel) => self.finish(Pick::Cancelled),
            _ => self.feed(event),
        }
    }

    /// Takes the panels off the screen at once, and a moment later lets go of them and reports: the event
    /// that ended the overlay is still being handled by one of its views.
    fn finish(self: &Rc<Self>, pick: Pick) {
        if self.finished.replace(true) {
            return;
        }
        for panel in self.panels.borrow().iter() {
            panel.orderOut(None);
        }
        // Whatever is under the pointer now sets its own cursor only when the pointer moves.
        NSCursor::arrowCursor().set();
        let session = Rc::clone(self);
        app::after(0.0, move || {
            session.panels.borrow_mut().clear();
            // The views hold the session, which holds the views.
            session.views.borrow_mut().clear();
            if let Some(done) = session.done.borrow_mut().take() {
                done(pick);
            }
        });
    }
}

/// The rectangle in the coordinates of the display's view.
fn local(rect: Rect, frame: &Rect) -> NSRect {
    NSRect::new(
        NSPoint::new(rect.x - frame.x, rect.y - frame.y),
        NSSize::new(rect.w, rect.h),
    )
}

fn text_width(text: &str, size: f64) -> f64 {
    let attributes = text_attributes(
        &style::font(size, Weight::Medium),
        &NSColor::whiteColor(),
        Fit::Clip,
        false,
    );
    // SAFETY: `attributes` holds valid attribute objects for their keys.
    unsafe { NSString::from_str(text).sizeWithAttributes(Some(&attributes)) }
        .width
        .ceil()
}

pub struct ViewIvars {
    session: Rc<Session>,
    index: usize,
}

define_class!(
    // SAFETY: NSView allows subclassing; the ivars are only touched on the main thread and need no Drop.
    #[unsafe(super(NSView))]
    #[thread_kind = MainThreadOnly]
    #[name = "FigCaptureView"]
    #[ivars = ViewIvars]
    struct CaptureView;

    unsafe impl NSObjectProtocol for CaptureView {}

    impl CaptureView {
        #[unsafe(method(isOpaque))]
        fn is_opaque(&self) -> bool {
            false
        }

        #[unsafe(method(acceptsFirstResponder))]
        fn accepts_first_responder(&self) -> bool {
            true
        }

        #[unsafe(method(acceptsFirstMouse:))]
        fn accepts_first_mouse(&self, _event: Option<&NSEvent>) -> bool {
            true
        }

        #[unsafe(method(needsPanelToBecomeKey))]
        fn needs_panel_to_become_key(&self) -> bool {
            true
        }

        #[unsafe(method(mouseMoved:))]
        fn mouse_moved(&self, event: &NSEvent) {
            self.set_cursor();
            self.ivars().session.pointer_moved(self.global(event));
        }

        #[unsafe(method(mouseEntered:))]
        fn mouse_entered(&self, event: &NSEvent) {
            self.set_cursor();
            self.ivars().session.pointer_moved(self.global(event));
        }

        #[unsafe(method(mouseDown:))]
        fn mouse_down(&self, event: &NSEvent) {
            self.set_cursor();
            if self.ivars().session.keyboard.get()
                && let Some(window) = self.window()
            {
                window.makeKeyWindow();
            }
            self.ivars().session.pressed_at(self.global(event));
        }

        // A way out for anyone whose keyboard did not reach the overlay.
        #[unsafe(method(rightMouseDown:))]
        fn right_mouse_down(&self, _event: &NSEvent) {
            self.ivars().session.feed_any(Event::Cancel);
        }

        #[unsafe(method(mouseDragged:))]
        fn mouse_dragged(&self, event: &NSEvent) {
            self.ivars().session.dragged_to(self.global(event));
        }

        #[unsafe(method(mouseUp:))]
        fn mouse_up(&self, event: &NSEvent) {
            self.ivars().session.released_at(self.global(event));
        }

        #[unsafe(method(keyDown:))]
        fn key_down(&self, event: &NSEvent) {
            self.ivars()
                .session
                .key(event.keyCode(), true, event.isARepeat());
        }

        #[unsafe(method(keyUp:))]
        fn key_up(&self, event: &NSEvent) {
            self.ivars().session.key(event.keyCode(), false, false);
        }

        #[unsafe(method(drawRect:))]
        fn draw_rect(&self, dirty: NSRect) {
            let ivars = self.ivars();
            draw(&ivars.session, ivars.index, dirty, self.bounds());
        }
    }
);

impl CaptureView {
    fn new(
        mtm: MainThreadMarker,
        frame: NSRect,
        session: Rc<Session>,
        index: usize,
    ) -> Retained<Self> {
        let this = mtm.alloc::<Self>().set_ivars(ViewIvars { session, index });
        // SAFETY: designated initializer with valid arguments on the main thread.
        let view: Retained<Self> = unsafe { msg_send![super(this), initWithFrame: frame] };
        // Mouse moves arrive while the app is inactive, over the whole view whatever its size becomes.
        // SAFETY: the owner is the view itself, which outlives its tracking areas, and there is no user info.
        let tracking = unsafe {
            NSTrackingArea::initWithRect_options_owner_userInfo(
                mtm.alloc(),
                NSRect::ZERO,
                NSTrackingAreaOptions::MouseMoved
                    | NSTrackingAreaOptions::MouseEnteredAndExited
                    | NSTrackingAreaOptions::ActiveAlways
                    | NSTrackingAreaOptions::InVisibleRect,
                Some(&view),
                None,
            )
        };
        view.addTrackingArea(&tracking);
        view
    }

    /// The event's position in global points. Events that carry no window are in screen space already.
    fn global(&self, event: &NSEvent) -> Point {
        match event.window(MainThreadMarker::from(self)) {
            Some(window) => {
                let at = event.locationInWindow();
                let origin = window.frame().origin;
                Point::new(origin.x + at.x, origin.y + at.y)
            }
            None => {
                let at = NSEvent::mouseLocation();
                Point::new(at.x, at.y)
            }
        }
    }

    fn set_cursor(&self) {
        match self.ivars().session.mode {
            Mode::Region => NSCursor::crosshairCursor().set(),
            Mode::Window(_) => NSCursor::arrowCursor().set(),
        }
    }
}

fn pill(rect: NSRect, text: &str, size: f64) {
    style::fill_capsule(rect, &style::black(0.78));
    style::stroke_rounded(rect, rect.size.height / 2.0, &style::white(0.18), 1.0);
    let attributes = text_attributes(
        &style::digits(size, Weight::Medium),
        &style::white(style::TEXT),
        Fit::Tail,
        true,
    );
    let line = NSRect::new(
        NSPoint::new(
            rect.origin.x + 8.0,
            rect.origin.y + (rect.size.height - size - 4.0) / 2.0,
        ),
        NSSize::new(rect.size.width - 16.0, size + 4.0),
    );
    draw_text(text, line, &attributes);
}

/// Everything in `dirty` of one display's view: the dim first, then what the state shows over it.
fn draw(session: &Session, index: usize, dirty: NSRect, bounds: NSRect) {
    let frame = session.displays[index].frame;
    // Copy, not blend: a rectangle that is drawn again must come out the same, however often it is.
    if let Some(context) = NSGraphicsContext::currentContext() {
        context.setCompositingOperation(NSCompositingOperation::Copy);
        style::black(DIM).setFill();
        NSBezierPath::fillRect(dirty);
        context.setCompositingOperation(NSCompositingOperation::SourceOver);
    }

    let cut_out = |rect: NSRect| {
        if let Some(context) = NSGraphicsContext::currentContext() {
            context.setCompositingOperation(NSCompositingOperation::Copy);
            style::white(SELECTION_ALPHA).setFill();
            NSBezierPath::fillRect(rect);
            context.setCompositingOperation(NSCompositingOperation::SourceOver);
        }
    };
    let outline = |rect: NSRect, width: f64| {
        style::stroke_rounded(rect, 0.0, &style::black(0.45), width + 2.0);
        style::stroke_rounded(rect, 0.0, &style::white(0.95), width);
    };

    match &session.mode {
        Mode::Region => {
            if let Some((display, rect)) = session.selector.borrow().selection() {
                if display != index {
                    return;
                }
                let rect_local = local(rect, &frame);
                cut_out(rect_local);
                outline(rect_local, 1.0);
                let text = size_text(&rect);
                let width = text_width(&text, 12.0) + 22.0;
                let below = rect_local.origin.y - READOUT_GAP - READOUT_HEIGHT;
                let y = if below >= 4.0 {
                    below
                } else {
                    // No room under the selection: inside it at the top, or above it.
                    (rect_local.origin.y + rect_local.size.height + READOUT_GAP)
                        .min(bounds.size.height - READOUT_HEIGHT - 4.0)
                };
                let x = rect_local
                    .origin
                    .x
                    .min(bounds.size.width - width - 4.0)
                    .max(4.0);
                pill(
                    NSRect::new(NSPoint::new(x, y), NSSize::new(width, READOUT_HEIGHT)),
                    &text,
                    12.0,
                );
            } else if session.pointer_display() == Some(index) {
                // The crosshair: two thin lines through the pointer, light with a dark edge.
                let at = session.pointer.get();
                let (x, y) = (at.x - frame.x, at.y - frame.y);
                for (color, width) in [(style::black(0.35), 3.0), (style::white(0.75), 1.0)] {
                    color.setFill();
                    NSBezierPath::fillRect(NSRect::new(
                        NSPoint::new(x - width / 2.0, 0.0),
                        NSSize::new(width, bounds.size.height),
                    ));
                    NSBezierPath::fillRect(NSRect::new(
                        NSPoint::new(0.0, y - width / 2.0),
                        NSSize::new(bounds.size.width, width),
                    ));
                }
                let hint = session.hint_rect(frame);
                pill(local(hint, &frame), session.hint(), 12.0);
            }
        }
        Mode::Window(windows) => {
            if let Some(window) = session.hover_window.get().and_then(|i| windows.get(i))
                && let Some(part) = window.frame.intersection(&frame)
            {
                let rect = local(part, &frame);
                cut_out(rect);
                outline(rect, 3.0);
                let label = if window.title.is_empty() {
                    window.owner.clone()
                } else {
                    format!("{} - {}", window.owner, window.title)
                };
                let width = (text_width(&label, 12.0) + 22.0).min(rect.size.width.max(120.0));
                let y = (rect.origin.y + rect.size.height + READOUT_GAP)
                    .min(bounds.size.height - READOUT_HEIGHT - 4.0);
                let x = rect.origin.x.min(bounds.size.width - width - 4.0).max(4.0);
                pill(
                    NSRect::new(NSPoint::new(x, y), NSSize::new(width, READOUT_HEIGHT)),
                    &label,
                    12.0,
                );
            }
            if session.pointer_display() == Some(index) && session.hover_window.get().is_none() {
                let hint = session.hint_rect(frame);
                pill(local(hint, &frame), session.hint(), 12.0);
            }
        }
    }
}

/// The overlay while it is up. Dropping it takes it down without an answer.
pub struct Overlay {
    session: Rc<Session>,
}

impl Overlay {
    /// Puts a panel over every display and starts listening; `on_done` is called once, on the main thread,
    /// after the panels are gone. The keyboard is not taken until `take_key`.
    pub fn show(
        mtm: MainThreadMarker,
        mode: Mode,
        displays: Vec<Display>,
        pointer: Point,
        strings: Strings,
        on_done: impl FnOnce(Pick) + 'static,
    ) -> Overlay {
        let selector = Selector::new(displays.clone());
        let session = Rc::new(Session {
            mode,
            displays,
            strings,
            selector: RefCell::new(selector),
            hover_window: Cell::new(None),
            pointer: Cell::new(pointer),
            pressed: Cell::new(false),
            keyboard: Cell::new(false),
            panels: RefCell::default(),
            views: RefCell::default(),
            done: RefCell::new(Some(Box::new(on_done))),
            finished: Cell::new(false),
        });
        if let Mode::Window(windows) = &session.mode {
            session.hover_window.set(window_at(windows, pointer));
        }
        session.selector.borrow_mut().handle(Event::Move(pointer));
        for (index, display) in session.displays.iter().enumerate() {
            let frame = NSRect::new(
                NSPoint::new(display.frame.x, display.frame.y),
                NSSize::new(display.frame.w, display.frame.h),
            );
            let panel = FigPanel::new(mtm, frame);
            panel.setLevel(NSMainMenuWindowLevel + 2);
            panel.setAcceptsMouseMovedEvents(true);
            // Where the system honours it, the overlay is also left out of any capture.
            panel.setSharingType(NSWindowSharingType::None);
            let view = CaptureView::new(
                mtm,
                NSRect::new(NSPoint::ZERO, frame.size),
                Rc::clone(&session),
                index,
            );
            panel.setContentView(Some(&view));
            let responder: &NSResponder = &view;
            panel.makeFirstResponder(Some(responder));
            panel.orderFrontRegardless();
            session.panels.borrow_mut().push(panel);
            session.views.borrow_mut().push(view);
        }
        Overlay { session }
    }

    /// Makes the panel under the pointer the key window, so that Return, Escape and Space reach it. The app
    /// does not become the active one.
    pub fn take_key(&self) {
        self.session.keyboard.set(true);
        if let Some(display) = self.session.pointer_display()
            && let Some(panel) = self.session.panels.borrow().get(display)
        {
            panel.makeKeyAndOrderFront(None);
        }
    }

    /// The window numbers of the panels, to leave them out of a capture.
    pub fn window_numbers(&self) -> Vec<u32> {
        self.session
            .panels
            .borrow()
            .iter()
            .filter_map(|p| u32::try_from(p.windowNumber()).ok())
            .collect()
    }

    /// Ends the overlay as if Escape had been pressed.
    pub fn cancel(&self) {
        self.session.feed_any(Event::Cancel);
    }

    /// The panel and the view of a display, for the checks that deliver events to them.
    pub fn panel(&self, display: usize) -> Option<Retained<FigPanel>> {
        self.session.panels.borrow().get(display).cloned()
    }

    pub fn view(&self, display: usize) -> Option<Retained<NSView>> {
        self.session
            .views
            .borrow()
            .get(display)
            .map(|v| Retained::into_super(v.clone()))
    }
}

impl Drop for Overlay {
    fn drop(&mut self) {
        if !self.session.finished.replace(true) {
            for panel in self.session.panels.borrow().iter() {
                panel.orderOut(None);
            }
            self.session.panels.borrow_mut().clear();
            self.session.views.borrow_mut().clear();
            self.session.done.borrow_mut().take();
        }
    }
}
