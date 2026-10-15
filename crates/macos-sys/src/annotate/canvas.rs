//! The views the editor is made of besides the toolbar: the canvas that takes the pointer, the clip view
//! that centres a small canvas, the pill that reports what was done, and the watcher of the text editor.

use super::window::Shared;
use crate::{
    style::{self, Weight, black, fill_rounded, white},
    widgets::{Fit, draw_text, text_attributes},
};
use core_annotate::Point;
use objc2::{DefinedClass, MainThreadMarker, MainThreadOnly, define_class, msg_send, rc::Retained};
use objc2_app_kit::{
    NSClipView, NSCursor, NSEvent, NSEventModifierFlags, NSStringDrawing, NSTextDelegate,
    NSTextViewDelegate, NSView, NSWorkspace,
};
use objc2_foundation::{
    NSNotification, NSObject, NSObjectProtocol, NSPoint, NSRect, NSSize, NSString,
};
use std::{
    cell::{Cell, RefCell},
    rc::Weak,
};

/// What a pointer event is about.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Phase {
    Down,
    Drag,
    Up,
}

pub struct CanvasIvars {
    host: RefCell<Weak<Shared>>,
}

define_class!(
    // SAFETY: NSView allows subclassing; the ivars are only touched on the main thread and need no Drop.
    #[unsafe(super(NSView))]
    #[thread_kind = MainThreadOnly]
    #[name = "FigAnnotateCanvas"]
    #[ivars = CanvasIvars]
    pub struct CanvasView;

    impl CanvasView {
        #[unsafe(method(isFlipped))]
        fn is_flipped(&self) -> bool {
            true
        }

        #[unsafe(method(isOpaque))]
        fn is_opaque(&self) -> bool {
            true
        }

        #[unsafe(method(acceptsFirstResponder))]
        fn accepts_first_responder(&self) -> bool {
            true
        }

        #[unsafe(method(acceptsFirstMouse:))]
        fn accepts_first_mouse(&self, _event: Option<&NSEvent>) -> bool {
            true
        }

        #[unsafe(method(drawRect:))]
        fn draw_rect(&self, _dirty: NSRect) {
            if let Some(host) = self.host() {
                host.paint(self.bounds());
            }
        }

        #[unsafe(method(mouseDown:))]
        fn mouse_down(&self, event: &NSEvent) {
            self.forward(Phase::Down, event);
        }

        #[unsafe(method(mouseDragged:))]
        fn mouse_dragged(&self, event: &NSEvent) {
            self.autoscroll(event);
            self.forward(Phase::Drag, event);
        }

        #[unsafe(method(mouseUp:))]
        fn mouse_up(&self, event: &NSEvent) {
            self.forward(Phase::Up, event);
        }

        #[unsafe(method(resetCursorRects))]
        fn reset_cursor_rects(&self) {
            if let Some(host) = self.host() {
                self.addCursorRect_cursor(self.bounds(), &host.cursor());
            }
        }
    }

    unsafe impl NSObjectProtocol for CanvasView {}
);

impl CanvasView {
    pub fn new(mtm: MainThreadMarker, frame: NSRect) -> Retained<Self> {
        let this = mtm.alloc::<Self>().set_ivars(CanvasIvars {
            host: RefCell::new(Weak::new()),
        });
        // SAFETY: designated initializer with valid arguments on the main thread.
        unsafe { msg_send![super(this), initWithFrame: frame] }
    }

    pub fn set_host(&self, host: Weak<Shared>) {
        *self.ivars().host.borrow_mut() = host;
    }

    fn host(&self) -> Option<std::rc::Rc<Shared>> {
        self.ivars().host.borrow().upgrade()
    }

    fn forward(&self, phase: Phase, event: &NSEvent) {
        let Some(host) = self.host() else {
            return;
        };
        let at = self.convertPoint_fromView(event.locationInWindow(), None);
        let shift = event.modifierFlags().contains(NSEventModifierFlags::Shift);
        host.pointer(
            phase,
            host.to_document(Point::new(at.x, at.y)),
            shift,
            event.clickCount(),
        );
    }
}

