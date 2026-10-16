//! The stage of the clipboard mascot: a click-through panel over the notch's screen, on which a small pixel
//! character comes out of the notch to fetch what was copied and sees files off as they leave the shelf.
//! It only draws what it is handed. Where everything is at each moment is worked out by the app
//! (`app-ui/src/features/mascot`), so the story is plain data that tests can look at.

use crate::{app, panel::FigPanel};
use objc2::{
    AnyThread, DefinedClass, MainThreadMarker, MainThreadOnly, Message, define_class, msg_send,
    rc::Retained, sel,
};
use objc2_app_kit::{
    NSBezierPath, NSBitmapImageFileType, NSBitmapImageRep, NSColor, NSCompositingOperation,
    NSEvent, NSGraphicsContext, NSImage, NSImageInterpolation, NSMainMenuWindowLevel, NSScreen,
    NSView, NSWindingRule, NSWorkspace,
};
use objc2_foundation::{
    NSData, NSDictionary, NSObject, NSObjectProtocol, NSPoint, NSRect, NSRunLoop,
    NSRunLoopCommonModes, NSSize, NSString,
};
use objc2_quartz_core::{CACurrentMediaTime, CADisplayLink};
use std::{
    cell::{Cell, RefCell},
    collections::HashMap,
    f64::consts::TAU,
    rc::{Rc, Weak},
};

/// Where the pointer is, in AppKit screen coordinates.
pub fn pointer() -> NSPoint {
    NSEvent::mouseLocation()
}

/// The frame of the screen that holds `point`, or of the main screen when none does.
pub fn screen_at(mtm: MainThreadMarker, point: NSPoint) -> Option<NSRect> {
    let screens = NSScreen::screens(mtm);
    let inside = |r: NSRect| {
        point.x >= r.origin.x
            && point.x < r.origin.x + r.size.width
            && point.y >= r.origin.y
            && point.y < r.origin.y + r.size.height
    };
    screens
        .iter()
        .map(|s| s.frame())
        .find(|r| inside(*r))
        .or_else(|| screens.firstObject().map(|s| s.frame()))
}

/// A row of square frames of pixel art.
pub struct Sheet {
    image: Retained<NSImage>,
    cell: f64,
    count: usize,
}

impl Sheet {
    /// `None` when the PNG cannot be read or is not a row of square frames.
    pub fn from_png(png: &[u8]) -> Option<Sheet> {
        let rep = NSBitmapImageRep::imageRepWithData(&NSData::with_bytes(png))?;
        let (w, h) = (rep.pixelsWide() as f64, rep.pixelsHigh() as f64);
        if h < 1.0 || w < h {
            return None;
        }
        // One point per pixel, whatever resolution the file claims; the drawing scales it up.
        rep.setSize(NSSize::new(w, h));
        let image = NSImage::initWithSize(NSImage::alloc(), NSSize::new(w, h));
        image.addRepresentation(&rep);
        Some(Sheet {
            image,
            cell: h,
            count: (w / h) as usize,
        })
    }

    /// The side of a frame, in pixels.
    pub fn cell(&self) -> f64 {
        self.cell
    }

    pub fn frames(&self) -> usize {
        self.count
    }
}

/// How a character reaches for things.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Ray {
    Silk,
    Tongue,
    /// A cone of light, `width` wide at its far end.
    Beam,
    Wisp,
    Magic,
    Bolt,
}

/// What travels to the notch: a stand-in for what was copied, never its content.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Chip {
    Text,
    Picture,
    /// The icon of this file.
    File(String),
}

/// What protects a file on its way out of the shelf.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Guard {
    Box,
    Bubble,
    /// A dashed ring that turns.
    Ring,
    /// Light from above.
    Cone,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Shape {
    /// A frame of the sheet centred on `at`, `scale` points per pixel.
    Sprite {
        frame: usize,
        at: NSPoint,
        scale: f64,
    },
    Ray {
        kind: Ray,
        from: NSPoint,
        to: NSPoint,
        width: f64,
        alpha: f64,
    },
    /// Where a ray sticks: a splat of silk, the end of a tongue, a spark.
    Spark {
        kind: Ray,
        at: NSPoint,
    },
    Chip {
        chip: Chip,
        at: NSPoint,
        scale: f64,
        alpha: f64,
    },
    Guard {
        kind: Guard,
        at: NSPoint,
        size: f64,
        turn: f64,
    },
    Tear {
        at: NSPoint,
        alpha: f64,
    },
}

/// One moment of the stage, in screen coordinates.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Frame {
    /// Drawn in order, back to front.
    pub shapes: Vec<Shape>,
    /// Nothing is drawn here: the notch, out of which the character comes and into which things go.
    pub hole: Option<NSRect>,
}

fn rgb(hex: u32, alpha: f64) -> Retained<NSColor> {
    let c = |shift: u32| f64::from((hex >> shift) & 0xff) / 255.0;
    NSColor::colorWithSRGBRed_green_blue_alpha(c(16), c(8), c(0), alpha)
}

fn rect(x: f64, y: f64, w: f64, h: f64) -> NSRect {
    NSRect::new(NSPoint::new(x, y), NSSize::new(w, h))
}

