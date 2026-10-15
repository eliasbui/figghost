//! The editor that opens on a captured image: arrows, shapes, text, highlighter, blur and crop, on a canvas
//! that grows when something is dragged past its edge. Owned by the `annotate` task; the `capture` task calls
//! `open` as it is declared here, so these types and the signature must not change.

use crate::{MainThreadMarker, pasteboard::Pasteboard};
use core_annotate::{Point, Session};
use objc2_app_kit::{NSEvent, NSEventModifierFlags, NSEventType};
use objc2_foundation::{NSPoint, NSRect, NSString};
use std::{path::PathBuf, rc::Rc};
use window::{Setup, Shared};

mod canvas;
pub mod render;
mod toolbar;
mod window;

pub use canvas::Phase;
pub use toolbar::Button;
pub use window::{Placement, write_unique};

/// Every label the editor shows, in the language of the app. The editor never has text of its own: the
/// caller fills this in, so the translations stay with the rest of the app.
#[derive(Debug, Clone)]
pub struct Strings {
    pub window_title: String,
    pub select: String,
    pub arrow: String,
    pub line: String,
    pub rectangle: String,
    pub ellipse: String,
    pub text: String,
    pub pen: String,
    pub highlighter: String,
    pub blur: String,
    pub crop: String,
    pub undo: String,
    pub redo: String,
    pub delete: String,
    pub copy: String,
    pub save: String,
    pub done: String,
    pub colour: String,
    pub thickness: String,
    pub fill: String,
    pub text_placeholder: String,
    pub saved: String,
    pub copied: String,
    pub save_failed: String,
    pub copy_failed: String,
}

impl Strings {
    /// English labels, for tests and examples.
    pub fn english() -> Self {
        let s = |text: &str| text.to_owned();
        Self {
            window_title: s("Edit capture"),
            select: s("Select"),
            arrow: s("Arrow"),
            line: s("Line"),
            rectangle: s("Rectangle"),
            ellipse: s("Ellipse"),
            text: s("Text"),
            pen: s("Pen"),
            highlighter: s("Highlighter"),
            blur: s("Blur"),
            crop: s("Crop"),
            undo: s("Undo"),
            redo: s("Redo"),
            delete: s("Delete"),
            copy: s("Copy"),
            save: s("Save"),
            done: s("Done"),
            colour: s("Colour"),
            thickness: s("Thickness"),
            fill: s("Fill"),
            text_placeholder: s("Type here"),
            saved: s("Saved"),
            copied: s("Copied"),
            save_failed: s("Could not save"),
            copy_failed: s("Could not copy"),
        }
    }
}

/// What the person did last before the window closed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Outcome {
    Copied,
    Saved(PathBuf),
    Closed,
}

pub struct Request {
    /// The captured image as PNG, of any size.
    pub png: Vec<u8>,
    /// Pixels per point of the display it came from, so that it opens at the size it was on screen.
    pub scale: f64,
    pub title: String,
    /// Where Save writes. The file name is made up here and an existing file is never overwritten.
    pub save_dir: PathBuf,
    pub strings: Strings,
    /// Called once, when the window closes.
    pub on_finish: Box<dyn Fn(Outcome)>,
}

/// Where the window goes and which clipboard Copy writes to. [`open`] reads both from the environment the
/// rest of the app uses for tests; checks pass them in.
pub struct Options {
    pub placement: Placement,
    pub pasteboard: Pasteboard,
}

impl Options {
    pub fn from_env() -> Self {
        Self {
            placement: Placement::from_env(),
            pasteboard: match std::env::var("FIGGHOST_PASTEBOARD") {
                Ok(name) => Pasteboard::named(&name),
                Err(_) => Pasteboard::general(),
            },
        }
    }
}

/// The open editor window. Dropping it closes the window.
pub struct Editor {
    shared: Option<Rc<Shared>>,
}

/// Opens the editor window next to nothing in particular: centred on the screen that has the pointer, in
/// front, taking the keyboard without making the app the active one. A picture that cannot be read opens
/// nothing and reports [`Outcome::Closed`] at once.
pub fn open(mtm: MainThreadMarker, request: Request) -> Editor {
    open_with(mtm, request, Options::from_env())
}

pub fn open_with(mtm: MainThreadMarker, request: Request, options: Options) -> Editor {
    let Some(source) = render::Source::from_png(&request.png) else {
        (request.on_finish)(Outcome::Closed);
        return Editor { shared: None };
    };
    let shared = Shared::build(
        mtm,
        source,
        Setup {
            request,
            placement: options.placement,
            pasteboard: options.pasteboard,
        },
    );
    shared.show();
    Editor {
        shared: Some(shared),
    }
}

