//! Dragging a window onto the notch: where the layouts to choose from appear, where the cells of the chosen one
//! show on the screen, where the window would land, and the state machine of one drag.
//! Everything is in the coordinates of `Window` frames and the Accessibility API: the origin at the top-left
//! corner of the primary screen, y growing downwards, so a screen above or left of it has negative values.
//! The platform layer converts pointer positions to these coordinates and reports what it can see; nothing here
//! touches the system, so every rule is covered by plain tests.
//!
//! A drag goes in two steps. At the top edge of a screen the layouts to choose from hang from the notch, each a
//! small screen with its cells. Hovering one shows its cells over the screen; moving down onto the screen shrinks the
//! row to a chip with the name of the layout, and the cell under the pointer lights up. Releasing in a cell puts the
//! window there.

use crate::{
    Rect,
    templates::{Cell, Template},
};

pub type Point = (f64, f64);

/// One layout in the row: a small screen with its cells, and its name under it.
pub const CELL: (f64, f64) = (84.0, 66.0);
pub const GAP: f64 = 8.0;
pub const PAD: f64 = 12.0;
/// The most layouts in one row; more go on further rows, as even as they can be.
pub const PER_ROW: usize = 10;
/// What the row shrinks to once a layout is chosen and the pointer is on the screen: a chip with its name.
pub const CHIP: (f64, f64) = (176.0, 26.0);
/// How far from the top edge the pointer must be for the row to appear on a screen without a notch; with one the
/// band is as tall as the cutout. Never less than the band that opens the notch, so the row is always up first.
pub const TRIGGER_HEIGHT: f64 = 8.0;
/// What `Display::inset` holds beyond the height of the cutout: the gap between the cutout and the shapes.
const CLEARANCE: f64 = 6.0;
/// The trigger reaches this far past each side of the notch, so the pointer need not hit the cutout exactly.
pub const TRIGGER_SLACK: f64 = 48.0;
/// Once shown, the row stays while the pointer is this close to it, so a slightly wild hand does not hide it.
pub const STAY_SLACK: f64 = 8.0;
/// A press this far below the top of a window still counts as on its title bar: toolbars share the title bar.
pub const TITLE_BAR_HEIGHT: f64 = 52.0;
/// The pointer must travel this far before a press is looked at as the start of a drag.
const DRAG_SLOP: f64 = 4.0;
/// A press that has not moved a window after this many drag events, or this far, was not on a window.
const MAX_CHECKS: u8 = 12;
const GIVE_UP_DISTANCE: f64 = 120.0;

/// Each layout's row, its place in that row and the length of the row. The rows are as even as they can be, with no
/// more than `PER_ROW` in one.
fn grid(count: usize) -> Vec<(usize, usize, usize)> {
    if count == 0 {
        return Vec::new();
    }
    let rows = count.div_ceil(PER_ROW);
    let (base, extra) = (count / rows, count % rows);
    let length = |row: usize| base + usize::from(row < extra);
    (0..rows)
        .flat_map(|row| (0..length(row)).map(move |column| (row, column, length(row))))
        .collect()
}

/// One screen as the drag sees it.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Display {
    pub frame: Rect,
    /// The usable area: below the menu bar and beside the Dock. Cells are placed inside it.
    pub area: Rect,
    /// Width of the notch cutout, or of the pill the notch shows on a screen without one.
    pub cutout: f64,
    /// Height the row must leave free at the top so it clears the cutout.
    pub inset: f64,
}

fn right(r: Rect) -> f64 {
    r.x + r.w
}

fn bottom(r: Rect) -> f64 {
    r.y + r.h
}

/// Edges are inclusive: a pointer pushed against the top of the screen sits exactly on the boundary.
pub fn contains(r: Rect, p: Point) -> bool {
    p.0 >= r.x && p.0 <= right(r) && p.1 >= r.y && p.1 <= bottom(r)
}

fn inflate(r: Rect, by: f64) -> Rect {
    Rect::new(r.x - by, r.y - by, r.w + 2.0 * by, r.h + 2.0 * by)
}

impl Display {
    /// The band along the top edge, centred on the notch, where the pointer makes the row appear.
    pub fn trigger(&self) -> Rect {
        let w = self.cutout + 2.0 * TRIGGER_SLACK;
        Rect::new(
            self.frame.x + (self.frame.w - w) / 2.0,
            self.frame.y,
            w,
            TRIGGER_HEIGHT.max(self.inset - CLEARANCE),
        )
    }

    /// The shape that hangs from the top edge and holds `count` layouts.
    pub fn panel(&self, count: usize) -> Rect {
        let grid = grid(count);
        let widest = grid.iter().map(|g| g.2).max().unwrap_or(0) as f64;
        let rows = grid.iter().map(|g| g.0 + 1).max().unwrap_or(1) as f64;
        let row = widest * CELL.0 + (widest - 1.0).max(0.0) * GAP + 2.0 * PAD;
        // Wide enough to cover the cutout, so on a real notch the shape grows out of it.
        let w = row.max(self.cutout + 2.0 * PAD);
        Rect::new(
            self.frame.x + (self.frame.w - w) / 2.0,
            self.frame.y,
            w,
            self.inset + rows * CELL.1 + (rows - 1.0) * GAP + PAD,
        )
    }

    /// The shape the row shrinks to, once a layout is chosen and the pointer is over the screen.
    pub fn chip(&self) -> Rect {
        let w = CHIP.0.max(self.cutout + 2.0 * PAD);
        Rect::new(
            self.frame.x + (self.frame.w - w) / 2.0,
            self.frame.y,
            w,
            self.inset + CHIP.1 + PAD / 2.0,
        )
    }

