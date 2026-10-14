//! Glue between the pasteboard, the SQLite history and the list UI.

use crate::{
    i18n::{self, Language},
    trace,
};
use core_clipboard::{
    Capture, Item, Kind, Payload, Rich, Store, is_ignored_app, should_record, title_of,
};
use macos_sys::{
    app, ax,
    pasteboard::{Content, Pasteboard, Write},
    permissions, synth,
    ui::{Activate, Row},
    workspace,
};
use std::{
    cell::{Cell, RefCell},
    env,
    path::Path,
    time::{Instant, SystemTime, UNIX_EPOCH},
};

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
fn when_focus_returns(previous_pid: Option<i32>, then: impl FnOnce(bool) + 'static) {
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
}

impl History {
    pub fn open(
        dir: &Path,
        limit: usize,
        ignored_apps: Vec<String>,
        auto_paste: bool,
        paste_plain: bool,
    ) -> core_clipboard::Result<Self> {
        let mut store = Store::open(dir)?;
        store.max_items = limit;
        // A named pasteboard keeps tests away from the user's real clipboard.
        let pasteboard = match env::var("FIGGHOST_PASTEBOARD") {
            Ok(name) => Pasteboard::named(&name),
            Err(_) => Pasteboard::general(),
        };
        Ok(Self {
            store: RefCell::new(store),
            pasteboard,
            last_seen: Cell::new(-1),
            own_change: Cell::new(-1),
            ignored_apps: RefCell::new(ignored_apps),
            auto_paste: Cell::new(auto_paste),
            paste_plain: Cell::new(paste_plain),
        })
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

    /// Records a new pasteboard item if there is one; returns whether the history changed.
    pub fn poll(&self) -> bool {
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
        let capture = Capture {
            payload,
            app_id: source.as_ref().map(|a| a.bundle_id.clone()),
            app_name: source.map(|a| a.name).filter(|n| !n.is_empty()),
            rich,
        };
        match self.store.borrow_mut().record(capture, now_ms()) {
            Ok(recorded) => recorded.is_some(),
            Err(e) => {
                eprintln!("history: cannot record item: {e}");
                false
            }
        }
    }

    pub fn rows(&self, query: &str) -> Vec<Row> {
        let store = self.store.borrow();
        let items = store.search(query, LIST_LIMIT).unwrap_or_else(|e| {
            eprintln!("history: search failed: {e}");
            Vec::new()
        });
        let now = now_ms();
        items.iter().map(|i| row_for(&store, i, now)).collect()
    }

    /// Puts the item back on the pasteboard, moves it to the top and pastes it into the frontmost app.
    /// Text keeps its formatting unless plain text is the default (`mode` flips which one applies).
    pub fn activate(&self, id: i64, mode: Activate) {
        let store = self.store.borrow();
        let payload = match store.payload(id) {
            Ok(Some(p)) => p,
            Ok(None) => return,
            Err(e) => {
                eprintln!("history: cannot load item {id}: {e}");
                return;
            }
        };
        let plain = match mode {
            Activate::Normal => self.paste_plain.get(),
            Activate::Alternate => !self.paste_plain.get(),
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

fn row_for(store: &Store, item: &Item, now: i64) -> Row {
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
}
