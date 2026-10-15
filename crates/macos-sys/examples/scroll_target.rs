//! A window with a long list of numbered lines at a fixed place, as a target for checking scrolling
//! capture. Its scrolling area covers 100 to 800 points from the left and 140 to 540 from the top of the
//! main screen.
//!
//!     scroll_target [seconds]

use objc2::{MainThreadMarker, MainThreadOnly, rc::Retained};
use objc2_app_kit::{
    NSApplication, NSApplicationActivationPolicy, NSBackingStoreType, NSColor, NSEventMask, NSFont,
    NSScreen, NSTextView, NSWindow, NSWindowStyleMask,
};
use objc2_foundation::{NSDate, NSDefaultRunLoopMode, NSPoint, NSRect, NSSize, NSString};
use std::{env, fmt::Write, time::Instant};

pub const LINES: usize = 300;

fn main() {
    let mtm = MainThreadMarker::new().expect("main thread");
    let app = NSApplication::sharedApplication(mtm);
    app.setActivationPolicy(NSApplicationActivationPolicy::Accessory);
    let seconds: f64 = env::args()
        .nth(1)
        .and_then(|a| a.parse().ok())
        .unwrap_or(30.0);
    let screen_height = NSScreen::screens(mtm)
        .firstObject()
        .unwrap()
        .frame()
        .size
        .height;
    let size = NSSize::new(700.0, 400.0);
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
    window.setLevel(objc2_app_kit::NSFloatingWindowLevel);
    let scroll = NSTextView::scrollableTextView(mtm);
    scroll.setFrame(NSRect::new(NSPoint::ZERO, size));
    let mut text = String::new();
    for i in 1..=LINES {
        let _ = writeln!(
            text,
            "Line {i:03}: the quick brown fox jumps over the lazy dog {}",
            i * 7919 % 1000
        );
    }
    if let Some(view) = scroll
        .documentView()
        .and_then(|v| v.downcast::<NSTextView>().ok())
    {
        view.setString(&NSString::from_str(&text));
        view.setFont(Some(&NSFont::monospacedSystemFontOfSize_weight(16.0, 0.0)));
        view.setTextColor(Some(&NSColor::blackColor()));
        view.setBackgroundColor(&NSColor::whiteColor());
        view.setEditable(false);
    }
    window.setContentView(Some(&scroll));
    window.orderFrontRegardless();
    app.finishLaunching();
    let start = Instant::now();
    // Events have to be taken from the queue and sent on, as NSApplication's own loop does, or the
    // scroll view never sees the wheel.
    while start.elapsed().as_secs_f64() < seconds {
        let until = NSDate::dateWithTimeIntervalSinceNow(0.05);
        // SAFETY: the default run loop mode is a framework constant.
        let event = unsafe {
            app.nextEventMatchingMask_untilDate_inMode_dequeue(
                NSEventMask::Any,
                Some(&until),
                NSDefaultRunLoopMode,
                true,
            )
        };
        if let Some(event) = event {
            app.sendEvent(&event);
        }
    }
}
