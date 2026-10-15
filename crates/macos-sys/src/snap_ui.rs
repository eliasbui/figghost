//! The macOS side of dragging a window onto the notch: the pointer events of every app, the window under the
//! pointer, the row of drop zones and the screens it hangs from. What a drag means lives in `core-window`; this
//! file only looks and draws. All positions are in the coordinates of `window::Frame` (origin at the top-left of
//! the primary screen, y down) unless a name says Cocoa.

use crate::{
    app,
    ax::Element,
    notch::{NotchGeometry, PILL_WIDTH},
    style::{self, BUTTON_RADIUS},
    window::{Frame, Window},
    workspace,
};
use block2::RcBlock;
use objc2::{
    AnyThread, DefinedClass, MainThreadMarker, MainThreadOnly, define_class, msg_send,
    rc::Retained, runtime::AnyObject,
};
use objc2_app_kit::{
    NSAccessibility, NSAppearance, NSAppearanceCustomization, NSAppearanceNameDarkAqua,
    NSBackingStoreType, NSBezierPath, NSBitmapImageFileType, NSColor, NSEvent, NSEventMask,
    NSEventType, NSGradient, NSMainMenuWindowLevel, NSPanel, NSScreen, NSStatusBar, NSView,
    NSWindowAnimationBehavior, NSWindowCollectionBehavior, NSWindowStyleMask,
};
use objc2_foundation::{NSDictionary, NSPoint, NSRect, NSSize, NSString};
use std::{
    cell::{Cell, RefCell},
    ptr::NonNull,
    rc::{Rc, Weak},
    time::Instant,
};

/// How long a request to an app that owns a window may take before it counts as failed: a hung app must not
/// freeze this one.
const AX_TIMEOUT_SECONDS: f32 = 0.4;
const OPEN_SECONDS: f64 = 0.46;
const CLOSE_SECONDS: f64 = 0.24;
const FRAME_SECONDS: f64 = 1.0 / 120.0;
/// The height the shape grows from and shrinks back to.
const REST_HEIGHT: f64 = 6.0;
/// The bottom corners of the shape; its top edge is cut off by the top of the screen.
const SHAPE_RADIUS: f64 = 22.0;

// ---------------------------------------------------------------- screens and coordinates

/// One screen: where it is, the usable part, and what the zones must clear at its top.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DisplayInfo {
    pub frame: Frame,
    pub area: Frame,
    pub cutout: f64,
    pub inset: f64,
}

fn primary_height(mtm: MainThreadMarker) -> f64 {
    NSScreen::screens(mtm)
        .firstObject()
        .map_or(0.0, |s| s.frame().size.height)
}

fn top_left(primary_height: f64, r: NSRect) -> Frame {
    Frame {
        x: r.origin.x,
        y: primary_height - (r.origin.y + r.size.height),
        w: r.size.width,
        h: r.size.height,
    }
}

/// Every screen, with the notch cutout it has or the pill the notch shows instead. With
/// `FIGGHOST_NOTCH_OFFSCREEN` set, the single screen the notch lives on in tests, far outside the real ones, so a
/// pointer on the real screens never reaches it.
pub fn displays(mtm: MainThreadMarker) -> Vec<DisplayInfo> {
    let height = primary_height(mtm);
    let info = |screen: NSRect, area: NSRect, geometry: NotchGeometry| DisplayInfo {
        frame: top_left(height, screen),
        area: top_left(height, area),
        cutout: geometry.notch.width,
        inset: geometry.top_inset(),
    };
    if std::env::var_os("FIGGHOST_NOTCH_OFFSCREEN").is_some() {
        return NotchGeometry::detect(mtm)
            .map(|g| vec![info(g.screen, g.screen, g)])
            .unwrap_or_default();
    }
    NSScreen::screens(mtm)
        .iter()
        .map(|s| {
            let inset = s.safeAreaInsets().top;
            let geometry = if inset > 0.0 {
                let (left, right) = (s.auxiliaryTopLeftArea(), s.auxiliaryTopRightArea());
                NotchGeometry {
                    screen: s.frame(),
                    notch: NSSize::new(right.origin.x - (left.origin.x + left.size.width), inset),
                    hardware: true,
                }
            } else {
                NotchGeometry {
                    screen: s.frame(),
                    notch: NSSize::new(PILL_WIDTH, NSStatusBar::systemStatusBar().thickness()),
                    hardware: false,
                }
            };
            info(s.frame(), s.visibleFrame(), geometry)
        })
        .collect()
}

