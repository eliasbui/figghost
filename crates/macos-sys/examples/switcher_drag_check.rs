//! Drives the strip of page icons in a window outside every screen with synthetic presses, drags and releases
//! delivered through the view's own methods, samples the lens frame by frame and checks that a click glides, that a
//! hold or a drag lifts the pill into a lens that trails the pointer and stays on the strip, that letting go chooses
//! the page under it once, and that a press whose release never comes puts it back. Writes a few frames to PNG.
//! Exits with status 1 on a failure. Reduce Motion and Escape are not exercised.
//! `cargo run -p macos-sys --example switcher_drag_check -- /tmp/switcher`

use macos_sys::{MainThreadMarker, switcher::PageSwitcher};
use objc2::{msg_send, rc::Retained};
use objc2_app_kit::{
    NSAppearance, NSAppearanceCustomization, NSAppearanceNameDarkAqua, NSApplication,
    NSApplicationActivationPolicy, NSBackingStoreType, NSBitmapImageFileType, NSBox, NSBoxType,
    NSColor, NSEvent, NSEventModifierFlags, NSEventType, NSWindow, NSWindowStyleMask,
};
use objc2_foundation::{NSDate, NSDictionary, NSPoint, NSRect, NSRunLoop, NSSize};
use std::{cell::RefCell, env, fs, path::PathBuf, process, rc::Rc, time::Instant};

const ORIGIN: NSPoint = NSPoint::new(20.0, 16.0);
const CELL: f64 = 30.0;
const PAD: f64 = 2.0;

/// The middle of a cell, in the strip's points.
fn centre(cell: f64) -> f64 {
    PAD + (cell + 0.5) * CELL
}

struct Rig {
    window: Retained<NSWindow>,
    strip: Retained<PageSwitcher>,
    chosen: Rc<RefCell<Vec<usize>>>,
    failures: RefCell<usize>,
    out: PathBuf,
}

impl Rig {
    fn send(&self, kind: NSEventType, x: f64) {
        let location = NSPoint::new(ORIGIN.x + x, ORIGIN.y + 14.0);
        let event = NSEvent::mouseEventWithType_location_modifierFlags_timestamp_windowNumber_context_eventNumber_clickCount_pressure(
            kind,
            location,
            NSEventModifierFlags::empty(),
            0.0,
            self.window.windowNumber(),
            None,
            0,
            1,
            1.0,
        )
        .expect("event");
        // SAFETY: the strip answers these mouse selectors, each taking an event.
        unsafe {
            match kind {
                NSEventType::LeftMouseDown => {
                    let _: () = msg_send![&*self.strip, mouseDown: &*event];
                }
                NSEventType::LeftMouseDragged => {
                    let _: () = msg_send![&*self.strip, mouseDragged: &*event];
                }
                _ => {
                    let _: () = msg_send![&*self.strip, mouseUp: &*event];
                }
            }
        }
    }

    /// Runs the run loop for `seconds`, returning the lens frame after each slice of it.
    fn pump(&self, seconds: f64) -> Vec<(f64, NSRect)> {
        let start = Instant::now();
        let mut samples = Vec::new();
        while start.elapsed().as_secs_f64() < seconds {
            NSRunLoop::mainRunLoop()
                .runUntilDate(&NSDate::dateWithTimeIntervalSinceNow(1.0 / 120.0));
            samples.push((start.elapsed().as_secs_f64(), self.strip.lens_frame()));
        }
        samples
    }

    /// Drags from `from` to `to` in the strip's points over `seconds`, a move event each frame.
    fn drag(&self, from: f64, to: f64, seconds: f64) -> Vec<(f64, f64, NSRect)> {
        let start = Instant::now();
        let mut samples = Vec::new();
        loop {
            let t = (start.elapsed().as_secs_f64() / seconds).min(1.0);
            let x = from + (to - from) * t;
            self.send(NSEventType::LeftMouseDragged, x);
            NSRunLoop::mainRunLoop()
                .runUntilDate(&NSDate::dateWithTimeIntervalSinceNow(1.0 / 120.0));
            samples.push((start.elapsed().as_secs_f64(), x, self.strip.lens_frame()));
            if t >= 1.0 {
                return samples;
            }
        }
    }

    fn check(&self, ok: bool, what: String) {
        println!("{} {what}", if ok { "ok  " } else { "FAIL" });
        if !ok {
            *self.failures.borrow_mut() += 1;
        }
    }

