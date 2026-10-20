//! The state of the modifier keys that can be read without any permission.

#[link(name = "CoreGraphics", kind = "framework")]
unsafe extern "C" {
    fn CGEventSourceFlagsState(state: i32) -> u64;
}

/// `kCGEventSourceStateCombinedSessionState`: what the keyboards and the software that posts keys have made of
/// the flags so far in this login session.
const COMBINED_SESSION_STATE: i32 = 0;
/// `kCGEventFlagMaskAlphaShift`, the bit of Caps Lock.
const ALPHA_SHIFT: u64 = 0x0001_0000;

/// Whether Caps Lock is on. Asking sees no key press, only the flags, so it needs no Input Monitoring access.
pub fn caps_lock_on() -> bool {
    // SAFETY: a plain query of the session's flags.
    let flags = unsafe { CGEventSourceFlagsState(COMBINED_SESSION_STATE) };
    flags & ALPHA_SHIFT != 0
}

#[cfg(test)]
mod tests {
    #[test]
    fn the_flag_can_be_read() {
        // Either answer is right: the test only has to survive the call.
        let _ = super::caps_lock_on();
    }
}
