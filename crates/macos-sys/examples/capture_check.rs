//! Checks the capture overlay without a screen: synthetic mouse and key events are delivered to its panels
//! with `sendEvent` (nothing is posted to the system), the panels sit far outside every display, and the
//! keyboard is never taken. It asserts the answers (rectangles, windows, cancellation), that the panels are
//! gone afterwards, and draws the overlay over an invented picture into PNG files for a look.
//! `cargo run -p macos-sys --example capture_check -- [output-folder]`

use core_capture::{Display, Pick, Point, Rect, WindowBox};
use macos_sys::{
    MainThreadMarker, app,
    capture::{
        self, image,
        overlay::{Mode, Overlay, Strings},
        screens, toast, windows,
    },
};
use objc2::AnyThread;
use objc2_app_kit::{
    NSApplication, NSBezierPath, NSBitmapImageFileType, NSColor, NSCompositingOperation, NSEvent,
    NSEventModifierFlags, NSEventType, NSGradient, NSGraphicsContext, NSImage, NSView,
};
use objc2_foundation::{NSDate, NSDictionary, NSPoint, NSRect, NSRunLoop, NSSize, NSString};
use std::{cell::RefCell, path::PathBuf, process, rc::Rc};

const ORIGIN_X: f64 = -30_000.0;
const WIDTH: f64 = 800.0;
const HEIGHT: f64 = 600.0;

fn displays() -> Vec<Display> {
    vec![
        Display {
            id: 9001,
            frame: Rect::new(ORIGIN_X, 0.0, WIDTH, HEIGHT),
            scale: 2.0,
        },
        Display {
            id: 9002,
            frame: Rect::new(ORIGIN_X + WIDTH, 120.0, WIDTH, HEIGHT),
            scale: 1.0,
        },
    ]
}

fn windows() -> Vec<WindowBox> {
    let win = |id, x: f64, y: f64, w: f64, h: f64, owner: &str, title: &str| WindowBox {
        id,
        frame: Rect::new(ORIGIN_X + x, y, w, h),
        owner: owner.into(),
        title: title.into(),
    };
    vec![
        win(11, 380.0, 200.0, 360.0, 300.0, "Notes", "Shopping list"),
        win(12, 60.0, 90.0, 520.0, 400.0, "Editor", "main.rs"),
    ]
}

fn strings() -> Strings {
    Strings {
        hint_region: "Drag to select. Space moves it. Return confirms. Escape cancels.".into(),
        hint_window: "Click a window to capture it. Escape cancels.".into(),
    }
}

fn spin(seconds: f64) {
    NSRunLoop::currentRunLoop().runUntilDate(&NSDate::dateWithTimeIntervalSinceNow(seconds));
}

type Answer = Rc<RefCell<Option<Pick>>>;

fn start(mtm: MainThreadMarker, mode: Mode, pointer: Point) -> (Overlay, Answer) {
    let answer: Answer = Rc::default();
    let slot = Rc::clone(&answer);
    let overlay = Overlay::show(mtm, mode, displays(), pointer, strings(), move |pick| {
        *slot.borrow_mut() = Some(pick);
    });
    (overlay, answer)
}

fn mouse(overlay: &Overlay, display: usize, kind: NSEventType, x: f64, y: f64) {
    let panel = overlay.panel(display).expect("panel");
    let event = NSEvent::mouseEventWithType_location_modifierFlags_timestamp_windowNumber_context_eventNumber_clickCount_pressure(
        kind,
        NSPoint::new(x, y),
        NSEventModifierFlags::empty(),
        0.0,
        panel.windowNumber(),
        None,
        0,
        1,
        1.0,
    )
    .expect("a mouse event");
    panel.sendEvent(&event);
}

fn key(overlay: &Overlay, display: usize, kind: NSEventType, code: u16) {
    let panel = overlay.panel(display).expect("panel");
    let event = NSEvent::keyEventWithType_location_modifierFlags_timestamp_windowNumber_context_characters_charactersIgnoringModifiers_isARepeat_keyCode(
        kind,
        NSPoint::ZERO,
        NSEventModifierFlags::empty(),
        0.0,
        panel.windowNumber(),
        None,
        &NSString::from_str(""),
        &NSString::from_str(""),
        false,
        code,
    )
    .expect("a key event");
    panel.sendEvent(&event);
}

