//! Draws the media tab with lyrics, in one language, into PNGs without showing any window, using the words the
//! app itself uses.
//!   `cargo run -p app-ui --example lyrics_preview -- /tmp/lyrics [en|vi]`

use std::{env, fs, path::Path, time::Instant};

use macos_sys::{
    MainThreadMarker,
    media_ui::{Lyrics, MediaView},
    now_playing::{Sample, Track},
};

// The example uses a few of the language tables; the rest of the file is for the app.
#[allow(dead_code)]
#[path = "../src/i18n.rs"]
mod i18n;
#[path = "../src/features/lyrics/strings.rs"]
mod strings;

use i18n::Language;
use strings::Notice;

fn main() {
    let mtm = MainThreadMarker::new().expect("main thread");
    let dir = env::args().nth(1).unwrap_or_else(|| "/tmp/lyrics".into());
    let language = Language::from_tag(&env::args().nth(2).unwrap_or_default()).unwrap_or_default();
    i18n::set(language);
    macos_sys::labels::set(language.labels("opt+p", "opt+delete"));
    fs::create_dir_all(&dir).expect("output directory");

    let media = MediaView::preview(mtm);
    media.show(Some(&Sample {
        track: Track {
            app: "com.spotify.client".into(),
            app_name: "Spotify".into(),
            title: "Nối vòng tay lớn".into(),
            artist: "Trịnh Công Sơn".into(),
            album: "Album".into(),
            playing: true,
            rate: 1.0,
            position_ms: 0,
            duration_ms: 0,
            artwork: None,
        },
        at: Instant::now(),
    }));

    let vi = language == Language::Vietnamese;
    let (previous, current, next) = if vi {
        (
            "Rừng núi dang tay nối lại biển xa",
            "Ta đi vòng tay lớn mãi để nối sơn hà",
            "Mặt đất bao la anh em ta về",
        )
    } else {
        (
            "Slow and quiet, the evening comes",
            "Nothing here is hurried at all",
            "A single note holds the room",
        )
    };
    let plain = format!("{previous}\n{current}\n{next}\n\n").repeat(4);
    let notice = |n: Notice| {
        let (title, detail) = language.lyrics_notice(n);
        Lyrics::Message { title, detail }
    };
    let pages = [
        (
            "synced",
            Lyrics::Synced {
                previous,
                current,
                next,
            },
        ),
        ("plain", Lyrics::Plain(&plain)),
        ("off", notice(Notice::Off)),
        ("fetching", notice(Notice::Fetching)),
        ("missing", notice(Notice::Missing)),
        ("failed", notice(Notice::Failed)),
        ("instrumental", notice(Notice::Instrumental)),
    ];
    for (name, lyrics) in pages {
        // Through Hidden, so that no slide is under way when the picture is taken.
        media.show_lyrics(&Lyrics::Hidden);
        media.show_lyrics(&lyrics);
        let path = Path::new(&dir).join(format!("{}_{name}.png", language.code()));
        fs::write(&path, media.snapshot_png(mtm).expect("png")).expect("write png");
        // The page was handed back by the snapshot; nothing else holds it.
        println!("wrote {}", path.display());
    }
}
