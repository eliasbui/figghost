//! The strip of page icons at the top of the open notch: a capsule with one glyph per page, the current
//! page in white. It replaces a row of text tabs, so any number of pages fit in a small space.

use crate::style::{self, Control, TEXT_SECONDARY, black, white};
use objc2::{
    AnyThread, DefinedClass, MainThreadMarker, MainThreadOnly, Message, define_class, msg_send,
    rc::{Retained, Weak},
    runtime::AnyObject,
    sel,
};
use objc2_app_kit::{NSEvent, NSScreen, NSTrackingArea, NSTrackingAreaOptions, NSView};
use objc2_foundation::{
    NSObject, NSObjectProtocol, NSPoint, NSRect, NSRunLoop, NSRunLoopCommonModes, NSSize, NSString,
};
use objc2_quartz_core::CADisplayLink;
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
};

/// A page's cell is 30 by 24 inside the well, about the 28 by 28 points macOS gives a control to hit.
pub const HEIGHT: f64 = 28.0;
const PAD: f64 = 2.0;
const ITEM_WIDTH: f64 = 30.0;
const GLYPH: f64 = 15.0;

/// The width that fits this many pages.
pub const fn width_for(count: usize) -> f64 {
    2.0 * PAD + count as f64 * ITEM_WIDTH
}

type SelectHandler = Rc<dyn Fn(usize)>;

pub struct SwitcherIvars {
    items: RefCell<Vec<(String, String)>>,
    selected: Cell<usize>,
    /// Where the chosen pill is drawn, which follows `selected` instead of jumping to it.
    shown: Cell<f64>,
    /// The display link that carries the pill, kept only while it is moving.
    glide: RefCell<Option<Retained<CADisplayLink>>>,
    /// Pages with something that needs the user, marked with a dot.
    badges: RefCell<Vec<usize>>,
    hovered: Cell<Option<usize>>,
    on_select: RefCell<Option<SelectHandler>>,
}

pub struct GlideIvars {
    switcher: Weak<PageSwitcher>,
}

// What the display link calls on every frame while the pill travels.
define_class!(
    // SAFETY: NSObject allows subclassing; the ivar is only touched on the main thread.
    #[unsafe(super(NSObject))]
    #[thread_kind = MainThreadOnly]
    #[name = "FigSwitcherGlide"]
    #[ivars = GlideIvars]
    struct Glide;

    impl Glide {
        #[unsafe(method(tick:))]
        fn tick(&self, link: &CADisplayLink) {
            // The last frame lets go of the display link, which lets go of this object: both are kept alive
            // until this call returns.
            let (_alive, _link) = (self.retain(), link.retain());
            if let Some(switcher) = self.ivars().switcher.load() {
                switcher.step();
            }
        }
    }

    unsafe impl NSObjectProtocol for Glide {}
);

impl Glide {
    fn new(mtm: MainThreadMarker, switcher: Weak<PageSwitcher>) -> Retained<Self> {
        let this = mtm.alloc::<Self>().set_ivars(GlideIvars { switcher });
        // SAFETY: NSObject's designated initializer.
        unsafe { msg_send![super(this), init] }
    }
}

define_class!(
    // SAFETY: NSView allows subclassing; the ivars are only touched on the main thread and need no Drop.
    #[unsafe(super(NSView))]
    #[thread_kind = MainThreadOnly]
    #[name = "FigPageSwitcher"]
    #[ivars = SwitcherIvars]
    pub struct PageSwitcher;

    impl PageSwitcher {
        #[unsafe(method(acceptsFirstMouse:))]
        fn accepts_first_mouse(&self, _event: Option<&NSEvent>) -> bool {
            true
        }

        #[unsafe(method(drawRect:))]
        fn draw_rect(&self, _dirty: NSRect) {
            self.paint();
        }

        #[unsafe(method(mouseMoved:))]
        fn mouse_moved(&self, event: &NSEvent) {
            let index = self.index_at(event);
            if self.ivars().hovered.replace(index) != index {
                let title = index.and_then(|i| self.ivars().items.borrow().get(i).map(|(_, t)| t.clone()));
                self.setToolTip(title.as_deref().map(NSString::from_str).as_deref());
                self.setNeedsDisplay(true);
            }
        }

        #[unsafe(method(mouseExited:))]
        fn mouse_exited(&self, _event: &NSEvent) {
            if self.ivars().hovered.replace(None).is_some() {
                self.setNeedsDisplay(true);
            }
        }

        #[unsafe(method(mouseDown:))]
        fn mouse_down(&self, event: &NSEvent) {
            if let Some(index) = self.index_at(event) {
                let select = self.ivars().on_select.borrow().clone();
                if let Some(select) = select {
                    select(index);
                }
            }
        }
    }

    unsafe impl NSObjectProtocol for PageSwitcher {}
);

