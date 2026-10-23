//! Checks snippets end to end, with Vietnamese typing on (Telex) and off: opens a window of its own with a text view,
//! starts the keyboard of `macos_sys::typing` with a few snippets, presses the keys of an abbreviation where the
//! hardware's keys enter the system, a key every 35 ms (`SNIPPETS_CHECK_GAP_MS`), and reads back what the text view got.
//! The text view has an Edit menu, so that the Command+V of a pasted snippet reaches it as it would reach any app. A
//! pasted snippet borrows the clipboard: the check reads the clipboard before (as text, without changing it) and
//! again after, and fails if it is not what it was. A program started from a terminal may not take the focus by
//! itself, so the window floats above the others and the check clicks into it once the window server says the click
//! lands on it, then puts the pointer back. Each key is posted only while Accessibility says the focus is in this
//! process, and the check stops at once otherwise, so no key lands in another app; it refuses to run while another
//! Vietnamese input method runs. `SNIPPETS_CHECK_FOCUS_ONLY=1` tries the focus alone and posts no key. The terminal
//! needs Accessibility access; leave the machine alone for the half minute it takes.
//! `cargo run -p macos-sys --example snippets_check`

use core_typing::{Settings, Snippet};
use macos_sys::{MainThreadMarker, app, ax, pasteboard::Pasteboard, permissions, typing};
use objc2::{rc::Retained, sel};
use objc2_app_kit::{
    NSApplication, NSApplicationActivationPolicy, NSBackingStoreType, NSFloatingWindowLevel,
    NSMenu, NSMenuItem, NSScreen, NSTextView, NSWindow, NSWindowStyleMask,
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
const DELETE: u16 = 51;

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
    match c {
        ' ' => return (SPACE, false),
        '\u{8}' => return (DELETE, false),
        _ => {}
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

/// The pause between two key presses, as a person types: `SNIPPETS_CHECK_GAP_MS`, 35 by default. Keys posted all at
/// once are not typing: the word typed again for one key could overtake a key let through just before it.
fn gap() -> Duration {
    let ms = std::env::var("SNIPPETS_CHECK_GAP_MS")
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

/// Keys typed, and the text the view should hold after them.
struct Case {
    /// Control+Space first: switches between Vietnamese and English.
    toggle_first: bool,
    keys: &'static str,
    expected: &'static str,
    /// The expansion is pasted, so the clipboard is borrowed and must come back.
    pasted: bool,
}

const fn case(
    toggle_first: bool,
    keys: &'static str,
    expected: &'static str,
    pasted: bool,
) -> Case {
    Case {
        toggle_first,
        keys,
        expected,
        pasted,
    }
}

/// The keyboard starts in Vietnamese, so the first cases are in Telex; one Control+Space then makes it English.
const CASES: &[Case] = &[
    // `;dd` shows as `;đ` while it is typed: the expansion must delete two characters, not three, and leave what
    // stood before it.
    case(false, "ba ;dd", "ba Do not disturb", false),
    case(false, "ba;dd", "baDo not disturb", false),
    // Keys that make marks inside a longer abbreviation.
    case(false, "Vieetj ;addr", "Việt 12 Le Loi", false),
    // A text of several lines is pasted, with no Return pressed.
    case(false, ";sig", "Best regards,\nAn", true),
    // Backspace takes the last key back.
    case(false, ";sg\u{8}ig", "Best regards,\nAn", true),
    // Vietnamese typing goes on after an expansion.
    case(false, ";dd Vieetj ", "Do not disturb Việt ", false),
    // The same in English.
    case(true, "ba ;dd", "ba Do not disturb", false),
    case(false, ";addr", "12 Le Loi", false),
    case(false, ";sg\u{8}ig", "Best regards,\nAn", true),
    case(false, "text ;sig text", "text Best regards,\nAn text", true),
    // Not an abbreviation: nothing is replaced.
    case(false, ";si g", ";si g", false),
    case(false, ";sg", ";sg", false),
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
        "snippets_check: stopped, the window of the check does not have the keyboard (nothing was posted): active {}, \
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
            "snippets_check: stopped, another window covers the window of the check (nothing was posted)"
        );
        process::exit(2);
    };
    println!("snippets_check: clicking into its window to get the keyboard");
    click(at);
    app::after(0.6, move || with_keyboard(run, tries - 1, next));
}

fn step(run: Rc<RefCell<Run>>, index: usize, clipboard: Option<String>) {
    if index == CASES.len() {
        let passed = run.borrow().passed;
        println!(
            "snippets_check: {passed} of {} matched, a key every {} ms",
            CASES.len(),
            gap().as_millis()
        );
        process::exit(if passed == CASES.len() { 0 } else { 1 });
    }
    if !has_keyboard(&run.borrow()) {
        with_keyboard(run, 3, move |run| step(run, index, clipboard));
        return;
    }
    if std::env::var_os("SNIPPETS_CHECK_FOCUS_ONLY").is_some() {
        println!(
            "snippets_check: the window has the keyboard; no key was posted (SNIPPETS_CHECK_FOCUS_ONLY)"
        );
        process::exit(0);
    }
    let case = &CASES[index];
    run.borrow().text.setString(&NSString::from_str(""));
    // Every case starts a new word in a new place, as a click would.
    run.borrow().keyboard.app_changed(None);
    let gap = gap();
    type_keys(case.toggle_first, case.keys, gap);
    let presses = case.keys.chars().count() + usize::from(case.toggle_first);
    // A paste is put back on the clipboard half a second after, and the check looks after that.
    let settle = if case.pasted { 1.6 } else { 0.8 };
    app::after(gap.as_secs_f64() * presses as f64 + settle, move || {
        if FOCUS_LOST.load(Ordering::Acquire) {
            eprintln!(
                "snippets_check: stopped, the focus left the window of the check during {:?}; the keys after that were not posted",
                case.keys
            );
            process::exit(2);
        }
        let got = run.borrow().text.string().to_string();
        let kept = Pasteboard::general().string() == clipboard;
        let matched = got == case.expected && kept;
        println!(
            "{} {:?} -> {got:?} (wanted {:?}, typing in {}, clipboard {})",
            if matched { "ok      " } else { "MISMATCH" },
            case.keys,
            case.expected,
            if run.borrow().keyboard.is_vietnamese() {
                "Vietnamese"
            } else {
                "English"
            },
            if kept { "as it was" } else { "CHANGED" },
        );
        if matched {
            run.borrow_mut().passed += 1;
        }
        step(run, index + 1, clipboard);
    });
}

/// A menu bar with Edit and Paste, which is what a text view needs to take Command+V.
fn install_paste_menu(mtm: MainThreadMarker, app: &NSApplication) {
    let bar = NSMenu::new(mtm);
    let top = NSMenuItem::new(mtm);
    let edit = NSMenu::initWithTitle(mtm.alloc(), &NSString::from_str("Edit"));
    // SAFETY: `paste:` is a standard action that the text view answers; the item is made on the main thread.
    let paste = unsafe {
        NSMenuItem::initWithTitle_action_keyEquivalent(
            mtm.alloc(),
            &NSString::from_str("Paste"),
            Some(sel!(paste:)),
            &NSString::from_str("v"),
        )
    };
    edit.addItem(&paste);
    top.setSubmenu(Some(&edit));
    bar.addItem(&top);
    app.setMainMenu(Some(&bar));
}

fn main() {
    let mtm = MainThreadMarker::new().expect("main thread");
    if !permissions::accessibility_trusted(false) {
        eprintln!("snippets_check: skipped, the terminal needs Accessibility access");
        process::exit(2);
    }
    // Only the focus is tried then, and no key is posted.
    let focus_only = std::env::var_os("SNIPPETS_CHECK_FOCUS_ONLY").is_some();
    if let Some(name) = typing::other_input_method().filter(|_| !focus_only) {
        eprintln!(
            "snippets_check: skipped, {name} is running and would put every mark a second time"
        );
        process::exit(2);
    }
    let app = NSApplication::sharedApplication(mtm);
    app.setActivationPolicy(NSApplicationActivationPolicy::Regular);
    install_paste_menu(mtm, &app);
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
    window.setTitle(&NSString::from_str("Snippets check"));
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

    let snippet = |abbr: &str, text: &str| Snippet {
        abbr: abbr.into(),
        text: text.into(),
    };
    let settings = Settings {
        toggle: Some(core_typing::Chord {
            code: SPACE,
            control: true,
            ..core_typing::Chord::default()
        }),
        snippets: vec![
            snippet(";dd", "Do not disturb"),
            snippet(";addr", "12 Le Loi"),
            snippet(";sig", "Best regards,\nAn"),
        ],
        ..Settings::default()
    };
    let keyboard = match typing::Keyboard::start(settings, true, None, |_| {}, |_, _| {}) {
        Ok(keyboard) => keyboard,
        Err(e) => {
            eprintln!("snippets_check: no keyboard tap ({e:?})");
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
    // Whatever the clipboard holds now is what it must hold after every case.
    let clipboard = Pasteboard::general().string();
    // Time for the window to come to the front; `step` clicks into it if it does not have the keyboard by then.
    app::after(1.0, move || step(run, 0, clipboard));
    app.run();
}
