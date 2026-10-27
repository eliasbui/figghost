//! The store of Notification Center, read and never written: the newest records at the start, then the ones after the
//! last record seen, by their numbers. A connection is opened read-only for each look and let go at once, so the
//! system's service is never held up.

use rusqlite::{Connection, OpenFlags, params};
use std::{fs, path::Path, time::Duration, time::SystemTime};

/// A record as the store keeps it: its number and the property list of what it says.
#[derive(Debug, Clone, PartialEq)]
pub struct Raw {
    pub id: i64,
    pub data: Vec<u8>,
}

/// How long a look waits while the service writes.
const BUSY: Duration = Duration::from_millis(250);

fn open(path: &Path) -> rusqlite::Result<Connection> {
    let conn = Connection::open_with_flags(
        path,
        OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )?;
    conn.busy_timeout(BUSY)?;
    Ok(conn)
}

/// The number of the newest record and the newest `count` records, oldest first. An error when the store cannot be
/// read, which is what a missing Full Disk Access looks like.
pub fn newest(path: &Path, count: usize) -> rusqlite::Result<(i64, Vec<Raw>)> {
    let conn = open(path)?;
    let last: i64 = conn.query_row("SELECT IFNULL(MAX(rec_id), 0) FROM record", [], |r| {
        r.get(0)
    })?;
    let mut statement = conn.prepare(
        "SELECT rec_id, data FROM record WHERE data IS NOT NULL ORDER BY rec_id DESC LIMIT ?1",
    )?;
    let mut rows = statement
        .query_map(params![count as i64], |r| {
            Ok(Raw {
                id: r.get(0)?,
                data: r.get(1)?,
            })
        })?
        .collect::<rusqlite::Result<Vec<Raw>>>()?;
    rows.reverse();
    Ok((last, rows))
}

/// The records after number `after`, oldest first, at most `count`.
pub fn after(path: &Path, after: i64, count: usize) -> rusqlite::Result<Vec<Raw>> {
    let conn = open(path)?;
    let mut statement = conn.prepare(
        "SELECT rec_id, data FROM record WHERE rec_id > ?1 AND data IS NOT NULL ORDER BY rec_id LIMIT ?2",
    )?;
    statement
        .query_map(params![after, count as i64], |r| {
            Ok(Raw {
                id: r.get(0)?,
                data: r.get(1)?,
            })
        })?
        .collect()
}

/// When the store and its log were last written: the store is looked into only when this changes.
pub fn stamp(path: &Path) -> Option<(SystemTime, Option<SystemTime>)> {
    let modified = |p: &Path| fs::metadata(p).and_then(|m| m.modified()).ok();
    Some((modified(path)?, modified(&path.with_file_name("db-wal"))))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    /// A folder of its own, removed when the test ends, also when it fails.
    struct TempDir(PathBuf);

    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    fn record(app: &str, title: &str) -> Vec<u8> {
        format!(
            r#"<?xml version="1.0" encoding="UTF-8"?><plist version="1.0"><dict>
            <key>app</key><string>{app}</string><key>date</key><real>814766740</real>
            <key>req</key><dict><key>titl</key><string>{title}</string><key>body</key><string>b</string></dict>
            </dict></plist>"#
        )
        .into_bytes()
    }

    /// A store laid out as the system's, with three records and one without data.
    fn store(name: &str) -> (TempDir, PathBuf) {
        let dir = TempDir(
            std::env::temp_dir().join(format!("figghost-notes-{name}-{}", std::process::id())),
        );
        let _ = fs::remove_dir_all(&dir.0);
        fs::create_dir_all(&dir.0).unwrap();
        let path = dir.0.join("db");
        let conn = Connection::open(&path).unwrap();
        conn.execute_batch(
            "CREATE TABLE record (rec_id INTEGER PRIMARY KEY, app_id INTEGER, uuid BLOB, data BLOB,
             request_date REAL, request_last_date REAL, delivered_date REAL, presented Bool, style INTEGER,
             snooze_fire_date REAL);",
        )
        .unwrap();
        for (id, data) in [
            (3, Some(record("com.example.mail", "one"))),
            (5, Some(record("com.example.chat", "two"))),
            (6, None),
            (9, Some(record("com.example.mail", "three"))),
        ] {
            conn.execute(
                "INSERT INTO record (rec_id, data) VALUES (?1, ?2)",
                params![id, data],
            )
            .unwrap();
        }
        (dir, path)
    }

    #[test]
    fn the_newest_come_oldest_first_with_the_number_of_the_last() {
        let (_dir, path) = store("newest");
        let (last, rows) = newest(&path, 2).unwrap();
        assert_eq!(last, 9);
        assert_eq!(rows.iter().map(|r| r.id).collect::<Vec<_>>(), [5, 9]);
        let fields = macos_sys::notification_store::decode(&rows[1].data).unwrap();
        assert_eq!(
            (fields.app.as_str(), fields.title.as_str()),
            ("com.example.mail", "three")
        );
    }

    #[test]
    fn what_came_after_a_number_is_read_and_a_record_without_data_is_left_out() {
        let (_dir, path) = store("after");
        let ids = |rows: Vec<Raw>| rows.iter().map(|r| r.id).collect::<Vec<_>>();
        assert_eq!(ids(after(&path, 3, 10).unwrap()), [5, 9]);
        assert_eq!(
            ids(after(&path, 3, 1).unwrap()),
            [5],
            "no more than asked for"
        );
        assert!(after(&path, 9, 10).unwrap().is_empty());
        assert!(stamp(&path).is_some(), "a store has a time it was written");
    }

    #[test]
    fn a_store_that_cannot_be_read_is_an_error_not_a_crash_and_is_never_made() {
        let dir = TempDir(
            std::env::temp_dir().join(format!("figghost-notes-none-{}", std::process::id())),
        );
        let missing = dir.0.join("db");
        assert!(newest(&missing, 5).is_err());
        assert!(after(&missing, 0, 5).is_err());
        assert!(!missing.exists(), "reading never creates the store");
        assert!(stamp(&missing).is_none());
    }
}
