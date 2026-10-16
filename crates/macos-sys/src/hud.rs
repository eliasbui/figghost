//! A volume indicator that hangs from the top edge of the screen like the notch, shown while volume keys
//! are being handled.

use crate::{
    app,
    notch::NotchGeometry,
    panel::FigPanel,
    style,
    widgets::{Fit, draw_text, text_attributes},
};
use dispatch2::DispatchQueue;
use objc2::{
    AnyThread, DefinedClass, MainThreadMarker, MainThreadOnly, define_class, msg_send, rc::Retained,
};
use objc2_app_kit::{
    NSBezierPath, NSBitmapImageFileType, NSColor, NSFont, NSFontWeightMedium, NSGlassEffectView,
    NSGlassEffectViewStyle, NSGradient, NSGraphicsContext, NSLineCapStyle, NSView,
};
use objc2_foundation::{NSDictionary, NSObjectProtocol, NSPoint, NSRect, NSSize};
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
/// The capsule that floats in the corner: smaller, and a whole capsule rather than a hang from the edge.
const CORNER_SIZE: NSSize = NSSize::new(188.0, 34.0);
const CORNER_ICON_X: f64 = 15.0;
const CORNER_TRACK_X: f64 = 48.0;
const CORNER_TRACK_WIDTH: f64 = 86.0;
const CORNER_READOUT_X: f64 = 138.0;
const CORNER_READOUT_WIDTH: f64 = 36.0;
/// Room between the floating capsule and the corner of the screen.
const CORNER_MARGIN: f64 = 12.0;

/// How the volume indicator looks.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HudStyle {
    /// Black, hanging from the cutout: the shape the hardware notch would have if it grew to show a level.
    Notch,
    /// The same hang, in Liquid Glass: it reads as part of the notch's own panel on the desktop.
    Glass,
    /// A small capsule floating under the menu bar in the corner, as the system's own indicator does.
    Corner,
    /// The same hang, with the level itself filling the shape from the left: it reads as a meter.
    Bar,
}

impl HudStyle {
    /// The style a settings word names.
    pub fn named(word: &str) -> Self {
        match word {
            "glass" => Self::Glass,
            "corner" => Self::Corner,
            "bar" => Self::Bar,
            _ => Self::Notch,
        }
    }

    /// What the glass behind the content is, for the styles that are glass.
    fn glass(self) -> Option<(NSGlassEffectViewStyle, Retained<NSColor>)> {
        let size = match self {
            Self::Glass => (NSGlassEffectViewStyle::Regular, 0.5),
            Self::Corner => (NSGlassEffectViewStyle::Clear, 0.34),
            _ => return None,
        };
        Some((size.0, style::black(size.1)))
    }

    /// Whether the view paints the shape itself; the glass styles leave it to the glass behind them.
    fn paints_shape(self) -> bool {
        matches!(self, Self::Notch | Self::Bar)
    }

    /// Whether the shape hangs from the top edge, which hides its top corners above the screen.
    fn hangs(self) -> bool {
        !matches!(self, Self::Corner)
    }

    /// The size of the panel.
    fn size(self, top_inset: f64) -> NSSize {
        match self {
            Self::Corner => CORNER_SIZE,
            _ => NSSize::new(WIDTH, top_inset + ROW_HEIGHT),
        }
    }

    /// The corner radius of the glass, which a capsule knows only once it has a height.
    fn radius(self, height: f64) -> f64 {
        match self {
            Self::Corner => height / 2.0,
            _ => CORNER_RADIUS,
        }
    }
}

pub struct HudIvars {
    style: Cell<HudStyle>,
    level: Cell<f64>,
    muted: Cell<bool>,
    /// Stands in for the glass while a snapshot is drawn, since the window server, which draws the glass, is
    /// not part of a bitmap offscreen.
    backdrop: Cell<bool>,
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
            let style = ivars.style.get();
            let (level, muted) = (ivars.level.get().clamp(0.0, 1.0), ivars.muted.get());
            let bounds = self.bounds();
            let corner = style.radius(bounds.size.height);

