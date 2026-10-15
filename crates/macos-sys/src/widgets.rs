//! Small building blocks shared by the notch pages: text drawing, a page root with a footer hint, and a
//! target object for controls.

use crate::style::{self, Tone, fill_capsule, white};
use objc2::{
    AnyThread, DefinedClass, MainThreadMarker, MainThreadOnly, define_class, msg_send,
    rc::Retained,
    runtime::{AnyObject, ProtocolObject},
};
use objc2_app_kit::{
    NSAccessibilitySliderRole, NSBitmapImageFileType, NSBitmapImageRep, NSColor,
    NSDeviceRGBColorSpace, NSEvent, NSFont, NSFontAttributeName, NSForegroundColorAttributeName,
    NSGraphicsContext, NSImage, NSLineBreakMode, NSMutableParagraphStyle,
    NSParagraphStyleAttributeName, NSStringDrawing, NSTextAlignment, NSView,
};
use objc2_foundation::{
    NSCopying, NSData, NSDictionary, NSMutableDictionary, NSNumber, NSObject, NSPoint, NSRect,
    NSSize, NSString,
};
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
};

/// What happens to text that does not fit on its line.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Fit {
    Clip,
    /// Ellipsis at the end.
    Tail,
    /// Ellipsis in the middle, which keeps both the start and the extension of a file name.
    Middle,
    /// Breaks anywhere, for text that has no spaces to break at, such as a command line.
    Wrap,
    /// Breaks between words, for sentences.
    Words,
}

/// A view drawn into PNG bytes, without showing it. For looking at the design and for checks.
pub fn view_png(view: &NSView) -> Option<Vec<u8>> {
    let bounds = view.bounds();
    let rep = view.bitmapImageRepForCachingDisplayInRect(bounds)?;
    view.cacheDisplayInRect_toBitmapImageRep(bounds, &rep);
    // SAFETY: an empty properties dictionary is valid for PNG output.
    let data = unsafe {
        rep.representationUsingType_properties(NSBitmapImageFileType::PNG, &NSDictionary::new())
    }?;
    Some(data.to_vec())
}

/// An SF Symbol in white on a clear background, `points` on its longer side at `scale` pixels per point,
/// as PNG bytes; `None` for a name the system does not know. A web page tints it with a CSS mask.
pub fn symbol_png(name: &str, points: f64, scale: f64) -> Option<Vec<u8>> {
    let image = style::symbol(name, points, &NSColor::whiteColor())?;
    png_at(&image, image.size(), scale)
}

/// Encoded image data (PNG, ICNS or any format NSImage reads) drawn `points` on its longer side at `scale`
/// pixels per point, as PNG bytes; `None` for data that is not an image.
pub fn image_png(data: &[u8], points: f64, scale: f64) -> Option<Vec<u8>> {
    let image = NSImage::initWithData(NSImage::alloc(), &NSData::with_bytes(data))?;
    let natural = image.size();
    let fit = points / natural.width.max(natural.height).max(1.0);
    png_at(
        &image,
        NSSize::new(natural.width * fit, natural.height * fit),
        scale,
    )
}

