//! Figghost's hello: each time the app is opened by hand, its ghost comes down out of the notch, blinks once and
//! goes back up. A click-through panel just under the notch plays it with Core Animation, so it is smooth on any
//! display and nothing runs once it is over. The art is two pictures of the ghost that differ only in the eyes.

use crate::{app, panel::FigPanel};
use objc2::{AnyThread, MainThreadMarker, rc::Retained, runtime::AnyObject};
use objc2_app_kit::{NSImage, NSMainMenuWindowLevel, NSView, NSWorkspace};
use objc2_foundation::{NSArray, NSData, NSNumber, NSPoint, NSRect, NSSize, NSString};
use objc2_quartz_core::{
    CAKeyframeAnimation, CALayer, CAMediaTiming, CAMediaTimingFunction, kCAAnimationDiscrete,
    kCAGravityResizeAspect,
};

/// The side of the pictures on screen, in points; the ghost takes about three quarters of it.
const SIDE: f64 = 112.0;
/// How far below the notch the ghost's centre comes to rest.
const DROP: f64 = 74.0;
/// The panel under the notch: room for the ghost at rest, its overshoot and its bobbing.
const PANEL: NSSize = NSSize::new(160.0, DROP + SIDE / 2.0 + 16.0);
/// The whole moment, in seconds.
pub const SECONDS: f64 = 2.9;
/// When the eyes close and open again, in seconds.
const BLINK: (f64, f64) = (1.35, 1.47);

/// Plays the hello once under `notch`, the resting notch in screen coordinates, and takes it off the screen when it
/// is over. `open` and `blink` are PNG pictures of the ghost with its eyes open and closed; nothing happens for
/// pictures that cannot be read. With Reduce Motion the ghost only fades in where it would rest, blinks and fades.
pub fn play(mtm: MainThreadMarker, notch: NSRect, open: &[u8], blink: &[u8]) {
    let (Some(open), Some(blink)) = (image(open), image(blink)) else {
        return;
    };
    let frame = NSRect::new(
        NSPoint::new(
            notch.origin.x + (notch.size.width - PANEL.width) / 2.0,
            notch.origin.y - PANEL.height,
        ),
        PANEL,
    );
    let panel = FigPanel::new(mtm, frame);
    // Above the notch, as the mascot is, and never in the way of a click.
    panel.setLevel(NSMainMenuWindowLevel + 2);
    panel.setIgnoresMouseEvents(true);
    let view = NSView::initWithFrame(mtm.alloc(), NSRect::new(NSPoint::ZERO, PANEL));
    view.setWantsLayer(true);
    panel.setContentView(Some(&view));
    let Some(root) = view.layer() else {
        return;
    };
    // The panel's top edge is the bottom of the notch and the ghost is cut off there, so it seems to come out of
    // the notch and go back into it.
    root.setMasksToBounds(true);

    let ghost = CALayer::new();
    ghost.setBounds(NSRect::new(NSPoint::ZERO, NSSize::new(SIDE, SIDE)));
    ghost.setPosition(NSPoint::new(PANEL.width / 2.0, PANEL.height - DROP));
    // Gone once the animations end, before the panel goes.
    ghost.setOpacity(0.0);
    let scale = panel.backingScaleFactor();
    let face = |picture: &NSImage| {
        let layer = CALayer::new();
        layer.setFrame(ghost.bounds());
        // SAFETY: a constant of Core Animation, read on the main thread.
        layer.setContentsGravity(unsafe { kCAGravityResizeAspect });
        layer.setContentsScale(scale);
        // SAFETY: on macOS an NSImage is a valid value for a layer's contents.
        unsafe { layer.setContents(Some(picture)) };
        ghost.addSublayer(&layer);
        layer
    };
    face(&open);
    // The closed eyes lie over the open ones and show only while it blinks.
    let closed = face(&blink);
    closed.setOpacity(0.0);
    root.addSublayer(&ghost);

    let still = NSWorkspace::sharedWorkspace().accessibilityDisplayShouldReduceMotion();
    animate(&ghost, &closed, still);
    panel.orderFrontRegardless();
    // The panel is kept alive by this closure until the moment is over.
    app::after(SECONDS + 0.1, move || panel.orderOut(None));
}

fn image(png: &[u8]) -> Option<Retained<NSImage>> {
    NSImage::initWithData(NSImage::alloc(), &NSData::with_bytes(png))
}

