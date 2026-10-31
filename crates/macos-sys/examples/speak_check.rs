//! Says one sentence in each language through the speech synthesizer and reports whether it started and ended.
//! `cargo run -p macos-sys --example speak_check`

use macos_sys::{MainThreadMarker, app, voice_out::{Speaker, language_of}};
use std::{cell::Cell, process, rc::Rc, time::Instant};

fn main() {
    let mtm = MainThreadMarker::new().expect("main thread");
    app::run_accessory(mtm, move || {
        let speaker = Rc::new(Speaker::new());
        for text in ["Hello.", "Xin chào bạn."] {
            speaker.say(text, language_of(text));
        }
        let started = Rc::new(Cell::new(false));
        let began = Instant::now();
        let (s, st) = (speaker.clone(), started.clone());
        let timer = app::every(0.1, move || {
            if s.is_speaking() {
                st.set(true);
            }
            let finished = st.get() && !s.is_speaking();
            if finished || began.elapsed().as_secs() > 12 {
                println!("started {}, finished {finished}, after {:.1}s", st.get(), began.elapsed().as_secs_f64());
                process::exit(if finished { 0 } else { 1 });
            }
        });
        std::mem::forget(timer);
    });
}
