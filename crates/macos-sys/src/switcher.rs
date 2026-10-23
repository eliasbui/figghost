//! The strip of page icons at the top of the open notch: a capsule with one glyph per page, the current
//! page in white. It replaces a row of text tabs, so any number of pages fit in a small space.
//!
//! A click chooses a page at once. Holding the button, or dragging, lifts the white pill into a glass lens that
//! trails the pointer across the strip; letting go chooses the page under the lens, the way the tab bar of
//! macOS 26 does.

use crate::style::{self, Control, TEXT_SECONDARY, black, white};
use objc2::{
    AnyThread, DefinedClass, MainThreadMarker, MainThreadOnly, Message, define_class, msg_send,
    rc::{Retained, Weak},
    runtime::{AnyClass, AnyObject},
    sel,
};
use objc2_app_kit::{
    NSAutoresizingMaskOptions, NSEvent, NSGlassEffectView, NSGlassEffectViewStyle, NSScreen,
    NSTrackingArea, NSTrackingAreaOptions, NSView, NSWindowOrderingMode,
};
use objc2_core_graphics::{CGEventSource, CGEventSourceStateID};
use objc2_foundation::{
    NSObject, NSObjectProtocol, NSPoint, NSRect, NSRunLoop, NSRunLoopCommonModes, NSSize, NSString,
};
use objc2_quartz_core::CADisplayLink;
use std::{
    cell::{Cell, OnceCell, RefCell},
    rc::Rc,
    time::{Duration, Instant},
};

/// A page's cell is 30 by 24 inside the well, about the 28 by 28 points macOS gives a control to hit.
pub const HEIGHT: f64 = 28.0;
const PAD: f64 = 2.0;
const ITEM_WIDTH: f64 = 30.0;
const GLYPH: f64 = 15.0;
/// How long the button has to stay down on a page before the pill lifts without the pointer moving: longer than
/// a click, short enough to feel like a press.
const HOLD: Duration = Duration::from_millis(200);
/// How far the pointer moves with the button down before the pill lifts at once, so a shaky click stays a click.
const SLOP: f64 = 3.0;
/// How much wider and taller the lifted lens is than the pill: it fills the strip's height and stands out of the
/// cell on both sides.
const LIFT_GROW: (f64, f64) = (8.0, 4.0);
/// The `Escape` key code, which puts a lifted lens back.
const ESCAPE: u16 = 53;

/// A damped spring towards a target: `omega` is how stiff it is (radians a second) and `zeta` how damped, where 1
/// arrives without overshooting and less bounces.
#[derive(Debug, Clone, Copy)]
struct Spring {
    omega: f64,
    zeta: f64,
}

/// A click: critically damped, so the pill leaves gently, never passes its page and is within half a point of a
/// cell three away in about 320 ms (90 percent of the way
/// in 150 ms, where the old fixed fraction a frame took 120 ms and started at full speed).
const GLIDE: Spring = Spring {
    omega: 26.0,
    zeta: 1.0,
};
/// The lens under a dragging pointer and its settling after: soft, so it trails the pointer and comes to rest
/// slowly, with less than one percent of overshoot, which reads as liquid rather than as a bounce.
const FOLLOW: Spring = Spring {
    omega: 11.0,
    zeta: 0.85,
};
/// The lift itself: a little bounce as the pill swells into the lens.
const LIFT: Spring = Spring {
    omega: 18.0,
    zeta: 0.7,
};

/// One frame of a spring, in steps of at most 1/240 of a second so a slow frame cannot make it unstable. Returns
/// the new position and velocity.
fn spring_step(mut x: f64, mut v: f64, target: f64, s: Spring, dt: f64) -> (f64, f64) {
    let steps = (dt * 240.0).ceil().max(1.0);
    let h = dt / steps;
    for _ in 0..steps as usize {
        v += (s.omega * s.omega * (target - x) - 2.0 * s.zeta * s.omega * v) * h;
        x += v * h;
    }
    (x, v)
}

/// Whether a position in cells is close enough to its target, and slow enough, to stop the frames: within half a
/// point and slower than 10 points a second, so the last jump is invisible.
fn at_rest(x: f64, v: f64, target: f64) -> bool {
    (target - x).abs() * ITEM_WIDTH < 0.5 && v.abs() * ITEM_WIDTH < 10.0
}

