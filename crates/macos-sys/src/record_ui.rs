//! Windows that belong in a recording on purpose: the key display, which names the shortcuts pressed,
//! and the camera bubble. A recording leaves this app's windows out unless their numbers are passed in
//! its options, so both stay on screen for the whole recording; the key display is see-through while
//! no key is shown.

use crate::{
    app,
    capture_ui::panel,
    event_tap::{self, EventTap, TapError, Verdict},
    reply,
    widgets::{Fit, draw_text, text_attributes},
};
use block2::RcBlock;
use core_capture::{Modifiers, key_label};
use objc2::{
    DefinedClass, MainThreadMarker, MainThreadOnly, define_class, msg_send, rc::Retained,
    runtime::Bool,
};
use objc2_app_kit::{
    NSBezierPath, NSColor, NSFont, NSPanel, NSStatusWindowLevel, NSStringDrawing, NSView,
};
use objc2_av_foundation::{
    AVAuthorizationStatus, AVCaptureDevice, AVCaptureDeviceInput, AVCaptureSession,
    AVCaptureSessionPresetMedium, AVCaptureVideoPreviewLayer, AVLayerVideoGravityResizeAspectFill,
    AVMediaTypeVideo,
};
use objc2_foundation::{NSPoint, NSRect, NSSize, NSString};
use std::{
    cell::{Cell, RefCell},
    rc::{Rc, Weak},
    sync::{Mutex, PoisonError},
};

const KEY_DOWN: u32 = 10;
const KEYCODE_FIELD: u32 = 9;
const AUTOREPEAT_FIELD: u32 = 8;
const KEY_FONT_SIZE: f64 = 20.0;
const KEY_HEIGHT: f64 = 44.0;
const KEY_SECONDS: f64 = 1.6;
const KEY_BOTTOM: f64 = 48.0;
const CAMERA_SIZE: f64 = 180.0;
const CAMERA_MARGIN: f64 = 24.0;

pub struct KeyViewIvars {
    text: RefCell<String>,
}

define_class!(
    // SAFETY: NSView allows subclassing; the ivars are only touched on the main thread and need no Drop.
    #[unsafe(super(NSView))]
    #[thread_kind = MainThreadOnly]
    #[name = "FigKeyDisplayView"]
    #[ivars = KeyViewIvars]
    struct KeyView;

    impl KeyView {
        #[unsafe(method(drawRect:))]
        fn draw_rect(&self, _dirty: NSRect) {
            let bounds = self.bounds();
            NSColor::colorWithWhite_alpha(0.08, 0.88).setFill();
            NSBezierPath::bezierPathWithRoundedRect_xRadius_yRadius(bounds, 12.0, 12.0).fill();
            let attrs = key_attributes();
            let line = KEY_FONT_SIZE * 1.25;
            draw_text(
                &self.ivars().text.borrow(),
                NSRect::new(
                    NSPoint::new(10.0, (bounds.size.height - line) / 2.0),
                    NSSize::new(bounds.size.width - 20.0, line),
                ),
                &attrs,
            );
        }
    }
);

fn key_attributes()
-> Retained<objc2_foundation::NSMutableDictionary<NSString, objc2::runtime::AnyObject>> {
    text_attributes(
        &NSFont::systemFontOfSize_weight(KEY_FONT_SIZE, 0.3),
        &NSColor::whiteColor(),
        Fit::Tail,
        true,
    )
}

thread_local! {
    /// The key display on screen, reached from the event tap's thread through the main queue.
    static SHOWN: RefCell<Weak<KeyDisplay>> = const { RefCell::new(Weak::new()) };
}

/// The shortcut just pressed, at the bottom of the recorded area, for a moment. Typing stays hidden:
/// see [`key_label`].
pub struct KeyDisplay {
    me: Weak<KeyDisplay>,
    panel: Retained<NSPanel>,
    view: Retained<KeyView>,
    area: NSRect,
    shown: Cell<u64>,
    _tap: EventTap,
}

impl KeyDisplay {
    /// `area` is the recorded area, in AppKit screen coordinates. Fails without Accessibility access,
    /// which the key tap needs.
    pub fn show(mtm: MainThreadMarker, area: NSRect) -> Result<Rc<Self>, TapError> {
        let tap = EventTap::spawn(event_tap::mask(KEY_DOWN), |kind, event| {
            if kind == KEY_DOWN
                && event.int(AUTOREPEAT_FIELD) == 0
                && let Some(label) = key_label(
                    event.int(KEYCODE_FIELD) as u16,
                    Modifiers::from_flags(event.flags()),
                )
            {
                app::on_main(move || {
                    if let Some(display) = SHOWN.with(|s| s.borrow().upgrade()) {
                        display.press(&label);
                    }
                });
            }
            Verdict::Pass
        })?;
        let frame = Self::frame(area, 120.0);
        let panel = panel(mtm, frame, NSStatusWindowLevel);
        panel.setIgnoresMouseEvents(true);
        panel.setAlphaValue(0.0);
        let ivars = KeyViewIvars {
            text: RefCell::new(String::new()),
        };
        // SAFETY: designated initializer with valid arguments on the main thread.
        let view: Retained<KeyView> = unsafe {
            msg_send![
                super(mtm.alloc::<KeyView>().set_ivars(ivars)),
                initWithFrame: NSRect::new(NSPoint::ZERO, frame.size)
            ]
        };
        panel.setContentView(Some(&view));
        panel.orderFrontRegardless();
        let display = Rc::new_cyclic(|me| Self {
            me: me.clone(),
            panel,
            view,
            area,
            shown: Cell::new(0),
            _tap: tap,
        });
        SHOWN.with(|s| *s.borrow_mut() = Rc::downgrade(&display));
        Ok(display)
    }

