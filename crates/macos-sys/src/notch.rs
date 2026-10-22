//! The notch surface: a shape that grows out of the top edge of the screen. At rest it covers the hardware cutout
//! (on a screen without one it is a notch-shaped tab, or only an invisible strip); while something is live, such as
//! music or a working coding agent, it widens into two ears beside the cutout; open, it hosts the tabs.
//!
//! The window is resized only when a movement starts or ends. The movement is laid out frame by frame on the
//! display's own clock, a spring computed here: Liquid Glass is drawn by SwiftUI inside its view and cannot be moved by
//! a Core Animation animation from outside, so everything in the shape, glass included, gets the shape's size on every
//! frame instead. Only the fades are Core Animation's.

use crate::{
    app, labels,
    panel::{FigPanel, Key},
    pasteboard::file_paths,
    silhouette::{self, Outline},
    style::{self, PANEL_RADIUS, Tone, Weight, white},
    switcher::{self, NotchOrb, PageSwitcher},
    ui::{AppCallbacks, Callbacks, ClipUi, ClipView, ListStyle, handle_key},
    widgets::{Fit, draw_text, text_attributes},
};
use block2::RcBlock;
use objc2::{
    AnyThread, DefinedClass, MainThreadMarker, MainThreadOnly, Message, define_class, msg_send,
    rc::Retained,
    runtime::{AnyClass, AnyObject, ProtocolObject},
    sel,
};
use objc2_app_kit::{
    NSAnimatablePropertyContainer, NSAnimationContext, NSApplication,
    NSApplicationDidChangeScreenParametersNotification, NSBezierPath, NSBitmapImageFileType,
    NSColor, NSCompositingOperation, NSDragOperation, NSDraggingDestination, NSDraggingInfo,
    NSEvent, NSEventMask, NSGlassEffectView, NSGlassEffectViewStyle, NSGradient,
    NSHapticFeedbackManager, NSHapticFeedbackPattern, NSHapticFeedbackPerformanceTime,
    NSHapticFeedbackPerformer, NSImage, NSMenu, NSPasteboardTypeFileURL, NSScreen, NSStatusBar,
    NSStringDrawing, NSTrackingArea, NSTrackingAreaOptions, NSView, NSVisualEffectBlendingMode,
    NSVisualEffectMaterial, NSVisualEffectState, NSVisualEffectView,
    NSWindowDidResignKeyNotification, NSWindowSharingType,
};
use objc2_foundation::{
    NSArray, NSData, NSDictionary, NSNotification, NSNotificationCenter, NSNumber, NSObject,
    NSObjectProtocol, NSPoint, NSRect, NSRunLoop, NSRunLoopCommonModes, NSSize, NSString,
};
use objc2_quartz_core::{
    CABasicAnimation, CACornerMask, CACurrentMediaTime, CADisplayLink, CALayer, CAMediaTiming,
    CAMediaTimingFunction, CAShapeLayer, CATransaction, kCACornerCurveContinuous,
};
use std::{
    cell::{Cell, RefCell},
    ptr::NonNull,
    rc::{Rc, Weak},
    sync::Arc,
};

pub const EXPANDED_SIZE: NSSize = NSSize::new(500.0, 440.0);
/// The width of the clipboard page, a strip of cards that scrolls sideways.
const STRIP_WIDTH: f64 = 660.0;
/// What the strip page needs below the room the header takes: the chips, the cards and the hint.
const STRIP_BODY: f64 = 254.0;
/// Height reserved above the pages for the tab bar.
pub const TAB_BAR_HEIGHT: f64 = 32.0;
/// Space between the left edge of the open notch and the page icons.
const SWITCHER_INSET: f64 = 16.0;
/// Room kept between the header and the cutout when the header sits beside it.
const BAND_GAP: f64 = 8.0;
/// Width of the resting shape on a screen without a hardware notch.
pub const PILL_WIDTH: f64 = 200.0;
const HOT_ZONE_HEIGHT: f64 = 4.0;
const TRIGGER_MIN_HEIGHT: f64 = 8.0;
const TRIGGER_SIDE_SLACK: f64 = 10.0;
/// The bottom corners of the resting shape, close to the hardware notch's own.
const COLLAPSED_RADIUS: f64 = 10.0;
const HOVER_CLOSE_DELAY: f64 = 0.35;
/// The narrowest an ear beside the cutout gets, as a share of its height; wide enough for a square picture.
const EAR_MIN_SHARE: f64 = 1.2;
/// Wide enough for an agent and its project ("Claude figghost"); longer text ends in an ellipsis.
const EAR_MAX: f64 = 128.0;
const EAR_PAD: f64 = 9.0;
/// How often moving bars are drawn while they are on screen.
const BARS_FRAME_SECONDS: f64 = 1.0 / 30.0;
const BAR_COUNT: usize = 4;
/// How far a page slides in from the side while it fades in, and how long that takes.
const PAGE_SLIDE: f64 = 18.0;
const PAGE_SLIDE_SECONDS: f64 = 0.30;
/// The radius of the concave corners where the open shape meets the top edge of the screen. The window is this much
/// wider than the shape on each side while it is open, so the fillets have room to be drawn.
const FLARE: f64 = 20.0;
/// The height of the smallest open shape: a shape this tall counts as fully open, one as tall as the closed shape
/// as closed, and in between it is mid-movement (see `silhouette::openness`).
const OPEN_REFERENCE: f64 = 150.0;
/// How far below the top edge the black that joins the shape to the screen's edge reaches, past the cutout.
const BAND_DEPTH: f64 = 46.0;
/// The height a detail line pair adds below the ears, and the room it leaves at the sides of its text.
const DETAIL_DROP: f64 = 34.0;
const DETAIL_PAD: f64 = 16.0;
/// How long a shape takes to fade in or out when nothing may move.
const DISSOLVE_SECONDS: f64 = 0.18;

/// What the open notch is made of.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Material {
    /// Liquid Glass with a dark tint: the desktop shows through, bent at the edges.
    Glass,
    /// Liquid Glass with a light tint, more of the desktop shows through.
    ClearGlass,
    /// Liquid Glass with a heavy dark tint: darker than `Glass` without becoming opaque, so the desktop is
    /// still there, folded along the top of the shape.
    Ink,
    /// Clear glass tinted with the accent colour the user chose: the one material that is a colour rather
    /// than a shade.
    Accent,
    /// Black, slightly lighter at the bottom, like the hardware notch grown larger.
    Black,
}

/// The Liquid Glass style and tint of a material, or `None` for the ones that are not glass. The tint is the
/// colour the glass is darkened or coloured with.
pub fn glass_tint(material: Material) -> Option<(NSGlassEffectViewStyle, Retained<NSColor>)> {
    let tint = |alpha: f64| style::black(alpha);
    Some(match material {
        Material::Glass => (NSGlassEffectViewStyle::Regular, tint(0.42)),
        Material::ClearGlass => (NSGlassEffectViewStyle::Clear, tint(0.18)),
        Material::Ink => (NSGlassEffectViewStyle::Regular, tint(0.72)),
        Material::Accent => (
            NSGlassEffectViewStyle::Clear,
            NSColor::controlAccentColor().colorWithAlphaComponent(0.55),
        ),
        Material::Black => return None,
    })
}

/// The vibrancy material standing in for a glass one before macOS 26. The tint cannot be applied to it, so
/// the two darker materials take the darker material instead.
pub fn vibrancy_of(material: Material) -> NSVisualEffectMaterial {
    match material {
        Material::Ink => NSVisualEffectMaterial::FullScreenUI,
        _ => NSVisualEffectMaterial::HUDWindow,
    }
}

/// How the shape moves between its states.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Motion {
    /// One small overshoot, which is what makes a surface feel like it has weight.
    Spring,
    /// Fast at first and slowing down, without overshoot.
    Smooth,
    /// No movement at all; Reduce Motion has the same effect.
    Instant,
}

/// What a screen without a hardware notch shows while nothing is happening.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Rest {
    /// A black tab the size of a notch, centred under the top edge.
    Notch,
    /// Nothing visible; the pointer at the top edge still opens it.
    Hidden,
}

/// Which screen the notch lives on.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Display {
    /// The built-in screen with a notch, else the main screen.
    Auto,
    /// The screen with the menu bar.
    Main,
    /// Whichever screen the pointer is on.
    Pointer,
    /// One screen by name, as the settings window offers them; a screen that is gone falls back to `Auto`.
    Screen(String),
}

/// The tab the notch shows when it opens.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OpenPage {
    /// The one that was showing when it closed.
    Last,
    /// The tab of what is live (playing music, a working agent), else the last one.
    Activity,
    /// Always this tab (a page id).
    Page(String),
}

/// How the notch looks and behaves; the settings window edits it.
#[derive(Debug, Clone, PartialEq)]
pub struct Style {
    pub material: Material,
    pub shadow: bool,
    pub motion: Motion,
    pub open_on_hover: bool,
    /// Seconds the pointer rests on the notch before it opens.
    pub hover_delay: f64,
    pub rest: Rest,
    /// Leaves the notch out of screenshots and screen recordings, this app's own included.
    pub hide_in_capture: bool,
    /// A tap on a Force Touch trackpad when the notch opens under the pointer.
    pub haptics: bool,
    pub display: Display,
    pub open_page: OpenPage,
    /// The ids of the tabs to show, in order. Empty shows every tab, in the order they were added.
    pub pages: Vec<String>,
    /// The least width of the open shape in points; 0 lets each page decide. A page narrower than this stays centred
    /// in it.
    pub min_width: f64,
    /// The least height of the open shape in points; 0 lets each page decide.
    pub min_height: f64,
    /// The size of the resting shape on a screen without a hardware notch; 0 keeps the default for that side.
    pub rest_width: f64,
    pub rest_height: f64,
}

impl Default for Style {
    fn default() -> Self {
        Self {
            material: Material::Glass,
            shadow: true,
            motion: Motion::Spring,
            open_on_hover: true,
            hover_delay: 0.3,
            rest: Rest::Notch,
            hide_in_capture: false,
            haptics: true,
            display: Display::Auto,
            open_page: OpenPage::Activity,
            pages: Vec::new(),
            min_width: 0.0,
            min_height: 0.0,
            rest_width: 0.0,
            rest_height: 0.0,
        }
    }
}

/// Notch position in AppKit screen coordinates (origin bottom-left).
#[derive(Debug, Clone, Copy)]
pub struct NotchGeometry {
    pub screen: NSRect,
    /// Hardware notch size, or the pill size on displays without one.
    pub notch: NSSize,
    pub hardware: bool,
}

/// The height of the menu bar. `NSStatusBar.thickness()` still answers 22 where the bar is drawn 30 high
/// (macOS 27), and the tab at rest has to fill the bar to look like a cutout, so the menu bar is asked for its
/// own height and the status bar is only the fallback.
fn menu_bar_height() -> f64 {
    let thickness = NSStatusBar::systemStatusBar().thickness();
    let Some(mtm) = MainThreadMarker::new() else {
        return thickness;
    };
    let height = NSApplication::sharedApplication(mtm)
        .mainMenu()
        .map_or(0.0, |menu| menu.menuBarHeight());
    if height > 0.0 { height } else { thickness }
}

impl NotchGeometry {
    /// Prefers the notched built-in display; falls back to the menu-bar screen with a pill-sized notch.
    pub fn detect(mtm: MainThreadMarker) -> Option<Self> {
        Self::detect_on(mtm, Display::Auto)
    }

    /// The geometry on the screen that `display` picks.
    pub fn detect_on(mtm: MainThreadMarker, display: Display) -> Option<Self> {
        // For tests: far outside every screen, so a notch that opens on its own does not flash on the real one.
        if std::env::var_os("FIGGHOST_NOTCH_OFFSCREEN").is_some() {
            return Some(Self {
                screen: NSRect::new(NSPoint::new(-30_000.0, 0.0), NSSize::new(1_920.0, 1_080.0)),
                notch: NSSize::new(PILL_WIDTH, 24.0),
                hardware: false,
            });
        }
        let screens = NSScreen::screens(mtm);
        let named = |name: &str| {
            screens
                .iter()
                .find(|s| s.localizedName().to_string() == name)
        };
        let chosen = match display {
            Display::Auto => screens
                .iter()
                .find(|s| s.safeAreaInsets().top > 0.0)
                .or_else(|| screens.firstObject()),
            Display::Main => screens.firstObject(),
            Display::Pointer => {
                let p = NSEvent::mouseLocation();
                screens
                    .iter()
                    .find(|s| contains(s.frame(), p))
                    .or_else(|| screens.firstObject())
            }
            Display::Screen(ref name) => named(name).or_else(|| screens.firstObject()),
        }?;
        Some(Self::of_screen(&chosen))
    }

    /// The geometry of the screen that holds `point`: where something that follows the pointer (the mascot)
    /// plays, even when the notch itself is on another screen.
    pub fn detect_at(mtm: MainThreadMarker, point: NSPoint) -> Option<Self> {
        // For tests: the notch is far outside every screen, so nothing of it may show on the real one.
        if std::env::var_os("FIGGHOST_NOTCH_OFFSCREEN").is_some() {
            return None;
        }
        let screens = NSScreen::screens(mtm);
        let chosen = screens
            .iter()
            .find(|s| contains(s.frame(), point))
            .or_else(|| screens.firstObject())?;
        Some(Self::of_screen(&chosen))
    }

    /// The notch of one screen: its cutout, or a pill as tall as the menu bar.
    pub fn of_screen(s: &NSScreen) -> Self {
        if s.safeAreaInsets().top > 0.0 {
            let (left, right) = (s.auxiliaryTopLeftArea(), s.auxiliaryTopRightArea());
            let width = right.origin.x - (left.origin.x + left.size.width);
            return Self {
                screen: s.frame(),
                notch: NSSize::new(width, s.safeAreaInsets().top),
                hardware: true,
            };
        }
        Self {
            screen: s.frame(),
            notch: NSSize::new(PILL_WIDTH, menu_bar_height()),
            hardware: false,
        }
    }

    /// A rect of the given size hugging the top edge, horizontally centered on the screen.
    pub fn frame(&self, width: f64, height: f64) -> NSRect {
        let (s, size) = (self.screen, NSSize::new(width, height));
        let origin = NSPoint::new(
            s.origin.x + (s.size.width - width) / 2.0,
            s.origin.y + s.size.height - height,
        );
        NSRect::new(origin, size)
    }

    /// The resting shape with nothing visible on a screen without a notch: exactly the cutout, or a thin hot zone.
    pub fn collapsed_frame(&self) -> NSRect {
        let size = self.rest_size(Rest::Hidden);
        self.frame(size.width, size.height)
    }

    /// The size of the resting shape.
    pub fn rest_size(&self, rest: Rest) -> NSSize {
        match (self.hardware, rest) {
            (true, _) | (false, Rest::Notch) => self.notch,
            (false, Rest::Hidden) => NSSize::new(PILL_WIDTH, HOT_ZONE_HEIGHT),
        }
    }

    /// Where the pointer must be to open a shape of this size. Taller than the shape when it is a thin strip, and
    /// checked against raw pointer positions: the topmost rows of the screen never deliver events to a window, so a
    /// tracking area alone would miss a pointer pushed against the edge.
    pub fn trigger_for(&self, size: NSSize) -> NSRect {
        let height = size.height.max(TRIGGER_MIN_HEIGHT);
        self.frame(size.width + 2.0 * TRIGGER_SIDE_SLACK, height)
    }

    pub fn trigger_rect(&self) -> NSRect {
        self.trigger_for(self.collapsed_frame().size)
    }

    pub fn expanded_frame(&self) -> NSRect {
        self.frame(EXPANDED_SIZE.width, EXPANDED_SIZE.height)
    }

