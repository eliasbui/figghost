//! Colour picker, and background removal for images.
//!
//! This folder belongs to one task. Shared code reaches it only through `features/mod.rs`, so everything the
//! feature needs lives here, in `web/features/vision.*` and in its own files of `macos-sys`.

use super::AttachEnv;
use crate::{config::parse_hotkey, i18n, settings::model::Fault};
use colour::{Format, Rgb};
use macos_sys::{
    MainThreadMarker, app, capture_ui,
    hotkey::Hotkey,
    pasteboard::{Pasteboard, Write},
    picker::{Callbacks, Picker, Source, Strings},
    screen_capture,
    status_item::{Entry, Item},
    vision as engine, workspace,
};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::{
    cell::{Cell, RefCell},
    env,
    rc::{Rc, Weak},
};

pub mod colour;
pub mod settings;
mod strings;

/// The privacy pane that holds the Screen Recording list.
const SCREEN_RECORDING_PANE: &str =
    "x-apple.systempreferences:com.apple.preference.security?Privacy_ScreenCapture";

pub const DEFAULT_SHORTCUT: &str = "ctrl+opt+cmd+8";

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    pub enabled: bool,
    /// A global shortcut; empty when the user removed it.
    pub shortcut: String,
    /// The format a pick starts in: `hex`, `rgb`, `hsl` or `hsb`.
    pub format: String,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            enabled: false,
            shortcut: DEFAULT_SHORTCUT.into(),
            format: Format::Hex.id().into(),
        }
    }
}

impl Config {
    /// The configured format, or HEX for a value the config file should not have held.
    pub fn format(&self) -> Format {
        Format::from_id(&self.format).unwrap_or(Format::Hex)
    }
}

/// The running feature. Created by `attach`, kept alive by the app context.
pub struct Feature {
    mtm: MainThreadMarker,
    me: Weak<Feature>,
    picker: Picker,
    hotkey: RefCell<Option<Hotkey>>,
    enabled: Cell<bool>,
    format: Cell<Format>,
}

/// Starts the feature. It has no page in the notch, so it runs wherever the app does.
pub fn attach(env: &AttachEnv) -> Option<Rc<Feature>> {
    let feature = Feature::new(env.mtm);
    if let Err(fault) = feature.configure(&env.config.vision) {
        eprintln!("colour picker: {} ({})", fault.code, fault.detail);
    }
    Some(feature)
}

fn pasteboard() -> Pasteboard {
    // A named pasteboard keeps tests away from the user's real clipboard, as the history does.
    match env::var("FIGGHOST_PASTEBOARD") {
        Ok(name) => Pasteboard::named(&name),
        Err(_) => Pasteboard::general(),
    }
}

impl Feature {
    fn new(mtm: MainThreadMarker) -> Rc<Self> {
        let text = i18n::current();
        let strings = Strings {
            hint: text.vision_hint().to_owned(),
            no_access: text.vision_no_access().to_owned(),
            formats: Format::ALL.map(|f| f.id().to_uppercase()).to_vec(),
        };
        let callbacks = Callbacks {
            render: Box::new(|[r, g, b], index| {
                colour::format(Rgb { r, g, b }, Format::ALL[index % Format::ALL.len()])
            }),
            on_copy: Box::new(|text| {
                pasteboard().write(Write::Text(&text));
            }),
        };
        let picker = Picker::new(mtm, strings, callbacks, Source::system(mtm));
        Rc::new_cyclic(|me| Self {
            mtm,
            me: me.clone(),
            picker,
            hotkey: RefCell::new(None),
            enabled: Cell::new(false),
            format: Cell::new(Format::Hex),
        })
    }

    fn pick(&self) {
        if !self.enabled.get() {
            return;
        }
        // The picture the loupe reads is the screenshots' (ScreenCaptureKit, macOS 14 and later).
        if !screen_capture::capture_supported() {
            capture_ui::toast(self.mtm, i18n::current().vision_needs_newer_macos());
            return;
        }
        self.picker.start_at_pointer(self.format.get().index());
    }

    fn register(&self, shortcut: &str) -> Result<(), Fault> {
        let spec = parse_hotkey(shortcut).map_err(|e| Fault::new("invalid_shortcut", e))?;
        let me = self.me.clone();
        let hotkey = Hotkey::register(self.mtm, spec.key_code, spec.modifiers, move || {
            if let Some(me) = me.upgrade() {
                me.pick();
            }
        })
        .map_err(|status| Fault::new("taken", format!("vision (status {status})")))?;
        *self.hotkey.borrow_mut() = Some(hotkey);
        Ok(())
    }

    /// Applies the settings after a change. An error is shown on the settings page and the change is undone.
    pub fn configure(&self, config: &Config) -> Result<(), Fault> {
        // The old shortcut goes first so that one that stays the same can be registered again.
        self.hotkey.borrow_mut().take();
        self.enabled.set(config.enabled);
        self.format.set(config.format());
        if !config.enabled {
            self.picker.cancel();
            return Ok(());
        }
        if config.shortcut.is_empty() {
            return Ok(());
        }
        self.register(&config.shortcut)
    }

    /// Releases the global shortcuts while the settings page records a new one.
    pub fn suspend_hotkeys(&self) {
        self.hotkey.borrow_mut().take();
    }

    /// Registers the global shortcuts again from the config.
    pub fn resume_hotkeys(&self, config: &Config) {
        if config.enabled
            && !config.shortcut.is_empty()
            && let Err(fault) = self.register(&config.shortcut)
        {
            eprintln!("colour picker: cannot register again ({})", fault.detail);
        }
    }

    /// Facts only the running app knows, such as a permission, merged into this feature's part of the page state.
    pub fn runtime(&self) -> Value {
        json!({
            "screen_access": screen_capture::screen_access(false),
            "background_removal": engine::background_removal_available(),
        })
    }

    /// A button of the settings page. `None` when the name is not this feature's.
    pub fn action(&self, name: &str, _config: &mut Config) -> Option<Result<(), Fault>> {
        if name != "vision_screen_access" {
            return None;
        }
        // The system's own dialog appears the first time only; after that the list in System Settings is
        // the only place to switch it on.
        if !screen_capture::screen_access(true) {
            workspace::open_url(SCREEN_RECORDING_PANE);
        }
        Some(Ok(()))
    }

    /// Entries for the menu bar item, between "Open clipboard" and the login item.
    pub fn menu_entries(this: &Rc<Self>) -> Vec<Entry> {
        let (pick, shown) = (Rc::clone(this), Rc::clone(this));
        let item = Item::new(i18n::current().vision_menu_pick(), move || {
            // The menu is still closing when the action runs; the loupe starts once it is gone.
            let pick = Rc::clone(&pick);
            app::after(0.15, move || pick.pick());
        })
        .symbol(crate::menu::symbols::COLOR_PICKER)
        .visible_when(move || shown.enabled.get());
        vec![Entry::Item(item)]
    }
}
