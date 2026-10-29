//! Video download: a link pasted on the settings page, or copied and chosen in the menu bar, is fetched into the Downloads
//! folder by yt-dlp, with the progress on the closed notch and on the page. It is off until the person turns it on.
//!
//! Figghost bundles nothing of yt-dlp: it uses the copy the person installed, found in the places Homebrew and pip put it,
//! and ffmpeg the same way when there is one. The tool is run with fixed arguments and no shell, with the link last and
//! after `--`, so that a link can never be read as an option. Only http and https links are accepted. Nothing is written
//! to disk but the video, and the list of downloads is kept in memory only.
//!
//! With net-merge on, yt-dlp goes through the proxy of `core-netmerge` on 127.0.0.1, which spreads its connections over
//! the Mac's links (Wi-Fi, Ethernet, a phone on USB); the proxy runs from the first download until the last one ends.
//!
//! The downloads of Lanes, the download manager by the same author, show on the closed notch too, whether this feature is
//! on or not: `lanes` reads the list Lanes writes for its browser extension.

use super::{AttachEnv, run_bounded_with};
use crate::{i18n, settings::model::Fault, trace};
use jobs::{Engine, Job, Request, SEARCH_PATH, Status};
use logic::Quality;
use macos_sys::{
    MainThreadMarker, app,
    notch::{Activity, Ear, Notch},
    pasteboard::Pasteboard,
    status_item::{Entry, Item},
    style::Tone,
    workspace,
};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::{
    cell::{Cell, RefCell},
    path::{Path, PathBuf},
    rc::{Rc, Weak},
    sync::{Arc, Mutex, PoisonError},
};

pub mod jobs;
pub mod lanes;
pub mod logic;
pub mod settings;
mod text;

/// The two slots of the closed notch: the download that goes on, and the short notice when one ends.
const RUNNING_SLOT: &str = "download";
const NOTICE_SLOT: &str = "download-done";
/// The same two for the downloads of Lanes.
const LANES_SLOT: &str = "lanes";
const LANES_NOTICE_SLOT: &str = "lanes-done";
/// Below a question of an agent that waits for an answer (30) and the notice of an agent that finished (25), so that a
/// download the person started never covers what waits for them; above agents at work (10), since a download is
/// something the person asked for by name.
const RUNNING_PRIORITY: u8 = 20;
/// The end of a download is news for a few seconds, and so takes the place of the progress while it lasts.
const NOTICE_PRIORITY: u8 = 22;
const NOTICE_SECONDS: f64 = 5.0;
/// How often the notch is brought up to date while a download is waiting or going on.
const SYNC_SECONDS: f64 = 0.5;
/// A command is stopped if it has not answered after this many seconds.
const VERSION_SECONDS: u64 = 10;
/// How often the list of Lanes is looked at: a look at the time the file was changed, unless it changed or something
/// is on the notch.
const LANES_SECONDS: f64 = 1.0;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    pub enabled: bool,
    /// Where videos go. Empty is the Downloads folder; otherwise an absolute path, used only while the folder is there.
    pub folder: String,
    /// One of `logic::Quality`'s keys: `best`, `1080`, `720` or `audio`.
    pub quality: String,
    /// A link to a playlist downloads all of it, not only the video it points at.
    pub playlist: bool,
    /// net-merge: a video comes in pieces over several connections at once, spread over every link of the Mac.
    pub net_merge: bool,
    /// The downloads of Lanes show on the closed notch, whether the switch of this feature is on or not.
    pub lanes: bool,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            enabled: false,
            folder: String::new(),
            quality: Quality::Best.key().to_owned(),
            playlist: false,
            net_merge: false,
            lanes: true,
        }
    }
}

impl Config {
    /// A config file can say anything: a quality that is not offered is the best one, and a folder that is not an absolute
    /// path is Downloads.
    pub fn sanitized(mut self) -> Self {
        if Quality::parse(&self.quality).is_none() {
            self.quality = Self::default().quality;
        }
        self.folder = self.folder.trim().to_owned();
        if !Path::new(&self.folder).is_absolute() {
            self.folder.clear();
        }
        self
    }

