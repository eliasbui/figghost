//! Glue between the pasteboard, the SQLite history and the list UI.

use crate::{
    i18n::{self, Language},
    trace,
};
use core_clipboard::{
    Board, Capture, Item, Kind, Payload, Rich, Sensitivity, Store, is_ignored_app, should_record,
    title_of,
};
use macos_sys::{
    app, ax,
    pasteboard::{Content, Pasteboard, Write},
    permissions, synth,
    ui::{Activate, Mark, Row},
    workspace,
};
use std::{
    cell::{Cell, RefCell},
    collections::HashMap,
    env,
    path::Path,
    time::{Instant, SystemTime, UNIX_EPOCH},
};

type RecordedHook = Box<dyn Fn(i64)>;
type CopiedHook = Box<dyn Fn(Copied)>;

/// What kind of thing a recorded copy was, for whoever shows that something was copied. Never its content,
/// apart from the paths of copied files.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Copied {
    Text,
    Image,
    Files(Vec<String>),
}

const LIST_LIMIT: usize = 200;
const FOCUS_POLL_SECONDS: f64 = 0.015;
const FOCUS_TIMEOUT_SECONDS: f64 = 0.8;
/// When the previous app cannot be confirmed as the focus owner, "focus left the popup" is accepted after this long.
const FOCUS_SETTLE_SECONDS: f64 = 0.25;
const KEYSTROKE_GAP_SECONDS: f64 = 0.03;

/// Calls `then(true)` once keyboard focus has left this process, meaning the popup is gone and another app
/// holds the keyboard again, or `then(false)` if that does not happen in time. A fixed delay is not enough:
/// macOS needs a variable time, often longer than 100 ms, to give the key window back, and a keystroke sent
/// earlier is lost.
pub(crate) fn when_focus_returns(previous_pid: Option<i32>, then: impl FnOnce(bool) + 'static) {
    let ours = std::process::id() as i32;
    let started = Instant::now();
    app::wait_until(
        FOCUS_POLL_SECONDS,
        FOCUS_TIMEOUT_SECONDS,
        move || {
            let settled = started.elapsed().as_secs_f64() >= FOCUS_SETTLE_SECONDS;
            match ax::focused_pid() {
                Some(pid) if pid == ours => false,
                Some(pid) => Some(pid) == previous_pid || settled,
                None => settled,
            }
        },
        then,
    );
}

pub fn now_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_millis() as i64)
}

pub fn ago(language: Language, now: i64, then: i64) -> String {
    let seconds = ((now - then) / 1000).max(0);
    match seconds {
        0..=44 => language.just_now().into(),
        45..=3599 => language.minutes_ago((seconds / 60).max(1)),
        3600..=86_399 => language.hours_ago(seconds / 3600),
        86_400..=2_591_999 => language.days_ago(seconds / 86_400),
        _ => language.months_ago(seconds / 2_592_000),
    }
}

fn file_title(language: Language, preview: &str) -> String {
    let names: Vec<&str> = preview
        .lines()
        .map(|p| {
            Path::new(p)
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or(p)
        })
        .collect();
    match names.as_slice() {
        [] => String::new(),
        [only] => (*only).into(),
        [first, rest @ ..] => language.files_title(first, rest.len()),
    }
}

fn empty_in(language: Language, query: &str) -> &'static str {
    if query.trim().is_empty() {
        language.history_empty()
    } else {
        language.no_matches()
    }
}

/// What the list says when it has nothing to show, in the app's language.
pub fn empty_text(query: &str) -> &'static str {
    empty_in(i18n::current(), query)
}

pub struct History {
    store: RefCell<Store>,
    pasteboard: Pasteboard,
    /// Change count already handled; `-1` makes the first poll capture whatever is on the pasteboard.
    last_seen: Cell<isize>,
    /// Change count produced by our own paste, which must not be recorded again.
    own_change: Cell<isize>,
    ignored_apps: RefCell<Vec<String>>,
    auto_paste: Cell<bool>,
    /// Text goes in without its formatting unless asked the other way.
    paste_plain: Cell<bool>,
    /// What to do with a copy that looks like a secret.
    sensitivity: Cell<Sensitivity>,
    /// Whether some masked item may be due to go, so the clock is only looked at when it can matter.
    expiring: Cell<bool>,
    /// Told the id of every copy that was recorded or moved to the top.
    on_recorded: RefCell<Option<RecordedHook>>,
    /// Told the kind of every copy that was recorded or moved to the top; separate from `on_recorded`, which
    /// the copy stack takes and gives back.
    on_copied: RefCell<Option<CopiedHook>>,
}

