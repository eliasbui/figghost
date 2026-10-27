//! The arrangement of the displays as plain geometry: where a display that was dragged lands, which origins make
//! another display the main one, and how the desktop fits the canvas of the tab. Rectangles of the desktop are in
//! points, y downwards from the top-left of the main display, as Core Graphics places displays; rectangles of the
//! canvas have y upwards, as AppKit draws.

use core_capture::Rect;

/// Two displays side by side share at least this much of an edge, or all of the shorter one's.
const MIN_SHARED: f64 = 40.0;
/// Displays that come closer than this to overlapping are taken to overlap.
const OVERLAP_SLACK: f64 = 0.5;

/// One display of the desktop: its id and where it is.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Place {
    pub id: u32,
    pub rect: Rect,
}

/// Where a display dropped at `rect` goes: against the side of another display nearest to where it was dropped,
/// sharing some of that side, covering no display, and lined up with the other's edges or middle when it is within
/// `snap` of them. `None` when there is no other display, or no side to put it against.
pub fn settle(rect: Rect, others: &[Rect], snap: f64) -> Option<Rect> {
    let mut best: Option<(f64, Rect)> = None;
    for other in others {
        let beside = |x: f64| Rect {
            x,
            y: slide(rect.y, rect.h, other.y, other.h, snap),
            ..rect
        };
        let over = |y: f64| Rect {
            x: slide(rect.x, rect.w, other.x, other.w, snap),
            y,
            ..rect
        };
        for candidate in [
            beside(other.max_x()),
            beside(other.x - rect.w),
            over(other.max_y()),
            over(other.y - rect.h),
        ] {
            if others.iter().any(|o| overlaps(&candidate, o)) {
                continue;
            }
            let distance = (candidate.x - rect.x).hypot(candidate.y - rect.y);
            if best.is_none_or(|(d, _)| distance < d) {
                best = Some((distance, candidate));
            }
        }
    }
    best.map(|(_, r)| Rect::new(r.x.round(), r.y.round(), r.w, r.h))
}

/// Where along a side that starts at `start` and is `side` long an edge `len` long that was dropped at `at` goes: in
/// far enough to share some of the side, and lined up with its start, end or middle when within `snap` of one.
fn slide(at: f64, len: f64, start: f64, side: f64, snap: f64) -> f64 {
    let shared = MIN_SHARED.min(len).min(side);
    let at = at.clamp(start - len + shared, start + side - shared);
    [start, start + side - len, start + (side - len) / 2.0]
        .into_iter()
        .filter(|line| (line - at).abs() <= snap)
        .min_by(|a, b| (a - at).abs().total_cmp(&(b - at).abs()))
        .unwrap_or(at)
}

fn overlaps(a: &Rect, b: &Rect) -> bool {
    a.x < b.max_x() - OVERLAP_SLACK
        && b.x < a.max_x() - OVERLAP_SLACK
        && a.y < b.max_y() - OVERLAP_SLACK
        && b.y < a.max_y() - OVERLAP_SLACK
}

/// The origins of every display once `moved` is at `to`, all shifted so that the main display stays at (0, 0):
/// moving the main display moves the others the other way, as in System Settings.
pub fn origins_after_move(
    places: &[Place],
    main: u32,
    moved: u32,
    to: Rect,
) -> Vec<(u32, i32, i32)> {
    let rects: Vec<(u32, Rect)> = places
        .iter()
        .map(|p| (p.id, if p.id == moved { to } else { p.rect }))
        .collect();
    let anchor = rects
        .iter()
        .find(|(id, _)| *id == main)
        .map_or((0.0, 0.0), |(_, r)| (r.x, r.y));
    origins_from(&rects, anchor)
}

/// The origins that make `id` the main display, the one at (0, 0), with the arrangement kept.
pub fn origins_with_main(places: &[Place], id: u32) -> Option<Vec<(u32, i32, i32)>> {
    let anchor = places.iter().find(|p| p.id == id)?.rect;
    let rects: Vec<(u32, Rect)> = places.iter().map(|p| (p.id, p.rect)).collect();
    Some(origins_from(&rects, (anchor.x, anchor.y)))
}

