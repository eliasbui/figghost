//! Prints, for each screen, whether the app in front is full screen there as the notch decides it, from the real list
//! of windows. It only reads: nothing is shown and no space changes. `fullscreen_check <seconds>` asks again every
//! second for that long, so that an app can be put in and out of full screen meanwhile.

use macos_sys::{notch::full_screen_on, pin_ui, window::Frame, workspace};
use objc2::MainThreadMarker;
use objc2_app_kit::NSScreen;

fn main() {
    let mtm = MainThreadMarker::new().expect("runs on the main thread");
    let seconds: u64 = std::env::args()
        .nth(1)
        .and_then(|s| s.parse().ok())
        .unwrap_or(0);
    for tick in 0..=seconds {
        if tick > 0 {
            std::thread::sleep(std::time::Duration::from_secs(1));
        }
        let front = workspace::frontmost_app();
        let pid = front.as_ref().map_or(0, |app| app.pid);
        let windows = pin_ui::on_screen_windows();
        let screens = NSScreen::screens(mtm);
        let primary = screens.firstObject().map_or(0.0, |s| s.frame().size.height);
        for s in screens.iter() {
            let f = s.frame();
            let screen = Frame {
                x: f.origin.x,
                y: primary - (f.origin.y + f.size.height),
                w: f.size.width,
                h: f.size.height,
            };
            let full = full_screen_on(&windows, pid, std::process::id() as i32, screen);
            println!(
                "{} front {} full screen: {}",
                s.localizedName(),
                front.as_ref().map_or("none", |app| app.name.as_str()),
                if full { "yes" } else { "no" }
            );
        }
    }
}
