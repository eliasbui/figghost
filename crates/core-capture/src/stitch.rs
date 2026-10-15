//! Stitching a scrolling capture: frames of one screen area, taken while its content scrolled down,
//! become one tall picture. Rows are compared by signature. A header or footer that stays put (a sticky
//! toolbar) is kept once, and the right edge is left out of comparisons because overlay scroll bars
//! appear there while scrolling.

const BYTES_PER_PIXEL: usize = 4;
/// Fewest rows two frames must share for their offset to count.
const MIN_OVERLAP: usize = 24;
/// Fewest rows with content (not one flat colour) that must agree.
const MIN_INFORMATIVE: usize = 8;
/// Share of compared rows that must match exactly.
const MATCH_SHARE: f64 = 0.9;

/// One frame: `height` rows of `width` pixels of 4 bytes, each row `stride` bytes after the last.
#[derive(Debug, Clone, Copy)]
pub struct FrameRef<'a> {
    pub data: &'a [u8],
    pub width: usize,
    pub height: usize,
    pub stride: usize,
}

impl FrameRef<'_> {
    fn row(&self, y: usize) -> &[u8] {
        &self.data[y * self.stride..y * self.stride + self.width * BYTES_PER_PIXEL]
    }
}

/// What a new frame did.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Step {
    /// The content moved up this many rows; they were added.
    Moved(usize),
    /// Nothing moved: the end of the content, or the app did not scroll.
    Still,
    /// The frame does not continue the previous one (the content changed, or it scrolled too far).
    Lost,
    /// The picture reached its height limit.
    Full,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Signature {
    hash: u64,
    /// Every pixel the same colour: says nothing about where the row belongs.
    flat: bool,
}

fn signature(row: &[u8], compared: usize) -> Signature {
    let row = &row[..compared * BYTES_PER_PIXEL];
    let first = row.get(..BYTES_PER_PIXEL).unwrap_or_default();
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    let mut flat = true;
    for (i, &byte) in row.iter().enumerate() {
        hash = (hash ^ u64::from(byte)).wrapping_mul(0x0000_0100_0000_01b3);
        flat &= byte == first[i % BYTES_PER_PIXEL];
    }
    Signature { hash, flat }
}

/// Builds the tall picture frame by frame.
pub struct Stitcher {
    width: usize,
    /// Pixels per row that are compared (the right margin is left out).
    compared: usize,
    height: usize,
    max_rows: usize,
    previous: Vec<Signature>,
    /// The picture so far, rows packed `width * 4` bytes apart, without the footer.
    out: Vec<u8>,
    /// Rows at the top and bottom that stay put, once known.
    bands: Option<(usize, usize)>,
    /// The latest frame's footer rows, added at the end.
    footer: Vec<u8>,
}

impl Stitcher {
    /// Starts from the first frame. `margin` pixels at the right edge are not compared; the picture
    /// stops growing at `max_rows`.
    pub fn new(first: FrameRef<'_>, margin: usize, max_rows: usize) -> Self {
        let compared = first.width.saturating_sub(margin).max(1);
        let mut out = Vec::with_capacity(first.width * BYTES_PER_PIXEL * first.height);
        for y in 0..first.height {
            out.extend_from_slice(first.row(y));
        }
        Self {
            width: first.width,
            compared,
            height: first.height,
            max_rows: max_rows.max(first.height),
            previous: (0..first.height)
                .map(|y| signature(first.row(y), compared))
                .collect(),
            out,
            bands: None,
            footer: Vec::new(),
        }
    }

    /// Rows in the picture so far.
    pub fn rows(&self) -> usize {
        (self.out.len() + self.footer.len()) / (self.width * BYTES_PER_PIXEL)
    }

