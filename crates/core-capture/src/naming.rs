//! File names and folders for saved captures: a name from a template, the rule that an existing file is
//! never overwritten, and the check of a folder that comes from the config file or from the settings page.

use std::{
    fs::{self, OpenOptions},
    io::{self, Write},
    path::{Component, Path, PathBuf},
};

/// How many files of the same second a folder may hold before saving gives up.
const MAX_COLLISIONS: u32 = 1000;
const MAX_FOLDER_LEN: usize = 1024;
/// Folders a capture is never saved into, whatever the settings say.
const SYSTEM_ROOTS: [&str; 8] = [
    "System", "usr", "bin", "sbin", "etc", "dev", "cores", "Library",
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LocalTime {
    pub year: i32,
    pub month: u32,
    pub day: u32,
    pub hour: u32,
    pub minute: u32,
    pub second: u32,
}

/// The file name without its extension: `{date}` becomes 2026-10-15 and `{time}` 21.04.33 (no colons, which
/// Finder shows as slashes).
pub fn file_stem(template: &str, t: LocalTime) -> String {
    let date = format!("{:04}-{:02}-{:02}", t.year, t.month, t.day);
    let time = format!("{:02}.{:02}.{:02}", t.hour, t.minute, t.second);
    template
        .replace("{date}", &date)
        .replace("{time}", &time)
        .replace(['/', ':'], "-")
}

/// The name of the `n`th try: the plain name first, then "name 2", "name 3".
pub fn candidate_name(stem: &str, extension: &str, n: u32) -> String {
    if n <= 1 {
        format!("{stem}.{extension}")
    } else {
        format!("{stem} {n}.{extension}")
    }
}

/// Writes `bytes` to a new file in `dir`, creating the folder first. The file is created with `create_new`,
/// so a file that exists, or appears meanwhile, is never overwritten or followed: the next name is tried.
pub fn save_unique(dir: &Path, stem: &str, extension: &str, bytes: &[u8]) -> io::Result<PathBuf> {
    fs::create_dir_all(dir)?;
    for n in 1..=MAX_COLLISIONS {
        let path = dir.join(candidate_name(stem, extension, n));
        match OpenOptions::new().write(true).create_new(true).open(&path) {
            Ok(mut file) => {
                if let Err(error) = file.write_all(bytes) {
                    drop(file);
                    let _ = fs::remove_file(&path);
                    return Err(error);
                }
                return Ok(path);
            }
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {}
            Err(error) => return Err(error),
        }
    }
    Err(io::Error::new(
        io::ErrorKind::AlreadyExists,
        "too many files with this name",
    ))
}

/// Where captures go when no folder is chosen: Pictures/Figghost in the home folder.
pub fn default_folder(home: &Path) -> PathBuf {
    home.join("Pictures").join("Figghost")
}

/// A folder from outside: an absolute path with no `..`, no control characters, not too long and not a
/// system folder. Returns the path as it will be used, without a trailing slash.
pub fn validate_folder(text: &str) -> Result<PathBuf, &'static str> {
    let text = text.trim();
    if text.is_empty() || text.len() > MAX_FOLDER_LEN {
        return Err("empty or too long");
    }
    if text.chars().any(char::is_control) {
        return Err("control characters");
    }
    let path = Path::new(text);
    if !path.is_absolute() {
        return Err("not an absolute path");
    }
    let mut clean = PathBuf::from("/");
    let mut first = true;
    for part in path.components() {
        match part {
            Component::RootDir => {}
            Component::Normal(name) => {
                if first && SYSTEM_ROOTS.iter().any(|root| name == *root) {
                    return Err("a system folder");
                }
                first = false;
                clean.push(name);
            }
            Component::CurDir => {}
            _ => return Err("contains .."),
        }
    }
    if clean == Path::new("/") {
        return Err("the root folder");
    }
    if clean.starts_with("/private/etc") {
        return Err("a system folder");
    }
    Ok(clean)
}