fn fill(r: NSRect, color: &NSColor) {
    color.setFill();
    NSBezierPath::fillRect(r);
}

/// A block of the art: the cell its bottom left corner sits in, and how many cells it is across and up.
/// Everything an effect draws is made of these, so a ray or a guard is the same square pixels the character
/// is drawn from, and never a line or an oval at some other size.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Bar {
    x: i32,
    y: i32,
    w: i32,
    h: i32,
}

impl Bar {
    /// A square of `n` by `n` pixels centred on the cell `(x, y)`; `n` is odd.
    fn square(x: i32, y: i32, n: i32) -> Self {
        Self {
            x: x - n / 2,
            y: y - n / 2,
            w: n,
            h: n,
        }
    }

    /// A block of `w` by `h` pixels centred on the cell `(x, y)`; both are odd.
    fn block(x: i32, y: i32, w: i32, h: i32) -> Self {
        Self {
            x: x - w / 2,
            y: y - h / 2,
            w,
            h,
        }
    }
}

/// The side of one pixel of the art the character is drawn from, in points. The effects are drawn on this grid
/// too. The sprite itself is placed on the device's own pixels, a finer grid, so it moves smoothly between them
/// while its art stays hard-edged.
fn art_pixel(frame: &Frame) -> f64 {
    frame
        .shapes
        .iter()
        .find_map(|s| match s {
            Shape::Sprite { scale, .. } => Some(*scale),
            _ => None,
        })
        .unwrap_or(2.0)
}

/// How many pixels of the art a width in points comes to: whole ones, and odd, so a block centred on a cell
/// covers the same either side of it.
fn cells(points: f64, px: f64) -> i32 {
    ((points / px).round().max(1.0) as i32) | 1
}

/// The cell of the art a point falls in.
fn cell_of(p: NSPoint, px: f64) -> (i32, i32) {
    ((p.x / px).floor() as i32, (p.y / px).floor() as i32)
}

/// Fills blocks of the art in one colour, as one path.
fn fill_cells(bars: &[Bar], color: &NSColor, px: f64) {
    let path = NSBezierPath::bezierPath();
    for b in bars {
        path.appendBezierPathWithRect(rect(
            f64::from(b.x) * px,
            f64::from(b.y) * px,
            f64::from(b.w) * px,
            f64::from(b.h) * px,
        ));
    }
    color.setFill();
    path.fill();
}

/// The cells a path of points passes through, one step every pixel of the art: the line of square pixels a ray
/// is drawn as, with no gaps in it and no half pixels.
fn line_cells(points: &[NSPoint], px: f64) -> Vec<(i32, i32)> {
    let mut out: Vec<(i32, i32)> = Vec::new();
    let mut carry = 0.0;
    for pair in points.windows(2) {
        let (a, b) = (pair[0], pair[1]);
        let len = (b.x - a.x).hypot(b.y - a.y);
        if len < 1e-9 {
            continue;
        }
        let (dx, dy) = ((b.x - a.x) / len, (b.y - a.y) / len);
        let mut at = carry;
        while at <= len {
            let cell = cell_of(NSPoint::new(a.x + dx * at, a.y + dy * at), px);
            if out.last() != Some(&cell) {
                out.push(cell);
            }
            at += px;
        }
        carry = at - len;
    }
    if let Some(end) = points.last() {
        let cell = cell_of(*end, px);
        if out.last() != Some(&cell) {
            out.push(cell);
        }
    }
    out
}

/// A ray: a line of blocks, `thick` pixels wide.
fn line(points: &[NSPoint], thick: i32, color: &NSColor, px: f64) {
    let bars: Vec<Bar> = line_cells(points, px)
        .into_iter()
        .map(|(x, y)| Bar::square(x, y, thick))
        .collect();
    fill_cells(&bars, color, px);
}

/// The rows of a filled circle `r` pixels across, centred on the cell `(x, y)`.
fn disc_cells(x: i32, y: i32, r: i32) -> Vec<Bar> {
    let r = r.max(0);
    (-r..=r)
        .map(|dy| {
            let span = f64::from(r * r - dy * dy).sqrt().floor() as i32;
            Bar {
                x: x - span,
                y: y + dy,
                w: 2 * span + 1,
                h: 1,
            }
        })
        .collect()
}

/// The pixels of a circle `r` across, `thick` pixels deep: what pixel art draws where a smooth oval would be.
/// With a `phase`, in turns, it comes out dotted and the dots march round as the phase moves.
fn ring_cells(x: i32, y: i32, r: i32, thick: i32, phase: Option<f64>) -> Vec<Bar> {
    let r = r.max(0);
    let mut out = Vec::new();
    for dy in -r..=r {
        let span = f64::from(r * r - dy * dy).sqrt().floor() as i32;
        for deep in 0..thick {
            let dx = span - deep;
            if dx < 0 {
                continue;
            }
            for side in [1, -1] {
                if (dx == 0 && side < 0) || phase.is_some_and(|p| !dotted(dx * side, dy, r, p)) {
                    continue;
                }
                out.push(Bar {
                    x: x + dx * side,
                    y: y + dy,
                    w: 1,
                    h: 1,
                });
            }
        }
    }
    out
}

