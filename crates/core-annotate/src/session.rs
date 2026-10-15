//! The editing session: a document, its history, the selection, the tool and style in hand, and the pointer
//! gestures that turn into commands. The window only reports where the pointer is and draws the result.

use crate::{
    document::{Command, Document, ExportPlan, History},
    geom::{Point, Rect, snap_angle, square_corner},
    shape::{Colour, Handle, Kind, Object, Style},
};

/// How far from a thin line a click still counts as on it, in points.
pub const HIT_TOLERANCE: f64 = 5.0;
/// How far from a grip a press still takes it.
pub const HANDLE_REACH: f64 = 8.0;
/// The size of a text box made by a click without a drag.
const DEFAULT_TEXT_WIDTH: f64 = 200.0;
/// What a duplicate is offset by, so that it does not hide its original.
const DUPLICATE_OFFSET: f64 = 12.0;
/// Pen points closer than this to the last one are dropped.
const PEN_STEP: f64 = 1.5;
const PEN_MAX_POINTS: usize = 20_000;
/// The smallest region a crop may keep.
const MIN_CROP: f64 = 8.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Tool {
    Select,
    Arrow,
    Line,
    Rectangle,
    Ellipse,
    Text,
    Pen,
    Highlighter,
    Blur,
    Crop,
}

#[derive(Debug, Clone, Copy, Default)]
pub struct Mods {
    /// Keeps a line on a multiple of 45 degrees and a box square.
    pub shift: bool,
}

/// What the window has to do after a pointer event.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Effect {
    None,
    /// Put a text editor over this text object. Call [`Session::finish_text`] when it is done.
    EditText(u64),
}

/// The height a text needs: `(text, font size, box width)`. The window supplies the real one, measured by the
/// text system.
pub type TextMeasure = Box<dyn Fn(&str, f64, f64) -> f64>;

enum Gesture {
    Create {
        id: u64,
        anchor: Point,
        base: Rect,
    },
    Move {
        id: u64,
        original: Object,
        grab: Point,
        base: Rect,
    },
    Resize {
        id: u64,
        handle: Handle,
        original: Object,
        base: Rect,
    },
    Crop {
        anchor: Point,
        current: Point,
    },
}

struct TextEdit {
    id: u64,
    /// The object has not been added to the history yet: it is dropped if no text is typed.
    fresh: bool,
    base: Rect,
    original: Option<Object>,
}

pub struct Session {
    doc: Document,
    history: History,
    selection: Option<u64>,
    tool: Tool,
    style: Style,
    gesture: Option<Gesture>,
    editing: Option<TextEdit>,
    next_id: u64,
    /// The object whose last change may be extended by the next one (arrow-key nudges, style changes) so
    /// that a run of them is one step to undo.
    run: Option<u64>,
    measure: TextMeasure,
}

fn estimate_text_height(text: &str, size: f64, _width: f64) -> f64 {
    text.lines().count().max(1) as f64 * size * 1.25
}

impl Session {
    pub fn new(width: u32, height: u32, scale: f64) -> Self {
        Self::with_document(Document::new(width, height, scale))
    }

    pub fn with_document(doc: Document) -> Self {
        Self {
            doc,
            history: History::default(),
            selection: None,
            tool: Tool::Select,
            style: Style::default(),
            gesture: None,
            editing: None,
            next_id: 1,
            run: None,
            measure: Box::new(estimate_text_height),
        }
    }

    pub fn set_text_measure(&mut self, measure: TextMeasure) {
        self.measure = measure;
    }

    pub fn doc(&self) -> &Document {
        &self.doc
    }

    pub fn tool(&self) -> Tool {
        self.tool
    }

    pub fn style(&self) -> Style {
        self.style
    }

    pub fn selection(&self) -> Option<u64> {
        self.selection
    }

    pub fn selected_object(&self) -> Option<&Object> {
        self.selection.and_then(|id| self.doc.object(id))
    }

    /// The text object a text editor is over.
    pub fn editing(&self) -> Option<u64> {
        self.editing.as_ref().map(|e| e.id)
    }

    /// Whether the pointer is down on something.
    pub fn gesture_active(&self) -> bool {
        self.gesture.is_some()
    }

    /// The region a crop drag has marked so far.
    pub fn crop_preview(&self) -> Option<Rect> {
        match self.gesture {
            Some(Gesture::Crop { anchor, current }) => Some(Rect::from_points(anchor, current)),
            _ => None,
        }
    }

