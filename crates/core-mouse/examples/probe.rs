//! Runs the scroll engine with a recording sink instead of posting events, so a synthetic wheel event
//! can be checked end to end without scrolling any window: `cargo run -p core-mouse --example probe -- 6`.

use core_mouse::{Control, Engine, Phase, Settings};
use std::{
    env,
    sync::{Arc, Mutex},
    thread,
    time::{Duration, Instant},
};

fn main() {
    let seconds: u64 = env::args().nth(1).and_then(|s| s.parse().ok()).unwrap_or(5);
    let control = Control::new(Settings::default());
    let frames = Arc::new(Mutex::new(Vec::new()));
    let sink = Arc::clone(&frames);
    let started = Instant::now();
    let engine = Engine::start_with(control, move |f| {
        sink.lock()
            .unwrap()
            .push((started.elapsed().as_millis(), f));
    })
    .expect("engine needs Accessibility access");
    eprintln!("probe running for {seconds}s");
    thread::sleep(Duration::from_secs(seconds));
    drop(engine);

    let frames = frames.lock().unwrap();
    let (mut dx, mut dy) = (0, 0);
    for (t, f) in frames.iter() {
        dx += f.dx;
        dy += f.dy;
        println!("t={t:>5}ms dx={:>4} dy={:>4} {:?}", f.dx, f.dy, f.phase);
    }
    let began = frames
        .iter()
        .filter(|(_, f)| f.phase == Phase::Began)
        .count();
    let ended = frames
        .iter()
        .filter(|(_, f)| f.phase == Phase::Ended)
        .count();
    println!(
        "frames={} total_dx={dx} total_dy={dy} began={began} ended={ended}",
        frames.len()
    );
}
