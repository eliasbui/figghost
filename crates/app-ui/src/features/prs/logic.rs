//! The review requests of the Pull requests tab: reading what `gh` and `glab` print about them, putting them in order, and
//! keeping only what is safe to show and to open. Nothing here runs a command or draws, so the tests feed it real answers
//! about public projects, kept in `testdata`.

use serde_json::Value;

/// How many requests the tab keeps.
pub const MAX_REQUESTS: usize = 20;
const MAX_TITLE: usize = 140;
const MAX_NAME: usize = 80;
const MAX_URL: usize = 500;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Host {
    GitHub,
    GitLab,
}

/// One pull request, or merge request, that is waiting for the person's review.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Request {
    pub host: Host,
    /// "cli/cli"
    pub repo: String,
    pub number: u64,
    pub title: String,
    pub author: String,
    /// When it last changed, in seconds since 1970.
    pub updated: i64,
    pub draft: bool,
    /// An https address; the one thing that is opened when the request is pressed.
    pub url: String,
}

fn tidy(text: &str, max: usize) -> String {
    let kept: String = text
        .chars()
        .map(|c| {
            if crate::features::is_hidden_char(c) {
                ' '
            } else {
                c
            }
        })
        .collect();
    kept.split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .chars()
        .take(max)
        .collect()
}

/// An address that may be opened in the browser: https, with a host, and nothing that is not printable.
pub fn safe_url(url: &str) -> Option<String> {
    let url = url.trim();
    let rest = url.strip_prefix("https://")?;
    let host = rest.split(['/', '?', '#']).next()?;
    let clean_host = !host.is_empty()
        && host.len() <= 253
        && !host.contains('@')
        && host
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'-' | b':'));
    (clean_host && url.len() <= MAX_URL && url.chars().all(|c| c.is_ascii_graphic()))
        .then(|| url.to_owned())
}

/// Seconds since 1970 of a time such as `2026-10-19T15:51:40Z` or `2026-10-20T04:14:55.915Z` or one with an offset like
/// `+07:00`; `None` for anything else.
pub fn iso_seconds(text: &str) -> Option<i64> {
    let bytes = text.as_bytes();
    if bytes.len() < 20 || !text.is_char_boundary(19) {
        return None;
    }
    let digits = |range: std::ops::Range<usize>| -> Option<i64> {
        let part = text.get(range)?;
        (!part.is_empty() && part.bytes().all(|b| b.is_ascii_digit())).then(|| part.parse().ok())?
    };
    let (year, month, day) = (digits(0..4)?, digits(5..7)?, digits(8..10)?);
    let (hour, minute, second) = (digits(11..13)?, digits(14..16)?, digits(17..19)?);
    if bytes[4] != b'-'
        || bytes[7] != b'-'
        || !matches!(bytes[10], b'T' | b't' | b' ')
        || bytes[13] != b':'
        || bytes[16] != b':'
    {
        return None;
    }
    if !(1..=12).contains(&month)
        || !(1..=31).contains(&day)
        || hour > 23
        || minute > 59
        || second > 60
    {
        return None;
    }
    // After the seconds: an optional fraction, then Z or an offset.
    let mut rest = &text[19..];
    if let Some(fraction) = rest.strip_prefix('.') {
        let end = fraction
            .find(|c: char| !c.is_ascii_digit())
            .unwrap_or(fraction.len());
        if end == 0 {
            return None;
        }
        rest = &fraction[end..];
    }
    let offset = match rest {
        "Z" | "z" => 0,
        _ => {
            let sign = match rest.as_bytes().first()? {
                b'+' => 1,
                b'-' => -1,
                _ => return None,
            };
            let body = &rest[1..];
            let (h, m) = (
                body.get(0..2)?.parse::<i64>().ok()?,
                body.get(3..5)?.parse::<i64>().ok()?,
            );
            if body.len() != 5 || body.as_bytes()[2] != b':' || h > 23 || m > 59 {
                return None;
            }
            sign * (h * 3600 + m * 60)
        }
    };
    // Days from 1970-01-01 to a civil date.
    let y = if month <= 2 { year - 1 } else { year };
    let era = y.div_euclid(400);
    let yoe = y - era * 400;
    let doy = (153 * (month + if month > 2 { -3 } else { 9 }) + 2) / 5 + day - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    let days = era * 146_097 + doe - 719_468;
    Some(days * 86_400 + hour * 3600 + minute * 60 + second - offset)
}

