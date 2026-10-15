//! File names for captures: "Figghost 2026-10-15 at 08.40.12.png", like the system's own screenshots.

use std::path::{Path, PathBuf};

/// A local date and time, broken down.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Stamp {
    pub year: i32,
    pub month: u32,
    pub day: u32,
    pub hour: u32,
    pub minute: u32,
    pub second: u32,
}

pub fn file_name(prefix: &str, at: Stamp, extension: &str) -> String {
    format!(
        "{prefix} {:04}-{:02}-{:02} at {:02}.{:02}.{:02}.{extension}",
        at.year, at.month, at.day, at.hour, at.minute, at.second
    )
}

/// `dir/name`, or with " (2)", " (3)" ... before the extension when that file already exists.
pub fn unique_path(dir: &Path, name: &str, exists: impl Fn(&Path) -> bool) -> PathBuf {
    let first = dir.join(name);
    if !exists(&first) {
        return first;
    }
    let (stem, extension) = match name.rsplit_once('.') {
        Some((stem, ext)) => (stem, format!(".{ext}")),
        None => (name, String::new()),
    };
    (2..)
        .map(|n| dir.join(format!("{stem} ({n}){extension}")))
        .find(|path| !exists(path))
        .unwrap_or(first)
}

#[cfg(test)]
mod tests {
    use super::*;

    const AT: Stamp = Stamp {
        year: 2026,
        month: 10,
        day: 5,
        hour: 8,
        minute: 4,
        second: 9,
    };

    #[test]
    fn names_read_like_system_screenshots() {
        assert_eq!(
            file_name("Figghost", AT, "png"),
            "Figghost 2026-10-05 at 08.04.09.png"
        );
    }

    #[test]
    fn taken_names_get_a_number() {
        let dir = Path::new("/tmp/shots");
        let taken = [
            dir.join("Figghost 2026-10-05 at 08.04.09.png"),
            dir.join("Figghost 2026-10-05 at 08.04.09 (2).png"),
        ];
        let path = unique_path(dir, "Figghost 2026-10-05 at 08.04.09.png", |p| {
            taken.iter().any(|t| t == p)
        });
        assert_eq!(path, dir.join("Figghost 2026-10-05 at 08.04.09 (3).png"));
        assert_eq!(
            unique_path(dir, "plain", |p| p == dir.join("plain")),
            dir.join("plain (2)")
        );
    }
}