    pub fn can_undo(&self) -> bool {
        self.history.can_undo()
    }

    pub fn can_redo(&self) -> bool {
        self.history.can_redo()
    }

    pub fn export_plan(&self) -> ExportPlan {
        self.doc.export_plan()
    }

    /// Ends a run of changes that would have been merged into one step.
    pub fn end_run(&mut self) {
        self.run = None;
    }

    pub fn set_tool(&mut self, tool: Tool) {
        self.cancel_gesture();
        self.run = None;
        self.tool = tool;
    }

    fn new_id(&mut self) -> u64 {
        let id = self.next_id;
        self.next_id += 1;
        id
    }

    fn execute(&mut self, command: Command) {
        self.doc.apply(&command);
        self.history.push(command);
        self.run = None;
    }

    /// A command that replaces the object at `index` and grows the canvas from `base` to hold it.
    fn change_command(&self, index: usize, before: Object, after: Object, base: Rect) -> Command {
        let grown = self.doc.grown(base, after.bounds());
        let mut steps = Vec::new();
        if grown != base {
            steps.push(Command::Canvas {
                before: base,
                after: grown,
            });
        }
        steps.push(Command::Replace {
            index,
            before,
            after,
        });
        Command::Batch(steps)
    }

    fn insert_command(&self, object: Object, base: Rect) -> Command {
        let grown = self.doc.grown(base, object.bounds());
        let mut steps = Vec::new();
        if grown != base {
            steps.push(Command::Canvas {
                before: base,
                after: grown,
            });
        }
        steps.push(Command::Insert {
            index: self.doc.objects().len(),
            object,
        });
        Command::Batch(steps)
    }

    fn fit_text(&self, object: &mut Object) {
        let size = object.style.font_size();
        if let Kind::Text { rect, text } = &mut object.kind {
            let probe = if text.is_empty() { " " } else { text.as_str() };
            rect.h = (self.measure)(probe, size, rect.w);
        }
    }

    // Pointer

    pub fn pointer_down(&mut self, p: Point) -> Effect {
        if self.editing.is_some() {
            return Effect::None;
        }
        self.cancel_gesture();
        self.run = None;
        if self.tool != Tool::Crop
            && let Some(object) = self.selected_object()
            && let Some(handle) = object.handle_at(p, HANDLE_REACH)
        {
            self.gesture = Some(Gesture::Resize {
                id: object.id,
                handle,
                original: object.clone(),
                base: self.doc.canvas(),
            });
            return Effect::None;
        }
        match self.tool {
            Tool::Select => match self.doc.hit_test(p, HIT_TOLERANCE) {
                Some(id) => {
                    self.selection = Some(id);
                    let original = self.doc.object(id).cloned().expect("hit object exists");
                    self.gesture = Some(Gesture::Move {
                        id,
                        original,
                        grab: p,
                        base: self.doc.canvas(),
                    });
                }
                None => self.selection = None,
            },
            Tool::Crop => {
                self.selection = None;
                self.gesture = Some(Gesture::Crop {
                    anchor: p,
                    current: p,
                });
            }
            Tool::Text => {
                let hit = self.doc.hit_test(p, HIT_TOLERANCE).filter(|id| {
                    matches!(
                        self.doc.object(*id).map(|o| &o.kind),
                        Some(Kind::Text { .. })
                    )
                });
                if let Some(id) = hit {
                    return self.edit_text(id);
                }
                self.begin_create(p);
            }
            _ => self.begin_create(p),
        }
        Effect::None
    }

    fn begin_create(&mut self, p: Point) {
        let kind = match self.tool {
            Tool::Arrow => Kind::Arrow { from: p, to: p },
            Tool::Line => Kind::Line { from: p, to: p },
            Tool::Rectangle => Kind::Rectangle(Rect::new(p.x, p.y, 0.0, 0.0)),
            Tool::Ellipse => Kind::Ellipse(Rect::new(p.x, p.y, 0.0, 0.0)),
            Tool::Blur => Kind::Blur(Rect::new(p.x, p.y, 0.0, 0.0)),
            Tool::Pen => Kind::Pen(vec![p]),
            Tool::Highlighter => Kind::Highlight(vec![p]),
            Tool::Text => Kind::Text {
                rect: Rect::new(p.x, p.y, 0.0, 0.0),
                text: String::new(),
            },
            Tool::Select | Tool::Crop => return,
        };
        let id = self.new_id();
        self.doc.push_live(Object {
            id,
            kind,
            style: self.style,
        });
        self.selection = Some(id);
        self.gesture = Some(Gesture::Create {
            id,
            anchor: p,
            base: self.doc.canvas(),
        });
    }