const ESCAPE: u16 = 53;
const RETURN: u16 = 36;
const SPACE: u16 = 49;

fn drag(overlay: &Overlay, display: usize, from: (f64, f64), to: (f64, f64)) {
    mouse(overlay, display, NSEventType::LeftMouseDown, from.0, from.1);
    mouse(overlay, display, NSEventType::LeftMouseDragged, to.0, to.1);
}

fn expect_region(
    answer: &Answer,
    display: usize,
    rect: Rect,
    what: &str,
    failures: &mut Vec<String>,
) {
    spin(0.05);
    match answer.borrow_mut().take() {
        Some(Pick::Region {
            display: d,
            rect: r,
        }) => {
            let want = displays()[display];
            if d != want || r != rect {
                failures.push(format!(
                    "{what}: got {r:?} on {}, wanted {rect:?} on {}",
                    d.id, want.id
                ));
            }
        }
        other => failures.push(format!("{what}: wanted a region, got {other:?}")),
    }
}

fn panels_open(mtm: MainThreadMarker) -> usize {
    NSApplication::sharedApplication(mtm)
        .windows()
        .iter()
        .filter(|w| w.isVisible() && w.class().name().to_string_lossy() == "FigPanel")
        .count()
}

fn draw_background(size: NSSize, windows: &[WindowBox], origin_x: f64, origin_y: f64) {
    let bounds = NSRect::new(NSPoint::ZERO, size);
    if let Some(gradient) = NSGradient::initWithStartingColor_endingColor(
        objc2::AnyThread::alloc(),
        &NSColor::colorWithSRGBRed_green_blue_alpha(0.74, 0.84, 0.98, 1.0),
        &NSColor::colorWithSRGBRed_green_blue_alpha(0.96, 0.82, 0.90, 1.0),
    ) {
        gradient.drawInRect_angle(bounds, 315.0);
    }
    // Windows from the back to the front, each with a title bar and a few lines of "text".
    for window in windows.iter().rev() {
        let frame = NSRect::new(
            NSPoint::new(window.frame.x - origin_x, window.frame.y - origin_y),
            NSSize::new(window.frame.w, window.frame.h),
        );
        NSColor::whiteColor().setFill();
        NSBezierPath::bezierPathWithRoundedRect_xRadius_yRadius(frame, 10.0, 10.0).fill();
        NSColor::colorWithSRGBRed_green_blue_alpha(0.86, 0.86, 0.88, 1.0).setFill();
        NSBezierPath::fillRect(NSRect::new(
            NSPoint::new(frame.origin.x, frame.origin.y + frame.size.height - 28.0),
            NSSize::new(frame.size.width, 28.0),
        ));
        NSColor::colorWithSRGBRed_green_blue_alpha(0.55, 0.62, 0.78, 1.0).setFill();
        for line in 0..8 {
            NSBezierPath::fillRect(NSRect::new(
                NSPoint::new(
                    frame.origin.x + 18.0,
                    frame.origin.y + frame.size.height - 62.0 - f64::from(line) * 26.0,
                ),
                NSSize::new(
                    (frame.size.width - 36.0) * (0.45 + 0.07 * f64::from(line % 5)),
                    9.0,
                ),
            ));
        }
    }
}

