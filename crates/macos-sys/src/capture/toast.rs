//! The small confirmation in the corner of a display after a capture: a thumbnail and two lines of text. It
//! never takes the keyboard or the pointer, fades in and out, and replaces the one before it.

use crate::{
    MainThreadMarker, app,
    panel::FigPanel,
    style::{self, Weight},
    widgets::{Fit, draw_text, text_attributes},
    workspace,
};
use core_capture::Display;
use objc2::{
    AnyThread, DefinedClass, MainThreadOnly, define_class, msg_send,
    rc::Retained,
    runtime::{AnyObject, ProtocolObject},
};
use objc2_app_kit::{
    NSBezierPath, NSColor, NSCompositingOperation, NSFont, NSFontAttributeName,
    NSForegroundColorAttributeName, NSGraphicsContext, NSImage, NSLineBreakMode,
    NSMutableParagraphStyle, NSParagraphStyleAttributeName, NSScreen, NSView, NSWindowSharingType,
};
use objc2_foundation::{
    NSData, NSMutableDictionary, NSNumber, NSObjectProtocol, NSPoint, NSRect, NSSize, NSString,
};
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
    time::Instant,
};

const WIDTH: f64 = 340.0;
const HEIGHT: f64 = 84.0;
const MARGIN: f64 = 18.0;
const RADIUS: f64 = 16.0;
const FADE_IN: f64 = 0.18;
const FADE_OUT: f64 = 0.28;
/// How far the confirmation travels while it appears.
const RISE: f64 = 10.0;
const FRAME_SECONDS: f64 = 1.0 / 60.0;

pub struct Spec {
    pub title: String,
    pub detail: String,
    /// The detail is a file name, which is shortened in the middle to keep its extension; any other text
    /// wraps onto a second line and is cut off at the end.
    pub detail_is_name: bool,
    /// PNG bytes of the picture.
    pub thumbnail: Option<Vec<u8>>,
    /// Where to show it; the main display when absent.
    pub display: Option<Display>,
    pub seconds: f64,
}

pub struct ViewIvars {
    title: String,
    detail: String,
    detail_is_name: bool,
    thumbnail: Option<Retained<NSImage>>,
    opaque: bool,
}

define_class!(
    // SAFETY: NSView allows subclassing; the ivars are only touched on the main thread and need no Drop.
    #[unsafe(super(NSView))]
    #[thread_kind = MainThreadOnly]
    #[name = "FigToastView"]
    #[ivars = ViewIvars]
    struct ToastView;

    unsafe impl NSObjectProtocol for ToastView {}

    impl ToastView {
        #[unsafe(method(isOpaque))]
        fn is_opaque(&self) -> bool {
            false
        }

        #[unsafe(method(drawRect:))]
        fn draw_rect(&self, _dirty: NSRect) {
            let ivars = self.ivars();
            let bounds = self.bounds();
            let ground = if ivars.opaque {
                NSColor::colorWithWhite_alpha(0.11, 1.0)
            } else {
                style::black(0.84)
            };
            style::fill_rounded(bounds, RADIUS, &ground);
            style::stroke_rounded(bounds, RADIUS, &style::white(0.14), 1.0);

            let slot = NSRect::new(NSPoint::new(14.0, 12.0), NSSize::new(100.0, HEIGHT - 24.0));
            if let Some(image) = &ivars.thumbnail {
                draw_thumbnail(image, slot);
            }
            let text_x = if ivars.thumbnail.is_some() { 128.0 } else { 18.0 };
            let width = WIDTH - text_x - 16.0;
            let title = text_attributes(
                &style::font(13.0, Weight::Semibold),
                &style::white(style::TEXT),
                Fit::Tail,
                false,
            );
            draw_text(
                &ivars.title,
                NSRect::new(NSPoint::new(text_x, HEIGHT - 36.0), NSSize::new(width, 18.0)),
                &title,
            );
            let font = style::font(12.0, Weight::Regular);
            let colour = style::white(style::TEXT_SECONDARY);
            let detail = if ivars.detail_is_name {
                text_attributes(&font, &colour, Fit::Middle, false)
            } else {
                word_wrapped(&font, &colour)
            };
            draw_text(
                &ivars.detail,
                NSRect::new(NSPoint::new(text_x, 12.0), NSSize::new(width, HEIGHT - 52.0)),
                &detail,
            );
        }
    }
);

impl ToastView {
    fn new(mtm: MainThreadMarker, frame: NSRect, ivars: ViewIvars) -> Retained<Self> {
        let this = mtm.alloc::<Self>().set_ivars(ivars);
        // SAFETY: designated initializer with valid arguments on the main thread.
        unsafe { msg_send![super(this), initWithFrame: frame] }
    }
}

