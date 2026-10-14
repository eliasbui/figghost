//! Shows the volume indicator for a few seconds.
//! `cargo run -p macos-sys --example hud_demo -- 0.56 [muted] [seconds]`

use macos_sys::{MainThreadMarker, app, hud::Hud};
use std::env;

fn main() {
    let mtm = MainThreadMarker::new().expect("main thread");
    let level: f32 = env::args()
        .nth(1)
        .and_then(|s| s.parse().ok())
        .unwrap_or(0.56);
    let muted = env::args().nth(2).is_some_and(|s| s == "muted");
    let seconds: f64 = env::args()
        .nth(3)
        .and_then(|s| s.parse().ok())
        .unwrap_or(3.0);
    app::run_accessory(mtm, move || {
        let hud = Hud::new(mtm);
        hud.show(level, muted);
        app::after(seconds, move || app::quit(mtm));
        hud
    });
}
