//! What Copy Stack needs from the keyboard. A global monitor reports modifier changes, so a double press of
//! a bare modifier key can be recognised, and while a stack runs a tap on the paste shortcut lets the app put
//! the next item on the pasteboard just before the focused app reads it. Neither ever swallows a key.

use crate::{event_tap::field, synth::SELF_TAG};
use block2::RcBlock;
use core_foundation::{
    base::TCFType,
    mach_port::{CFMachPort, CFMachPortInvalidate, CFMachPortRef},
    runloop::{CFRunLoop, CFRunLoopSource, kCFRunLoopCommonModes},
};
use objc2::rc::Retained;
use objc2::runtime::AnyObject;
use objc2_app_kit::{NSEvent, NSEventMask, NSEventModifierFlags, NSEventType};
use std::{
    cell::Cell,
    ffi::c_void,
    panic::{AssertUnwindSafe, catch_unwind},
    ptr::{self, NonNull},
};

/// The four modifier keys a tap can be made of.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Flags {
    pub option: bool,
    pub command: bool,
    pub control: bool,
    pub shift: bool,
}

/// What a monitored event means for recognising a tap.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Observed {
    /// The set of held modifier keys changed.
    Modifiers(Flags),
    /// A key or a button went down.
    Other,
}

pub fn observe(event: &NSEvent) -> Observed {
    if event.r#type() != NSEventType::FlagsChanged {
        return Observed::Other;
    }
    let flags = event.modifierFlags();
    Observed::Modifiers(Flags {
        option: flags.contains(NSEventModifierFlags::Option),
        command: flags.contains(NSEventModifierFlags::Command),
        control: flags.contains(NSEventModifierFlags::Control),
        shift: flags.contains(NSEventModifierFlags::Shift),
    })
}

/// Reports modifier changes, and the key and button presses that spoil a tap, from every app. The system only
/// delivers them when this process has Accessibility access; without it the handler is simply never called.
pub struct ModifierWatch(Retained<AnyObject>);

impl ModifierWatch {
    /// `handler` gets each event with its time in seconds since the system started, on the main thread.
    pub fn start(handler: impl Fn(Observed, f64) + 'static) -> Option<Self> {
        let block = RcBlock::new(move |event: NonNull<NSEvent>| {
            // SAFETY: the monitor hands out a valid event for the duration of the call.
            let event = unsafe { event.as_ref() };
            handler(observe(event), event.timestamp());
        });
        NSEvent::addGlobalMonitorForEventsMatchingMask_handler(
            NSEventMask::FlagsChanged
                | NSEventMask::KeyDown
                | NSEventMask::LeftMouseDown
                | NSEventMask::RightMouseDown
                | NSEventMask::OtherMouseDown,
            &block,
        )
        .map(Self)
    }
}

impl Drop for ModifierWatch {
    fn drop(&mut self) {
        // SAFETY: the token came from addGlobalMonitorForEventsMatchingMask and is removed once.
        unsafe { NSEvent::removeMonitor(&self.0) };
    }
}

const SESSION_EVENT_TAP: u32 = 1;
const HEAD_INSERT: u32 = 0;
const OPTION_DEFAULT: u32 = 0;
const KEY_DOWN: u32 = 10;
const DISABLED_BY_TIMEOUT: u32 = 0xFFFF_FFFE;
const DISABLED_BY_USER_INPUT: u32 = 0xFFFF_FFFF;
const KEYBOARD_AUTOREPEAT: u32 = 8;
const KEYBOARD_KEYCODE: u32 = 9;
const KEY_V: i64 = 9;
const FLAG_SHIFT: u64 = 0x0002_0000;
const FLAG_CONTROL: u64 = 0x0004_0000;
const FLAG_OPTION: u64 = 0x0008_0000;
const FLAG_COMMAND: u64 = 0x0010_0000;

type CGEventRef = *mut c_void;
type TapCallback = unsafe extern "C" fn(*mut c_void, u32, CGEventRef, *mut c_void) -> CGEventRef;

