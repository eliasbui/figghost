//! Presses one media or volume key at the HID level, as a keyboard would.
//! `cargo run -p macos-sys --example post_key -- up|down|mute|play|next|previous`

use macos_sys::system_keys::{self, SystemKey};

fn main() {
    let name = std::env::args().nth(1).unwrap_or_default();
    let key = match name.as_str() {
        "up" => SystemKey::SoundUp,
        "down" => SystemKey::SoundDown,
        "mute" => SystemKey::Mute,
        "play" => SystemKey::PlayPause,
        "next" => SystemKey::Next,
        "previous" => SystemKey::Previous,
        other => {
            eprintln!("unknown key '{other}'; use up, down, mute, play, next or previous");
            std::process::exit(2);
        }
    };
    system_keys::post(key);
    // The window server takes the events asynchronously; leaving at once can lose them.
    std::thread::sleep(std::time::Duration::from_millis(400));
}
