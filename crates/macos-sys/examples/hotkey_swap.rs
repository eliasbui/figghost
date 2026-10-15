//! Checks that a global shortcut can be released and registered again, which is how the settings change one.
//! `cargo run -p macos-sys --example hotkey_swap`

use macos_sys::{
    MainThreadMarker, app,
    hotkey::{Hotkey, modifier},
};
use std::process;

fn main() {
    let mtm = MainThreadMarker::new().expect("main thread");
    app::run_accessory(mtm, move || {
        // F19 with Control+Option+Shift+Command: a combination nothing else uses.
        let (key, mods) = (
            80,
            modifier::CONTROL | modifier::OPTION | modifier::SHIFT | modifier::COMMAND,
        );
        let mut failures = Vec::new();
        let first = Hotkey::register(mtm, key, mods, || {});
        if first.is_err() {
            failures.push("the first registration failed");
        }
        // Registering the same combination while it is held fails, as it must.
        let duplicate = Hotkey::register(mtm, key, mods, || {});
        if duplicate.is_ok() {
            failures.push("a second registration of the same combination was accepted");
        }
        drop(duplicate);
        drop(first);
        // After the release the combination is free again, as many times as needed.
        for round in 1..=3 {
            match Hotkey::register(mtm, key, mods, || {}) {
                Ok(h) => drop(h),
                Err(status) => {
                    println!("round {round}: status {status}");
                    failures.push("the combination could not be registered again after a release");
                }
            }
        }
        // A different combination can replace it.
        let other = Hotkey::register(mtm, 79, mods, || {});
        if other.is_err() {
            failures.push("a different combination could not be registered");
        }
        drop(other);
        for f in &failures {
            println!("FAIL: {f}");
        }
        println!(
            "{}",
            if failures.is_empty() {
                "hotkey swap ok"
            } else {
                "hotkey swap broken"
            }
        );
        process::exit(i32::from(!failures.is_empty()));
    });
}
