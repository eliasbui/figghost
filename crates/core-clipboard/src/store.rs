use crate::{
    Kind, classify_text,
    search::{fold, fuzzy_score, parse_query},
};
use rusqlite::{Connection, OptionalExtension, params, params_from_iter, types::Value};
use std::{
    collections::HashSet,
    fmt, fs, io,
    os::unix::fs::DirBuilderExt,
    path::{Path, PathBuf},
};

/// 2 added the pin key column and the rich text table.
const SCHEMA_VERSION: i64 = 2;
/// Formatting is kept per representation up to this size; a copy from a huge document keeps its plain text only.
const MAX_RICH_BYTES: usize = 2 << 20;
/// Letters a pinned item can be pasted with (Command plus the letter). The ones the editing menu uses, and a
/// few more that are easy to hit by accident, are left out.
pub const PIN_KEYS: &str = "bdegijklrsuy";
const MAX_TEXT_BYTES: usize = 1 << 20;
const MAX_IMAGE_BYTES: usize = 25 << 20;
const FOLD_CHARS: usize = 20_000;
const FUZZY_CANDIDATES: i64 = 1500;

const SCHEMA: &str = "
CREATE TABLE IF NOT EXISTS item (
    id INTEGER PRIMARY KEY,
    kind TEXT NOT NULL,
    hash INTEGER NOT NULL,
    text TEXT NOT NULL,
    fold TEXT NOT NULL,
    image TEXT,
    width INTEGER NOT NULL DEFAULT 0,
    height INTEGER NOT NULL DEFAULT 0,
    app_id TEXT,
    app_name TEXT,
    used_at INTEGER NOT NULL,
    uses INTEGER NOT NULL DEFAULT 1,
    pinned INTEGER NOT NULL DEFAULT 0,
    pin_key TEXT
);
CREATE TABLE IF NOT EXISTS rich (
    item_id INTEGER PRIMARY KEY,
    rtf BLOB,
    html BLOB
);
CREATE TRIGGER IF NOT EXISTS item_rich_ad AFTER DELETE ON item BEGIN
    DELETE FROM rich WHERE item_id = old.id;
END;
CREATE UNIQUE INDEX IF NOT EXISTS item_dedupe ON item(kind, hash);
CREATE INDEX IF NOT EXISTS item_recent ON item(pinned DESC, used_at DESC);
CREATE VIRTUAL TABLE IF NOT EXISTS item_fts USING fts5(
    fold, content='item', content_rowid='id', tokenize='trigram'
);
CREATE TRIGGER IF NOT EXISTS item_ai AFTER INSERT ON item BEGIN
    INSERT INTO item_fts(rowid, fold) VALUES (new.id, new.fold);
END;
CREATE TRIGGER IF NOT EXISTS item_ad AFTER DELETE ON item BEGIN
    INSERT INTO item_fts(item_fts, rowid, fold) VALUES ('delete', old.id, old.fold);
END;
";

#[derive(Debug)]
pub enum Error {
    Db(rusqlite::Error),
    Io(io::Error),
}

pub type Result<T> = std::result::Result<T, Error>;

impl From<rusqlite::Error> for Error {
    fn from(e: rusqlite::Error) -> Self {
        Error::Db(e)
    }
}

impl From<io::Error> for Error {
    fn from(e: io::Error) -> Self {
        Error::Io(e)
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::Db(e) => write!(f, "database: {e}"),
            Error::Io(e) => write!(f, "io: {e}"),
        }
    }
}

impl std::error::Error for Error {}

/// What the pasteboard held at capture time.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Payload {
    Text(String),
    Image {
        png: Vec<u8>,
        width: u32,
        height: u32,
    },
    Files(Vec<String>),
}

/// The formatted forms a text copy came with, kept so it can be pasted with its formatting.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Rich {
    pub rtf: Option<Vec<u8>>,
    pub html: Option<Vec<u8>>,
}

impl Rich {
    pub fn is_empty(&self) -> bool {
        self.rtf.is_none() && self.html.is_none()
    }
}

#[derive(Debug, Clone)]
pub struct Capture {
    pub payload: Payload,
    pub app_id: Option<String>,
    pub app_name: Option<String>,
    /// Formatting that came with a text copy; ignored for images and files.
    pub rich: Option<Rich>,
}

/// A history row without its full payload, so listing hundreds of rows stays cheap.
#[derive(Debug, Clone, PartialEq)]
pub struct Item {
    pub id: i64,
    pub kind: Kind,
    pub preview: String,
    pub image: Option<String>,
    pub width: u32,
    pub height: u32,
    pub app_name: Option<String>,
    pub used_at: i64,
    pub pinned: bool,
    /// The letter that pastes this item with Command, while it is pinned.
    pub pin_key: Option<char>,
}

