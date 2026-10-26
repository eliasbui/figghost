//! Shows the sky of the Weather tab on the real screen, in a small window of its own with no focus and no pointer,
//! lets it move for two seconds and photographs it with `screencapture`, which is what the offscreen pictures cannot
//! say: that the GPU's layer really reaches the window server. Needs the Screen Recording permission for the terminal
//! and an unlocked screen.
//! `cargo run -p macos-sys --example sky_live_check -- /tmp/sky_live.png`

use macos_sys::{
    MainThreadMarker, app,
    weather_fx::{Atmosphere, FxView},
};
use objc2::rc::Retained;
use objc2_app_kit::{
    NSBackingStoreType, NSColor, NSFloatingWindowLevel, NSScreen, NSView, NSWindow,
    NSWindowStyleMask,
};
use objc2_foundation::{NSPoint, NSRect, NSSize};
use std::{env, process, process::Command};

fn main() {
    let mtm = MainThreadMarker::new().expect("main thread");
    let out = env::args()
        .nth(1)
        .unwrap_or_else(|| "/tmp/sky_live.png".into());
    app::run_accessory(mtm, move || {
        let screen = NSScreen::mainScreen(mtm).expect("a screen").frame();
        let size = NSSize::new(580.0, 250.0);
        let area = NSRect::new(
            NSPoint::new(screen.origin.x + 60.0, screen.origin.y + 60.0),
            size,
        );
        // SAFETY: designated initializer with valid arguments on the main thread.
        let window = unsafe {
            NSWindow::initWithContentRect_styleMask_backing_defer(
                mtm.alloc(),
                area,
                NSWindowStyleMask::Borderless,
                NSBackingStoreType::Buffered,
                false,
            )
        };
        // SAFETY: false is the only value compatible with a window kept alive by `Retained`.
        unsafe { window.setReleasedWhenClosed(false) };
        window.setOpaque(false);
        window.setBackgroundColor(Some(&NSColor::blackColor()));
        window.setLevel(NSFloatingWindowLevel);
        window.setIgnoresMouseEvents(true);
        let inside = NSRect::new(NSPoint::ZERO, size);
        // SAFETY: designated initializer with a valid frame on the main thread.
        let root: Retained<NSView> =
            unsafe { objc2::msg_send![mtm.alloc::<NSView>(), initWithFrame: inside] };
        let fx = FxView::new(mtm, inside);
        root.addSubview(&fx);
        window.setContentView(Some(&root));
        window.orderFrontRegardless();
        fx.set(Some(Atmosphere {
            day: true,
            progress: 0.4,
            cloud: 1.0,
            rain: 0.8,
            thunder: true,
            wind: 0.6,
            wind_dir: 1.0,
            leaf: (0.95, 0.56, 0.16),
            ..Atmosphere::default()
        }));
        let moving = Retained::clone(&fx);
        let timer = app::every(1.0 / 30.0, move || {
            moving.tick(1.0 / 30.0);
        });
        // screencapture takes the rectangle with its origin at the top left of the main screen.
        let rect = format!(
            "{},{},{},{}",
            area.origin.x,
            screen.size.height - (area.origin.y + area.size.height),
            area.size.width,
            area.size.height
        );
        app::after(2.0, move || {
            let ok = Command::new("screencapture")
                .args(["-x", "-R", &rect])
                .arg(&out)
                .status()
                .is_ok_and(|s| s.success());
            println!("{} {out}", if ok { "wrote" } else { "could not write" });
            process::exit(if ok { 0 } else { 1 });
        });
        (window, root, fx, timer)
    });
}
