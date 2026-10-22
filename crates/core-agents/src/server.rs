//! The local socket the hook programs talk to. A permission request keeps its connection open until
//! somebody answers, the wait runs out, or the hook program goes away.

use crate::{
    event::{Agent, EventKind, HookEvent},
    wire::{self, MAX_LINE, Reply},
};
use std::{
    fs,
    io::{self, BufRead, BufReader, Read, Write},
    os::unix::{
        fs::PermissionsExt,
        net::{UnixListener, UnixStream},
    },
    path::{Path, PathBuf},
    sync::{
        Arc, OnceLock,
        atomic::{AtomicBool, AtomicU32, AtomicU64, AtomicUsize, Ordering},
        mpsc::{self, RecvTimeoutError, Sender},
    },
    thread::{self, JoinHandle},
    time::{Duration, Instant},
};

/// More connections than this at once is not a person using hooks.
const MAX_CONNECTIONS: usize = 32;
const READ_TIMEOUT: Duration = Duration::from_secs(2);
const SLICE: Duration = Duration::from_millis(50);
/// pi asks the same question in its own terminal at the same time, so holding it costs nothing; this only
/// bounds a pi that never hangs up.
const PI_QUESTION_WAIT: Duration = Duration::from_secs(60 * 60);

/// Milliseconds on a clock that never goes back, so a deadline can be kept in an atomic and moved.
fn clock_ms() -> u64 {
    static START: OnceLock<Instant> = OnceLock::new();
    START.get_or_init(Instant::now).elapsed().as_millis() as u64
}

/// The way to answer one permission request, from any thread. It stops working when the wait is over or
/// the hook program is gone, which is how the app learns that a question no longer needs an answer.
pub struct Ticket {
    reply: Sender<Reply>,
    open: Arc<AtomicBool>,
    /// When the server stops waiting and lets the agent ask, on `clock_ms`.
    deadline: Arc<AtomicU64>,
}

impl Ticket {
    /// Whether the answer was passed on; false when nobody is waiting for it any more.
    pub fn answer(&self, reply: Reply) -> bool {
        self.open.load(Ordering::SeqCst) && self.reply.send(reply).is_ok()
    }

    pub fn is_open(&self) -> bool {
        self.open.load(Ordering::SeqCst)
    }

    /// Keeps the agent waiting at least `wait` from now, while somebody is answering; never brings the end
    /// closer. The agent's own timeout for the hook still ends it.
    pub fn wait_at_least(&self, wait: Duration) {
        let until = clock_ms() + wait.as_millis() as u64;
        self.deadline.fetch_max(until, Ordering::SeqCst);
    }
}

/// Receives every event. Permission requests come with a ticket; the other events have none and are
/// already answered.
pub type Handler = dyn Fn(HookEvent, Option<Ticket>) + Send + Sync + 'static;

pub struct Server {
    path: PathBuf,
    stop: Arc<AtomicBool>,
    wait_seconds: Arc<AtomicU32>,
    thread: Option<JoinHandle<()>>,
}

impl Server {
    /// Starts listening. Fails with `AddrInUse` when another instance already serves this socket; a socket
    /// file left behind by a crash is replaced.
    pub fn start(path: &Path, wait_seconds: u32, handler: Arc<Handler>) -> io::Result<Server> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        if path.exists() {
            if UnixStream::connect(path).is_ok() {
                return Err(io::Error::new(
                    io::ErrorKind::AddrInUse,
                    "another instance is already serving the agent socket",
                ));
            }
            fs::remove_file(path)?;
        }
        let listener = UnixListener::bind(path)?;
        // Only this user may talk to it.
        fs::set_permissions(path, fs::Permissions::from_mode(0o600))?;
        let stop = Arc::new(AtomicBool::new(false));
        let wait = Arc::new(AtomicU32::new(wait_seconds));
        let thread = {
            let (stop, wait) = (Arc::clone(&stop), Arc::clone(&wait));
            thread::Builder::new()
                .name("fig-agent-server".into())
                .spawn(move || accept_loop(&listener, &stop, &wait, &handler))?
        };
        Ok(Server {
            path: path.to_owned(),
            stop,
            wait_seconds: wait,
            thread: Some(thread),
        })
    }

    /// Takes effect for the requests that arrive from now on.
    pub fn set_wait_seconds(&self, seconds: u32) {
        self.wait_seconds.store(seconds, Ordering::SeqCst);
    }
}

impl Drop for Server {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::SeqCst);
        // Wakes the accept call, which is the only thing the thread is waiting for. If the socket file was
        // deleted behind our back there is no way to reach it, and waiting would hang: the thread is left
        // to end with the process.
        let woken = UnixStream::connect(&self.path).is_ok();
        if let Some(thread) = self.thread.take()
            && woken
        {
            let _ = thread.join();
        }
        let _ = fs::remove_file(&self.path);
    }
}

