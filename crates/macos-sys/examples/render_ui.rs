//! Draws the clipboard UI into a PNG without showing any window, for checking layout and drawing.
//! `cargo run -p macos-sys --example render_ui -- /tmp/ui.png [thumbnail.png]`

use macos_sys::{
    MainThreadMarker,
    ui::{Callbacks, ClipUi, POPUP_SIZE, Row},
};
use objc2::rc::Retained;
use objc2_app_kit::{
    NSAppearance, NSAppearanceCustomization, NSAppearanceNameDarkAqua, NSApplication,
    NSApplicationActivationPolicy, NSBitmapImageFileType, NSBox, NSBoxType, NSColor,
};
use objc2_foundation::{NSDictionary, NSPoint, NSRect, NSSize};
use std::{env, fs, path::PathBuf};

fn sample_rows(thumb: Option<PathBuf>) -> Vec<Row> {
    let row = |id, title: &str, subtitle: &str, pinned, mono, thumb| Row {
        id,
        title: title.into(),
        subtitle: subtitle.into(),
        pinned,
        pin_key: pinned.then_some('b'),
        mono,
        thumb,
    };
    vec![
        row(
            1,
            "Meeting notes: ship the notch shelf on Friday",
            "Pinned, Safari, just now",
            true,
            false,
            None,
        ),
        row(
            2,
            "https://example.com/docs/getting-started",
            "Microsoft Edge, 2 minutes ago",
            false,
            false,
            None,
        ),
        row(
            3,
            "fn main() {     let total = items.iter().sum::<i32>();",
            "Zed, 5 minutes ago",
            false,
            true,
            None,
        ),
        row(
            4,
            "Image 96x64",
            "Screenshot, 1 hour ago",
            false,
            false,
            thumb,
        ),
        row(
            5,
            "A very long line to check that the ellipsis at the end shows when there is not enough room in the popup",
            "Ghostty, 3 hours ago",
            false,
            false,
            None,
        ),
        row(
            6,
            "report-final.pdf and 2 other files",
            "Finder, yesterday",
            false,
            false,
            None,
        ),
        row(
            7,
            "Nguyễn Văn Đức - 0901 234 567",
            "Contacts, 2 days ago",
            false,
            false,
            None,
        ),
    ]
}

fn main() {
    let mtm = MainThreadMarker::new().expect("main thread");
    let out = env::args().nth(1).unwrap_or_else(|| "/tmp/ui.png".into());
    let thumb = env::args().nth(2).map(PathBuf::from);
    let app = NSApplication::sharedApplication(mtm);
    app.setActivationPolicy(NSApplicationActivationPolicy::Accessory);

    let callbacks = Callbacks {
        on_query: Box::new(|_, _| {}),
        on_activate: Box::new(|_, _, _| {}),
        on_pin: Box::new(|_, _| {}),
        on_delete: Box::new(|_, _| {}),
        on_close: Box::new(|_| {}),
    };
    let ui = ClipUi::new(mtm, POPUP_SIZE, 0.0, callbacks);
    ui.view.set_rows(sample_rows(thumb), "");

    let backdrop = NSBox::new(mtm);
    backdrop.setBoxType(NSBoxType::Custom);
    backdrop.setBorderWidth(0.0);
    backdrop.setFillColor(&NSColor::colorWithWhite_alpha(0.16, 1.0));
    backdrop.setContentViewMargins(NSSize::ZERO);
    backdrop.setFrame(NSRect::new(NSPoint::ZERO, POPUP_SIZE));
    backdrop.setContentView(Some(&ui.view.root));
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
    fs::write(&out, png.to_vec()).expect("write png");
    println!("wrote {out}");
}
