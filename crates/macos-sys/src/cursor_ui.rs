//! Custom mouse cursors: the system's own cursors are registered again with pictures of the app's choosing,
//! animated by the window server itself (so the pointer moves as smoothly as any other, and costs nothing
//! while it stands still). This is the private WindowServer call that Mousecape and the paid cursor
//! changers use; it needs no permission, and it fails quietly (`apply` returns false) where it is missing.
//!
//! A picture is a `Frames`: a row of square frames of pixel art (`from_sheet`), every picture of a GIF or
//! other file ImageIO reads (`from_file`), or the drawn rainbow arrow (`neon`).

use block2::RcBlock;
use objc2::MainThreadMarker;
use objc2_app_kit::{
    NSApplicationWillTerminateNotification, NSCursor, NSEvent, NSEventMask, NSModalResponseOK,
    NSOpenPanel,
};
use objc2_core_graphics::{CGColorSpace, CGContext};
use objc2_foundation::{NSArray, NSNotification, NSNotificationCenter, NSString};
use std::ffi::{CString, c_void};
use std::path::PathBuf;
use std::ptr::{NonNull, null};
use std::sync::{Mutex, Once};
use std::sync::atomic::{AtomicBool, Ordering};

type Ref = *mut c_void;

#[repr(C)]
#[derive(Clone, Copy)]
struct Point {
    x: f64,
    y: f64,
}
#[repr(C)]
#[derive(Clone, Copy)]
struct Size {
    w: f64,
    h: f64,
}
#[repr(C)]
#[derive(Clone, Copy)]
struct Rect {
    x: f64,
    y: f64,
    w: f64,
    h: f64,
}

const PREMULTIPLIED_LAST: u32 = 1;
const INTERPOLATION_NONE: i32 = 1;
const INTERPOLATION_HIGH: i32 = 3;
const ROUND_JOIN: i32 = 1;
const UTF8: u32 = 0x0800_0100;
/// Pictures of a file beyond this many are ignored, and so are files this large (bytes) or pictures this wide.
const MAX_FRAMES: usize = 64;
const MAX_BYTES: usize = 16 << 20;
const MAX_SIDE: usize = 2048;
/// The side of a drawn frame of the rainbow arrow, in pixels, and how many frames a turn of colour takes.
const NEON_SIDE: usize = 128;
const NEON_FRAMES: usize = 12;

#[link(name = "CoreGraphics", kind = "framework")]
unsafe extern "C" {
    fn CGColorSpaceCreateDeviceRGB() -> Ref;
    fn CGColorSpaceRelease(space: Ref);
    fn CGBitmapContextCreateImage(context: Ref) -> Ref;
    fn CGContextRelease(context: Ref);
    fn CGImageRelease(image: Ref);
    fn CGImageGetWidth(image: Ref) -> usize;
    fn CGImageGetHeight(image: Ref) -> usize;
    fn CGImageCreateWithImageInRect(image: Ref, rect: Rect) -> Ref;
    fn CGContextDrawImage(context: Ref, rect: Rect, image: Ref);
    fn CGContextSetInterpolationQuality(context: Ref, quality: i32);
    fn CGContextSetRGBFillColor(context: Ref, r: f64, g: f64, b: f64, a: f64);
    fn CGContextSetRGBStrokeColor(context: Ref, r: f64, g: f64, b: f64, a: f64);
    fn CGContextSetLineWidth(context: Ref, width: f64);
    fn CGContextSetLineJoin(context: Ref, join: i32);
    fn CGContextBeginPath(context: Ref);
    fn CGContextMoveToPoint(context: Ref, x: f64, y: f64);
    fn CGContextAddLineToPoint(context: Ref, x: f64, y: f64);
    fn CGContextClosePath(context: Ref);
    fn CGContextDrawPath(context: Ref, mode: i32);
    fn CGSMainConnectionID() -> i32;
    fn CGSRegisterCursorWithImages(
        connection: i32,
        name: *const i8,
        global: bool,
        instantly: bool,
        size: Size,
        hotspot: Point,
        frames: usize,
        seconds: f64,
        images: Ref,
        seed: *mut i32,
    ) -> i32;
    fn CGSSetConnectionProperty(connection: i32, target: i32, key: Ref, value: Ref) -> i32;
    fn CoreCursorUnregisterAll(connection: i32) -> i32;
}

// Declared as `picker/sample.rs` does, so that the two agree.
#[link(name = "CoreGraphics", kind = "framework")]
unsafe extern "C-unwind" {
    fn CGBitmapContextCreate(
        data: *mut c_void,
        width: usize,
        height: usize,
        bits_per_component: usize,
        bytes_per_row: usize,
        space: *const CGColorSpace,
        bitmap_info: u32,
    ) -> *mut CGContext;
}

