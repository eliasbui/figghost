//! Prints what the Wi-Fi radio of this Mac says of its connection, and how long asking takes.
use std::time::Instant;

fn main() {
    let started = Instant::now();
    let radio = macos_sys::wifi::read();
    println!("{radio:#?}");
    if let Some(r) = &radio {
        println!("grade: {}", macos_sys::wifi::grade(r.rssi, r.noise));
    }
    println!("took {} ms", started.elapsed().as_millis());
}
