//! Presenting: the live picture of a display, of an area of one, of one window or of every window of one app, shown
//! for an audience full screen on another display, or in a window of its own that a video call can share. The picture
//! comes from ScreenCaptureKit and goes into a layer frame by frame without being copied. Over it the app can show a
//! part of the picture larger, darken all but a circle around the pointer, ring the pointer, ripple where it clicks,
//! cover the picture with black or white, or hold it still. This app's own windows are never in the picture.

use crate::{
    app,
    pin_ui::on_screen_windows,
    reply::{self, Ticket},
    style::Tone,
};
use block2::RcBlock;
use dispatch2::{DispatchQueue, DispatchRetained};
use objc2::{
    AnyThread, DefinedClass, MainThreadMarker, MainThreadOnly, define_class, msg_send,
    rc::Retained,
    runtime::{AnyObject, ProtocolObject},
};
use objc2_app_kit::{
    NSBackingStoreType, NSColor, NSNormalWindowLevel, NSPanel, NSPopUpMenuWindowLevel, NSScreen,
    NSStatusWindowLevel, NSView, NSWindow, NSWindowCollectionBehavior, NSWindowStyleMask,
    NSWindowWillCloseNotification,
};
use objc2_core_foundation::{CFRetained, CGPoint, CGRect, CGSize};
use objc2_core_graphics::{CGColor, CGMutablePath};
use objc2_core_media::{CMSampleBuffer, CMTime};
use objc2_foundation::{
    NSArray, NSError, NSNotification, NSNotificationCenter, NSNumber, NSObject, NSObjectProtocol,
    NSPoint, NSRect, NSSize, NSString,
};
use objc2_quartz_core::{
    CAAnimationGroup, CABasicAnimation, CALayer, CAMediaTiming, CAShapeLayer, CATransaction,
    kCAFillRuleEvenOdd, kCAGravityResize,
};
use objc2_screen_capture_kit::{
    SCCaptureResolutionType, SCContentFilter, SCDisplay, SCFrameStatus, SCRunningApplication,
    SCShareableContent, SCStream, SCStreamConfiguration, SCStreamDelegate, SCStreamFrameInfoStatus,
    SCStreamOutput, SCStreamOutputType, SCWindow,
};
use std::{
    cell::{Cell, RefCell},
    collections::HashMap,
    ffi::c_void,
    ptr::NonNull,
    rc::{Rc, Weak},
    sync::{
        Arc, Mutex, PoisonError,
        atomic::{AtomicBool, Ordering},
    },
    time::{Duration, Instant},
};

/// 32-bit BGRA, which a layer shows as it is.
const PIXEL_FORMAT: u32 = u32::from_be_bytes(*b"BGRA");
/// The picture is asked for no larger than this on its long side: enough to zoom into, light enough for 60 frames a
/// second.
const MOST_PIXELS: f64 = 4096.0;
/// The window of the window stage, on its long side, at first.
const WINDOW_SIDE: f64 = 960.0;
/// How often the frame of a window that is shown is looked at again, to place the pointer in it.
const WINDOW_LOOK: Duration = Duration::from_millis(250);
/// The width of the frame around what is shown, on the presenter's screen.
const BORDER: f64 = 3.0;
/// The ring around the pointer and the ripple of a click, as a share of the stage's height, and their limits.
const RING_SHARE: f64 = 0.032;
const RIPPLE_SECONDS: f64 = 0.45;

#[link(name = "CoreVideo", kind = "framework")]
unsafe extern "C" {
    fn CVPixelBufferGetIOSurface(pixel_buffer: *const c_void) -> *mut c_void;
}

/// What is presented.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Source {
    /// A whole display, by its `CGDirectDisplayID`.
    Display(u32),
    /// An area of a display, in the display's points with the origin at its bottom-left, as the overlay reports it.
    Area { display: u32, rect: NSRect },
    /// One window, by its number, wherever it goes and whatever covers it.
    Window(u32),
    /// Every window of the app that owns this window, on the display that window is on.
    App(u32),
}

/// Where the audience looks.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Stage {
    /// Full screen on a display, by its `CGDirectDisplayID`.
    Display(u32),
    /// A window of its own, which can be moved, resized and shared in a call.
    Window,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Options {
    pub fps: u32,
    /// The pointer is in the picture.
    pub cursor: bool,
    /// A frame around the area or display that is shown, on the presenter's screen.
    pub border: bool,
    /// The title of the window of the window stage.
    pub title: String,
}

/// What covers the picture.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum Cover {
    #[default]
    None,
    Black,
    White,
}

/// What the audience sees besides the picture. Positions are fractions of the picture from its bottom-left.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Look {
    /// The part of the picture that fills the stage: x, y, width, height.
    pub view: (f64, f64, f64, f64),
    /// Where the pointer is in the picture, while it is in it.
    pub pointer: Option<(f64, f64)>,
    pub ring: bool,
    /// The radius of the circle the spotlight leaves lit, as a share of the stage's height.
    pub spotlight: Option<f64>,
    pub cover: Cover,
}

impl Default for Look {
    fn default() -> Self {
        Self {
            view: (0.0, 0.0, 1.0, 1.0),
            pointer: None,
            ring: false,
            spotlight: None,
            cover: Cover::None,
        }
    }
}

/// Why a presentation ended by itself: the window of the window stage was closed (`None`), or the picture stopped
/// coming, with what the system said.
pub type Ended = Option<String>;

// ---------------------------------------------------------------- the frames

/// An IOSurface, which is safe to hand between threads.
struct Surface(Retained<AnyObject>);

