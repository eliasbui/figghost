//! Checks the drag wheel end to end: a `DragWheel` in this process, a file dragged out of a second process
//! (`drag_source`, standing in for Finder), a tap of Shift, Command or Control in the middle of the drag, and a drop
//! on a known slot. It checks that the offer got the kind and the file, that the wheel switched kind when a second
//! key was tapped, that `chosen` got the kind, the slot's id and the file, that a drop in the hub and a drag without
//! a key do nothing, and that a drag that starts in this process itself never opens a wheel.
//!
//! Warning: it posts real input for about half a minute, so leave the machine alone. It moves the pointer, presses
//! and releases the left button and taps the modifier keys Shift, Command and Control alone (no letter or other key
//! is ever sent). The terminal needs Accessibility access to post events and the screen has to be unlocked. The
//! pointer is put back afterwards. Nothing is moved or deleted: the dragged file is a temporary one and the drop
//! only copies.
//!
//! With a folder as the argument, it also puts a colourful window of its own behind the wheel and takes real
//! pictures of the screen with `screencapture` while the first run holds the wheel open (`glass-rest.png` with
//! the pointer in the hub, `glass-lit.png` with a slot lit). That shows the Liquid Glass, which no snapshot can.
//! It needs an unlocked screen and Screen Recording access for the terminal; without it the pictures are
//! blank or missing and the checks still run.
//!
//! Build the source first: `cargo build -p macos-sys --example drag_source`, then
//! `cargo run -p macos-sys --example wheel_check`.

use macos_sys::{
    MainThreadMarker, app,
    panel::FigPanel,
    shelf_ui::{ShelfCallbacks, ShelfView},
    wheel_ui::{self, DragWheel, Keys, Kind, Slot, Wheel},
};
use objc2::{MainThreadOnly, define_class, msg_send, rc::Retained};
use objc2_app_kit::{
    NSBackingStoreType, NSBox, NSBoxType, NSColor, NSGradient, NSPasteboard, NSPasteboardNameDrag,
    NSScreen, NSView, NSWindow, NSWindowStyleMask,
};
use objc2_core_foundation::CGPoint;
use objc2_core_graphics::{CGEvent, CGEventFlags, CGEventTapLocation, CGEventType, CGMouseButton};
use objc2_foundation::{NSPoint, NSRect, NSSize};
use std::{
    cell::{Cell, RefCell},
    env, fs,
    io::{BufRead, BufReader},
    path::PathBuf,
    process::{self, Command, Stdio},
    rc::Rc,
};

const SLOTS: usize = 6;
/// What the check's wheel shows: tools with symbols, so that a picture of it looks like the real thing.
const TITLES: [(&str, &str); SLOTS] = [
    ("Compress", "arrow.down.right.and.arrow.up.left"),
    ("Half size", "arrow.up.left.and.arrow.down.right"),
    ("Quarter size", "arrow.up.left.and.arrow.down.right"),
    ("No metadata", "eye.slash"),
    ("Cut out", "person.crop.rectangle"),
    ("Copy text", "text.viewfinder"),
];

define_class!(
    // SAFETY: NSView allows subclassing; it only draws.
    #[unsafe(super(NSView))]
    #[thread_kind = MainThreadOnly]
    #[name = "FigWheelBackdrop"]
    #[ivars = ()]
    struct Backdrop;

    impl Backdrop {
        #[unsafe(method(drawRect:))]
        fn draw_rect(&self, _dirty: NSRect) {
            let bounds = self.bounds();
            let colour = |r, g, b| NSColor::colorWithSRGBRed_green_blue_alpha(r, g, b, 1.0);
            if let Some(gradient) = NSGradient::initWithColors(
                objc2::AnyThread::alloc(),
                &objc2_foundation::NSArray::from_retained_slice(&[
                    colour(0.95, 0.25, 0.30),
                    colour(1.00, 0.70, 0.15),
                    colour(0.25, 0.80, 0.45),
                    colour(0.15, 0.50, 0.95),
                    colour(0.60, 0.30, 0.90),
                ]),
            ) {
                gradient.drawInRect_angle(bounds, 35.0);
            }
            // Stripes and a block of text-like lines, so that blur, edges and contrast under the glass show.
            let stripe = |x: f64, w: f64, white: bool| {
                NSColor::colorWithWhite_alpha(if white { 1.0 } else { 0.0 }, 0.55).set();
                objc2_app_kit::NSRectFill(NSRect::new(
                    NSPoint::new(x, bounds.origin.y),
                    NSSize::new(w, bounds.size.height),
                ));
            };
            for i in 0..14 {
                stripe(f64::from(i) * 48.0 + 10.0, 14.0, i % 2 == 0);
            }
            NSColor::colorWithWhite_alpha(1.0, 0.9).set();
            for row in 0..10 {
                objc2_app_kit::NSRectFill(NSRect::new(
                    NSPoint::new(40.0, 40.0 + f64::from(row) * 60.0),
                    NSSize::new(200.0 + f64::from(row % 4) * 90.0, 8.0),
                ));
            }
        }
    }
);

