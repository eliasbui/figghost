//! Checks the level meter end to end: meters the default output in 5 bands, plays a quiet test tone with
//! `afplay` at a low and then a high frequency, prints the levels about 10 times a second, and checks
//! that the band holding the tone rises above where it stood just before. Then starts and stops the
//! meter repeatedly and checks that no aggregate device or tap is left behind. Exits with 1 on a
//! failure, and with 2 when other audio, far louder than the tones, was playing: run it again in quiet.
//!
//! With a number of seconds it only meters what plays, printing once a second while the run loop runs,
//! so the meter follows a change of output (switch one in Control Center to try it): for measuring CPU.
//!
//! Needs the "System Audio Recording" permission for the app that launched it (the terminal); without
//! it the tap delivers silence, and the check says so.
//!
//!     cargo run -p macos-sys --example meter_check
//!     cargo run -p macos-sys --example meter_check -- 30

use macos_sys::{
    audio_meter::{self, Meter},
    audio_tap,
};
use objc2_foundation::{NSDate, NSRunLoop};
use std::{
    array, env, fs,
    path::Path,
    process::{self, Command},
    thread,
    time::{Duration, Instant},
};

const BANDS: usize = 5;
const TONE_AMPLITUDE: f64 = 0.01;
const RATE: u32 = 48_000;
/// The centres of the lowest and highest of 5 bands.
const LOW_HZ: f64 = 100.0;
const HIGH_HZ: f64 = 7_000.0;
/// How far the tone's band must rise, on the 0 to 1 scale, over its level just before the tone.
const RISE: f32 = 0.2;
/// A band already this high before the tone carries other audio, far louder than the quiet tone.
const MASKED: f32 = 0.3;
const RESTARTS: u32 = 10;

/// What a test tone did to its band.
#[derive(Debug, Clone, Copy, PartialEq)]
enum Tone {
    Rose,
    /// Other audio playing before or during the tone drowned it out.
    Masked,
    Missed,
}

fn write_tone(path: &Path, freq: f64, seconds: u32) {
    let frames = RATE * seconds;
    let mut data = Vec::with_capacity(44 + frames as usize * 4);
    let push32 = |d: &mut Vec<u8>, v: u32| d.extend_from_slice(&v.to_le_bytes());
    data.extend_from_slice(b"RIFF");
    push32(&mut data, 36 + frames * 4);
    data.extend_from_slice(b"WAVEfmt ");
    push32(&mut data, 16);
    data.extend_from_slice(&1u16.to_le_bytes());
    data.extend_from_slice(&2u16.to_le_bytes());
    push32(&mut data, RATE);
    push32(&mut data, RATE * 4);
    data.extend_from_slice(&4u16.to_le_bytes());
    data.extend_from_slice(&16u16.to_le_bytes());
    data.extend_from_slice(b"data");
    push32(&mut data, frames * 4);
    for i in 0..frames {
        let x = TONE_AMPLITUDE
            * (2.0 * std::f64::consts::PI * freq * f64::from(i) / f64::from(RATE)).sin();
        let s = (x * 32767.0) as i16;
        data.extend_from_slice(&s.to_le_bytes());
        data.extend_from_slice(&s.to_le_bytes());
    }
    fs::write(path, data).expect("write tone");
}

fn show(levels: &[f32]) -> String {
    levels
        .iter()
        .map(|l| format!("{l:.2}"))
        .collect::<Vec<_>>()
        .join(" ")
}

/// Prints the levels every 100 ms for `seconds` and returns their mean per band.
fn watch(meter: &Meter, label: &str, seconds: f64) -> [f32; BANDS] {
    let steps = (seconds * 10.0).round() as usize;
    let mut sum = [0.0f32; BANDS];
    let mut levels = [0.0f32; BANDS];
    for step in 1..=steps {
        thread::sleep(Duration::from_millis(100));
        meter.levels(&mut levels);
        println!("{label:>8} {:4.1}s  {}", step as f64 / 10.0, show(&levels));
        for (s, l) in sum.iter_mut().zip(levels) {
            *s += l;
        }
    }
    sum.map(|s| s / steps as f32)
}