    /// Adds a frame taken after scrolling. `expected` is how many rows the scroll should have moved,
    /// used to choose between equally good offsets in repetitive content.
    pub fn push(&mut self, frame: FrameRef<'_>, expected: Option<usize>) -> Step {
        if frame.width != self.width || frame.height != self.height {
            return Step::Lost;
        }
        let row_bytes = self.width * BYTES_PER_PIXEL;
        let next: Vec<Signature> = (0..frame.height)
            .map(|y| signature(frame.row(y), self.compared))
            .collect();
        let (top, bottom) = match self.bands {
            Some(bands) => bands,
            None => {
                let top = next
                    .iter()
                    .zip(&self.previous)
                    .take_while(|(a, b)| a == b)
                    .count();
                if top == self.height {
                    return Step::Still;
                }
                let bottom = next
                    .iter()
                    .rev()
                    .zip(self.previous.iter().rev())
                    .take_while(|(a, b)| a == b)
                    .count();
                (top, bottom)
            }
        };
        let end = self.height - bottom;
        if end <= top + MIN_OVERLAP {
            return Step::Lost;
        }
        if next[top..end] == self.previous[top..end] {
            return Step::Still;
        }
        let Some(shift) = best_shift(&self.previous, &next, top, end, expected) else {
            return Step::Lost;
        };
        if self.bands.is_none() {
            self.bands = Some((top, bottom));
            // The first frame's footer sits at the end of the picture; it comes back once, at the end.
            self.out.truncate(self.out.len() - bottom * row_bytes);
        }
        let room = self
            .max_rows
            .saturating_sub(self.out.len() / row_bytes + bottom);
        let added = shift.min(room);
        for y in end - shift..end - shift + added {
            self.out.extend_from_slice(frame.row(y));
        }
        self.footer.clear();
        for y in end..self.height {
            self.footer.extend_from_slice(frame.row(y));
        }
        self.previous = next;
        if added < shift {
            Step::Full
        } else {
            Step::Moved(shift)
        }
    }

    /// The picture: its packed pixels, width and height.
    pub fn finish(mut self) -> (Vec<u8>, usize, usize) {
        if self.bands.is_none() {
            return (self.out, self.width, self.height);
        }
        self.out.extend_from_slice(&self.footer);
        let rows = self.out.len() / (self.width * BYTES_PER_PIXEL);
        (self.out, self.width, rows)
    }
}

/// How far the moving band `top..end` scrolled from `previous` to `next`: the shift whose rows agree
/// best, then the one closest to `expected`, then the one sharing the most rows. `None` when no shift
/// agrees well enough.
fn best_shift(
    previous: &[Signature],
    next: &[Signature],
    top: usize,
    end: usize,
    expected: Option<usize>,
) -> Option<usize> {
    // (shift, matching share, distance from the expected shift, matching rows)
    let mut best: Option<(usize, f64, usize, usize)> = None;
    for shift in 1..(end - top).saturating_sub(MIN_OVERLAP) {
        let (mut informative, mut matching) = (0usize, 0usize);
        // Row `y` of the new frame shows what row `y + shift` of the previous one showed.
        for y in top..end - shift {
            let (now, before) = (next[y], previous[y + shift]);
            if now.flat && before.flat {
                continue;
            }
            informative += 1;
            if now == before {
                matching += 1;
            }
        }
        let share = matching as f64 / informative.max(1) as f64;
        if informative < MIN_INFORMATIVE || share < MATCH_SHARE {
            continue;
        }
        let distance = expected.map_or(0, |e| e.abs_diff(shift));
        let better = best.is_none_or(|(_, best_share, best_distance, best_matching)| {
            if (share - best_share).abs() > 1e-9 {
                share > best_share
            } else if distance != best_distance {
                distance < best_distance
            } else {
                matching > best_matching
            }
        });
        if better {
            best = Some((shift, share, distance, matching));
        }
    }
    best.map(|(shift, ..)| shift)
}

#[cfg(test)]
mod tests {
    use super::*;

    const W: usize = 24;
    const H: usize = 120;
    const HEADER: usize = 10;
    const FOOTER: usize = 8;
    const MARGIN: usize = 3;

    /// A document row: distinct content for each index, with blank rows now and then.
    fn doc_row(i: usize) -> Vec<u8> {
        if i % 7 == 3 {
            return vec![255; W * 4];
        }
        (0..W * 4)
            .map(|b| ((i * 31 + b * 7 + (i * b) % 13) % 251) as u8)
            .collect()
    }

    fn header_row(i: usize) -> Vec<u8> {
        vec![10 + i as u8; W * 4]
    }

    fn footer_row(i: usize) -> Vec<u8> {
        (0..W * 4)
            .map(|b| (b as u8).wrapping_add(i as u8 * 3))
            .collect()
    }

