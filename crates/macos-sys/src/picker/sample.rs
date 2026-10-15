//! Reading the pixels around the pointer out of the pictures of the displays that the screenshots freeze.

use super::contains;
use crate::screen_capture::DisplayShot;
use objc2_core_foundation::{CFRetained, CGPoint, CGRect, CGSize};
use objc2_core_graphics::{
    CGColorSpace, CGContext, CGImage, CGInterpolationQuality, kCGColorSpaceSRGB,
};
use objc2_foundation::{NSPoint, NSRect};
use std::{ffi::c_void, ptr::NonNull};

#[link(name = "CoreGraphics", kind = "framework")]
unsafe extern "C-unwind" {
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

/// `image` drawn pixel for pixel into a `cells` by `cells` bitmap in sRGB, with its top left corner at cell
/// (`left`, `top`). What the image does not cover stays black. `None` when it does not fit.
pub fn grid_from_image(image: &CGImage, cells: usize, (left, top): (usize, usize)) -> Option<Grid> {
    let (width, height) = (CGImage::width(Some(image)), CGImage::height(Some(image)));
    if cells == 0 || left.checked_add(width)? > cells || top.checked_add(height)? > cells {
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
    // The context measures from its bottom left corner; the rows in memory run from the top.
    let place = CGRect::new(
        CGPoint::new(left as f64, (cells - top - height) as f64),
        CGSize::new(width as f64, height as f64),
    );
    CGContext::draw_image(Some(&context), place, Some(image));
    drop(context);
    let rgb = bytes.chunks_exact(4).map(|p| [p[0], p[1], p[2]]).collect();
    Some(Grid { cells, rgb })
}

/// The top left pixel of the grid of `cells` around `pointer` in the picture of a display, which lies beyond
/// the picture near an edge. `frame` is the display in points (origin at the bottom left), `size` its picture
/// in pixels; the picture is the whole display, so the pixels per point follow from the two. The pixel under
/// the pointer is always one of the picture.
fn locate(frame: NSRect, size: (usize, usize), pointer: NSPoint, cells: usize) -> (isize, isize) {
    let (per_x, per_y) = (
        size.0 as f64 / frame.size.width,
        size.1 as f64 / frame.size.height,
    );
    let x = ((pointer.x - frame.origin.x) * per_x).floor() as isize;
    let y = ((frame.origin.y + frame.size.height - pointer.y) * per_y).floor() as isize;
    let half = (cells / 2) as isize;
    (
        x.clamp(0, size.0 as isize - 1) - half,
        y.clamp(0, size.1 as isize - 1) - half,
    )
}

/// The pixels around `pointer` (global points, origin at the bottom left of the main display) from the picture
/// of the display it is on. `None` when no display has it, or `cells` is 0.
pub fn grid_at(displays: &[DisplayShot], pointer: NSPoint, cells: usize) -> Option<Grid> {
    let display = displays.iter().find(|d| contains(d.frame, pointer))?;
    let (width, height) = (display.shot.width(), display.shot.height());
    if cells == 0 || width == 0 || height == 0 {
        return None;
    }
    let (left, top) = locate(display.frame, (width, height), pointer, cells);
    // The part of the grid that lies inside the picture; the rest of it stays black.
    let (first_x, first_y) = (left.max(0), top.max(0));
    let end_x = (left + cells as isize).min(width as isize);
    let end_y = (top + cells as isize).min(height as isize);
    let part = display.shot.crop(
        usize::try_from(first_x).ok()?,
        usize::try_from(first_y).ok()?,
        usize::try_from(end_x - first_x).ok()?,
        usize::try_from(end_y - first_y).ok()?,
    )?;
    grid_from_image(
        part.cg(),
        cells,
        (
            usize::try_from(first_x - left).ok()?,
            usize::try_from(first_y - top).ok()?,
        ),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::screen_capture::Shot;
    use objc2_foundation::NSSize;

    /// An image of `width` by `height` pixels made from a closure, through the same bitmap context path.
    fn image_of(
        width: usize,
        height: usize,
        colour: impl Fn(usize, usize) -> [u8; 3],
    ) -> CFRetained<CGImage> {
        let mut bytes = Vec::new();
        for y in 0..height {
            for x in 0..width {
                let [r, g, b] = colour(x, y);
                bytes.extend([r, g, b, 255]);
            }
        }
        let space = CGColorSpace::with_name(Some(unsafe { kCGColorSpaceSRGB })).unwrap();
        // SAFETY: `bytes` has the rows the context describes and outlives it; the image copies the pixels.
        let context = unsafe {
            let raw = CGBitmapContextCreate(
                bytes.as_mut_ptr().cast(),
                width,
                height,
                8,
                width * 4,
                CFRetained::as_ptr(&space).as_ptr(),
                RGBX,
            );
            CFRetained::from_raw(NonNull::new(raw).unwrap())
        };
        objc2_core_graphics::CGBitmapContextCreateImage(Some(&context)).unwrap()
    }

    /// A colour for every pixel that tells where it is, so a wrong pixel is a wrong number.
    fn pattern(x: usize, y: usize) -> [u8; 3] {
        [
            (x * 7 % 256) as u8,
            (y * 11 % 256) as u8,
            ((x + y) * 3 % 256) as u8,
        ]
    }

    /// A display of `points` (width, height) at `origin`, whose picture has `scale` pixels to the point.
    fn display(origin: (f64, f64), points: (f64, f64), scale: f64) -> DisplayShot {
        let size = ((points.0 * scale) as usize, (points.1 * scale) as usize);
        DisplayShot {
            id: 1,
            frame: NSRect::new(
                NSPoint::new(origin.0, origin.1),
                NSSize::new(points.0, points.1),
            ),
            shot: Shot::from_cg(image_of(size.0, size.1, pattern), scale),
        }
    }

    const BLACK: [u8; 3] = [0, 0, 0];

    #[test]
    fn an_image_of_the_grids_size_comes_out_pixel_for_pixel() {
        let grid = grid_from_image(&image_of(11, 11, pattern), 11, (0, 0)).unwrap();
        for y in 0..11 {
            for x in 0..11 {
                assert_eq!(grid.at(x, y), pattern(x, y), "({x}, {y})");
            }
        }
        assert_eq!(grid.centre(), pattern(5, 5));
    }

    #[test]
    fn a_smaller_image_sits_where_it_is_told_and_the_rest_is_black() {
        let grid = grid_from_image(&image_of(3, 2, pattern), 5, (2, 1)).unwrap();
        for y in 0..5 {
            for x in 0..5 {
                let inside = (2..5).contains(&x) && (1..3).contains(&y);
                let expected = if inside { pattern(x - 2, y - 1) } else { BLACK };
                assert_eq!(grid.at(x, y), expected, "({x}, {y})");
            }
        }
    }

    #[test]
    fn what_does_not_fit_or_has_no_cells_is_no_grid() {
        assert!(grid_from_image(&image_of(3, 3, pattern), 0, (0, 0)).is_none());
        assert!(grid_from_image(&image_of(3, 3, pattern), 5, (3, 0)).is_none());
        assert!(grid_from_image(&image_of(3, 3, pattern), 5, (0, 3)).is_none());
        assert!(grid_from_image(&image_of(3, 3, pattern), 5, (usize::MAX, 0)).is_none());
    }

    #[test]
    fn the_grid_is_centred_on_the_pixel_under_the_pointer() {
        // A picture of 100 by 80 points at one pixel to the point; (50.5, 40.5) is pixel 50 from the left and
        // pixel 39 from the top (the y axis of the screen runs up).
        let shown = [display((0.0, 0.0), (100.0, 80.0), 1.0)];
        let grid = grid_at(&shown, NSPoint::new(50.5, 40.5), 11).unwrap();
        for y in 0..11 {
            for x in 0..11 {
                assert_eq!(grid.at(x, y), pattern(45 + x, 34 + y), "({x}, {y})");
            }
        }
    }

    #[test]
    fn a_retina_picture_is_read_in_device_pixels() {
        // 100 by 80 points at two pixels to the point; the point (10.25, 79.75) is pixel (20, 0): the top row.
        let shown = [display((0.0, 0.0), (100.0, 80.0), 2.0)];
        let grid = grid_at(&shown, NSPoint::new(10.25, 79.75), 11).unwrap();
        assert_eq!(grid.centre(), pattern(20, 0));
        assert_eq!(grid.at(5, 6), pattern(20, 1), "the next row down");
        assert_eq!(
            grid.at(6, 5),
            pattern(21, 0),
            "the next pixel to the right, not the next point"
        );
        assert_eq!(grid.at(0, 5), pattern(15, 0));
    }

    #[test]
    fn beyond_an_edge_of_the_picture_the_grid_is_black() {
        let shown = [display((0.0, 0.0), (100.0, 80.0), 1.0)];
        // Top left: the pixel under the pointer is (0, 0); five rows above it and five columns left are outside.
        let grid = grid_at(&shown, NSPoint::new(0.2, 79.8), 11).unwrap();
        for y in 0..11 {
            for x in 0..11 {
                let expected = if x >= 5 && y >= 5 {
                    pattern(x - 5, y - 5)
                } else {
                    BLACK
                };
                assert_eq!(grid.at(x, y), expected, "top left ({x}, {y})");
            }
        }
        // Bottom right: the last pixel is (99, 79).
        let grid = grid_at(&shown, NSPoint::new(99.8, 0.2), 11).unwrap();
        for y in 0..11 {
            for x in 0..11 {
                let expected = if x <= 5 && y <= 5 {
                    pattern(94 + x, 74 + y)
                } else {
                    BLACK
                };
                assert_eq!(grid.at(x, y), expected, "bottom right ({x}, {y})");
            }
        }
    }

    #[test]
    fn the_bottom_and_left_edges_of_a_display_still_read_a_pixel() {
        // A pointer exactly on the lower edge belongs to the display and reads its bottom row.
        let shown = [display((0.0, 0.0), (100.0, 80.0), 1.0)];
        let grid = grid_at(&shown, NSPoint::new(0.0, 0.0), 11).unwrap();
        assert_eq!(grid.centre(), pattern(0, 79));
    }

    #[test]
    fn the_display_that_has_the_pointer_is_the_one_read() {
        // A second display to the left and a little higher, with its own origin below zero.
        let shown = [
            display((0.0, 0.0), (100.0, 80.0), 1.0),
            display((-100.0, 20.0), (100.0, 60.0), 2.0),
        ];
        // On the second: 50 points from its left edge is pixel 100, 30 points from its top edge is pixel 60.
        let grid = grid_at(&shown, NSPoint::new(-50.0, 50.0), 11).unwrap();
        assert_eq!(grid.centre(), pattern(100, 60));
        let grid = grid_at(&shown, NSPoint::new(10.5, 10.5), 11).unwrap();
        assert_eq!(grid.centre(), pattern(10, 69));
    }

    #[test]
    fn a_pointer_on_no_display_reads_nothing() {
        let shown = [display((0.0, 0.0), (100.0, 80.0), 1.0)];
        assert!(
            grid_at(&shown, NSPoint::new(100.0, 40.0), 11).is_none(),
            "the right edge is outside"
        );
        assert!(grid_at(&shown, NSPoint::new(-1.0, 40.0), 11).is_none());
        assert!(
            grid_at(&shown, NSPoint::new(50.0, 80.0), 11).is_none(),
            "so is the top edge"
        );
        assert!(grid_at(&[], NSPoint::new(1.0, 1.0), 11).is_none());
        assert!(grid_at(&shown, NSPoint::new(50.0, 40.0), 0).is_none());
    }

    #[test]
    fn a_scale_that_is_not_whole_is_followed() {
        // 100 by 80 points shown in 150 by 120 pixels: the pixel under (10, 70) is (15, 15).
        let frame = NSRect::new(NSPoint::new(0.0, 0.0), NSSize::new(100.0, 80.0));
        assert_eq!(
            locate(frame, (150, 120), NSPoint::new(10.0, 70.0), 11),
            (10, 10)
        );
    }
}
