#![forbid(unsafe_code)]
//! Window tiling geometry: where each layout puts a window inside the usable area of a screen.
//! Coordinates are whole pixels with the origin at the top-left corner, y growing downwards.

pub mod pin;
pub mod snap;
pub mod templates;

#[derive(Debug, Clone, Copy, PartialEq)]
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

    fn center(&self) -> (f64, f64) {
        (self.x + self.w / 2.0, self.y + self.h / 2.0)
    }

    fn overlap(&self, other: &Rect) -> f64 {
        let w = (self.x + self.w).min(other.x + other.w) - self.x.max(other.x);
        let h = (self.y + self.h).min(other.y + other.h) - self.y.max(other.y);
        if w > 0.0 && h > 0.0 { w * h } else { 0.0 }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Layout {
    Left,
    Right,
    Top,
    Bottom,
    TopLeft,
    TopRight,
    BottomLeft,
    BottomRight,
    Maximize,
    Center,
    FirstThird,
    CenterThird,
    LastThird,
}

impl Layout {
    pub const ALL: [Layout; 13] = [
        Layout::Left,
        Layout::Right,
        Layout::Top,
        Layout::Bottom,
        Layout::TopLeft,
        Layout::TopRight,
        Layout::BottomLeft,
        Layout::BottomRight,
        Layout::Maximize,
        Layout::Center,
        Layout::FirstThird,
        Layout::CenterThird,
        Layout::LastThird,
    ];

    /// Name used in the config file.
    pub const fn key(self) -> &'static str {
        match self {
            Layout::Left => "left",
            Layout::Right => "right",
            Layout::Top => "top",
            Layout::Bottom => "bottom",
            Layout::TopLeft => "top_left",
            Layout::TopRight => "top_right",
            Layout::BottomLeft => "bottom_left",
            Layout::BottomRight => "bottom_right",
            Layout::Maximize => "maximize",
            Layout::Center => "center",
            Layout::FirstThird => "first_third",
            Layout::CenterThird => "center_third",
            Layout::LastThird => "last_third",
        }
    }

    pub const fn default_shortcut(self) -> &'static str {
        match self {
            Layout::Left => "ctrl+opt+left",
            Layout::Right => "ctrl+opt+right",
            Layout::Top => "ctrl+opt+up",
            Layout::Bottom => "ctrl+opt+down",
            Layout::TopLeft => "ctrl+opt+u",
            Layout::TopRight => "ctrl+opt+i",
            Layout::BottomLeft => "ctrl+opt+j",
            Layout::BottomRight => "ctrl+opt+k",
            Layout::Maximize => "ctrl+opt+return",
            Layout::Center => "ctrl+opt+c",
            Layout::FirstThird => "ctrl+opt+d",
            Layout::CenterThird => "ctrl+opt+f",
            Layout::LastThird => "ctrl+opt+g",
        }
    }

    /// The frame this layout gives a window currently at `current`, inside `area`.
    pub fn place(self, area: Rect, current: Rect) -> Rect {
        let (x, w) = (area.x, area.w);
        let (y, h) = (area.y, area.h);
        match self {
            Layout::Left => slice(area, (0, 2), (0, 1)),
            Layout::Right => slice(area, (1, 2), (0, 1)),
            Layout::Top => slice(area, (0, 1), (0, 2)),
            Layout::Bottom => slice(area, (0, 1), (1, 2)),
            Layout::TopLeft => slice(area, (0, 2), (0, 2)),
            Layout::TopRight => slice(area, (1, 2), (0, 2)),
            Layout::BottomLeft => slice(area, (0, 2), (1, 2)),
            Layout::BottomRight => slice(area, (1, 2), (1, 2)),
            Layout::Maximize => area,
            Layout::FirstThird => slice(area, (0, 3), (0, 1)),
            Layout::CenterThird => slice(area, (1, 3), (0, 1)),
            Layout::LastThird => slice(area, (2, 3), (0, 1)),
            Layout::Center => {
                let (cw, ch) = (current.w.min(w), current.h.min(h));
                Rect::new(
                    (x + (w - cw) / 2.0).round(),
                    (y + (h - ch) / 2.0).round(),
                    cw,
                    ch,
                )
            }
        }
    }
}

/// Cell `(index, of)` along each axis. Edges are rounded individually, so neighbouring cells always
/// meet exactly even when the size does not divide evenly.
fn slice(area: Rect, (col, cols): (u32, u32), (row, rows): (u32, u32)) -> Rect {
    let edge = |origin: f64, total: f64, index: u32, of: u32| {
        origin + (total * f64::from(index) / f64::from(of)).round()
    };
    let (left, right) = (
        edge(area.x, area.w, col, cols),
        edge(area.x, area.w, col + 1, cols),
    );
    let (top, bottom) = (
        edge(area.y, area.h, row, rows),
        edge(area.y, area.h, row + 1, rows),
    );
    Rect::new(left, top, right - left, bottom - top)
}