/// The requests in what `gh search prs --json ...` printed. An entry that lacks what is shown, or whose address cannot be
/// opened safely, is left out and the rest stay; an answer that is not a list is an error.
pub fn parse_github(body: &str) -> Result<Vec<Request>, String> {
    let answer: Value = serde_json::from_str(body).map_err(|e| e.to_string())?;
    let list = answer.as_array().ok_or("not a list")?;
    Ok(list
        .iter()
        .filter_map(|item| {
            let repo = tidy(item["repository"]["nameWithOwner"].as_str()?, MAX_NAME);
            let title = tidy(item["title"].as_str()?, MAX_TITLE);
            (!repo.is_empty() && !title.is_empty()).then_some(())?;
            Some(Request {
                host: Host::GitHub,
                repo,
                number: item["number"].as_u64()?,
                title,
                author: tidy(
                    item["author"]["login"].as_str().unwrap_or_default(),
                    MAX_NAME,
                ),
                updated: iso_seconds(item["updatedAt"].as_str()?)?,
                draft: item["isDraft"].as_bool().unwrap_or(false),
                url: safe_url(item["url"].as_str()?)?,
            })
        })
        .collect())
}

/// The requests in what `glab api merge_requests...` printed.
pub fn parse_gitlab(body: &str) -> Result<Vec<Request>, String> {
    let answer: Value = serde_json::from_str(body).map_err(|e| e.to_string())?;
    let list = answer.as_array().ok_or("not a list")?;
    Ok(list
        .iter()
        .filter_map(|item| {
            // "gitlab-org/cli!4155": everything before the exclamation mark is the project.
            let full = item["references"]["full"].as_str()?;
            let repo = tidy(
                full.rsplit_once('!').map_or(full, |(project, _)| project),
                MAX_NAME,
            );
            let title = tidy(item["title"].as_str()?, MAX_TITLE);
            (!repo.is_empty() && !title.is_empty()).then_some(())?;
            Some(Request {
                host: Host::GitLab,
                repo,
                number: item["iid"].as_u64()?,
                title,
                author: tidy(
                    item["author"]["username"].as_str().unwrap_or_default(),
                    MAX_NAME,
                ),
                updated: iso_seconds(item["updated_at"].as_str()?)?,
                draft: item["draft"].as_bool().unwrap_or(false),
                url: safe_url(item["web_url"].as_str()?)?,
            })
        })
        .collect())
}

/// The number of the person who is asking, from what `glab api user` printed.
pub fn gitlab_user_id(body: &str) -> Option<u64> {
    serde_json::from_str::<Value>(body).ok()?["id"].as_u64()
}

/// All the requests of all the sources in one list: the ones changed last first, drafts only when they are wanted, none
/// twice, and no more than the tab keeps.
pub fn merged(sources: Vec<Vec<Request>>, drafts: bool) -> Vec<Request> {
    let mut all: Vec<Request> = sources
        .into_iter()
        .flatten()
        .filter(|r| drafts || !r.draft)
        .collect();
    all.sort_by(|a, b| {
        b.updated
            .cmp(&a.updated)
            .then_with(|| a.repo.cmp(&b.repo))
            .then_with(|| a.number.cmp(&b.number))
    });
    all.dedup_by(|a, b| a.url == b.url);
    all.truncate(MAX_REQUESTS);
    all
}

#[cfg(test)]
mod tests {
    use super::*;

    const GITHUB: &str = include_str!("testdata/github.json");
    const GITLAB: &str = include_str!("testdata/gitlab.json");

