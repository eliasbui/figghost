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
/// The floating pill centred under the menu bar: the capsule's layout, a little wider.
const ISLAND_SIZE: NSSize = NSSize::new(200.0, 36.0);
/// The vertical slider on the right edge: the level fills a column from the speaker at its foot.
const SIDE_SIZE: NSSize = NSSize::new(48.0, 212.0);
const SIDE_MARGIN: f64 = 16.0;
const SIDE_TRACK_Y: f64 = 54.0;
const SIDE_TRACK_HEIGHT: f64 = 116.0;
/// The big tile in the lower middle of the screen, with the level as a row of segments.
const TILE_SIZE: NSSize = NSSize::new(184.0, 184.0);
const TILE_SEGMENTS: usize = 16;
/// The round dial: a ring that fills clockwise around the speaker.
const RING_SIZE: NSSize = NSSize::new(132.0, 132.0);
const RING_WIDTH: f64 = 7.0;
/// The equalizer: a staircase of bars that light up from the left.
const EQ_SIZE: NSSize = NSSize::new(216.0, 72.0);
const EQ_BARS: usize = 14;
/// The wide slab: the capsule's layout stretched over a long track.
const WIDE_SIZE: NSSize = NSSize::new(420.0, 34.0);
/// Room between a floating indicator at the bottom of the screen and the edge, which clears the Dock.
const BOTTOM_FLOOR: f64 = 96.0;

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
    /// The capsule's layout floating at the middle of the top, like an island.
    Island,
    /// A tall slim slider on the right edge of the screen.
    Side,
    /// A large square tile low on the screen: a big speaker and the level as a row of segments.
    Tile,
    /// A round dial in Liquid Glass: the level is a ring that fills clockwise around the speaker.
    Ring,
    /// A glass slab of bars that light up from the left like a level meter.
    Equalizer,
    /// A long thin glass capsule with a long track.
    Wide,
}

/// Where a floating indicator goes on the screen: one of nine places, or the place its style has by itself.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum HudPosition {
    #[default]
    Auto,
    TopLeft,
    Top,
    TopRight,
    Left,
    Center,
    Right,
    BottomLeft,
    Bottom,
    BottomRight,
}

impl HudPosition {
    /// The place a settings word names.
    pub fn named(word: &str) -> Self {
        match word {
            "top-left" => Self::TopLeft,
            "top" => Self::Top,
            "top-right" => Self::TopRight,
            "left" => Self::Left,
            "center" => Self::Center,
            "right" => Self::Right,
            "bottom-left" => Self::BottomLeft,
            "bottom" => Self::Bottom,
            "bottom-right" => Self::BottomRight,
            _ => Self::Auto,
        }
    }

    /// Column and row, each -1, 0 or 1 from the left and from the top; `Auto` is the style's own place.
    fn cell(self, style: HudStyle) -> (i8, i8) {
        match self {
            Self::Auto => match style {
                HudStyle::Corner => (1, -1),
                HudStyle::Island => (0, -1),
                HudStyle::Side => (1, 0),
                HudStyle::Ring => (0, 0),
                _ => (0, 1),
            },
            Self::TopLeft => (-1, -1),
            Self::Top => (0, -1),
            Self::TopRight => (1, -1),
            Self::Left => (-1, 0),
            Self::Center => (0, 0),
            Self::Right => (1, 0),
            Self::BottomLeft => (-1, 1),
            Self::Bottom => (0, 1),
            Self::BottomRight => (1, 1),
        }
    }
}

impl HudStyle {
    /// The style a settings word names.
    pub fn named(word: &str) -> Self {
        match word {
            "glass" => Self::Glass,
            "corner" => Self::Corner,
            "bar" => Self::Bar,
            "island" => Self::Island,
            "side" => Self::Side,
            "tile" => Self::Tile,
            "ring" => Self::Ring,
            "equalizer" => Self::Equalizer,
            "wide" => Self::Wide,
            _ => Self::Notch,
        }
    }