    fn frame(area: NSRect, width: f64) -> NSRect {
        let width = width.min(area.size.width);
        NSRect::new(
            NSPoint::new(
                area.origin.x + (area.size.width - width) / 2.0,
                area.origin.y + KEY_BOTTOM.min(area.size.height / 4.0),
            ),
            NSSize::new(width, KEY_HEIGHT),
        )
    }

    /// The window number a recording includes.
    pub fn window_number(&self) -> u32 {
        self.panel.windowNumber() as u32
    }

    /// Shows `label`, then hides it again unless another key comes first.
    pub fn press(&self, label: &str) {
        // SAFETY: valid attributes for measuring a string on the main thread.
        let width =
            unsafe { NSString::from_str(label).sizeWithAttributes(Some(&key_attributes())) }.width;
        let frame = Self::frame(self.area, width.ceil() + 40.0);
        *self.view.ivars().text.borrow_mut() = label.to_owned();
        self.panel.setFrame_display(frame, true);
        self.view.setFrame(NSRect::new(NSPoint::ZERO, frame.size));
        self.view.setNeedsDisplay(true);
        self.panel.setAlphaValue(1.0);
        let shown = self.shown.get() + 1;
        self.shown.set(shown);
        let me = self.me.clone();
        app::after(KEY_SECONDS, move || {
            if let Some(display) = me.upgrade()
                && display.shown.get() == shown
            {
                display.panel.setAlphaValue(0.0);
            }
        });
    }

    pub fn close(&self) {
        self.panel.orderOut(None);
    }
}

/// Whether the camera may be used, asking the first time; `done` runs on the main thread.
pub fn camera_access(done: impl FnOnce(bool) + 'static) {
    // SAFETY: a framework constant, read once.
    let Some(video) = (unsafe { AVMediaTypeVideo }) else {
        return done(false);
    };
    // SAFETY: a class method taking a media type constant.
    match unsafe { AVCaptureDevice::authorizationStatusForMediaType(video) } {
        AVAuthorizationStatus::Authorized => done(true),
        AVAuthorizationStatus::NotDetermined => {
            let ticket = Mutex::new(Some(reply::expect(done)));
            let block = RcBlock::new(move |granted: Bool| {
                if let Some(ticket) = ticket.lock().unwrap_or_else(PoisonError::into_inner).take() {
                    ticket.send(granted.as_bool());
                }
            });
            // SAFETY: the block only moves Send data; AVFoundation retains it until the user answers.
            unsafe { AVCaptureDevice::requestAccessForMediaType_completionHandler(video, &block) };
        }
        _ => done(false),
    }
}

/// A capture session handed to a worker thread to start.
struct StartingSession(Retained<AVCaptureSession>);

// SAFETY: AVCaptureSession may be started from any thread; Apple asks for a background one because
// starting blocks while the camera powers up.
unsafe impl Send for StartingSession {}

/// The camera, mirrored, in a round window the user can drag; it starts in the bottom-right corner of
/// the recorded area.
pub struct CameraBubble {
    panel: Retained<NSPanel>,
    session: Retained<AVCaptureSession>,
}

impl CameraBubble {
    /// `None` when there is no camera or it cannot be opened; ask with [`camera_access`] first.
    pub fn show(mtm: MainThreadMarker, area: NSRect) -> Option<Self> {
        // SAFETY: AVFoundation calls with framework constants and live objects on the main thread.
        let (session, preview) = unsafe {
            let device = AVCaptureDevice::defaultDeviceWithMediaType(AVMediaTypeVideo?)?;
            let input = AVCaptureDeviceInput::deviceInputWithDevice_error(&device).ok()?;
            let session = AVCaptureSession::new();
            if session.canSetSessionPreset(AVCaptureSessionPresetMedium) {
                session.setSessionPreset(AVCaptureSessionPresetMedium);
            }
            if !session.canAddInput(&input) {
                return None;
            }
            session.addInput(&input);
            let preview = AVCaptureVideoPreviewLayer::layerWithSession(&session);
            if let Some(gravity) = AVLayerVideoGravityResizeAspectFill {
                preview.setVideoGravity(gravity);
            }
            if let Some(connection) = preview.connection()
                && connection.isVideoMirroringSupported()
            {
                connection.setAutomaticallyAdjustsVideoMirroring(false);
                connection.setVideoMirrored(true);
            }
            (session, preview)
        };
        let size = CAMERA_SIZE.min(area.size.width.min(area.size.height) / 2.0);
        let frame = NSRect::new(
            NSPoint::new(
                area.origin.x + area.size.width - size - CAMERA_MARGIN,
                area.origin.y + CAMERA_MARGIN,
            ),
            NSSize::new(size, size),
        );
        let panel = panel(mtm, frame, NSStatusWindowLevel);
        panel.setMovableByWindowBackground(true);
        preview.setCornerRadius(size / 2.0);
        preview.setMasksToBounds(true);
        let view =
            NSView::initWithFrame(NSView::alloc(mtm), NSRect::new(NSPoint::ZERO, frame.size));
        // A layer-hosting view: the layer is set before layers are turned on.
        view.setLayer(Some(&preview));
        view.setWantsLayer(true);
        panel.setContentView(Some(&view));
        panel.orderFrontRegardless();
        let starting = StartingSession(session.clone());
        std::thread::spawn(move || {
            let starting = starting;
            // SAFETY: see StartingSession.
            unsafe { starting.0.startRunning() };
        });
        Some(Self { panel, session })
    }

    pub fn window_number(&self) -> u32 {
        self.panel.windowNumber() as u32
    }

    pub fn close(&self) {
        // SAFETY: stopping a session is allowed from any thread.
        unsafe { self.session.stopRunning() };
        self.panel.orderOut(None);
    }
}