    /// Room the content leaves free at the top so it clears the cutout.
    pub fn top_inset(&self) -> f64 {
        if self.hardware {
            self.notch.height + 6.0
        } else {
            10.0
        }
    }
}

/// Fills its slice with levels from 0 to 1, one per bar.
pub type Levels = Rc<dyn Fn(&mut [f32])>;

/// What one side of the closed notch shows while something is live.
#[derive(Clone)]
pub enum Ear {
    Empty,
    /// An SF Symbol in a colour; `pulse` makes it breathe, for something that waits for the user.
    Symbol {
        name: &'static str,
        tone: Tone,
        pulse: bool,
    },
    /// A short text, such as a percentage or a count.
    Text {
        text: String,
        tone: Tone,
    },
    /// Encoded image bytes (album art), drawn with rounded corners. `id` tells a new picture from the same one.
    Artwork {
        id: u64,
        bytes: Arc<Vec<u8>>,
    },
    /// Bars that move with `levels` (0 to 1 each), asked about 30 times a second while they are on screen.
    Bars {
        levels: Levels,
        /// sRGB, 0 to 1.
        color: [f64; 3],
    },
}

impl Ear {
    /// How wide this side needs to be at the given height.
    fn width(&self, height: f64) -> f64 {
        let least = height * EAR_MIN_SHARE;
        match self {
            Ear::Text { text, .. } => {
                let font = style::font(text_size(height), Weight::Semibold);
                let attrs = text_attributes(&font, &white(1.0), Fit::Clip, false);
                // SAFETY: the attributes dictionary holds valid attribute objects.
                let width =
                    unsafe { NSString::from_str(text).sizeWithAttributes(Some(&attrs)) }.width;
                (width.ceil() + 2.0 * EAR_PAD).clamp(least, EAR_MAX)
            }
            Ear::Bars { .. } => least.max(BAR_COUNT as f64 * 5.5 + 2.0 * EAR_PAD),
            _ => least,
        }
    }
}

fn text_size(height: f64) -> f64 {
    (height * 0.45).clamp(11.0, 13.5)
}

/// Two lines of text under the ears of the closed notch: the track that just started, the device that connected.
/// The shape grows down by `DETAIL_DROP` to hold them, and goes back when the activity is replaced without them.
#[derive(Clone, PartialEq)]
pub struct Detail {
    pub title: String,
    pub subtitle: String,
}

impl Detail {
    fn fonts() -> (
        Retained<objc2_app_kit::NSFont>,
        Retained<objc2_app_kit::NSFont>,
    ) {
        (
            style::font(12.5, Weight::Semibold),
            style::font(10.5, Weight::Regular),
        )
    }

    /// How wide the shape has to be to show both lines.
    fn width(&self) -> f64 {
        let (title, subtitle) = Self::fonts();
        let measure = |text: &str, font: &objc2_app_kit::NSFont| {
            let attrs = text_attributes(font, &white(1.0), Fit::Clip, false);
            // SAFETY: the attributes dictionary holds valid attribute objects.
            unsafe { NSString::from_str(text).sizeWithAttributes(Some(&attrs)) }.width
        };
        measure(&self.title, &title)
            .max(measure(&self.subtitle, &subtitle))
            .ceil()
            + 2.0 * DETAIL_PAD
    }
}

/// Something live, shown in the closed notch.
#[derive(Clone)]
pub struct Activity {
    pub left: Ear,
    pub right: Ear,
    /// Two lines under the ears, when there is something to say that the ears cannot.
    pub detail: Option<Detail>,
    /// The tab that opens from it: on a click, and on a hover with the "activity" opening rule.
    pub page: Option<usize>,
    /// The highest priority is shown; among equals, the one that started last.
    pub priority: u8,
}

type Signal = Rc<dyn Fn()>;
type DragSignal = Rc<dyn Fn(bool)>;
type Resize = Rc<dyn Fn(NSSize)>;

pub struct RootIvars {
    /// Called when the pointer enters or leaves the window.
    on_pointer: RefCell<Option<Signal>>,
    on_click: RefCell<Option<Signal>>,
    /// Called with true when a drag carrying files enters the window and false when it leaves.
    on_drag: RefCell<Option<DragSignal>>,
    /// While the notch is open, a click is the pages' business.
    open: Cell<bool>,
}

define_class!(
    // SAFETY: NSView allows subclassing; the ivars are only touched on the main thread and need no Drop.
    #[unsafe(super(NSView))]
    #[thread_kind = MainThreadOnly]
    #[name = "FigNotchRoot"]
    #[ivars = RootIvars]
    struct RootView;

    impl RootView {
        #[unsafe(method(acceptsFirstMouse:))]
        fn accepts_first_mouse(&self, _event: Option<&NSEvent>) -> bool {
            true
        }

        #[unsafe(method(mouseEntered:))]
        fn mouse_entered(&self, _event: &NSEvent) {
            fire(&self.ivars().on_pointer);
        }

        #[unsafe(method(mouseExited:))]
        fn mouse_exited(&self, _event: &NSEvent) {
            fire(&self.ivars().on_pointer);
        }

        #[unsafe(method(mouseDown:))]
        fn mouse_down(&self, _event: &NSEvent) {
            if !self.ivars().open.get() {
                fire(&self.ivars().on_click);
            }
        }
    }

    unsafe impl NSObjectProtocol for RootView {}

    // The window is a drag destination only to notice files arriving: it opens the notch on the shelf. The shelf
    // page itself accepts the drop, so this never claims the operation.
    unsafe impl NSDraggingDestination for RootView {
        #[unsafe(method(draggingEntered:))]
        fn dragging_entered(&self, sender: &ProtocolObject<dyn NSDraggingInfo>) -> NSDragOperation {
            if !file_paths(&sender.draggingPasteboard()).is_empty() {
                self.signal_drag(true);
            }
            NSDragOperation::None
        }

        #[unsafe(method(draggingExited:))]
        fn dragging_exited(&self, _sender: Option<&ProtocolObject<dyn NSDraggingInfo>>) {
            self.signal_drag(false);
        }
    }
);

fn fire(which: &RefCell<Option<Signal>>) {
    let signal = which.borrow().clone();
    if let Some(signal) = signal {
        signal();
    }
}

impl RootView {
    fn new(mtm: MainThreadMarker, frame: NSRect) -> Retained<Self> {
        let ivars = RootIvars {
            on_pointer: RefCell::new(None),
            on_click: RefCell::new(None),
            on_drag: RefCell::new(None),
            open: Cell::new(false),
        };
        // SAFETY: designated initializer with valid arguments on the main thread.
        let this: Retained<Self> =
            unsafe { msg_send![super(mtm.alloc::<Self>().set_ivars(ivars)), initWithFrame: frame] };
        this.setWantsLayer(true);
        // SAFETY: extern statics are process-lifetime constants.
        this.registerForDraggedTypes(&NSArray::from_slice(&[unsafe { NSPasteboardTypeFileURL }]));
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

    fn signal_drag(&self, entered: bool) {
        let signal = self.ivars().on_drag.borrow().clone();
        if let Some(signal) = signal {
            signal(entered);
        }
    }
}

pub struct SurfaceIvars {
    on_resize: RefCell<Option<Resize>>,
}

define_class!(
    // SAFETY: NSView allows subclassing; the ivars are only touched on the main thread and need no Drop.
    #[unsafe(super(NSView))]
    #[thread_kind = MainThreadOnly]
    #[name = "FigNotchSurface"]
    #[ivars = SurfaceIvars]
    struct SurfaceView;

    impl SurfaceView {
        // Everything inside is placed here, from the top centre, whenever the shape changes size: on every frame of a
        // movement.
        #[unsafe(method(setFrameSize:))]
        fn set_frame_size(&self, size: NSSize) {
            // SAFETY: forwards the same selector and argument to the superclass.
            let _: () = unsafe { msg_send![super(self), setFrameSize: size] };
            let layout = self.ivars().on_resize.borrow().clone();
            if let Some(layout) = layout {
                layout(size);
            }
        }
    }
);

impl SurfaceView {
    fn new(mtm: MainThreadMarker) -> Retained<Self> {
        let ivars = SurfaceIvars {
            on_resize: RefCell::new(None),
        };
        // SAFETY: designated initializer with valid arguments on the main thread.
        let this: Retained<Self> = unsafe {
            msg_send![super(mtm.alloc::<Self>().set_ivars(ivars)), initWithFrame: NSRect::ZERO]
        };
        // The surface clips nothing: Liquid Glass inside a clipping layer cannot see what is behind the window and
        // turns flat grey. The pieces that need the rounded shape round themselves (`round_bottom`).
        this.setWantsLayer(true);
        this
    }
}

/// How a plain view paints itself: a colour, a gradient, or nothing.
#[derive(Debug, Clone, Copy, PartialEq)]
enum Paint {
    /// Black where it joins the screen's edge, a dark slate at the bottom: the surface reads as lit from below.
    Slate,
    /// Black at the top fading to nothing, so glass merges with a hardware notch.
    Band,
}

pub struct FillIvars {
    paint: Cell<Paint>,
}

define_class!(
    // SAFETY: NSView allows subclassing; the ivars are only touched on the main thread and need no Drop.
    #[unsafe(super(NSView))]
    #[thread_kind = MainThreadOnly]
    #[name = "FigNotchFill"]
    #[ivars = FillIvars]
    struct FillView;

    impl FillView {
        #[unsafe(method(drawRect:))]
        fn draw_rect(&self, _dirty: NSRect) {
            let bounds = self.bounds();
            let gradient = |top: &NSColor, bottom: &NSColor| {
                if let Some(g) = NSGradient::initWithStartingColor_endingColor(NSGradient::alloc(), bottom, top) {
                    g.drawInRect_angle(bounds, 90.0);
                }
            };
            match self.ivars().paint.get() {
                Paint::Slate => gradient(
                    &style::black(1.0),
                    &NSColor::colorWithSRGBRed_green_blue_alpha(0.17, 0.18, 0.20, 1.0),
                ),
                Paint::Band => {
                    // Solid over the first stretch, so the header sits on black, then gone into the glass.
                    let ink = |alpha: f64| NSColor::colorWithSRGBRed_green_blue_alpha(0.0, 0.0, 0.0, alpha);
                    let colors = NSArray::from_retained_slice(&[ink(0.0), ink(0.55), ink(0.94), ink(1.0)]);
                    let locations = [0.0_f64, 0.35, 0.62, 1.0];
                    // SAFETY: four locations for four colours, each between 0 and 1 and increasing.
                    let gradient = unsafe {
                        NSGradient::initWithColors_atLocations_colorSpace(
                            NSGradient::alloc(),
                            &colors,
                            locations.as_ptr(),
                            &objc2_app_kit::NSColorSpace::sRGBColorSpace(),
                        )
                    };
                    if let Some(gradient) = gradient {
                        gradient.drawInRect_angle(bounds, 90.0);
                    }
                }
            }
        }
    }
);

impl FillView {
    fn new(mtm: MainThreadMarker, paint: Paint) -> Retained<Self> {
        let this = mtm.alloc::<Self>().set_ivars(FillIvars {
            paint: Cell::new(paint),
        });
        // SAFETY: designated initializer with valid arguments on the main thread.
        let this: Retained<Self> = unsafe { msg_send![super(this), initWithFrame: NSRect::ZERO] };
        this.setWantsLayer(true);
        if paint != Paint::Band
            && let Some(layer) = this.layer()
        {
            round_bottom(&layer);
        }
        this
    }
}

/// Rounds the bottom corners of a layer and clips what it draws to them. A layer-backed view's layer is not flipped,
/// so the minimum y corners are the bottom ones; the top edge joins the edge of the screen and stays square.
fn round_bottom(layer: &CALayer) {
    layer.setMasksToBounds(true);
    layer.setMaskedCorners(CACornerMask::LayerMinXMinYCorner | CACornerMask::LayerMaxXMinYCorner);
    // SAFETY: an extern static constant.
    layer.setCornerCurve(unsafe { kCACornerCurveContinuous });
    layer.setCornerRadius(COLLAPSED_RADIUS);
}

/// One side of the closed notch.
struct EarState {
    ear: Ear,
    image: Option<Retained<NSImage>>,
    image_id: u64,
    levels: [f32; BAR_COUNT],
}

pub struct EarIvars {
    state: RefCell<EarState>,
}

define_class!(
    // SAFETY: NSView allows subclassing; the ivars are only touched on the main thread and need no Drop.
    #[unsafe(super(NSView))]
    #[thread_kind = MainThreadOnly]
    #[name = "FigNotchEar"]
    #[ivars = EarIvars]
    struct EarView;

    impl EarView {
        #[unsafe(method(drawRect:))]
        fn draw_rect(&self, _dirty: NSRect) {
            self.paint();
        }
    }
);

pub struct DetailIvars {
    detail: RefCell<Option<Detail>>,
}

define_class!(
    // SAFETY: NSView allows subclassing; the ivars are only touched on the main thread and need no Drop.
    #[unsafe(super(NSView))]
    #[thread_kind = MainThreadOnly]
    #[name = "FigNotchDetail"]
    #[ivars = DetailIvars]
    struct DetailView;

    impl DetailView {
        #[unsafe(method(drawRect:))]
        fn draw_rect(&self, _dirty: NSRect) {
            self.paint();
        }
    }
);

impl DetailView {
    fn new(mtm: MainThreadMarker) -> Retained<Self> {
        let this = mtm.alloc::<Self>().set_ivars(DetailIvars {
            detail: RefCell::new(None),
        });
        // SAFETY: designated initializer with valid arguments on the main thread.
        let this: Retained<Self> = unsafe { msg_send![super(this), initWithFrame: NSRect::ZERO] };
        this.setWantsLayer(true);
        this
    }

    fn set(&self, detail: Option<Detail>) {
        if *self.ivars().detail.borrow() == detail {
            return;
        }
        let was_shown = self.ivars().detail.borrow().is_some();
        *self.ivars().detail.borrow_mut() = detail;
        if was_shown {
            fade_up(self);
        }
        self.setNeedsDisplay(true);
    }

    fn paint(&self) {
        let Some(detail) = self.ivars().detail.borrow().clone() else {
            return;
        };
        let bounds = self.bounds();
        let (title_font, subtitle_font) = Detail::fonts();
        let (title_h, subtitle_h) = (15.0, 13.0);
        let bottom = (bounds.size.height - title_h - subtitle_h) / 2.0 - 1.0;
        let line = |text: &str, y: f64, height: f64, font: &objc2_app_kit::NSFont, alpha: f64| {
            draw_text(
                text,
                NSRect::new(
                    NSPoint::new(DETAIL_PAD / 2.0, y),
                    NSSize::new(bounds.size.width - DETAIL_PAD, height),
                ),
                &text_attributes(font, &white(alpha), Fit::Clip, true),
            );
        };
        line(&detail.subtitle, bottom, subtitle_h, &subtitle_font, 0.62);
        line(
            &detail.title,
            bottom + subtitle_h,
            title_h,
            &title_font,
            1.0,
        );
    }
}

/// What a view that shows something else now takes to get there: it comes up from nothing, so a picture or a
/// text that replaces another does not just snap. A view that is not showing (its own opacity is 0) stays as it is.
fn fade_up(view: &NSView) {
    if reduced_motion() {
        return;
    }
    let Some(layer) = view.layer() else {
        return;
    };
    let shown = layer.opacity();
    if shown < 0.5 {
        return;
    }
    let anim = CABasicAnimation::animationWithKeyPath(Some(&NSString::from_str("opacity")));
    // SAFETY: NSNumber is a valid value for an opacity animation.
    unsafe {
        anim.setFromValue(Some(&NSNumber::new_f64(0.0)));
        anim.setToValue(Some(&NSNumber::new_f64(f64::from(shown))));
    }
    anim.setDuration(0.2);
    anim.setTimingFunction(Some(&timing(0.25, 0.9, 0.3, 1.0)));
    layer.addAnimation_forKey(&anim, Some(&NSString::from_str("swap")));
}

/// Whether two ears look the same: same kind, same symbol, text or picture.
fn same_look(a: &Ear, b: &Ear) -> bool {
    match (a, b) {
        (Ear::Empty, Ear::Empty) => true,
        (
            Ear::Symbol {
                name: x,
                tone: s,
                pulse: p,
            },
            Ear::Symbol {
                name: y,
                tone: t,
                pulse: q,
            },
        ) => x == y && s == t && p == q,
        (Ear::Text { text: x, tone: s }, Ear::Text { text: y, tone: t }) => x == y && s == t,
        (Ear::Artwork { id: x, .. }, Ear::Artwork { id: y, .. }) => x == y,
        (Ear::Bars { .. }, Ear::Bars { .. }) => true,
        _ => false,
    }
}

impl EarView {
    fn new(mtm: MainThreadMarker) -> Retained<Self> {
        let this = mtm.alloc::<Self>().set_ivars(EarIvars {
            state: RefCell::new(EarState {
                ear: Ear::Empty,
                image: None,
                image_id: 0,
                levels: [0.0; BAR_COUNT],
            }),
        });
        // SAFETY: designated initializer with valid arguments on the main thread.
        let this: Retained<Self> = unsafe { msg_send![super(this), initWithFrame: NSRect::ZERO] };
        this.setWantsLayer(true);
        this
    }

