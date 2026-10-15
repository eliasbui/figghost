//! The small pill that shows how many copied items wait in the stack. It never takes the keyboard or the
//! pointer: a non-activating panel that ignores the mouse.

use crate::{
    panel::FigPanel,
    style::{self, Weight},
    widgets::{Fit, draw_text, text_attributes},
    workspace,
};
use objc2::{DefinedClass, MainThreadMarker, MainThreadOnly, define_class, msg_send, rc::Retained};
use objc2_app_kit::{NSBitmapImageFileType, NSScreen, NSStringDrawing, NSView};
use objc2_foundation::{NSDictionary, NSPoint, NSRect, NSSize};
use std::cell::RefCell;

const HEIGHT: f64 = 34.0;
const PADDING: f64 = 14.0;
const ICON: f64 = 14.0;
const GAP: f64 = 8.0;
/// Distance from the bottom of the usable screen area.
const MARGIN_BOTTOM: f64 = 28.0;

pub struct PillIvars {
    text: RefCell<String>,
    opaque: std::cell::Cell<bool>,
}

define_class!(
    // SAFETY: NSView allows subclassing; the ivars are only touched on the main thread and need no Drop.
    #[unsafe(super(NSView))]
    #[thread_kind = MainThreadOnly]
    #[name = "FigStackPill"]
    #[ivars = PillIvars]
    struct PillView;

    impl PillView {
        #[unsafe(method(isFlipped))]
        fn is_flipped(&self) -> bool {
            true
        }

        #[unsafe(method(drawRect:))]
        fn draw_rect(&self, _dirty: NSRect) {
            let bounds = self.bounds();
            let fill = if self.ivars().opaque.get() { 0.98 } else { 0.88 };
            style::fill_capsule(bounds, &style::black(fill));
            let edge = NSRect::new(
                NSPoint::new(0.5, 0.5),
                NSSize::new(bounds.size.width - 1.0, bounds.size.height - 1.0),
            );
            style::stroke_rounded(edge, HEIGHT / 2.0 - 0.5, &style::white(0.22), 1.0);
            style::draw_symbol(
                "square.stack.3d.up.fill",
                NSPoint::new(PADDING + ICON / 2.0, HEIGHT / 2.0),
                ICON,
                &style::white(0.95),
            );
            let attrs = text_attributes(
                &style::font(13.0, Weight::Medium),
                &style::white(style::TEXT),
                Fit::Clip,
                false,
            );
            let left = PADDING + ICON + GAP;
            draw_text(
                &self.ivars().text.borrow(),
                NSRect::new(
                    NSPoint::new(left, (HEIGHT - 16.0) / 2.0),
                    NSSize::new(bounds.size.width - left - PADDING, 16.0),
                ),
                &attrs,
            );
        }
    }
);

fn text_width(text: &str) -> f64 {
    // The text is drawn in a medium 13 point font; a generous average keeps the pill from clipping it.
    let attrs = text_attributes(
        &style::font(13.0, Weight::Medium),
        &style::white(1.0),
        Fit::Clip,
        false,
    );
    // SAFETY: `attrs` holds valid attribute objects for these keys.
    let size =
        unsafe { objc2_foundation::NSString::from_str(text).sizeWithAttributes(Some(&attrs)) };
    size.width.ceil()
}

pub struct Pill {
    panel: Retained<FigPanel>,
    view: Retained<PillView>,
}

impl Pill {
    pub fn new(mtm: MainThreadMarker) -> Self {
        let frame = NSRect::new(NSPoint::ZERO, NSSize::new(120.0, HEIGHT));
        let panel = FigPanel::new(mtm, frame);
        panel.setIgnoresMouseEvents(true);
        panel.setHasShadow(true);
        let this = mtm.alloc::<PillView>().set_ivars(PillIvars {
            text: RefCell::new(String::new()),
            opaque: std::cell::Cell::new(false),
        });
        // SAFETY: designated initializer with valid arguments on the main thread.
        let view: Retained<PillView> = unsafe { msg_send![super(this), initWithFrame: frame] };
        panel.setContentView(Some(&view));
        Self { panel, view }
    }

    /// Sets the words and resizes the pill to them, keeping it centred at the bottom of the screen.
    pub fn set_text(&self, text: &str) {
        let width = PADDING * 2.0 + ICON + GAP + text_width(text);
        *self.view.ivars().text.borrow_mut() = text.to_owned();
        self.view
            .ivars()
            .opaque
            .set(workspace::display_options().reduce_transparency);
        self.panel.setContentSize(NSSize::new(width, HEIGHT));
        self.view.setNeedsDisplay(true);
        self.panel.setFrameOrigin(self.origin(width));
    }

    fn origin(&self, width: f64) -> NSPoint {
        // For tests: far outside every screen, so a pill that appears on its own is never seen.
        if std::env::var_os("FIGGHOST_NOTCH_OFFSCREEN").is_some() {
            return NSPoint::new(-30_000.0, 100.0);
        }
        let area = MainThreadMarker::new()
            .and_then(NSScreen::mainScreen)
            .map(|s| s.visibleFrame())
            .unwrap_or_else(|| NSRect::new(NSPoint::ZERO, NSSize::new(1_440.0, 900.0)));
        NSPoint::new(
            area.origin.x + (area.size.width - width) / 2.0,
            area.origin.y + MARGIN_BOTTOM,
        )
    }

    /// Shows the pill without activating this app or taking the keyboard.
    pub fn show(&self) {
        self.panel.orderFrontRegardless();
    }

    pub fn hide(&self) {
        self.panel.orderOut(None);
    }

    pub fn is_visible(&self) -> bool {
        self.panel.isVisible()
    }

    /// The pill as a PNG, drawn without showing the window; for checking its look.
    pub fn snapshot_png(&self) -> Option<Vec<u8>> {
        let bounds = self.view.bounds();
        let rep = self.view.bitmapImageRepForCachingDisplayInRect(bounds)?;
        self.view.cacheDisplayInRect_toBitmapImageRep(bounds, &rep);
        // SAFETY: an empty properties dictionary is valid for PNG output.
        let png = unsafe {
            rep.representationUsingType_properties(NSBitmapImageFileType::PNG, &NSDictionary::new())
        }?;
        Some(png.to_vec())
    }

    pub fn frame(&self) -> NSRect {
        self.panel.frame()
    }
}
