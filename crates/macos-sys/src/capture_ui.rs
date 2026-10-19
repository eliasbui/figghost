//! What follows a capture: the quick panel in a screen corner (thumbnail and actions; the thumbnail drags
//! out as a file), pinned pictures that float above every window, and short notices.

use crate::{
    app,
    widgets::{ActionTarget, Fit, draw_text, text_attributes},
};
use objc2::{
    AnyThread, DefinedClass, MainThreadMarker, MainThreadOnly, define_class, msg_send,
    rc::Retained,
    runtime::{AnyObject, ProtocolObject},
    sel,
};
use objc2_app_kit::{
    NSBackingStoreType, NSBezierPath, NSButton, NSColor, NSControlSize, NSDragOperation,
    NSDraggingContext, NSDraggingItem, NSDraggingSession, NSDraggingSource, NSEvent, NSFont,
    NSImage, NSMenu, NSMenuItem, NSModalResponseOK, NSPanel, NSPasteboardWriting, NSSavePanel,
    NSScreen, NSStatusWindowLevel, NSTrackingArea, NSTrackingAreaOptions, NSView,
    NSWindowCollectionBehavior, NSWindowStyleMask,
};
use objc2_foundation::{NSArray, NSObjectProtocol, NSPoint, NSRect, NSSize, NSString, NSURL};
use std::{
    cell::{Cell, RefCell},
    path::{Path, PathBuf},
    rc::Rc,
};

const QUICK_WIDTH: f64 = 300.0;
const QUICK_PAD: f64 = 10.0;
const THUMB_MAX_HEIGHT: f64 = 170.0;
const BUTTON_HEIGHT: f64 = 24.0;
const SCREEN_MARGIN: f64 = 18.0;
const DRAG_THRESHOLD: f64 = 4.0;
const TOAST_SECONDS: f64 = 1.8;

pub(crate) fn panel(mtm: MainThreadMarker, frame: NSRect, level: isize) -> Retained<NSPanel> {
    // SAFETY: designated initializer with valid arguments on the main thread.
    let panel: Retained<NSPanel> = unsafe {
        msg_send![
            NSPanel::alloc(mtm),
            initWithContentRect: frame,
            styleMask: NSWindowStyleMask::Borderless | NSWindowStyleMask::NonactivatingPanel,
            backing: NSBackingStoreType::Buffered,
            defer: false
        ]
    };
    // SAFETY: false is the only value compatible with a window kept alive by `Retained`.
    unsafe { panel.setReleasedWhenClosed(false) };
    panel.setOpaque(false);
    panel.setBackgroundColor(Some(&NSColor::clearColor()));
    panel.setHasShadow(true);
    panel.setHidesOnDeactivate(false);
    panel.setLevel(level);
    panel.setCollectionBehavior(
        NSWindowCollectionBehavior::CanJoinAllSpaces
            | NSWindowCollectionBehavior::FullScreenAuxiliary
            | NSWindowCollectionBehavior::IgnoresCycle,
    );
    panel
}

fn button(
    mtm: MainThreadMarker,
    title: &str,
    frame: NSRect,
    target: &ActionTarget,
) -> Retained<NSButton> {
    let target: &AnyObject = target;
    // SAFETY: `fire:` takes the sender as its only argument; the caller keeps the target alive.
    let button = unsafe {
        NSButton::buttonWithTitle_target_action(
            &NSString::from_str(title),
            Some(target),
            Some(sel!(fire:)),
            mtm,
        )
    };
    button.setControlSize(NSControlSize::Small);
    button.setFont(Some(&NSFont::systemFontOfSize(11.0)));
    button.setFrame(frame);
    button
}

/// `rect` grown (or shrunk, for a negative `by`) by `by` on every side.
fn outset(rect: NSRect, by: f64) -> NSRect {
    NSRect::new(
        NSPoint::new(rect.origin.x - by, rect.origin.y - by),
        NSSize::new(rect.size.width + 2.0 * by, rect.size.height + 2.0 * by),
    )
}

/// The largest size with the aspect of `size` that fits in `bounds`, never larger than `size` itself.
fn fit(size: NSSize, bounds: NSSize) -> NSSize {
    if size.width <= 0.0 || size.height <= 0.0 {
        return NSSize::new(0.0, 0.0);
    }
    let scale = (bounds.width / size.width)
        .min(bounds.height / size.height)
        .min(1.0);
    NSSize::new((size.width * scale).floor(), (size.height * scale).floor())
}

