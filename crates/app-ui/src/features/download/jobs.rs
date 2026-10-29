//! The downloads in progress: a queue that lets two run at a time, a thread for each that reads what yt-dlp prints and, when
//! it left a picture and a sound apart, joins them, and the ways to stop one. There is no AppKit here, so the tests run it against a stand-in for yt-dlp.

use super::logic::{self, Line, Plan, Quality};
use macos_sys::process::kill_group;
use std::{
    fs,
    io::{BufRead, BufReader},
    os::unix::{fs::DirBuilderExt, process::CommandExt},
    path::{Path, PathBuf},
    process::{Command, Stdio},
    sync::{Arc, Mutex, MutexGuard},
    thread,
};

/// How many downloads run at once; the others wait their turn.
pub const MAX_RUNNING: usize = 2;
/// How many finished downloads are remembered, so that a long session does not grow the list for ever.
const KEEP_FINISHED: usize = 30;
/// What yt-dlp sees of the environment: the places a Mac keeps tools such as ffmpeg and a JavaScript runtime, since an
/// app started from the Finder has no search path of its own.
pub const SEARCH_PATH: &str = "/opt/homebrew/bin:/usr/local/bin:/usr/bin:/bin";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Status {
    Queued,
    Running,
    /// The picture and the sound yt-dlp left apart are being made into one file.
    Joining,
    Done,
    Failed,
    Cancelled,
}

impl Status {
    pub fn key(self) -> &'static str {
        match self {
            Status::Queued => "queued",
            Status::Running => "running",
            Status::Joining => "joining",
            Status::Done => "done",
            Status::Failed => "failed",
            Status::Cancelled => "cancelled",
        }
    }

    pub fn over(self) -> bool {
        !matches!(self, Status::Queued | Status::Running | Status::Joining)
    }
}

/// One download as the page and the notch see it.
#[derive(Debug, Clone)]
pub struct Job {
    pub id: u64,
    pub url: String,
    pub title: Option<String>,
    pub status: Status,
    pub percent: Option<f64>,
    pub speed: Option<f64>,
    pub eta_seconds: Option<u64>,
    /// The last file written, when there is one.
    pub file: Option<PathBuf>,
    /// The folder it goes to.
    pub folder: PathBuf,
    pub error: Option<String>,
    /// The error is that the video needs ffmpeg, which the page explains better than yt-dlp does.
    pub needs_ffmpeg: bool,
    /// Counts the downloads that have ended, so that a reader can tell which ones it has not seen end yet. 0 while it goes.
    pub ended: u64,
}

/// What a download needs to start.
#[derive(Debug, Clone)]
pub struct Request {
    pub program: PathBuf,
    pub url: String,
    pub folder: PathBuf,
    pub quality: Quality,
    pub playlist: bool,
    pub ffmpeg: Option<PathBuf>,
    /// net-merge: the proxy URL yt-dlp goes through.
    pub proxy: Option<String>,
}

impl Request {
    /// A sound asked for without ffmpeg: yt-dlp keeps the m4a as the site served it, in fragments.
    fn is_bare_sound(&self) -> bool {
        self.quality == Quality::Audio && self.ffmpeg.is_none()
    }
}

struct Entry {
    job: Job,
    /// Taken when the download starts.
    request: Option<Request>,
    /// The process group of yt-dlp while it runs.
    group: Option<u32>,
    cancelled: bool,
    /// The files yt-dlp said it wrote, in order.
    reported: Vec<PathBuf>,
}

#[derive(Default)]
struct Inner {
    next: u64,
    ended: u64,
    entries: Vec<Entry>,
}

/// The queue. Cloning gives another handle to the same one.
#[derive(Clone, Default)]
pub struct Engine {
    inner: Arc<Mutex<Inner>>,
}

fn lock(inner: &Mutex<Inner>) -> MutexGuard<'_, Inner> {
    inner.lock().unwrap_or_else(|e| e.into_inner())
}

impl Engine {
    /// Queues a download and starts it if there is a free place. The number it is known by.
    pub fn submit(&self, request: Request) -> u64 {
        let id = {
            let mut inner = lock(&self.inner);
            inner.next += 1;
            let id = inner.next;
            let job = Job {
                id,
                url: request.url.clone(),
                title: None,
                status: Status::Queued,
                percent: None,
                speed: None,
                eta_seconds: None,
                file: None,
                folder: request.folder.clone(),
                error: None,
                needs_ffmpeg: false,
                ended: 0,
            };
            inner.entries.push(Entry {
                job,
                request: Some(request),
                group: None,
                cancelled: false,
                reported: Vec::new(),
            });
            forget_old(&mut inner);
            id
        };
        start_waiting(&self.inner);
        id
    }