/// Where the lens heads for a pointer at `x` points in the strip: centred on the pointer, in cells, kept on the
/// strip's pages.
fn lens_target(x: f64, count: usize) -> f64 {
    ((x - PAD) / ITEM_WIDTH - 0.5).clamp(0.0, count.saturating_sub(1) as f64)
}

/// The page under a lens at `shown` cells: the one its centre is over, or the nearest one when it is off the end.
fn cell_under(shown: f64, count: usize) -> usize {
    (shown.round().max(0.0) as usize).min(count.saturating_sub(1))
}

/// How much a lens moving at `speed` cells a second stretches along its motion: a fraction of its width that grows
/// with speed and is capped so it never looks torn.
fn stretch(speed: f64) -> f64 {
    (speed.abs() * 0.025).min(0.16)
}

/// The pill at `shown` cells, `lift` of the way to the lens, stretched by `stretch`, inside a strip `width` wide.
fn lens_rect(shown: f64, lift: f64, stretch: f64, width: f64) -> NSRect {
    let w = (ITEM_WIDTH + LIFT_GROW.0 * lift) * (1.0 + stretch);
    let h = (HEIGHT - 2.0 * PAD + LIFT_GROW.1 * lift) * (1.0 - stretch / 2.0);
    let w = w.min(width);
    let centre = (PAD + (shown + 0.5) * ITEM_WIDTH).clamp(w / 2.0, (width - w / 2.0).max(w / 2.0));
    NSRect::new(
        NSPoint::new(centre - w / 2.0, (HEIGHT - h) / 2.0),
        NSSize::new(w, h),
    )
}

/// The width that fits this many pages.
pub const fn width_for(count: usize) -> f64 {
    2.0 * PAD + count as f64 * ITEM_WIDTH
}

type SelectHandler = Rc<dyn Fn(usize)>;

/// A press on a page that has not been let go of yet.
#[derive(Debug, Clone, Copy)]
struct Press {
    at: Instant,
    down_x: f64,
    /// Where the pointer is now, in the strip's points.
    pointer: f64,
    /// Whether the pill has become the lens, after a hold or a drag.
    lifted: bool,
    /// Whether the window had the keyboard when the press began, so losing it can put the lens back.
    was_key: bool,
}

