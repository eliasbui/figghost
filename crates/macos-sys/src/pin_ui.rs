//! The macOS side of keeping another app's window on top. A program can raise only its own windows, so a pinned
//! window is kept on top by showing a picture of it, taken with ScreenCaptureKit, in a panel above every ordinary
//! window, with a red border round it. While the real window is in front the panel shows only the border and lets
//! every click through; while something covers it the picture shows, refreshed many times a second, and a click on
//! it brings the real window forward. Pictures are taken one at a time rather than as a stream because the system
//! marks a window that a stream is recording by taking over its title bar with an indicator, and a picture leaves it
//! alone. What counts as covered is decided in `core-window::pin` from the list this file reads. All positions are in
//! `window::Frame` coordinates (origin at the top-left of the primary screen, y down).

use crate::{
    reply::{self, Ticket},
    window::Frame,
};
use block2::RcBlock;
use core_foundation::{
    array::CFArray,
    base::{CFType, CFTypeRef, TCFType},
    dictionary::CFDictionary,
    number::CFNumber,
    string::CFString,
};
use objc2::{
    AnyThread, DefinedClass, MainThreadMarker, MainThreadOnly, define_class, msg_send,
    rc::Retained,
    runtime::AnyObject,
};
use objc2_app_kit::{
    NSBackingStoreType, NSColor, NSEvent, NSModalPanelWindowLevel, NSPanel, NSScreen, NSView,
    NSWindowAnimationBehavior, NSWindowCollectionBehavior, NSWindowStyleMask,
};
use objc2_core_foundation::CFRetained;
use objc2_core_graphics::CGImage;
use objc2_foundation::{NSError, NSPoint, NSRect, NSSize};
use objc2_quartz_core::{CALayer, kCAGravityResize};
use objc2_screen_capture_kit::{
    SCCaptureResolutionType, SCContentFilter, SCScreenshotManager, SCShareableContent,
    SCStreamConfiguration, SCWindow,
};
use std::{
    cell::{Cell, RefCell},
    ffi::c_void,
    ptr::NonNull,
    rc::Rc,
    sync::{Mutex, PoisonError},
    time::{Duration, Instant},
};

/// The width of the red border, in points, which lies outside the window.
pub const BORDER: f64 = 3.0;
/// The corners of the border and of the copy, about those of a window.
const RADIUS: f64 = 10.0;

// ---------------------------------------------------------------- the list of windows

#[link(name = "CoreGraphics", kind = "framework")]
unsafe extern "C" {
    fn CGWindowListCopyWindowInfo(option: u32, relative_to_window: u32) -> *const c_void;
}

const ON_SCREEN_ONLY: u32 = 1 << 0;
const EXCLUDE_DESKTOP: u32 = 1 << 4;

/// One window of the system's list.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Listed {
    pub id: u32,
    pub pid: i32,
    pub layer: i32,
    pub frame: Frame,
    pub alpha: f64,
}

fn value(dict: &CFDictionary, key: &str) -> Option<CFType> {
    let key = CFString::new(key);
    let found = dict.find(key.as_concrete_TypeRef().cast::<c_void>())?;
    // SAFETY: the values of the window list are CF objects; the get rule leaves ownership with the dictionary.
    Some(unsafe { CFType::wrap_under_get_rule(*found as CFTypeRef) })
}

fn number(dict: &CFDictionary, key: &str) -> Option<CFNumber> {
    value(dict, key)?.downcast::<CFNumber>()
}

fn read(options: u32) -> Vec<Listed> {
    // SAFETY: a plain call; a non-null result is a new reference to a CFArray, owned from here on.
    let raw = unsafe { CGWindowListCopyWindowInfo(options, 0) };
    if raw.is_null() {
        return Vec::new();
    }
    // SAFETY: the create rule: the array is ours.
    let array: CFArray<CFType> = unsafe { CFArray::wrap_under_create_rule(raw.cast()) };
    array
        .iter()
        .filter_map(|item| {
            let dict = item.downcast::<CFDictionary>()?;
            let bounds = value(&dict, "kCGWindowBounds")?.downcast::<CFDictionary>()?;
            let side = |key: &str| number(&bounds, key)?.to_f64();
            Some(Listed {
                id: u32::try_from(number(&dict, "kCGWindowNumber")?.to_i64()?).ok()?,
                pid: i32::try_from(number(&dict, "kCGWindowOwnerPID")?.to_i64()?).ok()?,
                layer: i32::try_from(number(&dict, "kCGWindowLayer")?.to_i64()?).ok()?,
                frame: Frame {
                    x: side("X")?,
                    y: side("Y")?,
                    w: side("Width")?,
                    h: side("Height")?,
                },
                alpha: number(&dict, "kCGWindowAlpha")
                    .and_then(|a| a.to_f64())
                    .unwrap_or(1.0),
            })
        })
        .collect()
}

