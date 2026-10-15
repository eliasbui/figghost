//! Measures what one process sends to its audio devices without changing it: taps the process (it keeps
//! playing as before) and prints the RMS level of its left channel over a few seconds.
//!
//!     cargo run -p macos-sys --example tap_meter -- <pid> [seconds]
//!
//! With 0 seconds it only prints whether the process is playing now.

use macos_sys::audio_tap::{
    Muting, Route, Source, Tap, default_output, is_playing, process_object,
};
use std::{
    env,
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    },
    thread,
    time::Duration,
};

fn main() {
    let mut args = env::args().skip(1);
    let pid: i32 = args
        .next()
        .and_then(|a| a.parse().ok())
        .expect("usage: tap_meter <pid> [seconds]");
    let seconds: f64 = args.next().and_then(|a| a.parse().ok()).unwrap_or(2.0);
    let Some(object) = process_object(pid) else {
        println!("pid {pid} has no audio process object");
        std::process::exit(1);
    };
    if seconds == 0.0 {
        println!("pid {pid} playing {}", is_playing(object));
        return;
    }
    let output = default_output().expect("no output device");
    let sum = Arc::new(AtomicU64::new(0));
    let frames = Arc::new(AtomicU64::new(0));
    let tap = Tap::new(Source::Processes(&[object]), Muting::Unmuted, "tap meter").expect("tap");
    let (s, f) = (Arc::clone(&sum), Arc::clone(&frames));
    let mut route = Route::new(tap, &output.uid, "Figghost tap meter", move |_| {
        Box::new(
            move |input: &[f32], channels: usize, _: &mut [f32], _: usize| {
                let squares: f64 = input
                    .iter()
                    .step_by(channels.max(1))
                    .map(|&x| f64::from(x).powi(2))
                    .sum();
                let old = f64::from_bits(s.load(Ordering::Relaxed));
                s.store((old + squares).to_bits(), Ordering::Relaxed);
                f.fetch_add((input.len() / channels.max(1)) as u64, Ordering::Relaxed);
            },
        )
    })
    .expect("route");
    route.start().expect("start");
    thread::sleep(Duration::from_secs_f64(seconds));
    route.stop();
    let n = frames.load(Ordering::Relaxed).max(1) as f64;
    let rms = (f64::from_bits(sum.load(Ordering::Relaxed)) / n).sqrt();
    println!(
        "pid {pid} rms {rms:.6} frames {}",
        frames.load(Ordering::Relaxed)
    );
}