/// Whether one pixel of a dotted ring is drawn: dashes of about a pixel and a half round the circle.
fn dotted(dx: i32, dy: i32, r: i32, phase: f64) -> bool {
    let turns = f64::from(dy).atan2(f64::from(dx)) / TAU + phase;
    ((turns * TAU * f64::from(r) / 1.5) as i32).rem_euclid(2) == 0
}

/// A shape that widens from `near` to `far` pixels on its way to `to`: a cone of light, in whole pixels.
fn cone_cells(from: NSPoint, to: NSPoint, near: i32, far: i32, px: f64) -> Vec<Bar> {
    let (dx, dy) = (to.x - from.x, to.y - from.y);
    let steps = (dx.hypot(dy) / px).round().max(1.0) as i32;
    (0..=steps)
        .map(|i| {
            let f = f64::from(i) / f64::from(steps);
            let (x, y) = cell_of(NSPoint::new(from.x + dx * f, from.y + dy * f), px);
            let width = (f64::from(near) + f64::from(far - near) * f).round() as i32;
            Bar::square(x, y, width.max(1) | 1)
        })
        .collect()
}

/// The points a ray passes through: straight, or bent into waves or a zigzag for the styles that have them.
pub fn ray_points(kind: Ray, from: NSPoint, to: NSPoint) -> Vec<NSPoint> {
    let (dx, dy) = (to.x - from.x, to.y - from.y);
    let len = dx.hypot(dy);
    if len < 1.0 || !matches!(kind, Ray::Wisp | Ray::Bolt) {
        return vec![from, to];
    }
    let (nx, ny) = (-dy / len, dx / len);
    let at =
        |t: f64, off: f64| NSPoint::new(from.x + dx * t + nx * off, from.y + dy * t + ny * off);
    if kind == Ray::Wisp {
        let n = 32;
        (0..=n)
            .map(|i| {
                let t = f64::from(i) / f64::from(n);
                // Calm at both ends, wavy in between.
                let calm = (t * (1.0 - t) * 6.0).min(1.0);
                at(t, 4.0 * calm * (t * len / 22.0 * TAU).sin())
            })
            .collect()
    } else {
        let n = ((len / 16.0).round() as i32).max(3);
        (0..=n)
            .map(|i| {
                let side = if i == 0 || i == n {
                    0.0
                } else if i % 2 == 0 {
                    -5.0
                } else {
                    5.0
                };
                at(f64::from(i) / f64::from(n), side)
            })
            .collect()
    }
}

fn draw_ray(kind: Ray, from: NSPoint, to: NSPoint, width: f64, alpha: f64, px: f64) {
    let points = ray_points(kind, from, to);
    match kind {
        Ray::Silk => {
            line(&points, cells(3.0, px), &rgb(0x1D2B53, 0.5 * alpha), px);
            line(&points, 1, &rgb(0xC2C3C7, alpha), px);
        }
        Ray::Tongue => {
            line(&points, cells(7.0, px), &rgb(0x7E2553, alpha), px);
            line(&points, cells(4.0, px), &rgb(0xFF77A8, alpha), px);
        }
        Ray::Magic => {
            line(&points, cells(6.0, px), &rgb(0x29ADFF, 0.3 * alpha), px);
            line(&points, 1, &rgb(0x29ADFF, alpha), px);
        }
        Ray::Wisp => {
            line(&points, cells(6.0, px), &rgb(0x83769C, 0.75 * alpha), px);
            line(&points, 1, &rgb(0xFFF1E8, 0.8 * alpha), px);
        }
        Ray::Bolt => {
            line(&points, cells(6.0, px), &rgb(0xFFA300, alpha), px);
            line(&points, cells(3.0, px), &rgb(0xFFEC27, alpha), px);
        }
        Ray::Beam => {
            // `width` is half of what the beam is across at its far end.
            let wide = cells(2.0 * width, px);
            let cone = cone_cells(from, to, cells(6.0, px), wide, px);
            fill_cells(&cone, &rgb(0xFFEC27, 0.3 * alpha), px);
            let core = cone_cells(from, to, 1, cells(1.2 * width, px), px);
            fill_cells(&core, &rgb(0xFFF1E8, 0.25 * alpha), px);
        }
    }
}

/// A plus of two blocks, one pixel wide, where a ray sticks.
fn draw_spark(kind: Ray, at: NSPoint, px: f64) {
    let color = match kind {
        Ray::Silk => rgb(0xFFF1E8, 1.0),
        Ray::Tongue => rgb(0xFF004D, 1.0),
        Ray::Wisp => rgb(0x83769C, 1.0),
        Ray::Magic | Ray::Bolt | Ray::Beam => rgb(0xFFEC27, 1.0),
    };
    let (x, y) = cell_of(at, px);
    fill_cells(
        &[Bar::block(x, y, 1, 3), Bar::block(x, y, 3, 1)],
        &color,
        px,
    );
}

