//! What is playing, in whichever app plays it: a browser, Music, Spotify, a podcast app.
//!
//! The private MediaRemote framework knows, but on current macOS it only answers Apple's own processes. The
//! nowplaying-adapter library reads it from inside `/usr/bin/perl` (see that crate) and prints one JSON line per
//! change; the watcher runs it while active and reads the lines, so nothing is polled. Without the adapter (not
//! built, or it cannot run here) the watcher falls back to asking Music and Spotify through AppleScript in a helper
//! process, which has a hard timeout, so a hung player can never block the UI, and never launches a player that is
//! not running.

use crate::{app, workspace};
use objc2::MainThreadMarker;
use serde_json::Value;
use std::{
    cell::RefCell,
    collections::HashMap,
    fmt,
    hash::{DefaultHasher, Hash, Hasher},
    io::{BufRead, BufReader, Read},
    path::{Path, PathBuf},
    process::{Child, Command, Stdio},
    rc::Rc,
    sync::{
        Arc, Condvar, Mutex, PoisonError,
        atomic::{AtomicBool, AtomicU64, Ordering},
    },
    thread::{self, JoinHandle},
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

const SEPARATOR: char = '\u{1f}';
const POLL_INTERVAL: Duration = Duration::from_millis(1500);
const SCRIPT_TIMEOUT: Duration = Duration::from_secs(3);

const PERL: &str = "/usr/bin/perl";
/// Loads the library named by the first argument and calls its entry point, which never returns.
const PERL_SCRIPT: &str = "my $h = DynaLoader::dl_load_file($ARGV[0], 0) or die; \
                           my $s = DynaLoader::dl_find_symbol($h, 'figghost_np_stream') or die; \
                           DynaLoader::dl_install_xsub('main::np', $s); np();";
/// Loads the same library and calls its seek, which reads the position from FIGGHOST_NP_SEEK. (Not named `seek`:
/// perl has a function of that name.)
const SEEK_SCRIPT: &str = "my $h = DynaLoader::dl_load_file($ARGV[0], 0) or die; \
                           my $s = DynaLoader::dl_find_symbol($h, 'figghost_np_seek') or die; \
                           DynaLoader::dl_install_xsub('main::seek_to', $s); seek_to();";
const ADAPTER: &str = "libfigghost_nowplaying.dylib";
/// The adapter's longest line carries a few MB of artwork; a longer one is not the adapter's and is skipped.
const MAX_LINE: usize = 8 << 20;
/// This many failed runs in a row and the adapter is given up for the polling.
const MAX_FAILURES: u32 = 5;
/// A run that lasted this long worked: a failure after it starts the count again.
const HEALTHY_RUN: Duration = Duration::from_secs(60);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Player {
    Music,
    Spotify,
}

impl Player {
    const ALL: [Player; 2] = [Player::Music, Player::Spotify];

    const fn name(self) -> &'static str {
        match self {
            Player::Music => "Music",
            Player::Spotify => "Spotify",
        }
    }

    const fn bundle_id(self) -> &'static str {
        match self {
            Player::Music => "com.apple.Music",
            Player::Spotify => "com.spotify.client",
        }
    }

    /// Music reports the length in seconds, Spotify in milliseconds.
    const fn duration_to_ms(self) -> &'static str {
        match self {
            Player::Music => "* 1000",
            Player::Spotify => "",
        }
    }

    /// One line: state, title, artist, album, position in ms, length in ms, separated by character 31. The
    /// last three are optional on their own (a radio stream has no position), so each has its own `try`.
    fn script(self) -> String {
        format!(
            "if application id \"{id}\" is not running then return \"\"\n\
             tell application id \"{id}\"\n\
             try\n\
             set stateText to (player state as text)\n\
             if stateText is \"stopped\" then return \"\"\n\
             set albumText to \"\"\n\
             set positionText to \"\"\n\
             set lengthText to \"\"\n\
             try\n\
             set albumText to (album of current track)\n\
             end try\n\
             try\n\
             set positionText to ((round ((player position) * 1000) rounding down) as text)\n\
             end try\n\
             try\n\
             set lengthText to ((round ((duration of current track) {to_ms}) rounding down) as text)\n\
             end try\n\
             return stateText & (ASCII character 31) & (name of current track) & (ASCII character 31) & (artist of current track) & (ASCII character 31) & albumText & (ASCII character 31) & positionText & (ASCII character 31) & lengthText\n\
             on error\n\
             return \"\"\n\
             end try\n\
             end tell",
            id = self.bundle_id(),
            to_ms = self.duration_to_ms()
        )
    }
}

/// The picture of what plays, encoded as the app gave it (JPEG or PNG), and a hash of it so that a page can tell
/// a new picture from the one it already shows.
#[derive(Clone, PartialEq, Eq)]
pub struct Artwork {
    pub id: u64,
    pub bytes: Arc<Vec<u8>>,
}

impl Artwork {
    fn new(bytes: Vec<u8>) -> Self {
        let mut hasher = DefaultHasher::new();
        bytes.hash(&mut hasher);
        Self {
            id: hasher.finish(),
            bytes: Arc::new(bytes),
        }
    }
}

