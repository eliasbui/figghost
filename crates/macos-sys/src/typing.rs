//! The keyboard side of Vietnamese typing. A tap where the hardware's keys enter the system hands each key press to a
//! `core_typing::Session`; when the session wants a word typed again with its marks, the key is held back while the
//! deletions and the new text are posted to the session, so they reach the app before the next key can. Needs
//! Accessibility, as every tap does.
//!
//! How the text is posted depends on the app and on the element that has the focus (see `core_typing::plan`). Asking
//! another app what has its focus is a message that a busy app answers late, and a tap that waits is switched off by
//! the system, so a thread of its own asks and the tap reads the last answer.

use crate::{
    ax::Element,
    event_tap::{self, EventTap, EventView, TapError, Verdict, field},
    synth::SELF_TAG,
    workspace,
};
use core_typing::{Action, Delivery, Event, Focus, Key, Plan, Session, Settings, Then, key};
use objc2_app_kit::NSWorkspace;
use std::{
    ffi::c_void,
    sync::{
        Arc, Mutex, PoisonError,
        atomic::{AtomicBool, Ordering},
        mpsc,
    },
    thread,
    time::{Duration, Instant},
};

const LEFT_MOUSE_DOWN: u32 = 1;
const RIGHT_MOUSE_DOWN: u32 = 3;
const KEY_DOWN: u32 = 10;
const FLAGS_CHANGED: u32 = 12;
const OTHER_MOUSE_DOWN: u32 = 25;
/// `kCGKeyboardEventAutorepeat`.
const AUTOREPEAT_FIELD: u32 = 8;
/// `kCGKeyboardEventKeycode`.
const KEYCODE_FIELD: u32 = 9;

const FLAG_CAPS_LOCK: u64 = 0x0001_0000;
const FLAG_SHIFT: u64 = 0x0002_0000;
const FLAG_CONTROL: u64 = 0x0004_0000;
const FLAG_OPTION: u64 = 0x0008_0000;
const FLAG_COMMAND: u64 = 0x0010_0000;

const SESSION_EVENT_TAP: u32 = 1;
/// `kCGEventSourceStatePrivate`: the posted keys carry only the modifiers set on them, never those the person holds.
const SOURCE_STATE_PRIVATE: i32 = -1;
/// The most UTF-16 units one key event carries.
const MAX_CHUNK: usize = 20;
/// A narrow no-break space, typed and deleted again to dismiss the suggestion of an address bar.
const DISMISS: &str = "\u{202F}";

/// How old the answer about the focus may be before the next key asks again.
const FOCUS_FRESH: Duration = Duration::from_millis(250);
/// After a key that moves the focus, the focus is asked about again this much later, when it has arrived.
const FOCUS_SETTLE: Duration = Duration::from_millis(150);
/// The longest one question about the focus may take once the focused element is known.
const FOCUS_TIMEOUT_SECONDS: f32 = 0.25;

type CGEventRef = *mut c_void;

#[link(name = "CoreGraphics", kind = "framework")]
unsafe extern "C" {
    fn CGEventSourceCreate(state: i32) -> *mut c_void;
    fn CGEventCreateKeyboardEvent(source: *mut c_void, key: u16, down: bool) -> CGEventRef;
    fn CGEventKeyboardSetUnicodeString(event: CGEventRef, length: usize, text: *const u16);
    fn CGEventSetFlags(event: CGEventRef, flags: u64);
    fn CGEventSetIntegerValueField(event: CGEventRef, field: u32, value: i64);
    fn CGEventPost(tap: u32, event: CGEventRef);
}

#[link(name = "CoreFoundation", kind = "framework")]
unsafe extern "C" {
    fn CFRelease(cf: *const c_void);
}

/// The key press an event of the tap is, by the flags it carries.
pub fn key_of(code: u16, flags: u64) -> Key {
    Key {
        code,
        shift: flags & FLAG_SHIFT != 0,
        caps_lock: flags & FLAG_CAPS_LOCK != 0,
        control: flags & FLAG_CONTROL != 0,
        option: flags & FLAG_OPTION != 0,
        command: flags & FLAG_COMMAND != 0,
    }
}