    /// Replaces an object during a drag, growing the canvas from where it was when the drag began.
    fn show_live(&mut self, object: Object, base: Rect) {
        let grown = self.doc.grown(base, object.bounds());
        if let Some(index) = self.doc.index_of(object.id) {
            self.doc.set_live(index, object);
        }
        self.doc.set_canvas_live(grown);
    }

    pub fn pointer_drag(&mut self, p: Point, mods: Mods) {
        let Some(gesture) = self.gesture.take() else {
            return;
        };
        match &gesture {
            Gesture::Create { id, anchor, base } => {
                if let Some(mut object) = self.doc.object(*id).cloned() {
                    let end = if mods.shift {
                        snap_angle(*anchor, p)
                    } else {
                        p
                    };
                    let corner = if mods.shift {
                        square_corner(*anchor, p)
                    } else {
                        p
                    };
                    match &mut object.kind {
                        Kind::Arrow { to, .. } | Kind::Line { to, .. } => *to = end,
                        Kind::Rectangle(r) | Kind::Ellipse(r) | Kind::Blur(r) => {
                            *r = Rect::from_points(*anchor, corner);
                        }
                        Kind::Text { rect, .. } => *rect = Rect::from_points(*anchor, p),
                        Kind::Pen(points) | Kind::Highlight(points) => {
                            let far = points
                                .last()
                                .is_none_or(|last| last.distance(p) >= PEN_STEP);
                            if far && points.len() < PEN_MAX_POINTS {
                                points.push(p);
                            }
                        }
                    }
                    self.show_live(object, *base);
                }
            }
            Gesture::Move {
                id,
                original,
                grab,
                base,
            } => {
                let moved = original.translated(p.x - grab.x, p.y - grab.y);
                debug_assert_eq!(moved.id, *id);
                self.show_live(moved, *base);
            }
            Gesture::Resize {
                handle,
                original,
                base,
                ..
            } => {
                let mut resized = original.with_handle(*handle, p);
                self.fit_text(&mut resized);
                self.show_live(resized, *base);
            }
            Gesture::Crop { anchor, .. } => {
                self.gesture = Some(Gesture::Crop {
                    anchor: *anchor,
                    current: p,
                });
                return;
            }
        }
        self.gesture = Some(gesture);
    }

    pub fn pointer_up(&mut self, p: Point, mods: Mods) -> Effect {
        if self.gesture.is_none() {
            return Effect::None;
        }
        self.pointer_drag(p, mods);
        match self.gesture.take() {
            Some(Gesture::Create { id, base, .. }) => self.finish_create(id, base),
            Some(Gesture::Move {
                id, original, base, ..
            })
            | Some(Gesture::Resize {
                id, original, base, ..
            }) => {
                self.finish_change(id, original, base);
                Effect::None
            }
            Some(Gesture::Crop { anchor, current }) => {
                let region = Rect::from_points(anchor, current);
                if self.crop(region) {
                    self.tool = Tool::Select;
                }
                Effect::None
            }
            None => Effect::None,
        }
    }

    fn finish_create(&mut self, id: u64, base: Rect) -> Effect {
        let Some(mut object) = self.doc.object(id).cloned() else {
            return Effect::None;
        };
        let is_text = matches!(object.kind, Kind::Text { .. });
        if is_text {
            if let Kind::Text { rect, .. } = &mut object.kind {
                if rect.w < DEFAULT_TEXT_WIDTH / 2.0 && rect.h < 2.0 * object.style.font_size() {
                    rect.w = DEFAULT_TEXT_WIDTH;
                }
                rect.w = rect.w.max(crate::shape::MIN_TEXT_WIDTH);
            }
            self.fit_text(&mut object);
            self.show_live(object, base);
            self.editing = Some(TextEdit {
                id,
                fresh: true,
                base,
                original: None,
            });
            return Effect::EditText(id);
        }
        self.doc.remove_live(id);
        self.doc.set_canvas_live(base);
        if is_degenerate(&object) {
            self.selection = None;
            return Effect::None;
        }
        let command = self.insert_command(object, base);
        self.execute(command);
        self.selection = Some(id);
        Effect::None
    }

