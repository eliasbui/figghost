//! Draws the closed notch with each live island of the islands and utilities features into PNGs, without showing
//! anything on screen, so that a symbol name the system does not know (it would draw nothing) or a title too
//! long for the shape can be seen. `cargo run -p macos-sys --example render_islands -- /tmp/islands`

use macos_sys::{
    MainThreadMarker, app,
    notch::{Activity, Detail, Ear, Notch, NotchGeometry, Style},
    style::Tone,
    ui::AppCallbacks,
};
use objc2_foundation::{NSPoint, NSRect, NSSize};
use std::{cell::Cell, env, fs, path::PathBuf, process, rc::Rc};

fn symbol(name: &'static str, tone: Tone, pulse: bool) -> Ear {
    Ear::Symbol { name, tone, pulse }
}

fn text(value: &str, tone: Tone) -> Ear {
    Ear::Text {
        text: value.into(),
        tone,
    }
}

fn island(left: Ear, right: Ear, title: &str, subtitle: &str) -> Activity {
    Activity {
        left,
        right,
        detail: Some(Detail {
            title: title.into(),
            subtitle: subtitle.into(),
        }),
        page: None,
        priority: 20,
    }
}

fn main() {
    let dir = PathBuf::from(env::args().nth(1).unwrap_or_else(|| "/tmp/islands".into()));
    fs::create_dir_all(&dir).expect("output folder");
    let mtm = MainThreadMarker::new().expect("main thread");
    app::run_accessory(mtm, move || {
        let geometry = NotchGeometry {
            screen: NSRect::new(NSPoint::new(-30_000.0, 0.0), NSSize::new(1_920.0, 1_080.0)),
            notch: NSSize::new(185.0, 32.0),
            hardware: true,
        };
        let noop = || AppCallbacks {
            on_query: Box::new(|_, _| {}),
            on_activate: Box::new(|_, _, _| {}),
            on_pin: Box::new(|_, _| {}),
            on_delete: Box::new(|_, _| {}),
        };
        let notch = Rc::new(Notch::new(mtm, geometry, noop(), |_| {}));
        notch.configure(&Style::default());
        let shots: Vec<(&str, Activity)> = vec![
            ("caps-on", island(symbol("capslock.fill", Tone::Orange, false), Ear::Empty, "Caps Lock", "On")),
            ("caps-off", island(symbol("capslock", Tone::Gray, false), Ear::Empty, "Caps Lock", "Off")),
            ("plugged", island(symbol("powerplug.fill", Tone::Green, true), text("64%", Tone::Green), "Power adapter connected", "")),
            ("unplugged", island(symbol("battery.75", Tone::Gray, false), text("64%", Tone::Gray), "Running on battery", "")),
            ("full", island(symbol("battery.100", Tone::Green, false), text("100%", Tone::Green), "Battery full", "Charged to 100%")),
            ("low", island(symbol("battery.25", Tone::Red, true), text("15%", Tone::Red), "Battery low", "15% left, plug in soon")),
            ("drive", island(symbol("externaldrive.fill", Tone::Blue, true), symbol("checkmark.circle.fill", Tone::Green, false), "Backup", "Connected")),
            ("ejected", island(symbol("externaldrive", Tone::Gray, false), symbol("checkmark.circle.fill", Tone::Green, false), "Backup", "Safe to unplug")),
            ("eject-failed", island(symbol("exclamationmark.triangle.fill", Tone::Orange, false), Ear::Empty, "Backup", "The drive is in use and could not be ejected")),
            ("download", island(symbol("arrow.down.circle.fill", Tone::Blue, false), symbol("checkmark.circle.fill", Tone::Green, false), "Figghost-1.5.0.dmg", "Download finished, 48.3 MB")),
            ("camera", island(symbol("video.fill", Tone::Green, true), Ear::Empty, "Camera in use", "")),
            ("both", island(symbol("video.fill", Tone::Green, true), symbol("mic.fill", Tone::Orange, true), "Camera and microphone in use", "")),
            ("awake", island(symbol("cup.and.saucer.fill", Tone::Orange, false), text("1h", Tone::Orange), "Keeping this Mac awake", "For 1 hour")),
            ("awake-off", island(symbol("cup.and.saucer", Tone::Gray, false), Ear::Empty, "This Mac may sleep again", "")),
            ("mic-muted", island(symbol("mic.slash.fill", Tone::Red, false), Ear::Empty, "Microphone muted", "Every app hears nothing")),
            ("mic-live", island(symbol("mic.fill", Tone::Green, false), Ear::Empty, "Microphone on", "")),
            ("lock", island(symbol("lock.fill", Tone::Orange, true), text("0:42", Tone::Orange), "Keyboard locked", "Command and Escape unlock it")),
            ("unlocked", island(symbol("lock.open.fill", Tone::Green, false), Ear::Empty, "Keyboard unlocked", "")),
        ];
        let dir = Rc::new(dir);
        let step = Rc::new(Cell::new(0_usize));
        let timer = app::every(0.6, move || {
            let n = step.get();
            step.set(n + 1);
            // Two ticks per island: the first puts it on, the second takes its picture.
            let (index, phase) = (n / 2, n % 2);
            let Some((label, activity)) = shots.get(index) else {
                process::exit(0);
            };
            if phase == 0 {
                notch.set_activity("island", Some(activity.clone()));
            } else if let Some(png) = notch.snapshot_closed_png() {
                let path = dir.join(format!("{:02}-{label}.png", index + 1));
                fs::write(&path, png).expect("write png");
                println!("wrote {}", path.display());
            }
        });
        std::mem::forget(timer);
    });
}
