//! Dragging a window onto the notch: where the drop zones appear and the state machine of one drag.
//! Everything is in the coordinates of `Window` frames and the Accessibility API: the origin at the top-left
//! corner of the primary screen, y growing downwards, so a screen above or left of it has negative values.
//! The platform layer converts pointer positions to these coordinates and reports what it can see; nothing here
//! touches the system, so every rule is covered by plain tests.

use crate::{Layout, Rect};

pub type Point = (f64, f64);

/// Layouts a drop zone can apply, in the order the row shows them. Centering is left out: it keeps the size,
/// which a drop onto a row of shapes cannot express.
pub const OFFERED: [Layout; 12] = [
    Layout::Left,
    Layout::Right,
    Layout::FirstThird,
    Layout::CenterThird,
    Layout::LastThird,
    Layout::Maximize,
    Layout::Top,
    Layout::Bottom,
    Layout::TopLeft,
    Layout::TopRight,
    Layout::BottomLeft,
    Layout::BottomRight,
];

pub const DEFAULT_ZONES: [Layout; 6] = [
    Layout::Left,
    Layout::Right,
    Layout::FirstThird,
    Layout::CenterThird,
    Layout::LastThird,
    Layout::Maximize,
];

/// More shapes than this would make the row wider than the open notch.
pub const MAX_ZONES: usize = 8;

pub const CELL: (f64, f64) = (64.0, 44.0);
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

