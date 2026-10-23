//! Draws the Displays tab of the open notch into PNGs with sample displays, in both languages, without showing
//! anything, and moves a slider and chooses a profile as a person would to check that the app is told which display
//! and which setting: run it with FIGGHOST_NOTCH_OFFSCREEN=1.
//! `FIGGHOST_NOTCH_OFFSCREEN=1 cargo run -p macos-sys --example render_displays -- /tmp/displays`

use macos_sys::{
    MainThreadMarker, app,
    ddc::Control,
    displays_ui::{DisplaysDisplay, DisplaysTexts, DisplaysView, PAGE_SIZE, Screen},
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
        note: None,
    }
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
        note: None,
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
            println!("the sliders and the menus say which display and which setting");
            process::exit(0);
        });
        (notch, view)
    });
}
