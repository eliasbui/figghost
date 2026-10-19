//! The drag wheel: while files are dragged out of any app, tapping a key (Shift, Command or Control unless the
//! user chose others) opens a ring of slots around the pointer, and dropping the files on a slot hands them to the
//! app. This file notices the drag, owns the panel that takes the drop and draws the ring; what a slot does is the
//! caller's business.
//!
//! How a drag is noticed, as found by trying it against a second process dragging a file (examples/wheel_check):
//! - A global mouse monitor sees the left button, and the drag events of another app's drag session, without any
//!   permission.
//! - The drag pasteboard (`NSPasteboardNameDrag`) gets a new change count when a session starts, so a count that
//!   differs from the one at mouse down, with file URLs on it, means files are being dragged. Window drags and
//!   text selections leave the count alone or carry no file URL.
//! - The modifier keys are polled with `NSEvent::modifierFlags()` on a timer while such a drag lasts. That reads
//!   the session's key state and needs no permission, unlike relying on a flagsChanged monitor, which the system
//!   may withhold without Accessibility access.
//! - A wheel bound to a key that is not a modifier is opened by a global keyDown monitor that lives only while
//!   files are dragged. Monitors of keys need Accessibility, as the app's other key features do; the monitor hands
//!   on the code of the key and keeps nothing.
//! - A panel ordered front in the middle of the drag receives `draggingEntered`, `draggingUpdated` and
//!   `performDragOperation` like any window, but only for its opaque pixels: a drop on a transparent pixel goes
//!   to the window underneath, as usual. The hub is therefore drawn solid and refuses the drop.
//!
//! The global monitor does not see events that belong to this process, so a drag that starts in figghost itself
//! (the shelf, a capture) never opens a wheel.
//!
//! The look is the approved design of `design-ui/notes.md`: a round plate, slots as the equal sectors of a ring
//! with rounded corners, a hub that names the wheel in the colour of its kind, and the lit slot inverted. The
//! plate is Liquid Glass, tinted dark as the notch's glass is, with the slot view as its content; before macOS 26
//! a dark vibrancy disc stands in, and with Increase Contrast or Reduce Transparency the plate is the opaque
//! dark one the design was drawn with. The shadow is the window's own: Liquid Glass turns flat grey when
//! anything of the same window lies under it, so no shadow layer or view may sit below the glass. Every size
//! and colour is in the constants below.

use crate::{
    app,
    notch::{Material, glass_tint, vibrancy_of},
    pasteboard::file_paths,
    snap_ui::{Mouse, MouseMonitor},
    style::{self, Weight},
    widgets::{self, Fit},
    workspace,
};
use block2::RcBlock;
use objc2::{
    AnyThread, DefinedClass, MainThreadMarker, MainThreadOnly, define_class, msg_send,
    rc::Retained,
    runtime::{AnyClass, AnyObject, ProtocolObject},
};
use objc2_app_kit::{
    NSAccessibility, NSBackingStoreType, NSBezierPath, NSBitmapImageFileType, NSBitmapImageRep,
    NSColor, NSCompositingOperation, NSDeviceRGBColorSpace, NSDragOperation, NSDraggingDestination,
    NSDraggingInfo, NSEvent, NSEventMask, NSEventModifierFlags, NSGlassEffectView,
    NSGraphicsContext, NSLineJoinStyle, NSMainMenuWindowLevel, NSPanel, NSPasteboard,
    NSPasteboardNameDrag, NSPasteboardTypeFileURL, NSScreen, NSStringDrawing,
    NSStringDrawingOptions, NSStringNSExtendedStringDrawing, NSView, NSVisualEffectBlendingMode,
    NSVisualEffectState, NSVisualEffectView, NSWindingRule, NSWindowAnimationBehavior,
    NSWindowCollectionBehavior, NSWindowStyleMask,
};
use objc2_foundation::{
    NSArray, NSDictionary, NSObjectProtocol, NSPoint, NSRect, NSSize, NSString,
};
use std::{
    cell::{Cell, RefCell},
    f64::consts::{PI, TAU},
    path::PathBuf,
    ptr::NonNull,
    rc::{Rc, Weak},
    time::Instant,
};

// ---------------------------------------------------------------- sizes, in one place

/// The channel between two slots, and between the slots and the hub.
const GAP: f64 = 4.0;
/// The radius of every corner of a slot.
const CORNER: f64 = 7.0;
/// The plate beyond the outer edge of the slots.
const RIM: f64 = 6.0;
/// How far inside the screen the plate is kept.
const EDGE_MARGIN: f64 = 8.0;
/// Inner radius of the ring by number of slots (1 to 12), for wheels whose titles all fit on one short line, and
/// for the rest. The outer radius is the inner one plus the thickness.
const COMPACT_INNER: [f64; MAX_SLOTS] = [
    56.0, 56.0, 56.0, 56.0, 56.0, 56.0, 56.0, 56.0, 60.0, 70.0, 80.0, 90.0,
];
const REGULAR_INNER: [f64; MAX_SLOTS] = [
    56.0, 56.0, 56.0, 56.0, 56.0, 56.0, 60.0, 74.0, 90.0, 100.0, 120.0, 132.0,
];
const COMPACT_THICKNESS: f64 = 60.0;
const REGULAR_THICKNESS: f64 = 92.0;
/// The widest title that still counts as short, and the width titles wrap at in each size of wheel.
const COMPACT_TITLE_WIDTH: f64 = 36.0;
const REGULAR_TITLE_WIDTH: f64 = 66.0;
const REGULAR_TITLE_LINES: f64 = 2.0;
const TITLE_SIZE: f64 = 11.0;
const TITLE_LINE: f64 = 13.0;
const SYMBOL_SIZE: f64 = 18.0;
/// Between a slot's symbol and its title.
const SYMBOL_GAP: f64 = 3.0;
const HUB_SYMBOL_SIZE: f64 = 16.0;
const HUB_SYMBOL_GAP: f64 = 4.0;
const HUB_CAPTION_SIZE: f64 = 11.0;
const HUB_CAPTION_LINE: f64 = 13.0;
const HUB_TITLE_SIZE: f64 = 12.0;
const HUB_TITLE_LINE: f64 = 15.0;
const HUB_TEXT_LINES: f64 = 2.0;
/// The scale the wheel opens from.
const OPEN_SCALE: f64 = 0.94;
const OPEN_SECONDS: f64 = 0.12;
const OPEN_REDUCED_SECONDS: f64 = 0.10;
const SWITCH_SECONDS: f64 = 0.12;
const SWITCH_REDUCED_SECONDS: f64 = 0.08;
const CLOSE_SECONDS: f64 = 0.08;
const FRAME_SECONDS: f64 = 1.0 / 120.0;
/// How often the modifier keys are looked at while files are dragged.
const POLL_SECONDS: f64 = 0.016;
/// How long the wheel stays after the button goes up. The monitor sees the release a few milliseconds before the
/// drop reaches the panel, and taking the panel away first would cancel the drop.
const HIDE_DELAY_SECONDS: f64 = 0.25;

pub const MAX_SLOTS: usize = 12;

// ---------------------------------------------------------------- colours

fn srgb(r: f64, g: f64, b: f64, a: f64) -> Retained<NSColor> {
    NSColor::colorWithSRGBRed_green_blue_alpha(r, g, b, a)
}

/// What the plate under the slots is made of.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Plate {
    /// Liquid Glass, tinted dark; the view draws only the slots and the hub on it.
    Glass,
    /// A dark vibrancy disc, before macOS 26.
    Vibrancy,
    /// The solid dark plate, for Increase Contrast and Reduce Transparency.
    Opaque,
    /// A dark translucent disc drawn by the view, standing in for the glass in snapshots.
    Standin,
}

/// The colours of the wheel, which is dark in every appearance. On glass the slots are a very light white that
/// lets the glass show through, with the lit slot solid white; on the opaque plate every fill is opaque.
struct Palette {
    plate: Retained<NSColor>,
    hairline: Retained<NSColor>,
    slot: Retained<NSColor>,
    slot_lit: Retained<NSColor>,
    symbol: Retained<NSColor>,
    symbol_lit: Retained<NSColor>,
    title: Retained<NSColor>,
    title_lit: Retained<NSColor>,
    hub: Retained<NSColor>,
    caption: Retained<NSColor>,
    hub_title: Retained<NSColor>,
}

/// With Increase Contrast on, resting slots are lighter, titles brighter and the plate's edge stronger.
fn palette(plate: Plate, contrast: bool) -> Palette {
    if plate == Plate::Opaque {
        return Palette {
            plate: srgb(0.090, 0.094, 0.102, 1.0),
            hairline: style::white(if contrast { 0.30 } else { 0.10 }),
            slot: if contrast {
                srgb(0.227, 0.231, 0.247, 1.0)
            } else {
                srgb(0.176, 0.180, 0.192, 1.0)
            },
            slot_lit: srgb(1.0, 1.0, 1.0, 1.0),
            symbol: style::white(0.92),
            symbol_lit: style::black(0.85),
            title: style::white(if contrast { 0.92 } else { 0.72 }),
            title_lit: style::black(0.85),
            hub: srgb(0.055, 0.058, 0.064, 1.0),
            caption: style::white(style::TEXT_SECONDARY),
            hub_title: style::white(style::TEXT),
        };
    }
    Palette {
        // Only the stand-in draws a plate: the glass is the plate. Black, as the tint of the notch's glass.
        plate: style::black(0.55),
        hairline: style::white(0.12),
        // A very light fill, rather than hairlines: it keeps the shape of every slot, the channels and the
        // rounded corners of the design, which a hairline on a busy blurred background does not.
        slot: style::white(0.10),
        slot_lit: srgb(1.0, 1.0, 1.0, 1.0),
        symbol: style::white(style::TEXT),
        symbol_lit: style::black(0.85),
        title: style::white(style::TEXT),
        title_lit: style::black(0.85),
        // A recessed well: darker than the glass round it.
        hub: style::black(0.34),
        caption: style::white(0.75),
        hub_title: style::white(style::TEXT),
    }
}