impl PageSwitcher {
    pub fn new(mtm: MainThreadMarker, origin: NSPoint) -> Retained<Self> {
        let ivars = SwitcherIvars {
            items: RefCell::new(Vec::new()),
            selected: Cell::new(0),
            shown: Cell::new(0.0),
            glide: RefCell::new(None),
            badges: RefCell::new(Vec::new()),
            hovered: Cell::new(None),
            on_select: RefCell::new(None),
        };
        let frame = NSRect::new(origin, NSSize::new(width_for(0), HEIGHT));
        // SAFETY: designated initializer with valid arguments on the main thread.
        let this: Retained<Self> =
            unsafe { msg_send![super(mtm.alloc::<Self>().set_ivars(ivars)), initWithFrame: frame] };
        // SAFETY: valid options and owner; InVisibleRect ignores the (empty) rect.
        let area = unsafe {
            NSTrackingArea::initWithRect_options_owner_userInfo(
                NSTrackingArea::alloc(),
                NSRect::ZERO,
                NSTrackingAreaOptions::MouseMoved
                    | NSTrackingAreaOptions::MouseEnteredAndExited
                    | NSTrackingAreaOptions::ActiveAlways
                    | NSTrackingAreaOptions::InVisibleRect,
                Some(&this),
                None,
            )
        };
        this.addTrackingArea(&area);
        this
    }

    pub fn set_on_select(&self, f: impl Fn(usize) + 'static) {
        *self.ivars().on_select.borrow_mut() = Some(Rc::new(f));
    }

    /// One `(SF Symbol name, title)` per page. The strip resizes to fit them.
    pub fn set_items(&self, items: Vec<(String, String)>) {
        let width = width_for(items.len());
        *self.ivars().items.borrow_mut() = items;
        self.setFrameSize(NSSize::new(width, HEIGHT));
        self.setNeedsDisplay(true);
    }

    pub fn set_selected(&self, index: usize) {
        self.ivars().selected.set(index);
        self.start_glide();
    }

    /// Puts the pill on the chosen page at once, for a frame that must not catch it in flight: a snapshot.
    pub fn settle(&self) {
        self.ivars().shown.set(self.ivars().selected.get() as f64);
        if let Some(link) = self.ivars().glide.borrow_mut().take() {
            link.invalidate();
        }
        self.setNeedsDisplay(true);
    }

    /// Carries the pill to the page the user chose: one step of an exponential approach per frame, which
    /// starts fast and settles without arriving early, the way a liquid glass control follows a finger.
    pub fn step(&self) {
        let iv = self.ivars();
        let toward = iv.selected.get() as f64;
        let from = iv.shown.get();
        let left = toward - from;
        if left.abs() < 0.004 {
            iv.shown.set(toward);
            // A run loop keeps a display link that is only released, and the link keeps this view: it has to be
            // invalidated to stop the frames.
            if let Some(link) = iv.glide.borrow_mut().take() {
                link.invalidate();
            }
        } else {
            iv.shown.set(from + left * 0.28);
        }
        self.setNeedsDisplay(true);
    }

