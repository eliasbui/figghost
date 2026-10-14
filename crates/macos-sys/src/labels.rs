//! Text the UI layer draws itself. The app installs its translation once at startup, before any view is
//! built; examples and tests run with the English defaults.

use std::sync::OnceLock;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Labels {
    pub clipboard_tab: String,
    pub shelf_tab: String,
    pub search_placeholder: String,
    /// Keyboard hints along the bottom of the clipboard page.
    pub clipboard_hint: String,
    pub shelf_hint: String,
    pub shelf_empty: String,
    pub media_tab: String,
    pub media_idle: String,
    pub media_previous: String,
    pub media_play_pause: String,
    pub media_next: String,
    pub media_hint: String,
    pub media_playing: String,
    pub media_paused: String,
    /// The first word of "About Figghost", "Hide Figghost" and "Quit Figghost" in the application menu.
    pub menu_about: String,
    pub menu_settings: String,
    pub menu_hide: String,
    pub menu_hide_others: String,
    pub menu_show_all: String,
    pub menu_quit: String,
    pub menu_edit: String,
    pub menu_undo: String,
    pub menu_cut: String,
    pub menu_copy: String,
    pub menu_paste: String,
    pub menu_select_all: String,
    pub menu_window: String,
    pub menu_minimize: String,
    pub menu_zoom: String,
    pub menu_close: String,
}

impl Default for Labels {
    fn default() -> Self {
        Self {
            clipboard_tab: "Clipboard".into(),
            shelf_tab: "Shelf".into(),
            search_placeholder: "Search  (@image @link @code @file @pin)".into(),
            clipboard_hint: "Enter: paste    Opt+P: pin    Opt+Delete: delete    Esc: close".into(),
            shelf_hint:
                "Drop files to keep them    Double-click: open    Drag out: use    Delete: remove"
                    .into(),
            shelf_empty: "Drop files here to keep them handy".into(),
            media_tab: "Media".into(),
            media_idle: "Nothing is playing".into(),
            media_previous: "Previous".into(),
            media_play_pause: "Play / Pause".into(),
            media_next: "Next".into(),
            media_hint: "Controls whichever app is playing. Track details: Music and Spotify."
                .into(),
            media_playing: "Playing".into(),
            media_paused: "Paused".into(),
            menu_about: "About".into(),
            menu_settings: "Settings...".into(),
            menu_hide: "Hide".into(),
            menu_hide_others: "Hide Others".into(),
            menu_show_all: "Show All".into(),
            menu_quit: "Quit".into(),
            menu_edit: "Edit".into(),
            menu_undo: "Undo".into(),
            menu_cut: "Cut".into(),
            menu_copy: "Copy".into(),
            menu_paste: "Paste".into(),
            menu_select_all: "Select All".into(),
            menu_window: "Window".into(),
            menu_minimize: "Minimize".into(),
            menu_zoom: "Zoom".into(),
            menu_close: "Close".into(),
        }
    }
}

static LABELS: OnceLock<Labels> = OnceLock::new();

/// Installs the labels. Only the first call has an effect, so call it before building any UI.
pub fn set(labels: Labels) {
    let _ = LABELS.set(labels);
}

pub fn get() -> &'static Labels {
    LABELS.get_or_init(Labels::default)
}