    fn set(&self, ear: &Ear) {
        let changed = !same_look(&self.ivars().state.borrow().ear, ear);
        {
            let mut state = self.ivars().state.borrow_mut();
            if let Ear::Artwork { id, bytes } = ear
                && (state.image_id != *id || state.image.is_none())
            {
                let data = NSData::with_bytes(bytes);
                state.image = NSImage::initWithData(NSImage::alloc(), &data);
                state.image_id = *id;
            }
            if !matches!(ear, Ear::Artwork { .. }) {
                state.image = None;
                state.image_id = 0;
            }
            state.ear = ear.clone();
        }
        let pulse = matches!(ear, Ear::Symbol { pulse: true, .. });
        if changed && !matches!(ear, Ear::Empty) {
            fade_up(self);
        }
        if let Some(layer) = self.layer() {
            let key = NSString::from_str("pulse");
            if pulse {
                // SAFETY: only reads whether an animation is attached under this key.
                if unsafe { layer.animationForKey(&key) }.is_none() {
                    let anim = CABasicAnimation::animationWithKeyPath(Some(&NSString::from_str(
                        "opacity",
                    )));
                    // SAFETY: NSNumber is a valid value for an opacity animation.
                    unsafe {
                        anim.setFromValue(Some(&NSNumber::new_f64(1.0)));
                        anim.setToValue(Some(&NSNumber::new_f64(0.35)));
                    }
                    anim.setDuration(0.9);
                    anim.setAutoreverses(true);
                    anim.setRepeatCount(f32::INFINITY);
                    anim.setTimingFunction(Some(
                        &CAMediaTimingFunction::functionWithControlPoints(0.45, 0.0, 0.55, 1.0),
                    ));
                    layer.addAnimation_forKey(&anim, Some(&key));
                }
            } else {
                layer.removeAnimationForKey(&key);
            }
        }
        self.setNeedsDisplay(true);
    }

    /// Asks the bars for their levels and redraws them. Returns false when this side has no bars.
    fn tick(&self) -> bool {
        let levels = match &self.ivars().state.borrow().ear {
            Ear::Bars { levels, .. } => Rc::clone(levels),
            _ => return false,
        };
        let mut next = [0.0; BAR_COUNT];
        levels(&mut next);
        self.ivars().state.borrow_mut().levels = next;
        self.setNeedsDisplay(true);
        true
    }

    fn paint(&self) {
        let state = self.ivars().state.borrow();
        let bounds = self.bounds();
        let centre = NSPoint::new(bounds.size.width / 2.0, bounds.size.height / 2.0);
        let height = bounds.size.height;
        match &state.ear {
            Ear::Empty => {}
            Ear::Symbol { name, tone, .. } => {
                style::draw_symbol(name, centre, (height * 0.62).min(18.0), &tone.color());
            }
            Ear::Text { text, tone } => {
                let font = style::font(text_size(height), Weight::Semibold);
                let line = font.ascender() - font.descender();
                // Text wider than the widest ear ends in an ellipsis inside the padding, instead of being cut at the edge.
                draw_text(
                    text,
                    NSRect::new(
                        NSPoint::new(EAR_PAD, centre.y - line / 2.0 - 0.5),
                        NSSize::new((bounds.size.width - 2.0 * EAR_PAD).max(0.0), line + 1.0),
                    ),
                    &text_attributes(&font, &tone.color(), Fit::Tail, true),
                );
            }
            Ear::Artwork { .. } => {
                let side = (height - 8.0).max(8.0);
                let rect = NSRect::new(
                    NSPoint::new(centre.x - side / 2.0, centre.y - side / 2.0),
                    NSSize::new(side, side),
                );
                let path = NSBezierPath::bezierPathWithRoundedRect_xRadius_yRadius(rect, 5.0, 5.0);
                match &state.image {
                    Some(image) => {
                        objc2_app_kit::NSGraphicsContext::saveGraphicsState_class();
                        path.addClip();
                        draw_filled(image, rect);
                        objc2_app_kit::NSGraphicsContext::restoreGraphicsState_class();
                    }
                    None => {
                        white(0.16).setFill();
                        path.fill();
                    }
                }
            }
            Ear::Bars { color, .. } => {
                let ink =
                    NSColor::colorWithSRGBRed_green_blue_alpha(color[0], color[1], color[2], 1.0);
                let (bar, gap) = (3.0, 2.5);
                let span = BAR_COUNT as f64 * bar + (BAR_COUNT - 1) as f64 * gap;
                let tallest = (height * 0.62).max(6.0);
                for (i, level) in state.levels.iter().enumerate() {
                    let h = (f64::from(*level).clamp(0.0, 1.0) * tallest).max(bar);
                    let rect = NSRect::new(
                        NSPoint::new(
                            centre.x - span / 2.0 + i as f64 * (bar + gap),
                            centre.y - h / 2.0,
                        ),
                        NSSize::new(bar, h),
                    );
                    style::fill_capsule(rect, &ink);
                }
            }
        }
    }
}

/// A colour for bars next to a cover: its average colour, brightened enough to read on black. `None` for bytes
/// that are not a picture.
pub fn artwork_tint(bytes: &[u8]) -> Option<[f64; 3]> {
    let image = NSImage::initWithData(NSImage::alloc(), &NSData::with_bytes(bytes))?;
    // SAFETY: a one-pixel 8-bit RGBA bitmap with no planes passed in; AppKit allocates its own buffer.
    let rep = unsafe {
        objc2_app_kit::NSBitmapImageRep::initWithBitmapDataPlanes_pixelsWide_pixelsHigh_bitsPerSample_samplesPerPixel_hasAlpha_isPlanar_colorSpaceName_bytesPerRow_bitsPerPixel(
            objc2_app_kit::NSBitmapImageRep::alloc(),
            std::ptr::null_mut(),
            1,
            1,
            8,
            4,
            true,
            false,
            objc2_app_kit::NSCalibratedRGBColorSpace,
            4,
            32,
        )
    }?;
    let context = objc2_app_kit::NSGraphicsContext::graphicsContextWithBitmapImageRep(&rep)?;
    objc2_app_kit::NSGraphicsContext::saveGraphicsState_class();
    objc2_app_kit::NSGraphicsContext::setCurrentContext(Some(&context));
    // Drawn into one pixel, the picture is averaged by the scaling.
    image.drawInRect(NSRect::new(NSPoint::ZERO, NSSize::new(1.0, 1.0)));
    context.flushGraphics();
    objc2_app_kit::NSGraphicsContext::restoreGraphicsState_class();
    let color = rep.colorAtX_y(0, 0)?;
    let srgb = color.colorUsingColorSpace(&objc2_app_kit::NSColorSpace::sRGBColorSpace())?;
    let rgb = [
        srgb.redComponent(),
        srgb.greenComponent(),
        srgb.blueComponent(),
    ];
    Some(brighten(rgb))
}

/// Lifts a colour until its brightest part reads on black, keeping its hue; a cover that is nearly black has no hue
/// worth keeping and gives plain light bars.
fn brighten(rgb: [f64; 3]) -> [f64; 3] {
    let top = rgb.iter().copied().fold(0.0, f64::max);
    if top < 0.04 {
        return [0.92, 0.92, 0.95];
    }
    let scale = 0.95 / top;
    rgb.map(|c| (c * scale * 0.8 + 0.2).clamp(0.0, 1.0))
}

/// Draws an image so it fills `rect`, cropping what overflows, like a cover.
fn draw_filled(image: &NSImage, rect: NSRect) {
    let size = image.size();
    if size.width <= 0.0 || size.height <= 0.0 {
        return;
    }
    let scale = (rect.size.width / size.width).max(rect.size.height / size.height);
    let drawn = NSSize::new(size.width * scale, size.height * scale);
    let target = NSRect::new(
        NSPoint::new(
            rect.origin.x + (rect.size.width - drawn.width) / 2.0,
            rect.origin.y + (rect.size.height - drawn.height) / 2.0,
        ),
        drawn,
    );
    image.drawInRect_fromRect_operation_fraction(
        target,
        NSRect::ZERO,
        NSCompositingOperation::SourceOver,
        1.0,
    );
}

/// One tab of the expanded notch.
pub struct Page {
    /// Stable name of the tab, for the settings that order and hide tabs: clipboard, shelf, media, ...
    pub id: &'static str,
    pub title: String,
    /// The SF Symbol shown for this page in the strip at the top.
    pub symbol: &'static str,
    /// Fills `Notch::page_frame()`.
    pub root: Retained<NSView>,
    /// Runs whenever the page becomes visible, to refresh what it shows.
    pub on_show: Box<dyn Fn()>,
    /// Runs when the page stops being visible: another tab was chosen, or the notch closed.
    pub on_hide: Box<dyn Fn()>,
    /// Returns true when the key was consumed.
    pub on_key: Box<dyn Fn(Key) -> bool>,
    /// The notch switches to this page when files are dragged onto it.
    pub accepts_files: bool,
    /// The size of the open shape while this page shows, for a page whose root covers the header row too. `None`
    /// is the usual page: its root is the area below the header and the shape is as large as the root plus the
    /// header, so a page chooses its size with the frame it is built with, and changes it by resizing its root: the
    /// shape follows.
    pub size: Option<NSSize>,
}

/// Where the shape is, or is going.
#[derive(Debug, Clone, Copy, PartialEq)]
enum Shape {
    Rest,
    /// Widened by `ear` on each side of the cutout, for something live, and `drop` taller to hold two lines of text.
    Compact {
        ear: f64,
        drop: f64,
    },
    Open,
}

struct Inner {
    mtm: MainThreadMarker,
    panel: Retained<FigPanel>,
    root: Retained<RootView>,
    surface: Retained<SurfaceView>,
    /// Glass, or the slate gradient; replaced when the material changes.
    background: RefCell<Retained<NSView>>,
    /// The background when it is Liquid Glass, whose corners follow the shape's.
    glass: RefCell<Option<Retained<NSGlassEffectView>>>,
    /// Solid black over the background while the notch is closed, so the closed shape is a notch; it also draws the
    /// drop that the shape is while it grows out of the notch. It is wider than the surface by `FLARE` on each side.
    ink: Retained<NSView>,
    /// The black shape inside `ink`: the outline of the whole shape, fillets included.
    ink_shape: Retained<CAShapeLayer>,
    /// Black at the top of open glass on a screen with a hardware notch.
    band: Retained<FillView>,
    left: Retained<EarView>,
    right: Retained<EarView>,
    /// The two lines under the ears.
    detail: Retained<DetailView>,
    /// The shape's own size, rounded and clipping like it, so the pages show only inside the shape as it grows. Glass
    /// cannot be inside a clipping view (see `SurfaceView::new`), the pages can.
    clip: Retained<NSView>,
    /// Holds the tab bar and every page at their expanded-size positions.
    content: Retained<NSView>,
    view: ClipView,
    switcher: Retained<PageSwitcher>,
    /// The three dots at the left of the header, which show `orb_menu`.
    orb: Retained<NotchOrb>,
    orb_menu: RefCell<Option<Retained<NSMenu>>>,
    pages: RefCell<Vec<Page>>,
    /// The visible pages, in tab order (indices into `pages`).
    order: RefCell<Vec<usize>>,
    /// The ids of pages whose tab is left out of the strip for now (`Notch::set_page_dormant`).
    dormant: RefCell<Vec<String>>,
    /// The size the open shape moves to: that of the page it opens on, or the one chosen last.
    open_size: Cell<NSSize>,
    /// The size of `content`: as large as the largest page, which every page is centred at the top of.
    content_size: Cell<NSSize>,
    /// The size each page had when last looked at, to tell a page that grew from one that was only moved.
    known_sizes: RefCell<Vec<NSSize>>,
    badges: RefCell<Vec<usize>>,
    active: Cell<usize>,
    /// The page whose `on_show` ran last and whose `on_hide` has not run yet.
    shown: Cell<Option<usize>>,
    /// Set while files are being dragged over the notch, to open on the page that takes them.
    files_hint: Cell<bool>,
    geometry: Cell<NotchGeometry>,
    style: RefCell<Style>,
    shape: Cell<Shape>,
    inside: Cell<bool>,
    /// Off removes the panel from the screen and ignores the pointer.
    enabled: Cell<bool>,
    /// While something else is using the top edge (window snapping), the pointer does not open the notch.
    suppressed: Cell<bool>,
    /// The person asked the notch not to open when the pointer reaches it, for a while.
    snoozed: Cell<bool>,
    activities: RefCell<Vec<(&'static str, Activity, u64)>>,
    serial: Cell<u64>,
    /// The ears' bars are redrawn by this while they are on screen; nothing runs otherwise.
    bars: RefCell<Option<app::Timer>>,
    /// Bumped by every movement; a movement that finishes late does nothing when another has started since.
    generation: Cell<u64>,
    /// The shape as it is on screen now.
    pose: Cell<Pose>,
    /// The movement under way.
    flight: Cell<Option<Flight>>,
    /// Present only while a movement is under way.
    driver: RefCell<Option<Driver>>,
    me: Weak<Inner>,
}

fn reduced_motion() -> bool {
    crate::workspace::display_options().reduce_motion
}

/// `FIGGHOST_SLOW_MOTION=25` plays every movement 25 times slower, so it can be looked at frame by frame (see the
/// notch_shots example). 1 otherwise.
fn slow_motion() -> f64 {
    std::env::var("FIGGHOST_SLOW_MOTION")
        .ok()
        .and_then(|v| v.parse::<f64>().ok())
        .filter(|f| *f >= 1.0)
        .unwrap_or(1.0)
}

fn timing(c1x: f32, c1y: f32, c2x: f32, c2y: f32) -> Retained<CAMediaTimingFunction> {
    CAMediaTimingFunction::functionWithControlPoints(c1x, c1y, c2x, c2y)
}

/// Runs `changes` as one Core Animation group of `seconds` along `curve`.
fn animate(seconds: f64, curve: &CAMediaTimingFunction, changes: impl Fn() + 'static) {
    let curve = curve.retain();
    let block = RcBlock::new(move |context: NonNull<NSAnimationContext>| {
        // SAFETY: AppKit passes the live current context for the duration of the call.
        let context = unsafe { context.as_ref() };
        context.setDuration(seconds);
        context.setTimingFunction(Some(&curve));
        context.setAllowsImplicitAnimation(true);
        changes();
    });
    NSAnimationContext::runAnimationGroup(&block);
}

/// Changes made at once, with every implicit animation turned off.
fn instantly(changes: impl FnOnce()) {
    CATransaction::begin();
    CATransaction::setDisableActions(true);
    changes();
    CATransaction::commit();
}

fn union_size(a: NSSize, b: NSSize) -> NSSize {
    NSSize::new(a.width.max(b.width), a.height.max(b.height))
}

/// A damped spring from 0 to 1, as Core Animation and SwiftUI describe one: how long it takes to look done, and how
/// much it overshoots (0 for none). It can start already moving, so a movement that turns another around keeps its
/// pace.
#[derive(Debug, Clone, Copy, PartialEq)]
struct Spring {
    omega: f64,
    zeta: f64,
    /// Progress per second at the start.
    velocity: f64,
}

impl Spring {
    fn new(duration: f64, bounce: f64, velocity: f64) -> Self {
        Self {
            omega: std::f64::consts::TAU / duration,
            zeta: (1.0 - bounce).clamp(0.05, 1.0),
            velocity,
        }
    }

