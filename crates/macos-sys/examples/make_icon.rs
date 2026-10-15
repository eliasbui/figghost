//! Draws the app icon at every size macOS asks for and writes an `.iconset` folder; `iconutil -c icns`
//! turns it into the `.icns` file the bundle carries.
//! `cargo run -p macos-sys --example make_icon -- /tmp/Figghost.iconset`

use block2::RcBlock;
use macos_sys::MainThreadMarker;
use objc2::{AnyThread, runtime::Bool};
use objc2_app_kit::{
    NSAffineTransformNSAppKitAdditions, NSApplication, NSApplicationActivationPolicy, NSBezierPath,
    NSBitmapImageFileType, NSBitmapImageRep, NSColor, NSGradient, NSGraphicsContext, NSImage,
    NSShadow,
};
use objc2_foundation::{NSDictionary, NSPoint, NSRect, NSSize};
use std::{env, fs, path::Path};

/// Everything is laid out on a 1024 point canvas and scaled to the size being written.
const CANVAS: f64 = 1024.0;

fn color(r: f64, g: f64, b: f64, a: f64) -> objc2::rc::Retained<NSColor> {
    NSColor::colorWithSRGBRed_green_blue_alpha(r, g, b, a)
}

fn point(x: f64, y: f64) -> NSPoint {
    NSPoint::new(x, y)
}

/// The ghost, in a 100 by 100 box with the origin at the bottom left.
fn ghost() -> objc2::rc::Retained<NSBezierPath> {
    let path = NSBezierPath::bezierPath();
    path.moveToPoint(point(20.0, 20.0));
    path.lineToPoint(point(20.0, 60.0));
    path.appendBezierPathWithArcWithCenter_radius_startAngle_endAngle_clockwise(
        point(50.0, 60.0),
        30.0,
        180.0,
        0.0,
        true,
    );
    path.lineToPoint(point(80.0, 20.0));
    // Three rounded teeth along the bottom edge, right to left.
    for centre in [70.0, 50.0, 30.0] {
        path.appendBezierPathWithArcWithCenter_radius_startAngle_endAngle_clockwise(
            point(centre, 20.0),
            10.0,
            0.0,
            180.0,
            true,
        );
    }
    path.closePath();
    path
}

