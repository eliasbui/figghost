//! Shows the notch with sample content on all its tabs for a few seconds. It opens by itself unless
//! NO_OPEN is set, which leaves it to hover (for checking the pointer handling); `PAGE=1` opens on the
//! shelf and `PAGE=2` on the media tab.
//! `cargo run -p macos-sys --example notch_demo -- 6 [thumbnail.png]`

use macos_sys::{
    MainThreadMarker, app,
    media_ui::{MediaCallbacks, MediaView},
    notch::{Notch, NotchGeometry},
    now_playing::{Sample, Track},
    shelf_ui::{ShelfCallbacks, ShelfView},
    ui::{AppCallbacks, Row},
};
use std::{env, fs, path::PathBuf, rc::Rc, time::Instant};

fn rows(thumb: Option<PathBuf>) -> Vec<Row> {
    let row = |id, title: &str, subtitle: &str, pinned, thumb| Row {
        id,
        title: title.into(),
        subtitle: subtitle.into(),
        pinned,
        pin_key: pinned.then_some('b'),
        mono: false,
        thumb,
    };
    vec![
        row(
            1,
            "Meeting notes: ship the notch shelf on Friday",
            "Pinned, Safari, just now",
            true,
            None,
        ),
        row(
            2,
            "https://example.com/docs/getting-started",
            "Microsoft Edge, 2 minutes ago",
            false,
            None,
        ),
        row(3, "Image 96x64", "Screenshot, 1 hour ago", false, thumb),
        row(
            4,
            "Nguyễn Văn Đức - 0901 234 567",
            "Contacts, 2 days ago",
            false,
            None,
        ),
    ]
}

fn main() {
    let mtm = MainThreadMarker::new().expect("main thread");
    let seconds: f64 = env::args()
        .nth(1)
        .and_then(|s| s.parse().ok())
        .unwrap_or(6.0);
    let thumb = env::args().nth(2).map(PathBuf::from);
    app::run_accessory(mtm, move || {
        let geometry = NotchGeometry::detect(mtm).expect("a screen");
        eprintln!("geometry: {geometry:?}");
        let noop = || AppCallbacks {
            on_query: Box::new(|_, _| {}),
            on_activate: Box::new(|_, _, _| {}),
            on_pin: Box::new(|_, _| {}),
            on_delete: Box::new(|_, _| {}),
        };
        let notch = Rc::new(Notch::new(mtm, geometry, noop(), move |view| {
            view.set_rows(rows(thumb.clone()), "");
        }));

        let dir = env::temp_dir().join(format!("figghost-demo-{}", std::process::id()));
        fs::create_dir_all(&dir).expect("temp dir");
        let files: Vec<String> = [
            "Quarterly report.pdf",
            "holiday-photo.jpg",
            "notes.txt",
            "budget.xlsx",
        ]
        .iter()
        .map(|name| {
            let path = dir.join(name);
            fs::write(&path, name).expect("temp file");
            path.to_string_lossy().into_owned()
        })
        .collect();
        let shelf = ShelfView::new(
            mtm,
            notch.page_frame(),
            ShelfCallbacks {
                on_drop: Box::new(|_| {}),
                on_remove: Box::new(|_| {}),
                on_open: Box::new(|_| {}),
                actions_for: Box::new(|_| Vec::new()),
                on_action: Box::new(|_, _| {}),
            },
        );
        shelf.set_paths(files);
        notch.add_page(shelf.page(|| {}));

        let media = MediaView::new(
            mtm,
            notch.page_frame(),
            MediaCallbacks {
                on_previous: Box::new(|| {}),
                on_play_pause: Box::new(|| {}),
                on_next: Box::new(|| {}),
                on_seek: Box::new(|_| {}),
            },
        );
        media.show(Some(&Sample {
            track: Track {
                app: "com.apple.Music".into(),
                app_name: "Music".into(),
                title: "Blue in Green".into(),
                artist: "Miles Davis".into(),
                album: "Kind of Blue".into(),
                playing: true,
                rate: 1.0,
                position_ms: 0,
                duration_ms: 0,
                artwork: None,
            },
            at: Instant::now(),
        }));
        notch.add_page(media.page(|| {}, || {}));

        let page: usize = env::var("PAGE")
            .ok()
            .and_then(|p| p.parse().ok())
            .unwrap_or(0);
        let opener = Rc::clone(&notch);
        if env::var_os("NO_OPEN").is_none() {
            app::after(0.6, move || opener.open_page(page));
        }
        app::after(seconds, move || {
            let _ = fs::remove_dir_all(&dir);
            app::quit(mtm);
        });
        (notch, shelf, media)
    });
}
