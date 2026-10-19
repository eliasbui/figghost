//! One drag, from the press to the release, turned into calls on the platform. The decisions are the state machine's
//! (`core-window::snap`); this adds what only the system can tell it (is there a window, did it move) and what to do
//! about the answer. The platform is a trait so that every path can be run with a fake.

use core_window::{
    Rect,
    snap::{Display, Machine, Point, View, is_title_bar_press},
    templates::Template,
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
    /// Every screen, with its name.
    fn displays(&self) -> Vec<(Display, String)>;
    /// The window under the pointer, if there is one that is not ours.
    fn grab(&self, at: Point) -> Option<Pressed>;
    /// Shows the row of `templates` on this screen with the layout `view.hover` lit and `view.chosen` outlined, or the
    /// chip of the chosen one when `view.compact`. Called again whenever any of those change.
    fn show(&self, display: &Display, templates: &[Template], view: View);
    fn hide(&self);
    /// Shows where the window would land, gliding there from where it showed before, or fades it away.
    fn footprint(&self, frame: Option<Rect>);
    /// Shows the cells of the chosen layout over the part of the screen they fill, or takes them away.
    fn cells(&self, shown: Option<(Rect, Vec<Rect>)>);
    /// Keeps the notch closed while the row is up.
    fn hold_notch(&self, held: bool);
    /// Runs `f` shortly: the app that owns the window is still finishing its own drag when the button comes up.
    fn later(&self, f: Box<dyn FnOnce()>);
}

/// What the screens offer: a list for each screen that has one of its own, by name, and one for all the others.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Choice {
    pub default: Vec<Template>,
    pub screens: Vec<(String, Vec<Template>)>,
}

impl Choice {
    fn for_screen(&self, name: &str) -> &[Template] {
        self.screens
            .iter()
            .find(|(n, _)| n == name)
            .map_or(&self.default, |(_, templates)| templates)
    }
}

/// The screens as the drag found them and what each offers, since a drop is placed by them.
#[derive(Default)]
struct Drag {
    displays: Vec<Display>,
    templates: Vec<Vec<Template>>,
}

pub struct Controller<P> {
    platform: P,
    choice: RefCell<Choice>,
    machine: RefCell<Machine>,
    drag: RefCell<Drag>,
    held: RefCell<Option<Box<dyn Held>>>,
    shown: Cell<View>,
    landing: Cell<Option<Rect>>,
    cells: RefCell<Option<(Rect, Vec<Rect>)>>,
}

impl<P: Platform> Controller<P> {
    pub fn new(platform: P, choice: Choice) -> Self {
        Self {
            platform,
            choice: RefCell::new(choice),
            machine: RefCell::new(Machine::new()),
            drag: RefCell::default(),
            held: RefCell::new(None),
            shown: Cell::new(View::default()),
            landing: Cell::new(None),
            cells: RefCell::new(None),
        }
    }

    /// Takes new lists of layouts; a drag in progress is forgotten.
    pub fn set_choice(&self, choice: Choice) {
        *self.choice.borrow_mut() = choice;
        self.cancel();
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
            let title_bar = is_title_bar_press(p.frame, at, &p.role);
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
        let (displays, templates): (Vec<Display>, Vec<Vec<Template>>) = if on_title_bar {
            let choice = self.choice.borrow();
            self.platform
                .displays()
                .into_iter()
                .map(|(display, name)| (display, choice.for_screen(&name).to_vec()))
                .unzip()
        } else {
            (Vec::new(), Vec::new())
        };
        *self.drag.borrow_mut() = Drag {
            displays: displays.clone(),
            templates: templates.clone(),
        };
        *self.held.borrow_mut() = pressed.map(|p| p.window);
        self.machine
            .borrow_mut()
            .press(displays, templates, at, on_title_bar);
    }

