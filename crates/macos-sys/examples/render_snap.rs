//! Draws the drop zones of window snapping over a pretend desktop into PNG files, without showing anything on
//! screen: the zones are drawn from their own view, then composed over a backdrop with a menu bar, a Dock, the
//! window being dragged, the cutout on a notched screen and, while a zone is lit, the footprint where the window
//! will land. One whole screen per zone of the default list (`zone-*.png`), then close-ups of the shape at two
//! pixels per point (`close-*.png`). The captions mirror `app-ui/src/features/snap/text.rs`; `app-ui/snap_check`
//! draws the live panel with the real ones.
//! `cargo run -p macos-sys --example render_snap -- /tmp/snap`

use block2::RcBlock;
use core_window::{
    Layout, Rect,
    snap::{self, Display},
};
use macos_sys::{
    MainThreadMarker,
    snap_ui::{self, ZonesPanel, ZonesSpec},
    style,
    window::Frame,
};
use objc2::runtime::Bool;
use objc2_app_kit::{
    NSAffineTransformNSAppKitAdditions, NSApplication, NSApplicationActivationPolicy, NSBezierPath,
    NSBitmapImageFileType, NSBitmapImageRep, NSColor, NSGradient, NSImage,
};
use objc2_foundation::{NSAffineTransform, NSDictionary, NSPoint, NSRect, NSSize};
use std::{env, fs, path::PathBuf};

const SCREEN: (f64, f64) = (1440.0, 900.0);
const MENU_BAR: f64 = 24.0;
const DOCK: f64 = 76.0;

/// The short names of `text.rs`, English then Vietnamese.
fn caption(layout: Layout, vietnamese: bool) -> &'static str {
    let (en, vi) = match layout {
        Layout::Left => ("Left", "Trái"),
        Layout::Right => ("Right", "Phải"),
        Layout::Top => ("Top", "Trên"),
        Layout::Bottom => ("Bottom", "Dưới"),
        Layout::TopLeft => ("Top left", "Trên trái"),
        Layout::TopRight => ("Top right", "Trên phải"),
        Layout::BottomLeft => ("Bottom left", "Dưới trái"),
        Layout::BottomRight => ("Bottom right", "Dưới phải"),
        Layout::Maximize => ("Full", "Tối đa"),
        Layout::Center => ("Center", "Giữa"),
        Layout::FirstThird => ("Left third", "1/3 trái"),
        Layout::CenterThird => ("Center third", "1/3 giữa"),
        Layout::LastThird => ("Right third", "1/3 phải"),
    };
    if vietnamese { vi } else { en }
}

fn spec(display: &Display, layouts: &[Layout], vietnamese: bool) -> ZonesSpec {
    let panel = display.panel(layouts);
    ZonesSpec {
        label: "Snap zones".into(),
        width: panel.w,
        height: panel.h,
        cells: display
            .cells(layouts)
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
        names: layouts
            .iter()
            .map(|l| caption(*l, vietnamese).to_owned())
            .collect(),
    }
}

/// The pretend screen, with the usable area between the menu bar and the Dock.
fn display(notched: bool) -> Display {
    let frame = Rect::new(0.0, 0.0, SCREEN.0, SCREEN.1);
    Display {
        frame,
        area: Rect::new(0.0, MENU_BAR, SCREEN.0, SCREEN.1 - MENU_BAR - DOCK),
        cutout: if notched { 185.0 } else { 200.0 },
        inset: if notched { 38.0 } else { 10.0 },
    }
}

struct Scene {
    layouts: Vec<Layout>,
    hover: Option<usize>,
    notched: bool,
    dark: bool,
    vietnamese: bool,
}

/// A rect in top-left coordinates of the pretend screen, in the bottom-left ones of drawing.
fn cocoa(r: Rect) -> NSRect {
    NSRect::new(
        NSPoint::new(r.x, SCREEN.1 - r.y - r.h),
        NSSize::new(r.w, r.h),
    )
}

fn color(r: f64, g: f64, b: f64, a: f64) -> objc2::rc::Retained<NSColor> {
    NSColor::colorWithSRGBRed_green_blue_alpha(r, g, b, a)
}

