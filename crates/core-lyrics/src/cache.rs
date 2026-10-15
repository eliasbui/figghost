//! What was found for a track, kept on disk so the same song is never asked about twice, and "nothing found"
//! remembered for a while so a track without lyrics is not asked about again and again.
//!
//! A file is named by the hash of the track (see `Query::key`) and holds nothing but lyrics. The folder is
//! private to the user, its size and file count are capped, and a file that cannot be read is moved aside.

use crate::lrclib::Lyrics;
use serde_json::{Value, json};
use std::{
    fs,
    io::Write,
    os::unix::fs::{DirBuilderExt, OpenOptionsExt},
    path::{Path, PathBuf},
    time::SystemTime,
};

pub const MAX_ENTRIES: usize = 500;
pub const MAX_BYTES: u64 = 8 << 20;
/// How long "no lyrics" is believed: three days, after which the song is asked about once more.
pub const MISSING_SECONDS: u64 = 3 * 24 * 60 * 60;
const MAX_FILE_BYTES: u64 = 600 * 1024;
const VERSION: u64 = 1;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Entry {
    Found(Lyrics),
    Missing,
}

pub struct Cache {
    dir: PathBuf,
    max_entries: usize,
    max_bytes: u64,
}

fn valid_key(key: &str) -> bool {
    key.len() == 16 && key.bytes().all(|b| b.is_ascii_hexdigit())
}

impl Cache {
    pub fn new(dir: PathBuf) -> Self {
        Self::with_limits(dir, MAX_ENTRIES, MAX_BYTES)
    }

    pub fn with_limits(dir: PathBuf, max_entries: usize, max_bytes: u64) -> Self {
        Self {
            dir,
            max_entries,
            max_bytes,
        }
    }

    fn path(&self, key: &str) -> PathBuf {
        self.dir.join(format!("{key}.json"))
    }

    /// `None` when nothing is known: no file, an expired "missing", or a file that was moved aside.
    pub fn get(&self, key: &str, now: u64) -> Option<Entry> {
        if !valid_key(key) {
            return None;
        }
        let path = self.path(key);
        let meta = fs::metadata(&path).ok()?;
        let parsed = (meta.len() <= MAX_FILE_BYTES)
            .then(|| fs::read_to_string(&path).ok())
            .flatten()
            .and_then(|text| parse(&text));
        let Some((at, entry)) = parsed else {
            let _ = fs::rename(&path, path.with_extension("bad"));
            return None;
        };
        if entry == Entry::Missing && now.saturating_sub(at) >= MISSING_SECONDS {
            let _ = fs::remove_file(&path);
            return None;
        }
        Some(entry)
    }

    pub fn put(&self, key: &str, entry: &Entry, now: u64) {
        if !valid_key(key) {
            return;
        }
        let text = match entry {
            Entry::Missing => json!({ "v": VERSION, "at": now, "missing": true }),
            Entry::Found(l) => json!({
                "v": VERSION, "at": now, "synced": l.synced, "plain": l.plain, "instrumental": l.instrumental
            }),
        }
        .to_string();
        if fs::DirBuilder::new()
            .recursive(true)
            .mode(0o700)
            .create(&self.dir)
            .is_err()
        {
            return;
        }
        let (temp, path) = (self.dir.join(format!("{key}.tmp")), self.path(key));
        let written = fs::OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .mode(0o600)
            .open(&temp)
            .and_then(|mut file| file.write_all(text.as_bytes()))
            .and_then(|()| fs::rename(&temp, &path));
        if written.is_err() {
            let _ = fs::remove_file(&temp);
            return;
        }
        self.prune();
    }