/// What the text is posted as: lines of at most `size` UTF-16 units per event, with a Return between lines. A
/// character is never cut in two.
#[derive(Debug, PartialEq, Eq)]
enum Piece {
    Text(Vec<u16>),
    Return,
}

fn pieces(text: &str, size: usize) -> Vec<Piece> {
    let mut out = Vec::new();
    for (i, line) in text.split('\n').enumerate() {
        if i > 0 {
            out.push(Piece::Return);
        }
        let mut chunk: Vec<u16> = Vec::new();
        for c in line.chars() {
            let mut units = [0; 2];
            let units = c.encode_utf16(&mut units);
            if !chunk.is_empty() && chunk.len() + units.len() > size {
                out.push(Piece::Text(std::mem::take(&mut chunk)));
            }
            chunk.extend_from_slice(units);
        }
        if !chunk.is_empty() {
            out.push(Piece::Text(chunk));
        }
    }
    out
}

fn pause(micros: u32) {
    if micros > 0 {
        thread::sleep(Duration::from_micros(u64::from(micros)));
    }
}

/// Posts key events that carry only what is set on them, each stamped as this app's own.
struct Poster(*mut c_void);

// SAFETY: an event source is a CoreFoundation object; it is used by one thread at a time, the tap's.
unsafe impl Send for Poster {}

impl Drop for Poster {
    fn drop(&mut self) {
        if !self.0.is_null() {
            // SAFETY: the source came from CGEventSourceCreate, which returns a +1 reference.
            unsafe { CFRelease(self.0) };
        }
    }
}

impl Poster {
    fn new() -> Self {
        // SAFETY: creates a new source or returns null, which the event functions accept as "no source".
        Self(unsafe { CGEventSourceCreate(SOURCE_STATE_PRIVATE) })
    }

    fn post(&self, code: u16, flags: u64, text: Option<&[u16]>) {
        for down in [true, false] {
            // SAFETY: the event is created, configured, posted and released within this block; `text` outlives it.
            unsafe {
                let event = CGEventCreateKeyboardEvent(self.0, code, down);
                if event.is_null() {
                    return;
                }
                CGEventSetFlags(event, flags);
                if let Some(units) = text {
                    CGEventKeyboardSetUnicodeString(event, units.len(), units.as_ptr());
                }
                CGEventSetIntegerValueField(event, field::SOURCE_USER_DATA, SELF_TAG);
                CGEventPost(SESSION_EVENT_TAP, event);
                CFRelease(event);
            }
        }
    }

    fn key(&self, code: u16, flags: u64) {
        self.post(code, flags, None);
    }

    fn text(&self, text: &str, size: usize, gap: u32) {
        for piece in pieces(text, size) {
            match piece {
                Piece::Text(units) => self.post(0, 0, Some(&units)),
                Piece::Return => self.key(key::RETURN, 0),
            }
            pause(gap);
        }
    }

    /// Deletes `delete` characters before the cursor and types `text` the way `plan` says.
    fn replace(&self, plan: &Plan, mut delete: usize, text: &str) {
        let d = plan.delays;
        let size = if plan.delivery == Delivery::CharByChar {
            1
        } else {
            MAX_CHUNK
        };
        match plan.delivery {
            Delivery::Passthrough => return,
            Delivery::Selection if !text.is_empty() => {
                for _ in 0..delete {
                    self.key(key::LEFT, FLAG_SHIFT);
                    pause(d.delete);
                }
                if delete > 0 {
                    pause(d.after_delete);
                }
                self.text(text, size, d.text);
                pause(d.settle);
                return;
            }
            Delivery::EmptyCharPrefix => {
                self.text(DISMISS, size, 0);
                pause(d.delete.max(1_000));
                delete += 1;
            }
            Delivery::ClearSuggestion => {
                self.key(key::FORWARD_DELETE, 0);
                pause(3_000);
            }
            Delivery::Keys | Delivery::CharByChar | Delivery::Selection => {}
        }
        for _ in 0..delete {
            self.key(key::DELETE, 0);
            pause(d.delete);
        }
        if delete > 0 {
            pause(d.after_delete);
        }
        self.text(text, size, d.text);
        pause(d.settle);
    }
}

