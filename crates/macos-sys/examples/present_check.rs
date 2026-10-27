//! Runs a real presentation for a few seconds: prints where the displays are, then shows the display under the pointer
//! in a window of its own, zooms into its middle, lights the spotlight and ripples, and says whether frames arrived.
//! It needs Screen Recording access for the terminal and an unlocked screen; it posts no input and takes no focus
//! beyond its own window. `cargo run -p macos-sys --example present_check -- 5`

use macos_sys::{
    MainThreadMarker, app, display_layout,
    presenter::{Cover, Look, Options, Presentation, Source, Stage},
    screen_capture,
};
use std::{cell::RefCell, env, process, rc::Rc};

fn main() {
    let mtm = MainThreadMarker::new().expect("main thread");
    let seconds: f64 = env::args()
        .nth(1)
        .and_then(|s| s.parse().ok())
        .unwrap_or(5.0);
    app::run_accessory(mtm, move || {
        let layout = display_layout::displays(mtm);
        for d in &layout {
            println!(
                "display {} {:?} at {:?}{}{}{}",
                d.id,
                d.name,
                d.frame,
                if d.main { ", main" } else { "" },
                if d.built_in { ", built in" } else { "" },
                d.mirror_of
                    .map(|m| format!(", mirrors {m}"))
                    .unwrap_or_default()
            );
        }
        if !screen_capture::screen_access(false) {
            eprintln!("no Screen Recording access for this terminal: nothing to present");
            process::exit(2);
        }
        let pointer = screen_capture::pointer();
        let under = layout
            .iter()
            .find(|d| {
                let (x, _, w, _) = d.frame;
                pointer.x >= x && pointer.x < x + w
            })
            .or(layout.first())
            .map(|d| d.id)
            .expect("a display");
        let held: Rc<RefCell<Option<Presentation>>> = Rc::default();
        let keep = Rc::clone(&held);
        Presentation::start(
            mtm,
            Source::Display(under),
            Stage::Window,
            Options {
                fps: 30,
                cursor: true,
                border: true,
                title: "present_check".into(),
            },
            move |result| match result {
                Ok(presentation) => {
                    println!("presenting display {under} in a window");
                    presentation.show(Look {
                        view: (0.25, 0.25, 0.5, 0.5),
                        pointer: Some((0.5, 0.5)),
                        ring: true,
                        spotlight: Some(0.18),
                        cover: Cover::None,
                    });
                    presentation.ripple((0.5, 0.5));
                    // The stage and the frame, for `screencapture -l` to look at once they are on screen.
                    app::after(1.5, || {
                        let me = std::process::id() as i32;
                        for w in macos_sys::pin_ui::on_screen_windows()
                            .iter()
                            .filter(|w| w.pid == me)
                        {
                            println!("window {} at {:?}", w.id, w.frame);
                        }
                    });
                    *keep.borrow_mut() = Some(presentation);
                    let done = Rc::clone(&keep);
                    app::after(seconds, move || {
                        let got = done
                            .borrow()
                            .as_ref()
                            .is_some_and(Presentation::has_picture);
                        if let Some(p) = done.borrow_mut().take() {
                            p.stop();
                        }
                        if got {
                            println!("ok      frames reached the stage");
                            process::exit(0);
                        }
                        eprintln!("FAIL    no frame reached the stage (is the screen locked?)");
                        process::exit(1);
                    });
                }
                Err(e) => {
                    eprintln!("FAIL    cannot present: {e}");
                    process::exit(1);
                }
            },
            |ended| {
                eprintln!("ended by itself: {ended:?}");
                process::exit(1);
            },
        );
        held
    });
}