/// A window of its own, colourful and behind everything the wheel does, that ignores the pointer.
fn backdrop(mtm: MainThreadMarker, centre: (f64, f64), screen_height: f64) -> Retained<NSWindow> {
    let size = NSSize::new(760.0, 700.0);
    let origin = NSPoint::new(
        centre.0 - size.width / 2.0,
        screen_height - centre.1 - size.height / 2.0,
    );
    // SAFETY: designated initializer with valid arguments on the main thread.
    let window = unsafe {
        NSWindow::initWithContentRect_styleMask_backing_defer(
            mtm.alloc(),
            NSRect::new(origin, size),
            NSWindowStyleMask::Borderless,
            NSBackingStoreType::Buffered,
            false,
        )
    };
    // SAFETY: false is the only value compatible with a window kept alive by `Retained`.
    unsafe { window.setReleasedWhenClosed(false) };
    let view: Retained<Backdrop> = unsafe {
        msg_send![
            super(mtm.alloc::<Backdrop>().set_ivars(())),
            initWithFrame: NSRect::new(NSPoint::ZERO, size)
        ]
    };
    window.setContentView(Some(&view));
    window.setIgnoresMouseEvents(true);
    window.orderFrontRegardless();
    window
}

/// A real picture of a square of the screen round `centre` (top-left coordinates, points).
fn capture(folder: &str, name: &str, centre: (f64, f64), half: f64) {
    let region = format!(
        "{},{},{},{}",
        centre.0 - half,
        centre.1 - half,
        2.0 * half,
        2.0 * half
    );
    let out = format!("{folder}/{name}");
    match Command::new("/usr/sbin/screencapture")
        .args(["-x", "-R", &region, &out])
        .status()
    {
        Ok(status) if status.success() => println!("  wrote {out}"),
        other => println!("  screencapture failed: {other:?}"),
    }
}
const SHIFT: u16 = 56;
const COMMAND: u16 = 55;
const CONTROL: u16 = 59;

/// What the wheel reported, since the run began.
#[derive(Default)]
struct Seen {
    offers: Vec<(Kind, Vec<PathBuf>)>,
    chosen: Vec<(Kind, String, Vec<PathBuf>)>,
}

#[derive(Clone, Copy, PartialEq)]
enum Source {
    /// The tile of the second process.
    Other,
    /// A tile of a shelf in this process.
    Own,
}

