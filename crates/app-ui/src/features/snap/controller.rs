//! One drag, from the press to the release, turned into calls on the platform. The decisions are the state
//! machine's (`core-window::snap`); this adds what only the system can tell it (is there a window, did it move)
//! and what to do about the answer. The platform is a trait so that every path can be run with a fake.

use core_window::{
    Layout, Rect,
    snap::{self, Display, Machine, Point, View},
};
use macos_sys::snap_ui::Mouse;
use std::cell::{Cell, RefCell};

/// The window a press landed on.
pub trait Held {
    /// Whether the window has left where the press found it.
    fn moved(&self) -> bool;
    fn frame(&self) -> Option<Rect>;
    fn full_screen(&self) -> bool;
    fn place(&self, frame: Rect) -> bool;
}

pub struct Pressed {
    pub window: Box<dyn Held>,
    /// The Accessibility role of the element under the pointer.
    pub role: String,
    /// The window's frame at the press.
    pub frame: Rect,
}

pub trait Platform {
    fn trusted(&self) -> bool;
    fn displays(&self) -> Vec<Display>;
    /// The window under the pointer, if there is one that is not ours.
    fn grab(&self, at: Point) -> Option<Pressed>;
    /// Shows the row for this screen with the zone `hover` lit.
    fn show(&self, display: &Display, layouts: &[Layout], hover: Option<usize>);
    fn set_hover(&self, hover: Option<usize>);
    fn hide(&self);
    /// Shows where the window would land, gliding there from where it showed before, or fades it away.
    fn footprint(&self, frame: Option<Rect>);
    /// Keeps the notch closed while the row is up.
    fn hold_notch(&self, held: bool);
    /// Runs `f` shortly: the app that owns the window is still finishing its own drag when the button comes up.
    fn later(&self, f: Box<dyn FnOnce()>);
}

pub struct Controller<P> {
    platform: P,
    layouts: RefCell<Vec<Layout>>,
    machine: RefCell<Machine>,
    displays: RefCell<Vec<Display>>,
    held: RefCell<Option<Box<dyn Held>>>,
    shown: Cell<View>,
    landing: Cell<Option<Rect>>,
}

impl<P: Platform> Controller<P> {
    pub fn new(platform: P, layouts: Vec<Layout>) -> Self {
        Self {
            platform,
            machine: RefCell::new(Machine::new(layouts.clone())),
            layouts: RefCell::new(layouts),
            displays: RefCell::new(Vec::new()),
            held: RefCell::new(None),
            shown: Cell::new(View::default()),
            landing: Cell::new(None),
        }
    }

    pub fn set_zones(&self, layouts: Vec<Layout>) {
        self.machine.borrow_mut().set_zones(layouts.clone());
        *self.layouts.borrow_mut() = layouts;
        self.held.borrow_mut().take();
        self.sync();
    }

    /// Forgets the drag in progress and takes the row away.
    pub fn cancel(&self) {
        self.machine.borrow_mut().cancel();
        self.held.borrow_mut().take();
        self.sync();
    }

    pub fn handle(&self, mouse: Mouse) {
        match mouse {
            Mouse::Down(at) => self.press(at),
            Mouse::Dragged(at) => {
                let held = self.held.borrow();
                self.machine
                    .borrow_mut()
                    .drag(at, || held.as_ref().is_some_and(|w| w.moved()));
                drop(held);
                self.sync();
            }
            Mouse::Up(at) => self.release(at),
            Mouse::Lost => self.cancel(),
        }
    }

    fn press(&self, at: Point) {
        self.cancel();
        // Without Accessibility there is no window to find: nothing is looked at and nothing is done.
        if !self.platform.trusted() {
            crate::trace::log("snap: a press, but Accessibility is not granted");
            return;
        }
        let pressed = self.platform.grab(at).filter(|p| {
            let title_bar = snap::is_title_bar_press(p.frame, at, &p.role);
            if !title_bar {
                crate::trace::log(&format!(
                    "snap: a press on {} {:.0} points into a window, not its title bar",
                    p.role,
                    at.1 - p.frame.y
                ));
            }
            title_bar && !p.window.full_screen()
        });
        let on_title_bar = pressed.is_some();
        let displays = if on_title_bar {
            self.platform.displays()
        } else {
            Vec::new()
        };
        *self.displays.borrow_mut() = displays.clone();
        *self.held.borrow_mut() = pressed.map(|p| p.window);
        self.machine.borrow_mut().press(displays, at, on_title_bar);
    }