/// The zones a list of layout names asks for: unknown and repeated names dropped, the order of `OFFERED`, at most
/// `MAX_ZONES`, and the default row when nothing valid is left.
pub fn choose<S: AsRef<str>>(keys: &[S]) -> Vec<Layout> {
    let wanted: Vec<Layout> = keys
        .iter()
        .filter_map(|k| layout_from_key(k.as_ref()))
        .collect();
    let chosen: Vec<Layout> = OFFERED
        .into_iter()
        .filter(|l| wanted.contains(l))
        .take(MAX_ZONES)
        .collect();
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

    /// The shape that hangs from the top edge and holds `count` zones.
    pub fn panel(&self, count: usize) -> Rect {
        let n = count as f64;
        let row = n * CELL.0 + (n - 1.0).max(0.0) * GAP + 2.0 * PAD;
        // Wide enough to cover the cutout, so on a real notch the shape grows out of it.
        let w = row.max(self.cutout + 2.0 * PAD);
        Rect::new(
            self.frame.x + (self.frame.w - w) / 2.0,
            self.frame.y,
            w,
            self.inset + CELL.1 + PAD,
        )
    }

    /// The drawn shape of each zone, left to right.
    pub fn cells(&self, count: usize) -> Vec<Rect> {
        let panel = self.panel(count);
        let n = count as f64;
        let row = n * CELL.0 + (n - 1.0).max(0.0) * GAP;
        let left = panel.x + (panel.w - row) / 2.0;
        (0..count)
            .map(|i| {
                Rect::new(
                    left + i as f64 * (CELL.0 + GAP),
                    self.frame.y + self.inset,
                    CELL.0,
                    CELL.1,
                )
            })
            .collect()
    }

    /// The zone under the pointer. A zone owns the gaps beside it and the padding at the ends of the row, so
    /// moving along the row never passes over a dead spot; the strip above the shapes, which the trigger band
    /// overlaps on a screen without a notch, belongs to none.
    pub fn zone_at(&self, count: usize, p: Point) -> Option<usize> {
        let (panel, cells) = (self.panel(count), self.cells(count));
        let top = self.frame.y + self.inset;
        if p.1 < top || p.1 > bottom(panel) {
            return None;
        }
        let last = cells.len().saturating_sub(1);
        cells.iter().enumerate().position(|(i, c)| {
            let from = if i == 0 { panel.x } else { c.x - GAP / 2.0 };
            let to = if i == last {
                right(panel)
            } else {
                right(*c) + GAP / 2.0
            };
            p.0 >= from && (p.0 < to || (i == last && p.0 <= to))
        })
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

/// Roles that do something of their own when pressed; a drag that starts on one is not a window drag.
const INTERACTIVE: [&str; 17] = [
    "AXButton",
    "AXCheckBox",
    "AXRadioButton",
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
    "AXTabGroup",
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
    zones: usize,
    displays: Vec<Display>,
    phase: Phase,
    view: View,
}

impl Machine {
    pub fn new(zones: usize) -> Self {
        Self {
            zones,
            displays: Vec::new(),
            phase: Phase::Idle,
            view: View::default(),
        }
    }

    pub fn set_zones(&mut self, zones: usize) {
        self.zones = zones;
        self.cancel();
    }

    pub fn view(&self) -> View {
        self.view
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
                || contains(inflate(displays[d].panel(self.zones), STAY_SLACK), at)
        });
        self.view.shown = stays.or(entered);
        self.view.hover = self
            .view
            .shown
            .and_then(|d| displays[d].zone_at(self.zones, at));
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

    #[test]
    fn the_row_is_centred_and_hangs_from_the_top_of_a_notchless_screen() {
        let d = plain();
        let panel = d.panel(6);
        assert!(near(panel.x + panel.w / 2.0, 960.0));
        assert_eq!(panel.y, 0.0);
        assert!(near(panel.w, 6.0 * 64.0 + 5.0 * 8.0 + 24.0));
        assert!(near(panel.h, 10.0 + 44.0 + 12.0));
        let cells = d.cells(6);
        assert_eq!(cells.len(), 6);
        assert!(near(cells[0].x - panel.x, 12.0), "padding at the left");
        assert!(
            near(right(panel) - right(cells[5]), 12.0),
            "and at the right"
        );
        assert!(near(cells[1].x - right(cells[0]), GAP));
        assert!(cells.iter().all(|c| c.y == 10.0 && c.h == 44.0));
    }

    #[test]
    fn on_a_notched_screen_the_row_clears_the_cutout_and_covers_it() {
        let d = laptop();
        let panel = d.panel(2);
        // Two zones are narrower than the cutout, so the shape is widened to cover it.
        assert!(near(panel.w, 185.0 + 24.0));
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
            "a single zone is centred"
        );
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
        let d = plain();
        let cells = d.cells(6);
        for (i, c) in cells.iter().enumerate() {
            let middle = (c.x + c.w / 2.0, c.y + c.h / 2.0);
            assert_eq!(d.zone_at(6, middle), Some(i));
        }
        // The gap between two cells belongs to one of them, never to none.
        for pair in cells.windows(2) {
            let gap_middle = (right(pair[0]) + GAP / 2.0 - 0.01, pair[0].y + 10.0);
            assert!(d.zone_at(6, gap_middle).is_some());
        }
        // Padding at the ends belongs to the first and last zone.
        let panel = d.panel(6);
        assert_eq!(d.zone_at(6, (panel.x + 2.0, cells[0].y + 5.0)), Some(0));
        assert_eq!(d.zone_at(6, (right(panel), cells[0].y + 5.0)), Some(5));
        // Above the shapes, below the shape and beside it: none.
        assert_eq!(d.zone_at(6, (cells[2].x + 5.0, 0.0)), None);
        assert_eq!(d.zone_at(6, (cells[2].x + 5.0, bottom(panel) + 1.0)), None);
        assert_eq!(d.zone_at(6, (panel.x - 1.0, cells[0].y + 5.0)), None);
        assert_eq!(d.zone_at(6, (right(panel) + 1.0, cells[0].y + 5.0)), None);
        assert_eq!(d.zone_at(0, (960.0, 30.0)), None, "no zones, no hit");
    }

    #[test]
    fn zones_work_on_every_display_with_its_own_size_and_origin() {
        let displays = [plain(), laptop(), big()];
        for (i, d) in displays.iter().enumerate() {
            let c = d.cells(6)[3];
            let p = (c.x + c.w / 2.0, c.y + c.h / 2.0);
            assert_eq!(display_at(&displays, p), Some(i), "display {i}");
            assert_eq!(d.zone_at(6, p), Some(3));
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
    fn choosing_zones_drops_the_unknown_orders_and_caps() {
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
        assert_eq!(choose(&all).len(), MAX_ZONES);
        assert!(choose(&all).iter().all(|l| OFFERED.contains(l)));
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
    fn a_title_bar_press_is_the_top_strip_except_over_a_control() {
        let w = Rect::new(100.0, 200.0, 800.0, 600.0);
        assert!(is_title_bar_press(w, (300.0, 210.0), "AXWindow"));
        assert!(is_title_bar_press(w, (300.0, 240.0), "AXStaticText"));
        assert!(is_title_bar_press(w, (300.0, 251.9), "AXToolbar"));
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

    fn started(zones: usize) -> (Machine, Vec<Display>) {
        let displays = vec![plain(), laptop(), big()];
        let mut m = Machine::new(zones);
        m.press(displays.clone(), (500.0, 300.0), true);
        (m, displays)
    }

    #[test]
    fn a_drag_to_the_top_shows_the_zones_and_a_release_on_a_zone_drops() {
        let (mut m, displays) = started(6);
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
        let cell = displays[0].cells(6)[4];
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
        let (mut m, displays) = started(6);
        drag_to(&mut m, (960.0, 1.0));
        let cell = displays[0].cells(6)[1];
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
        // Down and away from the shape: hidden.
        drag_to(&mut m, (cell.x + 5.0, 400.0));
        assert_eq!(m.view(), View::default());
        assert_eq!(
            m.release((cell.x + 5.0, 400.0)),
            None,
            "released elsewhere: cancelled"
        );
        assert!(m.is_idle());
    }

    #[test]
    fn the_zones_follow_the_pointer_to_another_display() {
        let (mut m, displays) = started(6);
        drag_to(&mut m, (960.0, 0.0));
        assert_eq!(m.view().shown, Some(0));
        drag_to(&mut m, (-756.0, -982.0));
        assert_eq!(m.view().shown, Some(1));
        let cell = displays[1].cells(6)[0];
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
        let (mut m, displays) = started(6);
        drag_to(&mut m, (960.0, 0.0));
        let panel = displays[0].panel(6);
        drag_to(&mut m, (panel.x - STAY_SLACK + 1.0, 30.0));
        assert_eq!(m.view().shown, Some(0));
        drag_to(&mut m, (panel.x - STAY_SLACK - 1.0, 30.0));
        assert_eq!(m.view().shown, None);
    }

    #[test]
    fn a_release_while_still_waiting_for_the_window_to_move_drops_nothing() {
        let (mut m, _) = started(6);
        m.drag((500.0, 320.0), || false);
        assert_eq!(m.release((960.0, 0.0)), None);
        assert!(m.is_idle());
    }

    #[test]
    fn a_press_that_did_not_start_on_a_window_is_ignored_throughout() {
        let mut m = Machine::new(6);
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
        let mut m = Machine::new(6);
        m.press(Vec::new(), (0.0, 0.0), true);
        assert!(m.is_idle());
    }

    #[test]
    fn a_drag_that_never_moves_a_window_is_given_up() {
        // Selecting text in a title: the pointer travels but the window stays.
        let (mut m, _) = started(6);
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
        let (mut m, _) = started(6);
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
        let (mut m, _) = started(6);
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
        let (mut m, _) = started(6);
        drag_to(&mut m, (960.0, 0.0));
        assert_eq!(m.view().shown, Some(0));
        m.cancel();
        assert!(m.is_idle() && m.view() == View::default());
        drag_to(&mut m, (960.0, 0.0));
        assert_eq!(m.view(), View::default());

        let (mut m, _) = started(6);
        drag_to(&mut m, (960.0, 0.0));
        m.press(vec![plain()], (10.0, 10.0), false);
        assert!(m.is_idle() && m.view() == View::default());
    }

    #[test]
    fn changing_the_zones_in_the_middle_of_a_drag_cancels_it() {
        let (mut m, _) = started(6);
        drag_to(&mut m, (960.0, 0.0));
        m.set_zones(3);
        assert!(m.is_idle());
    }

    #[test]
    fn a_release_needs_the_zones_to_have_been_reached_through_the_top_edge() {
        let (mut m, displays) = started(6);
        let cell = displays[0].cells(6)[0];
        let on_cell = (cell.x + 4.0, cell.y + 4.0);
        // The window followed the pointer, but the shape never appeared: a jump straight onto its spot is no drop.
        drag_to(&mut m, (cell.x, 200.0));
        assert_eq!(m.release(on_cell), None);

        // Once shown, a quick release on a zone with no drag event in between still counts.
        let (mut m, _) = started(6);
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
