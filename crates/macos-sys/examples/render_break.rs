//! Draws the card of a break into PNGs in both languages without showing anything, and presses its two buttons the way
//! an assistive tool does to check that the app is told which.
//! `cargo run -p macos-sys --example render_break -- /tmp/break`

use macos_sys::{
    MainThreadMarker, app,
    break_ui::{BreakDisplay, BreakScreen},
};
use std::{cell::RefCell, env, fs, path::PathBuf, process, rc::Rc};

fn main() {
    let mtm = MainThreadMarker::new().expect("main thread");
    let dir = PathBuf::from(env::args().nth(1).unwrap_or_else(|| "/tmp/break".into()));
    fs::create_dir_all(&dir).expect("output directory");
    app::run_accessory(mtm, move || {
        let pressed: Rc<RefCell<Vec<&str>>> = Rc::default();
        let (skip, later) = (Rc::clone(&pressed), Rc::clone(&pressed));
        let screen = BreakScreen::new(
            mtm,
            move || skip.borrow_mut().push("skip"),
            move || later.borrow_mut().push("later"),
        );
        let scenes = [
            (
                "break_en",
                BreakDisplay {
                    title: "Look into the distance".into(),
                    hint:
                        "Rest your eyes on something about six metres away, and blink a few times."
                            .into(),
                    left: "0:18".into(),
                    skip: "Skip".into(),
                    later: "In 5 minutes".into(),
                },
            ),
            (
                "break_vi",
                BreakDisplay {
                    title: "Nhìn ra xa".into(),
                    hint: "Nhìn một vật cách bạn chừng sáu mét, và chớp mắt vài lần.".into(),
                    left: "4:52".into(),
                    skip: "Bỏ qua".into(),
                    later: "Để 5 phút nữa".into(),
                },
            ),
        ];
        for (name, display) in &scenes {
            let png = screen.card_png(display).expect("the card");
            let path = dir.join(format!("{name}.png"));
            fs::write(&path, png).expect("write png");
            println!("wrote {}", path.display());
        }
        let both = screen.press("skip") && screen.press("later") && !screen.press("other");
        let told = pressed.borrow().clone();
        if both && told == ["skip", "later"] {
            println!("ok: the buttons say which was pressed");
        } else {
            eprintln!("FAILED: pressed {told:?}");
            process::exit(1);
        }
        process::exit(0);
        #[allow(unreachable_code)]
        screen
    });
}