    #[test]
    fn a_time_is_read_to_the_second_with_or_without_a_fraction_or_an_offset() {
        assert_eq!(iso_seconds("2026-10-19T15:51:40Z"), Some(1_792_425_100));
        assert_eq!(iso_seconds("2026-10-20T04:14:55.915Z"), Some(1_792_469_695));
        assert_eq!(
            iso_seconds("2026-10-19T15:51:40+07:00"),
            Some(1_792_399_900)
        );
        assert_eq!(
            iso_seconds("2026-10-19T08:51:40-07:00"),
            Some(1_792_425_100)
        );
        assert_eq!(iso_seconds("1970-01-01T00:00:00Z"), Some(0));
        assert_eq!(iso_seconds("2000-02-29T12:00:00Z"), Some(951_825_600));
        for bad in [
            "",
            "2026-10-19",
            "2026-10-19T15:51:40",
            "2026-13-19T15:51:40Z",
            "2026-10-19T25:51:40Z",
            "2026-10-19T15:51:40.Z",
            "2026-10-19T15:51:40+0700",
            "2026-10-19T15:51:40+25:00",
            "yesterday at noon!!!!!",
            "２０２６-10-19T15:51:40Z",
            "2026/10/19T15:51:40Z",
        ] {
            assert_eq!(iso_seconds(bad), None, "{bad:?}");
        }
    }

    #[test]
    fn only_an_https_address_with_a_clean_host_can_be_opened() {
        assert_eq!(
            safe_url("https://github.com/cli/cli/pull/14752"),
            Some("https://github.com/cli/cli/pull/14752".into())
        );
        assert_eq!(
            safe_url("  https://gitlab.example.com:8443/a/b/-/merge_requests/1  "),
            Some("https://gitlab.example.com:8443/a/b/-/merge_requests/1".into())
        );
        for bad in [
            "",
            "http://github.com/x",
            "javascript:alert(1)",
            "file:///etc/passwd",
            "https://",
            "https:///x",
            "https://user@evil.com/x",
            "https://exa mple.com/x",
            "https://github.com/a\nb",
            "https://github.com/ü",
            "ftp://github.com/x",
            "github.com/x",
        ] {
            assert_eq!(safe_url(bad), None, "{bad:?}");
        }
        assert_eq!(
            safe_url(&format!("https://github.com/{}", "a".repeat(600))),
            None,
            "too long"
        );
    }

    #[test]
    fn the_real_answer_of_gh_is_read_into_requests() {
        let requests = parse_github(GITHUB).unwrap();
        assert_eq!(requests.len(), 4);
        let first = &requests[0];
        assert_eq!(first.host, Host::GitHub);
        assert_eq!(
            (first.repo.as_str(), first.number, first.author.as_str()),
            ("cli/cli", 14_752, "babakks")
        );
        assert_eq!(first.updated, 1_792_425_100);
        assert_eq!(first.url, "https://github.com/cli/cli/pull/14752");
        assert!(!first.draft);
        assert!(first.title.starts_with("Refreshable tokens"));
    }

