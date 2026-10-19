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
    // Start at top center of the head dome.
    path.moveToPoint(point(47.0, 90.0));
    // Left side of the head dome down to the cheek.
    path.curveToPoint_controlPoint1_controlPoint2(
        point(20.0, 62.0),
        point(32.0, 90.0),
        point(20.0, 77.0),
    );
    // Left flank curving gently inward towards the hem.
    path.curveToPoint_controlPoint1_controlPoint2(
        point(23.5, 30.0),
        point(20.0, 50.0),
        point(22.0, 39.0),
    );
    // First bottom wave (left lobe).
    path.curveToPoint_controlPoint1_controlPoint2(
        point(32.5, 19.0),
        point(24.5, 23.5),
        point(28.0, 19.0),
    );
    path.curveToPoint_controlPoint1_controlPoint2(
        point(41.0, 27.0),
        point(36.5, 19.0),
        point(39.0, 24.0),
    );
    // Second bottom wave (center lobe).
    path.curveToPoint_controlPoint1_controlPoint2(
        point(50.5, 20.0),
        point(43.0, 22.5),
        point(46.5, 20.0),
    );
    path.curveToPoint_controlPoint1_controlPoint2(
        point(59.5, 27.5),
        point(54.5, 20.0),
        point(57.5, 24.5),
    );
    // Third wave sweeping upward into an elegant tail curve.
    path.curveToPoint_controlPoint1_controlPoint2(
        point(83.0, 40.0),
        point(63.0, 23.5),
        point(74.0, 27.0),
    );
    // Rounded tip of the tail.
    path.curveToPoint_controlPoint1_controlPoint2(
        point(78.5, 47.0),
        point(85.5, 43.0),
        point(82.5, 47.0),
    );
    // Inner sweep of the tail into the right flank.
    path.curveToPoint_controlPoint1_controlPoint2(
        point(74.0, 62.0),
        point(75.0, 51.0),
        point(73.5, 56.5),
    );
    // Right side of head back to top center dome.
    path.curveToPoint_controlPoint1_controlPoint2(
        point(47.0, 90.0),
        point(74.0, 77.0),
        point(62.0, 90.0),
    );
    path.closePath();
    path
}

