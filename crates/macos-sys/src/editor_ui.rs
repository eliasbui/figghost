//! The capture editor: a window with tools along the top and the picture below. Arrows, lines, frames,
//! ellipses, highlights, text, numbered steps and pixelated areas are drawn over the picture, which can
//! be cropped and set on a gradient background. The result is rendered at the capture's own resolution.
//!
//! Keys: V select, A arrow, L line, R frame, O ellipse, H highlight, T text, N number, P pixelate,
//! C crop; Delete removes the selected mark; Command+Z and Command+Shift+Z undo and redo.

use crate::{
    app,
    screen_capture::Shot,
    widgets::{ActionTarget, Fit, draw_text, text_attributes},
};
use core_capture::{
    Background, Color, Document, GRADIENTS, Mark, PALETTE, Point, Rect, Shape, Tool, arrow_head,
    counter_radius, pixelate,
};
use objc2::{
    AnyThread, DefinedClass, MainThreadMarker, MainThreadOnly, define_class, msg_send,
    rc::Retained,
    runtime::{AnyObject, ProtocolObject},
    sel,
};
use objc2_app_kit::{
    NSAffineTransformNSAppKitAdditions, NSApplication, NSAttributedStringNSStringDrawing,
    NSBackingStoreType, NSBezierPath, NSBitmapImageRep, NSBorderType, NSButton, NSClipView,
    NSColor, NSControlSize, NSControlStateValueOn, NSDeviceRGBColorSpace, NSEvent,
    NSEventModifierFlags, NSFont, NSFontWeightBold, NSGradient, NSGraphicsContext, NSImage,
    NSLineCapStyle, NSLineJoinStyle, NSPopUpButton, NSScreen, NSScrollView,
    NSSegmentSwitchTracking, NSSegmentedControl, NSShadow, NSStringDrawing, NSTextField, NSView,
    NSWindow, NSWindowDelegate, NSWindowStyleMask,
};
use objc2_foundation::{
    NSAffineTransform, NSAttributedString, NSDictionary, NSNotification, NSObject,
    NSObjectProtocol, NSPoint, NSRect, NSSize, NSString,
};
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
};

const TOOLBAR_HEIGHT: f64 = 78.0;
const MIN_WIDTH: f64 = 1000.0;
const ACTION_WIDTH: f64 = 86.0;
const ACTION_GAP: f64 = 4.0;
const WIDTHS: [f64; 4] = [2.0, 4.0, 6.0, 9.0];
/// Opacity of a highlight; it is multiplied with the picture, so text under it stays readable.
const HIGHLIGHT_ALPHA: f64 = 0.35;
/// Smallest drag, in points, that makes a shape rather than counting as a click.
const MIN_DRAG: f64 = 4.0;
/// Pixelated blocks, in points.
const PIXEL_BLOCK: f64 = 10.0;

/// Labels of the editor's controls, in the user's language.
#[derive(Debug, Clone, Default)]
pub struct EditorTexts {
    pub title: String,
    /// One per `Tool::ALL`, in that order.
    pub tools: Vec<String>,
    pub undo: String,
    pub redo: String,
    pub background: String,
    pub copy: String,
    pub save: String,
    pub pin: String,
    pub done: String,
}

/// What the editor's buttons do with the finished picture.
pub struct EditorActions {
    pub on_copy: Box<dyn Fn(Shot)>,
    pub on_save: Box<dyn Fn(Shot)>,
    pub on_pin: Box<dyn Fn(Shot)>,
    /// Done: the editor closes after this.
    pub on_done: Box<dyn Fn(Shot)>,
    pub on_closed: Box<dyn Fn()>,
}

fn ns_color(c: Color) -> Retained<NSColor> {
    NSColor::colorWithSRGBRed_green_blue_alpha(c.r, c.g, c.b, c.a)
}

fn ns_rect(r: Rect) -> NSRect {
    NSRect::new(NSPoint::new(r.x, r.y), NSSize::new(r.w, r.h))
}

