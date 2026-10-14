//! Clipboard feature wiring: history, the cursor popup, the notch list, the polling timer and the hotkey.

use crate::{
    config::{Config, HotkeySpec, parse_hotkey},
    history::{History, empty_text},
};
use macos_sys::{
    MainThreadMarker, app,
    hotkey::Hotkey,
    notch::{Notch, NotchGeometry},
    ui::{AppCallbacks, ClipView, Popup},
};
use std::{cell::RefCell, path::Path, rc::Rc};

const POLL_SECONDS: f64 = 0.4;

pub struct Clipboard {
    pub history: Rc<History>,
    pub notch: Option<Rc<Notch>>,
    /// Opens the popup next to the pointer, or closes it when it is already open.
    pub open_popup: Rc<dyn Fn()>,
    mtm: MainThreadMarker,
    hotkey: RefCell<Option<Hotkey>>,
    _timer: app::Timer,
}

fn refresh(view: &ClipView, history: &History) {
    let query = view.query();
    view.set_rows(history.rows(&query), empty_text(&query));
}

/// Rebuilds the list after a delete or clipboard change without losing the user's place.
fn refresh_keep_place(view: &ClipView, history: &History) {
    let place = view.list.selected_index();
    refresh(view, history);
    view.list.keep_selection(place);
}

fn callbacks(history: &Rc<History>) -> AppCallbacks {
    let (query, activate, pin, delete) = (
        Rc::clone(history),
        Rc::clone(history),
        Rc::clone(history),
        Rc::clone(history),
    );
    AppCallbacks {
        on_query: Box::new(move |view, text| view.set_rows(query.rows(text), empty_text(text))),
        on_activate: Box::new(move |_, id, mode| activate.activate(id, mode)),
        on_pin: Box::new(move |view, id| {
            pin.toggle_pin(id);
            refresh(view, &pin);
            // The pinned item jumps to the top; keep the highlight on it.
            view.list.select_id(id);
        }),
        on_delete: Box::new(move |view, id| {
            delete.delete(id);
            refresh_keep_place(view, &delete);
        }),
    }
}

impl Clipboard {
    /// `None` when the history database cannot be opened; the rest of the app keeps running.
    pub fn start(mtm: MainThreadMarker, home: &Path, config: &Config) -> Option<Self> {
        let history = match History::open(
            home,
            config.history_limit,
            config.ignored_apps.clone(),
            config.auto_paste,
            config.paste_plain,
        ) {
            Ok(h) => Rc::new(h),
            Err(e) => {
                eprintln!("clipboard history disabled: {e}");
                return None;
            }
        };

        let popup = Rc::new(Popup::new(mtm, callbacks(&history)));
        let notch = NotchGeometry::detect(mtm).map(|geometry| {
            let on_expand = {
                let history = Rc::clone(&history);
                move |view: &ClipView| refresh(view, &history)
            };
            Rc::new(Notch::new(mtm, geometry, callbacks(&history), on_expand))
        });

        let timer = {
            let (history, popup, notch) = (Rc::clone(&history), Rc::clone(&popup), notch.clone());
            app::every(POLL_SECONDS, move || {
                if history.poll() {
                    if popup.is_visible() {
                        refresh_keep_place(&popup.ui.view, &history);
                    }
                    if let Some(notch) = notch.as_ref().filter(|n| n.is_expanded()) {
                        refresh_keep_place(notch.view(), &history);
                    }
                }
            })
        };

        let open_popup: Rc<dyn Fn()> = {
            let (history, popup) = (Rc::clone(&history), Rc::clone(&popup));
            Rc::new(move || {
                if popup.is_visible() {
                    popup.ui.view.hide_window();
                    return;
                }
                popup.ui.view.clear_query();
                refresh(&popup.ui.view, &history);
                popup.show_at_cursor(mtm);
            })
        };

        let this = Self {
            history,
            notch,
            open_popup,
            mtm,
            hotkey: RefCell::new(None),
            _timer: timer,
        };
        match parse_hotkey(&config.hotkey) {
            Ok(spec) => {
                if let Err(status) = this.set_hotkey(spec) {
                    eprintln!(
                        "hotkey: cannot register (status {status}); the shortcut is probably taken"
                    );
                }
            }
            Err(e) => eprintln!("hotkey: invalid shortcut in config: {e}"),
        }
        Some(this)
    }

    /// Registers the shortcut that opens the popup, replacing the previous one. Fails with the system's
    /// status when another app owns the combination; the popup then has no shortcut until a new one is set.
    pub fn set_hotkey(&self, spec: HotkeySpec) -> Result<(), i32> {
        // The old one goes first so the same combination can be registered again.
        self.hotkey.borrow_mut().take();
        let open = Rc::clone(&self.open_popup);
        let hotkey = Hotkey::register(self.mtm, spec.key_code, spec.modifiers, move || open())?;
        *self.hotkey.borrow_mut() = Some(hotkey);
        Ok(())
    }

    /// Releases the shortcut, for while the settings page is capturing a new key combination.
    pub fn suspend_hotkey(&self) {
        self.hotkey.borrow_mut().take();
    }
}
