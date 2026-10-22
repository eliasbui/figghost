//! Reads, on this Mac, every fact the live islands and the utilities rely on, and holds a keep-awake for a few
//! seconds so that `pmset -g assertions` can be looked at from another terminal. Nothing is changed except the
//! assertion, which goes when the program ends. Run: `cargo run -p macos-sys --example system_check [seconds]`.

use macos_sys::{awake::Awake, camera, modifiers, power, sensors, sysinfo, usage, volume, volumes};
use std::{thread, time::Duration};

fn main() {
    let seconds: u64 = std::env::args()
        .nth(1)
        .and_then(|s| s.parse().ok())
        .unwrap_or(0);
    match power::battery() {
        Some(b) => println!(
            "battery: {}%, charging {}, adapter {}, full {}",
            b.percent, b.charging, b.on_ac, b.charged
        ),
        None => println!("battery: none on this Mac"),
    }
    println!("caps lock: {}", modifiers::caps_lock_on());
    println!("camera in use: {}", camera::in_use());
    println!("microphone in use: {}", volume::microphone_in_use());
    println!("microphone muted: {:?}", volume::microphone_muted());
    let drives = volumes::external();
    println!("drives that can be ejected: {}", drives.len());
    for drive in drives {
        println!("  {} at {}", drive.name, drive.path);
    }
    let gib = |bytes: u64| bytes as f64 / (1u64 << 30) as f64;
    match sysinfo::cpu_ticks() {
        Some(cores) => println!(
            "processor: {} cores{}",
            cores.len(),
            sysinfo::core_kinds().map_or(String::new(), |(slow, fast)| format!(
                " ({slow} frugal first, then {fast} fast)"
            ))
        ),
        None => println!("processor: not readable"),
    }
    let started = std::time::Instant::now();
    let (celsius, fans) = (sensors::processor_celsius(), sensors::fans());
    println!(
        "heat: {} (read in {:?}), fans: {fans:?}",
        celsius.map_or("no sensors".into(), |c| format!("{c:.1} C")),
        started.elapsed()
    );
    let again = std::time::Instant::now();
    let _ = (sensors::processor_celsius(), sensors::fans());
    println!("read again in {:?}", again.elapsed());
    if let Some(m) = sysinfo::memory() {
        println!(
            "memory: {:.1} of {:.1} GB used (apps {:.1}, wired {:.1}, compressed {:.1}), pressure {:?}",
            gib(m.used()),
            gib(m.total),
            gib(m.app),
            gib(m.wired),
            gib(m.compressed),
            sysinfo::memory_pressure()
        );
    }
    if let Some((total, used)) = sysinfo::swap() {
        println!("swap: {:.1} of {:.1} GB used", gib(used), gib(total));
    }
    for interface in sysinfo::interfaces() {
        println!(
            "network {}: {} bytes in, {} bytes out (32 bit counters)",
            interface.name, interface.received, interface.sent
        );
    }
    if let Some((free, total)) = sysinfo::disk() {
        println!(
            "startup disk: {:.0} GB free of {:.0} GB",
            gib(free),
            gib(total)
        );
    }
    println!("graphics: {:?} %", sysinfo::gpu_percent());
    println!("thermal state: {:?}", sysinfo::thermal());
    println!("battery wear: {:?}", sysinfo::battery_wear());
    println!("idle: {:.1} s since the last input", usage::idle_seconds());
    for id in [
        "com.apple.finder",
        "com.apple.Safari",
        "com.apple.Terminal",
        "com.example.NotInstalled",
    ] {
        println!("category of {id}: {:?}", usage::app_category(id));
    }
    let hold = Awake::start(true, "Figghost system_check");
    println!(
        "keep awake: {}",
        if hold.is_some() { "granted" } else { "refused" }
    );
    if seconds > 0 {
        println!("holding it for {seconds} s: look at `pmset -g assertions`");
        thread::sleep(Duration::from_secs(seconds));
    }
}
