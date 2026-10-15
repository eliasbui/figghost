//! The document: an image, a canvas that can be larger or smaller than it, and the objects on top, in
//! drawing order. Everything that changes it is a [`Command`] that can be undone exactly.

use crate::{
    geom::{Point, Rect},
    pixels::PixelRect,
    shape::{Colour, Kind, Object},
};

/// The canvas never grows past this many pixels on a side, or past [`MAX_AREA`] in all, so that an
/// object dragged far away cannot ask for gigabytes when the picture is saved.
pub const MAX_SIDE: f64 = 8192.0;
pub const MAX_AREA: f64 = 48_000_000.0;

/// The smallest size of a blur block, in points.
const MIN_BLOCK: f64 = 6.0;

#[derive(Debug, Clone, PartialEq)]
pub struct Document {
    /// The picture's own size in pixels.
    image: (u32, u32),
    /// Pixels per point.
    scale: f64,
    canvas: Rect,
    background: Colour,
    objects: Vec<Object>,
}

/// How the finished picture is made: this many pixels, taken from the canvas at `origin`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ExportPlan {
    pub origin: Point,
    pub width: u32,
    pub height: u32,
    pub scale: f64,
}

/// One blur object, in the terms the drawing code needs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BlurJob {
    /// Pixels of the image to redact, in image pixels from its top-left corner (may lie outside it).
    pub region: PixelRect,
    /// The side of a block, in pixels.
    pub block: u32,
}

impl Document {
    /// A document with the canvas exactly as large as the picture. A scale that is not a sensible number
    /// is taken as 1.
    pub fn new(width: u32, height: u32, scale: f64) -> Self {
        let scale = if scale.is_finite() && (0.25..=16.0).contains(&scale) {
            scale
        } else {
            1.0
        };
        let canvas = Rect::new(
            0.0,
            0.0,
            f64::from(width) / scale,
            f64::from(height) / scale,
        );
        Self {
            image: (width, height),
            scale,
            canvas,
            background: Colour::CLEAR,
            objects: Vec::new(),
        }
    }

    pub fn scale(&self) -> f64 {
        self.scale
    }

    pub fn image_pixels(&self) -> (u32, u32) {
        self.image
    }

    /// Where the picture sits: always at the origin.
    pub fn image_rect(&self) -> Rect {
        Rect::new(
            0.0,
            0.0,
            f64::from(self.image.0) / self.scale,
            f64::from(self.image.1) / self.scale,
        )
    }

    pub fn canvas(&self) -> Rect {
        self.canvas
    }

    pub fn background(&self) -> Colour {
        self.background
    }

    /// Bottom to top.
    pub fn objects(&self) -> &[Object] {
        &self.objects
    }

    pub fn index_of(&self, id: u64) -> Option<usize> {
        self.objects.iter().position(|o| o.id == id)
    }

    pub fn object(&self, id: u64) -> Option<&Object> {
        self.objects.iter().find(|o| o.id == id)
    }

    /// The topmost object under `p`.
    pub fn hit_test(&self, p: Point, tolerance: f64) -> Option<u64> {
        self.objects
            .iter()
            .rev()
            .find(|o| o.hit(p, tolerance))
            .map(|o| o.id)
    }

    /// `rect` widened to whole pixels on every side, never narrower.
    pub fn snap_out(&self, rect: Rect) -> Rect {
        let s = self.scale;
        let (l, t) = (
            (rect.x * s + 1e-6).floor() / s,
            (rect.y * s + 1e-6).floor() / s,
        );
        let (r, b) = (
            (rect.right() * s - 1e-6).ceil() / s,
            (rect.bottom() * s - 1e-6).ceil() / s,
        );
        Rect::new(l, t, r - l, b - t)
    }

    /// `rect` with every edge moved to the nearest pixel.
    pub fn snap_round(&self, rect: Rect) -> Rect {
        let s = self.scale;
        let (l, t) = ((rect.x * s).round() / s, (rect.y * s).round() / s);
        let (r, b) = (
            (rect.right() * s).round() / s,
            (rect.bottom() * s).round() / s,
        );
        Rect::new(l, t, r - l, b - t)
    }

