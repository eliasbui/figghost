//! The newest release on GitHub: its version, the app to download and what changed, for the update check.
//!
//! The pages may not talk to the network (their content security policy blocks every request), so the app
//! makes the call with the same one-GET helper the Smart Select feature uses.

use core_smart::net::{self, NetError};
use serde_json::Value;

const LATEST: &str = "https://api.github.com/repos/eliasbui/figghost/releases/latest";
/// Every address taken from the answer, for a person or for curl, has to be under this one.
const RELEASES: &str = "https://github.com/eliasbui/figghost/releases/";
/// Long enough for a slow connection, short enough that the row never looks stuck.
const SECONDS: u64 = 10;
/// The most lines of notes the popup is given, and the longest one of them in characters.
const MAX_CHANGES: usize = 30;
const MAX_LINE: usize = 240;

#[derive(Debug, Clone, PartialEq)]
pub struct Release {
    /// Without the `v` of the tag.
    pub version: String,
    /// The release page, for a person to download from when the app cannot install by itself.
    pub url: String,
    pub asset: Option<Asset>,
    /// What changed, one plain line each.
    pub changes: Vec<String>,
}

/// The zip of the app that a release carries.
#[derive(Debug, Clone, PartialEq)]
pub struct Asset {
    pub url: String,
    pub size: u64,
    /// The SHA-256 that GitHub computed on upload, as lower case hex, when it says.
    pub sha256: Option<String>,
}

/// The latest published release.
pub fn latest() -> Result<Release, String> {
    let body = net::get(LATEST, &[], SECONDS).map_err(|e| match e {
        NetError::NoCurl => "curl is missing".to_owned(),
        NetError::Unreachable => "GitHub could not be reached".to_owned(),
        NetError::Status => "GitHub has no release to show".to_owned(),
        other => format!("{other:?}"),
    })?;
    parse(&body)
}

/// A version is dotted numbers and nothing else, since it ends up in file names and is compared with the app's.
fn is_version(text: &str) -> bool {
    !text.is_empty()
        && text
            .split('.')
            .all(|part| !part.is_empty() && part.bytes().all(|b| b.is_ascii_digit()))
}

/// What the release endpoint answers, cut down to what the update needs.
fn parse(body: &str) -> Result<Release, String> {
    let json: Value = serde_json::from_str(body).map_err(|e| e.to_string())?;
    let version = json["tag_name"]
        .as_str()
        .unwrap_or_default()
        .trim_start_matches('v')
        .to_owned();
    let url = json["html_url"].as_str().unwrap_or_default().to_owned();
    if !is_version(&version) || !url.starts_with(RELEASES) {
        return Err("the latest release has no version".into());
    }
    let name = format!("Figghost-{version}.zip");
    let asset = json["assets"]
        .as_array()
        .and_then(|assets| assets.iter().find(|a| a["name"] == name.as_str()))
        .and_then(|a| {
            let url = a["browser_download_url"]
                .as_str()
                .filter(|u| u.starts_with(RELEASES))?;
            Some(Asset {
                url: url.to_owned(),
                size: a["size"].as_u64()?,
                sha256: a["digest"].as_str().and_then(sha256_of),
            })
        });
    let changes = changes(json["body"].as_str().unwrap_or_default());
    Ok(Release {
        version,
        url,
        asset,
        changes,
    })
}

/// The hash in a digest such as `sha256:ab12...`.
fn sha256_of(digest: &str) -> Option<String> {
    let hex = digest.strip_prefix("sha256:")?;
    (hex.len() == 64 && hex.bytes().all(|b| b.is_ascii_hexdigit())).then(|| hex.to_lowercase())
}

/// The lines of the notes worth showing: the list under `Changes:` (what `scripts/release.sh` writes), or
/// every list item when the notes were written by hand. The release commit itself is not news, and a
/// commit's `feat(scope):` label is the author's, not the reader's.
pub(super) fn changes(notes: &str) -> Vec<String> {
    let lines: Vec<&str> = notes.lines().map(str::trim).collect();
    let start = lines
        .iter()
        .position(|line| line.eq_ignore_ascii_case("changes:"))
        .map_or(0, |at| at + 1);
    let mut found = Vec::new();
    let mut listing = false;
    for line in &lines[start..] {
        let Some(item) = line.strip_prefix("- ").or_else(|| line.strip_prefix("* ")) else {
            // The text after the list (the footer of the notes) is not part of it.
            if listing && !line.is_empty() {
                break;
            }
            continue;
        };
        listing = true;
        let item = tidy(item);
        if !item.is_empty() && !is_release_commit(&item) {
            found.push(item);
        }
        if found.len() == MAX_CHANGES {
            break;
        }
    }
    found
}

