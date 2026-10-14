//! Window tiling: global shortcuts that move and resize the focused window of the frontmost app.

use crate::{
    config::{TilingConfig, parse_hotkey},
    trace,
};
use core_window::{Layout, Rect, pick_area};
use macos_sys::{
    MainThreadMarker,
    hotkey::Hotkey,
    permissions,
    window::{Frame, Window, screen_areas},
};
use std::cell::RefCell;

/// A layout whose shortcut could not be registered.
pub struct Failure {
    pub layout: Layout,
    pub note: String,
}

pub struct Tiling {
    mtm: MainThreadMarker,
    hotkeys: RefCell<Vec<Hotkey>>,
}

fn rect(f: Frame) -> Rect {
    Rect::new(f.x, f.y, f.w, f.h)
}

fn frame(r: Rect) -> Frame {
    Frame {
        x: r.x,
        y: r.y,
        w: r.w,
        h: r.h,
    }
}

fn apply(mtm: MainThreadMarker, layout: Layout) {
    trace::log(&format!("tiling: shortcut for {} fired", layout.key()));
    if !permissions::accessibility_trusted(false) {
        trace::log("tiling: no Accessibility access");
        return;
    }
    let Some(window) = Window::focused() else {
        trace::log("tiling: no focused window");
        return;
    };
    // A full-screen window lives in its own space; resizing it would only fight the system.
    if window.is_full_screen() {
        return;
    }
    let Some(current) = window.frame().map(rect) else {
        return;
    };
    let areas: Vec<Rect> = screen_areas(mtm).into_iter().map(rect).collect();
    if let Some(area) = pick_area(current, &areas) {
        let target = layout.place(area, current);
        let ok = window.set_frame(frame(target));
        trace::log(&format!(
            "tiling: {current:?} -> {target:?} (applied: {ok})"
        ));
    }
}

impl Tiling {
    pub fn new(mtm: MainThreadMarker) -> Self {
        Self {
            mtm,
            hotkeys: RefCell::new(Vec::new()),
        }
    }

    /// Registers the shortcuts of `config`, replacing the previous ones (none at all when tiling is off).
    /// Returns every shortcut that could not be used, such as one another app already owns.
    pub fn configure(&self, config: &TilingConfig) -> Vec<Failure> {
        // The old ones go first so a shortcut that stays the same can be registered again.
        self.hotkeys.borrow_mut().clear();
        if !config.enabled {
            return Vec::new();
        }
        let (mtm, mut notes, mut hotkeys) = (self.mtm, Vec::new(), Vec::new());
        for (layout, shortcut) in config.bindings() {
            let spec = match parse_hotkey(&shortcut) {
                Ok(spec) => spec,
                Err(e) => {
                    notes.push(Failure {
                        layout,
                        note: format!("invalid shortcut '{shortcut}' for {}: {e}", layout.key()),
                    });
                    continue;
                }
            };
            match Hotkey::register(mtm, spec.key_code, spec.modifiers, move || {
                apply(mtm, layout)
            }) {
                Ok(hotkey) => hotkeys.push(hotkey),
                Err(status) => notes.push(Failure {
                    layout,
                    note: format!(
                        "cannot register '{shortcut}' for {} (status {status}); another app probably owns it",
                        layout.key()
                    ),
                }),
            }
        }
        *self.hotkeys.borrow_mut() = hotkeys;
        notes
    }

    /// Releases every shortcut, for while the settings page is capturing a new key combination.
    pub fn suspend(&self) {
        self.hotkeys.borrow_mut().clear();
    }
}
