//! The look shared by every page of the notch: one set of colours, type and shapes, symbols, and the
//! motion curves. The surfaces are dark, controls are translucent white on them, and the only
//! solid white is what the user is looking at or is about to press.

use block2::RcBlock;
use objc2::{AnyThread, Message, rc::Retained};
use objc2_app_kit::{
    NSBezierPath, NSColor, NSCompositingOperation, NSFont, NSFontWeight, NSFontWeightBold,
    NSFontWeightLight, NSFontWeightMedium, NSFontWeightRegular, NSFontWeightSemibold, NSGradient,
    NSGraphicsContext, NSImage, NSImageSymbolConfiguration, NSImageSymbolScale,
};
use objc2_foundation::{NSPoint, NSRect, NSSize, NSString};
use std::{cell::RefCell, collections::HashMap};

// One palette, one set of shapes, one set of times. Everything the app draws on a dark surface reads its
// numbers from here, so a change of look is a change to this block and to the two primitives below it.

/// Ink: the three levels of white text and symbols are drawn in.
pub const TEXT: f64 = 0.97;
pub const TEXT_SECONDARY: f64 = 0.62;
pub const TEXT_TERTIARY: f64 = 0.40;
/// Controls that rest on a surface: a chip, a small button, a tab that is not the chosen one.
pub const CONTROL: f64 = 0.11;
pub const CONTROL_HOVER: f64 = 0.19;
pub const CONTROL_PRESSED: f64 = 0.26;
/// The surfaces a page is made of. A card is lit from above: it is lighter where the light comes from, it keeps
/// a hairline edge all the way round, and its top edge carries the brighter specular line that reads as glass.
pub const CARD_TOP: f64 = 0.085;
pub const CARD_BOTTOM: f64 = 0.038;
pub const CARD_EDGE: f64 = 0.10;
pub const CARD_SPECULAR: f64 = 0.22;
/// A well is recessed into the surface instead of standing on it: it is darker than what holds it.
pub const WELL_TOP: f64 = 0.015;
pub const WELL_BOTTOM: f64 = 0.055;
pub const WELL_EDGE: f64 = 0.07;
/// The radius of the shapes that hold content, and of the buttons that sit in them.
pub const CARD_RADIUS: f64 = 18.0;
pub const BUTTON_RADIUS: f64 = 12.0;
/// The bottom corners of the open notch.
pub const PANEL_RADIUS: f64 = 28.0;

/// Motion: how long a surface takes to reach the user's finger, and how it gets there. Short enough that
/// nothing waits for a picture, long enough that the eye can follow it.
pub const FAST: f64 = 0.14;
pub const BASE: f64 = 0.24;
pub const SLOW: f64 = 0.40;
pub const GLIDE: f64 = 0.55;

pub fn white(alpha: f64) -> Retained<NSColor> {
    NSColor::colorWithWhite_alpha(1.0, alpha)
}

pub fn black(alpha: f64) -> Retained<NSColor> {
    NSColor::colorWithWhite_alpha(0.0, alpha)
}

/// The colour of a state: running, waiting, failed, done. These are the system colours, so a status reads the
/// same here as it does in the rest of macOS.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tone {
    Blue,
    Orange,
    Green,
    Red,
    Purple,
    Gray,
}

