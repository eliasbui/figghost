//! The LRCLIB client (https://lrclib.net/docs): one GET to `/api/get` through the system's curl.
//!
//! The arguments go to curl on standard input, so the names of the artist and the track never appear in the
//! list of processes. The answer is untrusted: its size, its text and its status are all checked.

use serde_json::Value;
use std::{
    io::{Read, Write},
    process::{Command, Stdio},
};

pub const ENDPOINT: &str = "https://lrclib.net/api/get";
/// LRCLIB asks every client to name itself with a version and a link to the project. There is no public
/// project to link to yet, and an invented link would be worse than none, so only the name and the version
/// are sent; add the link here when the project has one.
pub const USER_AGENT: &str = concat!("Figghost/", env!("CARGO_PKG_VERSION"));

const CURL: &str = "/usr/bin/curl";
const MAX_BODY: u64 = 1 << 20;
const MAX_LYRICS_BYTES: usize = 256 * 1024;
const MAX_NAME_CHARS: usize = 200;
const TIMEOUT_SECONDS: u64 = 10;
const DEFAULT_RETRY_SECONDS: u64 = 60;
const MAX_RETRY_SECONDS: u64 = 3600;

/// The signature of a track as LRCLIB wants it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Query {
    pub artist: String,
    pub title: String,
    pub album: String,
    /// Between 1 and 3600, which is what the server accepts.
    pub duration_secs: Option<u32>,
}

fn name(text: &str) -> String {
    text.chars()
        .filter(|c| !c.is_control())
        .take(MAX_NAME_CHARS)
        .collect::<String>()
        .trim()
        .to_owned()
}

impl Query {
    /// `None` when there is no artist or no title, which the server refuses anyway.
    pub fn new(artist: &str, title: &str, album: &str, duration_ms: u64) -> Option<Self> {
        let (artist, title) = (name(artist), name(title));
        if artist.is_empty() || title.is_empty() {
            return None;
        }
        let secs = (duration_ms + 500) / 1000;
        Some(Self {
            artist,
            title,
            album: name(album),
            duration_secs: (1..=3600)
                .contains(&secs)
                .then(|| u32::try_from(secs).unwrap_or(0)),
        })
    }

    /// A stable identifier of the track for the cache: FNV-1a of the lowercase signature, so no name is ever
    /// written to disk. The duration counts in steps of five seconds, which a re-release can shift by a little.
    pub fn key(&self) -> String {
        let signature = format!(
            "{}\u{1f}{}\u{1f}{}\u{1f}{}",
            self.artist.to_lowercase(),
            self.title.to_lowercase(),
            self.album.to_lowercase(),
            self.duration_secs.map_or(0, |d| (d + 2) / 5)
        );
        let hash = signature.bytes().fold(0xcbf2_9ce4_8422_2325_u64, |h, b| {
            (h ^ u64::from(b)).wrapping_mul(0x100_0000_01b3)
        });
        format!("{hash:016x}")
    }
}

/// What the server knows about a track. Nothing but lyrics is kept.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Lyrics {
    pub synced: Option<String>,
    pub plain: Option<String>,
    pub instrumental: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Outcome {
    Found(Lyrics),
    /// The server looked and has nothing.
    NotFound,
    /// Not an answer about the track (no network, rate limit, server trouble): ask again after this long.
    Retry {
        after_secs: u64,
    },
}

fn lyrics_text(value: &Value) -> Option<String> {
    let text = value.as_str()?.trim();
    (!text.is_empty() && text.len() <= MAX_LYRICS_BYTES).then(|| text.to_owned())
}

