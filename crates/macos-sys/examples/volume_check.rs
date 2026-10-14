//! Reads the output volume through CoreAudio, then writes the same values back, which changes nothing
//! audible. Checks that the device can be controlled and that a write is read back unchanged.

use macos_sys::volume;

fn main() {
    let Some(before) = volume::read() else {
        println!("the default output device has no volume this app can change");
        return;
    };
    println!("before: {before:?}");
    assert!(
        volume::set_level(before.level),
        "writing the same level back failed"
    );
    let mute_supported = volume::set_muted(before.muted);
    let after = volume::read().expect("still readable");
    println!("after:  {after:?} (mute control: {mute_supported})");
    assert!(
        (after.level - before.level).abs() < 1e-4,
        "the level changed"
    );
    assert_eq!(after.muted, before.muted, "the mute state changed");
    println!("ok");
}