const ITEM_COLUMNS: &str =
    "id, kind, substr(text, 1, 300), image, width, height, app_name, used_at, pinned, pin_key";

fn row_to_item(r: &rusqlite::Row<'_>) -> rusqlite::Result<Item> {
    Ok(Item {
        id: r.get(0)?,
        kind: Kind::parse(&r.get::<_, String>(1)?).unwrap_or(Kind::Text),
        preview: r.get(2)?,
        image: r.get(3)?,
        width: r.get(4)?,
        height: r.get(5)?,
        app_name: r.get(6)?,
        used_at: r.get(7)?,
        pinned: r.get::<_, i64>(8)? != 0,
        pin_key: r
            .get::<_, Option<String>>(9)?
            .and_then(|s| s.chars().next()),
    })
}

/// FNV-1a: stable across runs and Rust versions, which a persisted dedupe key needs.
fn fnv1a(bytes: &[u8]) -> u64 {
    bytes.iter().fold(0xcbf2_9ce4_8422_2325, |h, &b| {
        (h ^ u64::from(b)).wrapping_mul(0x100_0000_01b3)
    })
}

pub struct Store {
    conn: Connection,
    images: PathBuf,
    /// Unpinned items kept; older ones are deleted on insert.
    pub max_items: usize,
}

impl Store {
    pub fn open(dir: &Path) -> Result<Self> {
        let images = dir.join("images");
        // History can hold secrets that bypassed the concealed-type convention: owner-only access.
        fs::DirBuilder::new()
            .recursive(true)
            .mode(0o700)
            .create(&images)?;
        let conn = Connection::open(dir.join("history.sqlite"))?;
        conn.pragma_update(None, "journal_mode", "WAL")?;
        conn.pragma_update(None, "synchronous", "NORMAL")?;
        let version: i64 = conn.pragma_query_value(None, "user_version", |r| r.get(0))?;
        if version < SCHEMA_VERSION {
            conn.execute_batch(SCHEMA)?;
            // A database made by version 1 has the item table without the pin key column.
            let has_pin_key = conn
                .prepare("SELECT 1 FROM pragma_table_info('item') WHERE name = 'pin_key'")?
                .exists([])?;
            if !has_pin_key {
                conn.execute("ALTER TABLE item ADD COLUMN pin_key TEXT", [])?;
            }
            conn.pragma_update(None, "user_version", SCHEMA_VERSION)?;
        }
        let store = Self {
            conn,
            images,
            max_items: 500,
        };
        store.give_pinned_items_keys()?;
        Ok(store)
    }

    /// Pinned items made before pin keys existed get theirs, newest first.
    fn give_pinned_items_keys(&self) -> Result<()> {
        let ids: Vec<i64> = {
            let mut st = self.conn.prepare(
                "SELECT id FROM item WHERE pinned = 1 AND pin_key IS NULL ORDER BY used_at DESC, id DESC",
            )?;
            st.query_map([], |r| r.get(0))?
                .collect::<rusqlite::Result<_>>()?
        };
        for id in ids {
            if let Some(key) = self.free_pin_key()? {
                self.conn.execute(
                    "UPDATE item SET pin_key = ?2 WHERE id = ?1",
                    params![id, key.to_string()],
                )?;
            }
        }
        Ok(())
    }

    /// The first pin letter no pinned item has.
    fn free_pin_key(&self) -> Result<Option<char>> {
        let used: HashSet<char> = {
            let mut st = self
                .conn
                .prepare("SELECT pin_key FROM item WHERE pinned = 1 AND pin_key IS NOT NULL")?;
            st.query_map([], |r| r.get::<_, String>(0))?
                .filter_map(|k| k.ok()?.chars().next())
                .collect()
        };
        Ok(PIN_KEYS.chars().find(|c| !used.contains(c)))
    }

    pub fn image_path(&self, name: &str) -> PathBuf {
        self.images.join(name)
    }