fn origins_from(rects: &[(u32, Rect)], anchor: (f64, f64)) -> Vec<(u32, i32, i32)> {
    rects
        .iter()
        .map(|(id, r)| {
            (
                *id,
                (r.x - anchor.0).round() as i32,
                (r.y - anchor.1).round() as i32,
            )
        })
        .collect()
}

/// How the desktop sits on the canvas: the scale from points of the desktop to points of the canvas, the top-left of
/// the desktop's bounds, and where on the canvas that corner is.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Fit {
    pub scale: f64,
    left: f64,
    top: f64,
    canvas_left: f64,
    canvas_top: f64,
}

impl Fit {
    /// The desktop as large as it fits in a canvas of `width` x `height` with `margin` around it, in its middle.
    pub fn new(rects: &[Rect], width: f64, height: f64, margin: f64) -> Self {
        let left = rects.iter().map(|r| r.x).fold(f64::INFINITY, f64::min);
        let top = rects.iter().map(|r| r.y).fold(f64::INFINITY, f64::min);
        let right = rects
            .iter()
            .map(Rect::max_x)
            .fold(f64::NEG_INFINITY, f64::max);
        let bottom = rects
            .iter()
            .map(Rect::max_y)
            .fold(f64::NEG_INFINITY, f64::max);
        if !(left.is_finite() && top.is_finite() && right > left && bottom > top) {
            return Self {
                scale: 1.0,
                left: 0.0,
                top: 0.0,
                canvas_left: margin,
                canvas_top: height - margin,
            };
        }
        let (w, h) = (right - left, bottom - top);
        let scale = ((width - 2.0 * margin) / w)
            .min((height - 2.0 * margin) / h)
            .max(1e-6);
        Self {
            scale,
            left,
            top,
            canvas_left: (width - w * scale) / 2.0,
            canvas_top: height - (height - h * scale) / 2.0,
        }
    }

    /// A rectangle of the desktop on the canvas.
    pub fn to_canvas(self, r: Rect) -> Rect {
        Rect::new(
            self.canvas_left + (r.x - self.left) * self.scale,
            self.canvas_top - (r.y - self.top + r.h) * self.scale,
            r.w * self.scale,
            r.h * self.scale,
        )
    }