#[link(name = "ImageIO", kind = "framework")]
unsafe extern "C" {
    fn CGImageSourceCreateWithData(data: Ref, options: Ref) -> Ref;
    fn CGImageSourceGetCount(source: Ref) -> usize;
    fn CGImageSourceCreateImageAtIndex(source: Ref, index: usize, options: Ref) -> Ref;
}

#[allow(clashing_extern_declarations)]
#[link(name = "CoreFoundation", kind = "framework")]
unsafe extern "C" {
    static kCFTypeArrayCallBacks: c_void;
    static kCFBooleanTrue: Ref;
    fn CFRelease(object: Ref);
    fn CFDataCreate(allocator: Ref, bytes: *const u8, length: isize) -> Ref;
    fn CFArrayCreate(
        allocator: Ref,
        values: *const Ref,
        count: isize,
        callbacks: *const c_void,
    ) -> Ref;
    fn CFStringCreateWithCString(allocator: Ref, text: *const i8, encoding: u32) -> Ref;
}

/// What the system's own arrow and hand look like (size and hot spot), seen before the first replacement, so that
/// `was_reset` can tell when the window server shows one of them again.
static DEFAULTS: Mutex<Vec<[i32; 4]>> = Mutex::new(Vec::new());

fn signature(cursor: &NSCursor) -> [i32; 4] {
    let size = cursor.image().size();
    let spot = cursor.hotSpot();
    [size.width, size.height, spot.x, spot.y].map(|v| v.round() as i32)
}

/// Called before the first replacement. Cursors left registered by an earlier run are given back first, or the
/// "system" cursors seen here would be those.
fn remember_defaults() {
    let mut defaults = DEFAULTS.lock().unwrap_or_else(|e| e.into_inner());
    if defaults.is_empty() {
        restore();
        *defaults = vec![
            signature(&NSCursor::arrowCursor()),
            signature(&NSCursor::pointingHandCursor()),
        ];
    }
}

/// Whether the pointer shows the system's own arrow or hand again: the app in front sets its cursor whenever the
/// pointer crosses into another part of its window, and that puts the system's back.
#[allow(deprecated)]
pub fn was_reset() -> bool {
    let Some(current) = NSCursor::currentSystemCursor() else {
        return false;
    };
    let seen = signature(&current);
    DEFAULTS
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .contains(&seen)
}

/// Calls `f` when the pointer moves (anywhere but in this app's own windows) and the system's cursor is showing
/// again, at most every 50 ms. Stops when dropped.
pub struct Watch(Option<objc2::rc::Retained<objc2::runtime::AnyObject>>);

impl Watch {
    pub fn new(f: impl Fn() + 'static) -> Self {
        let last = std::cell::Cell::new(std::time::Instant::now());
        let handler = RcBlock::new(move |_: NonNull<NSEvent>| {
            if last.get().elapsed().as_millis() >= 50 {
                last.set(std::time::Instant::now());
                if was_reset() {
                    f();
                }
            }
        });
        Self(NSEvent::addGlobalMonitorForEventsMatchingMask_handler(
            NSEventMask::MouseMoved | NSEventMask::LeftMouseDragged,
            &handler,
        ))
    }
}

impl Drop for Watch {
    fn drop(&mut self) {
        if let Some(token) = self.0.take() {
            // SAFETY: the token came from addGlobalMonitorForEventsMatchingMask and is removed once.
            unsafe { NSEvent::removeMonitor(&token) };
        }
    }
}

/// Whether this process has replaced a cursor and not given it back.
static REPLACED: AtomicBool = AtomicBool::new(false);

/// The cursors that can be replaced.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Arrow,
    /// The hand over a link or a button.
    Link,
}

impl Kind {
    /// The name the window server knows it by (`-[NSCursor _coreCursorType]` is 0 for the arrow, 13 for the hand).
    fn name(self) -> &'static str {
        match self {
            Kind::Arrow => "com.apple.coregraphics.Arrow",
            Kind::Link => "com.apple.cursor.13",
        }
    }
}

/// The pictures of a cursor, kept as images of the system.
pub struct Frames {
    images: Vec<Ref>,
    /// Pixel art is enlarged with hard edges; anything else is smoothed.
    crisp: bool,
}

impl Drop for Frames {
    fn drop(&mut self) {
        for image in self.images.drain(..) {
            // SAFETY: each image was created or retained by this value and is released once.
            unsafe { CGImageRelease(image) };
        }
    }
}

