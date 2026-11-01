//! What the mirror and the teleprompter need that is not drawing: the text to read, and how fast it moves.

use std::{fs, path::Path, time::SystemTime};

/// Points a second for each speed the page offers, from 1 to 6.
pub const SPEEDS: [f64; 6] = [20.0, 32.0, 45.0, 60.0, 80.0, 110.0];
/// The sizes of the letters the page offers.
pub const TEXT_SIZES: [u32; 4] = [28, 36, 48, 60];
/// The sizes of the mirror, in points.
pub const MIRROR_SIZES: [u32; 3] = [200, 280, 360];
/// The most of a note that is read: more is not a script.
pub const MAX_CHARS: usize = 60_000;

/// How fast level `n` (1 to 6) moves; a level that is not offered is the middle one.
pub fn speed_of(level: u32) -> f64 {
    SPEEDS
        .get(level.wrapping_sub(1) as usize)
        .copied()
        .unwrap_or(SPEEDS[2])
}

/// The note written last in `dir` (`note-<number>.md`, as the Notes tab keeps them), with its text; `None` when there is
/// no note or it holds nothing. Only what the Notes tab itself lists is read: a name of its shape, a file of plain text no
/// longer than `MAX_CHARS`.
pub fn newest_note(dir: &Path) -> Option<String> {
    let mut found: Vec<(SystemTime, std::path::PathBuf)> = fs::read_dir(dir)
        .ok()?
        .flatten()
        .filter_map(|entry| {
            let name = entry.file_name().to_string_lossy().into_owned();
            let number = name.strip_prefix("note-")?.strip_suffix(".md")?;
            (!number.is_empty() && number.bytes().all(|b| b.is_ascii_digit())).then_some(())?;
            let meta = entry.metadata().ok()?;
            (meta.is_file() && meta.len() <= (MAX_CHARS as u64) * 4).then(|| {
                (
                    meta.modified().ok().unwrap_or(SystemTime::UNIX_EPOCH),
                    entry.path(),
                )
            })
        })
        .collect();
    found.sort_by(|a, b| b.0.cmp(&a.0).then_with(|| b.1.cmp(&a.1)));
    let text = fs::read_to_string(&found.first()?.1).ok()?;
    let text: String = text.chars().take(MAX_CHARS).collect();
    (!text.trim().is_empty()).then(|| text.trim().to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    fn dir(name: &str) -> std::path::PathBuf {
        let d = std::env::temp_dir().join(format!("figghost-mirror-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&d);
        fs::create_dir_all(&d).unwrap();
        d
    }

    #[test]
    fn a_speed_is_one_of_six_and_a_level_that_is_not_offered_is_the_middle_one() {
        assert_eq!((speed_of(1), speed_of(6)), (20.0, 110.0));
        assert!(
            SPEEDS.windows(2).all(|w| w[0] < w[1]),
            "each level is faster than the one before"
        );
        for bad in [0, 7, 99] {
            assert_eq!(speed_of(bad), SPEEDS[2]);
        }
    }

    #[test]
    fn the_note_written_last_is_the_one_to_read_and_only_notes_are_read() {
        let d = dir("notes");
        assert_eq!(newest_note(&d), None, "no note");
        fs::write(d.join("note-1.md"), "old words").unwrap();
        fs::write(d.join("other.md"), "not a note").unwrap();
        fs::write(d.join("note-x.md"), "not a number").unwrap();
        assert_eq!(newest_note(&d).as_deref(), Some("old words"));
        let newer = d.join("note-2.md");
        fs::write(&newer, "  fresh script\n").unwrap();
        let later = SystemTime::now() + Duration::from_secs(60);
        fs::File::options()
            .write(true)
            .open(&newer)
            .unwrap()
            .set_modified(later)
            .unwrap();
        assert_eq!(newest_note(&d).as_deref(), Some("fresh script"));
        fs::write(&newer, "   \n").unwrap();
        assert_eq!(
            newest_note(&d),
            None,
            "an empty note is no script, and the older one is not taken in its place"
        );
        let _ = fs::remove_dir_all(&d);
        assert_eq!(newest_note(&d), None, "a folder that is not there");
    }

    #[test]
    fn a_note_that_is_too_long_is_cut() {
        let d = dir("long");
        fs::write(d.join("note-1.md"), "a".repeat(MAX_CHARS * 2)).unwrap();
        assert_eq!(newest_note(&d).map(|t| t.chars().count()), Some(MAX_CHARS));
        let _ = fs::remove_dir_all(&d);
    }
}