/// The colour of a kind's hub ring and symbol: the system's blue, orange and purple, written out so that a
/// snapshot has the same pixels in every process.
fn accent(kind: Kind) -> Retained<NSColor> {
    match kind {
        Kind::Formats => srgb(0.039, 0.518, 1.0, 1.0),
        Kind::Tools => srgb(1.0, 0.584, 0.0, 1.0),
        Kind::Actions => srgb(0.749, 0.353, 0.949, 1.0),
    }
}

fn kind_symbol(kind: Kind) -> &'static str {
    match kind {
        Kind::Formats => "arrow.triangle.2.circlepath",
        Kind::Tools => "wrench.and.screwdriver",
        Kind::Actions => "bolt",
    }
}

// ---------------------------------------------------------------- the public shapes

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Formats,
    Tools,
    Actions,
}

/// A modifier key a wheel can be bound to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Modifier {
    Shift,
    Command,
    Control,
    Option,
}

/// What opens a wheel: a tap of a modifier key alone, or a press of any other key, by its virtual key code.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Trigger {
    Modifier(Modifier),
    Key(u16),
}

/// The key that opens each wheel; `None` means that wheel is off.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Keys {
    pub formats: Option<Trigger>,
    pub tools: Option<Trigger>,
    pub actions: Option<Trigger>,
}

impl Default for Keys {
    /// Shift for formats, Command for tools, Control for actions; Option stays a plain copy drag.
    fn default() -> Self {
        Self {
            formats: Some(Trigger::Modifier(Modifier::Shift)),
            tools: Some(Trigger::Modifier(Modifier::Command)),
            actions: Some(Trigger::Modifier(Modifier::Control)),
        }
    }
}

impl Keys {
    fn all(self) -> [(Option<Trigger>, Kind); 3] {
        [
            (self.formats, Kind::Formats),
            (self.tools, Kind::Tools),
            (self.actions, Kind::Actions),
        ]
    }

    /// The wheel bound to `trigger`, if any.
    fn wheel_of(self, trigger: Trigger) -> Option<Kind> {
        self.all()
            .into_iter()
            .find(|(bound, _)| *bound == Some(trigger))
            .map(|(_, kind)| kind)
    }

    /// Whether a wheel is bound to a key that is not a modifier, which takes watching the keys of other apps.
    fn any_key(self) -> bool {
        self.all()
            .into_iter()
            .any(|(bound, _)| matches!(bound, Some(Trigger::Key(_))))
    }
}

/// One slot of a wheel. `id` is opaque to this module and comes back in `chosen`.
#[derive(Debug, Clone, PartialEq)]
pub struct Slot {
    pub id: String,
    /// Short text drawn in the slot: "PNG", "Compress", "AirDrop". Long titles wrap to two lines; the hub shows
    /// the whole title while the slot is lit.
    pub title: String,
    /// SF Symbol name drawn above the title, or None for text only.
    pub symbol: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Wheel {
    /// 1 to MAX_SLOTS slots, in clockwise order starting at the top.
    pub slots: Vec<Slot>,
    /// Shown in the hub and read by assistive technology, e.g. "Convert 3 files".
    pub caption: String,
}

// ---------------------------------------------------------------- geometry

/// The radii of a wheel: the slots lie between `inner` and `outer`, the plate reaches `RIM` beyond, the hub
/// ends a `GAP` inside `inner`.
#[derive(Debug, Clone, Copy, PartialEq)]
struct Ring {
    inner: f64,
    outer: f64,
}

impl Ring {
    /// `compact` for wheels whose titles are all short, which can be smaller.
    fn new(count: usize, compact: bool) -> Self {
        let index = count.clamp(1, MAX_SLOTS) - 1;
        let (inner, thickness) = if compact {
            (COMPACT_INNER[index], COMPACT_THICKNESS)
        } else {
            (REGULAR_INNER[index], REGULAR_THICKNESS)
        };
        Self {
            inner,
            outer: inner + thickness,
        }
    }

    fn plate(self) -> f64 {
        self.outer + RIM
    }

    fn hub(self) -> f64 {
        self.inner - GAP
    }

    /// The side of the panel, which is the plate's diameter: the window's own shadow is drawn round it.
    fn side(self) -> f64 {
        2.0 * self.plate()
    }

    fn middle(self) -> f64 {
        (self.inner + self.outer) / 2.0
    }

    fn mix(self, to: Ring, t: f64) -> Ring {
        Ring {
            inner: style::mix(self.inner, to.inner, t),
            outer: style::mix(self.outer, to.outer, t),
        }
    }