pub struct BackdropIvars {
    hovered: Cell<bool>,
}

define_class!(
    // SAFETY: NSView allows subclassing; the ivars are only touched on the main thread and need no Drop.
    #[unsafe(super(NSView))]
    #[thread_kind = MainThreadOnly]
    #[name = "FigQuickBackdrop"]
    #[ivars = BackdropIvars]
    struct Backdrop;

    impl Backdrop {
        #[unsafe(method(drawRect:))]
        fn draw_rect(&self, _dirty: NSRect) {
            NSColor::colorWithWhite_alpha(0.08, 0.92).setFill();
            NSBezierPath::bezierPathWithRoundedRect_xRadius_yRadius(self.bounds(), 12.0, 12.0).fill();
        }

        #[unsafe(method(mouseEntered:))]
        fn mouse_entered(&self, _event: &NSEvent) {
            self.ivars().hovered.set(true);
        }

        #[unsafe(method(mouseExited:))]
        fn mouse_exited(&self, _event: &NSEvent) {
            self.ivars().hovered.set(false);
        }
    }
);

pub struct ThumbIvars {
    image: Retained<NSImage>,
    file: PathBuf,
    press: Cell<Option<NSPoint>>,
    on_click: RefCell<Option<Rc<dyn Fn()>>>,
}

define_class!(
    // SAFETY: NSView allows subclassing; the ivars are only touched on the main thread and need no Drop.
    #[unsafe(super(NSView))]
    #[thread_kind = MainThreadOnly]
    #[name = "FigQuickThumb"]
    #[ivars = ThumbIvars]
    struct Thumb;

    impl Thumb {
        #[unsafe(method(drawRect:))]
        fn draw_rect(&self, _dirty: NSRect) {
            let bounds = self.bounds();
            let path = NSBezierPath::bezierPathWithRoundedRect_xRadius_yRadius(bounds, 6.0, 6.0);
            path.addClip();
            self.ivars().image.drawInRect(bounds);
        }

        #[unsafe(method(acceptsFirstMouse:))]
        fn accepts_first_mouse(&self, _event: Option<&NSEvent>) -> bool {
            true
        }

        #[unsafe(method(mouseDown:))]
        fn mouse_down(&self, event: &NSEvent) {
            let point = self.convertPoint_fromView(event.locationInWindow(), None);
            self.ivars().press.set(Some(point));
        }

        #[unsafe(method(mouseDragged:))]
        fn mouse_dragged(&self, event: &NSEvent) {
            let Some(origin) = self.ivars().press.get() else {
                return;
            };
            let point = self.convertPoint_fromView(event.locationInWindow(), None);
            if (point.x - origin.x).hypot(point.y - origin.y) > DRAG_THRESHOLD {
                self.ivars().press.set(None);
                self.begin_drag(event);
            }
        }

        #[unsafe(method(mouseUp:))]
        fn mouse_up(&self, _event: &NSEvent) {
            if self.ivars().press.take().is_some() {
                let on_click = self.ivars().on_click.borrow().clone();
                if let Some(on_click) = on_click {
                    on_click();
                }
            }
        }
    }

    unsafe impl NSObjectProtocol for Thumb {}

    unsafe impl NSDraggingSource for Thumb {
        #[unsafe(method(draggingSession:sourceOperationMaskForDraggingContext:))]
        fn source_operation_mask(
            &self,
            _session: &NSDraggingSession,
            _context: NSDraggingContext,
        ) -> NSDragOperation {
            NSDragOperation::Copy
        }
    }
);

impl Thumb {
    fn begin_drag(&self, event: &NSEvent) {
        let ivars = self.ivars();
        let url = NSURL::fileURLWithPath(&NSString::from_str(&ivars.file.to_string_lossy()));
        let writer: Retained<ProtocolObject<dyn NSPasteboardWriting>> =
            ProtocolObject::from_retained(url);
        let item = NSDraggingItem::initWithPasteboardWriter(NSDraggingItem::alloc(), &writer);
        // SAFETY: the contents object is an NSImage, which NSDraggingItem accepts.
        unsafe { item.setDraggingFrame_contents(self.bounds(), Some(&ivars.image)) };
        self.beginDraggingSessionWithItems_event_source(
            &NSArray::from_slice(&[&*item]),
            event,
            ProtocolObject::from_ref(self),
        );
    }
}