/// An empty RGBA context the size of `width` by `height` pixels, null when there is no memory for it.
///
/// # Safety
/// `space` is a valid colour space.
unsafe fn bitmap(width: usize, height: usize, space: Ref) -> Ref {
    // SAFETY: the caller's promise; null data makes the context own its memory.
    unsafe {
        CGBitmapContextCreate(
            std::ptr::null_mut(),
            width,
            height,
            8,
            0,
            space as *const CGColorSpace,
            PREMULTIPLIED_LAST,
        ) as Ref
    }
}

/// Every picture ImageIO finds in `data`, at most `MAX_FRAMES`.
fn decode(data: &[u8]) -> Vec<Ref> {
    if data.is_empty() || data.len() > MAX_BYTES {
        return Vec::new();
    }
    // SAFETY: the bytes are copied by CFDataCreate; the data and the source are released before returning, and
    // the images keep what they need.
    unsafe {
        let bytes = CFDataCreate(null::<c_void>() as Ref, data.as_ptr(), data.len() as isize);
        if bytes.is_null() {
            return Vec::new();
        }
        let source = CGImageSourceCreateWithData(bytes, std::ptr::null_mut());
        CFRelease(bytes);
        if source.is_null() {
            return Vec::new();
        }
        let count = CGImageSourceGetCount(source).min(MAX_FRAMES);
        let images = (0..count)
            .map(|i| CGImageSourceCreateImageAtIndex(source, i, std::ptr::null_mut()))
            .filter(|image| !image.is_null())
            .collect();
        CFRelease(source);
        images
    }
}

fn side(image: Ref) -> (usize, usize) {
    // SAFETY: the image is valid.
    unsafe { (CGImageGetWidth(image), CGImageGetHeight(image)) }
}

fn release_all(images: Vec<Ref>) {
    drop(Frames {
        images,
        crisp: false,
    });
}

impl Frames {
    /// A PNG that is a row of square frames, each as wide as the picture is tall. `None` when it is not one.
    pub fn from_sheet(png: &[u8]) -> Option<Frames> {
        let mut whole = decode(png);
        let image = whole.pop()?;
        release_all(whole);
        let (w, h) = side(image);
        let usable = h > 0 && w >= h && w <= MAX_SIDE * MAX_FRAMES;
        let images: Vec<Ref> = if usable {
            (0..(w / h).min(MAX_FRAMES))
                .map(|i| {
                    let cell = Rect {
                        x: (i * h) as f64,
                        y: 0.0,
                        w: h as f64,
                        h: h as f64,
                    };
                    // SAFETY: the cell lies inside the image, which is valid.
                    unsafe { CGImageCreateWithImageInRect(image, cell) }
                })
                .filter(|cell| !cell.is_null())
                .collect()
        } else {
            Vec::new()
        };
        // SAFETY: the cells keep the picture alive; this is the one release of the reference taken above.
        unsafe { CGImageRelease(image) };
        (!images.is_empty()).then_some(Frames {
            images,
            crisp: true,
        })
    }

    /// Every picture of a file: the frames of a GIF or of an animated PNG, or the one picture of anything else
    /// ImageIO reads. `None` when nothing can be read or a picture is too large.
    pub fn from_file(data: &[u8]) -> Option<Frames> {
        let images = decode(data);
        if images.is_empty() || images.iter().any(|&i| side(i).0.max(side(i).1) > MAX_SIDE) {
            release_all(images);
            return None;
        }
        Some(Frames {
            images,
            crisp: false,
        })
    }

    /// An arrow whose colour turns through the rainbow, with a white edge and a dark one to read on any ground.
    pub fn neon() -> Frames {
        let images = (0..NEON_FRAMES)
            .filter_map(|i| neon_frame(i as f64 / NEON_FRAMES as f64))
            .collect::<Vec<_>>();
        Frames {
            images,
            crisp: false,
        }
    }

    pub fn len(&self) -> usize {
        self.images.len()
    }

    pub fn is_empty(&self) -> bool {
        self.images.is_empty()
    }

