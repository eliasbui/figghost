//! Follows what macOS reports as Now Playing, as the media tab does, and prints it once a second; with a file name,
//! also draws the media page with the last sample into it.
//!   `cargo build -p nowplaying-adapter && cargo run -p macos-sys --example nowplaying_check -- 20 [page.png]`
//! The adapter library is found next to the build folder, or at FIGGHOST_NOWPLAYING_DYLIB. The process id of the
//! adapter's perl is printed for measuring it: `ps -o %cpu,rss -p <pid>`.

use macos_sys::{MainThreadMarker, media_ui::MediaView, now_playing::Watcher};
use objc2_foundation::{NSDate, NSRunLoop};
use std::{cell::Cell, env, fs, process::Command, rc::Rc, time::Instant};

fn clock(ms: u64) -> String {
    let s = ms / 1000;
    format!("{}:{:02}", s / 60, s % 60)
}

/// The perl that runs the adapter for this process, once it started.
fn adapter_pid() -> Option<String> {
    let out = Command::new("pgrep")
        .args(["-P", &std::process::id().to_string(), "perl"])
        .output()
        .ok()?;
    let pid = String::from_utf8_lossy(&out.stdout).trim().to_owned();
    (!pid.is_empty()).then_some(pid)
}

fn main() {
    let mtm = MainThreadMarker::new().expect("main thread");
    let seconds: u32 = env::args()
        .nth(1)
        .and_then(|s| s.parse().ok())
        .unwrap_or(10);
    let watcher = Watcher::start();
    let changes = Rc::new(Cell::new(0u32));
    let counter = Rc::clone(&changes);
    watcher.set_on_change(move || counter.set(counter.get() + 1));
    watcher.set_active(true);
    let mut shown_pid = false;
    for second in 1..=seconds {
        // Runs the main queue too, where the change notifications arrive.
        NSRunLoop::mainRunLoop().runUntilDate(&NSDate::dateWithTimeIntervalSinceNow(1.0));
        if !shown_pid && let Some(pid) = adapter_pid() {
            println!("adapter perl pid {pid}");
            shown_pid = true;
        }
        let source = if watcher.streams() { "stream" } else { "poll" };
        let what = match watcher.sample() {
            Some(sample) => {
                let t = &sample.track;
                format!(
                    "{} ({}) | {:?} - {:?} | {} | {} / {} | artwork {} bytes",
                    t.app_name,
                    t.app,
                    t.title,
                    t.artist,
                    if t.playing { "playing" } else { "paused" },
                    clock(sample.position_ms(Instant::now())),
                    clock(t.duration_ms),
                    t.artwork.as_ref().map_or(0, |a| a.bytes.len()),
                )
            }
            None => "nothing playing".to_owned(),
        };
        println!("{second:>3}s [{source}] changes {}  {what}", changes.get());
    }
    if let Some(path) = env::args().nth(2) {
        let page = MediaView::preview(mtm);
        page.show(watcher.sample().as_ref());
        fs::write(&path, page.snapshot_png(mtm).expect("png")).expect("write png");
        println!("wrote {path}");
    }
    drop(watcher);
    println!(
        "stopped; adapter still running: {}",
        adapter_pid().is_some()
    );
}
