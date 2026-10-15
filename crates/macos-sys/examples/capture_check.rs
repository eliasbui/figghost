//! Freezes every display, lists the windows, and captures the front window on its own; writes PNGs to a
//! folder and checks their sizes against the displays. Needs Screen Recording access for the terminal.
//!
//!     cargo run -p macos-sys --example capture_check -- /tmp/figghost-capture

use macos_sys::screen_capture::{self, Frozen, Shot};
use objc2::MainThreadMarker;
use objc2_app_kit::{NSApplication, NSApplicationActivationPolicy};
use objc2_foundation::{NSDate, NSRunLoop};
use std::{
    cell::RefCell,
    env, fs,
    path::{Path, PathBuf},
    rc::Rc,
    time::Instant,
};

fn spin_until(done: impl Fn() -> bool) {
    let start = Instant::now();
    while !done() && start.elapsed().as_secs() < 10 {
        NSRunLoop::mainRunLoop().runUntilDate(&NSDate::dateWithTimeIntervalSinceNow(0.02));
    }
}

fn save(dir: &Path, name: &str, shot: &Shot) {
    let path = dir.join(name);
    fs::write(&path, shot.png().expect("png")).expect("write");
    println!(
        "  wrote {} ({}x{} px, {:.0}x{:.0} pt, scale {})",
        path.display(),
        shot.width(),
        shot.height(),
        shot.point_size().width,
        shot.point_size().height,
        shot.scale()
    );
}

fn main() {
    let mtm = MainThreadMarker::new().expect("main thread");
    NSApplication::sharedApplication(mtm)
        .setActivationPolicy(NSApplicationActivationPolicy::Accessory);
    let dir = PathBuf::from(
        env::args()
            .nth(1)
            .unwrap_or_else(|| "/tmp/figghost-capture".into()),
    );
    fs::create_dir_all(&dir).unwrap();
    println!("screen access: {}", screen_capture::screen_access(false));

    let started = Instant::now();
    let frozen: Rc<RefCell<Option<Result<Frozen, String>>>> = Rc::default();
    let slot = Rc::clone(&frozen);
    screen_capture::freeze(move |result| *slot.borrow_mut() = Some(result));
    spin_until(|| frozen.borrow().is_some());
    let frozen = frozen
        .borrow_mut()
        .take()
        .expect("no answer")
        .expect("freeze failed");
    println!(
        "froze {} display(s) in {:?}",
        frozen.displays.len(),
        started.elapsed()
    );
    let mut ok = true;
    for (i, d) in frozen.displays.iter().enumerate() {
        println!("display {} frame {:?}", d.id, d.frame);
        save(&dir, &format!("display-{i}.png"), &d.shot);
        let expected = (
            (d.frame.size.width * d.shot.scale()).round() as usize,
            (d.frame.size.height * d.shot.scale()).round() as usize,
        );
        if (d.shot.width(), d.shot.height()) != expected {
            println!("  FAIL: expected {expected:?}");
            ok = false;
        }
        if let Some(part) = d.shot.crop(0, 0, d.shot.width() / 2, d.shot.height() / 2) {
            save(&dir, &format!("display-{i}-top-left.png"), &part);
        }
    }
    for w in frozen.windows.iter().take(5) {
        println!("window {} [{}] '{}' {:?}", w.id, w.app, w.title, w.frame);
    }
    if let Some(front) = frozen.windows.first() {
        let shot: Rc<RefCell<Option<Result<Shot, String>>>> = Rc::default();
        let slot = Rc::clone(&shot);
        screen_capture::capture_window(&frozen, front.id, move |r| *slot.borrow_mut() = Some(r));
        spin_until(|| shot.borrow().is_some());
        match shot.borrow_mut().take() {
            Some(Ok(s)) => {
                save(&dir, "front-window.png", &s);
                // With its shadow the capture is larger than the window itself.
                if s.point_size().width < front.frame.size.width {
                    println!("  FAIL: smaller than the window");
                    ok = false;
                }
            }
            other => {
                println!("window capture failed: {:?}", other.map(|r| r.err()));
                ok = false;
            }
        }
    }
    println!("{}", if ok { "OK" } else { "FAIL" });
}