    /// Where the pointer counts as being on a slot: from the middle of the channel round the hub to the edge of
    /// the plate, so the rim and the channels between slots belong to the nearest slot by angle.
    fn slot_at(self, count: usize, dx: f64, dy: f64) -> Option<usize> {
        slot_at(count, dx, dy, self.inner - GAP / 2.0, self.plate())
    }
}

/// Which slot is under a point given relative to the wheel centre (y up, AppKit), for a wheel of `count` slots
/// laid clockwise from the top, each an equal angle. Slot 0 is centred on the top. None in the hub (distance
/// below `inner`) or beyond `outer`. A point exactly on the line between two slots belongs to the one clockwise
/// of it, so every point has one answer.
pub fn slot_at(count: usize, dx: f64, dy: f64, inner: f64, outer: f64) -> Option<usize> {
    let distance = dx.hypot(dy);
    if count == 0 || !(distance >= inner && distance <= outer) {
        return None;
    }
    let width = TAU / count as f64;
    // Clockwise from the top, in [0, tau).
    let angle = dx.atan2(dy).rem_euclid(TAU);
    Some((((angle + width / 2.0) / width).floor() as usize) % count)
}

/// Where the middle of slot `index` of `count` lies from the wheel centre (y up) on a ring.
fn slot_centre(count: usize, index: usize, ring: Ring) -> (f64, f64) {
    let angle = TAU * index as f64 / count.max(1) as f64;
    (ring.middle() * angle.sin(), ring.middle() * angle.cos())
}

/// Where the middle of slot `index` of this wheel lies from the wheel centre (y up), for checks that need to drop
/// on a known slot. The size of the ring follows the titles, so it takes the whole wheel.
pub fn slot_offset(wheel: &Wheel, index: usize) -> (f64, f64) {
    slot_centre(
        wheel.slots.len(),
        index,
        Ring::new(wheel.slots.len(), is_compact(wheel)),
    )
}

/// The centre of a square of `side` that is as near `centre` as can be while lying inside `area` (x, y, width,
/// height). An area smaller than the square gets it centred.
fn fit_centre(centre: (f64, f64), side: f64, area: (f64, f64, f64, f64)) -> (f64, f64) {
    let axis = |c: f64, from: f64, length: f64| {
        if length <= side {
            from + length / 2.0
        } else {
            c.clamp(from + side / 2.0, from + length - side / 2.0)
        }
    };
    (
        axis(centre.0, area.0, area.2),
        axis(centre.1, area.1, area.3),
    )
}

// ---------------------------------------------------------------- when to open, switch and close

/// The modifier keys the decisions look at.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
struct Mods {
    shift: bool,
    command: bool,
    control: bool,
    option: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Event {
    /// The left button went down.
    Down,
    /// The pointer moved with the button down. `files` says the drag carries file URLs.
    Drag { files: bool, mods: Mods },
    /// The modifier keys as they are now, from the timer.
    Mods(Mods),
    /// A key that is not a modifier went down in some app, by its virtual key code.
    Key(u16),
    /// The button went up, or its release was missed.
    Up,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Act {
    Nothing,
    /// Show the wheel of this kind, or switch to it.
    Open(Kind),
    /// Take the wheel away.
    Hide,
}

/// What a person is doing with the mouse and keys, reduced to when the wheel opens, switches and closes. Free of
/// AppKit so it can be tested; the rest of the file only feeds it.
#[derive(Debug, Default)]
struct Machine {
    down: bool,
    files: bool,
    mods: Mods,
    kind: Option<Kind>,
    keys: Keys,
}

impl Machine {
    fn step(&mut self, event: Event) -> Act {
        match event {
            Event::Down => self.reset(true),
            Event::Up => self.reset(false),
            Event::Drag { files, mods } => {
                if !self.down {
                    return Act::Nothing;
                }
                // Keys already held when the drag turns into a file drag were not tapped for it.
                let first = files && !self.files;
                self.files = files;
                if first || !files {
                    self.mods = mods;
                    Act::Nothing
                } else {
                    self.pressed(mods)
                }
            }
            Event::Mods(mods) => {
                if self.down && self.files {
                    self.pressed(mods)
                } else {
                    self.mods = mods;
                    Act::Nothing
                }
            }
            Event::Key(code) => {
                if self.tracking() {
                    self.open_for(Trigger::Key(code))
                } else {
                    Act::Nothing
                }
            }
        }
    }

    /// Opens the wheel bound to `trigger`, unless it is the one already open or none is.
    fn open_for(&mut self, trigger: Trigger) -> Act {
        match self.keys.wheel_of(trigger) {
            Some(kind) if self.kind != Some(kind) => {
                self.kind = Some(kind);
                Act::Open(kind)
            }
            _ => Act::Nothing,
        }
    }

    /// Whether the keys need watching: files are being dragged.
    fn tracking(&self) -> bool {
        self.down && self.files
    }

    fn reset(&mut self, down: bool) -> Act {
        let was_open = self.kind.is_some();
        *self = Self {
            down,
            keys: self.keys,
            ..Self::default()
        };
        if was_open { Act::Hide } else { Act::Nothing }
    }

    /// A key that went down since the last look opens the wheel bound to it, held keys notwithstanding, so
    /// Command can be tapped while Shift is still down. Two keys going down in the same look are no tap of
    /// either, and a key that no wheel is bound to is ignored: Option stays a plain copy drag unless it is bound.
    fn pressed(&mut self, now: Mods) -> Act {
        let before = std::mem::replace(&mut self.mods, now);
        let mut newly = [
            (now.shift && !before.shift, Modifier::Shift),
            (now.command && !before.command, Modifier::Command),
            (now.control && !before.control, Modifier::Control),
            (now.option && !before.option, Modifier::Option),
        ]
        .into_iter()
        .filter(|(down, _)| *down);
        match (newly.next(), newly.next()) {
            (Some((_, modifier)), None) => self.open_for(Trigger::Modifier(modifier)),
            _ => Act::Nothing,
        }
    }
}

/// The keys pressed in other apps, for a wheel bound to a key that is not a modifier. Started only while files are
/// dragged and removed when dropped. Key monitors need Accessibility; without it none starts and only the
/// modifier keys open wheels. Repeats of a held key are not presses.
struct KeyMonitor {
    token: Retained<AnyObject>,
}

impl KeyMonitor {
    fn start(handler: impl Fn(u16) + 'static) -> Option<Self> {
        let block = RcBlock::new(move |event: NonNull<NSEvent>| {
            // SAFETY: AppKit passes a live event for the duration of the call.
            let event = unsafe { event.as_ref() };
            if !event.isARepeat() {
                handler(event.keyCode());
            }
        });
        let token =
            NSEvent::addGlobalMonitorForEventsMatchingMask_handler(NSEventMask::KeyDown, &block)?;
        Some(Self { token })
    }
}

impl Drop for KeyMonitor {
    fn drop(&mut self) {
        // SAFETY: the token came from addGlobalMonitorForEventsMatchingMask and is removed once.
        unsafe { NSEvent::removeMonitor(&self.token) };
    }
}

fn mods_now() -> Mods {
    let flags = NSEvent::modifierFlags_class();
    Mods {
        shift: flags.contains(NSEventModifierFlags::Shift),
        command: flags.contains(NSEventModifierFlags::Command),
        control: flags.contains(NSEventModifierFlags::Control),
        option: flags.contains(NSEventModifierFlags::Option),
    }
}

// ---------------------------------------------------------------- text

type Attributes =
    Retained<objc2_foundation::NSMutableDictionary<NSString, objc2::runtime::AnyObject>>;

fn title_attributes(color: &NSColor) -> Attributes {
    widgets::text_attributes(
        &style::font(TITLE_SIZE, Weight::Medium),
        color,
        Fit::Words,
        true,
    )
}

fn text_options() -> NSStringDrawingOptions {
    NSStringDrawingOptions::UsesLineFragmentOrigin
        | NSStringDrawingOptions::TruncatesLastVisibleLine
}

/// The width of `text` on one line.
fn line_width(text: &str, attrs: &Attributes) -> f64 {
    // SAFETY: `attrs` holds valid attribute objects for these keys.
    unsafe { NSString::from_str(text).sizeWithAttributes(Some(attrs)) }.width
}

/// How many lines (1 up to `most`) `text` takes wrapped at `width`; the line height is the design's, not the
/// font's, so the blocks of all slots line up. A title that is wider than one line takes the most lines it may:
/// a measured bounding box says one line for text that is only a little too wide, and the text would then be
/// cut with an ellipsis when it could have wrapped.
fn line_count(text: &str, attrs: &Attributes, width: f64, most: f64) -> f64 {
    if line_width(text, attrs) <= width {
        1.0
    } else {
        most
    }
}

/// Draws `text` wrapped in `rect`, from its top edge. The box is a little taller than the lines, because the font's
/// own line height is a shade more than the design's, and a last line that does not quite fit would turn the one
/// above it into an ellipsis.
fn draw_wrapped(text: &str, rect: NSRect, attrs: &Attributes) {
    const SLACK: f64 = 4.0;
    let rect = NSRect::new(
        NSPoint::new(rect.origin.x, rect.origin.y - SLACK),
        NSSize::new(rect.size.width, rect.size.height + SLACK),
    );
    // SAFETY: called from drawRect: with a valid graphics context; `attrs` holds valid attribute objects.
    unsafe {
        NSString::from_str(text).drawWithRect_options_attributes_context(
            rect,
            text_options(),
            Some(attrs),
            None,
        );
    }
}

/// Whether every title fits on one short line, so the wheel can be the smaller one.
fn is_compact(wheel: &Wheel) -> bool {
    let attrs = title_attributes(&style::white(1.0));
    wheel
        .slots
        .iter()
        .all(|slot| line_width(&slot.title, &attrs) <= COMPACT_TITLE_WIDTH)
}

// ---------------------------------------------------------------- drawing

struct Look {
    kind: Kind,
    wheel: Wheel,
    compact: bool,
    plate: Plate,
    /// Increase Contrast was on when the wheel opened.
    contrast: bool,
}

impl Look {
    fn new(kind: Kind, wheel: Wheel, plate: Plate, contrast: bool) -> Self {
        let compact = is_compact(&wheel);
        Self {
            kind,
            wheel,
            compact,
            plate,
            contrast,
        }
    }

    fn ring(&self) -> Ring {
        Ring::new(self.wheel.slots.len(), self.compact)
    }
}

/// What a frame of the view shows besides the look: the slot lit, how much of the slot contents has faded in, and
/// the ring the wheel has now (between two sizes while it switches kind).
#[derive(Clone, Copy)]
struct Frame {
    hover: Option<usize>,
    reveal: f64,
    ring: Ring,
}

fn disc(centre: NSPoint, radius: f64) -> Retained<NSBezierPath> {
    NSBezierPath::bezierPathWithOvalInRect(NSRect::new(
        NSPoint::new(centre.x - radius, centre.y - radius),
        NSSize::new(2.0 * radius, 2.0 * radius),
    ))
}

/// The shape of slot `index` of `count`: the annular sector, a `GAP` away from its neighbours and the hub, with
/// rounded corners. Built by drawing the sector shrunk by `CORNER` and fattening it by the same, which rounds
/// the corners exactly; the path alone is the shrunk one, so it has to be filled and stroked (see `fill_slot`).
fn slot_path(centre: NSPoint, count: usize, index: usize, ring: Ring) -> Retained<NSBezierPath> {
    if count == 1 {
        // The whole ring, no channels and no corners.
        let path = disc(centre, ring.outer);
        path.appendBezierPath(&disc(centre, ring.inner));
        path.setWindingRule(NSWindingRule::EvenOdd);
        return path;
    }
    let angle = TAU * index as f64 / count as f64;
    let half = PI / count as f64;
    // The radial edges move a `GAP / 2 + CORNER` into the slot; the arcs shrink by `CORNER`.
    let shift = GAP / 2.0 + CORNER;
    let (near, far) = (ring.inner + CORNER, ring.outer - CORNER);
    let edge = |boundary: f64, side: f64, radius: f64| {
        let along = (radius * radius - shift * shift).sqrt();
        NSPoint::new(
            centre.x + along * boundary.sin() + side * shift * boundary.cos(),
            centre.y + along * boundary.cos() - side * shift * boundary.sin(),
        )
    };
    let degrees = |p: NSPoint| (p.y - centre.y).atan2(p.x - centre.x).to_degrees();
    let (first, last) = (angle - half, angle + half);
    let (near_first, far_first) = (edge(first, 1.0, near), edge(first, 1.0, far));
    let (far_last, near_last) = (edge(last, -1.0, far), edge(last, -1.0, near));
    let path = NSBezierPath::bezierPath();
    path.moveToPoint(near_first);
    path.lineToPoint(far_first);
    // AppKit angles run counter-clockwise from the right; the slots run clockwise.
    path.appendBezierPathWithArcWithCenter_radius_startAngle_endAngle_clockwise(
        centre,
        far,
        degrees(far_first),
        degrees(far_last),
        true,
    );
    path.lineToPoint(near_last);
    path.appendBezierPathWithArcWithCenter_radius_startAngle_endAngle_clockwise(
        centre,
        near,
        degrees(near_last),
        degrees(near_first),
        false,
    );
    path.closePath();
    path
}

fn fill_slot(path: &NSBezierPath, count: usize, color: &NSColor) {
    // The fill and the stroke that rounds the corners overlap, and a translucent colour would show the overlap as
    // a band. Copying the colour in, instead of blending it, makes the whole slot one colour.
    let context = NSGraphicsContext::currentContext();
    if let Some(context) = &context {
        context.setCompositingOperation(NSCompositingOperation::Copy);
    }
    color.set();
    path.fill();
    if count > 1 {
        path.setLineWidth(2.0 * CORNER);
        path.setLineJoinStyle(NSLineJoinStyle::Round);
        path.stroke();
    }
    if let Some(context) = &context {
        context.setCompositingOperation(NSCompositingOperation::SourceOver);
    }
}

/// The block of a symbol over a title, centred on `middle`, in a slot.
fn draw_slot_contents(
    slot: &Slot,
    middle: NSPoint,
    compact: bool,
    lit: bool,
    colors: &Palette,
    alpha: f64,
) {
    let (width, most) = if compact {
        (COMPACT_TITLE_WIDTH, 1.0)
    } else {
        (REGULAR_TITLE_WIDTH, REGULAR_TITLE_LINES)
    };
    let title_color = if lit {
        &colors.title_lit
    } else {
        &colors.title
    };
    let symbol_color = if lit {
        &colors.symbol_lit
    } else {
        &colors.symbol
    };
    let attrs = title_attributes(
        &title_color.colorWithAlphaComponent(title_color.alphaComponent() * alpha),
    );
    let lines = line_count(&slot.title, &attrs, width, most);
    let text_height = lines * TITLE_LINE;
    let symbol = slot.symbol.as_deref().and_then(|name| {
        style::symbol(
            name,
            SYMBOL_SIZE,
            &symbol_color.colorWithAlphaComponent(symbol_color.alphaComponent() * alpha),
        )
    });
    let block = text_height
        + if symbol.is_some() {
            SYMBOL_SIZE + SYMBOL_GAP
        } else {
            0.0
        };
    let top = middle.y + block / 2.0;
    if let Some(image) = symbol {
        let natural = image.size();
        image.drawInRect(NSRect::new(
            NSPoint::new(
                middle.x - natural.width / 2.0,
                top - SYMBOL_SIZE / 2.0 - natural.height / 2.0,
            ),
            natural,
        ));
    }
    draw_wrapped(
        &slot.title,
        NSRect::new(
            NSPoint::new(middle.x - width / 2.0, middle.y - block / 2.0),
            NSSize::new(width, text_height),
        ),
        &attrs,
    );
}

/// The hub: a recessed disc in the kind's colour, its symbol, and under it the title of the lit slot or else the
/// caption.
fn draw_hub(look: &Look, hover: Option<usize>, centre: NSPoint, ring: Ring, colors: &Palette) {
    let radius = ring.hub();
    colors.hub.set();
    disc(centre, radius).fill();
    let tint = accent(look.kind);
    tint.set();
    let edge = disc(centre, radius - 0.75);
    edge.setLineWidth(1.5);
    edge.stroke();

    let lit = hover.and_then(|i| look.wheel.slots.get(i));
    let (text, color, font, line) = match lit {
        Some(slot) => (
            slot.title.as_str(),
            &colors.hub_title,
            style::font(HUB_TITLE_SIZE, Weight::Semibold),
            HUB_TITLE_LINE,
        ),
        None => (
            look.wheel.caption.as_str(),
            &colors.caption,
            style::font(HUB_CAPTION_SIZE, Weight::Medium),
            HUB_CAPTION_LINE,
        ),
    };
    let width = 1.6 * radius;
    let attrs = widgets::text_attributes(&font, color, Fit::Words, true);
    let text_height = if text.is_empty() {
        0.0
    } else {
        line_count(text, &attrs, width, HUB_TEXT_LINES) * line
    };
    let block = HUB_SYMBOL_SIZE
        + if text_height > 0.0 {
            HUB_SYMBOL_GAP + text_height
        } else {
            0.0
        };
    let top = centre.y + block / 2.0;
    if let Some(image) = style::small_symbol(kind_symbol(look.kind), HUB_SYMBOL_SIZE, &tint) {
        let natural = image.size();
        image.drawInRect(NSRect::new(
            NSPoint::new(
                centre.x - natural.width / 2.0,
                top - HUB_SYMBOL_SIZE / 2.0 - natural.height / 2.0,
            ),
            natural,
        ));
    }
    if text_height > 0.0 {
        draw_wrapped(
            text,
            NSRect::new(
                NSPoint::new(
                    centre.x - width / 2.0,
                    top - HUB_SYMBOL_SIZE - HUB_SYMBOL_GAP - text_height,
                ),
                NSSize::new(width, text_height),
            ),
            &attrs,
        );
    }
}

fn paint(look: &Look, frame: Frame, bounds: NSRect) {
    let colors = palette(look.plate, look.contrast);
    let centre = NSPoint::new(
        bounds.origin.x + bounds.size.width / 2.0,
        bounds.origin.y + bounds.size.height / 2.0,
    );
    let ring = frame.ring;
    let count = look.wheel.slots.len();

    if look.plate == Plate::Opaque {
        colors.plate.set();
        disc(centre, ring.plate()).fill();
        colors.hairline.set();
        let edge = disc(centre, ring.plate() - 0.5);
        edge.setLineWidth(1.0);
        edge.stroke();
    }
    for (i, slot) in look.wheel.slots.iter().enumerate() {
        let lit = frame.hover == Some(i);
        let path = slot_path(centre, count, i, ring);
        fill_slot(
            &path,
            count,
            if lit { &colors.slot_lit } else { &colors.slot },
        );
        if frame.reveal > 0.0 {
            let (dx, dy) = slot_centre(count, i, ring);
            let middle = NSPoint::new(centre.x + dx, centre.y + dy);
            draw_slot_contents(slot, middle, look.compact, lit, &colors, frame.reveal);
        }
    }
    draw_hub(look, frame.hover, centre, ring, &colors);
    if look.plate == Plate::Standin {
        // Under everything already drawn: the slots were copied in and must not be covered.
        if let Some(context) = NSGraphicsContext::currentContext() {
            context.setCompositingOperation(NSCompositingOperation::DestinationOver);
        }
        colors.plate.set();
        disc(centre, ring.plate()).fill();
        colors.hairline.set();
        let edge = disc(centre, ring.plate() - 0.5);
        edge.setLineWidth(1.0);
        edge.stroke();
        if let Some(context) = NSGraphicsContext::currentContext() {
            context.setCompositingOperation(NSCompositingOperation::SourceOver);
        }
    }
}

// ---------------------------------------------------------------- the view that takes the drop

type DropHandler = Rc<dyn Fn(usize, Vec<PathBuf>)>;
type Offer = Box<dyn Fn(Kind, &[PathBuf]) -> Option<Wheel>>;
type Chosen = Box<dyn Fn(Kind, &str, Vec<PathBuf>)>;

struct ViewIvars {
    look: RefCell<Option<Look>>,
    hover: Cell<Option<usize>>,
    /// How much of the slot contents shows (1 at rest).
    reveal: Cell<f64>,
    /// While the wheel switches kind: the ring it came from and how far it has moved to the new one (0 to 1).
    morph: Cell<Option<(Ring, f64)>>,
    /// Called with the slot and the dragged paths when files are dropped on a slot.
    on_drop: RefCell<Option<DropHandler>>,
}

define_class!(
    // SAFETY: NSView allows subclassing; the ivars are only touched on the main thread and need no Drop.
    #[unsafe(super(NSView))]
    #[thread_kind = MainThreadOnly]
    #[name = "FigWheelView"]
    #[ivars = ViewIvars]
    struct WheelView;

    impl WheelView {
        #[unsafe(method(isOpaque))]
        fn is_opaque(&self) -> bool {
            false
        }

        #[unsafe(method(drawRect:))]
        fn draw_rect(&self, _dirty: NSRect) {
            if let Some(look) = self.ivars().look.borrow().as_ref() {
                let target = look.ring();
                let ring = match self.ivars().morph.get() {
                    Some((from, t)) => from.mix(target, t),
                    None => target,
                };
                paint(
                    look,
                    Frame {
                        hover: self.ivars().hover.get(),
                        reveal: self.ivars().reveal.get(),
                        ring,
                    },
                    self.bounds(),
                );
            }
        }
    }

    unsafe impl NSObjectProtocol for WheelView {}

    unsafe impl NSDraggingDestination for WheelView {
        #[unsafe(method(draggingEntered:))]
        fn dragging_entered(&self, sender: &ProtocolObject<dyn NSDraggingInfo>) -> NSDragOperation {
            self.track(sender)
        }

        #[unsafe(method(draggingUpdated:))]
        fn dragging_updated(&self, sender: &ProtocolObject<dyn NSDraggingInfo>) -> NSDragOperation {
            self.track(sender)
        }

        #[unsafe(method(draggingExited:))]
        fn dragging_exited(&self, _sender: Option<&ProtocolObject<dyn NSDraggingInfo>>) {
            self.set_hover(None);
        }

        #[unsafe(method(performDragOperation:))]
        fn perform_drag_operation(&self, sender: &ProtocolObject<dyn NSDraggingInfo>) -> bool {
            let slot = self.slot_under(sender);
            let handler = self.ivars().on_drop.borrow().clone();
            let paths = file_paths(&sender.draggingPasteboard())
                .into_iter()
                .map(PathBuf::from)
                .collect::<Vec<_>>();
            match (slot, handler) {
                // The slot stays lit while the wheel fades away.
                (Some(slot), Some(handler)) if !paths.is_empty() => {
                    handler(slot, paths);
                    true
                }
                _ => {
                    self.set_hover(None);
                    false
                }
            }
        }
    }
);

impl WheelView {
    fn new(mtm: MainThreadMarker) -> Retained<Self> {
        let ivars = ViewIvars {
            look: RefCell::new(None),
            hover: Cell::new(None),
            reveal: Cell::new(1.0),
            morph: Cell::new(None),
            on_drop: RefCell::new(None),
        };
        // SAFETY: designated initializer with valid arguments on the main thread.
        unsafe {
            msg_send![
                super(mtm.alloc::<Self>().set_ivars(ivars)),
                initWithFrame: NSRect::ZERO
            ]
        }
    }

    fn set_look(&self, look: Look) {
        self.setAccessibilityElement(true);
        self.setAccessibilityLabel(Some(&NSString::from_str(&look.wheel.caption)));
        *self.ivars().look.borrow_mut() = Some(look);
        self.ivars().hover.set(None);
        self.setNeedsDisplay(true);
    }

    fn ring(&self) -> Option<Ring> {
        self.ivars().look.borrow().as_ref().map(Look::ring)
    }

    fn set_hover(&self, hover: Option<usize>) {
        if self.ivars().hover.replace(hover) != hover {
            self.setNeedsDisplay(true);
        }
    }

    fn slot_under(&self, sender: &ProtocolObject<dyn NSDraggingInfo>) -> Option<usize> {
        let point = self.convertPoint_fromView(sender.draggingLocation(), None);
        let bounds = self.bounds();
        let look = self.ivars().look.borrow();
        let look = look.as_ref()?;
        look.ring().slot_at(
            look.wheel.slots.len(),
            point.x - (bounds.origin.x + bounds.size.width / 2.0),
            point.y - (bounds.origin.y + bounds.size.height / 2.0),
        )
    }

    /// Lights the slot under the drag, and accepts a copy only there: over the hub the pointer shows that a drop
    /// does nothing.
    fn track(&self, sender: &ProtocolObject<dyn NSDraggingInfo>) -> NSDragOperation {
        let slot = self.slot_under(sender);
        self.set_hover(slot);
        if slot.is_some() {
            NSDragOperation::Copy
        } else {
            NSDragOperation::None
        }
    }
}

define_class!(
    // SAFETY: NSPanel allows subclassing; nothing else is added.
    #[unsafe(super(NSPanel))]
    #[thread_kind = MainThreadOnly]
    #[name = "FigWheelPanel"]
    struct WheelPanel;

    impl WheelPanel {
        // The wheel never takes focus, however the window server is asked.
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

impl WheelPanel {
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
        // The shadow is the window's own: anything of the same window under the glass turns it flat grey.
        panel.setHasShadow(true);
        panel.setFloatingPanel(true);
        panel.setHidesOnDeactivate(false);
        panel.setLevel(NSMainMenuWindowLevel + 2);
        panel.setCollectionBehavior(
            NSWindowCollectionBehavior::CanJoinAllSpaces
                | NSWindowCollectionBehavior::Stationary
                | NSWindowCollectionBehavior::FullScreenAuxiliary
                | NSWindowCollectionBehavior::IgnoresCycle,
        );
        panel.setAnimationBehavior(NSWindowAnimationBehavior::None);
        panel
    }
}

// ---------------------------------------------------------------- the watcher

struct Shell {
    mtm: MainThreadMarker,
    me: Weak<Shell>,
    machine: RefCell<Machine>,
    board: Retained<NSPasteboard>,
    /// The drag pasteboard's change count when the button went down, and the one whose files are in `paths`.
    count_at_down: Cell<isize>,
    count_read: Cell<isize>,
    paths: RefCell<Vec<PathBuf>>,
    poll: RefCell<Option<app::Timer>>,
    /// Watches the keys of other apps while files are dragged, when a wheel is bound to a key that is no modifier.
    key_watch: RefCell<Option<KeyMonitor>>,
    /// Counts openings, so a delayed hide that a newer opening has outdated does nothing.
    epoch: Cell<u64>,
    /// Where the wheel was asked for (the pointer, Cocoa coordinates) and whether it is on screen and wanted.
    wanted: Cell<NSPoint>,
    shown: Cell<bool>,
    /// The middle of the wheel on screen, as fitted for the ring it has now (Cocoa coordinates).
    centre: Cell<(f64, f64)>,
    anim: RefCell<Option<app::Timer>>,
    panel: Retained<WheelPanel>,
    /// What the panel holds: one of the plates below, with the slot view inside it, never two at once.
    root: Retained<NSView>,
    /// The Liquid Glass plate, where the system has it, else the vibrancy one.
    glass: Option<Retained<NSGlassEffectView>>,
    effect: Retained<NSVisualEffectView>,
    mounted: Cell<Option<Plate>>,
    view: Retained<WheelView>,
    offer: Offer,
    chosen: Chosen,
}

impl Shell {
    fn handle(&self, mouse: Mouse) {
        match mouse {
            Mouse::Down(_) => {
                self.count_at_down.set(self.board.changeCount());
                self.count_read.set(-1);
                self.paths.borrow_mut().clear();
                self.feed(Event::Down);
            }
            Mouse::Dragged(_) => {
                let count = self.board.changeCount();
                let mut files = false;
                if count != self.count_at_down.get() {
                    if count != self.count_read.get() {
                        self.count_read.set(count);
                        *self.paths.borrow_mut() = file_paths(&self.board)
                            .into_iter()
                            .map(PathBuf::from)
                            .collect();
                    }
                    files = !self.paths.borrow().is_empty();
                }
                self.feed(Event::Drag {
                    files,
                    mods: mods_now(),
                });
            }
            Mouse::Up(_) | Mouse::Lost => self.feed(Event::Up),
        }
    }

    fn feed(&self, event: Event) {
        let (act, tracking, any_key) = {
            let mut machine = self.machine.borrow_mut();
            let act = machine.step(event);
            (act, machine.tracking(), machine.keys.any_key())
        };
        if tracking && self.poll.borrow().is_none() {
            let weak = self.me.clone();
            *self.poll.borrow_mut() = Some(app::every(POLL_SECONDS, move || {
                if let Some(shell) = weak.upgrade() {
                    shell.feed(Event::Mods(mods_now()));
                }
            }));
        } else if !tracking {
            self.poll.borrow_mut().take();
        }
        if tracking && any_key && self.key_watch.borrow().is_none() {
            let weak = self.me.clone();
            *self.key_watch.borrow_mut() = KeyMonitor::start(move |code| {
                if let Some(shell) = weak.upgrade() {
                    shell.feed(Event::Key(code));
                }
            });
        } else if !tracking {
            self.key_watch.borrow_mut().take();
        }
        match act {
            Act::Nothing => {}
            Act::Open(kind) => self.open(kind),
            Act::Hide => self.hide_soon(),
        }
    }

    /// Runs `tick` with the progress (0 to 1) of an animation of `seconds`, every frame, then `done`.
    fn animate(
        &self,
        seconds: f64,
        tick: impl Fn(&Shell, f64) + 'static,
        done: impl Fn(&Shell) + 'static,
    ) {
        let (started, weak) = (Instant::now(), self.me.clone());
        *self.anim.borrow_mut() = Some(app::every(FRAME_SECONDS, move || {
            let Some(shell) = weak.upgrade() else {
                return;
            };
            let t = (started.elapsed().as_secs_f64() / seconds).min(1.0);
            tick(&shell, t);
            if t >= 1.0 {
                shell.anim.borrow_mut().take();
                done(&shell);
            }
        }));
    }

    fn open(&self, kind: Kind) {
        self.epoch.set(self.epoch.get() + 1);
        let paths = self.paths.borrow().clone();
        let wheel = (self.offer)(kind, &paths).map(|mut wheel| {
            wheel.slots.truncate(MAX_SLOTS);
            wheel
        });
        let Some(wheel) = wheel.filter(|w| !w.slots.is_empty()) else {
            self.fade_out();
            return;
        };
        let options = workspace::display_options();
        // The glass cannot be made opaque, so these two settings get the solid plate the design was drawn with.
        let plate = if options.reduce_transparency || options.increase_contrast {
            Plate::Opaque
        } else if self.glass.is_some() {
            Plate::Glass
        } else {
            Plate::Vibrancy
        };
        let look = Look::new(kind, wheel, plate, options.increase_contrast);
        let view = &self.view;
        let target = look.ring();
        let before = view.ring().filter(|_| self.shown.get());
        view.set_look(look);
        self.mount(plate);
        self.anim.borrow_mut().take();
        if !self.shown.get() {
            self.wanted.set(NSEvent::mouseLocation());
        }
        self.shown.set(true);
        let reduced = options.reduce_motion;
        view.registerForDraggedTypes(&NSArray::from_slice(&[
            // SAFETY: extern statics are process-lifetime constants.
            unsafe { NSPasteboardTypeFileURL },
        ]));
        self.centre.set(self.fitted_centre(target));
        match before {
            // A key for another wheel: the ring moves to its new size and the new slots fade in. The panel is
            // as large as the ring is at each moment, so the glass has the shape of the plate throughout.
            Some(from) => {
                view.ivars().reveal.set(0.0);
                view.ivars()
                    .morph
                    .set(if reduced { None } else { Some((from, 0.0)) });
                let start = if reduced { target } else { from };
                self.place(start.side(), start.side());
                self.panel.setAlphaValue(1.0);
                self.animate(
                    if reduced {
                        SWITCH_REDUCED_SECONDS
                    } else {
                        SWITCH_SECONDS
                    },
                    move |shell, t| {
                        let k = style::ease_out(t);
                        let side = match shell.view.ivars().morph.get() {
                            Some((from, _)) => {
                                shell.view.ivars().morph.set(Some((from, k)));
                                from.mix(target, k).side()
                            }
                            None => target.side(),
                        };
                        shell.view.ivars().reveal.set(k);
                        shell.place(side, side);
                    },
                    move |shell| {
                        shell.view.ivars().morph.set(None);
                        shell.place(target.side(), target.side());
                    },
                );
            }
            None => {
                view.ivars().morph.set(None);
                view.ivars().reveal.set(1.0);
                let first = if reduced { 1.0 } else { OPEN_SCALE };
                // The panel itself grows, with the content scaled to it: glass cannot be scaled by drawing.
                self.place(target.side() * first, target.side());
                self.panel.setAlphaValue(0.0);
                self.panel.orderFrontRegardless();
                self.animate(
                    if reduced {
                        OPEN_REDUCED_SECONDS
                    } else {
                        OPEN_SECONDS
                    },
                    move |shell, t| {
                        let k = style::ease_out(t);
                        shell.panel.setAlphaValue(k);
                        let scale = if reduced {
                            1.0
                        } else {
                            style::mix(OPEN_SCALE, 1.0, k)
                        };
                        shell.place(target.side() * scale, target.side());
                    },
                    |_| {},
                );
            }
        }
    }

    /// Puts the plate that `plate` names into the panel, and the slot view into it.
    fn mount(&self, plate: Plate) {
        if self.mounted.replace(Some(plate)) == Some(plate) {
            return;
        }
        self.view.removeFromSuperview();
        for old in [self.glass.as_ref().map(|g| &***g), Some(&**self.effect)]
            .into_iter()
            .flatten()
        {
            old.removeFromSuperview();
        }
        if let Some(glass) = &self.glass {
            glass.setContentView(None);
        }
        match plate {
            Plate::Glass => {
                if let Some(glass) = &self.glass {
                    glass.setContentView(Some(&self.view));
                    self.root.addSubview(glass);
                }
            }
            Plate::Vibrancy => {
                self.effect.addSubview(&self.view);
                self.root.addSubview(&self.effect);
            }
            Plate::Opaque | Plate::Standin => self.root.addSubview(&self.view),
        }
    }

    /// Puts the panel round the wheel's middle, `side` points wide, with the content laid out as `logical` points
    /// wide: the open animation shows it a little smaller by scaling the view's bounds, not by drawing.
    fn place(&self, side: f64, logical: f64) {
        let (cx, cy) = self.centre.get();
        self.panel.setFrame_display(
            NSRect::new(
                NSPoint::new(cx - side / 2.0, cy - side / 2.0),
                NSSize::new(side, side),
            ),
            true,
        );
        let content = NSRect::new(NSPoint::ZERO, NSSize::new(side, side));
        match self.mounted.get() {
            Some(Plate::Glass) => {
                if let Some(glass) = &self.glass {
                    glass.setFrame(content);
                    glass.setCornerRadius(side / 2.0);
                }
            }
            Some(Plate::Vibrancy) => {
                self.effect.setFrame(content);
                if let Some(layer) = self.effect.layer() {
                    layer.setCornerRadius(side / 2.0);
                    layer.setMasksToBounds(true);
                }
            }
            _ => {}
        }
        self.view.setFrame(content);
        self.view.setBoundsSize(NSSize::new(logical, logical));
        self.view.setNeedsDisplay(true);
        // The window's shadow follows what is drawn, which changed.
        self.panel.invalidateShadow();
    }

    /// The middle of the wheel: where it was asked for, moved the least that keeps the plate inside the visible
    /// part of the screen the pointer is on.
    fn fitted_centre(&self, ring: Ring) -> (f64, f64) {
        let pointer = self.wanted.get();
        let screens = NSScreen::screens(self.mtm);
        let screen = screens
            .iter()
            .find(|s| {
                let f = s.frame();
                pointer.x >= f.origin.x
                    && pointer.x < f.origin.x + f.size.width
                    && pointer.y >= f.origin.y
                    && pointer.y < f.origin.y + f.size.height
            })
            .or_else(|| NSScreen::mainScreen(self.mtm));
        match screen {
            Some(screen) => {
                let a = screen.visibleFrame();
                fit_centre(
                    (pointer.x, pointer.y),
                    2.0 * (ring.plate() + EDGE_MARGIN),
                    (a.origin.x, a.origin.y, a.size.width, a.size.height),
                )
            }
            None => (pointer.x, pointer.y),
        }
    }

    /// Takes the panel away at once.
    fn hide_now(&self) {
        self.anim.borrow_mut().take();
        self.shown.set(false);
        self.view.unregisterDraggedTypes();
        self.view.set_hover(None);
        self.view.ivars().morph.set(None);
        self.view.ivars().reveal.set(1.0);
        self.panel.orderOut(None);
        self.panel.setAlphaValue(1.0);
    }

    /// Fades the wheel out where it is, the slot that is lit staying lit.
    fn fade_out(&self) {
        if !self.panel.isVisible() || !self.shown.replace(false) {
            return;
        }
        self.view.unregisterDraggedTypes();
        let from = self.panel.alphaValue();
        self.animate(
            CLOSE_SECONDS,
            move |shell, t| shell.panel.setAlphaValue(from * (1.0 - t)),
            |shell| shell.hide_now(),
        );
    }

    fn hide_soon(&self) {
        let (weak, epoch) = (self.me.clone(), self.epoch.get());
        app::after(HIDE_DELAY_SECONDS, move || {
            if let Some(shell) = weak.upgrade()
                && shell.epoch.get() == epoch
            {
                shell.fade_out();
            }
        });
    }

    /// Files were dropped on slot `index`: the wheel fades away and the owner hears of it once the drop is over.
    fn dropped(&self, index: usize, paths: Vec<PathBuf>) {
        let choice = self.view.ivars().look.borrow().as_ref().and_then(|look| {
            look.wheel
                .slots
                .get(index)
                .map(|slot| (look.kind, slot.id.clone()))
        });
        self.epoch.set(self.epoch.get() + 1);
        self.fade_out();
        if let Some((kind, id)) = choice {
            let weak = self.me.clone();
            // Not inside the drag callback, which the system is still waiting on.
            app::after(0.0, move || {
                if let Some(shell) = weak.upgrade() {
                    (shell.chosen)(kind, &id, paths);
                }
            });
        }
    }
}

impl Drop for Shell {
    fn drop(&mut self) {
        self.poll.borrow_mut().take();
        self.key_watch.borrow_mut().take();
        self.anim.borrow_mut().take();
        self.panel.orderOut(None);
        self.panel.close();
    }
}

/// Watches file drags and owns the wheel panel. Dropping it removes the monitor and the panel.
pub struct DragWheel {
    shell: Rc<Shell>,
    _monitor: MouseMonitor,
}

impl DragWheel {
    /// `keys` says which modifier opens which wheel. `offer` runs on the main thread each time a wheel is about
    /// to open or switch; it gets the kind and the
    /// dragged paths and returns the wheel, or None to show nothing for that kind.
    /// `chosen` runs on the main thread after the files were dropped on a slot.
    /// None when the system refuses a monitor.
    pub fn start(
        mtm: MainThreadMarker,
        keys: Keys,
        offer: impl Fn(Kind, &[PathBuf]) -> Option<Wheel> + 'static,
        chosen: impl Fn(Kind, &str, Vec<PathBuf>) + 'static,
    ) -> Option<Self> {
        // SAFETY: extern statics are process-lifetime constants.
        let board = NSPasteboard::pasteboardWithName(unsafe { NSPasteboardNameDrag });
        let panel = WheelPanel::new(mtm);
        let view = WheelView::new(mtm);
        let root = NSView::initWithFrame(mtm.alloc(), NSRect::ZERO);
        panel.setContentView(Some(&root));
        // The glass the way the notch has it: the regular style with its dark tint. Before macOS 26 the class is
        // missing and a dark vibrancy material stands in.
        let glass = AnyClass::get(c"NSGlassEffectView")
            .and_then(|_| glass_tint(Material::Glass))
            .map(|(look, tint)| {
                let glass = NSGlassEffectView::initWithFrame(mtm.alloc(), NSRect::ZERO);
                glass.setStyle(look);
                glass.setTintColor(Some(&tint));
                glass
            });
        let effect = NSVisualEffectView::initWithFrame(mtm.alloc(), NSRect::ZERO);
        effect.setMaterial(vibrancy_of(Material::Glass));
        effect.setBlendingMode(NSVisualEffectBlendingMode::BehindWindow);
        effect.setState(NSVisualEffectState::Active);
        effect.setWantsLayer(true);
        let shell = Rc::new_cyclic(|me| Shell {
            mtm,
            me: me.clone(),
            machine: RefCell::new(Machine {
                keys,
                ..Machine::default()
            }),
            board,
            count_at_down: Cell::new(0),
            count_read: Cell::new(-1),
            paths: RefCell::new(Vec::new()),
            poll: RefCell::new(None),
            key_watch: RefCell::new(None),
            epoch: Cell::new(0),
            wanted: Cell::new(NSPoint::ZERO),
            shown: Cell::new(false),
            centre: Cell::new((0.0, 0.0)),
            anim: RefCell::new(None),
            panel,
            root,
            glass,
            effect,
            mounted: Cell::new(None),
            view,
            offer: Box::new(offer),
            chosen: Box::new(chosen),
        });
        let weak = Rc::downgrade(&shell);
        *shell.view.ivars().on_drop.borrow_mut() = Some(Rc::new(move |index, paths| {
            if let Some(shell) = weak.upgrade() {
                shell.dropped(index, paths);
            }
        }));
        let weak = Rc::downgrade(&shell);
        let monitor = MouseMonitor::start(mtm, move |mouse| {
            if let Some(shell) = weak.upgrade() {
                shell.handle(mouse);
            }
        })?;
        Some(Self {
            shell,
            _monitor: monitor,
        })
    }

    /// Binds the wheels to other keys from the next tap on.
    pub fn set_keys(&self, keys: Keys) {
        self.shell.machine.borrow_mut().keys = keys;
    }

    /// Whether the wheel is on screen, for checks.
    pub fn is_open(&self) -> bool {
        self.shell.panel.isVisible()
    }
}

/// Draws a wheel without a screen, for the render example and QA. `hover` lights one slot. Two pixels per point,
/// transparent outside the plate. Liquid Glass is drawn by the window server and cannot be drawn offscreen, so the
/// plate is a dark translucent disc standing in for it, as the volume indicator's snapshots do for their glass
/// styles: what a snapshot shows is the layout, the sizes and the ink, not the glass.
pub fn snapshot_png(
    mtm: MainThreadMarker,
    kind: Kind,
    wheel: &Wheel,
    hover: Option<usize>,
) -> Option<Vec<u8>> {
    snapshot(mtm, kind, wheel, hover, Plate::Standin)
}

/// The same for the solid dark plate that Increase Contrast and Reduce Transparency get, which is drawn by the
/// view itself and so is exact.
pub fn snapshot_opaque_png(
    mtm: MainThreadMarker,
    kind: Kind,
    wheel: &Wheel,
    hover: Option<usize>,
) -> Option<Vec<u8>> {
    snapshot(mtm, kind, wheel, hover, Plate::Opaque)
}

fn snapshot(
    mtm: MainThreadMarker,
    kind: Kind,
    wheel: &Wheel,
    hover: Option<usize>,
    plate: Plate,
) -> Option<Vec<u8>> {
    let view = WheelView::new(mtm);
    let look = Look::new(kind, wheel.clone(), plate, plate == Plate::Opaque);
    let side = look.ring().side();
    view.setFrameSize(NSSize::new(side, side));
    view.set_look(look);
    view.ivars().hover.set(hover);
    let bounds = view.bounds();
    // SAFETY: a plain RGBA bitmap with valid dimensions.
    let rep = unsafe {
        NSBitmapImageRep::initWithBitmapDataPlanes_pixelsWide_pixelsHigh_bitsPerSample_samplesPerPixel_hasAlpha_isPlanar_colorSpaceName_bytesPerRow_bitsPerPixel(
            NSBitmapImageRep::alloc(),
            std::ptr::null_mut(),
            (bounds.size.width * 2.0).round() as isize,
            (bounds.size.height * 2.0).round() as isize,
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

#[cfg(test)]
mod tests {
    use super::*;

    const COUNTS: [usize; 5] = [1, 4, 6, 9, 12];

    #[test]
    fn every_slot_is_hit_at_its_middle_angle() {
        for compact in [true, false] {
            for count in COUNTS {
                let ring = Ring::new(count, compact);
                for index in 0..count {
                    let (dx, dy) = slot_centre(count, index, ring);
                    assert_eq!(
                        ring.slot_at(count, dx, dy),
                        Some(index),
                        "{count}:{index}:{compact}"
                    );
                }
            }
        }
    }

    #[test]
    fn slot_zero_is_at_the_top_and_slots_run_clockwise() {
        assert_eq!(slot_at(4, 0.0, 100.0, 50.0, 150.0), Some(0));
        assert_eq!(slot_at(4, 100.0, 0.0, 50.0, 150.0), Some(1));
        assert_eq!(slot_at(4, 0.0, -100.0, 50.0, 150.0), Some(2));
        assert_eq!(slot_at(4, -100.0, 0.0, 50.0, 150.0), Some(3));
        // Just left of the top belongs to the last slot, just right of it to the first.
        assert_eq!(slot_at(6, -5.0, 100.0, 50.0, 150.0), Some(0));
        assert_eq!(slot_at(6, 5.0, 100.0, 50.0, 150.0), Some(0));
        assert_eq!(slot_at(6, 100.0, 60.0, 50.0, 150.0), Some(1));
    }

    #[test]
    fn the_hub_and_beyond_the_outer_radius_are_no_slot() {
        for count in COUNTS {
            assert_eq!(slot_at(count, 0.0, 0.0, 50.0, 150.0), None);
            assert_eq!(slot_at(count, 30.0, 30.0, 50.0, 150.0), None);
            assert_eq!(slot_at(count, 0.0, 151.0, 50.0, 150.0), None);
            assert_eq!(slot_at(count, -110.0, -110.0, 50.0, 150.0), None);
            assert!(slot_at(count, 0.0, 50.0, 50.0, 150.0).is_some());
            assert!(slot_at(count, 0.0, 150.0, 50.0, 150.0).is_some());
        }
        assert_eq!(slot_at(0, 0.0, 100.0, 50.0, 150.0), None);
    }

    #[test]
    fn the_ring_ends_in_the_middle_of_the_channel_and_at_the_plate() {
        let ring = Ring::new(6, true);
        assert_eq!(ring.hub(), 52.0);
        assert_eq!(ring.plate(), 122.0);
        // The hub refuses a drop up to the middle of its channel; the rim still belongs to a slot.
        assert_eq!(ring.slot_at(6, 0.0, 53.9), None);
        assert_eq!(ring.slot_at(6, 0.0, 54.1), Some(0));
        assert_eq!(ring.slot_at(6, 0.0, 121.9), Some(0));
        assert_eq!(ring.slot_at(6, 0.0, 122.1), None);
    }

    #[test]
    fn the_sizes_follow_the_table_of_the_design() {
        let plate = |count, compact| 2.0 * Ring::new(count, compact).plate();
        assert_eq!(plate(1, true), 244.0);
        assert_eq!(plate(6, true), 244.0);
        assert_eq!(plate(9, true), 252.0);
        assert_eq!(plate(12, true), 312.0);
        assert_eq!(plate(6, false), 308.0);
        assert_eq!(plate(7, false), 316.0);
        assert_eq!(plate(8, false), 344.0);
        assert_eq!(plate(12, false), 460.0);
        // Out of range counts are clamped, not a panic.
        assert_eq!(Ring::new(0, true), Ring::new(1, true));
        assert_eq!(Ring::new(40, false), Ring::new(12, false));
        let ring = Ring::new(6, false);
        assert_eq!(ring.side(), 2.0 * ring.plate());
    }

    #[test]
    fn a_point_on_a_boundary_has_one_answer_and_the_sides_differ() {
        for count in [4_usize, 6, 9, 12] {
            let width = TAU / count as f64;
            for index in 0..count {
                // The line between this slot and the next one clockwise.
                let angle = width * (index as f64 + 0.5);
                let at = |a: f64| slot_at(count, 100.0 * a.sin(), 100.0 * a.cos(), 50.0, 150.0);
                assert!(at(angle).is_some());
                assert_eq!(at(angle - 1e-6), Some(index), "{count}:{index} before");
                assert_eq!(
                    at(angle + 1e-6),
                    Some((index + 1) % count),
                    "{count}:{index} after"
                );
            }
        }
        // A lone slot owns the whole ring.
        assert_eq!(slot_at(1, -100.0, 0.0, 50.0, 150.0), Some(0));
        assert_eq!(slot_at(1, 0.0, -100.0, 50.0, 150.0), Some(0));
    }

    #[test]
    fn the_wheel_is_pushed_back_inside_the_screen() {
        let area = (0.0, 0.0, 1000.0, 800.0);
        assert_eq!(fit_centre((500.0, 400.0), 100.0, area), (500.0, 400.0));
        assert_eq!(fit_centre((10.0, 790.0), 100.0, area), (50.0, 750.0));
        assert_eq!(fit_centre((990.0, 10.0), 100.0, area), (950.0, 50.0));
        // A second screen to the left of the first.
        let left = (-1000.0, 100.0, 1000.0, 700.0);
        assert_eq!(fit_centre((-5.0, 300.0), 100.0, left), (-50.0, 300.0));
        // Smaller than the wheel: centred.
        assert_eq!(
            fit_centre((50.0, 50.0), 100.0, (0.0, 0.0, 60.0, 60.0)),
            (30.0, 30.0)
        );
    }

    fn only(shift: bool, command: bool, control: bool) -> Mods {
        Mods {
            shift,
            command,
            control,
            option: false,
        }
    }
    const NONE: Mods = Mods {
        shift: false,
        command: false,
        control: false,
        option: false,
    };

    /// A machine with the button down and files being dragged, no key held.
    fn dragging() -> Machine {
        let mut m = Machine::default();
        assert_eq!(m.step(Event::Down), Act::Nothing);
        assert_eq!(
            m.step(Event::Drag {
                files: true,
                mods: NONE
            }),
            Act::Nothing
        );
        m
    }

    #[test]
    fn nothing_opens_without_a_file_drag() {
        let mut m = Machine::default();
        // No button down at all.
        assert_eq!(m.step(Event::Mods(only(true, false, false))), Act::Nothing);
        assert_eq!(
            m.step(Event::Drag {
                files: true,
                mods: NONE
            }),
            Act::Nothing
        );
        assert!(!m.tracking());
        // Button down, dragging something that is not files.
        m.step(Event::Down);
        for _ in 0..3 {
            assert_eq!(
                m.step(Event::Drag {
                    files: false,
                    mods: only(true, false, false)
                }),
                Act::Nothing
            );
        }
        assert_eq!(m.step(Event::Mods(only(false, true, false))), Act::Nothing);
        assert!(!m.tracking());
    }

    #[test]
    fn each_of_the_three_keys_opens_its_wheel() {
        for (mods, kind) in [
            (only(true, false, false), Kind::Formats),
            (only(false, true, false), Kind::Tools),
            (only(false, false, true), Kind::Actions),
        ] {
            let mut m = dragging();
            assert!(m.tracking());
            assert_eq!(m.step(Event::Mods(mods)), Act::Open(kind));
            // Holding it changes nothing, and neither does releasing it.
            assert_eq!(m.step(Event::Mods(mods)), Act::Nothing);
            assert_eq!(m.step(Event::Mods(NONE)), Act::Nothing);
            assert_eq!(
                m.step(Event::Drag {
                    files: true,
                    mods: NONE
                }),
                Act::Nothing
            );
            // The wheel closes with the button.
            assert_eq!(m.step(Event::Up), Act::Hide);
        }
    }

    #[test]
    fn a_key_seen_by_a_drag_event_opens_too() {
        let mut m = dragging();
        let act = m.step(Event::Drag {
            files: true,
            mods: only(false, true, false),
        });
        assert_eq!(act, Act::Open(Kind::Tools));
    }

    #[test]
    fn another_key_switches_and_the_same_key_does_not_repeat() {
        let mut m = dragging();
        assert_eq!(
            m.step(Event::Mods(only(true, false, false))),
            Act::Open(Kind::Formats)
        );
        assert_eq!(m.step(Event::Mods(NONE)), Act::Nothing);
        assert_eq!(
            m.step(Event::Mods(only(false, false, true))),
            Act::Open(Kind::Actions)
        );
        assert_eq!(m.step(Event::Mods(NONE)), Act::Nothing);
        // The key of the wheel that is already open.
        assert_eq!(m.step(Event::Mods(only(false, false, true))), Act::Nothing);
        assert_eq!(m.step(Event::Mods(NONE)), Act::Nothing);
        assert_eq!(
            m.step(Event::Mods(only(false, true, false))),
            Act::Open(Kind::Tools)
        );
    }

    #[test]
    fn a_second_key_tapped_while_the_first_is_held_switches() {
        let mut m = dragging();
        assert_eq!(
            m.step(Event::Mods(only(true, false, false))),
            Act::Open(Kind::Formats)
        );
        assert_eq!(
            m.step(Event::Mods(only(true, true, false))),
            Act::Open(Kind::Tools)
        );
        // Letting go of Command leaves Shift held, which is no new press.
        assert_eq!(m.step(Event::Mods(only(true, false, false))), Act::Nothing);
    }

    #[test]
    fn keys_held_before_the_drag_began_do_not_count() {
        let mut m = Machine::default();
        m.step(Event::Down);
        // Shift is already down when the pasteboard first carries files.
        let held = only(true, false, false);
        assert_eq!(
            m.step(Event::Drag {
                files: true,
                mods: held
            }),
            Act::Nothing
        );
        assert_eq!(m.step(Event::Mods(held)), Act::Nothing);
        // Letting go and pressing again is a tap.
        assert_eq!(m.step(Event::Mods(NONE)), Act::Nothing);
        assert_eq!(m.step(Event::Mods(held)), Act::Open(Kind::Formats));
    }

    #[test]
    fn two_keys_at_once_are_no_tap() {
        let mut m = dragging();
        let option = Mods {
            option: true,
            ..only(true, false, false)
        };
        assert_eq!(m.step(Event::Mods(option)), Act::Nothing);
        assert_eq!(m.step(Event::Mods(NONE)), Act::Nothing);
        assert_eq!(m.step(Event::Mods(only(true, true, false))), Act::Nothing);
        assert_eq!(m.step(Event::Up), Act::Nothing, "nothing was open");
    }

    fn option_only() -> Mods {
        Mods {
            option: true,
            ..NONE
        }
    }

    #[test]
    fn option_is_a_plain_copy_drag_unless_a_wheel_is_bound_to_it() {
        let mut m = dragging();
        assert_eq!(m.step(Event::Mods(option_only())), Act::Nothing);
        assert_eq!(m.step(Event::Up), Act::Nothing, "nothing was open");
    }

    #[test]
    fn option_bound_to_a_wheel_opens_it() {
        let mut m = Machine {
            keys: Keys {
                formats: Some(Trigger::Modifier(Modifier::Option)),
                tools: Some(Trigger::Modifier(Modifier::Command)),
                actions: None,
            },
            ..Machine::default()
        };
        m.step(Event::Down);
        m.step(Event::Drag {
            files: true,
            mods: NONE,
        });
        assert_eq!(m.step(Event::Mods(option_only())), Act::Open(Kind::Formats));
        assert_eq!(m.step(Event::Mods(NONE)), Act::Nothing);
        // Shift no longer belongs to any wheel, and neither does Control, which was switched off.
        assert_eq!(m.step(Event::Mods(only(true, false, false))), Act::Nothing);
        assert_eq!(m.step(Event::Mods(NONE)), Act::Nothing);
        assert_eq!(m.step(Event::Mods(only(false, false, true))), Act::Nothing);
        assert_eq!(m.step(Event::Mods(NONE)), Act::Nothing);
        assert_eq!(
            m.step(Event::Mods(only(false, true, false))),
            Act::Open(Kind::Tools)
        );
    }

    #[test]
    fn any_other_key_opens_its_wheel_while_files_are_dragged() {
        let f19 = 80;
        let mut m = Machine {
            keys: Keys {
                formats: Some(Trigger::Key(f19)),
                ..Keys::default()
            },
            ..Machine::default()
        };
        assert!(m.keys.any_key() && !Keys::default().any_key());
        // Not without a file drag: a key pressed at any other time is the app's own business.
        assert_eq!(m.step(Event::Key(f19)), Act::Nothing);
        m.step(Event::Down);
        m.step(Event::Drag {
            files: true,
            mods: NONE,
        });
        assert_eq!(m.step(Event::Key(f19)), Act::Open(Kind::Formats));
        // Pressed again it stays; a key no wheel has does nothing; the modifiers still work alongside.
        assert_eq!(m.step(Event::Key(f19)), Act::Nothing);
        assert_eq!(m.step(Event::Key(0)), Act::Nothing);
        assert_eq!(
            m.step(Event::Mods(only(false, true, false))),
            Act::Open(Kind::Tools)
        );
        assert_eq!(m.step(Event::Key(f19)), Act::Open(Kind::Formats));
        assert_eq!(m.step(Event::Up), Act::Hide);
        assert_eq!(m.step(Event::Key(f19)), Act::Nothing);
    }

    #[test]
    fn a_wheel_set_to_none_never_opens() {
        let mut m = Machine {
            keys: Keys {
                formats: None,
                tools: None,
                actions: None,
            },
            ..Machine::default()
        };
        m.step(Event::Down);
        m.step(Event::Drag {
            files: true,
            mods: NONE,
        });
        for mods in [
            only(true, false, false),
            only(false, true, false),
            only(false, false, true),
            option_only(),
        ] {
            assert_eq!(m.step(Event::Mods(mods)), Act::Nothing);
            assert_eq!(m.step(Event::Mods(NONE)), Act::Nothing);
        }
        assert_eq!(m.step(Event::Up), Act::Nothing);
    }

    #[test]
    fn keys_survive_the_button_going_up_and_down() {
        let keys = Keys {
            formats: Some(Trigger::Modifier(Modifier::Control)),
            tools: None,
            actions: Some(Trigger::Modifier(Modifier::Shift)),
        };
        let mut m = Machine {
            keys,
            ..Machine::default()
        };
        for _ in 0..2 {
            m.step(Event::Down);
            m.step(Event::Drag {
                files: true,
                mods: NONE,
            });
            assert_eq!(
                m.step(Event::Mods(only(true, false, false))),
                Act::Open(Kind::Actions)
            );
            assert_eq!(m.step(Event::Up), Act::Hide);
        }
        assert_eq!(m.keys, keys);
    }

    #[test]
    fn the_button_going_up_or_down_again_starts_over() {
        let mut m = dragging();
        assert_eq!(
            m.step(Event::Mods(only(true, false, false))),
            Act::Open(Kind::Formats)
        );
        assert_eq!(m.step(Event::Up), Act::Hide);
        assert!(!m.tracking());
        // A stray drag event after the release does nothing.
        assert_eq!(
            m.step(Event::Drag {
                files: true,
                mods: only(false, true, false)
            }),
            Act::Nothing
        );
        // A button that goes down while a wheel is still open (a missed release) hides it.
        let mut m = dragging();
        m.step(Event::Mods(only(false, true, false)));
        assert_eq!(m.step(Event::Down), Act::Hide);
        assert!(!m.tracking());
        // The next drag starts clean: the same key opens the same wheel again.
        m.step(Event::Drag {
            files: true,
            mods: NONE,
        });
        assert_eq!(
            m.step(Event::Mods(only(false, true, false))),
            Act::Open(Kind::Tools)
        );
    }
}