    fn release(&self, at: Point) {
        let dropped = self.machine.borrow_mut().release(at);
        let held = self.held.borrow_mut().take();
        self.sync();
        let (Some(dropped), Some(window)) = (dropped, held) else {
            return;
        };
        let display = self.displays.borrow().get(dropped.display).copied();
        let layout = self.layouts.borrow().get(dropped.zone).copied();
        let (Some(display), Some(layout)) = (display, layout) else {
            return;
        };
        self.platform.later(Box::new(move || {
            if let Some(current) = window.frame() {
                let target = display.place(layout, current);
                let ok = window.place(target);
                crate::trace::log(&format!(
                    "snap: {current:?} -> {target:?} as {} (applied: {ok})",
                    layout.key()
                ));
            }
        }));
    }

    /// Brings the platform in line with the state machine's view.
    fn sync(&self) {
        let now = self.machine.borrow().view();
        let before = self.shown.replace(now);
        if now == before {
            return;
        }
        match (before.shown, now.shown) {
            (_, Some(display)) if before.shown != now.shown => {
                if before.shown.is_none() {
                    self.platform.hold_notch(true);
                }
                if let Some(d) = self.displays.borrow().get(display) {
                    self.platform.show(d, &self.layouts.borrow(), now.hover);
                }
            }
            (Some(_), None) => {
                self.platform.hide();
                self.platform.hold_notch(false);
            }
            (Some(_), Some(_)) if before.hover != now.hover => self.platform.set_hover(now.hover),
            _ => {}
        }
        let landing = self.machine.borrow().footprint();
        if self.landing.replace(landing) != landing {
            self.platform.footprint(landing);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{cell::RefCell, rc::Rc};

    type Log = Rc<RefCell<Vec<String>>>;

    struct FakeWindow {
        log: Log,
        frame: Rect,
        moves: bool,
        full_screen: bool,
    }

    impl Held for FakeWindow {
        fn moved(&self) -> bool {
            self.log.borrow_mut().push("moved?".into());
            self.moves
        }
        fn frame(&self) -> Option<Rect> {
            Some(self.frame)
        }
        fn full_screen(&self) -> bool {
            self.full_screen
        }
        fn place(&self, frame: Rect) -> bool {
            self.log.borrow_mut().push(format!(
                "place {} {} {} {}",
                frame.x, frame.y, frame.w, frame.h
            ));
            true
        }
    }

    struct Fake {
        log: Log,
        trusted: bool,
        /// What a press finds: role, frame, whether the window follows the pointer, whether it is full screen.
        found: RefCell<Option<(String, Rect, bool, bool)>>,
        displays: Vec<Display>,
    }

    impl Platform for Fake {
        fn trusted(&self) -> bool {
            self.trusted
        }
        fn displays(&self) -> Vec<Display> {
            self.displays.clone()
        }
        fn grab(&self, _at: Point) -> Option<Pressed> {
            self.log.borrow_mut().push("grab".into());
            self.found
                .borrow()
                .clone()
                .map(|(role, frame, moves, full_screen)| Pressed {
                    window: Box::new(FakeWindow {
                        log: Rc::clone(&self.log),
                        frame,
                        moves,
                        full_screen,
                    }),
                    role,
                    frame,
                })
        }
        fn show(&self, display: &Display, layouts: &[Layout], hover: Option<usize>) {
            self.log.borrow_mut().push(format!(
                "show x{} n{} hover{hover:?}",
                display.frame.x,
                layouts.len()
            ));
        }
        fn set_hover(&self, hover: Option<usize>) {
            self.log.borrow_mut().push(format!("hover {hover:?}"));
        }
        fn hide(&self) {
            self.log.borrow_mut().push("hide".into());
        }
        fn footprint(&self, frame: Option<Rect>) {
            self.log
                .borrow_mut()
                .push(frame.map_or("footprint none".into(), |f| {
                    format!("footprint {} {} {} {}", f.x, f.y, f.w, f.h)
                }));
        }
        fn hold_notch(&self, held: bool) {
            self.log.borrow_mut().push(format!("notch {held}"));
        }
        fn later(&self, f: Box<dyn FnOnce()>) {
            self.log.borrow_mut().push("later".into());
            f();
        }
    }

    fn screen(x: f64, y: f64, w: f64, h: f64) -> Display {
        Display {
            frame: Rect::new(x, y, w, h),
            area: Rect::new(x, y + 25.0, w, h - 25.0),
            cutout: 200.0,
            inset: 10.0,
        }
    }

    fn window_frame() -> Rect {
        Rect::new(400.0, 300.0, 600.0, 400.0)
    }

    fn rig(trusted: bool, found: Option<(&str, bool, bool)>) -> (Controller<Fake>, Log) {
        let log: Log = Rc::default();
        let platform = Fake {
            log: Rc::clone(&log),
            trusted,
            found: RefCell::new(
                found.map(|(role, moves, full)| (role.to_owned(), window_frame(), moves, full)),
            ),
            displays: vec![
                screen(0.0, 0.0, 1920.0, 1080.0),
                screen(1920.0, -100.0, 2560.0, 1440.0),
            ],
        };
        (Controller::new(platform, snap::DEFAULT_ZONES.to_vec()), log)
    }

    fn lines(log: &Log) -> Vec<String> {
        log.borrow().clone()
    }

    /// The pointer position over the middle of zone `zone` of display `display`.
    fn over(c: &Controller<Fake>, display: usize, zone: usize) -> Point {
        let d = c.platform.displays[display];
        let cell = d.cells(&c.layouts.borrow())[zone];
        (cell.x + cell.w / 2.0, cell.y + cell.h / 2.0)
    }

    #[test]
    fn dragging_a_title_bar_onto_a_zone_snaps_the_window() {
        let (c, log) = rig(true, Some(("AXWindow", true, false)));
        c.handle(Mouse::Down((500.0, 310.0)));
        c.handle(Mouse::Dragged((520.0, 290.0)));
        c.handle(Mouse::Dragged((960.0, 1.0)));
        let on = over(&c, 0, 1);
        c.handle(Mouse::Dragged(on));
        c.handle(Mouse::Up(on));
        assert_eq!(
            lines(&log),
            [
                "grab",
                "moved?",
                "notch true",
                "show x0 n10 hoverNone",
                "hover Some(1)",
                // Where the window will land shows with the zone, and is exactly where it is put.
                "footprint 960 25 960 1055",
                "hide",
                "notch false",
                "footprint none",
                "later",
                // The right half of the usable area of the first screen.
                "place 960 25 960 1055",
            ]
        );
    }

    #[test]
    fn a_drop_on_the_second_screen_uses_that_screens_area() {
        let (c, log) = rig(true, Some(("AXStaticText", true, false)));
        c.handle(Mouse::Down((500.0, 310.0)));
        c.handle(Mouse::Dragged((520.0, 290.0)));
        c.handle(Mouse::Dragged((1920.0 + 1280.0, -100.0)));
        let on = over(&c, 1, 2);
        c.handle(Mouse::Dragged(on));
        c.handle(Mouse::Up(on));
        assert!(lines(&log).contains(&"show x1920 n10 hoverNone".to_owned()));
        assert!(lines(&log).contains(&"footprint 1920 -75 2560 1415".to_owned()));
        // Maximize: the whole usable area of the second screen.
        assert_eq!(lines(&log).last().unwrap(), "place 1920 -75 2560 1415");
    }

    #[test]
    fn without_accessibility_nothing_is_looked_at_or_shown() {
        let (c, log) = rig(false, Some(("AXWindow", true, false)));
        c.handle(Mouse::Down((500.0, 310.0)));
        c.handle(Mouse::Dragged((520.0, 290.0)));
        c.handle(Mouse::Dragged((960.0, 1.0)));
        c.handle(Mouse::Up((960.0, 1.0)));
        assert!(lines(&log).is_empty(), "{:?}", lines(&log));
    }

    #[test]
    fn a_drag_that_started_on_no_window_never_shows_the_zones() {
        let (c, log) = rig(true, None);
        c.handle(Mouse::Down((500.0, 310.0)));
        c.handle(Mouse::Dragged((520.0, 290.0)));
        c.handle(Mouse::Dragged((960.0, 1.0)));
        c.handle(Mouse::Up((960.0, 1.0)));
        assert_eq!(lines(&log), ["grab"]);
    }

    #[test]
    fn a_press_on_a_control_or_below_the_title_bar_does_not_count() {
        let (c, log) = rig(true, Some(("AXButton", true, false)));
        c.handle(Mouse::Down((500.0, 310.0)));
        c.handle(Mouse::Dragged((960.0, 1.0)));
        assert_eq!(lines(&log), ["grab"]);

        let (c, log) = rig(true, Some(("AXWindow", true, false)));
        // 200 points below the top of the window: the content area.
        c.handle(Mouse::Down((500.0, 500.0)));
        c.handle(Mouse::Dragged((960.0, 1.0)));
        assert_eq!(lines(&log), ["grab"]);
    }

    #[test]
    fn a_full_screen_window_is_left_alone() {
        let (c, log) = rig(true, Some(("AXWindow", true, true)));
        c.handle(Mouse::Down((500.0, 310.0)));
        c.handle(Mouse::Dragged((960.0, 1.0)));
        assert_eq!(lines(&log), ["grab"]);
    }

    #[test]
    fn a_window_that_does_not_follow_the_pointer_is_not_being_dragged() {
        let (c, log) = rig(true, Some(("AXWindow", false, false)));
        c.handle(Mouse::Down((500.0, 310.0)));
        for i in 0..5 {
            c.handle(Mouse::Dragged((
                520.0 + f64::from(i) * 100.0,
                150.0 - f64::from(i) * 40.0,
            )));
        }
        c.handle(Mouse::Dragged((960.0, 1.0)));
        c.handle(Mouse::Up((960.0, 1.0)));
        assert!(
            lines(&log).iter().all(|l| l == "grab" || l == "moved?"),
            "{:?}",
            lines(&log)
        );
    }

    #[test]
    fn leaving_the_zone_and_releasing_elsewhere_cancels() {
        let (c, log) = rig(true, Some(("AXWindow", true, false)));
        c.handle(Mouse::Down((500.0, 310.0)));
        c.handle(Mouse::Dragged((520.0, 290.0)));
        c.handle(Mouse::Dragged((960.0, 1.0)));
        let on = over(&c, 0, 0);
        c.handle(Mouse::Dragged(on));
        c.handle(Mouse::Dragged((960.0, 600.0)));
        c.handle(Mouse::Up((960.0, 600.0)));
        let l = lines(&log);
        assert!(l.contains(&"hover Some(0)".to_owned()) && l.contains(&"hide".to_owned()));
        assert!(l.contains(&"footprint 0 25 960 1055".to_owned()));
        assert!(
            !l.iter().any(|x| x.starts_with("place") || x == "later"),
            "{l:?}"
        );
        assert_eq!(&l[l.len() - 3..], ["hide", "notch false", "footprint none"]);
    }

    #[test]
    fn the_footprint_follows_the_lit_zone_and_leaves_with_it() {
        let (c, log) = rig(true, Some(("AXWindow", true, false)));
        c.handle(Mouse::Down((500.0, 310.0)));
        c.handle(Mouse::Dragged((520.0, 290.0)));
        c.handle(Mouse::Dragged((960.0, 1.0)));
        for zone in [0, 6, 6, 3] {
            c.handle(Mouse::Dragged(over(&c, 0, zone)));
        }
        // Up into the strip above the zones: still shown, nothing lit.
        c.handle(Mouse::Dragged((960.0, 1.0)));
        let l = lines(&log);
        let footprints: Vec<&str> = l
            .iter()
            .filter(|x| x.starts_with("footprint"))
            .map(String::as_str)
            .collect();
        assert_eq!(
            footprints,
            [
                "footprint 0 25 960 1055",
                // The top left quarter, once: staying on a zone does not ask again.
                "footprint 0 25 960 528",
                "footprint 0 25 640 1055",
                "footprint none",
            ]
        );
        assert!(!l.contains(&"hide".to_owned()), "the zones still show");
    }

    #[test]
    fn a_missed_release_takes_the_row_away_and_drops_nothing() {
        let (c, log) = rig(true, Some(("AXWindow", true, false)));
        c.handle(Mouse::Down((500.0, 310.0)));
        c.handle(Mouse::Dragged((520.0, 290.0)));
        c.handle(Mouse::Dragged((960.0, 1.0)));
        c.handle(Mouse::Lost);
        let l = lines(&log);
        assert_eq!(&l[l.len() - 2..], ["hide", "notch false"]);
        assert!(!l.iter().any(|x| x == "later"));
    }

    #[test]
    fn turning_the_feature_off_or_changing_the_zones_mid_drag_puts_everything_back() {
        let (c, log) = rig(true, Some(("AXWindow", true, false)));
        c.handle(Mouse::Down((500.0, 310.0)));
        c.handle(Mouse::Dragged((520.0, 290.0)));
        c.handle(Mouse::Dragged((960.0, 1.0)));
        c.set_zones(vec![Layout::Left, Layout::Right]);
        let l = lines(&log);
        assert_eq!(&l[l.len() - 2..], ["hide", "notch false"]);
        // The drag goes on, but is forgotten: nothing comes back until a new press.
        c.handle(Mouse::Dragged((960.0, 1.0)));
        assert_eq!(lines(&log).len(), l.len());
    }

    #[test]
    fn a_new_press_while_the_row_is_up_starts_over() {
        let (c, log) = rig(true, Some(("AXWindow", true, false)));
        c.handle(Mouse::Down((500.0, 310.0)));
        c.handle(Mouse::Dragged((520.0, 290.0)));
        c.handle(Mouse::Dragged((960.0, 1.0)));
        *c.platform.found.borrow_mut() = None;
        c.handle(Mouse::Down((100.0, 100.0)));
        let l = lines(&log);
        assert!(l.contains(&"hide".to_owned()) && l.last().unwrap() == "grab");
    }
}