/// The screen area a window belongs to: the one it overlaps most, or the nearest one when it is
/// entirely off screen.
pub fn pick_area(window: Rect, areas: &[Rect]) -> Option<Rect> {
    let by_overlap = areas
        .iter()
        .copied()
        .filter(|a| window.overlap(a) > 0.0)
        .max_by(|a, b| window.overlap(a).total_cmp(&window.overlap(b)));
    by_overlap.or_else(|| {
        let (wx, wy) = window.center();
        areas.iter().copied().min_by(|a, b| {
            let dist = |r: &Rect| {
                let (cx, cy) = r.center();
                (cx - wx).powi(2) + (cy - wy).powi(2)
            };
            dist(a).total_cmp(&dist(b))
        })
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const AREA: Rect = Rect::new(0.0, 25.0, 1921.0, 1055.0);
    const WINDOW: Rect = Rect::new(300.0, 200.0, 800.0, 600.0);

    fn place(layout: Layout) -> Rect {
        layout.place(AREA, WINDOW)
    }

    #[test]
    fn halves_meet_exactly_even_when_the_size_is_odd() {
        let (left, right) = (place(Layout::Left), place(Layout::Right));
        assert_eq!(left.x, AREA.x);
        assert_eq!(left.x + left.w, right.x);
        assert_eq!(right.x + right.w, AREA.x + AREA.w);
        assert_eq!((left.y, left.h), (AREA.y, AREA.h));
        let (top, bottom) = (place(Layout::Top), place(Layout::Bottom));
        assert_eq!(top.y + top.h, bottom.y);
        assert_eq!(bottom.y + bottom.h, AREA.y + AREA.h);
    }

    #[test]
    fn quarters_tile_the_area_without_gaps_or_overlap() {
        let quarters = [
            Layout::TopLeft,
            Layout::TopRight,
            Layout::BottomLeft,
            Layout::BottomRight,
        ]
        .map(place);
        let total: f64 = quarters.iter().map(|q| q.w * q.h).sum();
        assert_eq!(total, AREA.w * AREA.h);
        assert_eq!(quarters[0].x + quarters[0].w, quarters[1].x);
        assert_eq!(quarters[0].y + quarters[0].h, quarters[2].y);
        assert_eq!(quarters[3].x + quarters[3].w, AREA.x + AREA.w);
        assert_eq!(quarters[3].y + quarters[3].h, AREA.y + AREA.h);
    }

    #[test]
    fn thirds_tile_the_width() {
        let thirds = [Layout::FirstThird, Layout::CenterThird, Layout::LastThird].map(place);
        assert_eq!(thirds[0].x, AREA.x);
        assert_eq!(thirds[0].x + thirds[0].w, thirds[1].x);
        assert_eq!(thirds[1].x + thirds[1].w, thirds[2].x);
        assert_eq!(thirds[2].x + thirds[2].w, AREA.x + AREA.w);
        assert!(thirds.iter().all(|t| (t.y, t.h) == (AREA.y, AREA.h)));
    }

    #[test]
    fn maximize_fills_the_area_and_center_keeps_the_size() {
        assert_eq!(place(Layout::Maximize), AREA);
        let centered = place(Layout::Center);
        assert_eq!((centered.w, centered.h), (WINDOW.w, WINDOW.h));
        let (cx, cy) = (centered.x + centered.w / 2.0, centered.y + centered.h / 2.0);
        assert!((cx - (AREA.x + AREA.w / 2.0)).abs() <= 0.5);
        assert!((cy - (AREA.y + AREA.h / 2.0)).abs() <= 0.5);
        // A window larger than the screen is shrunk to fit, not left hanging over the edge.
        let huge = Layout::Center.place(AREA, Rect::new(0.0, 0.0, 5000.0, 4000.0));
        assert_eq!(huge, AREA);
    }

    #[test]
    fn every_layout_has_a_unique_key_and_shortcut() {
        let keys: std::collections::HashSet<_> = Layout::ALL.iter().map(|l| l.key()).collect();
        let shortcuts: std::collections::HashSet<_> =
            Layout::ALL.iter().map(|l| l.default_shortcut()).collect();
        assert_eq!((keys.len(), shortcuts.len()), (13, 13));
        assert!(Layout::ALL.iter().all(|l| l.place(AREA, WINDOW).w > 0.0));
    }

    #[test]
    fn picks_the_screen_a_window_mostly_sits_on() {
        let main = Rect::new(0.0, 25.0, 1920.0, 1055.0);
        let side = Rect::new(1920.0, 0.0, 1440.0, 900.0);
        let mostly_side = Rect::new(1800.0, 100.0, 800.0, 500.0);
        assert_eq!(pick_area(mostly_side, &[main, side]), Some(side));
        let mostly_main = Rect::new(1500.0, 100.0, 800.0, 500.0);
        assert_eq!(pick_area(mostly_main, &[main, side]), Some(main));
        let off_screen = Rect::new(-5000.0, 100.0, 400.0, 300.0);
        assert_eq!(
            pick_area(off_screen, &[main, side]),
            Some(main),
            "nearest screen wins"
        );
        assert_eq!(pick_area(WINDOW, &[]), None);
    }
}