/// Text that breaks at spaces; what does not fit in the rectangle is cut off.
fn word_wrapped(
    font: &NSFont,
    colour: &NSColor,
) -> Retained<NSMutableDictionary<NSString, AnyObject>> {
    let style = NSMutableParagraphStyle::new();
    style.setLineBreakMode(NSLineBreakMode::ByWordWrapping);
    let attributes = NSMutableDictionary::<NSString, AnyObject>::new();
    // SAFETY: extern statics are constants; the values are valid attribute objects for those keys.
    unsafe {
        attributes.setObject_forKey(font, ProtocolObject::from_ref(NSFontAttributeName));
        attributes.setObject_forKey(
            colour,
            ProtocolObject::from_ref(NSForegroundColorAttributeName),
        );
        attributes.setObject_forKey(
            &style,
            ProtocolObject::from_ref(NSParagraphStyleAttributeName),
        );
    }
    attributes
}

/// The picture fitted into `slot` without distortion, with rounded corners and a thin edge.
fn draw_thumbnail(image: &NSImage, slot: NSRect) {
    let natural = image.size();
    if natural.width <= 0.0 || natural.height <= 0.0 {
        return;
    }
    let fit = (slot.size.width / natural.width).min(slot.size.height / natural.height);
    let size = NSSize::new(natural.width * fit, natural.height * fit);
    let rect = NSRect::new(
        NSPoint::new(
            slot.origin.x + (slot.size.width - size.width) / 2.0,
            slot.origin.y + (slot.size.height - size.height) / 2.0,
        ),
        size,
    );
    NSGraphicsContext::saveGraphicsState_class();
    NSBezierPath::bezierPathWithRoundedRect_xRadius_yRadius(rect, 7.0, 7.0).addClip();
    image.drawInRect_fromRect_operation_fraction(
        rect,
        NSRect::ZERO,
        NSCompositingOperation::SourceOver,
        1.0,
    );
    NSGraphicsContext::restoreGraphicsState_class();
    style::stroke_rounded(rect, 7.0, &style::white(0.22), 1.0);
}

struct Toast {
    panel: Retained<FigPanel>,
    /// Bumped by every new animation, so a timer of an older one does nothing.
    generation: Cell<u64>,
    resting: NSRect,
}

thread_local! {
    static CURRENT: RefCell<Option<Rc<Toast>>> = const { RefCell::new(None) };
}

/// The window number of the confirmation on screen, to leave it out of a capture.
pub fn window_numbers() -> Vec<u32> {
    CURRENT.with(|c| {
        c.borrow()
            .as_ref()
            .and_then(|t| u32::try_from(t.panel.windowNumber()).ok())
            .into_iter()
            .collect()
    })
}

/// The area a corner confirmation may use on `display`, or on the main display.
fn visible_area(mtm: MainThreadMarker, display: Option<&Display>) -> NSRect {
    let key = NSString::from_str("NSScreenNumber");
    let screens = NSScreen::screens(mtm);
    let Some(display) = display else {
        return screens
            .firstObject()
            .map_or(NSRect::ZERO, |s| s.visibleFrame());
    };
    let found = screens.iter().find(|s| {
        s.deviceDescription()
            .objectForKey(&key)
            .and_then(|n| n.downcast_ref::<NSNumber>().map(NSNumber::unsignedIntValue))
            == Some(display.id)
    });
    // A display the system does not list (one that was just unplugged) keeps the frame it was given.
    found.map_or_else(
        || {
            NSRect::new(
                NSPoint::new(display.frame.x, display.frame.y),
                NSSize::new(display.frame.w, display.frame.h),
            )
        },
        |s| s.visibleFrame(),
    )
}

pub fn show(mtm: MainThreadMarker, spec: Spec) {
    if let Some(old) = CURRENT.with(|c| c.borrow_mut().take()) {
        old.generation.set(old.generation.get() + 1);
        old.panel.orderOut(None);
    }
    let area = visible_area(mtm, spec.display.as_ref());
    let resting = NSRect::new(
        NSPoint::new(
            area.origin.x + area.size.width - WIDTH - MARGIN,
            area.origin.y + MARGIN,
        ),
        NSSize::new(WIDTH, HEIGHT),
    );
    let panel = FigPanel::new(mtm, resting);
    panel.setIgnoresMouseEvents(true);
    panel.setSharingType(NSWindowSharingType::None);
    let thumbnail = spec
        .thumbnail
        .as_deref()
        .and_then(|png| NSImage::initWithData(NSImage::alloc(), &NSData::with_bytes(png)));
    let options = workspace::display_options();
    let view = ToastView::new(
        mtm,
        NSRect::new(NSPoint::ZERO, resting.size),
        ViewIvars {
            title: spec.title,
            detail: spec.detail,
            detail_is_name: spec.detail_is_name,
            thumbnail,
            opaque: options.reduce_transparency,
        },
    );
    panel.setContentView(Some(&view));
    let toast = Rc::new(Toast {
        panel,
        generation: Cell::new(0),
        resting,
    });
    CURRENT.with(|c| *c.borrow_mut() = Some(Rc::clone(&toast)));

    if options.reduce_motion {
        toast.panel.setAlphaValue(1.0);
        toast.panel.orderFrontRegardless();
        let hold = toast.clone();
        let generation = hold.generation.get();
        app::after(spec.seconds, move || hold.hide(generation, true));
        return;
    }
    toast.panel.setAlphaValue(0.0);
    toast.panel.orderFrontRegardless();
    let generation = toast.generation.get();
    let (appear, hold) = (toast.clone(), toast.clone());
    let seconds = spec.seconds;
    animate(
        &toast,
        generation,
        FADE_IN,
        style::ease_out,
        move |t| {
            appear.place(t, RISE * (1.0 - t));
        },
        move || {
            let again = hold.clone();
            app::after(seconds, move || again.hide(generation, false));
        },
    );
}

