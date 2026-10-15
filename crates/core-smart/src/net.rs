//! One GET request through the system's curl. It reads its arguments from standard input, so the text that is
//! being translated never appears in the list of processes, and it only talks https (loopback http is for
//! the tests).

use std::{
    io::{Read, Write},
    process::{Command, Stdio},
};

const CURL: &str = "/usr/bin/curl";
const MAX_BODY: u64 = 1 << 20;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NetError {
    /// The system's curl is missing.
    NoCurl,
    /// No connection, or no answer in time.
    Unreachable,
    /// The server answered with an error status.
    Status,
    TooLarge,
    Failed(String),
}

/// A value for a curl configuration file: quoted, with the characters that mean something escaped.
fn quote(value: &str) -> String {
    let mut out = String::from("\"");
    for c in value.chars() {
        match c {
            '\\' => out.push_str("\\\\"),
            '"' => out.push_str("\\\""),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            '\u{b}' => out.push_str("\\v"),
            c if c.is_control() => {}
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

fn config(url: &str, query: &[(&str, String)]) -> String {
    let mut config = format!("url = {}\n", quote(url));
    for (name, value) in query {
        config.push_str(&format!(
            "data-urlencode = {}\n",
            quote(&format!("{name}={value}"))
        ));
    }
    config
}

/// The body of `url` with `query` added to it, at most a mebibyte, within `seconds`.
pub fn get(url: &str, query: &[(&str, String)], seconds: u64) -> Result<String, NetError> {
    let protocol = if url.starts_with("http://127.0.0.1") {
        "=http"
    } else {
        "=https"
    };
    let mut child = Command::new(CURL)
        .args([
            "--silent",
            "--show-error",
            "--fail",
            "--get",
            "--globoff",
            "--max-time",
            &seconds.to_string(),
            "--proto",
            protocol,
            "--max-filesize",
            &MAX_BODY.to_string(),
            "--config",
            "-",
        ])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|e| {
            if e.kind() == std::io::ErrorKind::NotFound {
                NetError::NoCurl
            } else {
                NetError::Failed(e.to_string())
            }
        })?;
    if let Some(mut stdin) = child.stdin.take() {
        // A write that fails means curl has already stopped; its exit status tells why.
        let _ = stdin.write_all(config(url, query).as_bytes());
    }
    let mut body = Vec::new();
    if let Some(stdout) = child.stdout.take() {
        let _ = stdout.take(MAX_BODY + 1).read_to_end(&mut body);
    }
    if body.len() as u64 > MAX_BODY {
        let _ = child.kill();
        let _ = child.wait();
        return Err(NetError::TooLarge);
    }
    let status = child.wait().map_err(|e| NetError::Failed(e.to_string()))?;
    match status.code() {
        Some(0) => String::from_utf8(body).map_err(|_| NetError::Failed("not text".into())),
        // Could not resolve, could not connect, timed out, no response, TLS trouble.
        Some(5..=7 | 28 | 35 | 52 | 56 | 60) => Err(NetError::Unreachable),
        Some(22) => Err(NetError::Status),
        Some(63) => Err(NetError::TooLarge),
        Some(code) => Err(NetError::Failed(format!("curl exited with {code}"))),
        None => Err(NetError::Failed("curl was stopped".into())),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        io::{BufRead, BufReader},
        net::{TcpListener, TcpStream},
        thread,
        time::{Duration, Instant},
    };

    /// Serves one request on a free loopback port and hands back the request line that was received.
    fn serve(
        respond: impl FnOnce(&mut TcpStream) + Send + 'static,
    ) -> (String, thread::JoinHandle<String>) {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let url = format!(
            "http://127.0.0.1:{}/get",
            listener.local_addr().unwrap().port()
        );
        let handle = thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            let mut reader = BufReader::new(stream.try_clone().unwrap());
            let mut first = String::new();
            reader.read_line(&mut first).unwrap();
            loop {
                let mut line = String::new();
                if reader.read_line(&mut line).unwrap_or(0) == 0 || line == "\r\n" {
                    break;
                }
            }
            respond(&mut stream);
            first
        });
        (url, handle)
    }

    fn answer(status: &str, body: &str) -> impl FnOnce(&mut TcpStream) + Send + 'static {
        let response = format!(
            "HTTP/1.1 {status}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
            body.len()
        );
        move |stream| {
            let _ = stream.write_all(response.as_bytes());
        }
    }

    fn percent_decode(text: &str) -> String {
        let bytes = text.as_bytes();
        let mut out = Vec::new();
        let mut i = 0;
        while i < bytes.len() {
            if bytes[i] == b'%' {
                out.push(u8::from_str_radix(&text[i + 1..i + 3], 16).unwrap());
                i += 3;
            } else {
                out.push(if bytes[i] == b'+' { b' ' } else { bytes[i] });
                i += 1;
            }
        }
        String::from_utf8(out).unwrap()
    }

    #[test]
    fn the_text_travels_whole_and_the_body_comes_back() {
        let (url, server) = serve(answer("200 OK", r#"{"ok":true}"#));
        let text = "He said \"hi\"\nline two \\ \u{1ec7} & more=1";
        let body = get(
            &url,
            &[
                ("q", text.to_owned()),
                ("langpair", "Autodetect|vi".to_owned()),
            ],
            10,
        )
        .unwrap();
        assert_eq!(body, r#"{"ok":true}"#);
        let request = server.join().unwrap();
        let target = request.split(' ').nth(1).unwrap();
        let query = target.strip_prefix("/get?").unwrap();
        let params: Vec<(String, String)> = query
            .split('&')
            .map(|pair| {
                let (k, v) = pair.split_once('=').unwrap();
                (percent_decode(k), percent_decode(v))
            })
            .collect();
        assert_eq!(params[0], ("q".to_owned(), text.to_owned()));
        assert_eq!(
            params[1],
            ("langpair".to_owned(), "Autodetect|vi".to_owned())
        );
        assert_eq!(
            params.len(),
            2,
            "an ampersand in the text stays in the text"
        );
    }

    #[test]
    fn an_error_status_is_an_error() {
        let (url, server) = serve(answer("404 Not Found", "nope"));
        assert_eq!(get(&url, &[], 10), Err(NetError::Status));
        server.join().unwrap();
    }

    #[test]
    fn nobody_listening_is_unreachable() {
        let port = TcpListener::bind("127.0.0.1:0")
            .unwrap()
            .local_addr()
            .unwrap()
            .port();
        let url = format!("http://127.0.0.1:{port}/get");
        assert_eq!(get(&url, &[], 5), Err(NetError::Unreachable));
    }

    #[test]
    fn a_server_that_never_answers_is_given_up_on() {
        let (url, server) = serve(|_| thread::sleep(Duration::from_millis(2500)));
        let started = Instant::now();
        assert_eq!(get(&url, &[], 1), Err(NetError::Unreachable));
        assert!(started.elapsed() < Duration::from_secs(2));
        server.join().unwrap();
    }

    #[test]
    fn a_huge_answer_is_cut_off() {
        let (url, server) = serve(|stream| {
            let _ = stream.write_all(b"HTTP/1.1 200 OK\r\nConnection: close\r\n\r\n");
            let chunk = vec![b'x'; 64 * 1024];
            for _ in 0..40 {
                if stream.write_all(&chunk).is_err() {
                    break;
                }
            }
        });
        assert_eq!(get(&url, &[], 10), Err(NetError::TooLarge));
        server.join().unwrap();
    }

    #[test]
    fn only_https_is_spoken_beyond_loopback() {
        // Plain http to anywhere else is refused by curl itself before any connection is made.
        assert!(matches!(
            get("http://example.invalid/", &[], 5),
            Err(NetError::Failed(_))
        ));
        assert!(matches!(
            get("ftp://example.invalid/", &[], 5),
            Err(NetError::Failed(_))
        ));
    }

    #[test]
    fn values_are_quoted_for_a_curl_file() {
        assert_eq!(quote("a\"b\\c\nd\te\u{7}"), "\"a\\\"b\\\\c\\nd\\te\"");
        assert_eq!(
            config("https://x/y", &[("q", "hi".to_owned())]),
            "url = \"https://x/y\"\ndata-urlencode = \"q=hi\"\n"
        );
    }
}