    /// Stores a capture, or moves an identical existing item to the top. `None` means it was skipped.
    pub fn record(&mut self, cap: Capture, now: i64) -> Result<Option<i64>> {
        let (kind, text, hash, image, width, height) = match cap.payload {
            Payload::Text(t) => {
                if t.trim().is_empty() || t.len() > MAX_TEXT_BYTES {
                    return Ok(None);
                }
                (
                    classify_text(&t),
                    t.clone(),
                    fnv1a(t.as_bytes()),
                    None,
                    0,
                    0,
                )
            }
            Payload::Files(paths) => {
                if paths.is_empty() {
                    return Ok(None);
                }
                let t = paths.join("\n");
                (Kind::File, t.clone(), fnv1a(t.as_bytes()), None, 0, 0)
            }
            Payload::Image { png, width, height } => {
                if png.is_empty() || png.len() > MAX_IMAGE_BYTES {
                    return Ok(None);
                }
                let hash = fnv1a(&png);
                let name = format!("{hash:016x}.png");
                let path = self.images.join(&name);
                if !path.exists() {
                    fs::write(path, &png)?;
                }
                (Kind::Image, String::new(), hash, Some(name), width, height)
            }
        };

        let existing: Option<i64> = self
            .conn
            .query_row(
                "SELECT id FROM item WHERE kind = ?1 AND hash = ?2",
                params![kind.as_str(), hash as i64],
                |r| r.get(0),
            )
            .optional()?;
        if let Some(id) = existing {
            self.touch(id, now)?;
            self.keep_rich(id, kind, cap.rich.as_ref())?;
            return Ok(Some(id));
        }

        let folded = fold(&text.chars().take(FOLD_CHARS).collect::<String>());
        self.conn.execute(
            "INSERT INTO item (kind, hash, text, fold, image, width, height, app_id, app_name, used_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
            params![
                kind.as_str(),
                hash as i64,
                text,
                folded,
                image,
                width,
                height,
                cap.app_id,
                cap.app_name,
                now
            ],
        )?;
        let id = self.conn.last_insert_rowid();
        self.keep_rich(id, kind, cap.rich.as_ref())?;
        self.prune()?;
        Ok(Some(id))
    }

    /// Stores the formatting of a text copy, replacing what an earlier copy of the same text had.
    fn keep_rich(&self, id: i64, kind: Kind, rich: Option<&Rich>) -> Result<()> {
        if matches!(kind, Kind::Image | Kind::File) {
            return Ok(());
        }
        let usable = |data: &Option<Vec<u8>>| {
            data.clone()
                .filter(|d| !d.is_empty() && d.len() <= MAX_RICH_BYTES)
        };
        let Some(rich) = rich else {
            return Ok(());
        };
        let (rtf, html) = (usable(&rich.rtf), usable(&rich.html));
        if rtf.is_none() && html.is_none() {
            return Ok(());
        }
        self.conn.execute(
            "INSERT OR REPLACE INTO rich (item_id, rtf, html) VALUES (?1, ?2, ?3)",
            params![id, rtf, html],
        )?;
        Ok(())
    }

    /// The formatting stored with an item, if it had any.
    pub fn rich(&self, id: i64) -> Result<Option<Rich>> {
        Ok(self
            .conn
            .query_row("SELECT rtf, html FROM rich WHERE item_id = ?1", [id], |r| {
                Ok(Rich {
                    rtf: r.get(0)?,
                    html: r.get(1)?,
                })
            })
            .optional()?)
    }

    pub fn touch(&self, id: i64, now: i64) -> Result<()> {
        self.conn.execute(
            "UPDATE item SET used_at = ?2, uses = uses + 1 WHERE id = ?1",
            params![id, now],
        )?;
        Ok(())
    }

    /// Pinning gives the item a free pin letter (it keeps the one it has); unpinning takes it back.
    pub fn set_pinned(&self, id: i64, pinned: bool) -> Result<()> {
        if pinned {
            let key = self.free_pin_key()?.map(|c| c.to_string());
            self.conn.execute(
                "UPDATE item SET pinned = 1, pin_key = COALESCE(pin_key, ?2) WHERE id = ?1",
                params![id, key],
            )?;
        } else {
            self.conn.execute(
                "UPDATE item SET pinned = 0, pin_key = NULL WHERE id = ?1",
                [id],
            )?;
        }
        Ok(())
    }

    pub fn toggle_pinned(&self, id: i64) -> Result<()> {
        let pinned: Option<i64> = self
            .conn
            .query_row("SELECT pinned FROM item WHERE id = ?1", [id], |r| r.get(0))
            .optional()?;
        match pinned {
            Some(flag) => self.set_pinned(id, flag == 0),
            None => Ok(()),
        }
    }

