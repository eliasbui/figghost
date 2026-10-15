//! Reading the pixels around the pointer, and the Screen Recording permission that takes.

use objc2_core_foundation::{CFRetained, CGPoint, CGRect, CGSize};
use objc2_core_graphics::{
    CGColorSpace, CGContext, CGDirectDisplayID, CGDisplayBounds, CGImage, CGInterpolationQuality,
    CGMainDisplayID, CGWindowImageOption, CGWindowListOption, kCGColorSpaceSRGB,
};
// Deprecated for ScreenCaptureKit, which only offers asynchronous whole-frame captures; a few pixels thirty
// times a second is what this call is for.
#[allow(deprecated)]
use objc2_core_graphics::CGWindowListCreateImage;
use objc2_foundation::NSPoint;
use std::{ffi::c_void, ptr::NonNull};

#[link(name = "CoreGraphics", kind = "framework")]
unsafe extern "C-unwind" {
    fn CGPreflightScreenCaptureAccess() -> bool;
    fn CGRequestScreenCaptureAccess() -> bool;
    fn CGBitmapContextCreate(
        data: *mut c_void,
        width: usize,
        height: usize,
        bits_per_component: usize,
        bytes_per_row: usize,
        space: *const CGColorSpace,
        bitmap_info: u32,
    ) -> *mut CGContext;
}

/// kCGImageAlphaNoneSkipLast: four bytes a pixel, the last one unused, in the order red, green, blue.
const RGBX: u32 = 5;

/// Whether this process may read other apps' pixels. Does not ask.
pub fn screen_access() -> bool {
    // SAFETY: a plain query with no arguments.
    unsafe { CGPreflightScreenCaptureAccess() }
}

/// Asks for the permission: the system shows its own dialog the first time, and nothing afterwards.
pub fn request_screen_access() -> bool {
    // SAFETY: a plain call with no arguments.
    unsafe { CGRequestScreenCaptureAccess() }
}

/// A square of pixels around a point, row by row from the top, in sRGB.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Grid {
    pub cells: usize,
    pub rgb: Vec<[u8; 3]>,
}

impl Grid {
    pub fn centre(&self) -> [u8; 3] {
        self.rgb[(self.cells / 2) * self.cells + self.cells / 2]
    }

    pub fn at(&self, x: usize, y: usize) -> [u8; 3] {
        self.rgb[y * self.cells + x]
    }
}

/// The image reduced to exactly `cells` by `cells` pixels in sRGB. An image of that size comes out pixel for
/// pixel; any other is scaled without smoothing, so no colour is invented.
pub fn grid_from_image(image: &CGImage, cells: usize) -> Option<Grid> {
    if cells == 0 {
        return None;
    }
    let mut bytes = vec![0u8; cells * cells * 4];
    let space = CGColorSpace::with_name(Some(unsafe { kCGColorSpaceSRGB }))?;
    // SAFETY: `bytes` holds `cells` rows of `cells * 4` bytes and outlives the context, which is released
    // before this function returns; the colour space is a live object.
    let context = unsafe {
        let raw = CGBitmapContextCreate(
            bytes.as_mut_ptr().cast(),
            cells,
            cells,
            8,
            cells * 4,
            CFRetained::as_ptr(&space).as_ptr(),
            RGBX,
        );
        CFRetained::from_raw(NonNull::new(raw)?)
    };
    CGContext::set_interpolation_quality(Some(&context), CGInterpolationQuality::None);
    let whole = CGRect::new(
        CGPoint::new(0.0, 0.0),
        CGSize::new(cells as f64, cells as f64),
    );
    CGContext::draw_image(Some(&context), whole, Some(image));
    drop(context);
    let rgb = bytes.chunks_exact(4).map(|p| [p[0], p[1], p[2]]).collect();
    Some(Grid { cells, rgb })
}