    fn quality(&self) -> Quality {
        Quality::parse(&self.quality).unwrap_or(Quality::Best)
    }
}

/// What is known of the version of the tool found.
#[derive(Default)]
struct Version {
    /// The tool the text is for.
    path: Option<PathBuf>,
    text: String,
}

pub struct Feature {
    mtm: MainThreadMarker,
    /// `None` without a notch: downloads still work, there is only nothing to show them on.
    notch: Option<Weak<Notch>>,
    /// The person's home folder, which Downloads and the tool folders hang from.
    home: PathBuf,
    config: RefCell<Config>,
    engine: Engine,
    version: Arc<Mutex<Version>>,
    timer: RefCell<Option<app::Timer>>,
    /// What the notch shows of the running download, so that it is only told when that changes.
    shown: RefCell<String>,
    /// How many downloads had ended at the last look, so that an end is announced once.
    seen: Cell<u64>,
    /// The folder the last panel was used to choose, and the number of the choice, which the page takes over.
    picked: RefCell<(u64, String)>,
    /// net-merge: the proxy yt-dlp goes through, there from the first download with the switch on until the last one
    /// ends.
    merge: RefCell<Option<core_netmerge::Merge>>,
    /// Numbers the downloads for the proxy, which sends the first connection of each to a site by the Mac's own link.
    merged: Cell<u64>,
    /// The look at the list of Lanes, there while its switch is on.
    lanes_timer: RefCell<Option<app::Timer>>,
    lanes: RefCell<lanes::Seen>,
    /// The icon of Lanes, made the first time it is shown.
    lanes_icon: RefCell<Option<Arc<Vec<u8>>>>,
    me: Weak<Feature>,
}

/// Starts the feature. It is there even while it is off, so that turning it on needs no restart.
pub fn attach(env: &AttachEnv) -> Option<Rc<Feature>> {
    let config = env.config.download.clone().sanitized();
    let feature = Rc::new_cyclic(|me| Feature {
        mtm: env.mtm,
        notch: env.notch.map(Rc::downgrade),
        home: std::env::var_os("HOME")
            .map(PathBuf::from)
            .unwrap_or_default(),
        config: RefCell::new(config.clone()),
        engine: Engine::default(),
        version: Arc::default(),
        timer: RefCell::default(),
        shown: RefCell::default(),
        seen: Cell::new(0),
        picked: RefCell::default(),
        merge: RefCell::default(),
        merged: Cell::new(0),
        lanes_timer: RefCell::default(),
        lanes: RefCell::default(),
        lanes_icon: RefCell::default(),
        me: me.clone(),
    });
    // The app quitting does not run the threads to their end, and what yt-dlp started would be left running.
    let engine = feature.engine.clone();
    app::on_terminate(move || engine.shutdown());
    if config.enabled {
        feature.start();
    }
    feature.keep_lanes(config.lanes);
    Some(feature)
}

impl Feature {
    pub fn configure(&self, config: &Config) -> Result<(), Fault> {
        let config = config.clone().sanitized();
        let was = self.config.replace(config.clone());
        match (was.enabled, config.enabled) {
            (false, true) => self.start(),
            (true, false) => self.stop(),
            _ => {}
        }
        self.keep_lanes(config.lanes);
        Ok(())
    }

    /// Nothing runs while the feature is on and nothing is downloaded: the timer comes with the first download and goes
    /// with the last, once `sync` has put its end on the notch.
    fn start(&self) {
        self.sync();
    }

    /// Starts or stops the timer to match whether a download waits or goes on.
    fn keep_timer(&self, jobs: &[Job]) {
        let wanted = needs_timer(jobs);
        let mut timer = self.timer.borrow_mut();
        if wanted && timer.is_none() {
            trace::log("download: timer on");
            let weak = self.me.clone();
            *timer = Some(app::every(SYNC_SECONDS, move || {
                if let Some(f) = weak.upgrade() {
                    f.sync();
                }
            }));
        } else if !wanted && timer.is_some() {
            trace::log("download: timer off");
            timer.take();
        }
        if !wanted && self.merge.borrow_mut().take().is_some() {
            trace::log("download: net-merge off");
        }
    }

