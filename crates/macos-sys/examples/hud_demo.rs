//! Shows the volume indicator for a few seconds.
//! `cargo run -p macos-sys --example hud_demo -- 0.56 [muted] [seconds] [notch|glass|corner|bar]`
//! The style may also be given as the second argument, as `hud_demo 0.56 glass`.

use macos_sys::{
    MainThreadMarker, app,
    hud::{Hud, HudStyle},
};
use std::env;

fn main() {
    let mtm = MainThreadMarker::new().expect("main thread");
    let args: Vec<String> = env::args().skip(1).collect();
    let named = args
        .iter()
        .position(|a| ["notch", "glass", "corner", "bar"].contains(&a.as_str()));
    let style = HudStyle::named(named.map(|i| args[i].as_str()).unwrap_or("notch"));
    let level: f32 = args.first().and_then(|s| s.parse().ok()).unwrap_or(0.56);
    let muted = args.iter().any(|a| a == "muted");
    let seconds: f64 = args
        .iter()
        .filter_map(|s| s.parse().ok())
        .nth(1)
        .unwrap_or(3.0);
    app::run_accessory(mtm, move || {
        let hud = Hud::with_style(mtm, style);
        hud.show(level, muted);
        app::after(seconds, move || app::quit(mtm));
        hud
    });
}
