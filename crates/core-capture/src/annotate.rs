//! The editor's model: marks drawn over a capture, picking and moving them, undo and redo, numbered
//! steps, cropping, and the decorative background around the picture. Coordinates are the picture's
//! points with the origin at its bottom-left, as AppKit draws.

use crate::geometry::{Point, Rect};

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Color {
    pub r: f64,
    pub g: f64,
    pub b: f64,
    pub a: f64,
}

impl Color {
    pub const fn rgb(r: u8, g: u8, b: u8) -> Self {
        Self {
            r: r as f64 / 255.0,
            g: g as f64 / 255.0,
            b: b as f64 / 255.0,
            a: 1.0,
        }
    }

    pub const fn with_alpha(self, a: f64) -> Self {
        Self { a, ..self }
    }
}

/// The colours offered for marks.
pub const PALETTE: [Color; 7] = [
    Color::rgb(0xF2, 0x3B, 0x30),
    Color::rgb(0xFF, 0x95, 0x00),
    Color::rgb(0xFF, 0xD6, 0x0A),
    Color::rgb(0x34, 0xC7, 0x59),
    Color::rgb(0x0A, 0x84, 0xFF),
    Color::rgb(0x1C, 0x1C, 0x1E),
    Color::rgb(0xFF, 0xFF, 0xFF),
];

/// Top and bottom colours of the background gradients.
pub const GRADIENTS: [(Color, Color); 6] = [
    (Color::rgb(0x7F, 0x7F, 0xD5), Color::rgb(0x86, 0xA8, 0xE7)),
    (Color::rgb(0xF8, 0x9B, 0x29), Color::rgb(0xFF, 0x0F, 0x7B)),
    (Color::rgb(0x43, 0xE9, 0x7B), Color::rgb(0x38, 0xF9, 0xD7)),
    (Color::rgb(0x2C, 0x3E, 0x50), Color::rgb(0x4C, 0xA1, 0xAF)),
    (Color::rgb(0xF5, 0xF7, 0xFA), Color::rgb(0xC3, 0xCF, 0xE2)),
    (Color::rgb(0x0F, 0x0C, 0x29), Color::rgb(0x30, 0x2B, 0x63)),
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tool {
    Select,
    Arrow,
    Line,
    Rectangle,
    Ellipse,
    Highlight,
    Text,
    Counter,
    Pixelate,
    Crop,
}

impl Tool {
    pub const ALL: [Tool; 10] = [
        Tool::Select,
        Tool::Arrow,
        Tool::Line,
        Tool::Rectangle,
        Tool::Ellipse,
        Tool::Highlight,
        Tool::Text,
        Tool::Counter,
        Tool::Pixelate,
        Tool::Crop,
    ];
}

#[derive(Debug, Clone, PartialEq)]
pub enum Shape {
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
    /// A translucent marker over the picture.
    Highlight(Rect),
    /// `bounds` is the measured size of the text, kept for picking.
    Text {
        at: Point,
        text: String,
        size: f64,
        bounds: Rect,
    },
    /// A numbered circle for step-by-step instructions.
    Counter {
        center: Point,
        number: u32,
    },
    /// An area of the picture made unreadable.
    Pixelate(Rect),
}

#[derive(Debug, Clone, PartialEq)]
pub struct Mark {
    pub shape: Shape,
    pub color: Color,
    /// Line width; the size of a counter.
    pub width: f64,
}

/// Radius of a counter circle at line width 1; it grows with the width.
pub const COUNTER_RADIUS: f64 = 12.0;
/// How far from a line a click still picks it.
const HIT_SLOP: f64 = 5.0;

fn distance_to_segment(p: Point, a: Point, b: Point) -> f64 {
    let (dx, dy) = (b.x - a.x, b.y - a.y);
    let length = dx * dx + dy * dy;
    let t = if length == 0.0 {
        0.0
    } else {
        (((p.x - a.x) * dx + (p.y - a.y) * dy) / length).clamp(0.0, 1.0)
    };
    let (x, y) = (a.x + t * dx, a.y + t * dy);
    ((p.x - x).powi(2) + (p.y - y).powi(2)).sqrt()
}

fn grow(r: Rect, by: f64) -> Rect {
    Rect::new(r.x - by, r.y - by, r.w + 2.0 * by, r.h + 2.0 * by)
}

pub fn counter_radius(width: f64) -> f64 {
    COUNTER_RADIUS + width * 1.5
}

impl Mark {
    pub fn bounds(&self) -> Rect {
        match &self.shape {
            Shape::Arrow { from, to } | Shape::Line { from, to } => {
                grow(Rect::from_corners(*from, *to), self.width)
            }
            Shape::Rectangle(r) | Shape::Ellipse(r) | Shape::Highlight(r) | Shape::Pixelate(r) => {
                *r
            }
            Shape::Text { bounds, .. } => *bounds,
            Shape::Counter { center, .. } => {
                let radius = counter_radius(self.width);
                Rect::new(
                    center.x - radius,
                    center.y - radius,
                    2.0 * radius,
                    2.0 * radius,
                )
            }
        }
    }

    pub fn hit(&self, p: Point) -> bool {
        match &self.shape {
            Shape::Arrow { from, to } | Shape::Line { from, to } => {
                distance_to_segment(p, *from, *to) <= self.width / 2.0 + HIT_SLOP
            }
            Shape::Rectangle(r) | Shape::Ellipse(r) => {
                // An outline is picked on its edge; inside is still the picture underneath.
                let outer = grow(*r, self.width / 2.0 + HIT_SLOP);
                let inner = grow(*r, -(self.width / 2.0 + HIT_SLOP));
                outer.contains(p) && !(inner.w > 0.0 && inner.h > 0.0 && inner.contains(p))
            }
            Shape::Counter { center, .. } => {
                let radius = counter_radius(self.width);
                (p.x - center.x).hypot(p.y - center.y) <= radius
            }
            Shape::Highlight(r) | Shape::Pixelate(r) | Shape::Text { bounds: r, .. } => {
                grow(*r, HIT_SLOP).contains(p)
            }
        }
    }

    pub fn moved(&self, dx: f64, dy: f64) -> Mark {
        let shift = |p: Point| Point::new(p.x + dx, p.y + dy);
        let shape = match &self.shape {
            Shape::Arrow { from, to } => Shape::Arrow {
                from: shift(*from),
                to: shift(*to),
            },
            Shape::Line { from, to } => Shape::Line {
                from: shift(*from),
                to: shift(*to),
            },
            Shape::Rectangle(r) => Shape::Rectangle(r.offset(dx, dy)),
            Shape::Ellipse(r) => Shape::Ellipse(r.offset(dx, dy)),
            Shape::Highlight(r) => Shape::Highlight(r.offset(dx, dy)),
            Shape::Pixelate(r) => Shape::Pixelate(r.offset(dx, dy)),
            Shape::Text {
                at,
                text,
                size,
                bounds,
            } => Shape::Text {
                at: shift(*at),
                text: text.clone(),
                size: *size,
                bounds: bounds.offset(dx, dy),
            },
            Shape::Counter { center, number } => Shape::Counter {
                center: shift(*center),
                number: *number,
            },
        };
        Mark {
            shape,
            ..self.clone()
        }
    }
}

/// The three corners of an arrow head whose tip is at `to`.
pub fn arrow_head(from: Point, to: Point, width: f64) -> [Point; 3] {
    let angle = (to.y - from.y).atan2(to.x - from.x);
    let length = 10.0 + width * 3.0;
    let spread = 0.45;
    let corner = |a: f64| Point::new(to.x - length * a.cos(), to.y - length * a.sin());
    [to, corner(angle + spread), corner(angle - spread)]
}

/// The coloured frame around an exported picture.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Background {
    /// Index into `GRADIENTS`.
    pub gradient: usize,
    pub padding: f64,
    /// Corner radius of the picture inside it.
    pub radius: f64,
    pub shadow: bool,
}

