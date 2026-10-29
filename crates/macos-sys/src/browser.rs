//! The tab in front of a web browser, asked through AppleScript: its address and its title.
//!
//! Safari and the browsers built on Chromium answer; Firefox has no words for it. The first time, macOS asks the person
//! whether Figghost may control that browser. A Chromium window in incognito mode answers nothing; Safari does not say
//! which of its windows are private. A browser that is not running is never started, and the bundle identifier is only
//! ever one of the fixed list below, so nothing from outside reaches the script.

use crate::process::run_capture;
use std::{process::Command, time::Duration};

const SEPARATOR: char = '\u{1f}';

const SAFARI: [&str; 2] = ["com.apple.Safari", "com.apple.SafariTechnologyPreview"];
/// The browsers that answer with the words of Chromium (`active tab`, `mode`). Checked on Edge's own dictionary.
const CHROMIUM: [&str; 12] = [
    "com.google.Chrome",
    "com.google.Chrome.beta",
    "com.google.Chrome.dev",
    "com.google.Chrome.canary",
    "com.microsoft.edgemac",
    "com.microsoft.edgemac.Beta",
    "com.microsoft.edgemac.Dev",
    "com.microsoft.edgemac.Canary",
    "com.brave.Browser",
    "com.brave.Browser.beta",
    "com.vivaldi.Vivaldi",
    "org.chromium.Chromium",
];

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Tab {
    pub url: String,
    pub title: String,
}

/// Whether the app with this bundle identifier is a browser that can be asked.
pub fn is_browser(bundle_id: &str) -> bool {
    SAFARI.contains(&bundle_id) || CHROMIUM.contains(&bundle_id)
}

/// The tab in front of that browser, or `None` when it is not a browser that can be asked, has no window, is in
/// incognito mode, was not allowed, or did not answer within `timeout`. Blocks, so it belongs off the main thread; the
/// first ask waits for the person to answer macOS.
pub fn front_tab(bundle_id: &str, timeout: Duration) -> Option<Tab> {
    let script = script(bundle_id)?;
    parse(&run_capture(
        Command::new("/usr/bin/osascript").arg("-e").arg(script),
        timeout,
    )?)
}

fn script(bundle_id: &str) -> Option<String> {
    let id = *SAFARI
        .iter()
        .chain(CHROMIUM.iter())
        .find(|known| **known == bundle_id)?;
    let ask = if SAFARI.contains(&id) {
        "set t to current tab of front window\n\
         return (URL of t) & (character id 31) & (name of t)"
    } else {
        "if mode of front window is \"incognito\" then return \"\"\n\
         set t to active tab of front window\n\
         return (URL of t) & (character id 31) & (title of t)"
    };
    Some(format!(
        "if not (application id \"{id}\" is running) then return \"\"\n\
         tell application id \"{id}\"\n\
         if (count of windows) is 0 then return \"\"\n\
         {ask}\n\
         end tell"
    ))
}

fn parse(out: &str) -> Option<Tab> {
    let (url, title) = out.trim_end_matches(['\n', '\r']).split_once(SEPARATOR)?;
    let url = url.trim();
    (!url.is_empty()).then(|| Tab {
        url: url.to_owned(),
        title: title.trim().to_owned(),
    })
}

/// The site of an address, as it is counted: the host of an `http` or `https` address in small letters, without `www.`,
/// a port or a user. Anything else (a page of the browser itself, a file, a blank tab) is no site.
pub fn site_of(url: &str) -> Option<String> {
    let (scheme, rest) = url.trim().split_once("://")?;
    if !scheme.eq_ignore_ascii_case("http") && !scheme.eq_ignore_ascii_case("https") {
        return None;
    }
    let authority = rest.split(['/', '?', '#']).next()?;
    let host = authority.rsplit('@').next()?;
    let host = match host.strip_prefix('[') {
        // An address of IPv6 keeps its brackets and loses only the port after them.
        Some(inner) => format!("[{}]", inner.split(']').next()?),
        None => host.split(':').next()?.to_owned(),
    };
    let host = host.trim_end_matches('.').to_ascii_lowercase();
    let host = host.strip_prefix("www.").unwrap_or(&host);
    let usable = !host.is_empty()
        && host.len() <= 253
        && host
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | '_' | '[' | ']' | ':'));
    usable.then(|| host.to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_the_browsers_of_the_list_are_asked_and_each_with_its_own_words() {
        assert!(is_browser("com.apple.Safari") && is_browser("com.microsoft.edgemac"));
        assert!(!is_browser("org.mozilla.firefox") && !is_browser("com.apple.Terminal"));
        assert!(script("org.mozilla.firefox").is_none());
        assert!(script("com.google.Chrome\" to quit").is_none());
        let chrome = script("com.google.Chrome").unwrap();
        assert!(chrome.contains("incognito") && chrome.contains("active tab"));
        assert!(chrome.starts_with("if not (application id \"com.google.Chrome\" is running)"));
        let safari = script("com.apple.Safari").unwrap();
        assert!(safari.contains("current tab") && !safari.contains("incognito"));
    }

    #[test]
    fn an_answer_is_an_address_and_a_title_and_nothing_else_is_a_tab() {
        assert_eq!(
            parse("https://example.com/a\u{1f}An example\n"),
            Some(Tab {
                url: "https://example.com/a".into(),
                title: "An example".into()
            })
        );
        assert_eq!(
            parse("https://example.com\u{1f}\n").map(|t| t.title),
            Some(String::new())
        );
        for nothing in ["", "\n", "no separator\n", "\u{1f}A title\n"] {
            assert!(parse(nothing).is_none(), "{nothing:?}");
        }
    }

    #[test]
    fn a_site_is_the_host_in_small_letters_without_www_a_port_or_a_user() {
        let site = |url: &str| site_of(url);
        assert_eq!(
            site("https://www.YouTube.com/watch?v=1").as_deref(),
            Some("youtube.com")
        );
        assert_eq!(
            site("http://user:pw@docs.rs:8080/x").as_deref(),
            Some("docs.rs")
        );
        assert_eq!(
            site("https://vnexpress.net").as_deref(),
            Some("vnexpress.net")
        );
        assert_eq!(
            site("https://example.com.#top").as_deref(),
            Some("example.com")
        );
        assert_eq!(site("http://[::1]:3000/").as_deref(), Some("[::1]"));
        assert_eq!(
            site("HTTPS://mail.google.com?x").as_deref(),
            Some("mail.google.com")
        );
        for none in [
            "",
            "missing value",
            "chrome://newtab/",
            "file:///Users/me/a.html",
            "about:blank",
            "https://",
            "https://exa mple.com/",
        ] {
            assert!(site(none).is_none(), "{none}");
        }
    }
}