// ---------------------------------------------------------------- the pointer

/// What the left button did, with the pointer position.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Mouse {
    Down((f64, f64)),
    Dragged((f64, f64)),
    Up((f64, f64)),
    /// A drag event arrived but the button is not down: the release was missed.
    Lost,
}

/// A mouse event as the left-button action it is, or `None` for anything else. The position is the event's own,
/// so an event made in-process for a test carries its own position instead of the real pointer's.
pub fn translate(mtm: MainThreadMarker, event: &NSEvent) -> Option<Mouse> {
    translate_with(mtm, primary_height(mtm), event)
}

fn translate_with(mtm: MainThreadMarker, primary_height: f64, event: &NSEvent) -> Option<Mouse> {
    let location = if event.window(mtm).is_some() {
        NSEvent::mouseLocation()
    } else {
        event.locationInWindow()
    };
    let at = (location.x, primary_height - location.y);
    match event.r#type() {
        NSEventType::LeftMouseDown => Some(Mouse::Down(at)),
        NSEventType::LeftMouseDragged => Some(Mouse::Dragged(at)),
        NSEventType::LeftMouseUp => Some(Mouse::Up(at)),
        _ => None,
    }
}

/// Watches the left button over every app. Mouse events need no permission. Removed when dropped.
pub struct MouseMonitor {
    token: Retained<AnyObject>,
}

impl MouseMonitor {
    pub fn start(mtm: MainThreadMarker, handler: impl Fn(Mouse) + 'static) -> Option<Self> {
        // Drag events come at the rate of the mouse; the screens are looked at again only when a button goes down.
        let height = Cell::new(0.0);
        let block = RcBlock::new(move |event: NonNull<NSEvent>| {
            // SAFETY: AppKit passes a live event for the duration of the call.
            let event = unsafe { event.as_ref() };
            if height.get() == 0.0 || event.r#type() == NSEventType::LeftMouseDown {
                height.set(primary_height(mtm));
            }
            let Some(mouse) = translate_with(mtm, height.get(), event) else {
                return;
            };
            let lost =
                matches!(mouse, Mouse::Dragged(_)) && NSEvent::pressedMouseButtons() & 1 == 0;
            handler(if lost { Mouse::Lost } else { mouse });
        });
        let token = NSEvent::addGlobalMonitorForEventsMatchingMask_handler(
            NSEventMask::LeftMouseDown | NSEventMask::LeftMouseDragged | NSEventMask::LeftMouseUp,
            &block,
        )?;
        Some(Self { token })
    }
}

impl Drop for MouseMonitor {
    fn drop(&mut self) {
        // SAFETY: the token came from addGlobalMonitorForEventsMatchingMask and is removed once.
        unsafe { NSEvent::removeMonitor(&self.token) };
    }
}

// ---------------------------------------------------------------- the window under the pointer

/// The window a press landed on, kept to see whether it moves and to place it.
pub struct Held {
    window: Window,
    role: String,
    pressed_frame: Frame,
}

/// The window under a point and the role of the element pressed in it. `None` when there is none, when the
/// request fails or times out, or when Accessibility is not granted.
pub fn grab(x: f64, y: f64) -> Option<Held> {
    let element = Element::at_position(x, y, AX_TIMEOUT_SECONDS)?;
    let role = element.string("AXRole").unwrap_or_default();
    let window = Window::containing(&element)?;
    window.set_timeout(AX_TIMEOUT_SECONDS);
    let pressed_frame = window.frame()?;
    Some(Held {
        window,
        role,
        pressed_frame,
    })
}

impl Held {
    pub fn role(&self) -> &str {
        &self.role
    }

    pub fn pid(&self) -> Option<i32> {
        self.window.pid()
    }

    /// The window's frame when the button went down.
    pub fn frame_at_press(&self) -> Frame {
        self.pressed_frame
    }

    pub fn frame(&self) -> Option<Frame> {
        self.window.frame()
    }

    pub fn is_full_screen(&self) -> bool {
        self.window.is_full_screen()
    }

    /// Whether the window has left where the press found it.
    pub fn moved(&self) -> bool {
        self.window.frame().is_some_and(|now| {
            (now.x - self.pressed_frame.x).abs() >= 2.0
                || (now.y - self.pressed_frame.y).abs() >= 2.0
        })
    }

    pub fn place(&self, frame: Frame) -> bool {
        self.window.set_frame(frame)
    }
}

// ---------------------------------------------------------------- the drop zones