/// The pixels around `pointer` (global coordinates, origin at the bottom left of the main display, in
/// points) from everything on screen below the window `below`, so the picker's own windows are not in it.
/// `scale` is the backing scale of the display under the pointer. `None` when the system gives no image.
pub fn grab(pointer: NSPoint, cells: usize, below: u32, scale: f64) -> Option<Grid> {
    let scale = scale.max(1.0);
    let main: CGDirectDisplayID = CGMainDisplayID();
    let height = CGDisplayBounds(main).size.height;
    let half = (cells / 2) as f64;
    // Whole device pixels, so the grid lines up with what the display really shows.
    let (px, py) = (
        (pointer.x * scale).floor(),
        ((height - pointer.y) * scale).floor(),
    );
    let side = cells as f64 / scale;
    let rect = CGRect::new(
        CGPoint::new((px - half) / scale, (py - half) / scale),
        CGSize::new(side, side),
    );
    let (list, id) = if below == 0 {
        (CGWindowListOption::OptionOnScreenOnly, 0)
    } else {
        (CGWindowListOption::OptionOnScreenBelowWindow, below)
    };
    #[allow(deprecated)]
    let image = CGWindowListCreateImage(
        rect,
        list,
        id,
        CGWindowImageOption::BestResolution | CGWindowImageOption::BoundsIgnoreFraming,
    )?;
    grid_from_image(&image, cells)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// An image of `cells` by `cells` pixels made from a closure, through the same bitmap context path.
    fn image_of(cells: usize, colour: impl Fn(usize, usize) -> [u8; 3]) -> CFRetained<CGImage> {
        let mut bytes = Vec::new();
        for y in 0..cells {
            for x in 0..cells {
                let [r, g, b] = colour(x, y);
                bytes.extend([r, g, b, 255]);
            }
        }
        let space = CGColorSpace::with_name(Some(unsafe { kCGColorSpaceSRGB })).unwrap();
        // SAFETY: `bytes` has the rows the context describes and outlives it; the image copies the pixels.
        let context = unsafe {
            let raw = CGBitmapContextCreate(
                bytes.as_mut_ptr().cast(),
                cells,
                cells,
                8,
                cells * 4,
                CFRetained::as_ptr(&space).as_ptr(),
                RGBX,
            );
            CFRetained::from_raw(NonNull::new(raw).unwrap())
        };
        objc2_core_graphics::CGBitmapContextCreateImage(Some(&context)).unwrap()
    }

    #[test]
    fn an_image_of_the_right_size_comes_out_pixel_for_pixel() {
        let pattern = |x: usize, y: usize| [(x * 20) as u8, (y * 20) as u8, ((x + y) * 10) as u8];
        let grid = grid_from_image(&image_of(11, pattern), 11).unwrap();
        for y in 0..11 {
            for x in 0..11 {
                assert_eq!(grid.at(x, y), pattern(x, y), "({x}, {y})");
            }
        }
        assert_eq!(grid.centre(), pattern(5, 5));
    }

    #[test]
    fn a_larger_image_is_scaled_without_inventing_colours() {
        // Four flat quadrants, twice the size: every output pixel must be one of the four.
        let quadrant = |x: usize, y: usize| match (x < 11, y < 11) {
            (true, true) => [255, 0, 0],
            (false, true) => [0, 255, 0],
            (true, false) => [0, 0, 255],
            (false, false) => [255, 255, 255],
        };
        let grid = grid_from_image(&image_of(22, quadrant), 11).unwrap();
        assert!(
            grid.rgb
                .iter()
                .all(|p| (0..4).any(|i| *p == quadrant(i % 2 * 11, i / 2 * 11)))
        );
        assert_eq!(grid.at(0, 0), [255, 0, 0]);
        assert_eq!(grid.at(10, 0), [0, 255, 0]);
        assert_eq!(grid.at(0, 10), [0, 0, 255]);
        assert_eq!(grid.at(10, 10), [255, 255, 255]);
    }

    #[test]
    fn no_cells_is_no_grid() {
        assert!(grid_from_image(&image_of(3, |_, _| [1, 2, 3]), 0).is_none());
    }
}
