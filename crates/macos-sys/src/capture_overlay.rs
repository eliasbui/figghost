//! The selection overlay: every display shows its frozen picture, under the system crosshair
//! cursor, dimmed only while a selection is drawn. Drag to pick an area; Space switches to picking a window, which lights up under the
//! pointer; a click without a drag takes the window under the pointer; Return takes the whole display;
//! Escape or a right click cancels. A presentation asks for a whole display instead: the one under the pointer lights
//! up and a click takes it.
//!
//! The overlay panels take keyboard focus without activating the app (macOS no longer lets an app
//! activate itself), so the app in front stays in front.

use crate::{
    screen_capture::Frozen,
    widgets::{Fit, draw_text, text_attributes},
};
use objc2::{
    AnyThread, DefinedClass, MainThreadMarker, MainThreadOnly, define_class, msg_send, rc::Retained,
};
use objc2_app_kit::{
    NSApplication, NSBackingStoreType, NSBezierPath, NSColor, NSCursor, NSEvent, NSFont,
    NSFontWeightMedium, NSImage, NSPanel, NSScreenSaverWindowLevel, NSTrackingArea,
    NSTrackingAreaOptions, NSView, NSWindingRule, NSWindowCollectionBehavior, NSWindowStyleMask,
};
use objc2_foundation::{NSObjectProtocol, NSPoint, NSRect, NSSize};
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
};

const DIM: f64 = 0.38;
const LABEL_HEIGHT: f64 = 20.0;

/// What may be picked.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    /// An area, a window or a whole display.
    Any,
    /// The same, starting with windows lit up under the pointer.
    Window,
    /// Only an area (text recognition, scrolling capture, recording a region).
    AreaOnly,
    /// Only a whole display, lit up under the pointer (what to present).
    Display,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Picked {
    /// An area of one display, in that display's points with the origin at its bottom-left.
    Area {
        display: usize,
        rect: NSRect,
    },
    Window(u32),
    Display(usize),
    Cancelled,
}

type Done = Rc<RefCell<Option<Box<dyn FnOnce(Picked)>>>>;

/// Shared by every display's view: what is being picked and how to finish.
struct Session {
    mode: Mode,
    window_mode: Cell<bool>,
    done: Done,
    /// Window frames from front to back, in AppKit screen coordinates.
    windows: Vec<(u32, NSRect)>,
}

impl Session {
    fn finish(&self, picked: Picked) {
        if let Some(done) = self.done.borrow_mut().take() {
            done(picked);
        }
    }

    fn window_at(&self, global: NSPoint) -> Option<(u32, NSRect)> {
        self.windows.iter().copied().find(|(_, f)| {
            global.x >= f.origin.x
                && global.x < f.origin.x + f.size.width
                && global.y >= f.origin.y
                && global.y < f.origin.y + f.size.height
        })
    }
}

pub struct ViewIvars {
    display: usize,
    origin: NSPoint,
    picture: Retained<NSImage>,
    /// Pixels per point of the picture.
    scale: f64,
    session: Rc<Session>,
    pointer: Cell<Option<NSPoint>>,
    drag_start: Cell<Option<NSPoint>>,
}

fn rect_between(a: NSPoint, b: NSPoint) -> NSRect {
    NSRect::new(
        NSPoint::new(a.x.min(b.x), a.y.min(b.y)),
        NSSize::new((a.x - b.x).abs(), (a.y - b.y).abs()),
    )
}

/// The part of `r` inside `bounds`.
fn clip(r: NSRect, bounds: NSRect) -> NSRect {
    let x = r.origin.x.max(bounds.origin.x);
    let y = r.origin.y.max(bounds.origin.y);
    let right = (r.origin.x + r.size.width).min(bounds.origin.x + bounds.size.width);
    let top = (r.origin.y + r.size.height).min(bounds.origin.y + bounds.size.height);
    NSRect::new(
        NSPoint::new(x, y),
        NSSize::new((right - x).max(0.0), (top - y).max(0.0)),
    )
}

fn inset(r: NSRect, by: f64) -> NSRect {
    NSRect::new(
        NSPoint::new(r.origin.x + by, r.origin.y + by),
        NSSize::new(
            (r.size.width - 2.0 * by).max(0.0),
            (r.size.height - 2.0 * by).max(0.0),
        ),
    )
}

fn bounds_contain(bounds: NSRect, p: NSPoint) -> bool {
    p.x >= bounds.origin.x
        && p.y >= bounds.origin.y
        && p.x < bounds.origin.x + bounds.size.width
        && p.y < bounds.origin.y + bounds.size.height
}

