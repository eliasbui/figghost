//! The gestures of the region overlay as a state machine: press, drag, hold Space to move the selection
//! instead of resizing it, release or press Return to confirm, Escape to cancel. The overlay feeds it
//! points in global space and draws what it reports.

use crate::geometry::{Display, Point, Rect, clamp_point, display_index_at};

/// A selection smaller than this on either side is a click, not a region.
pub const MIN_SIDE: f64 = 3.0;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Event {
    /// The pointer moved with no button down.
    Move(Point),
    Down(Point),
    Drag(Point),
    Up(Point),
    /// Space went down (`true`) or up.
    Space(bool),
    /// Return.
    Confirm,
    /// Escape.
    Cancel,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Outcome {
    /// `display` indexes the displays the selector was made with; `rect` is inside that display.
    Selected {
        display: usize,
        rect: Rect,
    },
    Cancelled,
}

pub struct Selector {
    displays: Vec<Display>,
    /// The display the drag started on; a selection never leaves it.
    active: Option<usize>,
    anchor: Point,
    current: Point,
    /// Where the pointer was at the last event, so moving by Space can follow its change.
    last: Point,
    space: bool,
    hover: Point,
    finished: bool,
}

impl Selector {
    pub fn new(displays: Vec<Display>) -> Self {
        Self {
            displays,
            active: None,
            anchor: Point::default(),
            current: Point::default(),
            last: Point::default(),
            space: false,
            hover: Point::default(),
            finished: false,
        }
    }

    /// Where the pointer is while no drag is going on.
    pub fn hover(&self) -> Point {
        self.hover
    }

    /// The display and rectangle being selected, while the button is down.
    pub fn selection(&self) -> Option<(usize, Rect)> {
        self.active
            .map(|i| (i, Rect::from_corners(self.anchor, self.current)))
    }

    fn bounds(&self, display: usize) -> Rect {
        self.displays[display].frame
    }

    fn valid(rect: &Rect) -> bool {
        rect.w >= MIN_SIDE && rect.h >= MIN_SIDE
    }

    fn selected(&mut self) -> Option<Outcome> {
        let (display, rect) = self.selection()?;
        if !Self::valid(&rect) {
            return None;
        }
        self.finished = true;
        Some(Outcome::Selected { display, rect })
    }

    /// Moves the pointer of an ongoing drag: resizes the selection, or moves it while Space is held.
    fn drag_to(&mut self, p: Point) {
        let Some(display) = self.active else {
            return;
        };
        let bounds = self.bounds(display);
        let p = clamp_point(p, &bounds);
        if self.space {
            let rect = Rect::from_corners(self.anchor, self.current);
            let moved = rect
                .translated(p.x - self.last.x, p.y - self.last.y)
                .moved_into(&bounds);
            let (dx, dy) = (moved.x - rect.x, moved.y - rect.y);
            self.anchor = Point::new(self.anchor.x + dx, self.anchor.y + dy);
            self.current = Point::new(self.current.x + dx, self.current.y + dy);
        } else {
            self.current = p;
        }
        self.last = p;
    }

    /// Feeds one event; `Some` once the overlay has an answer.
    pub fn handle(&mut self, event: Event) -> Option<Outcome> {
        if self.finished || self.displays.is_empty() {
            return None;
        }
        match event {
            Event::Move(p) => {
                self.hover = p;
                None
            }
            Event::Down(p) => {
                let display = display_index_at(&self.displays, p)?;
                let p = clamp_point(p, &self.bounds(display));
                self.active = Some(display);
                (self.anchor, self.current, self.last, self.hover) = (p, p, p, p);
                None
            }
            Event::Drag(p) => {
                self.drag_to(p);
                None
            }
            Event::Up(p) => {
                self.drag_to(p);
                let outcome = self.selected();
                if outcome.is_none() {
                    // A click, or a selection too small to keep: start over.
                    self.active = None;
                    self.space = false;
                }
                outcome
            }
            Event::Space(on) => {
                self.space = on && self.active.is_some();
                None
            }
            Event::Confirm => self.selected(),
            Event::Cancel => {
                self.finished = true;
                Some(Outcome::Cancelled)
            }
        }
    }
}

/// "812 x 430", the readout beside a selection.
pub fn size_text(rect: &Rect) -> String {
    format!("{} x {}", rect.w.round() as i64, rect.h.round() as i64)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn p(x: f64, y: f64) -> Point {
        Point::new(x, y)
    }

    /// Two displays side by side, the second one higher.
    fn selector() -> Selector {
        Selector::new(vec![
            Display {
                id: 1,
                frame: Rect::new(0.0, 0.0, 1000.0, 800.0),
                scale: 2.0,
            },
            Display {
                id: 2,
                frame: Rect::new(1000.0, 200.0, 1000.0, 800.0),
                scale: 1.0,
            },
        ])
    }

    fn selected(display: usize, x: f64, y: f64, w: f64, h: f64) -> Option<Outcome> {
        Some(Outcome::Selected {
            display,
            rect: Rect::new(x, y, w, h),
        })
    }

    #[test]
    fn dragging_and_releasing_selects_the_rectangle() {
        let mut s = selector();
        assert_eq!(s.handle(Event::Down(p(100.0, 100.0))), None);
        assert_eq!(s.handle(Event::Drag(p(300.0, 250.0))), None);
        assert_eq!(
            s.selection(),
            Some((0, Rect::new(100.0, 100.0, 200.0, 150.0)))
        );
        assert_eq!(
            s.handle(Event::Up(p(300.0, 250.0))),
            selected(0, 100.0, 100.0, 200.0, 150.0)
        );
    }

    #[test]
    fn a_drag_up_and_to_the_left_is_the_same_selection() {
        let mut s = selector();
        s.handle(Event::Down(p(300.0, 250.0)));
        assert_eq!(
            s.handle(Event::Up(p(100.0, 100.0))),
            selected(0, 100.0, 100.0, 200.0, 150.0)
        );
    }

    #[test]
    fn the_selection_cannot_leave_the_display_it_started_on() {
        let mut s = selector();
        s.handle(Event::Down(p(900.0, 700.0)));
        // The pointer runs into the second display and far above the first.
        s.handle(Event::Drag(p(1500.0, 900.0)));
        assert_eq!(
            s.selection(),
            Some((0, Rect::new(900.0, 700.0, 100.0, 100.0)))
        );
        assert_eq!(
            s.handle(Event::Up(p(-50.0, -50.0))),
            selected(0, 0.0, 0.0, 900.0, 700.0)
        );
    }

    #[test]
    fn a_press_between_displays_goes_to_the_nearest_and_is_clamped() {
        let mut s = selector();
        s.handle(Event::Down(p(1500.0, 100.0)));
        // 100 is below the second display, which starts at 200.
        assert_eq!(s.selection().unwrap().0, 1);
        assert_eq!(
            s.handle(Event::Up(p(1700.0, 400.0))),
            selected(1, 1500.0, 200.0, 200.0, 200.0)
        );
    }

    #[test]
    fn space_moves_the_selection_and_releasing_it_resizes_again() {
        let mut s = selector();
        s.handle(Event::Down(p(100.0, 100.0)));
        s.handle(Event::Drag(p(200.0, 200.0)));
        s.handle(Event::Space(true));
        s.handle(Event::Drag(p(250.0, 230.0)));
        // Moved by (50, 30), same 100x100 size.
        assert_eq!(
            s.selection(),
            Some((0, Rect::new(150.0, 130.0, 100.0, 100.0)))
        );
        s.handle(Event::Space(false));
        s.handle(Event::Drag(p(350.0, 330.0)));
        assert_eq!(
            s.selection(),
            Some((0, Rect::new(150.0, 130.0, 200.0, 200.0)))
        );
    }

    #[test]
    fn a_moved_selection_stops_at_the_edge_of_the_display() {
        let mut s = selector();
        s.handle(Event::Down(p(900.0, 600.0)));
        s.handle(Event::Drag(p(950.0, 650.0)));
        s.handle(Event::Space(true));
        s.handle(Event::Drag(p(1010.0, 700.0)));
        let (_, rect) = s.selection().unwrap();
        assert_eq!((rect.w, rect.h), (50.0, 50.0));
        assert_eq!(rect.max_x(), 1000.0);
        // The pointer is dragged on to the next display: still clamped to the first one.
        s.handle(Event::Drag(p(1400.0, 900.0)));
        let (display, rect) = s.selection().unwrap();
        assert_eq!(display, 0);
        assert!(rect.max_x() <= 1000.0 && rect.max_y() <= 800.0);
        assert_eq!((rect.w, rect.h), (50.0, 50.0));
    }

    #[test]
    fn space_before_a_press_does_nothing() {
        let mut s = selector();
        s.handle(Event::Space(true));
        s.handle(Event::Down(p(100.0, 100.0)));
        s.handle(Event::Drag(p(200.0, 200.0)));
        assert_eq!(
            s.selection(),
            Some((0, Rect::new(100.0, 100.0, 100.0, 100.0)))
        );
    }

    #[test]
    fn return_confirms_the_selection_so_far() {
        let mut s = selector();
        assert_eq!(s.handle(Event::Confirm), None, "nothing is selected yet");
        s.handle(Event::Down(p(10.0, 10.0)));
        s.handle(Event::Drag(p(110.0, 60.0)));
        assert_eq!(
            s.handle(Event::Confirm),
            selected(0, 10.0, 10.0, 100.0, 50.0)
        );
    }

    #[test]
    fn a_click_selects_nothing_and_a_new_drag_can_follow() {
        let mut s = selector();
        s.handle(Event::Down(p(10.0, 10.0)));
        assert_eq!(s.handle(Event::Up(p(11.0, 11.0))), None);
        assert_eq!(s.selection(), None);
        assert_eq!(s.handle(Event::Confirm), None);
        s.handle(Event::Down(p(10.0, 10.0)));
        assert_eq!(
            s.handle(Event::Up(p(60.0, 60.0))),
            selected(0, 10.0, 10.0, 50.0, 50.0)
        );
    }

    #[test]
    fn escape_cancels_at_any_time_and_ends_the_session() {
        let mut s = selector();
        s.handle(Event::Down(p(10.0, 10.0)));
        s.handle(Event::Drag(p(100.0, 100.0)));
        assert_eq!(s.handle(Event::Cancel), Some(Outcome::Cancelled));
        assert_eq!(
            s.handle(Event::Up(p(100.0, 100.0))),
            None,
            "nothing after the end"
        );
        assert_eq!(s.handle(Event::Confirm), None);
    }

    #[test]
    fn a_selection_is_only_reported_once() {
        let mut s = selector();
        s.handle(Event::Down(p(10.0, 10.0)));
        assert!(s.handle(Event::Up(p(100.0, 100.0))).is_some());
        assert_eq!(s.handle(Event::Confirm), None);
    }

    #[test]
    fn the_readout_rounds_to_whole_points() {
        assert_eq!(size_text(&Rect::new(0.0, 0.0, 812.4, 429.6)), "812 x 430");
    }

    #[test]
    fn no_displays_means_no_answer() {
        let mut s = Selector::new(Vec::new());
        assert_eq!(s.handle(Event::Down(p(1.0, 1.0))), None);
        assert_eq!(s.handle(Event::Cancel), None);
    }
}