    /// Progress and its speed (per second) `t` seconds in.
    fn at(&self, t: f64) -> (f64, f64) {
        let (w, z) = (self.omega, self.zeta);
        // The distance left, y = progress - 1, starts at -1 with the given speed.
        if z < 1.0 {
            let wd = w * (1.0 - z * z).sqrt();
            let (a, b) = (-1.0, (self.velocity - z * w) / wd);
            let decay = (-z * w * t).exp();
            let (sin, cos) = (wd * t).sin_cos();
            let y = decay * (a * cos + b * sin);
            let dy = decay * ((-z * w * a + wd * b) * cos + (-z * w * b - wd * a) * sin);
            (1.0 + y, dy)
        } else {
            let (a, b) = (-1.0, self.velocity - w);
            let decay = (-w * t).exp();
            ((1.0 + (a + b * t) * decay), (b - w * (a + b * t)) * decay)
        }
    }

    /// Whether a movement over `distance` points is over: under half a point from the end, and nearly still.
    fn settled(&self, t: f64, distance: f64) -> bool {
        let (x, v) = self.at(t);
        ((1.0 - x) * distance).abs() < 0.5 && (v * distance).abs() < 30.0
    }

    /// When a movement over `distance` points is over, to the nearest frame.
    fn settling_time(&self, distance: f64) -> f64 {
        (0..=360)
            .map(|frame| f64::from(frame) / 120.0)
            .find(|&t| self.settled(t, distance))
            .unwrap_or(3.0)
    }
}

/// The shape at one moment: its size and the radius of its bottom corners.
#[derive(Debug, Clone, Copy, PartialEq)]
struct Pose {
    size: NSSize,
    radius: f64,
}

impl Pose {
    /// The pose `progress` of the way to `to`; past 1 it overshoots.
    fn toward(self, to: Pose, progress: f64) -> Pose {
        let mix = |a: f64, b: f64| (a + (b - a) * progress).max(0.0);
        Pose {
            size: NSSize::new(
                mix(self.size.width, to.size.width),
                mix(self.size.height, to.size.height),
            ),
            radius: mix(self.radius, to.radius),
        }
    }

    /// How far apart two poses are, in points: the larger change of the two sides.
    fn distance(self, to: Pose) -> f64 {
        (to.size.width - self.size.width)
            .abs()
            .max((to.size.height - self.size.height).abs())
    }
}

/// One movement of the shape.
#[derive(Debug, Clone, Copy)]
struct Flight {
    from: Pose,
    to: Pose,
    spring: Spring,
    /// The display time of its first frame; unknown until that frame, so work done just before (a page filling
    /// itself) delays the movement instead of eating its start.
    start: Option<f64>,
    generation: u64,
    /// Whether the movement is the open shape leaving or going back into a closed one, which is what pinches the
    /// top of the shape into a drop.
    drip: Drip,
}

/// How the top of the shape is pinched during a movement.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Drip {
    /// Not at all: a movement between closed shapes, or between open ones.
    None,
    /// Growing out of the closed notch: the neck is thin first and widens into the body, later than the body grows.
    Out,
    /// Going back into it: the neck narrows a little ahead of the body.
    In,
}

/// How long the neck of a drop takes to widen to the body, and how long it takes to draw in on the way back.
const NECK_OUT_SECONDS: f64 = 0.52;
const NECK_IN_SECONDS: f64 = 0.30;
const NECK_IN_DELAY: f64 = 0.06;

/// A critically damped approach from 1 to 0 over about `seconds`: how much of its distance is left.
fn relax(t: f64, seconds: f64) -> f64 {
    let w = std::f64::consts::TAU / seconds;
    (1.0 + w * t) * (-w * t).exp()
}

impl Flight {
    fn pose_at(&self, t: f64) -> Pose {
        self.from.toward(self.to, self.spring.at(t).0)
    }

    fn distance(&self) -> f64 {
        self.from.distance(self.to)
    }

    /// How pinched the top of the shape is `t` seconds in, from 0 (not at all) to 1 (as thin as the closed shape).
    fn pinch_at(&self, t: f64) -> f64 {
        match self.drip {
            Drip::None => 0.0,
            Drip::Out => relax(t, NECK_OUT_SECONDS),
            // The neck starts to draw in only once the glass has gone: a little after the movement begins.
            Drip::In => 0.55 * (1.0 - relax((t - NECK_IN_DELAY).max(0.0), NECK_IN_SECONDS)),
        }
    }
}

pub struct TickerIvars {
    inner: Weak<Inner>,
}

define_class!(
    // SAFETY: NSObject allows subclassing; the ivar is only touched on the main thread.
    #[unsafe(super(NSObject))]
    #[thread_kind = MainThreadOnly]
    #[name = "FigNotchTicker"]
    #[ivars = TickerIvars]
    struct Ticker;

    impl Ticker {
        #[unsafe(method(tick:))]
        fn tick(&self, link: &CADisplayLink) {
            // The last frame of a movement lets go of the display link, which lets go of this object: kept alive
            // until the call returns.
            let _alive = self.retain();
            if let Some(inner) = self.ivars().inner.upgrade() {
                inner.tick(link.timestamp(), link.targetTimestamp());
            }
        }
    }

    unsafe impl NSObjectProtocol for Ticker {}
);

impl Ticker {
    fn new(mtm: MainThreadMarker, inner: Weak<Inner>) -> Retained<Self> {
        let this = mtm.alloc::<Self>().set_ivars(TickerIvars { inner });
        // SAFETY: NSObject's designated initializer.
        unsafe { msg_send![super(this), init] }
    }
}

/// What calls a movement back on every frame: the display link of the notch's view, or on macOS 13, which has none,
/// a timer at a common display rate.
enum Driver {
    Link(Retained<CADisplayLink>),
    Timer(#[allow(dead_code)] app::Timer),
}

impl Drop for Driver {
    fn drop(&mut self) {
        // A run loop keeps a display link until it is invalidated.
        if let Driver::Link(link) = self {
            link.invalidate();
        }
    }
}

impl Inner {
    fn files_page(&self) -> Option<usize> {
        self.pages.borrow().iter().position(|p| p.accepts_files)
    }

    fn is_open(&self) -> bool {
        self.shape.get() == Shape::Open
    }

    fn page_visible(&self, index: usize) -> bool {
        self.order.borrow().contains(&index)
    }

    /// The size of a shape on this screen.
    fn size_of(&self, shape: Shape) -> NSSize {
        let geometry = self.geometry.get();
        match shape {
            Shape::Rest => geometry.rest_size(self.style.borrow().rest),
            Shape::Compact { ear, drop } => NSSize::new(
                geometry.notch.width + 2.0 * ear,
                geometry.notch.height + drop,
            ),
            Shape::Open => self.open_size.get(),
        }
    }

    fn radius_of(&self, shape: Shape) -> f64 {
        match shape {
            Shape::Rest => COLLAPSED_RADIUS,
            Shape::Compact { drop, .. } => ((self.geometry.get().notch.height + drop) / 2.0)
                .min(if drop > 0.0 { 20.0 } else { 13.0 }),
            Shape::Open => PANEL_RADIUS,
        }
    }

    /// The window that holds a shape. Its shadow is the window's own, drawn by the window server outside it.
    fn window_size(&self, shape: Shape) -> NSSize {
        if shape == Shape::Open {
            // The window holds the largest page, so a change of page never has to resize it.
            let largest = self.content_size.get();
            NSSize::new(largest.width + 2.0 * FLARE, largest.height)
        } else {
            self.size_of(shape)
        }
    }

    /// The surface's frame for a shape size inside the window as it is now: centred, against the top edge.
    fn surface_frame(&self, size: NSSize) -> NSRect {
        let bounds = self.root.bounds();
        NSRect::new(
            NSPoint::new(
                (bounds.size.width - size.width) / 2.0,
                bounds.size.height - size.height,
            ),
            size,
        )
    }

    /// Resizes the window around the top centre and puts the surface back where it was on screen.
    fn set_window(&self, size: NSSize) {
        let geometry = self.geometry.get();
        let frame = geometry.frame(size.width, size.height);
        if self.panel.frame() == frame {
            return;
        }
        let shown = self.surface.frame().size;
        instantly(|| {
            self.panel.setFrame_display(frame, false);
            self.surface.setFrame(self.surface_frame(shown));
        });
    }

    /// Lays out what is inside the surface for a size: called from the surface's own `setFrameSize:`.
    fn layout_inside(&self, size: NSSize) {
        let geometry = self.geometry.get();
        // The glass is taller than the surface by its radius: its rounded top corners are cut off above, so only the
        // bottom ones show, the same ones the surface clips to.
        let overhang = PANEL_RADIUS;
        self.background.borrow().setFrame(NSRect::new(
            NSPoint::ZERO,
            NSSize::new(size.width, size.height + overhang),
        ));
        self.ink.setFrame(NSRect::new(
            NSPoint::new(-FLARE, 0.0),
            NSSize::new(size.width + 2.0 * FLARE, size.height),
        ));
        let band = geometry.notch.height + BAND_DEPTH;
        self.band.setFrame(NSRect::new(
            NSPoint::new(0.0, size.height - band),
            NSSize::new(size.width, band),
        ));
        self.clip.setFrame(NSRect::new(NSPoint::ZERO, size));
        let content = self.content_size.get();
        self.content.setFrameOrigin(NSPoint::new(
            (size.width - content.width) / 2.0,
            size.height - content.height,
        ));
        self.layout_header();
        let ear_height = geometry.notch.height;
        let ear = match self.shape.get() {
            Shape::Compact { ear, .. } => ear,
            _ => self.left.frame().size.width,
        };
        self.detail.setFrame(NSRect::new(
            NSPoint::ZERO,
            NSSize::new(size.width, (size.height - ear_height).max(0.0)),
        ));
        let top = size.height - ear_height;
        // The ears stay against the outer edges, so they ride out with the shape as it widens.
        self.left.setFrame(NSRect::new(
            NSPoint::new(0.0, top),
            NSSize::new(ear, ear_height),
        ));
        self.right.setFrame(NSRect::new(
            NSPoint::new(size.width - ear, top),
            NSSize::new(ear, ear_height),
        ));
    }

    /// Moves the shape to `target`. The window grows to the largest shape at once, the shape moves inside it frame by
    /// frame, and the window shrinks back when the movement has ended (unless another has started since).
    fn move_to(&self, target: Shape) {
        let previous = self.shape.replace(target);
        let generation = self.generation.get() + 1;
        self.generation.set(generation);
        let open = target == Shape::Open;
        self.root.ivars().open.set(open);
        if open {
            self.content.setHidden(false);
        }
        let style = self.style.borrow().clone();
        let largest = union_size(self.window_size(Shape::Open), self.window_size(previous));
        self.set_window(union_size(largest, self.window_size(target)));
        let hidden_rest =
            target == Shape::Rest && !self.geometry.get().hardware && style.rest == Rest::Hidden;
        let alphas = Alphas {
            // The closed shape is black, like a notch; the material shows only when it opens.
            ink: if open { 0.0 } else { 1.0 },
            band: if open && style.material != Material::Black {
                1.0
            } else {
                0.0
            },
            ears: if matches!(target, Shape::Compact { .. }) {
                1.0
            } else {
                0.0
            },
            content: if open { 1.0 } else { 0.0 },
            // Fully transparent pixels would let the pointer through; this keeps the invisible strip catchable.
            surface: if hidden_rest { 0.01 } else { 1.0 },
        };
        // The open notch casts the window's own shadow, redrawn on every frame of a movement so it follows the shape.
        if open && style.shadow {
            self.panel.setHasShadow(true);
        }
        let from = self.pose.get();
        let to = Pose {
            size: self.size_of(target),
            radius: self.radius_of(target),
        };
        let grows = open || (matches!(target, Shape::Compact { .. }) && previous == Shape::Rest);
        if style.motion == Motion::Instant || reduced_motion() {
            self.flight.set(None);
            self.driver.borrow_mut().take();
            self.dissolve(to, alphas, grows, generation);
            return;
        }
        // Growing springs, with one small overshoot unless the motion is the smooth one; shrinking only slows down.
        let (duration, bounce) = match (grows, style.motion) {
            (true, Motion::Spring) => (if open { 0.42 } else { 0.34 }, 0.22),
            (true, _) => (0.36, 0.0),
            (false, _) => (0.26, 0.0),
        };
        let flight = Flight {
            from,
            to,
            spring: Spring::new(duration, bounce, self.carried_velocity(from, to)),
            start: None,
            generation,
            drip: self.drip_between(from, to),
        };
        self.flight.set(Some(flight));
        self.fade(alphas);
        if flight.distance() < 0.5 {
            self.land(generation);
            return;
        }
        // Without frames (a screen asleep, a window off screen) the movement still ends.
        let limit = flight.spring.settling_time(flight.distance()) + 0.25;
        let weak = self.me.clone();
        app::after(limit * slow_motion(), move || {
            if let Some(inner) = weak.upgrade() {
                inner.land(generation);
            }
        });
        self.start_ticking();
    }

    /// The speed the shape has now, as progress per second of a movement from `from` to `to`, so a movement that
    /// turns another around carries on from its pace instead of from a standstill.
    fn carried_velocity(&self, from: Pose, to: Pose) -> f64 {
        let Some(flight) = self.flight.get() else {
            return 0.0;
        };
        let Some(start) = flight.start else {
            return 0.0;
        };
        let speed = flight
            .spring
            .at((CACurrentMediaTime() - start) / slow_motion())
            .1;
        // Along the side that changes most in the new movement.
        let side = |p: Pose| {
            if (to.size.height - from.size.height).abs() >= (to.size.width - from.size.width).abs()
            {
                p.size.height
            } else {
                p.size.width
            }
        };
        let (before, now) = (side(flight.to) - side(flight.from), side(to) - side(from));
        if now.abs() < 1.0 {
            return 0.0;
        }
        (speed * before / now).clamp(-20.0, 20.0)
    }

    /// One frame of the movement under way, for the frame shown at `target` (display time).
    fn tick(&self, now: f64, target: f64) {
        let Some(mut flight) = self.flight.get() else {
            self.driver.borrow_mut().take();
            return;
        };
        let start = *flight.start.get_or_insert(now);
        self.flight.set(Some(flight));
        let t = (target - start).max(0.0) / slow_motion();
        if flight.spring.settled(t, flight.distance()) {
            self.land(flight.generation);
        } else {
            self.show_pose(flight.pose_at(t), flight.pinch_at(t));
        }
    }

