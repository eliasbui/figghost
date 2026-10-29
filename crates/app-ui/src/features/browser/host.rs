//! The program a browser starts for the extension, `Figghost chrome-extension://<id>/`, with native messaging on
//! standard input and output. It asks the running app what to block every two seconds and tells the extension when
//! that changes, and passes on an address to download. It knows nothing by itself: without the app, nothing is blocked.
//! It ends when the browser closes its end, or sends what the extension never sends.

use super::logic::{self, Ask};
use serde_json::{Value, json};
use std::{
    io::{self, BufRead, BufReader, Read, Write},
    os::unix::net::UnixStream,
    path::Path,
    sync::{Arc, Mutex},
    thread,
    time::Duration,
};

const LOOK_EVERY: Duration = Duration::from_secs(2);
const ASK_TIMEOUT: Duration = Duration::from_secs(3);

type Out = Arc<Mutex<io::Stdout>>;
type Last = Arc<Mutex<Option<Value>>>;

pub fn run(home: &Path, origin: &str) -> i32 {
    if !logic::is_our_origin(origin) {
        return 2;
    }
    let socket = core_agents::named_socket(home, "browser");
    let out: Out = Arc::new(Mutex::new(io::stdout()));
    let last: Last = Arc::default();
    {
        let (socket, out, last) = (socket.clone(), Arc::clone(&out), Arc::clone(&last));
        thread::spawn(move || {
            let mut input = io::stdin().lock();
            while let Ok(Some(message)) = logic::read_message(&mut input) {
                answer(&socket, &message, &out, &last);
            }
            std::process::exit(0);
        });
    }
    loop {
        tell_state(&socket, &out, &last, false);
        thread::sleep(LOOK_EVERY);
    }
}

/// One question to the app over its socket, one line each way; `None` when the app is not there or says nothing.
pub fn ask(socket: &Path, question: &Ask) -> Option<Value> {
    let mut stream = UnixStream::connect(socket).ok()?;
    stream.set_read_timeout(Some(ASK_TIMEOUT)).ok()?;
    stream.set_write_timeout(Some(ASK_TIMEOUT)).ok()?;
    writeln!(stream, "{}", question.line()).ok()?;
    let mut line = String::new();
    BufReader::new(stream.take(logic::MAX_MESSAGE as u64))
        .read_line(&mut line)
        .ok()?;
    serde_json::from_str(&line).ok()
}

/// What the extension is told to block, from the app's answer; with no answer, nothing.
pub fn state_of(answer: Option<&Value>) -> Value {
    let sites: Vec<Value> = answer
        .and_then(|a| a["sites"].as_array())
        .map(|s| s.iter().filter(|v| v.is_string()).cloned().collect())
        .unwrap_or_default();
    json!({
        "type": "state",
        "block": answer.is_some_and(|a| a["block"] == true) && !sites.is_empty(),
        "sites": sites,
        "app": answer.is_some(),
    })
}

fn tell_state(socket: &Path, out: &Out, last: &Last, always: bool) {
    let state = state_of(ask(socket, &Ask::State).as_ref());
    let Ok(mut last) = last.lock() else {
        return;
    };
    if always || last.as_ref() != Some(&state) {
        send(out, &state);
        *last = Some(state);
    }
}

fn answer(socket: &Path, message: &Value, out: &Out, last: &Last) {
    match message["type"].as_str() {
        Some("hello") => tell_state(socket, out, last, true),
        Some("download") => {
            let question = message["url"]
                .as_str()
                .and_then(|url| Ask::parse(&Ask::Download(url.to_owned()).line()));
            let reply = match question {
                Some(q) => {
                    ask(socket, &q).unwrap_or_else(|| json!({ "ok": false, "error": "no_app" }))
                }
                None => json!({ "ok": false, "error": "bad_url" }),
            };
            send(
                out,
                &json!({ "type": "download", "ok": reply["ok"] == true, "error": reply["error"] }),
            );
        }
        _ => {}
    }
}

fn send(out: &Out, message: &Value) {
    if let Ok(mut out) = out.lock() {
        let _ = logic::write_message(&mut *out, message);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{fs, os::unix::net::UnixListener};

    #[test]
    fn without_the_app_nothing_is_blocked_and_an_empty_list_blocks_nothing() {
        assert_eq!(
            state_of(None),
            json!({ "type": "state", "block": false, "sites": [], "app": false })
        );
        let on = json!({ "block": true, "sites": ["youtube.com", 5] });
        assert_eq!(state_of(Some(&on))["sites"], json!(["youtube.com"]));
        assert_eq!(state_of(Some(&on))["block"], json!(true));
        assert_eq!(
            state_of(Some(&json!({ "block": true, "sites": [] })))["block"],
            json!(false)
        );
    }

    #[test]
    fn a_question_goes_to_the_app_and_its_line_comes_back() {
        let dir = std::env::temp_dir().join(format!("figghost-host-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        let socket = dir.join("b.sock");
        let listener = UnixListener::bind(&socket).unwrap();
        let app = thread::spawn(move || {
            let (stream, _) = listener.accept().unwrap();
            let mut line = String::new();
            BufReader::new(&stream).read_line(&mut line).unwrap();
            let asked = Ask::parse(&line);
            writeln!(
                &stream,
                "{}",
                json!({ "block": asked == Some(Ask::State), "sites": ["x.com"] })
            )
            .unwrap();
        });
        let answer = ask(&socket, &Ask::State).unwrap();
        app.join().unwrap();
        assert_eq!(answer["block"], json!(true));
        assert!(
            ask(&dir.join("none.sock"), &Ask::State).is_none(),
            "no app, no answer"
        );
        fs::remove_dir_all(&dir).unwrap();
    }
}