impl Toast {
    fn place(&self, alpha: f64, lowered: f64) {
        self.panel.setAlphaValue(alpha);
        self.panel.setFrameOrigin(NSPoint::new(
            self.resting.origin.x,
            self.resting.origin.y - lowered,
        ));
    }

    fn hide(self: &Rc<Self>, generation: u64, at_once: bool) {
        if self.generation.get() != generation {
            return;
        }
        let next = generation + 1;
        self.generation.set(next);
        let (fade, last) = (self.clone(), self.clone());
        let close = move || {
            last.panel.orderOut(None);
            CURRENT.with(|c| {
                let mut slot = c.borrow_mut();
                if slot.as_ref().is_some_and(|t| Rc::ptr_eq(t, &last)) {
                    slot.take();
                }
            });
        };
        if at_once {
            close();
        } else {
            animate(
                self,
                next,
                FADE_OUT,
                style::ease_out,
                move |t| {
                    fade.panel.setAlphaValue(1.0 - t);
                },
                close,
            );
        }
    }
}

/// Calls `apply` with the progress (0 to 1, shaped by `curve`) every frame for `seconds`, then `finish`.
/// Stops quietly when the toast has moved on to another generation.
fn animate(
    toast: &Rc<Toast>,
    generation: u64,
    seconds: f64,
    curve: fn(f64) -> f64,
    apply: impl Fn(f64) + 'static,
    finish: impl FnOnce() + 'static,
) {
    struct Run {
        toast: Rc<Toast>,
        generation: u64,
        started: Instant,
        seconds: f64,
        curve: fn(f64) -> f64,
        apply: Box<dyn Fn(f64)>,
        finish: RefCell<Option<Box<dyn FnOnce()>>>,
    }
    fn step(run: Rc<Run>) {
        if run.toast.generation.get() != run.generation {
            return;
        }
        let progress = (run.started.elapsed().as_secs_f64() / run.seconds).min(1.0);
        (run.apply)((run.curve)(progress));
        if progress >= 1.0 {
            if let Some(finish) = run.finish.borrow_mut().take() {
                finish();
            }
            return;
        }
        app::after(FRAME_SECONDS, move || step(run));
    }
    step(Rc::new(Run {
        toast: Rc::clone(toast),
        generation,
        started: Instant::now(),
        seconds,
        curve,
        apply: Box::new(apply),
        finish: RefCell::new(Some(Box::new(finish))),
    }));
}

/// The confirmation drawn into PNG bytes without showing anything, for checking the design.
pub fn snapshot_png(mtm: MainThreadMarker, spec: &Spec, opaque: bool) -> Option<Vec<u8>> {
    use objc2_app_kit::NSBitmapImageFileType;
    use objc2_foundation::NSDictionary;
    let thumbnail = spec
        .thumbnail
        .as_deref()
        .and_then(|png| NSImage::initWithData(NSImage::alloc(), &NSData::with_bytes(png)));
    let view = ToastView::new(
        mtm,
        NSRect::new(NSPoint::ZERO, NSSize::new(WIDTH, HEIGHT)),
        ViewIvars {
            title: spec.title.clone(),
            detail: spec.detail.clone(),
            detail_is_name: spec.detail_is_name,
            thumbnail,
            opaque,
        },
    );
    let bounds = view.bounds();
    let rep = view.bitmapImageRepForCachingDisplayInRect(bounds)?;
    view.cacheDisplayInRect_toBitmapImageRep(bounds, &rep);
    // SAFETY: an empty properties dictionary is valid for PNG output.
    let data = unsafe {
        rep.representationUsingType_properties(NSBitmapImageFileType::PNG, &NSDictionary::new())
    }?;
    Some(data.to_vec())
}
