//! Draws the focus and media pages of the notch into PNGs without showing any window.
//! `cargo run -p macos-sys --example render_pages -- /tmp/pages`

use block2::RcBlock;
use macos_sys::{
    MainThreadMarker,
    focus_ui::{FocusCallbacks, FocusDisplay, FocusView},
    media_ui::{Lyrics, MediaCallbacks, MediaView},
    now_playing::{Artwork, Sample, Track},
};
use objc2::{rc::Retained, runtime::Bool};
use objc2_app_kit::{
    NSAppearance, NSAppearanceCustomization, NSAppearanceNameDarkAqua, NSApplication,
    NSApplicationActivationPolicy, NSBitmapImageFileType, NSBitmapImageRep, NSBox, NSBoxType,
    NSColor, NSImage, NSRectFill, NSView,
};
use objc2_foundation::{NSDate, NSDictionary, NSPoint, NSRect, NSRunLoop, NSSize};
use std::{env, fs, path::Path, sync::Arc, time::Instant};

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

/// A wide picture in three bands, as PNG bytes: on the square card the middle band shows whole and the outer two
/// are cut, which is what filling the card looks like.
fn wide_artwork() -> Vec<u8> {
    let draw = RcBlock::new(|rect: NSRect| {
        let third = rect.size.width / 3.0;
        for (i, (r, g, b)) in [(0.92, 0.33, 0.30), (0.97, 0.78, 0.30), (0.28, 0.52, 0.95)]
            .into_iter()
            .enumerate()
        {
            NSColor::colorWithSRGBRed_green_blue_alpha(r, g, b, 1.0).setFill();
            NSRectFill(NSRect::new(
                NSPoint::new(third * i as f64, 0.0),
                NSSize::new(third, rect.size.height),
            ));
        }
        Bool::YES
    });
    let image =
        NSImage::imageWithSize_flipped_drawingHandler(NSSize::new(320.0, 180.0), false, &draw);
    let tiff = image.TIFFRepresentation().expect("tiff");
    let rep = NSBitmapImageRep::imageRepWithData(&tiff).expect("bitmap");
    // SAFETY: an empty properties dictionary is valid for PNG output.
    unsafe {
        rep.representationUsingType_properties(NSBitmapImageFileType::PNG, &NSDictionary::new())
    }
    .expect("png")
    .to_vec()
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
            on_seek: Box::new(|_| {}),
        },
    );
    let sample = |playing, artwork: Option<Artwork>| Sample {
        track: Track {
            app: "com.apple.Music".into(),
            app_name: "Music".into(),
            title: "Blue in Green".into(),
            artist: "Miles Davis".into(),
            album: "Kind of Blue".into(),
            playing,
            rate: 1.0,
            position_ms: 128_000,
            duration_ms: 337_000,
            artwork,
        },
        at: Instant::now(),
    };
    media.show(Some(&sample(true, None)));
    render(&media.root, &Path::new(&dir).join("media.png"));
    let artwork = Artwork {
        id: 1,
        bytes: Arc::new(wide_artwork()),
    };
    let mut browser = sample(false, Some(artwork));
    browser.track.app = "com.microsoft.edgemac".into();
    browser.track.app_name = "Microsoft Edge".into();
    browser.track.title = "A long video title from a browser tab that does not fit".into();
    browser.track.artist = "Channel".into();
    browser.track.duration_ms = 5_400_000;
    browser.track.position_ms = 939_020;
    media.show(Some(&browser));
    render(&media.root, &Path::new(&dir).join("media_artwork.png"));
    media.show(None);
    render(&media.root, &Path::new(&dir).join("media_idle.png"));
    media.show(Some(&browser));

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
