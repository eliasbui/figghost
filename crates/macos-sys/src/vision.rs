//! Background removal with the Vision framework. Text recognition is not here: the screenshots have their own, in
//! `ocr.rs`.

use objc2::{AnyThread, rc::Retained, runtime::AnyClass};
use objc2_app_kit::{
    NSBitmapImageFileType, NSBitmapImageRep, NSDeviceRGBColorSpace, NSGraphicsContext,
};
use objc2_core_video::{
    CVPixelBuffer, CVPixelBufferGetBaseAddress, CVPixelBufferGetBytesPerRow,
    CVPixelBufferGetHeight, CVPixelBufferGetPixelFormatType, CVPixelBufferGetWidth,
    CVPixelBufferLockBaseAddress, CVPixelBufferLockFlags, CVPixelBufferUnlockBaseAddress,
};
use objc2_foundation::{
    NSArray, NSData, NSDictionary, NSOperatingSystemVersion, NSPoint, NSProcessInfo, NSRect, NSSize,
};
use objc2_vision::{VNGenerateForegroundInstanceMaskRequest, VNImageRequestHandler, VNRequest};

/// Largest side accepted, in pixels: a bound on the memory a hostile or huge image can take.
const MAX_SIDE: usize = 16_384;
/// kCVPixelFormatType_OneComponent32Float and kCVPixelFormatType_OneComponent8, the mask formats.
const FORMAT_FLOAT32: u32 = u32::from_be_bytes(*b"L00f");
const FORMAT_GRAY8: u32 = u32::from_be_bytes(*b"L008");

/// Whether the system can separate the main subject of an image (macOS 14 and later).
pub fn background_removal_available() -> bool {
    NSProcessInfo::processInfo().isOperatingSystemAtLeastVersion(NSOperatingSystemVersion {
        majorVersion: 14,
        minorVersion: 0,
        patchVersion: 0,
    }) && AnyClass::get(c"VNGenerateForegroundInstanceMaskRequest").is_some()
}

fn request_handler(png: &[u8]) -> Result<Retained<VNImageRequestHandler>, String> {
    if png.is_empty() {
        return Err("the image is empty".into());
    }
    let data = NSData::with_bytes(png);
    Ok(VNImageRequestHandler::initWithData_options(
        VNImageRequestHandler::alloc(),
        &data,
        &NSDictionary::new(),
    ))
}

/// The PNG with everything but the main subject made transparent, at the size of the original.
pub fn remove_background(png: &[u8]) -> Result<Vec<u8>, String> {
    if !background_removal_available() {
        return Err("background removal needs macOS 14 or later".into());
    }
    let handler = request_handler(png)?;
    // SAFETY: plain construction of a request object.
    let request = unsafe { VNGenerateForegroundInstanceMaskRequest::new() };
    let as_request: &VNRequest = &request;
    handler
        .performRequests_error(&NSArray::from_slice(&[as_request]))
        .map_err(|e| e.localizedDescription().to_string())?;
    const NO_SUBJECT: &str = "no subject was found in the image";
    // SAFETY: the request has finished; results is a plain getter.
    let observation = unsafe { request.results() }
        .and_then(|r| r.firstObject())
        .ok_or(NO_SUBJECT)?;
    // SAFETY: the observation and the handler are the pair the request just produced.
    let mask = unsafe {
        let instances = observation.allInstances();
        if instances.count() == 0 {
            return Err(NO_SUBJECT.into());
        }
        observation
            .generateScaledMaskForImageForInstances_fromRequestHandler_error(&instances, &handler)
            .map_err(|e| e.localizedDescription().to_string())?
    };

    let source = NSBitmapImageRep::imageRepWithData(&NSData::with_bytes(png))
        .ok_or("the image cannot be read")?;
    let (width, height) = (
        usize::try_from(source.pixelsWide()).unwrap_or(0),
        usize::try_from(source.pixelsHigh()).unwrap_or(0),
    );
    if width == 0 || height == 0 || width > MAX_SIDE || height > MAX_SIDE {
        return Err("the image has an unsupported size".into());
    }
    let canvas = rgba_canvas(width, height).ok_or("cannot allocate the output image")?;
    draw_into(&canvas, &source, width, height);
    apply_mask(&canvas, &mask, width, height)?;
    // SAFETY: an empty properties dictionary is valid for PNG output.
    let out = unsafe {
        canvas.representationUsingType_properties(NSBitmapImageFileType::PNG, &NSDictionary::new())
    };
    out.map(|data| data.to_vec())
        .ok_or_else(|| "cannot encode the result".into())
}

/// A premultiplied 8 bit RGBA bitmap of exactly `width` by `height` pixels, which is what the mask is
/// multiplied into.
fn rgba_canvas(width: usize, height: usize) -> Option<Retained<NSBitmapImageRep>> {
    // SAFETY: a null plane pointer asks AppKit to allocate the pixels; the other arguments describe an
    // ordinary interleaved RGBA bitmap, and the colour space constant lives for the process.
    unsafe {
        NSBitmapImageRep::initWithBitmapDataPlanes_pixelsWide_pixelsHigh_bitsPerSample_samplesPerPixel_hasAlpha_isPlanar_colorSpaceName_bytesPerRow_bitsPerPixel(
            NSBitmapImageRep::alloc(),
            std::ptr::null_mut(),
            width as isize,
            height as isize,
            8,
            4,
            true,
            false,
            NSDeviceRGBColorSpace,
            0,
            32,
        )
    }
}