fn is_click(r: NSRect) -> bool {
    r.size.width < 3.0 && r.size.height < 3.0
}

define_class!(
    // SAFETY: NSView allows subclassing; the ivars are only touched on the main thread and need no Drop.
    #[unsafe(super(NSView))]
    #[thread_kind = MainThreadOnly]
    #[name = "FigOverlayView"]
    #[ivars = ViewIvars]
    struct OverlayView;

    impl OverlayView {
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
            self.draw();
        }

        #[unsafe(method(mouseMoved:))]
        fn mouse_moved(&self, event: &NSEvent) {
            self.track(event);
        }

        #[unsafe(method(mouseDown:))]
        fn mouse_down(&self, event: &NSEvent) {
            let point = self.local(event);
            self.ivars().pointer.set(Some(point));
            let session = &self.ivars().session;
            if !session.window_mode.get() && session.mode != Mode::Display {
                self.ivars().drag_start.set(Some(point));
            }
        }

        #[unsafe(method(mouseDragged:))]
        fn mouse_dragged(&self, event: &NSEvent) {
            self.track(event);
        }

        #[unsafe(method(mouseUp:))]
        fn mouse_up(&self, event: &NSEvent) {
            let point = self.local(event);
            let ivars = self.ivars();
            let session = &ivars.session;
            let start = ivars.drag_start.take();
            if session.mode == Mode::Display {
                session.finish(Picked::Display(ivars.display));
                return;
            }
            match start.map(|s| rect_between(s, point)) {
                Some(rect) if !is_click(rect) => session.finish(Picked::Area {
                    display: ivars.display,
                    rect: clip(rect, self.bounds()),
                }),
                _ if session.mode != Mode::AreaOnly => {
                    if let Some((id, _)) = session.window_at(self.global(point)) {
                        session.finish(Picked::Window(id));
                    }
                }
                _ => {}
            }
            self.setNeedsDisplay(true);
        }

        #[unsafe(method(rightMouseDown:))]
        fn right_mouse_down(&self, _event: &NSEvent) {
            self.ivars().session.finish(Picked::Cancelled);
        }

        #[unsafe(method(keyDown:))]
        fn key_down(&self, event: &NSEvent) {
            let session = &self.ivars().session;
            match event.keyCode() {
                53 => session.finish(Picked::Cancelled),
                49 if matches!(session.mode, Mode::Any | Mode::Window)
                    && self.ivars().drag_start.get().is_none() =>
                {
                    session.window_mode.set(!session.window_mode.get());
                    redraw_all(self);
                }
                36 | 76 if session.mode != Mode::AreaOnly => {
                    session.finish(Picked::Display(self.ivars().display));
                }
                _ => {}
            }
        }

        #[unsafe(method(cursorUpdate:))]
        fn cursor_update(&self, _event: &NSEvent) {
            NSCursor::crosshairCursor().set();
        }
    }

    unsafe impl NSObjectProtocol for OverlayView {}
);

/// Every overlay view, so a mode switch on one display redraws the others.
fn redraw_all(view: &OverlayView) {
    if let Some(window) = view.window() {
        let app = NSApplication::sharedApplication(view.mtm());
        for other in app.windows().iter() {
            if other.level() == window.level()
                && let Some(content) = other.contentView()
            {
                content.setNeedsDisplay(true);
            }
        }
    }
}

impl OverlayView {
    fn local(&self, event: &NSEvent) -> NSPoint {
        self.convertPoint_fromView(event.locationInWindow(), None)
    }

    fn global(&self, local: NSPoint) -> NSPoint {
        let origin = self.ivars().origin;
        NSPoint::new(local.x + origin.x, local.y + origin.y)
    }

    fn track(&self, event: &NSEvent) {
        self.ivars().pointer.set(Some(self.local(event)));
        if let Some(window) = self.window()
            && !window.isKeyWindow()
        {
            window.makeKeyWindow();
        }
        NSCursor::crosshairCursor().set();
        if self.ivars().session.mode == Mode::Display {
            redraw_all(self);
        } else {
            self.setNeedsDisplay(true);
        }
    }

    fn dim(&self, hole: Option<NSRect>) {
        NSColor::colorWithWhite_alpha(0.0, DIM).setFill();
        let path = NSBezierPath::bezierPathWithRect(self.bounds());
        if let Some(hole) = hole {
            path.appendBezierPathWithRect(hole);
            path.setWindingRule(NSWindingRule::EvenOdd);
        }
        path.fill();
    }

