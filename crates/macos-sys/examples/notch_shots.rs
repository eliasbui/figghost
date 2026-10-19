//! Shows the notch on the real screen in each of its states, over a colourful backdrop window of its own (so only
//! that backdrop is ever captured), and writes a screenshot of each state to a folder:
//! rest, a live activity in the closed notch, open, and closed again.
//!
//!     cargo run -p macos-sys --example notch_shots -- <folder> [glass|clear|black] [--video] [spring|smooth|none]
//!
//! With `--video` it records the area to `<folder>/motion.mp4` instead, while the notch opens and closes twice:
//! from rest, and from a live activity. For looking at the movement frame by frame.
//!
//! Needs the Screen Recording permission for the terminal (for `screencapture`). It moves no pointer and takes no
//! keyboard focus.

use macos_sys::{
    MainThreadMarker, app,
    notch::{Activity, Ear, Material, Motion, Notch, NotchGeometry, Style},
    recorder::{RecordOptions, Recording, Target},
    screen_capture,
    style::Tone,
    ui::AppCallbacks,
};
use objc2::{AnyThread, MainThreadOnly, define_class, msg_send, rc::Retained};
use objc2_app_kit::{
    NSBackingStoreType, NSColor, NSGradient, NSMainMenuWindowLevel, NSScreen, NSView, NSWindow,
    NSWindowStyleMask,
};
use objc2_foundation::{NSObjectProtocol, NSPoint, NSRect, NSSize};
use std::{
    cell::{Cell, RefCell},
    env,
    path::PathBuf,
    process,
    process::Command,
    rc::Rc,
};

define_class!(
    // SAFETY: NSView allows subclassing; nothing else is added.
    #[unsafe(super(NSView))]
    #[thread_kind = MainThreadOnly]
    #[name = "FigShotsBackdrop"]
    struct Backdrop;

    impl Backdrop {
        #[unsafe(method(drawRect:))]
        fn draw_rect(&self, _dirty: NSRect) {
            let colors = [
                NSColor::colorWithSRGBRed_green_blue_alpha(0.98, 0.55, 0.45, 1.0),
                NSColor::colorWithSRGBRed_green_blue_alpha(0.55, 0.36, 0.95, 1.0),
                NSColor::colorWithSRGBRed_green_blue_alpha(0.18, 0.70, 0.98, 1.0),
            ];
            let list = objc2_foundation::NSArray::from_retained_slice(&colors);
            if let Some(g) = NSGradient::initWithColors(NSGradient::alloc(), &list) {
                g.drawInRect_angle(self.bounds(), -30.0);
            }
            // Something with edges behind the glass, so the refraction shows.
            for i in 0..12 {
                let x = 40.0 + f64::from(i) * 70.0;
                NSColor::colorWithWhite_alpha(1.0, 0.85).setFill();
                objc2_app_kit::NSBezierPath::fillRect(NSRect::new(
                    NSPoint::new(x, 0.0),
                    NSSize::new(14.0, self.bounds().size.height),
                ));
            }
        }
    }

    unsafe impl NSObjectProtocol for Backdrop {}
);

