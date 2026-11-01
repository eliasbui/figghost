//! What one connection really does, measured with the system's `curl` bound to that connection (`--interface`) and told
//! to ignore every proxy: whether the web opens through it, whether secure sites (HTTPS) open through it, how fast a
//! download and an upload go and how long it takes to connect. The reading of what `curl` prints is here and pure;
//! running it is `Feature::speed_test` and `Feature::check_web`.
//!
//! `networkQuality`, which the first version used, goes through the proxy settings of the system, which are Figghost's own
//! proxy while the wires are on, and it fails for a bound interface behind some networks (`-1009` and `bad URL`), so it
//! said a connection that works could not be tested.

use serde_json::{Value, json};

/// A page that answers `Success` where the internet is open and something else behind a sign-in; every Apple device asks it.
pub const HTTP_CHECK: &str = "http://captive.apple.com/hotspot-detect.html";
/// A small page over HTTPS, to see whether secure connections go through unchanged.
pub const HTTPS_CHECK: &str = "https://www.apple.com/library/test/success.html";
/// The test of the speed: bytes sent on request, and a place that takes what is sent.
pub const DOWNLOAD: &str = "https://speed.cloudflare.com/__down?bytes=40000000";
pub const UPLOAD: &str = "https://speed.cloudflare.com/__up";
/// The same without TLS, for a connection where secure sites do not open: it measures the line, not what the network
/// does to secure traffic, which the test says apart.
pub const DOWNLOAD_PLAIN: &str = "http://speed.cloudflare.com/__down?bytes=40000000";
pub const UPLOAD_PLAIN: &str = "http://speed.cloudflare.com/__up";
/// How much an upload sends at most.
pub const UPLOAD_BYTES: u64 = 12_000_000;
/// What `curl` prints after the page: its own exit code, the status, the bytes and speed each way, and the time to connect.
pub const WRITE_OUT: &str = "\n%{exitcode} %{http_code} %{size_download} %{speed_download} %{size_upload} %{speed_upload} %{time_connect}";

/// What one run of `curl` says.
#[derive(Debug, Clone, PartialEq)]
pub struct Run {
    pub exit: i32,
    pub http: u32,
    pub down_bytes: u64,
    /// Bytes a second.
    pub down_speed: f64,
    pub up_bytes: u64,
    pub up_speed: f64,
    pub connect_ms: f64,
}

/// The arguments that bind `curl` to an interface, keep it out of every proxy and make it print `WRITE_OUT`; `extra` goes
/// before the address. The interface is checked by the caller to be a name (`rules::valid_interface`).
pub fn args(interface: &str, seconds: u64, extra: &[&str], url: &str) -> Vec<String> {
    let mut args: Vec<String> = [
        "-s",
        "--interface",
        interface,
        "--noproxy",
        "*",
        "--max-time",
    ]
    .map(String::from)
    .to_vec();
    args.push(seconds.to_string());
    args.extend(["-w", WRITE_OUT].map(String::from));
    args.extend(extra.iter().map(|a| (*a).to_owned()));
    args.push(url.to_owned());
    args
}

/// The page, and what `curl` printed after it; `None` when the end is not what `WRITE_OUT` makes.
pub fn read(output: &str) -> Option<(String, Run)> {
    let (body, tail) = output.rsplit_once('\n')?;
    let mut fields = tail.split_whitespace();
    let exit = fields.next()?.parse().ok()?;
    let http = fields.next()?.parse().ok()?;
    let down_bytes = fields.next()?.parse().ok()?;
    let down_speed: f64 = fields.next()?.parse().ok()?;
    let up_bytes = fields.next()?.parse().ok()?;
    let up_speed: f64 = fields.next()?.parse().ok()?;
    let connect: f64 = fields.next()?.parse().ok()?;
    let sane = |n: f64| n.is_finite() && n >= 0.0;
    (fields.next().is_none() && sane(down_speed) && sane(up_speed) && sane(connect)).then(|| {
        (
            body.to_owned(),
            Run {
                exit,
                http,
                down_bytes,
                down_speed,
                up_bytes,
                up_speed,
                connect_ms: connect * 1000.0,
            },
        )
    })
}

/// What the plain web is like through a connection.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Http {
    Open,
    /// A page that is not the one that was asked for: the network wants a sign-in.
    Login,
    Closed,
}

impl Http {
    pub fn id(self) -> &'static str {
        match self {
            Http::Open => "ok",
            Http::Login => "login",
            Http::Closed => "none",
        }
    }
}

