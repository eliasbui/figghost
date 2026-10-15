//! Selection geometry. `Rect` is in points with the origin at the bottom-left, as AppKit measures a
//! screen; `PixelRect` is in image pixels with the origin at the top-left, as images are stored.

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Point {
    pub x: f64,
    pub y: f64,
}

impl Point {
    pub const fn new(x: f64, y: f64) -> Self {
        Self { x, y }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Rect {
    pub x: f64,
    pub y: f64,
    pub w: f64,
    pub h: f64,
}

/// A drag shorter than this, in points, is a click.
const CLICK_SLOP: f64 = 3.0;

impl Rect {
    pub const fn new(x: f64, y: f64, w: f64, h: f64) -> Self {
        Self { x, y, w, h }
    }

    /// The rectangle a drag from `a` to `b` covers, whichever way it went.
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

    /// The part of this rectangle inside `bounds`; zero-sized when they do not meet.
    pub fn clamped_to(&self, bounds: &Rect) -> Self {
        let x = self.x.max(bounds.x);
        let y = self.y.max(bounds.y);
        let w = (self.max_x().min(bounds.max_x()) - x).max(0.0);
        let h = (self.max_y().min(bounds.max_y()) - y).max(0.0);
        Self { x, y, w, h }
    }

    /// Too small to be a selection: the pointer barely moved.
    pub fn is_click(&self) -> bool {
        self.w < CLICK_SLOP && self.h < CLICK_SLOP
    }

    /// The same area moved by an offset.
    pub fn offset(&self, dx: f64, dy: f64) -> Self {
        Self {
            x: self.x + dx,
            y: self.y + dy,
            ..*self
        }
    }

    /// The pixels of an image of `image_w` x `image_h` that show this rectangle, when the image covers
    /// `bounds` (both in the same point coordinates). Edges are rounded outwards, so a selection never
    /// loses a partly covered pixel. `None` when nothing is left inside the image.
    pub fn to_pixels(&self, bounds: &Rect, image_w: usize, image_h: usize) -> Option<PixelRect> {
        if bounds.w <= 0.0 || bounds.h <= 0.0 {
            return None;
        }
        let r = self.clamped_to(bounds);
        let (sx, sy) = (image_w as f64 / bounds.w, image_h as f64 / bounds.h);
        let left = ((r.x - bounds.x) * sx).floor().max(0.0);
        let right = ((r.max_x() - bounds.x) * sx).ceil().min(image_w as f64);
        // Image rows count down from the top of `bounds`.
        let top = ((bounds.max_y() - r.max_y()) * sy).floor().max(0.0);
        let bottom = ((bounds.max_y() - r.y) * sy).ceil().min(image_h as f64);
        (right > left && bottom > top).then_some(PixelRect {
            x: left as usize,
            y: top as usize,
            w: (right - left) as usize,
            h: (bottom - top) as usize,
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PixelRect {
    pub x: usize,
    pub y: usize,
    pub w: usize,
    pub h: usize,
}

/// The front-most window under `point`; `windows` run from front to back.
pub fn pick_window<T: Copy>(point: Point, windows: &[(T, Rect)]) -> Option<T> {
    windows
        .iter()
        .find(|(_, frame)| frame.contains(point))
        .map(|(id, _)| *id)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_drag_in_any_direction_gives_the_same_rectangle() {
        let (a, b) = (Point::new(10.0, 50.0), Point::new(110.0, 20.0));
        let expected = Rect::new(10.0, 20.0, 100.0, 30.0);
        assert_eq!(Rect::from_corners(a, b), expected);
        assert_eq!(Rect::from_corners(b, a), expected);
        assert!(Rect::from_corners(a, Point::new(11.0, 51.0)).is_click());
        assert!(!expected.is_click());
    }

    #[test]
    fn selections_are_clamped_to_the_screen() {
        let screen = Rect::new(0.0, 0.0, 100.0, 100.0);
        let past = Rect::new(80.0, -10.0, 50.0, 30.0);
        assert_eq!(past.clamped_to(&screen), Rect::new(80.0, 0.0, 20.0, 20.0));
        let outside = Rect::new(200.0, 0.0, 10.0, 10.0);
        assert_eq!(outside.clamped_to(&screen).w, 0.0);
        assert_eq!(outside.to_pixels(&screen, 100, 100), None);
    }

    #[test]
    fn pixels_flip_the_vertical_axis_and_follow_the_scale() {
        // A 2x screen of 100 x 50 points, as a 200 x 100 image.
        let screen = Rect::new(0.0, 0.0, 100.0, 50.0);
        // 10 points from the left, touching the top edge.
        let top_left = Rect::new(10.0, 40.0, 20.0, 10.0);
        assert_eq!(
            top_left.to_pixels(&screen, 200, 100),
            Some(PixelRect {
                x: 20,
                y: 0,
                w: 40,
                h: 20
            })
        );
        let bottom = Rect::new(0.0, 0.0, 100.0, 5.0);
        assert_eq!(
            bottom.to_pixels(&screen, 200, 100),
            Some(PixelRect {
                x: 0,
                y: 90,
                w: 200,
                h: 10
            })
        );
    }

    #[test]
    fn a_second_screen_maps_from_its_own_origin() {
        let right_screen = Rect::new(1920.0, -200.0, 1440.0, 900.0);
        let sel = Rect::new(1920.0, 600.0, 100.0, 100.0);
        assert_eq!(
            sel.to_pixels(&right_screen, 2880, 1800),
            Some(PixelRect {
                x: 0,
                y: 0,
                w: 200,
                h: 200
            })
        );
    }

    #[test]
    fn partly_covered_pixels_are_kept() {
        let screen = Rect::new(0.0, 0.0, 10.0, 10.0);
        let sel = Rect::new(0.25, 0.25, 1.0, 1.0);
        let px = sel.to_pixels(&screen, 20, 20).unwrap();
        assert_eq!((px.x, px.w), (0, 3));
        assert_eq!((px.y, px.h), (17, 3));
    }

    #[test]
    fn the_front_window_wins_where_windows_overlap() {
        let windows = [
            (7u32, Rect::new(50.0, 50.0, 100.0, 100.0)),
            (3u32, Rect::new(0.0, 0.0, 400.0, 400.0)),
        ];
        assert_eq!(pick_window(Point::new(60.0, 60.0), &windows), Some(7));
        assert_eq!(pick_window(Point::new(10.0, 10.0), &windows), Some(3));
        assert_eq!(pick_window(Point::new(500.0, 10.0), &windows), None);
    }
}
