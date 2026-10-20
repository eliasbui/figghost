//! The Notes tab of the notch: a scratchpad for quick capture, with a list of notes at the left and an editor at the
//! right. It is off until the person turns it on.
//!
//! Each note is a plain text file in the `notes` folder of the data directory, so any other app can read it and
//! nothing is locked in. What is typed is written half a second after the last key and again when the tab closes;
//! a note that is deleted goes to the Trash, and a note that is left empty leaves no file. Nothing here needs a
//! permission and nothing leaves the Mac.

use super::AttachEnv;
use crate::{
    history::{ago, now_ms},
    i18n,
    settings::model::Fault,
    trace,
};
use logic::{Folder, Notes};
use macos_sys::{
    app,
    notes_ui::{NoteRow, NotesCallbacks, NotesDisplay, NotesView, PAGE_SIZE},
    workspace,
};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::{
    cell::{Cell, RefCell},
    path::Path,
    rc::{Rc, Weak},
};

pub mod logic;
pub mod settings;
mod text;

/// The choices of the size of the text, in points.
pub const TEXT_SIZES: [u32; 3] = macos_sys::notes_ui::TEXT_SIZES;
/// A change is written this long after the last key.
const SAVE_SECONDS: f64 = 0.5;
const FOLDER: &str = "notes";

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    pub enabled: bool,
    /// The size of the text in the editor, one of `TEXT_SIZES`.
    pub text_size: u32,
    /// Start a blank note each time the tab opens, instead of showing the last one.
    pub new_on_open: bool,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            enabled: false,
            text_size: 14,
            new_on_open: false,
        }
    }
}

impl Config {
    /// A config file can say anything; a size that is not offered is read as the default.
    pub fn sanitized(mut self) -> Self {
        if !TEXT_SIZES.contains(&self.text_size) {
            self.text_size = Self::default().text_size;
        }
        self
    }
}

pub struct Feature {
    view: Rc<NotesView>,
    folder: Folder,
    notes: RefCell<Notes>,
    config: RefCell<Config>,
    showing: Cell<bool>,
    /// Bumped whenever a save is wanted or the tab closes; a delayed save that finds another number does nothing.
    epoch: Cell<u64>,
    me: Weak<Feature>,
}

/// Adds the tab. `None` without a notch, which is where the tab lives.
pub fn attach(env: &AttachEnv) -> Option<Rc<Feature>> {
    let notch = env.notch?;
    let config = env.config.notes.clone().sanitized();
    let feature = Rc::new_cyclic(|me: &Weak<Feature>| {
        let (new, select, delete, edit, step) =
            (me.clone(), me.clone(), me.clone(), me.clone(), me.clone());
        let view = NotesView::new(
            env.mtm,
            notch.page_area(PAGE_SIZE.width, PAGE_SIZE.height),
            NotesCallbacks {
                on_new: Box::new(move || {
                    if let Some(f) = new.upgrade() {
                        f.start_new();
                    }
                }),
                on_select: Box::new(move |index| {
                    if let Some(f) = select.upgrade() {
                        f.select(index);
                    }
                }),
                on_delete: Box::new(move || {
                    if let Some(f) = delete.upgrade() {
                        f.delete();
                    }
                }),
                on_edit: Box::new(move |text| {
                    if let Some(f) = edit.upgrade() {
                        f.edited(&text);
                    }
                }),
                on_step: Box::new(move |delta| {
                    if let Some(f) = step.upgrade() {
                        f.step(delta);
                    }
                }),
            },
        );
        view.set_text_size(config.text_size);
        Feature {
            view,
            folder: Folder::new(env.home.join(FOLDER)),
            notes: RefCell::default(),
            config: RefCell::new(config),
            showing: Cell::new(false),
            epoch: Cell::new(0),
            me: me.clone(),
        }
    });
    let (shown, hidden) = (Rc::downgrade(&feature), Rc::downgrade(&feature));
    notch.add_page(feature.view.page(
        i18n::current().notes_tab().to_owned(),
        move || {
            if let Some(f) = shown.upgrade() {
                f.showing.set(true);
                f.begin();
            }
        },
        move || {
            if let Some(f) = hidden.upgrade() {
                f.showing.set(false);
                f.end();
            }
        },
    ));
    Some(feature)
}

impl Feature {
    /// Applies the settings after a change; a tab that is showing starts over with them.
    pub fn configure(&self, config: &Config) -> Result<(), Fault> {
        let config = config.clone().sanitized();
        self.view.set_text_size(config.text_size);
        *self.config.borrow_mut() = config;
        if self.showing.get() {
            self.flush();
            self.begin();
        }
        Ok(())
    }

    /// Facts only the running app knows, for the settings page.
    pub fn runtime(&self) -> Value {
        json!({
            "count": self.folder.count(),
            "folder": self.folder.dir().to_string_lossy(),
        })
    }

    /// A button of the settings page. `None` when the name is not this feature's.
    pub fn action(&self, name: &str) -> Option<Result<(), Fault>> {
        (name == "notes_folder").then(|| {
            // The folder is made on the first save, so a person who looks before writing anything finds it.
            let _ = std::fs::create_dir_all(self.folder.dir());
            workspace::open_path(self.folder.dir());
            Ok(())
        })
    }

