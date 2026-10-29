//! Draws the figures of the System tab as the menu bar shows them into PNGs, with sample figures, on a light and a
//! dark bar, plain and in colour, without showing anything on screen.
//! `cargo run -p macos-sys --example render_system_bar -- /tmp/system_bar`

use block2::RcBlock;
use macos_sys::system_bar::{self, Cell};
use objc2::runtime::Bool;
use objc2_app_kit::{
    NSAppearance, NSAppearanceNameAqua, NSAppearanceNameDarkAqua, NSBezierPath,
    NSBitmapImageFileType, NSBitmapImageRep, NSColor, NSCompositingOperation, NSGraphicsContext,
    NSImage,
};
use objc2_foundation::{NSDictionary, NSPoint, NSRect, NSSize};
use std::{cell::RefCell, env, fs, path::PathBuf};

fn text(label: &str, value: &str, widest: &str) -> Cell {
    Cell::Text {
        label: label.into(),
        value: value.into(),
        widest: widest.into(),
    }
}

fn stacked(label: &str, value: &str, widest: &str) -> Cell {
    Cell::Stacked {
        label: label.into(),
        value: value.into(),
        widest: widest.into(),
    }
}

fn rates(up: &str, down: &str) -> Cell {
    Cell::Rates {
        up: up.into(),
        down: down.into(),
        widest: "88.8 MB/s".into(),
    }
}

fn graph(seed: f64) -> Cell {
    Cell::Graph {
        history: (0..30)
            .map(|i| (0.45 + 0.35 * (i as f64 * 0.5 + seed).sin()).clamp(0.0, 1.0))
            .collect(),
    }
}

fn cores() -> Cell {
    Cell::Cores {
        loads: vec![0.62, 0.4, 0.71, 0.3, 0.92, 0.35, 0.88, 0.41, 0.77, 0.3],
        slow: 4,
    }
}

/// Points to pixels, as on a Retina screen. A picture drawn by a handler is drawn again at the size it is put at.
const SCALE: f64 = 2.0;

/// The picture of `cells` on a strip of menu bar, as a light or a dark bar paints it.
fn strip(cells: &[Cell], color: bool, dark: bool) -> Vec<u8> {
    let image = system_bar::image(cells, color);
    let size = image.size();
    let (pad, edge) = (12.0, 5.0);
    let whole = NSSize::new(
        (size.width + 2.0 * pad) * SCALE,
        (system_bar::HEIGHT + 2.0 * edge) * SCALE,
    );
    let at = NSRect::new(
        NSPoint::new(pad * SCALE, edge * SCALE),
        NSSize::new(size.width * SCALE, size.height * SCALE),
    );
    // A template is only a shape: paint it as the menu bar does, white on dark and black on light.
    let shape = image.clone();
    let paint = RcBlock::new(move |all: NSRect| {
        shape.drawInRect(all);
        if let Some(context) = NSGraphicsContext::currentContext() {
            context.setCompositingOperation(NSCompositingOperation::SourceAtop);
        }
        NSColor::colorWithWhite_alpha(if dark { 1.0 } else { 0.0 }, 1.0).setFill();
        NSBezierPath::fillRect(all);
        Bool::YES
    });
    let painted = NSImage::imageWithSize_flipped_drawingHandler(at.size, false, &paint);
    let draw = RcBlock::new(move |all: NSRect| {
        NSColor::colorWithWhite_alpha(if dark { 0.16 } else { 0.9 }, 1.0).setFill();
        NSBezierPath::fillRect(all);
        if color {
            image.drawInRect(at)
        } else {
            painted.drawInRect(at)
        };
        Bool::YES
    });
    let canvas = NSImage::imageWithSize_flipped_drawingHandler(whole, false, &draw);
    let out = RefCell::new(Vec::new());
    let render = RcBlock::new(|| *out.borrow_mut() = png(&canvas).expect("png"));
    // SAFETY: extern static appearance names.
    let name = unsafe {
        if dark {
            NSAppearanceNameDarkAqua
        } else {
            NSAppearanceNameAqua
        }
    };
    NSAppearance::appearanceNamed(name)
        .expect("a system appearance")
        .performAsCurrentDrawingAppearance(&render);
    drop(render);
    out.into_inner()
}

fn png(image: &NSImage) -> Option<Vec<u8>> {
    let tiff = image.TIFFRepresentation()?;
    let rep = NSBitmapImageRep::imageRepWithData(&tiff)?;
    // SAFETY: an empty properties dictionary is valid for PNG output.
    let png = unsafe {
        rep.representationUsingType_properties(NSBitmapImageFileType::PNG, &NSDictionary::new())
    }?;
    Some(png.to_vec())
}

fn main() {
    let dir = PathBuf::from(
        env::args()
            .nth(1)
            .unwrap_or_else(|| "/tmp/system_bar".into()),
    );
    fs::create_dir_all(&dir).expect("output directory");
    let scenes: Vec<(&str, Vec<Cell>)> = vec![
        (
            "stacked_vi",
            vec![
                stacked("CPU", "23%", "100%"),
                stacked("RAM", "79%", "100%"),
                rates("3.9 MB/s", "48.2 MB/s"),
                stacked("NHIỆT", "62°", "100°"),
            ],
        ),
        (
            "charts_en",
            vec![
                graph(0.0),
                stacked("CPU", "7%", "100%"),
                cores(),
                rates("0 B/s", "84 KB/s"),
            ],
        ),
        (
            "text_en",
            vec![
                text("CPU", "23%", "100%"),
                text("Mem", "79%", "100%"),
                text("Net", "52.1 MB/s", "88.8 MB/s"),
            ],
        ),
        (
            "missing_en",
            vec![
                stacked("GPU", "–", "100%"),
                stacked("DISK", "227 GB", "88.8 GB"),
            ],
        ),
    ];
    for (name, cells) in &scenes {
        for (look, color) in [("plain", false), ("colour", true)] {
            for (bar, dark) in [("light", false), ("dark", true)] {
                let path = dir.join(format!("{name}_{look}_{bar}.png"));
                fs::write(&path, strip(cells, color, dark)).expect("write png");
                println!("wrote {}", path.display());
            }
        }
    }
}