/// The size instead of the bytes, so that printing a track stays readable.
impl fmt::Debug for Artwork {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Artwork")
            .field("id", &self.id)
            .field("bytes", &self.bytes.len())
            .finish()
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Track {
    /// The bundle identifier of the app that plays; for a page in a browser, the browser's.
    pub app: String,
    /// The name Finder shows for that app, or the bundle identifier when there is none.
    pub app_name: String,
    pub title: String,
    pub artist: String,
    pub album: String,
    pub playing: bool,
    /// Seconds of the track per second of time while it plays: 1 at normal speed, 0 when paused.
    pub rate: f64,
    /// Where the player was in the track at the sample's moment; 0 when it does not say.
    pub position_ms: u64,
    /// 0 when the player does not say.
    pub duration_ms: u64,
    pub artwork: Option<Artwork>,
}

/// A track with the moment it was read, so that the position can be carried forward while it plays.
#[derive(Debug, Clone, PartialEq)]
pub struct Sample {
    pub track: Track,
    pub at: Instant,
}

impl Sample {
    /// Where the player is now: the position it reported plus the time since, at its rate, while it plays.
    pub fn position_ms(&self, now: Instant) -> u64 {
        let rate = if self.track.playing {
            self.track.rate
        } else {
            0.0
        };
        let moved = now.saturating_duration_since(self.at).as_secs_f64() * 1000.0 * rate;
        // `as` saturates: a rewind past the start is 0.
        let at = (self.track.position_ms as f64 + moved).round() as u64;
        match self.track.duration_ms {
            0 => at,
            length => at.min(length),
        }
    }
}

/// Parses the script's `state<US>title<US>artist<US>album<US>position<US>length` line; everything after the
/// artist may be missing. An empty line, or one without a title, means nothing is playing.
fn parse(player: Player, output: &str) -> Option<Track> {
    let mut parts = output.trim_end_matches(['\n', '\r']).split(SEPARATOR);
    let state = parts.next()?;
    let title = parts.next()?.trim();
    let artist = parts.next().unwrap_or("").trim();
    let album = parts.next().unwrap_or("").trim();
    let mut millis = || {
        parts
            .next()
            .and_then(|s| s.trim().parse::<u64>().ok())
            .unwrap_or(0)
    };
    let (position_ms, duration_ms) = (millis(), millis());
    let playing = state.trim() == "playing";
    (!title.is_empty()).then(|| Track {
        app: player.bundle_id().to_owned(),
        app_name: String::new(),
        title: title.to_owned(),
        artist: artist.to_owned(),
        album: album.to_owned(),
        playing,
        rate: if playing { 1.0 } else { 0.0 },
        position_ms,
        duration_ms,
        artwork: None,
    })
}

/// Seconds as whole milliseconds; `as` saturates, so a negative or NaN value is 0.
fn millis(seconds: f64) -> u64 {
    (seconds * 1000.0).round() as u64
}

/// Standard base64 with padding, as the adapter writes it; `None` for anything else.
fn base64_decode(text: &str) -> Option<Vec<u8>> {
    let text = text.as_bytes();
    if text.len() % 4 != 0 {
        return None;
    }
    let chunks = text.len() / 4;
    let mut out = Vec::with_capacity(chunks * 3);
    for (index, chunk) in text.chunks(4).enumerate() {
        let pad = chunk.iter().rev().take_while(|&&c| c == b'=').count();
        if pad > 2 || (pad > 0 && index + 1 != chunks) {
            return None;
        }
        let mut n = 0u32;
        for &c in &chunk[..4 - pad] {
            let value = match c {
                b'A'..=b'Z' => c - b'A',
                b'a'..=b'z' => c - b'a' + 26,
                b'0'..=b'9' => c - b'0' + 52,
                b'+' => 62,
                b'/' => 63,
                _ => return None,
            };
            n = n << 6 | u32::from(value);
        }
        n <<= 6 * pad;
        out.extend_from_slice(&n.to_be_bytes()[1..4 - pad]);
    }
    Some(out)
}

/// Reads one line of the adapter: `None` when it is not one, `Some(None)` when nothing plays. A line names the
/// artwork only when it changed, so `artwork` carries it from line to line. `wall_ms` is now in milliseconds since
/// 1970, to carry the position from when the player measured it to now.
fn parse_line(line: &str, artwork: &mut Option<Artwork>, wall_ms: f64) -> Option<Option<Track>> {
    let Value::Object(fields) = serde_json::from_str::<Value>(line).ok()? else {
        return None;
    };
    let text = |key: &str| fields.get(key).and_then(Value::as_str).unwrap_or("").trim();
    let number = |key: &str| {
        fields
            .get(key)
            .and_then(Value::as_f64)
            .filter(|n| n.is_finite())
    };
    if let Some(encoded) = fields.get("artwork").and_then(Value::as_str) {
        *artwork = base64_decode(encoded)
            .filter(|bytes| !bytes.is_empty())
            .map(Artwork::new);
    }
    let title = text("title");
    if title.is_empty() {
        return Some(None);
    }
    let playing = fields
        .get("playing")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let rate = if playing {
        number("rate").unwrap_or(1.0)
    } else {
        0.0
    };
    let measured = number("at_ms").filter(|&at| at > 0.0).unwrap_or(wall_ms);
    let seconds_since = ((wall_ms - measured) / 1000.0).max(0.0);
    let position = number("elapsed").unwrap_or(0.0) + rate * seconds_since;
    let app = [text("parent"), text("bundle")]
        .into_iter()
        .find(|id| !id.is_empty())
        .unwrap_or("");
    Some(Some(Track {
        app: app.to_owned(),
        app_name: String::new(),
        title: title.to_owned(),
        artist: text("artist").to_owned(),
        album: text("album").to_owned(),
        playing,
        rate,
        position_ms: millis(position),
        duration_ms: millis(number("duration").unwrap_or(0.0)),
        artwork: artwork.clone(),
    }))
}

/// Reads the next line into `line`, without its end; a line longer than MAX_LINE comes back empty. False at the
/// end of the stream.
fn next_line(reader: &mut impl BufRead, line: &mut Vec<u8>) -> bool {
    line.clear();
    match reader
        .by_ref()
        .take(MAX_LINE as u64)
        .read_until(b'\n', line)
    {
        Ok(0) | Err(_) => false,
        Ok(_) => {
            if line.last() == Some(&b'\n') {
                line.pop();
            } else if line.len() >= MAX_LINE {
                line.clear();
                return reader.skip_until(b'\n').is_ok();
            }
            true
        }
    }
}

fn wall_ms() -> f64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0.0, |d| d.as_secs_f64() * 1000.0)
}

