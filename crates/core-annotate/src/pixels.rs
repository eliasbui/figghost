//! The redaction itself: a region of the picture replaced by blocks of their average colour. The same
//! function makes the preview and the saved file, so what is shown is what is written.

/// A rectangle of pixels counted from the picture's top-left corner; it may reach outside the picture.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct PixelRect {
    pub x: i64,
    pub y: i64,
    pub w: u32,
    pub h: u32,
}

/// Rows of RGBA, 8 bits each, with the colour already multiplied by the alpha, and no padding.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Pixels {
    pub width: u32,
    pub height: u32,
    pub data: Vec<u8>,
}

impl Pixels {
    /// `None` when `data` is not exactly `width * height` pixels.
    pub fn new(width: u32, height: u32, data: Vec<u8>) -> Option<Self> {
        (data.len() as u64 == u64::from(width) * u64::from(height) * 4).then_some(Self {
            width,
            height,
            data,
        })
    }

    pub fn at(&self, x: u32, y: u32) -> [u8; 4] {
        let i = (y as usize * self.width as usize + x as usize) * 4;
        [
            self.data[i],
            self.data[i + 1],
            self.data[i + 2],
            self.data[i + 3],
        ]
    }
}

fn floor_div(a: i64, b: i64) -> i64 {
    a.div_euclid(b)
}