/// The last answer about the focus.
#[derive(Debug, Default)]
struct Seen {
    app: Option<String>,
    role: Option<String>,
    at: Option<Instant>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Ask {
    Now,
    /// Now and again a little later: the focus is on its way somewhere.
    Settle,
}

fn read_focus() -> Seen {
    let at = Some(Instant::now());
    let Some(focused) = Element::system_wide().and_then(|s| s.attribute("AXFocusedUIElement"))
    else {
        return Seen {
            at,
            ..Seen::default()
        };
    };
    focused.set_timeout(FOCUS_TIMEOUT_SECONDS);
    Seen {
        app: focused
            .pid()
            .and_then(workspace::running_app)
            .map(|a| a.bundle_id),
        role: focused.string("AXRole"),
        at,
    }
}

/// Asks about the focus whenever it is told to, and leaves the answer in `seen`. It ends when every sender is gone,
/// and is never waited for: a question to a hung app may take seconds.
fn watch_focus(seen: Arc<Mutex<Seen>>) -> mpsc::Sender<Ask> {
    let (tx, rx) = mpsc::channel::<Ask>();
    let spawned = thread::Builder::new()
        .name("fig-typing-focus".into())
        .spawn(move || {
            while let Ok(mut ask) = rx.recv() {
                while let Ok(more) = rx.try_recv() {
                    if more == Ask::Settle {
                        ask = Ask::Settle;
                    }
                }
                let publish = |found: Seen| {
                    *seen.lock().unwrap_or_else(PoisonError::into_inner) = found;
                };
                objc2::rc::autoreleasepool(|_| publish(read_focus()));
                if ask == Ask::Settle {
                    thread::sleep(FOCUS_SETTLE);
                    objc2::rc::autoreleasepool(|_| publish(read_focus()));
                }
            }
        });
    if let Err(e) = spawned {
        eprintln!("typing: cannot watch the focus ({e}); every app gets the usual pauses");
    }
    tx
}

/// The name of another Vietnamese input method that is running, whatever kind of app it is (most live in the menu
/// bar). With two at work every mark is put twice.
pub fn other_input_method() -> Option<String> {
    let ours = std::process::id() as i32;
    NSWorkspace::sharedWorkspace()
        .runningApplications()
        .iter()
        .filter(|app| app.processIdentifier() != ours)
        .find_map(|app| {
            let name = app
                .localizedName()
                .map(|n| n.to_string())
                .unwrap_or_default();
            let id = app
                .bundleIdentifier()
                .map(|n| n.to_string())
                .unwrap_or_default();
            (core_typing::is_other_input_method(&name) || core_typing::is_other_input_method(&id))
                .then_some(if name.is_empty() { id } else { name })
        })
}

/// What the main thread and the tap share.
struct Shared {
    vietnamese: AtomicBool,
    paused: AtomicBool,
    /// The focus went to another app: the word being typed is over.
    forget: AtomicBool,
    /// The settings and how many times they changed, so the tap notices a change at its next key.
    settings: Mutex<(u64, Settings)>,
    /// The app in front, for when the focused element cannot be asked.
    frontmost: Mutex<Option<String>>,
}

/// Vietnamese typing on the keyboard, for as long as this lives.
pub struct Keyboard {
    _tap: EventTap,
    shared: Arc<Shared>,
    seen: Arc<Mutex<Seen>>,
    ask: mpsc::Sender<Ask>,
}

impl Keyboard {
    /// Starts the tap. `on_toggle` is told the new language each time the shortcut switches it; it runs inside the
    /// input path, on the tap's thread, so it must hand anything slow to another thread.
    pub fn start(
        settings: Settings,
        vietnamese: bool,
        frontmost: Option<String>,
        on_toggle: impl Fn(bool) + Send + 'static,
    ) -> Result<Self, TapError> {
        let shared = Arc::new(Shared {
            vietnamese: AtomicBool::new(vietnamese),
            paused: AtomicBool::new(false),
            forget: AtomicBool::new(false),
            settings: Mutex::new((0, settings.clone())),
            frontmost: Mutex::new(frontmost),
        });
        let seen = Arc::new(Mutex::new(Seen::default()));
        let ask = watch_focus(Arc::clone(&seen));
        let _ = ask.send(Ask::Now);
        let mask = event_tap::mask(KEY_DOWN)
            | event_tap::mask(FLAGS_CHANGED)
            | event_tap::mask(LEFT_MOUSE_DOWN)
            | event_tap::mask(RIGHT_MOUSE_DOWN)
            | event_tap::mask(OTHER_MOUSE_DOWN);
        let mut tap = Tap {
            shared: Arc::clone(&shared),
            seen: Arc::clone(&seen),
            ask: ask.clone(),
            session: Session::new(settings),
            generation: 0,
            poster: Poster::new(),
            control: false,
            asked: None,
            on_toggle: Box::new(on_toggle),
        };
        let tap = EventTap::spawn_hid(mask, move |kind, event| tap.handle(kind, event))?;
        Ok(Self {
            _tap: tap,
            shared,
            seen,
            ask,
        })
    }