define_class!(
    // SAFETY: NSClipView allows subclassing; no ivars.
    #[unsafe(super(NSClipView))]
    #[thread_kind = MainThreadOnly]
    #[name = "FigAnnotateClip"]
    pub struct CenterClip;

    impl CenterClip {
        // A canvas smaller than the window sits in the middle of it instead of in the corner.
        #[unsafe(method(constrainBoundsRect:))]
        fn constrain_bounds_rect(&self, proposed: NSRect) -> NSRect {
            // SAFETY: forwards the same argument to NSClipView's own implementation.
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

impl CenterClip {
    pub fn new(mtm: MainThreadMarker) -> Retained<Self> {
        // SAFETY: NSClipView's designated initializer.
        unsafe { msg_send![mtm.alloc::<Self>(), init] }
    }
}

pub struct ToastIvars {
    text: RefCell<String>,
    error: Cell<bool>,
}

define_class!(
    // SAFETY: NSView allows subclassing; the ivars are only touched on the main thread and need no Drop.
    #[unsafe(super(NSView))]
    #[thread_kind = MainThreadOnly]
    #[name = "FigAnnotateToast"]
    #[ivars = ToastIvars]
    pub struct Toast;

    impl Toast {
        #[unsafe(method(hitTest:))]
        fn hit_test(&self, _point: NSPoint) -> *mut NSView {
            // The pill never takes a click from the canvas under it.
            std::ptr::null_mut()
        }

        #[unsafe(method(drawRect:))]
        fn draw_rect(&self, _dirty: NSRect) {
            let bounds = self.bounds();
            // Solid when the person asked for less transparency.
            let alpha = if NSWorkspace::sharedWorkspace().accessibilityDisplayShouldReduceTransparency() {
                1.0
            } else {
                0.86
            };
            fill_rounded(bounds, bounds.size.height / 2.0, &black(alpha));
            let ink = if self.ivars().error.get() {
                style::Tone::Red.color()
            } else {
                white(style::TEXT)
            };
            let font = style::font(13.0, Weight::Semibold);
            let line = font.ascender() - font.descender() + font.leading();
            draw_text(
                &self.ivars().text.borrow(),
                NSRect::new(
                    NSPoint::new(0.0, (bounds.size.height - line) / 2.0),
                    NSSize::new(bounds.size.width, line),
                ),
                &text_attributes(&font, &ink, Fit::Clip, true),
            );
        }
    }
);

impl Toast {
    pub fn new(mtm: MainThreadMarker) -> Retained<Self> {
        let this = mtm.alloc::<Self>().set_ivars(ToastIvars {
            text: RefCell::default(),
            error: Cell::new(false),
        });
        // SAFETY: designated initializer with valid arguments on the main thread.
        let this: Retained<Self> = unsafe { msg_send![super(this), initWithFrame: NSRect::ZERO] };
        this.setHidden(true);
        this
    }

    /// Shows `text` centred along the bottom of `parent_width`.
    pub fn show(&self, text: &str, error: bool, parent_width: f64) {
        let font = style::font(13.0, Weight::Semibold);
        let attrs = text_attributes(&font, &white(1.0), Fit::Clip, false);
        // SAFETY: the attributes dictionary holds valid attribute objects.
        let width = unsafe { NSString::from_str(text).sizeWithAttributes(Some(&attrs)) }
            .width
            .ceil()
            + 32.0;
        text.clone_into(&mut self.ivars().text.borrow_mut());
        self.ivars().error.set(error);
        self.setFrame(NSRect::new(
            NSPoint::new(((parent_width - width) / 2.0).max(0.0), 18.0),
            NSSize::new(width, 30.0),
        ));
        self.setHidden(false);
        self.setNeedsDisplay(true);
    }

    pub fn hide(&self) {
        self.setHidden(true);
    }
}

pub struct WatcherIvars {
    on_change: Box<dyn Fn()>,
}

define_class!(
    // SAFETY: NSObject allows subclassing; the ivars are only touched on the main thread and need no Drop.
    #[unsafe(super(NSObject))]
    #[thread_kind = MainThreadOnly]
    #[name = "FigAnnotateTextWatcher"]
    #[ivars = WatcherIvars]
    pub struct TextWatcher;

    unsafe impl NSObjectProtocol for TextWatcher {}

    unsafe impl NSTextDelegate for TextWatcher {
        #[unsafe(method(textDidChange:))]
        fn text_did_change(&self, _notification: &NSNotification) {
            (self.ivars().on_change)();
        }
    }

    unsafe impl NSTextViewDelegate for TextWatcher {}
);

impl TextWatcher {
    pub fn new(mtm: MainThreadMarker, on_change: impl Fn() + 'static) -> Retained<Self> {
        let this = mtm.alloc::<Self>().set_ivars(WatcherIvars {
            on_change: Box::new(on_change),
        });
        // SAFETY: NSObject's designated initializer.
        unsafe { msg_send![super(this), init] }
    }
}

pub fn cursor_for(drawing: bool, text: bool) -> Retained<NSCursor> {
    if text {
        NSCursor::IBeamCursor()
    } else if drawing {
        NSCursor::crosshairCursor()
    } else {
        NSCursor::arrowCursor()
    }
}