fn draw_chip(
    chip: &Chip,
    at: NSPoint,
    scale: f64,
    alpha: f64,
    icons: &RefCell<HashMap<String, Retained<NSImage>>>,
) {
    let s = scale;
    match chip {
        Chip::Text => {
            let (w, h) = (46.0 * s, 16.0 * s);
            let body = rect(at.x - w / 2.0, at.y - h / 2.0, w, h);
            fill(body, &rgb(0x1D2B53, alpha));
            fill(
                rect(
                    body.origin.x + s,
                    body.origin.y + s,
                    w - 2.0 * s,
                    h - 2.0 * s,
                ),
                &rgb(0x29ADFF, alpha),
            );
            let bar = 3.0 * s;
            fill(
                rect(body.origin.x + 4.0 * s, at.y - bar / 2.0, 14.0 * s, bar),
                &rgb(0xFFF1E8, alpha),
            );
            fill(
                rect(body.origin.x + 22.0 * s, at.y - bar / 2.0, 18.0 * s, bar),
                &rgb(0xFFF1E8, alpha),
            );
        }
        Chip::Picture => {
            let (w, h) = (40.0 * s, 28.0 * s);
            let (x, y) = (at.x - w / 2.0, at.y - h / 2.0);
            fill(rect(x, y, w, h), &rgb(0x5F574F, alpha));
            fill(
                rect(x + s, y + s, w - 2.0 * s, h - 2.0 * s),
                &rgb(0x29ADFF, alpha),
            );
            fill(
                rect(x + 27.0 * s, y + 17.0 * s, 6.0 * s, 6.0 * s),
                &rgb(0xFFEC27, alpha),
            );
            fill(
                rect(x + 6.0 * s, y + s, 14.0 * s, 13.0 * s),
                &rgb(0x008751, alpha),
            );
            fill(
                rect(x + 18.0 * s, y + s, 14.0 * s, 9.0 * s),
                &rgb(0x008751, alpha),
            );
            fill(
                rect(x + s, y + s, w - 2.0 * s, 5.0 * s),
                &rgb(0x00E436, alpha),
            );
        }
        Chip::File(path) => {
            let mut icons = icons.borrow_mut();
            if icons.len() > 32 {
                icons.clear();
            }
            let icon = icons
                .entry(path.clone())
                .or_insert_with(|| {
                    NSWorkspace::sharedWorkspace().iconForFile(&NSString::from_str(path))
                })
                .clone();
            let side = 36.0 * s;
            let dest = rect(at.x - side / 2.0, at.y - side / 2.0, side, side);
            // The icon is a vector or a large bitmap: drawn smoothly, unlike the pixel art.
            if let Some(ctx) = NSGraphicsContext::currentContext() {
                ctx.setImageInterpolation(NSImageInterpolation::High);
            }
            // SAFETY: plain drawing of an image into the current context.
            unsafe {
                icon.drawInRect_fromRect_operation_fraction_respectFlipped_hints(
                    dest,
                    NSRect::ZERO,
                    NSCompositingOperation::SourceOver,
                    alpha,
                    true,
                    None,
                )
            };
            if let Some(ctx) = NSGraphicsContext::currentContext() {
                ctx.setImageInterpolation(NSImageInterpolation::None);
            }
        }
    }
}

fn draw_guard(kind: Guard, at: NSPoint, size: f64, turn: f64, px: f64) {
    let (x, y) = cell_of(at, px);
    let r = (size / px).round().max(1.0) as i32;
    match kind {
        Guard::Box => {
            let side = 2 * r + 1;
            fill_cells(
                &[Bar {
                    x: x - r,
                    y: y - r,
                    w: side,
                    h: side,
                }],
                &rgb(0x29ADFF, 0.15),
                px,
            );
            let edge: Vec<Bar> = (-r..=r)
                .flat_map(|k| {
                    [
                        Bar::square(x + k, y + r, 1),
                        Bar::square(x + k, y - r, 1),
                        Bar::square(x + r, y + k, 1),
                        Bar::square(x - r, y + k, 1),
                    ]
                })
                .collect();
            fill_cells(&edge, &rgb(0x29ADFF, 0.9), px);
        }
        Guard::Bubble => {
            fill_cells(&disc_cells(x, y, r), &rgb(0xFFF1E8, 0.22), px);
            fill_cells(&ring_cells(x, y, r, 1, None), &rgb(0x83769C, 0.9), px);
        }
        Guard::Ring => {
            fill_cells(&disc_cells(x, y, r - 1), &rgb(0x29ADFF, 0.1), px);
            // The dots march round the ring while the file is carried: about one turn every few seconds.
            let phase = turn / (TAU * size.max(1.0));
            fill_cells(
                &ring_cells(x, y, r, 1, Some(phase)),
                &rgb(0x29ADFF, 0.9),
                px,
            );
        }
        Guard::Cone => {
            let top = NSPoint::new(at.x, at.y + size * 1.7);
            let bottom = NSPoint::new(at.x, at.y - size);
            // The light is as wide as the circle the other guards draw, `size` to either side of the file.
            let light = cone_cells(top, bottom, 1, cells(2.0 * size, px), px);
            fill_cells(&light, &rgb(0xFFEC27, 0.28), px);
            let core = cone_cells(top, bottom, 1, cells(1.2 * size, px), px);
            fill_cells(&core, &rgb(0xFFF1E8, 0.2), px);
        }
    }
}