    pub fn delete(&self, id: i64) -> Result<()> {
        let image: Option<Option<String>> = self
            .conn
            .query_row("SELECT image FROM item WHERE id = ?1", [id], |r| r.get(0))
            .optional()?;
        self.conn.execute("DELETE FROM item WHERE id = ?1", [id])?;
        if let Some(Some(name)) = image {
            let _ = fs::remove_file(self.images.join(name));
        }
        Ok(())
    }

    /// Changes how many unpinned items are kept; a lower limit trims the history right away.
    pub fn set_limit(&mut self, limit: usize) -> Result<()> {
        self.max_items = limit;
        self.prune()
    }

    /// Items in the history, pinned or not.
    pub fn count(&self) -> Result<usize> {
        let n: i64 = self
            .conn
            .query_row("SELECT COUNT(*) FROM item", [], |r| r.get(0))?;
        Ok(n.max(0) as usize)
    }

    /// Removes every unpinned item.
    pub fn clear_unpinned(&self) -> Result<()> {
        let ids: Vec<i64> = {
            let mut st = self.conn.prepare("SELECT id FROM item WHERE pinned = 0")?;
            st.query_map([], |r| r.get(0))?
                .collect::<rusqlite::Result<_>>()?
        };
        ids.into_iter().try_for_each(|id| self.delete(id))
    }

    fn prune(&self) -> Result<()> {
        let stale: Vec<i64> = {
            let mut st = self.conn.prepare(
                "SELECT id FROM item WHERE pinned = 0 ORDER BY used_at DESC, id DESC LIMIT -1 OFFSET ?1",
            )?;
            st.query_map([self.max_items as i64], |r| r.get(0))?
                .collect::<rusqlite::Result<_>>()?
        };
        stale.into_iter().try_for_each(|id| self.delete(id))
    }