fn accept_loop(
    listener: &UnixListener,
    stop: &AtomicBool,
    wait: &Arc<AtomicU32>,
    handler: &Arc<Handler>,
) {
    let active = Arc::new(AtomicUsize::new(0));
    for stream in listener.incoming() {
        if stop.load(Ordering::SeqCst) {
            break;
        }
        let Ok(stream) = stream else {
            continue;
        };
        if active.fetch_add(1, Ordering::SeqCst) >= MAX_CONNECTIONS {
            active.fetch_sub(1, Ordering::SeqCst);
            continue;
        }
        let (handler, wait, active_in) =
            (Arc::clone(handler), Arc::clone(wait), Arc::clone(&active));
        let spawned = thread::Builder::new()
            .name("fig-agent-conn".into())
            .spawn(move || {
                let seconds = u64::from(wait.load(Ordering::SeqCst));
                serve(stream, &*handler, Duration::from_secs(seconds));
                active_in.fetch_sub(1, Ordering::SeqCst);
            });
        if spawned.is_err() {
            active.fetch_sub(1, Ordering::SeqCst);
        }
    }
}

/// True once the other end has hung up. The stream must be in non-blocking mode.
fn peer_closed(stream: &UnixStream) -> bool {
    let mut byte = [0u8; 1];
    let mut reader = stream;
    match reader.read(&mut byte) {
        Ok(0) => true,
        Ok(_) => false,
        Err(e) if e.kind() == io::ErrorKind::WouldBlock => false,
        Err(_) => true,
    }
}

fn read_request(stream: &UnixStream) -> Option<HookEvent> {
    stream.set_read_timeout(Some(READ_TIMEOUT)).ok()?;
    let mut line = String::new();
    BufReader::new(stream)
        .take(MAX_LINE as u64)
        .read_line(&mut line)
        .ok()?;
    if !line.ends_with('\n') {
        return None;
    }
    wire::decode_request(&line).ok()
}

