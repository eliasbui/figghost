//! Finds the link that joins an online meeting in the text of an event. Only https links to the known services
//! count, and the host is checked as a whole, so a look-alike host or a link with a user name in front of the
//! real host is refused.

const MAX_URL: usize = 2048;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Service {
    Zoom,
    Meet,
    Teams,
    Webex,
}

impl Service {
    pub const fn name(self) -> &'static str {
        match self {
            Service::Zoom => "Zoom",
            Service::Meet => "Google Meet",
            Service::Teams => "Microsoft Teams",
            Service::Webex => "Webex",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Link {
    pub service: Service,
    pub url: String,
}

/// The first meeting link among the texts, taken in order; within one text, the first one that is valid.
pub fn find(texts: &[&str]) -> Option<Link> {
    texts.iter().find_map(|text| find_in(text))
}

fn find_in(text: &str) -> Option<Link> {
    let lower = text.to_ascii_lowercase();
    let mut from = 0;
    // Positions in `lower` are positions in `text`: lowercasing ASCII never changes a length.
    while let Some(at) = lower[from..].find("https://") {
        let start = from + at;
        let rest = &text[start..];
        let end = rest
            .find(|c: char| c.is_whitespace() || matches!(c, '<' | '>' | '"' | '\'' | '`' | '\\'))
            .unwrap_or(rest.len());
        if let Some(link) = check(&unwrap_entities(&rest[..end])) {
            return Some(link);
        }
        from = start + end.max("https://".len());
    }
    None
}

/// Notes written as HTML escape the ampersands of the query.
fn unwrap_entities(url: &str) -> String {
    url.replace("&amp;", "&")
}

fn check(candidate: &str) -> Option<Link> {
    let url = candidate.trim_end_matches(|c: char| ".,;:!?)]}".contains(c));
    if url.len() > MAX_URL || url.chars().any(char::is_control) {
        return None;
    }
    let (host, path) = split(url)?;
    if let Some(service) = classify(&host, path) {
        return Some(Link {
            service,
            url: url.to_owned(),
        });
    }
    // Outlook wraps the links of an invitation in a redirect that carries the real address.
    if host.ends_with(".safelinks.protection.outlook.com") {
        let wrapped = query_value(url, "url")?;
        return check(&percent_decode(&wrapped)?);
    }
    None
}

/// The lower-case host and the rest of the address starting at its path; `None` for an address with a user
/// name, a port other than 443 or a host that is not plain ASCII.
fn split(url: &str) -> Option<(String, &str)> {
    let rest = url.strip_prefix("https://").or_else(|| {
        url.get(..8)
            .filter(|s| s.eq_ignore_ascii_case("https://"))
            .map(|_| &url[8..])
    })?;
    let end = rest.find(['/', '?', '#']).unwrap_or(rest.len());
    let (authority, path) = rest.split_at(end);
    let host = authority.strip_suffix(":443").unwrap_or(authority);
    let ok = !host.is_empty()
        && host.contains('.')
        && host
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'.' || b == b'-')
        && !host.starts_with(['.', '-'])
        && !host.contains("..");
    ok.then(|| (host.to_ascii_lowercase(), path))
}

fn on_domain(host: &str, domain: &str) -> bool {
    host == domain || host.strip_suffix(domain).is_some_and(|p| p.ends_with('.'))
}

fn classify(host: &str, path: &str) -> Option<Service> {
    let route = path.split(['?', '#']).next().unwrap_or("");
    let first = |prefixes: &[&str]| prefixes.iter().any(|p| route.starts_with(p));
    if on_domain(host, "zoom.us") || on_domain(host, "zoomgov.com") {
        return first(&["/j/", "/my/", "/w/", "/wc/join/", "/s/"]).then_some(Service::Zoom);
    }
    if host == "meet.google.com" {
        let code = route.trim_start_matches('/');
        return (is_meet_code(code) || code.starts_with("lookup/")).then_some(Service::Meet);
    }
    if host == "g.co" && route.starts_with("/meet/") {
        return Some(Service::Meet);
    }
    if host == "teams.microsoft.com" {
        return first(&["/l/meetup-join/", "/meet/", "/l/meeting/"]).then_some(Service::Teams);
    }
    if host == "teams.live.com" {
        return first(&["/meet/", "/l/meetup-join/"]).then_some(Service::Teams);
    }
    if on_domain(host, "webex.com") && host != "www.webex.com" && route.len() > 1 {
        return Some(Service::Webex);
    }
    None
}

/// Meet codes are three letters, four letters and three letters.
fn is_meet_code(code: &str) -> bool {
    let parts: Vec<&str> = code.split('-').collect();
    matches!(parts.as_slice(), [a, b, c] if a.len() == 3 && b.len() == 4 && c.len() == 3
        && parts.iter().all(|p| p.bytes().all(|x| x.is_ascii_lowercase())))
}

fn query_value(url: &str, name: &str) -> Option<String> {
    let query = url.split_once('?')?.1.split('#').next()?;
    query.split('&').find_map(|pair| {
        let (key, value) = pair.split_once('=')?;
        (key == name).then(|| value.to_owned())
    })
}

fn percent_decode(text: &str) -> Option<String> {
    let bytes = text.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        match bytes[i] {
            b'%' => {
                let hex = text.get(i + 1..i + 3)?;
                out.push(u8::from_str_radix(hex, 16).ok()?);
                i += 3;
            }
            b'+' => {
                out.push(b' ');
                i += 1;
            }
            b => {
                out.push(b);
                i += 1;
            }
        }
    }
    String::from_utf8(out).ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn url(text: &str) -> Option<String> {
        find(&[text]).map(|l| l.url)
    }