/// A button of the quick panel: its label and what it does.
pub type QuickButton = (String, Box<dyn Fn()>);

/// A capture's thumbnail and actions, in the bottom-left corner of the screen it was taken on.
pub struct QuickPanel {
    panel: Retained<NSPanel>,
    backdrop: Retained<Backdrop>,
    _targets: Vec<Retained<ActionTarget>>,
}

impl QuickPanel {
    /// `file` is what dragging the thumbnail out delivers; `screen` is where the capture was taken
    /// (AppKit coordinates); `slot` stacks panels upwards when several are open. `on_click` runs when
    /// the thumbnail is clicked.
    pub fn show(
        mtm: MainThreadMarker,
        image: Retained<NSImage>,
        file: &Path,
        screen: NSRect,
        slot: usize,
        buttons: Vec<QuickButton>,
        on_click: Box<dyn Fn()>,
    ) -> Rc<Self> {
        let thumb_size = fit(
            image.size(),
            NSSize::new(QUICK_WIDTH - 2.0 * QUICK_PAD, THUMB_MAX_HEIGHT),
        );
        let height = QUICK_PAD * 3.0 + thumb_size.height.max(40.0) + BUTTON_HEIGHT;
        let visible = NSScreen::screens(mtm)
            .iter()
            .map(|s| s.visibleFrame())
            .find(|v| {
                v.origin.x >= screen.origin.x - 1.0
                    && v.origin.x < screen.origin.x + screen.size.width
            })
            .unwrap_or(screen);
        let frame = NSRect::new(
            NSPoint::new(
                visible.origin.x + SCREEN_MARGIN,
                visible.origin.y + SCREEN_MARGIN + slot as f64 * (height + 10.0),
            ),
            NSSize::new(QUICK_WIDTH, height),
        );
        let panel = panel(mtm, frame, NSStatusWindowLevel);
        let bounds = NSRect::new(NSPoint::ZERO, frame.size);
        // SAFETY: designated initializer with valid arguments on the main thread.
        let backdrop: Retained<Backdrop> = unsafe {
            msg_send![
                super(mtm.alloc::<Backdrop>().set_ivars(BackdropIvars { hovered: Cell::new(false) })),
                initWithFrame: bounds
            ]
        };
        // SAFETY: the backdrop owns the tracking area and outlives it.
        let tracking = unsafe {
            NSTrackingArea::initWithRect_options_owner_userInfo(
                NSTrackingArea::alloc(),
                bounds,
                NSTrackingAreaOptions::MouseEnteredAndExited | NSTrackingAreaOptions::ActiveAlways,
                Some(&backdrop),
                None,
            )
        };
        backdrop.addTrackingArea(&tracking);
        panel.setContentView(Some(&backdrop));

        let thumb_frame = NSRect::new(
            NSPoint::new(
                (QUICK_WIDTH - thumb_size.width) / 2.0,
                QUICK_PAD * 2.0
                    + BUTTON_HEIGHT
                    + (thumb_size.height.max(40.0) - thumb_size.height) / 2.0,
            ),
            thumb_size,
        );
        let ivars = ThumbIvars {
            image,
            file: file.to_owned(),
            press: Cell::new(None),
            on_click: RefCell::new(Some(Rc::from(on_click))),
        };
        // SAFETY: designated initializer with valid arguments on the main thread.
        let thumb: Retained<Thumb> = unsafe {
            msg_send![super(mtm.alloc::<Thumb>().set_ivars(ivars)), initWithFrame: thumb_frame]
        };
        backdrop.addSubview(&thumb);

        let width = (QUICK_WIDTH - 2.0 * QUICK_PAD) / buttons.len().max(1) as f64;
        let mut targets = Vec::new();
        for (i, (title, action)) in buttons.into_iter().enumerate() {
            let target = ActionTarget::new(mtm, action);
            let made = button(
                mtm,
                &title,
                NSRect::new(
                    NSPoint::new(QUICK_PAD + i as f64 * width, QUICK_PAD),
                    NSSize::new(width - 2.0, BUTTON_HEIGHT),
                ),
                &target,
            );
            backdrop.addSubview(&made);
            targets.push(target);
        }
        panel.orderFrontRegardless();
        Rc::new(Self {
            panel,
            backdrop,
            _targets: targets,
        })
    }