impl History {
    pub fn open(
        dir: &Path,
        limit: usize,
        ignored_apps: Vec<String>,
        auto_paste: bool,
        paste_plain: bool,
    ) -> core_clipboard::Result<Self> {
        // A named pasteboard keeps tests away from the user's real clipboard.
        let pasteboard = match env::var("FIGGHOST_PASTEBOARD") {
            Ok(name) => Pasteboard::named(&name),
            Err(_) => Pasteboard::general(),
        };
        Self::with_pasteboard(
            dir,
            limit,
            ignored_apps,
            auto_paste,
            paste_plain,
            pasteboard,
        )
    }

    fn with_pasteboard(
        dir: &Path,
        limit: usize,
        ignored_apps: Vec<String>,
        auto_paste: bool,
        paste_plain: bool,
        pasteboard: Pasteboard,
    ) -> core_clipboard::Result<Self> {
        let mut store = Store::open(dir)?;
        store.max_items = limit;
        let expiring = store.next_expiry()?.is_some();
        Ok(Self {
            store: RefCell::new(store),
            pasteboard,
            last_seen: Cell::new(-1),
            own_change: Cell::new(-1),
            ignored_apps: RefCell::new(ignored_apps),
            auto_paste: Cell::new(auto_paste),
            paste_plain: Cell::new(paste_plain),
            sensitivity: Cell::new(Sensitivity::Keep),
            expiring: Cell::new(expiring),
            on_recorded: RefCell::new(None),
            on_copied: RefCell::new(None),
        })
    }

    pub fn set_sensitivity(&self, sensitivity: Sensitivity) {
        self.sensitivity.set(sensitivity);
    }

    /// Registers who is told about each copy that is recorded, replacing the previous one.
    pub fn set_on_recorded(&self, hook: Option<RecordedHook>) {
        *self.on_recorded.borrow_mut() = hook;
    }

    /// Registers who is told what kind of thing each recorded copy was, replacing the previous one.
    pub fn set_on_copied(&self, hook: Option<CopiedHook>) {
        *self.on_copied.borrow_mut() = hook;
    }

    /// The pasteboard the history watches: the real one, or the private one of a test.
    pub fn pasteboard(&self) -> &Pasteboard {
        &self.pasteboard
    }

    /// Ignores the change that left the pasteboard at this change count: a copy that was made and undone on
    /// purpose (reading a selection) is not something the person copied.
    pub fn skip_change(&self, change_count: isize) {
        self.own_change.set(change_count);
    }

    /// Applies new settings to the running history: a lower limit trims it right away.
    pub fn configure(
        &self,
        limit: usize,
        ignored_apps: Vec<String>,
        auto_paste: bool,
        paste_plain: bool,
    ) {
        if let Err(e) = self.store.borrow_mut().set_limit(limit) {
            eprintln!("history: cannot apply the new limit: {e}");
        }
        *self.ignored_apps.borrow_mut() = ignored_apps;
        self.auto_paste.set(auto_paste);
        self.paste_plain.set(paste_plain);
    }

    /// Items in the history, pinned or not.
    pub fn count(&self) -> usize {
        self.store.borrow().count().unwrap_or(0)
    }

    /// Records a new pasteboard item if there is one and removes masked items whose time has come; returns
    /// whether the history changed.
    pub fn poll(&self) -> bool {
        let purged = self.purge_expired(now_ms());
        self.capture() || purged
    }

    fn purge_expired(&self, now: i64) -> bool {
        if !self.expiring.get() {
            return false;
        }
        let store = self.store.borrow();
        let removed = store.purge_expired(now).unwrap_or_else(|e| {
            eprintln!("history: cannot remove expired items: {e}");
            0
        });
        self.expiring
            .set(store.next_expiry().ok().flatten().is_some());
        removed > 0
    }