    fn finish_change(&mut self, id: u64, original: Object, base: Rect) {
        let Some(index) = self.doc.index_of(id) else {
            return;
        };
        let after = self.doc.objects()[index].clone();
        self.doc.set_live(index, original.clone());
        self.doc.set_canvas_live(base);
        if after == original {
            return;
        }
        let command = self.change_command(index, original, after, base);
        self.execute(command);
    }

    /// Drops a drag that is under way, as if it had not started.
    pub fn cancel_gesture(&mut self) {
        match self.gesture.take() {
            Some(Gesture::Create { id, base, .. }) => {
                self.doc.remove_live(id);
                self.doc.set_canvas_live(base);
                if self.selection == Some(id) {
                    self.selection = None;
                }
            }
            Some(Gesture::Move {
                id, original, base, ..
            })
            | Some(Gesture::Resize {
                id, original, base, ..
            }) => {
                if let Some(index) = self.doc.index_of(id) {
                    self.doc.set_live(index, original);
                }
                self.doc.set_canvas_live(base);
            }
            Some(Gesture::Crop { .. }) | None => {}
        }
    }

    /// A double click on text edits it, with any tool.
    pub fn double_click(&mut self, p: Point) -> Effect {
        self.cancel_gesture();
        match self.doc.hit_test(p, HIT_TOLERANCE) {
            Some(id)
                if matches!(
                    self.doc.object(id).map(|o| &o.kind),
                    Some(Kind::Text { .. })
                ) =>
            {
                self.edit_text(id)
            }
            _ => Effect::None,
        }
    }

    fn edit_text(&mut self, id: u64) -> Effect {
        self.selection = Some(id);
        self.run = None;
        self.editing = Some(TextEdit {
            id,
            fresh: false,
            base: self.doc.canvas(),
            original: self.doc.object(id).cloned(),
        });
        Effect::EditText(id)
    }

    /// The text editor is done with `text`. An empty text removes the object.
    pub fn finish_text(&mut self, text: &str) {
        let Some(edit) = self.editing.take() else {
            return;
        };
        let Some(live) = self.doc.object(edit.id).cloned() else {
            return;
        };
        let mut object = live;
        if let Kind::Text { text: slot, .. } = &mut object.kind {
            *slot = text.to_owned();
        }
        self.fit_text(&mut object);
        let empty = text.trim().is_empty();
        if edit.fresh {
            self.doc.remove_live(edit.id);
            self.doc.set_canvas_live(edit.base);
            if empty {
                self.selection = None;
                return;
            }
            let command = self.insert_command(object, edit.base);
            self.execute(command);
            self.selection = Some(edit.id);
            return;
        }
        let (Some(original), Some(index)) = (edit.original, self.doc.index_of(edit.id)) else {
            return;
        };
        if empty {
            self.execute(Command::Remove {
                index,
                object: original,
            });
            self.selection = None;
        } else if object != original {
            let command = self.change_command(index, original, object, edit.base);
            self.execute(command);
        }
    }

    // Commands

    /// Keeps `region` (clamped to the canvas, to whole pixels) and drops the rest. Objects that lie entirely
    /// outside it are removed, and come back with an undo. Returns whether anything was cropped.
    pub fn crop(&mut self, region: Rect) -> bool {
        if self.editing.is_some() {
            return false;
        }
        let before = self.doc.canvas();
        let Some(clipped) = region.intersection(&before) else {
            return false;
        };
        let kept = self.doc.snap_round(clipped);
        if kept.w < MIN_CROP || kept.h < MIN_CROP || kept == before {
            return false;
        }
        let mut steps: Vec<Command> = self
            .doc
            .objects()
            .iter()
            .enumerate()
            .filter(|(_, o)| !o.bounds().intersects(&kept))
            .map(|(index, o)| Command::Remove {
                index,
                object: o.clone(),
            })
            .rev()
            .collect();
        steps.push(Command::Canvas {
            before,
            after: kept,
        });
        self.execute(Command::Batch(steps));
        if self.selected_object().is_none() {
            self.selection = None;
        }
        true
    }