pub fn http_of(body: &str, run: Option<&Run>) -> Http {
    match run {
        Some(r) if r.exit == 0 && (200..300).contains(&r.http) => {
            if body.contains("Success") || body.trim().is_empty() {
                Http::Open
            } else {
                Http::Login
            }
        }
        _ => Http::Closed,
    }
}

/// What secure sites are like through a connection.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Https {
    Open,
    /// The certificate was not accepted: a network that inspects secure traffic, or one that is not what it says.
    Certificate,
    Closed,
}

impl Https {
    pub fn id(self) -> &'static str {
        match self {
            Https::Open => "ok",
            Https::Certificate => "tls",
            Https::Closed => "fail",
        }
    }
}

/// The exit codes of `curl` that are a secure connection not made: a handshake that failed, a certificate that was not
/// accepted or one that is not valid.
fn is_certificate(exit: i32) -> bool {
    matches!(
        exit,
        35 | 51 | 53 | 54 | 58 | 59 | 60 | 66 | 77 | 80 | 82 | 83 | 90 | 91
    )
}

pub fn https_of(run: Option<&Run>) -> Https {
    match run {
        Some(r) if r.exit == 0 && (200..400).contains(&r.http) => Https::Open,
        Some(r) if is_certificate(r.exit) => Https::Certificate,
        _ => Https::Closed,
    }
}

/// The two checks as the page gets them.
pub fn web(http: Http, https: Https) -> Value {
    json!({ "http": http.id(), "https": https.id() })
}

/// What a finished test found.
#[derive(Debug, Clone, PartialEq)]
pub struct Speed {
    pub down_bps: u64,
    pub up_bps: Option<u64>,
    pub rtt_ms: u32,
    pub http: Http,
    pub https: Https,
    /// The speed was measured without TLS, since secure sites do not open through this connection.
    pub plain: bool,
}

/// A download that moved anything gives its speed in bits a second; `None` for one that moved nothing.
pub fn down_of(run: &Run) -> Option<u64> {
    (run.down_bytes > 0 && run.down_speed > 0.0).then_some((run.down_speed * 8.0) as u64)
}

/// An upload that sent anything; one that sent nothing is not a speed of nothing.
pub fn up_of(run: &Run) -> Option<u64> {
    (run.up_bytes > 0 && run.up_speed > 0.0).then_some((run.up_speed * 8.0) as u64)
}

/// Why a test that measured nothing did not: the page the network shows instead (`login`), no way out at all (`offline`),
/// a certificate that was not accepted (`tls`), a connection that moved nothing in the time (`slow`), or something else
/// (`failed`).
pub fn why(http: Http, download: Option<&Run>) -> &'static str {
    match (http, download) {
        (Http::Login, _) => "login",
        (Http::Closed, _) => "offline",
        (_, Some(r)) if is_certificate(r.exit) => "tls",
        (_, Some(r)) if r.exit == 28 => "slow",
        _ => "failed",
    }
}

pub fn done(speed: &Speed, at_ms: i64) -> Value {
    json!({
        "state": "done", "down": speed.down_bps, "up": speed.up_bps, "rtt": speed.rtt_ms,
        "web": speed.http.id(), "https": speed.https.id(), "plain": speed.plain, "at": at_ms,
    })
}

pub fn failed(reason: &str, http: Option<Http>, https: Option<Https>) -> Value {
    json!({ "state": "failed", "reason": reason, "web": http.map(Http::id), "https": https.map(Https::id) })
}

