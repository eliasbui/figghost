//! Checks the mascot's stage without showing anything: the panel is placed far outside every screen and a short
//! story is played there, so the frames the screen hands the story arrive, the story ends on time, and nothing
//! goes on drawing after it. The story itself is checked in `app-ui` (`features/mascot/scene.rs`).
//! Exits with status 1 on a failure.
//! `cargo run -p macos-sys --example mascot_stage_check`

use macos_sys::{
    MainThreadMarker, NSPoint, NSRect, NSSize, app,
    mascot::{Frame, Sheet, Stage},
};
use objc2::MainThreadOnly;
use objc2_app_kit::{NSBitmapImageFileType, NSView};
use objc2_foundation::NSDictionary;
use std::{cell::Cell, process, rc::Rc};

/// The story to play, in seconds; the frames drawn after it should be none.
const SECONDS: f64 = 0.5;

/// A sheet of one small frame, drawn here: this check is about the frames of a story, not about the art, which
/// `app-ui/mascot_check` draws from the real sheets.
fn sheet(mtm: MainThreadMarker) -> Rc<Sheet> {
    let frame = NSRect::new(NSPoint::new(0.0, 0.0), NSSize::new(8.0, 8.0));
    let view = NSView::initWithFrame(NSView::alloc(mtm), frame);
    let bounds = view.bounds();
    let rep = view
        .bitmapImageRepForCachingDisplayInRect(bounds)
        .expect("a picture of an empty view");
    view.cacheDisplayInRect_toBitmapImageRep(bounds, &rep);
    // SAFETY: an empty properties dictionary is valid for PNG output.
    let png = unsafe {
        rep.representationUsingType_properties(NSBitmapImageFileType::PNG, &NSDictionary::new())
    }
    .expect("a PNG of an empty view")
    .to_vec();
    Rc::new(Sheet::from_png(&png).expect("the sheet can be read"))
}

fn main() {
    let mtm = MainThreadMarker::new().expect("main thread");
    app::run_accessory(mtm, move || {
        // A screen far outside every real one, so nothing shows; a window off every screen still takes its
        // frames from a display link, which is the path with anything to get wrong in it.
        let screen = NSRect::new(NSPoint::new(-30_000.0, 0.0), NSSize::new(1512.0, 982.0));
        let stage = Stage::new(mtm);
        let drawn = Rc::new(Cell::new(0_u32));
        let counted = Rc::clone(&drawn);
        stage.play(screen, &sheet(mtm), SECONDS, move |_| {
            counted.set(counted.get() + 1);
            Frame::default()
        });

        let ended = Rc::clone(&stage);
        let after_end = Rc::clone(&drawn);
        app::after(SECONDS + 0.4, move || {
            let mut failures = Vec::new();
            let frames = after_end.get();
            println!("frames drawn in {SECONDS} seconds: {frames}");
            if frames < 20 {
                failures.push(format!("the story was handed {frames} frames"));
            }
            if ended.playing() {
                failures.push("the story did not end".to_string());
            }
            // Nothing may draw again once the story is over: the display link goes with it.
            let quiet = Rc::clone(&after_end);
            let still = Rc::clone(&ended);
            app::after(0.4, move || {
                let later = quiet.get();
                if later != frames {
                    failures.push(format!("{later} frames drawn after the story ended"));
                }
                if still.playing() {
                    failures.push("a story started again by itself".to_string());
                }
                for f in &failures {
                    println!("FAIL: {f}");
                }
                println!(
                    "{}",
                    if failures.is_empty() {
                        "stage ok"
                    } else {
                        "stage broken"
                    }
                );
                process::exit(i32::from(!failures.is_empty()));
            });
        });
        (stage,)
    });
}