    /// Stops a download that waits or runs, and what it started with it. One that is over is left as it is, and so is one
    /// that joins its parts, which is a matter of a moment and ends with the file whole or the parts kept.
    pub fn cancel(&self, id: u64) {
        let mut inner = lock(&self.inner);
        let ended = inner.ended + 1;
        let Some(entry) = inner.entries.iter_mut().find(|e| e.job.id == id) else {
            return;
        };
        match entry.job.status {
            Status::Queued => {
                entry.request = None;
                entry.job.status = Status::Cancelled;
                entry.job.ended = ended;
                inner.ended = ended;
            }
            Status::Running => {
                entry.cancelled = true;
                if let Some(group) = entry.group {
                    kill_group(group);
                }
            }
            _ => {}
        }
    }

    /// Stops every download, for when the feature is turned off or the app quits.
    pub fn stop_all(&self) {
        let ids: Vec<u64> = lock(&self.inner)
            .entries
            .iter()
            .filter(|e| !e.job.status.over())
            .map(|e| e.job.id)
            .collect();
        // Waiting ones first, so that a place freed by a stopped one is not given to another.
        let mut ids = ids;
        ids.sort_by_key(|id| {
            lock(&self.inner)
                .entries
                .iter()
                .find(|e| e.job.id == *id)
                .map_or(0, |e| u8::from(e.job.status == Status::Running))
        });
        for id in ids {
            self.cancel(id);
        }
    }

    /// For the app quitting, which leaves no time for the threads to tidy up: stops everything and removes the folders of
    /// what was not finished.
    pub fn shutdown(&self) {
        self.stop_all();
        let temps: Vec<PathBuf> = lock(&self.inner)
            .entries
            .iter()
            .filter(|e| matches!(e.job.status, Status::Running | Status::Joining))
            .map(|e| temp_dir(&e.job.folder, e.job.id))
            .collect();
        for temp in temps {
            let _ = fs::remove_dir_all(temp);
        }
    }

    pub fn snapshot(&self) -> Vec<Job> {
        lock(&self.inner)
            .entries
            .iter()
            .map(|e| e.job.clone())
            .collect()
    }

    pub fn job(&self, id: u64) -> Option<Job> {
        lock(&self.inner)
            .entries
            .iter()
            .find(|e| e.job.id == id)
            .map(|e| e.job.clone())
    }

    /// Takes the downloads that are over off the list.
    pub fn clear_finished(&self) {
        lock(&self.inner).entries.retain(|e| !e.job.status.over());
    }
}

/// Where yt-dlp keeps what it has not finished, in a hidden folder inside the one downloads go to: no half-written file shows
/// in the folder, and the live islands, which take a name that starts with a dot for a file that is not complete, say
/// nothing of it.
fn temp_dir(folder: &Path, id: u64) -> PathBuf {
    folder.join(format!(".figghost-download-{}-{id}", std::process::id()))
}

/// Drops the oldest finished downloads beyond the ones kept.
fn forget_old(inner: &mut Inner) {
    let finished = inner.entries.iter().filter(|e| e.job.status.over()).count();
    let mut surplus = finished.saturating_sub(KEEP_FINISHED);
    inner.entries.retain(|e| {
        if surplus > 0 && e.job.status.over() {
            surplus -= 1;
            return false;
        }
        true
    });
}

/// Starts waiting downloads, oldest first, while fewer than `MAX_RUNNING` run.
fn start_waiting(shared: &Arc<Mutex<Inner>>) {
    let mut starting = Vec::new();
    {
        let mut inner = lock(shared);
        let mut running = inner
            .entries
            .iter()
            .filter(|e| matches!(e.job.status, Status::Running | Status::Joining))
            .count();
        for entry in &mut inner.entries {
            if running >= MAX_RUNNING {
                break;
            }
            if entry.job.status == Status::Queued
                && let Some(request) = entry.request.take()
            {
                entry.job.status = Status::Running;
                running += 1;
                starting.push((entry.job.id, request));
            }
        }
    }
    for (id, request) in starting {
        let shared = Arc::clone(shared);
        thread::spawn(move || run(&shared, id, request));
    }
}

/// Changes one entry under the lock.
fn update(shared: &Mutex<Inner>, id: u64, change: impl FnOnce(&mut Entry)) {
    if let Some(entry) = lock(shared).entries.iter_mut().find(|e| e.job.id == id) {
        change(entry);
    }
}

/// How a download ended, before the status is worked out.
struct Outcome {
    success: bool,
    error: Option<String>,
}