    pub fn is_hovered(&self) -> bool {
        self.backdrop.ivars().hovered.get()
    }

    pub fn close(&self) {
        self.panel.orderOut(None);
    }
}

pub struct PinIvars {
    image: Retained<NSImage>,
    menu: RefCell<Option<Retained<NSMenu>>>,
    on_close: RefCell<Option<Rc<dyn Fn()>>>,
}

define_class!(
    // SAFETY: NSView allows subclassing; the ivars are only touched on the main thread and need no Drop.
    #[unsafe(super(NSView))]
    #[thread_kind = MainThreadOnly]
    #[name = "FigPinView"]
    #[ivars = PinIvars]
    struct PinView;

    impl PinView {
        #[unsafe(method(drawRect:))]
        fn draw_rect(&self, _dirty: NSRect) {
            self.ivars().image.drawInRect(self.bounds());
        }

        #[unsafe(method(acceptsFirstMouse:))]
        fn accepts_first_mouse(&self, _event: Option<&NSEvent>) -> bool {
            true
        }

        #[unsafe(method(acceptsFirstResponder))]
        fn accepts_first_responder(&self) -> bool {
            true
        }

        #[unsafe(method(mouseDown:))]
        fn mouse_down(&self, event: &NSEvent) {
            if event.clickCount() >= 2 {
                self.close();
                return;
            }
            if let Some(window) = self.window() {
                window.performWindowDragWithEvent(event);
            }
        }

        #[unsafe(method(rightMouseDown:))]
        fn right_mouse_down(&self, event: &NSEvent) {
            if let Some(menu) = self.ivars().menu.borrow().as_ref() {
                NSMenu::popUpContextMenu_withEvent_forView(menu, event, self);
            }
        }

        #[unsafe(method(keyDown:))]
        fn key_down(&self, event: &NSEvent) {
            if event.keyCode() == 53 {
                self.close();
            }
        }

        /// Scrolling zooms the picture about its top-left corner.
        #[unsafe(method(scrollWheel:))]
        fn scroll_wheel(&self, event: &NSEvent) {
            let Some(window) = self.window() else {
                return;
            };
            let factor = (1.0 + event.scrollingDeltaY() * 0.01).clamp(0.8, 1.25);
            let frame = window.frame();
            let natural = self.ivars().image.size();
            let width = (frame.size.width * factor).clamp(64.0, natural.width * 4.0);
            let height = width * natural.height / natural.width.max(1.0);
            let top = frame.origin.y + frame.size.height;
            window.setFrame_display(
                NSRect::new(NSPoint::new(frame.origin.x, top - height), NSSize::new(width, height)),
                true,
            );
        }
    }
);

impl PinView {
    fn close(&self) {
        if let Some(window) = self.window() {
            window.orderOut(None);
        }
        let on_close = self.ivars().on_close.borrow().clone();
        if let Some(on_close) = on_close {
            on_close();
        }
    }
}

define_class!(
    // SAFETY: NSPanel allows subclassing; this one only allows key status, for Escape.
    #[unsafe(super(NSPanel))]
    #[thread_kind = MainThreadOnly]
    #[name = "FigPinPanel"]
    struct PinPanel;

    impl PinPanel {
        #[unsafe(method(canBecomeKeyWindow))]
        fn can_become_key_window(&self) -> bool {
            true
        }
    }
);

/// Labels of a pinned picture's right-click menu.
#[derive(Debug, Clone, Default)]
pub struct PinTexts {
    pub copy: String,
    pub save: String,
    pub close: String,
}

pub struct PinActions {
    pub on_copy: Box<dyn Fn()>,
    pub on_save: Box<dyn Fn()>,
    /// The pin closed itself (double click, Escape, the menu).
    pub on_closed: Box<dyn Fn()>,
}

/// A picture floating above every window: drag to move, scroll to zoom, double click or Escape to close.
pub struct Pin {
    panel: Retained<PinPanel>,
    _view: Retained<PinView>,
    _targets: Vec<Retained<ActionTarget>>,
}