/// How far from its point a shape may draw, for the part of the view to redraw.
fn reach(shape: &Shape, cell: f64) -> NSRect {
    let around = |p: NSPoint, r: f64| rect(p.x - r, p.y - r, 2.0 * r, 2.0 * r);
    match shape {
        Shape::Sprite { at, scale, .. } => around(*at, cell * scale / 2.0 + 2.0),
        Shape::Ray {
            from, to, width, ..
        } => {
            let pad = width.max(8.0) + 4.0;
            rect(
                from.x.min(to.x) - pad,
                from.y.min(to.y) - pad,
                (from.x - to.x).abs() + 2.0 * pad,
                (from.y - to.y).abs() + 2.0 * pad,
            )
        }
        Shape::Spark { at, .. } => around(*at, 8.0),
        Shape::Chip { at, scale, .. } => around(*at, 26.0 * scale + 2.0),
        Shape::Guard { at, size, .. } => around(*at, size * 2.0 + 4.0),
        Shape::Tear { at, .. } => around(*at, 8.0),
    }
}

fn union(a: NSRect, b: NSRect) -> NSRect {
    if a.size.width <= 0.0 || a.size.height <= 0.0 {
        return b;
    }
    if b.size.width <= 0.0 || b.size.height <= 0.0 {
        return a;
    }
    let (x0, y0) = (a.origin.x.min(b.origin.x), a.origin.y.min(b.origin.y));
    let x1 = (a.origin.x + a.size.width).max(b.origin.x + b.size.width);
    let y1 = (a.origin.y + a.size.height).max(b.origin.y + b.size.height);
    rect(x0, y0, x1 - x0, y1 - y0)
}

pub struct ViewIvars {
    frame: RefCell<Frame>,
    sheet: RefCell<Option<Rc<Sheet>>>,
    /// Where the view's bottom left corner is on the screen.
    origin: Cell<NSPoint>,
    icons: RefCell<HashMap<String, Retained<NSImage>>>,
    /// Painted under everything, for pictures taken without a screen.
    backdrop: Cell<Option<u32>>,
}

define_class!(
    // SAFETY: NSView allows subclassing; the ivars are only touched on the main thread and need no Drop.
    #[unsafe(super(NSView))]
    #[thread_kind = MainThreadOnly]
    #[name = "FigMascotView"]
    #[ivars = ViewIvars]
    struct MascotView;

    impl MascotView {
        #[unsafe(method(drawRect:))]
        fn draw_rect(&self, _dirty: NSRect) {
            self.paint();
        }

        #[unsafe(method(isOpaque))]
        fn is_opaque(&self) -> bool {
            false
        }
    }

    unsafe impl NSObjectProtocol for MascotView {}
);

impl MascotView {
    fn new(mtm: MainThreadMarker, frame: NSRect) -> Retained<Self> {
        let ivars = ViewIvars {
            frame: RefCell::new(Frame::default()),
            sheet: RefCell::new(None),
            origin: Cell::new(NSPoint::new(0.0, 0.0)),
            icons: RefCell::new(HashMap::new()),
            backdrop: Cell::new(None),
        };
        // SAFETY: designated initializer with valid arguments on the main thread.
        unsafe { msg_send![super(mtm.alloc::<Self>().set_ivars(ivars)), initWithFrame: frame] }
    }

    /// The part of the view the shapes cover, in view coordinates.
    fn covered(&self) -> NSRect {
        let ivars = self.ivars();
        let cell = ivars.sheet.borrow().as_ref().map_or(48.0, |s| s.cell);
        let o = ivars.origin.get();
        let all = ivars
            .frame
            .borrow()
            .shapes
            .iter()
            .map(|s| reach(s, cell))
            .fold(NSRect::ZERO, union);
        rect(
            all.origin.x - o.x,
            all.origin.y - o.y,
            all.size.width,
            all.size.height,
        )
    }

    /// Replaces what is drawn and redraws the parts that changed.
    fn set_frame(&self, frame: Frame) {
        let before = self.covered();
        *self.ivars().frame.borrow_mut() = frame;
        let dirty = union(before, self.covered());
        if dirty.size.width > 0.0 {
            self.setNeedsDisplayInRect(dirty);
        }
    }

