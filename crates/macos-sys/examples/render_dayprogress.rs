//! Draws the Day progress tab of the open notch into PNGs with a sample day, in both languages, without showing anything:
//! run it with FIGGHOST_NOTCH_OFFSCREEN=1.
//! `FIGGHOST_NOTCH_OFFSCREEN=1 cargo run -p macos-sys --example render_dayprogress -- /tmp/dayprogress`

use macos_sys::{
    MainThreadMarker, app,
    dayprogress_ui::{Block, DayDisplay, DayView, Mark, PAGE_SIZE, Row},
    notch::{Notch, NotchGeometry},
    ui::AppCallbacks,
};
use std::{env, fs, path::PathBuf, process, rc::Rc};

/// The line runs from 7:00 to 23:00, sixteen hours.
fn at(hour: f64, minute: f64) -> f64 {
    (hour + minute / 60.0 - 7.0) / 16.0
}

fn scene(vi: bool, now: f64) -> DayDisplay {
    let t = |en: &'static str, vi_text: &'static str| {
        if vi {
            vi_text.to_owned()
        } else {
            en.to_owned()
        }
    };
    let block = |from: (f64, f64), to: (f64, f64), lane, color| Block {
        start: at(from.0, from.1),
        end: at(to.0, to.1),
        lane,
        color,
        running: now >= at(from.0, from.1) && now < at(to.0, to.1),
    };
    DayDisplay {
        percent: format!("{:.0}%", now * 100.0),
        caption: t("of the day has gone", "của ngày đã trôi qua"),
        left: if vi {
            "còn 8 giờ 40 phút".into()
        } else {
            "8 h 40 min left".into()
        },
        now,
        ticks: [
            (7, "7 AM", "7h"),
            (10, "10 AM", "10h"),
            (13, "1 PM", "13h"),
            (16, "4 PM", "16h"),
            (19, "7 PM", "19h"),
            (22, "10 PM", "22h"),
        ]
        .iter()
        .map(|(hour, en, vi_text)| {
            (
                (if vi { *vi_text } else { *en }).to_owned(),
                at(f64::from(*hour), 0.0),
            )
        })
        .collect(),
        blocks: vec![
            block((9.0, 0.0), (9.0, 30.0), 0, 0x3478f6),
            block((11.0, 0.0), (12.0, 0.0), 0, 0x30d158),
            block((12.0, 30.0), (13.0, 30.0), 1, 0xff9f0a),
            block((14.0, 0.0), (16.0, 0.0), 0, 0xbf5af2),
            block((15.0, 0.0), (15.0, 45.0), 1, 0xff453a),
            block((17.0, 0.0), (17.0, 30.0), 0, 0x3478f6),
            block((20.0, 0.0), (20.0, 30.0), 2, 0x64d2ff),
        ],
        marks: vec![
            Mark {
                at: at(10.0, 30.0),
                color: 0xffd60a,
            },
            Mark {
                at: at(16.0, 30.0),
                color: 0xffd60a,
            },
            Mark {
                at: at(19.0, 0.0),
                color: 0xffd60a,
            },
        ],
        agenda: vec![
            Row {
                color: 0xbf5af2,
                title: t("Focus time", "Giờ tập trung"),
                when: t("now, until 4 PM", "đang diễn ra, đến 16h"),
                running: true,
            },
            Row {
                color: 0xff453a,
                title: t("Client call", "Gọi khách hàng"),
                when: t("in 40 min", "sau 40 phút"),
                running: false,
            },
            Row {
                color: 0x3478f6,
                title: "1:1".to_owned(),
                when: t("5:00 PM", "17:00"),
                running: false,
            },
        ],
        empty: t("Nothing more today", "Hôm nay hết việc rồi"),
        summary: if vi {
            "4 sự kiện nữa, 2 nhắc việc còn lại".into()
        } else {
            "4 events to go, 2 reminders left".into()
        },
        note: None,
    }
}

fn main() {
    let mtm = MainThreadMarker::new().expect("main thread");
    let dir = PathBuf::from(
        env::args()
            .nth(1)
            .unwrap_or_else(|| "/tmp/dayprogress".into()),
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
        let view = DayView::new(mtm, notch.page_area(PAGE_SIZE.width, PAGE_SIZE.height));
        let index = notch.page_count();
        notch.add_page(view.page("Day progress".into(), || {}, || {}));
        let (shot, shown) = (Rc::clone(&notch), Rc::clone(&view));
        app::after(0.4, move || {
            let mut scenes = vec![
                ("afternoon_en", scene(false, at(14.0, 20.0))),
                ("afternoon_vi", scene(true, at(14.0, 20.0))),
                ("morning_en", scene(false, at(8.0, 5.0))),
                ("evening_en", scene(false, at(19.0, 30.0))),
            ];
            for (name, title, hint) in [
                (
                    "access_en",
                    "The calendar is not allowed",
                    "Allow it in Settings, under Notch tabs, Calendar.",
                ),
                (
                    "off_vi",
                    "Tab Tiến độ ngày đang tắt",
                    "Bật trong Cài đặt, mục Tab của notch, Tiến độ ngày.",
                ),
            ] {
                scenes.push((
                    name,
                    DayDisplay {
                        note: Some((title.into(), hint.into())),
                        ..DayDisplay::default()
                    },
                ));
            }
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