/// What the row shows. Cells are relative to the shape's own top-left corner.
#[derive(Debug, Clone, PartialEq)]
pub struct ZonesSpec {
    /// What the row is, for assistive technology: the names of the zones in order.
    pub label: String,
    pub width: f64,
    pub height: f64,
    pub cells: Vec<Frame>,
    /// For each cell, the part of a small screen the layout fills, as fractions: x, y, width, height.
    pub glyphs: Vec<[f64; 4]>,
}

pub struct ViewIvars {
    spec: RefCell<ZonesSpec>,
    hover: Cell<Option<usize>>,
}

define_class!(
    // SAFETY: NSView allows subclassing; the ivars are only touched on the main thread and need no Drop.
    #[unsafe(super(NSView))]
    #[thread_kind = MainThreadOnly]
    #[name = "FigSnapZones"]
    #[ivars = ViewIvars]
    struct ZonesView;

    impl ZonesView {
        // Cells are laid out from the top edge, which is also where the shape is anchored while it grows.
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
            self.paint();
        }
    }
);

impl ZonesView {
    fn new(mtm: MainThreadMarker) -> Retained<Self> {
        let ivars = ViewIvars {
            spec: RefCell::new(ZonesSpec {
                label: String::new(),
                width: 0.0,
                height: 0.0,
                cells: Vec::new(),
                glyphs: Vec::new(),
            }),
            hover: Cell::new(None),
        };
        // SAFETY: designated initializer with valid arguments on the main thread.
        unsafe {
            msg_send![super(mtm.alloc::<Self>().set_ivars(ivars)), initWithFrame: NSRect::ZERO]
        }
    }

    fn adopt(&self, spec: &ZonesSpec) {
        *self.ivars().spec.borrow_mut() = spec.clone();
        self.setAccessibilityElement(true);
        self.setAccessibilityLabel(Some(&NSString::from_str(&spec.label)));
    }

    fn paint(&self) {
        let spec = self.ivars().spec.borrow();
        let bounds = self.bounds();
        // The shape overshoots the top edge so only its bottom corners are rounded, like the notch itself.
        let shape = NSRect::new(
            NSPoint::new(bounds.origin.x, bounds.origin.y - SHAPE_RADIUS),
            NSSize::new(bounds.size.width, bounds.size.height + SHAPE_RADIUS),
        );
        let path = NSBezierPath::bezierPathWithRoundedRect_xRadius_yRadius(
            shape,
            SHAPE_RADIUS,
            SHAPE_RADIUS,
        );
        let slate = NSColor::colorWithSRGBRed_green_blue_alpha(0.17, 0.18, 0.20, 1.0);
        if let Some(gradient) = NSGradient::initWithStartingColor_endingColor(
            NSGradient::alloc(),
            &slate,
            &style::black(1.0),
        ) {
            // The view is flipped, so the angle that runs from black at the top to slate at the bottom is
            // the opposite of the notch's.
            gradient.drawInBezierPath_angle(&path, 270.0);
        }
        let hover = self.ivars().hover.get();
        for (i, cell) in spec.cells.iter().enumerate() {
            let rect = NSRect::new(NSPoint::new(cell.x, cell.y), NSSize::new(cell.w, cell.h));
            let active = hover == Some(i);
            // The zone the window will land on is solid white with dark ink; the others rest as translucent
            // controls.
            let fill = style::white(if active { 1.0 } else { style::CONTROL });
            style::fill_rounded(rect, BUTTON_RADIUS, &fill);
            let ink = if active {
                style::black(0.85)
            } else {
                style::white(style::TEXT)
            };
            if let Some(glyph) = spec.glyphs.get(i) {
                draw_glyph(rect, *glyph, &ink);
            }
        }
    }
}

/// A small screen with the part a layout fills.
fn draw_glyph(cell: NSRect, part: [f64; 4], ink: &NSColor) {
    let (w, h) = (34.0, 22.0);
    let screen = NSRect::new(
        NSPoint::new(
            cell.origin.x + (cell.size.width - w) / 2.0,
            cell.origin.y + (cell.size.height - h) / 2.0,
        ),
        NSSize::new(w, h),
    );
    style::stroke_rounded(screen, 4.0, &ink.colorWithAlphaComponent(0.6), 1.5);
    let inner = NSRect::new(
        NSPoint::new(screen.origin.x + 3.0, screen.origin.y + 3.0),
        NSSize::new(w - 6.0, h - 6.0),
    );
    let filled = NSRect::new(
        NSPoint::new(
            inner.origin.x + part[0] * inner.size.width,
            inner.origin.y + part[1] * inner.size.height,
        ),
        NSSize::new(part[2] * inner.size.width, part[3] * inner.size.height),
    );
    style::fill_rounded(filled, 2.0, ink);
}

