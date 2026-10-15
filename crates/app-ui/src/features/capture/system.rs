//! The real `Backend`: the overlay, ScreenCaptureKit, the pasteboard, the editor and the confirmation.

use super::flow::{Backend, Captured, Note, PickMode};
use crate::i18n::{self, Language};
use core_capture::{Display, Grab, Pick, Point, Shot, naming::LocalTime};
use macos_sys::{
    MainThreadMarker,
    annotate::{self, Editor, Outcome, Request},
    app,
    capture::{
        self, background, grab,
        overlay::{Mode as OverlayMode, Overlay, Strings},
        screens, toast, windows,
    },
    pasteboard::{Pasteboard, Write},
    vision,
};
use std::{
    cell::{Cell, RefCell},
    collections::HashMap,
    path::Path,
    rc::{Rc, Weak},
};

/// How long the confirmation stays.
const TOAST_SECONDS: f64 = 4.0;
/// A text capture shows this much of the text in its confirmation.
const TEXT_PREVIEW_CHARS: usize = 90;

pub struct System {
    mtm: MainThreadMarker,
    me: Weak<System>,
    overlay: RefCell<Option<Overlay>>,
    /// The window numbers of the last overlay, which stay out of the picture even if the system has not yet
    /// taken the panels off the display.
    overlay_windows: RefCell<Vec<u32>>,
    editors: RefCell<HashMap<u64, Editor>>,
    next_editor: Cell<u64>,
}

impl System {
    pub fn new(mtm: MainThreadMarker) -> Rc<Self> {
        Rc::new_cyclic(|me| Self {
            mtm,
            me: me.clone(),
            overlay: RefCell::default(),
            overlay_windows: RefCell::default(),
            editors: RefCell::default(),
            next_editor: Cell::new(0),
        })
    }

    fn pasteboard() -> Pasteboard {
        match std::env::var("FIGGHOST_PASTEBOARD") {
            Ok(name) if !name.is_empty() => Pasteboard::named(&name),
            _ => Pasteboard::general(),
        }
    }

    /// The display the confirmation belongs on: the one with the pointer.
    fn display_with_pointer(&self) -> Option<Display> {
        let displays = screens::displays(self.mtm);
        core_capture::display_index_at(&displays, screens::pointer()).map(|i| displays[i])
    }
}

/// The text of a confirmation: a title, a detail, and whether the detail is a file name.
fn words(note: &Note, text: Language) -> (String, String, bool) {
    let plain = |title: &str, detail: &str| (title.to_owned(), detail.to_owned(), false);
    match note {
        Note::Captured(Captured {
            copied,
            saved,
            save_error,
            ..
        }) => match (copied, saved, save_error) {
            (true, Some(path), _) => (
                text.capture_copied_and_saved().to_owned(),
                file_name(path),
                true,
            ),
            (false, Some(path), _) => (text.capture_saved().to_owned(), file_name(path), true),
            (true, None, Some(error)) => plain(text.capture_copied_not_saved(), error),
            (false, None, Some(error)) => plain(text.capture_save_failed(), error),
            (true, None, None) => plain(text.capture_copied(), ""),
            (false, None, None) => plain(
                text.capture_kept_nothing(),
                text.capture_kept_nothing_detail(),
            ),
        },
        Note::Text(found) => plain(text.capture_text_copied(), &preview(found)),
        Note::NoText => plain(text.capture_text_none(), text.capture_text_none_detail()),
        Note::TextFailed(error) => plain(text.capture_text_failed(), error),
        Note::PermissionNeeded => plain(
            text.capture_permission_needed(),
            text.capture_permission_detail(),
        ),
        Note::NothingToRepeat => plain(
            text.capture_nothing_to_repeat(),
            text.capture_nothing_to_repeat_detail(),
        ),
        Note::Unavailable => plain(text.capture_unavailable(), ""),
        Note::Failed(message) => plain(text.capture_failed(), message),
    }
}

fn file_name(path: &Path) -> String {
    path.file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default()
}

/// The start of the text on one line, for the confirmation.
fn preview(text: &str) -> String {
    let one_line: String = text.split_whitespace().collect::<Vec<_>>().join(" ");
    if one_line.chars().count() <= TEXT_PREVIEW_CHARS {
        return one_line;
    }
    let cut: String = one_line.chars().take(TEXT_PREVIEW_CHARS).collect();
    format!("{cut}...")
}

impl Backend for System {
    fn available(&self) -> bool {
        grab::available()
    }

    fn permission(&self) -> bool {
        capture::permission_granted()
    }

    fn request_permission(&self) {
        capture::request_permission();
    }

    fn displays(&self) -> Vec<Display> {
        screens::displays(self.mtm)
    }

    fn pointer(&self) -> Point {
        screens::pointer()
    }