    /// The window at scroll position `at`, with a header, a footer and a scroll bar that differs every
    /// time in the right margin.
    fn frame(at: usize, scrollbar: u8) -> Vec<u8> {
        let mut data = Vec::new();
        for y in 0..H {
            let mut row = if y < HEADER {
                header_row(y)
            } else if y >= H - FOOTER {
                footer_row(y - (H - FOOTER))
            } else {
                doc_row(at + y - HEADER)
            };
            row[(W - MARGIN) * 4..].fill(scrollbar);
            data.extend(row);
        }
        data
    }

    fn view(data: &[u8]) -> FrameRef<'_> {
        FrameRef {
            data,
            width: W,
            height: H,
            stride: W * 4,
        }
    }

    fn expected(last_at: usize) -> Vec<u8> {
        let band = H - HEADER - FOOTER;
        let mut rows = Vec::new();
        for y in 0..HEADER {
            rows.extend(header_row(y));
        }
        for i in 0..last_at + band {
            rows.extend(doc_row(i));
        }
        for y in 0..FOOTER {
            rows.extend(footer_row(y));
        }
        rows
    }

    /// Compares pictures without the scroll bar columns.
    fn same_ignoring_margin(a: &[u8], b: &[u8]) -> bool {
        a.len() == b.len()
            && a.chunks(W * 4)
                .zip(b.chunks(W * 4))
                .all(|(x, y)| x[..(W - MARGIN) * 4] == y[..(W - MARGIN) * 4])
    }

    #[test]
    fn regular_scrolling_rebuilds_the_whole_page_with_one_header_and_footer() {
        let positions = [0, 40, 80, 120, 160, 175];
        let first = frame(positions[0], 1);
        let mut stitcher = Stitcher::new(view(&first), MARGIN, 10_000);
        for (k, &at) in positions.iter().enumerate().skip(1) {
            let data = frame(at, k as u8 * 40);
            let step = stitcher.push(view(&data), Some(40));
            assert_eq!(step, Step::Moved(at - positions[k - 1]), "frame {k}");
        }
        let (pixels, width, height) = stitcher.finish();
        assert_eq!(width, W);
        let want = expected(175);
        assert_eq!(height * W * 4, want.len());
        assert!(same_ignoring_margin(&pixels, &want));
    }

    #[test]
    fn a_frame_that_did_not_move_is_still_and_adds_nothing() {
        let first = frame(0, 1);
        let mut stitcher = Stitcher::new(view(&first), MARGIN, 10_000);
        let again = frame(0, 99);
        assert_eq!(stitcher.push(view(&again), None), Step::Still);
        assert_eq!(stitcher.rows(), H);
        let (_, _, height) = stitcher.finish();
        assert_eq!(height, H);
    }

    #[test]
    fn unrelated_content_is_lost_not_glued_on() {
        let first = frame(0, 1);
        let mut stitcher = Stitcher::new(view(&first), MARGIN, 10_000);
        let far = frame(5_000, 2);
        assert_eq!(stitcher.push(view(&far), Some(40)), Step::Lost);
        assert_eq!(stitcher.rows(), H);
    }

    #[test]
    fn the_height_limit_stops_the_picture() {
        let first = frame(0, 1);
        let mut stitcher = Stitcher::new(view(&first), MARGIN, H + 50);
        let second = frame(40, 2);
        assert_eq!(stitcher.push(view(&second), None), Step::Moved(40));
        let third = frame(80, 3);
        assert_eq!(stitcher.push(view(&third), None), Step::Full);
        let (_, _, height) = stitcher.finish();
        assert_eq!(height, H + 50);
    }

    #[test]
    fn the_expected_shift_settles_repetitive_content() {
        // A page of stripes repeating every 6 rows: shifts 6 apart all match equally well. (A shift of a
        // whole number of periods would look like no movement at all; 32 is not one.)
        let stripe = |i: usize| -> Vec<u8> { vec![(i % 6) as u8 * 40; W * 4] };
        let make = |at: usize| -> Vec<u8> {
            (0..H)
                .flat_map(|y| {
                    let mut row = stripe(at + y);
                    row[0] = 7;
                    row
                })
                .collect()
        };
        let first = make(0);
        let mut stitcher = Stitcher::new(view(&first), MARGIN, 10_000);
        let second = make(32);
        assert_eq!(stitcher.push(view(&second), Some(31)), Step::Moved(32));
    }
}
