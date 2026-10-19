//! The layouts a dragged window can be dropped into. A template is a set of cells, each a part of the usable area
//! of a screen written as fractions, so that every screen places it its own way. This holds the built-in templates,
//! the ones a person makes by dividing a grid and joining some of its parts, and the rule that picks among them.

use crate::Rect;

/// The edges of a cell as fractions of the usable area of a screen: 0 is its left and top, 1 its right and bottom.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Cell {
    pub x0: f64,
    pub y0: f64,
    pub x1: f64,
    pub y1: f64,
}

impl Cell {
    pub const fn new(x0: f64, y0: f64, x1: f64, y1: f64) -> Self {
        Self { x0, y0, x1, y1 }
    }

    /// The frame of the cell inside `area`. Each edge is rounded on its own, so the cells of a template meet exactly
    /// and cover the area whatever its size.
    pub fn place(self, area: Rect) -> Rect {
        let edge = |origin: f64, total: f64, fraction: f64| origin + (total * fraction).round();
        let (left, right) = (edge(area.x, area.w, self.x0), edge(area.x, area.w, self.x1));
        let (top, bottom) = (edge(area.y, area.h, self.y0), edge(area.y, area.h, self.y1));
        Rect::new(left, top, right - left, bottom - top)
    }

    /// The part the cell fills as x, y, width and height, for drawing a small screen.
    pub fn part(self) -> [f64; 4] {
        [self.x0, self.y0, self.x1 - self.x0, self.y1 - self.y0]
    }
}

/// One layout: its id, the cells a window can go in, and the name a person gave it (a built-in one is named by the
/// language of the app).
#[derive(Debug, Clone, PartialEq)]
pub struct Template {
    pub id: String,
    pub name: Option<String>,
    pub cells: Vec<Cell>,
}

/// Every built-in template, in the order the settings offer them.
pub const BUILT_IN: [&str; 10] = [
    "full",
    "halves",
    "rows",
    "thirds",
    "quarters",
    "main_left",
    "main_right",
    "left_stack",
    "right_stack",
    "grid_six",
];

/// What a screen offers until its owner chooses: the ones most often wanted.
pub const DEFAULT: [&str; 7] = [
    "full",
    "halves",
    "thirds",
    "quarters",
    "rows",
    "main_left",
    "main_right",
];

/// The cells between the given edges, row by row from the top left.
fn grid(columns: &[f64], rows: &[f64]) -> Vec<Cell> {
    rows.windows(2)
        .flat_map(|r| {
            columns
                .windows(2)
                .map(move |c| Cell::new(c[0], r[0], c[1], r[1]))
        })
        .collect()
}

/// A built-in template by id.
pub fn built_in(id: &str) -> Option<Template> {
    let (third, two_thirds) = (1.0 / 3.0, 2.0 / 3.0);
    let cells = match id {
        "full" => grid(&[0.0, 1.0], &[0.0, 1.0]),
        "halves" => grid(&[0.0, 0.5, 1.0], &[0.0, 1.0]),
        "rows" => grid(&[0.0, 1.0], &[0.0, 0.5, 1.0]),
        "thirds" => grid(&[0.0, third, two_thirds, 1.0], &[0.0, 1.0]),
        "quarters" => grid(&[0.0, 0.5, 1.0], &[0.0, 0.5, 1.0]),
        "main_left" => grid(&[0.0, two_thirds, 1.0], &[0.0, 1.0]),
        "main_right" => grid(&[0.0, third, 1.0], &[0.0, 1.0]),
        "left_stack" => vec![
            Cell::new(0.0, 0.0, 0.5, 1.0),
            Cell::new(0.5, 0.0, 1.0, 0.5),
            Cell::new(0.5, 0.5, 1.0, 1.0),
        ],
        "right_stack" => vec![
            Cell::new(0.0, 0.0, 0.5, 0.5),
            Cell::new(0.0, 0.5, 0.5, 1.0),
            Cell::new(0.5, 0.0, 1.0, 1.0),
        ],
        "grid_six" => grid(&[0.0, third, two_thirds, 1.0], &[0.0, 0.5, 1.0]),
        _ => return None,
    };
    Some(Template {
        id: id.to_owned(),
        name: None,
        cells,
    })
}