/// Makes the command that runs the adapter: perl loading the library, or a stand-in in tests.
type Launch = Box<dyn Fn() -> Command + Send>;

fn perl(adapter: PathBuf) -> Launch {
    Box::new(move || {
        let mut command = Command::new(PERL);
        command
            .args(["-MDynaLoader", "-e", PERL_SCRIPT])
            .arg(&adapter);
        command
    })
}

/// Moves what plays, in whichever app plays it, to `seconds` from its start. Returns at once, false when there is no
/// adapter to do it with; the watcher hears the new position when the player reports it.
pub fn seek(seconds: f64) -> bool {
    let Some(adapter) = find_adapter().filter(|_| seconds.is_finite() && seconds >= 0.0) else {
        return false;
    };
    let child = Command::new(PERL)
        .args(["-MDynaLoader", "-e", SEEK_SCRIPT])
        .arg(&adapter)
        .env("FIGGHOST_NP_SEEK", format!("{seconds:.3}"))
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn();
    match child {
        Ok(mut child) => {
            // Waited for off the main thread, so it leaves no zombie behind.
            thread::spawn(move || child.wait());
            true
        }
        Err(_) => false,
    }
}

/// Where the adapter library is: FIGGHOST_NOWPLAYING_DYLIB, then the app bundle's Frameworks folder, then next to
/// the program, then one folder up (examples and tests run from a folder inside the build folder).
fn find_adapter() -> Option<PathBuf> {
    let mut places: Vec<PathBuf> = std::env::var_os("FIGGHOST_NOWPLAYING_DYLIB")
        .map(PathBuf::from)
        .into_iter()
        .collect();
    if let Some(dir) = std::env::current_exe()
        .ok()
        .and_then(|exe| exe.parent().map(Path::to_path_buf))
    {
        places.extend([
            dir.join("../Frameworks").join(ADAPTER),
            dir.join(ADAPTER),
            dir.join("..").join(ADAPTER),
        ]);
    }
    places.into_iter().find(|place| place.is_file())
}

/// Runs a command and returns its stdout, or `None` when it fails or outlives `timeout` (then it is killed).
fn run_capture(command: &mut Command, timeout: Duration) -> Option<String> {
    let mut child = command
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .ok()?;
    let started = Instant::now();
    loop {
        match child.try_wait().ok()? {
            Some(status) if status.success() => {
                let mut out = String::new();
                child.stdout.take()?.read_to_string(&mut out).ok()?;
                return Some(out);
            }
            Some(_) => return None,
            None if started.elapsed() >= timeout => {
                let _ = child.kill();
                let _ = child.wait();
                return None;
            }
            None => thread::sleep(Duration::from_millis(15)),
        }
    }
}

fn is_running(player: Player) -> bool {
    run_capture(
        Command::new("pgrep").args(["-x", player.name()]),
        Duration::from_secs(1),
    )
    .is_some_and(|out| !out.trim().is_empty())
}

fn query(player: Player) -> Option<Sample> {
    if !is_running(player) {
        return None;
    }
    let out = run_capture(
        Command::new("osascript").arg("-e").arg(player.script()),
        SCRIPT_TIMEOUT,
    )?;
    // The player read its position near the end of the script, so the time of the answer is the time of the sample.
    let track = parse(player, &out)?;
    Some(Sample {
        track,
        at: Instant::now(),
    })
}

/// The track to show: one that is actually playing wins, otherwise a paused one.
fn current() -> Option<Sample> {
    let samples: Vec<Sample> = Player::ALL.into_iter().filter_map(query).collect();
    let playing = samples.iter().position(|s| s.track.playing);
    match playing {
        Some(i) => samples.into_iter().nth(i),
        None => samples.into_iter().next(),
    }
}

/// Whether two looks at the players show the same thing, apart from the position moving along.
fn same_track(a: Option<&Sample>, b: Option<&Sample>) -> bool {
    let shown = |s: &Sample| {
        let t = &s.track;
        (
            t.app.clone(),
            t.title.clone(),
            t.artist.clone(),
            t.album.clone(),
            t.playing,
            t.duration_ms,
        )
    };
    a.map(shown) == b.map(shown)
}

thread_local! {
    /// What each watcher calls on the main thread when what plays changes, by watcher.
    static LISTENERS: RefCell<HashMap<u64, Rc<dyn Fn()>>> = RefCell::new(HashMap::new());
    /// App names by bundle identifier, looked up once each on the main thread.
    static NAMES: RefCell<HashMap<String, String>> = RefCell::new(HashMap::new());
}

static NEXT_ID: AtomicU64 = AtomicU64::new(1);

/// The name Finder shows for the app, or its bundle identifier. NSWorkspace is not documented as safe off the main
/// thread, so there the answer is the bundle identifier.
fn app_name(bundle: &str) -> String {
    if bundle.is_empty() || MainThreadMarker::new().is_none() {
        return bundle.to_owned();
    }
    NAMES.with(|names| {
        names
            .borrow_mut()
            .entry(bundle.to_owned())
            .or_insert_with(|| workspace::app_name(bundle).unwrap_or_else(|| bundle.to_owned()))
            .clone()
    })
}

/// The adapter process while it runs; `stopped` once the watcher killed it on purpose.
struct Running {
    child: Child,
    stopped: bool,
}

struct Shared {
    id: u64,
    latest: Mutex<Option<Sample>>,
    active: AtomicBool,
    stop: AtomicBool,
    /// True while the adapter is the source, false once the watcher polls the players instead.
    streams: AtomicBool,
    kick: Mutex<bool>,
    wake: Condvar,
    running: Mutex<Option<Running>>,
}

impl Shared {
    fn wanted(&self) -> bool {
        self.active.load(Ordering::Acquire) && !self.stop.load(Ordering::Acquire)
    }