fn draw(px: f64) {
    let scale = px / CANVAS;
    let transform = objc2_foundation::NSAffineTransform::transform();
    transform.scaleBy(scale);
    transform.concat();

    // The tile: macOS squircle with standard icon grid dimensions.
    let tile_rect = NSRect::new(point(100.0, 100.0), NSSize::new(824.0, 824.0));
    let tile = NSBezierPath::bezierPathWithRoundedRect_xRadius_yRadius(tile_rect, 185.0, 185.0);

    // Deep ambient and contact shadows under the tile.
    NSGraphicsContext::saveGraphicsState_class();
    let shadow = NSShadow::new();
    shadow.setShadowOffset(NSSize::new(0.0, -18.0));
    shadow.setShadowBlurRadius(32.0);
    shadow.setShadowColor(Some(&color(0.05, 0.07, 0.20, 0.38)));
    shadow.set();
    color(0.16, 0.25, 0.70, 1.0).setFill();
    tile.fill();
    NSGraphicsContext::restoreGraphicsState_class();

    NSGraphicsContext::saveGraphicsState_class();
    let contact_shadow = NSShadow::new();
    contact_shadow.setShadowOffset(NSSize::new(0.0, -6.0));
    contact_shadow.setShadowBlurRadius(12.0);
    contact_shadow.setShadowColor(Some(&color(0.05, 0.07, 0.20, 0.24)));
    contact_shadow.set();
    tile.fill();
    NSGraphicsContext::restoreGraphicsState_class();

    // Liquid Glass tile interior.
    NSGraphicsContext::saveGraphicsState_class();
    tile.addClip();

    // Restrained 2-hue gradient: luminous royal cobalt-iris into deep midnight navy.
    let tile_gradient = NSGradient::initWithColors(
        NSGradient::alloc(),
        &objc2_foundation::NSArray::from_retained_slice(&[
            color(0.22, 0.38, 0.92, 1.0),
            color(0.14, 0.22, 0.62, 1.0),
            color(0.06, 0.08, 0.24, 1.0),
        ]),
    )
    .expect("tile gradient");
    tile_gradient.drawInRect_angle(tile_rect, -58.0);

    // Soft top light wash: diffuse glass refraction from above.
    NSGradient::initWithStartingColor_endingColor(
        NSGradient::alloc(),
        &color(1.0, 1.0, 1.0, 0.18),
        &color(1.0, 1.0, 1.0, 0.0),
    )
    .expect("top wash")
    .drawInRect_angle(
        NSRect::new(point(100.0, 480.0), NSSize::new(824.0, 444.0)),
        -90.0,
    );

    // Subtle specular glint on the upper glass surface.
    NSGradient::initWithStartingColor_endingColor(
        NSGradient::alloc(),
        &color(1.0, 1.0, 1.0, 0.32),
        &color(1.0, 1.0, 1.0, 0.0),
    )
    .expect("specular glint")
    .drawInRect_angle(
        NSRect::new(point(100.0, 890.0), NSSize::new(824.0, 34.0)),
        -90.0,
    );
    NSGraphicsContext::restoreGraphicsState_class();

    // Fine cut-glass rim around the tile: bright at the top, soft at the bottom.
    NSGraphicsContext::saveGraphicsState_class();
    tile.setLineWidth(3.5);
    color(1.0, 1.0, 1.0, 0.28).setStroke();
    tile.stroke();
    NSGraphicsContext::restoreGraphicsState_class();

    // Ghost transform: optically centered on the tile with generous proportions.
    let ghost_scale = 6.2;
    let place = objc2_foundation::NSAffineTransform::transform();
    place.translateXBy_yBy(512.0 - 51.5 * ghost_scale, 512.0 - 54.5 * ghost_scale - 2.0);
    place.scaleBy(ghost_scale);

    // Primary floating drop shadow under the ghost.
    NSGraphicsContext::saveGraphicsState_class();
    place.concat();
    let glyph = ghost();
    let drop = NSShadow::new();
    drop.setShadowOffset(NSSize::new(0.0, -3.4));
    drop.setShadowBlurRadius(5.8);
    drop.setShadowColor(Some(&color(0.03, 0.04, 0.16, 0.46)));
    drop.set();
    color(1.0, 1.0, 1.0, 1.0).setFill();
    glyph.fill();
    NSGraphicsContext::restoreGraphicsState_class();

    // Secondary contact shadow for grounding depth.
    NSGraphicsContext::saveGraphicsState_class();
    place.concat();
    let contact_drop = NSShadow::new();
    contact_drop.setShadowOffset(NSSize::new(0.0, -1.2));
    contact_drop.setShadowBlurRadius(2.2);
    contact_drop.setShadowColor(Some(&color(0.03, 0.04, 0.16, 0.30)));
    contact_drop.set();
    glyph.fill();
    NSGraphicsContext::restoreGraphicsState_class();

    // Ghost body: subtle vertical gradient from pure white to soft cool mist.
    NSGraphicsContext::saveGraphicsState_class();
    place.concat();
    glyph.addClip();
    let ghost_gradient = NSGradient::initWithStartingColor_endingColor(
        NSGradient::alloc(),
        &color(1.0, 1.0, 1.0, 1.0),
        &color(0.91, 0.93, 0.98, 1.0),
    )
    .expect("ghost gradient");
    ghost_gradient.drawInRect_angle(
        NSRect::new(point(15.0, 15.0), NSSize::new(75.0, 80.0)),
        -90.0,
    );

    // Soft ambient occlusion along bottom hem folds.
    let bottom_shade = NSGradient::initWithStartingColor_endingColor(
        NSGradient::alloc(),
        &color(0.60, 0.68, 0.88, 0.22),
        &color(1.0, 1.0, 1.0, 0.0),
    )
    .expect("bottom shade");
    bottom_shade.drawInRect_angle(
        NSRect::new(point(15.0, 18.0), NSSize::new(75.0, 25.0)),
        90.0,
    );

    // Soft rim highlight inside the ghost dome.
    glyph.setLineWidth(1.6);
    color(1.0, 1.0, 1.0, 0.70).setStroke();
    glyph.stroke();
    NSGraphicsContext::restoreGraphicsState_class();

    // Eyes with presence: expressive dark midnight capsules with catchlight gleams.
    NSGraphicsContext::saveGraphicsState_class();
    place.concat();
    color(0.08, 0.10, 0.24, 1.0).setFill();
    let eye_w = if px <= 32.0 { 12.5 } else { 10.8 };
    let eye_h = if px <= 32.0 { 16.5 } else { 15.5 };
    let left_eye_x = 47.0 - 9.8 - eye_w / 2.0;
    let right_eye_x = 47.0 + 9.8 - eye_w / 2.0;
    let eye_y = 48.5;
    let left_eye_rect = NSRect::new(point(left_eye_x, eye_y), NSSize::new(eye_w, eye_h));
    let right_eye_rect = NSRect::new(point(right_eye_x, eye_y), NSSize::new(eye_w, eye_h));
    let left_eye = NSBezierPath::bezierPathWithOvalInRect(left_eye_rect);
    let right_eye = NSBezierPath::bezierPathWithOvalInRect(right_eye_rect);
    left_eye.fill();
    right_eye.fill();

    // Catchlight gleams inside the eyes at large sizes.
    if px >= 64.0 {
        color(1.0, 1.0, 1.0, 0.88).setFill();
        let gleam_w = eye_w * 0.32;
        let gleam_h = eye_h * 0.26;
        NSBezierPath::bezierPathWithOvalInRect(NSRect::new(
            point(left_eye_x + eye_w * 0.50, eye_y + eye_h * 0.54),
            NSSize::new(gleam_w, gleam_h),
        ))
        .fill();
        NSBezierPath::bezierPathWithOvalInRect(NSRect::new(
            point(right_eye_x + eye_w * 0.50, eye_y + eye_h * 0.54),
            NSSize::new(gleam_w, gleam_h),
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
