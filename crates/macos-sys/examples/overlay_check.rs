//! Drives the selection overlay with synthetic input and checks what it reports: a drag picks an area,
//! Escape cancels, Return takes the display. The overlay covers every screen while it is up, so nothing
//! reaches another app. Writes a picture of the overlay mid-drag. Needs Accessibility and Screen
//! Recording access for the terminal.
//!
//!     cargo run -p macos-sys --example overlay_check -- /tmp/figghost-overlay.png

use macos_sys::{
    capture_overlay::{self, Mode, Picked},
    screen_capture::{self, Frozen},
};
use objc2::{MainThreadMarker, rc::Retained};
use objc2_app_kit::{
    NSApplication, NSApplicationActivationPolicy, NSBitmapImageFileType, NSEventMask, NSScreen,
};
use objc2_core_foundation::CGPoint;
use objc2_core_graphics::{CGEvent, CGEventTapLocation, CGEventType, CGMouseButton};
use objc2_foundation::{NSDate, NSDefaultRunLoopMode, NSDictionary};
use std::{cell::RefCell, env, fs, rc::Rc, time::Instant};

/// Runs the main loop and hands every event to its window, as `NSApplication::run` would.
fn spin(seconds: f64) {
    let mtm = MainThreadMarker::new().unwrap();
    let app = NSApplication::sharedApplication(mtm);
    let until = NSDate::dateWithTimeIntervalSinceNow(seconds);
    loop {
        // SAFETY: the default run loop mode is a process-lifetime constant.
        let event = unsafe {
            app.nextEventMatchingMask_untilDate_inMode_dequeue(
                NSEventMask::Any,
                Some(&until),
                NSDefaultRunLoopMode,
                true,
            )
        };
        match event {
            Some(event) => app.sendEvent(&event),
            None => break,
        }
    }
}

fn spin_until(done: impl Fn() -> bool) {
    let start = Instant::now();
    while !done() && start.elapsed().as_secs() < 10 {
        spin(0.02);
    }
}

/// Global top-left coordinates, as Core Graphics events use.
fn mouse(kind: CGEventType, x: f64, y: f64) {
    let event = CGEvent::new_mouse_event(None, kind, CGPoint::new(x, y), CGMouseButton::Left);
    CGEvent::post(CGEventTapLocation::HIDEventTap, event.as_deref());
    spin(0.03);
}

fn key(code: u16) {
    for down in [true, false] {
        let event = CGEvent::new_keyboard_event(None, code, down);
        CGEvent::post(CGEventTapLocation::HIDEventTap, event.as_deref());
    }
    spin(0.05);
}

fn freeze() -> Frozen {
    let frozen: Rc<RefCell<Option<Result<Frozen, String>>>> = Rc::default();
    let slot = Rc::clone(&frozen);
    screen_capture::freeze(move |r| *slot.borrow_mut() = Some(r));
    spin_until(|| frozen.borrow().is_some());
    frozen.borrow_mut().take().unwrap().expect("freeze")
}

fn run(mtm: MainThreadMarker, mode: Mode, act: impl FnOnce(), snapshot: Option<&str>) -> Picked {
    let frozen = freeze();
    let picked: Rc<RefCell<Option<Picked>>> = Rc::default();
    let slot = Rc::clone(&picked);
    let overlay = capture_overlay::pick(mtm, &frozen, mode, "Drag to select", move |p| {
        *slot.borrow_mut() = Some(p);
    });
    spin(0.3);
    act();
    if let Some(path) = snapshot {
        let app = NSApplication::sharedApplication(mtm);
        if let Some(view) = app.keyWindow().and_then(|w| w.contentView()) {
            let bounds = view.bounds();
            let rep = view.bitmapImageRepForCachingDisplayInRect(bounds).unwrap();
            view.cacheDisplayInRect_toBitmapImageRep(bounds, &rep);
            // SAFETY: an empty properties dictionary is valid for PNG output.
            let png: Retained<_> = unsafe {
                rep.representationUsingType_properties(
                    NSBitmapImageFileType::PNG,
                    &NSDictionary::new(),
                )
            }
            .unwrap();
            fs::write(path, png.to_vec()).unwrap();
            println!("wrote {path}");
        }
        mouse(CGEventType::LeftMouseUp, 420.0, 330.0);
    }
    spin_until(|| picked.borrow().is_some());
    drop(overlay);
    picked.borrow_mut().take().unwrap_or(Picked::Cancelled)
}

fn main() {
    let mtm = MainThreadMarker::new().expect("main thread");
    let app = NSApplication::sharedApplication(mtm);
    app.setActivationPolicy(NSApplicationActivationPolicy::Accessory);
    app.finishLaunching();
    let snapshot = env::args()
        .nth(1)
        .unwrap_or_else(|| "/tmp/figghost-overlay.png".into());
    let screen_height = NSScreen::screens(mtm)
        .firstObject()
        .unwrap()
        .frame()
        .size
        .height;
    let mut ok = true;

    // Drag from (120, 130) to (420, 330), measured from the top-left of the main screen.
    let picked = run(
        mtm,
        Mode::Any,
        || {
            mouse(CGEventType::MouseMoved, 120.0, 130.0);
            mouse(CGEventType::LeftMouseDown, 120.0, 130.0);
            for step in 1..=10 {
                let t = f64::from(step) / 10.0;
                mouse(
                    CGEventType::LeftMouseDragged,
                    120.0 + 300.0 * t,
                    130.0 + 200.0 * t,
                );
            }
        },
        Some(&snapshot),
    );
    println!("drag: {picked:?}");
    match picked {
        Picked::Area { display: 0, rect } => {
            let expected = (120.0, screen_height - 330.0, 300.0, 200.0);
            let got = (
                rect.origin.x,
                rect.origin.y,
                rect.size.width,
                rect.size.height,
            );
            if got != expected {
                println!("FAIL: expected {expected:?}");
                ok = false;
            }
        }
        other => {
            println!("FAIL: expected an area, got {other:?}");
            ok = false;
        }
    }

    let picked = run(mtm, Mode::Any, || key(53), None);
    println!("escape: {picked:?}");
    ok &= picked == Picked::Cancelled;

    let picked = run(
        mtm,
        Mode::Any,
        || {
            mouse(CGEventType::MouseMoved, 200.0, 200.0);
            key(36);
        },
        None,
    );
    println!("return: {picked:?}");
    ok &= picked == Picked::Display(0);

    let picked = run(
        mtm,
        Mode::AreaOnly,
        || {
            mouse(CGEventType::MouseMoved, 200.0, 200.0);
            key(36);
            key(53);
        },
        None,
    );
    println!("return in area-only mode is ignored, escape cancels: {picked:?}");
    ok &= picked == Picked::Cancelled;
    println!("{}", if ok { "OK" } else { "FAIL" });
}