    /// Wakes the thread to look at the flags again, and to poll at once when `poll`. Taking the lock orders this
    /// after any check the thread is about to sleep on, so the wake cannot be missed.
    fn nudge(&self, poll: bool) {
        *self.kick.lock().unwrap_or_else(PoisonError::into_inner) |= poll;
        self.wake.notify_all();
    }

    /// Ends the adapter process, if one runs, because it is no longer wanted.
    fn end_stream(&self) {
        if let Some(running) = self
            .running
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .as_mut()
        {
            running.stopped = true;
            let _ = running.child.kill();
        }
    }

    fn set_latest(&self, sample: Option<Sample>) {
        *self.latest.lock().unwrap_or_else(PoisonError::into_inner) = sample;
    }

    /// Tells the listener on the main thread that what plays changed.
    fn changed(&self) {
        let id = self.id;
        app::on_main(move || {
            let listener = LISTENERS.with(|l| l.borrow().get(&id).cloned());
            if let Some(listener) = listener {
                listener();
            }
        });
    }
}

/// Follows what plays in the background, only while `set_active(true)`: through the adapter's stream, which
/// costs nothing while nothing changes, or, without it, by polling Music and Spotify.
pub struct Watcher {
    shared: Arc<Shared>,
    thread: Option<JoinHandle<()>>,
}

impl Watcher {
    /// Starts idle; nothing runs before `set_active(true)`.
    pub fn start() -> Self {
        Self::with_adapter(find_adapter().map(perl))
    }

    /// `adapter` starts the stream, `None` to poll the players.
    fn with_adapter(adapter: Option<Launch>) -> Self {
        let shared = Arc::new(Shared {
            id: NEXT_ID.fetch_add(1, Ordering::Relaxed),
            latest: Mutex::new(None),
            active: AtomicBool::new(false),
            stop: AtomicBool::new(false),
            streams: AtomicBool::new(adapter.is_some()),
            kick: Mutex::new(false),
            wake: Condvar::new(),
            running: Mutex::new(None),
        });
        let worker = Arc::clone(&shared);
        let thread = thread::Builder::new()
            .name("fig-now-playing".into())
            .spawn(move || run(&worker, adapter))
            .ok();
        Self { shared, thread }
    }

    /// While inactive the adapter process is not running and nothing is polled.
    pub fn set_active(&self, active: bool) {
        self.shared.active.store(active, Ordering::Release);
        if !active {
            self.shared.end_stream();
        }
        self.shared.nudge(active);
    }

    /// Asks for an immediate refresh instead of waiting for the next poll; the stream needs none.
    pub fn poll_now(&self) {
        self.shared.nudge(true);
    }

    /// True while the system's Now Playing stream feeds the watcher; false once it fell back to polling Music and
    /// Spotify, which costs a few processes every POLL_INTERVAL while active.
    pub fn streams(&self) -> bool {
        self.shared.streams.load(Ordering::Acquire)
    }

    pub fn latest(&self) -> Option<Track> {
        self.sample().map(|s| s.track)
    }

    /// The last look at what plays, with the app's name filled in (on the main thread; elsewhere it is the bundle
    /// identifier).
    pub fn sample(&self) -> Option<Sample> {
        let mut sample = self
            .shared
            .latest
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone()?;
        sample.track.app_name = app_name(&sample.track.app);
        Some(sample)
    }

    /// `f` runs on the main thread each time what plays changes, until the watcher is dropped. Call on the main
    /// thread; a second call replaces the first listener.
    pub fn set_on_change(&self, f: impl Fn() + 'static) {
        LISTENERS.with(|l| l.borrow_mut().insert(self.shared.id, Rc::new(f)));
    }
}

impl Drop for Watcher {
    fn drop(&mut self) {
        self.shared.stop.store(true, Ordering::Release);
        self.shared.end_stream();
        self.shared.nudge(false);
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
        let id = self.shared.id;
        let _ = LISTENERS.try_with(|l| l.borrow_mut().remove(&id));
    }
}

fn run(shared: &Shared, adapter: Option<Launch>) {
    if let Some(adapter) = adapter {
        if !stream(shared, &adapter) {
            return;
        }
        // The adapter cannot do the job here: forget what it said and ask the players, at once.
        shared.streams.store(false, Ordering::Release);
        shared.set_latest(None);
        shared.changed();
        shared.nudge(true);
    }
    poll(shared);
}

/// How a run of the adapter ended.
enum Ended {
    /// Killed on purpose: turned off or dropped.
    Asked,
    /// Ended without a single line: it cannot run here.
    Silent,
    /// Ended by itself after it worked.
    Broke,
}

/// Runs the adapter whenever the watcher is active, and again after a crash with a growing pause. Returns true
/// when the adapter cannot do the job, so the caller polls instead, and false when the watcher is dropped.
fn stream(shared: &Shared, adapter: &Launch) -> bool {
    let mut failures = 0;
    loop {
        drop(
            shared
                .wake
                .wait_while(
                    shared.kick.lock().unwrap_or_else(PoisonError::into_inner),
                    |_| {
                        !shared.active.load(Ordering::Acquire)
                            && !shared.stop.load(Ordering::Acquire)
                    },
                )
                .unwrap_or_else(PoisonError::into_inner),
        );
        if shared.stop.load(Ordering::Acquire) {
            return false;
        }
        let started = Instant::now();
        match run_adapter(shared, adapter) {
            Ended::Asked => {}
            Ended::Silent => return true,
            Ended::Broke => {
                failures = if started.elapsed() >= HEALTHY_RUN {
                    1
                } else {
                    failures + 1
                };
                if failures >= MAX_FAILURES {
                    return true;
                }
                // 1, 2, 4, 8 seconds, cut short by being turned off or dropped.
                let pause = Duration::from_secs(1 << (failures - 1));
                drop(
                    shared
                        .wake
                        .wait_timeout_while(
                            shared.kick.lock().unwrap_or_else(PoisonError::into_inner),
                            pause,
                            |_| shared.wanted(),
                        )
                        .unwrap_or_else(PoisonError::into_inner),
                );
            }
        }
    }
}

