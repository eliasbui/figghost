//! Checks that the open notch changes size with its page: it opens at the size of the clipboard strip, moves to the
//! short Media page when that is chosen, grows to the full page when lyrics arrive and shrinks again when they go,
//! and closes. The shape is sampled as the window server draws it, outside every screen. Exits with status 1 on a
//! failure.
//! `cargo run -p macos-sys --example page_check`

use macos_sys::{
    MainThreadMarker, app,
    media_ui::{Lyrics, MediaCallbacks, MediaView},
    notch::{Notch, NotchGeometry, Style},
    ui::AppCallbacks,
};
use objc2_foundation::{NSPoint, NSRect, NSSize};
use std::{cell::RefCell, process, rc::Rc, time::Instant};

type Samples = Rc<RefCell<Vec<(u128, f64, f64)>>>;

fn main() {
    let mtm = MainThreadMarker::new().expect("main thread");
    app::run_accessory(mtm, move || {
        let geometry = NotchGeometry {
            screen: NSRect::new(NSPoint::new(-30_000.0, 0.0), NSSize::new(1_920.0, 1_080.0)),
            notch: NSSize::new(200.0, 24.0),
            hardware: false,
        };
        let noop = AppCallbacks {
            on_query: Box::new(|_, _| {}),
            on_activate: Box::new(|_, _, _| {}),
            on_pin: Box::new(|_, _| {}),
            on_delete: Box::new(|_, _| {}),
        };
        let notch = Rc::new(Notch::new(mtm, geometry, noop, |_| {}));
        notch.configure(&Style::default());
        let media = MediaView::new(
            mtm,
            notch.page_frame(),
            MediaCallbacks {
                on_previous: Box::new(|| {}),
                on_play_pause: Box::new(|| {}),
                on_next: Box::new(|| {}),
                on_seek: Box::new(|_| {}),
            },
        );
        notch.add_page(media.page(|| {}, || {}));

        let samples: Samples = Rc::default();
        let started = Instant::now();
        let (log, watched) = (Rc::clone(&samples), Rc::clone(&notch));
        let sampler = app::every(0.016, move || {
            let size = watched.visible_size();
            log.borrow_mut()
                .push((started.elapsed().as_millis(), size.width, size.height));
        });
        let opener = Rc::clone(&notch);
        app::after(0.2, move || opener.open());
        // The size of the clipboard strip, which is what the notch opened at.
        let strip = Rc::new(std::cell::Cell::new(NSSize::new(0.0, 0.0)));
        let (asked, noted) = (Rc::clone(&notch), Rc::clone(&strip));
        app::after(1.0, move || noted.set(asked.open_size()));
        let chooser = Rc::clone(&notch);
        app::after(1.2, move || chooser.open_page(1));
        let with_lyrics = Rc::clone(&media);
        app::after(2.2, move || {
            with_lyrics.show_lyrics(&Lyrics::Message {
                title: "Lyrics",
                detail: "None for this track",
            });
        });
        let without = Rc::clone(&media);
        app::after(3.2, move || without.show_lyrics(&Lyrics::Hidden));
        let closer = Rc::clone(&notch);
        app::after(4.2, move || closer.close());
        let log = Rc::clone(&samples);
        app::after(5.0, move || {
            let strip = strip.get();
            drop(sampler);
            let samples = log.borrow();
            let at = |ms: u128| {
                samples
                    .iter()
                    .min_by_key(|s| s.0.abs_diff(ms))
                    .map_or((0.0, 0.0), |s| (s.1, s.2))
            };
            let mut failures = Vec::new();
            let mut expect = |ms: u128, what: &str, want: (f64, f64)| {
                let got = at(ms);
                println!("  t={ms:>4} ms  {:>6.1} x {:>6.1}  {what}", got.0, got.1);
                if (got.0 - want.0).abs() > 1.0 || (got.1 - want.1).abs() > 1.0 {
                    failures.push(format!("{what}: {got:?}, expected {want:?}"));
                }
            };
            expect(1_100, "the clipboard strip", (strip.width, strip.height));
            expect(2_100, "the short Media page", (500.0, 218.0));
            expect(2_900, "the Media page with lyrics", (500.0, 440.0));
            expect(3_900, "the short Media page again", (500.0, 218.0));
            expect(4_900, "closed", (200.0, 24.0));
            // Between two pages the shape is on its way, not at one size or the other.
            let (w, h) = at(1_290);
            if w >= strip.width - 20.0 || w <= 500.0 + 20.0 || h <= 218.0 {
                failures.push(format!(
                    "the move between pages was not spread over time: {w} x {h}"
                ));
            }
            for failure in &failures {
                println!("FAIL: {failure}");
            }
            println!(
                "{}",
                if failures.is_empty() {
                    "pages ok"
                } else {
                    "pages broken"
                }
            );
            process::exit(i32::from(!failures.is_empty()));
        });
        (notch, media)
    });
}
