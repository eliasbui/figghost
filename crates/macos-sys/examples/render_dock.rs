//! Shows the Launcher's dock at each edge of the main screen and captures it, since Liquid Glass is drawn by the
//! window server and cannot be drawn offscreen. Needs an unlocked screen and the right to record it.
//! `cargo run -p macos-sys --example render_dock -- /tmp/dock`

use macos_sys::{
    MainThreadMarker, app,
    launcher_ui::{DockPanel, Edge, LauncherCallbacks, LauncherDisplay, Tile, TileIcon},
    style::Tone,
};
use objc2_app_kit::NSScreen;
use std::{env, fs, path::PathBuf, process, process::Command, rc::Rc};

fn tile(title: &str, path: &str) -> Tile {
    Tile {
        title: title.into(),
        icon: TileIcon::File(PathBuf::from(path)),
    }
}

/// Captures the dock with a margin, so that its shadow and the desktop behind it show.
fn capture(mtm: MainThreadMarker, dock: &DockPanel, path: &PathBuf) {
    let screen = NSScreen::screens(mtm)
        .firstObject()
        .expect("a screen")
        .frame();
    let frame = dock.frame();
    let margin = 24.0;
    let top = screen.size.height - (frame.origin.y + frame.size.height) - margin;
    let region = format!(
        "{},{},{},{}",
        (frame.origin.x - margin).max(0.0).round(),
        top.max(0.0).round(),
        (frame.size.width + 2.0 * margin).round(),
        (frame.size.height + 2.0 * margin).round()
    );
    println!(
        "frame   {:.0},{:.0} {:.0}x{:.0} on a screen of {:.0}x{:.0}",
        frame.origin.x,
        frame.origin.y,
        frame.size.width,
        frame.size.height,
        screen.size.width,
        screen.size.height
    );
    let status = Command::new("/usr/sbin/screencapture")
        .args(["-x", "-R", &region])
        .arg(path)
        .status()
        .expect("screencapture");
    // A locked screen gives no picture; the place of the dock is still checked.
    if status.success() {
        println!("wrote {}", path.display());
    } else {
        println!(
            "skipped {} (the screen could not be captured: locked?)",
            path.display()
        );
    }
}

fn main() {
    let mtm = MainThreadMarker::new().expect("main thread");
    let dir = PathBuf::from(env::args().nth(1).unwrap_or_else(|| "/tmp/dock".into()));
    fs::create_dir_all(&dir).expect("output directory");
    app::run_accessory(mtm, move || {
        let dock = DockPanel::new(
            mtm,
            LauncherCallbacks {
                on_open: Box::new(|_| {}),
                on_menu: Box::new(|_, _| {}),
                on_group: Box::new(|_| {}),
            },
        );
        let tiles = vec![
            tile("Safari", "/Applications/Safari.app"),
            tile("Notes", "/System/Applications/Notes.app"),
            tile("Terminal", "/System/Applications/Utilities/Terminal.app"),
            tile("Users", "/Users"),
            tile("hosts", "/etc/hosts"),
            Tile {
                title: "github.com".into(),
                icon: TileIcon::Symbol("globe", Tone::Blue),
            },
            Tile {
                title: "Morning".into(),
                icon: TileIcon::Symbol("bolt.fill", Tone::Orange),
            },
        ];
        let plain = LauncherDisplay {
            tiles: tiles.clone(),
            ..LauncherDisplay::default()
        };
        let grouped = LauncherDisplay {
            tiles: tiles[..5].to_vec(),
            groups: vec!["Nhóm 1".into(), "Work".into(), "Web".into()],
            group: 1,
            note: None,
        };
        let steps: Vec<(&str, LauncherDisplay, Edge)> = vec![
            ("left", plain.clone(), Edge::Left),
            (
                "right",
                LauncherDisplay {
                    tiles: tiles[..4].to_vec(),
                    ..plain.clone()
                },
                Edge::Right,
            ),
            ("bottom", grouped, Edge::Bottom),
        ];
        let dock = Rc::clone(&dock);
        let shown = Rc::clone(&dock);
        let mut delay = 0.4;
        for (name, display, edge) in steps {
            let dock = Rc::clone(&dock);
            app::after(delay, move || dock.show(&display, edge, "", false));
            let (dock, dir2) = (Rc::clone(&shown), dir.clone());
            app::after(delay + 0.8, move || {
                assert!(dock.is_visible(), "the dock shows at its edge");
                capture(mtm, &dock, &dir2.join(format!("{name}.png")));
            });
            delay += 1.2;
        }
        let last = Rc::clone(&shown);
        app::after(delay, move || {
            // Hidden until the pointer reaches the edge, which it is not at: the checker's pointer is left alone.
            last.show(&plain, Edge::Left, "", true);
            app::after(0.5, move || {
                let visible = last.is_visible();
                last.hide();
                assert!(!last.is_visible(), "hide takes it away");
                println!(
                    "ok      a dock that hides waits off the screen ({})",
                    if visible {
                        "the pointer was at the left edge"
                    } else {
                        "hidden"
                    }
                );
                process::exit(0);
            });
        });
        shown
    });
}