impl Default for Background {
    fn default() -> Self {
        Self {
            gradient: 0,
            padding: 48.0,
            radius: 10.0,
            shadow: true,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Default)]
struct State {
    marks: Vec<Mark>,
    crop: Option<Rect>,
    background: Option<Background>,
}

/// Everything drawn over one picture, with its undo history.
pub struct Document {
    pub width: f64,
    pub height: f64,
    state: State,
    undo: Vec<State>,
    redo: Vec<State>,
}

impl Document {
    /// A picture of `width` x `height` points with nothing on it.
    pub fn new(width: f64, height: f64) -> Self {
        Self {
            width,
            height,
            state: State::default(),
            undo: Vec::new(),
            redo: Vec::new(),
        }
    }

    pub fn marks(&self) -> &[Mark] {
        &self.state.marks
    }

    pub fn crop(&self) -> Option<Rect> {
        self.state.crop
    }

    pub fn background(&self) -> Option<Background> {
        self.state.background
    }

    pub fn can_undo(&self) -> bool {
        !self.undo.is_empty()
    }

    fn change(&mut self, edit: impl FnOnce(&mut State)) {
        let before = self.state.clone();
        edit(&mut self.state);
        if self.state != before {
            self.undo.push(before);
            self.redo.clear();
        }
    }

    /// Adds a mark in front of the others; returns its index.
    pub fn add(&mut self, mark: Mark) -> usize {
        self.change(|s| s.marks.push(mark));
        self.state.marks.len() - 1
    }