    fn release(&self, at: Point) {
        let dropped = self.machine.borrow_mut().release(at);
        let held = self.held.borrow_mut().take();
        self.sync();
        let (Some(dropped), Some(window)) = (dropped, held) else {
            return;
        };
        let drag = self.drag.borrow();
        let display = drag.displays.get(dropped.display).copied();
        let template = drag
            .templates
            .get(dropped.display)
            .and_then(|t| t.get(dropped.template));
        let (Some(display), Some(template)) = (display, template) else {
            return;
        };
        let Some(cell) = template.cells.get(dropped.cell).copied() else {
            return;
        };
        let id = template.id.clone();
        self.platform.later(Box::new(move || {
            if window.frame().is_some() {
                let target = display.place(cell);
                let ok = window.place(target);
                crate::trace::log(&format!(
                    "snap: -> {target:?} as cell {} of {id} (applied: {ok})",
                    dropped.cell
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
        // The row, or the chip, when the screen it hangs from or what it shows changes.
        let look = |v: View| (v.shown, v.hover, v.chosen, v.compact);
        match (before.shown, now.shown) {
            (_, Some(display)) if look(now) != look(before) => {
                if before.shown.is_none() {
                    self.platform.hold_notch(true);
                }
                let drag = self.drag.borrow();
                if let (Some(d), Some(t)) =
                    (drag.displays.get(display), drag.templates.get(display))
                {
                    self.platform.show(d, t, now);
                }
            }
            (Some(_), None) => {
                self.platform.hide();
                self.platform.hold_notch(false);
            }
            _ => {}
        }
        let cells = self.machine.borrow().overlay();
        if *self.cells.borrow() != cells {
            self.cells.borrow_mut().clone_from(&cells);
            self.platform.cells(cells);
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
    use core_window::templates::{self, built_in};
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
        displays: Vec<(Display, String)>,
    }

    impl Platform for Fake {
        fn trusted(&self) -> bool {
            self.trusted
        }
        fn displays(&self) -> Vec<(Display, String)> {
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
        fn show(&self, display: &Display, templates: &[Template], view: View) {
            self.log.borrow_mut().push(format!(
                "show x{} n{} hover{:?} chosen{:?} compact{}",
                display.frame.x,
                templates.len(),
                view.hover,
                view.chosen,
                view.compact
            ));
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
        fn cells(&self, shown: Option<(Rect, Vec<Rect>)>) {
            self.log
                .borrow_mut()
                .push(shown.map_or("cells none".into(), |(_, cells)| {
                    format!("cells {}", cells.len())
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

    fn defaults() -> Vec<Template> {
        templates::choose::<&str>(&[], &[])
    }

    const FULL: usize = 0;
    const THIRDS: usize = 2;
    const QUARTERS: usize = 3;

    fn rig_with(
        choice: Choice,
        trusted: bool,
        found: Option<(&str, bool, bool)>,
    ) -> (Controller<Fake>, Log) {
        let log: Log = Rc::default();
        let platform = Fake {
            log: Rc::clone(&log),
            trusted,
            found: RefCell::new(
                found.map(|(role, moves, full)| (role.to_owned(), window_frame(), moves, full)),
            ),
            displays: vec![
                (screen(0.0, 0.0, 1920.0, 1080.0), "Built-in".to_owned()),
                (screen(1920.0, -100.0, 2560.0, 1440.0), "DELL".to_owned()),
            ],
        };
        (Controller::new(platform, choice), log)
    }

    fn rig(trusted: bool, found: Option<(&str, bool, bool)>) -> (Controller<Fake>, Log) {
        let choice = Choice {
            default: defaults(),
            screens: Vec::new(),
        };
        rig_with(choice, trusted, found)
    }

    fn lines(log: &Log) -> Vec<String> {
        log.borrow().clone()
    }

    /// The last `n` lines of the log.
    fn tail(lines: &[String], n: usize) -> Vec<&str> {
        lines[lines.len() - n..]
            .iter()
            .map(String::as_str)
            .collect()
    }

    /// The pointer position over the middle of the tile of layout `layout` of display `display`.
    fn over(c: &Controller<Fake>, display: usize, layout: usize) -> Point {
        let d = c.platform.displays[display].0;
        let count = c
            .choice
            .borrow()
            .for_screen(&c.platform.displays[display].1)
            .len();
        let tile = d.cells(count)[layout];
        (tile.x + tile.w / 2.0, tile.y + tile.h / 2.0)
    }

    /// A press on a title bar, the window following the pointer to the top of the first screen.
    fn up_to_the_top(c: &Controller<Fake>) {
        c.handle(Mouse::Down((500.0, 310.0)));
        c.handle(Mouse::Dragged((520.0, 290.0)));
        c.handle(Mouse::Dragged((960.0, 1.0)));
    }

    #[test]
    fn dragging_a_title_bar_onto_a_layout_and_then_into_a_cell_snaps_the_window() {
        let (c, log) = rig(true, Some(("AXWindow", true, false)));
        up_to_the_top(&c);
        let tile = over(&c, 0, THIRDS);
        c.handle(Mouse::Dragged(tile));
        // Down onto the screen, in the first third.
        c.handle(Mouse::Dragged((400.0, 600.0)));
        c.handle(Mouse::Dragged((400.0, 700.0)));
        c.handle(Mouse::Up((400.0, 700.0)));
        assert_eq!(
            lines(&log),
            [
                "grab",
                "moved?",
                "notch true",
                "show x0 n7 hoverNone chosenNone compactfalse",
                "show x0 n7 hoverSome(2) chosenSome(2) compactfalse",
                // The cells of the layout show over the screen as soon as it is chosen.
                "cells 3",
                "show x0 n7 hoverNone chosenSome(2) compacttrue",
                // Where the window will land shows with the cell, and is exactly where it is put.
                "footprint 0 25 640 1055",
                "hide",
                "notch false",
                "cells none",
                "footprint none",
                "later",
                // The first third of the usable area of the first screen.
                "place 0 25 640 1055",
            ]
        );
    }

    #[test]
    fn a_layout_of_one_cell_is_dropped_on_in_the_row_and_on_the_second_screen_uses_that_screens_area()
     {
        let (c, log) = rig(true, Some(("AXStaticText", true, false)));
        c.handle(Mouse::Down((500.0, 310.0)));
        c.handle(Mouse::Dragged((520.0, 290.0)));
        c.handle(Mouse::Dragged((1920.0 + 1280.0, -100.0)));
        let on = over(&c, 1, FULL);
        c.handle(Mouse::Dragged(on));
        c.handle(Mouse::Up(on));
        assert!(
            lines(&log).contains(&"show x1920 n7 hoverNone chosenNone compactfalse".to_owned())
        );
        assert!(lines(&log).contains(&"footprint 1920 -75 2560 1415".to_owned()));
        // The whole usable area of the second screen.
        assert_eq!(lines(&log).last().unwrap(), "place 1920 -75 2560 1415");
    }

    #[test]
    fn each_screen_offers_what_is_chosen_for_it_by_name() {
        let choice = Choice {
            default: defaults(),
            screens: vec![(
                "DELL".to_owned(),
                templates::choose(&["quarters", "halves"], &[]),
            )],
        };
        let (c, log) = rig_with(choice, true, Some(("AXWindow", true, false)));
        c.handle(Mouse::Down((500.0, 310.0)));
        c.handle(Mouse::Dragged((520.0, 290.0)));
        c.handle(Mouse::Dragged((960.0, 1.0)));
        c.handle(Mouse::Dragged((1920.0 + 1280.0, -100.0)));
        let on = over(&c, 1, 0);
        c.handle(Mouse::Dragged(on));
        c.handle(Mouse::Dragged((1920.0 + 100.0, 50.0)));
        c.handle(Mouse::Up((1920.0 + 100.0, 50.0)));
        let l = lines(&log);
        assert!(
            l.contains(&"show x0 n7 hoverNone chosenNone compactfalse".to_owned()),
            "the others have the default"
        );
        assert!(
            l.contains(&"show x1920 n2 hoverNone chosenNone compactfalse".to_owned()),
            "DELL has two"
        );
        assert!(
            l.contains(&"cells 4".to_owned()),
            "its first layout is the quarters"
        );
        // The top left quarter of the usable area of DELL.
        assert_eq!(l.last().unwrap(), "place 1920 -75 1280 708");
    }

    #[test]
    fn without_accessibility_nothing_is_looked_at_or_shown() {
        let (c, log) = rig(false, Some(("AXWindow", true, false)));
        up_to_the_top(&c);
        c.handle(Mouse::Up((960.0, 1.0)));
        assert!(lines(&log).is_empty(), "{:?}", lines(&log));
    }

    #[test]
    fn a_drag_that_started_on_no_window_never_shows_the_row() {
        let (c, log) = rig(true, None);
        up_to_the_top(&c);
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
    fn releasing_on_the_row_over_a_layout_of_several_cells_or_away_from_everything_cancels() {
        let (c, log) = rig(true, Some(("AXWindow", true, false)));
        up_to_the_top(&c);
        let on = over(&c, 0, QUARTERS);
        c.handle(Mouse::Dragged(on));
        c.handle(Mouse::Up(on));
        let l = lines(&log);
        assert!(l.contains(&"cells 4".to_owned()));
        assert!(
            !l.iter().any(|x| x.starts_with("place") || x == "later"),
            "{l:?}"
        );
        assert_eq!(tail(&l, 3), ["hide", "notch false", "cells none"]);

        // Down onto the screen and away again to the menu bar: nothing lit, nothing dropped.
        let (c, log) = rig(true, Some(("AXWindow", true, false)));
        up_to_the_top(&c);
        c.handle(Mouse::Dragged(over(&c, 0, THIRDS)));
        c.handle(Mouse::Dragged((400.0, 600.0)));
        c.handle(Mouse::Dragged((100.0, 10.0)));
        c.handle(Mouse::Up((100.0, 10.0)));
        let l = lines(&log);
        assert!(
            !l.iter().any(|x| x.starts_with("place") || x == "later"),
            "{l:?}"
        );
        assert!(
            l.contains(&"footprint 0 25 640 1055".to_owned())
                && l.last().unwrap() == "footprint none"
        );
    }

    #[test]
    fn the_footprint_follows_the_lit_cell_and_leaves_with_it() {
        let (c, log) = rig(true, Some(("AXWindow", true, false)));
        up_to_the_top(&c);
        c.handle(Mouse::Dragged(over(&c, 0, QUARTERS)));
        for at in [
            (100.0, 100.0),
            (150.0, 120.0),
            (1500.0, 100.0),
            (1500.0, 900.0),
        ] {
            c.handle(Mouse::Dragged(at));
        }
        // Back up to the top edge: the row is whole again, nothing is lit.
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
                "footprint 0 25 960 528",
                // Staying in the cell does not ask again.
                "footprint 960 25 960 528",
                "footprint 960 553 960 527",
                "footprint none",
            ]
        );
        assert!(!l.contains(&"hide".to_owned()), "the row still shows");
        assert_eq!(
            l.iter().filter(|x| x.starts_with("cells")).count(),
            1,
            "the cells stay up"
        );
    }

    #[test]
    fn a_missed_release_takes_the_row_away_and_drops_nothing() {
        let (c, log) = rig(true, Some(("AXWindow", true, false)));
        up_to_the_top(&c);
        c.handle(Mouse::Lost);
        let l = lines(&log);
        assert_eq!(&l[l.len() - 2..], ["hide", "notch false"]);
        assert!(!l.iter().any(|x| x == "later"));
    }

    #[test]
    fn turning_the_feature_off_or_changing_the_layouts_mid_drag_puts_everything_back() {
        let (c, log) = rig(true, Some(("AXWindow", true, false)));
        up_to_the_top(&c);
        c.handle(Mouse::Dragged(over(&c, 0, THIRDS)));
        c.set_choice(Choice {
            default: vec![built_in("full").unwrap()],
            screens: Vec::new(),
        });
        let l = lines(&log);
        assert_eq!(tail(&l, 3), ["hide", "notch false", "cells none"]);
        // The drag goes on, but is forgotten: nothing comes back until a new press.
        c.handle(Mouse::Dragged((960.0, 1.0)));
        assert_eq!(lines(&log).len(), l.len());
    }

    #[test]
    fn a_new_press_while_the_row_is_up_starts_over() {
        let (c, log) = rig(true, Some(("AXWindow", true, false)));
        up_to_the_top(&c);
        *c.platform.found.borrow_mut() = None;
        c.handle(Mouse::Down((100.0, 100.0)));
        let l = lines(&log);
        assert!(l.contains(&"hide".to_owned()) && l.last().unwrap() == "grab");
    }
}