    /// Stops what runs and takes everything off the notch.
    fn stop(&self) {
        self.timer.borrow_mut().take();
        self.engine.stop_all();
        self.engine.clear_finished();
        self.merge.borrow_mut().take();
        self.shown.borrow_mut().clear();
        if let Some(notch) = self.notch.as_ref().and_then(Weak::upgrade) {
            notch.set_activity(RUNNING_SLOT, None);
            notch.set_activity(NOTICE_SLOT, None);
        }
    }

    /// The tools, found again each time, so that one installed while the app runs is noticed.
    fn tools(&self) -> (Option<PathBuf>, Option<PathBuf>) {
        let dirs = logic::tool_dirs(&self.home, &std::env::var("PATH").unwrap_or_default());
        (
            logic::find_tool("yt-dlp", &dirs),
            logic::find_tool("ffmpeg", &dirs),
        )
    }

    /// The version text of the tool found, which is asked for on a thread of its own the first time a tool is seen.
    fn version_of(&self, tool: Option<&Path>) -> String {
        let mut known = self.version.lock().unwrap_or_else(PoisonError::into_inner);
        if known.path.as_deref() == tool {
            return known.text.clone();
        }
        known.path = tool.map(Path::to_path_buf);
        known.text.clear();
        if let Some(tool) = tool {
            let (tool, shared) = (tool.to_path_buf(), Arc::clone(&self.version));
            std::thread::spawn(move || {
                let text = run_bounded_with(
                    &tool.to_string_lossy(),
                    &["--ignore-config", "--version"],
                    &[("PATH", SEARCH_PATH)],
                    VERSION_SECONDS,
                )
                .map(|out| out.trim().chars().take(40).collect::<String>())
                .unwrap_or_default();
                let mut known = shared.lock().unwrap_or_else(PoisonError::into_inner);
                if known.path.as_deref() == Some(tool.as_path()) {
                    known.text = text;
                }
            });
        }
        String::new()
    }

    /// Facts only the running app knows, for the settings page.
    pub fn runtime(&self) -> Value {
        let (ytdlp, ffmpeg) = self.tools();
        let version = self.version_of(ytdlp.as_deref());
        let path =
            |p: Option<PathBuf>| p.map_or(String::new(), |p| p.to_string_lossy().into_owned());
        let (seq, chosen) = self.picked.borrow().clone();
        let config = self.config.borrow();
        // The links are looked at only for a page that shows them: with the switch on.
        let net = if config.enabled && config.net_merge {
            self.merge
                .borrow()
                .as_ref()
                .map_or_else(core_netmerge::idle_stats, core_netmerge::Merge::stats)
        } else {
            core_netmerge::Stats::default()
        };
        json!({
            "ytdlp": path(ytdlp),
            "ytdlp_version": version,
            "ffmpeg": path(ffmpeg),
            "folder_path": self.folder().to_string_lossy(),
            "jobs": self.engine.snapshot().iter().map(job_json).collect::<Vec<_>>(),
            "picked": { "seq": seq, "path": chosen },
            "net_links": net.links.iter().map(link_json).collect::<Vec<_>>(),
            "net_paused": net.paused && !net.links.is_empty(),
            "lanes_installed": workspace::app_path(lanes::BUNDLE_ID).is_some(),
            "lanes_running": workspace::is_running(lanes::BUNDLE_ID),
        })
    }

    /// The proxy for one more download, started for the first one; `None` when it cannot start, and the download
    /// then goes as without net-merge.
    fn proxy(&self) -> Option<String> {
        let mut merge = self.merge.borrow_mut();
        if merge.is_none() {
            match core_netmerge::Merge::start() {
                Ok(started) => {
                    trace::log("download: net-merge on");
                    *merge = Some(started);
                }
                Err(e) => {
                    eprintln!("download: net-merge did not start: {e}");
                    return None;
                }
            }
        }
        self.merged.set(self.merged.get() + 1);
        let job = format!("dl{}", self.merged.get());
        merge.as_ref().map(|m| m.proxy_url("socks5h", &job))
    }