// SAFETY: IOSurface objects are thread-safe; the surface is only moved, then set as a layer's contents on the main
// thread.
unsafe impl Send for Surface {}

/// What the frames' queue and the main thread share.
struct Shared {
    id: u64,
    /// The newest frame not yet shown: a slow main thread skips frames instead of falling behind.
    latest: Mutex<Option<Surface>>,
    scheduled: AtomicBool,
    frozen: AtomicBool,
    ended: Mutex<Option<Ticket<Ended>>>,
}

impl Shared {
    fn end(&self, why: Ended) {
        if let Some(ticket) = self
            .ended
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .take()
        {
            ticket.send(why);
        }
    }

    /// A frame arrived on the frames' queue: it becomes the newest, and the main thread is asked to show it unless it
    /// was already asked.
    fn arrived(self: &Arc<Self>, surface: Surface) {
        if self.frozen.load(Ordering::Relaxed) {
            return;
        }
        *self.latest.lock().unwrap_or_else(PoisonError::into_inner) = Some(surface);
        if self.scheduled.swap(true, Ordering::AcqRel) {
            return;
        }
        let shared = Arc::clone(self);
        app::on_main(move || {
            shared.scheduled.store(false, Ordering::Release);
            let surface = shared
                .latest
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .take();
            if let Some(surface) = surface {
                STAGES.with(|stages| {
                    if let Some(inner) = stages.borrow().get(&shared.id).and_then(Weak::upgrade) {
                        inner.show_frame(&surface.0);
                    }
                });
            }
        });
    }
}

/// The surface of a complete frame; `None` for a frame that only says nothing changed.
fn surface_of(sample: &CMSampleBuffer) -> Option<Surface> {
    // SAFETY: the attachments of a sample buffer from ScreenCaptureKit are an array of dictionaries keyed by
    // SCStreamFrameInfo strings, toll-free bridged to NSArray and NSDictionary; the reads are plain messages.
    unsafe {
        let attachments = sample.sample_attachments_array(false)?;
        let array = CFRetained::as_ptr(&attachments)
            .as_ptr()
            .cast::<NSArray<AnyObject>>();
        let info = (*array).firstObject()?;
        let status: Option<Retained<AnyObject>> =
            msg_send![&*info, objectForKey: SCStreamFrameInfoStatus];
        let status = status?.downcast::<NSNumber>().ok()?.integerValue();
        if status != SCFrameStatus::Complete.0 {
            return None;
        }
        let image = sample.image_buffer()?;
        let surface = CVPixelBufferGetIOSurface(CFRetained::as_ptr(&image).as_ptr().cast());
        // The get rule: retaining keeps the surface after the sample buffer is let go.
        Retained::retain(surface.cast::<AnyObject>()).map(Surface)
    }
}

pub struct OutputIvars {
    shared: Arc<Shared>,
}

define_class!(
    // SAFETY: NSObject allows subclassing. ScreenCaptureKit calls the output on the frames' queue and the delegate on
    // its own; the ivars are an Arc of thread-safe parts.
    #[unsafe(super(NSObject))]
    #[name = "FigPresenterOutput"]
    #[ivars = OutputIvars]
    struct FrameOutput;

    unsafe impl NSObjectProtocol for FrameOutput {}

    unsafe impl SCStreamOutput for FrameOutput {
        #[unsafe(method(stream:didOutputSampleBuffer:ofType:))]
        fn did_output(
            &self,
            _stream: &SCStream,
            sample: &CMSampleBuffer,
            kind: SCStreamOutputType,
        ) {
            if kind != SCStreamOutputType::Screen {
                return;
            }
            if let Some(surface) = surface_of(sample) {
                self.ivars().shared.arrived(surface);
            }
        }
    }

    unsafe impl SCStreamDelegate for FrameOutput {
        #[unsafe(method(stream:didStopWithError:))]
        fn did_stop(&self, _stream: &SCStream, error: &NSError) {
            self.ivars()
                .shared
                .end(Some(error.localizedDescription().to_string()));
        }
    }
);

/// ScreenCaptureKit's snapshots of what is on screen are immutable, safe to move to the main thread.
struct Sendable<T>(T);

// SAFETY: see above; only SCShareableContent snapshots are wrapped.
unsafe impl<T> Send for Sendable<T> {}

thread_local! {
    /// The presentations running, by id, for the frames that hop to the main thread.
    static STAGES: RefCell<HashMap<u64, Weak<Inner>>> = RefCell::new(HashMap::new());
    static NEXT: Cell<u64> = const { Cell::new(1) };
}

// ---------------------------------------------------------------- the stage

pub struct StageIvars {
    on_resize: RefCell<Option<Box<dyn Fn()>>>,
}

define_class!(
    // SAFETY: NSView allows subclassing; the ivars are only touched on the main thread and need no Drop.
    #[unsafe(super(NSView))]
    #[thread_kind = MainThreadOnly]
    #[name = "FigPresenterStage"]
    #[ivars = StageIvars]
    struct StageView;

    impl StageView {
        #[unsafe(method(setFrameSize:))]
        fn set_frame_size(&self, size: NSSize) {
            // SAFETY: the superclass's own setter with the same argument.
            let _: () = unsafe { msg_send![super(self), setFrameSize: size] };
            if let Some(on_resize) = self.ivars().on_resize.borrow().as_ref() {
                on_resize();
            }
        }
    }
);