/// Draws `region` of the scene (top-left coordinates of the screen) at `scale` pixels per point.
fn render(zones_png: &[u8], scene: &Scene, region: Rect, scale: f64) -> Vec<u8> {
    let d = display(scene.notched);
    let panel = d.panel(&scene.layouts);
    let landing = scene.hover.map(|i| d.place(scene.layouts[i], d.area));
    let data = objc2_foundation::NSData::with_bytes(zones_png);
    let zones = NSImage::initWithData(objc2::AnyThread::alloc(), &data).expect("zones image");
    let (dark, notched, cutout) = (scene.dark, scene.notched, d.cutout);
    let size = NSSize::new(region.w * scale, region.h * scale);
    let draw = RcBlock::new(move |_: NSRect| {
        let place = NSAffineTransform::transform();
        place.scaleBy(scale);
        place.translateXBy_yBy(-region.x, -(SCREEN.1 - region.y - region.h));
        place.concat();
        // A desktop.
        let (a, b) = if dark {
            ((0.05, 0.08, 0.2), (0.3, 0.12, 0.42))
        } else {
            ((0.72, 0.8, 0.98), (0.98, 0.84, 0.9))
        };
        if let Some(g) = NSGradient::initWithStartingColor_endingColor(
            objc2::AnyThread::alloc(),
            &color(a.0, a.1, a.2, 1.0),
            &color(b.0, b.1, b.2, 1.0),
        ) {
            g.drawInRect_angle(
                NSRect::new(NSPoint::ZERO, NSSize::new(SCREEN.0, SCREEN.1)),
                -45.0,
            );
        }
        // Another app's window, left where it was.
        let other = cocoa(Rect::new(90.0, 420.0, 520.0, 330.0));
        style::fill_rounded(other, 12.0, &style::white(if dark { 0.16 } else { 0.9 }));
        // The window being dragged: its title bar under the pointer, just below the menu bar.
        let window = Rect::new(470.0, MENU_BAR + 6.0, 500.0, 330.0);
        style::fill_rounded(
            cocoa(window),
            12.0,
            &style::white(if dark { 0.22 } else { 0.97 }),
        );
        style::fill_rounded(
            cocoa(Rect::new(window.x, window.y, window.w, 34.0)),
            12.0,
            &style::black(if dark { 0.3 } else { 0.07 }),
        );
        // A menu bar along the top and a Dock at the bottom.
        style::white(if dark { 0.14 } else { 0.55 }).setFill();
        NSBezierPath::fillRect(cocoa(Rect::new(0.0, 0.0, SCREEN.0, MENU_BAR)));
        style::fill_rounded(
            cocoa(Rect::new(420.0, SCREEN.1 - DOCK + 8.0, 600.0, DOCK - 16.0)),
            18.0,
            &style::white(if dark { 0.2 } else { 0.45 }),
        );
        // Where the window will land, over every window and under the zones.
        if let Some(r) = landing {
            snap_ui::paint_footprint(cocoa(r));
        }
        // The hardware cutout.
        if notched {
            style::black(1.0).setFill();
            NSBezierPath::fillRect(cocoa(Rect::new(
                (SCREEN.0 - cutout) / 2.0,
                0.0,
                cutout,
                32.0,
            )));
        }
        // The zones hang from the top edge.
        zones.drawInRect(cocoa(panel));
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
    let panel_view = ZonesPanel::new(mtm);
    let write = |name: String, scene: &Scene, close_up: bool| {
        let d = display(scene.notched);
        let png = panel_view
            .snapshot_png(&spec(&d, &scene.layouts, scene.vietnamese), scene.hover)
            .expect("zones png");
        let out = if close_up {
            // The shape with a margin, cut from the top of the screen.
            let p = d.panel(&scene.layouts);
            let region = Rect::new(p.x - 60.0, 0.0, p.w + 120.0, p.h + 50.0);
            render(&png, scene, region, 2.0)
        } else {
            render(&png, scene, d.frame, 1.0)
        };
        let path = dir.join(format!("{name}.png"));
        fs::write(&path, out).expect("write png");
        println!("{}", path.display());
    };
    let ten = snap::DEFAULT_ZONES.to_vec();
    for (i, layout) in ten.iter().enumerate() {
        let scene = Scene {
            layouts: ten.clone(),
            hover: Some(i),
            notched: true,
            dark: i % 2 == 1,
            vietnamese: false,
        };
        write(format!("zone-{i:02}-{}", layout.key()), &scene, false);
    }
    let close_ups = [
        ("close-rest", ten.clone(), None, true, false, false),
        ("close-hover-full", ten.clone(), Some(2), true, true, false),
        (
            "close-hover-quarter",
            ten.clone(),
            Some(8),
            true,
            false,
            false,
        ),
        ("close-vietnamese", ten.clone(), Some(4), true, true, true),
        ("close-notchless", ten.clone(), Some(0), false, false, false),
        (
            "close-all-twelve",
            snap::OFFERED.to_vec(),
            Some(7),
            false,
            true,
            false,
        ),
        (
            "close-quarters-only",
            ten[6..].to_vec(),
            Some(1),
            true,
            false,
            false,
        ),
        (
            "close-two",
            vec![Layout::Left, Layout::Right],
            Some(0),
            true,
            true,
            false,
        ),
    ];
    for (name, layouts, hover, notched, dark, vietnamese) in close_ups {
        let scene = Scene {
            layouts,
            hover,
            notched,
            dark,
            vietnamese,
        };
        write(name.into(), &scene, true);
    }
}
