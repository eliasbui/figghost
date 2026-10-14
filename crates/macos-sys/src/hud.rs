//! A volume indicator that hangs from the top edge of the screen like the notch, shown while volume keys
//! are being handled.

use crate::{
    app,
    notch::NotchGeometry,
    panel::FigPanel,
    widgets::{Fit, draw_text, text_attributes},
};
use dispatch2::DispatchQueue;
use objc2::{DefinedClass, MainThreadMarker, MainThreadOnly, define_class, msg_send, rc::Retained};
use objc2_app_kit::{NSBezierPath, NSColor, NSFont, NSFontWeightMedium, NSLineCapStyle, NSView};
use objc2_foundation::{NSObjectProtocol, NSPoint, NSRect, NSSize};
use std::{
    cell::{Cell, RefCell},
    rc::{Rc, Weak},
};

const WIDTH: f64 = 280.0;
/// Height of the row with the speaker and the level, below the part that clears the notch cutout.
const ROW_HEIGHT: f64 = 38.0;
const HOLD_SECONDS: f64 = 1.4;
const CORNER_RADIUS: f64 = 18.0;
const ICON_X: f64 = 20.0;
const TRACK_X: f64 = 58.0;
const TRACK_WIDTH: f64 = 166.0;
const TRACK_HEIGHT: f64 = 6.0;
const READOUT_X: f64 = 230.0;
const READOUT_WIDTH: f64 = 38.0;

pub struct HudIvars {
    level: Cell<f64>,
    muted: Cell<bool>,
}

fn rounded(rect: NSRect, radius: f64) -> Retained<NSBezierPath> {
    NSBezierPath::bezierPathWithRoundedRect_xRadius_yRadius(rect, radius, radius)
}

/// Number of sound waves next to the speaker for a level.
fn waves(level: f64, muted: bool) -> u8 {
    if muted || level <= 0.0 {
        0
    } else if level < 1.0 / 3.0 {
        1
    } else if level < 2.0 / 3.0 {
        2
    } else {
        3
    }
}

/// A small speaker: a cone, then up to three waves, or a cross when muted.
fn draw_speaker(left: f64, middle: f64, level: f64, muted: bool) {
    NSColor::whiteColor().set();
    let body = NSBezierPath::bezierPath();
    body.moveToPoint(NSPoint::new(left, middle - 3.0));
    body.lineToPoint(NSPoint::new(left + 4.0, middle - 3.0));
    body.lineToPoint(NSPoint::new(left + 9.0, middle - 7.0));
    body.lineToPoint(NSPoint::new(left + 9.0, middle + 7.0));
    body.lineToPoint(NSPoint::new(left + 4.0, middle + 3.0));
    body.lineToPoint(NSPoint::new(left, middle + 3.0));
    body.closePath();
    body.fill();

    let line = |path: &NSBezierPath| {
        path.setLineWidth(1.7);
        path.setLineCapStyle(NSLineCapStyle::Round);
        path.stroke();
    };
    for wave in 1..=waves(level, muted) {
        let path = NSBezierPath::bezierPath();
        path.appendBezierPathWithArcWithCenter_radius_startAngle_endAngle(
            NSPoint::new(left + 9.0, middle),
            f64::from(wave) * 4.0 + 1.0,
            -42.0,
            42.0,
        );
        line(&path);
    }
    if muted {
        for (from, to) in [(-4.0, 4.0), (4.0, -4.0)] {
            let path = NSBezierPath::bezierPath();
            path.moveToPoint(NSPoint::new(left + 13.0, middle + from));
            path.lineToPoint(NSPoint::new(left + 21.0, middle + to));
            line(&path);
        }
    }
}