#[link(name = "CoreGraphics", kind = "framework")]
unsafe extern "C" {
    fn CGEventTapCreate(
        tap: u32,
        place: u32,
        options: u32,
        events_of_interest: u64,
        callback: TapCallback,
        user_info: *mut c_void,
    ) -> CFMachPortRef;
    fn CGEventTapEnable(tap: CFMachPortRef, enable: bool);
    fn CGEventGetIntegerValueField(event: CGEventRef, field: u32) -> i64;
    fn CGEventGetFlags(event: CGEventRef) -> u64;
}

/// Whether a key press is the paste shortcut of a person: Command+V, not repeated by holding the key, no
/// other modifier (Command+Shift+V is not a paste everywhere), and not the one this app posts itself when it
/// pastes from the popup.
pub fn is_paste_press(key_code: i64, flags: u64, auto_repeat: bool, user_data: i64) -> bool {
    key_code == KEY_V
        && flags & FLAG_COMMAND != 0
        && flags & (FLAG_SHIFT | FLAG_CONTROL | FLAG_OPTION) == 0
        && !auto_repeat
        && user_data != SELF_TAG
}

struct Ctx {
    tap: Cell<CFMachPortRef>,
    on_paste: Box<dyn Fn()>,
}

/// A tap on the paste shortcut, on the main run loop so its handler may use the pasteboard and the app's
/// state directly. The handler runs before the key reaches the focused app; the key always goes on to it.
/// Never drop this from inside the handler: ask for the drop to happen afterwards.
pub struct PasteTap {
    port: CFMachPort,
    source: CFRunLoopSource,
    ctx: *mut Ctx,
}

impl PasteTap {
    /// `None` when macOS refuses the tap, which is what a missing Accessibility access looks like.
    pub fn start(on_paste: impl Fn() + 'static) -> Option<Self> {
        let ctx = Box::into_raw(Box::new(Ctx {
            tap: Cell::new(ptr::null_mut()),
            on_paste: Box::new(on_paste),
        }));
        // SAFETY: `ctx` stays allocated until `drop`, after the tap is gone.
        let port = unsafe {
            CGEventTapCreate(
                SESSION_EVENT_TAP,
                HEAD_INSERT,
                OPTION_DEFAULT,
                1 << KEY_DOWN,
                trampoline,
                ctx.cast(),
            )
        };
        if port.is_null() {
            // SAFETY: the tap was never created, so nothing else holds `ctx`.
            drop(unsafe { Box::from_raw(ctx) });
            return None;
        }
        // SAFETY: `ctx` is valid; the callback reads this pointer only to re-arm the tap.
        unsafe { (*ctx).tap.set(port) };
        // SAFETY: CGEventTapCreate follows the create rule, so we own one reference.
        let port = unsafe { CFMachPort::wrap_under_create_rule(port) };
        let Ok(source) = port.create_runloop_source(0) else {
            // SAFETY: the tap is not attached to any run loop yet, so it cannot call back.
            drop(unsafe { Box::from_raw(ctx) });
            return None;
        };
        // SAFETY: kCFRunLoopCommonModes is a process-lifetime constant.
        CFRunLoop::get_main().add_source(&source, unsafe { kCFRunLoopCommonModes });
        // SAFETY: `port` is a valid tap.
        unsafe { CGEventTapEnable(port.as_concrete_TypeRef(), true) };
        Some(Self { port, source, ctx })
    }
}

impl Drop for PasteTap {
    fn drop(&mut self) {
        // SAFETY: `port` is a valid tap; kCFRunLoopCommonModes is a process-lifetime constant.
        unsafe {
            CGEventTapEnable(self.port.as_concrete_TypeRef(), false);
            CFRunLoop::get_main().remove_source(&self.source, kCFRunLoopCommonModes);
            CFMachPortInvalidate(self.port.as_concrete_TypeRef());
        }
        // SAFETY: the tap is disabled, detached and invalidated, so no callback can run, and this runs on
        // the thread that runs them, so none is in progress.
        drop(unsafe { Box::from_raw(self.ctx) });
    }
}