/// The most columns or rows a grid made by a person has.
pub const MAX_GRID: u8 = 6;
/// The most layouts a person can make.
pub const MAX_CUSTOM: usize = 12;
/// The longest name of one, in characters.
pub const MAX_NAME: usize = 24;

/// A layout a person made: a grid of `cols` by `rows` equal parts, some of them joined. `cells` are the joined parts
/// as `[column, row, column, row]` of the first part and of the one after the last, so that together they cover every
/// part of the grid once.
#[derive(Debug, Clone, PartialEq)]
pub struct Custom {
    pub id: String,
    pub name: String,
    pub cols: u8,
    pub rows: u8,
    pub cells: Vec<[u8; 4]>,
}

impl Custom {
    /// Whether `id` is shaped like the id of a custom layout: a `c` and one to four digits, which no built-in has.
    pub fn is_id(id: &str) -> bool {
        (2..=5).contains(&id.len())
            && id.starts_with('c')
            && id[1..].bytes().all(|b| b.is_ascii_digit())
    }

    /// What is wrong with it, for the message of a refusal. The page is not trusted, so every rule is kept here.
    pub fn check(&self) -> Result<(), &'static str> {
        if !Self::is_id(&self.id) {
            return Err("the id is a c and some digits");
        }
        let name = self.name.trim();
        if name.is_empty() || name.chars().count() > MAX_NAME || name.chars().any(char::is_control)
        {
            return Err("the name is 1 to 24 characters");
        }
        let grid = 1..=MAX_GRID;
        if !grid.contains(&self.cols) || !grid.contains(&self.rows) {
            return Err("a grid has 1 to 6 columns and rows");
        }
        let mut covered = vec![0u8; usize::from(self.cols) * usize::from(self.rows)];
        for &[c0, r0, c1, r1] in &self.cells {
            if c0 >= c1 || r0 >= r1 || c1 > self.cols || r1 > self.rows {
                return Err("a cell lies outside the grid or is empty");
            }
            for row in r0..r1 {
                for col in c0..c1 {
                    covered[usize::from(row) * usize::from(self.cols) + usize::from(col)] += 1;
                }
            }
        }
        if covered.iter().any(|&n| n != 1) {
            return Err("the cells must cover every part of the grid once");
        }
        Ok(())
    }

    pub fn template(&self) -> Template {
        let (cols, rows) = (f64::from(self.cols), f64::from(self.rows));
        Template {
            id: self.id.clone(),
            name: Some(self.name.trim().to_owned()),
            cells: self
                .cells
                .iter()
                .map(|&[c0, r0, c1, r1]| {
                    Cell::new(
                        f64::from(c0) / cols,
                        f64::from(r0) / rows,
                        f64::from(c1) / cols,
                        f64::from(r1) / rows,
                    )
                })
                .collect(),
        }
    }
}

/// The templates a list of ids asks for, in that order: an id that is unknown or repeated is dropped, and so is a
/// custom layout that does not check out. When none is left, the default ones stand in, so a screen always has some.
pub fn choose<S: AsRef<str>>(ids: &[S], custom: &[Custom]) -> Vec<Template> {
    let mut chosen: Vec<Template> = Vec::new();
    for id in ids.iter().map(AsRef::as_ref) {
        if chosen.iter().any(|t| t.id == id) {
            continue;
        }
        let found = built_in(id).or_else(|| {
            custom
                .iter()
                .find(|c| c.id == id && c.check().is_ok())
                .map(Custom::template)
        });
        chosen.extend(found);
    }
    if chosen.is_empty() {
        chosen = DEFAULT.iter().filter_map(|id| built_in(id)).collect();
    }
    chosen
}

#[cfg(test)]
mod tests {
    use super::*;

    const AREA: Rect = Rect::new(0.0, 25.0, 1921.0, 1055.0);

