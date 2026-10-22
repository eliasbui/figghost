//! Checks Vietnamese typing end to end: opens a window of its own with a text view, starts the keyboard of
//! `macos_sys::typing` in Vietnamese, presses the keys of a few words where the hardware's keys enter the system, a key
//! every 35 ms (`TYPING_CHECK_GAP_MS`), and reads back what the text view got. A program started from a terminal may
//! not take the focus by itself, so the window floats above the others and the check clicks into it once the window
//! server says the click lands on it, then puts the pointer back. Each key is posted only while Accessibility says the
//! focus is in this process, and the check stops at once otherwise, so no key lands in another app; it refuses to run
//! while another Vietnamese input method runs. `TYPING_CHECK_FOCUS_ONLY=1` tries the focus alone and posts no key. The
//! terminal needs Accessibility access; leave the machine alone for the ten seconds it takes.
//! `cargo run -p macos-sys --example typing_check`

use core_typing::Settings;
use macos_sys::{MainThreadMarker, app, ax, permissions, typing};
use objc2::rc::Retained;
use objc2_app_kit::{
    NSApplication, NSApplicationActivationPolicy, NSBackingStoreType, NSFloatingWindowLevel,
    NSScreen, NSTextView, NSWindow, NSWindowStyleMask,
};
use objc2_foundation::{NSPoint, NSRect, NSSize, NSString};
use std::{
    cell::RefCell,
    ffi::c_void,
    process, ptr,
    rc::Rc,
    sync::atomic::{AtomicBool, Ordering},
    thread,
    time::Duration,
};

const HID_EVENT_TAP: u32 = 0;
const LEFT_MOUSE_DOWN: u32 = 1;
const LEFT_MOUSE_UP: u32 = 2;
const FLAG_SHIFT: u64 = 0x0002_0000;
const FLAG_CONTROL: u64 = 0x0004_0000;
const SPACE: u16 = 49;

#[repr(C)]
#[derive(Clone, Copy)]
struct CGPoint {
    x: f64,
    y: f64,
}

