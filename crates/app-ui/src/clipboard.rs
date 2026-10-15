//! Clipboard feature wiring: history, the cursor popup, the notch list, the polling timer and the hotkey.

use crate::{
    config::{Config, HotkeySpec, parse_hotkey},
    history::{History, empty_text},
};
use macos_sys::{
    MainThreadMarker, app,
    hotkey::Hotkey,
    notch::{Notch, NotchGeometry},
    ui::{AppCallbacks, BoardChip, ClipView, Extras, Popup},
};
use std::{
    cell::{Cell, RefCell},
    path::Path,
    rc::Rc,
};

const POLL_SECONDS: f64 = 0.4;
/// While a copy stack runs, copies must not be lost to each other between two looks at the pasteboard.
const POLL_FAST_SECONDS: f64 = 0.1;

pub struct Clipboard {
    pub history: Rc<History>,
    pub notch: Option<Rc<Notch>>,
    /// Opens the popup next to the pointer, or closes it when it is already open.
    pub open_popup: Rc<dyn Fn()>,
    popup: Rc<Popup>,
    mtm: MainThreadMarker,
    hotkey: RefCell<Option<Hotkey>>,
    /// The shortcut that opens or closes the notch, when one is set.
    notch_hotkey: RefCell<Option<Hotkey>>,
    /// Looks at the pasteboard now and then; replaced by a faster one while a copy stack runs.
    timer: RefCell<Option<app::Timer>>,
    poll: Rc<dyn Fn()>,
    fast: Cell<bool>,
}

fn show(view: &ClipView, history: &History, query: &str) {
    let (rows, marks) = history.rows(query);
    view.set_rows_marked(rows, marks, empty_text(query));
}

fn refresh(view: &ClipView, history: &History) {
    show(view, history, &view.query());
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
        on_query: Box::new(move |view, text| show(view, &query, text)),
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

/// What a surface does for hidden rows and boards, in the app's language.
fn extras(history: &Rc<History>) -> Extras {
    let language = crate::i18n::current();
    let (reveal, board) = (Rc::clone(history), Rc::clone(history));
    Extras {
        on_reveal: Box::new(move |view, id| {
            if let Some(title) = reveal.reveal_title(id) {
                view.list.reveal(id, &title);
            }
        }),
        on_board: Box::new(move |view, id, target| {
            board.set_board(id, target);
            refresh(view, &board);
            // Filing pins the item, which moves it to the top; keep the highlight on it.
            view.list.select_id(id);
        }),
        menu_title: language.board_menu_title().to_owned(),
        no_board_title: language.no_board().to_owned(),
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

        popup.ui.view.set_extras(extras(&history));
        if let Some(notch) = &notch {
            notch.view().set_extras(extras(&history));
        }

        let poll: Rc<dyn Fn()> = {
            let (history, popup, notch) = (Rc::clone(&history), Rc::clone(&popup), notch.clone());
            Rc::new(move || {
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
            popup,
            mtm,
            hotkey: RefCell::new(None),
            notch_hotkey: RefCell::new(None),
            timer: RefCell::new(None),
            poll,
            fast: Cell::new(false),
        };
        this.start_timer(POLL_SECONDS);
        this.show_boards();
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
        if !config.notch.shortcut.is_empty() {
            let registered = parse_hotkey(&config.notch.shortcut)
                .map_err(|e| format!("invalid ({e})"))
                .and_then(|spec| {
                    this.set_notch_hotkey(Some(spec))
                        .map_err(|status| format!("taken (status {status})"))
                });
            if let Err(why) = registered {
                eprintln!("hotkey: the notch shortcut is {why}");
            }
        }
        Some(this)
    }

    fn start_timer(&self, seconds: f64) {
        let poll = Rc::clone(&self.poll);
        // The old timer is dropped, and so stopped, as the new one takes its place.
        *self.timer.borrow_mut() = Some(app::every(seconds, move || poll()));
    }

    /// Looks at the pasteboard more often (or back to the usual rate).
    pub fn set_fast_poll(&self, fast: bool) {
        if self.fast.replace(fast) != fast {
            self.start_timer(if fast {
                POLL_FAST_SECONDS
            } else {
                POLL_SECONDS
            });
        }
    }

    /// Puts a chip for every board on the popup and on the notch list.
    pub fn show_boards(&self) {
        let chips: Vec<BoardChip> = self
            .history
            .boards()
            .into_iter()
            .map(|b| BoardChip {
                id: b.id,
                filter: format!("@{}", b.token),
                name: b.name,
            })
            .collect();
        self.popup.ui.view.set_boards(chips.clone());
        if let Some(notch) = &self.notch {
            notch.view().set_boards(chips);
        }
        if self.popup.is_visible() {
            refresh_keep_place(&self.popup.ui.view, &self.history);
        }
        if let Some(notch) = self.notch.as_ref().filter(|n| n.is_expanded()) {
            refresh_keep_place(notch.view(), &self.history);
        }
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

    /// Registers the shortcut that opens or closes the notch, or removes it with `None`.
    pub fn set_notch_hotkey(&self, spec: Option<HotkeySpec>) -> Result<(), i32> {
        self.notch_hotkey.borrow_mut().take();
        let (Some(spec), Some(notch)) = (spec, self.notch.clone()) else {
            return Ok(());
        };
        let hotkey = Hotkey::register(self.mtm, spec.key_code, spec.modifiers, move || {
            notch.toggle();
        })?;
        *self.notch_hotkey.borrow_mut() = Some(hotkey);
        Ok(())
    }

    /// Releases the shortcuts, for while the settings page is capturing a new key combination.
    pub fn suspend_hotkey(&self) {
        self.hotkey.borrow_mut().take();
        self.notch_hotkey.borrow_mut().take();
    }
}
