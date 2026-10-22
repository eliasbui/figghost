//! The notes of the Notes tab: where they live (one plain text file each, so that any other app can read them),
//! how a title is found in a text, and how the list of them is kept in order while one is being written. Nothing
//! here draws and nothing here knows a language.

#![forbid(unsafe_code)]
use std::{
    cmp::Reverse,
    fs,
    io::{self, Write},
    path::{Path, PathBuf},
    time::UNIX_EPOCH,
};

/// More notes than this are not read; the rest stay where they are as files.
pub const MAX_NOTES: usize = 200;
/// A note holds no more characters than this: a longer text is cut when it is written, and a file that is longer is
/// not listed at all, so that a huge file cannot hang the editor and its end can never be lost by editing it.
pub const MAX_CHARS: usize = 50_000;
/// The longest title a row shows, in characters.
const TITLE_CHARS: usize = 60;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Note {
    /// The number in the file name: when the note was made, in milliseconds since 1970, nudged up if another
    /// note already has it.
    pub id: u64,
    pub text: String,
    /// When it was last written, in milliseconds since 1970.
    pub updated: i64,
}

impl Note {
    pub fn is_blank(&self) -> bool {
        self.text.trim().is_empty()
    }
}

/// `note-1760000000000.md`
fn file_name(id: u64) -> String {
    format!("note-{id}.md")
}

/// The id a file name carries, if it is one of ours: other files in the folder are left alone.
fn id_of(name: &str) -> Option<u64> {
    let digits = name.strip_prefix("note-")?.strip_suffix(".md")?;
    if digits.is_empty() || digits.len() > 20 || !digits.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    digits.parse().ok()
}

/// A text cut to `MAX_CHARS` characters, never in the middle of one.
pub fn clipped(text: &str) -> String {
    match text.char_indices().nth(MAX_CHARS) {
        Some((end, _)) => text[..end].to_owned(),
        None => text.to_owned(),
    }
}

fn lines(text: &str) -> impl Iterator<Item = &str> {
    text.lines().map(str::trim).filter(|l| !l.is_empty())
}

fn cut(line: &str, max: usize) -> String {
    if line.chars().count() <= max {
        return line.to_owned();
    }
    let kept: String = line.chars().take(max.saturating_sub(1)).collect();
    format!("{}…", kept.trim_end())
}

/// The first line that says anything, without the marks of a heading; empty for a blank note.
pub fn title(text: &str) -> String {
    lines(text)
        .next()
        .map(|l| cut(l.trim_start_matches('#').trim_start(), TITLE_CHARS))
        .unwrap_or_default()
}

/// The next line after the title, for the second line of a row; empty when there is none.
pub fn detail(text: &str) -> String {
    lines(text)
        .nth(1)
        .map(|l| cut(l, TITLE_CHARS))
        .unwrap_or_default()
}

/// The folder the notes are kept in.
pub struct Folder {
    dir: PathBuf,
}

impl Folder {
    pub fn new(dir: PathBuf) -> Self {
        Self { dir }
    }

    pub fn dir(&self) -> &Path {
        &self.dir
    }

    fn path(&self, id: u64) -> PathBuf {
        self.dir.join(file_name(id))
    }

    /// Every note in the folder, the one written last first, no more than `MAX_NOTES`. The files are put in order from
    /// what the folder says of them and only the newest are read, so that a folder with thousands does not stall the page.
    /// A file that cannot be read as text is skipped, not deleted, and so is one longer than a note may be: it would be
    /// cut when it was shown and its end lost with the next change.
    pub fn load(&self) -> Vec<Note> {
        let Ok(entries) = fs::read_dir(&self.dir) else {
            return Vec::new();
        };
        let mut files: Vec<(i64, u64, PathBuf)> = entries
            .flatten()
            .filter_map(|entry| {
                let id = id_of(entry.file_name().to_str()?)?;
                let meta = entry.metadata().ok().filter(fs::Metadata::is_file)?;
                // A file this large is not a note; reading it would stall the page.
                if meta.len() > (MAX_CHARS as u64) * 4 {
                    return None;
                }
                let updated = meta
                    .modified()
                    .ok()
                    .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
                    .map_or(0, |d| d.as_millis() as i64);
                Some((updated, id, entry.path()))
            })
            .collect();
        files.sort_by_key(|(updated, id, _)| Reverse((*updated, *id)));
        let mut notes = Vec::new();
        for (updated, id, path) in files {
            if notes.len() == MAX_NOTES {
                break;
            }
            let Ok(text) = fs::read_to_string(path) else {
                continue;
            };
            if text.trim().is_empty() || text.chars().count() > MAX_CHARS {
                continue;
            }
            notes.push(Note { id, text, updated });
        }
        notes
    }