    /// The canvas that holds `base` and `needed`, in whole pixels. It does not grow past the limits: along
    /// an axis that would, `base` is kept.
    pub fn grown(&self, base: Rect, needed: Rect) -> Rect {
        let wanted = self.snap_out(base.union(&needed));
        let s = self.scale;
        let mut r = wanted;
        if wanted.w * s > MAX_SIDE.max(base.w * s) {
            r.x = base.x;
            r.w = base.w;
        }
        if wanted.h * s > MAX_SIDE.max(base.h * s) {
            r.y = base.y;
            r.h = base.h;
        }
        if r.w * r.h * s * s > MAX_AREA.max(base.w * base.h * s * s) {
            return base;
        }
        r
    }

    pub fn export_plan(&self) -> ExportPlan {
        ExportPlan {
            origin: self.canvas.origin(),
            width: (self.canvas.w * self.scale).round().max(1.0) as u32,
            height: (self.canvas.h * self.scale).round().max(1.0) as u32,
            scale: self.scale,
        }
    }

    /// The side of a blur block, in pixels, for an object drawn at `width`.
    pub fn block_pixels(&self, width: f64) -> u32 {
        ((width * 3.0).max(MIN_BLOCK) * self.scale).round().max(2.0) as u32
    }

    /// What to redact for a blur object: the pixels under it that lie on the canvas. `None` when the object
    /// is not a blur or nothing of it is on the canvas.
    pub fn blur_job(&self, object: &Object) -> Option<BlurJob> {
        let Kind::Blur(rect) = object.kind else {
            return None;
        };
        let visible = rect.intersection(&self.canvas)?;
        let s = self.scale;
        let (x0, y0) = ((visible.x * s).floor(), (visible.y * s).floor());
        let (x1, y1) = ((visible.right() * s).ceil(), (visible.bottom() * s).ceil());
        let region = PixelRect {
            x: x0 as i64,
            y: y0 as i64,
            w: (x1 - x0) as u32,
            h: (y1 - y0) as u32,
        };
        (region.w > 0 && region.h > 0).then_some(BlurJob {
            region,
            block: self.block_pixels(object.style.width),
        })
    }

    /// Changes the document by `command`. A command that does not fit the document (an object that is not
    /// where it says) is ignored, so a bug there cannot panic the editor.
    pub fn apply(&mut self, command: &Command) {
        match command {
            Command::Insert { index, object } => {
                let at = (*index).min(self.objects.len());
                self.objects.insert(at, object.clone());
            }
            Command::Remove { index, object } => {
                if self.objects.get(*index).is_some_and(|o| o.id == object.id) {
                    self.objects.remove(*index);
                }
            }
            Command::Replace { index, after, .. } => {
                if let Some(slot) = self.objects.get_mut(*index) {
                    *slot = after.clone();
                }
            }
            Command::Canvas { after, .. } => self.canvas = *after,
            Command::Background { after, .. } => self.background = *after,
            Command::Batch(commands) => {
                for c in commands {
                    self.apply(c);
                }
            }
        }
    }

    /// Replaces an object while a drag is under way, without a command. The caller restores it or commits it.
    pub(crate) fn set_live(&mut self, index: usize, object: Object) {
        if let Some(slot) = self.objects.get_mut(index) {
            *slot = object;
        }
    }

    pub(crate) fn set_canvas_live(&mut self, canvas: Rect) {
        self.canvas = canvas;
    }

    pub(crate) fn push_live(&mut self, object: Object) {
        self.objects.push(object);
    }

    pub(crate) fn remove_live(&mut self, id: u64) {
        self.objects.retain(|o| o.id != id);
    }
}

/// A change to the document that knows how to undo itself.
#[derive(Debug, Clone, PartialEq)]
pub enum Command {
    Insert {
        index: usize,
        object: Object,
    },
    Remove {
        index: usize,
        object: Object,
    },
    Replace {
        index: usize,
        before: Object,
        after: Object,
    },
    Canvas {
        before: Rect,
        after: Rect,
    },
    Background {
        before: Colour,
        after: Colour,
    },
    /// Several changes that are one step for the person: undone together, in reverse.
    Batch(Vec<Command>),
}

impl Command {
    /// The object this command leaves in the document, if it leaves one: what is worth selecting after it.
    pub fn result_object(&self) -> Option<u64> {
        match self {
            Command::Insert { object, .. } => Some(object.id),
            Command::Replace { after, .. } => Some(after.id),
            Command::Batch(commands) => {
                let mut ids: Vec<u64> =
                    commands.iter().filter_map(Command::result_object).collect();
                ids.dedup();
                (ids.len() == 1).then(|| ids[0])
            }
            _ => None,
        }
    }

