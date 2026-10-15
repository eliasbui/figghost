//! Draws the mascot frame by frame into PNG files, without a screen: every character fetching a copy, and
//! guarding or missing a file dragged out of the shelf, at the moments that tell the story. The story is the
//! app's own (`src/features/mascot/scene.rs`), and so is the drawing (`macos_sys::mascot`).
//!
//!   cargo run -p app-ui --example mascot_check -- [out dir, default /tmp/mascot-check]

#![allow(dead_code)]

#[path = "../src/features/mascot/cast.rs"]
mod cast;
#[path = "../src/features/mascot/scene.rs"]
mod scene;

use cast::Character;
use macos_sys::{
    MainThreadMarker, NSPoint, NSRect, NSSize,
    mascot::{Chip, Sheet, Stage},
};
use scene::{Farewell, Follow, Place};
use std::{path::PathBuf, rc::Rc};

fn main() {
    let out = PathBuf::from(
        std::env::args()
            .nth(1)
            .unwrap_or_else(|| "/tmp/mascot-check".into()),
    );
    std::fs::create_dir_all(&out).expect("the output folder can be made");
    let mtm = MainThreadMarker::new().expect("runs on the main thread");
    // A 1512 x 982 screen with a 200 x 32 notch, like a 14 inch MacBook Pro; the pictures show the part of
    // it between the notch and the things the mascot goes for.
    let place = Place {
        hole: NSRect::new(NSPoint::new(656.0, 950.0), NSSize::new(200.0, 32.0)),
        screen: NSRect::new(NSPoint::new(0.0, 0.0), NSSize::new(1512.0, 982.0)),
    };
    let area = NSRect::new(NSPoint::new(306.0, 482.0), NSSize::new(900.0, 500.0));
    let backdrop = 0x1D2B53;
    let mut written = 0;
    let mut write = |name: String, png: Option<Vec<u8>>| {
        let png = png.unwrap_or_else(|| panic!("{name} could not be drawn"));
        std::fs::write(out.join(format!("{name}.png")), png).expect("the picture can be written");
        written += 1;
    };
    for c in Character::ALL {
        let sheet = Rc::new(Sheet::from_png(c.png()).expect("every sheet can be read"));
        let style = c.style();
        let fetch = scene::fetch(style, place, NSPoint::new(430.0, 600.0), Chip::Text);
        for t in [0.1, 0.3, 0.42, 0.6, 0.8, 1.0, 1.15, 1.3, 1.55] {
            write(
                format!("{}-fetch-{t:.2}", c.as_str()),
                Stage::render_png(mtm, &sheet, fetch.at(t), area, backdrop),
            );
        }
        for farewell in [Farewell::Guard, Farewell::Regret] {
            let name = match farewell {
                Farewell::Guard => "guard",
                Farewell::Regret => "regret",
            };
            // A file dragged from the notch down and to the left over a second and a half.
            let mut follow = Follow::new(style, place, farewell, NSPoint::new(756.0, 900.0));
            let mut frame = Default::default();
            for i in 0..=90 {
                let t = f64::from(i) / 60.0;
                let f = (t / 1.5).min(1.0);
                follow.moved(NSPoint::new(756.0 - 250.0 * f, 900.0 - 330.0 * f));
                frame = follow.frame(t);
                if i == 30 || i == 90 {
                    write(
                        format!("{}-{name}-drag-{t:.2}", c.as_str()),
                        Stage::render_png(mtm, &sheet, frame.clone(), area, backdrop),
                    );
                }
            }
            drop(frame);
            let finale = follow.finale(true);
            for t in [0.08, 0.3, 0.5, 0.9, 1.3] {
                write(
                    format!("{}-{name}-end-{t:.2}", c.as_str()),
                    Stage::render_png(mtm, &sheet, finale.at(t), area, backdrop),
                );
            }
        }
    }
    println!("wrote {written} pictures to {}", out.display());
}
