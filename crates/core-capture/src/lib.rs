#![forbid(unsafe_code)]
//! Screen capture logic that needs no system: selection geometry, which window lies under the pointer,
//! file names, stitching a scrolling capture, and the editor's annotation model.

mod annotate;
mod geometry;
mod naming;
mod stitch;

pub use annotate::{
    Background, Color, Document, GRADIENTS, Mark, PALETTE, Shape, Tool, arrow_head, counter_radius,
    pixelate,
};
pub use geometry::{PixelRect, Point, Rect, pick_window};
pub use naming::{Stamp, file_name, unique_path};
pub use stitch::{FrameRef, Step, Stitcher};