    /// The frames in `order` (indexes into this picture), drawn one above the next into one image `side`
    /// pixels wide, each fitted into a square and centred.
    fn stacked(&self, order: &[usize], side_px: usize) -> Option<Ref> {
        let (n, px) = (order.len(), side_px);
        // SAFETY: the context owns its memory; it and the colour space are released on every path.
        unsafe {
            let space = CGColorSpaceCreateDeviceRGB();
            let context = bitmap(px, px * n, space);
            CGColorSpaceRelease(space);
            if context.is_null() {
                return None;
            }
            let quality = if self.crisp {
                INTERPOLATION_NONE
            } else {
                INTERPOLATION_HIGH
            };
            CGContextSetInterpolationQuality(context, quality);
            for (row, &index) in order.iter().enumerate() {
                let Some(&image) = self.images.get(index) else {
                    continue;
                };
                let (w, h) = side(image);
                let scale = px as f64 / w.max(h).max(1) as f64;
                let (dw, dh) = (w as f64 * scale, h as f64 * scale);
                // The first frame goes at the top of the image, which is the high end of the context.
                let top = ((n - 1 - row) * px) as f64;
                let rect = Rect {
                    x: (px as f64 - dw) / 2.0,
                    y: top + (px as f64 - dh) / 2.0,
                    w: dw,
                    h: dh,
                };
                CGContextDrawImage(context, rect, image);
            }
            let stacked = CGBitmapContextCreateImage(context);
            CGContextRelease(context);
            (!stacked.is_null()).then_some(stacked)
        }
    }
}

/// One frame of the rainbow arrow, `phase` of a turn through the hues.
fn neon_frame(phase: f64) -> Option<Ref> {
    let px = NEON_SIDE;
    let (r, g, b) = hsv(phase, 0.85, 1.0);
    // The arrow in a unit square, y down, its tip at the top left.
    const OUTLINE: [(f64, f64); 7] = [
        (0.0, 0.0),
        (0.0, 0.80),
        (0.20, 0.62),
        (0.34, 0.94),
        (0.48, 0.88),
        (0.35, 0.57),
        (0.60, 0.57),
    ];
    let (margin, span) = (14.0, px as f64 - 28.0);
    // SAFETY: the context and the colour space are released on every path; the drawing calls only take it.
    unsafe {
        let space = CGColorSpaceCreateDeviceRGB();
        let context = bitmap(px, px, space);
        CGColorSpaceRelease(space);
        if context.is_null() {
            return None;
        }
        let path = |context: Ref| {
            CGContextBeginPath(context);
            for (i, (x, y)) in OUTLINE.iter().enumerate() {
                let (x, y) = (margin + x * span, px as f64 - (margin + y * span));
                if i == 0 {
                    CGContextMoveToPoint(context, x, y);
                } else {
                    CGContextAddLineToPoint(context, x, y);
                }
            }
            CGContextClosePath(context);
        };
        CGContextSetLineJoin(context, ROUND_JOIN);
        CGContextSetLineWidth(context, 13.0);
        CGContextSetRGBStrokeColor(context, 0.0, 0.0, 0.0, 0.55);
        path(context);
        CGContextDrawPath(context, 1);
        CGContextSetLineWidth(context, 7.0);
        CGContextSetRGBStrokeColor(context, 1.0, 1.0, 1.0, 1.0);
        CGContextSetRGBFillColor(context, r, g, b, 1.0);
        path(context);
        CGContextDrawPath(context, 4);
        let image = CGBitmapContextCreateImage(context);
        CGContextRelease(context);
        (!image.is_null()).then_some(image)
    }
}

/// Hue `h` of a turn (0 to 1), saturation and value to red, green and blue.
fn hsv(h: f64, s: f64, v: f64) -> (f64, f64, f64) {
    let h = h.rem_euclid(1.0) * 6.0;
    let (i, f) = (h.floor() as i32, h.fract());
    let (p, q, t) = (v * (1.0 - s), v * (1.0 - s * f), v * (1.0 - s * (1.0 - f)));
    match i {
        0 => (v, t, p),
        1 => (q, v, p),
        2 => (p, v, t),
        3 => (p, q, v),
        4 => (t, p, v),
        _ => (v, p, q),
    }
}

