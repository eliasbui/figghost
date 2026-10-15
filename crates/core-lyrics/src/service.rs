//! Asks about a track without ever making the interface wait. `Service::lookup` answers at once from memory
//! or from the cache on disk; when the answer has to come from the network, a single worker thread fetches it
//! while the caller keeps asking, and nothing runs at all while nobody is waiting for an answer.
//!
//! LRCLIB asks clients to send one request at a time with a short pause between them, and to obey a rate
//! limit; the worker does both, and a failed request is never remembered as "no lyrics".

use crate::{
    cache::{Cache, Entry},
    lrc::{self, Lrc},
    lrclib::{self, Lyrics, Outcome, Query},
};
use std::{
    sync::{Arc, Condvar, Mutex, PoisonError},
    thread,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

/// The pause between two requests to the server.
const REQUEST_GAP: Duration = Duration::from_millis(400);

/// Lyrics ready to show.
#[derive(Debug, PartialEq, Eq)]
pub struct Ready {
    /// Present only when the synced text holds at least one timed line.
    pub synced: Option<Lrc>,
    pub plain: Option<String>,
    pub instrumental: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Status {
    /// Not found locally, and fetching online is off.
    Offline,
    Fetching,
    /// The server has nothing for this track.
    Missing,
    /// The server could not be asked or did not give an answer; it is tried again later.
    Failed,
    Ready(Arc<Ready>),
}

fn ready(lyrics: &Lyrics) -> Ready {
    let synced = lyrics
        .synced
        .as_deref()
        .map(lrc::parse)
        .filter(|l| !l.is_empty());
    Ready {
        synced,
        plain: lyrics.plain.clone(),
        instrumental: lyrics.instrumental,
    }
}

fn unix_now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_secs())
}

struct Inner {
    /// The newest request nobody has started on yet; a newer one replaces it, because the listener has moved on.
    want: Option<Query>,
    busy: Option<String>,
    /// The last answer found for the track that is playing, so the polls of the interface cost nothing.
    memo: Option<(String, Status)>,
    /// While in the future, the server is not asked at all (rate limit, no network); after it every track is tried again.
    blocked_until: Option<Instant>,
    last_request: Option<Instant>,
    stop: bool,
}

struct Shared {
    inner: Mutex<Inner>,
    wake: Condvar,
    cache: Cache,
    endpoint: String,
    user_agent: String,
    gap: Duration,
}

pub struct Service {
    shared: Arc<Shared>,
}

impl Service {
    pub fn new(cache: Cache) -> Self {
        Self::with_endpoint(cache, lrclib::ENDPOINT, lrclib::USER_AGENT, REQUEST_GAP)
    }

    pub fn with_endpoint(cache: Cache, endpoint: &str, user_agent: &str, gap: Duration) -> Self {
        let shared = Arc::new(Shared {
            inner: Mutex::new(Inner {
                want: None,
                busy: None,
                memo: None,
                blocked_until: None,
                last_request: None,
                stop: false,
            }),
            wake: Condvar::new(),
            cache,
            endpoint: endpoint.to_owned(),
            user_agent: user_agent.to_owned(),
            gap,
        });
        // The worker thread starts with the first request, so a feature nobody uses costs nothing.
        Self { shared }
    }

    /// The lyrics for `query`. With `online` false the network is never touched.
    pub fn lookup(&self, query: &Query, online: bool) -> Status {
        let key = query.key();
        let mut inner = self
            .shared
            .inner
            .lock()
            .unwrap_or_else(PoisonError::into_inner);
        if let Some((k, status)) = &inner.memo
            && *k == key
        {
            return status.clone();
        }
        match self.shared.cache.get(&key, unix_now()) {
            Some(Entry::Found(lyrics)) => {
                let status = Status::Ready(Arc::new(ready(&lyrics)));
                inner.memo = Some((key, status.clone()));
                return status;
            }
            Some(Entry::Missing) => {
                inner.memo = Some((key, Status::Missing));
                return Status::Missing;
            }
            None => {}
        }
        if !online {
            return Status::Offline;
        }
        let blocked = inner.blocked_until.is_some_and(|t| Instant::now() < t);
        if blocked {
            return Status::Failed;
        }
        if inner.busy.as_deref() == Some(&key) || inner.want.as_ref().map(Query::key) == Some(key) {
            return Status::Fetching;
        }
        inner.want = Some(query.clone());
        let start = inner.busy.is_none() && !inner.stop;
        drop(inner);
        if start {
            self.spawn_worker();
        }
        self.shared.wake.notify_all();
        Status::Fetching
    }