fn serve(mut stream: UnixStream, handler: &Handler, wait: Duration) {
    let Some(event) = read_request(&stream) else {
        return;
    };
    if !matches!(event.kind, EventKind::PermissionRequest | EventKind::Question) {
        handler(event, None);
        let _ = stream.write_all(wire::encode_reply(&Reply::Pass).as_bytes());
        return;
    }
    let (reply_tx, reply_rx) = mpsc::channel();
    let open = Arc::new(AtomicBool::new(true));
    let wait = if event.agent == Agent::Pi && event.kind == EventKind::Question {
        PI_QUESTION_WAIT
    } else {
        wait
    };
    let deadline = Arc::new(AtomicU64::new(clock_ms() + wait.as_millis() as u64));
    handler(
        event,
        Some(Ticket {
            reply: reply_tx,
            open: Arc::clone(&open),
            deadline: Arc::clone(&deadline),
        }),
    );
    let _ = stream.set_nonblocking(true);
    let reply = loop {
        match reply_rx.recv_timeout(SLICE) {
            Ok(reply) => break reply,
            Err(RecvTimeoutError::Timeout) => {}
            // The app dropped the ticket without answering.
            Err(RecvTimeoutError::Disconnected) => break Reply::Pass,
        }
        if clock_ms() >= deadline.load(Ordering::SeqCst) || peer_closed(&stream) {
            break Reply::Pass;
        }
    };
    open.store(false, Ordering::SeqCst);
    let _ = stream.set_nonblocking(false);
    let _ = stream.write_all(wire::encode_reply(&reply).as_bytes());
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    fn event(kind: EventKind) -> HookEvent {
        HookEvent {
            agent: Agent::Claude,
            kind,
            session_id: "s".into(),
            cwd: "/w".into(),
            tool: Some("Bash".into()),
            detail: Some("ls".into()),
            message: None,
            questions: Vec::new(),
            at_ms: 1,
        }
    }

    fn socket(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("core-agents-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir.join("a.sock")
    }

    type Seen = Arc<Mutex<Vec<(HookEvent, bool)>>>;

    /// A handler that records what it gets and answers permission requests with `answer`, if any.
    fn recording(answer: Option<Reply>) -> (Arc<Handler>, Seen, Arc<Mutex<Vec<Ticket>>>) {
        let seen: Seen = Arc::default();
        let kept: Arc<Mutex<Vec<Ticket>>> = Arc::default();
        let (log, tickets) = (Arc::clone(&seen), Arc::clone(&kept));
        let handler: Arc<Handler> = Arc::new(move |event, ticket| {
            log.lock().unwrap().push((event, ticket.is_some()));
            if let Some(ticket) = ticket {
                match &answer {
                    Some(reply) => {
                        let reply = reply.clone();
                        thread::spawn(move || {
                            thread::sleep(Duration::from_millis(30));
                            ticket.answer(reply);
                        });
                    }
                    None => tickets.lock().unwrap().push(ticket),
                }
            }
        });
        (handler, seen, kept)
    }

    fn ask(path: &Path, event: &HookEvent) -> (Option<Reply>, Duration) {
        let started = Instant::now();
        let mut stream = UnixStream::connect(path).unwrap();
        stream
            .write_all(wire::encode_request(event).as_bytes())
            .unwrap();
        let mut line = String::new();
        BufReader::new(&stream).read_line(&mut line).unwrap();
        (wire::decode_reply(&line).ok(), started.elapsed())
    }

    #[test]
    fn a_permission_request_waits_for_its_answer() {
        let path = socket("allow");
        let (handler, seen, _) = recording(Some(Reply::Allow));
        let server = Server::start(&path, 5, handler).unwrap();
        let (reply, took) = ask(&path, &event(EventKind::PermissionRequest));
        assert_eq!(reply, Some(Reply::Allow));
        assert!(took < Duration::from_secs(2), "{took:?}");
        assert_eq!(seen.lock().unwrap().len(), 1);
        assert!(seen.lock().unwrap()[0].1, "a request comes with a ticket");
        drop(server);
        assert!(!path.exists(), "the socket file goes with the server");
    }

    #[test]
    fn a_denial_carries_its_message() {
        let path = socket("deny");
        let (handler, _, _) = recording(Some(Reply::Deny {
            message: "no thanks".into(),
        }));
        let _server = Server::start(&path, 5, handler).unwrap();
        let (reply, _) = ask(&path, &event(EventKind::PermissionRequest));
        assert_eq!(
            reply,
            Some(Reply::Deny {
                message: "no thanks".into()
            })
        );
    }

    #[test]
    fn other_events_are_answered_at_once_and_have_no_ticket() {
        let path = socket("quick");
        let (handler, seen, _) = recording(None);
        let _server = Server::start(&path, 5, handler).unwrap();
        let (reply, took) = ask(&path, &event(EventKind::Stop));
        assert_eq!(reply, Some(Reply::Pass));
        assert!(took < Duration::from_secs(1));
        let seen = seen.lock().unwrap();
        assert_eq!((seen[0].0.kind, seen[0].1), (EventKind::Stop, false));
    }

    #[test]
    fn nobody_answering_means_pass_after_the_wait_and_the_ticket_closes() {
        let path = socket("timeout");
        let (handler, _, kept) = recording(None);
        let _server = Server::start(&path, 1, handler).unwrap();
        let (reply, took) = ask(&path, &event(EventKind::PermissionRequest));
        assert_eq!(reply, Some(Reply::Pass));
        assert!(
            took >= Duration::from_millis(900) && took < Duration::from_secs(3),
            "{took:?}"
        );
        let tickets = kept.lock().unwrap();
        assert!(!tickets[0].is_open());
        assert!(
            !tickets[0].answer(Reply::Allow),
            "a late answer has nowhere to go"
        );
    }

    #[test]
    fn a_wait_kept_moving_by_the_app_outlasts_the_setting_and_ends_when_it_stops() {
        let path = socket("extend");
        let (handler, _, kept) = recording(None);
        let _server = Server::start(&path, 1, handler).unwrap();
        let keeper = {
            let kept = Arc::clone(&kept);
            thread::spawn(move || {
                // What the page does while the card is on screen: push the end forward every so often.
                let until = Instant::now() + Duration::from_millis(2_000);
                while Instant::now() < until {
                    if let Some(ticket) = kept.lock().unwrap().first() {
                        ticket.wait_at_least(Duration::from_millis(600));
                    }
                    thread::sleep(Duration::from_millis(100));
                }
            })
        };
        let (reply, took) = ask(&path, &event(EventKind::PermissionRequest));
        keeper.join().unwrap();
        assert_eq!(reply, Some(Reply::Pass));
        assert!(
            took >= Duration::from_millis(2_000) && took < Duration::from_millis(3_500),
            "{took:?}"
        );
    }

    #[test]
    fn moving_the_wait_never_brings_the_end_closer() {
        let (reply, _) = mpsc::channel();
        let ticket = Ticket {
            reply,
            open: Arc::new(AtomicBool::new(true)),
            deadline: Arc::new(AtomicU64::new(clock_ms() + 60_000)),
        };
        ticket.wait_at_least(Duration::from_secs(1));
        assert!(ticket.deadline.load(Ordering::SeqCst) >= clock_ms() + 50_000);
        ticket.wait_at_least(Duration::from_secs(120));
        assert!(ticket.deadline.load(Ordering::SeqCst) >= clock_ms() + 110_000);
    }

    #[test]
    fn a_question_from_pi_waits_past_the_setting_until_pi_hangs_up() {
        let path = socket("pi");
        let (handler, _, kept) = recording(None);
        let _server = Server::start(&path, 1, handler).unwrap();
        let question = HookEvent {
            agent: Agent::Pi,
            ..event(EventKind::Question)
        };
        let mut stream = UnixStream::connect(&path).unwrap();
        stream
            .write_all(wire::encode_request(&question).as_bytes())
            .unwrap();
        thread::sleep(Duration::from_millis(1_600));
        assert!(
            kept.lock().unwrap()[0].is_open(),
            "still waiting after the one second the setting gives"
        );
        drop(stream);
        let deadline = Instant::now() + Duration::from_secs(2);
        while kept.lock().unwrap()[0].is_open() && Instant::now() < deadline {
            thread::sleep(Duration::from_millis(10));
        }
        assert!(!kept.lock().unwrap()[0].is_open());
    }

    #[test]
    fn the_hook_program_going_away_closes_the_ticket_without_waiting_for_the_deadline() {
        let path = socket("hangup");
        let (handler, _, kept) = recording(None);
        let _server = Server::start(&path, 30, handler).unwrap();
        {
            let mut stream = UnixStream::connect(&path).unwrap();
            stream
                .write_all(wire::encode_request(&event(EventKind::PermissionRequest)).as_bytes())
                .unwrap();
            // Wait until the request has reached the app, then hang up as a killed hook program does.
            let deadline = Instant::now() + Duration::from_secs(2);
            while kept.lock().unwrap().is_empty() && Instant::now() < deadline {
                thread::sleep(Duration::from_millis(10));
            }
            assert_eq!(kept.lock().unwrap().len(), 1);
        }
        let deadline = Instant::now() + Duration::from_secs(2);
        while kept.lock().unwrap()[0].is_open() && Instant::now() < deadline {
            thread::sleep(Duration::from_millis(10));
        }
        assert!(!kept.lock().unwrap()[0].is_open());
    }

    #[test]
    fn a_ticket_dropped_unanswered_means_pass() {
        let path = socket("dropped");
        let handler: Arc<Handler> = Arc::new(|_, ticket| drop(ticket));
        let _server = Server::start(&path, 30, handler).unwrap();
        let (reply, took) = ask(&path, &event(EventKind::PermissionRequest));
        assert_eq!(reply, Some(Reply::Pass));
        assert!(took < Duration::from_secs(2));
    }

    #[test]
    fn garbage_is_ignored_and_the_server_keeps_working() {
        let path = socket("garbage");
        let (handler, seen, _) = recording(Some(Reply::Allow));
        let _server = Server::start(&path, 5, handler).unwrap();
        for junk in [&b"not json\n"[..], b"{\"agent\":", b"\n"] {
            let mut stream = UnixStream::connect(&path).unwrap();
            stream.write_all(junk).unwrap();
            drop(stream);
        }
        let (reply, _) = ask(&path, &event(EventKind::PermissionRequest));
        assert_eq!(reply, Some(Reply::Allow));
        assert_eq!(
            seen.lock().unwrap().len(),
            1,
            "the junk never reached the app"
        );
    }

    #[test]
    fn a_long_line_is_refused() {
        let path = socket("long");
        let (handler, seen, _) = recording(None);
        let _server = Server::start(&path, 5, handler).unwrap();
        let mut stream = UnixStream::connect(&path).unwrap();
        let mut huge = event(EventKind::Stop);
        huge.detail = Some("x".repeat(MAX_LINE * 2));
        // The server hangs up after the first 64 KiB, so the end of the write may fail: that is the point.
        let _ = stream.write_all(wire::encode_request(&huge).as_bytes());
        let mut sink = String::new();
        let _ = BufReader::new(&stream).read_line(&mut sink);
        assert!(seen.lock().unwrap().is_empty());
    }

    #[test]
    fn the_socket_is_private_to_the_user() {
        let path = socket("mode");
        let (handler, _, _) = recording(None);
        let _server = Server::start(&path, 5, handler).unwrap();
        assert_eq!(
            fs::metadata(&path).unwrap().permissions().mode() & 0o777,
            0o600
        );
    }

    #[test]
    fn a_second_instance_is_refused_and_a_stale_file_is_replaced() {
        let path = socket("twice");
        let (handler, _, _) = recording(None);
        let first = Server::start(&path, 5, Arc::clone(&handler)).unwrap();
        let second = Server::start(&path, 5, Arc::clone(&handler));
        assert_eq!(
            second.err().map(|e| e.kind()),
            Some(io::ErrorKind::AddrInUse)
        );
        // A crash leaves the file behind with nobody listening.
        std::mem::forget(first);
        let _ = fs::remove_file(&path);
        let listener = UnixListener::bind(&path).unwrap();
        drop(listener);
        assert!(path.exists());
        let again = Server::start(&path, 5, handler);
        assert!(again.is_ok());
    }
}