    /// Ends the movement `generation` where it was going, unless another has started since.
    fn land(&self, generation: u64) {
        let Some(flight) = self.flight.get().filter(|f| f.generation == generation) else {
            return;
        };
        self.flight.set(None);
        self.driver.borrow_mut().take();
        self.show_pose(flight.to, 0.0);
        self.settle(generation);
    }

    /// How a movement between two poses pinches the top of the shape: only one that opens a closed shape or closes an
    /// open one does.
    fn drip_between(&self, from: Pose, to: Pose) -> Drip {
        let (before, after) = (self.openness_of(from.size), self.openness_of(to.size));
        if before < 0.5 && after >= 0.5 {
            Drip::Out
        } else if before >= 0.5 && after < 0.5 {
            Drip::In
        } else {
            Drip::None
        }
    }

    /// How open a shape of this size reads: 0 for the closed shape, 1 for any open one (see `silhouette::openness`).
    fn openness_of(&self, size: NSSize) -> f64 {
        let closed = self.size_of(Shape::Rest).height;
        silhouette::openness(size.height, closed, OPEN_REFERENCE)
    }

    /// Draws the black outline of the shape at `pose`: the drop pinched at the top while it is part way open, with
    /// the concave fillets at the top once it is open.
    fn shape_ink(&self, pose: Pose, open: f64, pinch: f64) {
        let closed_half = self.size_of(self.closed_shape()).width / 2.0;
        let outline = Outline {
            width: pose.size.width,
            height: pose.size.height,
            radius: pose.radius,
            // The lip of the fillets only forms once the neck has filled out: a drop has no shoulders at its top.
            flare: FLARE * silhouette::smooth(0.5, 1.0, open) * (1.0 - pinch.clamp(0.0, 1.0)),
            pinch,
            neck_half: closed_half,
        };
        self.ink_shape.setFrame(NSRect::new(
            NSPoint::ZERO,
            NSSize::new(pose.size.width + 2.0 * FLARE, pose.size.height),
        ));
        self.ink_shape.setPath(Some(&outline.cg_path(FLARE)));
    }

    /// How much of the glass shows through the black drop, from 0 (none) to 1 (all of it): none until the shape is
    /// nearly open and the neck has filled out, because the glass is a rectangle and would show at the corners
    /// beside a pinched neck.
    fn reveal(open: f64, pinch: f64) -> f64 {
        silhouette::smooth(0.80, 1.0, open) * (1.0 - silhouette::smooth(0.02, 0.15, pinch))
    }

    /// The black drop gives way to the glass, and the pages appear in it, as the shape opens, and the other way
    /// round as it closes. Liquid Glass does not follow its view's opacity, so it is out of the way (hidden) until it
    /// is revealed and the black fades off it instead.
    fn apply_openness(&self, open: f64, pinch: f64) {
        let reveal = Self::reveal(open, pinch);
        self.ink.setAlphaValue(1.0 - reveal);
        {
            let background = self.background.borrow();
            background.setHidden(reveal <= 0.0);
            background.setAlphaValue(reveal);
        }
        self.content.setAlphaValue(reveal * reveal);
        let banded = self.style.borrow().material != Material::Black;
        self.band.setAlphaValue(if banded { reveal } else { 0.0 });
    }

    /// Lays the shape out at `pose`, all of it in this frame.
    fn show_pose(&self, pose: Pose, pinch: f64) {
        let open = self.openness_of(pose.size);
        instantly(|| {
            self.surface.setFrame(self.surface_frame(pose.size));
            self.set_radius(pose.radius);
            self.shape_ink(pose, open, pinch);
            self.apply_openness(open, pinch);
            // SwiftUI lays the glass out inside its view: now, so it has this frame's size in this frame.
            self.background.borrow().layoutSubtreeIfNeeded();
        });
        self.pose.set(pose);
        if self.panel.hasShadow() {
            self.panel.invalidateShadow();
        }
    }

    fn start_ticking(&self) {
        if self.driver.borrow().is_some() {
            return;
        }
        // The clock of the screen the notch is on; a window off every screen (the checks) still gets one.
        let screen = self
            .panel
            .screen()
            .or_else(|| NSScreen::mainScreen(self.mtm));
        let driver = if let Some(screen) =
            screen.filter(|s| s.respondsToSelector(sel!(displayLinkWithTarget:selector:)))
        {
            let ticker = Ticker::new(self.mtm, self.me.clone());
            // SAFETY: the ticker answers `tick:`, taking the display link, which is what the selector names.
            let link = unsafe { screen.displayLinkWithTarget_selector(&ticker, sel!(tick:)) };
            // SAFETY: the main run loop, in the common modes so frames also come while the pointer is held down.
            unsafe { link.addToRunLoop_forMode(&NSRunLoop::mainRunLoop(), NSRunLoopCommonModes) };
            Driver::Link(link)
        } else {
            let weak = self.me.clone();
            Driver::Timer(app::every(1.0 / 120.0, move || {
                if let Some(inner) = weak.upgrade() {
                    let now = CACurrentMediaTime();
                    inner.tick(now, now);
                }
            }))
        };
        *self.driver.borrow_mut() = Some(driver);
    }

    /// The pieces that fade on their own clocks while the shape moves: the black band at the top of the glass and
    /// the ears, which come in once there is room for them. The black drop, the glass and the pages are laid out
    /// frame by frame with the shape instead (`apply_openness`), because each is only as visible as the shape is open.
    fn fade(&self, alphas: Alphas) {
        let group =
            |seconds: f64, curve: Retained<CAMediaTimingFunction>, apply: fn(&Inner, &Alphas)| {
                let weak = self.me.clone();
                animate(seconds, &curve, move || {
                    if let Some(inner) = weak.upgrade() {
                        apply(&inner, &alphas);
                    }
                });
            };
        let quick = || timing(0.3, 0.0, 0.6, 1.0);
        let (seconds, curve) = if alphas.ears > 0.0 {
            (0.3, timing(0.6, 0.0, 0.4, 1.0))
        } else {
            (0.12, quick())
        };
        group(seconds, curve, |inner, a| {
            inner.left.animator().setAlphaValue(a.ears);
            inner.right.animator().setAlphaValue(a.ears);
            inner.detail.animator().setAlphaValue(a.ears);
        });
        group(0.2, quick(), |inner, a| {
            inner.surface.animator().setAlphaValue(a.surface);
        });
    }

    /// Without movement (the setting, or Reduce Motion): the new shape fades in where it is, and a shape that gets
    /// smaller first fades away, as macOS itself replaces movement with a fade.
    fn dissolve(&self, to: Pose, alphas: Alphas, grows: bool, generation: u64) {
        let curve = timing(0.3, 0.0, 0.3, 1.0);
        let weak = self.me.clone();
        let fade_in = curve.clone();
        let appear = move || {
            let Some(inner) = weak.upgrade() else {
                return;
            };
            if inner.generation.get() != generation {
                return;
            }
            instantly(|| {
                alphas.apply(&inner);
                inner.surface.setAlphaValue(0.0);
            });
            inner.show_pose(to, 0.0);
            let again = inner.me.clone();
            animate(DISSOLVE_SECONDS, &fade_in, move || {
                if let Some(inner) = again.upgrade() {
                    inner.surface.animator().setAlphaValue(alphas.surface);
                }
            });
            inner.settle(generation);
        };
        if grows {
            appear();
            return;
        }
        let weak = self.me.clone();
        animate(DISSOLVE_SECONDS, &curve, move || {
            if let Some(inner) = weak.upgrade() {
                inner.surface.animator().setAlphaValue(0.0);
            }
        });
        app::after(DISSOLVE_SECONDS * slow_motion(), appear);
    }

    /// The layers with the shape's rounded corners: the black of the closed notch, the pages' clip and the slate
    /// background. Glass has a radius of its own.
    fn rounded_layers(&self) -> Vec<Retained<CALayer>> {
        let background = self.background.borrow().clone();
        let mut layers: Vec<Retained<CALayer>> =
            [self.clip.layer()].into_iter().flatten().collect();
        if self.glass.borrow().is_none()
            && let Some(layer) = background.layer()
        {
            layers.push(layer);
        }
        layers
    }

    /// Sets the corner radius of every rounded piece of the shape.
    fn set_radius(&self, radius: f64) {
        for layer in self.rounded_layers() {
            layer.setCornerRadius(radius);
        }
        if let Some(glass) = self.glass.borrow().as_ref() {
            glass.setCornerRadius(radius.max(COLLAPSED_RADIUS));
        }
    }

    /// The end of a movement: the window shrinks to what the shape needs, and a closed notch hides its pages so a
    /// click on it reaches the shape.
    fn settle(&self, generation: u64) {
        if self.generation.get() != generation {
            return;
        }
        let shape = self.shape.get();
        // The open notch casts the window's own shadow, taken from what it draws; nothing behind the glass may darken
        // it, so the shadow cannot be a layer under the shape. It is off while the shape moves.
        let shadow = shape == Shape::Open && self.style.borrow().shadow;
        self.panel.setHasShadow(shadow);
        if shadow {
            self.panel.invalidateShadow();
            // The glass is laid out by SwiftUI a moment after the frame, so the shadow taken now can still be that of
            // the size before: a dark outline wider than the glass. Taking it again once the glass has caught up
            // removes it.
            for seconds in [0.08, 0.3] {
                let panel = self.panel.clone();
                crate::app::after(seconds, move || {
                    if panel.hasShadow() {
                        panel.invalidateShadow();
                    }
                });
            }
        }
        if shape != Shape::Open {
            self.content.setHidden(true);
            if !self.enabled.get() {
                return;
            }
            self.set_window(self.window_size(shape));
            // A key window cannot be told to resign directly: take it off screen and bring it back unfocused.
            if self.panel.isKeyWindow() {
                self.panel.orderOut(None);
                self.panel.orderFrontRegardless();
            }
        }
        self.sync_bars();
    }

    /// The activity on show: the highest priority, the newest among equals.
    fn current_activity(&self) -> Option<Activity> {
        self.activities
            .borrow()
            .iter()
            .max_by_key(|(_, activity, serial)| (activity.priority, *serial))
            .map(|(_, activity, _)| activity.clone())
    }

    /// The closed shape: widened for an activity, else at rest.
    fn closed_shape(&self) -> Shape {
        match self.current_activity() {
            Some(activity) => {
                let height = self.geometry.get().notch.height;
                let ear = activity
                    .left
                    .width(height)
                    .max(activity.right.width(height));
                // The text under the ears may need more width than the ears give.
                let wide = activity.detail.as_ref().map_or(0.0, |d| {
                    ((d.width() - self.geometry.get().notch.width) / 2.0).max(0.0)
                });
                Shape::Compact {
                    ear: ear.max(wide).round(),
                    drop: if activity.detail.is_some() {
                        DETAIL_DROP
                    } else {
                        0.0
                    },
                }
            }
            None => Shape::Rest,
        }
    }

    /// Shows the activity on show in the ears, and moves a closed notch to the shape it needs.
    fn refresh_activity(&self) {
        let activity = self.current_activity();
        let (left, right) = activity.as_ref().map_or((Ear::Empty, Ear::Empty), |a| {
            (a.left.clone(), a.right.clone())
        });
        self.left.set(&left);
        self.right.set(&right);
        self.detail
            .set(activity.as_ref().and_then(|a| a.detail.clone()));
        if !self.is_open() && self.enabled.get() {
            let target = self.closed_shape();
            if target != self.shape.get() {
                self.move_to(target);
            }
        }
        self.sync_bars();
    }

    /// Runs the bars' clock only while bars are on screen.
    fn sync_bars(&self) {
        let wanted = matches!(self.shape.get(), Shape::Compact { .. })
            && self.enabled.get()
            && self.current_activity().is_some_and(|a| {
                matches!(a.left, Ear::Bars { .. }) || matches!(a.right, Ear::Bars { .. })
            });
        let mut bars = self.bars.borrow_mut();
        match (wanted, bars.is_some()) {
            (true, false) => {
                let weak = self.me.clone();
                *bars = Some(app::every(BARS_FRAME_SECONDS, move || {
                    if let Some(inner) = weak.upgrade() {
                        inner.left.tick();
                        inner.right.tick();
                    }
                }));
            }
            (false, true) => {
                bars.take();
            }
            _ => {}
        }
    }

    fn select_page(&self, index: usize) {
        if !self.page_visible(index) {
            return;
        }
        let retarget: NSSize;
        {
            let pages = self.pages.borrow();
            let Some(page) = pages.get(index) else {
                return;
            };
            let previous = self.active.get();
            if let Some(old) = self.shown.get().filter(|&shown| shown != index)
                && let Some(old) = pages.get(old)
            {
                (old.on_hide)();
            }
            for (i, p) in pages.iter().enumerate() {
                p.root.setHidden(i != index);
            }
            self.active.set(index);
            self.shown.set(Some(index));
            self.sync_switcher();
            retarget = self.shape_size(index);
            (page.on_show)();
            if previous != index {
                // The page comes in from the side it sits on in the strip.
                let order = self.order.borrow();
                let at = |page: usize| order.iter().position(|&p| p == page);
                let rightwards = at(index) >= at(previous);
                let home = self.page_origin(self.page_size(index));
                self.fade_in(&page.root, home, rightwards);
            }
        }
        // The shape moves to the size of the page it now shows.
        if self.is_open() && retarget != self.open_size.get() {
            self.open_size.set(retarget);
            self.move_to(Shape::Open);
        }
        self.settle_focus();
    }

    /// Shows a page by sliding it in a little from the side while it fades in; instantly when the system asks
    /// for less motion.
    fn fade_in(&self, root: &NSView, home: NSPoint, rightwards: bool) {
        if reduced_motion() || self.style.borrow().motion == Motion::Instant {
            root.setAlphaValue(1.0);
            root.setFrameOrigin(home);
            return;
        }
        root.setAlphaValue(0.0);
        root.setFrameOrigin(NSPoint::new(
            home.x + if rightwards { PAGE_SLIDE } else { -PAGE_SLIDE },
            home.y,
        ));
        let root = root.retain();
        animate(
            PAGE_SLIDE_SECONDS,
            &timing(0.2, 0.9, 0.25, 1.0),
            move || {
                root.animator().setAlphaValue(1.0);
                root.animator().setFrameOrigin(home);
            },
        );
    }

    /// The search field only has focus on the clipboard page; other pages work from the keys the panel forwards.
    fn settle_focus(&self) {
        if self.panel.isKeyWindow() {
            if self.active.get() == 0 {
                self.panel.makeFirstResponder(Some(&self.view.search));
            } else {
                self.panel.makeFirstResponder(None);
            }
        }
    }

    /// The tab to show when the notch opens.
    fn page_to_open(&self) -> usize {
        let last = self.active.get();
        let fallback = |index: usize| {
            if self.page_visible(index) {
                index
            } else {
                self.order.borrow().first().copied().unwrap_or(0)
            }
        };
        if let Some(files) = self.files_page()
            && self.files_hint.replace(false)
            && self.page_visible(files)
        {
            return files;
        }
        let rule = self.style.borrow().open_page.clone();
        match rule {
            OpenPage::Last => fallback(last),
            OpenPage::Activity => {
                let live = self.current_activity().and_then(|a| a.page);
                fallback(live.filter(|p| self.page_visible(*p)).unwrap_or(last))
            }
            OpenPage::Page(id) => {
                let index = self.pages.borrow().iter().position(|p| p.id == id);
                fallback(index.unwrap_or(last))
            }
        }
    }

