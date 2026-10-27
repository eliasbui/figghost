//! Draws the Present tab of the open notch into PNGs, ready to start and while presenting, in both languages, without
//! showing anything (run it with FIGGHOST_NOTCH_OFFSCREEN=1), and checks that every button tells the app what was
//! pressed.
//! `FIGGHOST_NOTCH_OFFSCREEN=1 cargo run -p macos-sys --example render_present -- /tmp/present`

use macos_sys::{
    MainThreadMarker, app,
    notch::{Notch, NotchGeometry},
    present_ui::{Choice, Live, PAGE_SIZE, PresentCallbacks, PresentDisplay, PresentView},
    style::Tone,
    ui::AppCallbacks,
};
use std::{cell::RefCell, env, fs, path::PathBuf, process, rc::Rc};

fn choice(title: &str, symbol: &'static str) -> Choice {
    Choice {
        title: title.into(),
        symbol,
    }
}

fn ready(vi: bool) -> PresentDisplay {
    let (kinds, stages, labels, start, hint) = if vi {
        (
            ["Màn hình", "Vùng", "Cửa sổ", "Ứng dụng"],
            ["Màn hình tích hợp", "LG HDR 4K", "Cửa sổ riêng"],
            ["Trình chiếu gì", "Trình chiếu lên"],
            "Bắt đầu trình chiếu",
            "Kéo chọn phần màn hình muốn chiếu: khán giả chỉ thấy đúng phần đó, cập nhật liên tục. Bấm ⌃⌥P để bắt đầu hoặc dừng từ bất kỳ ứng dụng nào.",
        )
    } else {
        (
            ["Display", "Area", "Window", "App"],
            ["Built-in Retina Display", "LG HDR 4K", "Its own window"],
            ["What to present", "Where the audience looks"],
            "Start presenting",
            "Drag over the part of the screen to show: the audience sees only that part, live. ⌃⌥P starts and stops from any app.",
        )
    };
    PresentDisplay {
        sources: kinds
            .iter()
            .zip(["display", "rectangle.dashed", "macwindow", "app"])
            .map(|(t, s)| choice(t, s))
            .collect(),
        source: 1,
        source_label: labels[0].into(),
        stages: stages
            .iter()
            .zip(["display", "display", "macwindow.on.rectangle"])
            .map(|(t, s)| choice(t, s))
            .collect(),
        stage: Some(1),
        stage_label: labels[1].into(),
        start: start.into(),
        hint: hint.into(),
        ..PresentDisplay::default()
    }
}

fn live(vi: bool, held: bool) -> PresentDisplay {
    let names: [&str; 6] = if vi {
        [
            "Tự phóng to",
            "Đèn chiếu",
            "Vòng con trỏ",
            "Đứng hình",
            "Màn đen",
            "Màn trắng",
        ]
    } else {
        [
            "Auto zoom",
            "Spotlight",
            "Pointer ring",
            "Hold",
            "Black",
            "White",
        ]
    };
    let symbols = [
        "plus.magnifyingglass",
        "flashlight.on.fill",
        "cursorarrow.rays",
        "pause.fill",
        "rectangle.fill",
        "rectangle",
    ];
    let on = [true, false, true, held, false, false];
    PresentDisplay {
        live: Some(Live {
            title: match (vi, held) {
                (false, false) => "Presenting",
                (false, true) => "Picture held",
                (true, false) => "Đang trình chiếu",
                (true, true) => "Đang đứng hình",
            }
            .into(),
            tone: Some(if held { Tone::Orange } else { Tone::Green }),
            clock: "12:07".into(),
            what: if vi {
                "Một vùng của Màn hình tích hợp lên LG HDR 4K"
            } else {
                "An area of Built-in Retina Display on LG HDR 4K"
            }
            .into(),
            stop: if vi { "Dừng" } else { "Stop" }.into(),
            toggles: names
                .iter()
                .zip(symbols)
                .zip(on)
                .map(|((n, s), on)| (choice(n, s), on))
                .collect(),
            first_row: 3,
            zoom: "2x".into(),
            zoom_names: if vi {
                ["Thu nhỏ".into(), "Phóng to".into()]
            } else {
                ["Zoom out".into(), "Zoom in".into()]
            },
            keys: if vi {
                "⌃⌥Z phóng to, ⌃⌥X thu nhỏ, ⌃⌥0 toàn cảnh, ⌃⌥A tự phóng to, ⌃⌥L đèn chiếu, ⌃⌥M vòng con trỏ, ⌃⌥H đứng hình, ⌃⌥B màn đen, ⌃⌥W màn trắng, ⌃⌥P bắt đầu hoặc dừng"
            } else {
                "⌃⌥Z zoom in, ⌃⌥X zoom out, ⌃⌥0 whole picture, ⌃⌥A auto zoom, ⌃⌥L spotlight, ⌃⌥M pointer ring, ⌃⌥H hold, ⌃⌥B black, ⌃⌥W white, ⌃⌥P start or stop"
            }
            .into(),
        }),
        ..PresentDisplay::default()
    }
}