    pub fn configure(&self, settings: Settings) {
        let mut slot = self
            .shared
            .settings
            .lock()
            .unwrap_or_else(PoisonError::into_inner);
        slot.0 += 1;
        slot.1 = settings;
    }

    pub fn is_vietnamese(&self) -> bool {
        self.shared.vietnamese.load(Ordering::Acquire)
    }

    pub fn set_vietnamese(&self, on: bool) {
        self.shared.vietnamese.store(on, Ordering::Release);
        self.shared.forget.store(true, Ordering::Release);
    }

    /// While paused every key goes on as it is (another input method is running, or a shortcut is being recorded).
    pub fn set_paused(&self, paused: bool) {
        self.shared.paused.store(paused, Ordering::Release);
        self.shared.forget.store(true, Ordering::Release);
    }

    /// Another app came to the front.
    pub fn app_changed(&self, frontmost: Option<String>) {
        *self
            .shared
            .frontmost
            .lock()
            .unwrap_or_else(PoisonError::into_inner) = frontmost;
        // What was known of the focus belongs to the other app: the one in front stands for it until the next answer.
        *self.seen.lock().unwrap_or_else(PoisonError::into_inner) = Seen::default();
        self.shared.forget.store(true, Ordering::Release);
        let _ = self.ask.send(Ask::Settle);
    }
}

/// The tap's own state, on the tap's thread.
struct Tap {
    shared: Arc<Shared>,
    seen: Arc<Mutex<Seen>>,
    ask: mpsc::Sender<Ask>,
    session: Session,
    generation: u64,
    poster: Poster,
    /// Whether Control was down at the last change of the modifiers.
    control: bool,
    /// When the focus was last asked about from here.
    asked: Option<Instant>,
    on_toggle: Box<dyn Fn(bool) + Send>,
}

impl Tap {
    fn handle(&mut self, kind: u32, view: &EventView<'_>) -> Verdict {
        if view.int(field::SOURCE_USER_DATA) == SELF_TAG {
            return Verdict::Pass;
        }
        let event = match kind {
            KEY_DOWN => Event::Key(key_of(view.int(KEYCODE_FIELD) as u16, view.flags())),
            FLAGS_CHANGED => {
                let control = view.flags() & FLAG_CONTROL != 0;
                let pressed = control && !self.control;
                self.control = control;
                if !pressed {
                    return Verdict::Pass;
                }
                Event::ControlDown
            }
            LEFT_MOUSE_DOWN | RIGHT_MOUSE_DOWN | OTHER_MOUSE_DOWN => Event::Click,
            _ => return Verdict::Pass,
        };
        self.catch_up();
        if self.shared.paused.load(Ordering::Acquire) {
            self.session.forget();
            return Verdict::Pass;
        }
        self.ask_if_needed(&event);
        let vietnamese = self.shared.vietnamese.load(Ordering::Acquire);
        let action = {
            let seen = self.seen.lock().unwrap_or_else(PoisonError::into_inner);
            let frontmost = self
                .shared
                .frontmost
                .lock()
                .unwrap_or_else(PoisonError::into_inner);
            let focus = Focus {
                app: seen.app.as_deref().or(frontmost.as_deref()),
                role: seen.role.as_deref(),
            };
            self.session.handle(event, focus, vietnamese)
        };
        match action {
            Action::Pass => Verdict::Pass,
            // A held shortcut repeats; only its first press switches.
            Action::Toggle if view.int(AUTOREPEAT_FIELD) != 0 => Verdict::Drop,
            Action::Toggle => {
                let now = !vietnamese;
                self.shared.vietnamese.store(now, Ordering::Release);
                (self.on_toggle)(now);
                Verdict::Drop
            }
            Action::Type {
                plan,
                delete,
                text,
                then,
            } => {
                self.poster.replace(&plan, delete, &text);
                match (then, event) {
                    (Then::Pass, _) => Verdict::Pass,
                    (Then::Retype, Event::Key(key)) => {
                        self.poster
                            .key(key.code, if key.shift { FLAG_SHIFT } else { 0 });
                        Verdict::Drop
                    }
                    _ => Verdict::Drop,
                }
            }
        }
    }