    fn capture(&self) -> bool {
        let count = self.pasteboard.change_count();
        if count == self.last_seen.get() {
            return false;
        }
        self.last_seen.set(count);
        if count == self.own_change.get() {
            return false;
        }
        let types = self.pasteboard.types();
        if !should_record(types.iter().map(String::as_str)) {
            return false;
        }
        let source = workspace::frontmost_app();
        if source
            .as_ref()
            .is_some_and(|a| is_ignored_app(&a.bundle_id, &self.ignored_apps.borrow()))
        {
            return false;
        }
        let Some(content) = self.pasteboard.read() else {
            return false;
        };
        // Formatting is only kept for text; an image or a file list has none worth keeping.
        let rich = match &content {
            Content::Text(_) => self.pasteboard.read_rich().map(|r| Rich {
                rtf: r.rtf,
                html: r.html,
            }),
            _ => None,
        };
        let payload = match content {
            Content::Files(files) => Payload::Files(files),
            Content::Text(text) => Payload::Text(text),
            Content::Image(i) => Payload::Image {
                png: i.png,
                width: i.width,
                height: i.height,
            },
        };
        let copied = match &payload {
            Payload::Text(_) => Copied::Text,
            Payload::Image { .. } => Copied::Image,
            Payload::Files(files) => Copied::Files(files.clone()),
        };
        let capture = Capture {
            payload,
            app_id: source.as_ref().map(|a| a.bundle_id.clone()),
            app_name: source.map(|a| a.name).filter(|n| !n.is_empty()),
            rich,
        };
        let sensitivity = self.sensitivity.get();
        let recorded = self
            .store
            .borrow_mut()
            .record_with(capture, now_ms(), sensitivity);
        match recorded {
            Ok(Some(id)) => {
                if matches!(sensitivity, Sensitivity::Mask { .. }) {
                    self.expiring.set(true);
                }
                if let Some(hook) = &*self.on_recorded.borrow() {
                    hook(id);
                }
                if let Some(hook) = &*self.on_copied.borrow() {
                    hook(copied);
                }
                true
            }
            Ok(None) => false,
            Err(e) => {
                eprintln!("history: cannot record item: {e}");
                false
            }
        }
    }

    pub fn rows(&self, query: &str) -> (Vec<Row>, HashMap<i64, Mark>) {
        let store = self.store.borrow();
        let items = store.search(query, LIST_LIMIT).unwrap_or_else(|e| {
            eprintln!("history: search failed: {e}");
            Vec::new()
        });
        let boards: HashMap<i64, String> = store
            .boards()
            .unwrap_or_default()
            .into_iter()
            .map(|b| (b.id, b.name))
            .collect();
        let now = now_ms();
        let rows = items
            .iter()
            .map(|i| row_for(&store, i, now, &boards))
            .collect();
        let marks = items
            .iter()
            .filter(|i| i.masked || i.board.is_some())
            .map(|i| {
                let mark = Mark {
                    sensitive: i.masked,
                    hidden: i.masked,
                    board: i.board,
                };
                (i.id, mark)
            })
            .collect();
        (rows, marks)
    }

    pub fn boards(&self) -> Vec<Board> {
        self.store.borrow().boards().unwrap_or_default()
    }

    pub fn add_board(&self, name: &str) -> core_clipboard::Result<i64> {
        self.store.borrow().add_board(name)
    }

    pub fn rename_board(&self, id: i64, name: &str) -> core_clipboard::Result<()> {
        self.store.borrow().rename_board(id, name)
    }

    pub fn delete_board(&self, id: i64) -> core_clipboard::Result<()> {
        self.store.borrow().delete_board(id)
    }

    /// Files an item in a board, or takes it out (`None`).
    pub fn set_board(&self, item: i64, board: Option<i64>) {
        if let Err(e) = self.store.borrow().set_board(item, board) {
            eprintln!("history: cannot move item {item}: {e}");
        }
    }