    fn folder(&self) -> PathBuf {
        logic::resolve_folder(&self.config.borrow().folder, &self.home)
    }

    /// Queues a link. The reasons it may not be are faults the page words.
    pub(crate) fn submit(&self, url: &str) -> Result<u64, Fault> {
        let config = self.config.borrow().clone();
        if !config.enabled {
            return Err(Fault::new("download_off", ""));
        }
        let Some(url) = logic::valid_url(url) else {
            return Err(Fault::new("download_bad_url", ""));
        };
        let (Some(program), ffmpeg) = self.tools() else {
            return Err(Fault::new("download_missing", ""));
        };
        trace::log("download: queued");
        let proxy = if config.net_merge { self.proxy() } else { None };
        let id = self.engine.submit(Request {
            program,
            url: url.to_owned(),
            folder: self.folder(),
            quality: config.quality(),
            playlist: config.playlist,
            ffmpeg,
            proxy,
        });
        self.sync();
        Ok(id)
    }

    /// The menu bar entry: the link on the clipboard, when it is one.
    fn download_copied(&self) {
        if let Some(text) = Pasteboard::general().string() {
            let _ = self.submit(&text);
        }
    }

    /// The buttons of the settings page; what comes after the colon is checked here, since the page is not trusted: the
    /// link must be one, and a number must be that of a download in the list. `None` when the name is not this feature's.
    pub fn action(&self, name: &str) -> Option<Result<(), Fault>> {
        let (verb, arg) = name.split_once(':').unwrap_or((name, ""));
        let job = || arg.parse().ok().and_then(|id| self.engine.job(id));
        Some(match verb {
            "download_add" => self.submit(arg).map(|_| ()),
            "download_cancel" => {
                if let Some(job) = job() {
                    self.engine.cancel(job.id);
                }
                Ok(())
            }
            "download_reveal" => {
                if let Some(job) = job().filter(|j| j.status == Status::Done) {
                    reveal(&job);
                }
                Ok(())
            }
            "download_clear" => {
                self.engine.clear_finished();
                Ok(())
            }
            "lanes_open" => {
                if let Some(path) = workspace::app_path(lanes::BUNDLE_ID) {
                    workspace::open_path(&path);
                }
                Ok(())
            }
            "lanes_get" => {
                workspace::open_url(lanes::PAGE);
                Ok(())
            }
            "download_pick" => {
                // The panel runs a loop of its own, in which the settings model may be asked for the config; it is opened
                // after this action has returned and nothing is held.
                let me = self.me.clone();
                app::after(0.0, move || {
                    let Some(f) = me.upgrade() else { return };
                    let chosen =
                        workspace::pick_folder(f.mtm, i18n::current().download_pick_folder());
                    if let Some(path) = chosen {
                        let mut picked = f.picked.borrow_mut();
                        *picked = (picked.0 + 1, path.to_string_lossy().into_owned());
                    }
                });
                Ok(())
            }
            _ => return None,
        })
    }

    /// Brings the closed notch up to date: the percentage of the download that goes on, and for a few seconds the notice
    /// of one that ended.
    fn sync(&self) {
        let jobs = self.engine.snapshot();
        self.show_on_notch(&jobs);
        self.keep_timer(&jobs);
    }

