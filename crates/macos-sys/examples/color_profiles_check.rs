//! Lists each display with the colour profiles it can take and the one in use, as the Displays tab offers them.
//! `cargo run -p macos-sys --example color_profiles_check [--set-same]`
//!
//! `--set-same` sets each display's profile in use again and checks that it reads back the same: the picture does
//! not change, and a display that was on its own profile is put back on it.

use macos_sys::{MainThreadMarker, color_profiles};
use std::{env, process};

fn main() {
    let mtm = MainThreadMarker::new().expect("main thread");
    let screens = color_profiles::screens(mtm);
    if screens.is_empty() {
        eprintln!("no display");
        process::exit(1);
    }
    for screen in &screens {
        println!(
            "{} (display {}, own {})",
            screen.name, screen.display, screen.own
        );
        for (n, profile) in screen.profiles.iter().enumerate() {
            let mark = if screen.current == Some(n) { "*" } else { " " };
            println!("  {mark} {}  {}", profile.name, profile.path.display());
        }
    }
    if env::args().any(|a| a == "--set-same") {
        for screen in &screens {
            let Some(current) = screen.current else {
                continue;
            };
            let path = (current >= screen.own).then(|| screen.profiles[current].path.as_path());
            assert!(
                color_profiles::set(screen.display, path),
                "{} takes its profile",
                screen.name
            );
        }
        let again = color_profiles::screens(mtm);
        for (before, after) in screens.iter().zip(&again) {
            assert_eq!(
                before.current, after.current,
                "{} reads back the same",
                before.name
            );
        }
        println!("ok: each display took its profile again and reads back the same");
    }
}
