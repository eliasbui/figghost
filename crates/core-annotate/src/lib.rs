#![forbid(unsafe_code)]
//! The model of the annotation editor, with no interface: the document and its commands, hit testing and
//! handles, undo, the growth and cropping of the canvas, the redaction of blurred regions and the geometry
//! of the saved picture. The window only draws it and reports where the pointer is.

pub mod document;
pub mod geom;
pub mod layout;
pub mod naming;
pub mod pixels;
pub mod session;
pub mod shape;

pub use document::{BlurJob, Command, Document, ExportPlan, History};
pub use geom::{Point, Rect, Size};
pub use pixels::{PixelRect, Pixels, pixelate};
pub use session::{Effect, Mods, Session, Tool};
pub use shape::{Colour, Handle, Kind, Object, PALETTE, Style, WIDTHS};
