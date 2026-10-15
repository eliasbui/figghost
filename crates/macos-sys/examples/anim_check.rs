//! Checks the notch's open and close animation without showing anything: the notch is placed far outside
//! every screen and the size of the shape as the window server draws it is sampled while it opens and closes. With
//! Liquid Glass (macOS 26 on), the glass's own shape, which SwiftUI lays out inside its view, is sampled too: it must
//! have the shape's size on every frame, not jump to the end.
//! Exits with status 1 on a failure.
//! `cargo run -p macos-sys --example anim_check`

use macos_sys::{
    MainThreadMarker, app,
    notch::{EXPANDED_SIZE, Notch, NotchGeometry, Style},
    style::PANEL_RADIUS,
    ui::AppCallbacks,
};
use objc2::rc::Retained;
use objc2_app_kit::{NSApplication, NSView};
use objc2_foundation::{NSPoint, NSRect, NSSize};
use objc2_quartz_core::CALayer;
use std::{cell::RefCell, process, rc::Rc, time::Instant};

/// Milliseconds, the shape's width and height, and the glass's own shape when there is glass.
type Samples = Rc<RefCell<Vec<(u128, f64, f64, Option<NSSize>)>>>;

fn find_view(view: &NSView, name: &str) -> Option<Retained<NSView>> {
    view.subviews().iter().find_map(|sub| {
        if sub.class().name().to_string_lossy().contains(name) {
            Some(sub)
        } else {
            find_view(&sub, name)
        }
    })
}

fn find_layer(layer: &CALayer, name: &str) -> Option<Retained<CALayer>> {
    // SAFETY: a read of the sublayers of a live layer, on the main thread.
    let subs = unsafe { layer.sublayers() }?;
    subs.iter().find_map(|sub| {
        if sub.class().name().to_string_lossy() == name {
            Some(sub)
        } else {
            find_layer(&sub, name)
        }
    })
}

/// The size of the shape the glass draws: the element SwiftUI gives it inside the glass view.
fn glass_shape(mtm: MainThreadMarker) -> Option<NSSize> {
    let windows = NSApplication::sharedApplication(mtm).windows();
    windows.iter().find_map(|w| {
        let content = w.contentView()?;
        let glass = find_view(&content, "NSGlassEffectView")?;
        let layer = glass.layer()?;
        let element = find_layer(&layer, "CASDFElementLayer")?;
        Some(element.bounds().size)
    })
}

fn main() {
    let mtm = MainThreadMarker::new().expect("main thread");
    app::run_accessory(mtm, move || {
        let geometry = NotchGeometry {
            screen: NSRect::new(NSPoint::new(-30_000.0, 0.0), NSSize::new(1_920.0, 1_080.0)),
            notch: NSSize::new(200.0, 32.0),
            hardware: false,
        };
        let geometry = if std::env::var_os("ANIM_ON_SCREEN").is_some() {
            NotchGeometry::detect(mtm).expect("a screen")
        } else {
            geometry
        };
        let noop = AppCallbacks {
            on_query: Box::new(|_, _| {}),
            on_activate: Box::new(|_, _, _| {}),
            on_pin: Box::new(|_, _| {}),
            on_delete: Box::new(|_, _| {}),
        };
        let notch = Rc::new(Notch::new(mtm, geometry, noop, |_| {}));
        // Liquid Glass, the default.
        notch.configure(&Style::default());

        let samples: Samples = Rc::default();
        let started = Instant::now();
        let log = Rc::clone(&samples);
        let watched = Rc::clone(&notch);
        let sampler = app::every(0.008, move || {
            let size = watched.visible_size();
            log.borrow_mut().push((
                started.elapsed().as_millis(),
                size.width,
                size.height,
                glass_shape(mtm),
            ));
        });

        let opener = Rc::clone(&notch);
        app::after(0.2, move || opener.open());
        let closer = Rc::clone(&notch);
        app::after(1.4, move || closer.close());
        let log = Rc::clone(&samples);
        let check_notch = Rc::clone(&notch);
        app::after(2.4, move || {
            drop(sampler);
            let samples = log.borrow();
            let mut failures = Vec::new();
            let rest = samples.first().map_or(0.0, |s| s.2);
            let height_at = |ms: u128| {
                samples
                    .iter()
                    .min_by_key(|s| s.0.abs_diff(ms))
                    .map_or(0.0, |s| s.2)
            };
            let peak = samples
                .iter()
                .filter(|s| s.0 > 200 && s.0 < 1_400)
                .map(|s| s.2)
                .fold(0.0, f64::max);
            let opened = height_at(1_300);
            let closed = samples.last().map_or(0.0, |s| s.2);
            println!("samples: {}", samples.len());
            if std::env::var_os("ANIM_VERBOSE").is_some() {
                for (ms, w, h, glass) in samples.iter() {
                    let glass = glass.map_or(String::new(), |g| {
                        format!("  glass {:>6.1} {:>6.1}", g.width, g.height)
                    });
                    println!("    {ms:>5} {w:>6.1} {h:>6.1}{glass}");
                }
            }
            for ms in [150, 250, 330, 420, 520, 700, 1_300, 1_500, 1_600, 2_300] {
                println!("  t={ms:>4} ms  height {:.1}", height_at(ms));
            }
            println!("rest {rest:.1}, peak {peak:.1}, opened {opened:.1}, closed {closed:.1}");
            if samples.len() < 150 {
                failures.push("the animation did not produce frames at a steady rate");
            }
            if rest > 32.5 {
                failures.push("the notch did not start at its resting height");
            }
            if (opened - EXPANDED_SIZE.height).abs() > 0.5 {
                failures.push("the notch did not settle at the open height");
            }
            if peak < EXPANDED_SIZE.height || peak > EXPANDED_SIZE.height * 1.08 {
                failures.push("the spring should overshoot a little, and only a little");
            }
            if height_at(250) >= EXPANDED_SIZE.height * 0.9 || height_at(250) <= rest + 20.0 {
                failures.push("the motion is not spread over time");
            }
            if (closed - rest).abs() > 0.5 {
                failures.push("the notch did not come back to its resting height");
            }
            if check_notch.is_expanded() {
                failures.push("the notch still reports being open");
            }
            // The glass is as wide as the shape and taller by the radius it hides above the screen's edge.
            let with_glass: Vec<_> = samples.iter().filter(|s| s.3.is_some()).collect();
            let off_glass = with_glass
                .iter()
                .filter(|(_, w, h, g)| {
                    let g = g.expect("filtered");
                    (g.width - w).abs() > 0.5 || (g.height - (h + PANEL_RADIUS)).abs() > 0.5
                })
                .count();
            println!(
                "glass sampled {} times, off the shape {off_glass} times",
                with_glass.len()
            );
            if off_glass > 0 {
                failures.push("the glass does not have the shape's size on every frame");
            }
            for f in &failures {
                println!("FAIL: {f}");
            }
            println!(
                "{}",
                if failures.is_empty() {
                    "animation ok"
                } else {
                    "animation broken"
                }
            );
            process::exit(i32::from(!failures.is_empty()));
        });
        (notch,)
    });
}
