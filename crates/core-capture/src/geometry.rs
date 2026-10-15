//! Points, rectangles and displays. All of it is in AppKit's global space (points, the origin at the bottom
//! left of the primary display) except `PixelRect`, which is in the pixels of one image with the origin at
//! its top left. `flip_y` is the only place where the two vertical directions meet.

#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Point {
    pub x: f64,
    pub y: f64,
}

impl Point {
    pub const fn new(x: f64, y: f64) -> Self {
        Self { x, y }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Rect {
    pub x: f64,
    pub y: f64,
    pub w: f64,
    pub h: f64,
}

impl Rect {
    pub const fn new(x: f64, y: f64, w: f64, h: f64) -> Self {
        Self { x, y, w, h }
    }

    /// The rectangle two opposite corners span, whichever way the drag went.
    pub fn from_corners(a: Point, b: Point) -> Self {
        Self {
            x: a.x.min(b.x),
            y: a.y.min(b.y),
            w: (a.x - b.x).abs(),
            h: (a.y - b.y).abs(),
        }
    }

    pub fn max_x(&self) -> f64 {
        self.x + self.w
    }

    pub fn max_y(&self) -> f64 {
        self.y + self.h
    }

    pub fn contains(&self, p: Point) -> bool {
        p.x >= self.x && p.x < self.max_x() && p.y >= self.y && p.y < self.max_y()
    }

    /// The part both share, or `None` when it has no area.
    pub fn intersection(&self, other: &Rect) -> Option<Rect> {
        let x = self.x.max(other.x);
        let y = self.y.max(other.y);
        let w = self.max_x().min(other.max_x()) - x;
        let h = self.max_y().min(other.max_y()) - y;
        (w > 0.0 && h > 0.0).then_some(Rect { x, y, w, h })
    }

    pub fn translated(&self, dx: f64, dy: f64) -> Rect {
        Rect {
            x: self.x + dx,
            y: self.y + dy,
            ..*self
        }
    }

    /// Moves the rectangle, without resizing it, as little as it takes to lie inside `bounds`.
    pub fn moved_into(&self, bounds: &Rect) -> Rect {
        let x = self.x.min(bounds.max_x() - self.w).max(bounds.x);
        let y = self.y.min(bounds.max_y() - self.h).max(bounds.y);
        Rect { x, y, ..*self }
    }

    fn distance_to(&self, p: Point) -> f64 {
        let dx = (self.x - p.x).max(0.0).max(p.x - self.max_x());
        let dy = (self.y - p.y).max(0.0).max(p.y - self.max_y());
        dx.hypot(dy)
    }
}

/// The point itself, or the nearest one on the edge of `bounds`.
pub fn clamp_point(p: Point, bounds: &Rect) -> Point {
    Point {
        x: p.x.clamp(bounds.x, bounds.max_x()),
        y: p.y.clamp(bounds.y, bounds.max_y()),
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Display {
    /// The system's identifier (CGDirectDisplayID).
    pub id: u32,
    pub frame: Rect,
    /// Pixels per point.
    pub scale: f64,
}

/// The display that holds the point; a point on the far edge, or in a gap between displays, belongs to the
/// nearest one.
pub fn display_index_at(displays: &[Display], p: Point) -> Option<usize> {
    if let Some(i) = displays.iter().position(|d| d.frame.contains(p)) {
        return Some(i);
    }
    displays
        .iter()
        .enumerate()
        .min_by(|(_, a), (_, b)| a.frame.distance_to(p).total_cmp(&b.frame.distance_to(p)))
        .map(|(i, _)| i)
}

/// Height of the primary display, the one whose frame starts at the origin; CoreGraphics measures y down
/// from its top edge and AppKit up from its bottom edge.
pub fn primary_height(displays: &[Display]) -> f64 {
    displays
        .iter()
        .find(|d| d.frame.x == 0.0 && d.frame.y == 0.0)
        .or(displays.first())
        .map_or(0.0, |d| d.frame.h)
}

/// Turns a rectangle between AppKit's y (up from the bottom of the primary display) and CoreGraphics' y
/// (down from its top). Applying it twice gives the rectangle back.
pub fn flip_y(rect: Rect, primary_height: f64) -> Rect {
    Rect {
        y: primary_height - rect.max_y(),
        ..rect
    }
}

/// A rectangle of an image, in its pixels, from its top left corner.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PixelRect {
    pub x: u32,
    pub y: u32,
    pub w: u32,
    pub h: u32,
}

/// The pixels of `rect` in an image of `display` that is `image` pixels wide and high. The scale comes from
/// the image itself, so a display that captured at another size than its backing scale still crops the right
/// part. `None` when nothing of the rectangle is on that display.
pub fn pixel_rect(rect: Rect, display: &Display, image: (u32, u32)) -> Option<PixelRect> {
    let inside = rect.intersection(&display.frame)?;
    let sx = f64::from(image.0) / display.frame.w;
    let sy = f64::from(image.1) / display.frame.h;
    let left = (inside.x - display.frame.x) * sx;
    let right = (inside.max_x() - display.frame.x) * sx;
    // Pixel rows count from the top of the display.
    let top = (display.frame.max_y() - inside.max_y()) * sy;
    let bottom = (display.frame.max_y() - inside.y) * sy;
    let edge = |v: f64, limit: u32| (v.round().max(0.0) as u32).min(limit);
    let (x0, x1) = (edge(left, image.0), edge(right, image.0));
    let (y0, y1) = (edge(top, image.1), edge(bottom, image.1));
    (x1 > x0 && y1 > y0).then_some(PixelRect {
        x: x0,
        y: y0,
        w: x1 - x0,
        h: y1 - y0,
    })
}

/// A window on screen: its identifier, where it is, and what to call it.
#[derive(Debug, Clone, PartialEq)]
pub struct WindowBox {
    pub id: u32,
    pub frame: Rect,
    pub owner: String,
    pub title: String,
}

/// The window under the point, for windows listed from the front to the back.
pub fn window_at(windows: &[WindowBox], p: Point) -> Option<usize> {
    windows.iter().position(|w| w.frame.contains(p))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A laptop panel (2x) on the left of an external 1x display that sits higher, and a third display
    /// to the left of the origin: the layouts that break naive conversions.
    fn desk() -> Vec<Display> {
        vec![
            Display {
                id: 1,
                frame: Rect::new(0.0, 0.0, 1440.0, 900.0),
                scale: 2.0,
            },
            Display {
                id: 2,
                frame: Rect::new(1440.0, 300.0, 1920.0, 1080.0),
                scale: 1.0,
            },
            Display {
                id: 3,
                frame: Rect::new(-1280.0, -100.0, 1280.0, 800.0),
                scale: 2.0,
            },
        ]
    }

    #[test]
    fn a_drag_in_any_direction_is_the_same_rectangle() {
        let want = Rect::new(10.0, 20.0, 30.0, 40.0);
        let (a, b) = (Point::new(10.0, 20.0), Point::new(40.0, 60.0));
        let (c, d) = (Point::new(40.0, 20.0), Point::new(10.0, 60.0));
        assert_eq!(Rect::from_corners(a, b), want);
        assert_eq!(Rect::from_corners(b, a), want);
        assert_eq!(Rect::from_corners(c, d), want);
        assert_eq!(Rect::from_corners(d, c), want);
    }

    #[test]
    fn a_point_is_clamped_to_the_edge_of_its_display() {
        let frame = Rect::new(-1280.0, -100.0, 1280.0, 800.0);
        assert_eq!(
            clamp_point(Point::new(-2000.0, 50.0), &frame),
            Point::new(-1280.0, 50.0)
        );
        assert_eq!(
            clamp_point(Point::new(40.0, 900.0), &frame),
            Point::new(0.0, 700.0)
        );
        let inside = Point::new(-5.0, 5.0);
        assert_eq!(clamp_point(inside, &frame), inside);
    }

    #[test]
    fn a_moved_rectangle_keeps_its_size_and_stays_inside() {
        let bounds = Rect::new(0.0, 0.0, 100.0, 100.0);
        let r = Rect::new(80.0, -10.0, 30.0, 20.0);
        assert_eq!(r.moved_into(&bounds), Rect::new(70.0, 0.0, 30.0, 20.0));
        let fine = Rect::new(10.0, 10.0, 30.0, 20.0);
        assert_eq!(fine.moved_into(&bounds), fine);
    }

    #[test]
    fn the_display_of_a_point_includes_negative_origins_gaps_and_far_edges() {
        let d = desk();
        assert_eq!(display_index_at(&d, Point::new(100.0, 100.0)), Some(0));
        assert_eq!(display_index_at(&d, Point::new(2000.0, 500.0)), Some(1));
        assert_eq!(display_index_at(&d, Point::new(-10.0, 0.0)), Some(2));
        // The top edge of the primary display is not inside it, but it is the nearest.
        assert_eq!(display_index_at(&d, Point::new(700.0, 900.0)), Some(0));
        // Below the external display, in no display at all: the nearest one wins.
        assert_eq!(display_index_at(&d, Point::new(2000.0, 100.0)), Some(1));
        assert_eq!(display_index_at(&[], Point::default()), None);
    }

    #[test]
    fn flipping_twice_is_the_identity_and_uses_the_primary_height() {
        let d = desk();
        let height = primary_height(&d);
        assert_eq!(height, 900.0);
        let ext = d[1].frame;
        let cg = flip_y(ext, height);
        // The external display starts 300 above the primary's bottom: 900 - 1380 = -480 from its top.
        assert_eq!(cg, Rect::new(1440.0, -480.0, 1920.0, 1080.0));
        assert_eq!(flip_y(cg, height), ext);
        assert_eq!(
            flip_y(Rect::new(0.0, 0.0, 10.0, 10.0), height),
            Rect::new(0.0, 890.0, 10.0, 10.0)
        );
    }

    #[test]
    fn the_primary_display_is_found_by_its_origin_not_its_position_in_the_list() {
        let mut d = desk();
        d.reverse();
        assert_eq!(primary_height(&d), 900.0);
    }

    #[test]
    fn points_become_pixels_by_the_scale_of_each_display() {
        let d = desk();
        // 100x50 points at the top left of the 2x primary display: its top is at y = 900.
        let top_left = Rect::new(0.0, 850.0, 100.0, 50.0);
        assert_eq!(
            pixel_rect(top_left, &d[0], (2880, 1800)),
            Some(PixelRect {
                x: 0,
                y: 0,
                w: 200,
                h: 100
            })
        );
        // The same size at the bottom left of the 1x external display.
        let bottom_left = Rect::new(1440.0, 300.0, 100.0, 50.0);
        assert_eq!(
            pixel_rect(bottom_left, &d[1], (1920, 1080)),
            Some(PixelRect {
                x: 0,
                y: 1030,
                w: 100,
                h: 50
            })
        );
        // A display to the left of the origin with a negative y: -100 is its bottom edge.
        let at_left = Rect::new(-1280.0, -100.0, 64.0, 32.0);
        assert_eq!(
            pixel_rect(at_left, &d[2], (2560, 1600)),
            Some(PixelRect {
                x: 0,
                y: 1536,
                w: 128,
                h: 64
            })
        );
    }

    #[test]
    fn the_pixel_scale_follows_the_image_and_not_the_nominal_scale() {
        let d = desk();
        // The image came back at 1.5 pixels per point although the display says 2.
        let r = Rect::new(0.0, 450.0, 200.0, 100.0);
        let p = pixel_rect(r, &d[0], (2160, 1350)).unwrap();
        assert_eq!((p.w, p.h), (300, 150));
    }

    #[test]
    fn a_rectangle_outside_the_display_is_cut_or_refused() {
        let d = desk();
        let overhang = Rect::new(1400.0, 800.0, 100.0, 200.0);
        let p = pixel_rect(overhang, &d[0], (2880, 1800)).unwrap();
        // Only the 40 points that are on this display remain, and the part above it is gone.
        assert_eq!((p.x, p.w), (2800, 80));
        assert_eq!((p.y, p.h), (0, 200));
        assert_eq!(
            pixel_rect(Rect::new(5000.0, 0.0, 10.0, 10.0), &d[0], (2880, 1800)),
            None
        );
    }

    #[test]
    fn fractional_edges_round_to_whole_pixels_and_never_vanish() {
        let d = desk();
        let r = Rect::new(0.25, 899.0, 0.9, 0.6);
        let p = pixel_rect(r, &d[0], (2880, 1800)).unwrap();
        assert!(p.w >= 1 && p.h >= 1);
        let tiny = Rect::new(1440.0, 300.0, 0.1, 0.1);
        assert_eq!(pixel_rect(tiny, &d[1], (1920, 1080)), None);
    }

    #[test]
    fn the_front_window_under_the_point_wins() {
        let win = |id, x, y, w, h| WindowBox {
            id,
            frame: Rect::new(x, y, w, h),
            owner: String::new(),
            title: String::new(),
        };
        let list = vec![
            win(1, 0.0, 0.0, 100.0, 100.0),
            win(2, 50.0, 50.0, 200.0, 200.0),
        ];
        assert_eq!(window_at(&list, Point::new(60.0, 60.0)), Some(0));
        assert_eq!(window_at(&list, Point::new(150.0, 150.0)), Some(1));
        assert_eq!(window_at(&list, Point::new(500.0, 500.0)), None);
    }
}
