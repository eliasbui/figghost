//! Draws the Displays tab of the open notch into PNGs with sample displays, in both languages, without showing
//! anything, the arrangement of three displays among them, and moves a slider, chooses a profile and chooses what a
//! display is used as, as a person would, to check that the app is told which display and which setting: run it with
//! FIGGHOST_NOTCH_OFFSCREEN=1.
//! `FIGGHOST_NOTCH_OFFSCREEN=1 cargo run -p macos-sys --example render_displays -- /tmp/displays`

use macos_sys::{
    MainThreadMarker, NSPoint, NSRect, NSSize, app,
    ddc::Control,
    displays_ui::{
        ARRANGE_SIZE, ArrangeCallbacks, Arranged, Arrangement, DisplaysDisplay, DisplaysTexts,
        DisplaysView, PAGE_SIZE, Screen,
    },
    notch::{Notch, NotchGeometry},
    ui::AppCallbacks,
};
use std::{cell::RefCell, env, fs, path::PathBuf, process, rc::Rc};

fn screen(name: &str, brightness: f64, contrast: Option<f64>) -> Screen {
    Screen {
        name: name.to_owned(),
        brightness: Some(brightness),
        contrast,
        profiles: Vec::new(),
        profile: None,
    }
}

fn scene(count: usize) -> DisplaysDisplay {
    let all = [
        screen("S24i-30", 0.8, Some(0.75)),
        screen("LG HDR 4K", 0.35, Some(0.7)),
        screen("DELL U2723QE", 1.0, None),
    ];
    DisplaysDisplay {
        screens: all.into_iter().take(count).collect(),
        ..DisplaysDisplay::default()
    }
}

/// Three displays: the built-in one, the main one, a monitor above it to the right, and one that mirrors the monitor.
fn arranged(vietnamese: bool, view: usize) -> DisplaysDisplay {
    let at = |x: f64, y: f64, w: f64, h: f64| NSRect::new(NSPoint::new(x, y), NSSize::new(w, h));
    let middle = (ARRANGE_SIZE.height - 70.0) / 2.0;
    let screen = |name: &str, frame: NSRect, main: bool, mirrored_by: usize| Arranged {
        name: name.into(),
        frame,
        main,
        mirrored_by,
    };
    let (names, label, uses, hint, detail) = if vietnamese {
        (
            ["Độ sáng & màu", "Sắp xếp"],
            "Dùng làm",
            [
                "Màn hình chính",
                "Màn hình mở rộng",
                "Phản chiếu cho LG HDR 4K",
            ],
            "Kéo màn hình để đổi chỗ, kéo thanh trắng để chọn màn hình chính. Bấm lại vào một chồng để chọn màn hình đang phản chiếu.",
            "1512 x 982, liền máy, màn hình chính",
        )
    } else {
        (
            ["Brightness & color", "Arrange"],
            "Use as",
            ["Main display", "Extended display", "Mirror for LG HDR 4K"],
            "Drag a display to move it, or the white bar to choose the main display. Click a stack again for the display that mirrors it.",
            "1512 x 982, built in, main display",
        )
    };
    DisplaysDisplay {
        screens: vec![screen_card("S24i-30", 0.8, Some(0.75))],
        modes: Some((names.map(String::from), view)),
        arrangement: Some(Arrangement {
            screens: vec![
                screen(
                    "Built-in Retina Display",
                    at(40.0, middle - 20.0, 110.0, 71.0),
                    true,
                    0,
                ),
                screen("LG HDR 4K", at(150.0, middle + 2.0, 150.0, 84.0), false, 1),
            ],
            chosen: 0,
            title: "Built-in Retina Display".into(),
            detail: detail.into(),
            use_label: label.into(),
            uses: uses.map(String::from).to_vec(),
            using: Some(0),
            hint: hint.into(),
        }),
        ..DisplaysDisplay::default()
    }
}

fn screen_card(name: &str, brightness: f64, contrast: Option<f64>) -> Screen {
    screen(name, brightness, contrast)
}

/// The macOS profiles, after the display's own, with the one in use.
fn with_profiles(mut screen: Screen, current: usize) -> Screen {
    screen.profiles = [
        screen.name.as_str(),
        "ACES CG Linear (Academy Color Encoding System AP1)",
        "Adobe RGB (1998)",
        "Display P3",
        "Generic RGB Profile",
        "Rec. ITU-R BT.2020-1",
        "Rec. ITU-R BT.709-5",
        "sRGB IEC61966-2.1",
    ]
    .map(String::from)
    .to_vec();
    screen.profile = Some(current);
    screen
}

/// A built-in screen, which only has its profiles, beside external displays with sliders and profiles.
fn profiles_scene(count: usize) -> DisplaysDisplay {
    let built_in = Screen {
        brightness: None,
        contrast: None,
        ..screen("Built-in Retina Display", 0.0, None)
    };
    let all = [
        with_profiles(screen("S24i-30", 0.8, Some(0.75)), 7),
        with_profiles(built_in, 0),
        with_profiles(screen("DELL U2723QE", 1.0, None), 3),
    ];
    DisplaysDisplay {
        screens: all.into_iter().take(count).collect(),
        ..DisplaysDisplay::default()
    }
}