impl StageView {
    fn new(mtm: MainThreadMarker, frame: NSRect) -> Retained<Self> {
        let this = mtm.alloc::<Self>().set_ivars(StageIvars {
            on_resize: RefCell::default(),
        });
        // SAFETY: designated initializer with valid arguments on the main thread.
        let view: Retained<Self> = unsafe { msg_send![super(this), initWithFrame: frame] };
        view.setWantsLayer(true);
        view
    }
}

/// A frame drawn around what is shown, on the presenter's screen.
fn border_view(mtm: MainThreadMarker, frame: NSRect) -> Retained<NSView> {
    let view = NSView::initWithFrame(mtm.alloc(), frame);
    view.setWantsLayer(true);
    if let Some(layer) = view.layer() {
        layer.setBorderWidth(BORDER);
        layer.setBorderColor(Some(&Tone::Green.color().CGColor()));
        layer.setCornerRadius(BORDER * 2.0);
    }
    view
}

struct Layers {
    clip: Retained<CALayer>,
    content: Retained<CALayer>,
    spot: Retained<CAShapeLayer>,
    ring: Retained<CAShapeLayer>,
    ripple: Retained<CAShapeLayer>,
    cover: Retained<CALayer>,
}

fn color(white: f64, alpha: f64) -> CFRetained<CGColor> {
    CGColor::new_srgb(white, white, white, alpha)
}

/// Changes made at once, with every implicit animation turned off.
fn instantly(changes: impl FnOnce()) {
    CATransaction::begin();
    CATransaction::setDisableActions(true);
    changes();
    CATransaction::commit();
}

impl Layers {
    fn new(root: &CALayer) -> Self {
        let clip = CALayer::new();
        clip.setMasksToBounds(true);
        clip.setBackgroundColor(Some(&color(0.0, 1.0)));
        let content = CALayer::new();
        // SAFETY: an extern static constant that lives as long as the process.
        content.setContentsGravity(unsafe { kCAGravityResize });
        let spot = CAShapeLayer::layer();
        // SAFETY: an extern static constant that lives as long as the process.
        spot.setFillRule(unsafe { kCAFillRuleEvenOdd });
        spot.setFillColor(Some(&color(0.0, 0.62)));
        spot.setHidden(true);
        let ring = CAShapeLayer::layer();
        ring.setFillColor(Some(&CGColor::new_srgb(1.0, 0.84, 0.04, 0.22)));
        ring.setStrokeColor(Some(&CGColor::new_srgb(1.0, 0.84, 0.04, 0.9)));
        ring.setLineWidth(3.0);
        ring.setHidden(true);
        let ripple = CAShapeLayer::layer();
        ripple.setFillColor(None);
        ripple.setStrokeColor(Some(&CGColor::new_srgb(1.0, 0.84, 0.04, 0.95)));
        ripple.setLineWidth(3.0);
        ripple.setOpacity(0.0);
        let cover = CALayer::new();
        cover.setHidden(true);
        clip.addSublayer(&content);
        clip.addSublayer(&spot);
        clip.addSublayer(&ring);
        clip.addSublayer(&ripple);
        root.addSublayer(&clip);
        root.addSublayer(&cover);
        Self {
            clip,
            content,
            spot,
            ring,
            ripple,
            cover,
        }
    }
}

/// The largest rectangle of the picture's shape that fits `bounds`, in its middle.
pub fn fitted(picture: NSSize, bounds: NSSize) -> NSRect {
    if picture.width <= 0.0 || picture.height <= 0.0 {
        return NSRect::new(NSPoint::ZERO, bounds);
    }
    let scale = (bounds.width / picture.width).min(bounds.height / picture.height);
    let size = NSSize::new(picture.width * scale, picture.height * scale);
    NSRect::new(
        NSPoint::new(
            (bounds.width - size.width) / 2.0,
            (bounds.height - size.height) / 2.0,
        ),
        size,
    )
}

/// Where the picture goes inside its box when `view` of it fills the box: larger than the box, moved so the view is in
/// it.
pub fn content_frame(view: (f64, f64, f64, f64), boxed: NSSize) -> NSRect {
    let (x, y, w, h) = view;
    let (w, h) = (w.max(1e-6), h.max(1e-6));
    NSRect::new(
        NSPoint::new(-x / w * boxed.width, -y / h * boxed.height),
        NSSize::new(boxed.width / w, boxed.height / h),
    )
}

// ---------------------------------------------------------------- the presentation

struct Inner {
    mtm: MainThreadMarker,
    shared: Arc<Shared>,
    stream: Retained<SCStream>,
    _output: Retained<FrameOutput>,
    _queue: DispatchRetained<DispatchQueue>,
    window: Retained<NSWindow>,
    view: Retained<StageView>,
    layers: Layers,
    border: RefCell<Option<Retained<NSPanel>>>,
    /// The picture's size in points, which gives its shape on the stage.
    picture: NSSize,
    source: Source,
    /// The display the picture is of, for every source but a window.
    display: Option<u32>,
    /// Where the picture is on the screens, and when that was last looked at for a window.
    frame: Cell<Option<(NSRect, Instant)>>,
    look: Cell<Look>,
    subject: String,
    stage: Stage,
    closing: RefCell<Option<Retained<ProtocolObject<dyn NSObjectProtocol>>>>,
    stopped: Cell<bool>,
}

/// A presentation that runs until it is stopped or dropped.
pub struct Presentation {
    inner: Rc<Inner>,
}

fn error_text(error: *mut NSError) -> String {
    // SAFETY: the completion handler passes a valid NSError or null.
    unsafe { error.as_ref() }.map_or_else(
        || "unknown error".to_owned(),
        |e| e.localizedDescription().to_string(),
    )
}