pub fn running() -> Value {
    json!({ "state": "running" })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run(exit: i32, http: u32, down: u64, speed: f64) -> Run {
        Run {
            exit,
            http,
            down_bytes: down,
            down_speed: speed,
            up_bytes: 0,
            up_speed: 0.0,
            connect_ms: 40.0,
        }
    }

    #[test]
    fn the_arguments_bind_the_interface_and_leave_every_proxy() {
        let a = args("en1", 12, &["-o", "/dev/null"], "https://example.com/x");
        assert_eq!(&a[..3], ["-s", "--interface", "en1"]);
        assert!(a.windows(2).any(|w| w == ["--noproxy", "*"]));
        assert!(a.windows(2).any(|w| w == ["--max-time", "12"]));
        assert_eq!(a.last().unwrap(), "https://example.com/x");
        assert!(a.iter().any(|x| x == WRITE_OUT) && a.iter().any(|x| x == "-o"));
    }

    #[test]
    fn what_curl_prints_after_the_page_is_read_and_a_page_with_lines_is_kept_whole() {
        let out = "<html>\nSuccess\n</html>\n0 200 69 679.0 0 0.0 0.033510";
        let (body, r) = read(out).unwrap();
        assert_eq!(body, "<html>\nSuccess\n</html>");
        assert_eq!((r.exit, r.http, r.down_bytes), (0, 200, 69));
        assert!((r.connect_ms - 33.51).abs() < 0.01);
        for bad in [
            "",
            "no newline",
            "\n0 200 1 2",
            "\n0 200 1 2 3 4 5 6",
            "\nx 200 1 2 3 4 5",
            "\n0 200 1 -2 3 4 5",
            "\n0 200 1 nan 3 4 5",
        ] {
            assert!(read(bad).is_none(), "{bad:?}");
        }
    }

    #[test]
    fn the_plain_web_is_open_asks_a_sign_in_or_is_closed() {
        let ok = run(0, 200, 69, 100.0);
        assert_eq!(
            http_of("<HTML><BODY>Success</BODY></HTML>", Some(&ok)),
            Http::Open
        );
        assert_eq!(
            http_of("<html>Please sign in to the hotel Wi-Fi</html>", Some(&ok)),
            Http::Login
        );
        assert_eq!(
            http_of("", Some(&ok)),
            Http::Open,
            "no page and no error says nothing against it"
        );
        assert_eq!(http_of("Success", Some(&run(7, 0, 0, 0.0))), Http::Closed);
        assert_eq!(http_of("Success", Some(&run(0, 500, 5, 1.0))), Http::Closed);
        assert_eq!(http_of("Success", None), Http::Closed);
    }

    #[test]
    fn secure_sites_open_fail_on_a_certificate_or_fail() {
        assert_eq!(https_of(Some(&run(0, 200, 68, 1.0))), Https::Open);
        assert_eq!(
            https_of(Some(&run(0, 301, 0, 0.0))),
            Https::Open,
            "a redirect is an answer"
        );
        // What this Mac's curl said through its Wi-Fi, where the network opens secure connections and signs again.
        assert_eq!(https_of(Some(&run(60, 0, 0, 0.0))), Https::Certificate);
        assert_eq!(https_of(Some(&run(35, 0, 0, 0.0))), Https::Certificate);
        assert_eq!(https_of(Some(&run(7, 0, 0, 0.0))), Https::Closed);
        assert_eq!(https_of(Some(&run(28, 0, 0, 0.0))), Https::Closed);
        assert_eq!(https_of(None), Https::Closed);
    }

    #[test]
    fn a_speed_is_in_bits_and_a_transfer_that_moved_nothing_is_none() {
        assert_eq!(
            down_of(&run(0, 200, 40_000_000, 37_925_361.0)),
            Some(303_402_888)
        );
        assert_eq!(down_of(&run(28, 0, 0, 0.0)), None);
        assert_eq!(down_of(&run(28, 200, 1000, 0.0)), None);
        let mut up = run(0, 200, 0, 0.0);
        assert_eq!(up_of(&up), None);
        up.up_bytes = 12_000_000;
        up.up_speed = 9_469_656.0;
        assert_eq!(up_of(&up), Some(75_757_248));
        // A download that is cut short by the time limit still tells how fast it went.
        assert!(down_of(&run(28, 200, 15_000_000, 2_500_000.0)).is_some());
    }

    #[test]
    fn a_test_that_measured_nothing_says_why() {
        assert_eq!(why(Http::Login, None), "login");
        assert_eq!(why(Http::Closed, Some(&run(0, 200, 1, 1.0))), "offline");
        assert_eq!(why(Http::Open, Some(&run(60, 0, 0, 0.0))), "tls");
        assert_eq!(why(Http::Open, Some(&run(28, 0, 0, 0.0))), "slow");
        assert_eq!(why(Http::Open, Some(&run(22, 404, 0, 0.0))), "failed");
        assert_eq!(why(Http::Open, None), "failed");
    }

    #[test]
    fn the_results_are_what_the_page_reads() {
        let s = Speed {
            down_bps: 8_000_000,
            up_bps: None,
            rtt_ms: 35,
            http: Http::Open,
            https: Https::Certificate,
            plain: true,
        };
        let v = done(&s, 7);
        assert_eq!(
            (
                v["state"].clone(),
                v["down"].clone(),
                v["up"].clone(),
                v["https"].clone()
            ),
            (json!("done"), json!(8_000_000), json!(null), json!("tls"))
        );
        assert_eq!(v["plain"], json!(true));
        let f = failed("tls", Some(Http::Open), None);
        assert_eq!(
            (f["reason"].clone(), f["web"].clone(), f["https"].clone()),
            (json!("tls"), json!("ok"), json!(null))
        );
        assert_eq!(running()["state"], json!("running"));
        assert_eq!(
            web(Http::Login, Https::Closed),
            json!({ "http": "login", "https": "fail" })
        );
    }
}
