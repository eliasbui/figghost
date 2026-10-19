//! Prints the Bluetooth devices connected now.
fn main() {
    for d in macos_sys::bluetooth::connected() {
        println!("{d:?}");
    }
}