    fn paint(&self) {
        let ivars = self.ivars();
        let Some(sheet) = ivars.sheet.borrow().clone() else {
            return;
        };
        let Some(ctx) = NSGraphicsContext::currentContext() else {
            return;
        };
        let frame = ivars.frame.borrow();
        let o = ivars.origin.get();
        let local = |p: NSPoint| NSPoint::new(p.x - o.x, p.y - o.y);
        // Every effect is drawn on the art's own pixel grid, so the whole scene is one size of pixel.
        let px = art_pixel(&frame);
        // The sprite is placed on the device's own pixels, a finer grid: a whole point would be coarse enough
        // to see the movement step, and a fraction of one would soften the art.
        let step = 1.0 / self.window().map_or(1.0, |w| w.backingScaleFactor());
        let on_device = |v: f64| (v / step).round() * step;
        ctx.saveGraphicsState();
        if let Some(color) = ivars.backdrop.get() {
            fill(self.bounds(), &rgb(color, 1.0));
            if let Some(hole) = frame.hole {
                fill(
                    NSRect::new(local(hole.origin), hole.size),
                    &rgb(0x000000, 1.0),
                );
            }
        }
        // Hard edges, like the sprites.
        ctx.setImageInterpolation(NSImageInterpolation::None);
        ctx.setShouldAntialias(false);
        if let Some(hole) = frame.hole {
            // Everything but the hole: an even-odd path of the whole view and the hole.
            let clip = NSBezierPath::bezierPathWithRect(self.bounds());
            clip.appendBezierPathWithRect(NSRect::new(local(hole.origin), hole.size));
            clip.setWindingRule(NSWindingRule::EvenOdd);
            clip.addClip();
        }
        for shape in &frame.shapes {
            match shape {
                Shape::Sprite { frame, at, scale } => {
                    let side = sheet.cell * scale;
                    let p = local(*at);
                    let dest = rect(
                        on_device(p.x - side / 2.0),
                        on_device(p.y - side / 2.0),
                        side,
                        side,
                    );
                    let from = rect(*frame as f64 * sheet.cell, 0.0, sheet.cell, sheet.cell);
                    // SAFETY: plain drawing of an image into the current context.
                    unsafe {
                        sheet
                            .image
                            .drawInRect_fromRect_operation_fraction_respectFlipped_hints(
                                dest,
                                from,
                                NSCompositingOperation::SourceOver,
                                1.0,
                                true,
                                None,
                            )
                    };
                }
                Shape::Ray {
                    kind,
                    from,
                    to,
                    width,
                    alpha,
                } => draw_ray(*kind, local(*from), local(*to), *width, *alpha, px),
                Shape::Spark { kind, at } => draw_spark(*kind, local(*at), px),
                Shape::Chip {
                    chip,
                    at,
                    scale,
                    alpha,
                } => draw_chip(chip, local(*at), *scale, *alpha, &ivars.icons),
                Shape::Guard {
                    kind,
                    at,
                    size,
                    turn,
                } => draw_guard(*kind, local(*at), *size, *turn, px),
                Shape::Tear { at, alpha } => {
                    // A drop one pixel wide and three tall, falling from an eye.
                    let (x, y) = cell_of(local(*at), px);
                    fill_cells(&[Bar::block(x, y, 1, 3)], &rgb(0x29ADFF, *alpha), px);
                }
            }
        }
        ctx.restoreGraphicsState();
    }
}

/// What calls the story back on every frame: the display link of the screen the mascot plays on, or, on macOS
/// 13, which has none, a timer at a common display rate. The notch and the switcher keep their frames the same
/// way, and the panel is drawn in step with the screen instead of beside it.
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

struct TickerIvars {
    stage: Weak<Stage>,
}

define_class!(
    // SAFETY: NSObject allows subclassing; the ivar is only touched on the main thread and needs no Drop.
    #[unsafe(super(NSObject))]
    #[thread_kind = MainThreadOnly]
    #[name = "FigMascotTicker"]
    #[ivars = TickerIvars]
    struct Ticker;

    impl Ticker {
        #[unsafe(method(tick:))]
        fn tick(&self, link: &CADisplayLink) {
            // The last frame of a story lets go of the display link, which lets go of this object: kept alive
            // until the call returns.
            let _alive = self.retain();
            if let Some(stage) = self.ivars().stage.upgrade() {
                stage.tick(link.targetTimestamp());
            }
        }
    }

    unsafe impl NSObjectProtocol for Ticker {}
);

impl Ticker {
    fn new(mtm: MainThreadMarker, stage: Weak<Stage>) -> Retained<Self> {
        let this = mtm.alloc::<Self>().set_ivars(TickerIvars { stage });
        // SAFETY: NSObject's designated initializer.
        unsafe { msg_send![super(this), init] }
    }
}

/// One run of the story: the frame for each moment, and what is calling it back.
struct Play {
    at: Box<dyn Fn(f64) -> Frame>,
    seconds: f64,
    /// The display time of the first frame, which is where the story starts.
    start: Cell<Option<f64>>,
    /// Held for as long as the story plays; letting go of it stops the frames.
    _driver: Driver,
}

/// The panel the mascot plays on. Created once and kept; it is on screen only while something plays.
pub struct Stage {
    panel: Retained<FigPanel>,
    view: Retained<MascotView>,
    mtm: MainThreadMarker,
    run: RefCell<Option<Play>>,
}

impl Stage {
    pub fn new(mtm: MainThreadMarker) -> Rc<Self> {
        let frame = rect(0.0, 0.0, 100.0, 100.0);
        let panel = FigPanel::new(mtm, frame);
        // Above the notch, which it seems to come out of, and never in the way of a click or a drop.
        panel.setLevel(NSMainMenuWindowLevel + 2);
        panel.setIgnoresMouseEvents(true);
        let view = MascotView::new(mtm, frame);
        panel.setContentView(Some(&view));
        Rc::new(Self {
            panel,
            view,
            mtm,
            run: RefCell::new(None),
        })
    }

    fn place(&self, screen: NSRect, sheet: &Rc<Sheet>) {
        if self.panel.frame() != screen {
            self.panel.setFrame_display(screen, false);
            self.view
                .setFrame(rect(0.0, 0.0, screen.size.width, screen.size.height));
        }
        self.view.ivars().origin.set(screen.origin);
        *self.view.ivars().sheet.borrow_mut() = Some(Rc::clone(sheet));
    }

