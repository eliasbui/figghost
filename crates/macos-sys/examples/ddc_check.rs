//! Lists the external displays that answer over DDC/CI, with their brightness and contrast, and how long finding
//! them took. With `write`, it sets each display's brightness to the value it already has, which changes nothing
//! on the screen but checks that a set goes out.

use macos_sys::ddc::{self, Control};
use std::time::Instant;

fn main() {
    let write = std::env::args().nth(1).as_deref() == Some("write");
    let started = Instant::now();
    let monitors = ddc::monitors();
    println!(
        "{} external port(s) found in {:?}, a built-in screen is {}",
        monitors.len(),
        started.elapsed(),
        if ddc::built_in_screen_on() {
            "on"
        } else {
            "off or absent"
        }
    );
    for monitor in &monitors {
        let brightness = monitor.read(Control::Brightness);
        let contrast = monitor.read(Control::Contrast);
        println!(
            "  {:?} (port {:#x}): brightness {brightness:?}, contrast {contrast:?}",
            monitor.name, monitor.id
        );
        if write && let Some(level) = brightness {
            let sent = monitor.write_level(Control::Brightness, level.current);
            let after = monitor.read(Control::Brightness);
            println!("  set to the same value: sent {sent}, now {after:?}");
            assert_eq!(after, Some(level), "the brightness must be what it was");
        }
    }
}
