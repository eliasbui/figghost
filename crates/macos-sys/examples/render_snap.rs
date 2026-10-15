//! Draws the drop zones of window snapping over a pretend desktop into PNG files, without showing anything on
//! screen: the row is drawn from its own view, then composed over a backdrop with a menu bar, a window and, for
//! the notched case, the cutout.
//! `cargo run -p macos-sys --example render_snap -- /tmp/snap`

use block2::RcBlock;
use core_window::{
    Layout, Rect,
    snap::{self, Display},
};
use macos_sys::{
    MainThreadMarker,
    snap_ui::{ZonesPanel, ZonesSpec},
    style,
    window::Frame,
};
use objc2::runtime::Bool;
use objc2_app_kit::{
    NSApplication, NSApplicationActivationPolicy, NSBezierPath, NSBitmapImageFileType,
    NSBitmapImageRep, NSColor, NSGradient, NSImage,
};
use objc2_foundation::{NSDictionary, NSPoint, NSRect, NSSize};
use std::{env, fs, path::PathBuf};

fn spec(display: &Display, layouts: &[Layout]) -> ZonesSpec {
    let panel = display.panel(layouts.len());
    ZonesSpec {
        label: "Snap zones".into(),
        width: panel.w,
        height: panel.h,
        cells: display
            .cells(layouts.len())
            .into_iter()
            .map(|c| Frame {
                x: c.x - panel.x,
                y: c.y - panel.y,
                w: c.w,
                h: c.h,
            })
            .collect(),
        glyphs: layouts
            .iter()
            .map(|l| {
                let g = snap::glyph(*l);
                [g.x, g.y, g.w, g.h]
            })
            .collect(),
    }
}

fn display(cutout: f64, inset: f64) -> Display {
    let frame = Rect::new(0.0, 0.0, 900.0, 260.0);
    Display {
        frame,
        area: Rect::new(0.0, 24.0, 900.0, 236.0),
        cutout,
        inset,
    }
}

struct Scene {
    name: &'static str,
    layouts: Vec<Layout>,
    hover: Option<usize>,
    cutout: f64,
    inset: f64,
    hardware: bool,
    dark: bool,
}

fn compose(zones_png: Vec<u8>, scene: &Scene, panel: Rect) -> Vec<u8> {
    let size = NSSize::new(900.0, 260.0);
    let data = objc2_foundation::NSData::with_bytes(&zones_png);
    let zones = NSImage::initWithData(objc2::AnyThread::alloc(), &data).expect("zones image");
    let (dark, hardware, cutout) = (scene.dark, scene.hardware, scene.cutout);
    let draw = RcBlock::new(move |_: NSRect| {
        // A desktop.
        let (a, b) = if dark {
            ((0.05, 0.08, 0.2), (0.3, 0.12, 0.42))
        } else {
            ((0.72, 0.8, 0.98), (0.98, 0.84, 0.9))
        };
        let top = NSColor::colorWithSRGBRed_green_blue_alpha(a.0, a.1, a.2, 1.0);
        let bottom = NSColor::colorWithSRGBRed_green_blue_alpha(b.0, b.1, b.2, 1.0);
        if let Some(g) =
            NSGradient::initWithStartingColor_endingColor(objc2::AnyThread::alloc(), &top, &bottom)
        {
            g.drawInRect_angle(NSRect::new(NSPoint::ZERO, size), -45.0);
        }
        // A menu bar along the top (Cocoa coordinates: y up).
        style::white(if dark { 0.14 } else { 0.55 }).setFill();
        NSBezierPath::fillRect(NSRect::new(
            NSPoint::new(0.0, size.height - 24.0),
            NSSize::new(size.width, 24.0),
        ));
        // A window being dragged, under the row.
        let window = NSRect::new(NSPoint::new(250.0, 20.0), NSSize::new(400.0, 150.0));
        style::fill_rounded(window, 10.0, &style::white(if dark { 0.18 } else { 0.96 }));
        style::fill_rounded(
            NSRect::new(NSPoint::new(250.0, 140.0), NSSize::new(400.0, 30.0)),
            10.0,
            &style::black(if dark { 0.35 } else { 0.08 }),
        );
        // The hardware cutout.
        if hardware {
            style::black(1.0).setFill();
            NSBezierPath::fillRect(NSRect::new(
                NSPoint::new((size.width - cutout) / 2.0, size.height - 32.0),
                NSSize::new(cutout, 32.0),
            ));
        }
        // The row hangs from the top edge.
        zones.drawInRect(NSRect::new(
            NSPoint::new(panel.x, size.height - panel.h),
            NSSize::new(panel.w, panel.h),
        ));
        Bool::YES
    });
    let image = NSImage::imageWithSize_flipped_drawingHandler(size, false, &draw);
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
    NSApplication::sharedApplication(mtm)
        .setActivationPolicy(NSApplicationActivationPolicy::Accessory);
    let dir = PathBuf::from(env::args().nth(1).unwrap_or_else(|| "/tmp/snap".into()));
    fs::create_dir_all(&dir).expect("output directory");
    let six = snap::DEFAULT_ZONES.to_vec();
    let scenes = [
        Scene {
            name: "plain-rest",
            layouts: six.clone(),
            hover: None,
            cutout: 200.0,
            inset: 10.0,
            hardware: false,
            dark: false,
        },
        Scene {
            name: "plain-hover",
            layouts: six.clone(),
            hover: Some(1),
            cutout: 200.0,
            inset: 10.0,
            hardware: false,
            dark: true,
        },
        Scene {
            name: "notch-hover",
            layouts: six.clone(),
            hover: Some(3),
            cutout: 185.0,
            inset: 38.0,
            hardware: true,
            dark: false,
        },
        Scene {
            name: "notch-two",
            layouts: vec![Layout::Left, Layout::Right],
            hover: Some(0),
            cutout: 185.0,
            inset: 38.0,
            hardware: true,
            dark: true,
        },
        Scene {
            name: "plain-eight",
            layouts: snap::choose(&[
                "left",
                "right",
                "top",
                "bottom",
                "top_left",
                "top_right",
                "bottom_left",
                "bottom_right",
            ]),
            hover: Some(7),
            cutout: 200.0,
            inset: 10.0,
            hardware: false,
            dark: false,
        },
        Scene {
            name: "plain-maximize",
            layouts: six.clone(),
            hover: Some(5),
            cutout: 200.0,
            inset: 10.0,
            hardware: false,
            dark: false,
        },
    ];
    let panel_view = ZonesPanel::new(mtm);
    for scene in &scenes {
        let d = display(scene.cutout, scene.inset);
        let s = spec(&d, &scene.layouts);
        let png = panel_view.snapshot_png(&s, scene.hover).expect("row png");
        let out = compose(png, scene, d.panel(scene.layouts.len()));
        let path = dir.join(format!("{}.png", scene.name));
        fs::write(&path, out).expect("write png");
        println!("{}", path.display());
    }
}
