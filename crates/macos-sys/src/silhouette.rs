//! The outline of the notch at one moment of its movement.
//!
//! Open, the shape hangs from the top edge of the screen with concave fillets where it meets the menu bar band, and
//! its bottom corners are round. While it grows out of the closed notch, or goes back into it, its top is pinched in
//! toward the closed shape's width and widens into the body through an S-curve, so what leaves the notch is a drop
//! that fills out into the panel, not a rectangle that scales. The outline is plain geometry here, so it can be
//! tested; `cg_path` turns it into a Core Graphics path for a `CAShapeLayer`.

use objc2_core_foundation::CFRetained;
use objc2_core_graphics::{CGMutablePath, CGPath};

/// Control point distance of a cubic Bezier that follows a quarter of a circle.
const KAPPA: f64 = 0.552_284_75;

/// The share of the height the pinched neck and the S-curve below it take at full pinch.
const NECK_SHARE: f64 = 0.08;
const EASE_SHARE: f64 = 0.60;
/// How close to the closed width the neck gets at full pinch: it never closes up completely.
const NECK_DEPTH: f64 = 0.9;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Cmd {
    Move(f64, f64),
    Line(f64, f64),
    Cubic(f64, f64, f64, f64, f64, f64),
    Close,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Outline {
    pub width: f64,
    pub height: f64,
    /// Radius of the bottom corners.
    pub radius: f64,
    /// Radius of the concave corners at the top; 0 ends the shape square against the edge.
    pub flare: f64,
    /// How far the top is pinched in, from 0 (not at all) to 1 (down to `neck_half`).
    pub pinch: f64,
    /// Half the width of the closed shape: what the neck narrows toward.
    pub neck_half: f64,
}

/// The way an outline reads for a shape of this height, as the share of the way open it is (0 closed, 1 open) given
/// the height of the closed shape and of the smallest open one. Everything that depends on how open the shape is
/// (the pinch, the fillets, which layers show) reads it from the shape's own height, so a movement needs no extra
/// state: the shape is as open as it looks.
pub fn openness(height: f64, closed: f64, open: f64) -> f64 {
    if open <= closed {
        return if height > closed { 1.0 } else { 0.0 };
    }
    ((height - closed) / (open - closed)).clamp(0.0, 1.0)
}

/// Eases in and out between two edges.
pub fn smooth(edge0: f64, edge1: f64, x: f64) -> f64 {
    let t = ((x - edge0) / (edge1 - edge0)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

impl Outline {
    /// The half width of the top edge of the body: the body's own while nothing is pinched.
    fn top_half(&self) -> f64 {
        let half = self.width / 2.0;
        let floor = self.neck_half.min(half);
        (half - self.pinch.clamp(0.0, 1.0) * (half - floor) * NECK_DEPTH).max(floor)
    }

    /// The path as drawing commands, with the body's left edge at `margin` and its bottom edge at 0 (y grows up).
    pub fn commands(&self, margin: f64) -> Vec<Cmd> {
        let half = self.width / 2.0;
        let cx = margin + half;
        let top = self.height;
        let r = self.radius.min(half).min(self.height / 2.0).max(0.0);
        let th = self.top_half();
        // A fillet needs room: the margin outside the body, and some height inside it.
        let fr = self
            .flare
            .clamp(0.0, margin.max(0.0))
            .min((top - r).max(0.0) / 2.0);
        let pinch = self.pinch.clamp(0.0, 1.0);
        let avail = (top - fr - r).max(0.0);
        // The S-curve is longer than the step it makes, so the shoulder of the drop is round and not a ledge.
        let (mut waist, mut ease) = (
            (pinch * NECK_SHARE * top).min(avail * 0.5),
            (pinch * EASE_SHARE * top).max((half - th) * 1.3).min(avail),
        );
        if half - th < 0.5 {
            waist = 0.0;
            ease = 0.0;
        }
        ease = ease.min(avail - waist).max(0.0);
        let (y1, y2) = (top - fr - waist, top - fr - waist - ease);
        let k = KAPPA;
        let mut c = Vec::with_capacity(16);
        c.push(Cmd::Move(cx - th - fr, top));
        c.push(Cmd::Line(cx + th + fr, top));
        // The right fillet: a quarter circle about a point outside the shape.
        c.push(Cmd::Cubic(
            cx + th + fr - k * fr,
            top,
            cx + th,
            top - fr + k * fr,
            cx + th,
            top - fr,
        ));
        c.push(Cmd::Line(cx + th, y1));
        if ease > 0.0 {
            c.push(Cmd::Cubic(
                cx + th,
                y1 - ease * 0.55,
                cx + half,
                y2 + ease * 0.55,
                cx + half,
                y2,
            ));
        } else if (th - half).abs() > f64::EPSILON {
            c.push(Cmd::Line(cx + half, y1));
        }
        c.push(Cmd::Line(cx + half, r));
        c.push(Cmd::Cubic(
            cx + half,
            r - k * r,
            cx + half - r + k * r,
            0.0,
            cx + half - r,
            0.0,
        ));
        c.push(Cmd::Line(cx - half + r, 0.0));
        c.push(Cmd::Cubic(
            cx - half + r - k * r,
            0.0,
            cx - half,
            r - k * r,
            cx - half,
            r,
        ));
        c.push(Cmd::Line(cx - half, y2));
        if ease > 0.0 {
            c.push(Cmd::Cubic(
                cx - half,
                y2 + ease * 0.55,
                cx - th,
                y1 - ease * 0.55,
                cx - th,
                y1,
            ));
        } else if (th - half).abs() > f64::EPSILON {
            c.push(Cmd::Line(cx - th, y1));
        }
        c.push(Cmd::Line(cx - th, top - fr));
        c.push(Cmd::Cubic(
            cx - th,
            top - fr + k * fr,
            cx - th - fr + k * fr,
            top,
            cx - th - fr,
            top,
        ));
        c.push(Cmd::Close);
        c
    }

    /// The path for a layer, in a layer whose origin is the bottom left corner of the body's margin box.
    pub fn cg_path(&self, margin: f64) -> CFRetained<CGPath> {
        let path = CGMutablePath::new();
        for cmd in self.commands(margin) {
            // SAFETY: a null transform means no transform; the path is a valid mutable path owned here.
            unsafe {
                match cmd {
                    Cmd::Move(x, y) => {
                        CGMutablePath::move_to_point(Some(&path), std::ptr::null(), x, y)
                    }
                    Cmd::Line(x, y) => {
                        CGMutablePath::add_line_to_point(Some(&path), std::ptr::null(), x, y)
                    }
                    Cmd::Cubic(a, b, c, d, x, y) => CGMutablePath::add_curve_to_point(
                        Some(&path),
                        std::ptr::null(),
                        a,
                        b,
                        c,
                        d,
                        x,
                        y,
                    ),
                    Cmd::Close => CGMutablePath::close_subpath(Some(&path)),
                }
            }
        }
        CGPath::new_copy(Some(&path)).expect("copying a valid path")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn extent(commands: &[Cmd]) -> (f64, f64, f64, f64) {
        let (mut x0, mut x1, mut y0, mut y1) = (f64::MAX, f64::MIN, f64::MAX, f64::MIN);
        let mut see = |x: f64, y: f64| {
            x0 = x0.min(x);
            x1 = x1.max(x);
            y0 = y0.min(y);
            y1 = y1.max(y);
        };
        for c in commands {
            match *c {
                Cmd::Move(x, y) | Cmd::Line(x, y) => see(x, y),
                Cmd::Cubic(_, _, _, _, x, y) => see(x, y),
                Cmd::Close => {}
            }
        }
        (x0, x1, y0, y1)
    }

    fn plain() -> Outline {
        Outline {
            width: 500.0,
            height: 440.0,
            radius: 28.0,
            flare: 0.0,
            pinch: 0.0,
            neck_half: 100.0,
        }
    }

    #[test]
    fn a_plain_outline_is_the_body_with_a_square_top() {
        let (x0, x1, y0, y1) = extent(&plain().commands(20.0));
        assert_eq!((x0, x1), (20.0, 520.0));
        assert_eq!((y0, y1), (0.0, 440.0));
    }

    #[test]
    fn fillets_reach_out_into_the_margin_and_never_past_it() {
        let flared = Outline {
            flare: 20.0,
            ..plain()
        };
        let (x0, x1, _, y1) = extent(&flared.commands(20.0));
        assert_eq!(
            (x0, x1),
            (0.0, 540.0),
            "the top edge is a fillet wider than the body"
        );
        assert_eq!(y1, 440.0);
        let too_big = Outline {
            flare: 50.0,
            ..plain()
        };
        let (x0, x1, _, _) = extent(&too_big.commands(20.0));
        assert_eq!(
            (x0, x1),
            (0.0, 540.0),
            "a fillet is cut to the margin it has"
        );
    }

    #[test]
    fn the_pinch_narrows_the_top_toward_the_closed_width_and_not_below_it() {
        let full = Outline {
            pinch: 1.0,
            ..plain()
        };
        let neck = full.top_half();
        assert!((100.0..250.0).contains(&neck), "{neck}");
        assert!((neck - (250.0 - 150.0 * NECK_DEPTH)).abs() < 1e-9);
        let narrower_than_closed = Outline {
            neck_half: 400.0,
            pinch: 1.0,
            ..plain()
        };
        assert_eq!(
            narrower_than_closed.top_half(),
            250.0,
            "a body already narrower than the neck is not pinched"
        );
    }

    #[test]
    fn a_pinched_outline_leaves_the_body_at_its_full_width_below_the_neck() {
        let drop = Outline {
            pinch: 1.0,
            flare: 0.0,
            ..plain()
        };
        let (x0, x1, y0, y1) = extent(&drop.commands(20.0));
        assert_eq!((x0, x1), (20.0, 520.0));
        assert_eq!((y0, y1), (0.0, 440.0));
        let cubics = drop
            .commands(20.0)
            .iter()
            .filter(|c| matches!(c, Cmd::Cubic(..)))
            .count();
        assert!(
            cubics >= 6,
            "two bottom corners, two S-curves and the fillets: {cubics}"
        );
    }

    #[test]
    fn openness_follows_the_height() {
        assert_eq!(openness(32.0, 32.0, 150.0), 0.0);
        assert_eq!(openness(150.0, 32.0, 150.0), 1.0);
        assert_eq!(openness(440.0, 32.0, 150.0), 1.0);
        assert!((openness(91.0, 32.0, 150.0) - 0.5).abs() < 1e-9);
    }

    #[test]
    fn a_tiny_shape_does_not_produce_a_path_that_folds_over_itself() {
        let tiny = Outline {
            width: 40.0,
            height: 12.0,
            radius: 28.0,
            flare: 20.0,
            pinch: 1.0,
            neck_half: 10.0,
        };
        let (x0, x1, y0, y1) = extent(&tiny.commands(20.0));
        assert!(
            x0 >= 0.0 && x1 <= 80.0 && y0 >= 0.0 && y1 <= 12.0,
            "{x0} {x1} {y0} {y1}"
        );
    }
}