    /// The tab opened: read the folder again, so that what other apps changed shows, and open a note.
    fn begin(&self) {
        let config = self.config.borrow().clone();
        let language = i18n::current();
        if !config.enabled {
            trace::log("notes: off");
            self.view.show(&NotesDisplay {
                note: Some((
                    language.notes_off_title().to_owned(),
                    language.notes_off_hint().to_owned(),
                )),
                ..NotesDisplay::default()
            });
            self.view.set_text("");
            return;
        }
        *self.notes.borrow_mut() = Notes::new(self.folder.load());
        let found = self.notes.borrow().len();
        trace::log(&format!("notes: opened with {found} notes"));
        if found == 0 || config.new_on_open {
            self.notes.borrow_mut().start_new(now_ms());
        }
        self.render(true);
    }

    /// The tab closed: write what is not written, and forget the notes that were started and left empty.
    fn end(&self) {
        self.flush();
        self.epoch.set(self.epoch.get() + 1);
        self.notes.borrow_mut().drop_blank();
    }

    /// Draws the list and the line under the editor; with `text`, also puts the open note in the editor.
    fn render(&self, text: bool) {
        let language = i18n::current();
        let now = now_ms();
        let notes = self.notes.borrow();
        let rows = notes
            .rows()
            .into_iter()
            .map(|row| {
                let time = ago(language, now, row.updated);
                NoteRow {
                    title: if row.title.is_empty() {
                        language.notes_new().to_owned()
                    } else {
                        row.title
                    },
                    detail: if row.detail.is_empty() {
                        time
                    } else {
                        format!("{time}  {}", row.detail)
                    },
                    selected: row.selected,
                }
            })
            .collect();
        let current = notes.current().map(|n| n.text.clone()).unwrap_or_default();
        self.view.show(&NotesDisplay {
            caption: language.notes_tab().to_owned(),
            rows,
            footer: language.notes_count(logic::count(&current)),
            placeholder: language.notes_placeholder().to_owned(),
            new_label: language.notes_new().to_owned(),
            delete_label: language.notes_delete().to_owned(),
            note: None,
        });
        if text {
            self.view.set_text(&current);
        }
    }

    /// The person typed: remember it, show the new title, and write it a moment after the last key.
    fn edited(&self, text: &str) {
        self.notes.borrow_mut().edit(text, now_ms());
        self.render(false);
        let epoch = self.epoch.get() + 1;
        self.epoch.set(epoch);
        let weak = self.me.clone();
        app::after(SAVE_SECONDS, move || {
            if let Some(f) = weak.upgrade()
                && f.epoch.get() == epoch
            {
                f.flush();
            }
        });
    }

    /// Writes the open note if it changed since it was written.
    fn flush(&self) {
        let changed = self.notes.borrow_mut().take_dirty();
        if let Some(note) = changed {
            match self.folder.save(&note) {
                Ok(()) => trace::log("notes: saved"),
                Err(e) => eprintln!("notes: cannot write the note: {e}"),
            }
        }
    }

    fn start_new(&self) {
        self.flush();
        self.notes.borrow_mut().start_new(now_ms());
        self.render(true);
        self.view.focus_editor();
    }

    fn select(&self, index: usize) {
        self.flush();
        if self.notes.borrow_mut().select(index) {
            self.notes.borrow_mut().drop_blank();
            self.render(true);
        }
    }

    /// Up or down in the list, while the editor does not have the keys.
    fn step(&self, delta: i32) {
        let rows = self.notes.borrow().rows();
        let Some(at) = rows.iter().position(|r| r.selected) else {
            return;
        };
        let target = (at as i64 + i64::from(delta)).clamp(0, rows.len() as i64 - 1);
        if target as usize != at {
            self.select(target as usize);
        }
    }

    /// The open note goes to the Trash, and its neighbour opens.
    fn delete(&self) {
        let gone = self.notes.borrow_mut().delete_current();
        if let Some(note) = gone
            && let Err(e) = self
                .folder
                .remove(note.id, |path: &Path| workspace::trash(path))
        {
            eprintln!("notes: cannot move the note to the Trash: {e}");
        }
        if self.notes.borrow().current().is_none() {
            self.notes.borrow_mut().start_new(now_ms());
        }
        self.render(true);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::i18n::Language;

    #[test]
    fn the_feature_is_off_by_default_and_old_files_load() {
        let config: Config = serde_json::from_str("{}").unwrap();
        assert_eq!(config, Config::default());
        assert!(!config.enabled && !config.new_on_open);
        let partial: Config =
            serde_json::from_str(r#"{ "enabled": true, "future_key": 1 }"#).unwrap();
        assert!(partial.enabled && partial.text_size == 14);
    }

    #[test]
    fn a_size_that_is_not_offered_is_read_as_the_default() {
        for size in TEXT_SIZES {
            let config = Config {
                text_size: size,
                ..Config::default()
            };
            assert_eq!(config.sanitized().text_size, size);
        }
        for size in [0, 11, 13, 99, u32::MAX] {
            let config = Config {
                text_size: size,
                ..Config::default()
            };
            assert_eq!(config.sanitized().text_size, 14, "{size}");
        }
    }

    #[test]
    fn the_words_are_there_in_both_languages() {
        for language in [Language::English, Language::Vietnamese] {
            for text in [
                language.notes_tab(),
                language.notes_placeholder(),
                language.notes_new(),
                language.notes_delete(),
                language.notes_off_title(),
                language.notes_off_hint(),
            ] {
                assert!(!text.is_empty());
            }
            assert!(language.notes_count(0).contains('0'));
            assert!(language.notes_count(1234).contains("1234"));
        }
        assert_eq!(Language::English.notes_count(1), "1 character");
        assert_eq!(Language::English.notes_count(2), "2 characters");
    }
}
