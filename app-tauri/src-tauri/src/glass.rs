//! The window's glass. On macOS 26 the window is the system's Liquid Glass, with the web view transparent over it
//! (src/styles/base.css lays the window's own colour over it, as the native window of the current app does). On an older
//! system, or when the glass cannot be had, nothing is applied and the page paints a solid window itself.
use tauri::WebviewWindow;

/// How the window got its glass.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Glass {
    Liquid,
    None,
}

#[cfg(target_os = "macos")]
pub fn apply(window: &WebviewWindow) -> Glass {
    use window_vibrancy::{apply_liquid_glass, LiquidGlassOptions, NSGlassEffectViewStyle};
    // `Regular` and `Clear` are the two styles Apple documents; the others use private API and may break.
    let options = LiquidGlassOptions::new(NSGlassEffectViewStyle::Regular);
    match apply_liquid_glass(window, options) {
        Ok(()) => Glass::Liquid,
        Err(error) => {
            eprintln!("glass: not applied ({error})");
            Glass::None
        }
    }
}

#[cfg(not(target_os = "macos"))]
pub fn apply(_window: &WebviewWindow) -> Glass {
    Glass::None
}