/// The frame of a display in AppKit's screen coordinates.
pub fn screen_frame(mtm: MainThreadMarker, display: u32) -> Option<NSRect> {
    NSScreen::screens(mtm).iter().find_map(|screen| {
        let number = screen
            .deviceDescription()
            .objectForKey(&NSString::from_str("NSScreenNumber"))?
            .downcast::<NSNumber>()
            .ok()?
            .unsignedIntValue();
        (number == display).then(|| screen.frame())
    })
}

fn primary_height(mtm: MainThreadMarker) -> f64 {
    NSScreen::screens(mtm)
        .firstObject()
        .map_or(0.0, |s| s.frame().size.height)
}

impl Presentation {
    /// Starts presenting `source` on `stage`. `done` gets the presentation once its picture flows, or why it cannot
    /// start; `ended` is told once if it ends by itself. Both run on the main thread. Needs Screen Recording access.
    pub fn start(
        mtm: MainThreadMarker,
        source: Source,
        stage: Stage,
        options: Options,
        done: impl FnOnce(Result<Presentation, String>) + 'static,
        ended: impl FnOnce(Ended) + 'static,
    ) {
        // The stage and the frame come first, so that the picture can leave them out: a window that does not exist
        // yet cannot be named, and an app with no window is not listed at all.
        let staged = match stage_window(mtm, stage, &options.title) {
            Ok((window, view)) => Staged {
                window,
                view,
                border: if options.border {
                    border_panel(mtm, source)
                } else {
                    None
                },
            },
            Err(e) => return done(Err(e)),
        };
        let ticket = reply::expect(
            move |content: Result<Sendable<Retained<SCShareableContent>>, String>| {
                let built =
                    content.and_then(|c| build(mtm, &c.0, source, stage, staged, &options, ended));
                match built {
                    Ok(inner) => run(inner, done),
                    Err(e) => done(Err(e)),
                }
            },
        );
        let ticket = Mutex::new(Some(ticket));
        let handler = RcBlock::new(
            move |content: *mut SCShareableContent, error: *mut NSError| {
                let Some(ticket) = ticket.lock().unwrap_or_else(PoisonError::into_inner).take()
                else {
                    return;
                };
                // SAFETY: a non-null content pointer is a live snapshot for the call; retaining keeps it.
                match NonNull::new(content).and_then(|c| unsafe { Retained::retain(c.as_ptr()) }) {
                    Some(content) => ticket.send(Ok(Sendable(content))),
                    None => ticket.send(Err(error_text(error))),
                }
            },
        );
        // SAFETY: the handler only touches Send data and is retained by ScreenCaptureKit. Windows off screen are listed
        // too: the stage is not on screen yet.
        unsafe {
            SCShareableContent::getShareableContentExcludingDesktopWindows_onScreenWindowsOnly_completionHandler(
                false, false, &handler,
            );
        }
    }

    /// Ends it: the picture stops, the stage and the frame go away.
    pub fn stop(&self) {
        self.inner.stop();
    }

    /// The name of what is shown, for a window or an app: its title, or the app's name. Empty for a display.
    pub fn subject(&self) -> &str {
        &self.inner.subject
    }

    pub fn stage(&self) -> Stage {
        self.inner.stage
    }

    /// The display the picture is of, for every source but a single window.
    pub fn display(&self) -> Option<u32> {
        self.inner.display
    }

    /// Where the picture is on the screens now, in AppKit's coordinates, to place the pointer in it.
    pub fn picture_frame(&self) -> Option<NSRect> {
        self.inner.picture_frame()
    }

    /// Shows what the audience sees besides the picture; cheap when nothing changed.
    pub fn show(&self, look: Look) {
        if self.inner.look.replace(look) != look {
            instantly(|| self.inner.paint(look));
        }
    }

    /// Holds the picture still while `frozen`.
    pub fn freeze(&self, frozen: bool) {
        self.inner.shared.frozen.store(frozen, Ordering::Relaxed);
    }

    /// A ripple where a click landed, a fraction of the picture from its bottom-left.
    pub fn ripple(&self, at: (f64, f64)) {
        self.inner.ripple(at);
    }

    /// Whether a frame of the picture has reached the stage, for the checks.
    pub fn has_picture(&self) -> bool {
        // SAFETY: a plain property read of a layer on the main thread.
        unsafe { self.inner.layers.content.contents() }.is_some()
    }
}

impl Drop for Presentation {
    fn drop(&mut self) {
        self.inner.stop();
    }
}

