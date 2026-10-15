//! Cropping and encoding a captured image. Everything here works on in-memory images, so it is tested
//! without a screen.

use core_capture::{Display, PixelRect, Rect, Shot, pixel_rect};
use objc2::{AnyThread, rc::Retained};
use objc2_app_kit::{NSBitmapImageFileType, NSBitmapImageRep};
use objc2_core_graphics::CGImage;
use objc2_foundation::{NSDictionary, NSPoint, NSRect, NSSize};

pub fn size(image: &CGImage) -> (u32, u32) {
    (
        CGImage::width(Some(image)) as u32,
        CGImage::height(Some(image)) as u32,
    )
}

/// The part of the image inside `rect`, in pixels from its top left corner.
pub fn crop(image: &CGImage, rect: PixelRect) -> Option<impl std::ops::Deref<Target = CGImage>> {
    CGImage::with_image_in_rect(
        Some(image),
        NSRect::new(
            NSPoint::new(f64::from(rect.x), f64::from(rect.y)),
            NSSize::new(f64::from(rect.w), f64::from(rect.h)),
        ),
    )
}

/// PNG bytes of the image at its own pixel size.
pub fn png(image: &CGImage) -> Option<Vec<u8>> {
    let rep = NSBitmapImageRep::initWithCGImage(NSBitmapImageRep::alloc(), image);
    // SAFETY: an empty properties dictionary is valid for PNG output.
    let data = unsafe {
        rep.representationUsingType_properties(NSBitmapImageFileType::PNG, &NSDictionary::new())
    }?;
    Some(data.to_vec())
}

/// The picture of a display, or of `region` on it, as a finished shot. `scale` is used for a picture that is
/// not of a whole display (a window); a display's own scale is read back from the size of its image.
pub fn shot(image: &CGImage, region: Option<(Rect, &Display)>, scale: f64) -> Result<Shot, String> {
    let (width, height) = size(image);
    let (kept, scale) = match region {
        Some((rect, display)) => {
            let pixels = pixel_rect(rect, display, (width, height))
                .ok_or("the selection is outside the display")?;
            let cropped = crop(image, pixels).ok_or("cannot crop the image")?;
            (Some(cropped), f64::from(width) / display.frame.w)
        }
        None => (None, scale),
    };
    let image = kept.as_deref().unwrap_or(image);
    let (width, height) = size(image);
    let png = png(image).ok_or("cannot encode the image")?;
    Ok(Shot {
        png,
        width,
        height,
        scale,
    })
}

/// The size of a PNG and the RGBA of the pixel at (x, y) in generic RGB, for checks.
pub fn decode_pixel(png: &[u8], x: usize, y: usize) -> Option<(usize, usize, [u8; 4])> {
    use objc2_foundation::NSData;
    let rep = NSBitmapImageRep::imageRepWithData(&NSData::with_bytes(png))?;
    let colour = rep.colorAtX_y(x as isize, y as isize)?;
    let srgb = colour.colorUsingColorSpace(&objc2_app_kit::NSColorSpace::genericRGBColorSpace())?;
    let byte = |v: f64| (v * 255.0).round() as u8;
    Some((
        rep.pixelsWide() as usize,
        rep.pixelsHigh() as usize,
        [
            byte(srgb.redComponent()),
            byte(srgb.greenComponent()),
            byte(srgb.blueComponent()),
            byte(srgb.alphaComponent()),
        ],
    ))
}

