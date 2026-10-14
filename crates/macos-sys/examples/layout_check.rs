//! Checks that every page of the notch lands inside the window once it is open, including pages added
//! while the notch was still collapsed, and that the tab bar is centred. Exits with status 1 on a failure.
//! `cargo run -p macos-sys --example layout_check`

use macos_sys::{
    MainThreadMarker, app,
    media_ui::{MediaCallbacks, MediaView},
    notch::{EXPANDED_SIZE, Notch, NotchGeometry},
    shelf_ui::{ShelfCallbacks, ShelfView},
    ui::AppCallbacks,
};
use objc2_app_kit::NSApplication;
use std::{process, rc::Rc};

/// Prints a line per problem and returns how many there were.
fn inspect(mtm: MainThreadMarker) -> usize {
    let mut failures = 0;
    let mut fail = |message: String| {
        println!("FAIL: {message}");
        failures += 1;
    };
    let app = NSApplication::sharedApplication(mtm);
    let windows = app.windows();
    let Some(window) = windows
        .iter()
        .find(|w| w.frame().size.width == EXPANDED_SIZE.width)
    else {
        println!("FAIL: no window of the expanded size");
        return 1;
    };
    let Some(container) = window
        .contentView()
        .and_then(|v| v.subviews().firstObject())
    else {
        println!("FAIL: the shape has no content container");
        return 1;
    };
    let frame = container.frame();
    if (frame.origin.x, frame.origin.y) != (0.0, 0.0) {
        fail(format!(
            "the container sits at ({}, {}), expected (0, 0)",
            frame.origin.x, frame.origin.y
        ));
    }
    for (i, view) in container.subviews().iter().enumerate() {
        let f = view.frame();
        let inside = f.origin.x >= 0.0
            && f.origin.y >= 0.0
            && f.origin.x + f.size.width <= EXPANDED_SIZE.width + 0.5
            && f.origin.y + f.size.height <= EXPANDED_SIZE.height + 0.5;
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
        if name.contains("Segmented") {
            let middle = f.origin.x + f.size.width / 2.0;
            if (middle - EXPANDED_SIZE.width / 2.0).abs() > 1.0 {
                fail(format!(
                    "the tab bar is centred at {middle}, expected {}",
                    EXPANDED_SIZE.width / 2.0
                ));
            }
        }
    }
    failures
}

fn main() {
    let mtm = MainThreadMarker::new().expect("main thread");
    app::run_accessory(mtm, move || {
        let geometry = NotchGeometry::detect(mtm).expect("a screen");
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
            },
        );
        notch.add_page(media.page(|| {}, || {}));

        let opener = Rc::clone(&notch);
        app::after(0.3, move || opener.open());
        app::after(1.5, move || {
            let failures = inspect(mtm);
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
        (notch, shelf, media)
    });
}
