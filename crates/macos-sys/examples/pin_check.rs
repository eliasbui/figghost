//! Keeps one window of another process on top the way the app does, to look at the result: a red border round it,
//! and a picture of it, refreshed while it is covered, over whatever covers it.
//!
//!   pin_check <window number> <seconds>
//!
//! The window number is the one the window server gives (`kCGWindowNumber`). Needs Screen Recording for the copy.

use core_window::{
    Rect,
    pin::{self, Listed, State},
};
use macos_sys::{
    MainThreadMarker, app,
    pin_ui::{self, PinPanel, PinShots},
    window::{Frame, Window},
};
use std::{cell::RefCell, env, process, rc::Rc, time::Duration};

fn frame(r: Rect) -> Frame {
    Frame {
        x: r.x,
        y: r.y,
        w: r.w,
        h: r.h,
    }
}

fn main() {
    let mtm = MainThreadMarker::new().expect("main thread");
    let args: Vec<String> = env::args().skip(1).collect();
    let id: u32 = args
        .first()
        .and_then(|a| a.parse().ok())
        .expect("a window number");
    let seconds: f64 = args.get(1).and_then(|a| a.parse().ok()).unwrap_or(10.0);
    app::run_accessory(mtm, move || {
        let panel = Rc::new(PinPanel::new(mtm, move || {
            // The first window of the process stands in for the pinned one, which is enough for a test window.
            let pid = pin_ui::on_screen_windows()
                .iter()
                .find(|w| w.id == id)
                .map(|w| w.pid);
            let raised = pid.and_then(Window::first_of).is_some_and(|w| w.raise());
            println!("the copy was pressed, raised: {raised}");
        }));
        let shots: Rc<RefCell<Option<PinShots>>> = Rc::default();
        {
            let shots = Rc::clone(&shots);
            PinShots::prepare(id, move |prepared| match prepared {
                Ok(p) => {
                    println!("pictures can be taken");
                    *shots.borrow_mut() = Some(p);
                }
                Err(why) => println!("no pictures: {why}"),
            });
        }
        let own = process::id() as i32;
        let last = Rc::new(RefCell::new(String::new()));
        let timer = {
            let (panel, shots) = (Rc::clone(&panel), Rc::clone(&shots));
            app::every(0.066, move || {
                let started = std::time::Instant::now();
                let list = pin_ui::on_screen_windows();
                let took = started.elapsed();
                if std::env::var_os("PIN_CHECK_TIMES").is_some() {
                    println!("list of {} windows in {took:?}", list.len());
                }
                let listed: Vec<Listed> = list
                    .into_iter()
                    .map(|w| Listed {
                        id: w.id,
                        pid: w.pid,
                        layer: w.layer,
                        frame: Rect::new(w.frame.x, w.frame.y, w.frame.w, w.frame.h),
                        alpha: w.alpha,
                    })
                    .collect();
                let exists = listed.iter().any(|w| w.id == id) || pin_ui::window_exists(id);
                let state = pin::state(&listed, exists, id, own);
                let text = format!("{state:?}");
                if *last.borrow() != text {
                    println!("{text}");
                    *last.borrow_mut() = text;
                }
                let every = match state {
                    State::Covered(_) => Duration::from_millis(60),
                    _ => Duration::from_millis(500),
                };
                if let Some(shots) = &*shots.borrow() {
                    if !matches!(state, State::Gone | State::Away) {
                        shots.capture(&panel.copy_layer(), every);
                    }
                }
                match state {
                    State::Gone => process::exit(0),
                    State::Away => panel.hide(),
                    State::Clear(f) => panel.show(frame(f), false, true),
                    State::Covered(f) => panel.show(frame(f), true, true),
                }
            })
        };
        app::after(seconds, || process::exit(0));
        (panel, shots, timer)
    });
}