    /// Every pixel of `area` belongs to exactly one cell, so a drop never leaves a gap or an overlap.
    fn tiles(template: &Template, area: Rect) -> bool {
        let rects: Vec<Rect> = template.cells.iter().map(|c| c.place(area)).collect();
        let total: f64 = rects.iter().map(|r| r.w * r.h).sum();
        let overlap = rects.iter().enumerate().any(|(i, a)| {
            rects[i + 1..]
                .iter()
                .any(|b| a.x < b.x + b.w && b.x < a.x + a.w && a.y < b.y + b.h && b.y < a.y + a.h)
        });
        total == area.w * area.h
            && !overlap
            && rects
                .iter()
                .all(|r| r.w > 0.0 && r.h > 0.0 && r.x >= area.x && r.y >= area.y)
    }

    fn custom(cols: u8, rows: u8, cells: &[[u8; 4]]) -> Custom {
        Custom {
            id: "c1".into(),
            name: "Mine".into(),
            cols,
            rows,
            cells: cells.to_vec(),
        }
    }

    #[test]
    fn every_built_in_template_covers_a_screen_once_whatever_its_size() {
        for id in BUILT_IN {
            let template = built_in(id).expect(id);
            for area in [
                AREA,
                Rect::new(-1512.0, -982.0, 1511.0, 957.0),
                Rect::new(1920.0, -175.0, 2560.0, 1415.0),
                Rect::new(0.0, 0.0, 7.0, 5.0),
            ] {
                assert!(tiles(&template, area), "{id} on {area:?}");
            }
            assert_eq!(template.id, id);
            assert!(
                template.name.is_none(),
                "built-in ones are named by the language"
            );
        }
        assert!(built_in("center").is_none() && built_in("").is_none() && built_in("c1").is_none());
    }

    #[test]
    fn the_catalogue_has_no_repeats_and_holds_the_defaults() {
        let mut ids = BUILT_IN.to_vec();
        ids.sort_unstable();
        ids.dedup();
        assert_eq!(ids.len(), BUILT_IN.len());
        assert!(DEFAULT.iter().all(|id| BUILT_IN.contains(id)));
        assert!(
            BUILT_IN.iter().all(|id| !Custom::is_id(id)),
            "a custom id never names a built-in"
        );
    }

    #[test]
    fn the_cells_of_each_template_come_in_reading_order_for_the_ones_that_have_rows_and_columns() {
        let quarters = built_in("quarters").unwrap().cells;
        let at = |i: usize| (quarters[i].x0, quarters[i].y0);
        assert_eq!(
            [at(0), at(1), at(2), at(3)],
            [(0.0, 0.0), (0.5, 0.0), (0.0, 0.5), (0.5, 0.5)]
        );
        assert_eq!(built_in("full").unwrap().cells.len(), 1);
        assert_eq!(built_in("thirds").unwrap().cells.len(), 3);
        assert_eq!(built_in("grid_six").unwrap().cells.len(), 6);
        let main = built_in("main_left").unwrap().cells;
        assert!((main[0].x1 - 2.0 / 3.0).abs() < 1e-12 && main[0].x1 == main[1].x0);
    }

    #[test]
    fn a_cell_is_placed_inside_the_area_with_neighbours_meeting_exactly() {
        let halves = built_in("halves").unwrap().cells;
        let (left, right) = (halves[0].place(AREA), halves[1].place(AREA));
        assert_eq!(left.x, AREA.x);
        assert_eq!(
            left.x + left.w,
            right.x,
            "no gap in the middle of an odd width"
        );
        assert_eq!(right.x + right.w, AREA.x + AREA.w);
        assert_eq!((left.y, left.h), (AREA.y, AREA.h));
        assert_eq!(Cell::new(0.0, 0.0, 1.0, 1.0).place(AREA), AREA);
        assert_eq!(
            Cell::new(0.25, 0.5, 1.0, 1.0).part(),
            [0.25, 0.5, 0.75, 0.5]
        );
    }

