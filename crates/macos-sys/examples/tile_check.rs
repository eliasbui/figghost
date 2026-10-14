//! Checks the window tiling path against a window this process creates, never anyone else's:
//! reads it through the Accessibility API, applies several layouts and verifies the result.
//! `cargo run -p macos-sys --example tile_check`

use core_window::{Layout, Rect, pick_area};
use macos_sys::{
    MainThreadMarker,
    window::{Frame, Window, screen_areas},
};
use objc2_app_kit::{
    NSApplication, NSApplicationActivationPolicy, NSBackingStoreType, NSWindow, NSWindowStyleMask,
};
use objc2_foundation::{NSDate, NSPoint, NSRect, NSRunLoop, NSSize, NSString};
use std::process;

fn rect(f: Frame) -> Rect {
    Rect::new(f.x, f.y, f.w, f.h)
}

fn frame(r: Rect) -> Frame {
    Frame {
        x: r.x,
        y: r.y,
        w: r.w,
        h: r.h,
    }
}

fn settle(seconds: f64) {
    NSRunLoop::currentRunLoop().runUntilDate(&NSDate::dateWithTimeIntervalSinceNow(seconds));
}

fn main() {
    let mtm = MainThreadMarker::new().expect("main thread");
    let app = NSApplication::sharedApplication(mtm);
    app.setActivationPolicy(NSApplicationActivationPolicy::Accessory);

    // SAFETY: designated initializer with valid arguments on the main thread.
    let window = unsafe {
        NSWindow::initWithContentRect_styleMask_backing_defer(
            mtm.alloc(),
            NSRect::new(NSPoint::new(300.0, 300.0), NSSize::new(520.0, 360.0)),
            NSWindowStyleMask::Titled | NSWindowStyleMask::Resizable | NSWindowStyleMask::Closable,
            NSBackingStoreType::Buffered,
            false,
        )
    };
    window.setTitle(&NSString::from_str("tile_check"));
    window.orderFrontRegardless();
    app.finishLaunching();
    // The window shows up in the Accessibility tree a moment after it is ordered in.
    settle(0.8);

    let areas: Vec<Rect> = screen_areas(mtm).into_iter().map(rect).collect();
    println!("screen areas: {areas:?}");
    let Some(target) = Window::first_of(process::id() as i32) else {
        println!(
            "FAIL: no Accessibility window for this process (is Accessibility access granted?)"
        );
        process::exit(2);
    };

    let mut failed = false;
    for layout in [
        Layout::Left,
        Layout::Right,
        Layout::TopLeft,
        Layout::BottomRight,
        Layout::FirstThird,
        Layout::LastThird,
        Layout::Center,
        Layout::Maximize,
    ] {
        let current = rect(target.frame().expect("frame"));
        let area = pick_area(current, &areas).expect("a screen");
        let want = layout.place(area, current);
        target.set_frame(frame(want));
        settle(0.25);
        let got = rect(target.frame().expect("frame"));
        let close = |a: f64, b: f64| (a - b).abs() <= 2.0;
        let ok = close(got.x, want.x)
            && close(got.y, want.y)
            && close(got.w, want.w)
            && close(got.h, want.h);
        println!(
            "  {} {:<13} want {:?} got {:?}",
            if ok { "ok  " } else { "FAIL" },
            layout.key(),
            want,
            got
        );
        failed |= !ok;
    }
    window.close();
    println!(
        "{}",
        if failed {
            "tiling check: FAIL"
        } else {
            "tiling check: pass"
        }
    );
    process::exit(i32::from(failed));
}