/// The pixels of `region`, redacted. The picture is cut into square cells of `block` pixels, counted from
/// its top-left corner, and every pixel of a cell becomes the average of the picture's pixels in it. The
/// grid does not depend on the region, so moving or resizing a blur never changes the cells under it.
/// Where the region lies outside the picture the result is transparent.
pub fn pixelate(source: &Pixels, region: PixelRect, block: u32) -> Pixels {
    let (w, h) = (region.w as usize, region.h as usize);
    let mut data = vec![0u8; w * h * 4];
    let block = i64::from(block.max(1));
    let (sw, sh) = (i64::from(source.width), i64::from(source.height));
    let (rx1, ry1) = (
        region.x + i64::from(region.w),
        region.y + i64::from(region.h),
    );
    if w == 0 || h == 0 {
        return Pixels {
            width: region.w,
            height: region.h,
            data,
        };
    }
    for cy in floor_div(region.y, block)..=floor_div(ry1 - 1, block) {
        for cx in floor_div(region.x, block)..=floor_div(rx1 - 1, block) {
            let (cell_x0, cell_y0) = ((cx * block).max(0), (cy * block).max(0));
            let (cell_x1, cell_y1) = (((cx + 1) * block).min(sw), ((cy + 1) * block).min(sh));
            if cell_x0 >= cell_x1 || cell_y0 >= cell_y1 {
                continue;
            }
            let mut sum = [0u64; 4];
            for y in cell_y0..cell_y1 {
                let row = (y * sw) as usize * 4;
                for x in cell_x0..cell_x1 {
                    let i = row + x as usize * 4;
                    for (channel, total) in sum.iter_mut().enumerate() {
                        *total += u64::from(source.data[i + channel]);
                    }
                }
            }
            let count = ((cell_x1 - cell_x0) * (cell_y1 - cell_y0)) as u64;
            let mean = sum.map(|total| ((total + count / 2) / count) as u8);
            for y in cell_y0.max(region.y)..cell_y1.min(ry1) {
                for x in cell_x0.max(region.x)..cell_x1.min(rx1) {
                    let i = ((y - region.y) as usize * w + (x - region.x) as usize) * 4;
                    data[i..i + 4].copy_from_slice(&mean);
                }
            }
        }
    }
    Pixels {
        width: region.w,
        height: region.h,
        data,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn solid(w: u32, h: u32, px: [u8; 4]) -> Pixels {
        Pixels::new(w, h, px.repeat((w * h) as usize)).unwrap()
    }

    /// A picture that looks like small text: noisy, so that blocks of it are not flat.
    fn noisy(w: u32, h: u32) -> Pixels {
        let mut state = 0x9E3779B97F4A7C15u64;
        let mut data = Vec::new();
        for _ in 0..w * h {
            state ^= state << 13;
            state ^= state >> 7;
            state ^= state << 17;
            let v = (state >> 24) as u8;
            data.extend_from_slice(&[v, v, v, 255]);
        }
        Pixels::new(w, h, data).unwrap()
    }

    #[test]
    fn the_size_must_match_the_data() {
        assert!(Pixels::new(2, 2, vec![0; 16]).is_some());
        assert!(Pixels::new(2, 2, vec![0; 15]).is_none());
    }

    #[test]
    fn a_flat_picture_stays_flat() {
        let src = solid(20, 20, [10, 20, 30, 255]);
        let out = pixelate(
            &src,
            PixelRect {
                x: 3,
                y: 3,
                w: 10,
                h: 10,
            },
            4,
        );
        assert!(out.data.chunks(4).all(|p| p == [10, 20, 30, 255]));
    }

    #[test]
    fn a_cell_becomes_the_average_of_its_pixels() {
        // Two columns, black and white: any cell covering both is grey.
        let mut data = Vec::new();
        for _ in 0..4 {
            data.extend_from_slice(&[0, 0, 0, 255, 200, 200, 200, 255]);
        }
        let src = Pixels::new(2, 4, data).unwrap();
        let out = pixelate(
            &src,
            PixelRect {
                x: 0,
                y: 0,
                w: 2,
                h: 4,
            },
            2,
        );
        assert!(out.data.chunks(4).all(|p| p == [100, 100, 100, 255]));
    }

    #[test]
    fn every_pixel_of_a_cell_is_the_same_and_the_blocks_differ() {
        let src = noisy(64, 64);
        let out = pixelate(
            &src,
            PixelRect {
                x: 0,
                y: 0,
                w: 64,
                h: 64,
            },
            8,
        );
        for cy in 0..8 {
            for cx in 0..8 {
                let first = out.at(cx * 8, cy * 8);
                for y in 0..8 {
                    for x in 0..8 {
                        assert_eq!(out.at(cx * 8 + x, cy * 8 + y), first);
                    }
                }
            }
        }
        let distinct: std::collections::HashSet<[u8; 4]> = (0..8)
            .flat_map(|cy| (0..8).map(move |cx| (cx, cy)))
            .map(|(cx, cy)| out.at(cx * 8, cy * 8))
            .collect();
        assert!(
            distinct.len() > 20,
            "blocks keep their own average: {}",
            distinct.len()
        );
    }

    #[test]
    fn nothing_of_the_original_survives_in_a_block() {
        let src = noisy(64, 64);
        let out = pixelate(
            &src,
            PixelRect {
                x: 0,
                y: 0,
                w: 64,
                h: 64,
            },
            8,
        );
        let same = (0..64 * 64)
            .filter(|i| src.data[i * 4..i * 4 + 4] == out.data[i * 4..i * 4 + 4])
            .count();
        assert!(
            same < 64 * 64 / 20,
            "only chance matches: {same} of {}",
            64 * 64
        );
    }

    #[test]
    fn the_cells_do_not_depend_on_the_region() {
        let src = noisy(50, 40);
        let a = pixelate(
            &src,
            PixelRect {
                x: 5,
                y: 7,
                w: 30,
                h: 20,
            },
            6,
        );
        let b = pixelate(
            &src,
            PixelRect {
                x: 11,
                y: 13,
                w: 30,
                h: 25,
            },
            6,
        );
        for y in 13..27 {
            for x in 11..35 {
                assert_eq!(
                    a.at((x - 5) as u32, (y - 7) as u32),
                    b.at((x - 11) as u32, (y - 13) as u32),
                    "({x}, {y})"
                );
            }
        }
    }

    #[test]
    fn a_region_off_the_picture_is_transparent_and_the_edge_cell_averages_what_is_there() {
        let src = solid(10, 10, [50, 50, 50, 255]);
        let out = pixelate(
            &src,
            PixelRect {
                x: -5,
                y: 6,
                w: 20,
                h: 10,
            },
            4,
        );
        assert_eq!(out.at(0, 0), [0, 0, 0, 0], "left of the picture");
        assert_eq!(out.at(6, 0), [50, 50, 50, 255], "on it");
        assert_eq!(
            out.at(13, 0),
            [50, 50, 50, 255],
            "the partial cell at the right edge is all picture"
        );
        assert_eq!(out.at(15, 0), [0, 0, 0, 0], "past the right edge");
        assert_eq!(out.at(6, 5), [0, 0, 0, 0], "below the picture");
        let away = pixelate(
            &src,
            PixelRect {
                x: 100,
                y: 100,
                w: 4,
                h: 4,
            },
            4,
        );
        assert!(away.data.iter().all(|b| *b == 0));
    }

    #[test]
    fn a_block_of_one_changes_nothing_and_zero_is_one() {
        let src = noisy(16, 16);
        let region = PixelRect {
            x: 0,
            y: 0,
            w: 16,
            h: 16,
        };
        assert_eq!(pixelate(&src, region, 1), src);
        assert_eq!(pixelate(&src, region, 0), src);
    }

    #[test]
    fn transparent_pixels_average_as_premultiplied_values() {
        let src = Pixels::new(2, 1, vec![100, 0, 0, 100, 0, 0, 0, 0]).unwrap();
        let out = pixelate(
            &src,
            PixelRect {
                x: 0,
                y: 0,
                w: 2,
                h: 1,
            },
            2,
        );
        assert_eq!(out.at(0, 0), [50, 0, 0, 50]);
        assert_eq!(out.at(1, 0), [50, 0, 0, 50]);
    }

    #[test]
    fn an_empty_region_is_empty() {
        let src = solid(4, 4, [1, 2, 3, 255]);
        let out = pixelate(
            &src,
            PixelRect {
                x: 0,
                y: 0,
                w: 0,
                h: 3,
            },
            2,
        );
        assert!(out.data.is_empty() && out.width == 0 && out.height == 3);
    }
}