define_class!(
    // SAFETY: NSView allows subclassing; the ivars are only touched on the main thread and need no Drop.
    #[unsafe(super(NSView))]
    #[thread_kind = MainThreadOnly]
    #[name = "FigHudView"]
    #[ivars = HudIvars]
    struct HudView;

    impl HudView {
        #[unsafe(method(drawRect:))]
        fn draw_rect(&self, _dirty: NSRect) {
            let ivars = self.ivars();
            let (level, muted) = (ivars.level.get().clamp(0.0, 1.0), ivars.muted.get());
            let bounds = self.bounds();

            NSColor::blackColor().setFill();
            // Overshoot the top edge so only the bottom corners are rounded, like the hardware notch.
            let shape = NSRect::new(
                bounds.origin,
                NSSize::new(bounds.size.width, bounds.size.height + CORNER_RADIUS),
            );
            rounded(shape, CORNER_RADIUS).fill();

            let middle = ROW_HEIGHT / 2.0;
            draw_speaker(ICON_X, middle, level, muted);

            let track = NSRect::new(
                NSPoint::new(TRACK_X, middle - TRACK_HEIGHT / 2.0),
                NSSize::new(TRACK_WIDTH, TRACK_HEIGHT),
            );
            NSColor::colorWithWhite_alpha(1.0, 0.25).setFill();
            rounded(track, TRACK_HEIGHT / 2.0).fill();
            let shown = if muted { 0.0 } else { level };
            if shown > 0.0 {
                let fill = NSRect::new(
                    track.origin,
                    NSSize::new((TRACK_WIDTH * shown).max(TRACK_HEIGHT), TRACK_HEIGHT),
                );
                NSColor::whiteColor().setFill();
                rounded(fill, TRACK_HEIGHT / 2.0).fill();
            }

            if !muted {
                // SAFETY: extern static weight constant.
                let font = NSFont::monospacedDigitSystemFontOfSize_weight(13.0, unsafe {
                    NSFontWeightMedium
                });
                let attrs = text_attributes(&font, &NSColor::whiteColor(), Fit::Clip, true);
                let text = format!("{:.0}", level * 100.0);
                draw_text(
                    &text,
                    NSRect::new(
                        NSPoint::new(READOUT_X, middle - 8.0),
                        NSSize::new(READOUT_WIDTH, 17.0),
                    ),
                    &attrs,
                );
            }
        }
    }

    unsafe impl NSObjectProtocol for HudView {}
);

impl HudView {
    fn new(mtm: MainThreadMarker, frame: NSRect) -> Retained<Self> {
        let ivars = HudIvars {
            level: Cell::new(0.0),
            muted: Cell::new(false),
        };
        // SAFETY: designated initializer with valid arguments on the main thread.
        unsafe { msg_send![super(mtm.alloc::<Self>().set_ivars(ivars)), initWithFrame: frame] }
    }
}

thread_local! {
    static CURRENT: RefCell<Weak<Hud>> = const { RefCell::new(Weak::new()) };
}

pub struct Hud {
    mtm: MainThreadMarker,
    panel: Retained<FigPanel>,
    view: Retained<HudView>,
    /// Bumped by every show, so an older hide timer never closes a newer indicator.
    generation: Cell<u64>,
}

impl Hud {
    /// The most recently created `Hud` is the one `show_from_any_thread` reaches.
    pub fn new(mtm: MainThreadMarker) -> Rc<Self> {
        let frame = NSRect::new(NSPoint::ZERO, NSSize::new(WIDTH, ROW_HEIGHT));
        let panel = FigPanel::new(mtm, frame);
        // The indicator is look-only: clicks go to whatever is underneath.
        panel.setIgnoresMouseEvents(true);
        let view = HudView::new(mtm, frame);
        panel.setContentView(Some(&view));
        let hud = Rc::new(Self {
            mtm,
            panel,
            view,
            generation: Cell::new(0),
        });
        CURRENT.with(|current| *current.borrow_mut() = Rc::downgrade(&hud));
        hud
    }

    pub fn show(self: &Rc<Self>, level: f32, muted: bool) {
        let Some(geometry) = NotchGeometry::detect(self.mtm) else {
            return;
        };
        let ivars = self.view.ivars();
        ivars.level.set(f64::from(level));
        ivars.muted.set(muted);
        self.view.setNeedsDisplay(true);
        self.panel.setFrame_display(
            geometry.frame(WIDTH, geometry.top_inset() + ROW_HEIGHT),
            true,
        );
        self.panel.orderFrontRegardless();

        let generation = self.generation.get() + 1;
        self.generation.set(generation);
        let weak = Rc::downgrade(self);
        app::after(HOLD_SECONDS, move || {
            if let Some(hud) = weak.upgrade()
                && hud.generation.get() == generation
            {
                hud.panel.orderOut(None);
            }
        });
    }
}

/// Shows the indicator for a new volume from any thread, such as the event tap's.
pub fn show_from_any_thread(level: f32, muted: bool) {
    DispatchQueue::main().exec_async(move || {
        let hud = CURRENT.with(|current| current.borrow().upgrade());
        if let Some(hud) = hud {
            hud.show(level, muted);
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wave_count_follows_the_level_and_vanishes_when_muted() {
        assert_eq!(waves(0.0, false), 0);
        assert_eq!(waves(0.1, false), 1);
        assert_eq!(waves(0.5, false), 2);
        assert_eq!(waves(0.9, false), 3);
        assert_eq!(waves(1.0, false), 3);
        assert_eq!(waves(0.9, true), 0);
    }
}
