//! Checks the notch's open and close animation without showing anything: the notch is placed far outside
//! every screen and its panel size is sampled while it opens and closes. Exits with status 1 on a failure.
//! `cargo run -p macos-sys --example anim_check`

use macos_sys::{
    MainThreadMarker, app,
    notch::{EXPANDED_SIZE, Notch, NotchGeometry},
    ui::AppCallbacks,
};
use objc2_app_kit::NSApplication;
use objc2_foundation::{NSPoint, NSRect, NSSize};
use std::{cell::RefCell, process, rc::Rc, time::Instant};

type Samples = Rc<RefCell<Vec<(u128, f64, f64)>>>;

fn panel_size(mtm: MainThreadMarker) -> (f64, f64) {
    // The only panel of this process that is wider than a pill is the notch.
    NSApplication::sharedApplication(mtm)
        .windows()
        .iter()
        .map(|w| (w.frame().size.width, w.frame().size.height))
        .next()
        .unwrap_or((0.0, 0.0))
}

fn main() {
    let mtm = MainThreadMarker::new().expect("main thread");
    app::run_accessory(mtm, move || {
        let geometry = NotchGeometry {
            screen: NSRect::new(NSPoint::new(-30_000.0, 0.0), NSSize::new(1_920.0, 1_080.0)),
            notch: NSSize::new(200.0, 32.0),
            hardware: false,
        };
        let noop = AppCallbacks {
            on_query: Box::new(|_, _| {}),
            on_activate: Box::new(|_, _, _| {}),
            on_pin: Box::new(|_, _| {}),
            on_delete: Box::new(|_, _| {}),
        };
        let notch = Rc::new(Notch::new(mtm, geometry, noop, |_| {}));

        let samples: Samples = Rc::default();
        let started = Instant::now();
        let log = Rc::clone(&samples);
        let sampler = app::every(0.008, move || {
            let (w, h) = panel_size(mtm);
            log.borrow_mut().push((started.elapsed().as_millis(), w, h));
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
            for ms in [150, 250, 330, 420, 520, 700, 1_300, 1_500, 1_600, 2_300] {
                println!("  t={ms:>4} ms  height {:.1}", height_at(ms));
            }
            println!("rest {rest:.1}, peak {peak:.1}, opened {opened:.1}, closed {closed:.1}");
            if samples.len() < 150 {
                failures.push("the animation did not produce frames at a steady rate");
            }
            if rest > 10.0 {
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
