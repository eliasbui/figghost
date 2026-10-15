//! The look shared by every page of the notch: one set of colours, type and shapes, symbols, and the
//! motion curves. The surfaces are dark, controls are translucent white on them, and the only
//! solid white is what the user is looking at or is about to press.

use block2::RcBlock;
use objc2::{Message, rc::Retained};
use objc2_app_kit::{
    NSBezierPath, NSColor, NSCompositingOperation, NSFont, NSFontWeight, NSFontWeightBold,
    NSFontWeightLight, NSFontWeightMedium, NSFontWeightRegular, NSFontWeightSemibold, NSImage,
};
use objc2_foundation::{NSPoint, NSRect, NSSize, NSString};
use std::{cell::RefCell, collections::HashMap};

/// Alpha of white used for a control that rests on the dark surface.
pub const CONTROL: f64 = 0.10;
pub const CONTROL_HOVER: f64 = 0.17;
pub const CONTROL_PRESSED: f64 = 0.24;
pub const TEXT: f64 = 0.95;
pub const TEXT_SECONDARY: f64 = 0.58;
pub const TEXT_TERTIARY: f64 = 0.38;
/// The radius of the shapes that hold content, and of the buttons that sit in them.
pub const CARD_RADIUS: f64 = 18.0;
pub const BUTTON_RADIUS: f64 = 12.0;
/// The bottom corners of the open notch.
pub const PANEL_RADIUS: f64 = 28.0;

pub fn white(alpha: f64) -> Retained<NSColor> {
    NSColor::colorWithWhite_alpha(1.0, alpha)
}

pub fn black(alpha: f64) -> Retained<NSColor> {
    NSColor::colorWithWhite_alpha(0.0, alpha)
}

/// The colour of a state: running, waiting, failed, done.
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
            Tone::Blue => (0.25, 0.55, 1.0),
            Tone::Orange => (1.0, 0.62, 0.15),
            Tone::Green => (0.30, 0.82, 0.45),
            Tone::Red => (1.0, 0.36, 0.34),
            Tone::Purple => (0.68, 0.50, 1.0),
            Tone::Gray => (0.60, 0.61, 0.64),
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

type SymbolKey = (String, u32, [u32; 4]);

thread_local! {
    static SYMBOLS: RefCell<HashMap<SymbolKey, Retained<NSImage>>> = RefCell::new(HashMap::new());
}

/// An SF Symbol drawn at `size` points on its longer side in a single colour, or `None` for a name the
/// system does not know. The image draws itself when it is shown, so it stays sharp at any scale.
pub fn symbol(name: &str, size: f64, color: &NSColor) -> Option<Retained<NSImage>> {
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
    let key = (name.to_owned(), (size * 10.0) as u32, parts);
    if let Some(found) = SYMBOLS.with(|cache| cache.borrow().get(&key).cloned()) {
        return Some(found);
    }
    let base = NSImage::imageWithSystemSymbolName_accessibilityDescription(
        &NSString::from_str(name),
        None,
    )?;
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