    fn draw(&self) {
        let ivars = self.ivars();
        let bounds = self.bounds();
        ivars.picture.drawInRect(bounds);
        let session = &ivars.session;
        let pointer = ivars.pointer.get();

        if session.mode == Mode::Display {
            // The display under the pointer is lit, the others dimmed; each view asks where the pointer is, since only
            // the one under it hears it move.
            let global = NSEvent::mouseLocation();
            let here = NSPoint::new(global.x - ivars.origin.x, global.y - ivars.origin.y);
            if bounds_contain(bounds, here) {
                NSColor::colorWithRed_green_blue_alpha(0.25, 0.55, 1.0, 0.22).setFill();
                NSBezierPath::fillRect(bounds);
                NSColor::colorWithRed_green_blue_alpha(0.35, 0.65, 1.0, 0.9).setStroke();
                let border = NSBezierPath::bezierPathWithRect(inset(bounds, 2.0));
                border.setLineWidth(4.0);
                border.stroke();
            } else {
                self.dim(None);
            }
        } else if session.window_mode.get() {
            let hovered = pointer.and_then(|p| session.window_at(self.global(p)));
            let hole = hovered.map(|(_, f)| {
                NSRect::new(
                    NSPoint::new(f.origin.x - ivars.origin.x, f.origin.y - ivars.origin.y),
                    f.size,
                )
            });
            self.dim(hole);
            if let Some(hole) = hole {
                NSColor::colorWithRed_green_blue_alpha(0.25, 0.55, 1.0, 0.22).setFill();
                NSBezierPath::fillRect(hole);
                NSColor::colorWithRed_green_blue_alpha(0.35, 0.65, 1.0, 0.9).setStroke();
                let border = NSBezierPath::bezierPathWithRect(inset(hole, 1.0));
                border.setLineWidth(2.0);
                border.stroke();
            }
        } else if let (Some(start), Some(now)) = (ivars.drag_start.get(), pointer) {
            let selection = clip(rect_between(start, now), bounds);
            self.dim(Some(selection));
            NSColor::whiteColor().setStroke();
            let border = NSBezierPath::bezierPathWithRect(selection);
            border.setLineWidth(1.0);
            border.stroke();
            self.size_label(selection);
        } else {
            if let Some(p) = pointer {
                self.coords(p);
            }
        }
    }

    fn label(&self, text: &str, at: NSPoint) {
        // SAFETY: extern static weight constant.
        let font =
            NSFont::monospacedDigitSystemFontOfSize_weight(11.0, unsafe { NSFontWeightMedium });
        let attrs = text_attributes(&font, &NSColor::whiteColor(), Fit::Clip, true);
        let width = 8.0 * text.chars().count() as f64 + 16.0;
        let rect = NSRect::new(at, NSSize::new(width, LABEL_HEIGHT));
        NSColor::colorWithWhite_alpha(0.0, 0.75).setFill();
        NSBezierPath::bezierPathWithRoundedRect_xRadius_yRadius(rect, 5.0, 5.0).fill();
        draw_text(
            text,
            NSRect::new(
                NSPoint::new(rect.origin.x, rect.origin.y + 3.0),
                NSSize::new(width, 15.0),
            ),
            &attrs,
        );
    }

    fn size_label(&self, selection: NSRect) {
        let scale = self.ivars().scale;
        let text = format!(
            "{} x {}",
            (selection.size.width * scale).round(),
            (selection.size.height * scale).round()
        );
        let below = selection.origin.y - LABEL_HEIGHT - 6.0;
        let y = if below > 4.0 {
            below
        } else {
            selection.origin.y + selection.size.height + 6.0
        };
        self.label(&text, NSPoint::new(selection.origin.x, y));
    }

    /// The pixel position beside the system crosshair cursor.
    fn coords(&self, p: NSPoint) {
        let scale = self.ivars().scale;
        let text = format!(
            "{}, {}",
            (p.x * scale).floor(),
            ((self.bounds().size.height - p.y) * scale).floor()
        );
        self.label(&text, NSPoint::new(p.x + 14.0, p.y - LABEL_HEIGHT - 10.0));
    }
}

define_class!(
    // SAFETY: NSPanel allows subclassing; this one only allows key status without a title bar.
    #[unsafe(super(NSPanel))]
    #[thread_kind = MainThreadOnly]
    #[name = "FigOverlayPanel"]
    struct OverlayPanel;

    impl OverlayPanel {
        #[unsafe(method(canBecomeKeyWindow))]
        fn can_become_key_window(&self) -> bool {
            true
        }
    }
);

/// Keeps the overlay on screen; it closes by itself once something is picked.
pub struct Overlay {
    panels: RefCell<Vec<Retained<OverlayPanel>>>,
}

