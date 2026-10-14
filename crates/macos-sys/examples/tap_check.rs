//! Checks the process tap path end to end without anyone having to listen: plays a quiet test tone with
//! `afplay`, taps it (muting the original), plays it at half volume on an output device, and taps this
//! process's own output to measure what was actually sent.
//!
//! Output device: `TAP_OUTPUT=<device uid>`; default "Microsoft Teams Audio" or another virtual device if
//! there is one (silent), else the default output. Needs the "System Audio Recording" permission for
//! the app that launched it (the terminal).
//!
//!     cargo run -p macos-sys --example tap_check

use macos_sys::audio_tap::{
    self, Muting, Route, RouteFormat, Source, Tap, Transport, output_devices, process_object,
};
use std::{
    env, fs,
    process::Command,
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    },
    thread,
    time::{Duration, Instant},
};

const GAIN: f32 = 0.5;
const TONE_AMPLITUDE: f64 = 0.01;
const RATE: u32 = 48_000;
const LAUNCHD_LABEL: &str = "vn.figghost.tapcheck";

/// Sum of squares and count, as f64 bits and an integer, written from the IO thread.
#[derive(Default)]
struct Meter {
    sum: AtomicU64,
    count: AtomicU64,
}

impl Meter {
    fn add(&self, samples: &[f32], channels: usize) {
        let sum: f64 = samples
            .iter()
            .step_by(channels.max(1))
            .map(|&x| f64::from(x).powi(2))
            .sum();
        let old = f64::from_bits(self.sum.load(Ordering::Relaxed));
        self.sum.store((old + sum).to_bits(), Ordering::Relaxed);
        self.count
            .fetch_add((samples.len() / channels.max(1)) as u64, Ordering::Relaxed);
    }

    fn rms(&self) -> f64 {
        let n = self.count.load(Ordering::Relaxed).max(1) as f64;
        (f64::from_bits(self.sum.load(Ordering::Relaxed)) / n).sqrt()
    }

    fn reset(&self) {
        self.sum.store(0, Ordering::Relaxed);
        self.count.store(0, Ordering::Relaxed);
    }
}

fn write_tone(path: &std::path::Path, seconds: u32) {
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
            * (2.0 * std::f64::consts::PI * 440.0 * f64::from(i) / f64::from(RATE)).sin();
        let s = (x * 32767.0) as i16;
        data.extend_from_slice(&s.to_le_bytes());
        data.extend_from_slice(&s.to_le_bytes());
    }
    fs::write(path, data).expect("write tone");
}

fn wait_for(timeout: Duration, mut ready: impl FnMut() -> bool) -> bool {
    let start = Instant::now();
    while start.elapsed() < timeout {
        if ready() {
            return true;
        }
        thread::sleep(Duration::from_millis(20));
    }
    false
}