/// The filter, the stream's size and what is shown, for a source of this snapshot. Runs on the main thread.
fn build(
    mtm: MainThreadMarker,
    content: &SCShareableContent,
    source: Source,
    stage: Stage,
    staged: Staged,
    options: &Options,
    ended: impl FnOnce(Ended) + 'static,
) -> Result<Rc<Inner>, String> {
    let pid = std::process::id() as i32;
    // SAFETY: plain property reads on a live content snapshot; filters and configurations are created and set with
    // the documented argument types.
    let (filter, config, picture, display, subject) = unsafe {
        let own: Vec<Retained<SCRunningApplication>> = content
            .applications()
            .iter()
            .filter(|a| a.processID() == pid)
            .collect();
        let displays: Vec<Retained<SCDisplay>> = content.displays().iter().collect();
        let display_of = |id: u32| {
            displays
                .iter()
                .find(|d| d.displayID() == id)
                .cloned()
                .ok_or_else(|| "the display is gone".to_owned())
        };
        let window_of = |id: u32| {
            content
                .windows()
                .iter()
                .find(|w| w.windowID() == id)
                .ok_or_else(|| "the window is gone".to_owned())
        };
        let config = SCStreamConfiguration::new();
        let (filter, picture, display, subject) = match source {
            Source::Display(id) | Source::Area { display: id, .. } => {
                let display = display_of(id)?;
                // Every window of this app is left out: by the app, or one by one when the app is not listed.
                let filter = if own.is_empty() {
                    let ours: Vec<Retained<SCWindow>> = content
                        .windows()
                        .iter()
                        .filter(|w| w.owningApplication().is_some_and(|a| a.processID() == pid))
                        .collect();
                    SCContentFilter::initWithDisplay_excludingWindows(
                        SCContentFilter::alloc(),
                        &display,
                        &NSArray::from_retained_slice(&ours),
                    )
                } else {
                    SCContentFilter::initWithDisplay_excludingApplications_exceptingWindows(
                        SCContentFilter::alloc(),
                        &display,
                        &NSArray::from_retained_slice(&own),
                        &NSArray::<SCWindow>::new(),
                    )
                };
                let whole = NSSize::new(display.width() as f64, display.height() as f64);
                let picture = match source {
                    Source::Area { rect, .. } => {
                        config.setSourceRect(CGRect::new(
                            CGPoint::new(
                                rect.origin.x,
                                whole.height - rect.origin.y - rect.size.height,
                            ),
                            rect.size,
                        ));
                        rect.size
                    }
                    _ => whole,
                };
                (filter, picture, Some(id), String::new())
            }
            Source::Window(id) => {
                let window = window_of(id)?;
                let filter = SCContentFilter::initWithDesktopIndependentWindow(
                    SCContentFilter::alloc(),
                    &window,
                );
                config.setIgnoreShadowsSingleWindow(true);
                let rect = filter.contentRect();
                let title = window.title().map(|t| t.to_string()).unwrap_or_default();
                let subject = if title.is_empty() {
                    window
                        .owningApplication()
                        .map(|a| a.applicationName().to_string())
                        .unwrap_or_default()
                } else {
                    title
                };
                (filter, rect.size, None, subject)
            }
            Source::App(id) => {
                let window = window_of(id)?;
                let app = window
                    .owningApplication()
                    .ok_or_else(|| "the app is gone".to_owned())?;
                let frame = window.frame();
                let middle = CGPoint::new(
                    frame.origin.x + frame.size.width / 2.0,
                    frame.origin.y + frame.size.height / 2.0,
                );
                let display = displays
                    .iter()
                    .find(|d| {
                        let f = d.frame();
                        middle.x >= f.origin.x
                            && middle.x < f.origin.x + f.size.width
                            && middle.y >= f.origin.y
                            && middle.y < f.origin.y + f.size.height
                    })
                    .or(displays.first())
                    .cloned()
                    .ok_or_else(|| "no display".to_owned())?;
                let filter =
                    SCContentFilter::initWithDisplay_includingApplications_exceptingWindows(
                        SCContentFilter::alloc(),
                        &display,
                        &NSArray::from_retained_slice(&[Retained::clone(&app)]),
                        &NSArray::<SCWindow>::new(),
                    );
                let picture = NSSize::new(display.width() as f64, display.height() as f64);
                (
                    filter,
                    picture,
                    Some(display.displayID()),
                    app.applicationName().to_string(),
                )
            }
        };
        if picture.width < 1.0 || picture.height < 1.0 {
            return Err("nothing to show".into());
        }
        let scale = f64::from(filter.pointPixelScale())
            .max(1.0)
            .min(MOST_PIXELS / picture.width.max(picture.height));
        config.setWidth((picture.width * scale).round().max(2.0) as usize);
        config.setHeight((picture.height * scale).round().max(2.0) as usize);
        config.setMinimumFrameInterval(CMTime::new(1, options.fps.clamp(10, 120) as i32));
        config.setShowsCursor(options.cursor);
        config.setQueueDepth(5);
        config.setPixelFormat(PIXEL_FORMAT);
        config.setCaptureResolution(SCCaptureResolutionType::Best);
        (filter, config, picture, display, subject)
    };
    let Staged {
        window,
        view,
        border,
    } = staged;
    if stage == Stage::Window {
        window.setContentSize(window_size(picture));
        window.center();
    }
    let root = view.layer().ok_or("the stage has no layer")?;
    let layers = Layers::new(&root);
    let id = NEXT.with(|next| {
        let id = next.get();
        next.set(id + 1);
        id
    });
    let shared = Arc::new(Shared {
        id,
        latest: Mutex::new(None),
        scheduled: AtomicBool::new(false),
        frozen: AtomicBool::new(false),
        ended: Mutex::new(Some(reply::expect(ended))),
    });
    let output = FrameOutput::alloc().set_ivars(OutputIvars {
        shared: Arc::clone(&shared),
    });
    // SAFETY: NSObject's designated initializer.
    let output: Retained<FrameOutput> = unsafe { msg_send![super(output), init] };
    let queue = DispatchQueue::new("figghost.presenter", None);
    // SAFETY: a configured filter and configuration; the output and the queue live as long as the stream.
    let stream = unsafe {
        let stream = SCStream::initWithFilter_configuration_delegate(
            SCStream::alloc(),
            &filter,
            &config,
            Some(ProtocolObject::from_ref(&*output)),
        );
        stream
            .addStreamOutput_type_sampleHandlerQueue_error(
                ProtocolObject::from_ref(&*output),
                SCStreamOutputType::Screen,
                Some(&queue),
            )
            .map_err(|e| e.localizedDescription().to_string())?;
        stream
    };
    let inner = Rc::new(Inner {
        mtm,
        shared,
        stream,
        _output: output,
        _queue: queue,
        window,
        view,
        layers,
        border: RefCell::new(border),
        picture,
        source,
        display,
        frame: Cell::new(None),
        look: Cell::new(Look::default()),
        subject,
        stage,
        closing: RefCell::default(),
        stopped: Cell::new(false),
    });
    STAGES.with(|stages| stages.borrow_mut().insert(id, Rc::downgrade(&inner)));
    let weak = Rc::downgrade(&inner);
    *inner.view.ivars().on_resize.borrow_mut() = Some(Box::new(move || {
        if let Some(inner) = weak.upgrade() {
            instantly(|| inner.layout());
        }
    }));
    instantly(|| inner.layout());
    Ok(inner)
}