    fn show_on_notch(&self, jobs: &[Job]) {
        let seen = self.seen.get();
        self.seen
            .set(jobs.iter().map(|j| j.ended).max().unwrap_or(0).max(seen));
        let Some(notch) = self.notch.as_ref().and_then(Weak::upgrade) else {
            return;
        };
        let running = jobs
            .iter()
            .find(|j| matches!(j.status, Status::Running | Status::Joining));
        let key = running.map_or_else(String::new, |j| {
            if j.status == Status::Joining {
                i18n::current().download_joining().to_owned()
            } else {
                percent_text(j.percent)
            }
        });
        if *self.shown.borrow() != key {
            let activity = running.map(|_| Activity {
                left: Ear::Symbol {
                    name: "arrow.down.circle.fill",
                    tone: Tone::Blue,
                    pulse: false,
                },
                right: Ear::Text {
                    text: key.clone(),
                    tone: Tone::Blue,
                },
                detail: None,
                page: None,
                priority: RUNNING_PRIORITY,
            });
            notch.set_activity(RUNNING_SLOT, activity);
            *self.shown.borrow_mut() = key;
        }
        let latest = jobs
            .iter()
            .filter(|j| j.ended > seen && matches!(j.status, Status::Done | Status::Failed))
            .max_by_key(|j| j.ended);
        if let Some(job) = latest {
            let (symbol, tone) = if job.status == Status::Done {
                ("checkmark.circle.fill", Tone::Green)
            } else {
                ("exclamationmark.triangle.fill", Tone::Red)
            };
            let name = job
                .title
                .as_deref()
                .map_or_else(|| logic::host(&job.url), logic::short_name);
            super::islands::show_for(
                &notch,
                NOTICE_SLOT,
                Activity {
                    left: Ear::Symbol {
                        name: symbol,
                        tone,
                        pulse: false,
                    },
                    right: Ear::Text { text: name, tone },
                    detail: None,
                    page: None,
                    priority: NOTICE_PRIORITY,
                },
                Some(NOTICE_SECONDS),
            );
        }
    }

    /// Starts or stops the look at the list of Lanes to match its switch, and takes its downloads off the notch when it
    /// stops.
    fn keep_lanes(&self, on: bool) {
        if on && self.lanes_timer.borrow().is_none() {
            let weak = self.me.clone();
            *self.lanes_timer.borrow_mut() = Some(app::every(LANES_SECONDS, move || {
                if let Some(f) = weak.upgrade() {
                    f.look_at_lanes();
                }
            }));
            self.look_at_lanes();
        } else if !on && self.lanes_timer.borrow_mut().take().is_some() {
            *self.lanes.borrow_mut() = lanes::Seen::default();
            if let Some(notch) = self.notch.as_ref().and_then(Weak::upgrade) {
                notch.set_activity(LANES_SLOT, None);
                super::islands::clear_slot(&notch, LANES_NOTICE_SLOT);
            }
        }
    }

    /// Puts the downloads of Lanes on the notch. While the file has not changed and nothing is shown, this is a look at
    /// the time it was changed and nothing more; while something is shown it is read each time, so that a Lanes that
    /// quit in the middle of a download takes it off the notch.
    fn look_at_lanes(&self) {
        let path = lanes::progress_file(&self.home);
        let stamp = std::fs::metadata(&path).and_then(|m| m.modified()).ok();
        let mut seen = self.lanes.borrow_mut();
        if stamp == seen.stamp && seen.shown.is_empty() {
            return;
        }
        // A file that is gone is an empty list; one that does not read whole is being written, and waits.
        let items = match std::fs::read(&path) {
            Ok(bytes) => match lanes::parse(&bytes) {
                Some(items) => items,
                None => return,
            },
            Err(_) => Vec::new(),
        };
        seen.stamp = stamp;
        let ended = seen.watch.update(&items);
        let running = lanes::running(&items).filter(|_| workspace::is_running(lanes::BUNDLE_ID));
        let Some(notch) = self.notch.as_ref().and_then(Weak::upgrade) else {
            return;
        };
        let key = running.map_or_else(String::new, percent_text);
        if seen.shown != key {
            let activity = running.map(|_| Activity {
                left: self.lanes_ear(),
                right: Ear::Text {
                    text: key.clone(),
                    tone: Tone::Blue,
                },
                detail: None,
                page: None,
                priority: RUNNING_PRIORITY,
            });
            notch.set_activity(LANES_SLOT, activity);
            seen.shown = key;
        }
        if let Some((name, done)) = ended.into_iter().last() {
            let (symbol, tone) = if done {
                ("checkmark.circle.fill", Tone::Green)
            } else {
                ("exclamationmark.triangle.fill", Tone::Red)
            };
            super::islands::show_for(
                &notch,
                LANES_NOTICE_SLOT,
                Activity {
                    left: Ear::Symbol {
                        name: symbol,
                        tone,
                        pulse: false,
                    },
                    right: Ear::Text {
                        text: logic::short_name(&name),
                        tone,
                    },
                    detail: None,
                    page: None,
                    priority: NOTICE_PRIORITY,
                },
                Some(NOTICE_SECONDS),
            );
        }
    }