fn run(shared: &Arc<Mutex<Inner>>, id: u64, request: Request) {
    let temp = temp_dir(&request.folder, id);
    let mut outcome = fetch(shared, id, &request, &temp);
    if outcome.success
        && let Err(why) = join_parts(shared, id, &temp, request.is_bare_sound())
    {
        outcome = Outcome {
            success: false,
            error: Some(why),
        };
    }
    let _ = fs::remove_dir_all(&temp);
    {
        let mut inner = lock(shared);
        inner.ended += 1;
        let ended = inner.ended;
        if let Some(entry) = inner.entries.iter_mut().find(|e| e.job.id == id) {
            entry.group = None;
            entry.job.ended = ended;
            entry.job.speed = None;
            entry.job.eta_seconds = None;
            if entry.cancelled {
                entry.job.status = Status::Cancelled;
            } else if outcome.success {
                entry.job.status = Status::Done;
                entry.job.percent = Some(100.0);
            } else {
                let error = outcome
                    .error
                    .unwrap_or_else(|| "yt-dlp stopped without saying why".into());
                entry.job.needs_ffmpeg = logic::needs_ffmpeg(&error, request.ffmpeg.is_some());
                entry.job.error = Some(error);
                entry.job.status = Status::Failed;
            }
        }
    }
    start_waiting(shared);
}

/// Runs yt-dlp and follows it until it ends.
fn fetch(shared: &Arc<Mutex<Inner>>, id: u64, request: &Request, temp: &Path) -> Outcome {
    let failed = |why: &str| Outcome {
        success: false,
        error: Some(why.to_owned()),
    };
    if fs::DirBuilder::new()
        .recursive(true)
        .mode(0o700)
        .create(temp)
        .is_err()
    {
        return failed("The folder cannot be written to");
    }
    let args = logic::build_args(&Plan {
        url: &request.url,
        folder: &request.folder,
        temp,
        quality: request.quality,
        playlist: request.playlist,
        ffmpeg: request.ffmpeg.as_deref(),
        proxy: request.proxy.as_deref(),
    });
    let search = match std::env::var("PATH") {
        Ok(own) if !own.is_empty() => format!("{SEARCH_PATH}:{own}"),
        _ => SEARCH_PATH.to_owned(),
    };
    let spawned = Command::new(&request.program)
        .args(&args)
        .env("PATH", search)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        // A group of its own, so that what it starts, ffmpeg above all, is stopped with it.
        .process_group(0)
        .spawn();
    let Ok(mut child) = spawned else {
        return failed("yt-dlp could not be started");
    };
    let group = child.id();
    let wanted_stop = {
        let mut inner = lock(shared);
        let entry = inner.entries.iter_mut().find(|e| e.job.id == id);
        entry.is_none_or(|e| {
            e.group = Some(group);
            e.cancelled
        })
    };
    if wanted_stop {
        // Stopped between its start and the moment its group was known.
        kill_group(group);
    }
    let errors = child.stderr.take().map(|stderr| {
        thread::spawn(move || {
            let mut last = None;
            for line in BufReader::new(stderr).lines().map_while(Result::ok) {
                if let Line::Error(error) = logic::parse_line(&line) {
                    last = Some(error);
                }
            }
            last
        })
    });
    if let Some(stdout) = child.stdout.take() {
        for line in BufReader::new(stdout).lines().map_while(Result::ok) {
            read(shared, id, &line);
        }
    }
    let status = child.wait();
    // Anything the tool left running holds the pipes of the thread above open.
    kill_group(group);
    let error = errors.and_then(|t| t.join().ok()).flatten();
    Outcome {
        success: status.is_ok_and(|s| s.success()),
        error,
    }
}

/// yt-dlp without ffmpeg leaves a picture and a sound apart and still names the joined file, which is not there: this
/// makes the one file of them with AVFoundation, without encoding, and removes the two. Failing keeps both, since they are
/// what was downloaded. The words of the error are the person's to read. A sound asked for (`bare_sound`) is a single m4a
/// that is written again the same way, since the fragmented one of a site is not counted right by every player; the file
/// as downloaded stays when that fails.
fn join_parts(shared: &Mutex<Inner>, id: u64, temp: &Path, bare_sound: bool) -> Result<(), String> {
    let (reported, cancelled) = {
        let mut inner = lock(shared);
        inner
            .entries
            .iter_mut()
            .find(|e| e.job.id == id)
            .map_or((Vec::new(), true), |e| {
                (std::mem::take(&mut e.reported), e.cancelled)
            })
    };
    if cancelled {
        return Ok(());
    }
    if bare_sound {
        for sound in reported
            .iter()
            .filter(|p| p.extension().is_some_and(|e| e == "m4a") && p.is_file())
        {
            update(shared, id, |e| e.job.status = Status::Joining);
            let tidy = temp.join("tidy.m4a");
            macos_sys::mux::rewrite_sound(sound, &tidy)
                .and_then(|()| fs::rename(&tidy, sound).map_err(|e| e.to_string()))
                .map_err(|why| {
                    format!("The sound could not be written again as a plain m4a ({why}). The file is kept as it was downloaded and may show the wrong length in some players.")
                })?;
        }
    }
    for reported in reported.into_iter().filter(|p| !p.exists()) {
        let Some(parts) = logic::find_parts(&reported) else {
            continue;
        };
        update(shared, id, |e| e.job.status = Status::Joining);
        let joined = temp.join("joined.mp4");
        let target = logic::free_name(&reported, |p| p.exists());
        // A link and not a rename, so that a file that appeared meanwhile under the name is never replaced.
        macos_sys::mux::join(&parts.video, &parts.audio, &joined)
            .and_then(|()| fs::hard_link(&joined, &target).map_err(|e| e.to_string()))
            .map_err(|why| {
                format!("The picture and the sound could not be joined ({why}). Both parts are kept in the folder.")
            })?;
        let _ = fs::remove_file(&parts.video);
        let _ = fs::remove_file(&parts.audio);
        update(shared, id, |e| e.job.file = Some(target));
    }
    Ok(())
}

