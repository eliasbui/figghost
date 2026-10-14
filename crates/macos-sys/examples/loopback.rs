//! Checks `post_scroll`, `post_paste` and the media/volume keys end to end without delivering anything to
//! another app: an observer tap records and swallows the events this process posts.
//! `cargo run -p macos-sys --example loopback`

use macos_sys::{
    event_tap::{EventTap, SCROLL_WHEEL, Verdict, field, mask},
    synth::{SELF_TAG, ScrollPhase, post_paste, post_scroll},
    system_keys::{self, SYSTEM_DEFINED, SystemKey},
};
use std::{
    sync::{Arc, Mutex},
    thread,
    time::Duration,
};

const KEY_DOWN: u32 = 10;
const KEY_UP: u32 = 11;
const KEYBOARD_KEYCODE: u32 = 9;

fn main() {
    let seen = Arc::new(Mutex::new(Vec::<String>::new()));
    let log = Arc::clone(&seen);
    let tap = EventTap::spawn(
        mask(SCROLL_WHEEL) | mask(KEY_DOWN) | mask(KEY_UP) | mask(SYSTEM_DEFINED),
        move |kind, e| {
            let tagged = e.int(field::SOURCE_USER_DATA) == SELF_TAG;
            let line = if kind == SYSTEM_DEFINED {
                format!("system key {:?}", e.system_key())
            } else if kind == SCROLL_WHEEL {
                format!(
                    "scroll tagged={tagged} continuous={} phase={} point=({}, {}) delta=({}, {})",
                    e.int(field::SCROLL_IS_CONTINUOUS),
                    e.int(field::SCROLL_PHASE),
                    e.float(field::SCROLL_POINT_DELTA_AXIS_2),
                    e.float(field::SCROLL_POINT_DELTA_AXIS_1),
                    e.int(field::SCROLL_DELTA_AXIS_2),
                    e.int(field::SCROLL_DELTA_AXIS_1),
                )
            } else {
                format!(
                    "key {} tagged={tagged} keycode={} command={}",
                    if kind == KEY_DOWN { "down" } else { "up" },
                    e.int(KEYBOARD_KEYCODE),
                    e.flags() & 0x0010_0000 != 0
                )
            };
            log.lock().unwrap().push(line);
            // Only our own tagged events are swallowed for key events, so typing is never lost.
            // Every system-defined event is swallowed during this short window: a stray media or volume key
            // must never reach the user's players.
            if kind == SCROLL_WHEEL || kind == SYSTEM_DEFINED || tagged {
                Verdict::Drop
            } else {
                Verdict::Pass
            }
        },
    )
    .expect("needs Accessibility access");

    post_scroll(0, -30, ScrollPhase::Began);
    post_scroll(4, -20, ScrollPhase::Changed);
    post_scroll(0, 0, ScrollPhase::Ended);
    post_paste();
    system_keys::post(SystemKey::PlayPause);
    system_keys::post(SystemKey::SoundUp);
    thread::sleep(Duration::from_millis(300));
    drop(tap);
    for line in seen.lock().unwrap().iter() {
        println!("{line}");
    }
}
