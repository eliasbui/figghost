//! Dragging a window onto the notch: where the drop zones appear, where the window would land, and the state
//! machine of one drag.
//! Everything is in the coordinates of `Window` frames and the Accessibility API: the origin at the top-left
//! corner of the primary screen, y growing downwards, so a screen above or left of it has negative values.
//! The platform layer converts pointer positions to these coordinates and reports what it can see; nothing here
//! touches the system, so every rule is covered by plain tests.

use crate::{Layout, Rect};

pub type Point = (f64, f64);

/// Layouts a drop zone can apply, in the order the zones show them: first those that take the whole height of the
/// screen, then those that take half of it. Centering is left out: it keeps the size, which a drop onto a shape
/// cannot express.
pub const OFFERED: [Layout; 12] = [
    Layout::Left,
    Layout::Right,
    Layout::Maximize,
    Layout::FirstThird,
    Layout::CenterThird,
    Layout::LastThird,
    Layout::Top,
    Layout::Bottom,
    Layout::TopLeft,
    Layout::TopRight,
    Layout::BottomLeft,
    Layout::BottomRight,
];

/// Halves, the whole screen and thirds on the first row; the four quarters on the second.
pub const DEFAULT_ZONES: [Layout; 10] = [
    Layout::Left,
    Layout::Right,
    Layout::Maximize,
    Layout::FirstThird,
    Layout::CenterThird,
    Layout::LastThird,
    Layout::TopLeft,
    Layout::TopRight,
    Layout::BottomLeft,
    Layout::BottomRight,
];

/// Every offered layout fits: each row holds one kind, and there are six of each.
pub const MAX_ZONES: usize = OFFERED.len();

/// One zone: a small screen with the part the layout takes, and its name under it.
pub const CELL: (f64, f64) = (84.0, 66.0);
pub const GAP: f64 = 8.0;
pub const PAD: f64 = 12.0;
/// How far from the top edge the pointer must be for the zones to appear on a screen without a notch; with one the
/// band is as tall as the cutout. Never less than the band that opens the notch, so the zones are always up first.
pub const TRIGGER_HEIGHT: f64 = 8.0;
/// What `Display::inset` holds beyond the height of the cutout: the gap between the cutout and the shapes.
const CLEARANCE: f64 = 6.0;
/// The trigger reaches this far past each side of the notch, so the pointer need not hit the cutout exactly.
pub const TRIGGER_SLACK: f64 = 48.0;
/// Once shown, the zones stay while the pointer is this close to them, so a slightly wild hand does not hide them.
pub const STAY_SLACK: f64 = 8.0;
/// A press this far below the top of a window still counts as on its title bar: toolbars share the title bar.
pub const TITLE_BAR_HEIGHT: f64 = 52.0;
/// The pointer must travel this far before a press is looked at as the start of a drag.
const DRAG_SLOP: f64 = 4.0;
/// A press that has not moved a window after this many drag events, or this far, was not on a window.
const MAX_CHECKS: u8 = 12;
const GIVE_UP_DISTANCE: f64 = 120.0;

pub fn layout_from_key(key: &str) -> Option<Layout> {
    Layout::ALL.into_iter().find(|l| l.key() == key)
}

/// The zones a list of layout names asks for: unknown and repeated names dropped, the order of `OFFERED`, and the
/// default zones when nothing valid is left.
pub fn choose<S: AsRef<str>>(keys: &[S]) -> Vec<Layout> {
    let wanted: Vec<Layout> = keys
        .iter()
        .filter_map(|k| layout_from_key(k.as_ref()))
        .collect();
    let chosen: Vec<Layout> = OFFERED.into_iter().filter(|l| wanted.contains(l)).collect();
    if chosen.is_empty() {
        DEFAULT_ZONES.to_vec()
    } else {
        chosen
    }
}

/// Where a layout puts the window, as fractions of the screen area, for drawing the shape of a zone.
pub fn glyph(layout: Layout) -> Rect {
    let whole = Rect::new(0.0, 0.0, 360.0, 240.0);
    let r = layout.place(whole, whole);
    Rect::new(r.x / 360.0, r.y / 240.0, r.w / 360.0, r.h / 240.0)
}