    /// The drawn shape of each of `count` layouts, in order; each row is centred in the shape.
    pub fn cells(&self, count: usize) -> Vec<Rect> {
        let panel = self.panel(count);
        grid(count)
            .into_iter()
            .map(|(row, column, length)| {
                let n = length as f64;
                let left = panel.x + (panel.w - (n * CELL.0 + (n - 1.0) * GAP)) / 2.0;
                Rect::new(
                    left + column as f64 * (CELL.0 + GAP),
                    self.frame.y + self.inset + row as f64 * (CELL.1 + GAP),
                    CELL.0,
                    CELL.1,
                )
            })
            .collect()
    }

    /// The layout under the pointer. One owns half of each gap beside it, the end of its row up to the edge of the
    /// shape, and on the last row the padding below, so moving over the shape never passes over a dead spot; the
    /// strip above the shapes, which the trigger band overlaps on a screen without a notch, belongs to none.
    pub fn zone_at(&self, count: usize, p: Point) -> Option<usize> {
        let panel = self.panel(count);
        let top = self.frame.y + self.inset;
        if p.1 < top || p.1 > bottom(panel) || p.0 < panel.x || p.0 > right(panel) {
            return None;
        }
        let (grid, cells) = (grid(count), self.cells(count));
        let rows = grid.iter().map(|g| g.0 + 1).max()?;
        let row =
            (((p.1 - top + GAP / 2.0) / (CELL.1 + GAP)).floor().max(0.0) as usize).min(rows - 1);
        let in_row = || (0..cells.len()).filter(|&i| grid[i].0 == row);
        in_row()
            .find(|&i| p.0 < right(cells[i]) + GAP / 2.0)
            .or_else(|| in_row().next_back())
    }

    /// The cell of `template` under the pointer on this screen. The cells cover the usable area, so every point of it
    /// is in one; on an edge two cells share, the first.
    pub fn cell_at(&self, template: &Template, p: Point) -> Option<usize> {
        template
            .cells
            .iter()
            .position(|c| contains(c.place(self.area), p))
    }

    /// The frame `cell` gives a window on this display.
    pub fn place(&self, cell: Cell) -> Rect {
        cell.place(self.area)
    }
}

/// The display the pointer is on. A screen owns its left and top edges; the last screen also owns the right
/// and bottom ones, where the pointer can rest when it is pushed against the end of the desktop.
pub fn display_at(displays: &[Display], p: Point) -> Option<usize> {
    let half_open = |d: &Display| {
        p.0 >= d.frame.x && p.0 < right(d.frame) && p.1 >= d.frame.y && p.1 < bottom(d.frame)
    };
    displays
        .iter()
        .position(half_open)
        .or_else(|| displays.iter().position(|d| contains(d.frame, p)))
}

/// Roles that do something of their own when pressed; a drag that starts on one is not a window drag. A browser's
/// tabs (radio buttons in a tab group) are not among them: the tab strip is a browser's title bar, and a window with
/// one tab moves when its tab is dragged. Whether the window really moves is checked during the drag anyway.
const INTERACTIVE: [&str; 15] = [
    "AXButton",
    "AXCheckBox",
    "AXRadioGroup",
    "AXPopUpButton",
    "AXMenuButton",
    "AXMenuItem",
    "AXSlider",
    "AXTextField",
    "AXTextArea",
    "AXComboBox",
    "AXSearchField",
    "AXScrollBar",
    "AXLink",
    "AXIncrementor",
    "AXDisclosureTriangle",
];

/// Whether a press at `at` on an element with this role is on the title bar of `window`: the strip at its top,
/// except over a control. Whether the window then really moves is checked separately.
pub fn is_title_bar_press(window: Rect, at: Point, role: &str) -> bool {
    let dy = at.1 - window.y;
    at.0 >= window.x
        && at.0 <= right(window)
        && (0.0..TITLE_BAR_HEIGHT).contains(&dy)
        && !INTERACTIVE.contains(&role)
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum Phase {
    Idle,
    /// Pressed on a title bar; waiting to see the window follow the pointer.
    Pressed {
        start: Point,
        checks: u8,
    },
    Dragging,
}

/// What the screen should look like now.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct View {
    /// The display whose row is showing.
    pub shown: Option<usize>,
    /// The layout under the pointer in the row, on that display.
    pub hover: Option<usize>,
    /// The layout whose cells show over the screen: the last one the pointer was on, until the drag ends.
    pub chosen: Option<usize>,
    /// The cell of the chosen layout under the pointer, once the pointer is on the screen.
    pub cell: Option<usize>,
    /// The row has shrunk to a chip, because a layout is chosen and the pointer is on the screen.
    pub compact: bool,
}

/// Where a window was dropped: a cell of a layout on a display.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Dropped {
    pub display: usize,
    pub template: usize,
    pub cell: usize,
}

/// One drag from the press to the release. The caller reports what it sees; the machine decides what shows.
pub struct Machine {
    displays: Vec<Display>,
    /// The layouts each display offers, in the order of `displays`.
    templates: Vec<Vec<Template>>,
    phase: Phase,
    view: View,
}

impl Default for Machine {
    fn default() -> Self {
        Self::new()
    }
}

impl Machine {
    pub fn new() -> Self {
        Self {
            displays: Vec::new(),
            templates: Vec::new(),
            phase: Phase::Idle,
            view: View::default(),
        }
    }

    pub fn view(&self) -> View {
        self.view
    }

    /// Where the window lands if it is dropped now: the cell lit under the pointer, or the whole area when the pointer
    /// is on a layout of one cell. `None` while there is no such cell.
    pub fn footprint(&self) -> Option<Rect> {
        let shown = self.view.shown?;
        let (display, templates) = (self.displays.get(shown)?, self.templates.get(shown)?);
        let (template, cell) = match (self.view.chosen, self.view.cell, self.view.hover) {
            (Some(template), Some(cell), _) => (template, cell),
            (_, _, Some(template)) if templates.get(template)?.cells.len() == 1 => (template, 0),
            _ => return None,
        };
        let cell = *templates.get(template)?.cells.get(cell)?;
        Some(display.place(cell))
    }