    fn expand(&self, focus: bool, page: Option<usize>) {
        if !self.enabled.get() {
            return;
        }
        let first = !self.is_open();
        let page = page.unwrap_or_else(|| self.page_to_open());
        if first {
            self.open_size.set(self.shape_size(page));
            self.switcher.setHidden(false);
            for (i, p) in self.pages.borrow().iter().enumerate() {
                p.root.setHidden(i != page);
                p.root.setAlphaValue(1.0);
            }
            self.move_to(Shape::Open);
        }
        if first || page != self.active.get() {
            self.select_page(page);
        }
        if focus {
            self.panel.makeKeyAndOrderFront(None);
            self.settle_focus();
            // A page shows its key hints only while the notch has the keyboard (widgets::PageRoot).
            for page in self.pages.borrow().iter() {
                page.root.setNeedsDisplay(true);
            }
        }
    }

    fn collapse(&self) {
        if !self.is_open() {
            return;
        }
        self.view.clear_query();
        if let Some(shown) = self.shown.take()
            && let Some(page) = self.pages.borrow().get(shown)
        {
            (page.on_hide)();
        }
        self.move_to(self.closed_shape());
    }

    fn relayout(&self, geometry: NotchGeometry) {
        self.geometry.set(self.tuned(geometry));
        self.layout_header();
        let shape = self.shape.get();
        let window = self.window_size(shape);
        instantly(|| {
            self.panel
                .setFrame_display(geometry.frame(window.width, window.height), true);
        });
        // A movement under way lays the shape out on its next frame.
        if self.flight.get().is_none() {
            self.show_pose(
                Pose {
                    size: self.size_of(shape),
                    radius: self.radius_of(shape),
                },
                0.0,
            );
        }
    }

    fn handle_key(&self, key: Key) -> bool {
        if let Key::Tab(n) = key {
            let target = self
                .order
                .borrow()
                .get(usize::from(n).saturating_sub(1))
                .copied();
            if let Some(index) = target {
                self.select_page(index);
            }
            return true;
        }
        let pages = self.pages.borrow();
        pages
            .get(self.active.get())
            .is_some_and(|page| (page.on_key)(key))
    }

    /// Puts the three dots at the left of the header row and the page icons at its right, below the cutout.
    fn layout_header(&self) {
        let geometry = self.geometry.get();
        let content = self.content_size.get();
        let shape_width = self.surface.frame().size.width;
        // The content is centred on the surface, so the shape's left edge is at this x of the content: the header
        // rides the edges of the shape as it grows.
        let left = (content.width - shape_width) / 2.0;
        let band = self.header_in_band();
        // Beside the cutout the header is centred on its height; otherwise it is the row right below it.
        let y = if band {
            content.height - (geometry.notch.height + switcher::HEIGHT) / 2.0
        } else {
            content.height - geometry.top_inset() - 2.0 - switcher::HEIGHT
        };
        self.orb
            .setFrameOrigin(NSPoint::new(left + SWITCHER_INSET, y));
        let width = self.switcher.frame().size.width;
        self.switcher
            .setFrameOrigin(NSPoint::new(left + shape_width - SWITCHER_INSET - width, y));
        self.view.set_search_row(band);
    }

    /// The width of the open shape: the widest visible page's, never narrower than the user chose. The shape keeps
    /// it from tab to tab, so only its height follows the page.
    fn open_width(&self) -> f64 {
        let pages = self.pages.borrow();
        let widest = self
            .order
            .borrow()
            .iter()
            .filter_map(|&i| pages.get(i))
            .map(|p| {
                p.size
                    .map_or_else(|| p.root.frame().size.width, |s| s.width)
            })
            .fold(EXPANDED_SIZE.width, f64::max);
        widest.max(self.style.borrow().min_width)
    }

    /// Whether the header (the three dots and the page icons) sits in the band beside the cutout of a screen with a
    /// notch, instead of in a row below it. A page then starts right under the cutout and the open shape has no
    /// empty band across its top, as Apple asks of the Dynamic Island. Only when both fit on their side of it.
    fn header_in_band(&self) -> bool {
        let geometry = self.geometry.get();
        let side = (self.open_width() - geometry.notch.width) / 2.0 - SWITCHER_INSET - BAND_GAP;
        geometry.hardware
            && switcher::width_for(self.order.borrow().len()) <= side
            && switcher::ORB_WIDTH <= side
    }

    /// What lies above a page's root: the cutout, and the header row unless the header sits beside the cutout.
    fn header_height(&self) -> f64 {
        let top = self.geometry.get().top_inset();
        if self.header_in_band() {
            top
        } else {
            top + TAB_BAR_HEIGHT
        }
    }

    /// The size of the open shape on a page.
    fn page_size(&self, index: usize) -> NSSize {
        let top = self.header_height();
        self.pages.borrow().get(index).map_or(EXPANDED_SIZE, |p| {
            p.size.unwrap_or_else(|| {
                let root = p.root.frame().size;
                NSSize::new(root.width, root.height + top)
            })
        })
    }

    /// The size of the open shape on a page: as wide on every page (`open_width`), as tall as this page needs and
    /// no shorter than the user chose.
    fn shape_size(&self, index: usize) -> NSSize {
        let size = self.page_size(index);
        NSSize::new(
            self.open_width(),
            size.height.max(self.style.borrow().min_height),
        )
    }

    /// The geometry with the resting shape sized as the user chose. Only a screen without a hardware notch follows
    /// it: the cutout of a real one is what it is.
    fn tuned(&self, mut geometry: NotchGeometry) -> NotchGeometry {
        let style = self.style.borrow();
        if !geometry.hardware {
            if style.rest_width > 0.0 {
                geometry.notch.width = style.rest_width;
            }
            if style.rest_height > 0.0 {
                geometry.notch.height = style.rest_height;
            }
        }
        geometry
    }

    /// Where a page's root sits in the content: centred, against the top, below the header.
    fn page_origin(&self, size: NSSize) -> NSPoint {
        let content = self.content_size.get();
        NSPoint::new(
            (content.width - size.width) / 2.0,
            content.height - size.height,
        )
    }

    /// Sizes the content to the largest page and puts every page where it belongs in it.
    fn sync_content(&self) {
        let count = self.pages.borrow().len();
        let largest = (0..count)
            .map(|i| self.page_size(i))
            .fold(EXPANDED_SIZE, union_size);
        let largest = {
            let style = self.style.borrow();
            NSSize::new(
                largest.width.max(style.min_width),
                largest.height.max(style.min_height),
            )
        };
        self.content_size.set(largest);
        self.content.setFrameSize(largest);
        for i in 0..count {
            let origin = self.page_origin(self.page_size(i));
            if let Some(page) = self.pages.borrow().get(i) {
                let at = page.root.frame().origin;
                // A page that is sliding in keeps its x; its height above the bottom is right whatever it is doing.
                let sliding = self.shown.get() == Some(i) && self.is_open();
                let target = NSPoint::new(if sliding { at.x } else { origin.x }, origin.y);
                if at != target {
                    page.root.setFrameOrigin(target);
                }
            }
        }
        self.layout_inside(self.surface.frame().size);
    }

    /// A page's root changed size or place: the content and the shape follow it when it is the page on show.
    fn page_changed(&self, root: *const AnyObject) {
        let index = self
            .pages
            .borrow()
            .iter()
            .position(|p| std::ptr::eq(Retained::as_ptr(&p.root).cast::<AnyObject>(), root));
        let Some(index) = index else {
            return;
        };
        let size = self.page_size(index);
        {
            // Only a change of size matters: moving the root to its place posts the same notification.
            let mut known = self.known_sizes.borrow_mut();
            if known.get(index) == Some(&size) {
                return;
            }
            known.resize(index + 1, size);
            known[index] = size;
        }
        self.sync_content();
        let shape = self.shape_size(index);
        if self.is_open() && index == self.active.get() && shape != self.open_size.get() {
            self.open_size.set(shape);
            self.move_to(Shape::Open);
        }
    }

    /// The tab strip shows the visible pages in their order, with the badges of those pages.
    fn sync_switcher(&self) {
        let order = self.order.borrow();
        let pages = self.pages.borrow();
        let active = self.active.get();
        self.switcher.set_items(
            order
                .iter()
                .filter_map(|&i| pages.get(i))
                .map(|page| (page.symbol.to_owned(), page.title.clone()))
                .collect(),
        );
        self.switcher.set_selected(
            order
                .iter()
                .position(|&i| i == active)
                .unwrap_or(usize::MAX),
        );
        let badges = self.badges.borrow();
        for (slot, index) in order.iter().enumerate() {
            self.switcher.set_badge(slot, badges.contains(index));
        }
        drop((order, pages, badges));
        self.layout_header();
    }

    /// Recomputes the visible pages from the style.
    fn sync_order(&self) {
        let wanted = self.style.borrow().pages.clone();
        let dormant = self.dormant.borrow();
        let pages = self.pages.borrow();
        let awake = |index: usize| !dormant.iter().any(|id| *id == pages[index].id);
        let order: Vec<usize> = if wanted.is_empty() {
            let all: Vec<usize> = (0..pages.len()).filter(|&i| awake(i)).collect();
            // A tab nobody can reach is worse than one the user did not ask for.
            if all.is_empty() && !pages.is_empty() {
                vec![0]
            } else {
                all
            }
        } else {
            let mut order: Vec<usize> = wanted
                .iter()
                .filter_map(|id| pages.iter().position(|p| p.id == id))
                .filter(|&i| awake(i))
                .collect();
            order.dedup();
            if order.is_empty() {
                order.push(0);
            }
            order
        };
        drop((pages, dormant));
        *self.order.borrow_mut() = order;
        self.sync_switcher();
        // The number of tabs decides whether the header fits beside the cutout, and with it the height of a page.
        self.sync_content();
    }

    /// After the visible pages changed on their own: lays the strip out again, and moves off a tab that is gone.
    fn reorder(&self) {
        self.sync_order();
        if !self.page_visible(self.active.get()) {
            let first = self.order.borrow().first().copied().unwrap_or(0);
            if self.is_open() {
                self.select_page(first);
            } else {
                self.active.set(first);
            }
        }
        self.relayout(self.geometry.get());
        self.move_to(self.shape.get());
    }

    /// Replaces the background for the material in use.
    fn set_material(&self, material: Material) {
        let mtm = self.mtm;
        self.glass.borrow_mut().take();
        let glass_look = glass_tint(material);
        let view: Retained<NSView> = match material {
            Material::Black => Retained::into_super(FillView::new(mtm, Paint::Slate)),
            _ if glass_look.is_some() && AnyClass::get(c"NSGlassEffectView").is_some() => {
                let (look, tint) = glass_look.expect("checked just above");
                let glass = NSGlassEffectView::initWithFrame(mtm.alloc(), NSRect::ZERO);
                glass.setStyle(look);
                glass.setTintColor(Some(&tint));
                glass.setCornerRadius(self.radius_of(self.shape.get()).max(COLLAPSED_RADIUS));
                *self.glass.borrow_mut() = Some(Retained::clone(&glass));
                Retained::into_super(glass)
            }
            // Before macOS 26 there is no Liquid Glass: a dark vibrancy material stands in.
            _ => {
                let effect = NSVisualEffectView::initWithFrame(mtm.alloc(), NSRect::ZERO);
                effect.setMaterial(vibrancy_of(material));
                effect.setBlendingMode(NSVisualEffectBlendingMode::BehindWindow);
                effect.setState(NSVisualEffectState::Active);
                Retained::into_super(effect)
            }
        };
        let old = self.background.replace(view.clone());
        self.surface.addSubview_positioned_relativeTo(
            &view,
            objc2_app_kit::NSWindowOrderingMode::Below,
            Some(&old),
        );
        old.removeFromSuperview();
        self.layout_inside(self.surface.frame().size);
    }

