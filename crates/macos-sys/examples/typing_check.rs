//! Checks Vietnamese typing end to end: opens a window of its own with a text view, starts the keyboard of
//! `macos_sys::typing` in Vietnamese, posts the keys of a few words where the hardware's keys enter the system, and
//! reads back what the text view got. It posts real key events, so it posts only while its own window has the
//! keyboard (it looks before each word and stops otherwise), and it refuses to run while another Vietnamese input
//! method runs. The terminal needs Accessibility access; leave the machine alone for the five seconds it takes.
//! `cargo run -p macos-sys --example typing_check`

use core_typing::Settings;
use macos_sys::{MainThreadMarker, app, ax, permissions, typing};
use objc2::rc::Retained;
use objc2_app_kit::{
    NSApplication, NSApplicationActivationPolicy, NSBackingStoreType, NSTextView, NSWindow,
    NSWindowStyleMask,
};
use objc2_foundation::{NSPoint, NSRect, NSSize, NSString};
use std::{cell::RefCell, ffi::c_void, process, ptr, rc::Rc};

const HID_EVENT_TAP: u32 = 0;
const FLAG_SHIFT: u64 = 0x0002_0000;
const FLAG_CONTROL: u64 = 0x0004_0000;
const SPACE: u16 = 49;

#[link(name = "CoreGraphics", kind = "framework")]
unsafe extern "C" {
    fn CGEventCreateKeyboardEvent(source: *mut c_void, key: u16, down: bool) -> *mut c_void;
    fn CGEventSetFlags(event: *mut c_void, flags: u64);
    fn CGEventPost(tap: u32, event: *mut c_void);
}

#[link(name = "CoreFoundation", kind = "framework")]
unsafe extern "C" {
    fn CFRelease(cf: *const c_void);
}

/// The key that types `c` on an ANSI keyboard, and whether it needs Shift.
fn key_of(c: char) -> (u16, bool) {
    const LOWER: &str = "asdfhgzxcv\u{0}bqweryt123465=97-80]ou[ip\u{0}lj'k;\\,/nm.";
    if c == ' ' {
        return (SPACE, false);
    }
    let lower = c.to_ascii_lowercase();
    let code = LOWER
        .chars()
        .position(|l| l == lower)
        .unwrap_or_else(|| panic!("no key types {c:?}"));
    (code as u16, c.is_ascii_uppercase())
}

fn post(code: u16, flags: u64) {
    for down in [true, false] {
        // SAFETY: the event is created, configured, posted and released here.
        unsafe {
            let event = CGEventCreateKeyboardEvent(ptr::null_mut(), code, down);
            assert!(!event.is_null(), "no keyboard event");
            CGEventSetFlags(event, flags);
            CGEventPost(HID_EVENT_TAP, event);
            CFRelease(event);
        }
    }
}

fn type_keys(keys: &str) {
    for c in keys.chars() {
        let (code, shift) = key_of(c);
        post(code, if shift { FLAG_SHIFT } else { 0 });
    }
}

/// A word, or the switch to the other language before the keys.
struct Case {
    toggle_first: bool,
    keys: &'static str,
    expected: &'static str,
}

const CASES: &[Case] = &[
    Case {
        toggle_first: false,
        keys: "Tieengs Vieetj ",
        expected: "Tiếng Việt ",
    },
    Case {
        toggle_first: false,
        keys: "xin chaof cacs banj ",
        expected: "xin chào các bạn ",
    },
    Case {
        toggle_first: false,
        keys: "Ddaay laf nguwowif ",
        expected: "Đây là người ",
    },
    Case {
        toggle_first: false,
        keys: "text with ",
        expected: "text with ",
    },
    // Control+Space switches to English, and is not typed itself.
    Case {
        toggle_first: true,
        keys: "vieetj ",
        expected: "vieetj ",
    },
    Case {
        toggle_first: true,
        keys: "hoaf ",
        expected: "hoà ",
    },
];