/// Turns an HTTP answer into an outcome. `retry_after` is the `Retry-After` header in seconds, if it came.
pub fn interpret(status: u16, retry_after: Option<u64>, body: &str) -> Outcome {
    let retry = |default: u64| Outcome::Retry {
        after_secs: retry_after.unwrap_or(default).clamp(1, MAX_RETRY_SECONDS),
    };
    match status {
        200 => {
            let Ok(Value::Object(record)) = serde_json::from_str::<Value>(body) else {
                // A captive portal or a proxy can answer 200 with a page; that says nothing about the track.
                return retry(300);
            };
            let lyrics = Lyrics {
                synced: record.get("syncedLyrics").and_then(lyrics_text),
                plain: record.get("plainLyrics").and_then(lyrics_text),
                instrumental: record.get("instrumental") == Some(&Value::Bool(true)),
            };
            if lyrics.synced.is_none() && lyrics.plain.is_none() && !lyrics.instrumental {
                Outcome::NotFound
            } else {
                Outcome::Found(lyrics)
            }
        }
        400 | 404 => Outcome::NotFound,
        429 | 503 => retry(DEFAULT_RETRY_SECONDS),
        _ => retry(300),
    }
}

/// A value for a curl configuration file: quoted, with the characters that mean something escaped.
fn quote(value: &str) -> String {
    let mut out = String::from("\"");
    for c in value.chars() {
        match c {
            '\\' => out.push_str("\\\\"),
            '"' => out.push_str("\\\""),
            c if c.is_control() => {}
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

fn request_config(url: &str, query: &Query, user_agent: &str) -> String {
    let mut config = format!("url = {}\nuser-agent = {}\n", quote(url), quote(user_agent));
    let mut field = |key: &str, value: &str| {
        config.push_str(&format!(
            "data-urlencode = {}\n",
            quote(&format!("{key}={value}"))
        ));
    };
    field("artist_name", &query.artist);
    field("track_name", &query.title);
    if !query.album.is_empty() {
        field("album_name", &query.album);
    }
    if let Some(duration) = query.duration_secs {
        field("duration", &duration.to_string());
    }
    config
}

/// Asks `url` (the endpoint; a loopback address may be plain http, for the tests) about one track.
pub fn fetch(url: &str, query: &Query, user_agent: &str) -> Outcome {
    let retry = Outcome::Retry {
        after_secs: DEFAULT_RETRY_SECONDS,
    };
    let protocol = if url.starts_with("http://127.0.0.1") {
        "=http"
    } else {
        "=https"
    };
    let Ok(mut child) = Command::new(CURL)
        .args([
            "--silent",
            "--get",
            "--globoff",
            "--max-time",
            &TIMEOUT_SECONDS.to_string(),
            "--proto",
            protocol,
            "--max-filesize",
            &MAX_BODY.to_string(),
            "--write-out",
            "\n%{http_code} %header{retry-after}",
            "--config",
            "-",
        ])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
    else {
        return retry;
    };
    if let Some(mut stdin) = child.stdin.take() {
        // A write that fails means curl has already stopped; the missing status below says so.
        let _ = stdin.write_all(request_config(url, query, user_agent).as_bytes());
    }
    let mut out = Vec::new();
    if let Some(stdout) = child.stdout.take() {
        let _ = stdout.take(MAX_BODY + 64).read_to_end(&mut out);
    }
    if out.len() as u64 > MAX_BODY + 16 {
        let _ = child.kill();
    }
    let _ = child.wait();

    let Ok(text) = String::from_utf8(out) else {
        return retry;
    };
    let Some((body, trailer)) = text.rsplit_once('\n') else {
        return retry;
    };
    let mut parts = trailer.split_whitespace();
    let Some(status) = parts.next().and_then(|s| s.parse::<u16>().ok()) else {
        return retry;
    };
    let retry_after = parts.next().and_then(|s| s.parse::<u64>().ok());
    interpret(status, retry_after, body)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        io::{BufRead, BufReader},
        net::{TcpListener, TcpStream},
        thread,
    };

    const RECORD: &str = r#"{"id":3396226,"trackName":"I Want to Live","artistName":"Borislav Slavov",
        "albumName":"Baldur's Gate 3","duration":233.0,"instrumental":false,
        "plainLyrics":"Line one\nLine two","syncedLyrics":"[00:01.00]Line one\n[00:03.50]Line two"}"#;

    /// Serves one request on a free loopback port; returns the endpoint and a handle that gives back the
    /// request line and the headers that were received.
    fn serve(
        status: &str,
        extra_headers: &str,
        body: &str,
    ) -> (String, thread::JoinHandle<(String, String)>) {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let url = format!(
            "http://127.0.0.1:{}/api/get",
            listener.local_addr().unwrap().port()
        );
        let response = format!(
            "HTTP/1.1 {status}\r\n{extra_headers}Content-Length: {}\r\nConnection: close\r\n\r\n{body}",
            body.len()
        );
        let handle = thread::spawn(move || {
            let (mut stream, _): (TcpStream, _) = listener.accept().unwrap();
            let mut reader = BufReader::new(stream.try_clone().unwrap());
            let mut first = String::new();
            reader.read_line(&mut first).unwrap();
            let mut headers = String::new();
            loop {
                let mut line = String::new();
                if reader.read_line(&mut line).unwrap_or(0) == 0 || line == "\r\n" {
                    break;
                }
                headers.push_str(&line);
            }
            let _ = stream.write_all(response.as_bytes());
            (first, headers)
        });
        (url, handle)
    }

    fn query() -> Query {
        Query::new(
            "Borislav Slavov",
            "I Want to Live & \"more\"",
            "Baldur's Gate 3",
            233_400,
        )
        .unwrap()
    }

    #[test]
    fn a_found_record_comes_back_and_the_request_names_the_app() {
        let (url, server) = serve("200 OK", "Content-Type: application/json\r\n", RECORD);
        let outcome = fetch(&url, &query(), USER_AGENT);
        let Outcome::Found(lyrics) = outcome else {
            panic!("{outcome:?}")
        };
        assert!(lyrics.synced.unwrap().starts_with("[00:01.00]Line one"));
        assert_eq!(lyrics.plain.as_deref(), Some("Line one\nLine two"));
        let (request, headers) = server.join().unwrap();
        let target = request.split(' ').nth(1).unwrap();
        assert!(target.starts_with("/api/get?artist_name=Borislav+Slavov&track_name=I+Want+to+Live+%26+%22more%22&album_name=Baldur%27s+Gate+3&duration=233"), "{target}");
        assert!(
            headers
                .to_lowercase()
                .contains(&format!("user-agent: {}", USER_AGENT.to_lowercase())),
            "{headers}"
        );
    }

    #[test]
    fn a_missing_track_is_not_found() {
        let body =
            r#"{"code":404,"name":"TrackNotFound","message":"Failed to find specified track"}"#;
        let (url, server) = serve("404 Not Found", "", body);
        assert_eq!(fetch(&url, &query(), USER_AGENT), Outcome::NotFound);
        server.join().unwrap();
    }

    #[test]
    fn rate_limits_are_honoured_and_never_remembered_as_missing() {
        let (url, server) = serve("429 Too Many Requests", "Retry-After: 42\r\n", "slow down");
        assert_eq!(
            fetch(&url, &query(), USER_AGENT),
            Outcome::Retry { after_secs: 42 }
        );
        server.join().unwrap();
        assert_eq!(
            interpret(429, Some(999_999), ""),
            Outcome::Retry {
                after_secs: MAX_RETRY_SECONDS
            }
        );
        assert_eq!(
            interpret(503, None, ""),
            Outcome::Retry {
                after_secs: DEFAULT_RETRY_SECONDS
            }
        );
        assert!(matches!(interpret(500, None, "x"), Outcome::Retry { .. }));
    }

    #[test]
    fn garbage_is_not_trusted() {
        let (url, server) = serve("200 OK", "", "<html>sign in to the wifi</html>");
        assert!(matches!(
            fetch(&url, &query(), USER_AGENT),
            Outcome::Retry { .. }
        ));
        server.join().unwrap();
        for body in ["", "null", "[]", "\"text\"", "{\"syncedLyrics\":5}", "{}"] {
            let got = interpret(200, None, body);
            assert!(
                matches!(got, Outcome::NotFound | Outcome::Retry { .. }),
                "{body}: {got:?}"
            );
        }
    }

    #[test]
    fn no_server_means_retry_later() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let url = format!(
            "http://127.0.0.1:{}/api/get",
            listener.local_addr().unwrap().port()
        );
        drop(listener);
        assert!(matches!(
            fetch(&url, &query(), USER_AGENT),
            Outcome::Retry { .. }
        ));
    }

    #[test]
    fn only_https_leaves_the_machine() {
        // A plain http address that is not the loopback is refused by curl before anything is sent.
        let q = query();
        assert!(matches!(
            fetch("http://example.invalid/api/get", &q, USER_AGENT),
            Outcome::Retry { .. }
        ));
    }

    #[test]
    fn records_are_read_defensively() {
        let only_plain = interpret(
            200,
            None,
            r#"{"plainLyrics":"la la","syncedLyrics":null,"instrumental":false}"#,
        );
        assert_eq!(
            only_plain,
            Outcome::Found(Lyrics {
                synced: None,
                plain: Some("la la".into()),
                instrumental: false
            })
        );
        let instrumental = interpret(
            200,
            None,
            r#"{"plainLyrics":null,"syncedLyrics":null,"instrumental":true}"#,
        );
        assert_eq!(
            instrumental,
            Outcome::Found(Lyrics {
                instrumental: true,
                ..Lyrics::default()
            })
        );
        assert_eq!(
            interpret(
                200,
                None,
                r#"{"plainLyrics":"  ","syncedLyrics":"","instrumental":false}"#
            ),
            Outcome::NotFound
        );
        let huge = format!(
            r#"{{"plainLyrics":"{}"}}"#,
            "x".repeat(MAX_LYRICS_BYTES + 1)
        );
        assert_eq!(interpret(200, None, &huge), Outcome::NotFound);
        assert_eq!(
            interpret(400, None, "Failed to deserialize"),
            Outcome::NotFound
        );
    }

    #[test]
    fn a_response_over_the_limit_is_dropped() {
        let big = "x".repeat(2 << 20);
        let (url, server) = serve("200 OK", "", &big);
        assert!(matches!(
            fetch(&url, &query(), USER_AGENT),
            Outcome::Retry { .. }
        ));
        let _ = server.join();
    }

    #[test]
    fn queries_are_normalised_and_keys_are_stable() {
        assert!(Query::new("", "Song", "", 1000).is_none());
        assert!(Query::new("Band", "  ", "", 1000).is_none());
        let q = Query::new("Band\u{0}\n", " Song ", "", 233_600).unwrap();
        assert_eq!((q.artist.as_str(), q.title.as_str()), ("Band", "Song"));
        assert_eq!(q.duration_secs, Some(234));
        assert_eq!(Query::new("a", "b", "", 0).unwrap().duration_secs, None);
        assert_eq!(
            Query::new("a", "b", "", 4_000_000).unwrap().duration_secs,
            None
        );
        assert_eq!(
            Query::new(&"a".repeat(900), "b", "", 0)
                .unwrap()
                .artist
                .len(),
            MAX_NAME_CHARS
        );
        let key = |d| Query::new("Band", "Song", "Album", d).unwrap().key();
        assert_eq!(key(200_000), key(201_000));
        assert_ne!(key(200_000), key(260_000));
        assert_eq!(
            Query::new("BAND", "song", "album", 200_000).unwrap().key(),
            key(200_000)
        );
        assert_eq!(key(200_000).len(), 16);
        assert_eq!(key(200_000), "6a7a7a389f97031f");
    }
}