/// Plays the tone and compares `band`, and the band at the other end, with their levels just before.
fn tone(meter: &Meter, dir: &Path, freq: f64, band: usize) -> Tone {
    let label = format!("{freq} Hz");
    let before = watch(meter, "before", 1.0);
    let wav = dir.join(format!("{freq}.wav"));
    write_tone(&wav, freq, 4);
    let mut player = Command::new("/usr/bin/afplay")
        .arg(&wav)
        .spawn()
        .expect("afplay");
    // The tone starts and the bars rise.
    watch(meter, &label, 0.5);
    let during = watch(meter, &label, 2.0);
    let _ = player.kill();
    let _ = player.wait();
    let rise: [f32; BANDS] = array::from_fn(|b| during[b] - before[b]);
    let other = BANDS - 1 - band;
    let outcome = if rise[band] > RISE && rise[other] < RISE {
        Tone::Rose
    } else if before[band] > MASKED || rise[other] >= RISE {
        Tone::Masked
    } else {
        Tone::Missed
    };
    println!(
        "{label}: before {}, during {}, rise {}: {outcome:?}",
        show(&before),
        show(&during),
        show(&rise)
    );
    outcome
}

fn hold(meter: &Meter, seconds: f64) {
    let start = Instant::now();
    let mut levels = [0.0f32; BANDS];
    while start.elapsed().as_secs_f64() < seconds {
        NSRunLoop::mainRunLoop().runUntilDate(&NSDate::dateWithTimeIntervalSinceNow(1.0));
        meter.levels(&mut levels);
        let output = audio_tap::default_output().map(|d| d.name);
        println!(
            "{:5.1}s  {}  heard {}  output {}",
            start.elapsed().as_secs_f64(),
            show(&levels),
            meter.heard(),
            output.unwrap_or_default()
        );
    }
}

/// The counts once they are back to `before`, or after a second. The HAL still lists a destroyed device
/// and tap for a few milliseconds.
fn settled(before: (usize, usize)) -> (usize, usize) {
    let start = Instant::now();
    loop {
        let now = audio_tap::routes_and_taps();
        if now == before || start.elapsed() > Duration::from_secs(1) {
            return now;
        }
        thread::sleep(Duration::from_millis(10));
    }
}

fn fail(message: &str) -> ! {
    println!("FAIL: {message}");
    process::exit(1);
}

fn main() {
    let seconds: Option<f64> = env::args()
        .nth(1)
        .map(|a| a.parse().expect("usage: meter_check [seconds]"));
    println!("taps supported: {}", audio_tap::taps_supported());
    println!(
        "default output: {:?}",
        audio_tap::default_output().map(|d| d.name)
    );
    let before = audio_tap::routes_and_taps();
    let meter = audio_meter::start(BANDS)
        .unwrap_or_else(|e| fail(&format!("the meter did not start: {e}")));
    let running = audio_tap::routes_and_taps();
    println!("aggregate devices and taps: {before:?} before, {running:?} while metering");
    if let Some(seconds) = seconds {
        hold(&meter, seconds);
        return;
    }

    let dir = env::temp_dir().join(format!("figghost-meter-check-{}", process::id()));
    fs::create_dir_all(&dir).expect("temp dir");
    // The first levels arrive about a quarter of a second after the start; the baseline begins after.
    thread::sleep(Duration::from_millis(500));
    let tones = [
        tone(&meter, &dir, LOW_HZ, 0),
        tone(&meter, &dir, HIGH_HZ, BANDS - 1),
    ];
    let _ = fs::remove_dir_all(&dir);
    let heard = meter.heard();
    println!("heard: {heard}");

    drop(meter);
    let after = settled(before);
    let started = Instant::now();
    for _ in 0..RESTARTS {
        drop(audio_meter::start(BANDS).unwrap_or_else(|e| fail(&format!("restart: {e}"))));
    }
    let cycled = settled(before);
    println!(
        "after drop {after:?}, after {RESTARTS} more starts and stops {cycled:?} ({:?} each)",
        started.elapsed() / RESTARTS
    );
    if running != (before.0 + 1, before.1 + 1) {
        fail("the running meter's aggregate device and tap were not both listed");
    }
    if after != before || cycled != before {
        fail("an aggregate device or tap was left behind");
    }
    if !heard {
        fail(
            "the meter heard only silence, even during the test tones. Grant \"System Audio \
             Recording\" to the terminal app (System Settings, Privacy & Security) and run again.",
        );
    }
    if tones.contains(&Tone::Missed) {
        fail("a test tone did not lift its band although nothing else was playing");
    }
    if tones.contains(&Tone::Masked) {
        println!(
            "INCONCLUSIVE: other audio was playing, far louder than the quiet test tone. The meter \
             heard it and nothing was left behind; run again while nothing else plays."
        );
        process::exit(2);
    }
    println!("OK");
}