    pub fn undo(&mut self) -> bool {
        if self.editing.is_some() {
            return false;
        }
        self.cancel_gesture();
        self.run = None;
        let applied = self.history.undo(&mut self.doc);
        self.select_after(applied)
    }

    pub fn redo(&mut self) -> bool {
        if self.editing.is_some() {
            return false;
        }
        self.cancel_gesture();
        self.run = None;
        let applied = self.history.redo(&mut self.doc);
        self.select_after(applied)
    }

    /// After an undo or redo the object it touched is the selection, so that it can be acted on at once.
    /// Whatever was selected before stays when the step touched no object, as long as it still exists.
    fn select_after(&mut self, applied: Option<Command>) -> bool {
        let done = applied.is_some();
        if let Some(id) = applied.as_ref().and_then(Command::result_object) {
            self.selection = Some(id);
        }
        if self.selected_object().is_none() {
            self.selection = None;
        }
        done
    }

    pub fn delete_selected(&mut self) -> bool {
        let Some(object) = self
            .selected_object()
            .cloned()
            .filter(|_| self.editing.is_none())
        else {
            return false;
        };
        let Some(index) = self.doc.index_of(object.id) else {
            return false;
        };
        self.execute(Command::Remove { index, object });
        self.selection = None;
        true
    }

    pub fn duplicate_selected(&mut self) -> bool {
        let Some(object) = self
            .selected_object()
            .cloned()
            .filter(|_| self.editing.is_none())
        else {
            return false;
        };
        let mut copy = object.translated(DUPLICATE_OFFSET, DUPLICATE_OFFSET);
        copy.id = self.new_id();
        let id = copy.id;
        let command = self.insert_command(copy, self.doc.canvas());
        self.execute(command);
        self.selection = Some(id);
        true
    }

    /// Moves the selected object; a run of nudges is one step to undo.
    pub fn nudge(&mut self, dx: f64, dy: f64) -> bool {
        let Some(object) = self.selected_object().filter(|_| self.editing.is_none()) else {
            return false;
        };
        let moved = object.translated(dx, dy);
        self.change_selected(moved)
    }

    pub fn set_colour(&mut self, colour: Colour) {
        self.style.colour = colour;
        self.restyle(|s| s.colour = colour);
    }

    pub fn set_width(&mut self, width: f64) {
        self.style.width = width;
        self.restyle(|s| s.width = width);
    }

    pub fn set_fill(&mut self, fill: bool) {
        self.style.fill = fill;
        self.restyle(|s| s.fill = fill);
    }

    fn restyle(&mut self, change: impl Fn(&mut Style)) {
        if self.gesture.is_some() || self.editing.is_some() {
            return;
        }
        let Some(object) = self.selected_object() else {
            return;
        };
        let mut restyled = object.clone();
        change(&mut restyled.style);
        self.fit_text(&mut restyled);
        self.change_selected(restyled);
    }

    /// Replaces the selected object with `after`, extending the last step when it was a change to the same
    /// object.
    fn change_selected(&mut self, after: Object) -> bool {
        let id = after.id;
        let Some(index) = self.doc.index_of(id) else {
            return false;
        };
        let (mut before, mut base) = (self.doc.objects()[index].clone(), self.doc.canvas());
        let extend = self.run == Some(id);
        if extend && let Some(last) = self.history.take_last() {
            self.doc.apply(&last.inverse());
            before = self.doc.objects()[index].clone();
            base = self.doc.canvas();
        }
        if before == after {
            self.run = None;
            return false;
        }
        let command = self.change_command(index, before, after, base);
        self.execute(command);
        self.run = Some(id);
        true
    }
}

/// A gesture that was too small to mean a shape.
fn is_degenerate(object: &Object) -> bool {
    match &object.kind {
        Kind::Arrow { from, to } | Kind::Line { from, to } => from.distance(*to) < 4.0,
        Kind::Rectangle(r) | Kind::Ellipse(r) => r.w < 3.0 || r.h < 3.0,
        Kind::Blur(r) => r.w < 6.0 || r.h < 6.0,
        Kind::Pen(points) | Kind::Highlight(points) => {
            let frame = object.frame();
            points.len() < 2 || frame.w.hypot(frame.h) < 3.0
        }
        Kind::Text { .. } => false,
    }
}
