//! Screenshots, the way CleanShot X takes them: a shortcut or a menu item freezes every display, the
//! overlay picks an area, a window or a display, and the picture is saved, copied and shown in a quick
//! panel with copy, save, edit and pin. Text recognition copies the text of an area instead.

mod scroll;
mod text;

use crate::{
    config::{Config, parse_hotkey},
    i18n, trace,
};
use core_capture::{Rect, Stamp, file_name, unique_path};
use macos_sys::{
    MainThreadMarker, NSPoint, NSRect, app,
    capture_overlay::{self, Mode, Overlay, Picked},
    capture_ui::{self, Pin, PinActions, PinTexts, QuickActions, QuickPanel, QuickTexts},
    editor_ui::{EditorActions, EditorTexts, EditorWindow},
    hotkey::Hotkey,
    ocr,
    pasteboard::{Pasteboard, Write},
    screen_capture::{self, DisplayShot, Frozen, Shot},
    workspace,
};
use serde::{Deserialize, Serialize};
use std::{
    cell::{Cell, RefCell},
    collections::BTreeMap,
    env, fs,
    path::{Path, PathBuf},
    rc::{Rc, Weak},
};

const FILE_PREFIX: &str = "Figghost";
/// A quick panel the pointer is over stays; it is looked at again this often.
const HOVER_RECHECK_SECONDS: f64 = 2.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Area,
    Window,
    Screen,
    /// The display under the pointer, after a countdown.
    Timer,
    /// The text of an area, copied as text.
    Text,
    /// An area whose content scrolls by itself, stitched into one tall picture.
    Scroll,
}

impl Kind {
    pub const ALL: [Kind; 5] = [
        Kind::Area,
        Kind::Window,
        Kind::Screen,
        Kind::Timer,
        Kind::Text,
    ];

