//! Small building blocks shared by the notch pages: text drawing, a page root with a footer hint, and a
//! target object for controls.

use objc2::{
    DefinedClass, MainThreadMarker, MainThreadOnly, define_class, msg_send,
    rc::Retained,
    runtime::{AnyObject, ProtocolObject},
};
use objc2_app_kit::{
    NSColor, NSFont, NSFontAttributeName, NSForegroundColorAttributeName, NSLineBreakMode,
    NSMutableParagraphStyle, NSParagraphStyleAttributeName, NSStringDrawing, NSTextAlignment,
    NSView,
};
use objc2_foundation::{NSMutableDictionary, NSObject, NSPoint, NSRect, NSSize, NSString};

/// What happens to text that does not fit on its line.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Fit {
    Clip,
    /// Ellipsis at the end.
    Tail,
    /// Ellipsis in the middle, which keeps both the start and the extension of a file name.
    Middle,
}

pub(crate) fn text_attributes(
    font: &NSFont,
    color: &NSColor,
    fit: Fit,
    centered: bool,
) -> Retained<NSMutableDictionary<NSString, AnyObject>> {
    let style = NSMutableParagraphStyle::new();
    style.setLineBreakMode(match fit {
        Fit::Clip => NSLineBreakMode::ByClipping,
        Fit::Tail => NSLineBreakMode::ByTruncatingTail,
        Fit::Middle => NSLineBreakMode::ByTruncatingMiddle,
    });
    if centered {
        style.setAlignment(NSTextAlignment::Center);
    }
    let dict = NSMutableDictionary::<NSString, AnyObject>::new();
    // SAFETY: extern statics are constants; the values are valid attribute objects for those keys.
    unsafe {
        dict.setObject_forKey(font, ProtocolObject::from_ref(NSFontAttributeName));
        dict.setObject_forKey(
            color,
            ProtocolObject::from_ref(NSForegroundColorAttributeName),
        );
        dict.setObject_forKey(
            &style,
            ProtocolObject::from_ref(NSParagraphStyleAttributeName),
        );
    }
    dict
}

pub(crate) fn draw_text(
    text: &str,
    rect: NSRect,
    attrs: &NSMutableDictionary<NSString, AnyObject>,
) {
    // SAFETY: called from drawRect: with a valid graphics context; `attrs` holds valid attribute objects.
    unsafe { NSString::from_str(text).drawInRect_withAttributes(rect, Some(attrs)) };
}

pub struct PageRootIvars {
    hint: String,
}

define_class!(
    // SAFETY: NSView allows subclassing; the ivars are only touched on the main thread and need no Drop.
    #[unsafe(super(NSView))]
    #[thread_kind = MainThreadOnly]
    #[name = "FigPageRoot"]
    #[ivars = PageRootIvars]
    pub struct PageRoot;

    impl PageRoot {
        #[unsafe(method(drawRect:))]
        fn draw_rect(&self, _dirty: NSRect) {
            let attrs = text_attributes(
                &NSFont::systemFontOfSize(10.5),
                &NSColor::colorWithWhite_alpha(1.0, 0.38),
                Fit::Clip,
                true,
            );
            let rect = NSRect::new(
                NSPoint::new(0.0, 5.0),
                NSSize::new(self.bounds().size.width, 14.0),
            );
            draw_text(&self.ivars().hint, rect, &attrs);
        }
    }
);

impl PageRoot {
    /// A transparent page that draws `hint` centred along its bottom edge.
    pub fn new(mtm: MainThreadMarker, frame: NSRect, hint: &str) -> Retained<Self> {
        let this = mtm.alloc::<Self>().set_ivars(PageRootIvars {
            hint: hint.to_owned(),
        });
        // SAFETY: designated initializer with valid arguments on the main thread.
        unsafe { msg_send![super(this), initWithFrame: frame] }
    }
}

pub struct ActionIvars {
    action: Box<dyn Fn()>,
}

define_class!(
    // SAFETY: NSObject allows subclassing; the ivars are only touched on the main thread and need no Drop.
    #[unsafe(super(NSObject))]
    #[thread_kind = MainThreadOnly]
    #[name = "FigActionTarget"]
    #[ivars = ActionIvars]
    pub struct ActionTarget;

    impl ActionTarget {
        #[unsafe(method(fire:))]
        fn fire(&self, _sender: Option<&AnyObject>) {
            (self.ivars().action)();
        }
    }
);

impl ActionTarget {
    /// The target of a control's action: `control.setTarget(Some(&target)); control.setAction(sel!(fire:))`.
    /// Controls do not retain their target, so the caller must keep the returned object alive.
    pub fn new(mtm: MainThreadMarker, action: impl Fn() + 'static) -> Retained<Self> {
        let this = mtm.alloc::<Self>().set_ivars(ActionIvars {
            action: Box::new(action),
        });
        // SAFETY: NSObject's designated initializer.
        unsafe { msg_send![super(this), init] }
    }
}