/// The overlay's view of display 0 over the invented picture, as PNG bytes.
fn render(view: &NSView, windows: &[WindowBox], origin: (f64, f64)) -> Option<Vec<u8>> {
    let bounds = view.bounds();
    let overlay = view.bitmapImageRepForCachingDisplayInRect(bounds)?;
    view.cacheDisplayInRect_toBitmapImageRep(bounds, &overlay);
    let out = image::from_pixels(
        bounds.size.width as usize,
        bounds.size.height as usize,
        |_, _| [0, 0, 0, 0],
    )?;
    let context = NSGraphicsContext::graphicsContextWithBitmapImageRep(&out)?;
    NSGraphicsContext::saveGraphicsState_class();
    NSGraphicsContext::setCurrentContext(Some(&context));
    draw_background(bounds.size, windows, origin.0, origin.1);
    let layer = NSImage::initWithSize(NSImage::alloc(), bounds.size);
    layer.addRepresentation(&overlay);
    layer.drawInRect_fromRect_operation_fraction(
        bounds,
        NSRect::ZERO,
        NSCompositingOperation::SourceOver,
        1.0,
    );
    NSGraphicsContext::restoreGraphicsState_class();
    // SAFETY: an empty properties dictionary is valid for PNG output.
    unsafe {
        out.representationUsingType_properties(NSBitmapImageFileType::PNG, &NSDictionary::new())
    }
    .map(|d| d.to_vec())
}

fn save(folder: &PathBuf, name: &str, png: Option<Vec<u8>>, failures: &mut Vec<String>) {
    match png {
        Some(bytes) => {
            let _ = std::fs::create_dir_all(folder);
            let path = folder.join(name);
            match std::fs::write(&path, bytes) {
                Ok(()) => println!("wrote {}", path.display()),
                Err(e) => failures.push(format!("cannot write {}: {e}", path.display())),
            }
        }
        None => failures.push(format!("cannot render {name}")),
    }
}

/// What the real system reports: the displays, the window list and the capture API. Only metadata is read,
/// never a picture of the screen, and the permission dialog is never triggered.
fn system_facts(mtm: MainThreadMarker, failures: &mut Vec<String>) {
    let real = screens::displays(mtm);
    println!("displays: {}", real.len());
    if real.is_empty()
        || real
            .iter()
            .any(|d| d.scale < 1.0 || d.frame.w <= 0.0 || d.frame.h <= 0.0)
    {
        failures.push("the displays are not plausible".into());
    }
    if !real.iter().any(|d| d.frame.x == 0.0 && d.frame.y == 0.0) {
        failures.push("no display at the origin".into());
    }
    let list = windows::list(&real);
    println!("windows on screen: {}", list.len());
    if list
        .iter()
        .any(|w| w.frame.w < 24.0 || w.frame.h < 24.0 || w.id == 0)
    {
        failures.push("the window list holds a tiny window or one without a number".into());
    }
    let at = screens::pointer();
    if core_capture::display_index_at(&real, at).is_none() {
        failures.push("the pointer is on no display".into());
    }
    println!(
        "capture api available: {}, permission granted: {}",
        capture::grab::available(),
        capture::permission_granted()
    );
}