    /// Draws `frame` now and keeps it until the next call; `screen` is the screen it is on.
    pub fn show(&self, screen: NSRect, sheet: &Rc<Sheet>, frame: Frame) {
        self.place(screen, sheet);
        self.view.set_frame(frame);
        if !self.panel.isVisible() {
            self.panel.orderFrontRegardless();
        }
    }

    /// Plays `seconds` of animation in place of whatever was playing. `at(t)` is the frame `t` seconds in;
    /// the stage leaves the screen at the end.
    pub fn play(
        self: &Rc<Self>,
        screen: NSRect,
        sheet: &Rc<Sheet>,
        seconds: f64,
        at: impl Fn(f64) -> Frame + 'static,
    ) {
        self.show(screen, sheet, at(0.0));
        let play = Play {
            at: Box::new(at),
            seconds,
            start: Cell::new(None),
            _driver: self.driver(Rc::downgrade(self)),
        };
        // The story that was playing, if any, stops here, with its frames.
        *self.run.borrow_mut() = Some(play);
    }

    /// One frame of the story, for the moment the screen is drawing. Called by the driver only.
    fn tick(&self, now: f64) {
        let run = self.run.borrow();
        let Some(play) = run.as_ref() else {
            return;
        };
        let start = play.start.get().unwrap_or_else(|| {
            play.start.set(Some(now));
            now
        });
        let t = now - start;
        if t >= play.seconds {
            drop(run);
            self.hide();
        } else {
            self.view.set_frame((play.at)(t));
        }
    }

    /// The frames to play on: the display link of the screen the panel is on, or a timer where there is none.
    fn driver(&self, stage: Weak<Stage>) -> Driver {
        // A window off every screen (the checks) still gets one.
        let screen = self
            .panel
            .screen()
            .or_else(|| NSScreen::mainScreen(self.mtm));
        if let Some(screen) =
            screen.filter(|s| s.respondsToSelector(sel!(displayLinkWithTarget:selector:)))
        {
            let ticker = Ticker::new(self.mtm, stage);
            // SAFETY: the ticker answers `tick:`, taking the display link, which is what the selector names.
            let link = unsafe { screen.displayLinkWithTarget_selector(&ticker, sel!(tick:)) };
            // SAFETY: the main run loop, in the common modes so frames also come while the pointer is held down.
            unsafe { link.addToRunLoop_forMode(&NSRunLoop::mainRunLoop(), NSRunLoopCommonModes) };
            Driver::Link(link)
        } else {
            Driver::Timer(app::every(1.0 / 120.0, move || {
                if let Some(stage) = stage.upgrade() {
                    let now = CACurrentMediaTime();
                    stage.tick(now);
                }
            }))
        }
    }

    /// Whether an animation is playing.
    pub fn playing(&self) -> bool {
        self.run.borrow().is_some()
    }

    /// Takes the stage off the screen now, which stops the story with it.
    pub fn hide(&self) {
        self.run.borrow_mut().take();
        self.view.set_frame(Frame::default());
        self.panel.orderOut(None);
    }