/// The windows made before the picture is asked for.
struct Staged {
    window: Retained<NSWindow>,
    view: Retained<StageView>,
    border: Option<Retained<NSPanel>>,
}

/// The window of the window stage, of the picture's shape.
fn window_size(picture: NSSize) -> NSSize {
    let long = picture.width.max(picture.height).max(1.0);
    NSSize::new(
        (picture.width / long * WINDOW_SIDE).max(320.0),
        (picture.height / long * WINDOW_SIDE).max(200.0),
    )
}

/// A green frame around the area or display that is shown, which the audience does not see; none for a window or an
/// app, which move.
fn border_panel(mtm: MainThreadMarker, source: Source) -> Option<Retained<NSPanel>> {
    let frame = match source {
        Source::Display(id) => screen_frame(mtm, id)?,
        Source::Area { display, rect } => {
            let screen = screen_frame(mtm, display)?;
            NSRect::new(
                NSPoint::new(
                    screen.origin.x + rect.origin.x,
                    screen.origin.y + rect.origin.y,
                ),
                rect.size,
            )
        }
        Source::Window(_) | Source::App(_) => return None,
    };
    let outer = NSRect::new(
        NSPoint::new(frame.origin.x - BORDER, frame.origin.y - BORDER),
        NSSize::new(
            frame.size.width + 2.0 * BORDER,
            frame.size.height + 2.0 * BORDER,
        ),
    );
    let panel = crate::capture_ui::panel(mtm, outer, NSStatusWindowLevel);
    panel.setHasShadow(false);
    panel.setIgnoresMouseEvents(true);
    panel.setCollectionBehavior(
        NSWindowCollectionBehavior::CanJoinAllSpaces
            | NSWindowCollectionBehavior::Stationary
            | NSWindowCollectionBehavior::FullScreenAuxiliary
            | NSWindowCollectionBehavior::IgnoresCycle,
    );
    panel.setContentView(Some(&border_view(
        mtm,
        NSRect::new(NSPoint::ZERO, outer.size),
    )));
    Some(panel)
}

/// The window the audience looks at: borderless over a whole display, above its menu bar and Dock, or a titled window
/// of the picture's shape.
fn stage_window(
    mtm: MainThreadMarker,
    stage: Stage,
    title: &str,
) -> Result<(Retained<NSWindow>, Retained<StageView>), String> {
    let (frame, style) = match stage {
        Stage::Display(id) => (
            screen_frame(mtm, id).ok_or("the display to present on is gone")?,
            NSWindowStyleMask::Borderless,
        ),
        Stage::Window => (
            NSRect::new(NSPoint::ZERO, window_size(NSSize::new(16.0, 9.0))),
            NSWindowStyleMask::Titled
                | NSWindowStyleMask::Closable
                | NSWindowStyleMask::Miniaturizable
                | NSWindowStyleMask::Resizable,
        ),
    };
    // SAFETY: designated initializer with valid arguments on the main thread.
    let window = unsafe {
        NSWindow::initWithContentRect_styleMask_backing_defer(
            NSWindow::alloc(mtm),
            frame,
            style,
            NSBackingStoreType::Buffered,
            false,
        )
    };
    // SAFETY: false is the only value compatible with a window kept alive by `Retained`.
    unsafe { window.setReleasedWhenClosed(false) };
    window.setBackgroundColor(Some(&NSColor::blackColor()));
    window.setOpaque(true);
    window.setHidesOnDeactivate(false);
    match stage {
        Stage::Display(_) => {
            window.setLevel(NSPopUpMenuWindowLevel);
            window.setCollectionBehavior(
                NSWindowCollectionBehavior::CanJoinAllSpaces
                    | NSWindowCollectionBehavior::FullScreenAuxiliary
                    | NSWindowCollectionBehavior::Stationary
                    | NSWindowCollectionBehavior::IgnoresCycle,
            );
            window.setFrame_display(frame, false);
        }
        Stage::Window => {
            window.setLevel(NSNormalWindowLevel);
            window.setTitle(&NSString::from_str(title));
            window.center();
        }
    }
    let bounds = window
        .contentView()
        .map_or(NSRect::new(NSPoint::ZERO, frame.size), |v| v.bounds());
    let view = StageView::new(mtm, bounds);
    window.setContentView(Some(&view));
    Ok((window, view))
}

