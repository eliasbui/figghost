//! The few calls into the system that the backend makes itself. All the unsafe code of this crate is here, and no other
//! file knows which system it runs on: a feature that needs more of AppKit goes through this module (the adapter that the
//! current app has as `macos-sys`, which these calls are the first of).
use std::process::Command;

#[cfg(target_os = "macos")]
mod mac {
    #[link(name = "ApplicationServices", kind = "framework")]
    extern "C" {
        /// `Boolean AXIsProcessTrusted(void)`: whether the app may control the computer through Accessibility.
        fn AXIsProcessTrusted() -> u8;
    }

    pub fn accessibility_trusted() -> bool {
        // SAFETY: the function takes nothing and only reads the permission of the calling process.
        unsafe { AXIsProcessTrusted() != 0 }
    }
}

/// Whether the app has the Accessibility access it needs.
pub fn accessibility_trusted() -> bool {
    #[cfg(target_os = "macos")]
    {
        mac::accessibility_trusted()
    }
    #[cfg(not(target_os = "macos"))]
    {
        false
    }
}

/// Whether the program runs from inside an app bundle (Figghost.app/Contents/MacOS), which the login item and the
/// usage descriptions need.
pub fn bundled() -> bool {
    std::env::current_exe()
        .map(|path| path.to_string_lossy().contains(".app/Contents/"))
        .unwrap_or(false)
}

/// The first language of the system, as two letters ("vi", "en").
pub fn system_language() -> String {
    let out = Command::new("/usr/bin/defaults")
        .args(["read", "-g", "AppleLanguages"])
        .output();
    let text = out
        .ok()
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .unwrap_or_default();
    // The answer is a list, one quoted tag to a line: ("vi-VN", "en-VN").
    first_language(&text).unwrap_or_else(|| "en".to_owned())
}

fn first_language(list: &str) -> Option<String> {
    let start = list.find('"')? + 1;
    let tag = &list[start..start + list[start..].find('"')?];
    Some(tag.chars().take(2).collect::<String>().to_lowercase())
}

/// Opens an address, a file or a folder the way Finder would. `open` hands the work to the system and returns at once, so
/// it is waited for (an unwaited child stays in the process table until the app ends).
pub fn open(target: &str) -> Result<(), String> {
    let status = Command::new("/usr/bin/open")
        .arg(target)
        .status()
        .map_err(|e| e.to_string())?;
    if status.success() {
        Ok(())
    } else {
        Err(format!("open {target}: {status}"))
    }
}

#[cfg(test)]
mod tests {
    use super::first_language;

    #[test]
    fn the_first_language_of_the_list_is_read_as_two_letters() {
        assert_eq!(
            first_language("(\n    \"vi-VN\",\n    \"en-VN\"\n)").as_deref(),
            Some("vi")
        );
        assert_eq!(first_language("(\"en-US\")").as_deref(), Some("en"));
        assert_eq!(first_language("()"), None);
    }
}