    /// The cells of the chosen layout as frames on the screen that shows the row, with the area they fill.
    pub fn overlay(&self) -> Option<(Rect, Vec<Rect>)> {
        let shown = self.view.shown?;
        let display = self.displays.get(shown)?;
        let template = self.templates.get(shown)?.get(self.view.chosen?)?;
        Some((
            display.area,
            template.cells.iter().map(|c| display.place(*c)).collect(),
        ))
    }

    pub fn is_idle(&self) -> bool {
        self.phase == Phase::Idle
    }

    /// The left button went down at `at`. `templates` holds what each display offers. `on_title_bar` is true only when
    /// the platform found a window there and the press was on its title bar; without Accessibility it is always false
    /// and nothing happens.
    pub fn press(
        &mut self,
        displays: Vec<Display>,
        templates: Vec<Vec<Template>>,
        at: Point,
        on_title_bar: bool,
    ) {
        self.cancel();
        if on_title_bar && !displays.is_empty() && templates.len() == displays.len() {
            self.displays = displays;
            self.templates = templates;
            self.phase = Phase::Pressed {
                start: at,
                checks: 0,
            };
        }
    }

    /// The pointer moved with the button down. `moved` asks the platform whether the window has followed the
    /// pointer; it is called only while that is still unknown.
    pub fn drag(&mut self, at: Point, moved: impl FnOnce() -> bool) {
        match self.phase {
            Phase::Idle => {}
            Phase::Pressed { start, checks } => {
                let travelled = ((at.0 - start.0).powi(2) + (at.1 - start.1).powi(2)).sqrt();
                if travelled < DRAG_SLOP {
                    return;
                }
                if moved() {
                    self.phase = Phase::Dragging;
                    self.update(at);
                } else if checks + 1 >= MAX_CHECKS || travelled > GIVE_UP_DISTANCE {
                    self.cancel();
                } else {
                    self.phase = Phase::Pressed {
                        start,
                        checks: checks + 1,
                    };
                }
            }
            Phase::Dragging => self.update(at),
        }
    }

    /// The button came up. Returns where the window was dropped, if it was dropped in a cell.
    pub fn release(&mut self, at: Point) -> Option<Dropped> {
        let dropped = if self.phase == Phase::Dragging {
            self.update(at);
            self.dropped()
        } else {
            None
        };
        self.cancel();
        dropped
    }

    /// A cell lit under the pointer. A layout of one cell has nothing to choose, so a drop on it in the row is a drop
    /// in its cell.
    fn dropped(&self) -> Option<Dropped> {
        let display = self.view.shown?;
        let templates = self.templates.get(display)?;
        match (self.view.chosen, self.view.cell, self.view.hover) {
            (Some(template), Some(cell), _) => Some(Dropped {
                display,
                template,
                cell,
            }),
            (_, _, Some(template)) if templates.get(template)?.cells.len() == 1 => Some(Dropped {
                display,
                template,
                cell: 0,
            }),
            _ => None,
        }
    }

    /// Forgets the drag: the button was released unseen, a screen went away, the feature was turned off.
    pub fn cancel(&mut self) {
        self.phase = Phase::Idle;
        self.view = View::default();
    }

