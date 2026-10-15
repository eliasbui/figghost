//! Checks seeking from the media page end to end without a click: the page's bar is driven through the accessibility
//! tree, as VoiceOver would drive it, and the page seeks through the Now Playing adapter. Run it while a player that
//! takes seeks is what plays; it tells that player to go to 0:25 (0:10 of 5:00, plus one step of 5%).
//! Exits with status 1 when the bar is not a slider or nothing was sent.
//!   `cargo build -p nowplaying-adapter && cargo run -p macos-sys --example seek_check`

use macos_sys::{
    MainThreadMarker,
    media_ui::{MediaCallbacks, MediaView},
    now_playing::{self, Sample, Track},
};
use objc2::{msg_send, rc::Retained};
use objc2_app_kit::NSView;
use objc2_foundation::{NSPoint, NSRect, NSSize, NSString};
use std::{cell::Cell, process, rc::Rc, time::Instant};

fn find_bar(view: &NSView) -> Option<Retained<NSView>> {
    view.subviews().iter().find_map(|sub| {
        if sub.class().name().to_string_lossy() == "FigProgressBar" {
            Some(sub)
        } else {
            find_bar(&sub)
        }
    })
}

fn main() {
    let mtm = MainThreadMarker::new().expect("main thread");
    let sent: Rc<Cell<Option<(u64, bool)>>> = Rc::default();
    let record = Rc::clone(&sent);
    let view = MediaView::new(
        mtm,
        NSRect::new(NSPoint::ZERO, NSSize::new(500.0, 398.0)),
        MediaCallbacks {
            on_previous: Box::new(|| {}),
            on_play_pause: Box::new(|| {}),
            on_next: Box::new(|| {}),
            on_seek: Box::new(move |ms| {
                record.set(Some((ms, now_playing::seek(ms as f64 / 1000.0))));
            }),
        },
    );
    let track = Track {
        app: "fakeplayer".into(),
        app_name: String::new(),
        title: "Figghost seek check".into(),
        artist: "test".into(),
        album: String::new(),
        playing: false,
        rate: 0.0,
        position_ms: 10_000,
        duration_ms: 300_000,
        artwork: None,
    };
    view.show(Some(&Sample {
        track,
        at: Instant::now(),
    }));
    let Some(bar) = find_bar(&view.root) else {
        println!("FAIL: the page has no bar");
        process::exit(1);
    };
    // SAFETY: an accessibility query of a live view, on the main thread.
    let role: Option<Retained<NSString>> = unsafe { msg_send![&*bar, accessibilityRole] };
    let role = role.map(|r| r.to_string()).unwrap_or_default();
    println!("the bar's role: {role}");
    // SAFETY: an accessibility action of a live view, on the main thread.
    let stepped: bool = unsafe { msg_send![&*bar, accessibilityPerformIncrement] };
    let mut failures = Vec::new();
    if role != "AXSlider" {
        failures.push("the bar is not a slider");
    }
    if !stepped {
        failures.push("the bar refused to step");
    }
    match sent.get() {
        Some((ms, true)) if (24_000..=26_000).contains(&ms) => {
            println!("seek sent to {ms} ms")
        }
        Some((ms, true)) => {
            println!("seek sent to {ms} ms");
            failures.push("the seek went to the wrong place");
        }
        Some((_, false)) => failures.push("the adapter is missing"),
        None => failures.push("nothing was sent"),
    }
    for f in &failures {
        println!("FAIL: {f}");
    }
    println!(
        "{}",
        if failures.is_empty() {
            "seek ok"
        } else {
            "seek broken"
        }
    );
    // The seek runs in its own process, which outlives this one.
    process::exit(i32::from(!failures.is_empty()));
}