impl Pin {
    /// Shows `image` at its own size with its top-left corner at `top_left` (AppKit coordinates).
    pub fn show(
        mtm: MainThreadMarker,
        image: Retained<NSImage>,
        top_left: NSPoint,
        texts: &PinTexts,
        actions: PinActions,
    ) -> Rc<Self> {
        let size = image.size();
        let frame = NSRect::new(NSPoint::new(top_left.x, top_left.y - size.height), size);
        // SAFETY: designated initializer with valid arguments on the main thread.
        let panel: Retained<PinPanel> = unsafe {
            msg_send![
                PinPanel::alloc(mtm),
                initWithContentRect: frame,
                styleMask: NSWindowStyleMask::Borderless | NSWindowStyleMask::NonactivatingPanel,
                backing: NSBackingStoreType::Buffered,
                defer: false
            ]
        };
        // SAFETY: false is the only value compatible with a window kept alive by `Retained`.
        unsafe { panel.setReleasedWhenClosed(false) };
        panel.setHasShadow(true);
        panel.setOpaque(false);
        panel.setBackgroundColor(Some(&NSColor::clearColor()));
        panel.setHidesOnDeactivate(false);
        panel.setLevel(NSStatusWindowLevel);
        panel.setCollectionBehavior(
            NSWindowCollectionBehavior::CanJoinAllSpaces
                | NSWindowCollectionBehavior::FullScreenAuxiliary,
        );
        let ivars = PinIvars {
            image,
            menu: RefCell::new(None),
            on_close: RefCell::new(None),
        };
        // SAFETY: designated initializer with valid arguments on the main thread.
        let view: Retained<PinView> = unsafe {
            msg_send![
                super(mtm.alloc::<PinView>().set_ivars(ivars)),
                initWithFrame: NSRect::new(NSPoint::ZERO, size)
            ]
        };
        view.setAutoresizingMask(
            objc2_app_kit::NSAutoresizingMaskOptions::ViewWidthSizable
                | objc2_app_kit::NSAutoresizingMaskOptions::ViewHeightSizable,
        );
        panel.setContentView(Some(&view));

        let actions = Rc::new(actions);
        let menu = NSMenu::new(mtm);
        let mut targets = Vec::new();
        type Act = fn(&PinActions, &PinView);
        let entries: [(&str, Act); 3] = [
            (&texts.copy, |a, _| (a.on_copy)()),
            (&texts.save, |a, _| (a.on_save)()),
            (&texts.close, |_, v| v.close()),
        ];
        for (title, act) in entries {
            let (actions, view_ref) = (Rc::clone(&actions), view.clone());
            let target = ActionTarget::new(mtm, move || act(&actions, &view_ref));
            // SAFETY: `fire:` takes the sender as its only argument; the target lives with the pin.
            let item = unsafe {
                NSMenuItem::initWithTitle_action_keyEquivalent(
                    NSMenuItem::alloc(mtm),
                    &NSString::from_str(title),
                    Some(sel!(fire:)),
                    &NSString::from_str(""),
                )
            };
            // SAFETY: as above.
            unsafe { item.setTarget(Some(&target)) };
            menu.addItem(&item);
            targets.push(target);
        }
        *view.ivars().menu.borrow_mut() = Some(menu);
        {
            let actions = Rc::clone(&actions);
            *view.ivars().on_close.borrow_mut() = Some(Rc::new(move || (actions.on_closed)()));
        }
        panel.makeKeyAndOrderFront(None);
        panel.makeFirstResponder(Some(&view));
        Rc::new(Self {
            panel,
            _view: view,
            _targets: targets,
        })
    }

    pub fn close(&self) {
        self.panel.orderOut(None);
    }
}

/// The notice panel, its view and how many notices it has shown (so an old timer never hides a new one).
type ToastSlot = Option<(Retained<NSPanel>, Retained<ToastView>, u64)>;

thread_local! {
    static TOAST: RefCell<ToastSlot> = const { RefCell::new(None) };
}

pub struct ToastIvars {
    text: RefCell<String>,
}

define_class!(
    // SAFETY: NSView allows subclassing; the ivars are only touched on the main thread and need no Drop.
    #[unsafe(super(NSView))]
    #[thread_kind = MainThreadOnly]
    #[name = "FigToastView"]
    #[ivars = ToastIvars]
    struct ToastView;

    impl ToastView {
        #[unsafe(method(drawRect:))]
        fn draw_rect(&self, _dirty: NSRect) {
            let bounds = self.bounds();
            NSColor::colorWithWhite_alpha(0.08, 0.92).setFill();
            NSBezierPath::bezierPathWithRoundedRect_xRadius_yRadius(bounds, 14.0, 14.0).fill();
            let attrs = text_attributes(&NSFont::systemFontOfSize(13.0), &NSColor::whiteColor(), Fit::Tail, true);
            draw_text(
                &self.ivars().text.borrow(),
                NSRect::new(NSPoint::new(12.0, 8.0), NSSize::new(bounds.size.width - 24.0, 18.0)),
                &attrs,
            );
        }
    }
);

