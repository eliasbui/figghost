//! The actions of the page that the backend carries out itself. Each is a name the page sends (`{ t: 'action', name }`); the
//! ones that need an engine this backend does not have yet (scanning a disk, recording the screen, listening to the coding
//! agents) are not here, and the page is told so with a fault.
use crate::{native, windows, Backend};
use tauri::AppHandle;

const PRIVACY: &str = "x-apple.systempreferences:com.apple.preference.security?Privacy_";
const LOGIN_ITEMS: &str = "x-apple.systempreferences:com.apple.LoginItems-Settings.extension";

/// Carries out an action by its name; `Ok(false)` when this version does not know it.
pub fn run(app: &AppHandle, backend: &Backend, name: &str) -> Result<bool, String> {
    if let Some(rest) = name.strip_prefix("palette_") {
        return Ok(palette(app, backend, rest));
    }
    match name {
        "open_accessibility" | "check_permissions" => {
            native::open(&format!("{PRIVACY}Accessibility"))?
        }
        "open_camera_settings" => native::open(&format!("{PRIVACY}Camera"))?,
        // The current app asks the system for the access first; here the person is taken to the place where it is given.
        "capture_screen_access"
        | "present_screen_access"
        | "vision_screen_access"
        | "pin_screen_access" => native::open(&format!("{PRIVACY}ScreenCapture"))?,
        "open_login_settings" | "network_open_login" => native::open(LOGIN_ITEMS)?,
        "open_config" => {
            std::fs::create_dir_all(backend.dir()).map_err(|e| e.to_string())?;
            native::open(&backend.dir().to_string_lossy())?;
        }
        "restart" => {
            std::env::set_var(crate::SHOW_AGAIN, "1");
            app.restart()
        }
        "quit" => app.exit(0),
        "network_canvas_open" => windows::open(app, windows::NETWORK),
        "network_canvas_close" => windows::close(app, windows::NETWORK),
        _ => return Ok(false),
    }
    Ok(true)
}

/// The actions of the palette's window, without their `palette_` prefix. A tab of the notch, the agents and the reminders wait
/// for the engines that own them.
fn palette(app: &AppHandle, backend: &Backend, rest: &str) -> bool {
    let valid = |id: &str| {
        !id.is_empty()
            && id.len() <= 24
            && id
                .bytes()
                .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'_')
    };
    if rest == "close" {
        windows::close(app, windows::PALETTE);
        return true;
    }
    match rest.strip_prefix("page:").filter(|id| valid(id)) {
        Some(id) => {
            windows::close(app, windows::PALETTE);
            backend.show_page(app, id);
            true
        }
        None => false,
    }
}