fn draw(px: f64) {
    let scale = px / CANVAS;
    let transform = objc2_foundation::NSAffineTransform::transform();
    transform.scaleBy(scale);
    transform.concat();

    // The tile: a rounded square with the margin the icon grid asks for, and a soft shadow below it.
    let tile_rect = NSRect::new(point(100.0, 100.0), NSSize::new(824.0, 824.0));
    let tile = NSBezierPath::bezierPathWithRoundedRect_xRadius_yRadius(tile_rect, 185.0, 185.0);
    NSGraphicsContext::saveGraphicsState_class();
    let shadow = NSShadow::new();
    shadow.setShadowOffset(NSSize::new(0.0, -14.0));
    shadow.setShadowBlurRadius(26.0);
    shadow.setShadowColor(Some(&color(0.12, 0.1, 0.4, 0.32)));
    shadow.set();
    // The same tone as the middle of the gradient, so the anti-aliased edge never shows a dark outline.
    color(0.47, 0.36, 0.98, 1.0).setFill();
    tile.fill();
    NSGraphicsContext::restoreGraphicsState_class();

    NSGraphicsContext::saveGraphicsState_class();
    tile.addClip();
    // Colour: blue at the top left through violet to magenta at the bottom right.
    let body = NSGradient::initWithColors(
        NSGradient::alloc(),
        &objc2_foundation::NSArray::from_retained_slice(&[
            color(0.36, 0.62, 1.0, 1.0),
            color(0.47, 0.36, 0.98, 1.0),
            color(0.78, 0.40, 0.96, 1.0),
        ]),
    )
    .expect("gradient");
    body.drawInRect_angle(tile_rect, -52.0);
    // Light from above: a glass sheen over the upper half, brightest at the top edge.
    let sheen = NSBezierPath::bezierPathWithOvalInRect(NSRect::new(
        point(-120.0, 470.0),
        NSSize::new(1264.0, 980.0),
    ));
    sheen.addClip();
    NSGradient::initWithStartingColor_endingColor(
        NSGradient::alloc(),
        &color(1.0, 1.0, 1.0, 0.42),
        &color(1.0, 1.0, 1.0, 0.0),
    )
    .expect("sheen")
    .drawInRect_angle(
        NSRect::new(point(0.0, 470.0), NSSize::new(1024.0, 560.0)),
        -90.0,
    );
    NSGraphicsContext::restoreGraphicsState_class();

    // A fine bright rim on the edge, and a darker one inside it near the bottom, like cut glass.
    NSGraphicsContext::saveGraphicsState_class();
    tile.setLineWidth(5.0);
    color(1.0, 1.0, 1.0, 0.55).setStroke();
    tile.stroke();
    NSGraphicsContext::restoreGraphicsState_class();

    // The ghost, with a soft shadow, scaled to sit in the middle of the tile.
    NSGraphicsContext::saveGraphicsState_class();
    let place = objc2_foundation::NSAffineTransform::transform();
    place.translateXBy_yBy(512.0 - 50.0 * 5.6, 512.0 - 50.0 * 5.6 - 10.0);
    place.scaleBy(5.6);
    place.concat();
    let glyph = ghost();
    let drop = NSShadow::new();
    drop.setShadowOffset(NSSize::new(0.0, -2.2));
    drop.setShadowBlurRadius(4.0);
    drop.setShadowColor(Some(&color(0.1, 0.05, 0.4, 0.45)));
    drop.set();
    color(1.0, 1.0, 1.0, 1.0).setFill();
    glyph.fill();
    NSGraphicsContext::restoreGraphicsState_class();
    NSGraphicsContext::saveGraphicsState_class();
    place.concat();
    color(0.30, 0.24, 0.80, 1.0).setFill();
    for x in [40.5, 59.5] {
        NSBezierPath::bezierPathWithOvalInRect(NSRect::new(
            point(x - 4.6, 53.0),
            NSSize::new(9.2, 11.0),
        ))
        .fill();
    }
    NSGraphicsContext::restoreGraphicsState_class();
}

fn write_png(px: u32, path: &Path) {
    let size = f64::from(px);
    let image = NSImage::imageWithSize_flipped_drawingHandler(
        NSSize::new(size, size),
        false,
        &RcBlock::new(move |_: NSRect| {
            draw(size);
            Bool::YES
        }),
    );
    let tiff = image.TIFFRepresentation().expect("tiff");
    let rep = NSBitmapImageRep::imageRepWithData(&tiff).expect("bitmap");
    // SAFETY: an empty properties dictionary is valid for PNG output.
    let png = unsafe {
        rep.representationUsingType_properties(NSBitmapImageFileType::PNG, &NSDictionary::new())
    }
    .expect("png");
    fs::write(path, png.to_vec()).expect("write png");
    println!(
        "{} ({}x{})",
        path.display(),
        rep.pixelsWide(),
        rep.pixelsHigh()
    );
}

fn main() {
    let mtm = MainThreadMarker::new().expect("main thread");
    NSApplication::sharedApplication(mtm)
        .setActivationPolicy(NSApplicationActivationPolicy::Accessory);
    let dir = env::args()
        .nth(1)
        .unwrap_or_else(|| "/tmp/Figghost.iconset".into());
    fs::create_dir_all(&dir).expect("output directory");
    for (name, px) in [
        ("icon_16x16", 16),
        ("icon_16x16@2x", 32),
        ("icon_32x32", 32),
        ("icon_32x32@2x", 64),
        ("icon_128x128", 128),
        ("icon_128x128@2x", 256),
        ("icon_256x256", 256),
        ("icon_256x256@2x", 512),
        ("icon_512x512", 512),
        ("icon_512x512@2x", 1024),
    ] {
        write_png(px, &Path::new(&dir).join(format!("{name}.png")));
    }
}