    fn apply_style(&self, style: Style) {
        let before = self.style.replace(style.clone());
        self.panel.setSharingType(if style.hide_in_capture {
            NSWindowSharingType::None
        } else {
            NSWindowSharingType::ReadOnly
        });
        if before.material != style.material {
            self.set_material(style.material);
        }
        if (before.display != style.display
            || before.rest_width != style.rest_width
            || before.rest_height != style.rest_height)
            && let Some(geometry) = NotchGeometry::detect_on(self.mtm, style.display.clone())
        {
            self.geometry.set(self.tuned(geometry));
        }
        if before.min_width != style.min_width || before.min_height != style.min_height {
            self.sync_content();
            if self.is_open() {
                self.open_size.set(self.shape_size(self.active.get()));
            }
        }
        if before.pages != style.pages {
            self.sync_order();
            if !self.page_visible(self.active.get()) {
                let first = self.order.borrow().first().copied().unwrap_or(0);
                self.active.set(first);
            }
        }
        // The shapes may have changed size (rest style, shadow): put the closed shape where it belongs now.
        if !self.is_open() {
            let target = self.closed_shape();
            self.shape.set(target);
        }
        let shape = self.shape.get();
        self.relayout(self.geometry.get());
        self.move_to(shape);
    }
}

/// The opacities of the pieces of the surface for one shape.
#[derive(Debug, Clone, Copy)]
struct Alphas {
    ink: f64,
    band: f64,
    ears: f64,
    content: f64,
    surface: f64,
}

impl Alphas {
    fn apply(&self, inner: &Inner) {
        inner.ink.setAlphaValue(self.ink);
        inner.band.setAlphaValue(self.band);
        inner.left.setAlphaValue(self.ears);
        inner.right.setAlphaValue(self.ears);
        inner.detail.setAlphaValue(self.ears);
        inner.content.setAlphaValue(self.content);
        inner.surface.setAlphaValue(self.surface);
    }
}

/// Edges are inclusive: a pointer pushed against the top of the screen sits exactly on the boundary.
fn contains(rect: NSRect, p: NSPoint) -> bool {
    p.x >= rect.origin.x
        && p.x <= rect.origin.x + rect.size.width
        && p.y >= rect.origin.y
        && p.y <= rect.origin.y + rect.size.height
}

/// A tap under the fingers on a Force Touch trackpad.
fn tap() {
    let performer = NSHapticFeedbackManager::defaultPerformer();
    performer.performFeedbackPattern_performanceTime(
        NSHapticFeedbackPattern::Alignment,
        NSHapticFeedbackPerformanceTime::Now,
    );
}

/// Hover logic: opens shortly after the pointer enters the trigger area, closes shortly after it leaves the open
/// shape unless the search field has focus.
fn pointer_moved(inner: &Rc<Inner>, p: NSPoint) {
    if !inner.enabled.get() || inner.suppressed.get() {
        return;
    }
    let mut geometry = inner.geometry.get();
    // Following the pointer: the closed notch goes to whichever screen the pointer is on, so it (and the mascot)
    // is in front of the person, and a pointer at the top of that screen opens it.
    if inner.style.borrow().display == Display::Pointer
        && !inner.is_open()
        && !contains(geometry.screen, p)
        && let Some(found) = NotchGeometry::detect_on(inner.mtm, Display::Pointer)
    {
        inner.relayout(found);
        geometry = found;
    }
    let area = if inner.is_open() {
        geometry.frame(inner.open_size.get().width, inner.open_size.get().height)
    } else {
        geometry.trigger_for(inner.size_of(inner.shape.get()))
    };
    let inside = contains(area, p);
    if inner.inside.replace(inside) == inside {
        return;
    }
    let style = inner.style.borrow().clone();
    let inner = Rc::clone(inner);
    if inside {
        if !style.open_on_hover || inner.snoozed.get() {
            return;
        }
        app::after(style.hover_delay, move || {
            // Window snapping may have taken the top edge since the pointer arrived, or the notch was snoozed.
            if inner.inside.get()
                && !inner.is_open()
                && !inner.suppressed.get()
                && !inner.snoozed.get()
            {
                if style.haptics {
                    tap();
                }
                inner.expand(false, None);
            }
        });
    } else {
        inner.files_hint.set(false);
        app::after(HOVER_CLOSE_DELAY, move || {
            // A focused search field means the user is typing: leave it open.
            if !inner.inside.get() && inner.is_open() && !inner.panel.isKeyWindow() {
                inner.collapse();
            }
        });
    }
}

type Observer = Retained<ProtocolObject<dyn NSObjectProtocol>>;

pub struct Notch {
    inner: Rc<Inner>,
    ui: ClipUi,
    observers: Vec<Observer>,
    /// One per page: told when the page's root changes size, so the shape follows.
    page_observers: RefCell<Vec<Observer>>,
    /// Global pointer monitor; sees movement over other apps' windows, which tracking areas cannot.
    monitor: Option<Retained<AnyObject>>,
}

impl Drop for Notch {
    fn drop(&mut self) {
        let center = NSNotificationCenter::defaultCenter();
        for observer in self
            .observers
            .iter()
            .chain(self.page_observers.borrow().iter())
        {
            // SAFETY: each token was returned by the default center's addObserverForName.
            unsafe { center.removeObserver(AsRef::<AnyObject>::as_ref(&**observer)) };
        }
        if let Some(monitor) = &self.monitor {
            // SAFETY: the token came from addGlobalMonitorForEventsMatchingMask and is removed once.
            unsafe { NSEvent::removeMonitor(monitor) };
        }
    }
}

impl Notch {
    /// The notch starts with the clipboard page; `on_expand` refreshes its rows each time it is shown.
    pub fn new(
        mtm: MainThreadMarker,
        geometry: NotchGeometry,
        app: AppCallbacks,
        on_expand: impl Fn(&ClipView) + 'static,
    ) -> Self {
        let slot: Rc<RefCell<Option<Rc<Inner>>>> = Rc::default();
        let collapse = {
            let slot = Rc::clone(&slot);
            move || {
                let inner = slot.borrow().clone();
                if let Some(inner) = inner {
                    inner.collapse();
                }
            }
        };
        let on_activate = app.on_activate;
        let close_after_activate = collapse.clone();
        let callbacks = Callbacks {
            on_query: app.on_query,
            on_activate: Box::new(move |view, id, mode| {
                close_after_activate();
                on_activate(view, id, mode);
            }),
            on_pin: app.on_pin,
            on_delete: app.on_delete,
            on_close: Box::new(move |_| collapse()),
        };

        // The clipboard page: the search field shares the header row, the cards run along one row below the chips.
        let strip = NSSize::new(STRIP_WIDTH, geometry.top_inset() + STRIP_BODY);
        let ui = ClipUi::with_style(
            mtm,
            strip,
            geometry.top_inset(),
            callbacks,
            ListStyle::Strip,
        );
        let rest = geometry.rest_size(Rest::Notch);
        let panel = FigPanel::new(mtm, geometry.frame(rest.width, rest.height));
        let root = RootView::new(mtm, NSRect::new(NSPoint::ZERO, rest));
        panel.setContentView(Some(&root));
        if let Some(layer) = root.layer() {
            layer.setSpeed((1.0 / slow_motion()) as f32);
        }

        let surface = SurfaceView::new(mtm);
        root.addSubview(&surface);
        let background: Retained<NSView> = Retained::into_super(FillView::new(mtm, Paint::Slate));
        surface.addSubview(&background);
        let ink = NSView::initWithFrame(mtm.alloc(), NSRect::ZERO);
        ink.setWantsLayer(true);
        let ink_shape = CAShapeLayer::layer();
        // The drop and the fillets are drawn outside the surface's own bounds, in the margin the window leaves.
        ink.layer()
            .expect("a layer-backed view has a layer")
            .addSublayer(&ink_shape);
        surface.addSubview(&ink);
        let band = FillView::new(mtm, Paint::Band);
        band.setAlphaValue(0.0);
        surface.addSubview(&band);
        let (left, right) = (EarView::new(mtm), EarView::new(mtm));
        for ear in [&left, &right] {
            ear.setAlphaValue(0.0);
            surface.addSubview(ear);
        }
        let detail = DetailView::new(mtm);
        detail.setAlphaValue(0.0);
        surface.addSubview(&detail);
        // Fixed-size content: the shape grows around it instead of stretching it (see `layout_inside`).
        let content = NSView::initWithFrame(mtm.alloc(), NSRect::new(NSPoint::ZERO, EXPANDED_SIZE));
        content.setWantsLayer(true);
        // Hidden while closed, so clicks on the closed shape reach the shape itself.
        content.setHidden(true);
        content.setAlphaValue(0.0);
        let clip = NSView::initWithFrame(mtm.alloc(), NSRect::ZERO);
        clip.setWantsLayer(true);
        if let Some(layer) = clip.layer() {
            round_bottom(&layer);
        }
        surface.addSubview(&clip);
        clip.addSubview(&content);
        content.addSubview(&ui.view.root);

        let header_y = EXPANDED_SIZE.height - geometry.top_inset() - 2.0 - switcher::HEIGHT;
        let switcher = PageSwitcher::new(mtm, NSPoint::new(SWITCHER_INSET, header_y));
        content.addSubview(&switcher);
        let orb = NotchOrb::new(mtm, NSPoint::new(SWITCHER_INSET, header_y));
        content.addSubview(&orb);
        ui.view.root.setHidden(true);

        let inner = Rc::new_cyclic(|me| Inner {
            mtm,
            panel: Retained::clone(&panel),
            root: Retained::clone(&root),
            surface: Retained::clone(&surface),
            background: RefCell::new(background),
            glass: RefCell::new(None),
            ink,
            ink_shape,
            band,
            left,
            right,
            detail,
            clip,
            content,
            view: ui.view.clone(),
            switcher: Retained::clone(&switcher),
            orb: Retained::clone(&orb),
            orb_menu: RefCell::new(None),
            pages: RefCell::new(Vec::new()),
            order: RefCell::new(Vec::new()),
            dormant: RefCell::new(Vec::new()),
            open_size: Cell::new(EXPANDED_SIZE),
            content_size: Cell::new(EXPANDED_SIZE),
            known_sizes: RefCell::new(Vec::new()),
            badges: RefCell::new(Vec::new()),
            active: Cell::new(0),
            shown: Cell::new(None),
            files_hint: Cell::new(false),
            geometry: Cell::new(geometry),
            // Black until the app's settings arrive, so nothing glassy flashes before they do.
            style: RefCell::new(Style {
                material: Material::Black,
                ..Style::default()
            }),
            shape: Cell::new(Shape::Rest),
            inside: Cell::new(false),
            enabled: Cell::new(true),
            suppressed: Cell::new(false),
            snoozed: Cell::new(false),
            activities: RefCell::new(Vec::new()),
            serial: Cell::new(0),
            bars: RefCell::new(None),
            generation: Cell::new(0),
            pose: Cell::new(Pose {
                size: rest,
                radius: COLLAPSED_RADIUS,
            }),
            flight: Cell::new(None),
            driver: RefCell::new(None),
            me: me.clone(),
        });
        *slot.borrow_mut() = Some(Rc::clone(&inner));
        {
            let weak = Rc::downgrade(&inner);
            *surface.ivars().on_resize.borrow_mut() = Some(Rc::new(move |size| {
                if let Some(inner) = weak.upgrade() {
                    inner.layout_inside(size);
                }
            }));
        }
        instantly(|| {
            let frame = inner.surface_frame(rest);
            inner.surface.setFrame(frame);
        });
        // The black shape of the closed notch is drawn from here on; nothing else draws it.
        inner.show_pose(
            Pose {
                size: rest,
                radius: COLLAPSED_RADIUS,
            },
            0.0,
        );

        let clipboard_page = {
            let (view, callbacks) = (ui.view.clone(), Rc::clone(&ui.callbacks));
            let key_view = view.clone();
            Page {
                id: "clipboard",
                title: labels::get().clipboard_tab.clone(),
                symbol: "doc.on.clipboard.fill",
                root: Retained::into_super(Retained::clone(&ui.view.root)),
                on_show: Box::new(move || on_expand(&view)),
                on_hide: Box::new(|| {}),
                on_key: Box::new(move |key| handle_key(&key_view, &callbacks, key)),
                accepts_files: false,
                size: Some(strip),
            }
        };
        inner.pages.borrow_mut().push(clipboard_page);
        let first = inner.page_size(0);
        inner.known_sizes.borrow_mut().push(first);
        inner.sync_content();
        inner.sync_order();

        {
            let slot = Rc::clone(&slot);
            switcher.set_on_select(move |slot_index| {
                let inner = slot.borrow().clone();
                if let Some(inner) = inner {
                    let page = inner.order.borrow().get(slot_index).copied();
                    if let Some(page) = page {
                        inner.select_page(page);
                    }
                }
            });
        }
        {
            let weak = Rc::downgrade(&inner);
            orb.set_on_click(move |view, event| {
                let menu = weak
                    .upgrade()
                    .and_then(|inner| inner.orb_menu.borrow().clone());
                if let Some(menu) = menu {
                    NSMenu::popUpContextMenu_withEvent_forView(&menu, event, view);
                }
            });
        }
        let mut this = Self {
            inner: Rc::clone(&inner),
            ui,
            observers: Vec::new(),
            page_observers: RefCell::new(Vec::new()),
            monitor: None,
        };

        let key_inner = Rc::clone(&inner);
        panel.set_key_handler(move |key| key_inner.handle_key(key));
        let on_pointer = Rc::clone(&inner);
        *root.ivars().on_pointer.borrow_mut() = Some(Rc::new(move || {
            pointer_moved(&on_pointer, NSEvent::mouseLocation());
        }));
        let on_drag = Rc::clone(&inner);
        *root.ivars().on_drag.borrow_mut() = Some(Rc::new(move |entered| {
            if entered {
                // Files are arriving: open straight onto the page that takes them.
                on_drag.files_hint.set(true);
                on_drag.inside.set(true);
                on_drag.expand(false, None);
            } else {
                // Leaving the shape for a page inside it, or for good: ask where the pointer is now.
                pointer_moved(&on_drag, NSEvent::mouseLocation());
            }
        }));
        let on_click = Rc::clone(&inner);
        *root.ivars().on_click.borrow_mut() = Some(Rc::new(move || on_click.expand(true, None)));

        let center = NSNotificationCenter::defaultCenter();
        let window: &AnyObject = &panel;
        let resigned = Rc::clone(&inner);
        let on_resign = RcBlock::new(move |_: NonNull<NSNotification>| {
            if !resigned.inside.get() {
                resigned.collapse();
            }
        });
        let relaid = Rc::clone(&inner);
        let on_screens = RcBlock::new(move |_: NonNull<NSNotification>| {
            let display = relaid.style.borrow().display.clone();
            if let Some(geometry) = NotchGeometry::detect_on(mtm, display) {
                relaid.relayout(geometry);
            }
        });
        // SAFETY: the blocks are 'static and run on the main thread, where both notifications are posted.
        let observers = unsafe {
            vec![
                center.addObserverForName_object_queue_usingBlock(
                    Some(NSWindowDidResignKeyNotification),
                    Some(window),
                    None,
                    &on_resign,
                ),
                center.addObserverForName_object_queue_usingBlock(
                    Some(NSApplicationDidChangeScreenParametersNotification),
                    None,
                    None,
                    &on_screens,
                ),
            ]
        };

        let moved = Rc::clone(&inner);
        let on_move = RcBlock::new(move |_: NonNull<NSEvent>| {
            pointer_moved(&moved, NSEvent::mouseLocation());
        });
        let monitor = NSEvent::addGlobalMonitorForEventsMatchingMask_handler(
            NSEventMask::MouseMoved,
            &on_move,
        );

        panel.orderFrontRegardless();
        this.observers = observers;
        this.monitor = monitor;
        this
    }

    pub fn view(&self) -> &ClipView {
        &self.ui.view
    }

    /// The menu the three dots at the left of the header show.
    pub fn set_orb_menu(&self, menu: Retained<NSMenu>) {
        *self.inner.orb_menu.borrow_mut() = Some(menu);
    }

    /// Applies how the notch looks and behaves. Cheap to call again with the same style.
    pub fn configure(&self, style: &Style) {
        if *self.inner.style.borrow() != *style {
            self.inner.apply_style(style.clone());
        }
    }

    /// Area a page of the default size must fill: the shape below the tab bar.
    pub fn page_frame(&self) -> NSRect {
        self.page_frame_for(EXPANDED_SIZE)
    }

    /// The frame of a page that wants an area of this size below the header: the shape is as large as the page plus
    /// the header, and moves to it when the page shows.
    pub fn page_area(&self, width: f64, height: f64) -> NSRect {
        NSRect::new(NSPoint::ZERO, NSSize::new(width, height))
    }

    /// Area a page must fill when its open shape is `size` (`Page::size`): the shape below the tab bar.
    pub fn page_frame_for(&self, size: NSSize) -> NSRect {
        // Always the room below a header row, whatever the number of tabs is while the pages are being added: when the
        // header ends up beside the cutout, the shape is that much shorter instead.
        let top = self.inner.geometry.get().top_inset() + TAB_BAR_HEIGHT;
        NSRect::new(NSPoint::ZERO, NSSize::new(size.width, size.height - top))
    }

    /// How many tabs there are, so a caller can learn the number of the one it is about to add.
    pub fn page_count(&self) -> usize {
        self.inner.pages.borrow().len()
    }

    /// Adds a tab after the existing ones; Command and its number in the strip selects it.
    pub fn add_page(&self, page: Page) {
        page.root.setHidden(true);
        page.root.setPostsFrameChangedNotifications(true);
        let root = Retained::clone(&page.root);
        self.inner.content.addSubview(&page.root);
        self.inner.pages.borrow_mut().push(page);
        let index = self.inner.pages.borrow().len() - 1;
        let size = self.inner.page_size(index);
        self.inner.known_sizes.borrow_mut().push(size);
        self.inner.sync_content();
        self.inner.sync_order();
        // A page that changes size (the media page with lyrics) takes the shape with it.
        let weak = Rc::downgrade(&self.inner);
        let on_frame = RcBlock::new(move |notification: NonNull<NSNotification>| {
            // SAFETY: the notification is live for the call.
            let object = unsafe { notification.as_ref() }.object();
            if let (Some(inner), Some(object)) = (weak.upgrade(), object) {
                inner.page_changed(Retained::as_ptr(&object));
            }
        });
        let observed: &AnyObject = &root;
        // SAFETY: the block is 'static and runs on the main thread, where views post the notification.
        let token = unsafe {
            NSNotificationCenter::defaultCenter().addObserverForName_object_queue_usingBlock(
                Some(objc2_app_kit::NSViewFrameDidChangeNotification),
                Some(observed),
                None,
                &on_frame,
            )
        };
        self.page_observers.borrow_mut().push(token);
    }

    /// Takes the tab of a page out of the strip, or puts it back where it was, without touching what the person chose in
    /// `Style::pages`: the tab of a feature that is switched off has nothing to show. A dormant page is not opened by
    /// `open_page`, as with a page the user hid, and an open notch that is on it moves to the first tab.
    pub fn set_page_dormant(&self, id: &str, dormant: bool) {
        {
            let mut list = self.inner.dormant.borrow_mut();
            let at = list.iter().position(|d| d == id);
            match (dormant, at) {
                (true, None) => list.push(id.to_owned()),
                (false, Some(i)) => {
                    list.remove(i);
                }
                _ => return,
            }
        }
        self.inner.reorder();
    }

    /// The ids of the pages whose tab is in the strip, in the order of the strip.
    pub fn tab_ids(&self) -> Vec<&'static str> {
        let pages = self.inner.pages.borrow();
        self.inner
            .order
            .borrow()
            .iter()
            .filter_map(|&i| pages.get(i))
            .map(|p| p.id)
            .collect()
    }

