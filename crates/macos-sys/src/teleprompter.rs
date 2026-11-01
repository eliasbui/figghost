//! A teleprompter: text that scrolls by itself in a dark strip at the top of the screen, as close to the camera as the
//! menu bar lets it be, so that the eyes stay near the lens. The strip takes no click and no key (the app drives it with
//! shortcuts), floats above other windows, and may be left out of screenshots and recordings.

use crate::{app, capture_ui::panel};
use objc2::{MainThreadMarker, MainThreadOnly, rc::Retained};
use objc2_app_kit::{
    NSColor, NSFont, NSLineBreakMode, NSPanel, NSStatusWindowLevel, NSTextAlignment, NSTextField,
    NSView, NSWindowSharingType,
};
use objc2_foundation::{NSPoint, NSRect, NSSize, NSString};
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
    time::Instant,
};

const MARGIN: f64 = 24.0;
const PAD_TOP: f64 = 18.0;
const RADIUS: f64 = 18.0;
const FRAME_SECONDS: f64 = 1.0 / 60.0;

/// Where the scroll stands.
struct Scroll {
    /// How far the text has risen, in points.
    offset: Cell<f64>,
    /// Points a second.
    speed: Cell<f64>,
    paused: Cell<bool>,
    finished: Cell<bool>,
    last: Cell<Instant>,
    /// The size of the strip and of the text.
    view_height: Cell<f64>,
    text_height: Cell<f64>,
}

/// What the strip says and how it looks.
#[derive(Debug, Clone, Copy)]
pub struct Strip<'a> {
    pub text: &'a str,
    /// The size of the letters, in points.
    pub size: f64,
    /// How wide and how tall the strip is, in points.
    pub width: f64,
    pub height: f64,
    /// Points the text rises each second.
    pub speed: f64,
    /// The strip is left out of what is captured of the screen.
    pub hidden: bool,
}

pub struct Teleprompter {
    mtm: MainThreadMarker,
    panel: Retained<NSPanel>,
    view: Retained<NSView>,
    label: Retained<NSTextField>,
    scroll: Rc<Scroll>,
    timer: RefCell<Option<app::Timer>>,
}

/// The place of the text for a scroll: its top edge starts under the padding and rises with the offset.
fn text_y(view_height: f64, text_height: f64, offset: f64) -> f64 {
    view_height - PAD_TOP - text_height + offset
}

/// How far the text may rise before its last line has left the strip.
fn end_of(text_height: f64) -> f64 {
    text_height + PAD_TOP
}

impl Teleprompter {
    pub fn new(mtm: MainThreadMarker) -> Self {
        let frame = NSRect::new(NSPoint::ZERO, NSSize::new(600.0, 200.0));
        let panel = panel(mtm, frame, NSStatusWindowLevel);
        panel.setIgnoresMouseEvents(true);
        let view = NSView::initWithFrame(NSView::alloc(mtm), frame);
        let label = NSTextField::wrappingLabelWithString(&NSString::from_str(""), mtm);
        label.setSelectable(false);
        label.setAlignment(NSTextAlignment::Center);
        label.setLineBreakMode(NSLineBreakMode::ByWordWrapping);
        label.setMaximumNumberOfLines(0);
        label.setTextColor(Some(&NSColor::whiteColor()));
        label.setDrawsBackground(false);
        view.addSubview(&label);
        view.setWantsLayer(true);
        panel.setContentView(Some(&view));
        Self {
            mtm,
            panel,
            view,
            label,
            scroll: Rc::new(Scroll {
                offset: Cell::new(0.0),
                speed: Cell::new(40.0),
                paused: Cell::new(false),
                finished: Cell::new(false),
                last: Cell::new(Instant::now()),
                view_height: Cell::new(200.0),
                text_height: Cell::new(0.0),
            }),
            timer: RefCell::new(None),
        }
    }

    pub fn is_visible(&self) -> bool {
        self.panel.isVisible()
    }

