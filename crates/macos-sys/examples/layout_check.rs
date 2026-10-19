//! Checks that every page of the notch lands inside the window once it is open, including pages added
//! while the notch was still collapsed, that the strip of page icons has one icon per page and sits at the right of
//! the header with the three dots at its left, and that the shape is the size of the page it shows. Exits with
//! status 1 on a failure.
//! `cargo run -p macos-sys --example layout_check`

use macos_sys::{
    MainThreadMarker, app,
    focus_ui::{FocusCallbacks, FocusView},
    media_ui::{MediaCallbacks, MediaView},
    notch::{Notch, NotchGeometry},
    shelf_ui::{ShelfCallbacks, ShelfView},
    ui::AppCallbacks,
};
use objc2_app_kit::NSApplication;
use objc2_foundation::{NSPoint, NSRect, NSSize};
use std::{process, rc::Rc};

/// Prints a line per problem and returns how many there were.
fn inspect(mtm: MainThreadMarker, open: NSSize) -> usize {
    let mut failures = 0;
    let mut fail = |message: String| {
        println!("FAIL: {message}");
        failures += 1;
    };
    let app = NSApplication::sharedApplication(mtm);
    let windows = app.windows();
    // The open notch's window is at least as wide as the open shape (wider by the room for the fillets).
    let Some(window) = windows.iter().find(|w| w.frame().size.width >= open.width) else {
        println!("FAIL: no window of the expanded size");
        return 1;
    };
    // The window holds the shape; the shape holds the background, the ears and the pages' clip, and the clip holds the
    // content, which is the one with the page strip in it.
    let holds_strip = |view: &objc2_app_kit::NSView| {
        view.subviews()
            .iter()
            .any(|s| s.class().name().to_string_lossy().contains("PageSwitcher"))
    };
    let content_of = |shape: &objc2_app_kit::NSView| {
        shape
            .subviews()
            .iter()
            .find_map(|clip| clip.subviews().iter().find(|v| holds_strip(v)))
    };
    let Some(surface) = window
        .contentView()
        .and_then(|v| v.subviews().iter().find(|s| content_of(s).is_some()))
    else {
        println!("FAIL: no open shape holds the pages");
        return 1;
    };
    let shape = surface.frame().size;
    if shape != open {
        fail(format!(
            "the open shape is {shape:?}, expected the size of its page {open:?}"
        ));
    }
    let Some(container) = content_of(&surface) else {
        println!("FAIL: the shape has no content container");
        return 1;
    };
    // The content is as large as the largest page, centred on the shape and against its top.
    let frame = container.frame();
    let expected = (
        (shape.width - frame.size.width) / 2.0,
        shape.height - frame.size.height,
    );
    if (frame.origin.x - expected.0).abs() > 0.5 || (frame.origin.y - expected.1).abs() > 0.5 {
        fail(format!(
            "the container sits at ({}, {}), expected {expected:?}",
            frame.origin.x, frame.origin.y
        ));
    }
    // Where the shape's left edge is in the content.
    let left = (frame.size.width - shape.width) / 2.0;
    for (i, view) in container.subviews().iter().enumerate() {
        let f = view.frame();
        let inside = f.origin.x >= 0.0
            && f.origin.y >= 0.0
            && f.origin.x + f.size.width <= frame.size.width + 0.5
            && f.origin.y + f.size.height <= frame.size.height + 0.5;
        let name = view.class().name().to_string_lossy().into_owned();
        println!(
            "  {i}: {name} ({}, {}, {}, {}) hidden={}",
            f.origin.x,
            f.origin.y,
            f.size.width,
            f.size.height,
            view.isHidden()
        );
        if !inside {
            fail(format!("subview {i} ({name}) lies outside the window"));
        }
        if name.contains("PageSwitcher") {
            // One icon per page (clipboard, shelf, media, focus), at the right edge of the shape.
            let expected = macos_sys::switcher::width_for(4);
            if (f.size.width - expected).abs() > 0.5 {
                fail(format!(
                    "the page strip is {} wide, expected {expected}",
                    f.size.width
                ));
            }
            let gap = left + shape.width - (f.origin.x + f.size.width);
            if !(8.0..=24.0).contains(&gap) {
                fail(format!(
                    "the page strip ends {gap} short of the shape's right edge, expected about 16"
                ));
            }
        }
        if name.contains("NotchOrb") && !(8.0..=24.0).contains(&(f.origin.x - left)) {
            fail(format!(
                "the three dots start {} from the shape's left edge, expected about 16",
                f.origin.x - left
            ));
        }
    }
    failures
}

fn main() {
    let mtm = MainThreadMarker::new().expect("main thread");
    app::run_accessory(mtm, move || {
        // Far outside every screen: the layout is the same there, and nothing flashes on the real one.
        let geometry = NotchGeometry {
            screen: NSRect::new(NSPoint::new(-30_000.0, 0.0), NSSize::new(1_920.0, 1_080.0)),
            notch: NSSize::new(200.0, 32.0),
            hardware: false,
        };
        let noop = AppCallbacks {
            on_query: Box::new(|_, _| {}),
            on_activate: Box::new(|_, _, _| {}),
            on_pin: Box::new(|_, _| {}),
            on_delete: Box::new(|_, _| {}),
        };
        let notch = Rc::new(Notch::new(mtm, geometry, noop, |_| {}));
        // Added while the notch is collapsed, which is exactly where autoresizing used to misplace them.
        let shelf = ShelfView::new(
            mtm,
            notch.page_frame(),
            ShelfCallbacks {
                on_drop: Box::new(|_| {}),
                on_remove: Box::new(|_| {}),
                on_open: Box::new(|_| {}),
                actions_for: Box::new(|_| Vec::new()),
                on_action: Box::new(|_, _| {}),
            },
        );
        notch.add_page(shelf.page(|| {}));
        let media = MediaView::new(
            mtm,
            notch.page_frame(),
            MediaCallbacks {
                on_previous: Box::new(|| {}),
                on_play_pause: Box::new(|| {}),
                on_next: Box::new(|| {}),
                on_seek: Box::new(|_| {}),
            },
        );
        notch.add_page(media.page(|| {}, || {}));
        let focus = FocusView::new(
            mtm,
            notch.page_frame(),
            &[15, 25, 45, 60],
            FocusCallbacks {
                on_toggle: Box::new(|| {}),
                on_reset: Box::new(|| {}),
                on_preset: Box::new(|_| {}),
                on_adjust: Box::new(|_| {}),
            },
        );
        notch.add_page(focus.page(|| {}, || {}));

        let opener = Rc::clone(&notch);
        app::after(0.3, move || opener.open());
        let sized = Rc::clone(&notch);
        app::after(1.5, move || {
            let failures = inspect(mtm, sized.open_size());
            println!(
                "{}",
                if failures == 0 {
                    "layout ok"
                } else {
                    "layout broken"
                }
            );
            process::exit(i32::from(failures > 0));
        });
        (notch, shelf, media, focus)
    });
}