    /// The first line of a masked item, to show it once the user asked to see it.
    pub fn reveal_title(&self, id: i64) -> Option<String> {
        match self.store.borrow().payload(id) {
            Ok(Some(Payload::Text(text))) => Some(title_of(&text)),
            _ => None,
        }
    }

    /// Puts an item on the pasteboard exactly as `activate` would, without moving it in the history and
    /// without pasting it. False when the item is gone.
    pub fn stage(&self, id: i64) -> bool {
        let store = self.store.borrow();
        self.put_on_pasteboard(&store, id, self.paste_plain.get())
            .is_some()
    }

    /// Writes an item to the pasteboard; the change is remembered as ours so the poll skips it.
    fn put_on_pasteboard(&self, store: &Store, id: i64, plain: bool) -> Option<()> {
        let payload = match store.payload(id) {
            Ok(Some(p)) => p,
            Ok(None) => return None,
            Err(e) => {
                eprintln!("history: cannot load item {id}: {e}");
                return None;
            }
        };
        let count = match &payload {
            Payload::Text(t) => {
                let rich = if plain {
                    None
                } else {
                    store.rich(id).ok().flatten().filter(|r| !r.is_empty())
                };
                match rich {
                    Some(rich) => self.pasteboard.write(Write::RichText {
                        text: t,
                        rtf: rich.rtf.as_deref(),
                        html: rich.html.as_deref(),
                    }),
                    None => self.pasteboard.write(Write::Text(t)),
                }
            }
            Payload::Image { png, .. } => self.pasteboard.write(Write::Png(png)),
            Payload::Files(files) => self.pasteboard.write(Write::Files(files)),
        };
        self.own_change.set(count);
        Some(())
    }

    /// Puts the item back on the pasteboard, moves it to the top and pastes it into the frontmost app.
    /// Text keeps its formatting unless plain text is the default (`mode` flips which one applies).
    pub fn activate(&self, id: i64, mode: Activate) {
        let store = self.store.borrow();
        let plain = match mode {
            Activate::Normal => self.paste_plain.get(),
            Activate::Alternate => !self.paste_plain.get(),
        };
        if self.put_on_pasteboard(&store, id, plain).is_none() {
            return;
        }
        let _ = store.touch(id, now_ms());
        trace::log(&format!("activate id={id} plain={plain}"));
        // Without Accessibility access the keystroke would be dropped silently; the item is still on
        // the pasteboard for a manual Command+V.
        let paste = self.auto_paste.get() && permissions::accessibility_trusted(false);
        if paste || trace::enabled() {
            let previous = workspace::frontmost_app().map(|a| a.pid);
            let started = Instant::now();
            when_focus_returns(previous, move |ready| {
                trace::log(&format!(
                    "keyboard focus left the popup after {} ms (ready={ready}, previous app pid={previous:?})",
                    started.elapsed().as_millis()
                ));
                if paste && ready {
                    app::after(KEYSTROKE_GAP_SECONDS, || {
                        trace::log("post Command+V");
                        synth::post_paste();
                    });
                }
            });
        }
    }

    pub fn toggle_pin(&self, id: i64) {
        let _ = self.store.borrow().toggle_pinned(id);
    }

    pub fn delete(&self, id: i64) {
        let _ = self.store.borrow().delete(id);
    }

    pub fn clear_unpinned(&self) {
        let _ = self.store.borrow().clear_unpinned();
    }
}