/// The line without a conventional commit label, starting with a capital and cut to a sensible length.
fn tidy(item: &str) -> String {
    let item = item.trim();
    let item = match item.split_once(": ") {
        Some((label, rest)) if is_commit_label(label) => rest.trim_start(),
        _ => item,
    };
    let mut chars = item.chars();
    let mut line: String = chars
        .next()
        .map(|first| first.to_uppercase().chain(chars).collect())
        .unwrap_or_default();
    if line.chars().count() > MAX_LINE {
        line = line.chars().take(MAX_LINE - 1).collect::<String>() + "…";
    }
    line
}

/// `feat`, `fix(mole)`, `refactor!` and the like.
fn is_commit_label(label: &str) -> bool {
    let kind = label.split(['(', '!']).next().unwrap_or_default();
    const KINDS: [&str; 11] = [
        "feat", "fix", "perf", "refactor", "docs", "chore", "build", "ci", "test", "style",
        "revert",
    ];
    KINDS.contains(&kind) && !label.contains(' ')
}

/// `Release 0.5.0`, the commit that `scripts/release.sh` makes to bump the version.
fn is_release_commit(line: &str) -> bool {
    line.strip_prefix("Release ")
        .is_some_and(|rest| rest.starts_with(|c: char| c.is_ascii_digit()))
}

