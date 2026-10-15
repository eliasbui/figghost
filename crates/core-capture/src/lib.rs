#![forbid(unsafe_code)]
//! Screen capture with no screen: where a selection may go, how points become pixels across displays,
//! the gestures of the selection overlay, and the names and folders of the files that are saved.

pub mod geometry;
pub mod naming;
pub mod select;

pub use geometry::{
    Display, PixelRect, Point, Rect, WindowBox, clamp_point, display_index_at, flip_y, pixel_rect,
    primary_height, window_at,
};

/// What to take a picture of. Coordinates are AppKit's: points, the origin at the bottom left of the
/// primary display.
#[derive(Debug, Clone, PartialEq)]
pub enum Grab {
    Region {
        display: Display,
        rect: Rect,
    },
    Screen(Display),
    /// One window alone, with or without its shadow.
    Window {
        id: u32,
        shadow: bool,
    },
}

/// A finished picture: PNG bytes at native pixel size.
#[derive(Debug, Clone, PartialEq)]
pub struct Shot {
    pub png: Vec<u8>,
    pub width: u32,
    pub height: u32,
    /// Pixels per point of the display it came from.
    pub scale: f64,
}

/// What the person chose on the overlay.
#[derive(Debug, Clone, PartialEq)]
pub enum Pick {
    Cancelled,
    Region { display: Display, rect: Rect },
    Window(WindowBox),
}
