#![forbid(unsafe_code)]
//! Screen capture logic that needs no system: selection geometry, which window lies under the pointer,
//! file names.

mod geometry;
mod naming;

pub use geometry::{PixelRect, Point, Rect, pick_window};
pub use naming::{Stamp, file_name, unique_path};
