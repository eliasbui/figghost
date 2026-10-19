//! Draws the volume indicator in every style into PNGs, without showing anything on screen.
//! `cargo run -p macos-sys --example render_hud -- /tmp/hud`
//!
//! The glass styles appear as their content on black: Liquid Glass is drawn by the window server, not by the
//! view, so what these check is the layout and the ink of each style.

use macos_sys::{
    MainThreadMarker, app,
    hud::{Hud, HudStyle},
};
use std::{env, fs};

fn main() {
    let mtm = MainThreadMarker::new().expect("main thread");
    let out = env::args().nth(1).unwrap_or_else(|| "/tmp/hud".into());
    let folder = std::path::PathBuf::from(out);
    fs::create_dir_all(&folder).expect("make the folder");
    app::run_accessory(mtm, move || {
        let hud = Hud::new(mtm);
        for (style, name) in [
            (HudStyle::Notch, "notch"),
            (HudStyle::Glass, "glass"),
            (HudStyle::Corner, "corner"),
            (HudStyle::Bar, "bar"),
            (HudStyle::Island, "island"),
            (HudStyle::Side, "side"),
            (HudStyle::Tile, "tile"),
            (HudStyle::Ring, "ring"),
            (HudStyle::Equalizer, "equalizer"),
            (HudStyle::Wide, "wide"),
        ] {
            for (level, muted, suffix) in [
                (0.62, false, ""),
                (0.0, true, "-muted"),
                (0.95, false, "-loud"),
            ] {
                let Some(png) = hud.snapshot_png(style, level, muted) else {
                    eprintln!("{name}{suffix}: the snapshot failed");
                    continue;
                };
                let path = folder.join(format!("{name}{suffix}.png"));
                fs::write(&path, &png).expect("write png");
                println!("wrote {} ({} bytes)", path.display(), png.len());
            }
        }
        app::quit(mtm);
        hud
    });
}