fn main() {
    let args: Vec<String> = env::args().skip(1).collect();
    let folder = PathBuf::from(
        args.first()
            .cloned()
            .unwrap_or_else(|| "notch-shots".into()),
    );
    let video = args.iter().any(|a| a == "--video");
    let material = match args.get(1).map(String::as_str) {
        Some("clear") => Material::ClearGlass,
        Some("ink") => Material::Ink,
        Some("accent") => Material::Accent,
        Some("black") => Material::Black,
        _ => Material::Glass,
    };
    let motion = if args.iter().any(|a| a == "smooth") {
        Motion::Smooth
    } else if args.iter().any(|a| a == "none") {
        Motion::Instant
    } else {
        Motion::Spring
    };
    std::fs::create_dir_all(&folder).expect("output folder");
    let mtm = MainThreadMarker::new().expect("main thread");
    app::run_accessory(mtm, move || {
        let geometry = NotchGeometry::detect(mtm).expect("a screen");
        let screen = NSScreen::mainScreen(mtm).expect("a main screen").frame();
        let size = NSSize::new(900.0, 560.0);
        let area = NSRect::new(
            NSPoint::new(
                geometry.screen.origin.x + (geometry.screen.size.width - size.width) / 2.0,
                geometry.screen.origin.y + geometry.screen.size.height - size.height,
            ),
            size,
        );
        // SAFETY: designated initializer with valid arguments on the main thread.
        let backdrop_window = unsafe {
            NSWindow::initWithContentRect_styleMask_backing_defer(
                mtm.alloc(),
                area,
                NSWindowStyleMask::Borderless,
                NSBackingStoreType::Buffered,
                false,
            )
        };
        // SAFETY: false is the only value compatible with a window kept alive by `Retained`.
        unsafe { backdrop_window.setReleasedWhenClosed(false) };
        // Above the menu bar so nothing of the desktop is captured, below the notch.
        backdrop_window.setLevel(NSMainMenuWindowLevel);
        let view: Retained<Backdrop> =
            // SAFETY: designated initializer with valid arguments on the main thread.
            unsafe { msg_send![mtm.alloc::<Backdrop>(), initWithFrame: NSRect::new(NSPoint::ZERO, size)] };
        backdrop_window.setContentView(Some(&view));
        backdrop_window.orderFrontRegardless();

        let noop = AppCallbacks {
            on_query: Box::new(|_, _| {}),
            on_activate: Box::new(|_, _, _| {}),
            on_pin: Box::new(|_, _| {}),
            on_delete: Box::new(|_, _| {}),
        };
        let notch = Rc::new(Notch::new(mtm, geometry, noop, |_| {}));
        notch.configure(&Style {
            material,
            motion,
            ..Style::default()
        });

        // screencapture takes the rectangle with its origin at the top left of the main screen.
        let rect = format!(
            "{},{},{},{}",
            area.origin.x,
            screen.size.height - (area.origin.y + area.size.height),
            area.size.width,
            area.size.height
        );
        let slow: f64 = env::var("FIGGHOST_SLOW_MOTION")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(1.0);
        if slow > 1.0 {
            // What a movement has to carry along: the views of the notch and the layers inside its glass.
            for window in objc2_app_kit::NSApplication::sharedApplication(mtm)
                .windows()
                .iter()
            {
                if let Some(content) = window.contentView() {
                    dump(&content, 0);
                }
            }
            // The notch plays its movements `slow` times slower; screenshots follow it as fast as they can.
            let (n, folder, rect) = (Rc::clone(&notch), folder.clone(), rect.clone());
            app::after(1.0, move || {
                let started = std::time::Instant::now();
                n.open();
                std::thread::spawn(move || {
                    let mut i = 0;
                    while started.elapsed().as_secs_f64() < 1.3 * slow {
                        let t = started.elapsed().as_secs_f64() / slow;
                        let path = folder.join(format!("open-{i:03}-{:04.0}ms.png", t * 1000.0));
                        let _ = Command::new("screencapture")
                            .args(["-x", "-R", &rect])
                            .arg(&path)
                            .status();
                        i += 1;
                    }
                    process::exit(0);
                });
            });
            let timer = app::every(60.0, || {});
            return (notch, backdrop_window, view, timer);
        }
        if video {
            // screencapture -v waits forever here; ScreenCaptureKit records our own windows when told to.
            let movie = folder.join("motion.mp4");
            let _ = std::fs::remove_file(&movie);
            let include: Vec<u32> = objc2_app_kit::NSApplication::sharedApplication(mtm)
                .windows()
                .iter()
                .map(|w| w.windowNumber() as u32)
                .collect();
            let (n, rect) = (Rc::clone(&notch), area);
            let keep: Rc<RefCell<Option<Recording>>> = Rc::default();
            screen_capture::freeze(move |frozen| {
                let frozen = frozen.expect("the screen can be captured");
                let display = frozen
                    .displays
                    .iter()
                    .position(|d| d.frame.origin == NSPoint::ZERO)
                    .unwrap_or(0);
                let options = RecordOptions {
                    fps: 120,
                    system_audio: false,
                    microphone: false,
                    clicks: false,
                    cursor: false,
                    include_windows: include,
                };
                let target = Target::Area { display, rect };
                Recording::start(
                    &frozen,
                    target,
                    &movie,
                    &options,
                    move |started| {
                        let recording = started.expect("recording starts");
                        *keep.borrow_mut() = Some(recording);
                        let steps: [(f64, u8); 6] =
                            [(0.8, 0), (2.0, 1), (2.8, 2), (3.8, 0), (5.0, 1), (6.4, 3)];
                        for (at, what) in steps {
                            let (n, keep) = (Rc::clone(&n), Rc::clone(&keep));
                            app::after(at, move || match what {
                                0 => n.open(),
                                1 => n.close(),
                                2 => n.set_activity(
                                    "media",
                                    Some(Activity {
                                        left: Ear::Symbol {
                                            name: "music.note",
                                            tone: Tone::Purple,
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
                                ),
                                _ => {
                                    if let Some(r) = keep.borrow().as_ref() {
                                        r.stop();
                                    }
                                }
                            });
                        }
                    },
                    |finished| {
                        println!("motion: {finished:?}");
                        process::exit(0);
                    },
                );
            });
            let timer = app::every(60.0, || {});
            return (notch, backdrop_window, view, timer);
        }
        let shot = move |name: &str| {
            let path = folder.join(format!("{name}.png"));
            let ok = Command::new("screencapture")
                .args(["-x", "-R", &rect])
                .arg(&path)
                .status()
                .is_ok_and(|s| s.success());
            println!(
                "{name}: {}",
                if ok {
                    path.display().to_string()
                } else {
                    "failed".into()
                }
            );
        };
        let shot = Rc::new(shot);
        let phase = Rc::new(Cell::new(0u32));
        let clock = Rc::new(Cell::new(0.0f32));
        let (n, s, p, c) = (
            Rc::clone(&notch),
            Rc::clone(&shot),
            Rc::clone(&phase),
            Rc::clone(&clock),
        );
        let timer = app::every(0.9, move || {
            let step = p.get();
            p.set(step + 1);
            match step {
                1 => s("1-rest"),
                2 => {
                    let clock = Rc::clone(&c);
                    n.set_activity(
                        "media",
                        Some(Activity {
                            left: Ear::Symbol {
                                name: "music.note",
                                tone: Tone::Purple,
                                pulse: false,
                            },
                            right: Ear::Bars {
                                levels: Rc::new(move |out: &mut [f32]| {
                                    let t = clock.get() + 0.07;
                                    clock.set(t);
                                    for (i, v) in out.iter_mut().enumerate() {
                                        *v = 0.35
                                            + 0.6 * ((t * 6.0 + i as f32 * 1.7).sin() * 0.5 + 0.5);
                                    }
                                }),
                                color: [1.0, 0.75, 0.95],
                            },
                            detail: None,
                            page: Some(0),
                            priority: 20,
                        }),
                    );
                }
                3 => s("2-live"),
                4 => n.open(),
                5 => s("3-open"),
                6 => n.close(),
                7 => s("4-closed-live"),
                8 => {
                    n.set_activity("media", None);
                }
                9 => s("5-rest-again"),
                _ if step > 9 => process::exit(0),
                _ => {}
            }
        });
        (notch, backdrop_window, view, timer)
    });
}

fn dump(view: &NSView, depth: usize) {
    let name = view.class().name().to_string_lossy().into_owned();
    println!(
        "{:indent$}{name} {:?} alpha {:.2}{}",
        "",
        view.frame(),
        view.alphaValue(),
        if view.isHidden() { " hidden" } else { "" },
        indent = depth * 2
    );
    if name.contains("Glass")
        && let Some(layer) = view.layer()
    {
        dump_layer(&layer, depth + 1);
    }
    for sub in view.subviews().iter() {
        dump(&sub, depth + 1);
    }
}

fn dump_layer(layer: &objc2_quartz_core::CALayer, depth: usize) {
    println!(
        "{:indent$}layer {} {:?} radius {:.1} clips {} mask {}",
        "",
        layer.class().name().to_string_lossy(),
        layer.frame(),
        layer.cornerRadius(),
        layer.masksToBounds(),
        layer.mask().is_some(),
        indent = depth * 2
    );
    // SAFETY: a read of the sublayers of a live layer, on the main thread.
    if let Some(subs) = unsafe { layer.sublayers() } {
        for sub in subs.iter() {
            dump_layer(&sub, depth + 1);
        }
    }
}