/// Applies one line of what yt-dlp printed.
fn read(shared: &Mutex<Inner>, id: u64, line: &str) {
    match logic::parse_line(line) {
        Line::Title(title) => update(shared, id, |e| {
            e.job.title = Some(title).filter(|t| !t.is_empty());
        }),
        Line::Progress(p) => update(shared, id, |e| {
            // ponytail: with a video and an audio stream fetched apart the percentage starts again for the second;
            // one share across both streams would need yt-dlp's list of formats.
            e.job.percent = p.percent.or(e.job.percent);
            e.job.speed = p.speed;
            e.job.eta_seconds = p.eta_seconds;
        }),
        Line::File(path) => update(shared, id, |e| {
            e.reported.push(path.clone());
            e.job.file = Some(path);
        }),
        Line::Error(_) | Line::Other => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        os::unix::fs::PermissionsExt,
        process::Command,
        time::{Duration, Instant},
    };

    /// A directory that is removed with it.
    struct Dir(PathBuf);

    impl Dir {
        fn new(name: &str) -> Self {
            let path =
                std::env::temp_dir().join(format!("figghost-dl-{name}-{}", std::process::id()));
            let _ = fs::remove_dir_all(&path);
            fs::create_dir_all(&path).unwrap();
            Self(path)
        }
    }

    impl Drop for Dir {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    /// A stand-in for yt-dlp that does what the last part of the link says: `ok` prints progress and writes a file,
    /// `fail` says an error and fails, `slow` starts a sleep of the number of seconds given to `fake` and waits for it,
    /// `parts` and `joinable` leave a picture and a sound apart as yt-dlp does without ffmpeg while naming the joined file,
    /// with parts that cannot be joined or ones that can (the picture is the sample of the tests of `macos-sys`, which has
    /// a sound of its own, so that nothing but the picture is needed). It keeps what it was given in `<itself>.args`.
    fn fake(dir: &Dir, sleep: u32) -> PathBuf {
        let script = dir.0.join("yt-dlp");
        let body = format!(
            r#"#!/bin/sh
home=""; temp=""; prev=""; url=""
: > "$0.args"
for a in "$@"; do
  printf '%s\n' "$a" >> "$0.args"
  case "$prev" in -P) case "$a" in home:*) home="${{a#home:}}";; temp:*) temp="${{a#temp:}}";; esac;; esac
  prev="$a"; url="$a"
done
echo "junk" > "$temp/half.part"
case "$url" in
  *ok)
    echo "FGINFO|A fake video"
    echo "FGDL|downloading|1000|4000|NA|1000|3"
    echo "FGDL|downloading|2000|4000|NA|1000|2"
    echo "FGDL|downloading|4000|4000|NA|1000|0"
    echo "FGDL|finished|4000|4000|NA|1000|NA"
    echo done > "$home/A fake video [ok].mp4"
    echo "FGFILE|$home/A fake video [ok].mp4"
    ;;
  *fail)
    echo "FGINFO|A fake video"
    echo "FGDL|downloading|1000|4000|NA|1000|3"
    echo "WARNING: not an error" >&2
    echo "ERROR: [fake] fail: Video unavailable" >&2
    exit 1
    ;;
  *silent)
    exit 3
    ;;
  *slow)
    # The sleep exists before the progress is printed, so that whoever sees the progress finds it running.
    sleep {sleep} &
    echo "FGINFO|A slow video"
    echo "FGDL|downloading|1000|4000|NA|1000|3"
    wait
    ;;
  *parts)
    echo "FGINFO|A fake video"
    echo "FGDL|finished|10|10|NA|1|NA"
    echo video > "$home/A fake video [parts].f133.mp4"
    echo audio > "$home/A fake video [parts].f140.m4a"
    echo "FGFILE|$home/A fake video [parts].mp4"
    ;;
  *joinable)
    echo "FGINFO|A fake video"
    echo "FGDL|finished|10|10|NA|1|NA"
    cp "$0.sample" "$home/A fake video [joinable].f133.mp4"
    echo audio > "$home/A fake video [joinable].f140.m4a"
    echo "FGFILE|$home/A fake video [joinable].mp4"
    ;;
  *badtune)
    echo "FGINFO|A tune"
    echo "FGDL|finished|10|10|NA|1|NA"
    echo notasound > "$home/A tune [badtune].m4a"
    echo "FGFILE|$home/A tune [badtune].m4a"
    ;;
  *tune)
    echo "FGINFO|A tune"
    echo "FGDL|finished|10|10|NA|1|NA"
    cp "$0.tune" "$home/A tune [tune].m4a"
    echo "FGFILE|$home/A tune [tune].m4a"
    ;;
