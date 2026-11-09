//! The windows: the settings, and the windows of the features that draw one of their own (the diagram of Network, the
//! palette). A window is made when it is wanted and dropped when it is closed, so that the app costs little while nothing is
//! open: the menu bar item stays, and the web view goes with the window.
use crate::{glass, Backend};
use tauri::{
    AppHandle, Manager, Theme, WebviewUrl, WebviewWindow, WebviewWindowBuilder, WindowEvent,
};

/// The labels of the windows. The capabilities (capabilities/default.json) name the same three.
pub const SETTINGS: &str = "main";
pub const NETWORK: &str = "network";
pub const PALETTE: &str = "palette";

/// How a window is made: its title, its size and the least it may be, and whether the page reaches under the title bar, with
/// the traffic lights laid over it (the page keeps room for them) and the window transparent over the system's glass.
struct Spec {
    title: &'static str,
    width: f64,
    height: f64,
    min_width: f64,
    min_height: f64,
    overlay: bool,
}

/// The sizes are those of the current app (crates/app-ui/src/settings/mod.rs).
fn spec(label: &str) -> Option<Spec> {
    Some(match label {
        SETTINGS => Spec {
            title: "Figghost",
            width: 1000.0,
            height: 700.0,
            min_width: 820.0,
            min_height: 560.0,
            overlay: true,
        },
        NETWORK => Spec {
            title: "Figghost Network",
            width: 1100.0,
            height: 720.0,
            min_width: 640.0,
            min_height: 420.0,
            overlay: true,
        },
        // The palette is a field above a list: a title bar of its own leaves the field clear of the traffic lights.
        PALETTE => Spec {
            title: "Figghost",
            width: 640.0,
            height: 460.0,
            min_width: 480.0,
            min_height: 320.0,
            overlay: false,
        },
        _ => return None,
    })
}

/// A regular app has a Dock icon and a menu bar; an accessory one lives in the menu bar alone (macOS only).
pub fn policy(app: &AppHandle, regular: bool) {
    #[cfg(target_os = "macos")]
    {
        let policy = if regular {
            tauri::ActivationPolicy::Regular
        } else {
            tauri::ActivationPolicy::Accessory
        };
        let _ = app.set_activation_policy(policy);
    }
    #[cfg(not(target_os = "macos"))]
    let _ = (app, regular);
}

fn make(app: &AppHandle, label: &str, spec: &Spec) -> tauri::Result<WebviewWindow> {
    // The settings are the page itself; any other window is the same page told which canvas to show (src/app/App.tsx).
    let url = if label == SETTINGS {
        "index.html".to_owned()
    } else {
        format!("index.html?canvas={label}")
    };
    let builder = WebviewWindowBuilder::new(app, label, WebviewUrl::App(url.into()))
        .title(spec.title)
        .inner_size(spec.width, spec.height)
        .min_inner_size(spec.min_width, spec.min_height)
        .center()
        .transparent(spec.overlay);
    #[cfg(target_os = "macos")]
    let builder = if spec.overlay {
        builder
            .title_bar_style(tauri::TitleBarStyle::Overlay)
            .hidden_title(true)
            .traffic_light_position(tauri::Position::Logical(tauri::LogicalPosition::new(
                20.0, 22.0,
            )))
    } else {
        builder.hidden_title(true)
    };
    builder.build()
}

/// Opens a window, or brings it to the front. While a window is open the app is a regular one, with a Dock icon and a menu
/// bar; when the last is closed it goes back to the menu bar alone.
pub fn open(app: &AppHandle, label: &str) {
    let Some(spec) = spec(label) else { return };
    if let Some(window) = app.get_webview_window(label) {
        let _ = window.unminimize();
        let _ = window.show();
        let _ = window.set_focus();
        return;
    }
    policy(app, true);
    match make(app, label, &spec) {
        Ok(window) => {
            let backend = app.state::<Backend>();
            // A window that is transparent needs the system's glass under it; one with a title bar of its own does not.
            if spec.overlay {
                backend.set_glass(glass::apply(&window));
            }
            apply_theme(app);
            watch(app, &window);
            backend.keep_refreshing(app);
            // The app was in the background (a menu bar item has no window): the new window takes the keyboard.
            let _ = window.set_focus();
        }
        Err(error) => eprintln!("windows: {label} could not be opened ({error})"),
    }
}

/// The shortcut again closes the window it opened, as it does for the small windows of the current app.
pub fn toggle(app: &AppHandle, label: &str) {
    if app.get_webview_window(label).is_some() {
        close(app, label);
    } else {
        open(app, label);
    }
}

pub fn close(app: &AppHandle, label: &str) {
    if let Some(window) = app.get_webview_window(label) {
        let _ = window.close();
    }
}

/// When a window is gone and no other is left, the app goes back to the menu bar alone.
fn watch(app: &AppHandle, window: &WebviewWindow) {
    let handle = app.clone();
    let label = window.label().to_owned();
    window.on_window_event(move |event| {
        if matches!(event, WindowEvent::Destroyed)
            && handle.webview_windows().keys().all(|other| *other == label)
        {
            policy(&handle, false);
        }
    });
}

/// The windows follow the theme setting: the system's appearance, or always light or dark.
pub fn apply_theme(app: &AppHandle) {
    let theme = match app.state::<Backend>().theme().as_str() {
        "light" => Some(Theme::Light),
        "dark" => Some(Theme::Dark),
        _ => None,
    };
    for window in app.webview_windows().values() {
        let _ = window.set_theme(theme);
    }
}