/// The windows on the screens, front to back.
pub fn on_screen_windows() -> Vec<Listed> {
    read(ON_SCREEN_ONLY | EXCLUDE_DESKTOP)
}

/// Whether the window server still has this window, on screen or not: a minimised window, or one of a hidden app, is
/// not on screen but is not gone.
pub fn window_exists(id: u32) -> bool {
    read(0).iter().any(|w| w.id == id)
}

fn primary_height(mtm: MainThreadMarker) -> f64 {
    NSScreen::screens(mtm)
        .firstObject()
        .map_or(0.0, |s| s.frame().size.height)
}

// ---------------------------------------------------------------- the panel

define_class!(
    // SAFETY: NSPanel allows subclassing; nothing else is added.
    #[unsafe(super(NSPanel))]
    #[thread_kind = MainThreadOnly]
    #[name = "FigFloatPanel"]
    struct PinPanelClass;

    impl PinPanelClass {
        // A click on the copy never makes this app take the keyboard; the real window gets it.
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

pub struct ViewIvars {
    on_click: RefCell<Option<Rc<dyn Fn()>>>,
}

define_class!(
    // SAFETY: NSView allows subclassing; the ivars are only touched on the main thread and need no Drop.
    #[unsafe(super(NSView))]
    #[thread_kind = MainThreadOnly]
    #[name = "FigFloatView"]
    #[ivars = ViewIvars]
    struct PinView;

    impl PinView {
        #[unsafe(method(isOpaque))]
        fn is_opaque(&self) -> bool {
            false
        }

        // The press that brings the real window forward needs no earlier click to wake this panel.
        #[unsafe(method(acceptsFirstMouse:))]
        fn accepts_first_mouse(&self, _event: Option<&NSEvent>) -> bool {
            true
        }

        #[unsafe(method(mouseDown:))]
        fn mouse_down(&self, _event: &NSEvent) {
            let handler = self.ivars().on_click.borrow().clone();
            if let Some(handler) = handler {
                handler();
            }
        }
    }
);

struct Inner {
    mtm: MainThreadMarker,
    panel: Retained<PinPanelClass>,
    view: Retained<PinView>,
    /// The live copy of the window; the stream draws into it.
    copy: Retained<CALayer>,
    /// The border.
    ring: Retained<CALayer>,
    shown: Cell<bool>,
}

/// The panel that follows one pinned window: a red border round it, and the live copy over it while it is covered.
pub struct PinPanel {
    inner: Rc<Inner>,
}

impl PinPanel {
    /// `on_click` runs when the copy is pressed.
    pub fn new(mtm: MainThreadMarker, on_click: impl Fn() + 'static) -> Self {
        let style = NSWindowStyleMask::Borderless | NSWindowStyleMask::NonactivatingPanel;
        // SAFETY: designated initializer with valid arguments on the main thread.
        let panel: Retained<PinPanelClass> = unsafe {
            msg_send![
                mtm.alloc::<PinPanelClass>(),
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
        panel.setHasShadow(false);
        panel.setFloatingPanel(true);
        panel.setHidesOnDeactivate(false);
        // Over every ordinary window and floating panel, under the Dock, the menu bar and the menus.
        panel.setLevel(NSModalPanelWindowLevel);
        panel.setCollectionBehavior(
            NSWindowCollectionBehavior::CanJoinAllSpaces
                | NSWindowCollectionBehavior::Stationary
                | NSWindowCollectionBehavior::FullScreenAuxiliary
                | NSWindowCollectionBehavior::IgnoresCycle,
        );
        panel.setAnimationBehavior(NSWindowAnimationBehavior::None);
        // Clicks fall through to the real window until the copy shows.
        panel.setIgnoresMouseEvents(true);

        let ivars = ViewIvars {
            on_click: RefCell::new(Some(Rc::new(on_click))),
        };
        // SAFETY: designated initializer with valid arguments on the main thread.
        let view: Retained<PinView> = unsafe {
            msg_send![super(mtm.alloc::<PinView>().set_ivars(ivars)), initWithFrame: NSRect::ZERO]
        };
        view.setWantsLayer(true);
        panel.setContentView(Some(&view));

        let copy = CALayer::new();
        // SAFETY: a constant of Core Animation, read on the main thread.
        copy.setContentsGravity(unsafe { kCAGravityResize });
        copy.setCornerRadius(RADIUS);
        copy.setMasksToBounds(true);
        copy.setHidden(true);
        let ring = CALayer::new();
        ring.setBorderWidth(BORDER);
        ring.setCornerRadius(RADIUS + BORDER);
        ring.setBorderColor(Some(&NSColor::systemRedColor().CGColor()));
        if let Some(root) = view.layer() {
            root.addSublayer(&copy);
            root.addSublayer(&ring);
        }
        Self {
            inner: Rc::new(Inner {
                mtm,
                panel,
                view,
                copy,
                ring,
                shown: Cell::new(false),
            }),
        }
    }

    /// The layer a stream draws the copy into.
    pub fn copy_layer(&self) -> Retained<CALayer> {
        self.inner.copy.clone()
    }

    /// Puts the panel round `frame`, the frame of the pinned window, with the border and, when `mirror`, the live
    /// copy over the window; the copy takes the clicks and the border alone lets them through.
    pub fn show(&self, frame: Frame, mirror: bool, border: bool) {
        let inner = &self.inner;
        let height = primary_height(inner.mtm);
        let outer = NSRect::new(
            NSPoint::new(frame.x - BORDER, height - (frame.y + frame.h) - BORDER),
            NSSize::new(frame.w + 2.0 * BORDER, frame.h + 2.0 * BORDER),
        );
        if inner.panel.frame() != outer {
            inner.panel.setFrame_display(outer, false);
            let bounds = NSRect::new(NSPoint::ZERO, outer.size);
            inner.view.setFrame(bounds);
            inner.ring.setFrame(bounds);
            inner.copy.setFrame(NSRect::new(
                NSPoint::new(BORDER, BORDER),
                NSSize::new(frame.w, frame.h),
            ));
            inner.copy.setContentsScale(inner.panel.backingScaleFactor());
        }
        inner.copy.setHidden(!mirror);
        inner.ring.setHidden(!border);
        inner.panel.setIgnoresMouseEvents(!mirror);
        if !inner.shown.replace(true) {
            inner.panel.orderFrontRegardless();
        }
    }

    pub fn hide(&self) {
        if self.inner.shown.replace(false) {
            self.inner.panel.orderOut(None);
        }
    }

    pub fn is_shown(&self) -> bool {
        self.inner.shown.get()
    }

    /// The window number of the panel, for the list of windows to tell it from others.
    pub fn window_number(&self) -> isize {
        self.inner.panel.windowNumber()
    }
}

impl Drop for PinPanel {
    fn drop(&mut self) {
        self.inner.panel.orderOut(None);
        self.inner.panel.close();
    }
}

// ---------------------------------------------------------------- the picture

fn error_text(error: &NSError) -> String {
    error.localizedDescription().to_string()
}

/// ScreenCaptureKit objects and pictures made on another thread and used on the main one; they are not tied to a
/// thread.
struct Sendable<T>(T);

// SAFETY: see above; the wrapped objects are only moved, and used again on the main thread.
unsafe impl<T> Send for Sendable<T> {}

/// Takes pictures of one window, wherever it is and however it is covered.
pub struct PinShots {
    filter: Retained<SCContentFilter>,
    config: Retained<SCStreamConfiguration>,
    /// A picture is on its way; no second one is asked for meanwhile.
    busy: Rc<Cell<bool>>,
    last: Cell<Option<Instant>>,
}

fn configure(config: &SCStreamConfiguration, points: (f64, f64), scale: f64) {
    // SAFETY: plain setters on a configuration that belongs to this call.
    unsafe {
        config.setWidth((points.0 * scale).round().max(2.0) as usize);
        config.setHeight((points.1 * scale).round().max(2.0) as usize);
        config.setShowsCursor(false);
        config.setIgnoreShadowsSingleWindow(true);
        config.setShouldBeOpaque(false);
        config.setCaptureResolution(SCCaptureResolutionType::Best);
    }
}

impl PinShots {
    /// Prepares to take pictures of window `id`. `done` runs on the main thread with the shots, or with why there
    /// can be none (the window is gone, or Screen Recording is not allowed).
    pub fn prepare(id: u32, done: impl FnOnce(Result<PinShots, String>) + 'static) {
        let ticket: Ticket<Result<Sendable<PinShots>, String>> =
            reply::expect(move |r: Result<Sendable<PinShots>, String>| done(r.map(|s| s.0)));
        let pending = Mutex::new(Some(ticket));
        let handler = RcBlock::new(
            move |content: *mut SCShareableContent, error: *mut NSError| {
                let Some(ticket) = pending.lock().unwrap_or_else(PoisonError::into_inner).take()
                else {
                    return;
                };
                // SAFETY: a non-null content pointer is a live snapshot for the call; retaining keeps it.
                let content =
                    NonNull::new(content).and_then(|c| unsafe { Retained::retain(c.as_ptr()) });
                let Some(content) = content else {
                    // SAFETY: the handler passes a valid NSError or null.
                    let text = unsafe { error.as_ref() }
                        .map_or_else(|| "no shareable content".to_owned(), error_text);
                    ticket.send(Err(text));
                    return;
                };
                ticket.send(build(&content, id).map(Sendable));
            },
        );
        // SAFETY: the handler only touches Send data and is retained by ScreenCaptureKit.
        unsafe {
            SCShareableContent::getShareableContentExcludingDesktopWindows_onScreenWindowsOnly_completionHandler(
                false, false, &handler,
            );
        }
    }

    /// Follows a window that changed its size to `points`, at `scale` pixels to the point.
    pub fn resize(&self, points: (f64, f64), scale: f64) {
        configure(&self.config, points, scale);
    }

    /// Takes a picture into `layer`, unless one is on its way or the last was taken less than `every` ago. Returns
    /// whether one was asked for.
    pub fn capture(&self, layer: &Retained<CALayer>, every: Duration) -> bool {
        if self.busy.get() || self.last.get().is_some_and(|at| at.elapsed() < every) {
            return false;
        }
        self.busy.set(true);
        self.last.set(Some(Instant::now()));
        let (busy, layer) = (Rc::clone(&self.busy), layer.clone());
        let ticket = reply::expect(move |picture: Option<Sendable<CFRetained<CGImage>>>| {
            busy.set(false);
            if let Some(Sendable(picture)) = picture {
                let raw: *const CGImage = &*picture;
                // SAFETY: a CGImage is a valid value for a layer's contents, which keeps it.
                let _: () = unsafe { msg_send![&*layer, setContents: raw.cast::<AnyObject>().cast_mut()] };
            }
        });
        let ticket = Mutex::new(Some(ticket));
        let block = RcBlock::new(move |image: *mut CGImage, _error: *mut NSError| {
            let Some(ticket) = ticket.lock().unwrap_or_else(PoisonError::into_inner).take()
            else {
                return;
            };
            // SAFETY: a non-null image is live for the call; retaining as CF keeps it.
            let picture = NonNull::new(image).map(|p| Sendable(unsafe { CFRetained::retain(p) }));
            ticket.send(picture);
        });
        // SAFETY: a live filter and configuration; the block only moves Send data.
        unsafe {
            SCScreenshotManager::captureImageWithFilter_configuration_completionHandler(
                &self.filter,
                &self.config,
                Some(&block),
            );
        }
        true
    }
}

/// The pictures of one window, made on ScreenCaptureKit's queue from a fresh snapshot of what can be shared.
fn build(content: &SCShareableContent, id: u32) -> Result<PinShots, String> {
    // SAFETY: plain property reads on a live content snapshot; the filter and configuration are made with the
    // documented argument types.
    unsafe {
        let window: Retained<SCWindow> = content
            .windows()
            .iter()
            .find(|w| w.windowID() == id)
            .ok_or("the window is gone")?;
        let filter =
            SCContentFilter::initWithDesktopIndependentWindow(SCContentFilter::alloc(), &window);
        let scale = f64::from(filter.pointPixelScale()).max(1.0);
        let rect = filter.contentRect();
        let config = SCStreamConfiguration::new();
        configure(&config, (rect.size.width, rect.size.height), scale);
        Ok(PinShots {
            filter,
            config,
            busy: Rc::default(),
            last: Cell::new(None),
        })
    }
}

// ---------------------------------------------------------------- the screen

/// The scale of the screen a frame lies on, for the pixels of the copy.
pub fn scale_at(mtm: MainThreadMarker, frame: Frame) -> f64 {
    let height = primary_height(mtm);
    let at = NSPoint::new(frame.x + frame.w / 2.0, height - (frame.y + frame.h / 2.0));
    NSScreen::screens(mtm)
        .iter()
        .find(|s| {
            let f = s.frame();
            at.x >= f.origin.x
                && at.x <= f.origin.x + f.size.width
                && at.y >= f.origin.y
                && at.y <= f.origin.y + f.size.height
        })
        .map_or(2.0, |s| s.backingScaleFactor())
}