    /// Where a rectangle of the canvas is in the desktop.
    pub fn to_desktop(self, c: Rect) -> Rect {
        Rect::new(
            self.left + (c.x - self.canvas_left) / self.scale,
            self.top + (self.canvas_top - c.max_y()) / self.scale,
            c.w / self.scale,
            c.h / self.scale,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const LAPTOP: Rect = Rect::new(0.0, 0.0, 1512.0, 982.0);
    const MONITOR: Rect = Rect::new(1512.0, -458.0, 2560.0, 1440.0);

    #[test]
    fn a_display_dropped_near_a_side_goes_flush_against_it() {
        // Dropped a little to the right of the laptop with a gap and too high: flush, and in far enough to share.
        let dropped = Rect::new(1600.0, -1400.0, 2560.0, 1440.0);
        let placed = settle(dropped, &[LAPTOP], 20.0).unwrap();
        assert_eq!((placed.x, placed.w), (1512.0, 2560.0));
        assert_eq!(placed.y, -1400.0, "far from any line, it keeps its height");
        assert!(
            placed.max_y() >= LAPTOP.y + MIN_SHARED,
            "and shares the edge: {placed:?}"
        );
        // Dropped onto the laptop from the left: it goes to the left side, not through it.
        let left = settle(Rect::new(-1800.0, 100.0, 1920.0, 1080.0), &[LAPTOP], 20.0).unwrap();
        assert_eq!(left.x, -1920.0);
        // Dropped mostly above: it sits on top.
        let above = settle(Rect::new(-200.0, -1000.0, 1920.0, 1080.0), &[LAPTOP], 20.0).unwrap();
        assert_eq!(above.y, -1080.0);
    }

    #[test]
    fn edges_line_up_when_they_are_close() {
        let near_top = settle(Rect::new(1512.0, 12.0, 1920.0, 1080.0), &[LAPTOP], 20.0).unwrap();
        assert_eq!(near_top.y, 0.0, "the tops line up");
        let near_bottom =
            settle(Rect::new(1512.0, -110.0, 1920.0, 1080.0), &[LAPTOP], 20.0).unwrap();
        assert_eq!(near_bottom.max_y(), LAPTOP.max_y(), "the bottoms line up");
        let near_middle =
            settle(Rect::new(-200.0, -1090.0, 1920.0, 1080.0), &[LAPTOP], 20.0).unwrap();
        assert_eq!(
            near_middle.x,
            (LAPTOP.w - 1920.0) / 2.0,
            "the middles line up"
        );
        let free = settle(Rect::new(1512.0, -300.0, 1920.0, 1080.0), &[LAPTOP], 20.0).unwrap();
        assert_eq!(
            free.y, -300.0,
            "far from every line, it stays where it was dropped"
        );
    }

    #[test]
    fn a_display_never_lands_on_another() {
        // Between the laptop and the monitor, the right of the laptop is taken: it goes elsewhere.
        let others = [LAPTOP, MONITOR];
        let placed = settle(Rect::new(1500.0, 100.0, 1920.0, 1080.0), &others, 20.0).unwrap();
        for other in &others {
            assert!(!overlaps(&placed, other), "{placed:?} covers {other:?}");
        }
        assert!(
            settle(LAPTOP, &[], 20.0).is_none(),
            "alone, there is nothing to settle against"
        );
    }

    #[test]
    fn the_main_display_stays_at_the_origin() {
        let places = [
            Place {
                id: 1,
                rect: LAPTOP,
            },
            Place {
                id: 2,
                rect: MONITOR,
            },
        ];
        // The monitor moved to the left of the laptop.
        let moved = origins_after_move(&places, 1, 2, Rect::new(-2560.0, -458.0, 2560.0, 1440.0));
        assert_eq!(moved, [(1, 0, 0), (2, -2560, -458)]);
        // The laptop, which is the main display, moved under the monitor: the monitor moves instead.
        let under = origins_after_move(&places, 1, 1, Rect::new(1512.0, 982.0, 1512.0, 982.0));
        assert_eq!(under, [(1, 0, 0), (2, 0, -1440)]);
        // The monitor becomes the main display.
        assert_eq!(
            origins_with_main(&places, 2),
            Some(vec![(1, -1512, 458), (2, 0, 0)])
        );
        assert_eq!(origins_with_main(&places, 9), None);
    }

    #[test]
    fn the_desktop_fits_the_canvas_and_comes_back() {
        let fit = Fit::new(&[LAPTOP, MONITOR], 300.0, 160.0, 10.0);
        let laptop = fit.to_canvas(LAPTOP);
        let monitor = fit.to_canvas(MONITOR);
        for r in [laptop, monitor] {
            assert!(
                r.x >= 9.99 && r.y >= 9.99 && r.max_x() <= 290.01 && r.max_y() <= 150.01,
                "{r:?}"
            );
        }
        assert!(
            monitor.max_y() > laptop.max_y(),
            "the monitor is higher, so it is drawn higher"
        );
        let back = fit.to_desktop(monitor);
        assert!((back.x - MONITOR.x).abs() < 1e-6 && (back.y - MONITOR.y).abs() < 1e-6);
        assert!((back.w - MONITOR.w).abs() < 1e-6);
        let empty = Fit::new(&[], 300.0, 160.0, 10.0);
        assert!(empty.scale.is_finite() && empty.scale > 0.0);
    }
}