/// The frame of the main screen in Cocoa coordinates, for placing a picture that was not taken on a screen.
pub fn main_screen_frame(mtm: MainThreadMarker) -> NSRect {
    NSScreen::mainScreen(mtm).map_or(NSRect::ZERO, |screen| screen.frame())
}

/// A short notice near the top of the main screen that goes away by itself.
pub fn toast(mtm: MainThreadMarker, text: &str) {
    let Some(screen) = NSScreen::screens(mtm).firstObject() else {
        return;
    };
    let visible = screen.visibleFrame();
    let width = (7.4 * text.chars().count() as f64 + 40.0).clamp(160.0, 560.0);
    let frame = NSRect::new(
        NSPoint::new(
            visible.origin.x + (visible.size.width - width) / 2.0,
            visible.origin.y + visible.size.height - 64.0,
        ),
        NSSize::new(width, 34.0),
    );
    let generation = TOAST.with(|slot| {
        let mut slot = slot.borrow_mut();
        let (panel, view, generation) = slot.get_or_insert_with(|| {
            let panel = panel(mtm, frame, NSStatusWindowLevel);
            panel.setIgnoresMouseEvents(true);
            let ivars = ToastIvars {
                text: RefCell::new(String::new()),
            };
            // SAFETY: designated initializer with valid arguments on the main thread.
            let view: Retained<ToastView> = unsafe {
                msg_send![
                    super(mtm.alloc::<ToastView>().set_ivars(ivars)),
                    initWithFrame: NSRect::new(NSPoint::ZERO, frame.size)
                ]
            };
            panel.setContentView(Some(&view));
            (panel, view, 0)
        });
        *view.ivars().text.borrow_mut() = text.to_owned();
        panel.setFrame_display(frame, true);
        view.setFrame(NSRect::new(NSPoint::ZERO, frame.size));
        view.setNeedsDisplay(true);
        panel.orderFrontRegardless();
        *generation += 1;
        *generation
    });
    app::after(TOAST_SECONDS, move || {
        TOAST.with(|slot| {
            if let Some((panel, _, current)) = slot.borrow().as_ref()
                && *current == generation
            {
                panel.orderOut(None);
            }
        });
    });
}

define_class!(
    // SAFETY: NSView allows subclassing; it only draws a border.
    #[unsafe(super(NSView))]
    #[thread_kind = MainThreadOnly]
    #[name = "FigAreaBorder"]
    struct BorderView;

    impl BorderView {
        #[unsafe(method(drawRect:))]
        fn draw_rect(&self, _dirty: NSRect) {
            let path = NSBezierPath::bezierPathWithRect(outset(self.bounds(), -1.5));
            path.setLineWidth(2.0);
            let dashes = [6.0, 4.0];
            // SAFETY: the pattern has the two entries counted.
            unsafe { path.setLineDash_count_phase(dashes.as_ptr(), 2, 0.0) };
            NSColor::colorWithRed_green_blue_alpha(0.3, 0.6, 1.0, 0.95).setStroke();
            path.stroke();
        }
    }
);

pub struct KeysIvars {
    on_done: Rc<dyn Fn()>,
    on_cancel: Rc<dyn Fn()>,
}

define_class!(
    // SAFETY: NSView allows subclassing; the ivars are only touched on the main thread and need no Drop.
    #[unsafe(super(NSView))]
    #[thread_kind = MainThreadOnly]
    #[name = "FigAreaControls"]
    #[ivars = KeysIvars]
    struct ControlsView;

    impl ControlsView {
        #[unsafe(method(acceptsFirstResponder))]
        fn accepts_first_responder(&self) -> bool {
            true
        }

        #[unsafe(method(drawRect:))]
        fn draw_rect(&self, _dirty: NSRect) {
            NSColor::colorWithWhite_alpha(0.08, 0.92).setFill();
            NSBezierPath::bezierPathWithRoundedRect_xRadius_yRadius(self.bounds(), 10.0, 10.0).fill();
        }

        #[unsafe(method(keyDown:))]
        fn key_down(&self, event: &NSEvent) {
            match event.keyCode() {
                53 => (self.ivars().on_cancel)(),
                36 | 76 => (self.ivars().on_done)(),
                _ => {}
            }
        }
    }
);

