//! Runs window snapping through the real controller, the real AppKit platform (the row of layouts, the cells over the
//! screen, the footprint, the screens, the notch) and the real pointer translation, without a screen and without any
//! input event: the screen
//! is the pretend one the notch uses in tests (`FIGGHOST_NOTCH_OFFSCREEN`, far outside every real screen), and the
//! pointer is a list of points handed to the controller as the monitor would hand them.
//!
//! The dragged window is a script, not a real window: macOS does not list a window outside every screen among
//! the Accessibility windows of its app (checked), so the real find-and-move path cannot be driven without
//! putting a window on the screen. That path is covered by `macos-sys/grab_check` (find, read) and by the
//! tiling code that already places windows the same way.
//!
//!   FIGGHOST_NOTCH_OFFSCREEN=1 target/debug/examples/snap_check [out-dir]

#![allow(dead_code)]

mod trace {
    pub fn log(_: &str) {}
}
#[path = "../src/features/snap/controller.rs"]
mod controller;
#[path = "../src/i18n.rs"]
mod i18n;
#[path = "../src/features/snap/platform.rs"]
mod platform;
#[path = "../src/features/snap/text.rs"]
mod text;

use controller::{Choice, Controller, Held, Platform, Pressed};
use core_window::{
    Rect,
    snap::{self, View},
    templates::{self, Template},
};
use macos_sys::{
    MainThreadMarker,
    notch::{Notch, NotchGeometry},
    snap_ui::{self, Mouse},
    ui::AppCallbacks,
};
use objc2_app_kit::{
    NSApplication, NSApplicationActivationPolicy, NSEvent, NSEventModifierFlags, NSEventType,
    NSNormalWindowLevel, NSScreen, NSWindow, NSWindowCollectionBehavior,
};
use objc2_foundation::{NSDate, NSPoint, NSRect, NSRunLoop};
use std::{
    cell::{Cell, RefCell},
    fs,
    path::PathBuf,
    process,
    rc::Rc,
};

fn settle(seconds: f64) {
    NSRunLoop::currentRunLoop().runUntilDate(&NSDate::dateWithTimeIntervalSinceNow(seconds));
}

struct Report {
    failed: u32,
}

impl Report {
    fn check(&mut self, ok: bool, what: &str) {
        println!("  {} {what}", if ok { "ok  " } else { "FAIL" });
        if !ok {
            self.failed += 1;
        }
    }
}

fn near(a: f64, b: f64) -> bool {
    (a - b).abs() <= 2.0
}

fn same(a: Rect, b: Rect) -> bool {
    near(a.x, b.x) && near(a.y, b.y) && near(a.w, b.w) && near(a.h, b.h)
}

fn middle(r: Rect) -> (f64, f64) {
    (r.x + r.w / 2.0, r.y + r.h / 2.0)
}

/// The window being dragged, as a script: it moves when the check says it moves and keeps what it was told to
/// become.
struct Script {
    pressed: Cell<Rect>,
    frame: Cell<Rect>,
    present: Cell<bool>,
    role: RefCell<String>,
    full_screen: Cell<bool>,
    placed: RefCell<Vec<Rect>>,
}

struct Handle(Rc<Script>);

impl Held for Handle {
    fn moved(&self) -> bool {
        let (a, b) = (self.0.pressed.get(), self.0.frame.get());
        (a.x - b.x).abs() >= 2.0 || (a.y - b.y).abs() >= 2.0
    }
    fn frame(&self) -> Option<Rect> {
        Some(self.0.frame.get())
    }
    fn full_screen(&self) -> bool {
        self.0.full_screen.get()
    }
    fn place(&self, frame: Rect) -> bool {
        self.0.placed.borrow_mut().push(frame);
        self.0.frame.set(frame);
        true
    }
}

/// Everything real except the window under the pointer.
struct Hybrid {
    real: platform::AppKit,
    window: Rc<Script>,
}