fn main() {
    println!("taps supported: {}", audio_tap::taps_supported());
    let devices = output_devices();
    for d in &devices {
        println!(
            "output: {:<28} {:?} hardware volume: {}  uid={}",
            d.name, d.transport, d.hardware_volume, d.uid
        );
    }
    let default = audio_tap::default_output();
    println!("default output: {:?}", default.as_ref().map(|d| &d.name));
    let output = env::var("TAP_OUTPUT")
        .ok()
        .and_then(|uid| devices.iter().find(|d| d.uid == uid).cloned())
        .or_else(|| {
            devices
                .iter()
                .find(|d| d.transport == Transport::Virtual)
                .cloned()
        })
        .or(default)
        .expect("no output device");
    println!("playing the routed tone on: {}", output.name);

    let dir = env::temp_dir().join(format!("figghost-tap-check-{}", std::process::id()));
    fs::create_dir_all(&dir).unwrap();
    let wav = dir.join("tone.wav");
    write_tone(&wav, 4);
    // TAP_PLAYER=launchd starts the tone outside this process tree, the way another app's audio would be.
    let via_launchd = env::var("TAP_PLAYER").is_ok_and(|v| v == "launchd");
    let mut player = if via_launchd {
        let _ = Command::new("/bin/launchctl")
            .args(["remove", LAUNCHD_LABEL])
            .status();
        Command::new("/bin/launchctl")
            .args(["submit", "-l", LAUNCHD_LABEL, "--", "/usr/bin/afplay"])
            .arg(&wav)
            .status()
            .expect("launchctl");
        Command::new("/usr/bin/true").spawn().unwrap()
    } else {
        Command::new("/usr/bin/afplay")
            .arg(&wav)
            .spawn()
            .expect("afplay")
    };
    let mut pid = player.id() as i32;
    if via_launchd {
        wait_for(Duration::from_secs(3), || {
            let out = Command::new("/usr/bin/pgrep")
                .args(["-n", "afplay"])
                .output()
                .unwrap();
            pid = String::from_utf8_lossy(&out.stdout)
                .trim()
                .parse()
                .unwrap_or(0);
            pid > 0
        });
    }
    let mut object = None;
    let found = wait_for(Duration::from_secs(3), || {
        object = process_object(pid).filter(|&o| audio_tap::is_playing(o));
        object.is_some()
    });
    if !found {
        let _ = player.kill();
        panic!("afplay never showed up as a playing process");
    }
    let object = object.unwrap();
    println!(
        "afplay pid {pid} -> process object {object}, responsible pid {:?}",
        audio_tap::responsible_pid(pid)
    );

    let input = Arc::new(Meter::default());
    let tap = Tap::new(
        Source::Processes(&[object]),
        Muting::MutedWhileRead,
        "tap check",
    )
    .expect("create tap");
    println!("tap {} format {:?}", tap.uid(), tap.format());
    let meter = Arc::clone(&input);
    let mut route = Route::new(
        tap,
        &output.uid,
        "Figghost tap check",
        move |f: RouteFormat| {
            println!("route format {f:?}");
            Box::new(
                move |tap: &[f32], tap_ch: usize, out: &mut [f32], out_ch: usize| {
                    meter.add(tap, tap_ch);
                    let frames = (tap.len() / tap_ch).min(out.len() / out_ch);
                    for i in 0..frames {
                        for c in 0..out_ch {
                            out[i * out_ch + c] = tap[i * tap_ch + c.min(tap_ch - 1)] * GAIN;
                        }
                    }
                },
            )
        },
    )
    .expect("create route");
    let started = Instant::now();
    route.start().expect("start route");
    println!("route started in {:?}", started.elapsed());

    let own = process_object(std::process::id() as i32).expect("own process object");
    let sent = Arc::new(Meter::default());
    let monitor_tap = Tap::new(
        Source::Processes(&[own]),
        Muting::Unmuted,
        "tap check monitor",
    )
    .expect("monitor tap");
    let meter = Arc::clone(&sent);
    let mut monitor = Route::new(
        monitor_tap,
        &output.uid,
        "Figghost tap monitor",
        move |_| {
            Box::new(
                move |tap: &[f32], tap_ch: usize, _out: &mut [f32], _out_ch: usize| {
                    meter.add(tap, tap_ch);
                },
            )
        },
    )
    .expect("monitor route");
    monitor.start().expect("start monitor");

    thread::sleep(Duration::from_millis(600));
    input.reset();
    sent.reset();
    thread::sleep(Duration::from_millis(1500));
    let (rms_in, rms_out) = (input.rms(), sent.rms());
    println!("peak sent: {:.5}", route.take_peak());
    drop(monitor);
    drop(route);
    let _ = player.kill();
    let _ = player.wait();
    if via_launchd {
        let _ = Command::new("/bin/launchctl")
            .args(["remove", LAUNCHD_LABEL])
            .status();
    }
    let _ = fs::remove_dir_all(&dir);

    let expected_in = TONE_AMPLITUDE / 2f64.sqrt();
    println!("tapped RMS {rms_in:.6} (tone {expected_in:.6}), sent RMS {rms_out:.6}");
    if rms_in < expected_in * 0.5 {
        println!(
            "FAIL: the tap delivered (near) silence. Grant \"System Audio Recording\" to the terminal app."
        );
        std::process::exit(1);
    }
    let ratio = rms_out / rms_in;
    println!("sent / tapped = {ratio:.4} (expected {GAIN})");
    if (ratio - f64::from(GAIN)).abs() > 0.05 {
        println!("FAIL: the route did not apply the gain");
        std::process::exit(1);
    }
    println!("OK");
}