/// Starts the stream; the stage shows once the picture flows.
fn run(inner: Rc<Inner>, done: impl FnOnce(Result<Presentation, String>) + 'static) {
    let stream = inner.stream.clone();
    let started = reply::expect(move |error: Option<String>| match error {
        None if !inner.stopped.get() => {
            inner.show_stage();
            done(Ok(Presentation { inner }));
        }
        None => done(Err("stopped".into())),
        Some(e) => {
            inner.stop();
            done(Err(e));
        }
    });
    let started = Mutex::new(Some(started));
    let block = RcBlock::new(move |error: *mut NSError| {
        if let Some(ticket) = started
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .take()
        {
            // SAFETY: the handler passes a valid NSError or null.
            ticket.send(unsafe { error.as_ref() }.map(|e| e.localizedDescription().to_string()));
        }
    });
    // SAFETY: a configured stream; the block only moves Send data.
    unsafe { stream.startCaptureWithCompletionHandler(Some(&block)) };
}

impl Inner {
    fn show_stage(self: &Rc<Self>) {
        if let Some(border) = self.border.borrow().as_ref() {
            border.orderFrontRegardless();
        }
        match self.stage {
            Stage::Display(_) => self.window.orderFrontRegardless(),
            Stage::Window => {
                self.window.makeKeyAndOrderFront(None);
                // The person closing the window ends the presentation.
                let shared = Arc::clone(&self.shared);
                let block = RcBlock::new(move |_: NonNull<NSNotification>| shared.end(None));
                let observed: &AnyObject = &self.window;
                // SAFETY: the block is 'static and runs on the main thread, where the window posts the notification.
                let token = unsafe {
                    NSNotificationCenter::defaultCenter()
                        .addObserverForName_object_queue_usingBlock(
                            Some(NSWindowWillCloseNotification),
                            Some(observed),
                            None,
                            &block,
                        )
                };
                *self.closing.borrow_mut() = Some(token);
            }
        }
    }

    fn stop(&self) {
        if self.stopped.replace(true) {
            return;
        }
        // SAFETY: a stream that was started or is starting; no completion is needed.
        unsafe { self.stream.stopCaptureWithCompletionHandler(None) };
        if let Some(token) = self.closing.borrow_mut().take() {
            // SAFETY: the token came from the default center's addObserverForName and is removed once.
            unsafe {
                NSNotificationCenter::defaultCenter()
                    .removeObserver(AsRef::<AnyObject>::as_ref(&*token))
            };
        }
        self.window.orderOut(None);
        if let Some(border) = self.border.borrow_mut().take() {
            border.orderOut(None);
        }
        STAGES.with(|stages| stages.borrow_mut().remove(&self.shared.id));
    }

    /// Where the picture is, looked at again a few times a second: a window moves, and so does a display that is
    /// arranged anew.
    fn picture_frame(&self) -> Option<NSRect> {
        if let Some((frame, at)) = self.frame.get()
            && at.elapsed() < WINDOW_LOOK
        {
            return Some(frame);
        }
        let frame = match self.source {
            Source::Display(_) | Source::App(_) => {
                self.display.and_then(|d| screen_frame(self.mtm, d))
            }
            Source::Area { display, rect } => screen_frame(self.mtm, display).map(|screen| {
                NSRect::new(
                    NSPoint::new(
                        screen.origin.x + rect.origin.x,
                        screen.origin.y + rect.origin.y,
                    ),
                    rect.size,
                )
            }),
            Source::Window(id) => {
                let height = primary_height(self.mtm);
                on_screen_windows()
                    .into_iter()
                    .find(|w| w.id == id)
                    .map(|w| {
                        NSRect::new(
                            NSPoint::new(w.frame.x, height - w.frame.y - w.frame.h),
                            NSSize::new(w.frame.w, w.frame.h),
                        )
                    })
            }
        };
        if let Some(frame) = frame {
            self.frame.set(Some((frame, Instant::now())));
        }
        frame.or(self.frame.get().map(|(f, _)| f))
    }

    fn show_frame(&self, surface: &AnyObject) {
        if self.stopped.get() {
            return;
        }
        // SAFETY: an IOSurface is a valid value for a layer's contents, which keeps it.
        instantly(|| unsafe { self.layers.content.setContents(Some(surface)) });
    }

    /// Lays the picture's box out on the stage and paints what is over it.
    fn layout(&self) {
        let bounds = self.view.bounds();
        let boxed = fitted(self.picture, bounds.size);
        self.layers.clip.setFrame(boxed);
        self.layers.cover.setFrame(bounds);
        let inside = NSRect::new(NSPoint::ZERO, boxed.size);
        for layer in [&self.layers.spot, &self.layers.ring] {
            layer.setFrame(inside);
        }
        self.paint(self.look.get());
    }

    /// The pointer's place in the box of the picture, for a place in the picture.
    fn in_box(&self, look: &Look, at: (f64, f64)) -> NSPoint {
        let boxed = self.layers.clip.bounds().size;
        let (x, y, w, h) = look.view;
        NSPoint::new(
            (at.0 - x) / w.max(1e-6) * boxed.width,
            (at.1 - y) / h.max(1e-6) * boxed.height,
        )
    }