    /// The icon of Lanes, or the download symbol when there is none to be had.
    fn lanes_ear(&self) -> Ear {
        let mut icon = self.lanes_icon.borrow_mut();
        if icon.is_none() {
            *icon = workspace::app_icon_png(lanes::BUNDLE_ID, 64.0).map(Arc::new);
        }
        icon.as_ref().map_or(
            Ear::Symbol {
                name: "arrow.down.circle.fill",
                tone: Tone::Blue,
                pulse: false,
            },
            |bytes| Ear::Artwork {
                // The same picture each time, so that the notch does not draw it again.
                id: 0x006c_616e_6573,
                bytes: Arc::clone(bytes),
            },
        )
    }

    /// The entry of the menu bar panel; it does something only when the clipboard holds a link.
    pub fn menu_entries(this: &Rc<Self>) -> Vec<Entry> {
        let (act, shown) = (this.me.clone(), this.me.clone());
        vec![Entry::Item(
            Item::new(i18n::current().download_menu(), move || {
                if let Some(f) = act.upgrade() {
                    f.download_copied();
                }
            })
            .symbol("arrow.down.circle")
            .visible_when(move || shown.upgrade().is_some_and(|f| f.config.borrow().enabled)),
        )]
    }
}

/// Whether the notch needs looking after on a timer: only while a download waits, goes on or joins its parts. An end is
/// put on the notch by the look that finds it, after which nothing is left to wake for.
fn needs_timer(jobs: &[Job]) -> bool {
    jobs.iter().any(|j| !j.status.over())
}

fn percent_text(percent: Option<f64>) -> String {
    percent.map_or_else(|| "...".into(), |p| format!("{p:.0}%"))
}

/// Shows the file of a finished download in the Finder, or its folder when no file was named or it has been moved.
fn reveal(job: &Job) {
    match &job.file {
        Some(file) if file.exists() => workspace::reveal(file),
        _ if job.folder.is_dir() => workspace::open_path(&job.folder),
        _ => {}
    }
}

/// One link of net-merge as the page shows it.
fn link_json(link: &core_netmerge::LinkStats) -> Value {
    json!({
        "name": link.name,
        "label": link.label,
        "state": link.state,
        "primary": link.primary,
        "connections": link.connections,
        "speed": link.speed,
    })
}

