//! Text the UI layer draws itself. The app installs its translation once at startup, before any view is
//! built; examples and tests run with the English defaults.

use std::sync::OnceLock;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Labels {
    pub clipboard_tab: String,
    pub shelf_tab: String,
    pub search_placeholder: String,
    /// The same for the notch, where the filter chips already show what can be filtered.
    pub search_placeholder_short: String,
    /// Keyboard hints along the bottom of the clipboard page.
    pub clipboard_hint: String,
    /// The filter chips above the clipboard list.
    pub chip_all: String,
    pub chip_pinned: String,
    pub chip_links: String,
    pub chip_code: String,
    pub chip_images: String,
    pub chip_files: String,
    pub chip_text: String,
    pub shelf_hint: String,
    pub shelf_empty: String,
    pub agents_tab: String,
    pub agents_hint: String,
    pub media_tab: String,
    pub media_idle: String,
    pub media_previous: String,
    pub media_play_pause: String,
    pub media_next: String,
    pub media_hint: String,
    pub media_playing: String,
    pub media_paused: String,
    /// The first word of "About Figghost", "Hide Figghost" and "Quit Figghost" in the application menu.
    pub focus_tab: String,
    pub focus_start: String,
    pub focus_pause: String,
    pub focus_resume: String,
    pub focus_reset: String,
    pub focus_ready: String,
    pub focus_running: String,
    pub focus_paused: String,
    pub focus_done: String,
    pub focus_hint: String,
    /// After a number of minutes: "25 min".
    pub focus_minutes: String,
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
            search_placeholder_short: "Search".into(),
            clipboard_hint: "Enter: paste    Opt+P: pin    Opt+Delete: delete    Esc: close".into(),
            chip_all: "All".into(),
            chip_pinned: "Pinned".into(),
            chip_links: "Links".into(),
            chip_code: "Code".into(),
            chip_images: "Images".into(),
            chip_files: "Files".into(),
            chip_text: "Text".into(),
            shelf_hint:
                "Drop files to keep them    Double-click: open    Drag out: use    Delete: remove"
                    .into(),
            shelf_empty: "Drop files here to keep them handy".into(),
            agents_tab: "Agents".into(),
            agents_hint: "Return: allow    Opt+Delete: deny    Without an answer the agent asks in its terminal."
                .into(),
            media_tab: "Media".into(),
            media_idle: "Nothing is playing".into(),
            media_previous: "Previous".into(),
            media_play_pause: "Play / Pause".into(),
            media_next: "Next".into(),
            media_hint: "Controls whichever app is playing. Track details: Music and Spotify."
                .into(),
            media_playing: "Playing".into(),
            media_paused: "Paused".into(),
            focus_tab: "Focus".into(),
            focus_start: "Start".into(),
            focus_pause: "Pause".into(),
            focus_resume: "Resume".into(),
            focus_reset: "Reset".into(),
            focus_ready: "Pick a length, then start".into(),
            focus_running: "Stay with it".into(),
            focus_paused: "Paused".into(),
            focus_done: "Time is up".into(),
            focus_hint: "Return: start or pause    The notch opens when time is up.".into(),
            focus_minutes: "min".into(),
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
