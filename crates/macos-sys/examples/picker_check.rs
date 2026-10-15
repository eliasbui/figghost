//! Checks the colour picker in this process with a synthetic screen: panels sit far outside every display,
//! nothing real is read, no input is posted and no key focus is taken. Events are delivered with
//! `NSWindow::sendEvent`. Writes the loupe to PNG files in the folder given as the first argument.
//! `cargo run -p macos-sys --example picker_check -- <folder>`

use macos_sys::{
    MainThreadMarker,
    pasteboard::{Content, Pasteboard},
    picker::{Callbacks, Picker, Source, Strings, place, sample::Grid},
};
use objc2_app_kit::{
    NSApplication, NSApplicationActivationPolicy, NSEvent, NSEventModifierFlags, NSEventType,
    NSPanel,
};
use objc2_foundation::{NSDate, NSPoint, NSRect, NSRunLoop, NSSize, NSString};
use std::{
    cell::{Cell, RefCell},
    fs,
    path::PathBuf,
    process,
    rc::Rc,
};

const LEFT: NSRect = NSRect::new(NSPoint::new(-30000.0, 0.0), NSSize::new(800.0, 600.0));
const RIGHT: NSRect = NSRect::new(NSPoint::new(-29000.0, 0.0), NSSize::new(800.0, 600.0));

/// The colour of the synthetic screen at a pixel: blocks of four pixels, red follows x, green follows y,
/// blue is fixed, so the loupe has edges to show.
fn screen_colour(x: f64, y: f64) -> [u8; 3] {
    let block = |v: f64, step: f64| ((v / 4.0).floor() * step).rem_euclid(256.0) as u8;
    [block(x, 37.0), block(y, 53.0), 0x40]
}

fn hex(c: [u8; 3]) -> String {
    format!("#{:02X}{:02X}{:02X}", c[0], c[1], c[2])
}

fn key(panel: &NSPanel, code: u16, shift: bool) {
    let flags = if shift {
        NSEventModifierFlags::Shift
    } else {
        NSEventModifierFlags::empty()
    };
    let chars = NSString::from_str("");
    let event = NSEvent::keyEventWithType_location_modifierFlags_timestamp_windowNumber_context_characters_charactersIgnoringModifiers_isARepeat_keyCode(
        NSEventType::KeyDown, NSPoint::ZERO, flags, 0.0, panel.windowNumber(), None, &chars, &chars, false, code,
    )
    .expect("key event");
    panel.sendEvent(&event);
}

/// A mouse event at a point in global coordinates, delivered to `panel`.
fn mouse(panel: &NSPanel, kind: NSEventType, at: NSPoint) {
    let origin = panel.frame().origin;
    let local = NSPoint::new(at.x - origin.x, at.y - origin.y);
    let event = NSEvent::mouseEventWithType_location_modifierFlags_timestamp_windowNumber_context_eventNumber_clickCount_pressure(
        kind, local, NSEventModifierFlags::empty(), 0.0, panel.windowNumber(), None, 0, 1, 1.0,
    )
    .expect("mouse event");
    panel.sendEvent(&event);
}

fn spin(seconds: f64) {
    NSRunLoop::currentRunLoop().runUntilDate(&NSDate::dateWithTimeIntervalSinceNow(seconds));
}