/// Replaces the system cursor `kind` with `frames` in `order`, shown `size` points wide, `seconds` per frame,
/// with the point that clicks at `hotspot` (fractions of the width and height from the top left). False when
/// the window server refused or the order names no frame.
pub fn apply(
    kind: Kind,
    frames: &Frames,
    order: &[usize],
    size: f64,
    hotspot: (f64, f64),
    seconds: f64,
) -> bool {
    if order.is_empty() || order.iter().any(|&i| i >= frames.len()) {
        return false;
    }
    let size = size.clamp(8.0, 128.0);
    remember_defaults();
    let Some(stacked) = frames.stacked(order, (size * 2.0).round() as usize) else {
        return false;
    };
    let name = CString::new(kind.name()).expect("a name without a null byte");
    let mut seed = 0;
    // SAFETY: every pointer is valid for the calls; the array and the image are released after the window
    // server has copied them, and the connection is this process's own.
    let result = unsafe {
        let connection = CGSMainConnectionID();
        // Without this the window server shows the cursor of the app in front, whatever this one registered.
        let key = CFStringCreateWithCString(
            std::ptr::null_mut(),
            c"SetsCursorInBackground".as_ptr(),
            UTF8,
        );
        CGSSetConnectionProperty(connection, connection, key, kCFBooleanTrue);
        CFRelease(key);
        let array = CFArrayCreate(
            std::ptr::null_mut(),
            &stacked,
            1,
            &raw const kCFTypeArrayCallBacks,
        );
        let result = CGSRegisterCursorWithImages(
            connection,
            name.as_ptr(),
            true,
            true,
            Size { w: size, h: size },
            Point {
                x: hotspot.0.clamp(0.0, 1.0) * size,
                y: hotspot.1.clamp(0.0, 1.0) * size,
            },
            order.len(),
            seconds.max(0.02),
            array,
            &mut seed,
        );
        CFRelease(array);
        CGImageRelease(stacked);
        result
    };
    let done = result == 0;
    if done {
        REPLACED.store(true, Ordering::Relaxed);
        restore_at_quit();
    }
    done
}

/// Gives every cursor this process replaced back to the system.
pub fn restore() {
    // SAFETY: the connection is this process's own.
    unsafe { CoreCursorUnregisterAll(CGSMainConnectionID()) };
    REPLACED.store(false, Ordering::Relaxed);
}

/// The window server keeps a registered cursor after the process that registered it is gone, so the app
/// gives them back when it quits (a crash or a kill leaves them until `restore` or the next log in).
fn restore_at_quit() {
    static ONCE: Once = Once::new();
    ONCE.call_once(|| {
        let block = RcBlock::new(|_: NonNull<NSNotification>| {
            if REPLACED.load(Ordering::Relaxed) {
                restore();
            }
        });
        // SAFETY: the block is 'static and the notification is posted on the main thread; the observer lives
        // as long as the process, so it is leaked on purpose.
        let observer = unsafe {
            NSNotificationCenter::defaultCenter().addObserverForName_object_queue_usingBlock(
                Some(NSApplicationWillTerminateNotification),
                None,
                None,
                &block,
            )
        };
        std::mem::forget(observer);
    });
}

/// Lets the user choose a picture from disk. Blocks until the dialog closes; `None` if cancelled.
pub fn choose_file(mtm: MainThreadMarker, prompt: &str) -> Option<PathBuf> {
    let panel = NSOpenPanel::openPanel(mtm);
    panel.setCanChooseFiles(true);
    panel.setCanChooseDirectories(false);
    panel.setAllowsMultipleSelection(false);
    panel.setMessage(Some(&NSString::from_str(prompt)));
    let kinds: Vec<_> = ["png", "gif", "apng", "webp", "heic", "jpg", "jpeg", "tiff"]
        .map(NSString::from_str)
        .into();
    #[allow(deprecated)]
    panel.setAllowedFileTypes(Some(&NSArray::from_retained_slice(&kinds)));
    if panel.runModal() != NSModalResponseOK {
        return None;
    }
    panel
        .URLs()
        .firstObject()?
        .path()
        .map(|p| PathBuf::from(p.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hues_walk_the_rainbow() {
        assert_eq!(hsv(0.0, 1.0, 1.0), (1.0, 0.0, 0.0));
        let (r, g, b) = hsv(1.0 / 3.0, 1.0, 1.0);
        assert!(r < 1e-9 && (g - 1.0).abs() < 1e-9 && b < 1e-9);
        assert_eq!(hsv(1.0, 1.0, 1.0), hsv(0.0, 1.0, 1.0));
        assert_eq!(hsv(0.5, 0.0, 0.4), (0.4, 0.4, 0.4));
    }

    #[test]
    fn what_is_not_a_picture_makes_no_frames() {
        assert!(Frames::from_file(b"not a picture").is_none());
        assert!(Frames::from_file(&[]).is_none());
        assert!(Frames::from_sheet(b"nothing").is_none());
    }

    #[test]
    fn the_arrow_is_drawn_in_every_frame() {
        let frames = Frames::neon();
        assert_eq!(frames.len(), NEON_FRAMES);
        let order: Vec<usize> = (0..3).collect();
        assert!(frames.stacked(&order, 64).is_some());
        assert!(
            frames.stacked(&[99], 64).is_some(),
            "a missing frame is left blank, not fatal"
        );
        assert!(!apply(Kind::Arrow, &frames, &[], 32.0, (0.0, 0.0), 0.1));
        assert!(!apply(Kind::Arrow, &frames, &[99], 32.0, (0.0, 0.0), 0.1));
    }
}
