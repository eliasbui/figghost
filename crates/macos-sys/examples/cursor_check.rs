//! `cursor_check apply [neon|<sheet.png>|<file.gif>] [seconds]` replaces the arrow and the hand and waits
//! (0 seconds: leaves at once, without giving them back, to see what survives the process); `cursor_check
//! restore` gives them back. Look at the real pointer, or take `screencapture -C`.
use macos_sys::cursor_ui::{Frames, Kind, apply, restore};
use objc2::MainThreadMarker;
use objc2_app_kit::{
    NSApplication, NSApplicationActivationPolicy, NSBackingStoreType, NSColor, NSWindow,
    NSWindowStyleMask,
};
use objc2_foundation::{NSPoint, NSRect, NSSize};

#[link(name = "CoreGraphics", kind = "framework")]
unsafe extern "C" {
    fn CGWarpMouseCursorPosition(point: NSPoint) -> i32;
}

/// Shows a white window at (100, 100) of the screen (a size of 400 by 300) with the pointer in its middle, so a
/// screenshot with `screencapture -C -R100,100,400,300` shows the cursor on a clean ground, then quits.
fn show_window(seconds: u64) {
    let mtm = MainThreadMarker::new().expect("main thread");
    let app = NSApplication::sharedApplication(mtm);
    app.setActivationPolicy(NSApplicationActivationPolicy::Regular);
    let window = unsafe {
        NSWindow::initWithContentRect_styleMask_backing_defer(
            mtm.alloc(),
            NSRect::new(NSPoint::new(100.0, 680.0), NSSize::new(400.0, 300.0)),
            NSWindowStyleMask::Titled,
            NSBackingStoreType::Buffered,
            false,
        )
    };
    window.setBackgroundColor(Some(&NSColor::whiteColor()));
    window.makeKeyAndOrderFront(None);
    #[allow(deprecated)]
    app.activateIgnoringOtherApps(true);
    std::thread::spawn(move || {
        for _ in 0..seconds * 4 {
            std::thread::sleep(std::time::Duration::from_millis(250));
            // SAFETY: a plain call with a point.
            unsafe { CGWarpMouseCursorPosition(NSPoint::new(300.0, 250.0)) };
        }
        std::process::exit(0);
    });
    app.run();
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    match args.get(1).map(String::as_str) {
        Some("restore") => restore(),
        Some("window") => show_window(args.get(2).and_then(|s| s.parse().ok()).unwrap_or(10)),
        Some("apply") => {
            let what = args.get(2).map_or("neon", String::as_str);
            let frames = if what == "neon" {
                Frames::neon()
            } else {
                let data = std::fs::read(what).expect("a readable file");
                Frames::from_sheet(&data)
                    .or_else(|| Frames::from_file(&data))
                    .expect("a picture")
            };
            let env = |k: &str, d: f64| {
                std::env::var(k)
                    .ok()
                    .and_then(|v| v.parse().ok())
                    .unwrap_or(d)
            };
            let order: Vec<usize> = (0..frames.len().min(env("N", 99.0) as usize)).collect();
            let size = env("SIZE", 40.0);
            let (a, b) = (
                apply(Kind::Arrow, &frames, &order, size, (0.11, 0.11), 0.08),
                apply(Kind::Link, &frames, &order, size, (0.11, 0.11), 0.08),
            );
            println!("arrow {a} link {b} frames {}", frames.len());
            let seconds: u64 = args.get(3).and_then(|s| s.parse().ok()).unwrap_or(10);
            if args.get(4).map(String::as_str) == Some("window") {
                show_window(seconds);
            }
            std::thread::sleep(std::time::Duration::from_secs(seconds));
            if std::env::var_os("RESTORE").is_some() {
                restore();
                println!("restored");
                std::thread::sleep(std::time::Duration::from_secs(4));
            }
        }
        _ => eprintln!("usage: cursor_check apply [neon|file] [seconds] | restore"),
    }
}
