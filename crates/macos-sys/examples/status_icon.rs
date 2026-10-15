//! Writes the icon of the menu bar item to PNG files at 1x and 2x, as a template image is stored: black on a
//! clear background, because the menu bar reads only its shape.
//! `cargo run -p macos-sys --example status_icon -- /tmp/status-icon`

use macos_sys::status_item::ghost_icon;
use objc2::AnyThread;
use objc2_app_kit::{
    NSBitmapImageFileType, NSBitmapImageRep, NSDeviceRGBColorSpace, NSGraphicsContext,
};
use objc2_foundation::{NSDictionary, NSPoint, NSRect};
use std::{env, fs, path::Path};

fn write_png(scale: isize, path: &Path) {
    let icon = ghost_icon();
    let size = icon.size();
    // SAFETY: a null plane pointer asks AppKit to allocate the pixels; the other arguments describe an
    // ordinary interleaved RGBA bitmap, and the colour space constant lives for the process.
    let rep = unsafe {
        NSBitmapImageRep::initWithBitmapDataPlanes_pixelsWide_pixelsHigh_bitsPerSample_samplesPerPixel_hasAlpha_isPlanar_colorSpaceName_bytesPerRow_bitsPerPixel(
            NSBitmapImageRep::alloc(),
            std::ptr::null_mut(),
            size.width as isize * scale,
            size.height as isize * scale,
            8,
            4,
            true,
            false,
            NSDeviceRGBColorSpace,
            0,
            0,
        )
    }
    .expect("bitmap");
    // The size in points is what makes the context draw at this scale.
    rep.setSize(size);
    let context = NSGraphicsContext::graphicsContextWithBitmapImageRep(&rep).expect("context");
    NSGraphicsContext::saveGraphicsState_class();
    NSGraphicsContext::setCurrentContext(Some(&context));
    icon.drawInRect(NSRect::new(NSPoint::ZERO, size));
    context.flushGraphics();
    NSGraphicsContext::restoreGraphicsState_class();
    // SAFETY: an empty properties dictionary is valid for PNG output.
    let png = unsafe {
        rep.representationUsingType_properties(NSBitmapImageFileType::PNG, &NSDictionary::new())
    }
    .expect("png");
    fs::write(path, png.to_vec()).expect("write png");
    println!(
        "{} ({}x{} pixels, template: {})",
        path.display(),
        rep.pixelsWide(),
        rep.pixelsHigh(),
        icon.isTemplate()
    );
}

fn main() {
    let dir = env::args()
        .nth(1)
        .unwrap_or_else(|| "/tmp/status-icon".into());
    fs::create_dir_all(&dir).expect("output directory");
    for (name, scale) in [("status_icon.png", 1), ("status_icon@2x.png", 2)] {
        write_png(scale, &Path::new(&dir).join(name));
    }
}