    /// What the glass behind the content is, for the styles that are glass.
    fn glass(self) -> Option<(NSGlassEffectViewStyle, Retained<NSColor>)> {
        let size = match self {
            Self::Glass => (NSGlassEffectViewStyle::Regular, 0.5),
            Self::Corner | Self::Island => (NSGlassEffectViewStyle::Clear, 0.34),
            Self::Side | Self::Tile | Self::Ring | Self::Equalizer => {
                (NSGlassEffectViewStyle::Regular, 0.42)
            }
            Self::Wide => (NSGlassEffectViewStyle::Clear, 0.34),
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
        !self.floats()
    }

    /// Whether the style floats free of the top edge, and so can be put anywhere on the screen.
    pub fn floats(self) -> bool {
        !matches!(self, Self::Notch | Self::Glass | Self::Bar)
    }

    /// Whether the content is the capsule's layout (speaker, a track and the number in a row).
    fn compact(self) -> bool {
        matches!(self, Self::Corner | Self::Island | Self::Wide)
    }

    /// Where the speaker, the track and the number sit in a capsule layout: x of each and the widths of the
    /// track and of the number.
    fn row_layout(self) -> (f64, f64, f64, f64, f64) {
        match self {
            Self::Wide => (16.0, 48.0, 306.0, 364.0, 40.0),
            Self::Corner | Self::Island => (
                CORNER_ICON_X,
                CORNER_TRACK_X,
                CORNER_TRACK_WIDTH,
                CORNER_READOUT_X,
                CORNER_READOUT_WIDTH,
            ),
            _ => (ICON_X, TRACK_X, TRACK_WIDTH, READOUT_X, READOUT_WIDTH),
        }
    }

    /// The size of the panel.
    fn size(self, top_inset: f64) -> NSSize {
        match self {
            Self::Corner => CORNER_SIZE,
            Self::Island => ISLAND_SIZE,
            Self::Side => SIDE_SIZE,
            Self::Tile => TILE_SIZE,
            Self::Ring => RING_SIZE,
            Self::Equalizer => EQ_SIZE,
            Self::Wide => WIDE_SIZE,
            _ => NSSize::new(WIDTH, top_inset + ROW_HEIGHT),
        }
    }

    /// The corner radius of the glass, which a capsule knows only once it has a height.
    fn radius(self, height: f64) -> f64 {
        match self {
            Self::Corner | Self::Island | Self::Wide => height / 2.0,
            Self::Side => SIDE_SIZE.width / 2.0,
            Self::Tile => 36.0,
            Self::Ring => RING_SIZE.width / 2.0,
            Self::Equalizer => 26.0,
            _ => CORNER_RADIUS,
        }
    }
}

pub struct HudIvars {
    style: Cell<HudStyle>,
    level: Cell<f64>,
    muted: Cell<bool>,
    /// The brightness of a display is shown, with a sun in place of the speaker.
    sun: Cell<bool>,
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

/// The speaker for the volume, or the sun for the brightness of a display.
fn draw_glyph(sun: bool, left: f64, middle: f64, level: f64, muted: bool) {
    if sun {
        draw_sun(left, middle, level);
    } else {
        draw_speaker(left, middle, level, muted);
    }
}

/// A small sun in the speaker's room: a disc, and rays that grow longer as the level rises.
fn draw_sun(left: f64, middle: f64, level: f64) {
    NSColor::whiteColor().set();
    let centre = NSPoint::new(left + 10.0, middle);
    NSBezierPath::bezierPathWithOvalInRect(NSRect::new(
        NSPoint::new(centre.x - 4.0, centre.y - 4.0),
        NSSize::new(8.0, 8.0),
    ))
    .fill();
    let reach = 7.5 + 2.0 * level;
    for ray in 0..8 {
        let angle = f64::from(ray) * std::f64::consts::FRAC_PI_4;
        let (sin, cos) = angle.sin_cos();
        let path = NSBezierPath::bezierPath();
        path.moveToPoint(NSPoint::new(centre.x + 6.0 * cos, centre.y + 6.0 * sin));
        path.lineToPoint(NSPoint::new(centre.x + reach * cos, centre.y + reach * sin));
        path.setLineWidth(1.7);
        path.setLineCapStyle(NSLineCapStyle::Round);
        path.stroke();
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

/// The slider on the right edge: the number on top, the column that the level fills from below, the speaker at
/// the foot.
fn draw_side(size: NSSize, level: f64, muted: bool, sun: bool) {
    let x = (size.width - TRACK_HEIGHT) / 2.0;
    let track = NSRect::new(
        NSPoint::new(x, SIDE_TRACK_Y),
        NSSize::new(TRACK_HEIGHT, SIDE_TRACK_HEIGHT),
    );
    NSColor::colorWithWhite_alpha(1.0, 0.25).setFill();
    rounded(track, TRACK_HEIGHT / 2.0).fill();
    let shown = if muted { 0.0 } else { level };
    if shown > 0.0 {
        let fill = NSRect::new(
            track.origin,
            NSSize::new(TRACK_HEIGHT, (SIDE_TRACK_HEIGHT * shown).max(TRACK_HEIGHT)),
        );
        NSColor::whiteColor().setFill();
        rounded(fill, TRACK_HEIGHT / 2.0).fill();
    }
    draw_glyph(sun, size.width / 2.0 - 11.0, 28.0, level, muted);
    if !muted {
        // SAFETY: extern static weight constant.
        let font =
            NSFont::monospacedDigitSystemFontOfSize_weight(12.0, unsafe { NSFontWeightMedium });
        let attrs = text_attributes(&font, &NSColor::whiteColor(), Fit::Clip, true);
        draw_text(
            &format!("{:.0}", level * 100.0),
            NSRect::new(
                NSPoint::new(0.0, SIDE_TRACK_Y + SIDE_TRACK_HEIGHT + 14.0),
                NSSize::new(size.width, 17.0),
            ),
            &attrs,
        );
    }
}

/// The tile: the speaker drawn large, and under it the level as a row of segments.
fn draw_tile(size: NSSize, level: f64, muted: bool, sun: bool) {
    NSGraphicsContext::saveGraphicsState_class();
    let place = objc2_foundation::NSAffineTransform::transform();
    place.translateXBy_yBy(size.width / 2.0, size.height * 0.58);
    place.scaleBy(3.4);
    {
        use objc2_app_kit::NSAffineTransformNSAppKitAdditions as _;
        place.concat();
    }
    draw_glyph(sun, -11.0, 0.0, level, muted);
    NSGraphicsContext::restoreGraphicsState_class();

    let gap = 2.0;
    let width = (size.width - 48.0 - gap * (TILE_SEGMENTS as f64 - 1.0)) / TILE_SEGMENTS as f64;
    let lit = if muted {
        0
    } else {
        (level * TILE_SEGMENTS as f64).round() as usize
    };
    for i in 0..TILE_SEGMENTS {
        let segment = NSRect::new(
            NSPoint::new(24.0 + i as f64 * (width + gap), 26.0),
            NSSize::new(width, 7.0),
        );
        NSColor::colorWithWhite_alpha(1.0, if i < lit { 1.0 } else { 0.25 }).setFill();
        rounded(segment, 1.5).fill();
    }
}

/// The speaker drawn `scale` times as large, centred on `at`.
fn draw_speaker_scaled(at: NSPoint, scale: f64, level: f64, muted: bool, sun: bool) {
    NSGraphicsContext::saveGraphicsState_class();
    let place = objc2_foundation::NSAffineTransform::transform();
    place.translateXBy_yBy(at.x, at.y);
    place.scaleBy(scale);
    {
        use objc2_app_kit::NSAffineTransformNSAppKitAdditions as _;
        place.concat();
    }
    draw_glyph(sun, -11.0, 0.0, level, muted);
    NSGraphicsContext::restoreGraphicsState_class();
}

fn draw_number(level: f64, size: f64, area: NSRect) {
    // SAFETY: extern static weight constant.
    let font = NSFont::monospacedDigitSystemFontOfSize_weight(size, unsafe { NSFontWeightMedium });
    let attrs = text_attributes(&font, &NSColor::whiteColor(), Fit::Clip, true);
    draw_text(&format!("{:.0}", level * 100.0), area, &attrs);
}

/// The dial: the ring is the level, filling clockwise from the top; the speaker and the number sit inside.
fn draw_ring(size: NSSize, level: f64, muted: bool, sun: bool) {
    let centre = NSPoint::new(size.width / 2.0, size.height / 2.0);
    let radius = size.width / 2.0 - 14.0;
    let ring = |from: f64, to: f64, white: f64| {
        let path = NSBezierPath::bezierPath();
        path.appendBezierPathWithArcWithCenter_radius_startAngle_endAngle_clockwise(
            centre, radius, from, to, true,
        );
        path.setLineWidth(RING_WIDTH);
        path.setLineCapStyle(NSLineCapStyle::Round);
        NSColor::colorWithWhite_alpha(1.0, white).setStroke();
        path.stroke();
    };
    ring(90.0, -270.0, 0.25);
    let shown = if muted { 0.0 } else { level.min(0.999) };
    if shown > 0.0 {
        ring(90.0, 90.0 - 360.0 * shown, 1.0);
    }
    draw_speaker_scaled(
        NSPoint::new(centre.x, centre.y + 6.0),
        1.5,
        level,
        muted,
        sun,
    );
    if !muted {
        draw_number(
            level,
            12.0,
            NSRect::new(
                NSPoint::new(0.0, centre.y - 30.0),
                NSSize::new(size.width, 17.0),
            ),
        );
    }
}

/// The meter: a staircase of bars, the speaker at its foot, the bars lit up to the level.
fn draw_equalizer(size: NSSize, level: f64, muted: bool, sun: bool) {
    draw_glyph(sun, 16.0, size.height / 2.0, level, muted);
    let left = 52.0;
    let gap = 3.0;
    let width = (size.width - left - 18.0 - gap * (EQ_BARS as f64 - 1.0)) / EQ_BARS as f64;
    let lit = if muted {
        0
    } else {
        (level * EQ_BARS as f64).round() as usize
    };
    for i in 0..EQ_BARS {
        let rise = i as f64 / (EQ_BARS as f64 - 1.0);
        let height = 10.0 + rise * (size.height - 34.0);
        let bar = NSRect::new(
            NSPoint::new(left + i as f64 * (width + gap), 16.0),
            NSSize::new(width, height),
        );
        NSColor::colorWithWhite_alpha(1.0, if i < lit { 1.0 } else { 0.25 }).setFill();
        rounded(bar, width / 2.0).fill();
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
            let sun = ivars.sun.get();
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

            if style == HudStyle::Side {
                draw_side(bounds.size, level, muted, sun);
                return;
            }
            if style == HudStyle::Tile {
                draw_tile(bounds.size, level, muted, sun);
                return;
            }
            if style == HudStyle::Ring {
                draw_ring(bounds.size, level, muted, sun);
                return;
            }
            if style == HudStyle::Equalizer {
                draw_equalizer(bounds.size, level, muted, sun);
                return;
            }
            let middle = bounds.size.height / 2.0;
            let compact = style.compact();
            let (icon_x, track_x, track_width, readout_x, readout_width) = style.row_layout();
            draw_glyph(sun, icon_x, middle, level, muted);

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
            sun: Cell::new(false),
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
    position: Cell<HudPosition>,
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
            position: Cell::new(HudPosition::Auto),
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

    /// Where a floating style goes; the styles that hang from the notch stay where they are.
    pub fn set_position(&self, position: HudPosition) {
        self.position.set(position);
    }

    fn dress(&self, style: HudStyle) {
        let size = style.size(0.0);
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
                NSRect::new(
                    NSPoint::ZERO,
                    NSSize::new(size.width, size.height + overshoot),
                ),
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
            NSSize::new(
                size.width,
                if style.hangs() {
                    ROW_HEIGHT
                } else {
                    size.height
                },
            ),
        ));
        *self.bed.borrow_mut() = glass;
        self.view.setNeedsDisplay(true);
    }

    pub fn show(self: &Rc<Self>, level: f32, muted: bool) {
        self.show_glyph(level, muted, false);
    }

    /// Shows the brightness of a display, with a sun in place of the speaker.
    pub fn show_brightness(self: &Rc<Self>, level: f32) {
        self.show_glyph(level, false, true);
    }

    fn show_glyph(self: &Rc<Self>, level: f32, muted: bool, sun: bool) {
        let Some(geometry) = NotchGeometry::detect(self.mtm) else {
            return;
        };
        let ivars = self.view.ivars();
        ivars.level.set(f64::from(level));
        ivars.muted.set(muted);
        ivars.sun.set(sun);
        self.view.setNeedsDisplay(true);
        let style = self.style.get();
        let size = style.size(geometry.top_inset());
        self.panel
            .setFrame_display(frame_for(&geometry, style, self.position.get(), size), true);
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

/// Where the panel goes: the styles that hang sit under the cutout at the top edge of the screen, and the ones
/// that float go to the place the user chose, or to the one their style has by itself.
fn frame_for(
    geometry: &NotchGeometry,
    style: HudStyle,
    position: HudPosition,
    size: NSSize,
) -> NSRect {
    if style.hangs() {
        return geometry.frame(size.width, size.height);
    }
    let screen = geometry.screen;
    let margin = if style == HudStyle::Side {
        SIDE_MARGIN
    } else {
        CORNER_MARGIN
    };
    let top = geometry.top_inset().max(24.0) + CORNER_MARGIN;
    let (column, row) = position.cell(style);
    let x = match column {
        -1 => screen.origin.x + margin,
        0 => screen.origin.x + (screen.size.width - size.width) / 2.0,
        _ => screen.origin.x + screen.size.width - size.width - margin,
    };
    let y = match row {
        -1 => screen.origin.y + screen.size.height - top - size.height,
        0 => screen.origin.y + (screen.size.height - size.height) / 2.0,
        _ => screen.origin.y + BOTTOM_FLOOR,
    };
    NSRect::new(NSPoint::new(x, y), size)
}

/// Shows the indicator for a new brightness from any thread, such as a worker's.
pub fn show_brightness_from_any_thread(level: f32) {
    DispatchQueue::main().exec_async(move || {
        let hud = CURRENT.with(|current| current.borrow().upgrade());
        if let Some(hud) = hud {
            hud.show_brightness(level);
        }
    });
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
    pub fn snapshot_png(
        &self,
        style: HudStyle,
        level: f32,
        muted: bool,
        sun: bool,
    ) -> Option<Vec<u8>> {
        self.set_style(style);
        // The panel sizes its content, so it takes the size of the style as `show` would give it.
        self.panel
            .setFrame_display(NSRect::new(NSPoint::ZERO, style.size(0.0)), false);
        self.view.ivars().level.set(f64::from(level));
        self.view.ivars().muted.set(muted);
        self.view.ivars().sun.set(sun);
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
            let frame = frame_for(&geometry, style, HudPosition::Auto, size);
            // Centred on the screen and joined to its top edge.
            assert_eq!(frame.origin.x + size.width / 2.0, 720.0, "{style:?}");
            assert_eq!(frame.origin.y + size.height, 900.0, "{style:?}");
            // Its content row starts below the cutout, so the speaker and the bar are not behind it.
            assert!(frame.origin.y >= geometry.notch.height, "{style:?}");
        }
        let size = HudStyle::Corner.size(geometry.top_inset());
        let frame = frame_for(&geometry, HudStyle::Corner, HudPosition::Auto, size);
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
        // The new ones float: the island is centred under the menu bar, the side slider is on the right edge at
        // mid height, and the tile is centred above the bottom.
        let island = frame_for(
            &geometry,
            HudStyle::Island,
            HudPosition::Auto,
            HudStyle::Island.size(0.0),
        );
        assert_eq!(island.origin.x + island.size.width / 2.0, 720.0);
        assert!(island.origin.y + island.size.height <= 900.0 - 24.0);
        let side = frame_for(
            &geometry,
            HudStyle::Side,
            HudPosition::Auto,
            HudStyle::Side.size(0.0),
        );
        assert_eq!(side.origin.x + side.size.width, 1440.0 - SIDE_MARGIN);
        assert_eq!(side.origin.y + side.size.height / 2.0, 450.0);
        let tile = frame_for(
            &geometry,
            HudStyle::Tile,
            HudPosition::Auto,
            HudStyle::Tile.size(0.0),
        );
        assert_eq!(tile.origin.x + tile.size.width / 2.0, 720.0);
        assert!(!HudStyle::Island.hangs() && !HudStyle::Side.hangs() && !HudStyle::Tile.hangs());
        for word in ["island", "side", "tile", "ring", "equalizer", "wide"] {
            assert!(HudStyle::named(word).glass().is_some(), "{word}");
            assert!(HudStyle::named(word).floats(), "{word}");
        }
    }

    #[test]
    fn a_chosen_place_moves_a_floating_style_and_leaves_a_hanging_one_alone() {
        let geometry = NotchGeometry {
            screen: NSRect::new(NSPoint::new(0.0, 0.0), NSSize::new(1440.0, 900.0)),
            notch: NSSize::new(200.0, 32.0),
            hardware: true,
        };
        let ring = HudStyle::Ring.size(0.0);
        let at = |word: &str| frame_for(&geometry, HudStyle::Ring, HudPosition::named(word), ring);
        // By itself the dial is in the middle of the screen.
        assert_eq!(
            frame_for(&geometry, HudStyle::Ring, HudPosition::Auto, ring),
            at("center")
        );
        let left = at("bottom-left");
        assert_eq!(
            (left.origin.x, left.origin.y),
            (CORNER_MARGIN, BOTTOM_FLOOR)
        );
        let right = at("top-right");
        assert_eq!(right.origin.x + ring.width, 1440.0 - CORNER_MARGIN);
        assert!(right.origin.y + ring.height <= 900.0 - 24.0);
        assert_eq!(at("left").origin.y + ring.height / 2.0, 450.0);
        // The hang from the notch does not move.
        let notch = HudStyle::Notch.size(geometry.top_inset());
        assert_eq!(
            frame_for(&geometry, HudStyle::Notch, HudPosition::BottomLeft, notch),
            frame_for(&geometry, HudStyle::Notch, HudPosition::Auto, notch)
        );
        assert_eq!(HudPosition::named("nowhere"), HudPosition::Auto);
    }
}