/// Starts the adapter and reads its lines until it ends.
fn run_adapter(shared: &Shared, adapter: &Launch) -> Ended {
    let Ok(mut child) = adapter()
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
    else {
        return Ended::Silent;
    };
    let Some(stdout) = child.stdout.take() else {
        let _ = child.kill();
        let _ = child.wait();
        return Ended::Silent;
    };
    {
        let mut running = shared
            .running
            .lock()
            .unwrap_or_else(PoisonError::into_inner);
        // Turned off or dropped since the thread woke up: the flags were set before end_stream took this lock,
        // so they show here, and the process goes at once.
        let stopped = !shared.wanted();
        if stopped {
            let _ = child.kill();
        }
        *running = Some(Running { child, stopped });
    }
    let mut reader = BufReader::with_capacity(64 << 10, stdout);
    let (mut line, mut artwork, mut heard) = (Vec::new(), None, false);
    while next_line(&mut reader, &mut line) {
        let parsed = std::str::from_utf8(&line)
            .ok()
            .and_then(|text| parse_line(text, &mut artwork, wall_ms()));
        if let Some(track) = parsed {
            heard = true;
            shared.set_latest(track.map(|track| Sample {
                track,
                at: Instant::now(),
            }));
            shared.changed();
        }
    }
    let running = shared
        .running
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .take();
    let Some(Running { mut child, stopped }) = running else {
        return Ended::Asked;
    };
    // It may have closed its output and lived on.
    let _ = child.kill();
    let _ = child.wait();
    match (stopped, heard) {
        (true, _) => Ended::Asked,
        (false, true) => Ended::Broke,
        (false, false) => Ended::Silent,
    }
}

