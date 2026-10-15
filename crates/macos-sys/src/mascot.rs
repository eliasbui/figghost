//! The stage of the clipboard mascot: a click-through panel over the notch's screen, on which a small pixel
//! character comes out of the notch to fetch what was copied and sees files off as they leave the shelf.
//! It only draws what it is handed. Where everything is at each moment is worked out by the app
//! (`app-ui/src/features/mascot`), so the story is plain data that tests can look at.

use crate::{app, panel::FigPanel};
use objc2::{
    AnyThread, DefinedClass, MainThreadMarker, MainThreadOnly, define_class, msg_send, rc::Retained,
};
use objc2_app_kit::{
    NSBezierPath, NSBitmapImageFileType, NSBitmapImageRep, NSColor, NSCompositingOperation,
    NSEvent, NSGraphicsContext, NSImage, NSImageInterpolation, NSLineCapStyle, NSLineJoinStyle,
    NSMainMenuWindowLevel, NSScreen, NSView, NSWindingRule, NSWorkspace,
};
use objc2_foundation::{NSData, NSDictionary, NSObjectProtocol, NSPoint, NSRect, NSSize, NSString};
use std::{
    cell::{Cell, RefCell},
    collections::HashMap,
    f64::consts::TAU,
    rc::Rc,
    time::Instant,
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

fn stroke(points: &[NSPoint], width: f64, color: &NSColor) {
    let Some((first, rest)) = points.split_first() else {
        return;
    };
    let path = NSBezierPath::bezierPath();
    path.moveToPoint(*first);
    for p in rest {
        path.lineToPoint(*p);
    }
    path.setLineWidth(width);
    path.setLineCapStyle(NSLineCapStyle::Round);
    path.setLineJoinStyle(NSLineJoinStyle::Round);
    color.setStroke();
    path.stroke();
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

fn cone(from: NSPoint, to: NSPoint, near: f64, far: f64) -> Retained<NSBezierPath> {
    let (dx, dy) = (to.x - from.x, to.y - from.y);
    let len = dx.hypot(dy).max(1.0);
    let (nx, ny) = (-dy / len, dx / len);
    let path = NSBezierPath::bezierPath();
    path.moveToPoint(NSPoint::new(from.x + nx * near, from.y + ny * near));
    path.lineToPoint(NSPoint::new(to.x + nx * far, to.y + ny * far));
    path.lineToPoint(NSPoint::new(to.x - nx * far, to.y - ny * far));
    path.lineToPoint(NSPoint::new(from.x - nx * near, from.y - ny * near));
    path.closePath();
    path
}

fn draw_ray(kind: Ray, from: NSPoint, to: NSPoint, width: f64, alpha: f64) {
    let points = ray_points(kind, from, to);
    match kind {
        Ray::Silk => {
            stroke(&points, 3.0, &rgb(0x1D2B53, 0.5 * alpha));
            stroke(&points, 1.5, &rgb(0xC2C3C7, alpha));
        }
        Ray::Tongue => {
            stroke(&points, 7.0, &rgb(0x7E2553, alpha));
            stroke(&points, 4.5, &rgb(0xFF77A8, alpha));
        }
        Ray::Magic => {
            stroke(&points, 6.0, &rgb(0x29ADFF, 0.3 * alpha));
            stroke(&points, 2.0, &rgb(0x29ADFF, alpha));
        }
        Ray::Wisp => {
            stroke(&points, 6.0, &rgb(0x83769C, 0.75 * alpha));
            stroke(&points, 2.0, &rgb(0xFFF1E8, 0.8 * alpha));
        }
        Ray::Bolt => {
            stroke(&points, 6.0, &rgb(0xFFA300, alpha));
            stroke(&points, 2.5, &rgb(0xFFEC27, alpha));
        }
        Ray::Beam => {
            rgb(0xFFEC27, 0.3 * alpha).setFill();
            cone(from, to, 5.0, width).fill();
            rgb(0xFFF1E8, 0.25 * alpha).setFill();
            cone(from, to, 2.0, width * 0.6).fill();
        }
    }
}

/// A plus of pixels, two points each.
fn draw_spark(kind: Ray, at: NSPoint) {
    let color = match kind {
        Ray::Silk => rgb(0xFFF1E8, 1.0),
        Ray::Tongue => rgb(0xFF004D, 1.0),
        Ray::Wisp => rgb(0x83769C, 1.0),
        Ray::Magic | Ray::Bolt | Ray::Beam => rgb(0xFFEC27, 1.0),
    };
    let (x, y) = (at.x.round(), at.y.round());
    if kind == Ray::Tongue {
        fill(rect(x - 4.0, y - 2.0, 8.0, 4.0), &color);
        fill(rect(x - 2.0, y - 4.0, 4.0, 8.0), &color);
    } else {
        fill(rect(x - 2.0, y - 6.0, 4.0, 12.0), &color);
        fill(rect(x - 6.0, y - 2.0, 12.0, 4.0), &color);
    }
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

fn draw_guard(kind: Guard, at: NSPoint, size: f64, turn: f64) {
    let around = rect(at.x - size, at.y - size, 2.0 * size, 2.0 * size);
    match kind {
        Guard::Box => {
            fill(around, &rgb(0x29ADFF, 0.15));
            let path = NSBezierPath::bezierPathWithRect(around);
            path.setLineWidth(2.0);
            rgb(0x29ADFF, 0.9).setStroke();
            path.stroke();
        }
        Guard::Bubble => {
            let path = NSBezierPath::bezierPathWithOvalInRect(around);
            rgb(0xFFF1E8, 0.22).setFill();
            path.fill();
            path.setLineWidth(2.0);
            rgb(0x83769C, 0.9).setStroke();
            path.stroke();
        }
        Guard::Ring => {
            let path = NSBezierPath::bezierPathWithOvalInRect(around);
            rgb(0x29ADFF, 0.1).setFill();
            path.fill();
            path.setLineWidth(2.5);
            let dash = [8.0_f64, 6.0];
            // SAFETY: the pattern pointer is valid for the two entries named by the count.
            unsafe { path.setLineDash_count_phase(dash.as_ptr().cast_mut(), 2, turn) };
            rgb(0x29ADFF, 0.9).setStroke();
            path.stroke();
        }
        Guard::Cone => {
            let top = NSPoint::new(at.x, at.y + size * 1.7);
            let bottom = NSPoint::new(at.x, at.y - size);
            rgb(0xFFEC27, 0.28).setFill();
            cone(top, bottom, 5.0, size).fill();
            rgb(0xFFF1E8, 0.2).setFill();
            cone(top, bottom, 2.0, size * 0.6).fill();
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
                        (p.x - side / 2.0).round(),
                        (p.y - side / 2.0).round(),
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
                } => draw_ray(*kind, local(*from), local(*to), *width, *alpha),
                Shape::Spark { kind, at } => draw_spark(*kind, local(*at)),
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
                } => draw_guard(*kind, local(*at), *size, *turn),
                Shape::Tear { at, alpha } => {
                    let p = local(*at);
                    fill(
                        rect(p.x.round() - 2.0, p.y.round() - 3.0, 4.0, 6.0),
                        &rgb(0x29ADFF, *alpha),
                    );
                }
            }
        }
        ctx.restoreGraphicsState();
    }
}

/// The panel the mascot plays on. Created once and kept; it is on screen only while something plays.
pub struct Stage {
    panel: Retained<FigPanel>,
    view: Retained<MascotView>,
    run: RefCell<Option<app::Timer>>,
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
        self.run.borrow_mut().take();
        self.show(screen, sheet, at(0.0));
        let started = Instant::now();
        let me = Rc::downgrade(self);
        let timer = app::every(1.0 / 60.0, move || {
            let Some(me) = me.upgrade() else {
                return;
            };
            let t = started.elapsed().as_secs_f64();
            if t >= seconds {
                // Dropping the timer from its own callback is safe: the run loop holds it until it returns.
                me.hide();
            } else {
                me.view.set_frame(at(t));
            }
        });
        *self.run.borrow_mut() = Some(timer);
    }

    /// Whether an animation is playing.
    pub fn playing(&self) -> bool {
        self.run.borrow().is_some()
    }

    /// Takes the stage off the screen now.
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
}
