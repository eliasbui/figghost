//! Draws the Launcher tab of the open notch into PNGs with real app icons and a folder, without showing anything:
//! run it with FIGGHOST_NOTCH_OFFSCREEN=1.
//! `FIGGHOST_NOTCH_OFFSCREEN=1 cargo run -p macos-sys --example render_launcher -- /tmp/launcher`

use macos_sys::{
    MainThreadMarker, app,
    launcher_ui::{LauncherDisplay, LauncherView, PAGE_SIZE, Tile, TileIcon},
    notch::{Notch, NotchGeometry},
    style::Tone,
    ui::AppCallbacks,
};
use objc2::{msg_send, rc::Retained};
use objc2_app_kit::NSView;
use std::{cell::RefCell, env, fs, path::PathBuf, process, rc::Rc};

fn tile(title: &str, icon: TileIcon) -> Tile {
    Tile {
        title: title.into(),
        icon,
    }
}

fn app_tile(title: &str, path: &str) -> Tile {
    tile(title, TileIcon::File(PathBuf::from(path)))
}

fn main() {
    let mtm = MainThreadMarker::new().expect("main thread");
    let dir = PathBuf::from(env::args().nth(1).unwrap_or_else(|| "/tmp/launcher".into()));
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
        let pressed: Rc<RefCell<Vec<usize>>> = Rc::default();
        let log = Rc::clone(&pressed);
        let view = LauncherView::new(
            mtm,
            notch.page_area(PAGE_SIZE.width, PAGE_SIZE.height),
            move |i| log.borrow_mut().push(i),
        );
        let index = notch.page_count();
        let page = view.page("Launcher".into(), || {}, || {});
        let root: Retained<NSView> = page.root.clone();
        notch.add_page(page);
        let (shot, shown) = (Rc::clone(&notch), Rc::clone(&view));
        app::after(0.4, move || {
            let apps = vec![
                app_tile("Safari", "/Applications/Safari.app"),
                app_tile("Notes", "/System/Applications/Notes.app"),
                app_tile("Terminal", "/System/Applications/Utilities/Terminal.app"),
                app_tile("Calculator", "/System/Applications/Calculator.app"),
                app_tile("Documents", "/Users"),
                tile(
                    "Resize and share",
                    TileIcon::Symbol("bolt.fill", Tone::Orange),
                ),
            ];
            let mut many = apps.clone();
            many.extend([
                app_tile("Music", "/System/Applications/Music.app"),
                app_tile("Calendar", "/System/Applications/Calendar.app"),
                app_tile("Photos", "/System/Applications/Photos.app"),
                app_tile(
                    "A name that is far too long to fit",
                    "/System/Applications/Mail.app",
                ),
                tile(
                    "Morning routine",
                    TileIcon::Symbol("bolt.fill", Tone::Orange),
                ),
                app_tile("Downloads", "/Users/Shared"),
            ]);
            let scenes = vec![
                (
                    "six",
                    LauncherDisplay {
                        tiles: apps.clone(),
                        note: None,
                    },
                ),
                (
                    "twelve",
                    LauncherDisplay {
                        tiles: many,
                        note: None,
                    },
                ),
                (
                    "three",
                    LauncherDisplay {
                        tiles: apps[..3].to_vec(),
                        note: None,
                    },
                ),
                (
                    "empty",
                    LauncherDisplay {
                        tiles: Vec::new(),
                        note: Some((
                            "Nothing pinned yet".into(),
                            "Add apps, folders and Shortcuts in Settings, under Notch tabs, Launcher."
                                .into(),
                        )),
                    },
                ),
            ];
            // A press on a tile, the way an assistive tool makes one, tells the app which tile it was.
            shown.show(&scenes[0].1);
            let tiles: Vec<Retained<NSView>> =
                root.subviews().iter().filter(|v| !v.isHidden()).collect();
            assert_eq!(tiles.len(), 6, "a tile for each pin");
            // SAFETY: accessibilityPerformPress takes no arguments and returns whether it pressed.
            let pressed_ok: bool = unsafe { msg_send![&*tiles[2], accessibilityPerformPress] };
            assert!(pressed_ok, "the tile took the press");
            assert_eq!(*pressed.borrow(), [2], "the third tile is number 2");
            println!("ok      a press on a tile says which tile it was");
            for (name, display) in scenes {
                shown.show(&display);
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