    fn paint(&self, look: Look) {
        let boxed = self.layers.clip.bounds().size;
        self.layers
            .content
            .setFrame(content_frame(look.view, boxed));
        let pointer = look.pointer.map(|at| self.in_box(&look, at));
        match (look.spotlight, pointer) {
            (Some(share), Some(p)) => {
                let radius = (share * boxed.height).max(20.0);
                let path = CGMutablePath::new();
                // SAFETY: a fresh path and plain rectangles; a null transform is allowed.
                unsafe {
                    CGMutablePath::add_rect(
                        Some(&path),
                        std::ptr::null(),
                        CGRect::new(CGPoint::ZERO, boxed),
                    );
                    CGMutablePath::add_ellipse_in_rect(
                        Some(&path),
                        std::ptr::null(),
                        CGRect::new(
                            CGPoint::new(p.x - radius, p.y - radius),
                            CGSize::new(2.0 * radius, 2.0 * radius),
                        ),
                    );
                }
                self.layers.spot.setPath(Some(&path));
                self.layers.spot.setHidden(false);
            }
            _ => self.layers.spot.setHidden(true),
        }
        match pointer.filter(|_| look.ring) {
            Some(p) => {
                let radius = (RING_SHARE * boxed.height).clamp(14.0, 44.0);
                let path = CGMutablePath::new();
                // SAFETY: a fresh path and a plain rectangle; a null transform is allowed.
                unsafe {
                    CGMutablePath::add_ellipse_in_rect(
                        Some(&path),
                        std::ptr::null(),
                        CGRect::new(
                            CGPoint::new(p.x - radius, p.y - radius),
                            CGSize::new(2.0 * radius, 2.0 * radius),
                        ),
                    );
                }
                self.layers.ring.setPath(Some(&path));
                self.layers.ring.setHidden(false);
            }
            None => self.layers.ring.setHidden(true),
        }
        match look.cover {
            Cover::None => self.layers.cover.setHidden(true),
            Cover::Black | Cover::White => {
                let white = if look.cover == Cover::White { 1.0 } else { 0.0 };
                self.layers
                    .cover
                    .setBackgroundColor(Some(&color(white, 1.0)));
                self.layers.cover.setHidden(false);
            }
        }
    }

    fn ripple(&self, at: (f64, f64)) {
        let look = self.look.get();
        let p = self.in_box(&look, at);
        let radius = (RING_SHARE * 1.6 * self.layers.clip.bounds().size.height).clamp(20.0, 64.0);
        let ripple = &self.layers.ripple;
        instantly(|| {
            ripple.setFrame(CGRect::new(
                CGPoint::new(p.x - radius, p.y - radius),
                CGSize::new(2.0 * radius, 2.0 * radius),
            ));
            let path = CGMutablePath::new();
            // SAFETY: a fresh path and a plain rectangle; a null transform is allowed.
            unsafe {
                CGMutablePath::add_ellipse_in_rect(
                    Some(&path),
                    std::ptr::null(),
                    CGRect::new(CGPoint::ZERO, CGSize::new(2.0 * radius, 2.0 * radius)),
                );
            }
            ripple.setPath(Some(&path));
            ripple.setOpacity(0.0);
        });
        if crate::workspace::display_options().reduce_motion {
            return;
        }
        let grow =
            CABasicAnimation::animationWithKeyPath(Some(&NSString::from_str("transform.scale")));
        let fade = CABasicAnimation::animationWithKeyPath(Some(&NSString::from_str("opacity")));
        // SAFETY: NSNumber is a valid value for a scale and an opacity animation.
        unsafe {
            grow.setFromValue(Some(&NSNumber::new_f64(0.25)));
            grow.setToValue(Some(&NSNumber::new_f64(1.0)));
            fade.setFromValue(Some(&NSNumber::new_f64(0.95)));
            fade.setToValue(Some(&NSNumber::new_f64(0.0)));
        }
        let group = CAAnimationGroup::animation();
        group.setAnimations(Some(&NSArray::from_retained_slice(&[
            Retained::into_super(Retained::into_super(grow)),
            Retained::into_super(Retained::into_super(fade)),
        ])));
        group.setDuration(RIPPLE_SECONDS);
        ripple.addAnimation_forKey(&group, Some(&NSString::from_str("ripple")));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn close(a: NSRect, b: (f64, f64, f64, f64)) -> bool {
        (a.origin.x - b.0).abs() < 1e-9
            && (a.origin.y - b.1).abs() < 1e-9
            && (a.size.width - b.2).abs() < 1e-9
            && (a.size.height - b.3).abs() < 1e-9
    }

    #[test]
    fn the_picture_keeps_its_shape_on_the_stage() {
        // A 16:10 laptop on a 16:9 projector: bars at the sides.
        let boxed = fitted(NSSize::new(1512.0, 945.0), NSSize::new(1920.0, 1080.0));
        assert!(close(boxed, (96.0, 0.0, 1728.0, 1080.0)), "{boxed:?}");
        // A tall window in a wide window: bars at the sides too.
        let tall = fitted(NSSize::new(400.0, 800.0), NSSize::new(960.0, 540.0));
        assert!((tall.size.height - 540.0).abs() < 1e-9 && (tall.origin.x - 345.0).abs() < 1e-9);
        let nothing = fitted(NSSize::new(0.0, 0.0), NSSize::new(100.0, 50.0));
        assert!(close(nothing, (0.0, 0.0, 100.0, 50.0)));
    }

    #[test]
    fn a_zoomed_view_fills_the_box() {
        let boxed = NSSize::new(1600.0, 900.0);
        assert!(close(
            content_frame((0.0, 0.0, 1.0, 1.0), boxed),
            (0.0, 0.0, 1600.0, 900.0)
        ));
        // The right half of the top half, twice as large: the picture moves left and down.
        let zoomed = content_frame((0.5, 0.5, 0.5, 0.5), boxed);
        assert!(
            close(zoomed, (-1600.0, -900.0, 3200.0, 1800.0)),
            "{zoomed:?}"
        );
    }
}