fn main() {
    let folder = PathBuf::from(std::env::args().nth(1).expect("a folder for the PNG files"));
    fs::create_dir_all(&folder).unwrap();
    let mtm = MainThreadMarker::new().expect("main thread");
    NSApplication::sharedApplication(mtm)
        .setActivationPolicy(NSApplicationActivationPolicy::Accessory);

    let board = Pasteboard::named(&format!("figghost-picker-check-{}", process::id()));
    let board = Rc::new(board);
    let allowed = Rc::new(Cell::new(true));
    let grabs: Rc<RefCell<Vec<NSPoint>>> = Rc::default();
    let below_seen = Rc::new(Cell::new(0u32));

    let source = Source {
        access: {
            let a = Rc::clone(&allowed);
            Box::new(move || a.get())
        },
        grab: {
            let (grabs, below_seen) = (Rc::clone(&grabs), Rc::clone(&below_seen));
            Box::new(move |p, cells, below| {
                grabs.borrow_mut().push(p);
                below_seen.set(below);
                let half = (cells / 2) as f64;
                let mut rgb = Vec::new();
                for y in 0..cells {
                    for x in 0..cells {
                        rgb.push(screen_colour(
                            p.x.floor() + x as f64 - half,
                            p.y.floor() + y as f64 - half,
                        ));
                    }
                }
                Some(Grid { cells, rgb })
            })
        },
        screens: Box::new(|| vec![LEFT, RIGHT]),
    };
    let strings = Strings {
        hint: "Tab format   Click copy   Esc exit".into(),
        no_access: "Allow Screen Recording in System Settings.".into(),
        formats: ["HEX", "RGB", "HSL", "HSB"].map(String::from).to_vec(),
    };
    let callbacks = Callbacks {
        render: Box::new(|c, format| match format {
            0 => hex(c),
            1 => format!("rgb({}, {}, {})", c[0], c[1], c[2]),
            2 => "hsl".into(),
            _ => "hsb".into(),
        }),
        on_copy: {
            let board = Rc::clone(&board);
            Box::new(move |text| {
                board.write(macos_sys::pasteboard::Write::Text(&text));
            })
        },
    };
    let picker = Picker::new(mtm, strings, callbacks, source);

    let mut failures = 0;
    let mut check = |ok: bool, what: &str| {
        println!("{}: {what}", if ok { "pass" } else { "FAIL" });
        if !ok {
            failures += 1;
        }
    };
    let text = || match board.read() {
        Some(Content::Text(t)) => Some(t),
        _ => None,
    };

    // Start on the second display.
    let start = NSPoint::new(-28800.0, 300.0);
    picker.start(0, start);
    check(picker.is_active(), "the picker starts");
    let loupe = picker.loupe_frame().unwrap();
    check(
        loupe.origin.x >= RIGHT.origin.x
            && loupe.origin.x + loupe.size.width <= RIGHT.origin.x + RIGHT.size.width,
        "the loupe is inside the display under the pointer",
    );
    check(
        !(start.x >= loupe.origin.x
            && start.x <= loupe.origin.x + loupe.size.width
            && start.y >= loupe.origin.y
            && start.y <= loupe.origin.y + loupe.size.height),
        "the loupe does not cover the pointer",
    );
    check(
        below_seen.get() != 0,
        "the read is limited to what is below the catching panel",
    );
    fs::write(folder.join("loupe-hex.png"), picker.loupe_png().unwrap()).unwrap();

    let overlay = picker.catcher_window(1).unwrap();
    key(&overlay, 48, false);
    check(picker.format_index() == 1, "Tab goes to the next format");
    fs::write(folder.join("loupe-rgb.png"), picker.loupe_png().unwrap()).unwrap();
    key(&overlay, 48, true);
    key(&overlay, 48, true);
    check(
        picker.format_index() == 3,
        "Shift+Tab goes back, and wraps around",
    );
    key(&overlay, 48, false);
    check(
        picker.format_index() == 0,
        "Tab wraps around to the first format",
    );

    // A burst of moves is read a few times, and the last position is not lost.
    let before = grabs.borrow().len();
    for i in 0..200 {
        mouse(
            &overlay,
            NSEventType::MouseMoved,
            NSPoint::new(-28900.0 + f64::from(i) * 0.5, 200.0),
        );
    }
    let burst = grabs.borrow().len() - before;
    check(
        burst <= 2,
        &format!("200 moves at once made {burst} reads, not 200"),
    );
    spin(0.15);
    let last = *grabs.borrow().last().unwrap();
    check(
        (last.x - (-28900.0 + 199.0 * 0.5)).abs() < 1e-6,
        "the read after the burst is of the last position",
    );
    let idle = grabs.borrow().len();
    spin(0.3);
    check(
        grabs.borrow().len() == idle,
        "no reads while the pointer is still",
    );

    // The pointer moves to the first display.
    mouse(
        &overlay,
        NSEventType::MouseMoved,
        NSPoint::new(-29900.0, 100.0),
    );
    spin(0.1);
    let loupe = picker.loupe_frame().unwrap();
    check(
        loupe.origin.x >= LEFT.origin.x
            && loupe.origin.x + loupe.size.width <= LEFT.origin.x + LEFT.size.width,
        "the loupe follows the pointer to the other display",
    );
    let other = picker.catcher_window(0).unwrap();
    println!(
        "note: the panel of the first display is key: {}",
        other.isKeyWindow()
    );

    // A click copies the value in the current format and ends the picker.
    let target = NSPoint::new(-29900.0, 100.0);
    mouse(&other, NSEventType::LeftMouseDown, target);
    check(!picker.is_active(), "a click ends the picker");
    let expected = hex(screen_colour(target.x.floor(), target.y.floor()));
    check(
        text().as_deref() == Some(expected.as_str()),
        &format!("the click copied {expected}: {:?}", text()),
    );
    check(
        !other.isVisible(),
        "the panels are off the screen afterwards",
    );

    // The format chosen with Tab is the one copied.
    picker.start(0, start);
    let overlay = picker.catcher_window(1).unwrap();
    key(&overlay, 48, false);
    mouse(&overlay, NSEventType::LeftMouseDown, start);
    let expected = format!(
        "rgb({}, {}, {})",
        screen_colour(start.x.floor(), start.y.floor())[0],
        screen_colour(start.x.floor(), start.y.floor())[1],
        0x40
    );
    check(
        text().as_deref() == Some(expected.as_str()),
        &format!("Tab then click copied {expected}: {:?}", text()),
    );

    // Escape ends it and copies nothing.
    let count = board.change_count();
    picker.start(0, start);
    let overlay = picker.catcher_window(1).unwrap();
    key(&overlay, 53, false);
    check(
        !picker.is_active() && board.change_count() == count,
        "Escape ends it and copies nothing",
    );
    // So does the right button, and a second start works after an end.
    picker.start(0, start);
    let overlay = picker.catcher_window(1).unwrap();
    mouse(&overlay, NSEventType::RightMouseDown, start);
    check(
        !picker.is_active() && board.change_count() == count,
        "the right button ends it and copies nothing",
    );

    // Without the permission the loupe says so, and a click copies nothing.
    allowed.set(false);
    let reads = grabs.borrow().len();
    picker.start(0, start);
    check(
        grabs.borrow().len() == reads,
        "no pixels are read without the permission",
    );
    fs::write(folder.join("loupe-denied.png"), picker.loupe_png().unwrap()).unwrap();
    let overlay = picker.catcher_window(1).unwrap();
    mouse(
        &overlay,
        NSEventType::MouseMoved,
        NSPoint::new(-28700.0, 250.0),
    );
    mouse(&overlay, NSEventType::LeftMouseDown, start);
    check(
        !picker.is_active() && board.change_count() == count,
        "a click without the permission ends it and copies nothing",
    );
    allowed.set(true);

    // At an edge of the display the loupe moves to the other side.
    let corner = place(
        NSPoint::new(799.0, 1.0),
        NSSize::new(232.0, 226.0),
        NSRect::new(NSPoint::ZERO, NSSize::new(800.0, 600.0)),
    );
    check(
        corner.x < 799.0 && corner.x >= 0.0 && corner.y >= 0.0,
        "the loupe stays on the display in a corner",
    );

    // The registration the feature makes for its shortcut: a free combination is taken, and taken twice
    // inside one process is refused. F19 with three modifiers is a combination nothing else uses.
    {
        use macos_sys::hotkey::{Hotkey, modifier};
        let keys = modifier::CONTROL | modifier::OPTION | modifier::COMMAND;
        let first = Hotkey::register(mtm, 80, keys, || {});
        check(first.is_ok(), "a free shortcut can be registered");
        check(
            Hotkey::register(mtm, 80, keys, || {}).is_err(),
            "the same shortcut cannot be registered twice",
        );
        drop(first);
        check(
            Hotkey::register(mtm, 80, keys, || {}).is_ok(),
            "a released shortcut can be registered again",
        );
    }

    board.release();
    if failures > 0 {
        println!("{failures} checks failed");
        process::exit(1);
    }
    println!("all checks passed");
}
