//! Shows a plain window with known text at a fixed place for a few seconds, as a target for checking
//! text recognition and captures. The window's content area starts 100 points from the left and 140
//! from the top of the main screen.
//!
//!     text_window [seconds]

use objc2::{MainThreadMarker, MainThreadOnly, rc::Retained};
use objc2_app_kit::{
    NSApplication, NSApplicationActivationPolicy, NSBackingStoreType, NSColor, NSFont, NSScreen,
    NSTextField, NSWindow, NSWindowStyleMask,
};
use objc2_foundation::{NSDate, NSPoint, NSRect, NSRunLoop, NSSize, NSString};
use std::{env, time::Instant};

pub const LINES: [&str; 2] = [
    "Bộ trộn âm thanh và chụp màn hình",
    "Figghost OCR check 2026",
];

fn main() {
    let mtm = MainThreadMarker::new().expect("main thread");
    let app = NSApplication::sharedApplication(mtm);
    app.setActivationPolicy(NSApplicationActivationPolicy::Accessory);
    let seconds: f64 = env::args()
        .nth(1)
        .and_then(|a| a.parse().ok())
        .unwrap_or(15.0);
    let screen_height = NSScreen::screens(mtm)
        .firstObject()
        .unwrap()
        .frame()
        .size
        .height;
    let size = NSSize::new(900.0, 220.0);
    let frame = NSRect::new(
        NSPoint::new(100.0, screen_height - 140.0 - size.height),
        size,
    );
    // SAFETY: designated initializer with valid arguments on the main thread.
    let window: Retained<NSWindow> = unsafe {
        NSWindow::initWithContentRect_styleMask_backing_defer(
            NSWindow::alloc(mtm),
            frame,
            NSWindowStyleMask::Borderless,
            NSBackingStoreType::Buffered,
            false,
        )
    };
    // SAFETY: false is the only value compatible with a window kept alive by `Retained`.
    unsafe { window.setReleasedWhenClosed(false) };
    window.setBackgroundColor(Some(&NSColor::whiteColor()));
    window.setLevel(objc2_app_kit::NSFloatingWindowLevel);
    for (i, line) in LINES.iter().enumerate() {
        let label = NSTextField::labelWithString(&NSString::from_str(line), mtm);
        label.setFont(Some(&NSFont::systemFontOfSize(44.0)));
        label.setTextColor(Some(&NSColor::blackColor()));
        label.setFrame(NSRect::new(
            NSPoint::new(30.0, 130.0 - i as f64 * 90.0),
            NSSize::new(840.0, 60.0),
        ));
        window.contentView().unwrap().addSubview(&label);
    }
    window.orderFrontRegardless();
    let start = Instant::now();
    while start.elapsed().as_secs_f64() < seconds {
        NSRunLoop::mainRunLoop().runUntilDate(&NSDate::dateWithTimeIntervalSinceNow(0.1));
    }
}