esac
"#
        );
        fs::write(&script, body).unwrap();
        fs::write(
            dir.0.join("yt-dlp.sample"),
            include_bytes!("../../../../macos-sys/testdata/sound.mov"),
        )
        .unwrap();
        fs::set_permissions(&script, fs::Permissions::from_mode(0o755)).unwrap();
        script
    }

    fn request(program: &Path, folder: &Path, url: &str) -> Request {
        Request {
            program: program.to_path_buf(),
            url: url.to_owned(),
            folder: folder.to_path_buf(),
            quality: Quality::Best,
            playlist: false,
            ffmpeg: None,
            proxy: None,
        }
    }

    fn wait_for(engine: &Engine, id: u64, done: impl Fn(&Job) -> bool) -> Job {
        let until = Instant::now() + Duration::from_secs(20);
        loop {
            let job = engine.job(id).expect("the job is listed");
            if done(&job) {
                return job;
            }
            assert!(Instant::now() < until, "waited too long for {job:?}");
            thread::sleep(Duration::from_millis(20));
        }
    }

    fn running_now(sleep: u32) -> bool {
        Command::new("/usr/bin/pgrep")
            .args(["-f", &format!("sleep {sleep}")])
            .output()
            .is_ok_and(|out| out.status.success())
    }

    /// Whether the sleep is there within a few seconds: the process of a stand-in that has printed is not always listed
    /// the moment it is asked for.
    fn running(sleep: u32) -> bool {
        (0..100).any(|_| {
            let found = running_now(sleep);
            if !found {
                thread::sleep(Duration::from_millis(50));
            }
            found
        })
    }

    fn gone(sleep: u32) -> bool {
        (0..50).any(|_| {
            thread::sleep(Duration::from_millis(100));
            !running_now(sleep)
        })
    }

    #[test]
    fn a_download_reads_the_title_and_the_progress_and_ends_with_its_file() {
        let dir = Dir::new("ok");
        let folder = dir.0.join("out");
        fs::create_dir(&folder).unwrap();
        let program = fake(&dir, 1);
        let engine = Engine::default();
        let id = engine.submit(request(&program, &folder, "https://fake.test/ok"));
        let job = wait_for(&engine, id, |j| j.status.over());
        assert_eq!(job.status, Status::Done, "{job:?}");
        assert_eq!(job.title.as_deref(), Some("A fake video"));
        assert_eq!(job.percent, Some(100.0));
        assert_eq!(job.file, Some(folder.join("A fake video [ok].mp4")));
        assert!(job.file.as_ref().unwrap().exists());
        assert!(job.error.is_none() && job.ended == 1);
        let seen = fs::read_to_string(format!("{}.args", program.display())).unwrap();
        let args: Vec<&str> = seen.lines().collect();
        assert_eq!(args[args.len() - 2..], ["--", "https://fake.test/ok"]);
        assert_eq!(args[0], "--ignore-config");
        // What was not finished lived in a folder of its own, and it is gone: nothing but the file is left.
        let left: Vec<String> = fs::read_dir(&folder)
            .unwrap()
            .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
            .collect();
        assert_eq!(left, ["A fake video [ok].mp4"]);
    }

    #[test]
    fn what_is_not_finished_stays_where_the_live_islands_do_not_look() {
        let temp = temp_dir(Path::new("/Users/x/Downloads"), 3);
        let name = temp.file_name().unwrap().to_string_lossy().into_owned();
        assert_eq!(temp.parent(), Some(Path::new("/Users/x/Downloads")));
        assert!(
            crate::features::islands::logic::is_partial(&name),
            "{name} would be announced as a download"
        );
        // The islands list the files of the folder and not its folders, so what is inside never reaches them.
        assert!(crate::features::islands::logic::is_partial(
            "a [x].f137.mp4.part"
        ));
    }

    #[test]
    fn a_failure_keeps_the_last_error_line_and_leaves_no_file_behind() {
        let dir = Dir::new("fail");
        let program = fake(&dir, 1);
        let engine = Engine::default();
        let id = engine.submit(request(&program, &dir.0, "https://fake.test/fail"));
        let job = wait_for(&engine, id, |j| j.status.over());
        assert_eq!(job.status, Status::Failed);
        assert_eq!(job.error.as_deref(), Some("[fake] fail: Video unavailable"));
        assert!(!job.needs_ffmpeg);
        assert_eq!(job.percent, Some(25.0), "how far it got is kept");
        let hidden = fs::read_dir(&dir.0).unwrap().any(|e| {
            e.unwrap()
                .file_name()
                .to_string_lossy()
                .starts_with(".figghost")
        });
        assert!(!hidden, "the folder for what was not finished is removed");

        let id = engine.submit(request(&program, &dir.0, "https://fake.test/silent"));
        let job = wait_for(&engine, id, |j| j.status.over());
        assert_eq!(
            job.status,
            Status::Failed,
            "a failure with nothing said is still one"
        );
        assert!(job.error.is_some());
    }

    #[test]
    fn a_program_that_is_not_there_is_a_failure_and_not_a_hang() {
        let dir = Dir::new("missing");
        let engine = Engine::default();
        let id = engine.submit(request(
            &dir.0.join("no-such-tool"),
            &dir.0,
            "https://fake.test/ok",
        ));
        let job = wait_for(&engine, id, |j| j.status.over());
        assert_eq!(job.status, Status::Failed);
        let id = engine.submit(request(
            &fake(&dir, 1),
            &dir.0.join("not/there"),
            "https://fake.test/ok",
        ));
        // The folder is made if it can be; a folder that cannot be made fails.
        let job = wait_for(&engine, id, |j| j.status.over());
        assert!(matches!(job.status, Status::Done | Status::Failed));
    }

    #[test]
    fn only_two_run_at_once_the_rest_wait_and_a_waiting_one_can_be_cancelled_unstarted() {
        let dir = Dir::new("queue");
        let program = fake(&dir, 71);
        let engine = Engine::default();
        let ids: Vec<u64> = (0..4)
            .map(|_| engine.submit(request(&program, &dir.0, "https://fake.test/slow")))
            .collect();
        for id in &ids[..2] {
            wait_for(&engine, *id, |j| {
                j.status == Status::Running && j.percent.is_some()
            });
        }
        let states = |engine: &Engine| -> Vec<Status> {
            engine.snapshot().iter().map(|j| j.status).collect()
        };
        assert_eq!(
            states(&engine),
            [
                Status::Running,
                Status::Running,
                Status::Queued,
                Status::Queued
            ]
        );
        // The third never starts: its turn comes and it is gone.
        engine.cancel(ids[2]);
        let third = engine.job(ids[2]).unwrap();
        assert_eq!(third.status, Status::Cancelled);
        assert!(third.ended > 0);
        // Cancelling the first gives its place to the fourth, and the group of the first is stopped with its sleep.
        engine.cancel(ids[0]);
        let first = wait_for(&engine, ids[0], |j| j.status.over());
        assert_eq!(first.status, Status::Cancelled);
        wait_for(&engine, ids[3], |j| {
            j.status == Status::Running && j.percent.is_some()
        });
        assert_eq!(
            states(&engine),
            [
                Status::Cancelled,
                Status::Running,
                Status::Cancelled,
                Status::Running
            ]
        );
        engine.stop_all();
        for id in &ids {
            wait_for(&engine, *id, |j| j.status.over());
        }
        assert!(
            engine
                .snapshot()
                .iter()
                .all(|j| j.status == Status::Cancelled)
        );
        assert!(gone(71), "what the stand-in started was stopped with it");
    }

    #[test]
    fn cancelling_a_download_stops_the_processes_it_started_and_removes_what_it_left() {
        let dir = Dir::new("cancel");
        let program = fake(&dir, 72);
        let engine = Engine::default();
        let id = engine.submit(request(&program, &dir.0, "https://fake.test/slow"));
        wait_for(&engine, id, |j| j.percent.is_some());
        assert!(running(72), "the process under it is running");
        engine.cancel(id);
        let job = wait_for(&engine, id, |j| j.status.over());
        assert_eq!(job.status, Status::Cancelled);
        assert!(job.error.is_none(), "a cancel is not an error");
        assert!(gone(72), "the process under it was stopped as well");
        let hidden = fs::read_dir(&dir.0).unwrap().any(|e| {
            e.unwrap()
                .file_name()
                .to_string_lossy()
                .starts_with(".figghost")
        });
        assert!(!hidden, "the half-written files are removed");
        // A second cancel, and one for a number that never was, change nothing.
        engine.cancel(id);
        engine.cancel(9999);
        assert_eq!(engine.job(id).unwrap().status, Status::Cancelled);
    }

    #[test]
    fn a_picture_and_a_sound_left_apart_are_joined_into_the_file_yt_dlp_named() {
        let dir = Dir::new("join");
        let folder = dir.0.join("out");
        fs::create_dir(&folder).unwrap();
        let program = fake(&dir, 1);
        let engine = Engine::default();
        let id = engine.submit(request(&program, &folder, "https://fake.test/joinable"));
        let job = wait_for(&engine, id, |j| j.status.over());
        assert_eq!(job.status, Status::Done, "{job:?}");
        let file = folder.join("A fake video [joinable].mp4");
        assert_eq!(job.file.as_ref(), Some(&file));
        let tracks = macos_sys::mux::tracks(&file).expect("the joined file opens");
        assert_eq!((tracks.video, tracks.audio), (1, 1));
        let left: Vec<String> = fs::read_dir(&folder)
            .unwrap()
            .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
            .collect();
        assert_eq!(
            left,
            ["A fake video [joinable].mp4"],
            "the parts and the work folder are gone"
        );
    }

    #[test]
    fn a_sound_without_ffmpeg_is_written_again_and_one_that_cannot_be_stays_as_downloaded() {
        let dir = Dir::new("tune");
        let folder = dir.0.join("out");
        fs::create_dir(&folder).unwrap();
        let program = fake(&dir, 1);
        let sample = dir.0.join("sample.mov");
        fs::copy(dir.0.join("yt-dlp.sample"), &sample).unwrap();
        // AVFoundation tells the kind of a file by its extension, so the tune is written under the one it will have.
        macos_sys::mux::rewrite_sound(&sample, &dir.0.join("tune.m4a")).unwrap();
        fs::rename(dir.0.join("tune.m4a"), dir.0.join("yt-dlp.tune")).unwrap();
        let engine = Engine::default();
        let mut asked = request(&program, &folder, "https://fake.test/tune");
        asked.quality = Quality::Audio;
        let job = wait_for(&engine, engine.submit(asked), |j| j.status.over());
        assert_eq!(job.status, Status::Done, "{job:?}");
        let file = folder.join("A tune [tune].m4a");
        assert_eq!(job.file.as_ref(), Some(&file));
        let tracks = macos_sys::mux::tracks(&file).expect("the sound opens");
        assert_eq!((tracks.video, tracks.audio), (0, 1));

        let mut asked = request(&program, &folder, "https://fake.test/badtune");
        asked.quality = Quality::Audio;
        let job = wait_for(&engine, engine.submit(asked), |j| j.status.over());
        assert_eq!(job.status, Status::Failed, "{job:?}");
        let error = job.error.unwrap();
        assert!(error.contains("kept as it was downloaded"), "{error}");
        let bad = folder.join("A tune [badtune].m4a");
        assert_eq!(fs::read_to_string(bad).unwrap(), "notasound\n");
        let left = fs::read_dir(&folder).unwrap().count();
        assert_eq!(left, 2, "no work folder is left");
    }

    #[test]
    fn parts_that_cannot_be_joined_fail_the_download_and_stay_in_the_folder() {
        let dir = Dir::new("nojoin");
        let program = fake(&dir, 1);
        let engine = Engine::default();
        let id = engine.submit(request(&program, &dir.0, "https://fake.test/parts"));
        let job = wait_for(&engine, id, |j| j.status.over());
        assert_eq!(job.status, Status::Failed, "{job:?}");
        let error = job.error.unwrap();
        assert!(error.contains("could not be joined"), "{error}");
        assert!(error.contains("Both parts are kept"), "{error}");
        assert!(!job.needs_ffmpeg);
        assert!(dir.0.join("A fake video [parts].f133.mp4").exists());
        assert!(dir.0.join("A fake video [parts].f140.m4a").exists());
        assert!(!dir.0.join("A fake video [parts].mp4").exists());
        let hidden = fs::read_dir(&dir.0).unwrap().any(|e| {
            e.unwrap()
                .file_name()
                .to_string_lossy()
                .starts_with(".figghost")
        });
        assert!(!hidden, "the folder for the work is removed");
    }

    #[test]
    fn joining_is_a_status_of_its_own_that_has_not_ended() {
        assert_eq!(Status::Joining.key(), "joining");
        assert!(!Status::Joining.over());
        assert!(Status::Done.over() && Status::Failed.over() && Status::Cancelled.over());
        assert!(!Status::Queued.over() && !Status::Running.over());
    }

    /// The one real download: `cargo test -p app-ui real_download -- --ignored --nocapture`, with `FIGGHOST_TEST_URL` to try
    /// another link than the first video of YouTube and `FIGGHOST_TEST_QUALITY` (`best`, `1080`, `720`, `audio`) for
    /// another quality than the best, and `FIGGHOST_TEST_KEEP` for a path to copy the file to before the folder goes. It
    /// uses the yt-dlp that is installed and a folder of its own. `FIGGHOST_TEST_NET_MERGE` sends it through net-merge and
    /// prints what each link of this Mac carried.
    #[test]
    #[ignore = "needs the network and an installed yt-dlp"]
    fn real_download_saves_a_video_with_the_installed_yt_dlp() {
        let url = std::env::var("FIGGHOST_TEST_URL")
            .unwrap_or_else(|_| "https://www.youtube.com/watch?v=jNQXAC9IVRw".into());
        let quality = std::env::var("FIGGHOST_TEST_QUALITY")
            .ok()
            .and_then(|q| Quality::parse(&q))
            .unwrap_or(Quality::Best);
        let dirs = logic::tool_dirs(Path::new("/nonexistent"), &std::env::var("PATH").unwrap());
        let program = logic::find_tool("yt-dlp", &dirs).expect("yt-dlp is installed");
        let ffmpeg = logic::find_tool("ffmpeg", &dirs);
        let dir = Dir::new("real");
        let engine = Engine::default();
        let mut asked = request(&program, &dir.0, &url);
        asked.ffmpeg = ffmpeg.clone();
        asked.quality = quality;
        let merge = std::env::var_os("FIGGHOST_TEST_NET_MERGE")
            .map(|_| core_netmerge::Merge::start().expect("the proxy starts"));
        asked.proxy = merge.as_ref().map(|m| m.proxy_url("socks5h", "real"));
        let started = std::time::Instant::now();
        let id = engine.submit(asked);
        let mut best = 0.0_f64;
        let job = loop {
            let job = engine.job(id).unwrap();
            best = best.max(job.percent.unwrap_or(0.0));
            if job.status.over() {
                break job;
            }
            thread::sleep(Duration::from_millis(100));
        };
        println!("ffmpeg: {ffmpeg:?}");
        println!(
            "result: {:?} {:?} best percent {best} in {:.1} s",
            job.status,
            job.error,
            started.elapsed().as_secs_f64()
        );
        for link in merge.iter().flat_map(|m| m.stats().links) {
            println!(
                "net-merge: {} ({}) {}: {:.1} MB",
                link.label,
                link.name,
                link.state,
                link.received as f64 / 1e6
            );
        }
        println!("title: {:?} file: {:?}", job.title, job.file);
        assert_eq!(job.status, Status::Done, "{job:?}");
        let file = job.file.expect("the file is named");
        assert!(file.starts_with(&dir.0));
        assert!(fs::metadata(&file).unwrap().len() > 1000);
        if let Ok(keep) = std::env::var("FIGGHOST_TEST_KEEP") {
            fs::copy(&file, keep).unwrap();
        }
        let left: Vec<String> = fs::read_dir(&dir.0)
            .unwrap()
            .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
            .collect();
        println!("left in the folder: {left:?}");
        assert_eq!(left.len(), 1, "only the video is left: {left:?}");
        let tracks = macos_sys::mux::tracks(&file).expect("the file opens");
        println!("tracks: {tracks:?}");
        let (extension, video, audio) = if quality == Quality::Audio {
            ("m4a", 0, 1)
        } else {
            ("mp4", 1, 1)
        };
        assert_eq!(file.extension().and_then(|e| e.to_str()), Some(extension));
        assert_eq!((tracks.video, tracks.audio), (video, audio));
    }

    #[test]
    fn finished_downloads_can_be_cleared_and_only_the_latest_are_kept() {
        let dir = Dir::new("clear");
        let program = fake(&dir, 1);
        let engine = Engine::default();
        let last = (0..KEEP_FINISHED + 5)
            .map(|_| {
                let id = engine.submit(request(&program, &dir.0, "https://fake.test/fail"));
                wait_for(&engine, id, |j| j.status.over());
                id
            })
            .next_back()
            .unwrap();
        assert!(
            engine.snapshot().len() <= KEEP_FINISHED + 1,
            "the oldest are forgotten"
        );
        assert!(engine.job(last).is_some());
        let slow_dir = Dir::new("clear-slow");
        let slow = fake(&slow_dir, 73);
        let waiting = engine.submit(request(&slow, &dir.0, "https://fake.test/slow"));
        wait_for(&engine, waiting, |j| j.status == Status::Running);
        engine.clear_finished();
        let left = engine.snapshot();
        assert_eq!(left.len(), 1, "one that runs is not a finished one");
        assert_eq!(left[0].id, waiting);
        engine.stop_all();
        wait_for(&engine, waiting, |j| j.status.over());
    }
}