    pub fn inverse(&self) -> Command {
        match self {
            Command::Insert { index, object } => Command::Remove {
                index: *index,
                object: object.clone(),
            },
            Command::Remove { index, object } => Command::Insert {
                index: *index,
                object: object.clone(),
            },
            Command::Replace {
                index,
                before,
                after,
            } => Command::Replace {
                index: *index,
                before: after.clone(),
                after: before.clone(),
            },
            Command::Canvas { before, after } => Command::Canvas {
                before: *after,
                after: *before,
            },
            Command::Background { before, after } => Command::Background {
                before: *after,
                after: *before,
            },
            Command::Batch(commands) => {
                Command::Batch(commands.iter().rev().map(Command::inverse).collect())
            }
        }
    }
}

/// How many steps can be undone.
pub const HISTORY_LIMIT: usize = 500;

#[derive(Debug, Default)]
pub struct History {
    undo: Vec<Command>,
    redo: Vec<Command>,
}

impl History {
    /// Records a command that has just been applied. What was undone before it can no longer be redone.
    pub fn push(&mut self, command: Command) {
        self.redo.clear();
        self.undo.push(command);
        if self.undo.len() > HISTORY_LIMIT {
            self.undo.remove(0);
        }
    }

    /// Undoes the last command and returns the command that was applied to do it.
    pub fn undo(&mut self, doc: &mut Document) -> Option<Command> {
        let command = self.undo.pop()?;
        let inverse = command.inverse();
        doc.apply(&inverse);
        self.redo.push(command);
        Some(inverse)
    }

    /// Redoes the last undone command and returns it.
    pub fn redo(&mut self, doc: &mut Document) -> Option<Command> {
        let command = self.redo.pop()?;
        doc.apply(&command);
        self.undo.push(command.clone());
        Some(command)
    }

    pub fn can_undo(&self) -> bool {
        !self.undo.is_empty()
    }

    pub fn can_redo(&self) -> bool {
        !self.redo.is_empty()
    }

