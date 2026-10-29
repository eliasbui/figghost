//! Draws the Launcher tab of the open notch into PNGs with real app icons and a folder, without showing anything:
//! run it with FIGGHOST_NOTCH_OFFSCREEN=1.
//! `FIGGHOST_NOTCH_OFFSCREEN=1 cargo run -p macos-sys --example render_launcher -- /tmp/launcher`

use macos_sys::{
    MainThreadMarker, app,
    launcher_ui::{LauncherCallbacks, LauncherDisplay, LauncherView, PAGE_SIZE, Tile, TileIcon},
    notch::{Notch, NotchGeometry},
    panel::Key,
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
        let chosen: Rc<RefCell<Vec<usize>>> = Rc::default();
        let (log, picks) = (Rc::clone(&pressed), Rc::clone(&chosen));
        let view = LauncherView::new(
            mtm,
            notch.page_area(PAGE_SIZE.width, PAGE_SIZE.height),
            LauncherCallbacks {
                on_open: Box::new(move |i| log.borrow_mut().push(i)),
                on_menu: Box::new(|_, _| {}),
                on_group: Box::new(move |g| picks.borrow_mut().push(g)),
            },
        );
        let index = notch.page_count();
        let page = view.page("Launcher".into(), || {}, || {});
        let root: Retained<NSView> = page.root.clone();
        let on_key = Rc::new(page.on_key);
        notch.add_page(macos_sys::notch::Page {
            on_key: Box::new({
                let on_key = Rc::clone(&on_key);
                move |key| on_key(key)
            }),
            ..page
        });
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
            let groups: Vec<String> = ["Nhóm 1", "Work", "Web", "A group with a long name"]
                .map(String::from)
                .into();
            let web = vec![
                tile("github.com", TileIcon::Symbol("globe", Tone::Blue)),
                tile(
                    "me@example.com",
                    TileIcon::Symbol("envelope.fill", Tone::Blue),
                ),
                app_tile("hosts", "/etc/hosts"),
            ];
            let empty = Some((
                "Nothing pinned yet".to_owned(),
                "Add apps, folders and Shortcuts in Settings, under Notch tabs, Launcher."
                    .to_owned(),
            ));
            let scene = |tiles: Vec<Tile>, groups: &[String], group: usize| LauncherDisplay {
                tiles,
                note: None,
                groups: groups.to_vec(),
                group,
            };
            let scenes = vec![
                ("six", scene(apps.clone(), &[], 0)),
                ("twelve", scene(many.clone(), &[], 0)),
                ("three", scene(apps[..3].to_vec(), &[], 0)),
                (
                    "empty",
                    LauncherDisplay {
                        note: empty.clone(),
                        ..LauncherDisplay::default()
                    },
                ),
                ("groups-twelve", scene(many, &groups, 0)),
                ("groups-web", scene(web, &groups[..3], 2)),
                (
                    "groups-empty",
                    LauncherDisplay {
                        note: empty,
                        groups: groups[..2].to_vec(),
                        group: 1,
                        ..LauncherDisplay::default()
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
            // The arrow keys and the chips of the strip choose a group; there is no group before the first.
            shown.show(&scenes[5].1);
            assert!(!on_key(Key::Right), "there is no group after the last");
            assert!(!on_key(Key::Up), "other keys are left to the notch");
            assert!(on_key(Key::Left));
            shown.show(&scenes[4].1);
            assert!(!on_key(Key::Left), "there is no group before the first");
            assert!(on_key(Key::Right));
            let chips: Vec<Retained<NSView>> = root
                .subviews()
                .iter()
                .filter(|v| !v.isHidden() && v.frame().size.height < 40.0)
                .collect();
            assert_eq!(chips.len(), 4, "a chip for each group");
            // SAFETY: as above.
            let pressed_ok: bool = unsafe { msg_send![&*chips[3], accessibilityPerformPress] };
            assert!(pressed_ok);
            assert_eq!(*chosen.borrow(), [1, 1, 3], "the groups chosen, in order");
            shown.show(&scenes[0].1);
            assert!(!on_key(Key::Right), "one group has no other to go to");
            println!("ok      the arrow keys and the strip choose a group");
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