/// Draws the decoded image over the whole canvas, which starts out clear.
fn draw_into(canvas: &NSBitmapImageRep, source: &NSBitmapImageRep, width: usize, height: usize) {
    let Some(context) = NSGraphicsContext::graphicsContextWithBitmapImageRep(canvas) else {
        return;
    };
    NSGraphicsContext::saveGraphicsState_class();
    NSGraphicsContext::setCurrentContext(Some(&context));
    source.drawInRect(NSRect::new(
        NSPoint::new(0.0, 0.0),
        NSSize::new(width as f64, height as f64),
    ));
    context.flushGraphics();
    NSGraphicsContext::restoreGraphicsState_class();
}

/// Multiplies the canvas by the mask, pixel by pixel. The canvas is premultiplied, so scaling all four
/// channels keeps it valid.
fn apply_mask(
    canvas: &NSBitmapImageRep,
    mask: &CVPixelBuffer,
    width: usize,
    height: usize,
) -> Result<(), String> {
    let pixels = canvas.bitmapData();
    if pixels.is_null() {
        return Err("cannot reach the output pixels".into());
    }
    let stride = usize::try_from(canvas.bytesPerRow()).unwrap_or(0);
    // SAFETY: the buffer is locked for the whole read and unlocked on every path; every access below stays
    // inside the width, height and row length the buffer reports, and inside the canvas it was sized for.
    unsafe {
        if CVPixelBufferLockBaseAddress(mask, CVPixelBufferLockFlags::ReadOnly) != 0 {
            return Err("cannot read the mask".into());
        }
        let base = CVPixelBufferGetBaseAddress(mask).cast::<u8>();
        let (mw, mh) = (CVPixelBufferGetWidth(mask), CVPixelBufferGetHeight(mask));
        let row = CVPixelBufferGetBytesPerRow(mask);
        let format = CVPixelBufferGetPixelFormatType(mask);
        let usable =
            !base.is_null() && mw > 0 && mh > 0 && matches!(format, FORMAT_FLOAT32 | FORMAT_GRAY8);
        if usable {
            for y in 0..height {
                let line = base.add(y * mh / height * row);
                let out = pixels.add(y * stride);
                for x in 0..width {
                    let mx = x * mw / width;
                    let coverage = if format == FORMAT_FLOAT32 {
                        line.cast::<f32>().add(mx).read_unaligned().clamp(0.0, 1.0)
                    } else {
                        f32::from(*line.add(mx)) / 255.0
                    };
                    let px = out.add(x * 4);
                    for c in 0..4 {
                        *px.add(c) = (f32::from(*px.add(c)) * coverage).round() as u8;
                    }
                }
            }
        }
        CVPixelBufferUnlockBaseAddress(mask, CVPixelBufferLockFlags::ReadOnly);
        if usable {
            Ok(())
        } else {
            Err("the mask has an unexpected format".into())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use objc2_app_kit::{NSBezierPath, NSColor};

    /// A grey image, with a red disc in the middle when `disc` is set, as PNG.
    fn picture_png(side: usize, disc: bool) -> Vec<u8> {
        let canvas = rgba_canvas(side, side).unwrap();
        let context = NSGraphicsContext::graphicsContextWithBitmapImageRep(&canvas).unwrap();
        NSGraphicsContext::saveGraphicsState_class();
        NSGraphicsContext::setCurrentContext(Some(&context));
        let whole = NSRect::new(
            NSPoint::new(0.0, 0.0),
            NSSize::new(side as f64, side as f64),
        );
        NSColor::colorWithSRGBRed_green_blue_alpha(0.85, 0.85, 0.85, 1.0).setFill();
        NSBezierPath::fillRect(whole);
        if disc {
            NSColor::colorWithSRGBRed_green_blue_alpha(0.85, 0.1, 0.1, 1.0).setFill();
            let inset = side as f64 * 0.25;
            NSBezierPath::bezierPathWithOvalInRect(NSRect::new(
                NSPoint::new(inset, inset),
                NSSize::new(side as f64 - 2.0 * inset, side as f64 - 2.0 * inset),
            ))
            .fill();
        }
        context.flushGraphics();
        NSGraphicsContext::restoreGraphicsState_class();
        // SAFETY: an empty properties dictionary is valid for PNG output.
        unsafe {
            canvas.representationUsingType_properties(
                NSBitmapImageFileType::PNG,
                &NSDictionary::new(),
            )
        }
        .unwrap()
        .to_vec()
    }

    fn alpha_at(png: &[u8], x: usize, y: usize) -> (u8, usize, usize) {
        let rep = NSBitmapImageRep::imageRepWithData(&NSData::with_bytes(png)).unwrap();
        let (w, h) = (rep.pixelsWide() as usize, rep.pixelsHigh() as usize);
        let color = rep.colorAtX_y(x as isize, y as isize).unwrap();
        ((color.alphaComponent() * 255.0).round() as u8, w, h)
    }

    #[test]
    fn the_subject_stays_and_the_background_goes() {
        let side = 512;
        let out = remove_background(&picture_png(side, true)).unwrap();
        let (centre, w, h) = alpha_at(&out, side / 2, side / 2);
        assert_eq!((w, h), (side, side), "same size as the input");
        let corners = [(2, 2), (side - 3, 2), (2, side - 3), (side - 3, side - 3)]
            .map(|(x, y)| alpha_at(&out, x, y).0);
        eprintln!("alpha: centre {centre}, corners {corners:?}");
        assert!(centre >= 240, "{centre}");
        assert!(corners.iter().all(|a| *a <= 15), "{corners:?}");
    }

    #[test]
    fn background_removal_refuses_garbage() {
        assert!(remove_background(b"nothing").is_err());
        assert!(remove_background(&[]).is_err());
        // A flat image has no subject to keep.
        assert!(remove_background(&picture_png(64, false)).is_err());
    }

    #[test]
    fn this_system_can_do_it() {
        assert!(background_removal_available());
    }
}
