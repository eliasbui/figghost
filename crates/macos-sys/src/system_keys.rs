//! System-defined keys: media keys (play, next, previous) and volume keys. macOS delivers these as
//! NX_SYSDEFINED events with a packed data word instead of ordinary key events.

use crate::synth;
use objc2::rc::Retained;
use objc2_app_kit::{NSEvent, NSEventModifierFlags, NSEventType};
use objc2_foundation::NSPoint;

/// CGEvent type of a system-defined event, for event tap masks.
pub const SYSTEM_DEFINED: u32 = 14;
/// NSEvent subtype of media and volume keys (NX_SUBTYPE_AUX_CONTROL_BUTTONS).
const SUBTYPE_AUX_CONTROL_BUTTONS: i16 = 8;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SystemKey {
    SoundUp,
    SoundDown,
    Mute,
    PlayPause,
    Next,
    Previous,
    /// Brightness, illumination and other keys this app does not handle.
    Other(u8),
}

impl SystemKey {
    pub const fn from_code(code: u8) -> Self {
        match code {
            0 => SystemKey::SoundUp,
            1 => SystemKey::SoundDown,
            7 => SystemKey::Mute,
            16 => SystemKey::PlayPause,
            17 => SystemKey::Next,
            18 => SystemKey::Previous,
            other => SystemKey::Other(other),
        }
    }

    pub const fn code(self) -> u8 {
        match self {
            SystemKey::SoundUp => 0,
            SystemKey::SoundDown => 1,
            SystemKey::Mute => 7,
            SystemKey::PlayPause => 16,
            SystemKey::Next => 17,
            SystemKey::Previous => 18,
            SystemKey::Other(code) => code,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct KeyPress {
    pub key: SystemKey,
    pub down: bool,
    /// Auto-repeat of a key that is being held.
    pub repeat: bool,
}

const STATE_DOWN: i64 = 0xA;
const STATE_UP: i64 = 0xB;

/// Unpacks the `data1` word of an aux-control-button event: key code in the high half, key state and
/// repeat flag in the low half.
pub fn decode(data1: i64) -> KeyPress {
    let flags = data1 & 0xFFFF;
    KeyPress {
        key: SystemKey::from_code(((data1 >> 16) & 0xFF) as u8),
        down: (flags & 0xFF00) >> 8 == STATE_DOWN,
        repeat: flags & 1 != 0,
    }
}

fn encode(key: SystemKey, down: bool) -> i64 {
    (i64::from(key.code()) << 16) | ((if down { STATE_DOWN } else { STATE_UP }) << 8)
}

/// Reads a system key out of an event; `None` for any other kind of event.
pub fn from_ns_event(event: &NSEvent) -> Option<KeyPress> {
    (event.r#type() == NSEventType::SystemDefined
        && event.subtype().0 == SUBTYPE_AUX_CONTROL_BUTTONS)
        .then(|| decode(event.data1() as i64))
}

/// Presses and releases a system key at the HID level, as the keyboard would. Acts on whatever app owns
/// "Now Playing" (media keys) or on the system volume. Needs Accessibility access.
pub fn post(key: SystemKey) {
    for down in [true, false] {
        let flags = NSEventModifierFlags::from_bits_retain(if down { 0xA00 } else { 0xB00 });
        let event: Option<Retained<NSEvent>> =
            NSEvent::otherEventWithType_location_modifierFlags_timestamp_windowNumber_context_subtype_data1_data2(
                NSEventType::SystemDefined,
                NSPoint::ZERO,
                flags,
                0.0,
                0,
                None,
                SUBTYPE_AUX_CONTROL_BUTTONS,
                encode(key, down) as isize,
                -1,
            );
        if let Some(cg) = event.and_then(|e| e.CGEvent()) {
            synth::post_raw(Retained::as_ptr(&cg).cast_mut().cast());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decodes_key_state_and_repeat() {
        let press = decode(encode(SystemKey::PlayPause, true));
        assert_eq!(
            press,
            KeyPress {
                key: SystemKey::PlayPause,
                down: true,
                repeat: false
            }
        );
        let release = decode(encode(SystemKey::Next, false));
        assert_eq!(
            release,
            KeyPress {
                key: SystemKey::Next,
                down: false,
                repeat: false
            }
        );
        let held = decode(encode(SystemKey::SoundUp, true) | 1);
        assert_eq!(
            held,
            KeyPress {
                key: SystemKey::SoundUp,
                down: true,
                repeat: true
            }
        );
    }

    #[test]
    fn every_known_key_survives_a_round_trip() {
        for key in [
            SystemKey::SoundUp,
            SystemKey::SoundDown,
            SystemKey::Mute,
            SystemKey::PlayPause,
            SystemKey::Next,
            SystemKey::Previous,
            SystemKey::Other(3),
        ] {
            for down in [true, false] {
                let press = decode(encode(key, down));
                assert_eq!((press.key, press.down), (key, down));
            }
        }
    }

    #[test]
    fn real_world_words_decode() {
        // Values macOS sends for a volume-up press and release (key code 0, state 0xA / 0xB).
        assert_eq!(
            decode(0x0000_0A00),
            KeyPress {
                key: SystemKey::SoundUp,
                down: true,
                repeat: false
            }
        );
        assert_eq!(
            decode(0x0000_0B00),
            KeyPress {
                key: SystemKey::SoundUp,
                down: false,
                repeat: false
            }
        );
        // Mute (code 7), play/pause (code 16).
        assert_eq!(decode(0x0007_0A00).key, SystemKey::Mute);
        assert_eq!(decode(0x0010_0A00).key, SystemKey::PlayPause);
    }
}