    /// How many notes the folder holds, from the names alone, so that it is cheap to ask.
    pub fn count(&self) -> usize {
        fs::read_dir(&self.dir)
            .map(|entries| {
                entries
                    .flatten()
                    .filter(|e| e.file_name().to_str().and_then(id_of).is_some())
                    .filter(|e| e.metadata().is_ok_and(|m| m.is_file()))
                    .count()
            })
            .unwrap_or(0)
    }

    /// Writes the note whole or not at all: to a file of its own first, then renamed over the note. A blank note
    /// is not kept, so a note that was emptied leaves no file behind.
    pub fn save(&self, note: &Note) -> io::Result<()> {
        if note.is_blank() {
            return self.discard(note.id);
        }
        write_atomically(&self.path(note.id), &clipped(&note.text))
    }

    /// Removes a file that holds nothing worth keeping; a file that is not there is not an error.
    fn discard(&self, id: u64) -> io::Result<()> {
        match fs::remove_file(self.path(id)) {
            Err(e) if e.kind() != io::ErrorKind::NotFound => Err(e),
            _ => Ok(()),
        }
    }

    /// Puts a note's file in the trash through `trash`, so that a deleted note can still be got back.
    pub fn remove(&self, id: u64, trash: impl Fn(&Path) -> io::Result<()>) -> io::Result<()> {
        let path = self.path(id);
        if !path.exists() {
            return Ok(());
        }
        trash(&path)
    }
}

/// Writes `text` to `path` through a file of its own (`<name>.part`, made new so that a link left at that name is not
/// followed, and flushed to the disk) and then renames it over the target, so that a crash or a full disk never leaves
/// half of what was there, nor an empty file after a power cut.
pub fn write_atomically(path: &Path, text: &str) -> io::Result<()> {
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir)?;
    }
    let mut name = path.as_os_str().to_owned();
    name.push(".part");
    let partial = PathBuf::from(name);
    // What stands at that name, a leftover of a crash or a link somebody put there, goes first.
    let _ = fs::remove_file(&partial);
    let written = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&partial)
        .and_then(|mut file| {
            file.write_all(text.as_bytes())?;
            file.sync_all()
        });
    written
        .and_then(|()| fs::rename(&partial, path))
        .inspect_err(|_| {
            let _ = fs::remove_file(&partial);
        })
}

/// The notes in memory, newest first, and which one is open.
#[derive(Debug, Default)]
pub struct Notes {
    items: Vec<Note>,
    current: Option<u64>,
    dirty: bool,
}

/// One row of the list.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Row {
    pub title: String,
    pub detail: String,
    pub updated: i64,
    pub selected: bool,
}

impl Notes {
    pub fn new(mut items: Vec<Note>) -> Self {
        items.sort_by_key(|n| Reverse((n.updated, n.id)));
        items.truncate(MAX_NOTES);
        let current = items.first().map(|n| n.id);
        Self {
            items,
            current,
            dirty: false,
        }
    }

    pub fn len(&self) -> usize {
        self.items.len()
    }

    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    pub fn current(&self) -> Option<&Note> {
        self.items.iter().find(|n| Some(n.id) == self.current)
    }

    pub fn rows(&self) -> Vec<Row> {
        self.items
            .iter()
            .map(|n| Row {
                title: title(&n.text),
                detail: detail(&n.text),
                updated: n.updated,
                selected: Some(n.id) == self.current,
            })
            .collect()
    }

    /// Opens the note at `index` of the list; false when there is none.
    pub fn select(&mut self, index: usize) -> bool {
        match self.items.get(index) {
            Some(note) => {
                self.current = Some(note.id);
                true
            }
            None => false,
        }
    }

    /// Starts an empty note at the top of the list and opens it. A note that was started and left blank is
    /// dropped first, so that pressing the button twice does not pile up empty ones. The new note is not a file
    /// until it holds something.
    pub fn start_new(&mut self, now: i64) -> u64 {
        self.drop_blank();
        let newest = self.items.iter().map(|n| n.id).max().unwrap_or(0);
        let id = u64::try_from(now)
            .unwrap_or(0)
            .max(newest.saturating_add(1));
        self.items.insert(
            0,
            Note {
                id,
                text: String::new(),
                updated: now,
            },
        );
        self.current = Some(id);
        id
    }

    /// Removes the blank notes from the list, which have no file to lose.
    pub fn drop_blank(&mut self) {
        let current = self.current;
        self.items.retain(|n| !n.is_blank());
        if !self.items.iter().any(|n| Some(n.id) == current) {
            self.current = self.items.first().map(|n| n.id);
        }
    }