define_class!(
    // SAFETY: NSPanel allows subclassing; this one only allows key status, for Escape and Return.
    #[unsafe(super(NSPanel))]
    #[thread_kind = MainThreadOnly]
    #[name = "FigAreaPanel"]
    struct KeyPanel;

    impl KeyPanel {
        #[unsafe(method(canBecomeKeyWindow))]
        fn can_become_key_window(&self) -> bool {
            true
        }
    }
);

#[derive(Debug, Clone, Default)]
pub struct SessionTexts {
    pub done: String,
    pub cancel: String,
}

/// Where the session bar goes: under the area, else over it, else inside its bottom edge (a whole
/// display has no room around it).
fn bar_origin(area: NSRect, bar: NSSize, screen_bottom: f64, screen_top: f64) -> NSPoint {
    let x = area.origin.x + (area.size.width - bar.width) / 2.0;
    let below = area.origin.y - bar.height - 12.0;
    let above = area.origin.y + area.size.height + 12.0;
    let y = if below >= screen_bottom {
        below
    } else if above + bar.height <= screen_top {
        above
    } else {
        area.origin.y.max(screen_bottom) + 24.0
    };
    NSPoint::new(x, y)
}

/// Shown while a scrolling capture or a recording runs: a border around the area (clicks and scrolling
/// go through it) and a small bar with the progress, Done and Cancel. When the bar takes the keyboard,
/// Return is Done and Escape is Cancel; a recording leaves the keyboard to the apps being recorded.
pub struct AreaSession {
    border: Retained<NSPanel>,
    controls: Retained<KeyPanel>,
    status: Retained<objc2_app_kit::NSTextField>,
    _targets: Vec<Retained<ActionTarget>>,
}

impl AreaSession {
    /// `area` is in AppKit screen coordinates.
    pub fn show(
        mtm: MainThreadMarker,
        area: NSRect,
        texts: &SessionTexts,
        takes_keys: bool,
        on_done: impl Fn() + 'static,
        on_cancel: impl Fn() + 'static,
    ) -> Rc<Self> {
        let border_frame = outset(area, 3.0);
        let border = panel(mtm, border_frame, NSStatusWindowLevel);
        border.setHasShadow(false);
        border.setIgnoresMouseEvents(true);
        // SAFETY: designated initializer with valid arguments on the main thread.
        let view: Retained<BorderView> = unsafe {
            msg_send![
                super(mtm.alloc::<BorderView>().set_ivars(())),
                initWithFrame: NSRect::new(NSPoint::ZERO, border_frame.size)
            ]
        };
        border.setContentView(Some(&view));

        let size = NSSize::new(300.0, 40.0);
        let visible: Vec<NSRect> = NSScreen::screens(mtm)
            .iter()
            .map(|s| s.visibleFrame())
            .collect();
        let screen_bottom = visible
            .iter()
            .map(|v| v.origin.y)
            .fold(f64::INFINITY, f64::min);
        let screen_top = visible
            .iter()
            .map(|v| v.origin.y + v.size.height)
            .fold(f64::NEG_INFINITY, f64::max);
        let frame = NSRect::new(bar_origin(area, size, screen_bottom, screen_top), size);
        // SAFETY: designated initializer with valid arguments on the main thread.
        let controls: Retained<KeyPanel> = unsafe {
            msg_send![
                KeyPanel::alloc(mtm),
                initWithContentRect: frame,
                styleMask: NSWindowStyleMask::Borderless | NSWindowStyleMask::NonactivatingPanel,
                backing: NSBackingStoreType::Buffered,
                defer: false
            ]
        };
        // SAFETY: false is the only value compatible with a window kept alive by `Retained`.
        unsafe { controls.setReleasedWhenClosed(false) };
        controls.setOpaque(false);
        controls.setBackgroundColor(Some(&NSColor::clearColor()));
        controls.setHidesOnDeactivate(false);
        controls.setLevel(NSStatusWindowLevel);
        let on_done: Rc<dyn Fn()> = Rc::new(on_done);
        let on_cancel: Rc<dyn Fn()> = Rc::new(on_cancel);
        let ivars = KeysIvars {
            on_done: Rc::clone(&on_done),
            on_cancel: Rc::clone(&on_cancel),
        };
        // SAFETY: designated initializer with valid arguments on the main thread.
        let content: Retained<ControlsView> = unsafe {
            msg_send![
                super(mtm.alloc::<ControlsView>().set_ivars(ivars)),
                initWithFrame: NSRect::new(NSPoint::ZERO, frame.size)
            ]
        };
        controls.setContentView(Some(&content));
        let status = objc2_app_kit::NSTextField::labelWithString(&NSString::from_str(""), mtm);
        status.setFont(Some(&NSFont::monospacedDigitSystemFontOfSize_weight(
            12.0, 0.0,
        )));
        status.setTextColor(Some(&NSColor::whiteColor()));
        status.setFrame(NSRect::new(
            NSPoint::new(14.0, 11.0),
            NSSize::new(120.0, 18.0),
        ));
        content.addSubview(&status);
        let done_target = ActionTarget::new(mtm, move || on_done());
        let cancel_target = ActionTarget::new(mtm, move || on_cancel());
        content.addSubview(&button(
            mtm,
            &texts.done,
            NSRect::new(NSPoint::new(140.0, 8.0), NSSize::new(72.0, BUTTON_HEIGHT)),
            &done_target,
        ));
        content.addSubview(&button(
            mtm,
            &texts.cancel,
            NSRect::new(NSPoint::new(216.0, 8.0), NSSize::new(72.0, BUTTON_HEIGHT)),
            &cancel_target,
        ));
        border.orderFrontRegardless();
        if takes_keys {
            controls.makeKeyAndOrderFront(None);
            controls.makeFirstResponder(Some(&content));
        } else {
            controls.orderFrontRegardless();
        }
        Rc::new(Self {
            border,
            controls,
            status,
            _targets: vec![done_target, cancel_target],
        })
    }