    fn update(&mut self, at: Point) {
        let displays = &self.displays;
        let before = self.view;
        let entered = display_at(displays, at).filter(|&d| contains(displays[d].trigger(), at));
        // The row stays while the pointer is on it or by it, and once a layout is chosen also while the pointer is
        // over the usable area of the screen, where its cells are.
        let stays = before.shown.filter(|&d| {
            let display = &displays[d];
            let shape = if before.compact {
                display.chip()
            } else {
                display.panel(self.templates[d].len())
            };
            contains(display.trigger(), at)
                || contains(inflate(shape, STAY_SLACK), at)
                || (before.chosen.is_some() && contains(display.area, at))
        });
        let shown = stays.or(entered);
        let mut view = View {
            shown,
            ..View::default()
        };
        if let Some(d) = shown {
            let (display, templates) = (&displays[d], &self.templates[d]);
            // A screen the pointer has just come to starts over.
            let chosen = before.chosen.filter(|_| shown == before.shown);
            let at_top = contains(display.trigger(), at);
            let compact = chosen.is_some()
                && if before.compact {
                    // The way back to the row is the top edge or the chip itself.
                    !(at_top || contains(inflate(display.chip(), STAY_SLACK), at))
                } else {
                    !(at_top || contains(inflate(display.panel(templates.len()), STAY_SLACK), at))
                        && contains(display.area, at)
                };
            view.compact = compact;
            view.hover = if compact {
                None
            } else {
                display.zone_at(templates.len(), at)
            };
            view.chosen = view.hover.or(chosen);
            view.cell = view
                .chosen
                .filter(|_| compact)
                .and_then(|t| display.cell_at(&templates[t], at));
        }
        self.view = view;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::templates::{self, built_in};

    fn display(frame: Rect, cutout: f64, inset: f64) -> Display {
        // The usable area starts below a 25 point menu bar.
        let area = Rect::new(frame.x, frame.y + 25.0, frame.w, frame.h - 25.0);
        Display {
            frame,
            area,
            cutout,
            inset,
        }
    }

    /// A notchless desktop monitor, the primary one.
    fn plain() -> Display {
        display(Rect::new(0.0, 0.0, 1920.0, 1080.0), 200.0, 10.0)
    }

    /// A notched laptop to the left and above, so its origin is negative on both axes.
    fn laptop() -> Display {
        display(Rect::new(-1512.0, -982.0, 1512.0, 982.0), 185.0, 38.0)
    }

    /// A larger display on the right with another size, higher than the primary one.
    fn big() -> Display {
        display(Rect::new(1920.0, -200.0, 2560.0, 1440.0), 200.0, 10.0)
    }

    fn near(a: f64, b: f64) -> bool {
        (a - b).abs() < 1e-9
    }

    fn middle(c: Rect) -> Point {
        (c.x + c.w / 2.0, c.y + c.h / 2.0)
    }

    fn row(ids: &[&str]) -> Vec<Template> {
        templates::choose(ids, &[])
    }

    /// The seven layouts a screen offers until its owner chooses, in their order.
    fn offered() -> Vec<Template> {
        row(&templates::DEFAULT)
    }

    const FULL: usize = 0;
    const HALVES: usize = 1;
    const THIRDS: usize = 2;
    const QUARTERS: usize = 3;

    #[test]
    fn the_row_is_centred_and_hangs_from_the_top_of_a_notchless_screen() {
        let d = plain();
        let panel = d.panel(6);
        assert!(near(panel.x + panel.w / 2.0, 960.0));
        assert_eq!(panel.y, 0.0);
        assert!(near(panel.w, 6.0 * CELL.0 + 5.0 * GAP + 2.0 * PAD));
        assert!(near(panel.h, 10.0 + CELL.1 + PAD));
        let cells = d.cells(6);
        assert_eq!(cells.len(), 6);
        assert!(near(cells[0].x - panel.x, PAD), "padding at the left");
        assert!(
            near(right(panel) - right(cells[5]), PAD),
            "and at the right"
        );
        assert!(near(cells[1].x - right(cells[0]), GAP));
        assert!(cells.iter().all(|c| c.y == 10.0 && c.h == CELL.1));
    }

    #[test]
    fn on_a_notched_screen_the_row_clears_the_cutout_and_covers_it() {
        let d = laptop();
        let panel = d.panel(2);
        // Two layouts are narrower than the cutout, so the shape is widened to cover it.
        assert!(near(panel.w, 185.0 + 2.0 * PAD));
        assert!(near(panel.x + panel.w / 2.0, -1512.0 / 2.0));
        assert_eq!(panel.y, -982.0, "negative origins are kept");
        let cells = d.cells(2);
        assert!(cells.iter().all(|c| near(c.y, -982.0 + 38.0)));
        assert!(
            cells[0].x >= panel.x && right(cells[1]) <= right(panel),
            "the cells stay inside the shape"
        );
        let one = d.cells(1)[0];
        assert!(
            near(one.x + one.w / 2.0, -756.0),
            "a single layout is centred"
        );
    }

    #[test]
    fn more_layouts_than_fit_a_row_go_on_further_rows_as_even_as_they_can() {
        let d = laptop();
        let lengths = |count: usize| {
            let cells = d.cells(count);
            let mut ys: Vec<f64> = cells.iter().map(|c| c.y).collect();
            ys.dedup_by(|a, b| near(*a, *b));
            ys.iter()
                .map(|y| cells.iter().filter(|c| near(c.y, *y)).count())
                .collect::<Vec<_>>()
        };
        assert_eq!(lengths(1), [1]);
        assert_eq!(lengths(PER_ROW), [PER_ROW]);
        assert_eq!(lengths(8), [8]);
        assert_eq!(lengths(9), [9]);
        assert_eq!(lengths(10), [10]);
        assert_eq!(lengths(14), [7, 7]);
        assert_eq!(lengths(15), [8, 7]);
        assert_eq!(lengths(22), [8, 7, 7]);
        assert!(d.cells(0).is_empty());
        let panel = d.panel(11);
        assert!(near(panel.w, 6.0 * CELL.0 + 5.0 * GAP + 2.0 * PAD));
        assert!(near(panel.h, 38.0 + 2.0 * CELL.1 + GAP + PAD));
        let cells = d.cells(11);
        let centre = |first: usize, last: usize| (cells[first].x + right(cells[last])) / 2.0;
        assert!(
            near(centre(0, 5), -756.0) && near(centre(6, 10), -756.0),
            "each row is centred"
        );
        assert!(
            near(bottom(panel) - bottom(cells[10]), PAD),
            "padding below the last row"
        );
    }

    #[test]
    fn the_chip_is_centred_and_clears_the_cutout() {
        let plain_chip = plain().chip();
        assert!(near(plain_chip.x + plain_chip.w / 2.0, 960.0) && plain_chip.y == 0.0);
        assert!(near(plain_chip.w, CHIP.0.max(200.0 + 2.0 * PAD)));
        assert!(near(plain_chip.h, 10.0 + CHIP.1 + PAD / 2.0));
        let notched = laptop().chip();
        assert!(near(notched.x + notched.w / 2.0, -756.0));
        assert!(notched.w >= 185.0 + 2.0 * PAD, "it covers the cutout");
        assert!(
            notched.h > 38.0 + CHIP.1 - 1.0,
            "and the line under it clears the cutout"
        );
        for d in [plain(), laptop(), big()] {
            let (chip, panel) = (d.chip(), d.panel(7));
            assert!(
                chip.w <= panel.w && chip.h < panel.h,
                "it is smaller than the row"
            );
        }
    }

    #[test]
    fn the_trigger_is_a_band_on_the_top_edge_centred_on_the_cutout() {
        for d in [plain(), laptop(), big()] {
            let t = d.trigger();
            assert_eq!(t.y, d.frame.y);
            assert_eq!(t.h, TRIGGER_HEIGHT.max(d.inset - CLEARANCE));
            assert!(near(t.w, d.cutout + 2.0 * TRIGGER_SLACK));
            assert!(near(t.x + t.w / 2.0, d.frame.x + d.frame.w / 2.0));
            let top_centre = (d.frame.x + d.frame.w / 2.0, d.frame.y);
            assert!(contains(t, top_centre), "the exact top edge counts");
            assert!(!contains(t, (top_centre.0, d.frame.y + t.h + 1.0)));
            assert!(t.h < d.inset, "the band never reaches the shapes");
            assert!(
                !contains(t, (d.frame.x + 20.0, d.frame.y)),
                "far from the notch does not"
            );
        }
    }

    #[test]
    fn on_a_notched_screen_the_band_is_as_tall_as_the_cutout() {
        // The cutout is 32 points tall there (inset 38 less the 6 of clearance), as is the notch's own trigger.
        assert_eq!(laptop().trigger().h, 32.0);
        assert_eq!(plain().trigger().h, TRIGGER_HEIGHT);
    }

    #[test]
    fn the_pointer_maps_to_a_layout_or_to_none() {
        let d = plain();
        let cells = d.cells(6);
        for (i, c) in cells.iter().enumerate() {
            assert_eq!(d.zone_at(6, middle(*c)), Some(i));
        }
        // The gap between two cells belongs to one of them, never to none.
        for pair in cells.windows(2) {
            let gap_middle = (right(pair[0]) + GAP / 2.0 - 0.01, pair[0].y + 10.0);
            assert!(d.zone_at(6, gap_middle).is_some());
        }
        // Padding at the ends belongs to the first and last layout.
        let panel = d.panel(6);
        assert_eq!(d.zone_at(6, (panel.x + 2.0, cells[0].y + 5.0)), Some(0));
        assert_eq!(d.zone_at(6, (right(panel), cells[0].y + 5.0)), Some(5));
        // Above the shapes, below the shape and beside it: none.
        assert_eq!(d.zone_at(6, (cells[2].x + 5.0, 0.0)), None);
        assert_eq!(d.zone_at(6, (cells[2].x + 5.0, bottom(panel) + 1.0)), None);
        assert_eq!(d.zone_at(6, (panel.x - 1.0, cells[0].y + 5.0)), None);
        assert_eq!(d.zone_at(6, (right(panel) + 1.0, cells[0].y + 5.0)), None);
        assert_eq!(d.zone_at(0, (960.0, 30.0)), None, "no layouts, no hit");
    }

    #[test]
    fn the_pointer_maps_to_a_layout_on_every_row_without_a_dead_spot() {
        let d = plain();
        for count in [9, 10, 15, 22] {
            let cells = d.cells(count);
            for (i, c) in cells.iter().enumerate() {
                assert_eq!(d.zone_at(count, middle(*c)), Some(i), "{count}: layout {i}");
            }
            // Every point of the shape below the strip at the top belongs to a layout.
            let panel = d.panel(count);
            let mut y = d.frame.y + d.inset;
            while y <= bottom(panel) {
                let mut x = panel.x;
                while x <= right(panel) {
                    assert!(d.zone_at(count, (x, y)).is_some(), "{count}: {x} {y}");
                    x += 1.5;
                }
                y += 1.5;
            }
        }
        // The gap between two rows is split between them.
        let cells = d.cells(11);
        let split = bottom(cells[0]) + GAP / 2.0;
        assert_eq!(d.zone_at(11, (middle(cells[6]).0, split - 0.01)), Some(1));
        assert_eq!(d.zone_at(11, (middle(cells[6]).0, split)), Some(6));
        // The shorter row reaches the edges of the shape, and the last row the padding below it.
        let panel = d.panel(11);
        assert_eq!(d.zone_at(11, (panel.x + 1.0, middle(cells[6]).1)), Some(6));
        assert_eq!(
            d.zone_at(11, (right(panel) - 1.0, middle(cells[6]).1)),
            Some(10)
        );
        assert_eq!(
            d.zone_at(11, (middle(cells[10]).0, bottom(panel))),
            Some(10)
        );
    }

    #[test]
    fn rows_work_on_every_display_with_its_own_size_and_origin() {
        let displays = [plain(), laptop(), big()];
        for (i, d) in displays.iter().enumerate() {
            for layout in [3, 6] {
                let p = middle(d.cells(7)[layout]);
                assert_eq!(display_at(&displays, p), Some(i), "display {i}");
                assert_eq!(d.zone_at(7, p), Some(layout));
            }
        }
    }

    #[test]
    fn the_pointer_belongs_to_one_display_even_on_a_shared_edge() {
        let displays = [plain(), laptop(), big()];
        // The right edge of the primary screen is the left edge of the big one.
        assert_eq!(display_at(&displays, (1920.0, 10.0)), Some(2));
        assert_eq!(display_at(&displays, (1919.5, 10.0)), Some(0));
        assert_eq!(display_at(&displays, (-100.0, -1.0)), Some(1));
        // Pushed against the end of the desktop, the pointer still has a display.
        assert_eq!(display_at(&displays, (4480.0, 100.0)), Some(2));
        assert_eq!(display_at(&displays, (100.0, 1080.0)), Some(0));
        assert_eq!(display_at(&displays, (9000.0, 9000.0)), None);
        assert_eq!(display_at(&[], (0.0, 0.0)), None);
    }

    #[test]
    fn the_cell_under_the_pointer_is_found_on_every_display_and_lands_in_its_usable_area() {
        for d in [plain(), laptop(), big()] {
            for id in templates::BUILT_IN {
                let template = built_in(id).unwrap();
                for (i, cell) in template.cells.iter().enumerate() {
                    let frame = d.place(*cell);
                    assert_eq!(
                        d.cell_at(&template, middle(frame)),
                        Some(i),
                        "{id} cell {i}"
                    );
                    assert!(
                        frame.x >= d.area.x
                            && right(frame) <= right(d.area) + 1e-9
                            && frame.y >= d.area.y
                            && bottom(frame) <= bottom(d.area) + 1e-9,
                        "{id} cell {i} stays in the usable area"
                    );
                }
                // Above the usable area is the menu bar, which is no cell.
                assert_eq!(
                    d.cell_at(&template, (middle(d.area).0, d.frame.y + 5.0)),
                    None,
                    "{id}"
                );
            }
        }
        // The one cell of `full` is the whole usable area.
        let d = big();
        assert_eq!(d.place(built_in("full").unwrap().cells[0]), d.area);
    }

    /// A title bar press, shortly after which the window follows the pointer.
    fn drag_to(m: &mut Machine, to: Point) {
        m.drag(to, || true);
    }

    fn started() -> (Machine, Vec<Display>, Vec<Vec<Template>>) {
        let displays = vec![plain(), laptop(), big()];
        let offers = vec![offered(); 3];
        let mut m = Machine::new();
        m.press(displays.clone(), offers.clone(), (500.0, 300.0), true);
        (m, displays, offers)
    }

    #[test]
    fn a_drag_to_the_top_shows_the_row_and_the_pointer_on_a_layout_lights_it_and_shows_its_cells() {
        let (mut m, displays, _) = started();
        assert!(!m.is_idle());
        drag_to(&mut m, (520.0, 280.0));
        assert_eq!(m.view(), View::default(), "far from the top: nothing shows");
        drag_to(&mut m, (960.0, 2.0));
        assert_eq!(
            m.view(),
            View {
                shown: Some(0),
                ..View::default()
            },
            "the pointer at the notch: the row, nothing lit"
        );
        assert_eq!((m.footprint(), m.overlay()), (None, None));
        let tile = displays[0].cells(7)[THIRDS];
        drag_to(&mut m, (960.0, 30.0));
        drag_to(&mut m, middle(tile));
        assert_eq!(
            m.view(),
            View {
                shown: Some(0),
                hover: Some(THIRDS),
                chosen: Some(THIRDS),
                cell: None,
                compact: false
            }
        );
        // The cells of the layout show over the screen, none of them lit yet.
        let (area, cells) = m.overlay().expect("the cells show");
        assert_eq!(area, displays[0].area);
        assert_eq!(cells.len(), 3);
        assert_eq!(cells[0], Rect::new(0.0, 25.0, 640.0, 1055.0));
        assert_eq!(m.footprint(), None, "three cells: nothing to land in yet");
    }

    #[test]
    fn ten_layouts_fit_one_row_so_moving_down_keeps_the_first_choice() {
        let d = plain();
        let templates = templates::choose(&templates::BUILT_IN, &[]);
        let mut m = Machine::new();
        m.press(vec![d], vec![templates], (500.0, 300.0), true);
        drag_to(&mut m, (960.0, 2.0));
        let first = middle(d.cells(10)[0]);
        drag_to(&mut m, first);
        // This was the second row's first tile when ten layouts split into two rows.
        let below_picker = (first.0, d.frame.y + d.inset + CELL.1 + GAP + CELL.1 / 2.0);
        drag_to(&mut m, below_picker);
        assert_eq!(m.view().chosen, Some(0));
        assert!(m.view().compact);
        assert_eq!(
            m.release(below_picker),
            Some(Dropped {
                display: 0,
                template: 0,
                cell: 0
            })
        );
    }

    #[test]
    fn moving_down_onto_the_screen_shrinks_the_row_lights_a_cell_and_a_release_in_it_drops() {
        let (mut m, displays, _) = started();
        drag_to(&mut m, (960.0, 2.0));
        drag_to(&mut m, middle(displays[0].cells(7)[THIRDS]));
        // Down, away from the row, onto the first third of the screen.
        drag_to(&mut m, (400.0, 600.0));
        assert_eq!(
            m.view(),
            View {
                shown: Some(0),
                hover: None,
                chosen: Some(THIRDS),
                cell: Some(0),
                compact: true
            }
        );
        assert_eq!(m.footprint(), Some(Rect::new(0.0, 25.0, 640.0, 1055.0)));
        drag_to(&mut m, (1000.0, 600.0));
        assert_eq!(m.view().cell, Some(1));
        assert_eq!(m.footprint(), Some(Rect::new(640.0, 25.0, 640.0, 1055.0)));
        let last = (1700.0, 800.0);
        drag_to(&mut m, last);
        assert_eq!(m.view().cell, Some(2));
        assert_eq!(
            m.release(last),
            Some(Dropped {
                display: 0,
                template: THIRDS,
                cell: 2
            })
        );
        assert!(m.is_idle());
        assert_eq!(m.view(), View::default());
        assert_eq!(
            (m.footprint(), m.overlay()),
            (None, None),
            "gone once the drag ends"
        );
    }

    #[test]
    fn the_way_back_to_the_top_brings_the_row_back_and_another_layout_can_be_chosen() {
        let (mut m, displays, _) = started();
        drag_to(&mut m, (960.0, 2.0));
        drag_to(&mut m, middle(displays[0].cells(7)[THIRDS]));
        drag_to(&mut m, (400.0, 600.0));
        assert!(m.view().compact);
        // The pointer comes back to the top edge: the row is whole again, with the last choice still showing.
        drag_to(&mut m, (960.0, 2.0));
        assert_eq!(
            m.view(),
            View {
                shown: Some(0),
                hover: None,
                chosen: Some(THIRDS),
                cell: None,
                compact: false
            }
        );
        assert_eq!(m.overlay().map(|o| o.1.len()), Some(3));
        // Another layout takes its place, and the pointer goes down again into one of its cells.
        drag_to(&mut m, middle(displays[0].cells(7)[QUARTERS]));
        assert_eq!(m.view().chosen, Some(QUARTERS));
        assert_eq!(m.overlay().map(|o| o.1.len()), Some(4));
        drag_to(&mut m, (300.0, 500.0));
        assert_eq!((m.view().cell, m.view().compact), (Some(0), true));
        assert_eq!(
            m.release((300.0, 500.0)),
            Some(Dropped {
                display: 0,
                template: QUARTERS,
                cell: 0
            })
        );
    }

    #[test]
    fn the_chip_is_also_the_way_back_to_the_row() {
        let (mut m, displays, _) = started();
        drag_to(&mut m, (960.0, 2.0));
        drag_to(&mut m, middle(displays[0].cells(7)[HALVES]));
        drag_to(&mut m, (400.0, 600.0));
        assert!(m.view().compact);
        let chip = displays[0].chip();
        drag_to(
            &mut m,
            (chip.x + chip.w / 2.0, bottom(chip) + STAY_SLACK - 1.0),
        );
        assert!(!m.view().compact, "the pointer reached the chip");
        drag_to(&mut m, (400.0, 600.0));
        assert!(m.view().compact);
        // Away from the chip and the top edge, on the screen, the row stays small.
        drag_to(
            &mut m,
            (chip.x + chip.w / 2.0, bottom(chip) + STAY_SLACK + 20.0),
        );
        assert!(m.view().compact);
    }

    #[test]
    fn a_release_on_the_row_drops_only_for_a_layout_of_one_cell() {
        let (mut m, displays, _) = started();
        drag_to(&mut m, (960.0, 2.0));
        let on_halves = middle(displays[0].cells(7)[HALVES]);
        drag_to(&mut m, on_halves);
        assert_eq!(
            m.release(on_halves),
            None,
            "two cells: there is nothing to say which"
        );

        let (mut m, displays, _) = started();
        drag_to(&mut m, (960.0, 2.0));
        let on_full = middle(displays[0].cells(7)[FULL]);
        drag_to(&mut m, on_full);
        assert_eq!(
            m.footprint(),
            Some(displays[0].area),
            "the whole area is where it would go"
        );
        assert_eq!(
            m.release(on_full),
            Some(Dropped {
                display: 0,
                template: FULL,
                cell: 0
            })
        );
    }

    #[test]
    fn leaving_without_a_layout_hides_the_row_and_a_release_in_a_cell_needs_one() {
        let (mut m, displays, _) = started();
        drag_to(&mut m, (960.0, 1.0));
        // Down onto the screen without having been on a layout: the row goes, and no cells show.
        drag_to(&mut m, (400.0, 600.0));
        assert_eq!(m.view(), View::default());
        assert_eq!(m.release((400.0, 600.0)), None);

        let (mut m, displays_again, _) = started();
        assert_eq!(displays, displays_again);
        drag_to(&mut m, (960.0, 1.0));
        drag_to(&mut m, middle(displays[0].cells(7)[THIRDS]));
        drag_to(&mut m, (400.0, 600.0));
        assert!(m.view().cell.is_some());
        // The menu bar is not a cell: the pointer there, away from the chip, ends it all.
        drag_to(&mut m, (100.0, 10.0));
        assert_eq!(m.view(), View::default());
        assert_eq!((m.footprint(), m.overlay()), (None, None));
        assert_eq!(
            m.release((100.0, 10.0)),
            None,
            "released elsewhere: cancelled"
        );
        assert!(m.is_idle());
    }

    #[test]
    fn the_row_follows_the_pointer_to_another_display_and_starts_over() {
        let (mut m, displays, _) = started();
        drag_to(&mut m, (960.0, 0.0));
        drag_to(&mut m, middle(displays[0].cells(7)[THIRDS]));
        assert_eq!(m.view().chosen, Some(THIRDS));
        drag_to(&mut m, (-756.0, -982.0));
        assert_eq!(
            m.view(),
            View {
                shown: Some(1),
                ..View::default()
            },
            "another screen: its own row, nothing chosen"
        );
        let tile = displays[1].cells(7)[FULL];
        drag_to(&mut m, middle(tile));
        assert_eq!(
            m.release(middle(tile)),
            Some(Dropped {
                display: 1,
                template: FULL,
                cell: 0
            })
        );
    }

    #[test]
    fn each_display_offers_its_own_layouts() {
        let displays = vec![plain(), big()];
        let offers = vec![row(&["quarters"]), row(&["thirds", "halves", "full"])];
        let mut m = Machine::new();
        m.press(displays.clone(), offers, (500.0, 300.0), true);
        drag_to(&mut m, (960.0, 0.0));
        assert_eq!(m.view().shown, Some(0));
        // The row of the first screen has one layout, so the chip and the row are about the same size there.
        drag_to(&mut m, middle(displays[0].cells(1)[0]));
        assert_eq!(m.view().chosen, Some(0));
        assert_eq!(m.overlay().map(|o| o.1.len()), Some(4), "the quarters");
        drag_to(&mut m, (1920.0 + 1280.0, -200.0));
        assert_eq!(m.view().shown, Some(1));
        drag_to(&mut m, middle(displays[1].cells(3)[1]));
        assert_eq!(
            m.overlay().map(|o| o.1.len()),
            Some(2),
            "the halves, the second layout of that screen"
        );
    }

    #[test]
    fn the_row_stays_a_little_beyond_its_edge() {
        let (mut m, displays, _) = started();
        drag_to(&mut m, (960.0, 0.0));
        let panel = displays[0].panel(7);
        drag_to(&mut m, (panel.x - STAY_SLACK + 1.0, 30.0));
        assert_eq!(m.view().shown, Some(0));
        drag_to(&mut m, (panel.x - STAY_SLACK - 1.0, 30.0));
        assert_eq!(m.view().shown, None);
    }

    #[test]
    fn a_release_while_still_waiting_for_the_window_to_move_drops_nothing() {
        let (mut m, ..) = started();
        m.drag((500.0, 320.0), || false);
        assert_eq!(m.release((960.0, 0.0)), None);
        assert!(m.is_idle());
    }

    #[test]
    fn a_press_that_did_not_start_on_a_window_is_ignored_throughout() {
        let mut m = Machine::new();
        // No window under the pointer, a press over a control, or Accessibility not granted: the platform
        // reports the same thing, and the machine never leaves rest.
        m.press(vec![plain()], vec![offered()], (500.0, 300.0), false);
        assert!(m.is_idle());
        let mut asked = false;
        m.drag((960.0, 0.0), || {
            asked = true;
            true
        });
        assert!(!asked && m.view() == View::default());
        assert_eq!(m.release((960.0, 0.0)), None);
    }

    #[test]
    fn a_press_without_any_display_or_with_layouts_missing_for_one_is_ignored() {
        let mut m = Machine::new();
        m.press(Vec::new(), Vec::new(), (0.0, 0.0), true);
        assert!(m.is_idle());
        m.press(vec![plain(), big()], vec![offered()], (0.0, 0.0), true);
        assert!(m.is_idle(), "one list for two screens");
    }

    #[test]
    fn a_drag_that_never_moves_a_window_is_given_up() {
        // Selecting text in a title: the pointer travels but the window stays.
        let (mut m, ..) = started();
        let mut looks = 0;
        for i in 0..40 {
            m.drag((500.0 + f64::from(i), 300.0), || {
                looks += 1;
                false
            });
        }
        assert!(m.is_idle(), "gave up");
        assert!(looks <= usize::from(MAX_CHECKS), "looked {looks} times");
        // Reaching the top afterwards shows nothing.
        m.drag((960.0, 0.0), || true);
        assert_eq!(m.view(), View::default());
    }

    #[test]
    fn a_tiny_wobble_is_not_a_drag() {
        let (mut m, ..) = started();
        let mut asked = false;
        m.drag((501.0, 301.0), || {
            asked = true;
            true
        });
        assert!(
            !asked,
            "the window is not looked at before the pointer has really moved"
        );
        assert!(!m.is_idle(), "but the press is not forgotten");
    }

    #[test]
    fn the_window_is_looked_at_only_until_it_is_known_to_move() {
        let (mut m, ..) = started();
        let mut looks = 0;
        m.drag((520.0, 300.0), || {
            looks += 1;
            false
        });
        m.drag((540.0, 300.0), || {
            looks += 1;
            true
        });
        for x in 0..20 {
            m.drag((560.0 + f64::from(x), 300.0), || {
                looks += 1;
                true
            });
        }
        assert_eq!(looks, 2);
    }

    #[test]
    fn a_new_press_or_a_cancel_forgets_the_drag() {
        let (mut m, ..) = started();
        drag_to(&mut m, (960.0, 0.0));
        assert_eq!(m.view().shown, Some(0));
        m.cancel();
        assert!(m.is_idle() && m.view() == View::default());
        drag_to(&mut m, (960.0, 0.0));
        assert_eq!(m.view(), View::default());

        let (mut m, ..) = started();
        drag_to(&mut m, (960.0, 0.0));
        m.press(vec![plain()], vec![offered()], (10.0, 10.0), false);
        assert!(m.is_idle() && m.view() == View::default());
    }

    #[test]
    fn a_release_needs_the_row_to_have_been_reached_through_the_top_edge() {
        let (mut m, displays, _) = started();
        let tile = displays[0].cells(7)[FULL];
        let on_tile = (tile.x + 4.0, tile.y + 4.0);
        // The window followed the pointer, but the row never appeared: a jump straight onto its spot is no drop.
        drag_to(&mut m, (tile.x, 200.0));
        assert_eq!(m.release(on_tile), None);

        // Once shown, a quick release on a layout of one cell with no drag event in between still counts.
        let (mut m, ..) = started();
        drag_to(&mut m, (960.0, 0.0));
        assert_eq!(
            m.release(on_tile),
            Some(Dropped {
                display: 0,
                template: FULL,
                cell: 0
            })
        );
    }

    #[test]
    fn a_title_bar_press_is_the_top_strip_except_over_a_control() {
        let w = Rect::new(100.0, 200.0, 800.0, 600.0);
        assert!(is_title_bar_press(w, (300.0, 210.0), "AXWindow"));
        assert!(is_title_bar_press(w, (300.0, 240.0), "AXStaticText"));
        assert!(is_title_bar_press(w, (300.0, 251.9), "AXToolbar"));
        // A browser's tab strip is its title bar.
        assert!(is_title_bar_press(w, (300.0, 220.0), "AXTabGroup"));
        assert!(is_title_bar_press(w, (300.0, 220.0), "AXRadioButton"));
        assert!(
            !is_title_bar_press(w, (300.0, 252.0), "AXWindow"),
            "below the strip"
        );
        assert!(
            !is_title_bar_press(w, (300.0, 199.0), "AXWindow"),
            "above the window"
        );
        assert!(
            !is_title_bar_press(w, (99.0, 210.0), "AXWindow"),
            "left of the window"
        );
        assert!(
            !is_title_bar_press(w, (901.0, 210.0), "AXWindow"),
            "right of the window"
        );
        for role in [
            "AXButton",
            "AXTextField",
            "AXPopUpButton",
            "AXRadioGroup",
            "AXSlider",
        ] {
            assert!(!is_title_bar_press(w, (300.0, 210.0), role), "{role}");
        }
    }
}
