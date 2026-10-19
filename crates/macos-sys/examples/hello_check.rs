//! Plays the hello on the real screen, under the notch of the screen the notch would use, and quits when it is over.
//! It shows on screen, takes no focus and posts no input. With Reduce Motion on, it plays the still version.
//! `cargo run -p macos-sys --example hello_check`

use macos_sys::{MainThreadMarker, app, hello, notch::NotchGeometry};

const OPEN: &[u8] = include_bytes!("../../app-ui/assets/hello/ghost-open.png");
const BLINK: &[u8] = include_bytes!("../../app-ui/assets/hello/ghost-blink.png");

fn main() {
    let mtm = MainThreadMarker::new().expect("main thread");
    app::run_accessory(mtm, move || {
        let geometry = NotchGeometry::detect(mtm).expect("a screen");
        let resting = geometry.frame(geometry.notch.width, geometry.notch.height);
        println!(
            "hello under the {} at {:?}, for {} s",
            if geometry.hardware { "notch" } else { "pill" },
            resting,
            hello::SECONDS
        );
        hello::play(mtm, resting, OPEN, BLINK);
        app::after(hello::SECONDS + 0.5, move || app::quit(mtm));
    });
}
