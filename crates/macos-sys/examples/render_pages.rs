//! Draws the focus and media pages of the notch into PNGs without showing any window.
//! `cargo run -p macos-sys --example render_pages -- /tmp/pages`

use macos_sys::{
    MainThreadMarker,
    focus_ui::{FocusCallbacks, FocusDisplay, FocusView},
    media_ui::{Lyrics, MediaCallbacks, MediaView},
    now_playing::{Player, Track},
};
use objc2::rc::Retained;
use objc2_app_kit::{
    NSAppearance, NSAppearanceCustomization, NSAppearanceNameDarkAqua, NSApplication,
    NSApplicationActivationPolicy, NSBitmapImageFileType, NSBox, NSBoxType, NSColor, NSView,
};
use objc2_foundation::{NSDate, NSDictionary, NSPoint, NSRect, NSRunLoop, NSSize};
use std::{env, fs, path::Path};

const PAGE: NSSize = NSSize::new(500.0, 396.0);

fn render(content: &NSView, out: &Path) {
    let backdrop = NSBox::new(MainThreadMarker::new().expect("main thread"));
    backdrop.setBoxType(NSBoxType::Custom);
    backdrop.setBorderWidth(0.0);
    backdrop.setFillColor(&NSColor::blackColor());
    backdrop.setContentViewMargins(NSSize::ZERO);
    backdrop.setFrame(NSRect::new(NSPoint::ZERO, PAGE));
    backdrop.setContentView(Some(content));
    // SAFETY: extern statics are process-lifetime constants.
    let dark = unsafe { NSAppearance::appearanceNamed(NSAppearanceNameDarkAqua) };
    backdrop.setAppearance(dark.as_deref());
    let bounds = backdrop.bounds();
    let rep = backdrop
        .bitmapImageRepForCachingDisplayInRect(bounds)
        .expect("bitmap rep");
    backdrop.cacheDisplayInRect_toBitmapImageRep(bounds, &rep);
    // SAFETY: an empty properties dictionary is valid for PNG output.
    let png: Retained<_> = unsafe {
        rep.representationUsingType_properties(NSBitmapImageFileType::PNG, &NSDictionary::new())
    }
    .expect("png data");
    fs::write(out, png.to_vec()).expect("write png");
    println!("wrote {}", out.display());
}

fn main() {
    let mtm = MainThreadMarker::new().expect("main thread");
    let dir = env::args().nth(1).unwrap_or_else(|| "/tmp/pages".into());
    fs::create_dir_all(&dir).expect("output directory");
    NSApplication::sharedApplication(mtm)
        .setActivationPolicy(NSApplicationActivationPolicy::Accessory);
    let frame = NSRect::new(NSPoint::ZERO, PAGE);

    let focus = FocusView::new(
        mtm,
        frame,
        &[15, 25, 45, 60],
        FocusCallbacks {
            on_toggle: Box::new(|| {}),
            on_reset: Box::new(|| {}),
            on_preset: Box::new(|_| {}),
            on_adjust: Box::new(|_| {}),
        },
    );
    for (name, display) in [
        (
            "focus_ready",
            FocusDisplay {
                clock: "25:00",
                progress: 0.0,
                running: false,
                paused: false,
                finished: false,
                minutes: 25,
            },
        ),
        (
            "focus_running",
            FocusDisplay {
                clock: "17:42",
                progress: 0.29,
                running: true,
                paused: false,
                finished: false,
                minutes: 25,
            },
        ),
        (
            "focus_done",
            FocusDisplay {
                clock: "00:00",
                progress: 1.0,
                running: false,
                paused: false,
                finished: true,
                minutes: 25,
            },
        ),
    ] {
        focus.show(&display);
        render(&focus.root, &Path::new(&dir).join(format!("{name}.png")));
    }

    let media = MediaView::new(
        mtm,
        frame,
        MediaCallbacks {
            on_previous: Box::new(|| {}),
            on_play_pause: Box::new(|| {}),
            on_next: Box::new(|| {}),
        },
    );
    let track = |playing| Track {
        player: Player::Music,
        title: "Blue in Green".into(),
        artist: "Miles Davis".into(),
        album: "Kind of Blue".into(),
        playing,
        position_ms: 0,
        duration_ms: 337_000,
    };
    media.show(Some(&track(true)));
    render(&media.root, &Path::new(&dir).join("media.png"));

    let pages = [
        (
            "media_lyrics_synced",
            Lyrics::Synced {
                previous: "Slow and quiet, the evening comes",
                current: "Nothing here is hurried at all",
                next: "A single note holds the room",
            },
        ),
        (
            "media_lyrics_long",
            Lyrics::Synced {
                previous: "",
                current: "A very long sung line that has to wrap onto a second line because it does not fit the width of the page",
                next: "A very long upcoming line that is cut with an ellipsis at the end of the first line of text",
            },
        ),
        (
            "media_lyrics_none",
            Lyrics::Message {
                title: "No lyrics for this track",
                detail: "The lyrics service has nothing for this song.",
            },
        ),
        (
            "media_lyrics_off",
            Lyrics::Message {
                title: "Online lyrics are off",
                detail: "Turn on Fetch lyrics online in Settings to see lyrics for this track.",
            },
        ),
        (
            "media_lyrics_plain",
            Lyrics::Plain(&"Slow and quiet, the evening comes\nNothing here is hurried at all\nA single note holds the room\n\nSecond verse, the same calm\nThe light goes soft and long\nA single note holds the room\n\n".repeat(3)),
        ),
    ];
    for (name, lyrics) in pages {
        // Going through Hidden skips the slide between two lines, so the picture shows the settled page.
        media.show_lyrics(&Lyrics::Hidden);
        media.show_lyrics(&lyrics);
        render(&media.root, &Path::new(&dir).join(format!("{name}.png")));
    }

    // The slide between two lines, caught a little way in.
    media.show_lyrics(&Lyrics::Synced {
        previous: "Slow and quiet, the evening comes",
        current: "Nothing here is hurried at all",
        next: "A single note holds the room",
    });
    media.show_lyrics(&Lyrics::Synced {
        previous: "Nothing here is hurried at all",
        current: "A single note holds the room",
        next: "Second verse, the same calm",
    });
    NSRunLoop::mainRunLoop().runUntilDate(&NSDate::dateWithTimeIntervalSinceNow(0.08));
    render(
        &media.root,
        &Path::new(&dir).join("media_lyrics_sliding.png"),
    );
}
