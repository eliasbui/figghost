//! Opens the real glass window (never shown on screen), resizes it and checks that the web view inside
//! follows. Exits with status 1 on a failure.
//! `cargo run -p macos-sys --example window_check`

use macos_sys::{
    MainThreadMarker, app,
    webview::{WebWindow, WindowSpec},
};
use objc2_app_kit::NSApplication;
use objc2_foundation::NSSize;
use std::process;

fn main() {
    let mtm = MainThreadMarker::new().expect("main thread");
    app::run_accessory(mtm, move || {
        let window = WebWindow::new(
            mtm,
            &WindowSpec {
                title: "check",
                width: 1000.0,
                height: 700.0,
                min_width: 820.0,
                min_height: 560.0,
            },
            "<!doctype html><body>check</body>",
            |_| {},
            || {},
        );
        let check = std::rc::Rc::clone(&window);
        app::after(0.5, move || {
            let mut failures = Vec::new();
            let size = |window: &WebWindow| {
                let frame = window.host.view.frame();
                (frame.size.width.round(), frame.size.height.round())
            };
            let initial = size(&check);
            println!("web view at the start: {initial:?}");
            if initial != (1000.0, 700.0) {
                failures.push("the web view does not fill the window at the start");
            }
            let app = NSApplication::sharedApplication(mtm);
            let ns_window = app
                .windows()
                .iter()
                .find(|w| w.frame().size.width > 400.0)
                .expect("the window exists");
            for (w, h) in [(1180.0, 780.0), (840.0, 580.0), (1400.0, 900.0)] {
                ns_window.setContentSize(NSSize::new(w, h));
                let now = size(&check);
                println!("resized to {w}x{h}: web view is {now:?}");
                if now != (w, h) {
                    failures.push("the web view did not follow a resize");
                }
            }
            // AppKit applies the minimum while the user drags the edge, so check what it was told.
            let floor = ns_window.contentMinSize();
            println!("minimum content size: {}x{}", floor.width, floor.height);
            if (floor.width, floor.height) != (820.0, 560.0) {
                failures.push("the minimum size was not set");
            }
            for f in &failures {
                println!("FAIL: {f}");
            }
            println!(
                "{}",
                if failures.is_empty() {
                    "window layout ok"
                } else {
                    "window layout broken"
                }
            );
            process::exit(i32::from(!failures.is_empty()));
        });
        window
    });
}