    /// Runs one worker until the queue is empty; a later request starts a new one.
    fn spawn_worker(&self) {
        let shared = Arc::clone(&self.shared);
        let mut inner = shared.inner.lock().unwrap_or_else(PoisonError::into_inner);
        if inner.busy.is_some() {
            return;
        }
        inner.busy = Some(String::new());
        drop(inner);
        let spawned = thread::Builder::new()
            .name("fig-lyrics".into())
            .spawn(move || work(&shared));
        if spawned.is_err() {
            self.shared
                .inner
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .busy = None;
        }
    }
}

impl Drop for Service {
    fn drop(&mut self) {
        // The worker may be inside a request of up to ten seconds; it is not waited for.
        self.shared
            .inner
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .stop = true;
        self.shared.wake.notify_all();
    }
}

fn work(shared: &Shared) {
    loop {
        let query = {
            let mut inner = shared.inner.lock().unwrap_or_else(PoisonError::into_inner);
            // Respect the pause after the previous request, and take the newest request once it is over.
            loop {
                if inner.stop {
                    inner.busy = None;
                    return;
                }
                let wait = inner
                    .last_request
                    .map_or(Duration::ZERO, |t| shared.gap.saturating_sub(t.elapsed()));
                if inner.want.is_some() && wait.is_zero() {
                    break;
                }
                if inner.want.is_none() {
                    inner.busy = None;
                    return;
                }
                inner = shared
                    .wake
                    .wait_timeout(inner, wait)
                    .unwrap_or_else(PoisonError::into_inner)
                    .0;
            }
            let Some(query) = inner.want.take() else {
                inner.busy = None;
                return;
            };
            inner.busy = Some(query.key());
            inner.last_request = Some(Instant::now());
            query
        };

        let outcome = lrclib::fetch(&shared.endpoint, &query, &shared.user_agent);
        let key = query.key();
        let mut inner = shared.inner.lock().unwrap_or_else(PoisonError::into_inner);
        match outcome {
            Outcome::Found(lyrics) => {
                shared
                    .cache
                    .put(&key, &Entry::Found(lyrics.clone()), unix_now());
                inner.memo = Some((key, Status::Ready(Arc::new(ready(&lyrics)))));
            }
            Outcome::NotFound => {
                shared.cache.put(&key, &Entry::Missing, unix_now());
                inner.memo = Some((key, Status::Missing));
            }
            Outcome::Retry { after_secs } => {
                inner.blocked_until = Some(Instant::now() + Duration::from_secs(after_secs));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        io::{BufRead, BufReader, Write},
        net::TcpListener,
        path::PathBuf,
        sync::atomic::{AtomicUsize, Ordering},
    };

    struct Dir(PathBuf);
    impl Dir {
        fn new(name: &str) -> Self {
            let dir = std::env::temp_dir()
                .join(format!("figghost-lyrics-svc-{name}-{}", std::process::id()));
            let _ = std::fs::remove_dir_all(&dir);
            Self(dir)
        }
        fn cache(&self) -> Cache {
            Cache::new(self.0.clone())
        }
    }
    impl Drop for Dir {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    /// A server that answers every request with `answer(path)` and counts them. It stops with the process.
    fn server(
        answer: impl Fn(&str) -> (String, String) + Send + 'static,
    ) -> (String, Arc<AtomicUsize>) {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let url = format!(
            "http://127.0.0.1:{}/api/get",
            listener.local_addr().unwrap().port()
        );
        let hits = Arc::new(AtomicUsize::new(0));
        let counter = Arc::clone(&hits);
        thread::spawn(move || {
            for stream in listener.incoming().flatten() {
                counter.fetch_add(1, Ordering::SeqCst);
                let mut stream = stream;
                let mut reader = BufReader::new(stream.try_clone().unwrap());
                let mut first = String::new();
                let _ = reader.read_line(&mut first);
                loop {
                    let mut line = String::new();
                    if reader.read_line(&mut line).unwrap_or(0) == 0 || line == "\r\n" {
                        break;
                    }
                }
                let (status, body) = answer(first.split(' ').nth(1).unwrap_or(""));
                let _ = write!(
                    stream,
                    "HTTP/1.1 {status}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                    body.len()
                );
            }
        });
        (url, hits)
    }

    fn query(title: &str) -> Query {
        Query::new("Band", title, "Album", 200_000).unwrap()
    }

    fn settle(service: &Service, q: &Query, online: bool) -> Status {
        let started = Instant::now();
        loop {
            let status = service.lookup(q, online);
            if status != Status::Fetching || started.elapsed() > Duration::from_secs(10) {
                return status;
            }
            thread::sleep(Duration::from_millis(20));
        }
    }

    fn svc(dir: &Dir, url: &str) -> Service {
        Service::with_endpoint(dir.cache(), url, "test/1.0", Duration::from_millis(50))
    }

    const RECORD: &str = r#"{"plainLyrics":"one\ntwo","syncedLyrics":"[00:01.00]one\n[00:02.00]two","instrumental":false}"#;

    #[test]
    fn a_fetched_track_is_cached_and_never_asked_again() {
        let dir = Dir::new("found");
        let (url, hits) = server(|_| ("200 OK".into(), RECORD.into()));
        let service = svc(&dir, &url);
        let status = settle(&service, &query("Song"), true);
        let Status::Ready(ready) = &status else {
            panic!("{status:?}")
        };
        assert_eq!(ready.synced.as_ref().unwrap().len(), 2);
        assert_eq!(ready.plain.as_deref(), Some("one\ntwo"));
        assert_eq!(service.lookup(&query("Song"), true), status);
        drop(service);
        // A new run of the app reads the file and does not touch the network, even when fetching is off.
        let service = svc(&dir, &url);
        assert!(matches!(
            service.lookup(&query("Song"), false),
            Status::Ready(_)
        ));
        assert_eq!(hits.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn fetching_off_never_touches_the_network() {
        let dir = Dir::new("offline");
        let (url, hits) = server(|_| ("200 OK".into(), RECORD.into()));
        let service = svc(&dir, &url);
        for _ in 0..3 {
            assert_eq!(service.lookup(&query("Song"), false), Status::Offline);
        }
        thread::sleep(Duration::from_millis(200));
        assert_eq!(hits.load(Ordering::SeqCst), 0);
    }

    #[test]
    fn a_track_without_lyrics_is_remembered_as_missing() {
        let dir = Dir::new("missing");
        let (url, hits) = server(|_| ("404 Not Found".into(), r#"{"code":404}"#.into()));
        let service = svc(&dir, &url);
        assert_eq!(settle(&service, &query("Song"), true), Status::Missing);
        assert_eq!(service.lookup(&query("Song"), true), Status::Missing);
        drop(service);
        let service = svc(&dir, &url);
        assert_eq!(service.lookup(&query("Song"), true), Status::Missing);
        assert_eq!(hits.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn garbage_and_rate_limits_fail_without_being_remembered() {
        let dir = Dir::new("garbage");
        let (url, hits) = server(|_| ("200 OK".into(), "<html>portal</html>".into()));
        let service = svc(&dir, &url);
        assert_eq!(settle(&service, &query("Song"), true), Status::Failed);
        // Blocked: another track is not even asked about.
        assert_eq!(service.lookup(&query("Other"), true), Status::Failed);
        assert_eq!(hits.load(Ordering::SeqCst), 1);
        assert!(std::fs::read_dir(&dir.0).map_or(true, |mut d| d.next().is_none()));
        // Fetching turned off still answers from local data only.
        assert_eq!(service.lookup(&query("Song"), false), Status::Offline);
    }

    #[test]
    fn the_newest_request_wins_when_the_listener_skips_ahead() {
        let dir = Dir::new("skip");
        let (url, hits) = server(|path| {
            let body = if path.contains("track_name=C") {
                RECORD
            } else {
                "{}"
            };
            ("200 OK".into(), body.into())
        });
        let service =
            Service::with_endpoint(dir.cache(), &url, "test/1.0", Duration::from_millis(300));
        for title in ["A", "B", "C"] {
            assert_eq!(service.lookup(&query(title), true), Status::Fetching);
        }
        assert!(matches!(
            settle(&service, &query("C"), true),
            Status::Ready(_)
        ));
        // A may have started before B and C arrived, but B never reaches the server.
        assert!(hits.load(Ordering::SeqCst) <= 2);
    }

    #[test]
    fn requests_are_spaced_by_the_gap() {
        let dir = Dir::new("gap");
        let (url, _) = server(|_| ("404 Not Found".into(), "{}".into()));
        let service =
            Service::with_endpoint(dir.cache(), &url, "test/1.0", Duration::from_millis(300));
        let started = Instant::now();
        assert_eq!(settle(&service, &query("A"), true), Status::Missing);
        assert_eq!(service.lookup(&query("B"), true), Status::Fetching);
        assert_eq!(settle(&service, &query("B"), true), Status::Missing);
        assert!(started.elapsed() >= Duration::from_millis(300));
    }

    #[test]
    fn synced_text_without_timed_lines_falls_back_to_plain() {
        let lyrics = Lyrics {
            synced: Some("no stamps here".into()),
            plain: Some("words".into()),
            instrumental: false,
        };
        let r = ready(&lyrics);
        assert!(r.synced.is_none() && r.plain.as_deref() == Some("words"));
    }
}