fn ns_point(p: Point) -> NSPoint {
    NSPoint::new(p.x, p.y)
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum Drag {
    None,
    /// A new shape from `start` to the pointer.
    Draw {
        start: Point,
        now: Point,
    },
    /// The selected mark, moved by the pointer.
    Move {
        index: usize,
        start: Point,
        now: Point,
    },
}

/// The editor's state, shared by the canvas and the controls.
struct Editor {
    doc: RefCell<Document>,
    tool: Cell<Tool>,
    color: Cell<usize>,
    width: Cell<f64>,
    gradient: Cell<usize>,
    selected: Cell<Option<usize>>,
    drag: Cell<Drag>,
    shot: Shot,
    picture: Retained<NSImage>,
    pixelated: Retained<NSImage>,
    /// Text being typed and where it goes, in picture points.
    typing: RefCell<Option<(Retained<NSTextField>, Point)>>,
}

impl Editor {
    fn mark_color(&self) -> Color {
        PALETTE[self.color.get()]
    }

    fn text_size(&self) -> f64 {
        14.0 + self.width.get() * 2.0
    }

    /// Where the visible picture sits on the canvas, and the shift from picture points to canvas points.
    fn placement(&self) -> (Rect, Point) {
        let doc = self.doc.borrow();
        let (_, _, content) = doc.layout();
        let visible = doc.visible();
        (
            content,
            Point::new(content.x - visible.x, content.y - visible.y),
        )
    }

    fn to_picture(&self, canvas: NSPoint) -> Point {
        let (_, shift) = self.placement();
        Point::new(canvas.x - shift.x, canvas.y - shift.y)
    }

    /// The shape a drag from `a` to `b` makes with the current tool.
    fn shape_for(&self, a: Point, b: Point) -> Option<Shape> {
        let rect = Rect::from_corners(a, b);
        let long = (b.x - a.x).hypot(b.y - a.y) >= MIN_DRAG;
        let wide = rect.w >= MIN_DRAG && rect.h >= MIN_DRAG;
        match self.tool.get() {
            Tool::Arrow if long => Some(Shape::Arrow { from: a, to: b }),
            Tool::Line if long => Some(Shape::Line { from: a, to: b }),
            Tool::Rectangle if wide => Some(Shape::Rectangle(rect)),
            Tool::Ellipse if wide => Some(Shape::Ellipse(rect)),
            Tool::Highlight if wide => Some(Shape::Highlight(rect)),
            Tool::Pixelate if wide => Some(Shape::Pixelate(rect)),
            _ => None,
        }
    }

    /// Draws everything onto the current graphics context, whose coordinates are canvas points.
    /// `export` leaves out the selection and the shape being dragged.
    fn draw(&self, export: bool) {
        let doc = self.doc.borrow();
        let (width, height, content) = doc.layout();
        let (_, shift) = self.placement();
        let Some(context) = NSGraphicsContext::currentContext() else {
            return;
        };
        let canvas = NSRect::new(NSPoint::ZERO, NSSize::new(width, height));
        let content_rect = ns_rect(content);
        let background = doc.background();
        let clip = match background {
            Some(bg) => {
                let (top, bottom) = GRADIENTS[bg.gradient % GRADIENTS.len()];
                if let Some(gradient) = NSGradient::initWithStartingColor_endingColor(
                    NSGradient::alloc(),
                    &ns_color(bottom),
                    &ns_color(top),
                ) {
                    gradient.drawInRect_angle(canvas, 90.0);
                }
                let shape = NSBezierPath::bezierPathWithRoundedRect_xRadius_yRadius(
                    content_rect,
                    bg.radius,
                    bg.radius,
                );
                if bg.shadow {
                    context.saveGraphicsState();
                    let shadow = NSShadow::new();
                    shadow.setShadowBlurRadius(28.0);
                    shadow.setShadowOffset(NSSize::new(0.0, -10.0));
                    shadow.setShadowColor(Some(&NSColor::colorWithWhite_alpha(0.0, 0.45)));
                    shadow.set();
                    NSColor::blackColor().setFill();
                    shape.fill();
                    context.restoreGraphicsState();
                }
                shape
            }
            None => NSBezierPath::bezierPathWithRect(content_rect),
        };

        context.saveGraphicsState();
        clip.addClip();
        let picture_rect = NSRect::new(
            NSPoint::new(shift.x, shift.y),
            NSSize::new(doc.width, doc.height),
        );
        self.picture.drawInRect(picture_rect);
        let transform = NSAffineTransform::transform();
        transform.translateXBy_yBy(shift.x, shift.y);
        for mark in doc.marks() {
            if let Shape::Pixelate(r) = mark.shape {
                context.saveGraphicsState();
                NSBezierPath::bezierPathWithRect(ns_rect(r.offset(shift.x, shift.y))).addClip();
                self.pixelated.drawInRect(picture_rect);
                context.restoreGraphicsState();
            }
        }
        context.saveGraphicsState();
        transform.concat();
        for mark in doc.marks() {
            draw_mark(mark);
        }
        if !export {
            match self.drag.get() {
                Drag::Draw { start, now } => {
                    if self.tool.get() == Tool::Crop {
                        dashed(Rect::from_corners(start, now), &NSColor::whiteColor());
                    } else if let Some(shape) = self.shape_for(start, now) {
                        draw_mark(&Mark {
                            shape,
                            color: self.mark_color(),
                            width: self.width.get(),
                        });
                    }
                }
                Drag::Move { index, start, now } => {
                    if let Some(mark) = doc.marks().get(index) {
                        dashed(
                            mark.moved(now.x - start.x, now.y - start.y).bounds(),
                            &NSColor::systemBlueColor(),
                        );
                    }
                }
                Drag::None => {}
            }
            if let (Some(index), Drag::None) = (self.selected.get(), self.drag.get())
                && let Some(mark) = doc.marks().get(index)
            {
                dashed(mark.bounds(), &NSColor::systemBlueColor());
            }
        }
        context.restoreGraphicsState();
        context.restoreGraphicsState();
    }
}

fn dashed(r: Rect, color: &NSColor) {
    let path = NSBezierPath::bezierPathWithRect(ns_rect(Rect::new(
        r.x - 3.0,
        r.y - 3.0,
        r.w + 6.0,
        r.h + 6.0,
    )));
    path.setLineWidth(1.0);
    let dashes = [4.0, 3.0];
    // SAFETY: the pattern has the two entries counted.
    unsafe { path.setLineDash_count_phase(dashes.as_ptr(), 2, 0.0) };
    color.setStroke();
    path.stroke();
}

fn text_attrs(size: f64, color: Color) -> Retained<NSDictionary<NSString, AnyObject>> {
    // SAFETY: extern static weight constant.
    let font = NSFont::systemFontOfSize_weight(size, unsafe { NSFontWeightBold });
    let attrs = text_attributes(&font, &ns_color(color), Fit::Clip, false);
    Retained::into_super(attrs)
}

/// The size `text` takes when drawn as a text mark.
fn measure(text: &str, size: f64) -> NSSize {
    let attrs = text_attrs(size, PALETTE[0]);
    // SAFETY: the attributes are valid string drawing attributes.
    let string = unsafe {
        NSAttributedString::initWithString_attributes(
            NSAttributedString::alloc(),
            &NSString::from_str(text),
            Some(&attrs),
        )
    };
    string.size()
}

fn draw_mark(mark: &Mark) {
    let color = ns_color(mark.color);
    let stroke = |path: &NSBezierPath| {
        path.setLineWidth(mark.width);
        path.setLineCapStyle(NSLineCapStyle::Round);
        path.setLineJoinStyle(NSLineJoinStyle::Round);
        color.setStroke();
        path.stroke();
    };
    match &mark.shape {
        Shape::Arrow { from, to } => {
            let [tip, a, b] = arrow_head(*from, *to, mark.width);
            let base = Point::new((a.x + b.x) / 2.0, (a.y + b.y) / 2.0);
            let shaft = NSBezierPath::bezierPath();
            shaft.moveToPoint(ns_point(*from));
            shaft.lineToPoint(ns_point(base));
            stroke(&shaft);
            let head = NSBezierPath::bezierPath();
            head.moveToPoint(ns_point(tip));
            head.lineToPoint(ns_point(a));
            head.lineToPoint(ns_point(b));
            head.closePath();
            color.setFill();
            head.fill();
        }
        Shape::Line { from, to } => {
            let line = NSBezierPath::bezierPath();
            line.moveToPoint(ns_point(*from));
            line.lineToPoint(ns_point(*to));
            stroke(&line);
        }
        Shape::Rectangle(r) => stroke(&NSBezierPath::bezierPathWithRoundedRect_xRadius_yRadius(
            ns_rect(*r),
            3.0,
            3.0,
        )),
        Shape::Ellipse(r) => stroke(&NSBezierPath::bezierPathWithOvalInRect(ns_rect(*r))),
        Shape::Highlight(r) => {
            // Like a marker pen: always see-through, and multiplied so text under it stays dark.
            let Some(context) = NSGraphicsContext::currentContext() else {
                return;
            };
            context.saveGraphicsState();
            context.setCompositingOperation(objc2_app_kit::NSCompositingOperation::Multiply);
            ns_color(mark.color.with_alpha(HIGHLIGHT_ALPHA * 2.0)).setFill();
            NSBezierPath::fillRect(ns_rect(*r));
            context.restoreGraphicsState();
        }
        Shape::Text { at, text, size, .. } => {
            let attrs = text_attrs(*size, mark.color);
            // SAFETY: called while drawing, with valid attributes.
            unsafe {
                NSString::from_str(text).drawAtPoint_withAttributes(ns_point(*at), Some(&attrs))
            };
        }
        Shape::Counter { center, number } => {
            let radius = counter_radius(mark.width);
            let circle = NSRect::new(
                NSPoint::new(center.x - radius, center.y - radius),
                NSSize::new(2.0 * radius, 2.0 * radius),
            );
            color.setFill();
            NSBezierPath::bezierPathWithOvalInRect(circle).fill();
            // SAFETY: extern static weight constant.
            let font = NSFont::monospacedDigitSystemFontOfSize_weight(radius * 1.1, unsafe {
                NSFontWeightBold
            });
            let attrs = text_attributes(&font, &NSColor::whiteColor(), Fit::Clip, true);
            let line = radius * 1.4;
            draw_text(
                &number.to_string(),
                NSRect::new(
                    NSPoint::new(circle.origin.x, center.y - line / 2.0),
                    NSSize::new(2.0 * radius, line),
                ),
                &attrs,
            );
        }
        // Drawn with the picture, below every other mark.
        Shape::Pixelate(_) => {}
    }
}

pub struct CanvasIvars {
    editor: Rc<Editor>,
    on_change: RefCell<Option<Rc<dyn Fn()>>>,
}

define_class!(
    // SAFETY: NSView allows subclassing; the ivars are only touched on the main thread and need no Drop.
    #[unsafe(super(NSView))]
    #[thread_kind = MainThreadOnly]
    #[name = "FigEditorCanvas"]
    #[ivars = CanvasIvars]
    struct Canvas;

    impl Canvas {
        #[unsafe(method(acceptsFirstResponder))]
        fn accepts_first_responder(&self) -> bool {
            true
        }

        #[unsafe(method(drawRect:))]
        fn draw_rect(&self, _dirty: NSRect) {
            self.ivars().editor.draw(false);
        }

        #[unsafe(method(mouseDown:))]
        fn mouse_down(&self, event: &NSEvent) {
            self.commit_text();
            if let Some(window) = self.window() {
                window.makeFirstResponder(Some(self));
            }
            let editor = &self.ivars().editor;
            let p = editor.to_picture(self.convertPoint_fromView(event.locationInWindow(), None));
            match editor.tool.get() {
                Tool::Select => {
                    let hit = editor.doc.borrow().hit(p);
                    editor.selected.set(hit);
                    if let Some(index) = hit {
                        editor.drag.set(Drag::Move { index, start: p, now: p });
                    }
                }
                Tool::Text => self.begin_text(p),
                Tool::Counter => {
                    let number = editor.doc.borrow().next_counter();
                    editor.doc.borrow_mut().add(Mark {
                        shape: Shape::Counter { center: p, number },
                        color: editor.mark_color(),
                        width: editor.width.get(),
                    });
                    self.changed();
                }
                _ => editor.drag.set(Drag::Draw { start: p, now: p }),
            }
            self.setNeedsDisplay(true);
        }

        #[unsafe(method(mouseDragged:))]
        fn mouse_dragged(&self, event: &NSEvent) {
            let editor = &self.ivars().editor;
            let p = editor.to_picture(self.convertPoint_fromView(event.locationInWindow(), None));
            match editor.drag.get() {
                Drag::Draw { start, .. } => editor.drag.set(Drag::Draw { start, now: p }),
                Drag::Move { index, start, .. } => editor.drag.set(Drag::Move { index, start, now: p }),
                Drag::None => return,
            }
            self.setNeedsDisplay(true);
        }

        #[unsafe(method(mouseUp:))]
        fn mouse_up(&self, _event: &NSEvent) {
            let editor = &self.ivars().editor;
            match editor.drag.replace(Drag::None) {
                Drag::Draw { start, now } => {
                    if editor.tool.get() == Tool::Crop {
                        let rect = Rect::from_corners(start, now);
                        if rect.w >= MIN_DRAG && rect.h >= MIN_DRAG {
                            editor.doc.borrow_mut().set_crop(Some(rect));
                            self.relayout();
                        }
                    } else if let Some(shape) = editor.shape_for(start, now) {
                        let index = editor.doc.borrow_mut().add(Mark {
                            shape,
                            color: editor.mark_color(),
                            width: editor.width.get(),
                        });
                        editor.selected.set(Some(index));
                    }
                    self.changed();
                }
                Drag::Move { index, start, now } => {
                    if (now.x - start.x).hypot(now.y - start.y) > 0.5 {
                        editor.doc.borrow_mut().translate(index, now.x - start.x, now.y - start.y);
                        self.changed();
                    }
                }
                Drag::None => {}
            }
            self.setNeedsDisplay(true);
        }

        #[unsafe(method(keyDown:))]
        fn key_down(&self, event: &NSEvent) {
            if !self.handle_key(event) {
                // SAFETY: forwards the same event to NSView's own implementation.
                let _: () = unsafe { msg_send![super(self), keyDown: event] };
            }
        }

        #[unsafe(method(undo:))]
        fn undo(&self, _sender: Option<&AnyObject>) {
            if self.ivars().editor.doc.borrow_mut().undo() {
                self.after_history();
            }
        }

        #[unsafe(method(redo:))]
        fn redo(&self, _sender: Option<&AnyObject>) {
            if self.ivars().editor.doc.borrow_mut().redo() {
                self.after_history();
            }
        }

        /// The text field's action: Return ends the text.
        #[unsafe(method(commitText:))]
        fn commit_text_action(&self, _sender: Option<&AnyObject>) {
            self.commit_text();
        }
    }
);

impl Canvas {
    fn changed(&self) {
        let on_change = self.ivars().on_change.borrow().clone();
        if let Some(on_change) = on_change {
            on_change();
        }
    }

    fn after_history(&self) {
        self.ivars().editor.selected.set(None);
        self.relayout();
        self.changed();
    }

    /// Resizes the canvas to the document's layout (after a crop or a background change).
    fn relayout(&self) {
        let (width, height, _) = self.ivars().editor.doc.borrow().layout();
        self.setFrameSize(NSSize::new(width, height));
        self.setNeedsDisplay(true);
    }

    fn handle_key(&self, event: &NSEvent) -> bool {
        let editor = &self.ivars().editor;
        let flags = event.modifierFlags();
        let command = flags.contains(NSEventModifierFlags::Command);
        let shift = flags.contains(NSEventModifierFlags::Shift);
        let key = event
            .charactersIgnoringModifiers()
            .map(|s| s.to_string().to_lowercase())
            .unwrap_or_default();
        match (event.keyCode(), command) {
            (51 | 117, false) => {
                if let Some(index) = editor.selected.take() {
                    editor.doc.borrow_mut().remove(index);
                    self.changed();
                    self.setNeedsDisplay(true);
                }
                true
            }
            (53, false) => {
                editor.selected.set(None);
                self.setNeedsDisplay(true);
                true
            }
            (6, true) => {
                let done = if shift {
                    editor.doc.borrow_mut().redo()
                } else {
                    editor.doc.borrow_mut().undo()
                };
                if done {
                    self.after_history();
                }
                true
            }
            (_, false) => {
                let tool = match key.as_str() {
                    "v" => Tool::Select,
                    "a" => Tool::Arrow,
                    "l" => Tool::Line,
                    "r" => Tool::Rectangle,
                    "o" => Tool::Ellipse,
                    "h" => Tool::Highlight,
                    "t" => Tool::Text,
                    "n" => Tool::Counter,
                    "p" => Tool::Pixelate,
                    "c" => Tool::Crop,
                    _ => return false,
                };
                editor.tool.set(tool);
                self.changed();
                true
            }
            _ => false,
        }
    }

    fn begin_text(&self, at: Point) {
        let editor = &self.ivars().editor;
        let (_, shift) = editor.placement();
        let size = editor.text_size();
        let mtm = self.mtm();
        let field = NSTextField::initWithFrame(
            NSTextField::alloc(mtm),
            NSRect::new(
                NSPoint::new(at.x + shift.x - 2.0, at.y + shift.y - 4.0),
                NSSize::new(320.0, size * 1.6),
            ),
        );
        // SAFETY: extern static weight constant.
        field.setFont(Some(&NSFont::systemFontOfSize_weight(size, unsafe {
            NSFontWeightBold
        })));
        field.setTextColor(Some(&ns_color(editor.mark_color())));
        field.setBezeled(false);
        field.setDrawsBackground(false);
        field.setFocusRingType(objc2_app_kit::NSFocusRingType::None);
        // SAFETY: `commitText:` is implemented by the canvas, which outlives the field.
        unsafe {
            field.setTarget(Some(self));
            field.setAction(Some(sel!(commitText:)));
        }
        self.addSubview(&field);
        if let Some(window) = self.window() {
            window.makeFirstResponder(Some(&field));
        }
        *editor.typing.borrow_mut() = Some((field, at));
    }

    fn commit_text(&self) {
        let editor = &self.ivars().editor;
        let Some((field, at)) = editor.typing.borrow_mut().take() else {
            return;
        };
        let text = field.stringValue().to_string();
        field.removeFromSuperview();
        if text.trim().is_empty() {
            return;
        }
        let size = editor.text_size();
        let measured = measure(&text, size);
        editor.doc.borrow_mut().add(Mark {
            shape: Shape::Text {
                at,
                text,
                size,
                bounds: Rect::new(at.x, at.y, measured.width, measured.height),
            },
            color: editor.mark_color(),
            width: editor.width.get(),
        });
        self.changed();
        self.setNeedsDisplay(true);
    }
}

define_class!(
    // SAFETY: NSClipView allows subclassing; this one only keeps a small document centred.
    #[unsafe(super(NSClipView))]
    #[thread_kind = MainThreadOnly]
    #[name = "FigCenteringClipView"]
    struct CenteringClip;

    impl CenteringClip {
        #[unsafe(method(constrainBoundsRect:))]
        fn constrain_bounds_rect(&self, proposed: NSRect) -> NSRect {
            // SAFETY: calls NSClipView's own implementation with the same argument.
            let mut rect: NSRect = unsafe { msg_send![super(self), constrainBoundsRect: proposed] };
            if let Some(document) = self.documentView() {
                let size = document.frame().size;
                if rect.size.width > size.width {
                    rect.origin.x = (size.width - rect.size.width) / 2.0;
                }
                if rect.size.height > size.height {
                    rect.origin.y = (size.height - rect.size.height) / 2.0;
                }
            }
            rect
        }
    }
);

pub struct DelegateIvars {
    on_close: RefCell<Option<Box<dyn Fn()>>>,
}

define_class!(
    // SAFETY: NSObject allows subclassing; the ivars are only touched on the main thread and need no Drop.
    #[unsafe(super(NSObject))]
    #[thread_kind = MainThreadOnly]
    #[name = "FigEditorDelegate"]
    #[ivars = DelegateIvars]
    struct EditorDelegate;

    unsafe impl NSObjectProtocol for EditorDelegate {}

    unsafe impl NSWindowDelegate for EditorDelegate {
        #[unsafe(method(windowWillClose:))]
        fn window_will_close(&self, _notification: &NSNotification) {
            if let Some(on_close) = self.ivars().on_close.borrow_mut().take() {
                on_close();
            }
        }
    }
);

/// One open editor; it stays open until the user closes it or presses Done.
pub struct EditorWindow {
    window: Retained<NSWindow>,
    _canvas: Retained<Canvas>,
    _delegate: Retained<EditorDelegate>,
    _targets: Vec<Retained<ActionTarget>>,
}

/// The picture made blocky, drawn inside pixelated marks.
fn pixelated_image(shot: &Shot) -> Retained<NSImage> {
    shot.pixels()
        .and_then(|mut pixels| {
            let block = (PIXEL_BLOCK * shot.scale()).round().max(2.0) as usize;
            pixelate(
                &mut pixels.data,
                pixels.width,
                pixels.height,
                pixels.stride,
                block,
            );
            shot.with_pixels(&pixels)
        })
        .map_or_else(|| shot.ns_image(), |p| p.ns_image())
}

/// A small square of a palette colour, as a segment label.
fn swatch(color: Color) -> Retained<NSImage> {
    let size = NSSize::new(14.0, 14.0);
    let fill = ns_color(color);
    NSImage::imageWithSize_flipped_drawingHandler(
        size,
        false,
        &block2::RcBlock::new(move |rect: NSRect| {
            fill.setFill();
            NSBezierPath::bezierPathWithRoundedRect_xRadius_yRadius(rect, 3.0, 3.0).fill();
            NSColor::colorWithWhite_alpha(0.5, 0.6).setStroke();
            let inner = NSRect::new(
                NSPoint::new(rect.origin.x + 0.5, rect.origin.y + 0.5),
                NSSize::new(rect.size.width - 1.0, rect.size.height - 1.0),
            );
            NSBezierPath::bezierPathWithRoundedRect_xRadius_yRadius(inner, 3.0, 3.0).stroke();
            objc2::runtime::Bool::YES
        }),
    )
}

impl EditorWindow {
    pub fn open(
        mtm: MainThreadMarker,
        shot: &Shot,
        texts: &EditorTexts,
        actions: EditorActions,
    ) -> Rc<Self> {
        Self::build(mtm, shot, texts, actions, true)
    }

    /// The editor without putting it on screen, for checking its layout.
    pub fn hidden(
        mtm: MainThreadMarker,
        shot: &Shot,
        texts: &EditorTexts,
        actions: EditorActions,
    ) -> Rc<Self> {
        Self::build(mtm, shot, texts, actions, false)
    }

    /// A picture of the window's content as it would be drawn.
    pub fn snapshot_png(&self) -> Option<Vec<u8>> {
        let view = self.window.contentView()?;
        let bounds = view.bounds();
        let rep = view.bitmapImageRepForCachingDisplayInRect(bounds)?;
        view.cacheDisplayInRect_toBitmapImageRep(bounds, &rep);
        // SAFETY: an empty properties dictionary is valid for PNG output.
        unsafe {
            rep.representationUsingType_properties(
                objc2_app_kit::NSBitmapImageFileType::PNG,
                &NSDictionary::new(),
            )
        }
        .map(|d| d.to_vec())
    }

    fn build(
        mtm: MainThreadMarker,
        shot: &Shot,
        texts: &EditorTexts,
        actions: EditorActions,
        show: bool,
    ) -> Rc<Self> {
        let size = shot.point_size();
        let pixelated = pixelated_image(shot);
        let editor = Rc::new(Editor {
            doc: RefCell::new(Document::new(size.width, size.height)),
            tool: Cell::new(Tool::Arrow),
            color: Cell::new(0),
            width: Cell::new(WIDTHS[1]),
            gradient: Cell::new(0),
            selected: Cell::new(None),
            drag: Cell::new(Drag::None),
            shot: shot.clone(),
            picture: shot.ns_image(),
            pixelated,
            typing: RefCell::new(None),
        });

        let visible = NSScreen::mainScreen(mtm).map_or(
            NSRect::new(NSPoint::ZERO, NSSize::new(1280.0, 800.0)),
            |s| s.visibleFrame(),
        );
        let frame_size = NSSize::new(
            (size.width + 80.0).clamp(MIN_WIDTH, visible.size.width.max(MIN_WIDTH) * 0.95),
            (size.height + TOOLBAR_HEIGHT + 80.0).clamp(520.0, visible.size.height * 0.9),
        );
        let style = NSWindowStyleMask::Titled
            | NSWindowStyleMask::Closable
            | NSWindowStyleMask::Miniaturizable
            | NSWindowStyleMask::Resizable;
        // SAFETY: designated initializer with valid arguments on the main thread.
        let window = unsafe {
            NSWindow::initWithContentRect_styleMask_backing_defer(
                mtm.alloc(),
                NSRect::new(NSPoint::ZERO, frame_size),
                style,
                NSBackingStoreType::Buffered,
                false,
            )
        };
        // SAFETY: false is the only value compatible with a window kept alive by `Retained`.
        unsafe { window.setReleasedWhenClosed(false) };
        window.setTitle(&NSString::from_str(&texts.title));
        window.setMinSize(NSSize::new(MIN_WIDTH, 420.0));
        let content = window.contentView().expect("content view");
        let bounds = content.bounds();

        let scroll_frame = NSRect::new(
            NSPoint::ZERO,
            NSSize::new(bounds.size.width, bounds.size.height - TOOLBAR_HEIGHT),
        );
        let scroll = NSScrollView::initWithFrame(NSScrollView::alloc(mtm), scroll_frame);
        scroll.setAutoresizingMask(
            objc2_app_kit::NSAutoresizingMaskOptions::ViewWidthSizable
                | objc2_app_kit::NSAutoresizingMaskOptions::ViewHeightSizable,
        );
        scroll.setHasVerticalScroller(true);
        scroll.setHasHorizontalScroller(true);
        scroll.setBorderType(NSBorderType::NoBorder);
        scroll.setBackgroundColor(&NSColor::windowBackgroundColor());
        scroll.setAllowsMagnification(true);
        scroll.setMinMagnification(0.1);
        scroll.setMaxMagnification(8.0);
        // SAFETY: designated initializer with valid arguments on the main thread.
        let clip: Retained<CenteringClip> = unsafe {
            msg_send![super(mtm.alloc::<CenteringClip>().set_ivars(())), initWithFrame: scroll_frame]
        };
        scroll.setContentView(&clip);
        let canvas_ivars = CanvasIvars {
            editor: Rc::clone(&editor),
            on_change: RefCell::new(None),
        };
        // SAFETY: designated initializer with valid arguments on the main thread.
        let canvas: Retained<Canvas> = unsafe {
            msg_send![
                super(mtm.alloc::<Canvas>().set_ivars(canvas_ivars)),
                initWithFrame: NSRect::new(NSPoint::ZERO, size)
            ]
        };
        scroll.setDocumentView(Some(&canvas));
        content.addSubview(&scroll);
        let fit =
            (scroll_frame.size.width / size.width).min(scroll_frame.size.height / size.height);
        if fit < 1.0 {
            scroll.setMagnification(fit * 0.95);
        }

        let actions = Rc::new(actions);
        let window_ref = window.clone();
        let mut targets = Vec::new();
        let top = bounds.size.height;
        let row1 = top - 36.0;
        let row2 = top - 70.0;
        let flexible_bottom = objc2_app_kit::NSAutoresizingMaskOptions::ViewMinYMargin;

        // Copy, save, pin and done take the right end of the first row; the tools take the rest.
        let actions_width = 4.0 * ACTION_WIDTH + 3.0 * ACTION_GAP;
        let tools = NSSegmentedControl::initWithFrame(
            NSSegmentedControl::alloc(mtm),
            NSRect::new(
                NSPoint::new(12.0, row1),
                NSSize::new(bounds.size.width - 24.0 - actions_width - 12.0, 26.0),
            ),
        );
        tools.setSegmentCount(Tool::ALL.len() as isize);
        for (i, label) in texts.tools.iter().enumerate() {
            tools.setLabel_forSegment(&NSString::from_str(label), i as isize);
        }
        tools.setTrackingMode(NSSegmentSwitchTracking::SelectOne);
        tools.setSelectedSegment(1);
        tools.setAutoresizingMask(
            flexible_bottom | objc2_app_kit::NSAutoresizingMaskOptions::ViewWidthSizable,
        );
        {
            let (editor, tools_ref, canvas_ref) =
                (Rc::clone(&editor), tools.clone(), canvas.clone());
            let target = ActionTarget::new(mtm, move || {
                let index = tools_ref.selectedSegment().max(0) as usize;
                canvas_ref.commit_text();
                editor.tool.set(Tool::ALL[index.min(Tool::ALL.len() - 1)]);
                editor.selected.set(None);
                canvas_ref.setNeedsDisplay(true);
            });
            // SAFETY: `fire:` takes the sender as its only argument; the target is kept by the window.
            unsafe {
                tools.setTarget(Some(&target));
                tools.setAction(Some(sel!(fire:)));
            }
            targets.push(target);
        }
        content.addSubview(&tools);

        let colors = NSSegmentedControl::initWithFrame(
            NSSegmentedControl::alloc(mtm),
            NSRect::new(
                NSPoint::new(12.0, row2),
                NSSize::new(PALETTE.len() as f64 * 30.0, 24.0),
            ),
        );
        colors.setSegmentCount(PALETTE.len() as isize);
        for (i, color) in PALETTE.iter().enumerate() {
            colors.setImage_forSegment(Some(&swatch(*color)), i as isize);
            colors.setWidth_forSegment(26.0, i as isize);
        }
        colors.setTrackingMode(NSSegmentSwitchTracking::SelectOne);
        colors.setSelectedSegment(0);
        colors.setAutoresizingMask(flexible_bottom);
        {
            let (editor, colors_ref) = (Rc::clone(&editor), colors.clone());
            let target = ActionTarget::new(mtm, move || {
                editor.color.set(
                    colors_ref
                        .selectedSegment()
                        .clamp(0, PALETTE.len() as isize - 1) as usize,
                );
            });
            // SAFETY: as above.
            unsafe {
                colors.setTarget(Some(&target));
                colors.setAction(Some(sel!(fire:)));
            }
            targets.push(target);
        }
        content.addSubview(&colors);

        let mut x = 12.0 + PALETTE.len() as f64 * 30.0 + 10.0;
        let width_popup = NSPopUpButton::initWithFrame_pullsDown(
            NSPopUpButton::alloc(mtm),
            NSRect::new(NSPoint::new(x, row2 - 1.0), NSSize::new(70.0, 26.0)),
            false,
        );
        for w in WIDTHS {
            width_popup.addItemWithTitle(&NSString::from_str(&format!("{w:.0} px")));
        }
        width_popup.selectItemAtIndex(1);
        width_popup.setAutoresizingMask(flexible_bottom);
        {
            let (editor, popup_ref) = (Rc::clone(&editor), width_popup.clone());
            let target = ActionTarget::new(mtm, move || {
                editor.width.set(
                    WIDTHS[popup_ref
                        .indexOfSelectedItem()
                        .clamp(0, WIDTHS.len() as isize - 1) as usize],
                );
            });
            // SAFETY: as above.
            unsafe {
                width_popup.setTarget(Some(&target));
                width_popup.setAction(Some(sel!(fire:)));
            }
            targets.push(target);
        }
        content.addSubview(&width_popup);
        x += 78.0;

        let make_button = |title: &str, x: f64, width: f64, target: &ActionTarget| {
            let target_object: &AnyObject = target;
            // SAFETY: `fire:` takes the sender as its only argument; the caller keeps the target alive.
            let button = unsafe {
                NSButton::buttonWithTitle_target_action(
                    &NSString::from_str(title),
                    Some(target_object),
                    Some(sel!(fire:)),
                    mtm,
                )
            };
            button.setControlSize(NSControlSize::Regular);
            button.setFrame(NSRect::new(
                NSPoint::new(x, row2 - 2.0),
                NSSize::new(width, 28.0),
            ));
            button.setAutoresizingMask(flexible_bottom);
            button
        };
        for (title, redo) in [(&texts.undo, false), (&texts.redo, true)] {
            let canvas_ref = canvas.clone();
            let target = ActionTarget::new(mtm, move || {
                let editor = &canvas_ref.ivars().editor;
                let done = if redo {
                    editor.doc.borrow_mut().redo()
                } else {
                    editor.doc.borrow_mut().undo()
                };
                if done {
                    canvas_ref.after_history();
                }
            });
            content.addSubview(&make_button(title, x, 84.0, &target));
            targets.push(target);
            x += 88.0;
        }

        let gradient_popup = NSPopUpButton::initWithFrame_pullsDown(
            NSPopUpButton::alloc(mtm),
            NSRect::new(NSPoint::new(x + 74.0, row2 - 1.0), NSSize::new(64.0, 26.0)),
            false,
        );
        for (i, (top_color, _)) in GRADIENTS.iter().enumerate() {
            gradient_popup.addItemWithTitle(&NSString::from_str(&format!("{}", i + 1)));
            if let Some(item) = gradient_popup.itemAtIndex(i as isize) {
                item.setImage(Some(&swatch(*top_color)));
            }
        }
        gradient_popup.setAutoresizingMask(flexible_bottom);
        let background_check = {
            let (canvas_ref, popup_ref) = (canvas.clone(), gradient_popup.clone());
            let slot: Rc<RefCell<Option<Retained<NSButton>>>> = Rc::default();
            let reader = Rc::clone(&slot);
            let target = ActionTarget::new(mtm, move || {
                let editor = &canvas_ref.ivars().editor;
                let on = reader
                    .borrow()
                    .as_ref()
                    .is_some_and(|b| b.state() == NSControlStateValueOn);
                let gradient = popup_ref.indexOfSelectedItem().max(0) as usize;
                editor.gradient.set(gradient);
                editor
                    .doc
                    .borrow_mut()
                    .set_background(on.then_some(Background {
                        gradient,
                        ..Background::default()
                    }));
                canvas_ref.relayout();
            });
            let target_object: &AnyObject = &target;
            // SAFETY: as above.
            let check = unsafe {
                NSButton::checkboxWithTitle_target_action(
                    &NSString::from_str(&texts.background),
                    Some(target_object),
                    Some(sel!(fire:)),
                    mtm,
                )
            };
            check.setFrame(NSRect::new(
                NSPoint::new(x + 6.0, row2 + 2.0),
                NSSize::new(70.0, 22.0),
            ));
            check.setAutoresizingMask(flexible_bottom);
            *slot.borrow_mut() = Some(check.clone());
            // SAFETY: as above.
            unsafe {
                gradient_popup.setTarget(Some(&target));
                gradient_popup.setAction(Some(sel!(fire:)));
            }
            targets.push(target);
            check
        };
        content.addSubview(&background_check);
        content.addSubview(&gradient_popup);

        type Finish = fn(&EditorActions, Shot);
        let finishers: [(&str, Finish, bool); 4] = [
            (&texts.copy, |a, s| (a.on_copy)(s), false),
            (&texts.save, |a, s| (a.on_save)(s), false),
            (&texts.pin, |a, s| (a.on_pin)(s), false),
            (&texts.done, |a, s| (a.on_done)(s), true),
        ];
        let mut right = bounds.size.width - 12.0;
        for (title, act, closes) in finishers.into_iter().rev() {
            let (canvas_ref, actions, window) =
                (canvas.clone(), Rc::clone(&actions), window_ref.clone());
            let target = ActionTarget::new(mtm, move || {
                canvas_ref.commit_text();
                if let Some(shot) = render(&canvas_ref.ivars().editor) {
                    act(&actions, shot);
                }
                if closes {
                    window.performClose(None);
                }
            });
            right -= ACTION_WIDTH;
            let button = make_button(title, right, ACTION_WIDTH, &target);
            button.setFrameOrigin(NSPoint::new(right, row1 - 1.0));
            button.setAutoresizingMask(
                flexible_bottom | objc2_app_kit::NSAutoresizingMaskOptions::ViewMinXMargin,
            );
            if closes {
                button.setKeyEquivalent(&NSString::from_str("\r"));
            }
            content.addSubview(&button);
            targets.push(target);
            right -= ACTION_GAP;
        }

        {
            let tools_ref = tools.clone();
            let editor_ref = Rc::downgrade(&editor);
            *canvas.ivars().on_change.borrow_mut() = Some(Rc::new(move || {
                if let Some(editor) = editor_ref.upgrade() {
                    let index = Tool::ALL
                        .iter()
                        .position(|t| *t == editor.tool.get())
                        .unwrap_or(0);
                    tools_ref.setSelectedSegment(index as isize);
                }
            }));
        }

        let delegate = {
            let actions = Rc::clone(&actions);
            let ivars = DelegateIvars {
                on_close: RefCell::new(Some(Box::new(move || {
                    (actions.on_closed)();
                    leave_regular_when_alone(mtm);
                }))),
            };
            // SAFETY: NSObject's designated initializer.
            let delegate: Retained<EditorDelegate> =
                unsafe { msg_send![super(mtm.alloc::<EditorDelegate>().set_ivars(ivars)), init] };
            delegate
        };
        window.setDelegate(Some(ProtocolObject::from_ref(&*delegate)));
        window.center();
        if show {
            app::set_regular(mtm, true);
            window.makeKeyAndOrderFront(None);
            window.makeFirstResponder(Some(&canvas));
            app::activate(mtm);
        }
        Rc::new(Self {
            window,
            _canvas: canvas,
            _delegate: delegate,
            _targets: targets,
        })
    }

    pub fn close(&self) {
        self.window.performClose(None);
    }
}

/// Back to an accessory app once no titled window is left on screen.
fn leave_regular_when_alone(mtm: MainThreadMarker) {
    app::after(0.0, move || {
        let app_ref = NSApplication::sharedApplication(mtm);
        let others = app_ref
            .windows()
            .iter()
            .any(|w| w.isVisible() && w.styleMask().contains(NSWindowStyleMask::Titled));
        if !others {
            app::set_regular(mtm, false);
        }
    });
}

/// The finished picture at the capture's own resolution.
fn render(editor: &Editor) -> Option<Shot> {
    let (width, height, _) = editor.doc.borrow().layout();
    let scale = editor.shot.scale();
    let (pixels_wide, pixels_high) = (
        (width * scale).round() as isize,
        (height * scale).round() as isize,
    );
    // SAFETY: a plain RGBA bitmap with valid dimensions.
    let rep = unsafe {
        NSBitmapImageRep::initWithBitmapDataPlanes_pixelsWide_pixelsHigh_bitsPerSample_samplesPerPixel_hasAlpha_isPlanar_colorSpaceName_bytesPerRow_bitsPerPixel(
            NSBitmapImageRep::alloc(),
            std::ptr::null_mut(),
            pixels_wide.max(1),
            pixels_high.max(1),
            8,
            4,
            true,
            false,
            NSDeviceRGBColorSpace,
            0,
            0,
        )
    }?;
    // Drawing in points lands on pixels at the capture's scale.
    rep.setSize(NSSize::new(width, height));
    let context = NSGraphicsContext::graphicsContextWithBitmapImageRep(&rep)?;
    NSGraphicsContext::saveGraphicsState_class();
    NSGraphicsContext::setCurrentContext(Some(&context));
    editor.draw(true);
    context.flushGraphics();
    NSGraphicsContext::restoreGraphicsState_class();
    Shot::from_bitmap(&rep, scale)
}

/// Draws a document over a shot without a window, for checks: returns the rendered picture.
pub fn render_document(shot: &Shot, document: Document) -> Option<Shot> {
    let size = shot.point_size();
    debug_assert!((document.width - size.width).abs() < 0.5);
    let editor = Editor {
        doc: RefCell::new(document),
        tool: Cell::new(Tool::Arrow),
        color: Cell::new(0),
        width: Cell::new(WIDTHS[1]),
        gradient: Cell::new(0),
        selected: Cell::new(None),
        drag: Cell::new(Drag::None),
        shot: shot.clone(),
        picture: shot.ns_image(),
        pixelated: pixelated_image(shot),
        typing: RefCell::new(None),
    };
    render(&editor)
}
