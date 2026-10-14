//! Test target for paste delivery: a regular app with a read-only text view that appends every key event
//! and key-window change it sees to a log file. Wrap it in a bundle and start it with `open -n` so macOS
//! activates it, and it plays the app the user was working in before the popup opened.
//! `paste_target <log file>`

use block2::RcBlock;
use macos_sys::MainThreadMarker;
use objc2_app_kit::{
    NSApplication, NSApplicationActivationPolicy, NSBackingStoreType, NSEvent, NSEventMask,
    NSEventModifierFlags, NSEventType, NSTextView, NSWindow, NSWindowDidBecomeKeyNotification,
    NSWindowDidResignKeyNotification, NSWindowStyleMask,
};
use objc2_foundation::{NSNotification, NSNotificationCenter, NSPoint, NSRect, NSSize, NSString};
use std::{
    env,
    fs::OpenOptions,
    io::Write,
    path::PathBuf,
    ptr::NonNull,
    rc::Rc,
    time::{SystemTime, UNIX_EPOCH},
};

fn main() {
    let mtm = MainThreadMarker::new().expect("main thread");
    let log_path: PathBuf = env::args()
        .nth(1)
        .expect("usage: paste_target <log file>")
        .into();
    let log = Rc::new(move |line: String| {
        if let Ok(mut file) = OpenOptions::new().create(true).append(true).open(&log_path) {
            let epoch = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map_or(0, |d| d.as_millis());
            let _ = writeln!(file, "{epoch} {line}");
        }
    });

    let app = NSApplication::sharedApplication(mtm);
    app.setActivationPolicy(NSApplicationActivationPolicy::Regular);

    let frame = NSRect::new(NSPoint::new(200.0, 200.0), NSSize::new(360.0, 120.0));
    // SAFETY: designated initializer with valid arguments on the main thread.
    let window = unsafe {
        NSWindow::initWithContentRect_styleMask_backing_defer(
            mtm.alloc(),
            frame,
            NSWindowStyleMask::Titled | NSWindowStyleMask::Closable | NSWindowStyleMask::Resizable,
            NSBackingStoreType::Buffered,
            false,
        )
    };
    window.setTitle(&NSString::from_str("PasteTarget"));
    let text = NSTextView::initWithFrame(mtm.alloc(), NSRect::new(NSPoint::ZERO, frame.size));
    text.setEditable(false);
    text.setString(&NSString::from_str("paste target: logs key events only"));
    window.setContentView(Some(&text));
    window.makeKeyAndOrderFront(None);
    window.makeFirstResponder(Some(&text));

    let log_keys = Rc::clone(&log);
    let on_key = RcBlock::new(move |event: NonNull<NSEvent>| -> *mut NSEvent {
        // SAFETY: the monitor hands out a valid event for the duration of the call.
        let e = unsafe { event.as_ref() };
        let phase = if e.r#type() == NSEventType::KeyDown {
            "down"
        } else {
            "up"
        };
        log_keys(format!(
            "key {phase} code={} command={}",
            e.keyCode(),
            e.modifierFlags().contains(NSEventModifierFlags::Command)
        ));
        event.as_ptr()
    });
    // SAFETY: the block is 'static and runs on the main thread.
    let _monitor = unsafe {
        NSEvent::addLocalMonitorForEventsMatchingMask_handler(
            NSEventMask::KeyDown | NSEventMask::KeyUp,
            &on_key,
        )
    };

    let center = NSNotificationCenter::defaultCenter();
    let (became, resigned) = (Rc::clone(&log), Rc::clone(&log));
    let on_become =
        RcBlock::new(move |_: NonNull<NSNotification>| became("window became key".into()));
    let on_resign =
        RcBlock::new(move |_: NonNull<NSNotification>| resigned("window resigned key".into()));
    // SAFETY: the blocks are 'static and run on the main thread, where the notifications are posted.
    let _observers = unsafe {
        [
            center.addObserverForName_object_queue_usingBlock(
                Some(NSWindowDidBecomeKeyNotification),
                None,
                None,
                &on_become,
            ),
            center.addObserverForName_object_queue_usingBlock(
                Some(NSWindowDidResignKeyNotification),
                None,
                None,
                &on_resign,
            ),
        ]
    };

    log("started".into());
    app.run();
}
