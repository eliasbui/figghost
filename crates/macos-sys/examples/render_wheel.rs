//! Draws the drag wheel into PNG files without showing anything on screen: each kind at 4, 6, 9 and 12 slots,
//! at rest and with a slot lit. Names are `<kind>-<slots>-<rest|hover>.png`.
//!
//! The wheel's plate is Liquid Glass, which the window server draws and no snapshot can: these pictures stand a
//! dark translucent disc in for it and lay the result over a colourful backdrop, so that they show the layout, the
//! sizes and the ink, not the glass. `wheel_check <folder>` takes real pictures of it. The `opaque` folder holds the
//! solid plate that Increase Contrast and Reduce Transparency get, which is drawn exactly and kept transparent
//! round the disc.
//! The slots are examples of what the app offers, not its real lists.
//! `cargo run -p macos-sys --example render_wheel -- /tmp/wheel`

use block2::RcBlock;
use macos_sys::{
    MainThreadMarker, app,
    wheel_ui::{self, Kind, Slot, Wheel},
};
use objc2::runtime::Bool;
use objc2_app_kit::{NSBitmapImageFileType, NSBitmapImageRep, NSColor, NSGradient, NSImage};
use objc2_foundation::{NSData, NSDictionary, NSPoint, NSRect, NSSize};
use std::{env, fs, path::PathBuf};

/// `png` (a wheel, two pixels per point) over a colourful backdrop with stripes and lines, as PNG bytes.
fn on_colour(png: &[u8]) -> Option<Vec<u8>> {
    let wheel = NSImage::initWithData(objc2::AnyThread::alloc(), &NSData::with_bytes(png))?;
    let size = wheel.size();
    let draw = RcBlock::new(move |bounds: NSRect| {
        let colour = |r, g, b| NSColor::colorWithSRGBRed_green_blue_alpha(r, g, b, 1.0);
        if let Some(gradient) = NSGradient::initWithColors(
            objc2::AnyThread::alloc(),
            &objc2_foundation::NSArray::from_retained_slice(&[
                colour(0.95, 0.25, 0.30),
                colour(1.00, 0.70, 0.15),
                colour(0.25, 0.80, 0.45),
                colour(0.15, 0.50, 0.95),
                colour(0.60, 0.30, 0.90),
            ]),
        ) {
            gradient.drawInRect_angle(bounds, 35.0);
        }
        for i in 0..24 {
            NSColor::colorWithWhite_alpha(if i % 2 == 0 { 1.0 } else { 0.0 }, 0.55).set();
            objc2_app_kit::NSRectFill(NSRect::new(
                NSPoint::new(f64::from(i) * 40.0 + 8.0, 0.0),
                NSSize::new(12.0, bounds.size.height),
            ));
        }
        NSColor::colorWithWhite_alpha(1.0, 0.9).set();
        for row in 0..14 {
            objc2_app_kit::NSRectFill(NSRect::new(
                NSPoint::new(30.0, 30.0 + f64::from(row) * 52.0),
                NSSize::new(180.0 + f64::from(row % 4) * 80.0, 7.0),
            ));
        }
        wheel.drawInRect(bounds);
        Bool::YES
    });
    let image = NSImage::imageWithSize_flipped_drawingHandler(size, false, &draw);
    let tiff = image.TIFFRepresentation()?;
    let rep = NSBitmapImageRep::imageRepWithData(&tiff)?;
    // SAFETY: an empty properties dictionary is valid for PNG output.
    let data = unsafe {
        rep.representationUsingType_properties(NSBitmapImageFileType::PNG, &NSDictionary::new())
    }?;
    Some(data.to_vec())
}

