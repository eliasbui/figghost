//! What can be drawn: the kinds of object, their style, and the geometry that hit testing, handles and the
//! growth of the canvas share with the drawing code.

use crate::geom::{Point, Rect, bounds_of, distance_to_segment, in_triangle};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Colour {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub a: u8,
}

impl Colour {
    pub const fn rgb(r: u8, g: u8, b: u8) -> Self {
        Self { r, g, b, a: 255 }
    }

    pub const CLEAR: Colour = Colour {
        r: 0,
        g: 0,
        b: 0,
        a: 0,
    };
}

/// The swatches of the toolbar: red first, which is what a new drawing uses.
pub const PALETTE: [Colour; 8] = [
    Colour::rgb(255, 59, 48),
    Colour::rgb(255, 149, 0),
    Colour::rgb(255, 214, 10),
    Colour::rgb(52, 199, 89),
    Colour::rgb(10, 132, 255),
    Colour::rgb(175, 82, 222),
    Colour::rgb(255, 255, 255),
    Colour::rgb(28, 28, 30),
];

/// The thickness choices, in points. For text they are font sizes (see [`Style::font_size`]).
pub const WIDTHS: [f64; 5] = [2.0, 4.0, 6.0, 9.0, 14.0];

/// How much of the colour a highlighter lets through.
pub const HIGHLIGHT_ALPHA: f64 = 0.4;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Style {
    pub colour: Colour,
    pub width: f64,
    /// Rectangles and ellipses only.
    pub fill: bool,
}

impl Default for Style {
    fn default() -> Self {
        Self {
            colour: PALETTE[0],
            width: WIDTHS[1],
            fill: false,
        }
    }
}

impl Style {
    /// The size of text drawn in this style.
    pub fn font_size(&self) -> f64 {
        8.0 + 3.0 * self.width
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum Kind {
    Arrow {
        from: Point,
        to: Point,
    },
    Line {
        from: Point,
        to: Point,
    },
    Rectangle(Rect),
    Ellipse(Rect),
    /// A box of text. The height is what the text needs, set by the view that lays it out.
    Text {
        rect: Rect,
        text: String,
    },
    Pen(Vec<Point>),
    Highlight(Vec<Point>),
    /// A region that is redacted when the image is drawn: its pixels are replaced by blocks.
    Blur(Rect),
}

#[derive(Debug, Clone, PartialEq)]
pub struct Object {
    pub id: u64,
    pub kind: Kind,
    pub style: Style,
}

/// A grip on a selected object.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Handle {
    Start,
    End,
    NorthWest,
    North,
    NorthEast,
    East,
    SouthEast,
    South,
    SouthWest,
    West,
}

/// The widest a box of text may be made narrower than.
pub const MIN_TEXT_WIDTH: f64 = 24.0;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ArrowHead {
    pub tip: Point,
    pub left: Point,
    pub right: Point,
    /// Where the shaft stops, inside the head, so that the two join without a notch.
    pub shaft_end: Point,
}

/// The head of an arrow from `from` to `to`, or `None` for an arrow of no length.
pub fn arrow_head(from: Point, to: Point, width: f64) -> Option<ArrowHead> {
    let len = from.distance(to);
    if len < 1e-6 {
        return None;
    }
    let (ux, uy) = ((to.x - from.x) / len, (to.y - from.y) / len);
    let head = (4.0 * width + 8.0).min(len);
    let (bx, by) = (to.x - ux * head, to.y - uy * head);
    let half = head * 0.5;
    Some(ArrowHead {
        tip: to,
        left: Point::new(bx - uy * half, by + ux * half),
        right: Point::new(bx + uy * half, by - ux * half),
        shaft_end: Point::new(to.x - ux * head * 0.75, to.y - uy * head * 0.75),
    })
}

impl Object {
    /// The width a stroke is drawn at; a highlighter is a wide marker.
    pub fn stroke_width(&self) -> f64 {
        match self.kind {
            Kind::Highlight(_) => self.style.width * 4.0,
            Kind::Blur(_) | Kind::Text { .. } => 0.0,
            _ => self.style.width,
        }
    }

