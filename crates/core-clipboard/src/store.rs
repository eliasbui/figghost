use crate::{
    Kind, classify_text,
    search::{fold, fuzzy_score, parse_query},
    sensitive,
};
use rusqlite::{Connection, OptionalExtension, params, params_from_iter, types::Value};
use std::{
    collections::HashSet,
    fmt, fs, io,
    os::unix::fs::DirBuilderExt,
    path::{Path, PathBuf},
};

/// 2 added the pin key column and the rich text table. 3 added boards and sensitive items.
const SCHEMA_VERSION: i64 = 3;
/// The copy of a database made before it is moved to version 3, so an upgrade can always be undone by hand.
const BACKUP_NAME: &str = "history.sqlite.v2.bak";
/// Boards that pinned items can be filed in.
pub const MAX_BOARDS: usize = 8;
pub const MAX_BOARD_NAME_CHARS: usize = 24;
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
    pin_key TEXT,
    board INTEGER,
    masked INTEGER NOT NULL DEFAULT 0,
    expires_at INTEGER
);
CREATE TABLE IF NOT EXISTS board (
    id INTEGER PRIMARY KEY,
    name TEXT NOT NULL,
    token TEXT NOT NULL UNIQUE,
    position INTEGER NOT NULL
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

/// Columns version 3 added to `item`, with the indexes that need them.
const V3_COLUMNS: [(&str, &str); 3] = [
    ("board", "INTEGER"),
    ("masked", "INTEGER NOT NULL DEFAULT 0"),
    ("expires_at", "INTEGER"),
];
const V3_INDEXES: &str = "
CREATE INDEX IF NOT EXISTS item_board ON item(board) WHERE board IS NOT NULL;
CREATE INDEX IF NOT EXISTS item_expiry ON item(expires_at) WHERE expires_at IS NOT NULL;
";

/// Why a board change was refused.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BoardError {
    Empty,
    TooLong,
    /// Control characters in the name.
    BadChars,
    /// The name would be read as a filter (`@link`, or a start of one).
    Reserved,
    Taken,
    Full,
    Missing,
}

impl BoardError {
    /// A stable code for the settings page.
    pub const fn code(self) -> &'static str {
        match self {
            BoardError::Empty => "board_empty",
            BoardError::TooLong => "board_too_long",
            BoardError::BadChars => "board_bad_chars",
            BoardError::Reserved => "board_reserved",
            BoardError::Taken => "board_taken",
            BoardError::Full => "board_full",
            BoardError::Missing => "board_missing",
        }
    }
}

#[derive(Debug)]
pub enum Error {
    Db(rusqlite::Error),
    Io(io::Error),
    Board(BoardError),
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
            Error::Board(e) => write!(f, "board: {}", e.code()),
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

/// What to do with a text copy that looks like a secret.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Sensitivity {
    /// Store it like any other copy.
    Keep,
    /// Store it hidden (not searchable, shown as dots) and delete it after this many minutes.
    Mask { minutes: u32 },
    /// Do not store it at all.
    Skip,
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
    /// The board a pinned item is filed in.
    pub board: Option<i64>,
    /// A likely secret: its preview is empty and it is not in the search index.
    pub masked: bool,
}

/// A named group of pinned items.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Board {
    pub id: i64,
    pub name: String,
    /// What follows `@` in a search to see only this board's items.
    pub token: String,
    pub items: usize,
}

/// A masked item never gives its text to a list or a search result.
const ITEM_COLUMNS: &str = "id, kind, CASE WHEN masked = 1 THEN '' ELSE substr(text, 1, 300) END, \
     image, width, height, app_name, used_at, pinned, pin_key, board, masked";

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
        board: r.get(10)?,
        masked: r.get::<_, i64>(11)? != 0,
    })
}

/// What a name is searched by: folded, with its spaces turned into underscores so it is one word.
fn board_token(name: &str) -> String {
    fold(name.trim())
        .split_whitespace()
        .collect::<Vec<_>>()
        .join("_")
}

/// Control characters and the invisible ones (zero width, line and paragraph separators, byte order mark),
/// which would make a name that cannot be told from another.
fn is_unusable(c: char) -> bool {
    c.is_control()
        || matches!(
            c,
            '\u{200b}'..='\u{200f}' | '\u{2028}' | '\u{2029}' | '\u{2060}' | '\u{feff}'
        )
}

fn check_board_name(name: &str) -> std::result::Result<(String, String), BoardError> {
    if name.chars().any(is_unusable) {
        return Err(BoardError::BadChars);
    }
    // Runs of spaces count as one, so "A  B" and "A B" are the same name.
    let name = name.split_whitespace().collect::<Vec<_>>().join(" ");
    if name.is_empty() {
        return Err(BoardError::Empty);
    }
    if name.chars().count() > MAX_BOARD_NAME_CHARS {
        return Err(BoardError::TooLong);
    }
    let token = board_token(&name);
    // A name the query parser reads as a filter (or a start of one) could never be searched for.
    let probe = parse_query(&format!("@{token}"));
    if probe.kind.is_some() || probe.pinned_only || probe.terms != [format!("@{token}")] {
        return Err(BoardError::Reserved);
    }
    Ok((name, token))
}