struct Run {
    name: &'static str,
    source: Source,
    /// The keys tapped, in order, once the pointer is at the middle of the screen.
    taps: &'static [u16],
    /// The slot the files are released on; `None` releases them in the hub.
    slot: Option<usize>,
    /// The kinds `offer` should have been asked for, in order.
    offers: &'static [Kind],
    /// What `chosen` should have got, if anything.
    chosen: Option<(Kind, &'static str)>,
}

const RUNS: &[Run] = &[
    Run {
        name: "Shift then Control, drop on slot 1",
        source: Source::Other,
        taps: &[SHIFT, CONTROL],
        slot: Some(1),
        offers: &[Kind::Formats, Kind::Actions],
        chosen: Some((Kind::Actions, "s1")),
    },
    Run {
        name: "Command, drop on slot 4",
        source: Source::Other,
        taps: &[COMMAND],
        slot: Some(4),
        offers: &[Kind::Tools],
        chosen: Some((Kind::Tools, "s4")),
    },
    Run {
        name: "Shift, drop in the hub",
        source: Source::Other,
        taps: &[SHIFT],
        slot: None,
        offers: &[Kind::Formats],
        chosen: None,
    },
    Run {
        name: "no key tapped",
        source: Source::Other,
        taps: &[],
        slot: Some(0),
        offers: &[],
        chosen: None,
    },
    Run {
        name: "Shift during a drag that started here",
        source: Source::Own,
        taps: &[SHIFT],
        slot: Some(0),
        offers: &[],
        chosen: None,
    },
];

fn post_mouse(kind: CGEventType, at: (f64, f64)) {
    let event = CGEvent::new_mouse_event(None, kind, CGPoint::new(at.0, at.1), CGMouseButton::Left);
    CGEvent::post(CGEventTapLocation::HIDEventTap, event.as_deref());
}

fn post_key(code: u16, down: bool, flags: CGEventFlags) {
    let event = CGEvent::new_keyboard_event(None, code, down);
    CGEvent::set_flags(event.as_deref(), flags);
    CGEvent::post(CGEventTapLocation::HIDEventTap, event.as_deref());
}

fn flag_of(code: u16) -> CGEventFlags {
    match code {
        SHIFT => CGEventFlags::MaskShift,
        COMMAND => CGEventFlags::MaskCommand,
        _ => CGEventFlags::MaskControl,
    }
}

fn pointer() -> (f64, f64) {
    let event = CGEvent::new(None);
    let at = CGEvent::location(event.as_deref());
    (at.x, at.y)
}

fn test_wheel() -> Wheel {
    Wheel {
        slots: (0..SLOTS)
            .map(|i| Slot {
                id: format!("s{i}"),
                title: TITLES[i].0.to_owned(),
                symbol: Some(TITLES[i].1.to_owned()),
            })
            .collect(),
        caption: "wheel check".into(),
    }
}

/// Writes one line per check and remembers whether any failed.
fn check(failed: &Cell<bool>, ok: bool, what: &str) {
    println!("  {}: {what}", if ok { "pass" } else { "FAIL" });
    if !ok {
        failed.set(true);
    }
}

fn same_file(a: &PathBuf, b: &PathBuf) -> bool {
    match (fs::canonicalize(a), fs::canonicalize(b)) {
        (Ok(a), Ok(b)) => a == b,
        _ => a == b,
    }
}

/// A shelf with one file, on a panel with an opaque body (the window server hit-tests only opaque pixels), for a
/// drag that starts in this process.
fn own_source(mtm: MainThreadMarker, file: &str) -> (objc2::rc::Retained<FigPanel>, Rc<ShelfView>) {
    let size = NSSize::new(420.0, 260.0);
    let view = ShelfView::new(
        mtm,
        NSRect::new(NSPoint::ZERO, size),
        ShelfCallbacks {
            on_drop: Box::new(|_| {}),
            on_remove: Box::new(|_| {}),
            on_open: Box::new(|_| {}),
            actions_for: Box::new(|_| Vec::new()),
            on_action: Box::new(|_, _| {}),
        },
    );
    view.set_paths(vec![file.to_owned()]);
    let panel = FigPanel::new(mtm, NSRect::new(NSPoint::new(120.0, 80.0), size));
    let body = NSBox::new(mtm);
    body.setBoxType(NSBoxType::Custom);
    body.setBorderWidth(0.0);
    body.setFillColor(&NSColor::blackColor());
    body.setContentViewMargins(NSSize::ZERO);
    body.setFrame(NSRect::new(NSPoint::ZERO, size));
    body.setContentView(Some(&view.root));
    panel.setContentView(Some(&body));
    panel.orderFrontRegardless();
    (panel, view)
}

fn main() {
    let mtm = MainThreadMarker::new().expect("main thread");
    let source = env::current_exe()
        .expect("own path")
        .with_file_name("drag_source");
    if !source.exists() {
        eprintln!("build the source first: cargo build -p macos-sys --example drag_source");
        process::exit(2);
    }
    app::run_accessory(mtm, move || {
        let file = env::temp_dir().join(format!("figghost-wheel-{}.txt", process::id()));
        fs::write(&file, "drag me").expect("temp file");
        let file_text = file.to_string_lossy().into_owned();
        // The file is removed when the check ends, whether the checks passed or not. A panic inside AppKit's
        // callback aborts without unwinding, so a hook removes it there instead of a guard.
        {
            let (file, previous) = (file.clone(), std::panic::take_hook());
            std::panic::set_hook(Box::new(move |info| {
                let _ = fs::remove_file(&file);
                previous(info);
            }));
        }

        let seen: Rc<RefCell<Seen>> = Rc::default();
        let wheel = {
            let (offered, chosen) = (Rc::clone(&seen), Rc::clone(&seen));
            Rc::new(
                DragWheel::start(
                    mtm,
                    Keys::default(),
                    move |kind, paths| {
                        offered.borrow_mut().offers.push((kind, paths.to_vec()));
                        Some(test_wheel())
                    },
                    move |kind, id, paths| {
                        chosen
                            .borrow_mut()
                            .chosen
                            .push((kind, id.to_owned(), paths));
                    },
                )
                .expect("the system gave a mouse monitor"),
            )
        };

        let mut child = Command::new(&source)
            .arg(&file)
            .arg("60")
            .stdout(Stdio::piped())
            .spawn()
            .expect("start drag_source");
        let mut line = String::new();
        BufReader::new(child.stdout.take().expect("pipe"))
            .read_line(&mut line)
            .expect("the source reports its tile");
        let tile: Vec<f64> = line
            .split_whitespace()
            .skip(1)
            .filter_map(|s| s.parse().ok())
            .collect();
        let other_tile = (tile[0], tile[1]);

        let (own_panel, own_view) = own_source(mtm, &file_text);
        let primary = NSScreen::screens(mtm)
            .firstObject()
            .expect("a screen")
            .frame()
            .size;
        let own_tile = {
            let at = own_view.tile_center_on_screen(0).expect("own tile");
            (at.x, primary.height - at.y)
        };
        // The wheel opens where the pointer is. The middle of the screen leaves room for it on every side, and
        // keeps it clear of the two source windows on the left.
        let middle = ((primary.width / 2.0).max(800.0), primary.height / 2.0);
        let original = pointer();
        let shots: Option<String> = env::args().nth(1);
        let _backdrop = shots.as_ref().map(|folder| {
            fs::create_dir_all(folder).expect("make the folder");
            backdrop(mtm, middle, primary.height)
        });
        // SAFETY: extern statics are process-lifetime constants.
        let drag_board = NSPasteboard::pasteboardWithName(unsafe { NSPasteboardNameDrag });
        let count_at_start = Rc::new(Cell::new(0));
        let failed = Rc::new(Cell::new(false));

        let mut start = 0.6;
        for (index, run) in RUNS.iter().enumerate() {
            let tile = if run.source == Source::Own {
                own_tile
            } else {
                other_tile
            };
            let target = match run.slot {
                Some(slot) => {
                    let (dx, dy) = wheel_ui::slot_offset(&test_wheel(), slot);
                    (middle.0 + dx, middle.1 - dy)
                }
                None => middle,
            };
            let at = |t: f64, f: Box<dyn Fn()>| app::after(start + t, f);
            {
                let (seen, board, count) = (
                    Rc::clone(&seen),
                    drag_board.clone(),
                    Rc::clone(&count_at_start),
                );
                at(
                    0.0,
                    Box::new(move || {
                        count.set(board.changeCount());
                        *seen.borrow_mut() = Seen::default();
                        println!("{}", run.name);
                        post_mouse(CGEventType::MouseMoved, tile);
                    }),
                );
            }
            at(
                0.3,
                Box::new(move || post_mouse(CGEventType::LeftMouseDown, tile)),
            );
            for step in 1..=12 {
                let f = f64::from(step) / 12.0;
                let to = (
                    tile.0 + (middle.0 - tile.0) * f,
                    tile.1 + (middle.1 - tile.1) * f,
                );
                at(
                    0.4 + f64::from(step) * 0.05,
                    Box::new(move || post_mouse(CGEventType::LeftMouseDragged, to)),
                );
            }
            // Each tap: the key goes down with its flag and up with none, about a tenth of a second apart.
            for (i, &code) in run.taps.iter().enumerate() {
                let t = 1.2 + 0.4 * i as f64;
                at(t, Box::new(move || post_key(code, true, flag_of(code))));
                at(
                    t + 0.1,
                    Box::new(move || post_key(code, false, CGEventFlags::empty())),
                );
            }
            let settled = 1.2 + 0.4 * run.taps.len() as f64 + 0.3;
            {
                let (seen, wheel, failed) =
                    (Rc::clone(&seen), Rc::clone(&wheel), Rc::clone(&failed));
                let (file, board, count) =
                    (file.clone(), drag_board.clone(), Rc::clone(&count_at_start));
                at(
                    settled,
                    Box::new(move || {
                        // Without this, a drag that never began would pass the checks that expect nothing.
                        check(
                            &failed,
                            board.changeCount() != count.get(),
                            "a drag session began",
                        );
                        let seen = seen.borrow();
                        let kinds: Vec<Kind> = seen.offers.iter().map(|(k, _)| *k).collect();
                        check(
                            &failed,
                            kinds == run.offers,
                            &format!("offer asked for {kinds:?}, wanted {:?}", run.offers),
                        );
                        check(
                            &failed,
                            seen.offers
                                .iter()
                                .all(|(_, p)| p.len() == 1 && same_file(&p[0], &file)),
                            "every offer carried the dragged file",
                        );
                        check(
                            &failed,
                            wheel.is_open() == !run.offers.is_empty(),
                            &format!("wheel open: {}", wheel.is_open()),
                        );
                    }),
                );
            }
            // The pointer travels from the middle to the slot, then rests there a moment so the panel's drag
            // callbacks see where it is.
            for step in 1..=12 {
                let f = f64::from(step) / 12.0;
                let to = (
                    middle.0 + (target.0 - middle.0) * f,
                    middle.1 + (target.1 - middle.1) * f,
                );
                at(
                    settled + 0.1 + f64::from(step) * 0.04,
                    Box::new(move || post_mouse(CGEventType::LeftMouseDragged, to)),
                );
            }
            for i in 1..=4 {
                let nudge = (target.0 + f64::from(i % 2), target.1);
                at(
                    settled + 0.7 + f64::from(i) * 0.1,
                    Box::new(move || post_mouse(CGEventType::LeftMouseDragged, nudge)),
                );
            }
            at(
                settled + 1.2,
                Box::new(move || post_mouse(CGEventType::LeftMouseUp, (target.0 + 1.0, target.1))),
            );
            {
                let (seen, wheel, failed) =
                    (Rc::clone(&seen), Rc::clone(&wheel), Rc::clone(&failed));
                let file = file.clone();
                at(
                    settled + 2.2,
                    Box::new(move || {
                        let seen = seen.borrow();
                        match run.chosen {
                            Some((kind, id)) => {
                                let got = seen.chosen.first();
                                check(
                                    &failed,
                                    seen.chosen.len() == 1,
                                    &format!("chosen ran {} time(s)", seen.chosen.len()),
                                );
                                check(
                                    &failed,
                                    got.is_some_and(|(k, i, p)| {
                                        *k == kind
                                            && i == id
                                            && p.len() == 1
                                            && same_file(&p[0], &file)
                                    }),
                                    &format!(
                                        "chosen got {:?}, wanted {kind:?} {id} and the file",
                                        got.map(|(k, i, _)| (k, i))
                                    ),
                                );
                            }
                            None => check(
                                &failed,
                                seen.chosen.is_empty(),
                                &format!("chosen did not run ({} calls)", seen.chosen.len()),
                            ),
                        }
                        check(&failed, !wheel.is_open(), "wheel closed after the release");
                    }),
                );
            }
            if let (Some(folder), 0) = (shots.clone(), index) {
                at(
                    settled + 0.25,
                    Box::new({
                        let folder = folder.clone();
                        move || capture(&folder, "glass-rest.png", middle, 260.0)
                    }),
                );
                at(
                    settled + 1.1,
                    Box::new(move || capture(&folder, "glass-lit.png", middle, 260.0)),
                );
            }
            start += settled + 2.5;
        }
        {
            let failed = Rc::clone(&failed);
            let file = file.clone();
            app::after(start, move || {
                post_mouse(CGEventType::MouseMoved, original);
                let _ = child.kill();
                let _ = child.wait();
                let _ = fs::remove_file(&file);
                let ok = !failed.get();
                println!(
                    "{}",
                    if ok {
                        "wheel check: pass"
                    } else {
                        "wheel check: FAIL"
                    }
                );
                process::exit(i32::from(!ok));
            });
        }
        (wheel, own_panel, own_view, _backdrop)
    });
}
