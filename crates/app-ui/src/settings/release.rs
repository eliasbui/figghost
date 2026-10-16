//! The newest release on GitHub, for the "Check for updates" row of the About page.
//!
//! The page itself may not talk to the network (its content security policy blocks every request), so the
//! app makes the call with the same one-GET helper the Smart Select feature uses.

use core_smart::net::{self, NetError};
use serde_json::Value;

const LATEST: &str = "https://api.github.com/repos/eliasbui/figghost/releases/latest";
/// Long enough for a slow connection, short enough that the row never looks stuck.
const SECONDS: u64 = 10;

pub struct Release {
    pub version: String,
    pub url: String,
}

/// The latest published release: its version without the `v`, and the page a person can download from.
pub fn latest() -> Result<Release, String> {
    let body = net::get(LATEST, &[], SECONDS).map_err(|e| match e {
        NetError::NoCurl => "curl is missing".to_owned(),
        NetError::Unreachable => "GitHub could not be reached".to_owned(),
        NetError::Status => "GitHub has no release to show".to_owned(),
        other => format!("{other:?}"),
    })?;
    parse(&body)
}

/// What the release endpoint answers, cut down to the two fields the row needs.
fn parse(body: &str) -> Result<Release, String> {
    let json: Value = serde_json::from_str(body).map_err(|e| e.to_string())?;
    let version = json["tag_name"]
        .as_str()
        .unwrap_or_default()
        .trim_start_matches('v')
        .to_owned();
    let url = json["html_url"].as_str().unwrap_or_default().to_owned();
    if version.is_empty() || url.is_empty() {
        return Err("the latest release has no version".into());
    }
    Ok(Release { version, url })
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
        let body = r#"{ "tag_name": "v0.4.0", "html_url": "https://github.com/eliasbui/figghost/releases/tag/v0.4.0",
            "name": "Figghost 0.4.0", "draft": false }"#;
        let release = parse(body).unwrap();
        assert_eq!(release.version, "0.4.0");
        assert_eq!(
            release.url,
            "https://github.com/eliasbui/figghost/releases/tag/v0.4.0"
        );

        // A body without the fields the row needs is a failure, not an empty version.
        assert!(parse("not json").is_err());
        assert!(parse(r#"{ "message": "Not Found" }"#).is_err());
    }

    /// The real call. Run it by hand: `cargo test -p app-ui -- --ignored --nocapture github_answers`.
    #[test]
    #[ignore = "asks GitHub over the network"]
    fn github_answers_with_a_release() {
        let release = latest().expect("GitHub answered");
        println!("latest release: {} ({})", release.version, release.url);
        assert!(
            release
                .url
                .starts_with("https://github.com/eliasbui/figghost/releases/"),
            "{} is not a release page",
            release.url
        );
        assert_eq!(
            release.version.split('.').count(),
            3,
            "{} is not a three number version",
            release.version
        );
    }
}