            if style.paints_shape() || ivars.backdrop.get() {
                if ivars.backdrop.get() {
                    // The bitmap starts blank: black under everything keeps the rounded edges clean in a
                    // snapshot, the way they are on the desktop.
                    style::black(1.0).setFill();
                    NSBezierPath::fillRect(bounds);
                }
                // Black, a shade lighter at the bottom, like the notch itself; the styles that hang overshoot
                // the top edge by their radius, so only the bottom corners are round.
                let shape = NSRect::new(
                    bounds.origin,
                    NSSize::new(
                        bounds.size.width,
                        bounds.size.height + if style.hangs() { corner } else { 0.0 },
                    ),
                );
                paint_slate(shape, corner);
                if style == HudStyle::Bar && !muted && level > 0.0 {
                    // The level is the background of the shape: a meter that fills as the sound grows.
                    let meter = NSRect::new(
                        shape.origin,
                        NSSize::new(shape.size.width * level, shape.size.height),
                    );
                    NSGraphicsContext::saveGraphicsState_class();
                    rounded(shape, corner).addClip();
                    paint_level(meter, corner);
                    NSGraphicsContext::restoreGraphicsState_class();
                }
            }

            let middle = bounds.size.height / 2.0;
            let compact = style == HudStyle::Corner;
            let (icon_x, track_x, track_width, readout_x, readout_width) = if compact {
                (
                    CORNER_ICON_X,
                    CORNER_TRACK_X,
                    CORNER_TRACK_WIDTH,
                    CORNER_READOUT_X,
                    CORNER_READOUT_WIDTH,
                )
            } else {
                (ICON_X, TRACK_X, TRACK_WIDTH, READOUT_X, READOUT_WIDTH)
            };
            draw_speaker(icon_x, middle, level, muted);

            // The meter style has already drawn the level; the others get the thin track.
            if style != HudStyle::Bar {
                let track = NSRect::new(
                    NSPoint::new(track_x, middle - TRACK_HEIGHT / 2.0),
                    NSSize::new(track_width, TRACK_HEIGHT),
                );
                NSColor::colorWithWhite_alpha(1.0, 0.25).setFill();
                rounded(track, TRACK_HEIGHT / 2.0).fill();
                let shown = if muted { 0.0 } else { level };
                if shown > 0.0 {
                    let fill = NSRect::new(
                        track.origin,
                        NSSize::new((track_width * shown).max(TRACK_HEIGHT), TRACK_HEIGHT),
                    );
                    NSColor::whiteColor().setFill();
                    rounded(fill, TRACK_HEIGHT / 2.0).fill();
                }
            }

            if !muted {
                // SAFETY: extern static weight constant.
                let size = if compact { 12.0 } else { 13.0 };
                let font = NSFont::monospacedDigitSystemFontOfSize_weight(size, unsafe {
                    NSFontWeightMedium
                });
                let attrs = text_attributes(&font, &NSColor::whiteColor(), Fit::Clip, true);
                let text = format!("{:.0}", level * 100.0);
                draw_text(
                    &text,
                    NSRect::new(
                        NSPoint::new(readout_x, middle - 8.0),
                        NSSize::new(readout_width, 17.0),
                    ),
                    &attrs,
                );
            }
        }
    }

    unsafe impl NSObjectProtocol for HudView {}
);

/// The shape of the indicator: black at the top, a dark slate where it meets the screen, as the notch's own
/// black material is drawn.
fn paint_slate(rect: NSRect, radius: f64) {
    let path = NSBezierPath::bezierPathWithRoundedRect_xRadius_yRadius(rect, radius, radius);
    // SAFETY: extern static constructors on the main thread, and a plain gradient.
    let gradient = NSGradient::initWithStartingColor_endingColor(
        NSGradient::alloc(),
        &NSColor::blackColor(),
        &NSColor::colorWithSRGBRed_green_blue_alpha(0.17, 0.18, 0.20, 1.0),
    );
    match gradient {
        Some(gradient) => gradient.drawInBezierPath_angle(&path, -90.0),
        None => {
            NSColor::blackColor().setFill();
            path.fill();
        }
    }
}