impl Platform for Hybrid {
    fn trusted(&self) -> bool {
        true
    }
    fn displays(&self) -> Vec<(snap::Display, String)> {
        self.real.displays()
    }
    fn grab(&self, _at: snap::Point) -> Option<Pressed> {
        let w = &self.window;
        w.present.get().then(|| {
            w.pressed.set(w.frame.get());
            Pressed {
                window: Box::new(Handle(Rc::clone(w))),
                role: w.role.borrow().clone(),
                frame: w.frame.get(),
            }
        })
    }
    fn show(&self, display: &snap::Display, templates: &[Template], view: View) {
        self.real.show(display, templates, view);
    }
    fn hide(&self) {
        self.real.hide();
    }
    fn footprint(&self, frame: Option<Rect>) {
        self.real.footprint(frame);
    }
    fn cells(&self, shown: Option<(Rect, Vec<Rect>)>) {
        self.real.cells(shown);
    }
    fn hold_notch(&self, held: bool) {
        self.real.hold_notch(held);
    }
    fn later(&self, f: Box<dyn FnOnce()>) {
        self.real.later(f);
    }
}

/// The panel that holds a view of class `view`: the row, the cells or the footprint.
fn panel_of(mtm: MainThreadMarker, view: &str) -> Option<objc2::rc::Retained<NSWindow>> {
    NSApplication::sharedApplication(mtm)
        .windows()
        .iter()
        .find(|w| {
            w.contentView()
                .is_some_and(|v| v.class().name().to_str() == Ok(view))
        })
}

fn row_panel(mtm: MainThreadMarker) -> Option<objc2::rc::Retained<NSWindow>> {
    panel_of(mtm, "FigSnapZones")
}

fn footprint_panel(mtm: MainThreadMarker) -> Option<objc2::rc::Retained<NSWindow>> {
    panel_of(mtm, "FigSnapFootprint")
}

fn cells_panel(mtm: MainThreadMarker) -> Option<objc2::rc::Retained<NSWindow>> {
    panel_of(mtm, "FigSnapCells")
}

fn visible_cells(mtm: MainThreadMarker) -> bool {
    cells_panel(mtm).is_some_and(|w| w.isVisible() && w.alphaValue() > 0.5)
}

fn visible_row(mtm: MainThreadMarker) -> bool {
    row_panel(mtm).is_some_and(|w| w.isVisible() && w.alphaValue() > 0.5)
}

fn visible_footprint(mtm: MainThreadMarker) -> bool {
    footprint_panel(mtm).is_some_and(|w| w.isVisible() && w.alphaValue() > 0.5)
}

/// A window frame in the coordinates of the drag: top-left origin, y down.
fn top_left(mtm: MainThreadMarker, f: NSRect) -> Rect {
    let primary = NSScreen::screens(mtm)
        .firstObject()
        .map_or(0.0, |s| s.frame().size.height);
    Rect::new(
        f.origin.x,
        primary - (f.origin.y + f.size.height),
        f.size.width,
        f.size.height,
    )
}

/// Where the footprint is now, while it is on screen.
fn footprint_frame(mtm: MainThreadMarker) -> Option<Rect> {
    footprint_panel(mtm)
        .filter(|w| w.isVisible())
        .map(|w| top_left(mtm, w.frame()))
}

fn live_png(mtm: MainThreadMarker) -> Option<Vec<u8>> {
    let view = row_panel(mtm)?.contentView()?;
    macos_sys::widgets::view_png(&view)
}