    fn pick(&self, mode: PickMode, done: Box<dyn FnOnce(Pick)>) {
        let text = i18n::current();
        let displays = screens::displays(self.mtm);
        let mode = match mode {
            PickMode::Region => OverlayMode::Region,
            PickMode::Window => OverlayMode::Window(windows::list(&displays)),
        };
        let strings = Strings {
            hint_region: text.capture_hint_region().to_owned(),
            hint_window: text.capture_hint_window().to_owned(),
        };
        let me = self.me.clone();
        // An earlier overlay that is still up is dropped, which takes it down without an answer.
        self.overlay.borrow_mut().take();
        let overlay = Overlay::show(
            self.mtm,
            mode,
            displays,
            screens::pointer(),
            strings,
            move |pick| {
                if let Some(system) = me.upgrade() {
                    system.overlay.borrow_mut().take();
                }
                done(pick);
            },
        );
        overlay.take_key();
        *self.overlay_windows.borrow_mut() = overlay.window_numbers();
        *self.overlay.borrow_mut() = Some(overlay);
    }

    fn grab(&self, what: Grab, done: Box<dyn FnOnce(Result<Shot, String>)>) {
        let mut exclude = toast::window_numbers();
        exclude.extend(self.overlay_windows.borrow().iter().copied());
        grab::grab(what, exclude, done);
    }

    fn copy_png(&self, png: &[u8]) {
        System::pasteboard().write(Write::Png(png));
    }

    fn copy_text(&self, text: &str) {
        System::pasteboard().write(Write::Text(text));
    }

    fn recognize(&self, png: Vec<u8>, done: Box<dyn FnOnce(Result<String, String>)>) {
        let languages = i18n::current().capture_text_languages();
        background::spawn(move || vision::recognize_text(&png, &languages), done);
    }

    fn edit(&self, shot: &Shot, folder: &Path, title: &str) {
        let key = self.next_editor.get();
        self.next_editor.set(key + 1);
        let me = self.me.clone();
        let request = Request {
            png: shot.png.clone(),
            scale: shot.scale,
            title: title.to_owned(),
            save_dir: folder.to_path_buf(),
            strings: i18n::current().capture_editor_strings(),
            on_finish: Box::new(move |_: Outcome| {
                // The editor is closing while this runs: let go of it a moment later.
                let me = me.clone();
                app::after(0.0, move || {
                    if let Some(system) = me.upgrade() {
                        system.editors.borrow_mut().remove(&key);
                    }
                });
            }),
        };
        let editor = annotate::open(self.mtm, request);
        self.editors.borrow_mut().insert(key, editor);
    }

    fn notify(&self, note: Note) {
        let (title, detail, detail_is_name) = words(&note, i18n::current());
        let thumbnail = match note {
            Note::Captured(Captured { png, .. }) => Some(png),
            _ => None,
        };
        toast::show(
            self.mtm,
            toast::Spec {
                title,
                detail,
                detail_is_name,
                thumbnail,
                display: self.display_with_pointer(),
                seconds: TOAST_SECONDS,
            },
        );
    }

    fn now(&self) -> LocalTime {
        screens::local_time()
    }

    fn name_template(&self) -> String {
        i18n::current().capture_file_template().to_owned()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn captured(copied: bool, saved: bool, error: Option<&str>) -> Note {
        Note::Captured(Captured {
            png: Vec::new(),
            copied,
            saved: saved.then(|| PathBuf::from("/Users/me/Pictures/Figghost/Shot.png")),
            save_error: error.map(str::to_owned),
        })
    }

    #[test]
    fn every_outcome_has_words_in_both_languages() {
        let notes = [
            captured(true, true, None),
            captured(false, true, None),
            captured(true, false, Some("read-only")),
            captured(false, false, Some("read-only")),
            captured(true, false, None),
            captured(false, false, None),
            Note::Text("hello".into()),
            Note::NoText,
            Note::TextFailed("not available".into()),
            Note::PermissionNeeded,
            Note::NothingToRepeat,
            Note::Unavailable,
            Note::Failed("no display".into()),
        ];
        for language in Language::ALL {
            for note in &notes {
                let (title, _, _) = words(note, language);
                assert!(!title.is_empty(), "{note:?} has no title in {language:?}");
            }
        }
    }

    #[test]
    fn a_saved_capture_names_its_file_and_an_error_shows_the_reason() {
        let (title, detail, is_name) = words(&captured(true, true, None), Language::English);
        assert_eq!(title, "Screenshot copied and saved");
        assert_eq!((detail.as_str(), is_name), ("Shot.png", true));
        let (_, detail, is_name) =
            words(&captured(true, false, Some("read-only")), Language::English);
        assert_eq!((detail.as_str(), is_name), ("read-only", false));
        let (_, detail, _) = words(
            &Note::TextFailed("text recognition is not available yet".into()),
            Language::English,
        );
        assert_eq!(detail, "text recognition is not available yet");
    }

    #[test]
    fn recognised_text_is_shown_on_one_line_and_cut() {
        assert_eq!(preview("one\n two\t three"), "one two three");
        let long = "word ".repeat(100);
        let shown = preview(&long);
        assert!(shown.ends_with("...") && shown.chars().count() <= TEXT_PREVIEW_CHARS + 3);
    }
}