    /// The geometry itself, without the thickness of its stroke.
    pub fn frame(&self) -> Rect {
        match &self.kind {
            Kind::Arrow { from, to } | Kind::Line { from, to } => Rect::from_points(*from, *to),
            Kind::Rectangle(r) | Kind::Ellipse(r) | Kind::Blur(r) => *r,
            Kind::Text { rect, .. } => *rect,
            Kind::Pen(points) | Kind::Highlight(points) => bounds_of(points).unwrap_or_default(),
        }
    }

    /// Everything the object paints, which is what the canvas has to grow to hold.
    pub fn bounds(&self) -> Rect {
        let half = self.stroke_width() / 2.0;
        let mut bounds = self.frame().inflate(half);
        if let Kind::Arrow { from, to } = self.kind
            && let Some(head) = arrow_head(from, to, self.style.width)
        {
            let corners = [head.left, head.right, head.tip];
            if let Some(extra) = bounds_of(&corners) {
                bounds = bounds.union(&extra);
            }
        }
        bounds
    }

    pub fn translated(&self, dx: f64, dy: f64) -> Object {
        let mv = |p: &Point| p.offset(dx, dy);
        let kind = match &self.kind {
            Kind::Arrow { from, to } => Kind::Arrow {
                from: mv(from),
                to: mv(to),
            },
            Kind::Line { from, to } => Kind::Line {
                from: mv(from),
                to: mv(to),
            },
            Kind::Rectangle(r) => Kind::Rectangle(r.translated(dx, dy)),
            Kind::Ellipse(r) => Kind::Ellipse(r.translated(dx, dy)),
            Kind::Blur(r) => Kind::Blur(r.translated(dx, dy)),
            Kind::Text { rect, text } => Kind::Text {
                rect: rect.translated(dx, dy),
                text: text.clone(),
            },
            Kind::Pen(points) => Kind::Pen(points.iter().map(mv).collect()),
            Kind::Highlight(points) => Kind::Highlight(points.iter().map(mv).collect()),
        };
        Object {
            id: self.id,
            kind,
            style: self.style,
        }
    }

    /// Whether a click at `p` lands on the object. `tolerance` is how far off a thin line may be hit.
    pub fn hit(&self, p: Point, tolerance: f64) -> bool {
        let reach = (self.stroke_width() / 2.0).max(1.0) + tolerance;
        match &self.kind {
            Kind::Arrow { from, to } => {
                distance_to_segment(p, *from, *to) <= reach
                    || arrow_head(*from, *to, self.style.width)
                        .is_some_and(|h| in_triangle(p, h.tip, h.left, h.right))
            }
            Kind::Line { from, to } => distance_to_segment(p, *from, *to) <= reach,
            Kind::Rectangle(r) => {
                let inner = r.inflate(-reach);
                r.inflate(reach).contains(p)
                    && (self.style.fill || inner.is_empty() || !inner.contains(p))
            }
            Kind::Ellipse(r) => {
                let inside = |grow: f64| {
                    let (a, b) = (r.w / 2.0 + grow, r.h / 2.0 + grow);
                    if a <= 0.0 || b <= 0.0 {
                        return false;
                    }
                    let c = r.centre();
                    ((p.x - c.x) / a).powi(2) + ((p.y - c.y) / b).powi(2) <= 1.0
                };
                inside(reach) && (self.style.fill || !inside(-reach))
            }
            Kind::Text { rect, .. } | Kind::Blur(rect) => rect.inflate(tolerance).contains(p),
            Kind::Pen(points) | Kind::Highlight(points) => match points.as_slice() {
                [] => false,
                [only] => only.distance(p) <= reach,
                _ => points
                    .windows(2)
                    .any(|w| distance_to_segment(p, w[0], w[1]) <= reach),
            },
        }
    }

    /// The grips of the object and where they are.
    pub fn handles(&self) -> Vec<(Handle, Point)> {
        match &self.kind {
            Kind::Arrow { from, to } | Kind::Line { from, to } => {
                vec![(Handle::Start, *from), (Handle::End, *to)]
            }
            Kind::Text { rect, .. } => vec![
                (Handle::West, Point::new(rect.x, rect.centre().y)),
                (Handle::East, Point::new(rect.right(), rect.centre().y)),
            ],
            _ => {
                // A stroke is grabbed by the outside of what it paints; a box by its edges.
                let r = match self.kind {
                    Kind::Pen(_) | Kind::Highlight(_) => self.bounds(),
                    _ => self.frame(),
                };
                let c = r.centre();
                vec![
                    (Handle::NorthWest, Point::new(r.x, r.y)),
                    (Handle::North, Point::new(c.x, r.y)),
                    (Handle::NorthEast, Point::new(r.right(), r.y)),
                    (Handle::East, Point::new(r.right(), c.y)),
                    (Handle::SouthEast, Point::new(r.right(), r.bottom())),
                    (Handle::South, Point::new(c.x, r.bottom())),
                    (Handle::SouthWest, Point::new(r.x, r.bottom())),
                    (Handle::West, Point::new(r.x, c.y)),
                ]
            }
        }
    }

