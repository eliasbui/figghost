//! A lock for the keyboard, so that it can be wiped without typing garbage into whatever is open: while it
//! lives, every key press and every system key (volume, brightness, media) is swallowed before any app sees it.
//! The pointer is not touched. Command and Escape together let the keys go again, so does dropping it, and so does
//! the time it was started for: the tap itself stops swallowing when either has come, whatever the rest of the app
//! is doing, so a keyboard cannot stay dead behind an app that has stopped answering. Needs Accessibility, as every
//! tap does.

use crate::event_tap::{self, EventTap, TapError, Verdict};
use std::{
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::{Duration, Instant},
};

const KEY_DOWN: u32 = 10;
const KEY_UP: u32 = 11;
const FLAGS_CHANGED: u32 = 12;
const SYSTEM_DEFINED: u32 = 14;
/// `kCGKeyboardEventKeycode`.
const KEYCODE_FIELD: u32 = 9;
const ESCAPE: i64 = 53;
/// `kCGEventFlagMaskCommand`.
const COMMAND: u64 = 0x0010_0000;

pub struct KeyLock {
    _tap: EventTap,
    released: Arc<AtomicBool>,
}

impl KeyLock {
    /// Locks the keys for at most `longest`.
    pub fn start(longest: Duration) -> Result<Self, TapError> {
        let released = Arc::new(AtomicBool::new(false));
        let flag = Arc::clone(&released);
        let until = Instant::now() + longest;
        let mask = event_tap::mask(KEY_DOWN)
            | event_tap::mask(KEY_UP)
            | event_tap::mask(FLAGS_CHANGED)
            | event_tap::mask(SYSTEM_DEFINED);
        let tap = EventTap::spawn(mask, move |kind, event| {
            if flag.load(Ordering::Acquire) || Instant::now() >= until {
                return Verdict::Pass;
            }
            let escape = kind == KEY_DOWN && event.int(KEYCODE_FIELD) == ESCAPE;
            if escape && event.flags() & COMMAND != 0 {
                flag.store(true, Ordering::Release);
            }
            Verdict::Drop
        })?;
        Ok(Self {
            _tap: tap,
            released,
        })
    }

    /// The person pressed Command and Escape.
    pub fn was_released(&self) -> bool {
        self.released.load(Ordering::Acquire)
    }
}