    /// Shows the strip at the top of `visible` (the part of a screen the menu bar leaves).
    pub fn show(&self, visible: NSRect, strip: &Strip) {
        let Strip {
            text,
            size,
            width,
            height,
            speed,
            hidden,
        } = *strip;
        let width = width.min(visible.size.width - 16.0).max(240.0);
        let frame = NSRect::new(
            NSPoint::new(
                visible.origin.x + (visible.size.width - width) / 2.0,
                visible.origin.y + visible.size.height - height - 4.0,
            ),
            NSSize::new(width, height),
        );
        self.panel.setFrame_display(frame, true);
        self.view.setFrame(NSRect::new(NSPoint::ZERO, frame.size));
        if let Some(layer) = self.view.layer() {
            layer.setCornerRadius(RADIUS);
            layer.setMasksToBounds(true);
            let colour = NSColor::colorWithSRGBRed_green_blue_alpha(0.0, 0.0, 0.0, 0.86);
            layer.setBackgroundColor(Some(&colour.CGColor()));
        }
        self.panel.setSharingType(if hidden {
            NSWindowSharingType::None
        } else {
            NSWindowSharingType::ReadOnly
        });
        self.label
            .setFont(Some(&NSFont::boldSystemFontOfSize(size)));
        self.label.setStringValue(&NSString::from_str(text));
        let inner = width - 2.0 * MARGIN;
        self.label.setPreferredMaxLayoutWidth(inner);
        let fit = self.label.sizeThatFits(NSSize::new(inner, 1.0e7));
        let text_height = fit.height.ceil();
        self.scroll.view_height.set(height);
        self.scroll.text_height.set(text_height);
        self.label.setFrame(NSRect::new(
            NSPoint::new(MARGIN, 0.0),
            NSSize::new(inner, text_height),
        ));
        self.scroll.speed.set(speed);
        self.restart();
        self.scroll.paused.set(false);
        self.panel.orderFrontRegardless();
        let (scroll, label) = (Rc::clone(&self.scroll), self.label.clone());
        let _ = self.mtm;
        *self.timer.borrow_mut() = Some(app::every(FRAME_SECONDS, move || tick(&scroll, &label)));
    }

    pub fn hide(&self) {
        self.timer.borrow_mut().take();
        self.panel.orderOut(None);
    }

    /// Back to the first line.
    pub fn restart(&self) {
        self.scroll.offset.set(0.0);
        self.scroll.finished.set(false);
        self.scroll.last.set(Instant::now());
        place(&self.scroll, &self.label);
    }

    pub fn set_speed(&self, points_per_second: f64) {
        self.scroll.speed.set(points_per_second.clamp(0.0, 400.0));
    }

    pub fn speed(&self) -> f64 {
        self.scroll.speed.get()
    }

    /// Pauses or goes on; at the end, goes on from the start. Returns whether it is paused now.
    pub fn toggle_pause(&self) -> bool {
        if self.scroll.finished.get() {
            self.restart();
            self.scroll.paused.set(false);
            return false;
        }
        let paused = !self.scroll.paused.get();
        self.scroll.paused.set(paused);
        self.scroll.last.set(Instant::now());
        paused
    }

    pub fn finished(&self) -> bool {
        self.scroll.finished.get()
    }
}

/// The part of the screen the pointer is on that the menu bar and the Dock leave, or of the first screen when the pointer
/// is on none.
pub fn pointer_screen(mtm: MainThreadMarker) -> Option<NSRect> {
    use objc2_app_kit::{NSEvent, NSScreen};
    let pointer = NSEvent::mouseLocation();
    let screens = NSScreen::screens(mtm);
    let screen = screens
        .iter()
        .find(|s| {
            let f = s.frame();
            pointer.x >= f.origin.x
                && pointer.x < f.origin.x + f.size.width
                && pointer.y >= f.origin.y
                && pointer.y < f.origin.y + f.size.height
        })
        .or_else(|| screens.firstObject())?;
    Some(screen.visibleFrame())
}

fn place(scroll: &Scroll, label: &NSTextField) {
    let y = text_y(
        scroll.view_height.get(),
        scroll.text_height.get(),
        scroll.offset.get(),
    );
    label.setFrameOrigin(NSPoint::new(MARGIN, y));
}

fn tick(scroll: &Scroll, label: &NSTextField) {
    let now = Instant::now();
    let dt = now
        .duration_since(scroll.last.replace(now))
        .as_secs_f64()
        .min(0.1);
    if scroll.paused.get() || scroll.finished.get() {
        return;
    }
    // The first screen of text stands still for a moment before it moves, so that the eyes can find it.
    let offset = scroll.offset.get() + scroll.speed.get() * dt;
    let end = end_of(scroll.text_height.get());
    if offset >= end {
        scroll.offset.set(end);
        scroll.finished.set(true);
    } else {
        scroll.offset.set(offset);
    }
    place(scroll, label);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_text_starts_under_the_padding_and_rises_until_it_has_left() {
        assert_eq!(
            text_y(200.0, 600.0, 0.0),
            200.0 - PAD_TOP - 600.0,
            "its top edge is under the padding"
        );
        assert_eq!(
            text_y(200.0, 600.0, 100.0),
            text_y(200.0, 600.0, 0.0) + 100.0
        );
        let end = end_of(600.0);
        assert!(
            text_y(200.0, 600.0, end) > 200.0 - 1.0,
            "at the end the last line is above the strip"
        );
    }
}