/// Asks Music and Spotify every POLL_INTERVAL while active, and at once when nudged to; until dropped.
fn poll(shared: &Shared) {
    loop {
        {
            let mut kicked = shared.kick.lock().unwrap_or_else(PoisonError::into_inner);
            if !*kicked && !shared.stop.load(Ordering::Acquire) {
                kicked = if shared.active.load(Ordering::Acquire) {
                    shared
                        .wake
                        .wait_timeout(kicked, POLL_INTERVAL)
                        .unwrap_or_else(PoisonError::into_inner)
                        .0
                } else {
                    // Hidden tab: no timer at all, only set_active, poll_now or drop wake the thread.
                    shared
                        .wake
                        .wait_while(kicked, |asked| {
                            !*asked && !shared.stop.load(Ordering::Acquire)
                        })
                        .unwrap_or_else(PoisonError::into_inner)
                };
            }
            *kicked = false;
        }
        if shared.stop.load(Ordering::Acquire) {
            return;
        }
        if shared.active.load(Ordering::Acquire) {
            let sample = current();
            let changed = {
                let mut latest = shared.latest.lock().unwrap_or_else(PoisonError::into_inner);
                let changed = !same_track(latest.as_ref(), sample.as_ref());
                *latest = sample;
                changed
            };
            if changed {
                shared.changed();
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn line(state: &str, title: &str, artist: &str) -> String {
        format!("{state}{SEPARATOR}{title}{SEPARATOR}{artist}\n")
    }

    #[test]
    fn parses_playing_and_paused_tracks() {
        let t = parse(
            Player::Music,
            &line("playing", "Blue in Green", "Miles Davis"),
        )
        .unwrap();
        assert_eq!(
            t,
            Track {
                app: "com.apple.Music".into(),
                app_name: String::new(),
                title: "Blue in Green".into(),
                artist: "Miles Davis".into(),
                album: String::new(),
                playing: true,
                rate: 1.0,
                position_ms: 0,
                duration_ms: 0,
                artwork: None,
            }
        );
        let paused = parse(Player::Spotify, &line("paused", "Song", "Band")).unwrap();
        assert!(!paused.playing);
        assert_eq!(
            (paused.app.as_str(), paused.rate),
            ("com.spotify.client", 0.0)
        );
    }

    #[test]
    fn titles_may_contain_pipes_and_non_ascii_text() {
        let t = parse(
            Player::Music,
            &line("playing", "A | B - Nhạc Việt", "Ca sĩ"),
        )
        .unwrap();
        assert_eq!(
            (t.title.as_str(), t.artist.as_str()),
            ("A | B - Nhạc Việt", "Ca sĩ")
        );
    }

    #[test]
    fn empty_or_titleless_output_means_nothing_is_playing() {
        assert_eq!(parse(Player::Music, ""), None);
        assert_eq!(parse(Player::Music, "\n"), None);
        assert_eq!(parse(Player::Music, &line("playing", "  ", "Band")), None);
        assert_eq!(parse(Player::Music, "stopped"), None);
    }

    #[test]
    fn album_position_and_length_are_read_when_present() {
        let full = format!(
            "playing{SEPARATOR}Song{SEPARATOR}Band{SEPARATOR}Album{SEPARATOR}83250{SEPARATOR}233000\n"
        );
        let t = parse(Player::Spotify, &full).unwrap();
        assert_eq!(
            (t.album.as_str(), t.position_ms, t.duration_ms),
            ("Album", 83_250, 233_000)
        );
        // A stream has no position, and a hostile value is not a number: both are zero, never an error.
        let odd =
            format!("playing{SEPARATOR}Radio{SEPARATOR}Host{SEPARATOR}{SEPARATOR}{SEPARATOR}-5\n");
        let t = parse(Player::Music, &odd).unwrap();
        assert_eq!((t.album.as_str(), t.position_ms, t.duration_ms), ("", 0, 0));
        let huge = format!(
            "paused{SEPARATOR}S{SEPARATOR}B{SEPARATOR}A{SEPARATOR}99999999999999999999999{SEPARATOR}1\n"
        );
        assert_eq!(parse(Player::Music, &huge).unwrap().position_ms, 0);
    }

    #[test]
    fn the_position_moves_at_the_rate_while_playing_and_stays_inside_the_track() {
        let at = Instant::now();
        let mut sample = Sample {
            track: parse(
                Player::Music,
                &format!(
                    "playing{SEPARATOR}S{SEPARATOR}B{SEPARATOR}A{SEPARATOR}10000{SEPARATOR}12000"
                ),
            )
            .unwrap(),
            at,
        };
        assert_eq!(sample.position_ms(at + Duration::from_millis(500)), 10_500);
        assert_eq!(sample.position_ms(at + Duration::from_secs(60)), 12_000);
        assert_eq!(
            sample.position_ms(at.checked_sub(Duration::from_secs(1)).unwrap_or(at)),
            10_000
        );
        sample.track.rate = 2.0;
        assert_eq!(sample.position_ms(at + Duration::from_millis(500)), 11_000);
        sample.track.rate = -4.0;
        assert_eq!(sample.position_ms(at + Duration::from_secs(10)), 0);
        sample.track.rate = 1.0;
        sample.track.playing = false;
        assert_eq!(sample.position_ms(at + Duration::from_secs(5)), 10_000);
        sample.track.playing = true;
        sample.track.duration_ms = 0;
        assert_eq!(sample.position_ms(at + Duration::from_secs(60)), 70_000);
    }

    #[test]
    fn a_missing_artist_is_tolerated() {
        let t = parse(
            Player::Spotify,
            &format!("playing{SEPARATOR}Podcast episode"),
        )
        .unwrap();
        assert_eq!(
            (t.title.as_str(), t.artist.as_str()),
            ("Podcast episode", "")
        );
    }

    const NOW_MS: f64 = 1_792_049_130_000.0;

    fn adapter(line: &str) -> Option<Option<Track>> {
        parse_line(line, &mut None, NOW_MS)
    }

    #[test]
    fn a_complete_adapter_line_is_a_track_at_the_present_position() {
        let t = adapter(
            r#"{"playing":true,"title":" Video ","artist":"Channel","album":"","duration":2304.76,"elapsed":939.02,"rate":1.0,"at_ms":1792049126808,"bundle":"com.microsoft.edgemac","parent":"","pid":16572}"#,
        )
        .unwrap()
        .unwrap();
        assert_eq!(
            t,
            Track {
                app: "com.microsoft.edgemac".into(),
                app_name: String::new(),
                title: "Video".into(),
                artist: "Channel".into(),
                album: String::new(),
                playing: true,
                rate: 1.0,
                // 939.02 s at the timestamp, 3.192 s before now.
                position_ms: 942_212,
                duration_ms: 2_304_760,
                artwork: None,
            }
        );
    }

    #[test]
    fn a_browser_helper_counts_as_its_browser_and_the_rate_carries_the_position() {
        let t = adapter(
            r#"{"playing":true,"title":"T","elapsed":10,"rate":2,"at_ms":1792049129000,"bundle":"com.apple.WebKit.GPU","parent":"com.apple.Safari"}"#,
        )
        .unwrap()
        .unwrap();
        assert_eq!(
            (t.app.as_str(), t.rate, t.position_ms),
            ("com.apple.Safari", 2.0, 12_000)
        );
        // Paused: no rate and no movement, whatever the line says.
        let t = adapter(r#"{"playing":false,"title":"T","elapsed":10,"rate":1,"at_ms":1}"#)
            .unwrap()
            .unwrap();
        assert_eq!((t.rate, t.position_ms), (0.0, 10_000));
        // A timestamp in the future does not move the position back.
        let t = adapter(r#"{"playing":true,"title":"T","elapsed":10,"at_ms":1792049190000}"#)
            .unwrap()
            .unwrap();
        assert_eq!(t.position_ms, 10_000);
    }

    #[test]
    fn missing_null_and_wrongly_typed_fields_are_unknown() {
        let t = adapter(r#"{"title":"T"}"#).unwrap().unwrap();
        assert_eq!(
            (
                t.playing,
                t.rate,
                t.position_ms,
                t.duration_ms,
                t.app.as_str()
            ),
            (false, 0.0, 0, 0, "")
        );
        let t = adapter(
            r#"{"playing":true,"title":"T","artist":null,"duration":null,"elapsed":null,"rate":null,"at_ms":null,"bundle":null,"parent":null,"pid":null}"#,
        )
        .unwrap()
        .unwrap();
        assert_eq!(
            (t.rate, t.position_ms, t.duration_ms, t.artist.as_str()),
            (1.0, 0, 0, "")
        );
        let t = adapter(
            r#"{"playing":"yes","title":"T","artist":5,"duration":"long","elapsed":[1],"rate":{},"at_ms":"now","bundle":7}"#,
        )
        .unwrap()
        .unwrap();
        assert_eq!(
            (
                t.playing,
                t.artist.as_str(),
                t.duration_ms,
                t.position_ms,
                t.app.as_str()
            ),
            (false, "", 0, 0, "")
        );
        assert_eq!(
            adapter(r#"{"playing":true,"title":-1,"duration":-5}"#),
            Some(None)
        );
        // Out of range numbers saturate instead of wrapping.
        let t = adapter(r#"{"title":"T","duration":-5,"elapsed":1e300}"#)
            .unwrap()
            .unwrap();
        assert_eq!((t.duration_ms, t.position_ms), (0, u64::MAX));
    }

    #[test]
    fn nothing_playing_is_a_line_without_a_title() {
        assert_eq!(
            adapter(
                r#"{"playing":false,"title":"","artist":"","album":"","duration":null,"elapsed":null,"rate":null,"at_ms":1792049126808,"bundle":"","parent":"","pid":0}"#
            ),
            Some(None)
        );
        assert_eq!(adapter(r#"{"playing":true,"title":"   "}"#), Some(None));
    }

    #[test]
    fn garbage_is_not_a_line() {
        for garbage in [
            "",
            "   ",
            "not json",
            "{",
            r#"{"title":"T""#,
            "[1,2,3]",
            "42",
            "\"title\"",
            "null",
            "{\"title\":\"T\"} trailing",
        ] {
            assert_eq!(adapter(garbage), None, "{garbage:?}");
        }
    }

    #[test]
    fn the_artwork_stays_until_a_line_changes_or_removes_it() {
        let mut artwork = None;
        let mut read = |line: &str| {
            parse_line(line, &mut artwork, NOW_MS)
                .unwrap()
                .unwrap()
                .artwork
        };
        let first = read(r#"{"title":"T","artwork":"Zm9vYmFy","mime":"image/jpeg"}"#).unwrap();
        assert_eq!(first.bytes.as_slice(), b"foobar");
        let same = read(r#"{"title":"T"}"#).unwrap();
        assert_eq!(same.id, first.id);
        assert!(Arc::ptr_eq(&same.bytes, &first.bytes), "kept, not copied");
        assert!(
            read(r#"{"title":"T","artwork":42}"#).is_some(),
            "not a picture, no change"
        );
        let other = read(r#"{"title":"Next","artwork":"Zm9v","mime":"image/png"}"#).unwrap();
        assert_ne!(other.id, first.id);
        assert_eq!(read(r#"{"title":"T","artwork":"","mime":""}"#), None);
        assert_eq!(read(r#"{"title":"T"}"#), None);
        assert_eq!(read(r#"{"title":"T","artwork":"not base64!"}"#), None);
        // A line about nothing playing can still carry the next picture.
        assert_eq!(
            parse_line(r#"{"title":"","artwork":"Zm9v"}"#, &mut artwork, NOW_MS),
            Some(None)
        );
        assert_eq!(artwork.map(|a| a.bytes.to_vec()), Some(b"foo".to_vec()));
    }

    #[test]
    fn base64_decodes_what_the_adapter_encodes_and_nothing_else() {
        for (encoded, plain) in [
            ("", &b""[..]),
            ("Zg==", b"f"),
            ("Zm8=", b"fo"),
            ("Zm9v", b"foo"),
            ("Zm9vYg==", b"foob"),
            ("Zm9vYmE=", b"fooba"),
            ("Zm9vYmFy", b"foobar"),
            ("//79", &[0xff, 0xfe, 0xfd]),
        ] {
            assert_eq!(
                base64_decode(encoded).as_deref(),
                Some(plain),
                "{encoded:?}"
            );
        }
        for bad in [
            "Zg=",
            "Z===",
            "Zg==Zg==",
            "Z=g=",
            "Zm9v!A==",
            "Zm9v\nYmFy",
            "é123",
        ] {
            assert_eq!(base64_decode(bad), None, "{bad:?}");
        }
    }

    #[test]
    fn huge_lines_are_skipped_and_the_next_line_is_read() {
        let mut input = Vec::new();
        input.extend_from_slice(br#"{"title":"Before"}"#);
        input.push(b'\n');
        input.extend(std::iter::repeat_n(b'x', MAX_LINE + 10));
        input.push(b'\n');
        input.extend_from_slice(br#"{"title":"After"}"#);
        let mut reader = std::io::Cursor::new(input);
        let mut line = Vec::new();
        let mut lines = Vec::new();
        while next_line(&mut reader, &mut line) {
            lines.push(String::from_utf8(line.clone()).unwrap());
        }
        assert_eq!(lines, [r#"{"title":"Before"}"#, "", r#"{"title":"After"}"#]);
        // A big picture within the limit is a line like any other.
        let picture = "QUJD".repeat(1 << 20);
        let big = format!(r#"{{"title":"T","artwork":"{picture}"}}"#);
        let t = adapter(&big).unwrap().unwrap();
        assert_eq!(t.artwork.unwrap().bytes.len(), 3 << 20);
    }

    #[test]
    fn helper_commands_are_bounded_by_the_timeout() {
        let out = run_capture(
            Command::new("/bin/echo").arg("hello"),
            Duration::from_secs(2),
        );
        assert_eq!(out.as_deref(), Some("hello\n"));
        let started = Instant::now();
        assert_eq!(
            run_capture(
                Command::new("/bin/sleep").arg("5"),
                Duration::from_millis(150)
            ),
            None
        );
        assert!(
            started.elapsed() < Duration::from_secs(2),
            "a hung helper must be killed"
        );
        assert_eq!(
            run_capture(&mut Command::new("/bin/false"), Duration::from_secs(2)),
            None
        );
        assert_eq!(
            run_capture(
                &mut Command::new("/nonexistent/binary"),
                Duration::from_secs(1)
            ),
            None
        );
    }

    /// Compiling reads the scripting dictionary from disk and never launches the player.
    #[test]
    fn scripts_compile_against_the_players_that_are_installed() {
        let apps = [
            (Player::Music, "/System/Applications/Music.app"),
            (Player::Spotify, "/Applications/Spotify.app"),
        ];
        for (player, path) in apps {
            if !std::path::Path::new(path).exists() {
                continue;
            }
            let dir = std::env::temp_dir().join(format!(
                "figghost-osacompile-{}-{}",
                std::process::id(),
                player.name()
            ));
            std::fs::create_dir_all(&dir).unwrap();
            let (source, output) = (dir.join("probe.applescript"), dir.join("probe.scpt"));
            std::fs::write(&source, player.script()).unwrap();
            let compiled = run_capture(
                Command::new("osacompile")
                    .arg("-o")
                    .arg(&output)
                    .arg(&source),
                Duration::from_secs(20),
            );
            let _ = std::fs::remove_dir_all(&dir);
            assert!(
                compiled.is_some(),
                "the {} script does not compile",
                player.name()
            );
        }
    }

    /// Runs the real script against a player that is closed: it must answer "nothing" and must not
    /// launch the app, which a plain `tell application` would do.
    #[test]
    fn the_script_never_launches_a_closed_player() {
        for player in Player::ALL {
            if is_running(player) {
                continue;
            }
            let out = run_capture(
                Command::new("osascript").arg("-e").arg(player.script()),
                SCRIPT_TIMEOUT,
            );
            assert_eq!(
                out.as_deref().map(str::trim),
                Some(""),
                "{} answered",
                player.name()
            );
            thread::sleep(Duration::from_millis(400));
            assert!(
                !is_running(player),
                "{} was launched by the script",
                player.name()
            );
        }
    }

    #[test]
    fn scripts_target_the_right_application() {
        assert!(Player::Music.script().contains("com.apple.Music"));
        assert!(Player::Spotify.script().contains("com.spotify.client"));
        assert!(Player::Music.script().contains("ASCII character 31"));
        assert!(Player::Music.script().contains("current track) * 1000"));
        assert!(!Player::Spotify.script().contains("current track) * 1000"));
        assert!(Player::Spotify.script().contains("player position"));
    }

    #[test]
    fn a_new_watcher_is_idle_and_stops_cleanly() {
        let watcher = Watcher::start();
        assert_eq!(watcher.latest(), None);
        watcher.set_active(true);
        watcher.set_active(false);
        drop(watcher);
    }

    #[test]
    fn without_an_adapter_the_watcher_polls() {
        let watcher = Watcher::with_adapter(None);
        assert!(!watcher.streams());
    }

    /// perl starts, cannot load the file and exits without a line: the watcher must give the adapter up.
    #[test]
    fn a_bogus_adapter_falls_back_to_polling() {
        let dir = std::env::temp_dir().join(format!("figghost-np-bogus-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let not_a_library = dir.join(ADAPTER);
        std::fs::write(&not_a_library, "not a library").unwrap();
        for path in [not_a_library.clone(), dir.join("missing.dylib")] {
            let watcher = Watcher::with_adapter(Some(perl(path.clone())));
            assert!(watcher.streams());
            watcher.set_active(true);
            let started = Instant::now();
            while watcher.streams() && started.elapsed() < Duration::from_secs(10) {
                thread::sleep(Duration::from_millis(20));
            }
            assert!(!watcher.streams(), "{} was not given up", path.display());
            watcher.set_active(false);
        }
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// A stand-in for the adapter: a shell script, which gets the file `log` as $1.
    fn fake(script: &'static str, log: &Path) -> Launch {
        let log = log.to_owned();
        Box::new(move || {
            let mut command = Command::new("/bin/sh");
            command.args(["-c", script, "fake"]).arg(&log);
            command
        })
    }

    fn wait_for(what: impl Fn() -> bool) -> bool {
        let started = Instant::now();
        while !what() && started.elapsed() < Duration::from_secs(10) {
            thread::sleep(Duration::from_millis(20));
        }
        what()
    }

    /// The process ids the stand-in wrote, one per start.
    fn starts(log: &Path) -> Vec<String> {
        std::fs::read_to_string(log)
            .unwrap_or_default()
            .lines()
            .map(str::to_owned)
            .collect()
    }

    fn alive(pid: &str) -> bool {
        Command::new("/bin/kill")
            .args(["-0", pid])
            .stderr(Stdio::null())
            .status()
            .is_ok_and(|s| s.success())
    }

    fn log_file(name: &str) -> PathBuf {
        let log = std::env::temp_dir().join(format!("figghost-np-{name}-{}", std::process::id()));
        let _ = std::fs::remove_file(&log);
        log
    }

    #[test]
    fn an_adapter_that_dies_after_working_is_started_again() {
        let log = log_file("restart");
        let watcher = Watcher::with_adapter(Some(fake(
            r#"echo $$ >> "$1"; echo '{"playing":true,"title":"T"}'; exit 3"#,
            &log,
        )));
        watcher.set_active(true);
        assert!(wait_for(|| starts(&log).len() >= 2), "not started again");
        assert!(watcher.streams(), "one crash is not a reason to give up");
        assert_eq!(watcher.latest().map(|t| t.title), Some("T".to_owned()));
        drop(watcher);
        let _ = std::fs::remove_file(&log);
    }

    #[test]
    fn the_adapter_runs_only_while_active_and_dies_with_the_watcher() {
        let log = log_file("lifetime");
        let watcher = Watcher::with_adapter(Some(fake(
            r#"echo $$ >> "$1"; echo '{"playing":true,"title":"T"}'; exec sleep 30"#,
            &log,
        )));
        thread::sleep(Duration::from_millis(200));
        assert!(starts(&log).is_empty(), "started while inactive");
        watcher.set_active(true);
        assert!(wait_for(|| watcher.latest().is_some()));
        let first = starts(&log)[0].clone();
        assert!(alive(&first));
        watcher.set_active(false);
        assert!(wait_for(|| !alive(&first)), "still running while inactive");
        watcher.set_active(true);
        assert!(wait_for(|| starts(&log).len() == 2), "not started again");
        let second = starts(&log)[1].clone();
        assert!(alive(&second));
        assert!(watcher.streams(), "being turned off is not a failure");
        drop(watcher);
        assert!(!alive(&second), "outlived the watcher");
        let _ = std::fs::remove_file(&log);
    }

    #[test]
    fn same_track_ignores_the_position_only() {
        let sample = |position_ms, playing| Sample {
            track: Track {
                position_ms,
                playing,
                ..parse(Player::Music, &line("playing", "S", "B")).unwrap()
            },
            at: Instant::now(),
        };
        assert!(same_track(None, None));
        assert!(same_track(
            Some(&sample(1_000, true)),
            Some(&sample(9_000, true))
        ));
        assert!(!same_track(
            Some(&sample(1_000, true)),
            Some(&sample(1_000, false))
        ));
        assert!(!same_track(Some(&sample(1_000, true)), None));
    }
}