/// Each zone's row, its place in that row and the length of the row. A layout that takes the whole height of the
/// screen goes on the first row and one that takes half of it on the second, so each row reads as one kind of
/// split whatever was chosen; with only one kind there is one row.
fn grid(layouts: &[Layout]) -> Vec<(usize, usize, usize)> {
    let tall = |l: &Layout| glyph(*l).h > 0.75;
    let first = layouts.iter().filter(|l| tall(l)).count();
    let split = first > 0 && first < layouts.len();
    let lengths = if split {
        [first, layouts.len() - first]
    } else {
        [layouts.len(), 0]
    };
    let mut next = [0, 0];
    layouts
        .iter()
        .map(|l| {
            let row = usize::from(split && !tall(l));
            next[row] += 1;
            (row, next[row] - 1, lengths[row])
        })
        .collect()
}

/// One screen as the drag sees it.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Display {
    pub frame: Rect,
    /// The usable area: below the menu bar and beside the Dock. Layouts are applied inside it.
    pub area: Rect,
    /// Width of the notch cutout, or of the pill the notch shows on a screen without one.
    pub cutout: f64,
    /// Height the zones must leave free at the top so they clear the cutout.
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
    /// The band along the top edge, centred on the notch, where the pointer makes the zones appear.
    pub fn trigger(&self) -> Rect {
        let w = self.cutout + 2.0 * TRIGGER_SLACK;
        Rect::new(
            self.frame.x + (self.frame.w - w) / 2.0,
            self.frame.y,
            w,
            TRIGGER_HEIGHT.max(self.inset - CLEARANCE),
        )
    }

    /// The shape that hangs from the top edge and holds the zones of `layouts`.
    pub fn panel(&self, layouts: &[Layout]) -> Rect {
        let grid = grid(layouts);
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

    /// The drawn shape of each zone, in the order of `layouts`; each row is centred in the shape.
    pub fn cells(&self, layouts: &[Layout]) -> Vec<Rect> {
        let panel = self.panel(layouts);
        grid(layouts)
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

    /// The zone under the pointer. A zone owns half of each gap beside it, the end of its row up to the edge of the
    /// shape, and on the last row the padding below, so moving over the shape never passes over a dead spot; the
    /// strip above the shapes, which the trigger band overlaps on a screen without a notch, belongs to none.
    pub fn zone_at(&self, layouts: &[Layout], p: Point) -> Option<usize> {
        let panel = self.panel(layouts);
        let top = self.frame.y + self.inset;
        if p.1 < top || p.1 > bottom(panel) || p.0 < panel.x || p.0 > right(panel) {
            return None;
        }
        let (grid, cells) = (grid(layouts), self.cells(layouts));
        let two_rows = grid.iter().any(|g| g.0 == 1);
        let row = usize::from(two_rows && p.1 >= top + CELL.1 + GAP / 2.0);
        let in_row = || (0..cells.len()).filter(|&i| grid[i].0 == row);
        in_row()
            .find(|&i| p.0 < right(cells[i]) + GAP / 2.0)
            .or_else(|| in_row().next_back())
    }

    /// The frame `layout` gives a window currently at `current`, on this display.
    pub fn place(&self, layout: Layout, current: Rect) -> Rect {
        layout.place(self.area, current)
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

/// What the zones should look like now.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct View {
    /// The display whose zones are showing.
    pub shown: Option<usize>,
    /// The zone under the pointer, on that display.
    pub hover: Option<usize>,
}

/// Where a window was dropped.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Dropped {
    pub display: usize,
    pub zone: usize,
}

/// One drag from the press to the release. The caller reports what it sees; the machine decides what shows.
pub struct Machine {
    zones: Vec<Layout>,
    displays: Vec<Display>,
    phase: Phase,
    view: View,
}

impl Machine {
    pub fn new(zones: Vec<Layout>) -> Self {
        Self {
            zones,
            displays: Vec::new(),
            phase: Phase::Idle,
            view: View::default(),
        }
    }

    pub fn set_zones(&mut self, zones: Vec<Layout>) {
        self.zones = zones;
        self.cancel();
    }

    pub fn view(&self) -> View {
        self.view
    }

    /// Where the window lands if it is dropped now: the lit zone's layout inside the usable area of the display
    /// that shows the zones, the frame a drop applies. `None` while no zone is lit.
    pub fn footprint(&self) -> Option<Rect> {
        let display = self.displays.get(self.view.shown?)?;
        let layout = *self.zones.get(self.view.hover?)?;
        // Every offered layout takes a fixed part of the area whatever the window's size, so the area stands in
        // for the window.
        Some(display.place(layout, display.area))
    }

    pub fn is_idle(&self) -> bool {
        self.phase == Phase::Idle
    }

    /// The left button went down at `at`. `on_title_bar` is true only when the platform found a window there
    /// and the press was on its title bar; without Accessibility it is always false and nothing happens.
    pub fn press(&mut self, displays: Vec<Display>, at: Point, on_title_bar: bool) {
        self.cancel();
        if on_title_bar && !displays.is_empty() {
            self.displays = displays;
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

    /// The button came up. Returns the zone the window was dropped on, if it was dropped on one.
    pub fn release(&mut self, at: Point) -> Option<Dropped> {
        let dropped = if self.phase == Phase::Dragging {
            self.update(at);
            self.view
                .shown
                .zip(self.view.hover)
                .map(|(display, zone)| Dropped { display, zone })
        } else {
            None
        };
        self.cancel();
        dropped
    }

    /// Forgets the drag: the button was released unseen, a screen went away, the feature was turned off.
    pub fn cancel(&mut self) {
        self.phase = Phase::Idle;
        self.view = View::default();
    }

    fn update(&mut self, at: Point) {
        let displays = &self.displays;
        let entered = display_at(displays, at).filter(|&d| contains(displays[d].trigger(), at));
        let stays = self.view.shown.filter(|&d| {
            contains(displays[d].trigger(), at)
                || contains(inflate(displays[d].panel(&self.zones), STAY_SLACK), at)
        });
        self.view.shown = stays.or(entered);
        self.view.hover = self
            .view
            .shown
            .and_then(|d| displays[d].zone_at(&self.zones, at));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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

    /// The layouts that take the whole height of the screen: one row of six.
    fn six() -> Vec<Layout> {
        OFFERED[..6].to_vec()
    }

    fn middle(c: Rect) -> Point {
        (c.x + c.w / 2.0, c.y + c.h / 2.0)
    }

    #[test]
    fn the_row_is_centred_and_hangs_from_the_top_of_a_notchless_screen() {
        let d = plain();
        let panel = d.panel(&six());
        assert!(near(panel.x + panel.w / 2.0, 960.0));
        assert_eq!(panel.y, 0.0);
        assert!(near(panel.w, 6.0 * CELL.0 + 5.0 * GAP + 2.0 * PAD));
        assert!(near(panel.h, 10.0 + CELL.1 + PAD));
        let cells = d.cells(&six());
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
        let two = [Layout::Left, Layout::Right];
        let panel = d.panel(&two);
        // Two zones are narrower than the cutout, so the shape is widened to cover it.
        assert!(near(panel.w, 185.0 + 2.0 * PAD));
        assert!(near(panel.x + panel.w / 2.0, -1512.0 / 2.0));
        assert_eq!(panel.y, -982.0, "negative origins are kept");
        let cells = d.cells(&two);
        assert!(cells.iter().all(|c| near(c.y, -982.0 + 38.0)));
        assert!(
            cells[0].x >= panel.x && right(cells[1]) <= right(panel),
            "the cells stay inside the shape"
        );
        let one = d.cells(&[Layout::Maximize])[0];
        assert!(
            near(one.x + one.w / 2.0, -756.0),
            "a single zone is centred"
        );
    }

    #[test]
    fn the_default_zones_hang_in_two_rows_of_one_kind_each() {
        let d = laptop();
        let panel = d.panel(&DEFAULT_ZONES);
        assert!(near(panel.w, 6.0 * CELL.0 + 5.0 * GAP + 2.0 * PAD));
        assert!(near(panel.h, 38.0 + 2.0 * CELL.1 + GAP + PAD));
        let cells = d.cells(&DEFAULT_ZONES);
        let (first, second) = cells.split_at(6);
        assert!(
            first.iter().all(|c| near(c.y, -982.0 + 38.0)),
            "halves, full and thirds first"
        );
        assert!(
            second
                .iter()
                .all(|c| near(c.y, -982.0 + 38.0 + CELL.1 + GAP)),
            "then the quarters"
        );
        assert!(near(second[1].x - right(second[0]), GAP));
        let centre = |row: &[Rect]| (row[0].x + right(row[row.len() - 1])) / 2.0;
        assert!(
            near(centre(first), -756.0) && near(centre(second), -756.0),
            "each row is centred"
        );
        assert!(
            near(bottom(panel) - bottom(second[0]), PAD),
            "padding below the last row"
        );
    }

    #[test]
    fn each_zone_goes_on_the_row_of_its_kind_whatever_the_order() {
        let d = plain();
        let mixed = [
            Layout::TopLeft,
            Layout::Left,
            Layout::Bottom,
            Layout::Maximize,
        ];
        let cells = d.cells(&mixed);
        let (upper, lower) = (10.0, 10.0 + CELL.1 + GAP);
        assert_eq!(
            cells.iter().map(|c| c.y).collect::<Vec<_>>(),
            [lower, upper, lower, upper]
        );
        assert!(
            cells[1].x < cells[3].x && cells[0].x < cells[2].x,
            "in the given order"
        );
        for (i, c) in cells.iter().enumerate() {
            assert_eq!(d.zone_at(&mixed, middle(*c)), Some(i), "zone {i}");
        }
        // One kind alone is one row.
        let quarters = &DEFAULT_ZONES[6..];
        assert!(d.cells(quarters).iter().all(|c| c.y == upper));
        assert!(near(d.panel(quarters).h, 10.0 + CELL.1 + PAD));
        // Every offered layout fits: two rows of six.
        let all = d.panel(&OFFERED);
        assert!(near(all.w, d.panel(&six()).w) && near(all.h, d.panel(&DEFAULT_ZONES).h));
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
    fn the_pointer_maps_to_a_zone_or_to_none() {
        let (d, six) = (plain(), six());
        let cells = d.cells(&six);
        for (i, c) in cells.iter().enumerate() {
            assert_eq!(d.zone_at(&six, middle(*c)), Some(i));
        }
        // The gap between two cells belongs to one of them, never to none.
        for pair in cells.windows(2) {
            let gap_middle = (right(pair[0]) + GAP / 2.0 - 0.01, pair[0].y + 10.0);
            assert!(d.zone_at(&six, gap_middle).is_some());
        }
        // Padding at the ends belongs to the first and last zone.
        let panel = d.panel(&six);
        assert_eq!(d.zone_at(&six, (panel.x + 2.0, cells[0].y + 5.0)), Some(0));
        assert_eq!(d.zone_at(&six, (right(panel), cells[0].y + 5.0)), Some(5));
        // Above the shapes, below the shape and beside it: none.
        assert_eq!(d.zone_at(&six, (cells[2].x + 5.0, 0.0)), None);
        assert_eq!(
            d.zone_at(&six, (cells[2].x + 5.0, bottom(panel) + 1.0)),
            None
        );
        assert_eq!(d.zone_at(&six, (panel.x - 1.0, cells[0].y + 5.0)), None);
        assert_eq!(
            d.zone_at(&six, (right(panel) + 1.0, cells[0].y + 5.0)),
            None
        );
        assert_eq!(d.zone_at(&[], (960.0, 30.0)), None, "no zones, no hit");
    }

    #[test]
    fn the_pointer_maps_to_a_zone_on_either_row_without_a_dead_spot() {
        let (d, zones) = (plain(), DEFAULT_ZONES);
        let cells = d.cells(&zones);
        for (i, c) in cells.iter().enumerate() {
            assert_eq!(d.zone_at(&zones, middle(*c)), Some(i), "zone {i}");
        }
        // The gap between the rows is split between them. The shorter row is centred, so its first zone sits under
        // the second of the row above.
        let split = bottom(cells[0]) + GAP / 2.0;
        assert_eq!(
            d.zone_at(&zones, (middle(cells[6]).0, split - 0.01)),
            Some(1)
        );
        assert_eq!(d.zone_at(&zones, (middle(cells[6]).0, split)), Some(6));
        // The shorter row reaches the edges of the shape, and the last row the padding below it.
        let panel = d.panel(&zones);
        let low = middle(cells[6]).1;
        assert_eq!(d.zone_at(&zones, (panel.x + 1.0, low)), Some(6));
        assert_eq!(d.zone_at(&zones, (right(panel) - 1.0, low)), Some(9));
        assert_eq!(
            d.zone_at(&zones, (middle(cells[8]).0, bottom(panel))),
            Some(8)
        );
        // Every point of the shape below the strip at the top belongs to a zone.
        let mut y = d.frame.y + d.inset;
        while y <= bottom(panel) {
            let mut x = panel.x;
            while x <= right(panel) {
                assert!(d.zone_at(&zones, (x, y)).is_some(), "{x} {y}");
                x += 1.5;
            }
            y += 1.5;
        }
    }

    #[test]
    fn zones_work_on_every_display_with_its_own_size_and_origin() {
        let displays = [plain(), laptop(), big()];
        for (i, d) in displays.iter().enumerate() {
            for zone in [3, 8] {
                let p = middle(d.cells(&DEFAULT_ZONES)[zone]);
                assert_eq!(display_at(&displays, p), Some(i), "display {i}");
                assert_eq!(d.zone_at(&DEFAULT_ZONES, p), Some(zone));
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
    fn a_drop_applies_the_layout_inside_the_usable_area_of_that_display() {
        let d = big();
        let current = Rect::new(2000.0, 100.0, 800.0, 600.0);
        let left = d.place(Layout::Left, current);
        assert_eq!((left.x, left.y), (1920.0, -200.0 + 25.0));
        assert_eq!((left.w, left.h), (1280.0, 1440.0 - 25.0));
        assert_eq!(d.place(Layout::Maximize, current), d.area);
        let third = d.place(Layout::LastThird, current);
        assert_eq!(right(third), right(d.area));
    }

    #[test]
    fn choosing_zones_drops_the_unknown_and_orders_them() {
        assert_eq!(choose::<&str>(&[]), DEFAULT_ZONES.to_vec());
        assert_eq!(choose(&["nonsense", ""]), DEFAULT_ZONES.to_vec());
        assert_eq!(
            choose(&["maximize", "left", "left", "bogus", "right"]),
            vec![Layout::Left, Layout::Right, Layout::Maximize]
        );
        assert_eq!(
            choose(&["center"]),
            DEFAULT_ZONES.to_vec(),
            "center is not offered"
        );
        let all: Vec<&str> = OFFERED.iter().map(|l| l.key()).collect();
        assert_eq!(choose(&all), OFFERED.to_vec());
        assert_eq!(OFFERED.len(), MAX_ZONES);
        assert!(DEFAULT_ZONES.iter().all(|l| OFFERED.contains(l)));
    }

    #[test]
    fn every_offered_layout_has_a_shape_inside_the_unit_square() {
        for layout in OFFERED {
            let g = glyph(layout);
            assert!(g.w > 0.0 && g.h > 0.0);
            assert!(g.x >= 0.0 && g.y >= 0.0 && right(g) <= 1.0 + 1e-9 && bottom(g) <= 1.0 + 1e-9);
        }
        let third = glyph(Layout::CenterThird);
        assert!(near(third.x, 1.0 / 3.0) && near(third.w, 1.0 / 3.0) && near(third.h, 1.0));
        assert_eq!(glyph(Layout::Maximize), Rect::new(0.0, 0.0, 1.0, 1.0));
    }

    #[test]
    fn an_offered_layout_lands_in_the_same_place_whatever_the_window() {
        // What the footprint relies on: it is drawn before the window's final frame is known.
        for d in [plain(), laptop(), big()] {
            for layout in OFFERED {
                for window in [
                    Rect::new(5.0, 5.0, 300.0, 200.0),
                    Rect::new(-900.0, 40.0, 3000.0, 2000.0),
                ] {
                    assert_eq!(
                        d.place(layout, window),
                        d.place(layout, d.area),
                        "{layout:?}"
                    );
                }
            }
        }
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

    // A drag as the platform reports it. The window follows the pointer, so `moved` is true from the first look.
    fn drag_to(m: &mut Machine, to: Point) {
        m.drag(to, || true);
    }

    fn started() -> (Machine, Vec<Display>) {
        let displays = vec![plain(), laptop(), big()];
        let mut m = Machine::new(DEFAULT_ZONES.to_vec());
        m.press(displays.clone(), (500.0, 300.0), true);
        (m, displays)
    }

    #[test]
    fn a_drag_to_the_top_shows_the_zones_and_a_release_on_a_zone_drops() {
        let (mut m, displays) = started();
        assert!(!m.is_idle());
        drag_to(&mut m, (520.0, 280.0));
        assert_eq!(m.view(), View::default(), "far from the top: nothing shows");
        drag_to(&mut m, (960.0, 2.0));
        assert_eq!(
            m.view(),
            View {
                shown: Some(0),
                hover: None
            },
            "the pointer at the notch"
        );
        let cell = displays[0].cells(&DEFAULT_ZONES)[4];
        let on_cell = (cell.x + 10.0, cell.y + 10.0);
        drag_to(&mut m, (960.0, 30.0));
        drag_to(&mut m, on_cell);
        assert_eq!(
            m.view(),
            View {
                shown: Some(0),
                hover: Some(4)
            }
        );
        assert_eq!(
            m.release(on_cell),
            Some(Dropped {
                display: 0,
                zone: 4
            })
        );
        assert!(m.is_idle());
        assert_eq!(m.view(), View::default());
    }

    #[test]
    fn leaving_a_zone_clears_it_and_leaving_the_shape_hides_it_and_a_release_there_cancels() {
        let (mut m, displays) = started();
        drag_to(&mut m, (960.0, 1.0));
        let cell = displays[0].cells(&DEFAULT_ZONES)[1];
        let on_cell = (cell.x + 5.0, cell.y + 5.0);
        drag_to(&mut m, on_cell);
        assert_eq!(m.view().hover, Some(1));
        // Up into the area above the shapes: still shown, no zone.
        drag_to(&mut m, (cell.x + 5.0, 0.5));
        assert_eq!(
            m.view(),
            View {
                shown: Some(0),
                hover: None
            }
        );
        drag_to(&mut m, on_cell);
        assert_eq!(m.view().hover, Some(1));
        // Down and away from the shape: hidden, and nothing shows where the window would land.
        drag_to(&mut m, (cell.x + 5.0, 400.0));
        assert_eq!(m.view(), View::default());
        assert_eq!(m.footprint(), None);
        assert_eq!(
            m.release((cell.x + 5.0, 400.0)),
            None,
            "released elsewhere: cancelled"
        );
        assert!(m.is_idle());
    }

    #[test]
    fn the_footprint_is_the_frame_the_lit_zone_gives_the_window_on_that_display() {
        let (mut m, displays) = started();
        assert_eq!(m.footprint(), None, "nothing shows yet");
        let top_centre = (1920.0 + 1280.0, -200.0);
        drag_to(&mut m, top_centre);
        assert_eq!(m.view().shown, Some(2));
        assert_eq!(m.footprint(), None, "shown, but no zone is lit");
        let cells = displays[2].cells(&DEFAULT_ZONES);
        let window = Rect::new(2100.0, 300.0, 640.0, 480.0);
        for (zone, layout) in DEFAULT_ZONES.into_iter().enumerate() {
            drag_to(&mut m, middle(cells[zone]));
            assert_eq!(m.view().hover, Some(zone));
            assert_eq!(
                m.footprint(),
                Some(displays[2].place(layout, window)),
                "{layout:?}"
            );
        }
        // Up into the strip above the shapes: still shown, nothing lit, nothing to land on.
        drag_to(&mut m, (top_centre.0, top_centre.1 + 1.0));
        assert_eq!(m.view().shown, Some(2));
        assert_eq!(m.footprint(), None);
        let quarter = middle(cells[9]);
        drag_to(&mut m, quarter);
        let shown = m.footprint();
        assert_eq!(
            m.release(quarter),
            Some(Dropped {
                display: 2,
                zone: 9
            })
        );
        assert_eq!(shown, Some(displays[2].place(Layout::BottomRight, window)));
        assert_eq!(m.footprint(), None, "gone once the drag ends");
    }

    #[test]
    fn the_zones_follow_the_pointer_to_another_display() {
        let (mut m, displays) = started();
        drag_to(&mut m, (960.0, 0.0));
        assert_eq!(m.view().shown, Some(0));
        drag_to(&mut m, (-756.0, -982.0));
        assert_eq!(m.view().shown, Some(1));
        let cell = displays[1].cells(&DEFAULT_ZONES)[0];
        let on_cell = (cell.x + 3.0, cell.y + 3.0);
        drag_to(&mut m, on_cell);
        assert_eq!(
            m.release(on_cell),
            Some(Dropped {
                display: 1,
                zone: 0
            })
        );
    }

    #[test]
    fn the_shape_stays_a_little_beyond_its_edge() {
        let (mut m, displays) = started();
        drag_to(&mut m, (960.0, 0.0));
        let panel = displays[0].panel(&DEFAULT_ZONES);
        drag_to(&mut m, (panel.x - STAY_SLACK + 1.0, 30.0));
        assert_eq!(m.view().shown, Some(0));
        drag_to(&mut m, (panel.x - STAY_SLACK - 1.0, 30.0));
        assert_eq!(m.view().shown, None);
    }

    #[test]
    fn a_release_while_still_waiting_for_the_window_to_move_drops_nothing() {
        let (mut m, _) = started();
        m.drag((500.0, 320.0), || false);
        assert_eq!(m.release((960.0, 0.0)), None);
        assert!(m.is_idle());
    }

    #[test]
    fn a_press_that_did_not_start_on_a_window_is_ignored_throughout() {
        let mut m = Machine::new(DEFAULT_ZONES.to_vec());
        // No window under the pointer, a press over a control, or Accessibility not granted: the platform
        // reports the same thing, and the machine never leaves rest.
        m.press(vec![plain()], (500.0, 300.0), false);
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
    fn a_press_without_any_display_is_ignored() {
        let mut m = Machine::new(DEFAULT_ZONES.to_vec());
        m.press(Vec::new(), (0.0, 0.0), true);
        assert!(m.is_idle());
    }

    #[test]
    fn a_drag_that_never_moves_a_window_is_given_up() {
        // Selecting text in a title: the pointer travels but the window stays.
        let (mut m, _) = started();
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
        let (mut m, _) = started();
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
        let (mut m, _) = started();
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
        let (mut m, _) = started();
        drag_to(&mut m, (960.0, 0.0));
        assert_eq!(m.view().shown, Some(0));
        m.cancel();
        assert!(m.is_idle() && m.view() == View::default());
        drag_to(&mut m, (960.0, 0.0));
        assert_eq!(m.view(), View::default());

        let (mut m, _) = started();
        drag_to(&mut m, (960.0, 0.0));
        m.press(vec![plain()], (10.0, 10.0), false);
        assert!(m.is_idle() && m.view() == View::default());
    }

    #[test]
    fn changing_the_zones_in_the_middle_of_a_drag_cancels_it() {
        let (mut m, _) = started();
        drag_to(&mut m, (960.0, 0.0));
        m.set_zones(vec![Layout::Left]);
        assert!(m.is_idle());
    }

    #[test]
    fn a_release_needs_the_zones_to_have_been_reached_through_the_top_edge() {
        let (mut m, displays) = started();
        let cell = displays[0].cells(&DEFAULT_ZONES)[0];
        let on_cell = (cell.x + 4.0, cell.y + 4.0);
        // The window followed the pointer, but the shape never appeared: a jump straight onto its spot is no drop.
        drag_to(&mut m, (cell.x, 200.0));
        assert_eq!(m.release(on_cell), None);

        // Once shown, a quick release on a zone with no drag event in between still counts.
        let (mut m, _) = started();
        drag_to(&mut m, (960.0, 0.0));
        assert_eq!(
            m.release(on_cell),
            Some(Dropped {
                display: 0,
                zone: 0
            })
        );
    }

    #[test]
    fn a_layout_name_round_trips() {
        for layout in Layout::ALL {
            assert_eq!(layout_from_key(layout.key()), Some(layout));
        }
        assert_eq!(layout_from_key("Left"), None);
    }
}