    /// Takes in what the main thread changed since the last event.
    fn catch_up(&mut self) {
        if self.shared.forget.swap(false, Ordering::AcqRel) {
            self.session.forget();
        }
        let slot = self
            .shared
            .settings
            .lock()
            .unwrap_or_else(PoisonError::into_inner);
        if slot.0 != self.generation {
            self.generation = slot.0;
            self.session.configure(slot.1.clone());
        }
    }

    /// Keeps the answer about the focus fresh while keys come, and asks twice after a key that moves the focus.
    fn ask_if_needed(&mut self, event: &Event) {
        let moves_focus = match event {
            Event::Click => true,
            Event::Key(key) => {
                key.command
                    || key.control
                    || matches!(key.code, key::TAB | key::RETURN | key::ENTER | key::ESCAPE)
            }
            Event::ControlDown => false,
        };
        let now = Instant::now();
        if moves_focus {
            self.asked = Some(now);
            let _ = self.ask.send(Ask::Settle);
            return;
        }
        let stale = self
            .seen
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .at
            .is_none_or(|at| now.duration_since(at) > FOCUS_FRESH);
        let recently = self
            .asked
            .is_some_and(|at| now.duration_since(at) < FOCUS_FRESH / 2);
        if stale && !recently {
            self.asked = Some(now);
            let _ = self.ask.send(Ask::Now);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_flags_of_an_event_make_the_key() {
        let key = key_of(0, FLAG_SHIFT | FLAG_CAPS_LOCK);
        assert!(key.shift && key.caps_lock && !key.control && !key.option && !key.command);
        let key = key_of(9, FLAG_COMMAND | FLAG_OPTION | FLAG_CONTROL);
        assert_eq!(key.code, 9);
        assert!(key.command && key.option && key.control && !key.shift);
        // The numeric pad and Fn flags are not modifiers of a key.
        assert_eq!(key_of(1, 0x0020_0000 | 0x0080_0000), key_of(1, 0));
    }

    #[test]
    fn text_is_cut_into_events_without_cutting_a_character() {
        let units = |s: &str| Piece::Text(s.encode_utf16().collect());
        assert_eq!(pieces("việt", MAX_CHUNK), vec![units("việt")]);
        assert_eq!(pieces("abc", 1), vec![units("a"), units("b"), units("c")]);
        let long = "ệ".repeat(25);
        assert_eq!(
            pieces(&long, MAX_CHUNK),
            vec![units(&"ệ".repeat(20)), units(&"ệ".repeat(5))]
        );
        // A character outside the first plane takes two units, which stay together.
        let wide = format!("{}😀", "a".repeat(19));
        assert_eq!(
            pieces(&wide, MAX_CHUNK),
            vec![units(&"a".repeat(19)), units("😀")]
        );
        assert_eq!(pieces("😀", 1), vec![units("😀")], "even one at a time");
        assert_eq!(
            pieces("a\nb\n", MAX_CHUNK),
            vec![units("a"), Piece::Return, units("b"), Piece::Return],
            "a line break is the Return key"
        );
        assert!(pieces("", MAX_CHUNK).is_empty());
    }

    #[test]
    fn the_keyboard_can_be_started_and_dropped() {
        // Without Accessibility access macOS refuses the tap and there is nothing to drop. English, so that a
        // person typing meanwhile is not changed.
        if let Ok(keyboard) = Keyboard::start(Settings::default(), false, None, |_| {}) {
            keyboard.configure(Settings::default());
            keyboard.app_changed(Some("com.apple.TextEdit".into()));
            drop(keyboard);
        }
    }
}