    /// A picture of one frame over `area` of the screen, on a solid backdrop with the hole painted black, for
    /// checks that cannot look at a screen.
    pub fn render_png(
        mtm: MainThreadMarker,
        sheet: &Rc<Sheet>,
        frame: Frame,
        area: NSRect,
        backdrop: u32,
    ) -> Option<Vec<u8>> {
        let view = MascotView::new(mtm, rect(0.0, 0.0, area.size.width, area.size.height));
        view.ivars().origin.set(area.origin);
        view.ivars().backdrop.set(Some(backdrop));
        *view.ivars().sheet.borrow_mut() = Some(Rc::clone(sheet));
        *view.ivars().frame.borrow_mut() = frame;
        let bounds = view.bounds();
        let rep = view.bitmapImageRepForCachingDisplayInRect(bounds)?;
        view.cacheDisplayInRect_toBitmapImageRep(bounds, &rep);
        // SAFETY: an empty properties dictionary is valid for PNG output.
        unsafe {
            rep.representationUsingType_properties(NSBitmapImageFileType::PNG, &NSDictionary::new())
        }
        .map(|d| d.to_vec())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rays_start_and_end_where_they_are_told() {
        let (a, b) = (NSPoint::new(10.0, 200.0), NSPoint::new(300.0, 20.0));
        for kind in [
            Ray::Silk,
            Ray::Tongue,
            Ray::Beam,
            Ray::Wisp,
            Ray::Magic,
            Ray::Bolt,
        ] {
            let points = ray_points(kind, a, b);
            assert!(points.len() >= 2, "{kind:?}");
            let (first, last) = (points[0], points[points.len() - 1]);
            assert!(
                (first.x - a.x).abs() < 1e-9 && (first.y - a.y).abs() < 1e-9,
                "{kind:?} starts at the actor"
            );
            assert!(
                (last.x - b.x).abs() < 1e-9 && (last.y - b.y).abs() < 1e-9,
                "{kind:?} ends at the target"
            );
        }
        // A ray of no length is a single straight step, never a division by zero.
        assert_eq!(ray_points(Ray::Wisp, a, a).len(), 2);
    }

    #[test]
    fn the_redrawn_part_covers_both_frames() {
        let a = rect(0.0, 0.0, 10.0, 10.0);
        let b = rect(20.0, 30.0, 5.0, 5.0);
        assert_eq!(union(a, b), rect(0.0, 0.0, 25.0, 35.0));
        assert_eq!(union(NSRect::ZERO, b), b);
        let spark = reach(
            &Shape::Spark {
                kind: Ray::Silk,
                at: NSPoint::new(50.0, 50.0),
            },
            48.0,
        );
        assert!(spark.origin.x < 50.0 && spark.origin.x + spark.size.width > 50.0);
    }

    #[test]
    fn a_ray_is_a_line_of_whole_pixels_with_no_gaps_in_it() {
        let (a, b) = (NSPoint::new(10.0, 200.0), NSPoint::new(300.0, 20.0));
        let cells = line_cells(&[a, b], 2.0);
        assert_eq!(cells[0], cell_of(a, 2.0), "it starts where it is told");
        assert_eq!(
            cells[cells.len() - 1],
            cell_of(b, 2.0),
            "and reaches the target"
        );
        assert!(cells.len() > 100, "one pixel at a time: {}", cells.len());
        for pair in cells.windows(2) {
            let steps = ((pair[1].0 - pair[0].0).abs(), (pair[1].1 - pair[0].1).abs());
            assert!(steps.0 <= 1 && steps.1 <= 1, "no gaps: {pair:?}");
            assert!(steps != (0, 0), "and no pixel drawn twice: {pair:?}");
        }
        // A ray of no length is one pixel, never a loop that goes on for ever.
        assert_eq!(line_cells(&[a, a], 2.0), vec![cell_of(a, 2.0)]);
    }

    #[test]
    fn every_width_comes_out_as_whole_odd_pixels() {
        for points in [0.5, 1.0, 1.5, 3.0, 6.0, 7.5, 29.0] {
            let n = cells(points, 1.5);
            assert!(n >= 1 && n % 2 == 1, "{points} points comes to {n} pixels");
        }
    }

    #[test]
    fn a_disc_is_a_symmetrical_circle_of_whole_rows() {
        let disc = disc_cells(0, 0, 4);
        assert_eq!(disc.len(), 9, "one row for every pixel of the diameter");
        assert_eq!(disc[4].w, 9, "the middle row is the full width");
        for (i, bar) in disc.iter().enumerate() {
            let mirror = &disc[disc.len() - 1 - i];
            assert_eq!(bar.w, mirror.w, "row {i} across the middle");
            assert_eq!(bar.x, -bar.w / 2, "centred on the cell");
            assert_eq!(bar.h, 1);
        }
    }

    #[test]
    fn a_ring_is_hollow_and_a_dotted_one_leaves_pixels_out() {
        let solid = ring_cells(0, 0, 6, 1, None);
        let dotted = ring_cells(0, 0, 6, 1, Some(0.0));
        assert!(dotted.len() < solid.len(), "a dotted ring is not a ring");
        assert!(dotted.len() * 4 > solid.len(), "but only just dotted");
        for bar in &solid {
            assert_eq!((bar.w, bar.h), (1, 1), "a ring is one pixel thick");
            assert!(bar.x.abs() <= 6 && bar.y.abs() <= 6);
        }
        // It goes all the way round: both sides and the top.
        for (x, y) in [(6, 0), (-6, 0), (0, 6), (0, -6)] {
            let pixel = Bar { x, y, w: 1, h: 1 };
            assert!(solid.contains(&pixel), "{pixel:?} is on the ring");
        }
        // A thicker ring keeps the edge and adds to what is inside it.
        assert!(ring_cells(0, 0, 6, 2, None).len() > solid.len());
    }

    #[test]
    fn a_cone_widens_from_one_end_to_the_other() {
        let cone = cone_cells(
            NSPoint::new(100.0, 100.0),
            NSPoint::new(100.0, 0.0),
            1,
            9,
            2.0,
        );
        let width = |b: &Bar| b.w;
        assert_eq!(width(&cone[0]), 1, "narrow where it starts");
        assert_eq!(width(&cone[cone.len() - 1]), 9, "wide where it lands");
        assert!(cone.iter().all(|b| b.w % 2 == 1 && b.h == b.w));
        assert!(
            cone.windows(2).all(|pair| pair[1].w >= pair[0].w),
            "it only widens"
        );
    }

    #[test]
    fn the_art_the_effects_are_drawn_on_is_the_size_of_the_one_the_character_is() {
        for scale in [1.5, 2.0] {
            let frame = Frame {
                shapes: vec![
                    Shape::Tear {
                        at: NSPoint::new(0.0, 0.0),
                        alpha: 1.0,
                    },
                    Shape::Sprite {
                        frame: 0,
                        at: NSPoint::new(0.0, 0.0),
                        scale,
                    },
                ],
                hole: None,
            };
            assert_eq!(art_pixel(&frame), scale);
        }
        assert_eq!(
            art_pixel(&Frame::default()),
            2.0,
            "a frame with nothing in it still picks a size"
        );
    }
}