/// (title, SF Symbol) for each kind, twelve entries so every size can be cut from the front. The tools and actions
/// are the ones the app offers, with a few long titles at the end to see them wrap.
fn entries(kind: Kind) -> [(&'static str, Option<&'static str>); 12] {
    match kind {
        Kind::Formats => [
            ("PNG", Some("photo")),
            ("JPEG", Some("photo")),
            ("HEIC", Some("photo")),
            ("TIFF", Some("photo")),
            ("AVIF", Some("photo")),
            ("GIF", Some("photo")),
            ("BMP", Some("photo")),
            ("PDF", Some("doc.richtext")),
            ("ZIP", Some("doc.zipper")),
            ("TAR", Some("doc.zipper")),
            ("TGZ", Some("doc.zipper")),
            ("DOCX", Some("doc.text")),
        ],
        Kind::Tools => [
            ("Compress", Some("arrow.down.right.and.arrow.up.left")),
            ("Half size", Some("arrow.up.left.and.arrow.down.right")),
            ("Quarter size", Some("arrow.up.left.and.arrow.down.right")),
            ("No metadata", Some("eye.slash")),
            ("Cut out", Some("person.crop.rectangle")),
            ("Copy text", Some("text.viewfinder")),
            ("Annotate", Some("pencil.tip.crop.circle")),
            ("Merge PDFs", Some("rectangle.stack")),
            ("Split pages", Some("scissors")),
            ("Max 1080p", Some("arrow.down.right.and.arrow.up.left")),
            ("Remove background", Some("person.crop.rectangle")),
            ("Compress 1080p", Some("arrow.down.right.and.arrow.up.left")),
        ],
        Kind::Actions => [
            ("Copy Path", Some("link")),
            ("Copy", Some("doc.on.doc")),
            ("AirDrop", Some("dot.radiowaves.left.and.right")),
            ("Terminal", Some("terminal")),
            ("Reveal", Some("folder")),
            ("Float", Some("tray.and.arrow.down")),
            ("Open with preview", Some("eye")),
            ("Share", Some("square.and.arrow.up")),
            ("Rename", Some("pencil")),
            ("Tag", Some("tag")),
            ("Get info", Some("info.circle")),
            ("Move to Trash", Some("trash")),
        ],
    }
}

fn wheel(kind: Kind, count: usize) -> Wheel {
    Wheel {
        slots: entries(kind)
            .iter()
            .take(count)
            .map(|(title, symbol)| Slot {
                id: title.to_lowercase().replace(' ', "_"),
                title: (*title).to_owned(),
                symbol: symbol.map(str::to_owned),
            })
            .collect(),
        caption: match kind {
            Kind::Formats => "Convert 3 files",
            Kind::Tools => "Tools for 3 files",
            Kind::Actions => "3 files",
        }
        .to_owned(),
    }
}

fn main() {
    let mtm = MainThreadMarker::new().expect("main thread");
    let folder = PathBuf::from(env::args().nth(1).unwrap_or_else(|| "/tmp/wheel".into()));
    fs::create_dir_all(folder.join("opaque")).expect("make the folder");
    app::run_accessory(mtm, move || {
        let mut failed = 0;
        for (kind, kind_name) in [
            (Kind::Formats, "formats"),
            (Kind::Tools, "tools"),
            (Kind::Actions, "actions"),
        ] {
            for count in [4, 6, 9, 12] {
                for (hover, hover_name) in [(None, "rest"), (Some(1), "hover")] {
                    let name = format!("{kind_name}-{count}-{hover_name}.png");
                    let wheel = wheel(kind, count);
                    let glass = wheel_ui::snapshot_png(mtm, kind, &wheel, hover)
                        .and_then(|png| on_colour(&png));
                    let opaque = wheel_ui::snapshot_opaque_png(mtm, kind, &wheel, hover);
                    for (png, path) in [
                        (glass, folder.join(&name)),
                        (opaque, folder.join("opaque").join(&name)),
                    ] {
                        match png {
                            Some(png) => {
                                fs::write(&path, &png).expect("write png");
                                println!("wrote {} ({} bytes)", path.display(), png.len());
                            }
                            None => {
                                failed += 1;
                                eprintln!("{}: the snapshot failed", path.display());
                            }
                        }
                    }
                }
            }
        }
        if failed > 0 {
            std::process::exit(1);
        }
        app::quit(mtm);
    });
}