    fn start_glide(&self) {
        if self.ivars().glide.borrow().is_some() {
            self.setNeedsDisplay(true);
            return;
        }
        // The screen's display link keeps the pill in step with the frames the window server draws, and lets go
        // of the run loop the moment it settles. macOS 13 has none: there the pill arrives at once.
        let mtm = MainThreadMarker::new().expect("main thread");
        let screen = self
            .window()
            .and_then(|window| window.screen())
            .or_else(|| NSScreen::mainScreen(mtm));
        let Some(screen) =
            screen.filter(|s| s.respondsToSelector(sel!(displayLinkWithTarget:selector:)))
        else {
            self.ivars().shown.set(self.ivars().selected.get() as f64);
            self.setNeedsDisplay(true);
            return;
        };
        let target = Glide::new(mtm, Weak::from_retained(&self.retain()));
        // SAFETY: the target answers `tick:`, taking the display link, which is what the selector names.
        let link = unsafe {
            screen.displayLinkWithTarget_selector(AsRef::<AnyObject>::as_ref(&target), sel!(tick:))
        };
        // SAFETY: the main run loop, in the common modes so frames also come while the pointer is held down.
        unsafe { link.addToRunLoop_forMode(&NSRunLoop::mainRunLoop(), NSRunLoopCommonModes) };
        self.ivars().glide.borrow_mut().replace(link);
        self.setNeedsDisplay(true);
    }

    /// Marks a page icon with a dot, or clears the mark.
    pub fn set_badge(&self, index: usize, on: bool) {
        let mut badges = self.ivars().badges.borrow_mut();
        badges.retain(|&i| i != index);
        if on {
            badges.push(index);
        }
        drop(badges);
        self.setNeedsDisplay(true);
    }

    fn index_at(&self, event: &NSEvent) -> Option<usize> {
        let point = self.convertPoint_fromView(event.locationInWindow(), None);
        let relative = point.x - PAD;
        if relative < 0.0 || point.y < 0.0 || point.y > HEIGHT {
            return None;
        }
        let index = (relative / ITEM_WIDTH) as usize;
        (index < self.ivars().items.borrow().len()).then_some(index)
    }

    fn paint(&self) {
        let bounds = self.bounds();
        // A well holds the row of pages; the pill that marks the chosen one is the only thing that stands out of it.
        style::well(bounds, HEIGHT / 2.0);
        let items = self.ivars().items.borrow();
        let (selected, hovered) = (self.ivars().selected.get(), self.ivars().hovered.get());
        let shown = self.ivars().shown.get();
        let pill = NSRect::new(
            NSPoint::new(PAD + shown * ITEM_WIDTH, PAD),
            NSSize::new(ITEM_WIDTH, HEIGHT - 2.0 * PAD),
        );
        style::card_bright(pill, pill.size.height / 2.0);
        for (i, (symbol, _)) in items.iter().enumerate() {
            let cell = NSRect::new(
                NSPoint::new(PAD + i as f64 * ITEM_WIDTH, PAD),
                NSSize::new(ITEM_WIDTH, HEIGHT - 2.0 * PAD),
            );
            let on = i == selected;
            if !on && hovered == Some(i) {
                style::control(cell, cell.size.height / 2.0, Control::Hover);
            }
            let ink = if on {
                black(0.88)
            } else {
                white(TEXT_SECONDARY + 0.12)
            };
            style::draw_symbol(
                symbol,
                NSPoint::new(
                    cell.origin.x + cell.size.width / 2.0,
                    cell.origin.y + cell.size.height / 2.0,
                ),
                GLYPH,
                &ink,
            );
            if self.ivars().badges.borrow().contains(&i) {
                let dot = NSRect::new(
                    NSPoint::new(
                        cell.origin.x + cell.size.width - 9.0,
                        cell.origin.y + cell.size.height - 7.5,
                    ),
                    NSSize::new(6.0, 6.0),
                );
                style::fill_circle(dot, &style::Tone::Orange.color());
            }
        }
    }
}

/// The capsule with three dots at the left of the header: a menu for the things that are not a page. Hovering it
/// spreads the dots and lifts the capsule, pressing it hands the click to its owner, which shows the menu.
pub const ORB_WIDTH: f64 = 46.0;

type OrbClick = Rc<dyn Fn(&NSView, &NSEvent)>;

