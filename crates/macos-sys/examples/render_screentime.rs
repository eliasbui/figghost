//! Draws the Screen Time tab of the open notch into PNGs with sample figures and real app icons, in both languages,
//! without showing anything: run it with FIGGHOST_NOTCH_OFFSCREEN=1.
//! `FIGGHOST_NOTCH_OFFSCREEN=1 cargo run -p macos-sys --example render_screentime -- /tmp/screentime`

use macos_sys::{
    MainThreadMarker, app,
    notch::{Notch, NotchGeometry},
    screentime_ui::{Icon, PAGE_SIZE, Row, ScreenTimeCallbacks, ScreenTimeDisplay, ScreenTimeView},
    ui::AppCallbacks,
};
use std::{cell::RefCell, env, fs, path::PathBuf, process, rc::Rc};

fn app_row(path: &str, title: &str, value: &str, fraction: f64) -> Row {
    Row {
        icon: Icon::App(PathBuf::from(path)),
        title: title.into(),
        value: value.into(),
        fraction,
    }
}

fn symbol_row(symbol: &'static str, title: &str, value: &str, fraction: f64) -> Row {
    Row {
        icon: Icon::Symbol(symbol),
        title: title.into(),
        value: value.into(),
        fraction,
    }
}

fn apps(vi: bool) -> ScreenTimeDisplay {
    let (hours, min) = if vi { ("giờ", "phút") } else { ("h", "min") };
    let value = |h: u32, m: u32| {
        if h > 0 {
            format!("{h} {hours} {m} {min}")
        } else {
            format!("{m} {min}")
        }
    };
    ScreenTimeDisplay {
        caption: if vi {
            "Hôm nay".into()
        } else {
            "Today".into()
        },
        total: value(5, 41),
        rows: vec![
            app_row("/Applications/Xcode.app", "Xcode", &value(2, 12), 1.0),
            app_row(
                "/System/Applications/Utilities/Terminal.app",
                "Terminal",
                &value(1, 31),
                0.69,
            ),
            app_row(
                "/System/Applications/Safari.app",
                "Safari",
                &value(1, 4),
                0.49,
            ),
            app_row("/System/Applications/Mail.app", "Mail", &value(0, 28), 0.21),
            app_row(
                "/System/Applications/Notes.app",
                "Notes",
                &value(0, 17),
                0.13,
            ),
            app_row(
                "/System/Applications/Music.app",
                "Music",
                &value(0, 9),
                0.07,
            ),
        ],
        empty: String::new(),
        ranges: if vi {
            ["Hôm nay".into(), "7 ngày".into()]
        } else {
            ["Today".into(), "7 days".into()]
        },
        groups: if vi {
            ["Ứng dụng".into(), "Nhóm".into()]
        } else {
            ["Apps".into(), "Groups".into()]
        },
        range: 0,
        group: 0,
        note: None,
    }
}

fn groups(vi: bool) -> ScreenTimeDisplay {
    let (hours, min) = if vi { ("giờ", "phút") } else { ("h", "min") };
    let value = |h: u32, m: u32| {
        if h > 0 {
            format!("{h} {hours} {m} {min}")
        } else {
            format!("{m} {min}")
        }
    };
    ScreenTimeDisplay {
        caption: if vi {
            "7 ngày qua".into()
        } else {
            "Last 7 days".into()
        },
        total: value(31, 12),
        rows: vec![
            symbol_row(
                "hammer",
                if vi {
                    "Công cụ lập trình"
                } else {
                    "Developer tools"
                },
                &value(14, 3),
                1.0,
            ),
            symbol_row(
                "checkmark.circle",
                if vi { "Năng suất" } else { "Productivity" },
                &value(8, 40),
                0.62,
            ),
            symbol_row(
                "wrench.and.screwdriver",
                if vi { "Tiện ích" } else { "Utilities" },
                &value(5, 2),
                0.36,
            ),
            symbol_row(
                "bubble.left.and.bubble.right",
                if vi {
                    "Mạng xã hội"
                } else {
                    "Social networking"
                },
                &value(2, 15),
                0.16,
            ),
            symbol_row(
                "questionmark.circle",
                if vi { "Khác" } else { "Other" },
                &value(1, 12),
                0.09,
            ),
        ],
        range: 1,
        group: 1,
        ..apps(vi)
    }
}

fn main() {
    let mtm = MainThreadMarker::new().expect("main thread");
    let dir = PathBuf::from(
        env::args()
            .nth(1)
            .unwrap_or_else(|| "/tmp/screentime".into()),
    );
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
        let log: Rc<RefCell<Vec<String>>> = Rc::default();
        let (range, group) = (Rc::clone(&log), Rc::clone(&log));
        let view = ScreenTimeView::new(
            mtm,
            notch.page_area(PAGE_SIZE.width, PAGE_SIZE.height),
            ScreenTimeCallbacks {
                on_range: Box::new(move |i| range.borrow_mut().push(format!("range {i}"))),
                on_group: Box::new(move |i| group.borrow_mut().push(format!("group {i}"))),
            },
        );
        let index = notch.page_count();
        notch.add_page(view.page("Screen Time".into(), || {}, || {}));
        let (shot, shown) = (Rc::clone(&notch), Rc::clone(&view));
        app::after(0.4, move || {
            let scenes = vec![
                ("apps_en", apps(false)),
                ("groups_en", groups(false)),
                ("apps_vi", apps(true)),
                (
                    "empty_en",
                    ScreenTimeDisplay {
                        total: "0 min".into(),
                        rows: Vec::new(),
                        empty: "Nothing yet today. It counts the app in front while you are at the Mac.".into(),
                        ..apps(false)
                    },
                ),
                (
                    "off_en",
                    ScreenTimeDisplay {
                        note: Some(("The Screen Time tab is off".into(), "Turn it on in Settings, under Notch tabs, Screen Time.".into())),
                        ..ScreenTimeDisplay::default()
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