pub struct SwitcherIvars {
    items: RefCell<Vec<(String, String)>>,
    selected: Cell<usize>,
    /// Where the chosen pill is drawn, in cells, which follows `selected` (or the pointer) instead of jumping.
    shown: Cell<f64>,
    velocity: Cell<f64>,
    /// How far the pill has become the lens, from 0 to about 1, with its velocity.
    lift: Cell<(f64, f64)>,
    press: Cell<Option<Press>>,
    /// The lens is settling after a drag, which it does with the drag's slow spring.
    from_drag: Cell<bool>,
    /// Reduce Motion, read when a movement starts: then the pill and the lens jump instead of travelling.
    still: Cell<bool>,
    /// Synthetic presses in the checks have no real button behind them, so they turn off the look at it.
    watch_button: Cell<bool>,
    last_frame: Cell<Option<Instant>>,
    /// The display link that carries the pill, kept only while it is moving or pressed.
    glide: RefCell<Option<Retained<CADisplayLink>>>,
    /// The Liquid Glass lens, made on the first lift on macOS 26 and later and hidden at rest.
    glass: RefCell<Option<Retained<NSGlassEffectView>>>,
    /// The icons, drawn by a view above the glass so the lens never blurs them.
    ink: OnceCell<Retained<SwitcherInk>>,
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
            crate::sysinfo::count_frame();
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

pub struct InkIvars {
    switcher: Weak<PageSwitcher>,
}

// The icons and badges of the strip, above the lens. It takes no clicks: they go through to the strip.
define_class!(
    // SAFETY: NSView allows subclassing; the ivar is only touched on the main thread.
    #[unsafe(super(NSView))]
    #[thread_kind = MainThreadOnly]
    #[name = "FigPageSwitcherInk"]
    #[ivars = InkIvars]
    pub struct SwitcherInk;

    impl SwitcherInk {
        #[unsafe(method(drawRect:))]
        fn draw_rect(&self, _dirty: NSRect) {
            if let Some(switcher) = self.ivars().switcher.load() {
                switcher.paint_ink();
            }
        }

        #[unsafe(method(hitTest:))]
        fn hit_test(&self, _point: NSPoint) -> *mut NSView {
            std::ptr::null_mut()
        }
    }

    unsafe impl NSObjectProtocol for SwitcherInk {}
);

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
                self.redraw();
            }
        }

        #[unsafe(method(mouseExited:))]
        fn mouse_exited(&self, _event: &NSEvent) {
            if self.ivars().hovered.replace(None).is_some() {
                self.redraw();
            }
        }

        #[unsafe(method(mouseDown:))]
        fn mouse_down(&self, event: &NSEvent) {
            if let Some(index) = self.index_at(event) {
                let x = self.x_of(event);
                self.ivars().still.set(crate::workspace::display_options().reduce_motion);
                self.ivars().press.set(Some(Press {
                    at: Instant::now(),
                    down_x: x,
                    pointer: x,
                    lifted: false,
                    was_key: self.window().is_some_and(|w| w.isKeyWindow()),
                }));
                self.ivars().from_drag.set(false);
                let select = self.ivars().on_select.borrow().clone();
                if let Some(select) = select {
                    select(index);
                }
                // The frames also time the hold, so they run even when the page was already the chosen one.
                self.start_glide();
            }
        }

        #[unsafe(method(mouseDragged:))]
        fn mouse_dragged(&self, event: &NSEvent) {
            if let Some(mut press) = self.ivars().press.get() {
                press.pointer = self.x_of(event);
                press.lifted |= (press.pointer - press.down_x).abs() > SLOP;
                self.ivars().press.set(Some(press));
                self.start_glide();
            }
        }

        #[unsafe(method(mouseUp:))]
        fn mouse_up(&self, _event: &NSEvent) {
            let Some(press) = self.ivars().press.take() else {
                return;
            };
            if press.lifted {
                self.ivars().from_drag.set(true);
                let iv = self.ivars();
                let chosen = cell_under(iv.shown.get(), iv.items.borrow().len());
                if chosen != iv.selected.get() {
                    let select = iv.on_select.borrow().clone();
                    if let Some(select) = select {
                        select(chosen);
                    }
                }
            }
            self.start_glide();
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
            velocity: Cell::new(0.0),
            lift: Cell::new((0.0, 0.0)),
            press: Cell::new(None),
            from_drag: Cell::new(false),
            still: Cell::new(false),
            watch_button: Cell::new(true),
            last_frame: Cell::new(None),
            glide: RefCell::new(None),
            glass: RefCell::new(None),
            ink: OnceCell::new(),
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
        let ink = mtm.alloc::<SwitcherInk>().set_ivars(InkIvars {
            switcher: Weak::from_retained(&this),
        });
        // SAFETY: NSView's designated initializer with a valid frame.
        let ink: Retained<SwitcherInk> =
            unsafe { msg_send![super(ink), initWithFrame: this.bounds()] };
        ink.setAutoresizingMask(
            NSAutoresizingMaskOptions::ViewWidthSizable
                | NSAutoresizingMaskOptions::ViewHeightSizable,
        );
        this.addSubview(&ink);
        let _ = this.ivars().ink.set(ink);
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
        self.redraw();
    }

    pub fn set_selected(&self, index: usize) {
        self.ivars().selected.set(index);
        self.start_glide();
    }

    /// Puts the pill on the chosen page at once, for a frame that must not catch it in flight: a snapshot, or the
    /// notch closing under a lifted lens, which puts it back.
    pub fn settle(&self) {
        let iv = self.ivars();
        iv.shown.set(iv.selected.get() as f64);
        iv.velocity.set(0.0);
        iv.lift.set((0.0, 0.0));
        iv.press.set(None);
        iv.from_drag.set(false);
        self.stop_frames();
        self.layout_glass();
        self.redraw();
    }

    /// Where the pill or the lens is drawn now, in the strip's points.
    pub fn lens_frame(&self) -> NSRect {
        let iv = self.ivars();
        let speed = if iv.still.get() {
            0.0
        } else {
            iv.velocity.get()
        };
        lens_rect(
            iv.shown.get(),
            iv.lift.get().0.max(0.0),
            stretch(speed) * iv.lift.get().0.clamp(0.0, 1.0),
            self.bounds().size.width,
        )
    }

    /// The checks feed the strip synthetic presses, which no real button holds down: they turn off the look at the
    /// button that puts the lens back when its release never arrives.
    pub fn set_watch_button(&self, on: bool) {
        self.ivars().watch_button.set(on);
    }

    /// One frame: the pill follows the chosen page (or the lens the pointer) on its spring, and the lift swells or
    /// relaxes on its own, until everything is at rest and the frames stop.
    pub fn step(&self) {
        let iv = self.ivars();
        let now = Instant::now();
        let dt = iv
            .last_frame
            .replace(Some(now))
            .map_or(1.0 / 60.0, |t| (now - t).as_secs_f64().min(1.0 / 20.0));
        let count = iv.items.borrow().len();
        let mut press = iv.press.get();
        if let Some(p) = press.as_mut() {
            if self.press_lost(p) {
                // Put back where it was, without choosing anything.
                press = None;
                iv.from_drag.set(true);
            } else {
                p.lifted |= p.at.elapsed() >= HOLD;
            }
        }
        iv.press.set(press);
        let lifted = press.filter(|p| p.lifted);
        let (target, spring) = match lifted {
            Some(p) => (lens_target(p.pointer, count), FOLLOW),
            None if iv.from_drag.get() => (iv.selected.get() as f64, FOLLOW),
            None => (iv.selected.get() as f64, GLIDE),
        };
        let lift_to = if lifted.is_some() { 1.0 } else { 0.0 };
        // A pill off the strip (no page chosen, or one that just appeared) jumps rather than flying in from afar.
        let visible = (-1.0..=count as f64).contains(&iv.shown.get()) && target < count as f64;
        if iv.still.get() || !visible {
            iv.shown.set(target);
            iv.velocity.set(0.0);
            iv.lift.set((lift_to, 0.0));
        } else {
            let (x, v) = spring_step(iv.shown.get(), iv.velocity.get(), target, spring, dt);
            iv.shown.set(x);
            iv.velocity.set(v);
            let (l, lv) = iv.lift.get();
            iv.lift.set(spring_step(l, lv, lift_to, LIFT, dt));
        }
        let (l, lv) = iv.lift.get();
        if press.is_none()
            && at_rest(iv.shown.get(), iv.velocity.get(), target)
            && at_rest(l, lv, 0.0)
        {
            iv.shown.set(target);
            iv.velocity.set(0.0);
            iv.lift.set((0.0, 0.0));
            iv.from_drag.set(false);
            self.stop_frames();
        }
        self.layout_glass();
        self.redraw();
    }

    /// Whether a press can no longer end in a release here: the button is up without its release reaching the strip,
    /// Escape is down, or the window lost the keyboard.
    fn press_lost(&self, press: &Press) -> bool {
        (self.ivars().watch_button.get() && NSEvent::pressedMouseButtons() & 1 == 0)
            || CGEventSource::key_state(CGEventSourceStateID::CombinedSessionState, ESCAPE)
            || (press.was_key && !self.window().is_some_and(|w| w.isKeyWindow()))
    }

    /// A run loop keeps a display link that is only released, and the link keeps this view: it has to be
    /// invalidated to stop the frames.
    fn stop_frames(&self) {
        if let Some(link) = self.ivars().glide.borrow_mut().take() {
            link.invalidate();
        }
        self.ivars().last_frame.set(None);
    }

    fn start_glide(&self) {
        if self.ivars().glide.borrow().is_some() {
            self.redraw();
            return;
        }
        if self.ivars().press.get().is_none() {
            self.ivars()
                .still
                .set(crate::workspace::display_options().reduce_motion);
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
            self.ivars().press.set(None);
            self.redraw();
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
        self.redraw();
    }

    /// Places the glass lens over the pill while it is lifted, fading in with the lift, and hides it at rest. The
    /// glass is made on the first lift, on macOS 26 and later.
    fn layout_glass(&self) {
        let iv = self.ivars();
        let lift = iv.lift.get().0;
        if iv.glass.borrow().is_none()
            && lift > 0.0
            && AnyClass::get(c"NSGlassEffectView").is_some()
        {
            let mtm = MainThreadMarker::new().expect("main thread");
            let glass = NSGlassEffectView::initWithFrame(mtm.alloc(), NSRect::ZERO);
            // Clear glass over the dark well reads as a lens: the strip shows through it, brighter at the rim.
            glass.setStyle(NSGlassEffectViewStyle::Clear);
            if let Some(ink) = iv.ink.get() {
                self.addSubview_positioned_relativeTo(
                    &glass,
                    NSWindowOrderingMode::Below,
                    Some(ink),
                );
            }
            iv.glass.replace(Some(glass));
        }
        if let Some(glass) = iv.glass.borrow().as_ref() {
            let frame = self.lens_frame();
            glass.setHidden(lift <= 0.01);
            glass.setAlphaValue(lift.clamp(0.0, 1.0));
            glass.setFrame(frame);
            glass.setCornerRadius(frame.size.height.min(frame.size.width) / 2.0);
        }
    }

    fn redraw(&self) {
        self.setNeedsDisplay(true);
        if let Some(ink) = self.ivars().ink.get() {
            ink.setNeedsDisplay(true);
        }
    }

    /// Marks a page icon with a dot, or clears the mark.
    pub fn set_badge(&self, index: usize, on: bool) {
        let mut badges = self.ivars().badges.borrow_mut();
        badges.retain(|&i| i != index);
        if on {
            badges.push(index);
        }
        drop(badges);
        self.redraw();
    }

    fn x_of(&self, event: &NSEvent) -> f64 {
        self.convertPoint_fromView(event.locationInWindow(), None).x
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

    fn lifted(&self) -> bool {
        self.ivars().press.get().is_some_and(|p| p.lifted)
    }

    /// The page the pill marks: the chosen one, or while a lens is held, the one it would choose if let go now.
    fn marked(&self) -> usize {
        let iv = self.ivars();
        if self.lifted() {
            cell_under(iv.shown.get(), iv.items.borrow().len())
        } else {
            iv.selected.get()
        }
    }

    fn paint(&self) {
        let bounds = self.bounds();
        // A well holds the row of pages; the pill that marks the chosen one is the only thing that stands out of it.
        style::well(bounds, HEIGHT / 2.0);
        let (hovered, lift) = (self.ivars().hovered.get(), self.ivars().lift.get().0);
        if let Some(i) = hovered.filter(|&i| i != self.marked() && !self.lifted()) {
            let cell = NSRect::new(
                NSPoint::new(PAD + i as f64 * ITEM_WIDTH, PAD),
                NSSize::new(ITEM_WIDTH, HEIGHT - 2.0 * PAD),
            );
            style::control(cell, cell.size.height / 2.0, Control::Hover);
        }
        let pill = self.lens_frame();
        let radius = pill.size.height.min(pill.size.width) / 2.0;
        // With glass the white pill fades as the lens appears over it; without, the drawn pill is the lens.
        if self.ivars().glass.borrow().is_some() {
            style::card_bright_alpha(pill, radius, 1.0 - lift.clamp(0.0, 1.0));
        } else {
            style::card_bright(pill, radius);
        }
    }

    fn paint_ink(&self) {
        let items = self.ivars().items.borrow();
        let marked = self.marked();
        let lift = self.ivars().lift.get().0.clamp(0.0, 1.0);
        let on_glass = self.ivars().glass.borrow().is_some() && lift > 0.5;
        for (i, (symbol, _)) in items.iter().enumerate() {
            let cell = NSRect::new(
                NSPoint::new(PAD + i as f64 * ITEM_WIDTH, PAD),
                NSSize::new(ITEM_WIDTH, HEIGHT - 2.0 * PAD),
            );
            let on = i == marked;
            let ink = match (on, on_glass) {
                (true, true) => white(0.98),
                (true, false) => black(0.88),
                _ => white(TEXT_SECONDARY + 0.12),
            };
            // The page under the lens grows with it, so it stays readable through the glass.
            let size = if on { GLYPH + 3.0 * lift } else { GLYPH };
            style::draw_symbol(
                symbol,
                NSPoint::new(
                    cell.origin.x + cell.size.width / 2.0,
                    cell.origin.y + cell.size.height / 2.0,
                ),
                size,
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

#[cfg(test)]
mod tests {
    use super::*;

    /// Runs a spring at 60 frames a second from `from` to `to` and returns how long it took to come to rest and the
    /// farthest it went past the target, in cells.
    fn run(s: Spring, from: f64, to: f64) -> (f64, f64) {
        let (mut x, mut v, mut t, mut past) = (from, 0.0, 0.0, 0.0_f64);
        while !at_rest(x, v, to) {
            (x, v) = spring_step(x, v, to, s, 1.0 / 60.0);
            t += 1.0 / 60.0;
            past = past.max((x - to) * (to - from).signum());
            assert!(t < 5.0, "the spring never came to rest");
        }
        (t, past)
    }

    #[test]
    fn a_click_glides_a_few_cells_in_time_without_passing_its_page() {
        for cells in 1..=4 {
            let (t, past) = run(GLIDE, 0.0, cells as f64);
            assert!(t <= 0.35, "{cells} cells took {t} s");
            assert!(past <= 0.0, "{cells} cells went {past} past");
        }
        // Slower than the old 28 percent a frame, whose first frame covered more than a quarter of the way.
        let (x, _) = spring_step(0.0, 0.0, 3.0, GLIDE, 1.0 / 60.0);
        assert!(x < 0.28 * 3.0);
    }

    #[test]
    fn the_lens_trails_and_settles_slowly_with_little_overshoot() {
        let (t, past) = run(FOLLOW, 0.0, 2.0);
        assert!((0.4..1.5).contains(&t), "settled in {t} s");
        assert!(past * ITEM_WIDTH < 1.0, "went {past} cells past");
        // Behind a pointer that jumps two cells, after a tenth of a second it is still less than half way.
        let mut x = (0.0, 0.0);
        for _ in 0..6 {
            x = spring_step(x.0, x.1, 2.0, FOLLOW, 1.0 / 60.0);
        }
        assert!(x.0 < 1.0, "{}", x.0);
    }

    #[test]
    fn a_long_frame_does_not_blow_the_spring_up() {
        let (x, v) = spring_step(0.0, 0.0, 3.0, LIFT, 1.0 / 20.0);
        assert!(x > 0.0 && x < 3.5 && v.is_finite());
        let (t, past) = run(LIFT, 0.0, 1.0);
        assert!(t < 1.0 && past < 0.1, "{t} {past}");
    }

    #[test]
    fn the_lens_follows_the_pointer_inside_the_strip() {
        // The pointer over the middle of the third cell centres the lens on it.
        assert_eq!(lens_target(PAD + 2.5 * ITEM_WIDTH, 5), 2.0);
        assert_eq!(lens_target(-50.0, 5), 0.0);
        assert_eq!(lens_target(PAD + 0.2 * ITEM_WIDTH, 5), 0.0);
        assert_eq!(lens_target(width_for(5) + 80.0, 5), 4.0);
        assert_eq!(lens_target(100.0, 0), 0.0);
    }

    #[test]
    fn the_cell_under_the_lens_is_the_nearest_page() {
        assert_eq!(cell_under(0.0, 5), 0);
        assert_eq!(cell_under(1.49, 5), 1);
        assert_eq!(cell_under(1.51, 5), 2);
        assert_eq!(cell_under(-3.0, 5), 0);
        assert_eq!(cell_under(9.0, 5), 4);
        assert_eq!(cell_under(lens_target(-50.0, 5), 5), 0);
        assert_eq!(cell_under(lens_target(500.0, 5), 5), 4);
        assert_eq!(cell_under(2.0, 0), 0);
    }

    #[test]
    fn the_lens_stays_in_the_strip_and_stretches_with_speed() {
        let width = width_for(4);
        for shown in [-1.0, 0.0, 1.5, 3.0, 5.0] {
            let r = lens_rect(shown, 1.0, stretch(20.0), width);
            assert!(
                r.origin.x >= -1e-9 && r.origin.x + r.size.width <= width + 1e-9,
                "{shown}: {r:?}"
            );
            assert!(r.origin.y >= 0.0 && r.origin.y + r.size.height <= HEIGHT);
        }
        let rest = lens_rect(1.0, 0.0, 0.0, width);
        assert_eq!(rest.origin.x, PAD + ITEM_WIDTH);
        assert_eq!(rest.size, NSSize::new(ITEM_WIDTH, HEIGHT - 2.0 * PAD));
        let lifted = lens_rect(1.0, 1.0, 0.0, width);
        assert!(lifted.size.width > rest.size.width && lifted.size.height > rest.size.height);
        assert!(stretch(0.0) == 0.0 && stretch(3.0) > 0.0 && stretch(-100.0) == 0.16);
    }
}
