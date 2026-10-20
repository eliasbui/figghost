//! Draws the Devices tab of the open notch into PNGs with sample devices, in both languages, without showing anything:
//! run it with FIGGHOST_NOTCH_OFFSCREEN=1.
//! `FIGGHOST_NOTCH_OFFSCREEN=1 cargo run -p macos-sys --example render_devices -- /tmp/devices`

use macos_sys::{
    MainThreadMarker, app,
    devices_ui::{Card, DevicesDisplay, DevicesView, PAGE_SIZE, Reading},
    notch::{Notch, NotchGeometry},
    ui::AppCallbacks,
};
use std::{env, fs, path::PathBuf, process, rc::Rc};

fn reading(label: &str, percent: u8, low: bool) -> Reading {
    Reading {
        label: label.to_owned(),
        percent,
        low,
    }
}

fn card(symbol: &'static str, name: &str, readings: Vec<Reading>, note: &str) -> Card {
    Card {
        symbol,
        name: name.to_owned(),
        low: readings.iter().any(|r| r.low),
        readings,
        note: note.to_owned(),
    }
}

fn scene(vi: bool, count: usize) -> DevicesDisplay {
    let t = |en: &'static str, vi_text: &'static str| if vi { vi_text } else { en };
    let all = vec![
        card(
            "headphones",
            t("Sample Headphones", "Tai nghe mẫu"),
            vec![reading("", 100, false)],
            "",
        ),
        card(
            "computermouse.fill",
            t("Sample Mouse", "Chuột mẫu"),
            vec![reading("", 18, true)],
            t("Low", "Pin yếu"),
        ),
        card(
            "keyboard",
            t("Sample Keyboard", "Bàn phím mẫu"),
            vec![reading("", 64, false)],
            "",
        ),
        card(
            "headphones",
            t("Sample Earbuds", "Tai nghe nhét tai mẫu"),
            vec![
                reading("L", 82, false),
                reading("R", 76, false),
                reading(t("Case", "Hộp"), 54, false),
            ],
            "",
        ),
        card(
            "hifispeaker.fill",
            t("Sample Speaker", "Loa mẫu"),
            vec![],
            t("Does not report a charge", "Không báo mức pin"),
        ),
        card(
            "gamecontroller.fill",
            t("Sample Controller", "Tay cầm mẫu"),
            vec![reading("", 9, true)],
            t("Low", "Pin yếu"),
        ),
    ];
    DevicesDisplay {
        cards: all.into_iter().take(count).collect(),
        footer: t("Updated just now", "Vừa cập nhật").to_owned(),
        note: None,
    }
}

fn main() {
    let mtm = MainThreadMarker::new().expect("main thread");
    let dir = PathBuf::from(env::args().nth(1).unwrap_or_else(|| "/tmp/devices".into()));
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
        let view = DevicesView::new(mtm, notch.page_area(PAGE_SIZE.width, PAGE_SIZE.height));
        let index = notch.page_count();
        notch.add_page(view.page("Devices".into(), || {}, || {}));
        let (shot, shown) = (Rc::clone(&notch), Rc::clone(&view));
        app::after(0.4, move || {
            let mut scenes = vec![
                ("six_en", scene(false, 6)),
                ("six_vi", scene(true, 6)),
                ("three_en", scene(false, 3)),
                ("one_en", scene(false, 1)),
            ];
            for (name, title, hint) in [
                (
                    "none_en",
                    "No Bluetooth device is connected",
                    "Connect a mouse, keyboard or headphones and its charge shows here.",
                ),
                (
                    "problem_vi",
                    "Không đọc được thiết bị Bluetooth",
                    "macOS chưa trả lời kịp. Thử lại sau một lát.",
                ),
            ] {
                scenes.push((
                    name,
                    DevicesDisplay {
                        note: Some((title.into(), hint.into())),
                        ..DevicesDisplay::default()
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