    #[test]
    fn a_grid_a_person_made_becomes_a_template_that_covers_the_screen() {
        // Three columns by two rows, the first column joined into one tall cell and the other four left apart.
        let layout = custom(
            3,
            2,
            &[
                [0, 0, 1, 2],
                [1, 0, 2, 1],
                [2, 0, 3, 1],
                [1, 1, 2, 2],
                [2, 1, 3, 2],
            ],
        );
        assert_eq!(layout.check(), Ok(()));
        let template = layout.template();
        assert_eq!(template.name.as_deref(), Some("Mine"));
        assert_eq!(template.cells.len(), 5);
        assert!(tiles(&template, AREA));
        assert_eq!(template.cells[0], Cell::new(0.0, 0.0, 1.0 / 3.0, 1.0));
        // The whole screen as one cell is a layout too.
        assert_eq!(custom(1, 1, &[[0, 0, 1, 1]]).check(), Ok(()));
        // The biggest grid.
        let cells: Vec<[u8; 4]> = (0..6)
            .flat_map(|r| (0..6).map(move |c| [c, r, c + 1, r + 1]))
            .collect();
        let big = custom(6, 6, &cells);
        assert_eq!(big.check(), Ok(()));
        assert!(tiles(&big.template(), AREA));
    }

    #[test]
    fn a_grid_that_does_not_add_up_is_refused() {
        let ok = [[0, 0, 1, 1], [1, 0, 2, 1]];
        assert_eq!(custom(2, 1, &ok).check(), Ok(()));
        for (what, bad) in [
            ("a part left out", custom(2, 1, &[[0, 0, 1, 1]])),
            ("a part twice", custom(2, 1, &[[0, 0, 2, 1], [1, 0, 2, 1]])),
            (
                "a cell outside",
                custom(2, 1, &[[0, 0, 1, 1], [1, 0, 3, 1]]),
            ),
            ("an empty cell", custom(2, 1, &[[0, 0, 0, 1], [0, 0, 2, 1]])),
            (
                "a backwards cell",
                custom(2, 1, &[[1, 0, 0, 1], [1, 0, 2, 1]]),
            ),
            ("no cells", custom(2, 1, &[])),
            ("no columns", custom(0, 1, &[])),
            ("a grid too wide", custom(7, 1, &[[0, 0, 7, 1]])),
            ("a grid too tall", custom(1, 7, &[[0, 0, 1, 7]])),
        ] {
            assert!(bad.check().is_err(), "{what}");
        }
        let mut named = custom(2, 1, &ok);
        let long = "x".repeat(MAX_NAME + 1);
        for name in ["", "   ", "a\nb", long.as_str()] {
            named.name = name.into();
            assert!(named.check().is_err(), "{name:?}");
        }
        named.name = "x".repeat(MAX_NAME);
        assert_eq!(named.check(), Ok(()));
        for id in ["", "c", "1", "cx", "c1x", "c123456", "left", "C1"] {
            named.id = id.into();
            assert!(named.check().is_err(), "{id:?}");
        }
        assert!(Custom::is_id("c0") && Custom::is_id("c9999"));
    }

    #[test]
    fn choosing_keeps_the_order_and_drops_what_is_unknown_repeated_or_broken() {
        let mine = custom(2, 1, &[[0, 0, 1, 1], [1, 0, 2, 1]]);
        let mut broken = custom(2, 1, &[[0, 0, 1, 1]]);
        broken.id = "c2".into();
        let ids = |chosen: &[Template]| chosen.iter().map(|t| t.id.clone()).collect::<Vec<_>>();
        let chosen = choose(
            &["thirds", "bogus", "c1", "thirds", "c2", "c3", "", "full"],
            &[mine.clone(), broken],
        );
        assert_eq!(ids(&chosen), ["thirds", "c1", "full"]);
        assert_eq!(chosen[1].name.as_deref(), Some("Mine"));
        // Nothing usable left: the default ones, so a screen always has something to offer.
        for none in [choose::<&str>(&[], &[]), choose(&["bogus", "c1"], &[])] {
            assert_eq!(ids(&none), DEFAULT);
        }
        assert_eq!(ids(&choose(&BUILT_IN, &[])), BUILT_IN);
    }
}
