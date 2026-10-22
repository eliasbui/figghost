//! A thread that looks at the clipboard twice a second and reports each new copy, and that notices when a second copy
//! of the app asked this one to show its window.

use core_clipboard::{Capture, Payload};
use std::{
    collections::hash_map::DefaultHasher,
    fs,
    hash::{Hash, Hasher},
    path::PathBuf,
    thread,
    time::Duration,
};

const TICK: Duration = Duration::from_millis(500);
/// The file a second start of the app leaves for the first one, which shows its window and removes it.
pub const SHOW_REQUEST: &str = "show-request";

pub enum Watched {
    Copied(Capture),
    ShowAsked,
}

// shortcut: polling, not the system's change events (AddClipboardFormatListener, XFixes, data-control); switch when
// the half second of delay or the wake-ups matter.
pub fn start(home: PathBuf, send: impl Fn(Watched) + Send + 'static) {
    let spawned = thread::Builder::new()
        .name("fig-clipboard".into())
        .spawn(move || {
            let mut board = arboard::Clipboard::new()
                .inspect_err(|e| eprintln!("clipboard: {e}"))
                .ok();
            let mut last = None;
            #[cfg(windows)]
            let mut sequence = None;
            loop {
                thread::sleep(TICK);
                if fs::remove_file(home.join(SHOW_REQUEST)).is_ok() {
                    send(Watched::ShowAsked);
                }
                let Some(board) = board.as_mut() else {
                    continue;
                };
                #[cfg(windows)]
                {
                    let now = clipboard_win::seq_num();
                    if now == sequence {
                        continue;
                    }
                    sequence = now;
                    if private_on_windows() {
                        continue;
                    }
                }
                let Some(payload) = read(board) else {
                    continue;
                };
                let key = fingerprint(&payload);
                if last == Some(key) {
                    continue;
                }
                last = Some(key);
                send(Watched::Copied(Capture {
                    payload,
                    app_id: None,
                    app_name: None,
                    rich: None,
                }));
            }
        });
    if let Err(e) = spawned {
        eprintln!("clipboard: cannot start the watcher: {e}");
    }
}

fn fingerprint(payload: &Payload) -> u64 {
    let mut hasher = DefaultHasher::new();
    match payload {
        Payload::Text(text) => text.hash(&mut hasher),
        Payload::Files(paths) => paths.hash(&mut hasher),
        Payload::Image { png, .. } => png.hash(&mut hasher),
    }
    hasher.finish()
}

/// Files first: a file manager often puts the path as text next to the files.
fn read(board: &mut arboard::Clipboard) -> Option<Payload> {
    if let Ok(paths) = board.get().file_list()
        && !paths.is_empty()
    {
        return Some(Payload::Files(
            paths
                .iter()
                .map(|p| p.to_string_lossy().into_owned())
                .collect(),
        ));
    }
    if let Ok(text) = board.get_text()
        && !text.trim().is_empty()
    {
        return Some(Payload::Text(text));
    }
    read_image(board)
}

/// Windows says when the clipboard changed, so a picture is read once per copy.
#[cfg(windows)]
fn read_image(board: &mut arboard::Clipboard) -> Option<Payload> {
    let image = board.get_image().ok()?;
    let (width, height) = (image.width as u32, image.height as u32);
    let rgba = image::RgbaImage::from_raw(width, height, image.bytes.into_owned())?;
    let mut png = std::io::Cursor::new(Vec::new());
    rgba.write_to(&mut png, image::ImageFormat::Png).ok()?;
    Some(Payload::Image {
        png: png.into_inner(),
        width,
        height,
    })
}

// shortcut: no pictures on Linux, where without change events a picture would be read whole twice a second; add them
// with the XFixes or data-control events.
#[cfg(not(windows))]
fn read_image(_board: &mut arboard::Clipboard) -> Option<Payload> {
    None
}

/// Password managers mark what they copy with these formats so that clipboard tools leave it alone.
#[cfg(windows)]
fn private_on_windows() -> bool {
    [
        "ExcludeClipboardContentFromMonitorProcessing",
        "Clipboard Viewer Ignore",
    ]
    .iter()
    .any(|name| {
        clipboard_win::register_format(name)
            .is_some_and(|format| clipboard_win::is_format_avail(format.get()))
    })
}
