//! Synthetic input events.

use crate::event_tap::field;
use std::{ffi::c_void, ptr};

/// Stamped on every event this app posts so the scroll tap can recognise and ignore its own output.
pub const SELF_TAG: i64 = 0x4647_4D53;

const SESSION_EVENT_TAP: u32 = 1;
const HID_EVENT_TAP: u32 = 0;
const UNIT_PIXEL: u32 = 0;
const KEY_V: u16 = 9;
const KEY_EQUALS: u16 = 24;
const KEY_MINUS: u16 = 27;
const FLAG_COMMAND: u64 = 0x0010_0000;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScrollPhase {
    Began = 1,
    Changed = 2,
    Ended = 4,
}

type CGEventRef = *mut c_void;

#[link(name = "CoreGraphics", kind = "framework")]
unsafe extern "C" {
    fn CGEventCreateScrollWheelEvent2(
        source: *mut c_void,
        units: u32,
        wheel_count: u32,
        wheel1: i32,
        wheel2: i32,
        wheel3: i32,
    ) -> CGEventRef;
    fn CGEventCreateKeyboardEvent(source: *mut c_void, key: u16, down: bool) -> CGEventRef;
    fn CGEventSetFlags(event: CGEventRef, flags: u64);
    fn CGEventSetIntegerValueField(event: CGEventRef, field: u32, value: i64);
    fn CGEventSetDoubleValueField(event: CGEventRef, field: u32, value: f64);
    fn CGEventPost(tap: u32, event: CGEventRef);
}

#[link(name = "CoreFoundation", kind = "framework")]
unsafe extern "C" {
    fn CFRelease(cf: *const c_void);
}

/// Posts an event someone else created (an NSEvent's CGEvent) at the HID level.
pub(crate) fn post_raw(event: *mut c_void) {
    // SAFETY: the caller passes a live CGEvent; posting does not take ownership.
    unsafe { CGEventPost(HID_EVENT_TAP, event) };
}

/// Posts one pixel-based continuous scroll event at the current pointer position.
pub fn post_scroll(dx: i32, dy: i32, phase: ScrollPhase) {
    // SAFETY: the event is created, configured, posted and released within this call.
    unsafe {
        let event = CGEventCreateScrollWheelEvent2(ptr::null_mut(), UNIT_PIXEL, 2, dy, dx, 0);
        if event.is_null() {
            return;
        }
        // A held modifier (Shift for sideways scrolling, Option for boosting) was already acted on by the
        // engine; an app seeing it again would apply it twice, or turn a trailing glide into a Command+scroll.
        CGEventSetFlags(event, 0);
        CGEventSetIntegerValueField(event, field::SCROLL_IS_CONTINUOUS, 1);
        CGEventSetDoubleValueField(event, field::SCROLL_POINT_DELTA_AXIS_1, f64::from(dy));
        CGEventSetDoubleValueField(event, field::SCROLL_POINT_DELTA_AXIS_2, f64::from(dx));
        CGEventSetIntegerValueField(event, field::SCROLL_PHASE, phase as i64);
        CGEventSetIntegerValueField(event, field::SOURCE_USER_DATA, SELF_TAG);
        CGEventPost(SESSION_EVENT_TAP, event);
        CFRelease(event);
    }
}

/// Presses and releases one key with the given modifier flags, tagged as ours.
fn post_key(key: u16, flags: u64) {
    for down in [true, false] {
        // SAFETY: as in `post_scroll`.
        unsafe {
            let event = CGEventCreateKeyboardEvent(ptr::null_mut(), key, down);
            if event.is_null() {
                return;
            }
            CGEventSetFlags(event, flags);
            CGEventSetIntegerValueField(event, field::SOURCE_USER_DATA, SELF_TAG);
            CGEventPost(SESSION_EVENT_TAP, event);
            CFRelease(event);
        }
    }
}

/// Posts Command+V to whichever app currently has keyboard focus. Needs Accessibility access.
pub fn post_paste() {
    post_key(KEY_V, FLAG_COMMAND);
}

/// Posts Command+Plus or Command+Minus, the zoom shortcut of browsers, editors and image viewers.
pub fn post_zoom(zoom_in: bool) {
    post_key(if zoom_in { KEY_EQUALS } else { KEY_MINUS }, FLAG_COMMAND);
}