    /// Removes the oldest files until both caps hold. Only files of this cache are touched.
    fn prune(&self) {
        let Ok(read) = fs::read_dir(&self.dir) else {
            return;
        };
        let mut files: Vec<(SystemTime, u64, PathBuf)> = read
            .filter_map(Result::ok)
            .filter(|e| {
                let name = e.file_name();
                let name = Path::new(&name);
                let stem_ok = name
                    .file_stem()
                    .and_then(|s| s.to_str())
                    .is_some_and(valid_key);
                stem_ok
                    && matches!(
                        name.extension().and_then(|x| x.to_str()),
                        Some("json" | "bad" | "tmp")
                    )
            })
            .filter_map(|e| {
                let meta = e.metadata().ok()?;
                Some((meta.modified().ok()?, meta.len(), e.path()))
            })
            .collect();
        files.sort();
        let mut count = files.len();
        let mut bytes: u64 = files.iter().map(|f| f.1).sum();
        for (_, size, path) in files {
            if count <= self.max_entries && bytes <= self.max_bytes {
                break;
            }
            if fs::remove_file(path).is_ok() {
                count -= 1;
                bytes -= size;
            }
        }
    }
}

fn parse(text: &str) -> Option<(u64, Entry)> {
    let Value::Object(map) = serde_json::from_str(text).ok()? else {
        return None;
    };
    if map.get("v")?.as_u64()? != VERSION {
        return None;
    }
    let at = map.get("at")?.as_u64()?;
    if map.get("missing") == Some(&Value::Bool(true)) {
        return Some((at, Entry::Missing));
    }
    let field = |name: &str| match map.get(name)? {
        Value::String(s) => Some(Some(s.clone())),
        Value::Null => Some(None),
        _ => None,
    };
    let lyrics = Lyrics {
        synced: field("synced")?,
        plain: field("plain")?,
        instrumental: map.get("instrumental")?.as_bool()?,
    };
    // A record with no lyrics at all would be shown as "nothing", so it is as good as damaged.
    (lyrics.synced.is_some() || lyrics.plain.is_some() || lyrics.instrumental)
        .then_some((at, Entry::Found(lyrics)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{os::unix::fs::PermissionsExt, time::Duration};

    struct Dir(PathBuf);
    impl Dir {
        fn new(name: &str) -> Self {
            let dir =
                std::env::temp_dir().join(format!("figghost-lyrics-{name}-{}", std::process::id()));
            let _ = fs::remove_dir_all(&dir);
            Self(dir)
        }
    }
    impl Drop for Dir {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    fn key(n: u64) -> String {
        format!("{n:016x}")
    }

    fn found(text: &str) -> Entry {
        Entry::Found(Lyrics {
            synced: Some(format!("[00:01.00]{text}")),
            plain: Some(text.into()),
            instrumental: false,
        })
    }

    #[test]
    fn what_was_stored_comes_back_and_the_folder_is_private() {
        let dir = Dir::new("roundtrip");
        let cache = Cache::new(dir.0.clone());
        assert_eq!(cache.get(&key(1), 100), None);
        cache.put(&key(1), &found("hello"), 100);
        cache.put(&key(2), &Entry::Missing, 100);
        cache.put(
            &key(3),
            &Entry::Found(Lyrics {
                instrumental: true,
                ..Lyrics::default()
            }),
            100,
        );
        assert_eq!(cache.get(&key(1), 200), Some(found("hello")));
        assert_eq!(cache.get(&key(2), 200), Some(Entry::Missing));
        assert!(matches!(cache.get(&key(3), 200), Some(Entry::Found(l)) if l.instrumental));
        let mode = |p: &Path| fs::metadata(p).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode(&dir.0), 0o700);
        assert_eq!(mode(&dir.0.join(format!("{}.json", key(1)))), 0o600);
        // Only the hash and the lyrics are on disk.
        let text = fs::read_to_string(dir.0.join(format!("{}.json", key(1)))).unwrap();
        assert!(text.contains("hello") && !text.contains("artist") && !text.contains("title"));
    }

    #[test]
    fn missing_expires_but_found_does_not() {
        let dir = Dir::new("expiry");
        let cache = Cache::new(dir.0.clone());
        cache.put(&key(1), &Entry::Missing, 1000);
        cache.put(&key(2), &found("kept"), 1000);
        assert_eq!(
            cache.get(&key(1), 1000 + MISSING_SECONDS - 1),
            Some(Entry::Missing)
        );
        assert_eq!(cache.get(&key(1), 1000 + MISSING_SECONDS), None);
        assert!(!dir.0.join(format!("{}.json", key(1))).exists());
        assert_eq!(cache.get(&key(2), 1000 + 400 * 86_400), Some(found("kept")));
    }

    #[test]
    fn a_damaged_file_is_moved_aside_and_asked_again() {
        let dir = Dir::new("corrupt");
        let cache = Cache::new(dir.0.clone());
        cache.put(&key(1), &found("x"), 1);
        let path = dir.0.join(format!("{}.json", key(1)));
        for bad in [
            "{ not json".to_owned(),
            String::new(),
            "[]".into(),
            r#"{"v":2,"at":1,"missing":true}"#.into(),
            r#"{"v":1,"at":1,"synced":5,"plain":null,"instrumental":false}"#.into(),
            r#"{"v":1,"at":1,"synced":null,"plain":null,"instrumental":false}"#.into(),
            "x".repeat(700 * 1024),
        ] {
            fs::write(&path, &bad).unwrap();
            assert_eq!(cache.get(&key(1), 2), None, "{}", &bad[..bad.len().min(30)]);
            assert!(!path.exists());
            assert!(path.with_extension("bad").exists());
        }
        // The slot is usable again.
        cache.put(&key(1), &found("again"), 3);
        assert_eq!(cache.get(&key(1), 4), Some(found("again")));
    }

    #[test]
    fn the_count_and_the_size_are_capped_oldest_first() {
        let dir = Dir::new("caps");
        let cache = Cache::with_limits(dir.0.clone(), 5, 1 << 30);
        for n in 0..12 {
            cache.put(&key(n), &found("line"), 10);
            let path = dir.0.join(format!("{}.json", key(n)));
            let when = SystemTime::UNIX_EPOCH + Duration::from_secs(1_000_000 + n * 10);
            fs::File::options()
                .write(true)
                .open(&path)
                .unwrap()
                .set_modified(when)
                .unwrap();
        }
        let left = |dir: &Path| fs::read_dir(dir).unwrap().count();
        assert_eq!(left(&dir.0), 5);
        assert!(cache.get(&key(11), 20).is_some() && cache.get(&key(0), 20).is_none());

        let dir = Dir::new("bytes");
        let cache = Cache::with_limits(dir.0.clone(), 1000, 3000);
        for n in 0..10 {
            cache.put(&key(n), &found(&"w".repeat(400)), 10);
            let path = dir.0.join(format!("{}.json", key(n)));
            let when = SystemTime::UNIX_EPOCH + Duration::from_secs(1_000_000 + n * 10);
            fs::File::options()
                .write(true)
                .open(&path)
                .unwrap()
                .set_modified(when)
                .unwrap();
        }
        let total: u64 = fs::read_dir(&dir.0)
            .unwrap()
            .map(|e| e.unwrap().metadata().unwrap().len())
            .sum();
        assert!(total <= 3000 && left(&dir.0) >= 2, "{total}");
        assert!(cache.get(&key(9), 20).is_some());
    }

    #[test]
    fn other_files_and_odd_keys_are_left_alone() {
        let dir = Dir::new("others");
        let cache = Cache::with_limits(dir.0.clone(), 1, 1 << 30);
        fs::create_dir_all(&dir.0).unwrap();
        fs::write(dir.0.join("notes.txt"), "mine").unwrap();
        cache.put(&key(1), &found("a"), 1);
        cache.put(&key(2), &found("b"), 1);
        assert!(dir.0.join("notes.txt").exists());
        for bad in [
            "../escape",
            "",
            "short",
            "zzzzzzzzzzzzzzzz",
            "0123456789abcdef0",
        ] {
            cache.put(bad, &found("x"), 1);
            assert_eq!(cache.get(bad, 1), None);
        }
        assert!(!dir.0.parent().unwrap().join("escape.json").exists());
    }
}
