//! Points, rectangles and the few distance functions hit testing needs. All in points, y growing downwards.

#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Point {
    pub x: f64,
    pub y: f64,
}

impl Point {
    pub const fn new(x: f64, y: f64) -> Self {
        Self { x, y }
    }

    pub fn distance(self, other: Point) -> f64 {
        (self.x - other.x).hypot(self.y - other.y)
    }

    pub fn offset(self, dx: f64, dy: f64) -> Point {
        Point::new(self.x + dx, self.y + dy)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Size {
    pub w: f64,
    pub h: f64,
}

impl Size {
    pub const fn new(w: f64, h: f64) -> Self {
        Self { w, h }
    }
}

/// A rectangle with a non-negative width and height.
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

    /// The rectangle with these two opposite corners, whichever way round they are.
    pub fn from_points(a: Point, b: Point) -> Self {
        Self::new(
            a.x.min(b.x),
            a.y.min(b.y),
            (a.x - b.x).abs(),
            (a.y - b.y).abs(),
        )
    }

    pub fn right(&self) -> f64 {
        self.x + self.w
    }

    pub fn bottom(&self) -> f64 {
        self.y + self.h
    }

    pub fn origin(&self) -> Point {
        Point::new(self.x, self.y)
    }

    pub fn size(&self) -> Size {
        Size::new(self.w, self.h)
    }

    pub fn centre(&self) -> Point {
        Point::new(self.x + self.w / 2.0, self.y + self.h / 2.0)
    }

    pub fn is_empty(&self) -> bool {
        self.w <= 0.0 || self.h <= 0.0
    }

    pub fn contains(&self, p: Point) -> bool {
        p.x >= self.x && p.x <= self.right() && p.y >= self.y && p.y <= self.bottom()
    }

    /// Whether `other` lies entirely inside this rectangle.
    pub fn contains_rect(&self, other: &Rect) -> bool {
        other.x >= self.x
            && other.y >= self.y
            && other.right() <= self.right()
            && other.bottom() <= self.bottom()
    }

    pub fn union(&self, other: &Rect) -> Rect {
        let (x, y) = (self.x.min(other.x), self.y.min(other.y));
        Rect::new(
            x,
            y,
            self.right().max(other.right()) - x,
            self.bottom().max(other.bottom()) - y,
        )
    }

    pub fn intersection(&self, other: &Rect) -> Option<Rect> {
        let (x, y) = (self.x.max(other.x), self.y.max(other.y));
        let (r, b) = (
            self.right().min(other.right()),
            self.bottom().min(other.bottom()),
        );
        (r > x && b > y).then(|| Rect::new(x, y, r - x, b - y))
    }

    pub fn intersects(&self, other: &Rect) -> bool {
        self.intersection(other).is_some()
    }

    /// Grows by `d` on every side; a negative `d` shrinks, down to nothing.
    pub fn inflate(&self, d: f64) -> Rect {
        let (w, h) = ((self.w + 2.0 * d).max(0.0), (self.h + 2.0 * d).max(0.0));
        Rect::new(
            self.x + (self.w - w) / 2.0,
            self.y + (self.h - h) / 2.0,
            w,
            h,
        )
    }

    pub fn translated(&self, dx: f64, dy: f64) -> Rect {
        Rect::new(self.x + dx, self.y + dy, self.w, self.h)
    }
}

/// The smallest rectangle holding every point, or `None` for no points.
pub fn bounds_of(points: &[Point]) -> Option<Rect> {
    let first = points.first()?;
    let mut r = Rect::new(first.x, first.y, 0.0, 0.0);
    for p in &points[1..] {
        r = r.union(&Rect::new(p.x, p.y, 0.0, 0.0));
    }
    Some(r)
}

/// Distance from `p` to the segment `a`-`b`.
pub fn distance_to_segment(p: Point, a: Point, b: Point) -> f64 {
    let (dx, dy) = (b.x - a.x, b.y - a.y);
    let len2 = dx * dx + dy * dy;
    if len2 <= f64::EPSILON {
        return p.distance(a);
    }
    let t = (((p.x - a.x) * dx + (p.y - a.y) * dy) / len2).clamp(0.0, 1.0);
    p.distance(Point::new(a.x + t * dx, a.y + t * dy))
}

/// Whether `p` is inside the triangle `a`, `b`, `c`, whichever way round it is wound.
pub fn in_triangle(p: Point, a: Point, b: Point, c: Point) -> bool {
    let side = |u: Point, v: Point| (v.x - u.x) * (p.y - u.y) - (v.y - u.y) * (p.x - u.x);
    let (d1, d2, d3) = (side(a, b), side(b, c), side(c, a));
    let negative = d1 < 0.0 || d2 < 0.0 || d3 < 0.0;
    let positive = d1 > 0.0 || d2 > 0.0 || d3 > 0.0;
    !(negative && positive)
}

/// The end of a drag from `from` to `to` turned to the nearest multiple of 45 degrees, at the same length.
pub fn snap_angle(from: Point, to: Point) -> Point {
    let (dx, dy) = (to.x - from.x, to.y - from.y);
    let len = dx.hypot(dy);
    if len == 0.0 {
        return to;
    }
    let step = std::f64::consts::FRAC_PI_4;
    let angle = (dy.atan2(dx) / step).round() * step;
    Point::new(from.x + len * angle.cos(), from.y + len * angle.sin())
}

/// The corner of a square that starts at `from` and is dragged towards `to`: the longer side wins.
pub fn square_corner(from: Point, to: Point) -> Point {
    let (dx, dy) = (to.x - from.x, to.y - from.y);
    let side = dx.abs().max(dy.abs());
    Point::new(from.x + side * dx.signum(), from.y + side * dy.signum())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_rectangle_from_two_corners_does_not_care_about_their_order() {
        let a = Point::new(10.0, 40.0);
        let b = Point::new(4.0, 6.0);
        assert_eq!(Rect::from_points(a, b), Rect::new(4.0, 6.0, 6.0, 34.0));
        assert_eq!(Rect::from_points(a, b), Rect::from_points(b, a));
    }

    #[test]
    fn union_and_intersection() {
        let a = Rect::new(0.0, 0.0, 10.0, 10.0);
        let b = Rect::new(5.0, -5.0, 10.0, 10.0);
        assert_eq!(a.union(&b), Rect::new(0.0, -5.0, 15.0, 15.0));
        assert_eq!(a.intersection(&b), Some(Rect::new(5.0, 0.0, 5.0, 5.0)));
        assert_eq!(
            a.intersection(&Rect::new(10.0, 0.0, 5.0, 5.0)),
            None,
            "touching is not overlapping"
        );
        assert!(a.union(&b).contains_rect(&a) && a.union(&b).contains_rect(&b));
    }

    #[test]
    fn inflating_by_a_negative_amount_stops_at_nothing() {
        let r = Rect::new(0.0, 0.0, 10.0, 4.0).inflate(-3.0);
        assert_eq!((r.w, r.h), (4.0, 0.0));
        assert!(r.is_empty());
        let grown = Rect::new(2.0, 2.0, 4.0, 4.0).inflate(1.0);
        assert_eq!(grown, Rect::new(1.0, 1.0, 6.0, 6.0));
    }

    #[test]
    fn distance_to_a_segment_clamps_to_its_ends() {
        let (a, b) = (Point::new(0.0, 0.0), Point::new(10.0, 0.0));
        assert_eq!(distance_to_segment(Point::new(5.0, 3.0), a, b), 3.0);
        assert_eq!(distance_to_segment(Point::new(-4.0, 3.0), a, b), 5.0);
        assert_eq!(distance_to_segment(Point::new(13.0, 4.0), a, b), 5.0);
        assert_eq!(
            distance_to_segment(Point::new(3.0, 4.0), a, a),
            5.0,
            "a point is a segment too"
        );
    }

    #[test]
    fn triangle_membership_does_not_depend_on_the_winding() {
        let (a, b, c) = (
            Point::new(0.0, 0.0),
            Point::new(10.0, 0.0),
            Point::new(0.0, 10.0),
        );
        let inside = Point::new(2.0, 2.0);
        let outside = Point::new(8.0, 8.0);
        assert!(in_triangle(inside, a, b, c) && in_triangle(inside, a, c, b));
        assert!(!in_triangle(outside, a, b, c) && !in_triangle(outside, a, c, b));
    }

    #[test]
    fn dragging_with_shift_snaps_to_eighths_of_a_turn() {
        let o = Point::new(0.0, 0.0);
        let p = snap_angle(o, Point::new(100.0, 8.0));
        assert!((p.y).abs() < 1e-9 && (p.x - (100.0f64).hypot(8.0)).abs() < 1e-9);
        let d = snap_angle(o, Point::new(50.0, 60.0));
        assert!(
            (d.x - d.y).abs() < 1e-9,
            "close to the diagonal goes onto it"
        );
        let s = square_corner(o, Point::new(-30.0, 10.0));
        assert_eq!((s.x, s.y), (-30.0, 30.0));
    }

    #[test]
    fn bounds_of_points() {
        assert_eq!(bounds_of(&[]), None);
        let pts = [
            Point::new(3.0, 4.0),
            Point::new(-1.0, 9.0),
            Point::new(7.0, 0.0),
        ];
        assert_eq!(bounds_of(&pts), Some(Rect::new(-1.0, 0.0, 8.0, 9.0)));
    }
}