    fn take_chosen(&self) -> Vec<usize> {
        std::mem::take(&mut *self.chosen.borrow_mut())
    }

    fn shoot(&self, name: &str) {
        let view = self.window.contentView().expect("content");
        let bounds = view.bounds();
        let rep = view
            .bitmapImageRepForCachingDisplayInRect(bounds)
            .expect("rep");
        view.cacheDisplayInRect_toBitmapImageRep(bounds, &rep);
        // SAFETY: an empty properties dictionary is valid for PNG output.
        let png = unsafe {
            rep.representationUsingType_properties(NSBitmapImageFileType::PNG, &NSDictionary::new())
        }
        .expect("png");
        let path = self.out.join(format!("{name}.png"));
        fs::write(&path, png.to_vec()).expect("write png");
        println!("wrote {}", path.display());
    }
}

fn mid(r: NSRect) -> f64 {
    r.origin.x + r.size.width / 2.0
}

/// When the lens centre first came within half a point of `x` and stayed there.
fn settled_at(samples: &[(f64, NSRect)], x: f64) -> Option<f64> {
    let last_off = samples
        .iter()
        .rposition(|(_, r)| (mid(*r) - x).abs() >= 0.5);
    match last_off {
        None => samples.first().map(|s| s.0),
        Some(i) => samples.get(i + 1).map(|s| s.0),
    }
}