fn copy_private(from: &Path, to: &Path) -> io::Result<()> {
    use std::os::unix::fs::OpenOptionsExt;
    let mut source = fs::File::open(from)?;
    let mut target = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(to)?;
    io::copy(&mut source, &mut target)?;
    target.sync_all()
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
        // Before anything touches the file, not even the journal mode.
        Self::back_up_before_upgrade(&conn, dir)?;
        conn.pragma_update(None, "journal_mode", "WAL")?;
        conn.pragma_update(None, "synchronous", "NORMAL")?;
        Self::migrate(&conn)?;
        let store = Self {
            conn,
            images,
            max_items: 500,
        };
        store.give_pinned_items_keys()?;
        Ok(store)
    }

    /// Copies an existing database once to `BACKUP_NAME` before it is moved to the current version. A copy
    /// that is already there is the older one and is kept.
    fn back_up_before_upgrade(conn: &Connection, dir: &Path) -> Result<()> {
        let version: i64 = conn.pragma_query_value(None, "user_version", |r| r.get(0))?;
        let existing = conn
            .prepare("SELECT 1 FROM sqlite_master WHERE type = 'table' AND name = 'item'")?
            .exists([])?;
        let backup = dir.join(BACKUP_NAME);
        if version < SCHEMA_VERSION && existing && !backup.exists() {
            // A file that was in write-ahead mode keeps recent rows in its log; fold them in before copying.
            conn.query_row("PRAGMA wal_checkpoint(TRUNCATE)", [], |_| Ok(()))?;
            copy_private(&dir.join("history.sqlite"), &backup)?;
        }
        Ok(())
    }

    /// Brings the database to the current version, all in one transaction, so a failure leaves it as it was.
    fn migrate(conn: &Connection) -> Result<()> {
        let version: i64 = conn.pragma_query_value(None, "user_version", |r| r.get(0))?;
        if version >= SCHEMA_VERSION {
            return Ok(());
        }
        let tx = conn.unchecked_transaction()?;
        tx.execute_batch(SCHEMA)?;
        let has_column = |name: &str| -> Result<bool> {
            Ok(tx
                .prepare("SELECT 1 FROM pragma_table_info('item') WHERE name = ?1")?
                .exists([name])?)
        };
        // A database made by version 1 has the item table without the pin key column.
        if !has_column("pin_key")? {
            tx.execute("ALTER TABLE item ADD COLUMN pin_key TEXT", [])?;
        }
        for (name, definition) in V3_COLUMNS {
            if !has_column(name)? {
                tx.execute(
                    &format!("ALTER TABLE item ADD COLUMN {name} {definition}"),
                    [],
                )?;
            }
        }
        tx.execute_batch(V3_INDEXES)?;
        tx.pragma_update(None, "user_version", SCHEMA_VERSION)?;
        tx.commit()?;
        Ok(())
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
        self.record_with(cap, now, Sensitivity::Keep)
    }

    /// `record`, with a policy for text that looks like a secret.
    pub fn record_with(
        &mut self,
        cap: Capture,
        now: i64,
        sensitivity: Sensitivity,
    ) -> Result<Option<i64>> {
        // The moment a masked item is deleted, or None for an item that stays.
        let mut expires_at = None;
        let (kind, text, hash, image, width, height) = match cap.payload {
            Payload::Text(t) => {
                if t.trim().is_empty() || t.len() > MAX_TEXT_BYTES {
                    return Ok(None);
                }
                if sensitivity != Sensitivity::Keep && sensitive::detect(&t).is_some() {
                    match sensitivity {
                        Sensitivity::Skip => return Ok(None),
                        Sensitivity::Mask { minutes } => {
                            expires_at = Some(now + i64::from(minutes) * 60_000);
                        }
                        Sensitivity::Keep => {}
                    }
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
            match expires_at {
                Some(at) => self.mask(id, at)?,
                None => self.keep_rich(id, kind, cap.rich.as_ref())?,
            }
            return Ok(Some(id));
        }

        let masked = expires_at.is_some();
        // A masked item is left out of the search index, so no query can find out what it holds.
        let folded = if masked {
            String::new()
        } else {
            fold(&text.chars().take(FOLD_CHARS).collect::<String>())
        };
        self.conn.execute(
            "INSERT INTO item (kind, hash, text, fold, image, width, height, app_id, app_name, used_at, masked, expires_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)",
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
                now,
                masked,
                expires_at
            ],
        )?;
        let id = self.conn.last_insert_rowid();
        if !masked {
            self.keep_rich(id, kind, cap.rich.as_ref())?;
        }
        self.prune()?;
        Ok(Some(id))
    }

    /// Hides an item that was stored in the clear and sets when it goes away. A pinned item is the user's
    /// choice to keep, so it is left alone.
    fn mask(&self, id: i64, expires_at: i64) -> Result<()> {
        let row: Option<(i64, i64, String)> = self
            .conn
            .query_row(
                "SELECT pinned, masked, fold FROM item WHERE id = ?1",
                [id],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
            )
            .optional()?;
        let Some((pinned, masked, fold)) = row else {
            return Ok(());
        };
        if pinned != 0 {
            return Ok(());
        }
        if masked == 0 {
            // The index entry is removed with the text it was made from, then an empty one is put in.
            self.conn.execute(
                "INSERT INTO item_fts(item_fts, rowid, fold) VALUES ('delete', ?1, ?2)",
                params![id, fold],
            )?;
            self.conn
                .execute("UPDATE item SET masked = 1, fold = '' WHERE id = ?1", [id])?;
            self.conn
                .execute("INSERT INTO item_fts(rowid, fold) VALUES (?1, '')", [id])?;
            self.conn
                .execute("DELETE FROM rich WHERE item_id = ?1", [id])?;
        }
        self.conn.execute(
            "UPDATE item SET expires_at = ?2 WHERE id = ?1",
            params![id, expires_at],
        )?;
        Ok(())
    }

    /// Whether an item is a masked one.
    pub fn is_masked(&self, id: i64) -> Result<bool> {
        let masked: Option<i64> = self
            .conn
            .query_row("SELECT masked FROM item WHERE id = ?1", [id], |r| r.get(0))
            .optional()?;
        Ok(masked.is_some_and(|m| m != 0))
    }

    /// Deletes masked items whose time has come; returns how many went.
    pub fn purge_expired(&self, now: i64) -> Result<usize> {
        let ids: Vec<i64> = {
            let mut st = self.conn.prepare(
                "SELECT id FROM item WHERE pinned = 0 AND expires_at IS NOT NULL AND expires_at <= ?1",
            )?;
            st.query_map([now], |r| r.get(0))?
                .collect::<rusqlite::Result<_>>()?
        };
        let count = ids.len();
        ids.into_iter().try_for_each(|id| self.delete(id))?;
        Ok(count)
    }

    /// When the next masked item is due to go, so a caller can sleep until then.
    pub fn next_expiry(&self) -> Result<Option<i64>> {
        Ok(self.conn.query_row(
            "SELECT MIN(expires_at) FROM item WHERE pinned = 0 AND expires_at IS NOT NULL",
            [],
            |r| r.get(0),
        )?)
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
                "UPDATE item SET pinned = 1, pin_key = COALESCE(pin_key, ?2), expires_at = NULL WHERE id = ?1",
                params![id, key],
            )?;
        } else {
            self.conn.execute(
                "UPDATE item SET pinned = 0, pin_key = NULL, board = NULL WHERE id = ?1",
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

    /// The boards in the order they were made.
    pub fn boards(&self) -> Result<Vec<Board>> {
        let mut st = self.conn.prepare(
            "SELECT b.id, b.name, b.token, COUNT(i.id) FROM board b
             LEFT JOIN item i ON i.board = b.id
             GROUP BY b.id ORDER BY b.position, b.id",
        )?;
        Ok(st
            .query_map([], |r| {
                Ok(Board {
                    id: r.get(0)?,
                    name: r.get(1)?,
                    token: r.get(2)?,
                    items: r.get::<_, i64>(3)?.max(0) as usize,
                })
            })?
            .collect::<rusqlite::Result<_>>()?)
    }

    fn token_taken(&self, token: &str, except: Option<i64>) -> Result<bool> {
        Ok(self
            .conn
            .prepare("SELECT 1 FROM board WHERE token = ?1 AND id IS NOT ?2")?
            .exists(params![token, except])?)
    }

    pub fn add_board(&self, name: &str) -> Result<i64> {
        let (name, token) = check_board_name(name).map_err(Error::Board)?;
        let count: i64 = self
            .conn
            .query_row("SELECT COUNT(*) FROM board", [], |r| r.get(0))?;
        if count as usize >= MAX_BOARDS {
            return Err(Error::Board(BoardError::Full));
        }
        if self.token_taken(&token, None)? {
            return Err(Error::Board(BoardError::Taken));
        }
        self.conn.execute(
            "INSERT INTO board (name, token, position)
             VALUES (?1, ?2, COALESCE((SELECT MAX(position) + 1 FROM board), 0))",
            params![name, token],
        )?;
        Ok(self.conn.last_insert_rowid())
    }

    pub fn rename_board(&self, id: i64, name: &str) -> Result<()> {
        let (name, token) = check_board_name(name).map_err(Error::Board)?;
        if self.token_taken(&token, Some(id))? {
            return Err(Error::Board(BoardError::Taken));
        }
        let changed = self.conn.execute(
            "UPDATE board SET name = ?2, token = ?3 WHERE id = ?1",
            params![id, name, token],
        )?;
        if changed == 0 {
            return Err(Error::Board(BoardError::Missing));
        }
        Ok(())
    }

    /// Removes a board. Its items stay pinned, without a board.
    pub fn delete_board(&self, id: i64) -> Result<()> {
        let tx = self.conn.unchecked_transaction()?;
        tx.execute("UPDATE item SET board = NULL WHERE board = ?1", [id])?;
        tx.execute("DELETE FROM board WHERE id = ?1", [id])?;
        tx.commit()?;
        Ok(())
    }

    /// Files an item in a board (pinning it, since boards hold pinned items) or takes it out of its board,
    /// leaving it pinned.
    pub fn set_board(&self, item: i64, board: Option<i64>) -> Result<()> {
        if let Some(board) = board {
            let exists = self
                .conn
                .prepare("SELECT 1 FROM board WHERE id = ?1")?
                .exists([board])?;
            if !exists {
                return Err(Error::Board(BoardError::Missing));
            }
            self.set_pinned(item, true)?;
        }
        self.conn.execute(
            "UPDATE item SET board = ?2 WHERE id = ?1",
            params![item, board],
        )?;
        Ok(())
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
        let mut q = parse_query(raw);
        let mut filters = String::new();
        let mut args: Vec<Value> = Vec::new();
        if let Some(kind) = q.kind {
            filters.push_str(" AND kind = ?");
            args.push(Value::Text(kind.as_str().into()));
        }
        if q.pinned_only {
            filters.push_str(" AND pinned = 1");
        }
        if q.terms.iter().any(|t| t.starts_with('@')) {
            // A word that names a board is that board's filter; any other `@word` stays a search term.
            for board in self.boards()? {
                let word = format!("@{}", board.token);
                if let Some(at) = q.terms.iter().position(|t| *t == word) {
                    q.terms.remove(at);
                    filters.push_str(" AND board = ?");
                    args.push(Value::Integer(board.id));
                    break;
                }
            }
        }

        let filter_args = args.clone();
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
        // The id breaks ties, so items used in the same millisecond keep a stable order, newest first.
        sql.push_str(" ORDER BY pinned DESC, used_at DESC, id DESC LIMIT ?");
        args.push(Value::Integer(limit as i64));

        let mut items: Vec<Item> = self
            .conn
            .prepare(&sql)?
            .query_map(params_from_iter(args), row_to_item)?
            .collect::<rusqlite::Result<_>>()?;

        let needle = q.terms.join("");
        if items.len() < limit && needle.chars().count() >= 3 {
            let found: HashSet<i64> = items.iter().map(|i| i.id).collect();
            let mut scored = self.fuzzy_candidates(&filters, &filter_args, &needle, &found)?;
            scored.sort_by(|a, b| b.0.cmp(&a.0).then(b.1.used_at.cmp(&a.1.used_at)));
            let room = limit - items.len();
            items.extend(scored.into_iter().take(room).map(|(_, item)| item));
        }
        Ok(items)
    }

    fn fuzzy_candidates(
        &self,
        filters: &str,
        filter_args: &[Value],
        needle: &str,
        skip: &HashSet<i64>,
    ) -> Result<Vec<(i32, Item)>> {
        let mut args = filter_args.to_vec();
        args.push(Value::Integer(FUZZY_CANDIDATES));
        let sql = format!(
            "SELECT {ITEM_COLUMNS}, substr(fold, 1, 600) FROM item WHERE 1 = 1{filters}
             ORDER BY used_at DESC, id DESC LIMIT ?"
        );
        let rows: Vec<(Item, String)> = self
            .conn
            .prepare(&sql)?
            .query_map(params_from_iter(args), |r| {
                Ok((row_to_item(r)?, r.get(12)?))
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
    fn items_used_in_the_same_millisecond_list_newest_first() {
        let dir = TempDir::new();
        let mut s = Store::open(&dir.0).unwrap();
        let first = s.record(text("first"), 5).unwrap().unwrap();
        let second = s.record(text("second"), 5).unwrap().unwrap();
        let third = s.record(text("third"), 5).unwrap().unwrap();
        assert_eq!(ids(&s.search("", 10).unwrap()), [third, second, first]);
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

    /// The database of version 2, as the app wrote it before boards: pin keys and formatting, no board columns.
    const VERSION_2_SCHEMA: &str = "
        CREATE TABLE item (
            id INTEGER PRIMARY KEY, kind TEXT NOT NULL, hash INTEGER NOT NULL, text TEXT NOT NULL,
            fold TEXT NOT NULL, image TEXT, width INTEGER NOT NULL DEFAULT 0, height INTEGER NOT NULL DEFAULT 0,
            app_id TEXT, app_name TEXT, used_at INTEGER NOT NULL, uses INTEGER NOT NULL DEFAULT 1,
            pinned INTEGER NOT NULL DEFAULT 0, pin_key TEXT
        );
        CREATE TABLE rich (item_id INTEGER PRIMARY KEY, rtf BLOB, html BLOB);
        CREATE TRIGGER item_rich_ad AFTER DELETE ON item BEGIN DELETE FROM rich WHERE item_id = old.id; END;
        CREATE UNIQUE INDEX item_dedupe ON item(kind, hash);
        CREATE INDEX item_recent ON item(pinned DESC, used_at DESC);
        CREATE VIRTUAL TABLE item_fts USING fts5(fold, content='item', content_rowid='id', tokenize='trigram');
        CREATE TRIGGER item_ai AFTER INSERT ON item BEGIN INSERT INTO item_fts(rowid, fold) VALUES (new.id, new.fold); END;
        CREATE TRIGGER item_ad AFTER DELETE ON item BEGIN INSERT INTO item_fts(item_fts, rowid, fold) VALUES ('delete', old.id, old.fold); END;
        PRAGMA user_version = 2;
    ";

    fn user_version(s: &Store) -> i64 {
        s.conn
            .pragma_query_value(None, "user_version", |r| r.get(0))
            .unwrap()
    }

    /// A version 2 file with a pinned item that has a letter and formatting, a plain one and an image.
    fn write_version_2(dir: &Path) {
        let conn = Connection::open(dir.join("history.sqlite")).unwrap();
        conn.execute_batch(VERSION_2_SCHEMA).unwrap();
        for (i, (text, pinned, key)) in [
            ("Nguyễn Văn An", 1, Some("b")),
            ("plain note about boards", 0, None),
            ("another pinned thing", 1, Some("d")),
        ]
        .iter()
        .enumerate()
        {
            conn.execute(
                "INSERT INTO item (kind, hash, text, fold, used_at, pinned, pin_key) VALUES ('text', ?1, ?2, ?3, ?4, ?5, ?6)",
                params![i as i64 + 7, text, crate::fold(text), i as i64 * 10, pinned, key],
            )
            .unwrap();
        }
        conn.execute(
            "INSERT INTO item (kind, hash, text, fold, image, width, height, used_at) VALUES ('image', 99, '', '', 'abc.png', 4, 3, 50)",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO rich (item_id, rtf, html) VALUES (1, x'7b72746631', x'3c623e')",
            [],
        )
        .unwrap();
    }

    #[test]
    fn a_database_from_version_two_is_upgraded_without_losing_anything() {
        let dir = TempDir::new();
        write_version_2(&dir.0);
        let before = fs::read(dir.0.join("history.sqlite")).unwrap();
        let s = Store::open(&dir.0).unwrap();
        assert_eq!(user_version(&s), 3);
        let items = s.search("", 10).unwrap();
        assert_eq!(items.len(), 4);
        let by = |text: &str| items.iter().find(|i| i.preview == text).unwrap().clone();
        let name = by("Nguyễn Văn An");
        assert!(
            name.pinned && name.pin_key == Some('b'),
            "pin letters survive"
        );
        assert_eq!(by("another pinned thing").pin_key, Some('d'));
        assert!(items.iter().all(|i| i.board.is_none() && !i.masked));
        let rich = s.rich(name.id).unwrap().unwrap();
        assert_eq!(rich.rtf.as_deref(), Some(&b"{rtf1"[..]));
        assert_eq!(rich.html.as_deref(), Some(&b"<b>"[..]));
        assert_eq!(items.iter().filter(|i| i.kind == Kind::Image).count(), 1);
        // The index still finds old rows, with and without accents.
        assert_eq!(ids(&s.search("nguyen", 10).unwrap()), [name.id]);
        assert_eq!(s.search("about boards", 10).unwrap().len(), 1);
        // New things work on top of old rows, and a letter taken by an old pin is not given again.
        let board = s.add_board("Work").unwrap();
        s.set_board(by("plain note about boards").id, Some(board))
            .unwrap();
        let filed = s.search("@work", 10).unwrap();
        assert_eq!(filed.len(), 1);
        assert_eq!(
            filed[0].pin_key,
            Some('e'),
            "b and d are taken by the old pins, so the next letter is e"
        );
        // The old file is kept once, exactly as it was.
        let backup = dir.0.join(BACKUP_NAME);
        assert_eq!(fs::read(&backup).unwrap(), before);
        drop(s);
        let old = Connection::open(&backup).unwrap();
        let version: i64 = old
            .pragma_query_value(None, "user_version", |r| r.get(0))
            .unwrap();
        assert_eq!(version, 2);
        let columns: i64 = old
            .query_row(
                "SELECT COUNT(*) FROM pragma_table_info('item') WHERE name IN ('board', 'masked')",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(columns, 0, "the copy is the version 2 file");
    }

    #[test]
    fn the_backup_is_written_once_and_kept_private() {
        use std::os::unix::fs::PermissionsExt;
        let dir = TempDir::new();
        write_version_2(&dir.0);
        let backup = dir.0.join(BACKUP_NAME);
        assert!(!backup.exists());
        let mut s = Store::open(&dir.0).unwrap();
        let first = fs::read(&backup).unwrap();
        assert_eq!(
            fs::metadata(&backup).unwrap().permissions().mode() & 0o777,
            0o600
        );
        s.record(text("new after the upgrade"), 1000).unwrap();
        drop(s);
        // Opening a version 3 file again changes nothing about the copy.
        let s = Store::open(&dir.0).unwrap();
        assert_eq!(fs::read(&backup).unwrap(), first);
        assert_eq!(s.count().unwrap(), 5);
        drop(s);
        // A copy that exists is never replaced, even if the main file is somehow back at version 2.
        fs::write(&backup, b"older copy").unwrap();
        let conn = Connection::open(dir.0.join("history.sqlite")).unwrap();
        conn.pragma_update(None, "user_version", 2).unwrap();
        drop(conn);
        Store::open(&dir.0).unwrap();
        assert_eq!(fs::read(&backup).unwrap(), b"older copy");
    }

    #[test]
    fn a_fresh_database_needs_no_backup_and_version_one_gets_one() {
        let dir = TempDir::new();
        let s = Store::open(&dir.0).unwrap();
        assert_eq!(user_version(&s), 3);
        assert!(!dir.0.join(BACKUP_NAME).exists());
        drop(s);

        let old = TempDir::new();
        {
            let conn = Connection::open(old.0.join("history.sqlite")).unwrap();
            conn.execute_batch(VERSION_1_SCHEMA).unwrap();
            conn.execute(
                "INSERT INTO item (kind, hash, text, fold, used_at, pinned) VALUES ('text', 1, 'v1 pin', 'v1 pin', 5, 1)",
                [],
            )
            .unwrap();
        }
        let s = Store::open(&old.0).unwrap();
        assert!(old.0.join(BACKUP_NAME).exists());
        let items = s.search("", 10).unwrap();
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].pin_key, PIN_KEYS.chars().next());
        assert_eq!(user_version(&s), 3);
        assert!(s.boards().unwrap().is_empty());
    }

    #[test]
    fn a_board_holds_pinned_items_and_is_searched_by_its_name() {
        let dir = TempDir::new();
        let mut s = Store::open(&dir.0).unwrap();
        let a = s
            .record(text("quarterly report draft"), 1)
            .unwrap()
            .unwrap();
        let b = s
            .record(text("snippet: sign off politely"), 2)
            .unwrap()
            .unwrap();
        let c = s
            .record(text("report from the dentist"), 3)
            .unwrap()
            .unwrap();
        let work = s.add_board("Work").unwrap();
        let snippets = s.add_board("  Snippets  ").unwrap();
        assert_eq!(s.boards().unwrap().len(), 2);

        // Filing pins the item; the board and the letter come together.
        s.set_board(a, Some(work)).unwrap();
        s.set_board(b, Some(snippets)).unwrap();
        let all = s.search("", 10).unwrap();
        let item = |id: i64| all.iter().find(|i| i.id == id).unwrap().clone();
        assert!(item(a).pinned && item(a).board == Some(work) && item(a).pin_key.is_some());
        assert_eq!(item(c).board, None);

        assert_eq!(ids(&s.search("@work", 10).unwrap()), [a]);
        assert_eq!(ids(&s.search("@WORK", 10).unwrap()), [a]);
        assert_eq!(ids(&s.search("@snippets", 10).unwrap()), [b]);
        // The board combines with words, with fuzzy matching and with the other filters.
        assert_eq!(ids(&s.search("@work report", 10).unwrap()), [a]);
        assert_eq!(ids(&s.search("report @work", 10).unwrap()), [a]);
        assert_eq!(ids(&s.search("@work qrtrly", 10).unwrap()), [a]);
        assert!(s.search("@work dentist", 10).unwrap().is_empty());
        assert_eq!(ids(&s.search("@work @pin", 10).unwrap()), [a]);
        assert_eq!(ids(&s.search("@work @text", 10).unwrap()), [a]);
        assert!(s.search("@work @image", 10).unwrap().is_empty());
        // Without the board, the words search everything as before.
        assert_eq!(
            ids(&s.search("report", 10).unwrap()),
            [a, c],
            "pinned first"
        );
        // A word that is not a board stays a search term.
        assert!(s.search("@nowhere", 10).unwrap().is_empty());

        let counts: Vec<(String, usize)> = s
            .boards()
            .unwrap()
            .into_iter()
            .map(|b| (b.name, b.items))
            .collect();
        assert_eq!(counts, [("Work".into(), 1), ("Snippets".into(), 1)]);

        // Moving between boards and out of one; the item stays pinned.
        s.set_board(a, Some(snippets)).unwrap();
        assert_eq!(ids(&s.search("@snippets", 10).unwrap()).len(), 2);
        s.set_board(a, None).unwrap();
        let after = s.search("@pin", 10).unwrap();
        assert!(
            after
                .iter()
                .any(|i| i.id == a && i.pinned && i.board.is_none())
        );
        // Unpinning takes the item out of its board.
        s.set_board(b, Some(work)).unwrap();
        s.set_pinned(b, false).unwrap();
        assert_eq!(s.search("@work", 10).unwrap().len(), 0);
    }

    #[test]
    fn deleting_a_board_unpins_nothing() {
        let dir = TempDir::new();
        let mut s = Store::open(&dir.0).unwrap();
        let a = s.record(text("one"), 1).unwrap().unwrap();
        let b = s.record(text("two"), 2).unwrap().unwrap();
        let keep = s.add_board("Keep").unwrap();
        let other = s.add_board("Other").unwrap();
        s.set_board(a, Some(keep)).unwrap();
        s.set_board(b, Some(other)).unwrap();
        let letter = s.search("@keep", 10).unwrap()[0].pin_key;
        s.delete_board(keep).unwrap();
        let all = s.search("", 10).unwrap();
        let first = all.iter().find(|i| i.id == a).unwrap();
        assert!(first.pinned && first.board.is_none() && first.pin_key == letter);
        assert_eq!(all.iter().find(|i| i.id == b).unwrap().board, Some(other));
        assert_eq!(s.boards().unwrap().len(), 1);
        // The name can be used again, and a missing board is reported.
        assert!(s.add_board("Keep").is_ok());
        assert!(matches!(
            s.set_board(a, Some(keep)),
            Err(Error::Board(BoardError::Missing))
        ));
        s.delete_board(9999).unwrap();
    }

    #[test]
    fn board_names_are_checked() {
        let dir = TempDir::new();
        let s = Store::open(&dir.0).unwrap();
        let refused = |s: &Store, name: &str| match s.add_board(name) {
            Err(Error::Board(e)) => e,
            other => panic!("{name:?} gave {other:?}"),
        };
        assert_eq!(refused(&s, ""), BoardError::Empty);
        assert_eq!(refused(&s, "   "), BoardError::Empty);
        assert_eq!(
            refused(&s, &"x".repeat(MAX_BOARD_NAME_CHARS + 1)),
            BoardError::TooLong
        );
        assert_eq!(refused(&s, "tab\there"), BoardError::BadChars);
        assert_eq!(refused(&s, "line\nbreak"), BoardError::BadChars);
        assert_eq!(
            refused(&s, "\u{200b}"),
            BoardError::BadChars,
            "an invisible name"
        );
        assert_eq!(refused(&s, "wo\u{200b}rk"), BoardError::BadChars);
        // Names that the search would read as a filter, or as the start of one, could never be searched.
        for name in [
            "link", "Pin", "pinned", "code", "cod", "img", "File", "t", "p",
        ] {
            assert_eq!(refused(&s, name), BoardError::Reserved, "{name}");
        }
        assert!(s.add_board(&"x".repeat(MAX_BOARD_NAME_CHARS)).is_ok());
        let first = s.add_board("Công việc").unwrap();
        assert_eq!(refused(&s, "cong viec"), BoardError::Taken);
        assert_eq!(refused(&s, "CÔNG  VIỆC"), BoardError::Taken);
        assert_eq!(s.boards().unwrap()[1].token, "cong_viec");
        s.rename_board(first, "  Công   việc ").unwrap();
        assert_eq!(
            s.boards().unwrap()[1].name,
            "Công việc",
            "inner spaces are collapsed"
        );
        // Renaming to itself in another case is fine; to another board's name is not.
        s.rename_board(first, "CÔNG VIỆC").unwrap();
        let second = s.add_board("Home").unwrap();
        assert!(matches!(
            s.rename_board(second, "công việc"),
            Err(Error::Board(BoardError::Taken))
        ));
        assert!(matches!(
            s.rename_board(second, "link"),
            Err(Error::Board(BoardError::Reserved))
        ));
        assert!(matches!(
            s.rename_board(4242, "Fresh"),
            Err(Error::Board(BoardError::Missing))
        ));
        assert_eq!(s.boards().unwrap().len(), 3);
        // There is a limit.
        for i in s.boards().unwrap().len()..MAX_BOARDS {
            s.add_board(&format!("board {i}")).unwrap();
        }
        assert_eq!(refused(&s, "one more"), BoardError::Full);
        assert_eq!(s.boards().unwrap().len(), MAX_BOARDS);
    }

    #[test]
    fn renaming_a_board_moves_its_search_word_and_keeps_its_items() {
        let dir = TempDir::new();
        let mut s = Store::open(&dir.0).unwrap();
        let a = s.record(text("filed item"), 1).unwrap().unwrap();
        let board = s.add_board("Công việc").unwrap();
        s.set_board(a, Some(board)).unwrap();
        assert_eq!(ids(&s.search("@cong_viec", 10).unwrap()), [a]);
        assert_eq!(ids(&s.search("@Công_việc", 10).unwrap()), [a]);
        s.rename_board(board, "Personal").unwrap();
        assert_eq!(ids(&s.search("@personal", 10).unwrap()), [a]);
        assert!(s.search("@cong_viec", 10).unwrap().is_empty());
        assert_eq!(s.boards().unwrap()[0].items, 1);
        // Boards keep the order they were made in.
        s.add_board("Zed").unwrap();
        s.add_board("Alpha").unwrap();
        let names: Vec<String> = s.boards().unwrap().into_iter().map(|b| b.name).collect();
        assert_eq!(names, ["Personal", "Zed", "Alpha"]);
    }

    #[test]
    fn search_and_dedupe_behave_as_before_with_boards_and_masks_in_the_file() {
        let dir = TempDir::new();
        let mut s = Store::open(&dir.0).unwrap();
        let a = s.record(text("alpha beta"), 1).unwrap().unwrap();
        let b = s.record(text("gamma"), 2).unwrap().unwrap();
        let board = s.add_board("Group").unwrap();
        s.set_board(a, Some(board)).unwrap();
        // The same text again is the same item, filed or not; pinned items still list first.
        assert_eq!(s.record(text("alpha beta"), 3).unwrap(), Some(a));
        assert_eq!(s.record(text("gamma"), 4).unwrap(), Some(b));
        assert_eq!(s.count().unwrap(), 2);
        assert_eq!(ids(&s.search("", 10).unwrap()), [a, b]);
        assert_eq!(ids(&s.search("alp", 10).unwrap()), [a]);
        assert_eq!(ids(&s.search("ga", 10).unwrap()), [b]);
    }

    fn card() -> Capture {
        text("my card is 4111 1111 1111 1111 ok")
    }

    #[test]
    fn a_secret_is_stored_masked_and_hidden_from_lists_and_search() {
        let dir = TempDir::new();
        let mut s = Store::open(&dir.0).unwrap();
        let visible = s
            .record(text("meeting notes for friday"), 1)
            .unwrap()
            .unwrap();
        let id = s
            .record_with(card(), 2, Sensitivity::Mask { minutes: 10 })
            .unwrap()
            .unwrap();
        let items = s.search("", 10).unwrap();
        let secret = items.iter().find(|i| i.id == id).unwrap();
        assert!(secret.masked);
        assert_eq!(secret.preview, "", "a list never receives the text");
        assert!(!items.iter().find(|i| i.id == visible).unwrap().masked);
        // Nothing about its content finds it: not the digits, not a word, not a fuzzy guess.
        for query in ["4111", "1111 1111", "card", "my card is", "mcrd", "ok"] {
            assert!(
                s.search(query, 10).unwrap().iter().all(|i| i.id != id),
                "{query}"
            );
        }
        // The text is still there for a deliberate paste.
        assert!(s.is_masked(id).unwrap() && !s.is_masked(visible).unwrap());
        assert_eq!(s.payload(id).unwrap(), Some(Payload::Text(card_text())));
        // It goes away when its time comes, and not before.
        assert_eq!(s.next_expiry().unwrap(), Some(2 + 10 * 60_000));
        assert_eq!(s.purge_expired(2 + 10 * 60_000 - 1).unwrap(), 0);
        assert_eq!(s.purge_expired(2 + 10 * 60_000).unwrap(), 1);
        assert_eq!(ids(&s.search("", 10).unwrap()), [visible]);
        assert_eq!(s.next_expiry().unwrap(), None);
        // The index is still consistent after deleting a masked row.
        s.conn
            .execute(
                "INSERT INTO item_fts(item_fts) VALUES ('integrity-check')",
                [],
            )
            .unwrap();
        assert_eq!(ids(&s.search("friday", 10).unwrap()), [visible]);
    }

    fn card_text() -> String {
        "my card is 4111 1111 1111 1111 ok".into()
    }

    #[test]
    fn the_policy_decides_what_happens_to_a_secret() {
        let dir = TempDir::new();
        let mut s = Store::open(&dir.0).unwrap();
        // Keep stores it like any copy.
        let kept = s
            .record_with(card(), 1, Sensitivity::Keep)
            .unwrap()
            .unwrap();
        assert!(!s.search("", 10).unwrap()[0].masked);
        assert_eq!(ids(&s.search("4111", 10).unwrap()), [kept]);
        s.delete(kept).unwrap();
        // Skip stores nothing, and ordinary text is stored whatever the policy.
        assert_eq!(s.record_with(card(), 2, Sensitivity::Skip).unwrap(), None);
        assert_eq!(s.count().unwrap(), 0);
        let plain = s
            .record_with(text("nothing secret here"), 3, Sensitivity::Skip)
            .unwrap();
        assert!(plain.is_some());
        let plain_mask = s
            .record_with(text("also fine"), 3, Sensitivity::Mask { minutes: 5 })
            .unwrap()
            .unwrap();
        assert!(
            !s.search("", 10)
                .unwrap()
                .iter()
                .any(|i| i.id == plain_mask && i.masked)
        );
        assert_eq!(s.next_expiry().unwrap(), None);
        // Images and file lists are never inspected.
        let files = Capture {
            payload: Payload::Files(vec!["/tmp/password: hunter2.txt".into()]),
            app_id: None,
            app_name: None,
            rich: None,
        };
        let id = s
            .record_with(files, 4, Sensitivity::Mask { minutes: 5 })
            .unwrap()
            .unwrap();
        assert!(!s.is_masked(id).unwrap());
    }

    #[test]
    fn copying_a_secret_again_is_one_item_and_a_stored_copy_becomes_masked() {
        let dir = TempDir::new();
        let mut s = Store::open(&dir.0).unwrap();
        let rich = Capture {
            rich: Some(Rich {
                rtf: Some(b"rtf".to_vec()),
                html: None,
            }),
            ..card()
        };
        // Stored in the clear while the setting was off.
        let id = s.record(rich, 1).unwrap().unwrap();
        assert_eq!(ids(&s.search("4111", 10).unwrap()), [id]);
        assert!(s.rich(id).unwrap().is_some());
        // The setting is switched on and the same text is copied: same item, now masked and out of the index.
        let again = s
            .record_with(card(), 2, Sensitivity::Mask { minutes: 1 })
            .unwrap()
            .unwrap();
        assert_eq!(again, id);
        assert_eq!(s.count().unwrap(), 1);
        assert!(s.search("4111", 10).unwrap().is_empty());
        assert!(s.search("", 10).unwrap()[0].masked);
        assert_eq!(s.rich(id).unwrap(), None);
        s.conn
            .execute(
                "INSERT INTO item_fts(item_fts) VALUES ('integrity-check')",
                [],
            )
            .unwrap();
        // Copying it once more pushes the deadline back.
        s.record_with(card(), 60_000, Sensitivity::Mask { minutes: 1 })
            .unwrap();
        assert_eq!(s.next_expiry().unwrap(), Some(120_000));
        // A pinned item is the user's choice: it is neither masked nor expired.
        let kept = s.record(text("password: hunter2"), 3).unwrap().unwrap();
        s.set_pinned(kept, true).unwrap();
        s.record_with(
            text("password: hunter2"),
            4,
            Sensitivity::Mask { minutes: 1 },
        )
        .unwrap();
        assert!(!s.is_masked(kept).unwrap());
        assert_eq!(s.purge_expired(i64::MAX).unwrap(), 1);
        assert!(s.search("", 10).unwrap().iter().any(|i| i.id == kept));
    }

    #[test]
    fn pinning_a_masked_item_keeps_it_past_its_deadline() {
        let dir = TempDir::new();
        let mut s = Store::open(&dir.0).unwrap();
        let id = s
            .record_with(card(), 1, Sensitivity::Mask { minutes: 1 })
            .unwrap()
            .unwrap();
        s.set_pinned(id, true).unwrap();
        assert_eq!(s.next_expiry().unwrap(), None);
        assert_eq!(s.purge_expired(i64::MAX).unwrap(), 0);
        let item = &s.search("@pin", 10).unwrap()[0];
        assert!(item.masked && item.pinned && item.preview.is_empty());
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