fn run(mtm: MainThreadMarker, folder: PathBuf) -> Vec<String> {
    let mut failures = Vec::new();
    system_facts(mtm, &mut failures);
    let pointer = Point::new(ORIGIN_X + 200.0, 300.0);
    let none: &[WindowBox] = &[];

    // A drag from the top left to the bottom right, then release.
    let (overlay, answer) = start(mtm, Mode::Region, pointer);
    if panels_open(mtm) != 2 {
        failures.push(format!("two panels expected, found {}", panels_open(mtm)));
    }
    mouse(&overlay, 0, NSEventType::MouseMoved, 200.0, 300.0);
    save(
        &folder,
        "overlay-idle.png",
        render(&overlay.view(0).unwrap(), none, (ORIGIN_X, 0.0)),
        &mut failures,
    );
    drag(&overlay, 0, (100.0, 100.0), (300.0, 250.0));
    save(
        &folder,
        "overlay-selecting.png",
        render(&overlay.view(0).unwrap(), none, (ORIGIN_X, 0.0)),
        &mut failures,
    );
    save(
        &folder,
        "overlay-other-display.png",
        render(&overlay.view(1).unwrap(), none, (ORIGIN_X + WIDTH, 120.0)),
        &mut failures,
    );
    mouse(&overlay, 0, NSEventType::LeftMouseUp, 300.0, 250.0);
    expect_region(
        &answer,
        0,
        Rect::new(ORIGIN_X + 100.0, 100.0, 200.0, 150.0),
        "drag and release",
        &mut failures,
    );
    drop(overlay);
    if panels_open(mtm) != 0 {
        failures.push(format!(
            "panels left after the answer: {}",
            panels_open(mtm)
        ));
    }

    // The same drag backwards, ending beyond the display: clamped to its edges.
    let (overlay, answer) = start(mtm, Mode::Region, pointer);
    drag(&overlay, 0, (700.0, 500.0), (2000.0, 2000.0));
    mouse(&overlay, 0, NSEventType::LeftMouseUp, 2000.0, 2000.0);
    expect_region(
        &answer,
        0,
        Rect::new(ORIGIN_X + 700.0, 500.0, 100.0, 100.0),
        "clamped to the display",
        &mut failures,
    );
    drop(overlay);

    // On the second display, whose frame is higher: the answer names that display.
    let (overlay, answer) = start(mtm, Mode::Region, Point::new(ORIGIN_X + 1000.0, 300.0));
    drag(&overlay, 1, (50.0, 60.0), (150.0, 160.0));
    mouse(&overlay, 1, NSEventType::LeftMouseUp, 150.0, 160.0);
    expect_region(
        &answer,
        1,
        Rect::new(ORIGIN_X + WIDTH + 50.0, 180.0, 100.0, 100.0),
        "second display",
        &mut failures,
    );
    drop(overlay);

    // Return confirms while the button is still down.
    let (overlay, answer) = start(mtm, Mode::Region, pointer);
    drag(&overlay, 0, (10.0, 10.0), (110.0, 60.0));
    key(&overlay, 0, NSEventType::KeyDown, RETURN);
    expect_region(
        &answer,
        0,
        Rect::new(ORIGIN_X + 10.0, 10.0, 100.0, 50.0),
        "return",
        &mut failures,
    );
    drop(overlay);

    // Space moves the selection instead of resizing it.
    let (overlay, answer) = start(mtm, Mode::Region, pointer);
    drag(&overlay, 0, (100.0, 100.0), (200.0, 200.0));
    key(&overlay, 0, NSEventType::KeyDown, SPACE);
    mouse(&overlay, 0, NSEventType::LeftMouseDragged, 250.0, 230.0);
    save(
        &folder,
        "overlay-moving.png",
        render(&overlay.view(0).unwrap(), none, (ORIGIN_X, 0.0)),
        &mut failures,
    );
    key(&overlay, 0, NSEventType::KeyUp, SPACE);
    mouse(&overlay, 0, NSEventType::LeftMouseUp, 250.0, 230.0);
    expect_region(
        &answer,
        0,
        Rect::new(ORIGIN_X + 150.0, 130.0, 100.0, 100.0),
        "space moves",
        &mut failures,
    );
    drop(overlay);

    // Escape cancels, also in the middle of a drag.
    let (overlay, answer) = start(mtm, Mode::Region, pointer);
    drag(&overlay, 0, (100.0, 100.0), (200.0, 200.0));
    key(&overlay, 0, NSEventType::KeyDown, ESCAPE);
    spin(0.05);
    if !matches!(answer.borrow_mut().take(), Some(Pick::Cancelled)) {
        failures.push("escape did not cancel".into());
    }
    drop(overlay);
    if panels_open(mtm) != 0 {
        failures.push("panels left after escape".into());
    }

    // A click is not a region: the overlay stays and a later drag works.
    let (overlay, answer) = start(mtm, Mode::Region, pointer);
    mouse(&overlay, 0, NSEventType::LeftMouseDown, 50.0, 50.0);
    mouse(&overlay, 0, NSEventType::LeftMouseUp, 50.0, 50.0);
    spin(0.05);
    if answer.borrow().is_some() {
        failures.push("a click gave an answer".into());
    }
    drag(&overlay, 0, (50.0, 50.0), (150.0, 150.0));
    mouse(&overlay, 0, NSEventType::LeftMouseUp, 150.0, 150.0);
    expect_region(
        &answer,
        0,
        Rect::new(ORIGIN_X + 50.0, 50.0, 100.0, 100.0),
        "drag after a click",
        &mut failures,
    );
    drop(overlay);

    // Window mode: the front window under the pointer is chosen by a click.
    let list = windows();
    let (overlay, answer) = start(mtm, Mode::Window(list.clone()), pointer);
    mouse(&overlay, 0, NSEventType::MouseMoved, 450.0, 300.0);
    save(
        &folder,
        "overlay-window.png",
        render(&overlay.view(0).unwrap(), &list, (ORIGIN_X, 0.0)),
        &mut failures,
    );
    mouse(&overlay, 0, NSEventType::LeftMouseDown, 450.0, 300.0);
    mouse(&overlay, 0, NSEventType::LeftMouseUp, 450.0, 300.0);
    spin(0.05);
    match answer.borrow_mut().take() {
        Some(Pick::Window(w)) if w.id == 11 => {}
        other => failures.push(format!("window mode: wanted window 11, got {other:?}")),
    }
    drop(overlay);

    // A click on bare background in window mode chooses nothing; Escape cancels.
    let (overlay, answer) = start(mtm, Mode::Window(list), pointer);
    mouse(&overlay, 0, NSEventType::LeftMouseDown, 790.0, 590.0);
    mouse(&overlay, 0, NSEventType::LeftMouseUp, 790.0, 590.0);
    spin(0.05);
    if answer.borrow().is_some() {
        failures.push("a click on nothing chose a window".into());
    }
    key(&overlay, 0, NSEventType::KeyDown, ESCAPE);
    spin(0.05);
    if !matches!(answer.borrow_mut().take(), Some(Pick::Cancelled)) {
        failures.push("escape did not cancel window mode".into());
    }
    drop(overlay);

    // Taking the overlay down by dropping it ends it quietly.
    let (overlay, answer) = start(mtm, Mode::Region, pointer);
    drop(overlay);
    spin(0.05);
    if answer.borrow().is_some() || panels_open(mtm) != 0 {
        failures.push("dropping the overlay left something behind".into());
    }

    // The confirmation, drawn without a window, with a thumbnail made here.
    let thumb = image::from_pixels(160, 100, |x, y| {
        [(x * 255 / 160) as u8, (y * 255 / 100) as u8, 200, 255]
    })
    .and_then(|rep| rep.CGImage())
    .and_then(|cg| image::png(&cg));
    for (name, title, detail, opaque) in [
        (
            "toast-saved.png",
            "Screenshot copied and saved",
            "Figghost 2026-10-15 at 21.04.33.png",
            false,
        ),
        (
            "toast-text-error.png",
            "Text could not be read",
            "text recognition is not available yet",
            false,
        ),
        (
            "toast-opaque.png",
            "Screenshot copied",
            "Reduce Transparency is on",
            true,
        ),
    ] {
        let spec = toast::Spec {
            title: title.into(),
            detail: detail.into(),
            detail_is_name: name.contains("saved"),
            thumbnail: thumb.clone(),
            display: None,
            seconds: 3.0,
        };
        save(
            &folder,
            name,
            toast::snapshot_png(mtm, &spec, opaque),
            &mut failures,
        );
    }

    // The confirmation on a display outside every screen: it appears, then goes away by itself.
    let off = displays()[0];
    toast::show(
        mtm,
        toast::Spec {
            title: "Screenshot copied".into(),
            detail: "offscreen".into(),
            detail_is_name: false,
            thumbnail: thumb,
            display: Some(off),
            seconds: 0.3,
        },
    );
    spin(0.1);
    if toast::window_numbers().is_empty() {
        failures.push("the confirmation has no window while it shows".into());
    }
    spin(1.0);
    if !toast::window_numbers().is_empty() {
        failures.push("the confirmation did not go away".into());
    }
    failures
}

fn main() {
    let mtm = MainThreadMarker::new().expect("main thread");
    let folder = std::env::args().nth(1).map_or_else(
        || PathBuf::from("/tmp/figghost-capture-check"),
        PathBuf::from,
    );
    app::run_accessory(mtm, move || {
        let failures = run(mtm, folder);
        for failure in &failures {
            println!("FAIL: {failure}");
        }
        if failures.is_empty() {
            println!("capture overlay checks passed");
        }
        process::exit(i32::from(!failures.is_empty()));
    });
}