    pub fn remove(&mut self, index: usize) {
        if index < self.state.marks.len() {
            self.change(|s| {
                s.marks.remove(index);
            });
        }
    }

    pub fn replace(&mut self, index: usize, mark: Mark) {
        if index < self.state.marks.len() {
            self.change(|s| s.marks[index] = mark);
        }
    }

    /// Moves a mark; one undo step however far it went.
    pub fn translate(&mut self, index: usize, dx: f64, dy: f64) {
        if let Some(moved) = self.state.marks.get(index).map(|m| m.moved(dx, dy)) {
            self.replace(index, moved);
        }
    }

    /// Crops to `rect` (kept inside the picture), or removes the crop with `None`.
    pub fn set_crop(&mut self, rect: Option<Rect>) {
        let picture = Rect::new(0.0, 0.0, self.width, self.height);
        let rect = rect
            .map(|r| r.clamped_to(&picture))
            .filter(|r| r.w >= 1.0 && r.h >= 1.0);
        self.change(|s| s.crop = rect);
    }

    pub fn set_background(&mut self, background: Option<Background>) {
        self.change(|s| s.background = background);
    }

    pub fn undo(&mut self) -> bool {
        let Some(previous) = self.undo.pop() else {
            return false;
        };
        self.redo.push(std::mem::replace(&mut self.state, previous));
        true
    }

    pub fn redo(&mut self) -> bool {
        let Some(next) = self.redo.pop() else {
            return false;
        };
        self.undo.push(std::mem::replace(&mut self.state, next));
        true
    }

    /// The front-most mark under `p`.
    pub fn hit(&self, p: Point) -> Option<usize> {
        self.state.marks.iter().rposition(|m| m.hit(p))
    }

    /// The number the next counter gets.
    pub fn next_counter(&self) -> u32 {
        self.state
            .marks
            .iter()
            .filter_map(|m| match m.shape {
                Shape::Counter { number, .. } => Some(number),
                _ => None,
            })
            .max()
            .map_or(1, |n| n + 1)
    }

    /// The part of the picture that is kept: the crop, or all of it.
    pub fn visible(&self) -> Rect {
        self.state
            .crop
            .unwrap_or(Rect::new(0.0, 0.0, self.width, self.height))
    }

