//! Draws the Pomodoro tab of the open notch into PNGs with sample figures, in both languages, without showing anything
//! (run it with FIGGHOST_NOTCH_OFFSCREEN=1), and checks that the buttons tell the app what was pressed.
//! `FIGGHOST_NOTCH_OFFSCREEN=1 cargo run -p macos-sys --example render_pomodoro -- /tmp/pomodoro`

use macos_sys::{
    MainThreadMarker, app,
    notch::{Notch, NotchGeometry},
    pomodoro_ui::{Hue, Labels, PAGE_SIZE, PomodoroCallbacks, PomodoroDisplay, PomodoroView},
    ui::AppCallbacks,
};
use objc2::{msg_send, rc::Retained};
use objc2_app_kit::NSView;
use std::{cell::RefCell, env, fs, path::PathBuf, process, rc::Rc};

fn labels(vi: bool) -> Labels {
    if vi {
        Labels {
            start: "Bắt đầu".into(),
            pause: "Tạm dừng".into(),
            resume: "Tiếp tục".into(),
            skip: "Chuyển sang giai đoạn kế".into(),
            reset: "Bắt đầu lại từ phiên đầu".into(),
        }
    } else {
        Labels {
            start: "Start".into(),
            pause: "Pause".into(),
            resume: "Resume".into(),
            skip: "Skip to the next stage".into(),
            reset: "Start over from the first session".into(),
        }
    }
}

fn scene(vi: bool, hue: Hue, progress: f64, running: bool, paused: bool) -> PomodoroDisplay {
    let stage = match (vi, hue) {
        (false, Hue::Work) => "Focus",
        (false, Hue::ShortBreak) => "Short break",
        (false, Hue::LongBreak) => "Long break",
        (true, Hue::Work) => "Tập trung",
        (true, Hue::ShortBreak) => "Nghỉ ngắn",
        (true, Hue::LongBreak) => "Nghỉ dài",
    };
    let total = match hue {
        Hue::Work => 25.0 * 60.0,
        Hue::ShortBreak => 5.0 * 60.0,
        Hue::LongBreak => 15.0 * 60.0,
    };
    let left = (total * (1.0 - progress)).round() as u64;
    PomodoroDisplay {
        hue,
        stage: stage.into(),
        clock: format!("{:02}:{:02}", left / 60, left % 60),
        status: match (vi, running, paused) {
            (false, true, _) => "Running",
            (false, _, true) => "Paused",
            (false, _, _) => "Ready",
            (true, true, _) => "Đang chạy",
            (true, _, true) => "Tạm dừng",
            (true, _, _) => "Sẵn sàng",
        }
        .into(),
        progress,
        running,
        paused,
        round: if vi {
            "Phiên 3 trên 4".into()
        } else {
            "Session 3 of 4".into()
        },
        dots: (2, 4),
        dot_active: hue == Hue::Work && (running || paused),
        today_caption: if vi {
            "Hôm nay".into()
        } else {
            "Today".into()
        },
        today: if vi {
            "5 phiên, 125 phút".into()
        } else {
            "5 sessions, 125 min".into()
        },
        streak_caption: if vi {
            "Chuỗi ngày".into()
        } else {
            "Streak".into()
        },
        streak: if vi {
            "6 ngày".into()
        } else {
            "6 days".into()
        },
        week: [0.4, 0.8, 0.0, 1.0, 0.6, 0.2, 0.7],
        week_labels: if vi {
            ["T4", "T5", "T6", "T7", "CN", "T2", "T3"].map(String::from)
        } else {
            ["W", "T", "F", "S", "S", "M", "T"].map(String::from)
        },
        labels: labels(vi),
        note: None,
    }
}

fn main() {
    let mtm = MainThreadMarker::new().expect("main thread");
    let dir = PathBuf::from(env::args().nth(1).unwrap_or_else(|| "/tmp/pomodoro".into()));
    fs::create_dir_all(&dir).expect("output directory");
    app::run_accessory(mtm, move || {
        let geometry = NotchGeometry::detect(mtm).expect("a screen");
        let noop = AppCallbacks {
            on_query: Box::new(|_, _| {}),
            on_activate: Box::new(|_, _, _| {}),
            on_pin: Box::new(|_, _| {}),
            on_delete: Box::new(|_, _| {}),
        };
        let notch = Rc::new(Notch::new(mtm, geometry, noop, |view| {
            view.set_rows(Vec::new(), "")
        }));
        let log: Rc<RefCell<Vec<&'static str>>> = Rc::default();
        let (toggle, skip, reset) = (Rc::clone(&log), Rc::clone(&log), Rc::clone(&log));
        let view = PomodoroView::new(
            mtm,
            notch.page_area(PAGE_SIZE.width, PAGE_SIZE.height),
            PomodoroCallbacks {
                on_toggle: Box::new(move || toggle.borrow_mut().push("toggle")),
                on_skip: Box::new(move || skip.borrow_mut().push("skip")),
                on_reset: Box::new(move || reset.borrow_mut().push("reset")),
            },
        );
        let index = notch.page_count();
        let page = view.page("Pomodoro".into(), || {}, || {});
        let root: Retained<NSView> = page.root.clone();
        notch.add_page(page);
        let (shot, shown) = (Rc::clone(&notch), Rc::clone(&view));
        app::after(0.4, move || {
            // The buttons, pressed the way an assistive tool presses them, tell the app which one it was.
            shown.show(scene(false, Hue::Work, 0.4, true, false));
            let buttons: Vec<Retained<NSView>> =
                root.subviews().iter().filter(|v| !v.isHidden()).collect();
            assert_eq!(buttons.len(), 3, "a button for start, skip and reset");
            for button in &buttons {
                // SAFETY: accessibilityPerformPress takes no arguments and returns whether it pressed.
                let pressed: bool = unsafe { msg_send![&**button, accessibilityPerformPress] };
                assert!(pressed, "the button took the press");
            }
            assert_eq!(*log.borrow(), ["toggle", "skip", "reset"]);
            println!("ok      each button tells the app which one it was");

            let scenes = vec![
                ("ready_en", scene(false, Hue::Work, 0.0, false, false)),
                ("running_en", scene(false, Hue::Work, 0.62, true, false)),
                ("paused_vi", scene(true, Hue::Work, 0.3, false, true)),
                (
                    "short_break_en",
                    scene(false, Hue::ShortBreak, 0.4, true, false),
                ),
                (
                    "long_break_en",
                    scene(false, Hue::LongBreak, 0.8, true, false),
                ),
                (
                    "off_en",
                    PomodoroDisplay {
                        note: Some((
                            "The Pomodoro tab is off".into(),
                            "Turn it on in Settings, under Notch tabs, Pomodoro.".into(),
                        )),
                        ..PomodoroDisplay::default()
                    },
                ),
            ];
            for (name, display) in scenes {
                shown.show(display);
                let png = shot.snapshot_png(index).expect("the page");
                let path = dir.join(format!("{name}.png"));
                fs::write(&path, png).expect("write png");
                println!("wrote {}", path.display());
            }
            process::exit(0);
        });
        (notch, view)
    });
}