    /// The handle under `p`, the nearest one when several are within `reach`.
    pub fn handle_at(&self, p: Point, reach: f64) -> Option<Handle> {
        self.handles()
            .into_iter()
            .map(|(h, at)| (h, at.distance(p)))
            .filter(|(_, d)| *d <= reach)
            .min_by(|a, b| a.1.total_cmp(&b.1))
            .map(|(h, _)| h)
    }

    /// Where the geometry's own edge goes when the grip of a stroke, which sits half a stroke outside it, is
    /// dragged to `to`.
    fn inside(&self, handle: Handle, to: Point) -> Point {
        let half = self.stroke_width() / 2.0;
        let dx = match handle {
            Handle::NorthWest | Handle::West | Handle::SouthWest => half,
            Handle::NorthEast | Handle::East | Handle::SouthEast => -half,
            _ => 0.0,
        };
        let dy = match handle {
            Handle::NorthWest | Handle::North | Handle::NorthEast => half,
            Handle::SouthWest | Handle::South | Handle::SouthEast => -half,
            _ => 0.0,
        };
        to.offset(dx, dy)
    }

    /// This object with `handle` dragged to `to`. `self` is the object as it was when the drag began.
    pub fn with_handle(&self, handle: Handle, to: Point) -> Object {
        let kind = match &self.kind {
            Kind::Arrow { from, to: end } => match handle {
                Handle::Start => Kind::Arrow { from: to, to: *end },
                _ => Kind::Arrow { from: *from, to },
            },
            Kind::Line { from, to: end } => match handle {
                Handle::Start => Kind::Line { from: to, to: *end },
                _ => Kind::Line { from: *from, to },
            },
            Kind::Rectangle(r) => Kind::Rectangle(drag_edges(*r, handle, to)),
            Kind::Ellipse(r) => Kind::Ellipse(drag_edges(*r, handle, to)),
            Kind::Blur(r) => Kind::Blur(drag_edges(*r, handle, to)),
            Kind::Text { rect, text } => {
                let mut moved = drag_edges(*rect, handle, to);
                if moved.w < MIN_TEXT_WIDTH {
                    moved.w = MIN_TEXT_WIDTH;
                    if matches!(handle, Handle::West) {
                        moved.x = rect.right() - MIN_TEXT_WIDTH;
                    } else {
                        moved.x = rect.x;
                    }
                }
                // The height is the text's own.
                moved.y = rect.y;
                moved.h = rect.h;
                Kind::Text {
                    rect: moved,
                    text: text.clone(),
                }
            }
            Kind::Pen(points) => Kind::Pen(rescale(
                points,
                self.frame(),
                handle,
                self.inside(handle, to),
            )),
            Kind::Highlight(points) => Kind::Highlight(rescale(
                points,
                self.frame(),
                handle,
                self.inside(handle, to),
            )),
        };
        Object {
            id: self.id,
            kind,
            style: self.style,
        }
    }
}

/// `rect` with the edges that `handle` holds moved to `to`; the rectangle stays valid when they cross.
fn drag_edges(rect: Rect, handle: Handle, to: Point) -> Rect {
    let (mut l, mut t, mut r, mut b) = (rect.x, rect.y, rect.right(), rect.bottom());
    match handle {
        Handle::NorthWest | Handle::West | Handle::SouthWest => l = to.x,
        Handle::NorthEast | Handle::East | Handle::SouthEast => r = to.x,
        _ => {}
    }
    match handle {
        Handle::NorthWest | Handle::North | Handle::NorthEast => t = to.y,
        Handle::SouthWest | Handle::South | Handle::SouthEast => b = to.y,
        _ => {}
    }
    Rect::from_points(Point::new(l, t), Point::new(r, b))
}

/// Points stretched from the rectangle they filled to the one `handle` dragged to `to` makes.
fn rescale(points: &[Point], from: Rect, handle: Handle, to: Point) -> Vec<Point> {
    let target = drag_edges(from, handle, to);
    let map = |v: f64, f0: f64, fs: f64, t0: f64, ts: f64| {
        if fs < 1e-9 {
            v - f0 + t0
        } else {
            t0 + (v - f0) / fs * ts
        }
    };
    points
        .iter()
        .map(|p| {
            Point::new(
                map(p.x, from.x, from.w, target.x, target.w),
                map(p.y, from.y, from.h, target.y, target.h),
            )
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn obj(kind: Kind) -> Object {
        Object {
            id: 1,
            kind,
            style: Style::default(),
        }
    }

    fn thin(kind: Kind) -> Object {
        Object {
            id: 1,
            kind,
            style: Style {
                width: 2.0,
                ..Style::default()
            },
        }
    }

    #[test]
    fn a_line_is_hit_within_its_thickness_plus_the_tolerance() {
        let line = thin(Kind::Line {
            from: Point::new(0.0, 0.0),
            to: Point::new(100.0, 0.0),
        });
        assert!(line.hit(Point::new(50.0, 0.0), 0.0));
        assert!(line.hit(Point::new(50.0, 1.0), 0.0), "half the width of 2");
        assert!(!line.hit(Point::new(50.0, 3.0), 0.0));
        assert!(
            line.hit(Point::new(50.0, 7.0), 6.0),
            "the tolerance reaches further"
        );
        assert!(!line.hit(Point::new(50.0, 8.0), 6.0));
        assert!(
            !line.hit(Point::new(120.0, 0.0), 6.0),
            "past the end is a miss"
        );
    }

    #[test]
    fn an_empty_rectangle_is_hit_on_its_outline_only_and_a_filled_one_everywhere() {
        let r = Rect::new(10.0, 10.0, 100.0, 60.0);
        let mut shape = thin(Kind::Rectangle(r));
        assert!(shape.hit(Point::new(10.0, 40.0), 0.0), "the left edge");
        assert!(shape.hit(Point::new(60.0, 70.0), 0.0), "the bottom edge");
        assert!(
            !shape.hit(Point::new(60.0, 40.0), 0.0),
            "the middle of an empty box"
        );
        shape.style.fill = true;
        assert!(shape.hit(Point::new(60.0, 40.0), 0.0));
        assert!(!shape.hit(Point::new(200.0, 40.0), 0.0));
    }

    #[test]
    fn a_tiny_rectangle_is_all_outline() {
        let shape = obj(Kind::Rectangle(Rect::new(0.0, 0.0, 3.0, 3.0)));
        assert!(
            shape.hit(Point::new(1.5, 1.5), 0.0),
            "the inside has no room to be a miss"
        );
    }

    #[test]
    fn an_ellipse_is_hit_on_its_curve_not_on_its_bounding_box() {
        let e = thin(Kind::Ellipse(Rect::new(0.0, 0.0, 100.0, 100.0)));
        assert!(e.hit(Point::new(0.0, 50.0), 0.0));
        assert!(e.hit(Point::new(50.0, 100.0), 0.0));
        assert!(
            !e.hit(Point::new(2.0, 2.0), 0.0),
            "the corner of the box is outside the curve"
        );
        assert!(
            !e.hit(Point::new(50.0, 50.0), 0.0),
            "and so is the middle of an empty one"
        );
        let mut filled = e.clone();
        filled.style.fill = true;
        assert!(filled.hit(Point::new(50.0, 50.0), 0.0));
        assert!(!filled.hit(Point::new(2.0, 2.0), 0.0));
    }

    #[test]
    fn text_and_blur_are_hit_anywhere_inside() {
        let t = obj(Kind::Text {
            rect: Rect::new(0.0, 0.0, 80.0, 20.0),
            text: "hello".into(),
        });
        assert!(t.hit(Point::new(40.0, 10.0), 0.0));
        assert!(!t.hit(Point::new(90.0, 10.0), 4.0));
        let b = obj(Kind::Blur(Rect::new(0.0, 0.0, 50.0, 50.0)));
        assert!(b.hit(Point::new(25.0, 25.0), 0.0));
    }

    #[test]
    fn a_pen_stroke_is_hit_along_its_path() {
        let pen = thin(Kind::Pen(vec![
            Point::new(0.0, 0.0),
            Point::new(50.0, 0.0),
            Point::new(50.0, 50.0),
        ]));
        assert!(pen.hit(Point::new(25.0, 0.5), 0.0));
        assert!(pen.hit(Point::new(50.0, 30.0), 0.0));
        assert!(
            !pen.hit(Point::new(25.0, 25.0), 2.0),
            "the inside of the bend"
        );
        let dot = thin(Kind::Pen(vec![Point::new(5.0, 5.0)]));
        assert!(dot.hit(Point::new(5.5, 5.0), 0.0));
        assert!(!obj(Kind::Pen(vec![])).hit(Point::new(0.0, 0.0), 10.0));
    }

    #[test]
    fn the_head_of_an_arrow_can_be_clicked_where_the_shaft_is_thin() {
        let arrow = thin(Kind::Arrow {
            from: Point::new(0.0, 0.0),
            to: Point::new(100.0, 0.0),
        });
        let head = arrow_head(Point::new(0.0, 0.0), Point::new(100.0, 0.0), 2.0).unwrap();
        let near_wing = Point::new(head.left.x + 1.0, head.left.y * 0.5);
        assert!(arrow.hit(near_wing, 0.0));
        assert!(!arrow.hit(Point::new(50.0, 12.0), 0.0), "beside the shaft");
    }

    #[test]
    fn an_arrow_head_never_longer_than_the_arrow() {
        let head = arrow_head(Point::new(0.0, 0.0), Point::new(6.0, 0.0), 14.0).unwrap();
        assert!((head.left.x - 0.0).abs() < 1e-9, "the base is at the start");
        assert!(arrow_head(Point::new(1.0, 1.0), Point::new(1.0, 1.0), 4.0).is_none());
    }

    #[test]
    fn bounds_include_the_stroke_and_the_head() {
        let line = thin(Kind::Line {
            from: Point::new(10.0, 10.0),
            to: Point::new(50.0, 10.0),
        });
        assert_eq!(line.bounds(), Rect::new(9.0, 9.0, 42.0, 2.0));
        let arrow = thin(Kind::Arrow {
            from: Point::new(10.0, 10.0),
            to: Point::new(50.0, 10.0),
        });
        let b = arrow.bounds();
        assert!(
            b.contains_rect(&line.bounds()) && b.h > 4.0,
            "the head sticks out sideways: {b:?}"
        );
        let marker = obj(Kind::Highlight(vec![
            Point::new(0.0, 0.0),
            Point::new(10.0, 0.0),
        ]));
        assert_eq!(
            marker.bounds(),
            Rect::new(-8.0, -8.0, 26.0, 16.0),
            "a marker is four times as wide"
        );
    }

    #[test]
    fn translating_moves_every_point_and_keeps_the_rest() {
        let shape = Object {
            id: 7,
            kind: Kind::Pen(vec![Point::new(0.0, 0.0), Point::new(4.0, 4.0)]),
            style: Style {
                width: 9.0,
                ..Style::default()
            },
        };
        let moved = shape.translated(3.0, -2.0);
        assert_eq!(
            moved.kind,
            Kind::Pen(vec![Point::new(3.0, -2.0), Point::new(7.0, 2.0)])
        );
        assert_eq!((moved.id, moved.style), (7, shape.style));
        assert_eq!(moved.translated(-3.0, 2.0), shape);
    }

    #[test]
    fn a_line_has_two_grips_and_a_box_has_eight() {
        let line = obj(Kind::Line {
            from: Point::new(0.0, 0.0),
            to: Point::new(10.0, 10.0),
        });
        assert_eq!(line.handles().len(), 2);
        let boxed = obj(Kind::Rectangle(Rect::new(0.0, 0.0, 10.0, 20.0)));
        let handles = boxed.handles();
        assert_eq!(handles.len(), 8);
        assert!(handles.contains(&(Handle::SouthEast, Point::new(10.0, 20.0))));
        assert!(handles.contains(&(Handle::North, Point::new(5.0, 0.0))));
        let text = obj(Kind::Text {
            rect: Rect::new(0.0, 0.0, 100.0, 30.0),
            text: String::new(),
        });
        assert_eq!(
            text.handles().len(),
            2,
            "only the width of a text box is the user's to set"
        );
    }

    #[test]
    fn the_nearest_grip_wins() {
        let boxed = obj(Kind::Rectangle(Rect::new(0.0, 0.0, 10.0, 10.0)));
        assert_eq!(
            boxed.handle_at(Point::new(1.0, 1.0), 6.0),
            Some(Handle::NorthWest)
        );
        assert_eq!(
            boxed.handle_at(Point::new(5.0, 1.0), 6.0),
            Some(Handle::North)
        );
        assert_eq!(boxed.handle_at(Point::new(5.0, 5.0), 3.0), None);
    }

    #[test]
    fn dragging_a_corner_past_the_opposite_one_flips_the_box_instead_of_inverting_it() {
        let boxed = obj(Kind::Rectangle(Rect::new(10.0, 10.0, 20.0, 20.0)));
        let flipped = boxed.with_handle(Handle::NorthWest, Point::new(50.0, 40.0));
        assert_eq!(
            flipped.kind,
            Kind::Rectangle(Rect::new(30.0, 30.0, 20.0, 10.0))
        );
        let wider = boxed.with_handle(Handle::East, Point::new(60.0, 999.0));
        assert_eq!(
            wider.kind,
            Kind::Rectangle(Rect::new(10.0, 10.0, 50.0, 20.0)),
            "an edge handle leaves the other axis alone"
        );
    }

    #[test]
    fn a_text_box_keeps_its_height_and_a_minimum_width() {
        let t = obj(Kind::Text {
            rect: Rect::new(10.0, 10.0, 100.0, 30.0),
            text: "x".into(),
        });
        let narrow = t.with_handle(Handle::East, Point::new(11.0, 500.0));
        let Kind::Text { rect, .. } = narrow.kind else {
            unreachable!()
        };
        assert_eq!(rect, Rect::new(10.0, 10.0, MIN_TEXT_WIDTH, 30.0));
        let from_left = t.with_handle(Handle::West, Point::new(100.0, 0.0));
        let Kind::Text { rect, .. } = from_left.kind else {
            unreachable!()
        };
        assert_eq!(
            rect.right(),
            110.0,
            "the right edge stays when the left one is dragged too close"
        );
        assert_eq!(rect.w, MIN_TEXT_WIDTH);
        let flipped = t.with_handle(Handle::West, Point::new(200.0, 0.0));
        let Kind::Text { rect, .. } = flipped.kind else {
            unreachable!()
        };
        assert_eq!(
            (rect.x, rect.w),
            (110.0, 90.0),
            "past the other edge it flips like any box"
        );
    }

    #[test]
    fn resizing_a_stroke_stretches_its_points_with_its_frame() {
        let pen = obj(Kind::Pen(vec![
            Point::new(0.0, 0.0),
            Point::new(10.0, 5.0),
            Point::new(20.0, 10.0),
        ]));
        // The grip is two points outside the stroke's own corner (the pen is four points thick).
        let big = pen.with_handle(Handle::SouthEast, Point::new(42.0, 32.0));
        assert_eq!(
            big.kind,
            Kind::Pen(vec![
                Point::new(0.0, 0.0),
                Point::new(20.0, 15.0),
                Point::new(40.0, 30.0)
            ])
        );
        let flat = obj(Kind::Highlight(vec![
            Point::new(0.0, 5.0),
            Point::new(10.0, 5.0),
        ]));
        let moved = flat.with_handle(Handle::South, Point::new(0.0, 25.0));
        let Kind::Highlight(points) = moved.kind else {
            unreachable!()
        };
        assert!(
            points.iter().all(|p| p.y.is_finite()),
            "a stroke with no height does not divide by zero"
        );
    }

    #[test]
    fn the_end_of_a_line_follows_its_handle() {
        let line = obj(Kind::Line {
            from: Point::new(0.0, 0.0),
            to: Point::new(10.0, 0.0),
        });
        let moved = line.with_handle(Handle::Start, Point::new(-5.0, 7.0));
        assert_eq!(
            moved.kind,
            Kind::Line {
                from: Point::new(-5.0, 7.0),
                to: Point::new(10.0, 0.0)
            }
        );
    }

    #[test]
    fn text_size_follows_the_thickness_choices() {
        let sizes: Vec<f64> = WIDTHS
            .iter()
            .map(|w| {
                Style {
                    width: *w,
                    ..Style::default()
                }
                .font_size()
            })
            .collect();
        assert_eq!(sizes, [14.0, 20.0, 26.0, 35.0, 50.0]);
    }
}