fn main() {
    let mtm = MainThreadMarker::new().expect("main thread");
    let app = NSApplication::sharedApplication(mtm);
    app.setActivationPolicy(NSApplicationActivationPolicy::Accessory);
    let out = PathBuf::from(env::args().nth(1).unwrap_or_else(|| "/tmp/switcher".into()));
    fs::create_dir_all(&out).expect("folder");

    let size = NSSize::new(200.0, 60.0);
    // SAFETY: a borderless window with a valid frame on the main thread.
    let window = unsafe {
        NSWindow::initWithContentRect_styleMask_backing_defer(
            mtm.alloc(),
            NSRect::new(NSPoint::new(-20000.0, -20000.0), size),
            NSWindowStyleMask::Borderless,
            NSBackingStoreType::Buffered,
            false,
        )
    };
    // SAFETY: the window is kept by this function, not by closing.
    unsafe { window.setReleasedWhenClosed(false) };
    let backdrop = NSBox::new(mtm);
    backdrop.setBoxType(NSBoxType::Custom);
    backdrop.setBorderWidth(0.0);
    backdrop.setFillColor(&NSColor::blackColor());
    backdrop.setContentViewMargins(NSSize::ZERO);
    // SAFETY: extern statics are process-lifetime constants.
    let dark = unsafe { NSAppearance::appearanceNamed(NSAppearanceNameDarkAqua) };
    backdrop.setAppearance(dark.as_deref());
    window.setContentView(Some(&backdrop));
    window.orderFrontRegardless();

    let strip = PageSwitcher::new(mtm, ORIGIN);
    backdrop.addSubview(&strip);
    strip.set_items(
        [
            "doc.on.clipboard",
            "music.note",
            "calendar",
            "timer",
            "gearshape",
        ]
        .iter()
        .map(|s| (s.to_string(), s.to_string()))
        .collect(),
    );
    strip.set_watch_button(false);
    let chosen = Rc::new(RefCell::new(Vec::new()));
    {
        // As the notch does: the chosen page comes back to the strip as the selection.
        let (chosen, weak) = (Rc::clone(&chosen), objc2::rc::Weak::from_retained(&strip));
        strip.set_on_select(move |i| {
            chosen.borrow_mut().push(i);
            if let Some(strip) = weak.load() {
                strip.set_selected(i);
            }
        });
    }
    strip.set_selected(0);
    strip.settle();
    let rig = Rig {
        window,
        strip,
        chosen,
        failures: RefCell::new(0),
        out,
    };
    let rest_height = rig.strip.lens_frame().size.height;
    rig.shoot("0-rest");

    // A click on the fourth page chooses it on the press and glides there without lifting.
    rig.send(NSEventType::LeftMouseDown, centre(3.0));
    let mut samples = rig.pump(0.06);
    rig.send(NSEventType::LeftMouseUp, centre(3.0));
    samples.extend(rig.pump(0.6));
    let chosen = rig.take_chosen();
    rig.check(
        chosen == [3],
        format!("a click chooses once on the press: {chosen:?}"),
    );
    let tallest = samples.iter().map(|s| s.1.size.height).fold(0.0, f64::max);
    rig.check(
        tallest <= rest_height + 0.01,
        format!("a click does not lift (tallest {tallest})"),
    );
    let settle = settled_at(&samples, centre(3.0));
    rig.check(
        settle.is_some_and(|t| t <= 0.4),
        format!("a click glides three cells in {settle:?} s"),
    );

    // A drag from the fourth page to the first: the lens lifts, trails the pointer, and nothing is chosen yet.
    rig.send(NSEventType::LeftMouseDown, centre(3.0));
    let chosen = rig.take_chosen();
    rig.check(
        chosen == [3],
        format!("the press chooses its own page again: {chosen:?}"),
    );
    let drag = rig.drag(centre(3.0), centre(0.0), 0.25);
    let (_, x, lens) = *drag.last().expect("samples");
    rig.check(
        mid(lens) - x > 15.0,
        format!(
            "the lens trails the pointer: {:.1} behind when it arrives",
            mid(lens) - x
        ),
    );
    rig.check(
        lens.size.height > rest_height + 1.0,
        format!("the lens is lifted ({:.1} tall)", lens.size.height),
    );
    let widest = drag.iter().map(|s| s.2.size.width).fold(0.0, f64::max);
    rig.check(
        widest > CELL + 8.0,
        format!("it stretches while moving (widest {widest:.1})"),
    );
    rig.shoot("1-dragging");
    // Then out past the left end, held there.
    let mut held = rig
        .drag(centre(0.0), -60.0, 0.1)
        .into_iter()
        .map(|s| (s.0, s.2))
        .collect::<Vec<_>>();
    held.extend(rig.pump(1.0));
    let inside = held.iter().all(|(_, r)| {
        r.origin.x >= -0.01 && r.origin.x + r.size.width <= rig.strip.frame().size.width + 0.01
    });
    rig.check(inside, "the lens stays on the strip".into());
    let (_, lens) = *held.last().expect("samples");
    rig.check(
        lens.size.height > rest_height + 1.0,
        "it stays lifted while held".into(),
    );
    let chosen = rig.take_chosen();
    rig.check(
        chosen.is_empty(),
        format!("no page is chosen while dragging: {chosen:?}"),
    );
    rig.shoot("2-held-outside");
    rig.send(NSEventType::LeftMouseUp, -60.0);
    let after = rig.pump(1.2);
    let chosen = rig.take_chosen();
    rig.check(
        chosen == [0],
        format!("letting go outside chooses the nearest page once: {chosen:?}"),
    );
    let (_, last) = *after.last().expect("samples");
    rig.check(
        (last.size.height - rest_height).abs() < 0.01 && (mid(last) - centre(0.0)).abs() < 0.5,
        format!("the lens shrinks back into the cell: {last:?}"),
    );
    rig.shoot("3-settled");

    // A hold without moving lifts, then a drag past the right end chooses the last page.
    rig.send(NSEventType::LeftMouseDown, centre(1.0));
    rig.take_chosen();
    let hold = rig.pump(0.5);
    let (_, lens) = *hold.last().expect("samples");
    rig.check(
        lens.size.height > rest_height + 1.0,
        format!("a hold lifts ({:.1} tall)", lens.size.height),
    );
    rig.shoot("4-held");
    rig.drag(centre(1.0), 300.0, 0.3);
    rig.pump(0.8);
    rig.send(NSEventType::LeftMouseUp, 300.0);
    rig.pump(1.2);
    let chosen = rig.take_chosen();
    rig.check(
        chosen == [4],
        format!("letting go past the right end chooses the last page: {chosen:?}"),
    );

    // A release that never comes: once the real button is seen up, the lens goes back and nothing is chosen.
    rig.send(NSEventType::LeftMouseDown, centre(4.0));
    rig.take_chosen();
    rig.drag(centre(4.0), centre(1.0), 0.2);
    rig.strip.set_watch_button(true);
    let back = rig.pump(1.2);
    rig.send(NSEventType::LeftMouseUp, centre(1.0));
    rig.pump(0.1);
    let chosen = rig.take_chosen();
    rig.check(
        chosen.is_empty(),
        format!("a lost release chooses nothing: {chosen:?}"),
    );
    let (_, last) = *back.last().expect("samples");
    rig.check(
        (mid(last) - centre(4.0)).abs() < 0.5 && (last.size.height - rest_height).abs() < 0.01,
        format!("and the lens goes back: {last:?}"),
    );

    let failures = *rig.failures.borrow();
    println!("{failures} failure(s)");
    process::exit(i32::from(failures > 0));
}