/// The level of the meter style: the accent colour, lit from the top, so it reads as a bar of light.
fn paint_level(rect: NSRect, radius: f64) {
    let path = NSBezierPath::bezierPathWithRoundedRect_xRadius_yRadius(rect, radius, radius);
    let accent = NSColor::controlAccentColor();
    let gradient = NSGradient::initWithStartingColor_endingColor(
        NSGradient::alloc(),
        &accent.colorWithAlphaComponent(0.95),
        &accent.colorWithAlphaComponent(0.62),
    );
    match gradient {
        Some(gradient) => gradient.drawInBezierPath_angle(&path, -90.0),
        None => {
            accent.setFill();
            path.fill();
        }
    }
}

impl HudView {
    fn new(mtm: MainThreadMarker, frame: NSRect, style: HudStyle) -> Retained<Self> {
        let ivars = HudIvars {
            style: Cell::new(style),
            level: Cell::new(0.0),
            muted: Cell::new(false),
            backdrop: Cell::new(false),
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
    /// The glass the content sits on, for the styles that are glass.
    bed: RefCell<Option<Retained<NSGlassEffectView>>>,
    style: Cell<HudStyle>,
    /// Bumped by every show, so an older hide timer never closes a newer indicator.
    generation: Cell<u64>,
}

impl Hud {
    /// The most recently created `Hud` is the one `show_from_any_thread` reaches.
    pub fn new(mtm: MainThreadMarker) -> Rc<Self> {
        Self::with_style(mtm, HudStyle::Notch)
    }

    /// The same, in the look the user chose in the settings.
    pub fn with_style(mtm: MainThreadMarker, style: HudStyle) -> Rc<Self> {
        let frame = NSRect::new(NSPoint::ZERO, NSSize::new(WIDTH, ROW_HEIGHT));
        let panel = FigPanel::new(mtm, frame);
        // The indicator is look-only: clicks go to whatever is underneath.
        panel.setIgnoresMouseEvents(true);
        let view = HudView::new(mtm, frame, style);
        let hud = Rc::new(Self {
            mtm,
            panel,
            view,
            bed: RefCell::default(),
            style: Cell::new(style),
            generation: Cell::new(0),
        });
        hud.dress(style);
        CURRENT.with(|current| *current.borrow_mut() = Rc::downgrade(&hud));
        hud
    }

    /// Changes the look. A glass style puts a Liquid Glass view behind the content; the others let the view
    /// paint its own black shape.
    pub fn set_style(&self, style: HudStyle) {
        if self.style.replace(style) == style {
            return;
        }
        self.view.ivars().style.set(style);
        self.dress(style);
    }

    fn dress(&self, style: HudStyle) {
        let size = self.view.frame().size;
        // The glass is as tall as the shape, which for the hanging styles overshoots the top edge by the corner
        // radius so its top corners stay off the screen, as the view's own shape does.
        let overshoot = if style.hangs() {
            style.radius(size.height)
        } else {
            0.0
        };
        let glass = style.glass().and_then(|(look, tint)| {
            // Before macOS 26 there is no Liquid Glass: the view falls back to its own black shape.
            objc2::runtime::AnyClass::get(c"NSGlassEffectView")?;
            let glass = NSGlassEffectView::initWithFrame(
                self.mtm.alloc(),
                NSRect::new(NSPoint::ZERO, NSSize::new(size.width, size.height + overshoot)),
            );
            glass.setStyle(look);
            glass.setTintColor(Some(&tint));
            glass.setCornerRadius(style.radius(size.height));
            Some(glass)
        });
        match &glass {
            Some(glass) => {
                glass.setContentView(Some(&self.view));
                self.panel.setContentView(Some(glass));
            }
            None => self.panel.setContentView(Some(&self.view)),
        }
        self.view.setFrame(NSRect::new(
            NSPoint::ZERO,
            NSSize::new(size.width, if style.hangs() { ROW_HEIGHT } else { size.height }),
        ));
        *self.bed.borrow_mut() = glass;
        self.view.setNeedsDisplay(true);
    }

    pub fn show(self: &Rc<Self>, level: f32, muted: bool) {
        let Some(geometry) = NotchGeometry::detect(self.mtm) else {
            return;
        };
        let ivars = self.view.ivars();
        ivars.level.set(f64::from(level));
        ivars.muted.set(muted);
        self.view.setNeedsDisplay(true);
        let style = self.style.get();
        let size = style.size(geometry.top_inset());
        self.panel
            .setFrame_display(frame_for(&geometry, style, size), true);
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

/// Where the panel goes: the styles that hang sit under the cutout at the top edge of the screen, and the
/// capsule floats in the corner, below the menu bar.
fn frame_for(geometry: &NotchGeometry, style: HudStyle, size: NSSize) -> NSRect {
    if style.hangs() {
        return geometry.frame(size.width, size.height);
    }
    let screen = geometry.screen;
    let top = geometry.top_inset().max(24.0) + CORNER_MARGIN;
    NSRect::new(
        NSPoint::new(
            screen.origin.x + screen.size.width - size.width - CORNER_MARGIN,
            screen.origin.y + screen.size.height - top - size.height,
        ),
        size,
    )
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

impl Hud {
    /// Draws the indicator of one style with one level into PNG bytes, without showing anything on screen.
    /// The glass is drawn by the window server, which is not part of a bitmap offscreen, so a glass style is
    /// drawn with the black shape standing in for it; the layout and the ink are what this checks.
    pub fn snapshot_png(&self, style: HudStyle, level: f32, muted: bool) -> Option<Vec<u8>> {
        self.set_style(style);
        self.view.ivars().level.set(f64::from(level));
        self.view.ivars().muted.set(muted);
        self.view.ivars().backdrop.set(true);
        self.view.setNeedsDisplay(true);
        let bounds = self.view.bounds();
        let rep = self.view.bitmapImageRepForCachingDisplayInRect(bounds)?;
        self.view.cacheDisplayInRect_toBitmapImageRep(bounds, &rep);
        // SAFETY: an empty properties dictionary is valid for PNG output.
        unsafe {
            rep.representationUsingType_properties(NSBitmapImageFileType::PNG, &NSDictionary::new())
        }
        .map(|data| data.to_vec())
    }
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

    #[test]
    fn a_hanging_style_reaches_below_the_cutout_and_a_capsule_keeps_off_the_menu_bar() {
        let geometry = NotchGeometry {
            screen: NSRect::new(NSPoint::new(0.0, 0.0), NSSize::new(1440.0, 900.0)),
            notch: NSSize::new(200.0, 32.0),
            hardware: true,
        };
        for style in [HudStyle::Notch, HudStyle::Glass, HudStyle::Bar] {
            let size = style.size(geometry.top_inset());
            let frame = frame_for(&geometry, style, size);
            // Centred on the screen and joined to its top edge.
            assert_eq!(frame.origin.x + size.width / 2.0, 720.0, "{style:?}");
            assert_eq!(frame.origin.y + size.height, 900.0, "{style:?}");
            // Its content row starts below the cutout, so the speaker and the bar are not behind it.
            assert!(frame.origin.y >= geometry.notch.height, "{style:?}");
        }
        let size = HudStyle::Corner.size(geometry.top_inset());
        let frame = frame_for(&geometry, HudStyle::Corner, size);
        // In from the right edge, below the menu bar: floating, joined to no edge.
        assert_eq!(frame.origin.x + size.width, 1440.0 - CORNER_MARGIN);
        assert!(frame.origin.y + size.height <= 900.0 - 24.0);
        assert_eq!(HudStyle::Corner.radius(size.height), size.height / 2.0);
        assert!(!HudStyle::Corner.hangs());
        // Only the glass styles have glass behind them, and only two styles paint a shape themselves.
        assert!(HudStyle::Glass.glass().is_some() && HudStyle::Corner.glass().is_some());
        assert!(HudStyle::Notch.glass().is_none() && HudStyle::Bar.glass().is_none());
        assert!(HudStyle::Notch.paints_shape() && HudStyle::Bar.paints_shape());
        assert!(!HudStyle::Glass.paints_shape() && !HudStyle::Corner.paints_shape());
    }
}