#[link(name = "CoreGraphics", kind = "framework")]
unsafe extern "C" {
    fn CGEventCreate(source: *mut c_void) -> *mut c_void;
    fn CGEventGetLocation(event: *mut c_void) -> CGPoint;
    fn CGEventCreateMouseEvent(
        source: *mut c_void,
        kind: u32,
        at: CGPoint,
        button: u32,
    ) -> *mut c_void;
    fn CGWarpMouseCursorPosition(at: CGPoint) -> i32;
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

/// Clicks at `at` (top-left origin, as Accessibility measures) and puts the pointer back where it was.
fn click(at: CGPoint) {
    // SAFETY: every event is created, posted and released here; the location event only reports where the pointer is.
    unsafe {
        let here = CGEventCreate(ptr::null_mut());
        let back = (!here.is_null()).then(|| CGEventGetLocation(here));
        if !here.is_null() {
            CFRelease(here);
        }
        for kind in [LEFT_MOUSE_DOWN, LEFT_MOUSE_UP] {
            let event = CGEventCreateMouseEvent(ptr::null_mut(), kind, at, 0);
            assert!(!event.is_null(), "no mouse event");
            CGEventPost(HID_EVENT_TAP, event);
            CFRelease(event);
        }
        if let Some(back) = back {
            CGWarpMouseCursorPosition(back);
        }
    }
}

/// The pause between two key presses, as a person types: `TYPING_CHECK_GAP_MS`, 35 by default. Keys posted all at
/// once are not typing: the word typed again for one key could overtake a key let through just before it.
fn gap() -> Duration {
    let ms = std::env::var("TYPING_CHECK_GAP_MS")
        .ok()
        .and_then(|v| v.parse::<u64>().ok())
        .unwrap_or(35);
    Duration::from_millis(ms)
}

/// Set as soon as a key was not posted because the focus had left the window of the check.
static FOCUS_LOST: AtomicBool = AtomicBool::new(false);

/// Presses the keys on a thread of its own, one every `gap`, so the main thread is free to take them in. Before every
/// key it asks Accessibility where the focus is (from this thread the main thread can answer), and posts nothing more
/// once it has left this process: a key must never land in another app.
fn type_keys(toggle_first: bool, keys: &'static str, gap: Duration) {
    thread::spawn(move || {
        let ours = Some(process::id() as i32);
        let presses = toggle_first
            .then_some((SPACE, FLAG_CONTROL))
            .into_iter()
            .chain(keys.chars().map(|c| {
                let (code, shift) = key_of(c);
                (code, if shift { FLAG_SHIFT } else { 0 })
            }));
        for (code, flags) in presses {
            if ax::focused_pid() != ours {
                FOCUS_LOST.store(true, Ordering::Release);
                return;
            }
            post(code, flags);
            thread::sleep(gap);
        }
    });
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

/// The middle of the window in the coordinates of mouse events (top-left origin), when the window server says a click
/// there lands on this window and on nothing else.
fn own_point(run: &Run) -> Option<CGPoint> {
    let main = NSScreen::screens(run.mtm).firstObject()?.frame();
    let frame = run.window.frame();
    let middle = NSPoint::new(
        frame.origin.x + frame.size.width / 2.0,
        frame.origin.y + frame.size.height / 2.0,
    );
    let hit = NSWindow::windowNumberAtPoint_belowWindowWithWindowNumber(middle, 0, run.mtm);
    (hit == run.window.windowNumber()).then_some(CGPoint {
        x: middle.x,
        y: main.origin.y + main.size.height - middle.y,
    })
}

/// Whether this app is active with its window key: then the keys go to the text view. (Accessibility is not asked:
/// a question about this very process, from its main thread, waits for that thread and times out.)
fn has_keyboard(run: &Run) -> bool {
    NSApplication::sharedApplication(run.mtm).isActive() && run.window.isKeyWindow()
}

fn stop_without_keyboard(run: &Run) -> ! {
    eprintln!(
        "typing_check: stopped, the window of the check does not have the keyboard (nothing was posted): active {}, \
         key window {}",
        NSApplication::sharedApplication(run.mtm).isActive(),
        run.window.isKeyWindow()
    );
    process::exit(2);
}

/// Runs `next` once the window has the keyboard, clicking into it (where nothing covers it) up to `tries` times.
fn with_keyboard(run: Rc<RefCell<Run>>, tries: u32, next: impl FnOnce(Rc<RefCell<Run>>) + 'static) {
    if has_keyboard(&run.borrow()) {
        next(run);
        return;
    }
    if tries == 0 {
        stop_without_keyboard(&run.borrow());
    }
    let Some(at) = own_point(&run.borrow()) else {
        eprintln!(
            "typing_check: stopped, another window covers the window of the check (nothing was posted)"
        );
        process::exit(2);
    };
    println!("typing_check: clicking into its window to get the keyboard");
    click(at);
    app::after(0.6, move || with_keyboard(run, tries - 1, next));
}

fn step(run: Rc<RefCell<Run>>, index: usize) {
    if index == CASES.len() {
        let passed = run.borrow().passed;
        println!(
            "typing_check: {passed} of {} matched, a key every {} ms",
            CASES.len(),
            gap().as_millis()
        );
        process::exit(if passed == CASES.len() { 0 } else { 1 });
    }
    if !has_keyboard(&run.borrow()) {
        with_keyboard(run, 3, move |run| step(run, index));
        return;
    }
    if std::env::var_os("TYPING_CHECK_FOCUS_ONLY").is_some() {
        println!(
            "typing_check: the window has the keyboard; no key was posted (TYPING_CHECK_FOCUS_ONLY)"
        );
        process::exit(0);
    }
    let case = &CASES[index];
    run.borrow().text.setString(&NSString::from_str(""));
    let gap = gap();
    type_keys(case.toggle_first, case.keys, gap);
    let presses = case.keys.chars().count() + usize::from(case.toggle_first);
    app::after(gap.as_secs_f64() * presses as f64 + 0.8, move || {
        if FOCUS_LOST.load(Ordering::Acquire) {
            eprintln!(
                "typing_check: stopped, the focus left the window of the check during {:?}; the keys after that were not posted",
                case.keys
            );
            process::exit(2);
        }
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
    // Only the focus is tried then, and no key is posted.
    let focus_only = std::env::var_os("TYPING_CHECK_FOCUS_ONLY").is_some();
    if let Some(name) = typing::other_input_method().filter(|_| !focus_only) {
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
    window.setLevel(NSFloatingWindowLevel);
    window.center();
    window.makeKeyAndOrderFront(None);
    window.orderFrontRegardless();
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
    // Time for the window to come to the front; `step` clicks into it if it does not have the keyboard by then.
    app::after(1.0, move || step(run, 0));
    app.run();
}
