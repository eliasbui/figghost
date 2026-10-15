//! The displays, the pointer and the clock, in the terms `core-capture` uses.

use crate::MainThreadMarker;
use core_capture::{Display, Point, Rect, naming::LocalTime};
use objc2_app_kit::{NSEvent, NSScreen};
use objc2_foundation::{NSNumber, NSString};

/// Every display with its frame in global points and its pixels per point; the first is the primary one.
pub fn displays(mtm: MainThreadMarker) -> Vec<Display> {
    let key = NSString::from_str("NSScreenNumber");
    NSScreen::screens(mtm)
        .iter()
        .map(|screen| {
            let frame = screen.frame();
            let id = screen
                .deviceDescription()
                .objectForKey(&key)
                .and_then(|n| n.downcast_ref::<NSNumber>().map(NSNumber::unsignedIntValue))
                .unwrap_or(0);
            Display {
                id,
                frame: Rect::new(
                    frame.origin.x,
                    frame.origin.y,
                    frame.size.width,
                    frame.size.height,
                ),
                scale: screen.backingScaleFactor(),
            }
        })
        .collect()
}

pub fn pointer() -> Point {
    let at = NSEvent::mouseLocation();
    Point::new(at.x, at.y)
}

/// The local date and time now.
pub fn local_time() -> LocalTime {
    let mut now: libc::time_t = 0;
    // SAFETY: `time` writes the current time through a valid pointer.
    unsafe { libc::time(&mut now) };
    // SAFETY: `tm` is plain data, and all-zero is a valid value for it.
    let mut parts: libc::tm = unsafe { std::mem::zeroed() };
    // SAFETY: both pointers are valid for the call, and `localtime_r` does not keep them.
    unsafe { libc::localtime_r(&now, &mut parts) };
    LocalTime {
        year: parts.tm_year + 1900,
        month: (parts.tm_mon + 1) as u32,
        day: parts.tm_mday as u32,
        hour: parts.tm_hour as u32,
        minute: parts.tm_min as u32,
        second: parts.tm_sec as u32,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_clock_gives_a_plausible_local_time() {
        let t = local_time();
        assert!(t.year >= 2024);
        assert!((1..=12).contains(&t.month) && (1..=31).contains(&t.day));
        assert!(t.hour < 24 && t.minute < 60 && t.second < 62);
    }
}