    /// Takes back the last command without undoing it, to be replaced by one that covers more.
    pub(crate) fn take_last(&mut self) -> Option<Command> {
        self.undo.pop()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::shape::Style;

    fn rect_object(id: u64, r: Rect) -> Object {
        Object {
            id,
            kind: Kind::Rectangle(r),
            style: Style::default(),
        }
    }

    #[test]
    fn a_new_document_is_the_picture_at_its_size_in_points() {
        let doc = Document::new(2000, 1000, 2.0);
        assert_eq!(doc.canvas(), Rect::new(0.0, 0.0, 1000.0, 500.0));
        assert_eq!(doc.image_rect(), doc.canvas());
        let plan = doc.export_plan();
        assert_eq!((plan.width, plan.height), (2000, 1000));
    }

    #[test]
    fn a_scale_that_is_not_a_number_is_one() {
        for bad in [0.0, -2.0, f64::NAN, f64::INFINITY, 1000.0] {
            assert_eq!(Document::new(100, 50, bad).scale(), 1.0, "{bad}");
        }
        assert_eq!(Document::new(100, 50, 1.5).scale(), 1.5);
    }

    #[test]
    fn growing_rounds_outwards_to_whole_pixels() {
        let doc = Document::new(100, 100, 2.0);
        let grown = doc.grown(doc.canvas(), Rect::new(-10.26, 10.0, 5.0, 5.0));
        assert_eq!(grown.x, -10.5, "0.26 pt is 0.52 px, so one more pixel");
        assert!(grown.contains_rect(&doc.canvas()));
        assert_eq!(grown.right(), 50.0);
        let px = grown.w * 2.0;
        assert_eq!(px, px.round());
    }

    #[test]
    fn growing_never_shrinks_and_an_inside_object_changes_nothing() {
        let doc = Document::new(100, 100, 1.0);
        assert_eq!(
            doc.grown(doc.canvas(), Rect::new(10.0, 10.0, 20.0, 20.0)),
            doc.canvas()
        );
        let wide = Rect::new(-50.0, -50.0, 300.0, 300.0);
        let big = doc.grown(doc.canvas(), wide);
        assert_eq!(doc.grown(big, Rect::new(0.0, 0.0, 1.0, 1.0)), big);
    }

    #[test]
    fn the_canvas_has_a_limit() {
        let doc = Document::new(100, 100, 1.0);
        let far = doc.grown(doc.canvas(), Rect::new(0.0, 0.0, 1.0e7, 50.0));
        assert_eq!(far.x, 0.0);
        assert_eq!(far.w, 100.0, "too wide, so the width stays");
        let down = doc.grown(doc.canvas(), Rect::new(0.0, -100.0, 50.0, 4000.0));
        assert_eq!(
            (down.y, down.h),
            (-100.0, 4000.0),
            "an axis that fits still grows"
        );
        let mut big = Document::new(9000, 10, 1.0);
        big.canvas = Rect::new(0.0, 0.0, 9000.0, 10.0);
        let still = big.grown(big.canvas(), Rect::new(0.0, 0.0, 9000.0, 10.0));
        assert_eq!(
            still,
            big.canvas(),
            "a picture already past the limit opens whole"
        );
        let area = doc.grown(
            Rect::new(0.0, 0.0, 6000.0, 6000.0),
            Rect::new(0.0, 0.0, 7000.0, 7000.0),
        );
        assert_eq!(area.w, 6000.0, "the area limit keeps the base");
    }

    #[test]
    fn export_size_is_the_canvas_times_the_scale() {
        let mut doc = Document::new(300, 200, 2.0);
        doc.canvas = doc.grown(doc.canvas(), Rect::new(-33.3, -7.7, 400.0, 300.0));
        let plan = doc.export_plan();
        assert_eq!(f64::from(plan.width), (doc.canvas().w * 2.0).round());
        assert_eq!(f64::from(plan.height), (doc.canvas().h * 2.0).round());
        assert_eq!(plan.origin, doc.canvas().origin());
    }

    #[test]
    fn the_topmost_object_is_hit_first() {
        let mut doc = Document::new(100, 100, 1.0);
        let mut low = rect_object(1, Rect::new(0.0, 0.0, 50.0, 50.0));
        low.style.fill = true;
        let mut high = rect_object(2, Rect::new(10.0, 10.0, 50.0, 50.0));
        high.style.fill = true;
        doc.push_live(low);
        doc.push_live(high);
        assert_eq!(doc.hit_test(Point::new(20.0, 20.0), 0.0), Some(2));
        assert_eq!(doc.hit_test(Point::new(5.0, 5.0), 0.0), Some(1));
        assert_eq!(doc.hit_test(Point::new(90.0, 90.0), 0.0), None);
    }

    #[test]
    fn commands_undo_to_the_exact_document() {
        let mut doc = Document::new(100, 100, 1.0);
        let before = doc.clone();
        let a = rect_object(1, Rect::new(0.0, 0.0, 10.0, 10.0));
        let b = rect_object(2, Rect::new(5.0, 5.0, 10.0, 10.0));
        let commands = [
            Command::Insert {
                index: 0,
                object: a.clone(),
            },
            Command::Insert {
                index: 1,
                object: b.clone(),
            },
            Command::Replace {
                index: 0,
                before: a.clone(),
                after: a.translated(4.0, 4.0),
            },
            Command::Canvas {
                before: Rect::new(0.0, 0.0, 100.0, 100.0),
                after: Rect::new(-20.0, 0.0, 120.0, 100.0),
            },
            Command::Remove {
                index: 1,
                object: b,
            },
            Command::Background {
                before: Colour::CLEAR,
                after: Colour::rgb(1, 2, 3),
            },
        ];
        let mut history = History::default();
        let mut snapshots = vec![doc.clone()];
        for c in commands {
            doc.apply(&c);
            history.push(c);
            snapshots.push(doc.clone());
        }
        for expected in snapshots.iter().rev().skip(1) {
            assert!(history.undo(&mut doc).is_some());
            assert_eq!(&doc, expected);
        }
        assert_eq!(doc, before);
        assert!(history.undo(&mut doc).is_none());
        for expected in snapshots.iter().skip(1) {
            assert!(history.redo(&mut doc).is_some());
            assert_eq!(&doc, expected);
        }
        assert!(history.redo(&mut doc).is_none());
    }

    #[test]
    fn a_new_command_ends_the_redo_branch() {
        let mut doc = Document::new(10, 10, 1.0);
        let mut history = History::default();
        let insert = Command::Insert {
            index: 0,
            object: rect_object(1, Rect::new(0.0, 0.0, 1.0, 1.0)),
        };
        doc.apply(&insert);
        history.push(insert);
        history.undo(&mut doc);
        assert!(history.can_redo());
        let other = Command::Background {
            before: Colour::CLEAR,
            after: Colour::rgb(9, 9, 9),
        };
        doc.apply(&other);
        history.push(other);
        assert!(!history.can_redo());
    }

    #[test]
    fn a_batch_undoes_in_reverse_so_indexes_stay_right() {
        let mut doc = Document::new(10, 10, 1.0);
        let objects: Vec<Object> = (1..=3)
            .map(|i| rect_object(i, Rect::new(f64::from(i as u32), 0.0, 1.0, 1.0)))
            .collect();
        for o in &objects {
            doc.push_live(o.clone());
        }
        let before = doc.clone();
        // Removing from the highest index down keeps the lower ones where they are.
        let batch = Command::Batch(vec![
            Command::Remove {
                index: 2,
                object: objects[2].clone(),
            },
            Command::Remove {
                index: 0,
                object: objects[0].clone(),
            },
        ]);
        doc.apply(&batch);
        assert_eq!(doc.objects().len(), 1);
        doc.apply(&batch.inverse());
        assert_eq!(doc, before);
    }

    #[test]
    fn a_command_that_does_not_fit_is_ignored() {
        let mut doc = Document::new(10, 10, 1.0);
        let before = doc.clone();
        doc.apply(&Command::Remove {
            index: 4,
            object: rect_object(1, Rect::new(0.0, 0.0, 1.0, 1.0)),
        });
        doc.apply(&Command::Replace {
            index: 9,
            before: rect_object(1, Rect::new(0.0, 0.0, 1.0, 1.0)),
            after: rect_object(1, Rect::new(2.0, 0.0, 1.0, 1.0)),
        });
        assert_eq!(doc, before);
    }

    #[test]
    fn history_is_bounded() {
        let mut doc = Document::new(10, 10, 1.0);
        let mut history = History::default();
        for i in 0..(HISTORY_LIMIT + 20) {
            let c = Command::Background {
                before: Colour::CLEAR,
                after: Colour::rgb((i % 250) as u8, 0, 0),
            };
            doc.apply(&c);
            history.push(c);
        }
        let mut steps = 0;
        while history.undo(&mut doc).is_some() {
            steps += 1;
        }
        assert_eq!(steps, HISTORY_LIMIT);
    }

    #[test]
    fn a_blur_job_covers_whole_pixels_and_stays_on_the_canvas() {
        let doc = Document::new(200, 100, 2.0);
        let blur = Object {
            id: 1,
            kind: Kind::Blur(Rect::new(10.2, 5.0, 20.0, 10.0)),
            style: Style::default(),
        };
        let job = doc.blur_job(&blur).unwrap();
        assert_eq!(
            job.region,
            PixelRect {
                x: 20,
                y: 10,
                w: 41,
                h: 20
            }
        );
        assert_eq!(
            job.block, 24,
            "four points of thickness give twelve points of block, two pixels each"
        );
        let half_out = Object {
            kind: Kind::Blur(Rect::new(90.0, 40.0, 40.0, 40.0)),
            ..blur.clone()
        };
        let job = doc.blur_job(&half_out).unwrap();
        assert_eq!(
            (job.region.x, job.region.w),
            (180, 20),
            "cut at the canvas edge"
        );
        let away = Object {
            kind: Kind::Blur(Rect::new(500.0, 0.0, 10.0, 10.0)),
            ..blur.clone()
        };
        assert!(doc.blur_job(&away).is_none());
        let not_blur = rect_object(2, Rect::new(0.0, 0.0, 5.0, 5.0));
        assert!(doc.blur_job(&not_blur).is_none());
    }

    #[test]
    fn the_blur_block_has_a_floor() {
        let doc = Document::new(10, 10, 1.0);
        assert_eq!(doc.block_pixels(1.0), 6);
        assert_eq!(doc.block_pixels(14.0), 42);
    }
}