    #[test]
    fn a_zoom_invitation_with_a_password_and_dial_in_numbers() {
        let notes = "Jane Doe is inviting you to a scheduled Zoom meeting.\n\nTopic: Weekly sync\n\
            Join Zoom Meeting\nhttps://acme.zoom.us/j/81234567890?pwd=Zk1xT3BoQ0d2bE9nWUFrUQ.1&from=addon\n\n\
            Meeting ID: 812 3456 7890\nOne tap mobile\n+14086380968,,81234567890# US\n";
        let link = find(&[notes]).unwrap();
        assert_eq!(link.service, Service::Zoom);
        assert_eq!(
            link.url,
            "https://acme.zoom.us/j/81234567890?pwd=Zk1xT3BoQ0d2bE9nWUFrUQ.1&from=addon"
        );
    }

    #[test]
    fn a_google_meet_link_in_the_location_with_tracking_parameters() {
        let link = find(&["https://meet.google.com/abc-defg-hij?authuser=0&hs=122&utm_source=cal"]);
        assert_eq!(link.unwrap().service, Service::Meet);
        assert!(url("Join: https://meet.google.com/lookup/abcd1234").is_some());
        assert!(url("https://meet.google.com/landing").is_none());
        assert!(url("https://meet.google.com/ABC-defg-hij").is_none());
    }

    #[test]
    fn a_teams_invitation_among_other_links() {
        let notes = "Microsoft Teams meeting\nJoin: https://teams.microsoft.com/l/meetup-join/19%3ameeting_ZDk4NzM%40thread.v2/0?context=%7b%22Tid%22%3a%22x%22%7d\n\
            Learn more: https://aka.ms/JoinTeamsMeeting | Meeting options: https://teams.microsoft.com/meetingOptions/?organizerId=1";
        let link = find(&[notes]).unwrap();
        assert_eq!(link.service, Service::Teams);
        assert!(
            link.url
                .starts_with("https://teams.microsoft.com/l/meetup-join/")
        );
        assert!(url("https://teams.live.com/meet/9876543210?p=abc").is_some());
        assert!(url("https://teams.microsoft.com/meetingOptions/?organizerId=1").is_none());
    }

    #[test]
    fn a_webex_link_but_not_the_marketing_site() {
        let link = find(&["https://acme.webex.com/acme/j.php?MTID=m12ab"]).unwrap();
        assert_eq!(link.service, Service::Webex);
        assert!(url("https://acme.webex.com/meet/jane.doe").is_some());
        assert!(url("https://www.webex.com/downloads.html").is_none());
        assert!(url("https://acme.webex.com/").is_none());
    }

    #[test]
    fn a_link_buried_in_long_notes_is_found() {
        let filler = "Agenda item and a very long discussion about the roadmap. ".repeat(80);
        let notes =
            format!("{filler}\nJoin here (https://us02web.zoom.us/j/5551234567).\n{filler}");
        assert_eq!(url(&notes).unwrap(), "https://us02web.zoom.us/j/5551234567");
    }

    #[test]
    fn punctuation_and_markup_around_a_link_are_not_part_of_it() {
        assert_eq!(
            url("<https://zoom.us/j/123456789>").unwrap(),
            "https://zoom.us/j/123456789"
        );
        assert_eq!(
            url("see https://zoom.us/j/123456789.").unwrap(),
            "https://zoom.us/j/123456789"
        );
        assert_eq!(
            url(r#"<a href="https://zoom.us/j/1234?pwd=a&amp;x=1">join</a>"#).unwrap(),
            "https://zoom.us/j/1234?pwd=a&x=1"
        );
        assert_eq!(
            url("HTTPS://Zoom.US/j/123456789").unwrap(),
            "HTTPS://Zoom.US/j/123456789"
        );
    }

    #[test]
    fn only_https_and_only_the_real_host() {
        assert!(url("http://zoom.us/j/123456789").is_none());
        assert!(url("https://zoom.us.evil.com/j/123456789").is_none());
        assert!(url("https://evilzoom.us/j/123456789").is_none());
        assert!(url("https://zoom.us@evil.com/j/123456789").is_none());
        assert!(url("https://evil.com/?next=https://zoom.us/j/1").is_none());
        assert!(url("https://zoom.us:8443/j/123456789").is_none());
        assert!(url("https://zoom.us:443/j/123456789").is_some());
        assert!(url("https://zoom.us/pricing").is_none());
        assert!(url("javascript:alert(1)").is_none());
        assert!(url("https://z\u{43e}om.us/j/1").is_none());
        assert!(url(&format!("https://zoom.us/j/{}", "1".repeat(3000))).is_none());
    }

    #[test]
    fn an_outlook_safe_link_is_unwrapped_and_checked() {
        let wrapped = "https://nam02.safelinks.protection.outlook.com/?url=https%3A%2F%2Fteams.microsoft.com%2Fl%2Fmeetup-join%2F19%253ameeting_x%2F0&data=05";
        assert_eq!(find(&[wrapped]).unwrap().service, Service::Teams);
        let hostile = "https://nam02.safelinks.protection.outlook.com/?url=https%3A%2F%2Fevil.com%2Fj%2F1&data=05";
        assert!(url(hostile).is_none());
    }

    #[test]
    fn the_url_field_wins_over_the_location_and_the_notes() {
        let link = find(&[
            "https://meet.google.com/abc-defg-hij",
            "https://zoom.us/j/111111111",
            "",
        ])
        .unwrap();
        assert_eq!(link.service, Service::Meet);
        assert_eq!(
            find(&["", "Room 4", "https://zoom.us/j/111111111"])
                .unwrap()
                .service,
            Service::Zoom
        );
        assert!(find(&["", "", ""]).is_none());
    }
}