/// `image` drawn over `size` points of a clear bitmap with `scale` pixels per point, as PNG bytes.
fn png_at(image: &NSImage, size: NSSize, scale: f64) -> Option<Vec<u8>> {
    let wide = (size.width * scale).round() as isize;
    let high = (size.height * scale).round() as isize;
    if wide < 1 || high < 1 {
        return None;
    }
    // SAFETY: a null plane pointer asks AppKit to allocate the pixels; the other arguments describe an
    // ordinary interleaved RGBA bitmap, and the colour space constant lives for the process.
    let rep = unsafe {
        NSBitmapImageRep::initWithBitmapDataPlanes_pixelsWide_pixelsHigh_bitsPerSample_samplesPerPixel_hasAlpha_isPlanar_colorSpaceName_bytesPerRow_bitsPerPixel(
            NSBitmapImageRep::alloc(),
            std::ptr::null_mut(),
            wide,
            high,
            8,
            4,
            true,
            false,
            NSDeviceRGBColorSpace,
            0,
            32,
        )
    }?;
    // Drawing in points lands on pixels at `scale`, so the image picks its sharpest representation.
    rep.setSize(size);
    let context = NSGraphicsContext::graphicsContextWithBitmapImageRep(&rep)?;
    NSGraphicsContext::saveGraphicsState_class();
    NSGraphicsContext::setCurrentContext(Some(&context));
    image.drawInRect(NSRect::new(NSPoint::ZERO, size));
    context.flushGraphics();
    NSGraphicsContext::restoreGraphicsState_class();
    // SAFETY: an empty properties dictionary is valid for PNG output.
    let png = unsafe {
        rep.representationUsingType_properties(NSBitmapImageFileType::PNG, &NSDictionary::new())
    }?;
    Some(png.to_vec())
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
        Fit::Wrap => NSLineBreakMode::ByCharWrapping,
        Fit::Words => NSLineBreakMode::ByWordWrapping,
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

/// Told where a draggable bar was moved: the fraction under the pointer, and whether the button has come up.
type Seek = Rc<dyn Fn(f64, bool)>;

/// How thick a draggable bar's track is, and while it is held.
const TRACK: f64 = 4.0;
const TRACK_HELD: f64 = 8.0;
/// How far one step of VoiceOver's increment or decrement moves a draggable bar.
const ACCESSIBLE_STEP: f64 = 0.05;

pub struct BarIvars {
    progress: Cell<f64>,
    tone: Cell<Tone>,
    /// Set for a bar that can be dragged, with its name for VoiceOver.
    seek: RefCell<Option<(Seek, String)>>,
    /// While the button is down the bar shows the pointer's place, not what it is told.
    held: Cell<bool>,
}

define_class!(
    // SAFETY: NSView allows subclassing; the ivars are only touched on the main thread.
    #[unsafe(super(NSView))]
    #[thread_kind = MainThreadOnly]
    #[name = "FigProgressBar"]
    #[ivars = BarIvars]
    pub struct ProgressBar;

    impl ProgressBar {
        #[unsafe(method(drawRect:))]
        fn draw_rect(&self, _dirty: NSRect) {
            let mut bounds = self.bounds();
            if self.seekable() {
                // A thin track across the middle of a taller view, which is easier to hit; thicker while held.
                let thickness = if self.ivars().held.get() { TRACK_HELD } else { TRACK }
                    .min(bounds.size.height);
                bounds = NSRect::new(
                    NSPoint::new(
                        bounds.origin.x,
                        bounds.origin.y + (bounds.size.height - thickness) / 2.0,
                    ),
                    NSSize::new(bounds.size.width, thickness),
                );
            }
            fill_capsule(bounds, &white(style::CONTROL));
            let progress = self.ivars().progress.get().clamp(0.0, 1.0);
            if progress > 0.0 {
                // Never narrower than a dot, so a bar that has just started is still seen.
                let width = (bounds.size.width * progress).max(bounds.size.height);
                let fill = NSRect::new(bounds.origin, NSSize::new(width, bounds.size.height));
                fill_capsule(fill, &self.ivars().tone.get().color());
            }
        }

        #[unsafe(method(acceptsFirstMouse:))]
        fn accepts_first_mouse(&self, _event: Option<&NSEvent>) -> bool {
            self.seekable()
        }

        #[unsafe(method(mouseDown:))]
        fn mouse_down(&self, event: &NSEvent) {
            if !self.seekable() {
                // SAFETY: forwards the same selector and argument to the superclass.
                let _: () = unsafe { msg_send![super(self), mouseDown: event] };
                return;
            }
            self.ivars().held.set(true);
            self.scrub(event, false);
        }

        #[unsafe(method(mouseDragged:))]
        fn mouse_dragged(&self, event: &NSEvent) {
            if self.ivars().held.get() {
                self.scrub(event, false);
            } else {
                // SAFETY: as above.
                let _: () = unsafe { msg_send![super(self), mouseDragged: event] };
            }
        }

        #[unsafe(method(mouseUp:))]
        fn mouse_up(&self, event: &NSEvent) {
            if self.ivars().held.replace(false) {
                self.scrub(event, true);
            } else {
                // SAFETY: as above.
                let _: () = unsafe { msg_send![super(self), mouseUp: event] };
            }
        }

        // A draggable bar is a slider for VoiceOver and for anything that drives the interface through the
        // accessibility tree.
        #[unsafe(method(isAccessibilityElement))]
        fn is_accessibility_element(&self) -> bool {
            self.seekable()
        }

        #[unsafe(method_id(accessibilityRole))]
        fn accessibility_role(&self) -> Option<Retained<NSString>> {
            // SAFETY: an extern static constant that lives as long as the process.
            self.seekable().then(|| unsafe { NSAccessibilitySliderRole }.copy())
        }

        #[unsafe(method_id(accessibilityLabel))]
        fn accessibility_label(&self) -> Option<Retained<NSString>> {
            let seek = self.ivars().seek.borrow();
            seek.as_ref().map(|(_, name)| NSString::from_str(name))
        }

        #[unsafe(method_id(accessibilityValue))]
        fn accessibility_value(&self) -> Option<Retained<NSNumber>> {
            Some(NSNumber::new_f64(self.ivars().progress.get().clamp(0.0, 1.0)))
        }

        #[unsafe(method(accessibilityPerformIncrement))]
        fn accessibility_increment(&self) -> bool {
            self.step(ACCESSIBLE_STEP)
        }

        #[unsafe(method(accessibilityPerformDecrement))]
        fn accessibility_decrement(&self) -> bool {
            self.step(-ACCESSIBLE_STEP)
        }
    }
);

impl ProgressBar {
    /// A thin capsule that fills from the left; at 0 only the track shows.
    pub fn new(mtm: MainThreadMarker, frame: NSRect) -> Retained<Self> {
        let this = mtm.alloc::<Self>().set_ivars(BarIvars {
            progress: Cell::new(0.0),
            tone: Cell::new(Tone::Blue),
            seek: RefCell::new(None),
            held: Cell::new(false),
        });
        // SAFETY: designated initializer with valid arguments on the main thread.
        unsafe { msg_send![super(this), initWithFrame: frame] }
    }

    /// The progress to show; ignored while the bar is held, which shows the pointer's place.
    pub fn set(&self, progress: f64, tone: Tone) {
        if !self.ivars().held.get() {
            self.ivars().progress.set(progress);
        }
        self.ivars().tone.set(tone);
        self.setNeedsDisplay(true);
    }

    /// Makes the bar draggable, named `name` for VoiceOver: `f` hears the fraction under the pointer while the bar is
    /// dragged, and once more with `true` when the button comes up. The view should then be taller than the track
    /// it draws, so it is easy to hit.
    pub fn set_on_seek(&self, name: &str, f: impl Fn(f64, bool) + 'static) {
        *self.ivars().seek.borrow_mut() = Some((Rc::new(f), name.to_owned()));
        self.setNeedsDisplay(true);
    }

    /// Whether the button is down on the bar.
    pub fn held(&self) -> bool {
        self.ivars().held.get()
    }

    fn seekable(&self) -> bool {
        self.ivars().seek.borrow().is_some()
    }

    fn moved_to(&self, fraction: f64, done: bool) {
        self.ivars().progress.set(fraction);
        self.setNeedsDisplay(true);
        let seek = self
            .ivars()
            .seek
            .borrow()
            .as_ref()
            .map(|(f, _)| Rc::clone(f));
        if let Some(seek) = seek {
            seek(fraction, done);
        }
    }

    fn scrub(&self, event: &NSEvent, done: bool) {
        let point = self.convertPoint_fromView(event.locationInWindow(), None);
        let fraction = (point.x / self.bounds().size.width.max(1.0)).clamp(0.0, 1.0);
        self.moved_to(fraction, done);
    }

    fn step(&self, by: f64) -> bool {
        if !self.seekable() {
            return false;
        }
        self.moved_to((self.ivars().progress.get() + by).clamp(0.0, 1.0), true);
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn decoded(png: &[u8]) -> Retained<NSBitmapImageRep> {
        NSBitmapImageRep::imageRepWithData(&NSData::with_bytes(png)).expect("a valid PNG")
    }

    #[test]
    fn a_symbol_is_white_on_clear_at_the_asked_size() {
        let rep = decoded(&symbol_png("plus", 16.0, 2.0).expect("plus is a symbol"));
        assert_eq!(rep.pixelsWide().max(rep.pixelsHigh()), 32);
        let centre = rep
            .colorAtX_y(rep.pixelsWide() / 2, rep.pixelsHigh() / 2)
            .unwrap();
        assert!(
            centre.alphaComponent() > 0.9 && centre.redComponent() > 0.9,
            "{centre:?}"
        );
        assert!(rep.colorAtX_y(0, 0).unwrap().alphaComponent() < 0.1);
        assert!(symbol_png("figghost.no.such.symbol", 16.0, 2.0).is_none());
    }

    #[test]
    fn image_data_is_drawn_again_at_the_asked_size() {
        let source = symbol_png("plus", 10.0, 1.0).unwrap();
        let rep = decoded(&image_png(&source, 24.0, 2.0).expect("a PNG is an image"));
        assert_eq!(rep.pixelsWide().max(rep.pixelsHigh()), 48);
        assert!(image_png(b"not an image", 24.0, 2.0).is_none());
    }
}