/// The movements, as keyframes over the whole moment.
fn animate(ghost: &CALayer, closed: &CALayer, still: bool) {
    let top = PANEL.height;
    let rest = PANEL.height - DROP;
    let (ease_out, ease_in, ease_in_out, linear) = (
        // Quick at first, settling at the end: the drop out of the notch.
        curve(0.2, 0.9, 0.3, 1.0),
        curve(0.5, 0.0, 0.8, 0.4),
        curve(0.45, 0.0, 0.55, 1.0),
        curve(0.0, 0.0, 1.0, 1.0),
    );
    if !still {
        // Down out of the notch past its resting place by 4 points, back up to it, a slow bob of 3 points either
        // way, and back up into the notch.
        let y = [
            (0.0, top),
            (0.15, top),
            (0.70, rest - 4.0),
            (0.85, rest),
            (1.15, rest + 3.0),
            (1.45, rest),
            (1.75, rest - 3.0),
            (2.05, rest),
            (2.10, rest),
            (2.70, top),
            (SECONDS, top),
        ];
        let y_curves = [
            &linear,
            &ease_out,
            &ease_in_out,
            &ease_in_out,
            &ease_in_out,
            &ease_in_out,
            &ease_in_out,
            &linear,
            &ease_in,
            &linear,
        ];
        ghost.addAnimation_forKey(&track("position.y", &y, &y_curves), Some(&NSString::from_str("drop")));
        let grow = [
            (0.0, 0.55),
            (0.15, 0.55),
            (0.85, 1.0),
            (2.10, 1.0),
            (2.70, 0.55),
            (SECONDS, 0.55),
        ];
        let grow_curves = [&linear, &ease_out, &linear, &ease_in, &linear];
        ghost.addAnimation_forKey(
            &track("transform.scale", &grow, &grow_curves),
            Some(&NSString::from_str("grow")),
        );
    }
    let fade = [
        (0.0, 0.0),
        (0.15, 0.0),
        (0.55, 1.0),
        (2.25, 1.0),
        (2.70, 0.0),
        (SECONDS, 0.0),
    ];
    let fade_curves = [&linear, &ease_out, &linear, &ease_in, &linear];
    ghost.addAnimation_forKey(&track("opacity", &fade, &fade_curves), Some(&NSString::from_str("fade")));

    // The blink is a cut, not a fade: closed from the first moment to the second, open again after.
    let blink = CAKeyframeAnimation::animationWithKeyPath(Some(&NSString::from_str("opacity")));
    // SAFETY: numbers are valid values for an opacity animation.
    unsafe { blink.setValues(Some(&numbers(&[0.0, 1.0, 0.0]))) };
    blink.setKeyTimes(Some(&times(&[0.0, BLINK.0, BLINK.1, SECONDS])));
    // SAFETY: a constant of Core Animation, read on the main thread.
    blink.setCalculationMode(unsafe { kCAAnimationDiscrete });
    blink.setDuration(SECONDS);
    closed.addAnimation_forKey(&blink, Some(&NSString::from_str("blink")));
}

fn curve(c1x: f32, c1y: f32, c2x: f32, c2y: f32) -> Retained<CAMediaTimingFunction> {
    CAMediaTimingFunction::functionWithControlPoints(c1x, c1y, c2x, c2y)
}

/// A keyframe animation of `key` through `points` (seconds into the moment, value), with `curves[i]` from point
/// `i` to point `i + 1`.
fn track(
    key: &str,
    points: &[(f64, f64)],
    curves: &[&Retained<CAMediaTimingFunction>],
) -> Retained<CAKeyframeAnimation> {
    let anim = CAKeyframeAnimation::animationWithKeyPath(Some(&NSString::from_str(key)));
    let values: Vec<f64> = points.iter().map(|&(_, v)| v).collect();
    let moments: Vec<f64> = points.iter().map(|&(t, _)| t).collect();
    // SAFETY: numbers are valid values for position, scale and opacity animations.
    unsafe { anim.setValues(Some(&numbers(&values))) };
    anim.setKeyTimes(Some(&times(&moments)));
    let curves: Vec<Retained<CAMediaTimingFunction>> = curves.iter().map(|&c| c.clone()).collect();
    anim.setTimingFunctions(Some(&NSArray::from_retained_slice(&curves)));
    anim.setDuration(SECONDS);
    anim
}

fn numbers(values: &[f64]) -> Retained<NSArray> {
    let objects: Vec<Retained<AnyObject>> = values
        .iter()
        .map(|&v| Retained::into_super(Retained::into_super(Retained::into_super(NSNumber::new_f64(v)))))
        .collect();
    NSArray::from_retained_slice(&objects)
}

/// Moments in seconds as fractions of the whole, which is how Core Animation takes them.
fn times(seconds: &[f64]) -> Retained<NSArray<NSNumber>> {
    let fractions: Vec<Retained<NSNumber>> = seconds.iter().map(|&s| NSNumber::new_f64(s / SECONDS)).collect();
    NSArray::from_retained_slice(&fractions)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_moments_stay_inside_the_whole_and_in_order() {
        assert!(BLINK.0 < BLINK.1 && BLINK.1 < SECONDS);
        // The ghost rests inside the panel, with room below for its overshoot and its bob.
        assert!(PANEL.height - DROP - SIDE / 2.0 >= 4.0 + 3.0);
    }
}