    pub fn set_status(&self, text: &str) {
        self.status.setStringValue(&NSString::from_str(text));
    }

    pub fn close(&self) {
        self.border.orderOut(None);
        self.controls.orderOut(None);
    }
}

/// Asks where to save a file, starting from `directory` with `name` filled in; `None` if cancelled.
pub fn ask_save_path(mtm: MainThreadMarker, directory: &Path, name: &str) -> Option<PathBuf> {
    let panel = NSSavePanel::savePanel(mtm);
    panel.setNameFieldStringValue(&NSString::from_str(name));
    panel.setDirectoryURL(Some(&NSURL::fileURLWithPath(&NSString::from_str(
        &directory.to_string_lossy(),
    ))));
    panel.setLevel(NSStatusWindowLevel + 1);
    if panel.runModal() != NSModalResponseOK {
        return None;
    }
    panel.URL()?.path().map(|p| PathBuf::from(p.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_session_bar_goes_under_then_over_then_inside_the_area() {
        let bar = NSSize::new(300.0, 40.0);
        let area = |y, h| NSRect::new(NSPoint::new(100.0, y), NSSize::new(600.0, h));
        assert_eq!(
            bar_origin(area(200.0, 300.0), bar, 0.0, 1050.0),
            NSPoint::new(250.0, 148.0)
        );
        assert_eq!(
            bar_origin(area(20.0, 300.0), bar, 0.0, 1050.0),
            NSPoint::new(250.0, 332.0)
        );
        assert_eq!(
            bar_origin(area(0.0, 1080.0), bar, 0.0, 1050.0),
            NSPoint::new(250.0, 24.0)
        );
    }

    #[test]
    fn thumbnails_keep_their_aspect_and_never_grow() {
        let f = fit(NSSize::new(1920.0, 1080.0), NSSize::new(280.0, 170.0));
        assert_eq!((f.width, f.height), (280.0, 157.0));
        let tall = fit(NSSize::new(400.0, 2000.0), NSSize::new(280.0, 170.0));
        assert_eq!((tall.width, tall.height), (34.0, 170.0));
        let small = fit(NSSize::new(50.0, 20.0), NSSize::new(280.0, 170.0));
        assert_eq!((small.width, small.height), (50.0, 20.0));
    }
}