    /// Name in the config file.
    pub const fn key(self) -> &'static str {
        match self {
            Kind::Area => "area",
            Kind::Window => "window",
            Kind::Screen => "screen",
            Kind::Timer => "timer",
            Kind::Text => "text",
            Kind::Scroll => "scroll",
        }
    }

    /// Shortcuts that macOS does not use itself; the rest are in the menu only until given one.
    pub const fn default_shortcut(self) -> &'static str {
        match self {
            Kind::Area => "cmd+shift+2",
            Kind::Text => "cmd+shift+1",
            Kind::Window | Kind::Screen | Kind::Timer | Kind::Scroll => "",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct CaptureConfig {
    /// Where captures are saved; empty uses the folder of macOS's own screenshots, or the Desktop.
    pub folder: String,
    pub save: bool,
    pub copy: bool,
    pub quick_panel: bool,
    /// How long the quick panel stays; 0 keeps it until closed.
    pub quick_seconds: f64,
    pub timer_seconds: u32,
    /// Shortcut per capture kind (`area`, `window`, `screen`, `timer`, `text`, `scroll`); empty turns
    /// it off.
    pub shortcuts: BTreeMap<String, String>,
}

impl Default for CaptureConfig {
    fn default() -> Self {
        Self {
            folder: String::new(),
            save: true,
            copy: true,
            quick_panel: true,
            quick_seconds: 8.0,
            timer_seconds: 5,
            shortcuts: BTreeMap::new(),
        }
    }
}

impl CaptureConfig {
    pub fn sanitized(mut self) -> Self {
        if !self.quick_seconds.is_finite() {
            self.quick_seconds = 8.0;
        }
        self.quick_seconds = self.quick_seconds.clamp(0.0, 600.0);
        self.timer_seconds = self.timer_seconds.clamp(1, 60);
        self
    }

    /// Every capture kind with a shortcut: the user's, else the built-in one.
    pub fn bindings(&self) -> Vec<(Kind, String)> {
        Kind::ALL
            .into_iter()
            .map(|kind| {
                let shortcut = self.shortcuts.get(kind.key()).map_or_else(
                    || kind.default_shortcut().to_owned(),
                    |s| s.trim().to_owned(),
                );
                (kind, shortcut)
            })
            .filter(|(_, shortcut)| !shortcut.is_empty())
            .collect()
    }

    pub fn folder(&self, home: &Path) -> PathBuf {
        let folder = self.folder.trim();
        if let Some(rest) = folder.strip_prefix("~/") {
            return home.join(rest);
        }
        if !folder.is_empty() {
            return PathBuf::from(folder);
        }
        screen_capture::system_screenshot_folder().unwrap_or_else(|| home.join("Desktop"))
    }
}

fn user_home() -> PathBuf {
    env::var_os("HOME").map(PathBuf::from).unwrap_or_default()
}

fn pasteboard() -> Pasteboard {
    match env::var("FIGGHOST_PASTEBOARD") {
        Ok(name) => Pasteboard::named(&name),
        Err(_) => Pasteboard::general(),
    }
}

fn stamp() -> Stamp {
    let (year, month, day, hour, minute, second) = screen_capture::local_time();
    Stamp {
        year,
        month,
        day,
        hour,
        minute,
        second,
    }
}

/// The display under the pointer, else the first one.
fn display_under_pointer(frozen: &Frozen) -> usize {
    let p = screen_capture::pointer();
    frozen
        .displays
        .iter()
        .position(|d| {
            let f = d.frame;
            p.x >= f.origin.x
                && p.x < f.origin.x + f.size.width
                && p.y >= f.origin.y
                && p.y < f.origin.y + f.size.height
        })
        .unwrap_or(0)
}

/// The display a rectangle (AppKit screen coordinates) is mostly on.
fn display_of(frozen: &Frozen, frame: NSRect) -> Option<&DisplayShot> {
    let center = NSPoint::new(
        frame.origin.x + frame.size.width / 2.0,
        frame.origin.y + frame.size.height / 2.0,
    );
    frozen
        .displays
        .iter()
        .find(|d| {
            let f = d.frame;
            center.x >= f.origin.x
                && center.x < f.origin.x + f.size.width
                && center.y >= f.origin.y
                && center.y < f.origin.y + f.size.height
        })
        .or(frozen.displays.first())
}

/// What a finished capture is, and where it came from on screen.
struct Taken {
    shot: Shot,
    /// The display it was taken on, for placing the quick panel.
    screen: NSRect,
    /// Where the picture was, for pinning it in place.
    place: NSRect,
}

pub struct CaptureFeature {
    mtm: MainThreadMarker,
    me: Weak<CaptureFeature>,
    config: Rc<RefCell<Config>>,
    home: PathBuf,
    hotkeys: RefCell<Vec<Hotkey>>,
    overlay: RefCell<Option<Rc<Overlay>>>,
    busy: Cell<bool>,
    quick: RefCell<Vec<(u64, Rc<QuickPanel>)>>,
    pins: RefCell<Vec<(u64, Rc<Pin>)>>,
    scrolling: RefCell<Option<Rc<scroll::Scrolling>>>,
    editors: RefCell<Vec<(u64, Rc<EditorWindow>)>>,
    next_id: Cell<u64>,
}

impl CaptureFeature {
    pub fn start(mtm: MainThreadMarker, config: &Rc<RefCell<Config>>) -> Rc<Self> {
        let feature = Rc::new_cyclic(|me| Self {
            mtm,
            me: me.clone(),
            config: Rc::clone(config),
            home: user_home(),
            hotkeys: RefCell::new(Vec::new()),
            overlay: RefCell::new(None),
            busy: Cell::new(false),
            quick: RefCell::new(Vec::new()),
            pins: RefCell::new(Vec::new()),
            scrolling: RefCell::new(None),
            editors: RefCell::new(Vec::new()),
            next_id: Cell::new(1),
        });
        for note in feature.configure() {
            eprintln!("capture: {note}");
        }
        feature
    }

    /// Registers the shortcuts of the config, replacing the previous ones; returns the ones that failed.
    pub fn configure(&self) -> Vec<String> {
        self.hotkeys.borrow_mut().clear();
        let bindings = self.config.borrow().capture.bindings();
        let mut notes = Vec::new();
        let mut hotkeys = Vec::new();
        for (kind, shortcut) in bindings {
            let spec = match parse_hotkey(&shortcut) {
                Ok(spec) => spec,
                Err(e) => {
                    notes.push(format!(
                        "invalid shortcut '{shortcut}' for {}: {e}",
                        kind.key()
                    ));
                    continue;
                }
            };
            let me = self.me.clone();
            match Hotkey::register(self.mtm, spec.key_code, spec.modifiers, move || {
                if let Some(feature) = me.upgrade() {
                    feature.take(kind);
                }
            }) {
                Ok(hotkey) => hotkeys.push(hotkey),
                Err(status) => notes.push(format!(
                    "cannot register '{shortcut}' for {} (status {status}); another app probably owns it",
                    kind.key()
                )),
            }
        }
        *self.hotkeys.borrow_mut() = hotkeys;
        notes
    }

    pub fn open_folder(&self) {
        let folder = self.config.borrow().capture.folder(&self.home);
        let _ = fs::create_dir_all(&folder);
        workspace::open_path(&folder);
    }

    /// Starts a capture; ignored while another one is being picked.
    pub fn take(&self, kind: Kind) {
        trace::log(&format!("capture: {} requested", kind.key()));
        if self.busy.get() || self.scrolling.borrow().is_some() {
            return;
        }
        let text = i18n::current();
        if !screen_capture::screen_access(false) {
            screen_capture::screen_access(true);
            capture_ui::toast(self.mtm, text.capture_needs_permission());
            return;
        }
        self.busy.set(true);
        if kind == Kind::Timer {
            let seconds = self.config.borrow().capture.timer_seconds;
            self.countdown(seconds);
            return;
        }
        self.freeze_then(kind);
    }

    fn countdown(&self, left: u32) {
        if left == 0 {
            self.freeze_then(Kind::Timer);
            return;
        }
        capture_ui::toast(self.mtm, &i18n::current().capture_countdown(left));
        let me = self.me.clone();
        app::after(1.0, move || {
            if let Some(feature) = me.upgrade() {
                feature.countdown(left - 1);
            }
        });
    }

    fn freeze_then(&self, kind: Kind) {
        let me = self.me.clone();
        screen_capture::freeze(move |result| {
            let Some(feature) = me.upgrade() else {
                return;
            };
            match result {
                Ok(frozen) => feature.frozen(kind, Rc::new(frozen)),
                Err(e) => feature.fail(&e),
            }
        });
    }

    fn fail(&self, detail: &str) {
        self.busy.set(false);
        eprintln!("capture: {detail}");
        capture_ui::toast(self.mtm, &i18n::current().capture_failed(detail));
    }

    fn frozen(&self, kind: Kind, frozen: Rc<Frozen>) {
        trace::log(&format!(
            "capture: froze {} display(s)",
            frozen.displays.len()
        ));
        let text = i18n::current();
        let (mode, hint) = match kind {
            Kind::Screen | Kind::Timer => {
                let index = display_under_pointer(&frozen);
                self.picked(kind, &frozen, Picked::Display(index));
                return;
            }
            Kind::Area => (Mode::Any, text.capture_hint_any()),
            Kind::Window => (Mode::Window, text.capture_hint_window()),
            Kind::Text => (Mode::AreaOnly, text.capture_hint_text()),
            Kind::Scroll => (Mode::AreaOnly, text.capture_hint_scroll()),
        };
        let me = self.me.clone();
        let held = Rc::clone(&frozen);
        let overlay = capture_overlay::pick(self.mtm, &frozen, mode, hint, move |picked| {
            if let Some(feature) = me.upgrade() {
                feature.overlay.borrow_mut().take();
                feature.picked(kind, &held, picked);
            }
        });
        *self.overlay.borrow_mut() = Some(overlay);
    }

    fn picked(&self, kind: Kind, frozen: &Rc<Frozen>, picked: Picked) {
        trace::log(&format!("capture: picked {picked:?}"));
        match picked {
            Picked::Cancelled => self.busy.set(false),
            Picked::Display(index) => match frozen.displays.get(index) {
                Some(d) => self.finish(
                    kind,
                    Taken {
                        shot: d.shot.clone(),
                        screen: d.frame,
                        place: d.frame,
                    },
                ),
                None => self.fail("display gone"),
            },
            Picked::Area { display, rect } if kind == Kind::Scroll => {
                self.busy.set(false);
                let (Some(d), Some(feature)) = (frozen.displays.get(display), self.me.upgrade())
                else {
                    return;
                };
                let Some(capture) = screen_capture::AreaCapture::new(frozen, display, rect) else {
                    return self.fail("display gone");
                };
                let area = NSRect::new(
                    NSPoint::new(
                        d.frame.origin.x + rect.origin.x,
                        d.frame.origin.y + rect.origin.y,
                    ),
                    rect.size,
                );
                let scrolling = scroll::Scrolling::start(&feature, capture, area, d.frame);
                *self.scrolling.borrow_mut() = Some(scrolling);
            }
            Picked::Area { display, rect } => {
                let Some(d) = frozen.displays.get(display) else {
                    return self.fail("display gone");
                };
                let size = d.frame.size;
                let area = Rect::new(
                    rect.origin.x,
                    rect.origin.y,
                    rect.size.width,
                    rect.size.height,
                );
                let bounds = Rect::new(0.0, 0.0, size.width, size.height);
                let pixels = area.to_pixels(&bounds, d.shot.width(), d.shot.height());
                match pixels.and_then(|p| d.shot.crop(p.x, p.y, p.w, p.h)) {
                    Some(shot) => {
                        let place = NSRect::new(
                            NSPoint::new(
                                d.frame.origin.x + rect.origin.x,
                                d.frame.origin.y + rect.origin.y,
                            ),
                            rect.size,
                        );
                        self.finish(
                            kind,
                            Taken {
                                shot,
                                screen: d.frame,
                                place,
                            },
                        );
                    }
                    None => self.busy.set(false),
                }
            }
            Picked::Window(id) => {
                let frame = frozen.windows.iter().find(|w| w.id == id).map(|w| w.frame);
                let screen = frame
                    .and_then(|f| display_of(frozen, f))
                    .map_or(NSRect::ZERO, |d| d.frame);
                let me = self.me.clone();
                screen_capture::capture_window(frozen, id, move |result| {
                    let Some(feature) = me.upgrade() else {
                        return;
                    };
                    match result {
                        Ok(shot) => feature.finish(
                            kind,
                            Taken {
                                shot,
                                screen,
                                place: frame.unwrap_or(screen),
                            },
                        ),
                        Err(e) => feature.fail(&e),
                    }
                });
            }
        }
    }

    fn finish(&self, kind: Kind, taken: Taken) {
        self.busy.set(false);
        if kind == Kind::Text {
            self.recognize(&taken.shot);
            return;
        }
        self.deliver(taken);
    }

    fn recognize(&self, shot: &Shot) {
        let mtm = self.mtm;
        ocr::recognize(shot, move |result| {
            let text = i18n::current();
            match result {
                Ok(found) if !found.is_empty() => {
                    let mut parts = found.lines.clone();
                    parts.extend(found.codes.iter().cloned());
                    pasteboard().write(Write::Text(&parts.join("\n")));
                    capture_ui::toast(mtm, &text.capture_text_copied(parts.len()));
                }
                Ok(_) => capture_ui::toast(mtm, text.capture_no_text()),
                Err(e) => capture_ui::toast(mtm, &text.capture_failed(&e)),
            }
        });
    }

    /// Saves, copies and shows the quick panel, as the config asks.
    fn deliver(&self, taken: Taken) {
        let text = i18n::current();
        let Some(png) = taken.shot.png() else {
            return self.fail("cannot encode the picture");
        };
        let config = self.config.borrow().capture.clone();
        let name = file_name(FILE_PREFIX, stamp(), "png");
        let folder = if config.save {
            config.folder(&self.home)
        } else {
            env::temp_dir().join("figghost-captures")
        };
        let path = unique_path(&folder, &name, |p| p.exists());
        if let Err(e) = fs::create_dir_all(&folder).and_then(|()| fs::write(&path, &png)) {
            capture_ui::toast(self.mtm, &text.capture_write_failed(&e.to_string()));
        }
        trace::log(&format!("capture: wrote {}", path.display()));
        if config.copy {
            pasteboard().write(Write::Png(&png));
        }
        if config.quick_panel {
            self.show_quick(taken, path, config.quick_seconds);
        }
    }

    fn show_quick(&self, taken: Taken, path: PathBuf, seconds: f64) {
        let text = i18n::current();
        let id = self.next_id.replace(self.next_id.get() + 1);
        let slot = self.quick.borrow().len();
        let taken = Rc::new(taken);
        let (image, screen) = (taken.shot.ns_image(), taken.screen);
        let me = self.me.clone();
        let (held, file) = (Rc::clone(&taken), path.clone());
        let with = move |act: fn(&CaptureFeature, u64, &Rc<Taken>, &Path)| {
            let (me, taken, path) = (me.clone(), Rc::clone(&held), file.clone());
            Box::new(move || {
                if let Some(feature) = me.upgrade() {
                    act(&feature, id, &taken, &path);
                }
            }) as Box<dyn Fn()>
        };
        let actions = QuickActions {
            on_copy: with(|f, _, t, _| f.copy(&t.shot)),
            on_save: with(|f, _, t, _| f.save_as(&t.shot)),
            on_edit: with(|f, id, t, path| {
                f.close_quick(id);
                f.edit(t, path);
            }),
            on_pin: with(|f, id, t, _| {
                f.close_quick(id);
                f.pin(t);
            }),
            on_close: with(|f, id, _, _| f.close_quick(id)),
        };
        let texts = QuickTexts {
            copy: text.capture_copy().into(),
            save: text.capture_save().into(),
            edit: text.capture_edit().into(),
            pin: text.capture_pin().into(),
            close: text.capture_close().into(),
        };
        let panel = QuickPanel::show(self.mtm, image, &path, screen, slot, &texts, actions);
        self.quick.borrow_mut().push((id, panel));
        if seconds > 0.0 {
            self.close_quick_later(id, seconds);
        }
    }

    /// Opens the editor; Done writes the edited picture over the capture's file.
    fn edit(&self, taken: &Rc<Taken>, path: &Path) {
        let text = i18n::current();
        let id = self.next_id.replace(self.next_id.get() + 1);
        let me = self.me.clone();
        let screen = taken.screen;
        let with = |act: fn(&CaptureFeature, Shot, NSRect, &Path)| {
            let (me, path) = (me.clone(), path.to_owned());
            Box::new(move |shot: Shot| {
                if let Some(feature) = me.upgrade() {
                    act(&feature, shot, screen, &path);
                }
            }) as Box<dyn Fn(Shot)>
        };
        let actions = EditorActions {
            on_copy: with(|f, shot, _, _| f.copy(&shot)),
            on_save: with(|f, shot, _, _| f.save_as(&shot)),
            on_pin: with(|f, shot, screen, _| {
                let size = shot.point_size();
                let place = NSRect::new(
                    NSPoint::new(
                        screen.origin.x + (screen.size.width - size.width).max(0.0) / 2.0,
                        screen.origin.y + (screen.size.height - size.height).max(0.0) / 2.0,
                    ),
                    size,
                );
                f.pin(&Rc::new(Taken {
                    shot,
                    screen,
                    place,
                }));
            }),
            on_done: with(|f, shot, _, path| f.finish_edit(&shot, path)),
            on_closed: {
                let me = me.clone();
                Box::new(move || {
                    if let Some(feature) = me.upgrade() {
                        feature.editors.borrow_mut().retain(|(i, _)| *i != id);
                    }
                })
            },
        };
        let tools = [
            text.editor_select(),
            text.editor_arrow(),
            text.editor_line(),
            text.editor_frame(),
            text.editor_ellipse(),
            text.editor_highlight(),
            text.editor_text(),
            text.editor_counter(),
            text.editor_pixelate(),
            text.editor_crop(),
        ];
        let texts = EditorTexts {
            title: text.editor_title().into(),
            tools: tools.iter().map(|t| (*t).to_owned()).collect(),
            undo: text.editor_undo().into(),
            redo: text.editor_redo().into(),
            background: text.editor_background().into(),
            copy: text.capture_copy().into(),
            save: text.capture_save().into(),
            pin: text.capture_pin().into(),
            done: text.editor_done().into(),
        };
        let window = EditorWindow::open(self.mtm, &taken.shot, &texts, actions);
        self.editors.borrow_mut().push((id, window));
    }

    /// Done in the editor: the edited picture replaces the capture's file and, as configured, the clipboard.
    fn finish_edit(&self, shot: &Shot, path: &Path) {
        let text = i18n::current();
        let Some(png) = shot.png() else {
            return;
        };
        if let Err(e) = fs::write(path, &png) {
            capture_ui::toast(self.mtm, &text.capture_write_failed(&e.to_string()));
            return;
        }
        if self.config.borrow().capture.copy {
            pasteboard().write(Write::Png(&png));
        }
        let name = path
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();
        capture_ui::toast(self.mtm, &text.capture_saved(&name));
    }

    fn close_quick_later(&self, id: u64, seconds: f64) {
        let me = self.me.clone();
        app::after(seconds, move || {
            let Some(feature) = me.upgrade() else {
                return;
            };
            let hovered = feature
                .quick
                .borrow()
                .iter()
                .any(|(i, panel)| *i == id && panel.is_hovered());
            if hovered {
                feature.close_quick_later(id, HOVER_RECHECK_SECONDS);
            } else {
                feature.close_quick(id);
            }
        });
    }

    fn close_quick(&self, id: u64) {
        let mut quick = self.quick.borrow_mut();
        if let Some(index) = quick.iter().position(|(i, _)| *i == id) {
            quick.remove(index).1.close();
        }
    }

    fn copy(&self, shot: &Shot) {
        if let Some(png) = shot.png() {
            pasteboard().write(Write::Png(&png));
            capture_ui::toast(self.mtm, i18n::current().capture_copied());
        }
    }

    fn save_as(&self, shot: &Shot) {
        let text = i18n::current();
        let folder = self.config.borrow().capture.folder(&self.home);
        let name = file_name(FILE_PREFIX, stamp(), "png");
        let Some(path) = capture_ui::ask_save_path(self.mtm, &folder, &name) else {
            return;
        };
        match shot.png().map(|png| fs::write(&path, png)) {
            Some(Ok(())) => {
                let name = path
                    .file_name()
                    .map(|n| n.to_string_lossy().into_owned())
                    .unwrap_or_default();
                capture_ui::toast(self.mtm, &text.capture_saved(&name));
            }
            Some(Err(e)) => capture_ui::toast(self.mtm, &text.capture_write_failed(&e.to_string())),
            None => {}
        }
    }

    /// Floats the picture where it was taken.
    fn pin(&self, taken: &Rc<Taken>) {
        let text = i18n::current();
        let id = self.next_id.replace(self.next_id.get() + 1);
        let place = taken.place;
        let top_left = NSPoint::new(place.origin.x, place.origin.y + place.size.height);
        let me = self.me.clone();
        let with = move |act: fn(&CaptureFeature, u64, &Rc<Taken>)| {
            let (me, taken) = (me.clone(), Rc::clone(taken));
            Box::new(move || {
                if let Some(feature) = me.upgrade() {
                    act(&feature, id, &taken);
                }
            }) as Box<dyn Fn()>
        };
        let actions = PinActions {
            on_copy: with(|f, _, t| f.copy(&t.shot)),
            on_save: with(|f, _, t| f.save_as(&t.shot)),
            on_closed: with(|f, id, _| f.pins.borrow_mut().retain(|(i, _)| *i != id)),
        };
        let texts = PinTexts {
            copy: text.capture_copy().into(),
            save: text.capture_save().into(),
            close: text.capture_close().into(),
        };
        let pin = Pin::show(self.mtm, taken.shot.ns_image(), top_left, &texts, actions);
        self.pins.borrow_mut().push((id, pin));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn built_in_shortcuts_parse_and_can_be_turned_off() {
        let mut config = CaptureConfig::default();
        let bindings = config.bindings();
        assert_eq!(bindings.len(), 2);
        for (kind, shortcut) in &bindings {
            assert!(parse_hotkey(shortcut).is_ok(), "{}: {shortcut}", kind.key());
        }
        config.shortcuts.insert("area".into(), String::new());
        config
            .shortcuts
            .insert("window".into(), "cmd+shift+5".into());
        let keys: Vec<&str> = config.bindings().iter().map(|(k, _)| k.key()).collect();
        assert_eq!(keys, ["window", "text"]);
    }

    #[test]
    fn the_folder_follows_the_config_then_the_system() {
        let home = Path::new("/Users/someone");
        let mut config = CaptureConfig {
            folder: "~/Pictures/Shots".into(),
            ..CaptureConfig::default()
        };
        assert_eq!(config.folder(home), home.join("Pictures/Shots"));
        config.folder = "/Volumes/Data/shots".into();
        assert_eq!(config.folder(home), PathBuf::from("/Volumes/Data/shots"));
    }

    #[test]
    fn hand_edited_numbers_are_clamped() {
        let config: CaptureConfig =
            serde_json::from_str(r#"{"quick_seconds": -4, "timer_seconds": 999}"#).unwrap();
        let config = config.sanitized();
        assert_eq!((config.quick_seconds, config.timer_seconds), (0.0, 60));
        assert!(config.save && config.copy && config.quick_panel);
    }
}