impl Overlay {
    pub fn close(&self) {
        for panel in self.panels.borrow_mut().drain(..) {
            panel.orderOut(None);
        }
    }
}

impl Drop for Overlay {
    fn drop(&mut self) {
        self.close();
    }
}

/// Shows the overlay over every frozen display; `done` gets what was picked once, then the overlay is
/// gone.
pub fn pick(
    mtm: MainThreadMarker,
    frozen: &Frozen,
    mode: Mode,
    _hint: &str,
    done: impl FnOnce(Picked) + 'static,
) -> Rc<Overlay> {
    let overlay = Rc::new(Overlay {
        panels: RefCell::new(Vec::new()),
    });
    let weak = Rc::downgrade(&overlay);
    let done: Done = Rc::new(RefCell::new(Some(Box::new(move |picked: Picked| {
        if let Some(overlay) = weak.upgrade() {
            overlay.close();
        }
        done(picked);
    }))));
    let session = Rc::new(Session {
        mode,
        window_mode: Cell::new(mode == Mode::Window),
        done,
        windows: frozen.windows.iter().map(|w| (w.id, w.frame)).collect(),
    });
    let pointer = NSEvent::mouseLocation();
    let mut panels = Vec::new();
    for (index, display) in frozen.displays.iter().enumerate() {
        let frame = display.frame;
        // SAFETY: designated initializer with valid arguments on the main thread.
        let panel: Retained<OverlayPanel> = unsafe {
            msg_send![
                OverlayPanel::alloc(mtm),
                initWithContentRect: frame,
                styleMask: NSWindowStyleMask::Borderless | NSWindowStyleMask::NonactivatingPanel,
                backing: NSBackingStoreType::Buffered,
                defer: false
            ]
        };
        // SAFETY: false is the only value compatible with a window kept alive by `Retained`.
        unsafe { panel.setReleasedWhenClosed(false) };
        panel.setLevel(NSScreenSaverWindowLevel);
        panel.setCollectionBehavior(
            NSWindowCollectionBehavior::CanJoinAllSpaces
                | NSWindowCollectionBehavior::FullScreenAuxiliary
                | NSWindowCollectionBehavior::Stationary
                | NSWindowCollectionBehavior::IgnoresCycle,
        );
        panel.setHidesOnDeactivate(false);
        panel.setAcceptsMouseMovedEvents(true);
        panel.setOpaque(true);
        let local = NSPoint::new(pointer.x - frame.origin.x, pointer.y - frame.origin.y);
        let inside = local.x >= 0.0
            && local.y >= 0.0
            && local.x < frame.size.width
            && local.y < frame.size.height;
        let ivars = ViewIvars {
            display: index,
            origin: frame.origin,
            picture: display.shot.ns_image(),
            scale: display.shot.scale(),
            session: Rc::clone(&session),
            pointer: Cell::new(inside.then_some(local)),
            drag_start: Cell::new(None),
        };
        // SAFETY: designated initializer with valid arguments on the main thread.
        let view: Retained<OverlayView> = unsafe {
            msg_send![
                super(mtm.alloc::<OverlayView>().set_ivars(ivars)),
                initWithFrame: NSRect::new(NSPoint::ZERO, frame.size)
            ]
        };
        panel.setContentView(Some(&view));
        // SAFETY: the view owns the tracking area and outlives it.
        let tracking = unsafe {
            NSTrackingArea::initWithRect_options_owner_userInfo(
                NSTrackingArea::alloc(),
                view.bounds(),
                NSTrackingAreaOptions::MouseMoved
                    | NSTrackingAreaOptions::CursorUpdate
                    | NSTrackingAreaOptions::ActiveAlways
                    | NSTrackingAreaOptions::InVisibleRect,
                Some(&view),
                None,
            )
        };
        view.addTrackingArea(&tracking);
        if inside || frozen.displays.len() == 1 {
            panel.makeKeyAndOrderFront(None);
            panel.makeFirstResponder(Some(&view));
        } else {
            panel.orderFrontRegardless();
        }
        panels.push(panel);
    }
    NSCursor::crosshairCursor().set();
    *overlay.panels.borrow_mut() = panels;
    overlay
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn drags_and_clicks_are_told_apart() {
        let r = rect_between(NSPoint::new(10.0, 40.0), NSPoint::new(4.0, 2.0));
        assert_eq!(
            (r.origin.x, r.origin.y, r.size.width, r.size.height),
            (4.0, 2.0, 6.0, 38.0)
        );
        assert!(!is_click(r));
        assert!(is_click(rect_between(
            NSPoint::new(1.0, 1.0),
            NSPoint::new(2.0, 3.0)
        )));
    }
}