/// The calls below are for checks: they drive the window the way input would, without any real input.
impl Editor {
    pub fn is_open(&self) -> bool {
        self.shared.as_ref().is_some_and(|s| !s.is_finished())
    }

    /// Changes the model directly, as a script, and updates the window to match.
    pub fn with_session<R>(&self, change: impl FnOnce(&mut Session) -> R) -> Option<R> {
        self.shared.as_ref().map(|s| s.edit(change))
    }

    /// What Copy and Save write.
    pub fn export_png(&self) -> Option<Vec<u8>> {
        self.shared.as_ref()?.export()
    }

    /// The content of the window drawn to PNG, without showing it.
    pub fn snapshot_png(&self) -> Option<Vec<u8>> {
        self.shared.as_ref()?.snapshot_png()
    }

    pub fn frame(&self) -> Option<NSRect> {
        self.shared.as_ref().map(|s| s.panel.frame())
    }

    pub fn is_key(&self) -> bool {
        self.shared.as_ref().is_some_and(|s| s.panel.isKeyWindow())
    }

    /// Delivers a mouse event at a point of the document, through the window's own event handling.
    pub fn send_mouse(&self, phase: Phase, at: Point, shift: bool, clicks: isize) {
        let Some(shared) = &self.shared else {
            return;
        };
        let in_view = shared.to_view(at);
        let in_window = shared
            .canvas
            .convertPoint_toView(NSPoint::new(in_view.x, in_view.y), None);
        let kind = match phase {
            Phase::Down => NSEventType::LeftMouseDown,
            Phase::Drag => NSEventType::LeftMouseDragged,
            Phase::Up => NSEventType::LeftMouseUp,
        };
        let flags = if shift {
            NSEventModifierFlags::Shift
        } else {
            NSEventModifierFlags::empty()
        };
        if let Some(event) = NSEvent::mouseEventWithType_location_modifierFlags_timestamp_windowNumber_context_eventNumber_clickCount_pressure(
            kind,
            in_window,
            flags,
            0.0,
            shared.panel.windowNumber(),
            None,
            0,
            clicks,
            if phase == Phase::Up { 0.0 } else { 1.0 },
        ) {
            shared.panel.sendEvent(&event);
        }
    }

    /// Presses a key: `characters` as the key types them without modifiers, and its virtual key code.
    pub fn send_key(&self, characters: &str, key_code: u16, flags: NSEventModifierFlags) {
        let Some(shared) = &self.shared else {
            return;
        };
        let chars = NSString::from_str(characters);
        if let Some(event) = NSEvent::keyEventWithType_location_modifierFlags_timestamp_windowNumber_context_characters_charactersIgnoringModifiers_isARepeat_keyCode(
            NSEventType::KeyDown,
            NSPoint::ZERO,
            flags,
            0.0,
            shared.panel.windowNumber(),
            None,
            &chars,
            &chars,
            false,
            key_code,
        ) {
            shared.panel.sendEvent(&event);
        }
    }

    /// Clicks a toolbar control, through the window's own event handling.
    pub fn click(&self, which: Button) {
        let Some(shared) = &self.shared else {
            return;
        };
        let Some(centre) = shared.control_centre(which) else {
            return;
        };
        for kind in [NSEventType::LeftMouseDown, NSEventType::LeftMouseUp] {
            if let Some(event) = NSEvent::mouseEventWithType_location_modifierFlags_timestamp_windowNumber_context_eventNumber_clickCount_pressure(
                kind,
                centre,
                NSEventModifierFlags::empty(),
                0.0,
                shared.panel.windowNumber(),
                None,
                0,
                1,
                1.0,
            ) {
                shared.panel.sendEvent(&event);
            }
        }
    }

    pub fn click_tool(&self, tool: core_annotate::Tool) {
        self.click(Button::Tool(tool));
    }

    /// Whether the window is the kind that takes the keyboard without making the app the active one.
    pub fn takes_keyboard_without_activating(&self) -> bool {
        self.shared.as_ref().is_some_and(|s| {
            s.panel.canBecomeKeyWindow()
                && s.panel
                    .styleMask()
                    .contains(objc2_app_kit::NSWindowStyleMask::NonactivatingPanel)
        })
    }

    /// Types into the text editor, as the input method would hand the text over.
    pub fn type_text(&self, text: &str) {
        if let Some(shared) = &self.shared {
            shared.type_text(text);
        }
    }

    /// The frame of the text editor over the canvas, when one is open.
    pub fn text_editor_frame(&self) -> Option<NSRect> {
        self.shared.as_ref()?.text_overlay_frame()
    }

    pub fn close(&self) {
        if let Some(shared) = &self.shared {
            shared.close();
        }
    }
}

impl Drop for Editor {
    fn drop(&mut self) {
        self.close();
    }
}