/// Whether `latest` comes after `current`. Both are dotted, and anything after the numbers (`-beta.1`) is
/// ignored, so a pre-release counts as the version it leads to.
pub fn newer(latest: &str, current: &str) -> bool {
    fn parts(version: &str) -> Vec<u64> {
        version
            .split(['.', '-', '+'])
            .map(|part| part.parse().unwrap_or(0))
            .collect()
    }
    let (mut latest, mut current) = (parts(latest), parts(current));
    let len = latest.len().max(current.len());
    latest.resize(len, 0);
    current.resize(len, 0);
    latest > current
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The answer for 0.5.0, as GitHub gave it, cut down to the fields that matter.
    const ANSWER: &str = r#"{
        "tag_name": "v0.5.0",
        "html_url": "https://github.com/eliasbui/figghost/releases/tag/v0.5.0",
        "name": "Figghost 0.5.0", "draft": false, "prerelease": false,
        "assets": [
            { "name": "notes.txt", "size": 5, "digest": null,
              "browser_download_url": "https://github.com/eliasbui/figghost/releases/download/v0.5.0/notes.txt" },
            { "name": "Figghost-0.5.0.zip", "size": 4352386,
              "digest": "sha256:B7D6E838147FEEB7921D8E738E60F9E85E593F828017F57C827D68C013E1D031",
              "browser_download_url": "https://github.com/eliasbui/figghost/releases/download/v0.5.0/Figghost-0.5.0.zip" }
        ],
        "body": "Figghost 0.5.0, for Apple silicon Macs with macOS 13 or later.\n\n    brew tap eliasbui/figghost\n    brew install --cask eliasbui/figghost/figghost\n\nWithout Homebrew:\n\n    curl -fsSL https://example.com/install.sh | sh\n\nChanges:\n\n- Release 0.5.0\n- feat(mole): add a Mole page for health, clean, optimize, purge, uninstall, disk usage and history\n\nThe app is signed ad hoc and not notarized.\n- this line is the footer, not a change\n"
    }"#;

    #[test]
    fn a_later_version_is_newer() {
        assert!(newer("0.4.0", "0.3.0"));
        assert!(newer("0.3.1", "0.3.0"));
        assert!(newer("1.0.0", "0.9.9"));
        assert!(
            newer("0.4.0-beta.1", "0.3.0"),
            "a pre-release of a new version is news"
        );
    }

    #[test]
    fn the_running_version_is_not_newer() {
        assert!(!newer("0.3.0", "0.3.0"));
        assert!(!newer("0.2.9", "0.3.0"));
        assert!(!newer("0.3", "0.3.0"));
        assert!(!newer("", "0.3.0"));
    }

    #[test]
    fn a_release_is_read_from_the_answer() {
        let release = parse(ANSWER).unwrap();
        assert_eq!(release.version, "0.5.0");
        assert_eq!(
            release.url,
            "https://github.com/eliasbui/figghost/releases/tag/v0.5.0"
        );
        let asset = release.asset.expect("the zip is among the assets");
        assert_eq!(
            asset.url,
            "https://github.com/eliasbui/figghost/releases/download/v0.5.0/Figghost-0.5.0.zip"
        );
        assert_eq!(asset.size, 4_352_386);
        assert_eq!(
            asset.sha256.as_deref(),
            Some("b7d6e838147feeb7921d8e738e60f9e85e593f828017f57c827d68c013e1d031"),
            "the hash is kept in lower case"
        );

        // A body without the fields the update needs is a failure, not an empty version.
        assert!(parse("not json").is_err());
        assert!(parse(r#"{ "message": "Not Found" }"#).is_err());
    }

    #[test]
    fn an_address_outside_the_releases_is_not_trusted() {
        let page =
            r#"{ "tag_name": "v1.0.0", "html_url": "https://evil.example/releases/tag/v1.0.0" }"#;
        assert!(parse(page).is_err(), "the release page");

        let zip = r#"{ "tag_name": "v1.0.0", "html_url": "https://github.com/eliasbui/figghost/releases/tag/v1.0.0",
            "assets": [{ "name": "Figghost-1.0.0.zip", "size": 1, "browser_download_url": "https://evil.example/Figghost-1.0.0.zip" }] }"#;
        assert!(parse(zip).unwrap().asset.is_none(), "the zip");
    }

    #[test]
    fn an_odd_version_is_refused() {
        for tag in ["v", "vnext", "v1..2", "v1.2.x", "v../../etc", "v1.0.0-rc1"] {
            let body = format!(
                r#"{{ "tag_name": "{tag}", "html_url": "https://github.com/eliasbui/figghost/releases/tag/{tag}" }}"#
            );
            assert!(parse(&body).is_err(), "{tag}");
        }
    }

    #[test]
    fn a_missing_or_malformed_digest_leaves_the_zip_unchecked_not_unusable() {
        for digest in ["", "md5:abcd", "sha256:xyz", "sha256:abcd"] {
            let body = format!(
                r#"{{ "tag_name": "v1.0.0", "html_url": "https://github.com/eliasbui/figghost/releases/tag/v1.0.0",
                "assets": [{{ "name": "Figghost-1.0.0.zip", "size": 9, "digest": "{digest}",
                "browser_download_url": "https://github.com/eliasbui/figghost/releases/download/v1.0.0/Figghost-1.0.0.zip" }}] }}"#
            );
            let asset = parse(&body).unwrap().asset.unwrap();
            assert_eq!(asset.sha256, None, "{digest:?}");
        }
    }

    #[test]
    fn the_changes_are_the_list_under_changes() {
        let release = parse(ANSWER).unwrap();
        assert_eq!(
            release.changes,
            [
                "Add a Mole page for health, clean, optimize, purge, uninstall, disk usage and history"
            ],
            "the release commit and the footer are left out, the label is dropped"
        );
    }

    #[test]
    fn notes_written_by_hand_are_listed_as_they_are() {
        let notes = "A short word first.\n\n* Faster scrolling\n* fix(snap): zones no longer flicker\n- A line that is\n  a bit long\n\nThanks!";
        assert_eq!(
            changes(notes),
            [
                "Faster scrolling",
                "Zones no longer flicker",
                "A line that is"
            ]
        );
        assert!(changes("Nothing listed here.").is_empty());
        assert!(changes("").is_empty());
    }

    #[test]
    fn labels_are_only_dropped_when_they_are_labels() {
        assert_eq!(tidy("feat: add x"), "Add x");
        assert_eq!(tidy("fix(mole)!: stop y"), "Stop y");
        assert_eq!(tidy("Mole: add x"), "Mole: add x");
        assert_eq!(tidy("note to self: add x"), "Note to self: add x");
        assert_eq!(tidy("feat(a b): c"), "Feat(a b): c");
        assert_eq!(tidy("đã sửa lỗi"), "Đã sửa lỗi");
    }

    #[test]
    fn a_long_list_and_a_long_line_are_cut() {
        let list: String = (0..MAX_CHANGES + 10)
            .map(|n| format!("- change {n}\n"))
            .collect();
        assert_eq!(changes(&list).len(), MAX_CHANGES);

        let long = tidy(&"x".repeat(1000));
        assert_eq!(long.chars().count(), MAX_LINE);
        assert!(long.ends_with('…'));
    }

    /// The real call. Run it by hand: `cargo test -p app-ui -- --ignored --nocapture github_answers`.
    #[test]
    #[ignore = "asks GitHub over the network"]
    fn github_answers_with_a_release() {
        let release = latest().expect("GitHub answered");
        println!(
            "latest release: {} ({}), {} changes",
            release.version,
            release.url,
            release.changes.len()
        );
        assert!(
            release.url.starts_with(RELEASES),
            "{} is not a release page",
            release.url
        );
        assert_eq!(
            release.version.split('.').count(),
            3,
            "{} is not a three number version",
            release.version
        );
        let asset = release.asset.expect("the release carries the app");
        println!("zip: {} bytes, sha256 {:?}", asset.size, asset.sha256);
        assert!(asset.size > 1_000_000, "{} bytes is not an app", asset.size);
    }
}