fn main() {
    let mtm = MainThreadMarker::new().expect("main thread");
    let dir = PathBuf::from(env::args().nth(1).unwrap_or_else(|| "/tmp/present".into()));
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
        let said = |what: &'static str| {
            let log = Rc::clone(&log);
            Box::new(move |i: usize| log.borrow_mut().push(format!("{what} {i}")))
                as Box<dyn Fn(usize)>
        };
        let (started, stopped, zoomed) = (Rc::clone(&log), Rc::clone(&log), Rc::clone(&log));
        let view = PresentView::new(
            mtm,
            notch.page_area(PAGE_SIZE.width, PAGE_SIZE.height),
            PresentCallbacks {
                on_source: said("source"),
                on_stage: said("stage"),
                on_start: Box::new(move || started.borrow_mut().push("start".into())),
                on_stop: Box::new(move || stopped.borrow_mut().push("stop".into())),
                on_toggle: said("toggle"),
                on_zoom: Box::new(move |inwards| {
                    zoomed
                        .borrow_mut()
                        .push(format!("zoom {}", if inwards { "in" } else { "out" }))
                }),
            },
        );
        let index = notch.page_count();
        notch.add_page(view.page("Present".into(), || {}, || {}));
        let (shot, shown) = (Rc::clone(&notch), Rc::clone(&view));
        app::after(0.4, move || {
            // Ready: the choices and the button; the controls of a presentation are not there to press.
            shown.show(ready(false));
            for what in ["source 3", "stage 2", "start"] {
                assert!(shown.press(what), "{what} is pressable while ready");
            }
            assert!(!shown.press("stop") && !shown.press("toggle 0") && !shown.press("zoom in"));
            // Presenting: the switches of both rows count on from one another, and the zoom says which way.
            shown.show(live(false, false));
            for what in [
                "toggle 0", "toggle 2", "toggle 3", "toggle 5", "zoom in", "zoom out", "stop",
            ] {
                assert!(shown.press(what), "{what} is pressable while presenting");
            }
            assert!(!shown.press("start") && !shown.press("source 0"));
            assert_eq!(
                *log.borrow(),
                [
                    "source 3", "stage 2", "start", "toggle 0", "toggle 2", "toggle 3", "toggle 5",
                    "zoom in", "zoom out", "stop"
                ]
            );
            println!("ok      each button tells the app which one it was");
            let scenes = [
                ("ready_en", ready(false)),
                ("ready_vi", ready(true)),
                ("live_en", live(false, false)),
                ("held_vi", live(true, true)),
                (
                    "off_vi",
                    PresentDisplay {
                        note: Some((
                            "Tab Trình chiếu đang tắt".into(),
                            "Bật trong Cài đặt, mục Tab của notch, Trình chiếu.".into(),
                        )),
                        ..PresentDisplay::default()
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
