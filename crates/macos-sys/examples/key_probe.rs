//! Prints every system-defined key event the session sees, and lets all of them through.
//! `cargo run -p macos-sys --example key_probe -- 8`

use macos_sys::{
    event_tap::{EventTap, Verdict, mask},
    system_keys::SYSTEM_DEFINED,
};
use std::{
    env, thread,
    time::{Duration, Instant},
};

fn main() {
    let seconds: u64 = env::args().nth(1).and_then(|s| s.parse().ok()).unwrap_or(8);
    let started = Instant::now();
    let _tap = EventTap::spawn(mask(SYSTEM_DEFINED), move |kind, event| {
        println!(
            "{:>6} ms  kind={kind} key={:?} flags={:#x}",
            started.elapsed().as_millis(),
            event.system_key(),
            event.flags()
        );
        Verdict::Pass
    })
    .expect("needs Accessibility access");
    println!("listening for {seconds} s");
    thread::sleep(Duration::from_secs(seconds));
}
