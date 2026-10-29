//! Two switches of the Mac that System Settings keeps deep in its pages: the dark appearance, and the icons on the
//! desktop.
//!
//! The appearance is changed through System Events, as the Shortcuts action does, so macOS asks once whether Figghost
//! may control System Events. The icons are a setting the Finder reads when it starts, so the Finder is started again
//! after it is written; its windows come back where they were. The two that change something block, so they belong off
//! the main thread.

use crate::process::run_capture;
use objc2_foundation::{NSNumber, NSString, NSUserDefaults};
use std::{process::Command, time::Duration};

/// The first change waits for the person to answer macOS.
const TIMEOUT: Duration = Duration::from_secs(30);

fn domain_value(domain: &str, key: &str) -> Option<objc2::rc::Retained<objc2::runtime::AnyObject>> {
    NSUserDefaults::standardUserDefaults()
        .persistentDomainForName(&NSString::from_str(domain))?
        .objectForKey(&NSString::from_str(key))
}

/// Whether the Mac is in its dark appearance.
pub fn dark() -> bool {
    domain_value("NSGlobalDomain", "AppleInterfaceStyle")
        .and_then(|v| v.downcast::<NSString>().ok())
        .is_some_and(|style| style.to_string().eq_ignore_ascii_case("dark"))
}

/// Puts the Mac in its dark or its light appearance; false when it could not.
pub fn set_dark(on: bool) -> bool {
    let script = format!(
        "tell application id \"com.apple.systemevents\" to tell appearance preferences to set dark mode to {on}"
    );
    run_capture(
        Command::new("/usr/bin/osascript").arg("-e").arg(script),
        TIMEOUT,
    )
    .is_some()
}

/// Whether the Finder shows the icons on the desktop, which it does unless told otherwise.
pub fn icons_shown() -> bool {
    domain_value("com.apple.finder", "CreateDesktop")
        .and_then(|v| v.downcast::<NSNumber>().ok())
        .is_none_or(|shown| shown.boolValue())
}

/// Shows or hides the icons on the desktop and starts the Finder again so that it reads it; false when it could not.
pub fn show_icons(on: bool) -> bool {
    let written = run_capture(
        Command::new("/usr/bin/defaults").args([
            "write",
            "com.apple.finder",
            "CreateDesktop",
            "-bool",
            if on { "true" } else { "false" },
        ]),
        TIMEOUT,
    )
    .is_some();
    // The Finder is kept running by the system, which starts it again at once.
    written
        && run_capture(
            Command::new("/usr/bin/killall").arg("Finder"),
            Duration::from_secs(5),
        )
        .is_some()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn defaults_read(args: &[&str]) -> Option<String> {
        let out = Command::new("/usr/bin/defaults")
            .arg("read")
            .args(args)
            .output()
            .ok()?;
        out.status
            .success()
            .then(|| String::from_utf8_lossy(&out.stdout).trim().to_owned())
    }

    #[test]
    fn the_readings_agree_with_what_defaults_says_and_change_nothing() {
        let dark_said = defaults_read(&["-g", "AppleInterfaceStyle"])
            .is_some_and(|style| style.eq_ignore_ascii_case("dark"));
        assert_eq!(dark(), dark_said);
        let shown_said = defaults_read(&["com.apple.finder", "CreateDesktop"])
            .is_none_or(|value| !matches!(value.as_str(), "0" | "false" | "NO"));
        assert_eq!(icons_shown(), shown_said);
    }
}