    /// The size of the finished picture, and where the visible part of the capture goes in it.
    pub fn layout(&self) -> (f64, f64, Rect) {
        let visible = self.visible();
        match self.state.background {
            Some(bg) => {
                let pad = bg.padding.max(0.0);
                (
                    visible.w + 2.0 * pad,
                    visible.h + 2.0 * pad,
                    Rect::new(pad, pad, visible.w, visible.h),
                )
            }
            None => (
                visible.w,
                visible.h,
                Rect::new(0.0, 0.0, visible.w, visible.h),
            ),
        }
    }
}

/// Makes `data` (4 bytes per pixel, rows `stride` bytes apart) blocky: every `block` x `block` square
/// takes the average colour of its pixels.
pub fn pixelate(data: &mut [u8], width: usize, height: usize, stride: usize, block: usize) {
    let block = block.max(1);
    for top in (0..height).step_by(block) {
        for left in (0..width).step_by(block) {
            let (bottom, right) = ((top + block).min(height), (left + block).min(width));
            let mut sum = [0u64; 4];
            for y in top..bottom {
                for x in left..right {
                    let i = y * stride + x * 4;
                    for (c, total) in sum.iter_mut().enumerate() {
                        *total += u64::from(data[i + c]);
                    }
                }
            }
            let count = ((bottom - top) * (right - left)) as u64;
            let average = sum.map(|total| (total / count) as u8);
            for y in top..bottom {
                for x in left..right {
                    let i = y * stride + x * 4;
                    data[i..i + 4].copy_from_slice(&average);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn mark(shape: Shape) -> Mark {
        Mark {
            shape,
            color: PALETTE[0],
            width: 4.0,
        }
    }

    #[test]
    fn undo_and_redo_walk_the_history() {
        let mut doc = Document::new(400.0, 300.0);
        assert!(!doc.can_undo());
        doc.add(mark(Shape::Rectangle(Rect::new(10.0, 10.0, 50.0, 40.0))));
        doc.add(mark(Shape::Line {
            from: Point::new(0.0, 0.0),
            to: Point::new(100.0, 100.0),
        }));
        doc.translate(0, 5.0, 5.0);
        assert_eq!(doc.marks().len(), 2);
        assert!(doc.undo());
        assert_eq!(doc.marks()[0].bounds(), Rect::new(10.0, 10.0, 50.0, 40.0));
        assert!(doc.undo() && doc.undo());
        assert!(doc.marks().is_empty() && !doc.undo());
        assert!(doc.redo() && doc.redo());
        assert_eq!(doc.marks().len(), 2);
        // A new change forgets what could be redone.
        doc.remove(1);
        assert!(!doc.redo());
    }

    #[test]
    fn picking_finds_the_front_most_mark_near_the_pointer() {
        let mut doc = Document::new(400.0, 300.0);
        doc.add(mark(Shape::Highlight(Rect::new(0.0, 0.0, 200.0, 200.0))));
        doc.add(mark(Shape::Arrow {
            from: Point::new(20.0, 20.0),
            to: Point::new(180.0, 20.0),
        }));
        assert_eq!(doc.hit(Point::new(100.0, 24.0)), Some(1));
        assert_eq!(doc.hit(Point::new(100.0, 100.0)), Some(0));
        assert_eq!(doc.hit(Point::new(300.0, 250.0)), None);
    }

    #[test]
    fn an_outline_is_picked_on_its_edge_not_in_its_middle() {
        let frame = mark(Shape::Rectangle(Rect::new(100.0, 100.0, 100.0, 100.0)));
        assert!(frame.hit(Point::new(101.0, 150.0)));
        assert!(!frame.hit(Point::new(150.0, 150.0)));
        let counter = mark(Shape::Counter {
            center: Point::new(50.0, 50.0),
            number: 1,
        });
        assert!(counter.hit(Point::new(55.0, 55.0)));
        assert!(!counter.hit(Point::new(90.0, 90.0)));
    }

    #[test]
    fn counters_keep_counting_up() {
        let mut doc = Document::new(100.0, 100.0);
        assert_eq!(doc.next_counter(), 1);
        for _ in 0..3 {
            let number = doc.next_counter();
            doc.add(mark(Shape::Counter {
                center: Point::new(10.0, 10.0),
                number,
            }));
        }
        assert_eq!(doc.next_counter(), 4);
        doc.remove(0);
        assert_eq!(doc.next_counter(), 4);
    }

    #[test]
    fn crops_stay_inside_the_picture_and_the_background_pads_them() {
        let mut doc = Document::new(400.0, 300.0);
        doc.set_crop(Some(Rect::new(-50.0, 100.0, 200.0, 500.0)));
        assert_eq!(doc.visible(), Rect::new(0.0, 100.0, 150.0, 200.0));
        assert_eq!(
            doc.layout(),
            (150.0, 200.0, Rect::new(0.0, 0.0, 150.0, 200.0))
        );
        doc.set_background(Some(Background::default()));
        assert_eq!(
            doc.layout(),
            (246.0, 296.0, Rect::new(48.0, 48.0, 150.0, 200.0))
        );
        doc.set_crop(Some(Rect::new(500.0, 500.0, 10.0, 10.0)));
        assert_eq!(doc.crop(), None);
    }

    #[test]
    fn arrow_heads_point_at_the_end_of_the_arrow() {
        let [tip, a, b] = arrow_head(Point::new(0.0, 0.0), Point::new(100.0, 0.0), 4.0);
        assert_eq!(tip, Point::new(100.0, 0.0));
        assert!(a.x < 100.0 && b.x < 100.0);
        assert!((a.y + b.y).abs() < 1e-9, "symmetric about the shaft");
    }

    #[test]
    fn pixelation_averages_each_block() {
        // 4 x 2 pixels, blocks of 2: the left block averages 0 and 100, the right 200 and 255.
        let mut data = vec![
            0, 0, 0, 255, 100, 100, 100, 255, 200, 0, 0, 255, 255, 0, 0, 255, //
            0, 0, 0, 255, 100, 100, 100, 255, 200, 0, 0, 255, 255, 0, 0, 255,
        ];
        pixelate(&mut data, 4, 2, 16, 2);
        assert_eq!(&data[0..4], &[50, 50, 50, 255]);
        assert_eq!(&data[4..8], &[50, 50, 50, 255]);
        assert_eq!(&data[8..12], &[227, 0, 0, 255]);
        assert_eq!(&data[16 + 12..16 + 16], &[227, 0, 0, 255]);
    }

    #[test]
    fn moving_a_mark_moves_every_point_of_it() {
        let text = mark(Shape::Text {
            at: Point::new(10.0, 10.0),
            text: "Xin chào".into(),
            size: 20.0,
            bounds: Rect::new(10.0, 10.0, 80.0, 24.0),
        });
        let moved = text.moved(5.0, -5.0);
        assert_eq!(moved.bounds(), Rect::new(15.0, 5.0, 80.0, 24.0));
        let Shape::Text { at, .. } = moved.shape else {
            panic!("still text");
        };
        assert_eq!(at, Point::new(15.0, 5.0));
    }
}
