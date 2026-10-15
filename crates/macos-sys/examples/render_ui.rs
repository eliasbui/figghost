//! Draws the clipboard UI into a PNG without showing any window, for checking layout and drawing.
//! `cargo run -p macos-sys --example render_ui -- /tmp/ui.png [thumbnail.png]`

use macos_sys::{
    MainThreadMarker,
    notch::EXPANDED_SIZE,
    stack::pill::Pill,
    ui::{BoardChip, Callbacks, ClipUi, ListStyle, Mark, POPUP_SIZE, Row},
};
use objc2::rc::Retained;
use objc2_app_kit::{
    NSAppearance, NSAppearanceCustomization, NSAppearanceNameDarkAqua, NSApplication,
    NSApplicationActivationPolicy, NSBitmapImageFileType, NSBox, NSBoxType, NSColor,
};
use objc2_foundation::{NSDictionary, NSPoint, NSRect, NSSize};
use std::{collections::HashMap, env, fs, path::PathBuf};

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

fn snapshot(mtm: MainThreadMarker, ui: &ClipUi, size: NSSize, out: &str) {
    let backdrop = NSBox::new(mtm);
    backdrop.setBoxType(NSBoxType::Custom);
    backdrop.setBorderWidth(0.0);
    backdrop.setFillColor(&NSColor::colorWithWhite_alpha(0.16, 1.0));
    backdrop.setContentViewMargins(NSSize::ZERO);
    backdrop.setFrame(NSRect::new(NSPoint::ZERO, size));
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
    fs::write(out, png.to_vec()).expect("write png");
    println!("wrote {out}");
}

fn callbacks() -> Callbacks {
    Callbacks {
        on_query: Box::new(|_, _| {}),
        on_activate: Box::new(|_, _, _| {}),
        on_pin: Box::new(|_, _| {}),
        on_delete: Box::new(|_, _| {}),
        on_close: Box::new(|_| {}),
    }
}

fn boards() -> Vec<BoardChip> {
    ["Work", "Snippets", "Cong viec hang ngay cua toi"]
        .into_iter()
        .enumerate()
        .map(|(i, name)| BoardChip {
            id: i as i64 + 1,
            name: name.into(),
            filter: format!("@{}", name.to_lowercase().replace(' ', "_")),
        })
        .collect()
}

/// Rows with a filed item, a hidden secret (a lock and dots), a revealed one and a plain item.
fn board_rows() -> (Vec<Row>, HashMap<i64, Mark>) {
    let row = |id, title: &str, subtitle: &str, pinned, key: Option<char>| Row {
        id,
        title: title.into(),
        subtitle: subtitle.into(),
        pinned,
        pin_key: key,
        mono: false,
        thumb: None,
    };
    let rows = vec![
        row(
            1,
            "Quarterly report draft v3",
            "Pinned, Work, Pages, 3 minutes ago",
            true,
            Some('b'),
        ),
        row(2, "", "Sensitive, Safari, just now", false, None),
        row(
            3,
            "",
            "Pinned, Sensitive, 1Password Mini, 2 minutes ago",
            true,
            Some('d'),
        ),
        row(
            4,
            "password: hunter2xyz",
            "Sensitive, Notes, 1 minute ago",
            false,
            None,
        ),
        row(
            5,
            "Sign off politely: Best regards, An",
            "Pinned, Snippets, Notes, 1 hour ago",
            true,
            Some('e'),
        ),
        row(
            6,
            "https://example.com/docs/getting-started",
            "Microsoft Edge, 2 hours ago",
            false,
            None,
        ),
    ];
    let marks = HashMap::from([
        (
            1,
            Mark {
                board: Some(1),
                ..Mark::default()
            },
        ),
        (
            2,
            Mark {
                sensitive: true,
                hidden: true,
                board: None,
            },
        ),
        (
            3,
            Mark {
                sensitive: true,
                hidden: true,
                board: None,
            },
        ),
        (
            4,
            Mark {
                sensitive: true,
                hidden: false,
                board: None,
            },
        ),
        (
            5,
            Mark {
                board: Some(2),
                ..Mark::default()
            },
        ),
    ]);
    (rows, marks)
}

fn main() {
    let mtm = MainThreadMarker::new().expect("main thread");
    let out = env::args().nth(1).unwrap_or_else(|| "/tmp/ui.png".into());
    let thumb = env::args().nth(2).map(PathBuf::from);
    let app = NSApplication::sharedApplication(mtm);
    app.setActivationPolicy(NSApplicationActivationPolicy::Accessory);

    let ui = ClipUi::new(mtm, POPUP_SIZE, 0.0, callbacks());
    ui.view.set_rows(sample_rows(thumb), "");
    snapshot(mtm, &ui, POPUP_SIZE, &out);

    // The popup with boards: their chips are the only row above the list.
    let stem = out.strip_suffix(".png").unwrap_or(&out).to_owned();
    let popup = ClipUi::new(mtm, POPUP_SIZE, 0.0, callbacks());
    popup.view.set_boards(boards());
    let (rows, marks) = board_rows();
    popup.view.set_rows_marked(rows, marks, "");
    snapshot(mtm, &popup, POPUP_SIZE, &format!("{stem}-popup-boards.png"));

    // The notch list: the filter chips, then the board chips on a row of their own.
    let size = EXPANDED_SIZE;
    let cards = ClipUi::with_style(mtm, size, 0.0, callbacks(), ListStyle::Cards);
    cards.view.set_boards(boards());
    let (rows, marks) = board_rows();
    cards.view.set_rows_marked(rows, marks, "");
    cards
        .view
        .search
        .setStringValue(&objc2_foundation::NSString::from_str("@work "));
    cards.view.sync_chips();
    snapshot(mtm, &cards, size, &format!("{stem}-cards-boards.png"));

    // The notch list as it was before there were any boards.
    let plain = ClipUi::with_style(mtm, size, 0.0, callbacks(), ListStyle::Cards);
    plain.view.set_rows(sample_rows(None), "");
    snapshot(mtm, &plain, size, &format!("{stem}-cards.png"));

    // The pill, drawn without being shown.
    let pill = Pill::new(mtm);
    pill.set_text("Copy Stack: 3 to paste");
    let bytes = pill.snapshot_png().expect("the pill draws");
    let pill_path = format!("{stem}-pill.png");
    fs::write(&pill_path, bytes).expect("write png");
    println!("wrote {pill_path}");
    assert!(!pill.is_visible(), "rendering never shows the pill");
}