fn main() {
    let mtm = MainThreadMarker::new().expect("main thread");
    let dir = PathBuf::from(env::args().nth(1).unwrap_or_else(|| "/tmp/displays".into()));
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
        let moved: Rc<RefCell<Vec<(usize, Control, f64)>>> = Rc::default();
        let chosen: Rc<RefCell<Vec<(usize, usize)>>> = Rc::default();
        let (told, picked) = (Rc::clone(&moved), Rc::clone(&chosen));
        let used: Rc<RefCell<Vec<String>>> = Rc::default();
        let note = |what: &'static str| {
            let used = Rc::clone(&used);
            Rc::new(move |index: usize| used.borrow_mut().push(format!("{what} {index}")))
                as Rc<dyn Fn(usize)>
        };
        let dropped = Rc::clone(&used);
        let callbacks = ArrangeCallbacks {
            on_mode: note("mode"),
            on_choose: note("choose"),
            on_move: Rc::new(move |index, frame: NSRect| {
                dropped
                    .borrow_mut()
                    .push(format!("move {index} {:.0}", frame.origin.x))
            }),
            on_main: note("main"),
            on_use: note("use"),
        };
        let view = DisplaysView::new(
            mtm,
            notch.page_area(PAGE_SIZE.width, PAGE_SIZE.height),
            DisplaysTexts {
                brightness: "Brightness".into(),
                contrast: "Contrast".into(),
                profile: "Color profile".into(),
            },
            Rc::new(move |index, control, value| told.borrow_mut().push((index, control, value))),
            Rc::new(move |index, choice| picked.borrow_mut().push((index, choice))),
            callbacks,
        );
        let index = notch.page_count();
        notch.add_page(view.page("Displays".into(), || {}, || {}));
        let (shot, shown) = (Rc::clone(&notch), Rc::clone(&view));
        app::after(0.4, move || {
            let mut scenes = vec![
                ("one", scene(1)),
                ("two", scene(2)),
                ("three", scene(3)),
                ("profiles_one", profiles_scene(1)),
                ("profiles_three", profiles_scene(3)),
                ("arrange_en", arranged(false, 1)),
                ("arrange_vi", arranged(true, 1)),
                ("arrange_cards", arranged(false, 0)),
            ];
            for (name, title, hint) in [
                (
                    "none_en",
                    "No display to set",
                    "No external display answers over DDC/CI. Turn DDC/CI on in the display's own menu, or connect it directly instead of through a hub or adapter.",
                ),
                (
                    "off_vi",
                    "Tab Màn hình đang tắt",
                    "Bật trong Cài đặt, mục Tab của notch, Màn hình.",
                ),
            ] {
                scenes.push((
                    name,
                    DisplaysDisplay {
                        screens: Vec::new(),
                        note: Some((title.into(), hint.into())),
                        ..DisplaysDisplay::default()
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

            shown.show(scene(3));
            assert!(shown.press(1, Control::Contrast, 0.5));
            assert!(shown.press(0, Control::Brightness, 0.25));
            assert!(
                !shown.press(2, Control::Contrast, 0.5),
                "a display without contrast has no slider for it"
            );
            assert_eq!(
                *moved.borrow(),
                [(1, Control::Contrast, 0.5), (0, Control::Brightness, 0.25)]
            );
            // A new reading moves the sliders and leaves them where they are; a new display makes them again.
            shown.show(scene(3));
            shown.show(scene(2));
            assert!(shown.press(1, Control::Brightness, 1.0));
            // A profile chosen in a menu names its display and its place in the list; a card without one has none.
            shown.show(profiles_scene(3));
            assert!(shown.choose(1, 3));
            assert!(
                shown.press(0, Control::Contrast, 0.4),
                "the sliders still work beside a menu"
            );
            shown.show(scene(2));
            assert!(
                !shown.choose(0, 1),
                "a display without profiles has no menu"
            );
            assert_eq!(*chosen.borrow(), [(1, 3)]);
            // The arrangement: its menu says the choice, a drop says the display and where; the cards are gone.
            shown.show(arranged(false, 1));
            assert!(shown.choose_use(2));
            shown.drop_screen(
                1,
                NSRect::new(NSPoint::new(12.0, 30.0), NSSize::new(150.0, 84.0)),
            );
            assert!(
                !shown.press(0, Control::Brightness, 0.5),
                "no slider while arranging"
            );
            shown.show(arranged(false, 0));
            assert!(
                shown.press(0, Control::Brightness, 0.5),
                "the sliders are back on the first view"
            );
            assert!(
                !shown.choose_use(0),
                "and the menu of the arrangement is gone"
            );
            assert_eq!(*used.borrow(), ["use 2", "move 1 12"]);
            println!("the sliders and the menus say which display and which setting");
            process::exit(0);
        });
        (notch, view)
    });
}