/// The path with the home folder written as "~", for showing.
pub fn tilde(path: &Path, home: &Path) -> String {
    match path.strip_prefix(home) {
        Ok(rest) if !home.as_os_str().is_empty() => {
            if rest.as_os_str().is_empty() {
                "~".to_owned()
            } else {
                format!("~/{}", rest.display())
            }
        }
        _ => path.display().to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn time() -> LocalTime {
        LocalTime {
            year: 2026,
            month: 10,
            day: 15,
            hour: 21,
            minute: 4,
            second: 33,
        }
    }

    fn scratch(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "figghost-core-capture-{name}-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&dir);
        dir
    }

    #[test]
    fn the_template_fills_in_the_date_and_the_time_without_colons() {
        assert_eq!(
            file_stem("Figghost {date} at {time}", time()),
            "Figghost 2026-10-15 at 21.04.33"
        );
        assert_eq!(
            file_stem("Figghost {date} lúc {time}", time()),
            "Figghost 2026-10-15 lúc 21.04.33"
        );
        assert_eq!(file_stem("a/b:{time}", time()), "a-b-21.04.33");
    }

    #[test]
    fn the_nth_name_is_numbered_from_two() {
        assert_eq!(candidate_name("Shot", "png", 1), "Shot.png");
        assert_eq!(candidate_name("Shot", "png", 2), "Shot 2.png");
        assert_eq!(candidate_name("Shot", "png", 10), "Shot 10.png");
    }

    #[test]
    fn saving_creates_the_folder_and_never_overwrites() {
        let dir = scratch("save").join("nested").join("Figghost");
        let first = save_unique(&dir, "Shot", "png", b"one").unwrap();
        let second = save_unique(&dir, "Shot", "png", b"two").unwrap();
        let third = save_unique(&dir, "Shot", "png", b"three").unwrap();
        assert_eq!(first, dir.join("Shot.png"));
        assert_eq!(second, dir.join("Shot 2.png"));
        assert_eq!(third, dir.join("Shot 3.png"));
        assert_eq!(fs::read(&first).unwrap(), b"one");
        assert_eq!(fs::read(&second).unwrap(), b"two");
        let _ = fs::remove_dir_all(dir.parent().unwrap().parent().unwrap());
    }

    #[test]
    fn a_gap_in_the_numbers_is_not_filled_over_a_file_that_is_there() {
        let dir = scratch("gap");
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join("Shot.png"), b"mine").unwrap();
        fs::write(dir.join("Shot 3.png"), b"also mine").unwrap();
        let saved = save_unique(&dir, "Shot", "png", b"new").unwrap();
        assert_eq!(saved, dir.join("Shot 2.png"));
        assert_eq!(fs::read(dir.join("Shot.png")).unwrap(), b"mine");
        assert_eq!(fs::read(dir.join("Shot 3.png")).unwrap(), b"also mine");
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_symbolic_link_in_the_way_is_not_followed() {
        let dir = scratch("link");
        fs::create_dir_all(&dir).unwrap();
        let target = dir.join("precious");
        fs::write(&target, b"keep").unwrap();
        std::os::unix::fs::symlink(&target, dir.join("Shot.png")).unwrap();
        let saved = save_unique(&dir, "Shot", "png", b"new").unwrap();
        assert_eq!(saved, dir.join("Shot 2.png"));
        assert_eq!(fs::read(&target).unwrap(), b"keep");
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn saving_into_a_file_reports_the_error_and_leaves_nothing() {
        let dir = scratch("notdir");
        fs::create_dir_all(&dir).unwrap();
        let file = dir.join("plain");
        fs::write(&file, b"x").unwrap();
        assert!(save_unique(&file, "Shot", "png", b"x").is_err());
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn the_default_folder_is_pictures_figghost() {
        assert_eq!(
            default_folder(Path::new("/Users/me")),
            PathBuf::from("/Users/me/Pictures/Figghost")
        );
    }

    #[test]
    fn a_folder_from_outside_must_be_a_plain_absolute_path() {
        assert_eq!(
            validate_folder(" /Users/me/Shots/ ").unwrap(),
            PathBuf::from("/Users/me/Shots")
        );
        assert_eq!(
            validate_folder("/Volumes/Big Disk/Shots").unwrap(),
            PathBuf::from("/Volumes/Big Disk/Shots")
        );
        for bad in [
            "",
            "   ",
            "relative/dir",
            "~/Pictures",
            "/Users/me/../../etc",
            "/Users/me/\u{0}x",
            "/Users/me/line\nbreak",
            "/",
            "/System/Library",
            "/usr/local",
            "/etc",
            "/private/etc/ssh",
            "/Library/LaunchDaemons",
        ] {
            assert!(validate_folder(bad).is_err(), "{bad:?} should be refused");
        }
        assert!(validate_folder(&format!("/{}", "a".repeat(2000))).is_err());
    }

    #[test]
    fn the_home_folder_is_shown_as_a_tilde() {
        let home = Path::new("/Users/me");
        assert_eq!(
            tilde(Path::new("/Users/me/Pictures/Figghost"), home),
            "~/Pictures/Figghost"
        );
        assert_eq!(tilde(Path::new("/Users/me"), home), "~");
        assert_eq!(
            tilde(Path::new("/Volumes/Disk/Shots"), home),
            "/Volumes/Disk/Shots"
        );
        assert_eq!(tilde(Path::new("/Users/meow/x"), home), "/Users/meow/x");
        assert_eq!(tilde(Path::new("/x"), Path::new("")), "/x");
    }
}
