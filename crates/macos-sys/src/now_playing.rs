//! What Music and Spotify are playing, read through AppleScript in a helper process.
//!
//! The private MediaRemote framework, which would cover every app, is closed to third-party processes on
//! recent macOS, so this covers the two players that script their state. The helper process has a hard
//! timeout, so a hung player can never block the UI, and a player that is not running is never launched.

use std::{
    io::Read,
    process::{Command, Stdio},
    sync::{
        Arc, Condvar, Mutex, PoisonError,
        atomic::{AtomicBool, Ordering},
    },
    thread::{self, JoinHandle},
    time::{Duration, Instant},
};

const SEPARATOR: char = '\u{1f}';
const POLL_INTERVAL: Duration = Duration::from_millis(1500);
const SCRIPT_TIMEOUT: Duration = Duration::from_secs(3);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Player {
    Music,
    Spotify,
}

impl Player {
    const ALL: [Player; 2] = [Player::Music, Player::Spotify];

    pub const fn name(self) -> &'static str {
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Track {
    pub player: Player,
    pub title: String,
    pub artist: String,
    pub album: String,
    pub playing: bool,
    /// Where the player is in the track when the script ran; 0 when it does not say.
    pub position_ms: u64,
    /// 0 when the player does not say.
    pub duration_ms: u64,
}

/// A track with the moment it was read, so that the position can be carried forward while it plays.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Sample {
    pub track: Track,
    pub at: Instant,
}

impl Sample {
    /// Where the player is now: the position it reported plus the time since, while it plays.
    pub fn position_ms(&self, now: Instant) -> u64 {
        let moved = if self.track.playing {
            u64::try_from(now.saturating_duration_since(self.at).as_millis()).unwrap_or(0)
        } else {
            0
        };
        let at = self.track.position_ms.saturating_add(moved);
        match self.track.duration_ms {
            0 => at,
            length => at.min(length),
        }
    }
}

/// Parses the script's `state<US>title<US>artist<US>album<US>position<US>length` line; everything after the
/// artist may be missing. An empty line, or one without a title, means nothing is playing.
pub fn parse(player: Player, output: &str) -> Option<Track> {
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
    (!title.is_empty()).then(|| Track {
        player,
        title: title.to_owned(),
        artist: artist.to_owned(),
        album: album.to_owned(),
        playing: state.trim() == "playing",
        position_ms,
        duration_ms,
    })
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

struct Shared {
    latest: Mutex<Option<Sample>>,
    active: AtomicBool,
    stop: AtomicBool,
    kick: Mutex<bool>,
    wake: Condvar,
}

/// Polls in the background, and only while `set_active(true)`, so an idle app costs nothing.
pub struct Watcher {
    shared: Arc<Shared>,
    thread: Option<JoinHandle<()>>,
}

impl Watcher {
    pub fn start() -> Self {
        let shared = Arc::new(Shared {
            latest: Mutex::new(None),
            active: AtomicBool::new(false),
            stop: AtomicBool::new(false),
            kick: Mutex::new(false),
            wake: Condvar::new(),
        });
        let worker = Arc::clone(&shared);
        let thread = thread::Builder::new()
            .name("fig-now-playing".into())
            .spawn(move || run(&worker))
            .ok();
        Self { shared, thread }
    }

    pub fn set_active(&self, active: bool) {
        self.shared.active.store(active, Ordering::Release);
        if active {
            self.poll_now();
        }
    }

    /// Asks for an immediate refresh instead of waiting for the next poll.
    pub fn poll_now(&self) {
        *self
            .shared
            .kick
            .lock()
            .unwrap_or_else(PoisonError::into_inner) = true;
        self.shared.wake.notify_all();
    }

    pub fn latest(&self) -> Option<Track> {
        self.sample().map(|s| s.track)
    }

    pub fn sample(&self) -> Option<Sample> {
        self.shared
            .latest
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone()
    }
}

impl Drop for Watcher {
    fn drop(&mut self) {
        self.shared.stop.store(true, Ordering::Release);
        self.shared.wake.notify_all();
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

fn run(shared: &Shared) {
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
            *shared.latest.lock().unwrap_or_else(PoisonError::into_inner) = sample;
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
                player: Player::Music,
                title: "Blue in Green".into(),
                artist: "Miles Davis".into(),
                album: String::new(),
                playing: true,
                position_ms: 0,
                duration_ms: 0
            }
        );
        assert!(
            !parse(Player::Spotify, &line("paused", "Song", "Band"))
                .unwrap()
                .playing
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
    fn the_position_moves_while_playing_and_stays_inside_the_track() {
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
}