fn main() {
    if std::env::var_os("FIGGHOST_NOTCH_OFFSCREEN").is_none() {
        eprintln!(
            "refusing to run without FIGGHOST_NOTCH_OFFSCREEN=1: the pretend screen keeps it off the real ones"
        );
        process::exit(2);
    }
    let out = PathBuf::from(
        std::env::args()
            .nth(1)
            .unwrap_or_else(|| "/tmp/snap-live".into()),
    );
    fs::create_dir_all(&out).expect("output directory");
    let mtm = MainThreadMarker::new().expect("main thread");
    NSApplication::sharedApplication(mtm)
        .setActivationPolicy(NSApplicationActivationPolicy::Accessory);
    let mut r = Report { failed: 0 };

    println!("the pointer and the screens");
    let screens = snap_ui::displays(mtm);
    r.check(
        screens.len() == 1 && screens[0].frame.x <= -10_000.0,
        &format!(
            "one pretend screen far outside the real ones: {:?}",
            screens.first().map(|s| s.frame)
        ),
    );
    let screen = screens[0].clone();
    // Events made in-process and never posted: the monitor's translation turns them into the controller's input.
    let made = |kind, x: f64, y: f64| {
        NSEvent::mouseEventWithType_location_modifierFlags_timestamp_windowNumber_context_eventNumber_clickCount_pressure(
            kind,
            NSPoint::new(x, y),
            NSEventModifierFlags::empty(),
            0.0,
            0,
            None,
            1,
            1,
            1.0,
        )
        .expect("event")
    };
    let down = snap_ui::translate(mtm, &made(NSEventType::LeftMouseDown, 100.0, 900.0));
    let drag = snap_ui::translate(mtm, &made(NSEventType::LeftMouseDragged, 100.0, 900.0));
    let up = snap_ui::translate(mtm, &made(NSEventType::LeftMouseUp, 100.0, 900.0));
    let moved = snap_ui::translate(mtm, &made(NSEventType::MouseMoved, 100.0, 900.0));
    r.check(
        matches!(down, Some(Mouse::Down(_)))
            && matches!(drag, Some(Mouse::Dragged(_)))
            && matches!(up, Some(Mouse::Up(_))),
        "the three left button events are translated",
    );
    r.check(moved.is_none(), "a plain pointer move is not a drag event");
    let at = |m: Option<Mouse>| match m {
        Some(Mouse::Down(p) | Mouse::Dragged(p) | Mouse::Up(p)) => Some(p),
        _ => None,
    };
    r.check(
        at(down) == at(drag) && at(drag) == at(up) && at(down).is_some_and(|p| p.0 == 100.0),
        "the position is the event's own, flipped to top-left",
    );
    r.check(
        snap_ui::MouseMonitor::start(mtm, |_| {}).is_some(),
        "the global monitor can be installed (and is removed again)",
    );

    println!("the real panel, screen and notch under the real controller");
    let geometry = NotchGeometry::detect(mtm).expect("pretend screen");
    let noop = AppCallbacks {
        on_query: Box::new(|_, _| {}),
        on_activate: Box::new(|_, _, _| {}),
        on_pin: Box::new(|_, _| {}),
        on_delete: Box::new(|_, _| {}),
    };
    let notch = Rc::new(Notch::new(mtm, geometry, noop, |_| {}));
    let layouts = templates::choose::<&str>(&[], &[]);
    let (full, thirds, quarters) = (0, 2, 3);
    let start = Rect::new(screen.frame.x + 700.0, screen.frame.y + 300.0, 520.0, 360.0);
    let script = Rc::new(Script {
        pressed: Cell::new(start),
        frame: Cell::new(start),
        present: Cell::new(true),
        role: RefCell::new("AXWindow".into()),
        full_screen: Cell::new(false),
        placed: RefCell::default(),
    });
    let real = platform::AppKit::new(mtm, Some(Rc::clone(&notch)));
    let display = real.displays()[0].0;
    let platform = Hybrid {
        real,
        window: Rc::clone(&script),
    };
    let c = Controller::new(
        platform,
        Choice {
            default: layouts.clone(),
            screens: Vec::new(),
        },
    );
    let spec_panel = display.panel(layouts.len());
    let chip = display.chip();
    let tiles = display.cells(layouts.len());
    let centre_x = display.frame.x + display.frame.w / 2.0;
    let top = display.frame.y;
    let title = |f: Rect| (f.x + f.w / 2.0, f.y + 12.0);
    // A point in the usable area, in the cell `cell` of the layout `layout`.
    let in_cell = |layout: usize, cell: usize| {
        let frame = display.place(layouts[layout].cells[cell]);
        (frame.x + frame.w / 2.0, frame.y + frame.h / 2.0)
    };

    // Moves the window and the pointer together, as a drag does, in a few steps.
    let drag_to = |pointer: &mut (f64, f64), to: (f64, f64)| {
        let steps = 6;
        let (dx, dy) = (
            (to.0 - pointer.0) / f64::from(steps),
            (to.1 - pointer.1) / f64::from(steps),
        );
        for _ in 0..steps {
            pointer.0 += dx;
            pointer.1 += dy;
            let now = script.frame.get();
            script
                .frame
                .set(Rect::new(now.x + dx, now.y + dy, now.w, now.h));
            c.handle(Mouse::Dragged(*pointer));
            settle(0.03);
        }
    };

    println!("drag onto the thirds, down into the second cell and drop");
    let mut pointer = title(start);
    c.handle(Mouse::Down(pointer));
    r.check(!visible_row(mtm), "nothing shows at the press");
    notch.open();
    r.check(
        notch.is_expanded(),
        "the notch is open before the row comes (the case to handle)",
    );
    drag_to(&mut pointer, (centre_x, top + 1.0));
    settle(0.9);
    r.check(
        visible_row(mtm),
        "the row shows once the pointer reaches the top edge",
    );
    r.check(!notch.is_expanded(), "the open notch was closed for it");
    if let Some(panel) = row_panel(mtm) {
        let f = panel.frame();
        r.check(
            near(f.size.width, spec_panel.w) && near(f.size.height, spec_panel.h),
            &format!(
                "the row has its full size at rest: {} x {}",
                f.size.width, f.size.height
            ),
        );
        r.check(
            near(f.origin.x + f.size.width / 2.0, centre_x),
            "and is centred on the screen",
        );
        r.check(
            !panel.isKeyWindow() && !panel.canBecomeKeyWindow() && !panel.canBecomeMainWindow(),
            "and cannot take the keyboard focus",
        );
        r.check(panel.ignoresMouseEvents(), "and lets clicks through");
    }
    r.check(
        !visible_footprint(mtm) && !visible_cells(mtm),
        "no cells and no footprint while no layout is chosen",
    );
    let rest = live_png(mtm);
    let on = middle(tiles[thirds]);
    c.handle(Mouse::Dragged(on));
    settle(0.4);
    if let (Some(a), Some(b)) = (rest, live_png(mtm)) {
        r.check(a != b, "pointing at a layout changes what is drawn");
        fs::write(out.join("live-hover.png"), b).expect("write");
    }
    r.check(visible_cells(mtm), "its cells show over the screen at once");
    if let Some(cells) = cells_panel(mtm) {
        r.check(
            same(top_left(mtm, cells.frame()), display.area),
            &format!(
                "over the usable area: {:?}, want {:?}",
                top_left(mtm, cells.frame()),
                display.area
            ),
        );
        r.check(
            cells.ignoresMouseEvents()
                && !cells.canBecomeKeyWindow()
                && !cells.canBecomeMainWindow(),
            "they let clicks through and cannot take the focus",
        );
        r.check(
            cells
                .collectionBehavior()
                .contains(NSWindowCollectionBehavior::IgnoresCycle),
            "and stay out of the window cycle",
        );
        if let (Some(f), Some(row)) = (footprint_panel(mtm), row_panel(mtm)) {
            r.check(
                cells.level() < f.level() && f.level() < row.level() && cells.level() > NSNormalWindowLevel,
                &format!(
                    "they sit under the footprint, which sits under the row, over ordinary windows: {} < {} < {}",
                    cells.level(),
                    f.level(),
                    row.level()
                ),
            );
        }
        fs::write(
            out.join("live-cells.png"),
            macos_sys::widgets::view_png(&cells.contentView().expect("view")).unwrap_or_default(),
        )
        .expect("write");
    }
    r.check(
        !visible_footprint(mtm),
        "three cells: nothing is lit while the pointer is on the layout",
    );

    // Down onto the screen, into the first third: the row shrinks to a chip and the cell lights.
    let first = display.place(layouts[thirds].cells[0]);
    pointer = on;
    drag_to(&mut pointer, in_cell(thirds, 0));
    settle(0.5);
    if let Some(panel) = row_panel(mtm) {
        let f = panel.frame();
        r.check(
            near(f.size.width, chip.w) && near(f.size.height, chip.h),
            &format!(
                "the row has shrunk to the chip: {} x {}, want {} x {}",
                f.size.width, f.size.height, chip.w, chip.h
            ),
        );
        r.check(
            near(f.origin.x + f.size.width / 2.0, centre_x),
            "which is centred as the row was",
        );
        fs::write(out.join("live-chip.png"), live_png(mtm).unwrap_or_default()).expect("write");
    }
    r.check(
        visible_cells(mtm) && visible_footprint(mtm) && footprint_frame(mtm).is_some_and(|f| same(f, first)),
        &format!(
            "the cells stay and the first one is lit where the window will land: {:?}, want {first:?}",
            footprint_frame(mtm)
        ),
    );
    if let (Some(f), Some(row)) = (footprint_panel(mtm), row_panel(mtm)) {
        r.check(
            f.ignoresMouseEvents() && !f.canBecomeKeyWindow() && !f.canBecomeMainWindow(),
            "the footprint lets clicks through and cannot take the focus",
        );
        r.check(
            f.level() < row.level() && f.level() > NSNormalWindowLevel,
            &format!(
                "it sits under the row and over ordinary windows: {} < {} ",
                f.level(),
                row.level()
            ),
        );
    }

    // Over another cell, the footprint moves to that cell's frame: gliding, or at once with Reduce Motion.
    let want = display.place(layouts[thirds].cells[1]);
    let before = footprint_frame(mtm).expect("a footprint");
    c.handle(Mouse::Dragged(in_cell(thirds, 1)));
    let mut seen = Vec::new();
    let began = std::time::Instant::now();
    while began.elapsed().as_secs_f64() < 0.4 {
        settle(0.008);
        seen.extend(footprint_frame(mtm));
    }
    let between = |f: &Rect| {
        f.x > before.x + 2.0 && f.x < want.x - 2.0 && !same(*f, before) && !same(*f, want)
    };
    if macos_sys::workspace::display_options().reduce_motion {
        r.check(
            seen.first().is_some_and(|f| same(*f, want)),
            "with Reduce Motion it moves to the second cell at once",
        );
    } else {
        r.check(
            seen.iter().any(between),
            "it glides to the second cell through the frames between",
        );
    }
    r.check(
        seen.last().is_some_and(|f| same(*f, want)) && visible_footprint(mtm),
        &format!("and rests there: {:?}", seen.last()),
    );
    let on_second = in_cell(thirds, 1);
    c.handle(Mouse::Dragged(in_cell(thirds, 2)));
    settle(0.3);
    c.handle(Mouse::Dragged(on_second));
    settle(0.4);
    r.check(
        footprint_frame(mtm).is_some_and(|f| same(f, want)),
        "back over the second cell, it is back there",
    );

    c.handle(Mouse::Up(on_second));
    settle(0.4);
    r.check(
        script.placed.borrow().len() == 1 && same(script.placed.borrow()[0], want),
        &format!(
            "the window is placed in the second third, where the footprint was: {:?}",
            script.placed.borrow().first()
        ),
    );
    settle(0.5);
    r.check(!visible_row(mtm), "the row is gone after the drop");
    r.check(
        footprint_frame(mtm).is_none() && !visible_cells(mtm),
        "and so are the footprint and the cells",
    );

    println!(
        "the way back to the top, another layout, and a layout of one cell dropped on in the row"
    );
    script.frame.set(start);
    script.placed.borrow_mut().clear();
    pointer = title(start);
    c.handle(Mouse::Down(pointer));
    drag_to(&mut pointer, (centre_x, top + 1.0));
    settle(0.8);
    c.handle(Mouse::Dragged(middle(tiles[thirds])));
    drag_to(&mut pointer, in_cell(thirds, 0));
    settle(0.5);
    r.check(
        visible_cells(mtm) && visible_footprint(mtm),
        "the cells and a lit one show",
    );
    // Back up to the top edge: the chip is the row again, with the cells still showing, nothing lit.
    drag_to(&mut pointer, (centre_x, top + 1.0));
    settle(0.5);
    r.check(
        row_panel(mtm).is_some_and(|w| near(w.frame().size.width, spec_panel.w)),
        "the row is whole again at the top edge",
    );
    r.check(
        visible_cells(mtm) && footprint_frame(mtm).is_none(),
        "with the last choice still on the screen and nothing lit",
    );
    c.handle(Mouse::Dragged(middle(tiles[quarters])));
    settle(0.3);
    r.check(
        visible_cells(mtm),
        "pointing at the quarters shows their cells",
    );
    c.handle(Mouse::Dragged(middle(tiles[full])));
    settle(0.5);
    let whole = display.area;
    r.check(
        footprint_frame(mtm).is_some_and(|f| same(f, whole)),
        &format!(
            "on the layout of one cell the whole area is the footprint: {:?}",
            footprint_frame(mtm)
        ),
    );
    c.handle(Mouse::Up(middle(tiles[full])));
    settle(0.4);
    r.check(
        script.placed.borrow().len() == 1 && same(script.placed.borrow()[0], whole),
        &format!(
            "and a drop there fills it: {:?}",
            script.placed.borrow().first()
        ),
    );
    settle(0.5);

    println!("drag onto a layout, go down, leave and release elsewhere");
    script.frame.set(start);
    script.placed.borrow_mut().clear();
    pointer = title(start);
    c.handle(Mouse::Down(pointer));
    drag_to(&mut pointer, (centre_x, top + 1.0));
    settle(0.8);
    r.check(visible_row(mtm), "the row shows");
    c.handle(Mouse::Dragged(middle(tiles[quarters])));
    settle(0.3);
    r.check(visible_cells(mtm), "the cells show over a layout");
    c.handle(Mouse::Up(middle(tiles[quarters])));
    settle(0.5);
    r.check(
        script.placed.borrow().is_empty() && !visible_row(mtm) && !visible_cells(mtm),
        "released on a layout of four cells: nothing is placed and everything goes",
    );
    script.frame.set(start);
    pointer = title(start);
    c.handle(Mouse::Down(pointer));
    drag_to(&mut pointer, (centre_x, top + 1.0));
    settle(0.8);
    c.handle(Mouse::Dragged(middle(tiles[quarters])));
    drag_to(&mut pointer, in_cell(quarters, 3));
    settle(0.5);
    r.check(visible_footprint(mtm), "the footprint shows in a cell");
    // Off the edge of the screen, to another one: no cell is there, and all of it goes. (The pretend screen has no
    // menu bar to leave by; `core-window` tests that.)
    let away = (display.frame.x - 60.0, top + 500.0);
    drag_to(&mut pointer, away);
    settle(0.6);
    r.check(
        !visible_row(mtm) && !visible_cells(mtm) && footprint_frame(mtm).is_none(),
        "leaving the screen hides the row, the cells and the footprint",
    );
    c.handle(Mouse::Up(away));
    settle(0.3);
    r.check(
        script.placed.borrow().is_empty(),
        "released elsewhere: the window is not touched",
    );

    println!("drags that must not show the row");
    let silent = |label: &str, r: &mut Report, setup: &dyn Fn()| {
        script.frame.set(start);
        setup();
        c.handle(Mouse::Down((start.x + start.w / 2.0, start.y + 12.0)));
        for i in 1..=14 {
            let k = f64::from(i);
            c.handle(Mouse::Dragged((
                start.x + 260.0 + k * 9.0,
                start.y + 12.0 - k * 12.0,
            )));
        }
        c.handle(Mouse::Dragged((centre_x, top + 1.0)));
        settle(0.7);
        r.check(!visible_row(mtm), label);
        c.handle(Mouse::Up((centre_x, top + 1.0)));
    };
    silent("a press on no window", &mut r, &|| {
        script.present.set(false)
    });
    script.present.set(true);
    *script.role.borrow_mut() = "AXButton".into();
    silent("a press on a control of the title bar", &mut r, &|| {});
    *script.role.borrow_mut() = "AXWindow".into();
    silent(
        "a press on the title bar of a window that stays put",
        &mut r,
        &|| {},
    );
    script.full_screen.set(true);
    silent("a full-screen window", &mut r, &|| {});
    script.full_screen.set(false);
    // Pressed below the title bar strip.
    script.frame.set(start);
    c.handle(Mouse::Down((start.x + 260.0, start.y + 200.0)));
    drag_to(
        &mut (start.x + 260.0, start.y + 200.0),
        (centre_x, top + 1.0),
    );
    settle(0.7);
    r.check(!visible_row(mtm), "a press in the content of the window");
    c.handle(Mouse::Up((centre_x, top + 1.0)));
    r.check(
        script.placed.borrow().is_empty(),
        "and nothing was ever placed",
    );

    println!("a missed release");
    script.frame.set(start);
    pointer = title(start);
    c.handle(Mouse::Down(pointer));
    drag_to(&mut pointer, (centre_x, top + 1.0));
    settle(0.8);
    r.check(visible_row(mtm), "the row shows");
    c.handle(Mouse::Dragged(middle(tiles[full])));
    settle(0.3);
    r.check(
        visible_footprint(mtm) && visible_cells(mtm),
        "the footprint and the cells show over a layout of one cell",
    );
    c.handle(Mouse::Lost);
    settle(0.6);
    r.check(
        !visible_row(mtm) && footprint_frame(mtm).is_none() && !visible_cells(mtm),
        "a lost release takes all three away",
    );
    r.check(script.placed.borrow().is_empty(), "and places nothing");

    println!("motion");
    if macos_sys::workspace::display_options().reduce_motion {
        println!(
            "  SKIP Reduce Motion is on: the row appears at once, which is its behaviour then"
        );
    } else {
        script.frame.set(start);
        pointer = title(start);
        c.handle(Mouse::Down(pointer));
        drag_to(&mut pointer, (centre_x, top + 40.0));
        let sizes = |seconds: f64| {
            let mut seen = Vec::new();
            let began = std::time::Instant::now();
            while began.elapsed().as_secs_f64() < seconds {
                settle(0.008);
                seen.push(row_panel(mtm).map_or((0.0, 0.0), |w| {
                    if w.isVisible() {
                        (w.frame().size.width, w.frame().size.height)
                    } else {
                        (0.0, 0.0)
                    }
                }));
            }
            seen
        };
        c.handle(Mouse::Dragged((centre_x, top + 1.0)));
        let opening: Vec<f64> = sizes(0.8).iter().map(|s| s.1).collect();
        let full_height = spec_panel.h;
        let peak = opening.iter().copied().fold(0.0, f64::max);
        r.check(
            opening.iter().any(|h| *h > 0.0 && *h < full_height * 0.5),
            "it starts small and grows",
        );
        r.check(
            peak > full_height * 1.01 && peak < full_height * 1.1,
            &format!("with one small overshoot on a spring: peak {peak:.1} of {full_height}"),
        );
        r.check(
            near(*opening.last().unwrap(), full_height),
            "and comes to rest at its full size",
        );
        // Onto a layout and down onto the screen: the row eases down to the chip, without a bounce.
        c.handle(Mouse::Dragged(middle(tiles[thirds])));
        settle(0.3);
        c.handle(Mouse::Dragged(in_cell(thirds, 0)));
        let shrinking = sizes(0.6);
        let narrowing = shrinking.windows(2).all(|w| w[1].0 <= w[0].0 + 0.5);
        r.check(
            narrowing,
            "going down onto the screen, the row only narrows (ease out, no bounce)",
        );
        r.check(
            shrinking
                .last()
                .is_some_and(|s| near(s.0, chip.w) && near(s.1, chip.h)),
            "and comes to rest as the chip",
        );
        c.handle(Mouse::Lost);
        let closing: Vec<f64> = sizes(0.6).iter().map(|s| s.1).collect();
        let falling = closing
            .windows(2)
            .all(|w| w[1] <= w[0] + 0.5 || w[1] == 0.0);
        r.check(falling, "leaving, it only shrinks (ease out, no bounce)");
        r.check(
            *closing.last().unwrap() == 0.0,
            "and is off screen at the end",
        );
    }

    println!(
        "{}",
        if r.failed == 0 {
            "snap check: pass"
        } else {
            "snap check: FAIL"
        }
    );
    process::exit(i32::from(r.failed != 0));
}
