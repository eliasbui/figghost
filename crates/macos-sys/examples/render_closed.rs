//! Draws the closed notch in each of its states into PNGs, without showing anything on screen: at rest, with a
//! symbol and a value in its ears, with the cover and bars of playing music, and with the name of the track that
//! has just started under them. For a screen with a hardware notch and for one without.
//! `cargo run -p macos-sys --example render_closed -- /tmp/closed`

use macos_sys::{
    MainThreadMarker, app,
    notch::{Activity, Detail, Ear, Notch, NotchGeometry, Style},
    style::Tone,
    ui::AppCallbacks,
};
use objc2_foundation::{NSPoint, NSRect, NSSize};
use std::{cell::Cell, env, fs, path::PathBuf, process, rc::Rc};

fn main() {
    let dir = PathBuf::from(env::args().nth(1).unwrap_or_else(|| "/tmp/closed".into()));
    fs::create_dir_all(&dir).expect("output folder");
    let mtm = MainThreadMarker::new().expect("main thread");
    app::run_accessory(mtm, move || {
        let on = |hardware: bool| NotchGeometry {
            screen: NSRect::new(NSPoint::new(-30_000.0, 0.0), NSSize::new(1_920.0, 1_080.0)),
            notch: if hardware {
                NSSize::new(185.0, 32.0)
            } else {
                NSSize::new(200.0, 24.0)
            },
            hardware,
        };
        let noop = || AppCallbacks {
            on_query: Box::new(|_, _| {}),
            on_activate: Box::new(|_, _, _| {}),
            on_pin: Box::new(|_, _| {}),
            on_delete: Box::new(|_, _| {}),
        };
        let notches: Vec<(&str, Rc<Notch>)> = [("hardware", true), ("pill", false)]
            .into_iter()
            .map(|(name, hardware)| {
                let notch = Rc::new(Notch::new(mtm, on(hardware), noop(), |_| {}));
                notch.configure(&Style::default());
                (name, notch)
            })
            .collect();
        let dir = Rc::new(dir);
        let step = Rc::new(Cell::new(0_u32));
        let timer = app::every(0.9, move || {
            let n = step.get();
            step.set(n + 1);
            let save = |label: &str| {
                for (name, notch) in &notches {
                    if let Some(png) = notch.snapshot_closed_png() {
                        let path = dir.join(format!("{name}-{label}.png"));
                        fs::write(&path, png).expect("write png");
                        println!("wrote {}", path.display());
                    }
                }
            };
            match n {
                1 => save("1-rest"),
                2 => {
                    for (_, notch) in &notches {
                        notch.set_activity(
                            "a",
                            Some(Activity {
                                left: Ear::Symbol {
                                    name: "terminal",
                                    tone: Tone::Blue,
                                    pulse: false,
                                },
                                right: Ear::Text {
                                    text: "38%".into(),
                                    tone: Tone::Gray,
                                },
                                detail: None,
                                page: Some(0),
                                priority: 20,
                            }),
                        );
                    }
                }
                4 => save("2-ears"),
                5 => {
                    for (_, notch) in &notches {
                        notch.set_activity(
                            "a",
                            Some(Activity {
                                left: Ear::Symbol {
                                    name: "headphones",
                                    tone: Tone::Green,
                                    pulse: false,
                                },
                                right: Ear::Text {
                                    text: "82%".into(),
                                    tone: Tone::Green,
                                },
                                detail: Some(Detail {
                                    title: "AirPods Pro".into(),
                                    subtitle: "Connected".into(),
                                }),
                                page: Some(0),
                                priority: 20,
                            }),
                        );
                    }
                }
                7 => save("3-detail"),
                8 => {
                    for (_, notch) in &notches {
                        notch.set_activity("a", None);
                    }
                }
                10 => {
                    save("4-rest-again");
                    process::exit(0);
                }
                _ => {}
            }
        });
        // Kept alive by the closure's owner: the timer stops when it is dropped.
        std::mem::forget(timer);
    });
}
