//! The macOS side of dragging a window onto the notch: the pointer events of every app, the window under the
//! pointer, the row of layouts and the screens it hangs from, the cells of the chosen layout over the screen, and the
//! footprint that shows where the window will land.
//! What a drag means lives in `core-window`; this file only looks and draws. All positions are in the coordinates
//! of `window::Frame` (origin at the top-left of the primary screen, y down) unless a name says Cocoa.

use crate::{
    app,
    ax::Element,
    notch::{NotchGeometry, PILL_WIDTH},
    style::{self, BUTTON_RADIUS, Weight},
    widgets::{self, Fit},
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
    NSBackingStoreType, NSBezierPath, NSBitmapImageFileType, NSBitmapImageRep, NSColor,
    NSDeviceRGBColorSpace, NSEvent, NSEventMask, NSEventType, NSGradient, NSMainMenuWindowLevel,
    NSPanel, NSScreen, NSStatusBar, NSView, NSWindowAnimationBehavior, NSWindowCollectionBehavior,
    NSWindowStyleMask,
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
/// The small screen in a layout's tile, 16 by 10 like most displays, and how far below the top of the tile it sits.
const GLYPH: (f64, f64) = (50.0, 31.0);
const GLYPH_TOP: f64 = 9.0;
/// The small screen in the chip the row shrinks to, and the space round it.
const CHIP_GLYPH: (f64, f64) = (30.0, 19.0);
const CHIP_MARGIN: f64 = 14.0;
/// The height of the line in the chip, as core-window's `snap::CHIP`.
const CHIP_LINE: f64 = 26.0;
/// The shape changes size, from the row to the chip and back, in this time.
const RESIZE_SECONDS: f64 = 0.22;
/// A space between the cells drawn over the screen, so that where one ends and the next begins shows.
const CELL_GAP: f64 = 4.0;
const CELL_RADIUS: f64 = 14.0;
/// The footprint glides to another zone's frame quickly enough to keep up with the pointer.
const GLIDE_SECONDS: f64 = 0.18;
const FADE_IN_SECONDS: f64 = 0.12;
const FADE_OUT_SECONDS: f64 = 0.2;
/// Close to the corners of a window, so the footprint reads as one.
const FOOTPRINT_RADIUS: f64 = 12.0;

// ---------------------------------------------------------------- screens and coordinates

/// One screen: its name, where it is, the usable part, and what the row must clear at its top.
#[derive(Debug, Clone, PartialEq)]
pub struct DisplayInfo {
    pub name: String,
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

/// A rect in screen coordinates, as a window frame: top-left origin, y down.
pub fn frame_of(mtm: MainThreadMarker, r: NSRect) -> Frame {
    top_left(primary_height(mtm), r)
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
    let info = |name: String, screen: NSRect, area: NSRect, geometry: NotchGeometry| DisplayInfo {
        name,
        frame: top_left(height, screen),
        area: top_left(height, area),
        cutout: geometry.notch.width,
        inset: geometry.top_inset(),
    };
    if std::env::var_os("FIGGHOST_NOTCH_OFFSCREEN").is_some() {
        return NotchGeometry::detect(mtm)
            .map(|g| vec![info("Test screen".into(), g.screen, g.screen, g)])
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
            info(
                s.localizedName().to_string(),
                s.frame(),
                s.visibleFrame(),
                geometry,
            )
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

// ---------------------------------------------------------------- the row of layouts

/// What the shape shows: the row of layouts to choose from, or the chip the row shrinks to. Cells are relative to
/// the shape's own top-left corner.
#[derive(Debug, Clone, PartialEq)]
pub struct ZonesSpec {
    /// What the shape is, for assistive technology: the names of the layouts in order.
    pub label: String,
    pub width: f64,
    pub height: f64,
    /// The tile of each layout.
    pub cells: Vec<Frame>,
    /// For each layout, the part of a small screen each of its cells fills, as fractions: x, y, width, height.
    pub glyphs: Vec<Vec<[f64; 4]>>,
    /// For each layout, the short name written under its small screen.
    pub names: Vec<String>,
    /// The chip a chosen layout shrinks the row to. `None` for the row.
    pub chip: Option<ChipSpec>,
}

/// The chip: the small screen of the chosen layout and its name.
#[derive(Debug, Clone, PartialEq)]
pub struct ChipSpec {
    /// The part of the small screen each cell of the layout fills, as fractions: x, y, width, height.
    pub parts: Vec<[f64; 4]>,
    pub name: String,
    /// The height of the strip at the top that the cutout takes; the line of the chip is below it.
    pub top: f64,
}

pub struct ViewIvars {
    spec: RefCell<ZonesSpec>,
    hover: Cell<Option<usize>>,
    /// The layout whose cells show over the screen, outlined in its tile while the pointer is elsewhere.
    selected: Cell<Option<usize>>,
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
                names: Vec::new(),
                chip: None,
            }),
            hover: Cell::new(None),
            selected: Cell::new(None),
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
        if let Some(chip) = &spec.chip {
            paint_chip(bounds, chip);
            return;
        }
        let hover = self.ivars().hover.get();
        let selected = self.ivars().selected.get();
        let font = style::font(11.0, Weight::Medium);
        for (i, cell) in spec.cells.iter().enumerate() {
            let rect = NSRect::new(NSPoint::new(cell.x, cell.y), NSSize::new(cell.w, cell.h));
            let active = hover == Some(i);
            // The layout the pointer is on is solid white with dark ink, its cells in the accent colour of the
            // footprint on the screen; the others rest as translucent controls, and the one whose cells are on the
            // screen while the pointer is elsewhere keeps an outline.
            let fill = style::white(if active { 1.0 } else { style::CONTROL });
            style::fill_rounded(rect, BUTTON_RADIUS, &fill);
            if selected == Some(i) && !active {
                style::stroke_rounded(rect, BUTTON_RADIUS, &NSColor::controlAccentColor(), 1.5);
            }
            let (ink, part) = if active {
                (style::black(0.85), NSColor::controlAccentColor())
            } else {
                (style::white(style::TEXT), style::white(style::TEXT))
            };
            if let Some(parts) = spec.glyphs.get(i) {
                let (w, h) = GLYPH;
                let screen = NSRect::new(
                    NSPoint::new(
                        rect.origin.x + (rect.size.width - w) / 2.0,
                        rect.origin.y + GLYPH_TOP,
                    ),
                    NSSize::new(w, h),
                );
                draw_glyph(screen, parts, &ink, &part);
            }
            if let Some(name) = spec.names.get(i) {
                let color = if active {
                    style::black(0.85)
                } else {
                    style::white(0.72)
                };
                let line = NSRect::new(
                    NSPoint::new(
                        rect.origin.x + 4.0,
                        rect.origin.y + GLYPH_TOP + GLYPH.1 + 5.0,
                    ),
                    NSSize::new(rect.size.width - 8.0, 15.0),
                );
                let attrs = widgets::text_attributes(&font, &color, Fit::Tail, true);
                widgets::draw_text(name, line, &attrs);
            }
        }
    }
}

/// The chip, centred on its line below the strip the cutout takes.
fn paint_chip(bounds: NSRect, chip: &ChipSpec) {
    let screen = NSRect::new(
        NSPoint::new(CHIP_MARGIN, chip.top + (CHIP_LINE - CHIP_GLYPH.1) / 2.0),
        NSSize::new(CHIP_GLYPH.0, CHIP_GLYPH.1),
    );
    draw_glyph(
        screen,
        &chip.parts,
        &style::white(style::TEXT),
        &style::white(style::TEXT),
    );
    let font = style::font(12.0, Weight::Medium);
    let text = NSRect::new(
        NSPoint::new(
            screen.origin.x + screen.size.width + 9.0,
            chip.top + (CHIP_LINE - 16.0) / 2.0,
        ),
        NSSize::new(
            (bounds.size.width - screen.size.width - CHIP_MARGIN * 2.0 - 9.0).max(0.0),
            16.0,
        ),
    );
    let attrs = widgets::text_attributes(&font, &style::white(0.9), Fit::Tail, false);
    widgets::draw_text(&chip.name, text, &attrs);
}

/// A small screen with the part each cell of a layout fills, in `screen`: an outline, and the cells inside it with a
/// hair of space between them when there is more than one.
fn draw_glyph(screen: NSRect, parts: &[[f64; 4]], ink: &NSColor, fill: &NSColor) {
    style::fill_rounded(screen, 5.0, &ink.colorWithAlphaComponent(0.08));
    style::stroke_rounded(screen, 5.0, &ink.colorWithAlphaComponent(0.5), 1.25);
    let inner = NSRect::new(
        NSPoint::new(screen.origin.x + 3.0, screen.origin.y + 3.0),
        NSSize::new(screen.size.width - 6.0, screen.size.height - 6.0),
    );
    let space = if parts.len() > 1 { 0.75 } else { 0.0 };
    for part in parts {
        let rect = NSRect::new(
            NSPoint::new(
                inner.origin.x + part[0] * inner.size.width + space,
                inner.origin.y + part[1] * inner.size.height + space,
            ),
            NSSize::new(
                (part[2] * inner.size.width - 2.0 * space).max(0.5),
                (part[3] * inner.size.height - 2.0 * space).max(0.5),
            ),
        );
        style::fill_rounded(rect, 2.0, fill);
    }
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

    /// Moves the panel from where it is to `open` at height `to_height`, hanging from the top edge of `open`, and its
    /// opacity to `alpha`. The width follows, since the row becomes a chip and back.
    fn animate(
        &self,
        open: NSRect,
        to_height: f64,
        seconds: f64,
        curve: fn(f64) -> f64,
        alpha: f64,
        done: impl Fn() + 'static,
    ) {
        let from = self.panel.frame();
        let from_alpha = self.panel.alphaValue();
        let (started, weak) = (Instant::now(), self.me.clone());
        *self.anim.borrow_mut() = Some(app::every(FRAME_SECONDS, move || {
            let Some(inner) = weak.upgrade() else {
                return;
            };
            let t = (started.elapsed().as_secs_f64() / seconds).min(1.0);
            let k = curve(t);
            let h = lerp(from.size.height, to_height, k).max(1.0);
            let w = lerp(from.size.width, open.size.width, k);
            let x = lerp(from.origin.x, open.origin.x, k);
            inner.panel.setFrame_display(
                NSRect::new(
                    NSPoint::new(x, open.origin.y + open.size.height - h),
                    NSSize::new(w, h),
                ),
                true,
            );
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

/// The row of layouts, a non-activating panel that grows out of the top edge of a screen. It never takes focus
/// and passes every click through.
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

    /// Shows the shape hanging from the top edge at `top_left` (the corner of the shape in `Frame` coordinates),
    /// with the layout `hover` lit and the layout `selected` outlined. Called again with the same place and the same
    /// spec it only moves the highlight; with another spec, the row or the chip, the shape moves to the new size.
    pub fn show(
        &self,
        top_left: (f64, f64),
        spec: &ZonesSpec,
        hover: Option<usize>,
        selected: Option<usize>,
    ) {
        let inner = &self.inner;
        inner.view.ivars().hover.set(hover);
        inner.view.ivars().selected.set(selected);
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
        // Grown out of the edge when it first appears; eased from one shape to the other afterwards.
        let appearing = !inner.panel.isVisible();
        if appearing {
            inner.panel.setAlphaValue(0.0);
            inner
                .panel
                .setFrame_display(Inner::frame_with_height(open, REST_HEIGHT), false);
            inner.panel.orderFrontRegardless();
        }
        let (seconds, curve): (f64, fn(f64) -> f64) = if appearing {
            (OPEN_SECONDS, style::spring)
        } else {
            (RESIZE_SECONDS, style::ease_out)
        };
        inner.animate(open, open.size.height, seconds, curve, 1.0, || {});
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

    /// Draws the shape with the layout `hover` lit into PNG bytes without showing anything: the view is laid out at
    /// its full size and drawn from there, at two pixels per point as a Retina screen shows it. Transparent outside
    /// the shape. For checking the design.
    pub fn snapshot_png(
        &self,
        spec: &ZonesSpec,
        hover: Option<usize>,
        selected: Option<usize>,
    ) -> Option<Vec<u8>> {
        let view = &self.inner.view;
        view.adopt(spec);
        view.ivars().hover.set(hover);
        view.ivars().selected.set(selected);
        view.setFrameSize(NSSize::new(spec.width, spec.height));
        let bounds = view.bounds();
        // SAFETY: a plain RGBA bitmap with valid dimensions.
        let rep = unsafe {
            NSBitmapImageRep::initWithBitmapDataPlanes_pixelsWide_pixelsHigh_bitsPerSample_samplesPerPixel_hasAlpha_isPlanar_colorSpaceName_bytesPerRow_bitsPerPixel(
                NSBitmapImageRep::alloc(),
                std::ptr::null_mut(),
                (bounds.size.width * 2.0).round().max(1.0) as isize,
                (bounds.size.height * 2.0).round().max(1.0) as isize,
                8,
                4,
                true,
                false,
                NSDeviceRGBColorSpace,
                0,
                0,
            )
        }?;
        // Drawing in points lands on two pixels each.
        rep.setSize(bounds.size);
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

// ---------------------------------------------------------------- where the window will land

/// The footprint filling `rect`: a soft wash of the system accent colour inside a thin border of it. Shared by the
/// panel and the design check, which draws it over a pretend desktop.
pub fn paint_footprint(rect: NSRect) {
    let accent = NSColor::controlAccentColor();
    style::fill_rounded(rect, FOOTPRINT_RADIUS, &accent.colorWithAlphaComponent(0.2));
    style::stroke_rounded(
        rect,
        FOOTPRINT_RADIUS,
        &accent.colorWithAlphaComponent(0.9),
        2.0,
    );
}

define_class!(
    // SAFETY: NSView allows subclassing; it only draws the footprint over its bounds.
    #[unsafe(super(NSView))]
    #[thread_kind = MainThreadOnly]
    #[name = "FigSnapFootprint"]
    struct FootprintView;

    impl FootprintView {
        #[unsafe(method(isOpaque))]
        fn is_opaque(&self) -> bool {
            false
        }

        #[unsafe(method(drawRect:))]
        fn draw_rect(&self, _dirty: NSRect) {
            paint_footprint(self.bounds());
        }
    }
);

struct Glide {
    panel: Retained<SnapPanel>,
    /// Where it is heading, in Cocoa coordinates; `None` while it fades away or is gone.
    target: Cell<Option<NSRect>>,
    anim: RefCell<Option<app::Timer>>,
    me: Weak<Glide>,
}

impl Glide {
    /// Moves the panel to `to` and its opacity to `alpha` on an ease-out curve, from wherever they are now.
    fn animate(&self, to: NSRect, alpha: f64, seconds: f64, done: impl Fn() + 'static) {
        let (from, from_alpha) = (self.panel.frame(), self.panel.alphaValue());
        let (started, weak) = (Instant::now(), self.me.clone());
        *self.anim.borrow_mut() = Some(app::every(FRAME_SECONDS, move || {
            let Some(glide) = weak.upgrade() else {
                return;
            };
            let t = (started.elapsed().as_secs_f64() / seconds).min(1.0);
            let k = style::ease_out(t);
            if from != to {
                let frame = NSRect::new(
                    NSPoint::new(
                        lerp(from.origin.x, to.origin.x, k),
                        lerp(from.origin.y, to.origin.y, k),
                    ),
                    NSSize::new(
                        lerp(from.size.width, to.size.width, k),
                        lerp(from.size.height, to.size.height, k),
                    ),
                );
                glide.panel.setFrame_display(frame, true);
            }
            glide.panel.setAlphaValue(lerp(from_alpha, alpha, k));
            if t >= 1.0 {
                glide.anim.borrow_mut().take();
                done();
            }
        }));
    }
}

/// Where the dragged window will land: a translucent rounded rectangle on the screen, under the zones and over
/// every ordinary window. It never takes focus, passes every click through and stays out of the window cycle.
pub struct Footprint {
    mtm: MainThreadMarker,
    glide: Rc<Glide>,
}

impl Footprint {
    pub fn new(mtm: MainThreadMarker) -> Self {
        let panel = SnapPanel::new(mtm);
        // Just under the zones, which are one level higher.
        panel.setLevel(NSMainMenuWindowLevel + 1);
        // SAFETY: designated initializer with valid arguments on the main thread.
        let view: Retained<FootprintView> = unsafe {
            msg_send![
                super(mtm.alloc::<FootprintView>().set_ivars(())),
                initWithFrame: NSRect::ZERO
            ]
        };
        panel.setContentView(Some(&view));
        let glide = Rc::new_cyclic(|me| Glide {
            panel,
            target: Cell::new(None),
            anim: RefCell::new(None),
            me: me.clone(),
        });
        Self { mtm, glide }
    }

    /// Shows the footprint over `frame`. Already showing, it glides there from where it is, or jumps with Reduce
    /// Motion; it fades in and out either way.
    pub fn show(&self, frame: Frame) {
        let glide = &self.glide;
        let to = NSRect::new(
            NSPoint::new(frame.x, primary_height(self.mtm) - (frame.y + frame.h)),
            NSSize::new(frame.w, frame.h),
        );
        if glide.target.replace(Some(to)) == Some(to) {
            return;
        }
        let panel = &glide.panel;
        let arriving = !panel.isVisible();
        if arriving || workspace::display_options().reduce_motion {
            panel.setFrame_display(to, true);
        }
        if arriving {
            panel.setAlphaValue(0.0);
            panel.orderFrontRegardless();
        }
        let seconds = if arriving {
            FADE_IN_SECONDS
        } else {
            GLIDE_SECONDS
        };
        glide.animate(to, 1.0, seconds, || {});
    }

    /// Fades the footprint away where it is.
    pub fn hide(&self) {
        let glide = &self.glide;
        if glide.target.take().is_none() {
            return;
        }
        let weak = glide.me.clone();
        glide.animate(glide.panel.frame(), 0.0, FADE_OUT_SECONDS, move || {
            if let Some(glide) = weak.upgrade() {
                glide.panel.orderOut(None);
            }
        });
    }
}

impl Drop for Footprint {
    fn drop(&mut self) {
        self.glide.anim.borrow_mut().take();
        self.glide.panel.orderOut(None);
        self.glide.panel.close();
    }
}

// ---------------------------------------------------------------- the cells of the chosen layout

pub struct CellsIvars {
    /// The cells in the view's own coordinates, with the origin at the bottom left.
    cells: RefCell<Vec<NSRect>>,
}

define_class!(
    // SAFETY: NSView allows subclassing; the ivars are only touched on the main thread and need no Drop.
    #[unsafe(super(NSView))]
    #[thread_kind = MainThreadOnly]
    #[name = "FigSnapCells"]
    #[ivars = CellsIvars]
    struct CellsView;

    impl CellsView {
        #[unsafe(method(isOpaque))]
        fn is_opaque(&self) -> bool {
            false
        }

        #[unsafe(method(drawRect:))]
        fn draw_rect(&self, _dirty: NSRect) {
            for cell in self.ivars().cells.borrow().iter() {
                paint_cell(*cell);
            }
        }
    }
);

/// One cell as it rests: a faint shade with a light line inside a dark one, so that it shows on a light desktop and
/// on a dark one alike, with a space between it and the next cell.
fn paint_cell(cell: NSRect) {
    let inset = |r: NSRect, by: f64| {
        NSRect::new(
            NSPoint::new(r.origin.x + by, r.origin.y + by),
            NSSize::new(r.size.width - 2.0 * by, r.size.height - 2.0 * by),
        )
    };
    let rect = inset(cell, CELL_GAP / 2.0);
    style::fill_rounded(rect, CELL_RADIUS, &style::black(0.14));
    style::stroke_rounded(rect, CELL_RADIUS, &style::black(0.3), 3.0);
    style::stroke_rounded(
        inset(rect, 1.0),
        CELL_RADIUS - 1.0,
        &style::white(0.75),
        1.5,
    );
}

/// The cells of the chosen layout over the part of the screen they fill, under the footprint that lights one of
/// them and over every ordinary window. It never takes focus, passes every click through and stays out of the
/// window cycle.
pub struct CellsOverlay {
    mtm: MainThreadMarker,
    view: Retained<CellsView>,
    glide: Rc<Glide>,
}

impl CellsOverlay {
    pub fn new(mtm: MainThreadMarker) -> Self {
        let panel = SnapPanel::new(mtm);
        // Under the footprint, which is one level higher, and the row, which is two.
        panel.setLevel(NSMainMenuWindowLevel);
        let view: Retained<CellsView> = {
            let ivars = CellsIvars {
                cells: RefCell::new(Vec::new()),
            };
            // SAFETY: designated initializer with valid arguments on the main thread.
            unsafe {
                msg_send![super(mtm.alloc::<CellsView>().set_ivars(ivars)), initWithFrame: NSRect::ZERO]
            }
        };
        panel.setContentView(Some(&view));
        let glide = Rc::new_cyclic(|me| Glide {
            panel,
            target: Cell::new(None),
            anim: RefCell::new(None),
            me: me.clone(),
        });
        Self { mtm, view, glide }
    }

    /// Shows `cells` over `area`, both in `Frame` coordinates. Already showing, only the cells change; it fades in
    /// and out otherwise.
    pub fn show(&self, area: Frame, cells: &[Frame]) {
        let to = NSRect::new(
            NSPoint::new(area.x, primary_height(self.mtm) - (area.y + area.h)),
            NSSize::new(area.w, area.h),
        );
        *self.view.ivars().cells.borrow_mut() = cells
            .iter()
            .map(|c| {
                NSRect::new(
                    NSPoint::new(c.x - area.x, area.h - (c.y - area.y) - c.h),
                    NSSize::new(c.w, c.h),
                )
            })
            .collect();
        self.view.setNeedsDisplay(true);
        let glide = &self.glide;
        if glide.target.replace(Some(to)) == Some(to) && glide.panel.isVisible() {
            return;
        }
        let panel = &glide.panel;
        panel.setFrame_display(to, true);
        if !panel.isVisible() {
            panel.setAlphaValue(0.0);
            panel.orderFrontRegardless();
        }
        glide.animate(to, 1.0, FADE_IN_SECONDS, || {});
    }

    /// Fades the cells away where they are.
    pub fn hide(&self) {
        let glide = &self.glide;
        if glide.target.take().is_none() {
            return;
        }
        let weak = glide.me.clone();
        glide.animate(glide.panel.frame(), 0.0, FADE_OUT_SECONDS, move || {
            if let Some(glide) = weak.upgrade() {
                glide.panel.orderOut(None);
            }
        });
    }

    pub fn is_shown(&self) -> bool {
        self.glide.target.get().is_some()
    }

    /// Draws the cells in `area`-sized PNG bytes without showing anything, at two pixels per point, on a clear
    /// background. For checking the design.
    pub fn snapshot_png(&self, area: Frame, cells: &[Frame]) -> Option<Vec<u8>> {
        *self.view.ivars().cells.borrow_mut() = cells
            .iter()
            .map(|c| {
                NSRect::new(
                    NSPoint::new(c.x - area.x, area.h - (c.y - area.y) - c.h),
                    NSSize::new(c.w, c.h),
                )
            })
            .collect();
        self.view.setFrameSize(NSSize::new(area.w, area.h));
        widgets::view_png(&self.view)
    }
}

impl Drop for CellsOverlay {
    fn drop(&mut self) {
        self.glide.anim.borrow_mut().take();
        self.glide.panel.orderOut(None);
        self.glide.panel.close();
    }
}
