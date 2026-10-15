//! Checks the Accessibility half of window snapping against whatever is under one point of the real screen, and
//! prints only yes or no and the kind of element, never a title, a path or a position: the points are the middle of
//! the primary screen and its corner, and nothing is moved, pressed or shown.
//!
//! It answers: does the hit test work on this system, is the window found from the element, can its frame be
//! read, does an unmoved window read as unmoved, and how long does one press cost (it runs on every left click
//! while the feature is on). A point outside every screen must find nothing, quickly.
//! `cargo run -p macos-sys --example grab_check`

use macos_sys::{
    MainThreadMarker, permissions,
    snap_ui::{displays, grab},
};
use std::{process, time::Instant};

fn main() {
    let mtm = MainThreadMarker::new().expect("main thread");
    if !permissions::accessibility_trusted(false) {
        println!("SKIP Accessibility is not granted to this program");
        process::exit(3);
    }
    let screens = displays(mtm);
    let Some(first) = screens.first() else {
        println!("FAIL no screen");
        process::exit(1);
    };
    // The first of a few points where the system reports anything at all; a locked or empty desktop reports nothing.
    let points = [
        (
            first.frame.x + first.frame.w / 2.0,
            first.frame.y + first.frame.h / 2.0,
        ),
        (first.frame.x + 1.0, first.frame.y + 1.0),
        (first.frame.x, first.frame.y),
    ];
    let Some((x, y)) = points
        .into_iter()
        .find(|(x, y)| macos_sys::ax::Element::at_position(*x, *y, 0.4).is_some())
    else {
        println!(
            "SKIP the system reports no element at any of the points tried (a locked, asleep or empty desktop)"
        );
        process::exit(3);
    };
    let mut failed = false;
    let mut check = |ok: bool, what: String| {
        println!("  {} {what}", if ok { "ok  " } else { "FAIL" });
        failed |= !ok;
    };

    // Which kind of thing is there, to tell "nothing to find" from "the lookup failed".
    let element = macos_sys::ax::Element::at_position(x, y, 0.4);
    println!(
        "  info the hit test found {}",
        element.as_ref().map_or("nothing".to_owned(), |e| format!(
            "an element of role {:?}",
            e.string("AXRole")
        ))
    );
    let started = Instant::now();
    let held = grab(x, y);
    let first_ms = started.elapsed().as_secs_f64() * 1000.0;
    check(
        held.is_some(),
        format!("a window was found under the point ({first_ms:.1} ms)"),
    );
    if let Some(held) = &held {
        check(
            !held.role().is_empty(),
            format!("the element has a role: {}", held.role()),
        );
        check(
            held.pid().is_some_and(|pid| pid > 0),
            "the window belongs to a process".into(),
        );
        check(held.frame().is_some(), "its frame can be read".into());
        check(
            !held.moved(),
            "a window that nobody moves reads as unmoved".into(),
        );
        check(
            held.frame_at_press().w > 0.0 && held.frame_at_press().h > 0.0,
            "the frame at the press has a size".into(),
        );
    }
    let mut worst = first_ms;
    for _ in 0..20 {
        let started = Instant::now();
        let _ = grab(x, y);
        worst = worst.max(started.elapsed().as_secs_f64() * 1000.0);
    }
    check(
        worst < 100.0,
        format!("a press costs at most {worst:.1} ms over 21 tries"),
    );

    let started = Instant::now();
    let nothing = grab(-1.0e6, -1.0e6);
    let ms = started.elapsed().as_secs_f64() * 1000.0;
    check(
        nothing.is_none(),
        format!("a point outside every screen finds nothing ({ms:.1} ms)"),
    );
    println!(
        "{}",
        if failed {
            "grab check: FAIL"
        } else {
            "grab check: pass"
        }
    );
    process::exit(i32::from(failed));
}