impl Tone {
    pub fn color(self) -> Retained<NSColor> {
        let (r, g, b) = match self {
            Tone::Blue => (0.039, 0.518, 1.0),
            Tone::Orange => (1.0, 0.584, 0.0),
            Tone::Green => (0.188, 0.820, 0.345),
            Tone::Red => (1.0, 0.271, 0.227),
            Tone::Purple => (0.749, 0.353, 0.950),
            Tone::Gray => (0.557, 0.557, 0.576),
        };
        NSColor::colorWithSRGBRed_green_blue_alpha(r, g, b, 1.0)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Weight {
    Light,
    Regular,
    Medium,
    Semibold,
    Bold,
}

pub fn font(size: f64, weight: Weight) -> Retained<NSFont> {
    // SAFETY: extern static weight constants.
    let w: NSFontWeight = unsafe {
        match weight {
            Weight::Light => NSFontWeightLight,
            Weight::Regular => NSFontWeightRegular,
            Weight::Medium => NSFontWeightMedium,
            Weight::Semibold => NSFontWeightSemibold,
            Weight::Bold => NSFontWeightBold,
        }
    };
    NSFont::systemFontOfSize_weight(size, w)
}

/// Digits that keep their width, so a clock does not jitter.
pub fn digits(size: f64, weight: Weight) -> Retained<NSFont> {
    // SAFETY: extern static weight constants.
    let w: NSFontWeight = unsafe {
        match weight {
            Weight::Light => NSFontWeightLight,
            Weight::Regular => NSFontWeightRegular,
            Weight::Medium => NSFontWeightMedium,
            Weight::Semibold => NSFontWeightSemibold,
            Weight::Bold => NSFontWeightBold,
        }
    };
    NSFont::monospacedDigitSystemFontOfSize_weight(size, w)
}

pub fn fill_rounded(rect: NSRect, radius: f64, color: &NSColor) {
    color.setFill();
    NSBezierPath::bezierPathWithRoundedRect_xRadius_yRadius(rect, radius, radius).fill();
}

pub fn fill_capsule(rect: NSRect, color: &NSColor) {
    fill_rounded(rect, rect.size.height / 2.0, color);
}

pub fn stroke_rounded(rect: NSRect, radius: f64, color: &NSColor, width: f64) {
    color.setStroke();
    let inset = NSRect::new(
        NSPoint::new(rect.origin.x + width / 2.0, rect.origin.y + width / 2.0),
        NSSize::new(rect.size.width - width, rect.size.height - width),
    );
    let path = NSBezierPath::bezierPathWithRoundedRect_xRadius_yRadius(inset, radius, radius);
    path.setLineWidth(width);
    path.stroke();
}

pub fn fill_circle(rect: NSRect, color: &NSColor) {
    color.setFill();
    NSBezierPath::bezierPathWithOvalInRect(rect).fill();
}

/// A vertical two-stop gradient inside a rounded shape: light at the top, darker towards the bottom.
pub fn gradient_rounded(rect: NSRect, radius: f64, top: f64, bottom: f64) {
    let path = NSBezierPath::bezierPathWithRoundedRect_xRadius_yRadius(rect, radius, radius);
    let Some(gradient) =
        NSGradient::initWithStartingColor_endingColor(NSGradient::alloc(), &white(top), &white(bottom))
    else {
        return;
    };
    // -90° draws from the top of the shape to its bottom.
    gradient.drawInBezierPath_angle(&path, -90.0);
}

/// The lit top edge of a glass shape: the hairline of the whole shape, brighter only along its top, where the
/// light would fall on it. It is what makes a flat fill read as a pane with thickness.
pub fn specular_edge(rect: NSRect, radius: f64) {
    let band = NSRect::new(
        rect.origin,
        NSSize::new(rect.size.width, rect.origin.y + 2.0),
    );
    NSGraphicsContext::saveGraphicsState_class();
    NSBezierPath::bezierPathWithRect(band).addClip();
    stroke_rounded(rect, radius, &white(CARD_SPECULAR), 1.0);
    NSGraphicsContext::restoreGraphicsState_class();
}

/// The surface a page holds its content on: a card in the notch, a row in the clipboard list, a tile.
pub fn card(rect: NSRect, radius: f64) {
    gradient_rounded(rect, radius, CARD_TOP, CARD_BOTTOM);
    stroke_rounded(rect, radius, &white(CARD_EDGE), 0.5);
    specular_edge(rect, radius);
}

/// The same, for the card the user is on: it lifts, and the accent colour runs through it.
pub fn card_selected(rect: NSRect, radius: f64, tone: &Tone) {
    card_selected_color(rect, radius, &tone.color());
}

/// The same, tinted with any colour: the system accent, or the tone a page gives a state.
pub fn card_selected_color(rect: NSRect, radius: f64, tint: &NSColor) {
    let path = NSBezierPath::bezierPathWithRoundedRect_xRadius_yRadius(rect, radius, radius);
    let Some(gradient) = NSGradient::initWithStartingColor_endingColor(
        NSGradient::alloc(),
        &tint.colorWithAlphaComponent(0.58),
        &tint.colorWithAlphaComponent(0.34),
    ) else {
        return;
    };
    gradient.drawInBezierPath_angle(&path, -90.0);
    stroke_rounded(rect, radius, &tint.colorWithAlphaComponent(0.55), 1.0);
    specular_edge(rect, radius);
}

/// A well: a recessed area — a search field, a list's background, the track a row of tabs slides in.
pub fn well(rect: NSRect, radius: f64) {
    gradient_rounded(rect, radius, WELL_TOP, WELL_BOTTOM);
    stroke_rounded(rect, radius, &white(WELL_EDGE), 0.5);
}

/// Bright glass: the pill that marks the chosen tab or segment. It is the one shape on the surface the light
/// goes through, so the ink on it is dark.
pub fn card_bright(rect: NSRect, radius: f64) {
    card_bright_alpha(rect, radius, 1.0);
}

/// The same at `dim` of its brightness, which is how a solid button shows rest, hover and press.
pub fn card_bright_alpha(rect: NSRect, radius: f64, dim: f64) {
    let dim = dim.clamp(0.0, 1.0);
    gradient_rounded(rect, radius, 0.96 * dim, 0.84 * dim);
    stroke_rounded(rect, radius, &white(0.99 * dim), 0.5);
    specular_edge(rect, radius);
}

/// A chip, a badge, a control that is not the chosen one yet: the resting, hovered and pressed alpha of white
/// over whatever holds it.
pub fn control(rect: NSRect, radius: f64, state: Control) {
    let alpha = match state {
        Control::Rest => CONTROL,
        Control::Chosen => CONTROL_HOVER + 0.06,
        Control::Hover => CONTROL_HOVER,
        Control::Pressed => CONTROL_PRESSED,
        Control::Disabled => CONTROL * 0.62,
    };
    fill_rounded(rect, radius, &white(alpha));
    stroke_rounded(rect, radius, &white(alpha + 0.06), 0.5);
    specular_edge(rect, radius);
}

/// How a control is being used, which is all that separates its looks.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Control {
    Rest,
    Chosen,
    Hover,
    Pressed,
    Disabled,
}

type SymbolKey = (String, u32, bool, [u32; 4]);

thread_local! {
    static SYMBOLS: RefCell<HashMap<SymbolKey, Retained<NSImage>>> = RefCell::new(HashMap::new());
}

/// An SF Symbol drawn at `size` points on its longer side in a single colour, or `None` for a name the
/// system does not know. The image draws itself when it is shown, so it stays sharp at any scale.
pub fn symbol(name: &str, size: f64, color: &NSColor) -> Option<Retained<NSImage>> {
    symbol_variant(name, size, color, false)
}

/// The same, drawn from SF Symbols' Small variant: heavier strokes and less detail, the one the system asks
/// for below 17 points. The Medium variant scaled down to that size is what looks thin and mushy there.
pub fn small_symbol(name: &str, size: f64, color: &NSColor) -> Option<Retained<NSImage>> {
    symbol_variant(name, size, color, true)
}

fn symbol_variant(name: &str, size: f64, color: &NSColor, small: bool) -> Option<Retained<NSImage>> {
    let srgb = color
        .colorUsingColorSpace(&objc2_app_kit::NSColorSpace::sRGBColorSpace())
        .unwrap_or_else(|| color.retain());
    let parts = [
        srgb.redComponent(),
        srgb.greenComponent(),
        srgb.blueComponent(),
        srgb.alphaComponent(),
    ]
    .map(|c| (c * 255.0).round() as u32);
    let key = (name.to_owned(), (size * 10.0) as u32, small, parts);
    if let Some(found) = SYMBOLS.with(|cache| cache.borrow().get(&key).cloned()) {
        return Some(found);
    }
    let base = NSImage::imageWithSystemSymbolName_accessibilityDescription(
        &NSString::from_str(name),
        None,
    )?;
    // SF Symbols' Small variant, which is the one the system uses below 17 points; a configuration the
    // system will not apply leaves the symbol as it is.
    let base = if small {
        // SAFETY: extern statics are process-lifetime constants.
        let weight = unsafe { NSFontWeightRegular };
        base.imageWithSymbolConfiguration(
            &NSImageSymbolConfiguration::configurationWithPointSize_weight_scale(
                size,
                weight,
                NSImageSymbolScale::Small,
            ),
        )
        .unwrap_or(base)
    } else {
        base
    };
    let natural = base.size();
    let scale = size / natural.width.max(natural.height).max(1.0);
    let target = NSSize::new(natural.width * scale, natural.height * scale);
    let tint = color.retain();
    let draw = RcBlock::new(move |rect: NSRect| {
        base.drawInRect(rect);
        tint.set();
        objc2_app_kit::NSRectFillUsingOperation(rect, NSCompositingOperation::SourceIn);
        objc2::runtime::Bool::YES
    });
    let image = NSImage::imageWithSize_flipped_drawingHandler(target, false, &draw);
    SYMBOLS.with(|cache| cache.borrow_mut().insert(key, image.clone()));
    Some(image)
}

/// Draws a symbol centred in `rect`, fitted to `size` points. Does nothing for an unknown name.
pub fn draw_symbol(name: &str, centre: NSPoint, size: f64, color: &NSColor) {
    if let Some(image) = symbol(name, size, color) {
        let natural = image.size();
        let rect = NSRect::new(
            NSPoint::new(
                centre.x - natural.width / 2.0,
                centre.y - natural.height / 2.0,
            ),
            natural,
        );
        image.drawInRect(rect);
    }
}

/// How far an animation has got once `t` (0 to 1) of its time has passed.
pub fn ease_out(t: f64) -> f64 {
    let t = t.clamp(0.0, 1.0);
    1.0 - (1.0 - t).powi(3)
}

/// The same for something that both starts and stops moving, such as a page sliding in.
pub fn ease_in_out(t: f64) -> f64 {
    let t = t.clamp(0.0, 1.0);
    if t < 0.5 {
        4.0 * t * t * t
    } else {
        1.0 - (-2.0 * t + 2.0).powi(3) / 2.0
    }
}

/// A value between two states, moved `t` of the way from one to the other.
pub fn mix(from: f64, to: f64, t: f64) -> f64 {
    from + (to - from) * t.clamp(0.0, 1.0)
}

/// A spring that settles with one small overshoot, which is what makes a surface feel like it has weight.
pub fn spring(t: f64) -> f64 {
    let t = t.clamp(0.0, 1.0);
    if t >= 1.0 {
        return 1.0;
    }
    // Damped oscillation: one overshoot of about 4%, at rest well before the end.
    1.0 - (-10.0 * t).exp() * (8.0 * t).cos()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_curves_start_at_zero_and_end_at_one() {
        for curve in [ease_out, spring] {
            assert!(curve(0.0).abs() < 1e-9);
            assert!((curve(1.0) - 1.0).abs() < 1e-9);
            assert!(
                (curve(2.0) - 1.0).abs() < 1e-9,
                "past the end it stays at the end"
            );
        }
    }

    #[test]
    fn the_spring_overshoots_a_little_and_never_runs_away() {
        let peak = (0..=100)
            .map(|i| spring(f64::from(i) / 100.0))
            .fold(0.0, f64::max);
        assert!(peak > 1.02 && peak < 1.08, "{peak}");
        assert!(
            (spring(0.9) - 1.0).abs() < 0.005,
            "close to rest before the end: {}",
            spring(0.9)
        );
    }

    #[test]
    fn ease_out_only_moves_forward() {
        let mut last = -1.0;
        for i in 0..=50 {
            let v = ease_out(f64::from(i) / 50.0);
            assert!(v >= last);
            last = v;
        }
    }
}