struct Run {
    mtm: MainThreadMarker,
    window: Retained<NSWindow>,
    text: Retained<NSTextView>,
    keyboard: typing::Keyboard,
    passed: usize,
}

fn has_keyboard(run: &Run) -> bool {
    let app = NSApplication::sharedApplication(run.mtm);
    app.isActive() && run.window.isKeyWindow() && ax::focused_pid() == Some(process::id() as i32)
}

fn step(run: Rc<RefCell<Run>>, index: usize) {
    if index == CASES.len() {
        let passed = run.borrow().passed;
        println!("typing_check: {passed} of {} matched", CASES.len());
        process::exit(if passed == CASES.len() { 0 } else { 1 });
    }
    if !has_keyboard(&run.borrow()) {
        eprintln!(
            "typing_check: stopped, the window of the check does not have the keyboard (nothing was posted)"
        );
        process::exit(2);
    }
    let case = &CASES[index];
    run.borrow().text.setString(&NSString::from_str(""));
    if case.toggle_first {
        post(SPACE, FLAG_CONTROL);
    }
    type_keys(case.keys);
    app::after(0.8, move || {
        let got = run.borrow().text.string().to_string();
        let matched = got == case.expected;
        println!(
            "{} {:?} -> {got:?} (wanted {:?}, typing in {})",
            if matched { "ok      " } else { "MISMATCH" },
            case.keys,
            case.expected,
            if run.borrow().keyboard.is_vietnamese() {
                "Vietnamese"
            } else {
                "English"
            },
        );
        if matched {
            run.borrow_mut().passed += 1;
        }
        step(run, index + 1);
    });
}

fn main() {
    let mtm = MainThreadMarker::new().expect("main thread");
    if !permissions::accessibility_trusted(false) {
        eprintln!("typing_check: skipped, the terminal needs Accessibility access");
        process::exit(2);
    }
    if let Some(name) = typing::other_input_method() {
        eprintln!(
            "typing_check: skipped, {name} is running and would put every mark a second time"
        );
        process::exit(2);
    }
    let app = NSApplication::sharedApplication(mtm);
    app.setActivationPolicy(NSApplicationActivationPolicy::Regular);
    let frame = NSRect::new(NSPoint::new(240.0, 240.0), NSSize::new(420.0, 140.0));
    // SAFETY: designated initializer with valid arguments on the main thread.
    let window = unsafe {
        NSWindow::initWithContentRect_styleMask_backing_defer(
            mtm.alloc(),
            frame,
            NSWindowStyleMask::Titled,
            NSBackingStoreType::Buffered,
            false,
        )
    };
    // The check keeps its window; nothing else may release it while the app runs.
    // SAFETY: setting a flag on a window this thread made.
    unsafe { window.setReleasedWhenClosed(false) };
    window.setTitle(&NSString::from_str("Typing check"));
    let text = NSTextView::initWithFrame(mtm.alloc(), NSRect::new(NSPoint::ZERO, frame.size));
    text.setRichText(false);
    // Spelling correction and text replacement would change the words after they arrive.
    text.setAutomaticSpellingCorrectionEnabled(false);
    text.setAutomaticTextReplacementEnabled(false);
    text.setAutomaticQuoteSubstitutionEnabled(false);
    window.setContentView(Some(&text));
    window.makeKeyAndOrderFront(None);
    window.makeFirstResponder(Some(&text));
    app::activate(mtm);

    let settings = Settings {
        toggle: Some(core_typing::Chord {
            code: SPACE,
            control: true,
            ..core_typing::Chord::default()
        }),
        ..Settings::default()
    };
    let keyboard = match typing::Keyboard::start(settings, true, None, |_| {}) {
        Ok(keyboard) => keyboard,
        Err(e) => {
            eprintln!("typing_check: no keyboard tap ({e:?})");
            process::exit(2);
        }
    };
    let run = Rc::new(RefCell::new(Run {
        mtm,
        window,
        text,
        keyboard,
        passed: 0,
    }));
    // Time for the window to come to the front and for the focus to be asked about once.
    app::after(1.0, move || step(run, 0));
    app.run();
}