unsafe extern "C" fn trampoline(
    _proxy: *mut c_void,
    kind: u32,
    event: CGEventRef,
    info: *mut c_void,
) -> CGEventRef {
    // SAFETY: `info` is the Ctx leaked by `PasteTap::start`; it lives until the tap is dropped.
    let ctx = unsafe { &*info.cast::<Ctx>() };
    if matches!(kind, DISABLED_BY_TIMEOUT | DISABLED_BY_USER_INPUT) {
        // macOS unplugs taps that stall; put it back instead of staying dead.
        // SAFETY: the stored pointer is the live tap set up in `start`.
        unsafe { CGEventTapEnable(ctx.tap.get(), true) };
        return event;
    }
    if kind == KEY_DOWN {
        // SAFETY: `event` is the live event handed to the callback.
        let (code, flags, repeat, tag) = unsafe {
            (
                CGEventGetIntegerValueField(event, KEYBOARD_KEYCODE),
                CGEventGetFlags(event),
                CGEventGetIntegerValueField(event, KEYBOARD_AUTOREPEAT) != 0,
                CGEventGetIntegerValueField(event, field::SOURCE_USER_DATA),
            )
        };
        if is_paste_press(code, flags, repeat, tag) {
            // A panic must never unwind through CoreGraphics frames or hold back the user's key.
            let _ = catch_unwind(AssertUnwindSafe(|| (ctx.on_paste)()));
        }
    }
    event
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::rc::Rc;

    #[link(name = "CoreGraphics", kind = "framework")]
    unsafe extern "C" {
        fn CGEventCreateKeyboardEvent(source: *mut c_void, key: u16, down: bool) -> CGEventRef;
        fn CGEventSetFlags(event: CGEventRef, flags: u64);
        fn CGEventSetIntegerValueField(event: CGEventRef, field: u32, value: i64);
    }
    #[link(name = "CoreFoundation", kind = "framework")]
    unsafe extern "C" {
        fn CFRelease(cf: *const c_void);
    }

    #[test]
    fn only_a_plain_command_v_by_a_person_counts() {
        assert!(is_paste_press(9, FLAG_COMMAND, false, 0));
        // Caps Lock, Fn and the like do not matter; other modifiers do.
        assert!(is_paste_press(
            9,
            FLAG_COMMAND | 0x0001_0000 | 0x0080_0000,
            false,
            0
        ));
        assert!(!is_paste_press(9, FLAG_COMMAND | FLAG_SHIFT, false, 0));
        assert!(!is_paste_press(9, FLAG_COMMAND | FLAG_OPTION, false, 0));
        assert!(!is_paste_press(9, FLAG_COMMAND | FLAG_CONTROL, false, 0));
        assert!(!is_paste_press(9, 0, false, 0), "a plain V is typing");
        assert!(!is_paste_press(8, FLAG_COMMAND, false, 0), "Command+C");
        assert!(
            !is_paste_press(9, FLAG_COMMAND, true, 0),
            "a held key repeats"
        );
        assert!(
            !is_paste_press(9, FLAG_COMMAND, false, SELF_TAG),
            "the paste this app posts itself"
        );
    }

    /// Runs the tap's callback on an event that is created but never posted, so nothing reaches the system.
    #[test]
    fn the_callback_runs_for_a_paste_and_always_lets_the_event_through() {
        let count = Rc::new(Cell::new(0));
        let counted = Rc::clone(&count);
        let ctx = Box::into_raw(Box::new(Ctx {
            tap: Cell::new(ptr::null_mut()),
            on_paste: Box::new(move || counted.set(counted.get() + 1)),
        }));
        let make = |key: u16, flags: u64, repeat: i64, tag: i64| {
            // SAFETY: the event is created, configured and released in this test, and never posted.
            unsafe {
                let event = CGEventCreateKeyboardEvent(ptr::null_mut(), key, true);
                assert!(!event.is_null());
                CGEventSetFlags(event, flags);
                CGEventSetIntegerValueField(event, KEYBOARD_AUTOREPEAT, repeat);
                CGEventSetIntegerValueField(event, field::SOURCE_USER_DATA, tag);
                event
            }
        };
        let deliver = |kind: u32, event: CGEventRef| {
            // SAFETY: `ctx` is live for the whole test and `event` is a valid event.
            let out = unsafe { trampoline(ptr::null_mut(), kind, event, ctx.cast()) };
            assert_eq!(out, event, "the event always goes on");
        };
        let release = |event: CGEventRef| {
            // SAFETY: the event was created with a +1 count above.
            unsafe { CFRelease(event) };
        };
        for (key, flags, repeat, tag, kind, expected) in [
            (9u16, FLAG_COMMAND, 0, 0, KEY_DOWN, 1),
            (9, FLAG_COMMAND, 1, 0, KEY_DOWN, 1),
            (9, FLAG_COMMAND, 0, SELF_TAG, KEY_DOWN, 1),
            (9, FLAG_COMMAND | FLAG_SHIFT, 0, 0, KEY_DOWN, 1),
            (8, FLAG_COMMAND, 0, 0, KEY_DOWN, 1),
            (9, FLAG_COMMAND, 0, 0, KEY_DOWN, 2),
            // Not a key down at all (a key up, for instance): never a paste.
            (9, FLAG_COMMAND, 0, 0, 11, 2),
        ] {
            let event = make(key, flags, repeat, tag);
            deliver(kind, event);
            release(event);
            assert_eq!(
                count.get(),
                expected,
                "key {key} flags {flags:#x} kind {kind}"
            );
        }
        // A panicking handler does not escape the callback.
        // SAFETY: nothing else holds `ctx`; it is replaced by a fresh one that panics.
        drop(unsafe { Box::from_raw(ctx) });
        let panicking = Box::into_raw(Box::new(Ctx {
            tap: Cell::new(ptr::null_mut()),
            on_paste: Box::new(|| panic!("handler failed")),
        }));
        let event = make(9, FLAG_COMMAND, 0, 0);
        // SAFETY: as above.
        let out = unsafe { trampoline(ptr::null_mut(), KEY_DOWN, event, panicking.cast()) };
        assert_eq!(out, event);
        release(event);
        // SAFETY: as above.
        drop(unsafe { Box::from_raw(panicking) });
    }

    fn key_event(kind: NSEventType, flags: NSEventModifierFlags, code: u16) -> Retained<NSEvent> {
        NSEvent::keyEventWithType_location_modifierFlags_timestamp_windowNumber_context_characters_charactersIgnoringModifiers_isARepeat_keyCode(
            kind,
            objc2_foundation::NSPoint::ZERO,
            flags,
            1.0,
            0,
            None,
            &objc2_foundation::NSString::from_str(""),
            &objc2_foundation::NSString::from_str(""),
            false,
            code,
        )
        .expect("a synthetic key event")
    }

    #[test]
    fn modifier_events_are_read_as_the_four_keys_and_other_events_spoil_a_tap() {
        let alt = key_event(NSEventType::FlagsChanged, NSEventModifierFlags::Option, 58);
        assert_eq!(
            observe(&alt),
            Observed::Modifiers(Flags {
                option: true,
                ..Flags::default()
            })
        );
        let chord = key_event(
            NSEventType::FlagsChanged,
            NSEventModifierFlags::Option | NSEventModifierFlags::Command,
            55,
        );
        assert_eq!(
            observe(&chord),
            Observed::Modifiers(Flags {
                option: true,
                command: true,
                ..Flags::default()
            })
        );
        // Caps Lock and Fn are not part of a tap.
        let caps = key_event(
            NSEventType::FlagsChanged,
            NSEventModifierFlags::CapsLock | NSEventModifierFlags::Function,
            57,
        );
        assert_eq!(observe(&caps), Observed::Modifiers(Flags::default()));
        let up = key_event(NSEventType::FlagsChanged, NSEventModifierFlags::empty(), 58);
        assert_eq!(observe(&up), Observed::Modifiers(Flags::default()));
        let key = key_event(NSEventType::KeyDown, NSEventModifierFlags::Option, 0);
        assert_eq!(observe(&key), Observed::Other);
    }

    #[test]
    fn the_tap_can_be_made_and_dropped() {
        // Without Accessibility access macOS refuses the tap and there is nothing to drop.
        if let Some(tap) = PasteTap::start(|| {}) {
            drop(tap);
        }
    }
}
