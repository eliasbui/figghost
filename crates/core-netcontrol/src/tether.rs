//! Sharing this Mac's connection with an Android phone on USB (beta), through gnirehtet (Genymobile, Apache 2.0):
//! `adb reverse` carries the phone's traffic to a relay on the Mac, and an app on the phone acts as its VPN. Neither
//! side needs an administrator or root. Figghost only finds the tools, lists the phones and runs `gnirehtet autorun`.

use std::path::{Path, PathBuf};

/// Where Homebrew and the usual installs put the tools.
pub const TOOL_DIRS: [&str; 3] = ["/opt/homebrew/bin", "/usr/local/bin", "/usr/bin"];

/// The program of that name in the first of `dirs` that has it.
pub fn find_in(name: &str, dirs: &[&Path]) -> Option<PathBuf> {
    dirs.iter().map(|d| d.join(name)).find(|p| p.is_file())
}

pub fn find(name: &str) -> Option<PathBuf> {
    let dirs: Vec<&Path> = TOOL_DIRS.iter().map(Path::new).collect();
    find_in(name, &dirs)
}

/// The app gnirehtet installs on the phone, wherever its package put it, for `GNIREHTET_APK`.
pub fn apk_near(gnirehtet: &Path) -> Option<PathBuf> {
    let real = gnirehtet
        .canonicalize()
        .unwrap_or_else(|_| gnirehtet.to_owned());
    let dir = real.parent()?;
    [
        dir.join("gnirehtet.apk"),
        dir.join("../libexec/gnirehtet.apk"),
        dir.join("../share/gnirehtet/gnirehtet.apk"),
        dir.join("../share/gnirehtet.apk"),
    ]
    .into_iter()
    .find(|p| p.is_file())
}

/// The phones `adb devices` lists as ready, by serial; those not authorised yet or offline are left out.
pub fn parse_devices(text: &str) -> Vec<String> {
    text.lines()
        .skip_while(|l| !l.starts_with("List of devices"))
        .skip(1)
        .filter_map(|l| {
            let mut parts = l.split_whitespace();
            let serial = parts.next()?;
            (parts.next() == Some("device")).then(|| serial.to_owned())
        })
        .collect()
}

/// Phones that are plugged in but have not allowed this Mac yet: they say so on their screen.
pub fn count_unauthorized(text: &str) -> usize {
    text.lines()
        .filter(|l| l.split_whitespace().nth(1) == Some("unauthorized"))
        .count()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ready_phones_are_listed_and_the_others_counted() {
        let text = "* daemon started successfully\nList of devices attached\nR58M12ABC\tdevice\nemulator-5554\toffline\nXYZ\tunauthorized\n\n";
        assert_eq!(parse_devices(text), ["R58M12ABC"]);
        assert_eq!(count_unauthorized(text), 1);
        assert!(parse_devices("").is_empty());
    }

    #[test]
    fn a_tool_is_found_in_the_first_folder_that_has_it() {
        let dir = std::env::temp_dir().join(format!("figghost-tether-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("adb"), "").unwrap();
        assert_eq!(
            find_in("adb", &[Path::new("/nowhere"), &dir]),
            Some(dir.join("adb"))
        );
        assert_eq!(find_in("gnirehtet", &[&dir]), None);
        std::fs::write(dir.join("gnirehtet.apk"), "").unwrap();
        assert_eq!(
            apk_near(&dir.join("adb")).map(|p| p.file_name().unwrap().to_owned()),
            Some("gnirehtet.apk".into())
        );
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