    /// The ids and titles of the tabs, in the order they were added, for the settings page.
    pub fn page_ids(&self) -> Vec<(&'static str, String)> {
        self.inner
            .pages
            .borrow()
            .iter()
            .map(|p| (p.id, p.title.clone()))
            .collect()
    }

    /// Shows, replaces or (with `None`) ends a live activity in the closed notch. `slot` names its source.
    pub fn set_activity(&self, slot: &'static str, activity: Option<Activity>) {
        {
            let mut activities = self.inner.activities.borrow_mut();
            let at = activities.iter().position(|(s, _, _)| *s == slot);
            match (activity, at) {
                (Some(activity), Some(i)) => activities[i].1 = activity,
                (Some(activity), None) => {
                    let serial = self.inner.serial.get() + 1;
                    self.inner.serial.set(serial);
                    activities.push((slot, activity, serial));
                }
                (None, Some(i)) => {
                    activities.remove(i);
                }
                (None, None) => return,
            }
        }
        self.inner.refresh_activity();
    }

    /// Draws the open notch showing `page` into PNG bytes, without showing anything on screen: the shape is laid out
    /// at its open size inside its own window and drawn on black from there. For checking the design.
    pub fn snapshot_png(&self, page: usize) -> Option<Vec<u8>> {
        let inner = &self.inner;
        if page >= inner.pages.borrow().len() {
            return None;
        }
        let was_open = inner.is_open();
        let before = inner.surface.frame();
        // The shape the page opens to on screen, as wide as on every other page.
        let size = inner.shape_size(page);
        let before_size = inner.open_size.replace(size);
        let open_pose = Pose {
            size,
            radius: PANEL_RADIUS,
        };
        instantly(|| {
            inner.content.setHidden(false);
            inner.content.setAlphaValue(1.0);
            inner.switcher.setHidden(false);
            inner.surface.setFrame(NSRect::new(NSPoint::ZERO, size));
            // The whole outline, fillets included, in black: glass cannot be drawn off screen, so black stands in
            // behind the pages.
            inner.shape_ink(open_pose, 1.0, 0.0);
            inner.ink.setAlphaValue(1.0);
        });
        let shape = inner.shape.replace(Shape::Open);
        inner.select_page(page);
        inner.switcher.settle();
        if let Some(shown) = inner.pages.borrow().get(page) {
            shown.root.setAlphaValue(1.0);
            // Not part way through sliding in.
            shown
                .root
                .setFrameOrigin(inner.page_origin(inner.page_size(page)));
        }
        // The fillets reach out of the surface by `FLARE` on each side.
        let bounds = NSRect::new(
            NSPoint::new(-FLARE, 0.0),
            NSSize::new(size.width + 2.0 * FLARE, size.height),
        );
        let png = inner
            .surface
            .bitmapImageRepForCachingDisplayInRect(bounds)
            .and_then(|rep| {
                inner
                    .surface
                    .cacheDisplayInRect_toBitmapImageRep(bounds, &rep);
                // SAFETY: an empty properties dictionary is valid for PNG output.
                unsafe {
                    rep.representationUsingType_properties(
                        NSBitmapImageFileType::PNG,
                        &NSDictionary::new(),
                    )
                }
            })
            .map(|data| data.to_vec());
        inner.shape.set(shape);
        inner.open_size.set(before_size);
        inner.show_pose(inner.pose.get(), 0.0);
        if !was_open {
            if let Some(shown) = inner.shown.take()
                && let Some(page) = inner.pages.borrow().get(shown)
            {
                (page.on_hide)();
            }
            instantly(|| {
                inner.content.setHidden(true);
                inner.content.setAlphaValue(0.0);
                inner.surface.setFrame(before);
            });
        }
        png
    }

    /// Draws the closed notch as it is now (rest, ears, ears with text) into PNG bytes, without showing anything:
    /// the surface and the margin the fillets use, drawn from where it stands. Call it once a movement has settled.
    pub fn snapshot_closed_png(&self) -> Option<Vec<u8>> {
        let inner = &self.inner;
        let size = inner.surface.frame().size;
        let bounds = NSRect::new(
            NSPoint::new(-FLARE, 0.0),
            NSSize::new(size.width + 2.0 * FLARE, size.height),
        );
        inner
            .surface
            .bitmapImageRepForCachingDisplayInRect(bounds)
            .and_then(|rep| {
                inner
                    .surface
                    .cacheDisplayInRect_toBitmapImageRep(bounds, &rep);
                // SAFETY: an empty properties dictionary is valid for PNG output.
                unsafe {
                    rep.representationUsingType_properties(
                        NSBitmapImageFileType::PNG,
                        &NSDictionary::new(),
                    )
                }
            })
            .map(|data| data.to_vec())
    }

    /// The size of the open shape on the page it is on, or on the one it opens on next.
    pub fn open_size(&self) -> NSSize {
        self.inner.open_size.get()
    }

    pub fn is_expanded(&self) -> bool {
        self.inner.is_open()
    }

    /// The size the shape has on screen right now, mid-movement included: read from what the window server draws.
    pub fn visible_size(&self) -> NSSize {
        let Some(layer) = self.inner.surface.layer() else {
            return NSSize::new(0.0, 0.0);
        };
        // SAFETY: the presentation layer is a read-only copy of the layer as it is drawn now.
        let shown = unsafe { layer.presentationLayer() }.unwrap_or(layer);
        shown.bounds().size
    }

    /// Shows or removes the notch panel. While it is off, hovering the top edge does nothing.
    pub fn set_enabled(&self, enabled: bool) {
        if self.inner.enabled.replace(enabled) == enabled {
            return;
        }
        if enabled {
            self.inner.panel.orderFrontRegardless();
            let target = self.inner.closed_shape();
            self.inner.move_to(target);
        } else {
            self.inner.collapse();
            self.inner.panel.orderOut(None);
            self.inner.sync_bars();
        }
    }

    /// Keeps the notch closed and deaf to the pointer while the top edge belongs to something else, such as the
    /// drop zones of a window being dragged. An open notch closes. Lifting it makes the next pointer move count as
    /// a fresh arrival.
    pub fn set_suppressed(&self, suppressed: bool) {
        if self.inner.suppressed.replace(suppressed) == suppressed {
            return;
        }
        self.inner.inside.set(false);
        if suppressed {
            self.inner.collapse();
        }
    }

    /// Snoozes the notch: while it is on, the pointer reaching the top edge does not open it. A click on it, a
    /// shortcut and anything that opens it by itself (a coding agent that asks, an alert) still do, so nothing that
    /// waits for the person is lost. An open notch closes.
    pub fn set_snoozed(&self, snoozed: bool) {
        if self.inner.snoozed.replace(snoozed) == snoozed {
            return;
        }
        if snoozed {
            self.inner.collapse();
        }
    }

    pub fn is_snoozed(&self) -> bool {
        self.inner.snoozed.get()
    }

    /// Whether the pointer is over the notch right now; something that opened it by itself leaves it open while the
    /// user is there.
    pub fn pointer_inside(&self) -> bool {
        self.inner.inside.get()
    }

    /// Marks a page's icon with a dot while it has something that needs the user.
    pub fn set_page_badge(&self, index: usize, on: bool) {
        {
            let mut badges = self.inner.badges.borrow_mut();
            badges.retain(|&i| i != index);
            if on {
                badges.push(index);
            }
        }
        self.inner.sync_switcher();
    }

    /// Closes the notch, as moving the pointer away does.
    pub fn close(&self) {
        self.inner.collapse();
    }

    /// Opens without taking keyboard focus, as hovering does.
    pub fn open(&self) {
        self.inner.expand(false, None);
    }

    /// Opens and focuses the search field, as a click on the notch does.
    pub fn open_focused(&self) {
        self.inner.expand(true, None);
    }

    /// Opens if closed, closes if open: for a shortcut.
    pub fn toggle(&self) {
        if self.inner.is_open() {
            self.inner.collapse();
        } else {
            self.inner.expand(true, None);
        }
    }

    /// Opens on the given page (0 is the clipboard). A page the user has hidden is not shown.
    pub fn open_page(&self, index: usize) {
        if self.inner.page_visible(index) {
            self.inner.expand(false, Some(index));
        }
    }

    /// Whether the user can see this page's tab.
    pub fn page_shown(&self, index: usize) -> bool {
        self.inner.page_visible(index)
    }

    /// Whether the notch is on a screen with a hardware notch.
    pub fn on_hardware_notch(&self) -> bool {
        self.inner.geometry.get().hardware
    }

    /// Where the notch is: its screen and its resting size.
    pub fn geometry(&self) -> NotchGeometry {
        self.inner.geometry.get()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn geometry(hardware: bool) -> NotchGeometry {
        NotchGeometry {
            screen: NSRect::new(NSPoint::new(0.0, 0.0), NSSize::new(1512.0, 982.0)),
            notch: NSSize::new(200.0, 32.0),
            hardware,
        }
    }

    #[test]
    fn frame_hugs_top_edge_and_is_centered() {
        let f = geometry(true).frame(200.0, 32.0);
        assert_eq!((f.origin.x, f.origin.y), (656.0, 950.0));
        assert_eq!(f.origin.x, 1512.0 - (f.origin.x + f.size.width));
        assert_eq!(f.origin.y + f.size.height, 982.0);
    }

    #[test]
    fn collapsed_shape_is_the_cutout_or_a_thin_hot_zone() {
        let hardware = geometry(true).collapsed_frame();
        assert_eq!((hardware.size.width, hardware.size.height), (200.0, 32.0));
        let plain = geometry(false).collapsed_frame();
        assert_eq!(
            (plain.size.width, plain.size.height),
            (PILL_WIDTH, HOT_ZONE_HEIGHT)
        );
        // Both stay glued to the top edge so the pointer reaches them by moving up.
        for f in [hardware, plain] {
            assert_eq!(f.origin.y + f.size.height, 982.0);
        }
    }

    #[test]
    fn a_screen_without_a_notch_can_show_one_or_nothing() {
        let plain = geometry(false);
        assert_eq!(plain.rest_size(Rest::Notch).height, 32.0);
        assert_eq!(plain.rest_size(Rest::Hidden).height, HOT_ZONE_HEIGHT);
        // The hardware cutout is always the cutout, whatever the setting says.
        assert_eq!(geometry(true).rest_size(Rest::Hidden).height, 32.0);
    }

    #[test]
    fn trigger_area_is_taller_than_the_invisible_resting_shape() {
        let plain = geometry(false);
        let trigger = plain.trigger_rect();
        assert!(trigger.size.height >= TRIGGER_MIN_HEIGHT);
        assert!(trigger.size.width > plain.collapsed_frame().size.width);
        // The cutout is already tall enough on a real notch.
        let hardware = geometry(true);
        assert_eq!(
            hardware.trigger_rect().size.height,
            hardware.collapsed_frame().size.height
        );
        // A widened shape is caught over its whole width.
        let wide = hardware.trigger_for(NSSize::new(300.0, 32.0));
        assert_eq!(wide.size.width, 300.0 + 2.0 * TRIGGER_SIDE_SLACK);
    }

    #[test]
    fn pointer_pushed_against_the_top_edge_counts_as_inside() {
        let g = geometry(false);
        let trigger = g.trigger_rect();
        let top = g.screen.origin.y + g.screen.size.height;
        let centre = g.screen.size.width / 2.0;
        assert!(
            contains(trigger, NSPoint::new(centre, top)),
            "exact top edge"
        );
        assert!(contains(trigger, NSPoint::new(centre, top - 1.0)));
        assert!(!contains(
            trigger,
            NSPoint::new(centre, top - TRIGGER_MIN_HEIGHT - 1.0)
        ));
        assert!(!contains(trigger, NSPoint::new(centre + 200.0, top)));
    }

    #[test]
    fn expanded_shape_grows_down_from_the_same_top_edge() {
        let g = geometry(true);
        let (collapsed, expanded) = (g.collapsed_frame(), g.expanded_frame());
        assert_eq!(
            expanded.origin.y + expanded.size.height,
            collapsed.origin.y + collapsed.size.height
        );
        assert_eq!(
            expanded.origin.x + expanded.size.width / 2.0,
            collapsed.origin.x + collapsed.size.width / 2.0
        );
        assert!(
            g.top_inset() > g.notch.height,
            "content must clear the cutout"
        );
    }

    #[test]
    fn a_dark_or_grey_cover_still_gives_bars_that_read_on_black() {
        let dark = brighten([0.10, 0.02, 0.02]);
        assert!(dark[0] > 0.9 && dark[1] < dark[0], "{dark:?}");
        let grey = brighten([0.3, 0.3, 0.3]);
        assert!(grey.iter().all(|c| *c > 0.9));
        let black = brighten([0.0, 0.0, 0.0]);
        assert!(black.iter().all(|c| *c > 0.9));
    }

    #[test]
    fn ears_are_wide_enough_for_a_square_and_never_too_wide() {
        let height = 32.0;
        let symbol = Ear::Symbol {
            name: "terminal",
            tone: Tone::Blue,
            pulse: false,
        };
        assert!(symbol.width(height) >= height);
        let long = Ear::Text {
            text: "a very long text that should never fit".into(),
            tone: Tone::Gray,
        };
        assert_eq!(long.width(height), EAR_MAX);
        let bars = Ear::Bars {
            levels: Rc::new(|_: &mut [f32]| {}),
            color: [1.0, 1.0, 1.0],
        };
        assert!(bars.width(height) >= height * EAR_MIN_SHARE);
    }

    #[test]
    fn the_opening_spring_overshoots_a_little_and_settles_in_time() {
        let spring = Spring::new(0.42, 0.22, 0.0);
        assert_eq!(spring.at(0.0), (0.0, 0.0));
        let samples: Vec<f64> = (0..=120)
            .map(|f| spring.at(f64::from(f) / 120.0).0)
            .collect();
        let peak = samples.iter().copied().fold(0.0, f64::max);
        assert!(
            (1.005..1.04).contains(&peak),
            "one small overshoot, got {peak}"
        );
        assert!(
            spring.at(0.2).0 > 0.85,
            "most of the way in a fifth of a second"
        );
        let distance = 418.0;
        let done = spring.settling_time(distance);
        assert!((0.4..0.9).contains(&done), "settles in {done} s");
        assert!(((1.0 - spring.at(done).0) * distance).abs() < 0.5);
    }

    #[test]
    fn a_spring_without_bounce_never_overshoots_and_keeps_a_carried_speed() {
        let spring = Spring::new(0.3, 0.0, 0.0);
        let mut last = 0.0;
        for f in 1..=120 {
            let x = spring.at(f64::from(f) / 120.0).0;
            assert!(x >= last && x <= 1.0, "monotonic up to the end");
            last = x;
        }
        let moving = Spring::new(0.42, 0.22, -3.0);
        assert!(
            (moving.at(0.0).1 + 3.0).abs() < 1e-9,
            "starts at the speed it was given"
        );
        assert!(
            moving.at(0.01).0 < 0.0,
            "first carries on the way it was going"
        );
        assert!(moving.settled(moving.settling_time(418.0), 418.0));
    }

    #[test]
    fn poses_mix_size_and_radius_and_measure_the_larger_side() {
        let (a, b) = (
            Pose {
                size: NSSize::new(200.0, 22.0),
                radius: 10.0,
            },
            Pose {
                size: EXPANDED_SIZE,
                radius: PANEL_RADIUS,
            },
        );
        assert_eq!(a.toward(b, 1.0), b);
        assert_eq!(a.toward(b, 0.0), a);
        assert_eq!(a.distance(b), 418.0);
        assert!(
            a.toward(b, 1.02).size.height > EXPANDED_SIZE.height,
            "overshoot shows"
        );
    }
}