fn row_for(store: &Store, item: &Item, now: i64, boards: &HashMap<i64, String>) -> Row {
    let language = i18n::current();
    let title = match item.kind {
        Kind::Image => language.image_title(item.width, item.height),
        Kind::File => file_title(language, &item.preview),
        _ => title_of(&item.preview),
    };
    let mut parts = Vec::new();
    if item.pinned {
        parts.push(language.pinned().to_owned());
    }
    if let Some(board) = item.board.and_then(|id| boards.get(&id)) {
        parts.push(board.clone());
    }
    if item.masked {
        parts.push(language.sensitive_item().to_owned());
    }
    if let Some(app) = &item.app_name {
        parts.push(app.clone());
    }
    parts.push(ago(language, now, item.used_at));
    Row {
        pin_key: item.pin_key,
        id: item.id,
        title,
        subtitle: parts.join(", "),
        pinned: item.pinned,
        mono: item.kind == Kind::Code,
        thumb: item.image.as_deref().map(|name| store.image_path(name)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const NOW: i64 = 10_000_000_000;

    #[test]
    fn relative_time_in_both_languages() {
        let (en, vi) = (Language::English, Language::Vietnamese);
        for (language, expected) in [
            (
                en,
                [
                    "just now",
                    "1 minute ago",
                    "59 minutes ago",
                    "3 hours ago",
                    "2 days ago",
                    "1 month ago",
                ],
            ),
            (
                vi,
                [
                    "vừa xong",
                    "1 phút trước",
                    "59 phút trước",
                    "3 giờ trước",
                    "2 ngày trước",
                    "1 tháng trước",
                ],
            ),
        ] {
            let ages = [
                5_000,
                60_000,
                59 * 60_000,
                3 * 3_600_000,
                2 * 86_400_000,
                40 * 86_400_000,
            ];
            for (age, want) in ages.into_iter().zip(expected) {
                assert_eq!(ago(language, NOW, NOW - age), want, "{language:?}");
            }
        }
        assert_eq!(
            ago(en, NOW, NOW + 5_000),
            "just now",
            "clock skew never goes negative"
        );
    }

    #[test]
    fn file_titles_use_names_not_paths() {
        let (en, vi) = (Language::English, Language::Vietnamese);
        assert_eq!(file_title(en, "/Users/a/report.pdf"), "report.pdf");
        assert_eq!(
            file_title(en, "/a/one.txt\n/b/two.txt\n/c/three.txt"),
            "one.txt and 2 other files"
        );
        assert_eq!(
            file_title(vi, "/a/one.txt\n/b/two.txt"),
            "one.txt và 1 tệp khác"
        );
        assert_eq!(file_title(en, ""), "");
    }

    #[test]
    fn empty_state_message_depends_on_query() {
        assert_eq!(
            empty_in(Language::English, "  "),
            "Nothing in the clipboard history yet"
        );
        assert_eq!(empty_in(Language::English, "abc"), "No matches");
        assert_eq!(
            empty_in(Language::Vietnamese, "abc"),
            "Không tìm thấy kết quả"
        );
    }

    mod secrets_boards_and_the_stack {
        use super::*;
        use std::{
            rc::Rc,
            sync::atomic::{AtomicUsize, Ordering},
        };

        fn history() -> (History, std::path::PathBuf) {
            static N: AtomicUsize = AtomicUsize::new(0);
            let n = N.fetch_add(1, Ordering::Relaxed);
            let dir = env::temp_dir().join(format!(
                "figghost-history-secret-{}-{n}",
                std::process::id()
            ));
            std::fs::create_dir_all(&dir).unwrap();
            let board =
                Pasteboard::named(&format!("figghost-test-secret-{}-{n}", std::process::id()));
            let history =
                History::with_pasteboard(&dir, 100, Vec::new(), false, false, board).unwrap();
            (history, dir)
        }

        fn copy(history: &History, text: &str) -> bool {
            history.pasteboard.write(Write::Text(text));
            history.poll()
        }

        fn finish(history: &History, dir: std::path::PathBuf) {
            history.pasteboard.release();
            let _ = std::fs::remove_dir_all(dir);
        }

        const SECRET: &str = "password: hunter2xyz";

        #[test]
        fn a_copied_secret_is_hidden_unsearchable_revealed_on_request_and_expires() {
            let (history, dir) = history();
            history.set_sensitivity(Sensitivity::Mask { minutes: 10 });
            assert!(copy(&history, "ordinary note"));
            assert!(copy(&history, SECRET));
            let (rows, marks) = history.rows("");
            assert_eq!(rows.len(), 2);
            let secret = &rows[0];
            assert_eq!(secret.title, "", "the list never receives the text");
            assert!(!secret.subtitle.contains("hunter"));
            assert!(secret.subtitle.contains("Sensitive"), "{}", secret.subtitle);
            assert_eq!(
                marks.get(&secret.id),
                Some(&Mark {
                    sensitive: true,
                    hidden: true,
                    board: None
                })
            );
            assert!(!marks.contains_key(&rows[1].id));
            // Search cannot find it, and the id gives the text only on request.
            assert_eq!(history.rows("hunter2xyz").0.len(), 0);
            assert_eq!(history.reveal_title(secret.id).as_deref(), Some(SECRET));
            // It expires; the ordinary note stays.
            assert!(!history.purge_expired(now_ms()), "not before its time");
            assert!(history.purge_expired(now_ms() + 11 * 60_000));
            assert_eq!(history.rows("").0.len(), 1);
            assert!(
                !history.purge_expired(now_ms() + 99 * 60_000),
                "nothing left to expire"
            );
            finish(&history, dir);
        }

        #[test]
        fn the_choice_decides_whether_a_secret_is_stored_masked_or_not_at_all() {
            let (history, dir) = history();
            assert!(copy(&history, SECRET), "off: stored like any copy");
            assert!(history.rows("").1.is_empty());
            history.clear_unpinned();
            history.set_sensitivity(Sensitivity::Skip);
            assert!(!copy(&history, SECRET), "never: not stored");
            assert!(copy(&history, "a plain note"), "other copies are stored");
            assert_eq!(history.rows("").0.len(), 1);
            finish(&history, dir);
        }

        #[test]
        fn staging_an_item_is_not_a_copy_and_does_not_reorder_the_history() {
            let (history, dir) = history();
            assert!(copy(&history, "first"));
            assert!(copy(&history, "second"));
            let before: Vec<i64> = history.rows("").0.iter().map(|r| r.id).collect();
            let oldest = before[1];
            assert!(history.stage(oldest));
            assert_eq!(
                history.pasteboard.read(),
                Some(Content::Text("first".into()))
            );
            assert!(!history.poll(), "our own write is not recorded again");
            let after: Vec<i64> = history.rows("").0.iter().map(|r| r.id).collect();
            assert_eq!(before, after, "the order did not change");
            assert!(!history.stage(9999), "a deleted item cannot be staged");
            finish(&history, dir);
        }

        #[test]
        fn the_hook_is_told_of_each_recorded_copy_in_order() {
            let (history, dir) = history();
            let heard = Rc::new(RefCell::new(Vec::new()));
            let sink = Rc::clone(&heard);
            history.set_on_recorded(Some(Box::new(move |id| sink.borrow_mut().push(id))));
            copy(&history, "one");
            copy(&history, "two");
            copy(&history, "one");
            let ids = heard.borrow().clone();
            assert_eq!(ids.len(), 3);
            assert_eq!(ids[0], ids[2], "the same text is the same item");
            assert_ne!(ids[0], ids[1]);
            history.set_on_recorded(None);
            copy(&history, "three");
            assert_eq!(heard.borrow().len(), 3, "a removed hook hears nothing");
            finish(&history, dir);
        }

        #[test]
        fn the_copied_hook_hears_real_copies_beside_the_stack_hook() {
            let (history, dir) = history();
            let heard = Rc::new(RefCell::new(Vec::new()));
            let sink = Rc::clone(&heard);
            history.set_on_copied(Some(Box::new(move |c| sink.borrow_mut().push(c))));
            // The copy stack takes the other hook and gives it back; this one stays.
            history.set_on_recorded(Some(Box::new(|_| {})));
            copy(&history, "one");
            history.set_on_recorded(None);
            copy(&history, "two");
            assert_eq!(*heard.borrow(), vec![Copied::Text, Copied::Text]);
            let first = history.rows("").0[1].id;
            assert!(history.stage(first));
            assert!(!history.poll());
            assert_eq!(heard.borrow().len(), 2, "our own write is not a copy");
            finish(&history, dir);
        }

        #[test]
        fn a_filed_item_shows_its_board_and_is_found_by_it() {
            let (history, dir) = history();
            copy(&history, "invoice 42");
            copy(&history, "unrelated");
            let id = history.rows("invoice").0[0].id;
            let board = history.add_board("Work").unwrap();
            history.set_board(id, Some(board));
            let (rows, marks) = history.rows("@work");
            assert_eq!(rows.len(), 1);
            assert!(rows[0].pinned && rows[0].pin_key.is_some());
            assert!(
                rows[0].subtitle.starts_with("Pinned, Work"),
                "{}",
                rows[0].subtitle
            );
            assert_eq!(marks[&id].board, Some(board));
            history.delete_board(board).unwrap();
            let (rows, marks) = history.rows("@pin");
            assert_eq!(rows.len(), 1, "deleting the board unpins nothing");
            assert!(marks.is_empty());
            finish(&history, dir);
        }
    }

    mod pasting {
        use super::*;
        use macos_sys::pasteboard::Rich as BoardRich;
        use std::sync::atomic::{AtomicUsize, Ordering};

        /// A history on a private pasteboard and a throwaway directory.
        fn history(paste_plain: bool) -> (History, std::path::PathBuf) {
            static N: AtomicUsize = AtomicUsize::new(0);
            let n = N.fetch_add(1, Ordering::Relaxed);
            let dir =
                env::temp_dir().join(format!("figghost-history-test-{}-{n}", std::process::id()));
            std::fs::create_dir_all(&dir).unwrap();
            let board =
                Pasteboard::named(&format!("figghost-test-history-{}-{n}", std::process::id()));
            let history =
                History::with_pasteboard(&dir, 100, Vec::new(), false, paste_plain, board).unwrap();
            (history, dir)
        }

        fn copy_formatted(history: &History) -> i64 {
            history.pasteboard.write(Write::RichText {
                text: "Hello world",
                rtf: Some(b"{\\rtf1 Hello world}"),
                html: Some(b"<b>Hello world</b>"),
            });
            assert!(history.poll(), "the copy is recorded");
            history.store.borrow().search("", 10).unwrap()[0].id
        }

        fn has_formatting(history: &History) -> bool {
            history
                .pasteboard
                .read_rich()
                .is_some_and(|r: BoardRich| r.rtf.is_some() && r.html.is_some())
        }

        #[test]
        fn formatted_text_is_pasted_with_its_formatting_and_the_other_key_removes_it() {
            let (history, dir) = history(false);
            let id = copy_formatted(&history);
            history.pasteboard.write(Write::Text("something else"));
            history.activate(id, Activate::Normal);
            assert!(
                has_formatting(&history),
                "the usual paste keeps the formatting"
            );
            assert_eq!(
                history.pasteboard.read(),
                Some(Content::Text("Hello world".into()))
            );
            history.activate(id, Activate::Alternate);
            assert!(!has_formatting(&history), "the other key pastes plain text");
            assert_eq!(
                history.pasteboard.read(),
                Some(Content::Text("Hello world".into()))
            );
            history.pasteboard.release();
            let _ = std::fs::remove_dir_all(dir);
        }

        #[test]
        fn plain_by_default_flips_the_two_keys() {
            let (history, dir) = history(true);
            let id = copy_formatted(&history);
            history.activate(id, Activate::Normal);
            assert!(!has_formatting(&history));
            history.activate(id, Activate::Alternate);
            assert!(has_formatting(&history));
            // The setting can change while the app runs.
            history.configure(100, Vec::new(), false, false);
            history.activate(id, Activate::Normal);
            assert!(has_formatting(&history));
            history.pasteboard.release();
            let _ = std::fs::remove_dir_all(dir);
        }

        #[test]
        fn our_own_paste_is_not_recorded_again_and_plain_copies_have_no_formatting() {
            let (history, dir) = history(false);
            let id = copy_formatted(&history);
            history.activate(id, Activate::Normal);
            assert!(!history.poll(), "the history does not record its own paste");
            history.pasteboard.write(Write::Text("just text"));
            assert!(history.poll());
            let newest = history.store.borrow().search("just text", 10).unwrap()[0].id;
            assert_eq!(history.store.borrow().rich(newest).unwrap(), None);
            history.activate(newest, Activate::Normal);
            assert!(
                !has_formatting(&history),
                "nothing to restore for a plain copy"
            );
            history.pasteboard.release();
            let _ = std::fs::remove_dir_all(dir);
        }
    }
}