pub struct OrbIvars {
    hovered: Cell<bool>,
    pressed: Cell<bool>,
    on_click: RefCell<Option<OrbClick>>,
}

define_class!(
    // SAFETY: NSView allows subclassing; the ivars are only touched on the main thread and need no Drop.
    #[unsafe(super(NSView))]
    #[thread_kind = MainThreadOnly]
    #[name = "FigNotchOrb"]
    #[ivars = OrbIvars]
    pub struct NotchOrb;

    impl NotchOrb {
        #[unsafe(method(acceptsFirstMouse:))]
        fn accepts_first_mouse(&self, _event: Option<&NSEvent>) -> bool {
            true
        }

        #[unsafe(method(drawRect:))]
        fn draw_rect(&self, _dirty: NSRect) {
            self.paint();
        }

        #[unsafe(method(mouseEntered:))]
        fn mouse_entered(&self, _event: &NSEvent) {
            self.ivars().hovered.set(true);
            self.setNeedsDisplay(true);
        }

        #[unsafe(method(mouseExited:))]
        fn mouse_exited(&self, _event: &NSEvent) {
            self.ivars().hovered.set(false);
            self.setNeedsDisplay(true);
        }

        #[unsafe(method(mouseDown:))]
        fn mouse_down(&self, event: &NSEvent) {
            self.ivars().pressed.set(true);
            self.setNeedsDisplay(true);
            let click = self.ivars().on_click.borrow().clone();
            if let Some(click) = click {
                // The menu runs its own event loop, so the press is over by the time this returns.
                click(self, event);
            }
            self.ivars().pressed.set(false);
            self.ivars().hovered.set(false);
            self.setNeedsDisplay(true);
        }
    }

    unsafe impl NSObjectProtocol for NotchOrb {}
);

impl NotchOrb {
    pub fn new(mtm: MainThreadMarker, origin: NSPoint) -> Retained<Self> {
        let ivars = OrbIvars {
            hovered: Cell::new(false),
            pressed: Cell::new(false),
            on_click: RefCell::new(None),
        };
        let frame = NSRect::new(origin, NSSize::new(ORB_WIDTH, HEIGHT));
        // SAFETY: designated initializer with valid arguments on the main thread.
        let this: Retained<Self> =
            unsafe { msg_send![super(mtm.alloc::<Self>().set_ivars(ivars)), initWithFrame: frame] };
        // SAFETY: valid options and owner; InVisibleRect ignores the (empty) rect.
        let area = unsafe {
            NSTrackingArea::initWithRect_options_owner_userInfo(
                NSTrackingArea::alloc(),
                NSRect::ZERO,
                NSTrackingAreaOptions::MouseEnteredAndExited
                    | NSTrackingAreaOptions::ActiveAlways
                    | NSTrackingAreaOptions::InVisibleRect,
                Some(&this),
                None,
            )
        };
        this.addTrackingArea(&area);
        this
    }

    pub fn set_on_click(&self, f: impl Fn(&NSView, &NSEvent) + 'static) {
        *self.ivars().on_click.borrow_mut() = Some(Rc::new(f));
    }

    fn paint(&self) {
        let bounds = self.bounds();
        let (hovered, pressed) = (self.ivars().hovered.get(), self.ivars().pressed.get());
        let state = if pressed {
            Control::Pressed
        } else if hovered {
            Control::Hover
        } else {
            Control::Rest
        };
        style::well(bounds, HEIGHT / 2.0);
        if hovered || pressed {
            style::control(bounds, HEIGHT / 2.0, state);
        }
        let (dot, gap) = (4.0, if hovered { 7.5 } else { 6.5 });
        let span = 3.0 * dot + 2.0 * gap;
        let ink = white(if hovered { 0.92 } else { TEXT_SECONDARY });
        for i in 0..3 {
            let x = bounds.size.width / 2.0 - span / 2.0 + f64::from(i) * (dot + gap);
            style::fill_circle(
                NSRect::new(
                    NSPoint::new(x, bounds.size.height / 2.0 - dot / 2.0),
                    NSSize::new(dot, dot),
                ),
                &ink,
            );
        }
    }
}
