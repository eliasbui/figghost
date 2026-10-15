//! Screen capture on macOS: the displays and their scales, the list of windows, the picture itself
//! (ScreenCaptureKit), the overlay on which a region or a window is chosen, the small confirmation, the
//! Screen Recording permission and the folder picker. The geometry and the gestures are in `core-capture`.

pub mod background;
pub mod folder;
pub mod grab;
pub mod image;
pub mod overlay;
pub mod screens;
pub mod toast;
pub mod windows;

use crate::workspace;
use objc2_core_graphics::{CGPreflightScreenCaptureAccess, CGRequestScreenCaptureAccess};

const PERMISSION_PANE: &str =
    "x-apple.systempreferences:com.apple.preference.security?Privacy_ScreenCapture";

/// Whether this process may capture the screen. It never shows a dialog.
pub fn permission_granted() -> bool {
    CGPreflightScreenCaptureAccess()
}

/// Asks for the permission: the system shows its dialog once, later calls only report the state. Returns
/// whether it is granted now.
pub fn request_permission() -> bool {
    CGRequestScreenCaptureAccess()
}

pub fn open_permission_settings() {
    workspace::open_url(PERMISSION_PANE);
}