    /// The open note's text changed: remember it, and bring the note to the top of the list.
    pub fn edit(&mut self, text: &str, now: i64) {
        let text = clipped(text);
        let Some(at) = self.items.iter().position(|n| Some(n.id) == self.current) else {
            return;
        };
        if self.items[at].text == text {
            return;
        }
        let mut note = self.items.remove(at);
        note.text = text;
        note.updated = now.max(note.updated);
        self.items.insert(0, note);
        self.dirty = true;
    }

    /// The open note, if it has changed since it was last asked for; asking clears the change.
    pub fn take_dirty(&mut self) -> Option<Note> {
        if !std::mem::take(&mut self.dirty) {
            return None;
        }
        self.current().cloned()
    }

    /// Takes the open note out of the list and opens its neighbour; gives the note back so that its file can go.
    pub fn delete_current(&mut self) -> Option<Note> {
        let at = self.items.iter().position(|n| Some(n.id) == self.current)?;
        let gone = self.items.remove(at);
        self.current = self
            .items
            .get(at.min(self.items.len().saturating_sub(1)))
            .map(|n| n.id);
        self.dirty = false;
        Some(gone)
    }
}

/// How many characters a text has, for the line under the editor.
pub fn count(text: &str) -> usize {
    text.chars().count()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        fs::File,
        time::{Duration, SystemTime},
    };

    fn scratch(name: &str) -> PathBuf {
        let dir =
            std::env::temp_dir().join(format!("figghost-notes-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        dir
    }

    fn note(id: u64, text: &str, updated: i64) -> Note {
        Note {
            id,
            text: text.into(),
            updated,
        }
    }

    #[test]
    fn a_title_is_the_first_line_that_says_something() {
        assert_eq!(title("Groceries\nmilk\neggs"), "Groceries");
        assert_eq!(title("\n\n   \n  Trip plan  \nday one"), "Trip plan");
        assert_eq!(title("## A heading"), "A heading");
        assert_eq!(title(""), "");
        assert_eq!(title(" \n\t\n"), "");
        let long = "x".repeat(200);
        let cut = title(&long);
        assert_eq!(cut.chars().count(), TITLE_CHARS);
        assert!(cut.ends_with('…'));
        let vietnamese =
            "Việc cần làm hôm nay là viết ghi chú thật dài để thử cắt ở giữa một chữ có dấu nhé";
        assert!(title(vietnamese).chars().count() <= TITLE_CHARS);
    }

    #[test]
    fn the_detail_is_the_line_after_the_title() {
        assert_eq!(detail("Groceries\nmilk\neggs"), "milk");
        assert_eq!(detail("Only a title"), "");
        assert_eq!(detail("A\n\n\nB"), "B");
    }

    #[test]
    fn only_our_own_file_names_are_notes() {
        assert_eq!(id_of("note-1760000000000.md"), Some(1_760_000_000_000));
        for other in [
            "notes.md",
            "note-.md",
            "note-12.txt",
            "note-+12.md",
            "note--5.md",
            "note-12a.md",
            "note-99999999999999999999999.md",
            ".DS_Store",
            "note-12.md.part",
        ] {
            assert_eq!(id_of(other), None, "{other}");
        }
        assert_eq!(id_of(&file_name(7)), Some(7));
    }

    #[test]
    fn a_text_is_cut_to_the_limit_on_a_character_boundary() {
        let text = "ệ".repeat(MAX_CHARS + 10);
        let cut = clipped(&text);
        assert_eq!(cut.chars().count(), MAX_CHARS);
        assert!(text.starts_with(&cut));
        assert_eq!(clipped("short"), "short");
    }

    #[test]
    fn saving_and_loading_round_trip_and_the_newest_comes_first() {
        let dir = scratch("roundtrip");
        let folder = Folder::new(dir.clone());
        assert!(
            folder.load().is_empty(),
            "a folder that is not there has no notes"
        );
        for (id, text) in [(10, "old"), (20, "middle"), (30, "newest")] {
            folder.save(&note(id, text, 0)).unwrap();
            let when = SystemTime::UNIX_EPOCH + Duration::from_secs(1_000 + id);
            File::options()
                .write(true)
                .open(dir.join(file_name(id)))
                .unwrap()
                .set_modified(when)
                .unwrap();
        }
        assert_eq!(folder.count(), 3);
        let loaded = folder.load();
        let texts: Vec<&str> = loaded.iter().map(|n| n.text.as_str()).collect();
        assert_eq!(texts, ["newest", "middle", "old"]);
        assert_eq!(loaded[0].id, 30);
        assert_eq!(loaded[0].updated, 1_030_000);
        // Nothing but the notes is left in the folder: no half-written file.
        let mut names: Vec<String> = fs::read_dir(&dir)
            .unwrap()
            .flatten()
            .map(|e| e.file_name().to_string_lossy().into_owned())
            .collect();
        names.sort();
        assert_eq!(names, ["note-10.md", "note-20.md", "note-30.md"]);
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn what_is_not_a_note_in_the_folder_is_left_alone() {
        let dir = scratch("strangers");
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join("readme.txt"), "mine").unwrap();
        fs::write(dir.join("note-5.md"), "a note").unwrap();
        fs::write(dir.join("note-6.md"), [0xff, 0xfe, 0x00]).unwrap();
        fs::write(dir.join("note-7.md"), "   \n").unwrap();
        fs::create_dir(dir.join("note-8.md")).unwrap();
        let loaded = Folder::new(dir.clone()).load();
        assert_eq!(loaded.len(), 1, "{loaded:?}");
        assert_eq!(loaded[0].text, "a note");
        assert!(dir.join("readme.txt").exists() && dir.join("note-6.md").exists());
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn a_file_too_long_to_be_a_note_is_left_alone_and_never_cut() {
        let dir = scratch("huge");
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join("note-1.md"), "x".repeat(MAX_CHARS * 5)).unwrap();
        fs::write(dir.join("note-2.md"), "y".repeat(MAX_CHARS + 500)).unwrap();
        fs::write(dir.join("note-3.md"), "z".repeat(MAX_CHARS)).unwrap();
        // Characters, not bytes: a text of 50,000 two-byte characters is a note.
        fs::write(dir.join("note-4.md"), "\u{1EC7}".repeat(MAX_CHARS)).unwrap();
        let loaded = Folder::new(dir.clone()).load();
        let mut ids: Vec<u64> = loaded.iter().map(|n| n.id).collect();
        ids.sort_unstable();
        assert_eq!(ids, [3, 4], "only what a note may hold is listed");
        assert!(
            loaded.iter().all(|n| n.text.chars().count() == MAX_CHARS),
            "and it is whole"
        );
        assert_eq!(
            fs::read_to_string(dir.join("note-2.md"))
                .unwrap()
                .chars()
                .count(),
            MAX_CHARS + 500,
            "what is not listed is not touched"
        );
        // A text that is written longer than that is cut, never in the middle of a character.
        let folder = Folder::new(dir.clone());
        folder
            .save(&note(9, &"w".repeat(MAX_CHARS + 10), 1))
            .unwrap();
        assert_eq!(
            fs::read_to_string(dir.join("note-9.md"))
                .unwrap()
                .chars()
                .count(),
            MAX_CHARS
        );
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn only_the_newest_files_are_read_and_blank_ones_do_not_take_a_place() {
        let dir = scratch("newest");
        fs::create_dir_all(&dir).unwrap();
        let stamp = |id: u64, text: &str| {
            fs::write(dir.join(file_name(id)), text).unwrap();
            File::options()
                .write(true)
                .open(dir.join(file_name(id)))
                .unwrap()
                .set_modified(SystemTime::UNIX_EPOCH + Duration::from_secs(1_000 + id))
                .unwrap();
        };
        let total = MAX_NOTES as u64 + 5;
        for id in 0..total {
            stamp(id, &format!("note {id}"));
        }
        // The newest ten files are blank, which is no note.
        for id in total..total + 10 {
            stamp(id, "   \n");
        }
        let loaded = Folder::new(dir.clone()).load();
        assert_eq!(loaded.len(), MAX_NOTES);
        assert_eq!(
            loaded[0].id,
            total - 1,
            "the newest note that has something in it comes first"
        );
        assert_eq!(loaded[MAX_NOTES - 1].id, total - MAX_NOTES as u64);
        assert!(loaded.iter().all(|n| !n.is_blank()));
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn a_stray_file_named_with_the_largest_number_does_not_overflow_the_next_id() {
        let mut notes = Notes::default();
        notes.start_new(5);
        let largest = u64::MAX;
        notes.items[0].id = largest;
        notes.items[0].text = "something".into();
        let next = notes.start_new(5);
        assert_eq!(
            next,
            u64::MAX,
            "no overflow; it is as far as the numbers go"
        );
    }

    #[test]
    fn at_most_the_limit_of_notes_is_read() {
        let dir = scratch("many");
        fs::create_dir_all(&dir).unwrap();
        for id in 0..(MAX_NOTES as u64 + 20) {
            fs::write(dir.join(file_name(id)), format!("note {id}")).unwrap();
        }
        assert_eq!(Folder::new(dir.clone()).load().len(), MAX_NOTES);
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn a_blank_note_leaves_no_file_and_emptying_one_removes_it() {
        let dir = scratch("blank");
        let folder = Folder::new(dir.clone());
        folder.save(&note(1, "something", 5)).unwrap();
        assert!(dir.join("note-1.md").exists());
        folder.save(&note(1, "  \n ", 6)).unwrap();
        assert!(!dir.join("note-1.md").exists());
        folder.save(&note(2, "", 6)).unwrap();
        assert!(!dir.join("note-2.md").exists());
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn a_deleted_note_goes_through_the_trash_and_a_missing_one_is_no_error() {
        let dir = scratch("trash");
        let folder = Folder::new(dir.clone());
        folder.save(&note(3, "keep me findable", 5)).unwrap();
        let trashed = std::cell::RefCell::new(Vec::new());
        folder
            .remove(3, |path| {
                trashed.borrow_mut().push(path.to_path_buf());
                fs::remove_file(path)
            })
            .unwrap();
        assert_eq!(trashed.borrow().len(), 1);
        assert!(trashed.borrow()[0].ends_with("note-3.md"));
        folder
            .remove(3, |_| panic!("there is nothing to trash"))
            .unwrap();
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn the_list_starts_on_the_newest_note() {
        let notes = Notes::new(vec![note(1, "a", 10), note(2, "b", 30), note(3, "c", 20)]);
        assert_eq!(notes.current().unwrap().id, 2);
        let rows = notes.rows();
        assert_eq!(
            rows.iter().map(|r| r.title.as_str()).collect::<Vec<_>>(),
            ["b", "c", "a"]
        );
        assert!(rows[0].selected && !rows[1].selected);
        assert!(Notes::default().current().is_none());
    }

    #[test]
    fn a_new_note_is_blank_on_top_and_not_piled_up() {
        let mut notes = Notes::new(vec![note(1, "a", 10)]);
        let first = notes.start_new(100);
        assert_eq!(notes.current().unwrap().id, first);
        assert!(notes.current().unwrap().is_blank());
        // Pressing the button again replaces the blank one.
        let second = notes.start_new(200);
        assert_eq!(notes.len(), 2);
        assert_ne!(first, second);
        // An id never repeats, even when the clock does.
        notes.edit("typed", 200);
        let third = notes.start_new(150);
        assert!(third > second, "{third} {second}");
        assert_eq!(notes.len(), 3);
    }

    #[test]
    fn editing_brings_a_note_to_the_top_and_marks_it_for_saving() {
        let mut notes = Notes::new(vec![note(1, "a", 10), note(2, "b", 30)]);
        assert!(notes.select(1));
        assert!(
            notes.take_dirty().is_none(),
            "opening a note changes nothing"
        );
        notes.edit("a, longer", 50);
        let rows = notes.rows();
        assert_eq!(rows[0].title, "a, longer");
        assert!(rows[0].selected);
        let saved = notes.take_dirty().expect("changed");
        assert_eq!(
            (saved.id, saved.text.as_str(), saved.updated),
            (1, "a, longer", 50)
        );
        assert!(notes.take_dirty().is_none(), "asking clears it");
        notes.edit("a, longer", 60);
        assert!(notes.take_dirty().is_none(), "the same text is no change");
        assert!(!notes.select(9), "there is no tenth note");
    }

    #[test]
    fn deleting_opens_the_neighbour_and_gives_the_note_back() {
        let mut notes = Notes::new(vec![note(1, "a", 10), note(2, "b", 20), note(3, "c", 30)]);
        notes.select(1);
        let gone = notes.delete_current().unwrap();
        assert_eq!(gone.id, 2);
        assert_eq!(notes.len(), 2);
        assert_eq!(
            notes.current().unwrap().id,
            1,
            "the one that moved up into its place"
        );
        notes.delete_current().unwrap();
        notes.delete_current().unwrap();
        assert!(notes.current().is_none() && notes.delete_current().is_none());
    }

    #[test]
    fn a_blank_note_is_dropped_when_it_is_left() {
        let mut notes = Notes::new(vec![note(1, "a", 10)]);
        notes.start_new(20);
        assert_eq!(notes.len(), 2);
        notes.select(1);
        notes.drop_blank();
        assert_eq!(notes.len(), 1);
        assert_eq!(notes.current().unwrap().id, 1);
    }

    #[test]
    fn characters_are_counted_not_bytes() {
        assert_eq!(count("Việt"), 4);
        assert_eq!(count(""), 0);
    }
}