define_class!(
    // SAFETY: NSPanel allows subclassing; nothing else is added.
    #[unsafe(super(NSPanel))]
    #[thread_kind = MainThreadOnly]
    #[name = "FigSnapPanel"]
    struct SnapPanel;

    impl SnapPanel {
        // The zones are only looked at: they never take focus, however the window server is asked.
        #[unsafe(method(canBecomeKeyWindow))]
        fn can_become_key_window(&self) -> bool {
            false
        }

        #[unsafe(method(canBecomeMainWindow))]
        fn can_become_main_window(&self) -> bool {
            false
        }
    }
);

impl SnapPanel {
    fn new(mtm: MainThreadMarker) -> Retained<Self> {
        let style = NSWindowStyleMask::Borderless | NSWindowStyleMask::NonactivatingPanel;
        // SAFETY: designated initializer with valid arguments on the main thread.
        let panel: Retained<Self> = unsafe {
            msg_send![
                mtm.alloc::<Self>(),
                initWithContentRect: NSRect::ZERO,
                styleMask: style,
                backing: NSBackingStoreType::Buffered,
                defer: false
            ]
        };
        // SAFETY: false is the only value compatible with a window kept alive by `Retained`.
        unsafe { panel.setReleasedWhenClosed(false) };
        panel.setOpaque(false);
        panel.setBackgroundColor(Some(&NSColor::clearColor()));
        panel.setHasShadow(false);
        panel.setFloatingPanel(true);
        panel.setHidesOnDeactivate(false);
        // The pointer is busy dragging another app's window; clicks must fall through whatever happens.
        panel.setIgnoresMouseEvents(true);
        panel.setLevel(NSMainMenuWindowLevel + 2);
        panel.setCollectionBehavior(
            NSWindowCollectionBehavior::CanJoinAllSpaces
                | NSWindowCollectionBehavior::Stationary
                | NSWindowCollectionBehavior::FullScreenAuxiliary
                | NSWindowCollectionBehavior::IgnoresCycle,
        );
        panel.setAnimationBehavior(NSWindowAnimationBehavior::None);
        // SAFETY: extern statics are process-lifetime constants.
        let dark = unsafe { NSAppearance::appearanceNamed(NSAppearanceNameDarkAqua) };
        panel.setAppearance(dark.as_deref());
        panel
    }
}

fn lerp(from: f64, to: f64, t: f64) -> f64 {
    from + (to - from) * t
}

struct Inner {
    mtm: MainThreadMarker,
    panel: Retained<SnapPanel>,
    view: Retained<ZonesView>,
    /// Where the shape rests when fully open, in Cocoa coordinates; `None` while it is not wanted.
    open_frame: Cell<Option<NSRect>>,
    anim: RefCell<Option<app::Timer>>,
    me: Weak<Inner>,
}

impl Inner {
    /// The shape at height `h`, hanging from the same top edge as the open one.
    fn frame_with_height(open: NSRect, h: f64) -> NSRect {
        NSRect::new(
            NSPoint::new(open.origin.x, open.origin.y + open.size.height - h),
            NSSize::new(open.size.width, h),
        )
    }

    fn animate(
        &self,
        open: NSRect,
        to_height: f64,
        seconds: f64,
        curve: fn(f64) -> f64,
        alpha: f64,
        done: impl Fn() + 'static,
    ) {
        let from_height = self.panel.frame().size.height;
        let from_alpha = self.panel.alphaValue();
        let (started, weak) = (Instant::now(), self.me.clone());
        *self.anim.borrow_mut() = Some(app::every(FRAME_SECONDS, move || {
            let Some(inner) = weak.upgrade() else {
                return;
            };
            let t = (started.elapsed().as_secs_f64() / seconds).min(1.0);
            let h = lerp(from_height, to_height, curve(t));
            inner
                .panel
                .setFrame_display(Inner::frame_with_height(open, h.max(1.0)), true);
            inner
                .panel
                .setAlphaValue(lerp(from_alpha, alpha, style::ease_out(t)));
            if t >= 1.0 {
                inner.anim.borrow_mut().take();
                done();
            }
        }));
    }
}

