//! The request itself: curl run as a child with the body and the headers on its standard input, its output read
//! line by line, and a flag that stops it at once.

use crate::wire::{self, Event, Kind, Message};
use core_smart::net::quote;
use std::{
    io::{BufRead, BufReader, Write},
    process::{Child, Command, Stdio},
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
    thread,
    time::Duration,
};

const CURL: &str = "/usr/bin/curl";
/// How often the flag is looked at while the answer is being waited for.
const WATCH: Duration = Duration::from_millis(40);

/// Raised from any thread to stop the answer.
pub type Cancel = Arc<AtomicBool>;

#[derive(Debug, Clone)]
pub struct Source {
    pub kind: Kind,
    /// The full address of the endpoint. Only https, or http to this machine.
    pub url: String,
    /// Empty for a model on this machine; for Codex the access token.
    pub key: String,
    /// The ChatGPT account of a Codex sign-in; empty otherwise.
    pub account: String,
    pub model: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Failure {
    NoCurl,
    /// No connection, or no answer in time.
    Unreachable,
    /// The service refused or failed, in its own words.
    Service(String),
    /// Stopped by the person.
    Cancelled,
    /// The stream ended with nothing in it.
    Empty,
}

fn config(source: &Source, system: &str, messages: &[Message]) -> String {
    let mut out = format!("url = {}\nrequest = \"POST\"\n", quote(&source.url));
    for (name, value) in wire::headers(source.kind, &source.key, &source.account) {
        out.push_str(&format!("header = {}\n", quote(&format!("{name}: {value}"))));
    }
    let body = wire::body(source.kind, &source.model, system, messages);
    out.push_str(&format!("data = {}\n", quote(&body)));
    out
}

fn allowed(url: &str) -> bool {
    url.starts_with("https://") || url.starts_with("http://127.0.0.1") || url.starts_with("http://localhost")
}

/// Asks `source` and calls `on_text` with each piece of the answer as it arrives. Returns the whole answer.
/// `seconds` bounds the whole call, the time spent writing included.
pub fn stream(
    source: &Source,
    system: &str,
    messages: &[Message],
    seconds: u64,
    cancel: &Cancel,
    on_text: &mut dyn FnMut(&str),
) -> Result<String, Failure> {
    if !allowed(&source.url) {
        return Err(Failure::Service("only https addresses are used".into()));
    }
    if cancel.load(Ordering::SeqCst) {
        return Err(Failure::Cancelled);
    }
    let proto = if source.url.starts_with("https://") { "=https" } else { "=http" };
    let mut child = Command::new(CURL)
        .args(["--silent", "--show-error", "--no-buffer", "--max-time", &seconds.to_string(), "--proto", proto, "--config", "-"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|e| {
            if e.kind() == std::io::ErrorKind::NotFound { Failure::NoCurl } else { Failure::Service(e.to_string()) }
        })?;
    if let Some(mut stdin) = child.stdin.take() {
        // A write that fails means curl has already stopped; its exit status tells why.
        let _ = stdin.write_all(config(source, system, messages).as_bytes());
    }
    let stdout = child.stdout.take().expect("stdout is piped");
    let child: Arc<Mutex<Child>> = Arc::new(Mutex::new(child));
    let finished = Arc::new(AtomicBool::new(false));
    let watcher = {
        let (child, cancel, finished) = (child.clone(), cancel.clone(), finished.clone());
        thread::spawn(move || {
            while !finished.load(Ordering::SeqCst) {
                if cancel.load(Ordering::SeqCst) {
                    let _ = child.lock().unwrap().kill();
                    return;
                }
                thread::sleep(WATCH);
            }
        })
    };

    let mut answer = String::new();
    let mut error = None;
    // What a service says instead of a stream when it refuses: a plain JSON body.
    let mut plain = String::new();
    for line in BufReader::new(stdout).lines() {
        let Ok(line) = line else { break };
        if plain.len() < 4096 && !line.starts_with("data:") && !line.starts_with("event:") && !line.starts_with(':') {
            plain.push_str(&line);
        }
        match wire::read_line(source.kind, &line) {
            Event::Text(piece) => {
                on_text(&piece);
                answer.push_str(&piece);
            }
            Event::Error(message) => error = Some(message),
            Event::Done => break,
            Event::Nothing => {}
        }
    }
    finished.store(true, Ordering::SeqCst);
    let _ = watcher.join();
    let status = {
        let mut child = child.lock().unwrap();
        // Stopping at the end of the stream leaves curl to finish by itself; one that has not is not waited for.
        let _ = child.kill();
        child.wait()
    };

    if cancel.load(Ordering::SeqCst) {
        return Err(Failure::Cancelled);
    }
    if let Some(message) = error {
        return Err(Failure::Service(message));
    }
    if answer.is_empty() && let Some(why) = wire::refusal(&plain) {
        return Err(Failure::Service(why));
    }
    if answer.is_empty() {
        // Nothing was said: curl's own exit code says whether the service could be reached at all.
        return match status.ok().and_then(|s| s.code()) {
            Some(0) | None => Err(Failure::Empty),
            Some(_) => Err(Failure::Unreachable),
        };
    }
    Ok(answer)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        io::Read,
        net::{TcpListener, TcpStream},
        sync::atomic::AtomicUsize,
        time::Instant,
    };

    /// Serves one request on a loopback port; hands back the request as received.
    fn serve(respond: impl FnOnce(&mut TcpStream) + Send + 'static) -> (String, thread::JoinHandle<String>) {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let url = format!("http://127.0.0.1:{}/v1/chat/completions", listener.local_addr().unwrap().port());
        let handle = thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            let mut reader = BufReader::new(stream.try_clone().unwrap());
            let mut head = String::new();
            let mut length = 0;
            loop {
                let mut line = String::new();
                if reader.read_line(&mut line).unwrap_or(0) == 0 || line == "\r\n" {
                    break;
                }
                if let Some(v) = line.to_ascii_lowercase().strip_prefix("content-length:") {
                    length = v.trim().parse().unwrap();
                }
                head.push_str(&line);
            }
            let mut body = vec![0; length];
            reader.read_exact(&mut body).unwrap();
            head.push_str(&String::from_utf8(body).unwrap());
            respond(&mut stream);
            head
        });
        (url, handle)
    }

    fn source(url: String) -> Source {
        Source { kind: Kind::OpenAi, url, key: "secret".into(), account: String::new(), model: "m".into() }
    }

    fn sse(parts: &[&str]) -> impl FnOnce(&mut TcpStream) + Send + 'static {
        let mut out = String::from("HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nConnection: close\r\n\r\n");
        for p in parts {
            out.push_str(&format!("data: {{\"choices\":[{{\"delta\":{{\"content\":{p:?}}}}}]}}\n\n"));
        }
        out.push_str("data: [DONE]\n\n");
        move |s| {
            let _ = s.write_all(out.as_bytes());
        }
    }

    #[test]
    fn the_answer_arrives_in_pieces_and_the_request_is_whole() {
        let (url, server) = serve(sse(&["Xin ", "chào", " bạn"]));
        let mut pieces = Vec::new();
        let said = stream(
            &source(url),
            "be brief",
            &[Message::user("hi \"there\"\nline two")],
            10,
            &Cancel::default(),
            &mut |p| pieces.push(p.to_owned()),
        )
        .unwrap();
        assert_eq!(said, "Xin chào bạn");
        assert_eq!(pieces, ["Xin ", "chào", " bạn"]);
        let request = server.join().unwrap();
        assert!(request.starts_with("POST /v1/chat/completions"));
        assert!(request.to_ascii_lowercase().contains("authorization: bearer secret"));
        assert!(request.contains(r#"hi \"there\"\nline two"#));
    }

    #[test]
    fn a_refusal_is_said_in_the_services_words() {
        let body = r#"{"error":{"message":"Incorrect API key"}}"#;
        let (url, _server) = serve(move |s| {
            let _ = s.write_all(
                format!("HTTP/1.1 401 Unauthorized\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}", body.len()).as_bytes(),
            );
        });
        let got = stream(&source(url), "s", &[Message::user("x")], 10, &Cancel::default(), &mut |_| {});
        assert_eq!(got, Err(Failure::Service("Incorrect API key".into())));
    }

    #[test]
    fn codex_sends_the_sign_in_and_a_bare_detail_is_read_as_the_reason() {
        let body = r#"{"detail":"Unauthorized"}"#;
        let (url, server) = serve(move |s| {
            let _ = s.write_all(
                format!("HTTP/1.1 401 Unauthorized\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}", body.len()).as_bytes(),
            );
        });
        let source = Source { kind: Kind::Codex, url, key: "tok".into(), account: "acct-1".into(), model: "m".into() };
        let got = stream(&source, "s", &[Message::user("x")], 10, &Cancel::default(), &mut |_| {});
        assert_eq!(got, Err(Failure::Service("Unauthorized".into())));
        let request = server.join().unwrap().to_ascii_lowercase();
        assert!(request.contains("authorization: bearer tok") && request.contains("chatgpt-account-id: acct-1"));
        assert!(request.contains("\"input\""), "the Responses format");
    }

    #[test]
    fn nothing_listening_is_unreachable() {
        let port = TcpListener::bind("127.0.0.1:0").unwrap().local_addr().unwrap().port();
        let url = format!("http://127.0.0.1:{port}/v1/chat/completions");
        let got = stream(&source(url), "s", &[Message::user("x")], 5, &Cancel::default(), &mut |_| {});
        assert_eq!(got, Err(Failure::Unreachable));
    }

    #[test]
    fn stopping_cuts_a_slow_answer_short() {
        let (url, _server) = serve(|s| {
            let _ = s.write_all(b"HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\n\r\n");
            let _ = s.write_all(b"data: {\"choices\":[{\"delta\":{\"content\":\"one\"}}]}\n\n");
            let _ = s.flush();
            thread::sleep(Duration::from_secs(8));
        });
        let cancel = Cancel::default();
        let seen = Arc::new(AtomicUsize::new(0));
        let started = Instant::now();
        let got = {
            let (cancel, seen) = (cancel.clone(), seen.clone());
            stream(&source(url), "s", &[Message::user("x")], 30, &cancel.clone(), &mut move |_| {
                seen.fetch_add(1, Ordering::SeqCst);
                cancel.store(true, Ordering::SeqCst);
            })
        };
        assert_eq!(got, Err(Failure::Cancelled));
        assert_eq!(seen.load(Ordering::SeqCst), 1);
        assert!(started.elapsed() < Duration::from_secs(5));
    }

    #[test]
    fn plain_http_to_another_machine_is_refused() {
        let s = Source { kind: Kind::OpenAi, url: "http://example.com/v1".into(), key: String::new(), account: String::new(), model: "m".into() };
        assert!(matches!(
            stream(&s, "s", &[], 5, &Cancel::default(), &mut |_| {}),
            Err(Failure::Service(_))
        ));
    }
}