    /// Full payload for pasting.
    pub fn payload(&self, id: i64) -> Result<Option<Payload>> {
        type Row = (String, String, Option<String>, u32, u32);
        let row: Option<Row> = self
            .conn
            .query_row(
                "SELECT kind, text, image, width, height FROM item WHERE id = ?1",
                [id],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?)),
            )
            .optional()?;
        let Some((kind, text, image, width, height)) = row else {
            return Ok(None);
        };
        Ok(Some(match Kind::parse(&kind) {
            Some(Kind::Image) => {
                let png = fs::read(self.images.join(image.unwrap_or_default()))?;
                Payload::Image { png, width, height }
            }
            Some(Kind::File) => Payload::Files(text.lines().map(str::to_owned).collect()),
            _ => Payload::Text(text),
        }))
    }

    /// Pinned items first, then by recency. Free text matches as a case and accent insensitive
    /// substring (FTS5 trigram index for terms of 3+ characters), then falls back to fuzzy
    /// subsequence matching for the remaining slots.
    pub fn search(&self, raw: &str, limit: usize) -> Result<Vec<Item>> {
        let q = parse_query(raw);
        let mut filters = String::new();
        let mut args: Vec<Value> = Vec::new();
        if let Some(kind) = q.kind {
            filters.push_str(" AND kind = ?");
            args.push(Value::Text(kind.as_str().into()));
        }
        if q.pinned_only {
            filters.push_str(" AND pinned = 1");
        }

        let mut sql = format!("SELECT {ITEM_COLUMNS} FROM item WHERE 1 = 1{filters}");
        let (strong, short): (Vec<&String>, Vec<&String>) =
            q.terms.iter().partition(|t| t.chars().count() >= 3);
        if !strong.is_empty() {
            let expr = strong
                .iter()
                .map(|t| format!("\"{}\"", t.replace('"', "\"\"")))
                .collect::<Vec<_>>()
                .join(" AND ");
            sql.push_str(" AND id IN (SELECT rowid FROM item_fts WHERE item_fts MATCH ?)");
            args.push(Value::Text(expr));
        }
        for term in &short {
            sql.push_str(" AND instr(fold, ?) > 0");
            args.push(Value::Text((*term).clone()));
        }
        sql.push_str(" ORDER BY pinned DESC, used_at DESC LIMIT ?");
        args.push(Value::Integer(limit as i64));

        let mut items: Vec<Item> = self
            .conn
            .prepare(&sql)?
            .query_map(params_from_iter(args), row_to_item)?
            .collect::<rusqlite::Result<_>>()?;

        let needle = q.terms.join("");
        if items.len() < limit && needle.chars().count() >= 3 {
            let found: HashSet<i64> = items.iter().map(|i| i.id).collect();
            let mut scored = self.fuzzy_candidates(&filters, &q, &needle, &found)?;
            scored.sort_by(|a, b| b.0.cmp(&a.0).then(b.1.used_at.cmp(&a.1.used_at)));
            let room = limit - items.len();
            items.extend(scored.into_iter().take(room).map(|(_, item)| item));
        }
        Ok(items)
    }

    fn fuzzy_candidates(
        &self,
        filters: &str,
        q: &crate::Query,
        needle: &str,
        skip: &HashSet<i64>,
    ) -> Result<Vec<(i32, Item)>> {
        let mut args: Vec<Value> = Vec::new();
        if let Some(kind) = q.kind {
            args.push(Value::Text(kind.as_str().into()));
        }
        args.push(Value::Integer(FUZZY_CANDIDATES));
        let sql = format!(
            "SELECT {ITEM_COLUMNS}, substr(fold, 1, 600) FROM item WHERE 1 = 1{filters}
             ORDER BY used_at DESC LIMIT ?"
        );
        let rows: Vec<(Item, String)> = self
            .conn
            .prepare(&sql)?
            .query_map(params_from_iter(args), |r| {
                Ok((row_to_item(r)?, r.get(10)?))
            })?
            .collect::<rusqlite::Result<_>>()?;
        Ok(rows
            .into_iter()
            .filter(|(item, _)| !skip.contains(&item.id))
            .filter_map(|(item, folded)| fuzzy_score(needle, &folded).map(|s| (s, item)))
            .collect())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};

    struct TempDir(PathBuf);

    impl TempDir {
        fn new() -> Self {
            static N: AtomicUsize = AtomicUsize::new(0);
            let dir = std::env::temp_dir().join(format!(
                "figghost-store-test-{}-{}",
                std::process::id(),
                N.fetch_add(1, Ordering::Relaxed)
            ));
            fs::create_dir_all(&dir).unwrap();
            Self(dir)
        }
    }

    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    fn text(s: &str) -> Capture {
        Capture {
            payload: Payload::Text(s.into()),
            app_id: None,
            app_name: Some("Test".into()),
            rich: None,
        }
    }

    fn rich_text(s: &str, rtf: &[u8], html: &[u8]) -> Capture {
        Capture {
            rich: Some(Rich {
                rtf: Some(rtf.to_vec()),
                html: Some(html.to_vec()),
            }),
            ..text(s)
        }
    }

    fn ids(items: &[Item]) -> Vec<i64> {
        items.iter().map(|i| i.id).collect()
    }

    #[test]
    fn identical_copy_moves_to_top_instead_of_duplicating() {
        let dir = TempDir::new();
        let mut s = Store::open(&dir.0).unwrap();
        let a = s.record(text("alpha"), 1).unwrap().unwrap();
        let b = s.record(text("beta"), 2).unwrap().unwrap();
        assert_eq!(ids(&s.search("", 10).unwrap()), [b, a]);
        assert_eq!(s.record(text("alpha"), 3).unwrap(), Some(a));
        assert_eq!(ids(&s.search("", 10).unwrap()), [a, b]);
    }

    #[test]
    fn skips_blank_and_oversized_text() {
        let dir = TempDir::new();
        let mut s = Store::open(&dir.0).unwrap();
        assert_eq!(s.record(text("  \n\t"), 1).unwrap(), None);
        assert_eq!(
            s.record(text(&"x".repeat(MAX_TEXT_BYTES + 1)), 1).unwrap(),
            None
        );
        assert!(s.search("", 10).unwrap().is_empty());
    }

    #[test]
    fn substring_search_ignores_case_and_vietnamese_accents() {
        let dir = TempDir::new();
        let mut s = Store::open(&dir.0).unwrap();
        let name = s.record(text("Nguyễn Văn An"), 1).unwrap().unwrap();
        let other = s.record(text("hello world"), 2).unwrap().unwrap();
        assert_eq!(ids(&s.search("nguyen", 10).unwrap()), [name]);
        assert_eq!(ids(&s.search("NGUYỄN", 10).unwrap()), [name]);
        // Mixed strong (3+ chars) and short terms, in any order.
        assert_eq!(ids(&s.search("an van", 10).unwrap()), [name]);
        assert_eq!(ids(&s.search("wor", 10).unwrap()), [other]);
        assert_eq!(ids(&s.search("he", 10).unwrap()), [other]);
        assert!(s.search("zzzz", 10).unwrap().is_empty());
    }

    #[test]
    fn fuzzy_fallback_matches_subsequences() {
        let dir = TempDir::new();
        let mut s = Store::open(&dir.0).unwrap();
        let id = s.record(text("clipboard manager"), 1).unwrap().unwrap();
        assert_eq!(ids(&s.search("cpbd", 10).unwrap()), [id]);
        // Exact substring hits come before fuzzy ones.
        let exact = s.record(text("cpbd exact"), 2).unwrap().unwrap();
        assert_eq!(ids(&s.search("cpbd", 10).unwrap()), [exact, id]);
        assert!(s.search("dbpc", 10).unwrap().is_empty());
    }

    #[test]
    fn kind_filters_narrow_results() {
        let dir = TempDir::new();
        let mut s = Store::open(&dir.0).unwrap();
        let link = s
            .record(text("https://example.com/docs"), 1)
            .unwrap()
            .unwrap();
        let code = s
            .record(text("fn main() {\n    let x = 1;\n}"), 2)
            .unwrap()
            .unwrap();
        let note = s.record(text("meeting notes example"), 3).unwrap().unwrap();
        let png = Capture {
            payload: Payload::Image {
                png: vec![1, 2, 3, 4],
                width: 2,
                height: 2,
            },
            app_id: None,
            app_name: None,
            rich: None,
        };
        let image = s.record(png, 4).unwrap().unwrap();
        assert_eq!(ids(&s.search("@link", 10).unwrap()), [link]);
        assert_eq!(ids(&s.search("@code", 10).unwrap()), [code]);
        assert_eq!(ids(&s.search("@image", 10).unwrap()), [image]);
        assert_eq!(ids(&s.search("@link example", 10).unwrap()), [link]);
        assert_eq!(ids(&s.search("example", 10).unwrap()), [note, link]);
    }

    #[test]
    fn pinned_items_sort_first_and_survive_pruning() {
        let dir = TempDir::new();
        let mut s = Store::open(&dir.0).unwrap();
        s.max_items = 2;
        let keep = s.record(text("keep me"), 1).unwrap().unwrap();
        s.toggle_pinned(keep).unwrap();
        for (i, t) in ["one", "two", "three", "four"].iter().enumerate() {
            s.record(text(t), 10 + i as i64).unwrap();
        }
        let all = s.search("", 10).unwrap();
        assert_eq!(all.len(), 3, "1 pinned + 2 newest unpinned");
        assert_eq!(all[0].id, keep);
        assert!(all[0].pinned);
        assert_eq!(ids(&s.search("@pin", 10).unwrap()), [keep]);
        s.toggle_pinned(keep).unwrap();
        assert!(s.search("@pin", 10).unwrap().is_empty());
        s.set_pinned(keep, true).unwrap();
        s.clear_unpinned().unwrap();
        assert_eq!(ids(&s.search("", 10).unwrap()), [keep]);
    }

    #[test]
    fn pinned_items_get_a_letter_and_give_it_back() {
        let dir = TempDir::new();
        let mut s = Store::open(&dir.0).unwrap();
        let a = s.record(text("alpha"), 1).unwrap().unwrap();
        let b = s.record(text("beta"), 2).unwrap().unwrap();
        let c = s.record(text("gamma"), 3).unwrap().unwrap();
        let key = |s: &Store, id: i64| {
            s.search("", 10)
                .unwrap()
                .into_iter()
                .find(|i| i.id == id)
                .unwrap()
                .pin_key
        };
        assert_eq!(key(&s, a), None);
        s.toggle_pinned(a).unwrap();
        s.toggle_pinned(b).unwrap();
        let first = PIN_KEYS.chars().next();
        assert_eq!(key(&s, a), first);
        assert_eq!(key(&s, b), PIN_KEYS.chars().nth(1));
        // Pinning again keeps the letter; unpinning frees it for the next item.
        s.set_pinned(a, true).unwrap();
        assert_eq!(key(&s, a), first);
        s.toggle_pinned(a).unwrap();
        assert_eq!(key(&s, a), None);
        s.toggle_pinned(c).unwrap();
        assert_eq!(key(&s, c), first, "the freed letter is reused");
        assert_eq!(
            key(&s, b),
            PIN_KEYS.chars().nth(1),
            "other letters never move"
        );
    }

    #[test]
    fn pin_letters_run_out_gracefully() {
        let dir = TempDir::new();
        let mut s = Store::open(&dir.0).unwrap();
        let total = PIN_KEYS.chars().count() + 2;
        let ids: Vec<i64> = (0..total)
            .map(|i| {
                s.record(text(&format!("item {i}")), i as i64)
                    .unwrap()
                    .unwrap()
            })
            .collect();
        for id in &ids {
            s.set_pinned(*id, true).unwrap();
        }
        let items = s.search("", 100).unwrap();
        assert_eq!(items.iter().filter(|i| i.pinned).count(), total);
        assert_eq!(
            items.iter().filter(|i| i.pin_key.is_some()).count(),
            PIN_KEYS.len()
        );
        let letters: HashSet<char> = items.iter().filter_map(|i| i.pin_key).collect();
        assert_eq!(letters.len(), PIN_KEYS.len(), "no letter is given twice");
    }

    #[test]
    fn formatting_is_kept_replaced_and_removed_with_the_item() {
        let dir = TempDir::new();
        let mut s = Store::open(&dir.0).unwrap();
        let id = s
            .record(rich_text("Hello", b"{\\rtf1 Hello}", b"<b>Hello</b>"), 1)
            .unwrap()
            .unwrap();
        let rich = s.rich(id).unwrap().unwrap();
        assert_eq!(rich.rtf.as_deref(), Some(&b"{\\rtf1 Hello}"[..]));
        assert_eq!(rich.html.as_deref(), Some(&b"<b>Hello</b>"[..]));
        // The same text copied again with other formatting replaces it; copied plain, it keeps what it had.
        let again = s
            .record(rich_text("Hello", b"{\\rtf1 NEW}", b"<i>Hello</i>"), 2)
            .unwrap()
            .unwrap();
        assert_eq!(again, id);
        assert_eq!(
            s.rich(id).unwrap().unwrap().html.as_deref(),
            Some(&b"<i>Hello</i>"[..])
        );
        s.record(text("Hello"), 3).unwrap();
        assert!(s.rich(id).unwrap().is_some());
        // Plain items have no formatting, and deleting an item takes its formatting with it.
        let plain = s.record(text("plain"), 4).unwrap().unwrap();
        assert_eq!(s.rich(plain).unwrap(), None);
        s.delete(id).unwrap();
        assert_eq!(s.rich(id).unwrap(), None);
    }

    #[test]
    fn huge_or_empty_formatting_is_dropped_but_the_text_is_kept() {
        let dir = TempDir::new();
        let mut s = Store::open(&dir.0).unwrap();
        let big = vec![b'x'; MAX_RICH_BYTES + 1];
        let id = s
            .record(rich_text("big copy", &big, b""), 1)
            .unwrap()
            .unwrap();
        assert_eq!(s.rich(id).unwrap(), None);
        assert_eq!(
            s.payload(id).unwrap(),
            Some(Payload::Text("big copy".into()))
        );
        // Images and files never carry formatting.
        let image = Capture {
            payload: Payload::Image {
                png: vec![1, 2, 3],
                width: 1,
                height: 1,
            },
            app_id: None,
            app_name: None,
            rich: Some(Rich {
                rtf: Some(b"x".to_vec()),
                html: None,
            }),
        };
        let image = s.record(image, 2).unwrap().unwrap();
        assert_eq!(s.rich(image).unwrap(), None);
    }

    /// The database the previous version of the app wrote: no pin key column, no formatting table.
    const VERSION_1_SCHEMA: &str = "
        CREATE TABLE item (
            id INTEGER PRIMARY KEY, kind TEXT NOT NULL, hash INTEGER NOT NULL, text TEXT NOT NULL,
            fold TEXT NOT NULL, image TEXT, width INTEGER NOT NULL DEFAULT 0, height INTEGER NOT NULL DEFAULT 0,
            app_id TEXT, app_name TEXT, used_at INTEGER NOT NULL, uses INTEGER NOT NULL DEFAULT 1,
            pinned INTEGER NOT NULL DEFAULT 0
        );
        CREATE UNIQUE INDEX item_dedupe ON item(kind, hash);
        CREATE INDEX item_recent ON item(pinned DESC, used_at DESC);
        CREATE VIRTUAL TABLE item_fts USING fts5(fold, content='item', content_rowid='id', tokenize='trigram');
        CREATE TRIGGER item_ai AFTER INSERT ON item BEGIN INSERT INTO item_fts(rowid, fold) VALUES (new.id, new.fold); END;
        CREATE TRIGGER item_ad AFTER DELETE ON item BEGIN INSERT INTO item_fts(item_fts, rowid, fold) VALUES ('delete', old.id, old.fold); END;
        PRAGMA user_version = 1;
    ";

    #[test]
    fn a_database_from_version_one_is_upgraded_without_losing_anything() {
        let dir = TempDir::new();
        {
            let conn = Connection::open(dir.0.join("history.sqlite")).unwrap();
            conn.execute_batch(VERSION_1_SCHEMA).unwrap();
            for (i, (text, pinned)) in [("old note", 0), ("old pin one", 1), ("old pin two", 1)]
                .iter()
                .enumerate()
            {
                conn.execute(
                    "INSERT INTO item (kind, hash, text, fold, used_at, pinned) VALUES ('text', ?1, ?2, ?3, ?4, ?5)",
                    params![i as i64 + 100, text, text, i as i64, pinned],
                )
                .unwrap();
            }
        }
        let mut s = Store::open(&dir.0).unwrap();
        let items = s.search("", 10).unwrap();
        assert_eq!(items.len(), 3, "every old item is still there");
        let pinned: Vec<&Item> = items.iter().filter(|i| i.pinned).collect();
        assert_eq!(pinned.len(), 2);
        // The old pins got letters, newest first.
        assert_eq!(pinned.iter().filter_map(|i| i.pin_key).count(), 2);
        assert_eq!(
            items
                .iter()
                .find(|i| i.preview == "old pin two")
                .unwrap()
                .pin_key,
            PIN_KEYS.chars().next()
        );
        // Search still works on the old rows, and new features work on top of them.
        assert_eq!(s.search("pin one", 10).unwrap().len(), 1);
        let new = s
            .record(rich_text("fresh", b"rtf", b"html"), 9)
            .unwrap()
            .unwrap();
        assert!(s.rich(new).unwrap().is_some());
        let version: i64 = s
            .conn
            .pragma_query_value(None, "user_version", |r| r.get(0))
            .unwrap();
        assert_eq!(version, SCHEMA_VERSION);
        drop(s);
        // Opening again changes nothing.
        let s = Store::open(&dir.0).unwrap();
        assert_eq!(s.count().unwrap(), 4);
    }

    #[test]
    fn lowering_the_limit_trims_at_once_and_count_follows() {
        let dir = TempDir::new();
        let mut s = Store::open(&dir.0).unwrap();
        for (i, t) in ["a", "b", "c", "d", "e"].iter().enumerate() {
            s.record(text(t), i as i64).unwrap();
        }
        assert_eq!(s.count().unwrap(), 5);
        let pinned = s.record(text("keep"), 9).unwrap().unwrap();
        s.set_pinned(pinned, true).unwrap();
        s.set_limit(2).unwrap();
        assert_eq!(
            s.count().unwrap(),
            3,
            "two newest unpinned items and the pinned one"
        );
        assert_eq!(s.max_items, 2);
        assert!(s.search("", 10).unwrap().iter().any(|i| i.id == pinned));
    }

    #[test]
    fn deleting_an_image_removes_its_file() {
        let dir = TempDir::new();
        let mut s = Store::open(&dir.0).unwrap();
        let cap = Capture {
            payload: Payload::Image {
                png: vec![9; 64],
                width: 8,
                height: 8,
            },
            app_id: None,
            app_name: None,
            rich: None,
        };
        let id = s.record(cap, 1).unwrap().unwrap();
        let item = s.search("", 10).unwrap().remove(0);
        let path = s.image_path(item.image.as_deref().unwrap());
        assert!(path.exists());
        assert!(matches!(
            s.payload(id).unwrap(),
            Some(Payload::Image { .. })
        ));
        s.delete(id).unwrap();
        assert!(!path.exists());
        assert!(s.search("", 10).unwrap().is_empty());
    }

    #[test]
    fn payload_round_trips_text_and_files() {
        let dir = TempDir::new();
        let mut s = Store::open(&dir.0).unwrap();
        let t = s.record(text("héllo\nworld"), 1).unwrap().unwrap();
        let files = Capture {
            payload: Payload::Files(vec!["/tmp/a.txt".into(), "/tmp/b b.txt".into()]),
            app_id: None,
            app_name: None,
            rich: None,
        };
        let f = s.record(files, 2).unwrap().unwrap();
        assert_eq!(
            s.payload(t).unwrap(),
            Some(Payload::Text("héllo\nworld".into()))
        );
        assert_eq!(
            s.payload(f).unwrap(),
            Some(Payload::Files(vec![
                "/tmp/a.txt".into(),
                "/tmp/b b.txt".into()
            ]))
        );
        assert_eq!(s.payload(9999).unwrap(), None);
    }

    #[test]
    fn reopening_keeps_history() {
        let dir = TempDir::new();
        let id = Store::open(&dir.0)
            .unwrap()
            .record(text("persist"), 1)
            .unwrap()
            .unwrap();
        let s = Store::open(&dir.0).unwrap();
        assert_eq!(ids(&s.search("persist", 10).unwrap()), [id]);
    }
}