/// The row of drop zones, a non-activating panel that grows out of the top edge of a screen. It never takes
/// focus and passes every click through.
pub struct ZonesPanel {
    inner: Rc<Inner>,
}

impl ZonesPanel {
    pub fn new(mtm: MainThreadMarker) -> Self {
        let panel = SnapPanel::new(mtm);
        let view = ZonesView::new(mtm);
        panel.setContentView(Some(&view));
        let inner = Rc::new_cyclic(|me| Inner {
            mtm,
            panel,
            view,
            open_frame: Cell::new(None),
            anim: RefCell::new(None),
            me: me.clone(),
        });
        Self { inner }
    }

    /// Shows the row hanging from the top edge at `top_left` (the corner of the shape in `Frame` coordinates),
    /// with the zone `hover` lit. Called again with the same place it only moves the highlight.
    pub fn show(&self, top_left: (f64, f64), spec: &ZonesSpec, hover: Option<usize>) {
        let inner = &self.inner;
        inner.view.ivars().hover.set(hover);
        let open = NSRect::new(
            NSPoint::new(
                top_left.0,
                primary_height(inner.mtm) - (top_left.1 + spec.height),
            ),
            NSSize::new(spec.width, spec.height),
        );
        let same =
            inner.open_frame.get() == Some(open) && *inner.view.ivars().spec.borrow() == *spec;
        if same {
            inner.view.setNeedsDisplay(true);
            return;
        }
        inner.view.adopt(spec);
        inner.view.setFrameSize(open.size);
        inner.view.setNeedsDisplay(true);
        inner.open_frame.set(Some(open));
        if workspace::display_options().reduce_motion {
            inner.anim.borrow_mut().take();
            inner.panel.setAlphaValue(1.0);
            inner.panel.setFrame_display(open, true);
            inner.panel.orderFrontRegardless();
            return;
        }
        if !inner.panel.isVisible() {
            inner.panel.setAlphaValue(0.0);
            inner
                .panel
                .setFrame_display(Inner::frame_with_height(open, REST_HEIGHT), false);
            inner.panel.orderFrontRegardless();
        }
        inner.animate(
            open,
            open.size.height,
            OPEN_SECONDS,
            style::spring,
            1.0,
            || {},
        );
    }

    /// Moves the highlight without touching the rest.
    pub fn set_hover(&self, hover: Option<usize>) {
        if self.inner.view.ivars().hover.replace(hover) != hover {
            self.inner.view.setNeedsDisplay(true);
        }
    }

    /// Takes the row away: it shrinks back into the edge and fades.
    pub fn hide(&self) {
        let inner = &self.inner;
        let Some(open) = inner.open_frame.take() else {
            return;
        };
        if workspace::display_options().reduce_motion || !inner.panel.isVisible() {
            inner.anim.borrow_mut().take();
            inner.panel.orderOut(None);
            return;
        }
        let weak = inner.me.clone();
        inner.animate(
            open,
            REST_HEIGHT,
            CLOSE_SECONDS,
            style::ease_out,
            0.0,
            move || {
                if let Some(inner) = weak.upgrade() {
                    inner.panel.orderOut(None);
                }
            },
        );
    }

    pub fn is_shown(&self) -> bool {
        self.inner.open_frame.get().is_some()
    }

    /// The panel's frame in Cocoa screen coordinates while it is on screen, for tests.
    pub fn frame(&self) -> NSRect {
        self.inner.panel.frame()
    }

    /// Draws the row with the zone `hover` lit into PNG bytes without showing anything: the view is laid out at its
    /// full size and drawn from there. Transparent outside the shape. For checking the design.
    pub fn snapshot_png(&self, spec: &ZonesSpec, hover: Option<usize>) -> Option<Vec<u8>> {
        let view = &self.inner.view;
        view.adopt(spec);
        view.ivars().hover.set(hover);
        view.setFrameSize(NSSize::new(spec.width, spec.height));
        let bounds = view.bounds();
        let rep = view.bitmapImageRepForCachingDisplayInRect(bounds)?;
        view.cacheDisplayInRect_toBitmapImageRep(bounds, &rep);
        // SAFETY: an empty properties dictionary is valid for PNG output.
        let data = unsafe {
            rep.representationUsingType_properties(NSBitmapImageFileType::PNG, &NSDictionary::new())
        }?;
        Some(data.to_vec())
    }
}

impl Drop for ZonesPanel {
    fn drop(&mut self) {
        self.inner.anim.borrow_mut().take();
        self.inner.panel.orderOut(None);
        self.inner.panel.close();
    }
}