fn job_json(job: &Job) -> Value {
    json!({
        "id": job.id,
        "url": job.url,
        "title": job.title.clone().unwrap_or_default(),
        "status": job.status.key(),
        "percent": job.percent.map(|p| (p * 10.0).round() / 10.0),
        "speed": job.speed.map(f64::round),
        "eta": job.eta_seconds,
        "error": job.error.clone().unwrap_or_default(),
        "needs_ffmpeg": job.needs_ffmpeg,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_are_off_download_the_best_into_downloads_and_leave_playlists_alone() {
        let config = Config::default();
        assert!(!config.enabled);
        assert_eq!(config.folder, "");
        assert_eq!(config.quality, "best");
        assert!(!config.playlist);
        assert!(!config.net_merge, "net-merge waits to be asked for");
        assert!(
            config.lanes,
            "Lanes shows its downloads, which costs nothing without it"
        );
    }

    #[test]
    fn a_config_that_says_something_odd_is_read_as_the_usual_one() {
        let odd = Config {
            enabled: true,
            folder: "  relative/path ".into(),
            quality: "4k".into(),
            playlist: true,
            net_merge: true,
            lanes: false,
        }
        .sanitized();
        assert_eq!(odd.quality, "best");
        assert_eq!(odd.folder, "", "a folder that is not absolute is Downloads");
        assert!(
            odd.enabled && odd.playlist && odd.net_merge,
            "what is right is kept"
        );
        let kept = Config {
            folder: " /Volumes/Backup/Videos ".into(),
            quality: "720".into(),
            ..Config::default()
        }
        .sanitized();
        assert_eq!(
            (kept.folder.as_str(), kept.quality.as_str()),
            ("/Volumes/Backup/Videos", "720"),
            "an absolute folder is kept even while it is not there, and used only once it is"
        );
        let parsed: Config = serde_json::from_str(r#"{"enabled":true}"#).unwrap();
        assert_eq!(
            parsed.quality, "best",
            "a key that is missing is the default"
        );
        assert!(
            !parsed.net_merge,
            "a file from before net-merge leaves it off"
        );
        assert!(parsed.lanes, "a file from before Lanes shows its downloads");
    }

    #[test]
    fn the_notch_says_the_percentage_or_dots_while_it_is_not_known() {
        assert_eq!(percent_text(Some(41.6)), "42%");
        assert_eq!(percent_text(Some(0.0)), "0%");
        assert_eq!(percent_text(None), "...");
    }

    fn job_with(status: Status) -> Job {
        Job {
            id: 1,
            url: "https://example.com/v".into(),
            title: None,
            status,
            percent: None,
            speed: None,
            eta_seconds: None,
            file: None,
            folder: PathBuf::from("/Users/x/Downloads"),
            error: None,
            needs_ffmpeg: false,
            ended: u64::from(status.over()),
        }
    }

    #[test]
    fn the_timer_runs_only_while_a_download_waits_goes_on_or_joins() {
        assert!(
            !needs_timer(&[]),
            "the feature on with nothing to do wakes nothing"
        );
        for over in [Status::Done, Status::Failed, Status::Cancelled] {
            assert!(
                !needs_timer(&[job_with(over), job_with(over)]),
                "{over:?} needs no look once the look that saw it has run"
            );
        }
        for active in [Status::Queued, Status::Running, Status::Joining] {
            assert!(needs_timer(&[job_with(active)]), "{active:?}");
            assert!(needs_timer(&[job_with(Status::Done), job_with(active)]));
        }
    }

    #[test]
    fn the_page_is_told_the_status_in_words_and_the_numbers_it_needs() {
        let job = Job {
            id: 7,
            url: "https://example.com/v".into(),
            title: Some("A title".into()),
            status: Status::Running,
            percent: Some(33.333),
            speed: Some(1_500_000.4),
            eta_seconds: Some(12),
            file: None,
            folder: PathBuf::from("/Users/x/Downloads"),
            error: None,
            needs_ffmpeg: false,
            ended: 0,
        };
        let json = job_json(&job);
        assert_eq!(json["status"], "running");
        assert_eq!(json["percent"], 33.3);
        assert_eq!(json["speed"], 1_500_000.0);
        assert_eq!(
            (json["eta"].clone(), json["title"].clone()),
            (json!(12), json!("A title"))
        );
        assert_eq!(json["error"], "");
        let keys = |v: &Value| -> Vec<String> {
            let mut keys: Vec<String> = v.as_object().unwrap().keys().cloned().collect();
            keys.sort();
            keys
        };
        let fixture: Value =
            serde_json::from_str(include_str!("../../../web/features/download.fixture.json"))
                .unwrap();
        assert_eq!(
            keys(&fixture["jobs"][0]),
            keys(&json),
            "the fixture shows a download with the keys of a real one"
        );
        let link = link_json(&core_netmerge::LinkStats {
            name: "en0".into(),
            label: "Ethernet".into(),
            ip: std::net::Ipv4Addr::new(10, 0, 0, 2),
            state: "up",
            primary: true,
            connections: 3,
            speed: 2_500_000,
            received: 9,
        });
        assert_eq!(
            (link["label"].clone(), link["connections"].clone()),
            (json!("Ethernet"), json!(3))
        );
        assert!(
            link.get("ip").is_none(),
            "the page is not told the addresses"
        );
        assert_eq!(
            keys(&fixture["net_links"][0]),
            keys(&link),
            "the fixture shows a link with the keys of a real one"
        );
    }
}