    #[test]
    fn the_real_answer_of_glab_is_read_into_requests_with_the_project_from_the_reference() {
        let requests = parse_gitlab(GITLAB).unwrap();
        assert_eq!(requests.len(), 3);
        let first = &requests[0];
        assert_eq!(first.host, Host::GitLab);
        assert_eq!(
            (first.repo.as_str(), first.number),
            ("gitlab-org/cli", 4155)
        );
        assert_eq!(
            first.updated, 1_792_469_695,
            "the fraction of a second is dropped"
        );
        assert!(
            first
                .url
                .starts_with("https://gitlab.com/gitlab-org/cli/-/merge_requests/")
        );
        assert!(requests[1].draft && !requests[0].draft);
        assert_eq!(
            gitlab_user_id(r#"{ "id": 4242, "username": "me" }"#),
            Some(4242)
        );
        assert_eq!(gitlab_user_id("not json"), None);
        assert_eq!(gitlab_user_id(r#"{ "id": "x" }"#), None);
    }

    #[test]
    fn an_entry_that_lacks_what_is_shown_or_has_an_unsafe_address_is_left_out_and_the_rest_stay() {
        let body = r#"[
          {"number":1,"title":"Fine","url":"https://github.com/a/b/pull/1","repository":{"nameWithOwner":"a/b"},"author":{"login":"me"},"updatedAt":"2026-10-19T15:51:40Z","isDraft":false},
          {"number":2,"title":"Plain http","url":"http://github.com/a/b/pull/2","repository":{"nameWithOwner":"a/b"},"updatedAt":"2026-10-19T15:51:40Z"},
          {"number":3,"title":"A script","url":"javascript:alert(1)","repository":{"nameWithOwner":"a/b"},"updatedAt":"2026-10-19T15:51:40Z"},
          {"number":4,"title":"","url":"https://github.com/a/b/pull/4","repository":{"nameWithOwner":"a/b"},"updatedAt":"2026-10-19T15:51:40Z"},
          {"number":5,"title":"No time","url":"https://github.com/a/b/pull/5","repository":{"nameWithOwner":"a/b"}},
          {"title":"No number","url":"https://github.com/a/b/pull/6","repository":{"nameWithOwner":"a/b"},"updatedAt":"2026-10-19T15:51:40Z"},
          {"number":7,"title":"No repo","url":"https://github.com/a/b/pull/7","updatedAt":"2026-10-19T15:51:40Z"},
          "a string", null, 5
        ]"#;
        let requests = parse_github(body).unwrap();
        assert_eq!(requests.len(), 1, "{requests:?}");
        assert_eq!(requests[0].title, "Fine");
        // A person with no name on the service is shown with none.
        let anonymous = r#"[{"number":9,"title":"x","url":"https://github.com/a/b/pull/9","repository":{"nameWithOwner":"a/b"},"updatedAt":"2026-10-19T15:51:40Z"}]"#;
        assert_eq!(parse_github(anonymous).unwrap()[0].author, "");
        for broken in ["", "not json", "{}", "null", "5", r#"{"items":[]}"#] {
            assert!(parse_github(broken).is_err(), "{broken}");
            assert!(parse_gitlab(broken).is_err(), "{broken}");
        }
        assert_eq!(parse_github("[]").unwrap(), Vec::new());
    }

    #[test]
    fn a_title_is_made_into_one_clean_line_and_cut() {
        let body = format!(
            r#"[{{"number":1,"title":"Line one\nline\ttwo\u0007  three {}","url":"https://github.com/a/b/pull/1","repository":{{"nameWithOwner":"a/b"}},"updatedAt":"2026-10-19T15:51:40Z"}}]"#,
            "x".repeat(400)
        );
        let title = &parse_github(&body).unwrap()[0].title;
        assert!(title.starts_with("Line one line two three x"), "{title}");
        assert_eq!(title.chars().count(), MAX_TITLE);
        assert!(!title.chars().any(char::is_control));
    }

    #[test]
    fn the_sources_are_put_together_newest_first_without_drafts_unless_wanted_and_without_repeats()
    {
        let github = parse_github(GITHUB).unwrap();
        let gitlab = parse_gitlab(GITLAB).unwrap();
        let all = merged(vec![github.clone(), gitlab.clone(), github.clone()], true);
        assert_eq!(all.len(), 7, "the same four requests twice are listed once");
        assert!(
            all.windows(2).all(|w| w[0].updated >= w[1].updated),
            "newest first"
        );
        assert_eq!(all[0].host, Host::GitLab);
        let without = merged(vec![github, gitlab], false);
        assert_eq!(without.len(), 6);
        assert!(without.iter().all(|r| !r.draft));
        let many: Vec<Request> = (0..50)
            .map(|i| Request {
                host: Host::GitHub,
                repo: "a/b".into(),
                number: i,
                title: "t".into(),
                author: String::new(),
                updated: i as i64,
                draft: false,
                url: format!("https://github.com/a/b/pull/{i}"),
            })
            .collect();
        let kept = merged(vec![many], false);
        assert_eq!(kept.len(), MAX_REQUESTS);
        assert_eq!(kept[0].number, 49, "the newest are the ones that stay");
    }
}