/// An image made of `pixel(x, y)`, for tests and for checks that need a background that is not a screenshot.
pub fn from_pixels(
    width: usize,
    height: usize,
    pixel: impl Fn(usize, usize) -> [u8; 4],
) -> Option<Retained<NSBitmapImageRep>> {
    use objc2_app_kit::NSCalibratedRGBColorSpace;
    // SAFETY: the planes pointer may be null so that the representation allocates its own buffer; the
    // constants are plain values.
    let rep = unsafe {
        NSBitmapImageRep::initWithBitmapDataPlanes_pixelsWide_pixelsHigh_bitsPerSample_samplesPerPixel_hasAlpha_isPlanar_colorSpaceName_bytesPerRow_bitsPerPixel(
            NSBitmapImageRep::alloc(),
            std::ptr::null_mut(),
            width as isize,
            height as isize,
            8,
            4,
            true,
            false,
            NSCalibratedRGBColorSpace,
            (width * 4) as isize,
            32,
        )
    }?;
    let data = rep.bitmapData();
    if data.is_null() {
        return None;
    }
    // SAFETY: the buffer holds `width * 4 * height` bytes that this representation owns, and nothing else
    // refers to it yet.
    let bytes = unsafe { std::slice::from_raw_parts_mut(data, width * height * 4) };
    for y in 0..height {
        for x in 0..width {
            let at = (y * width + x) * 4;
            bytes[at..at + 4].copy_from_slice(&pixel(x, y));
        }
    }
    Some(rep)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 400x200 pixels: red on the left half, green on the right; a blue block of 40x40 at the top left and
    /// a white one of 40x40 at the bottom right.
    fn picture() -> Retained<NSBitmapImageRep> {
        from_pixels(400, 200, |x, y| {
            if x < 40 && y < 40 {
                [0, 0, 255, 255]
            } else if x >= 360 && y >= 160 {
                [255, 255, 255, 255]
            } else if x < 200 {
                [255, 0, 0, 255]
            } else {
                [0, 255, 0, 255]
            }
        })
        .unwrap()
    }

    /// The same picture as a display of 200x100 points at 2x, whose frame starts at (-200, 50).
    fn display() -> Display {
        Display {
            id: 7,
            frame: Rect::new(-200.0, 50.0, 200.0, 100.0),
            scale: 2.0,
        }
    }

    fn close(a: [u8; 4], b: [u8; 4]) -> bool {
        a.iter().zip(b).all(|(x, y)| x.abs_diff(y) <= 3)
    }

    #[test]
    fn a_whole_image_keeps_its_pixels_and_encodes_as_png() {
        let rep = picture();
        let image = rep.CGImage().unwrap();
        let s = shot(&image, None, 2.0).unwrap();
        assert_eq!((s.width, s.height, s.scale), (400, 200, 2.0));
        assert_eq!(
            &s.png[..8],
            &[0x89, b'P', b'N', b'G', 0x0d, 0x0a, 0x1a, 0x0a]
        );
        let (w, h, top_left) = decode_pixel(&s.png, 10, 10).unwrap();
        assert_eq!((w, h), (400, 200));
        assert!(close(top_left, [0, 0, 255, 255]), "{top_left:?}");
        let (_, _, right) = decode_pixel(&s.png, 300, 100).unwrap();
        assert!(close(right, [0, 255, 0, 255]), "{right:?}");
    }

    #[test]
    fn a_region_is_cropped_at_native_pixels_from_the_top_of_the_display() {
        let rep = picture();
        let image = rep.CGImage().unwrap();
        let d = display();
        // The top left 20x20 points of the display: its top edge is y = 150.
        let region = Rect::new(-200.0, 130.0, 20.0, 20.0);
        let s = shot(&image, Some((region, &d)), 1.0).unwrap();
        assert_eq!((s.width, s.height), (40, 40));
        assert_eq!(s.scale, 2.0);
        let (_, _, p) = decode_pixel(&s.png, 20, 20).unwrap();
        assert!(close(p, [0, 0, 255, 255]), "all blue: {p:?}");
        // The bottom right corner of the display is the white block.
        let corner = Rect::new(-20.0, 50.0, 20.0, 20.0);
        let s = shot(&image, Some((corner, &d)), 1.0).unwrap();
        let (_, _, p) = decode_pixel(&s.png, 5, 5).unwrap();
        assert!(close(p, [255, 255, 255, 255]), "all white: {p:?}");
        // A region across the middle has red and green halves.
        let middle = Rect::new(-110.0, 90.0, 20.0, 20.0);
        let s = shot(&image, Some((middle, &d)), 1.0).unwrap();
        assert_eq!((s.width, s.height), (40, 40));
        let left = decode_pixel(&s.png, 5, 20).unwrap().2;
        let right = decode_pixel(&s.png, 35, 20).unwrap().2;
        assert!(
            close(left, [255, 0, 0, 255]) && close(right, [0, 255, 0, 255]),
            "{left:?} {right:?}"
        );
    }

    #[test]
    fn a_region_outside_the_display_is_an_error_not_a_crash() {
        let rep = picture();
        let image = rep.CGImage().unwrap();
        let outside = Rect::new(500.0, 500.0, 20.0, 20.0);
        assert!(shot(&image, Some((outside, &display())), 1.0).is_err());
    }
}
