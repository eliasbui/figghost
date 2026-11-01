//! Network (off by default): a diagram of the Mac's connections (Wi-Fi, Ethernet, a phone on USB) wired to the apps
//! that use them, with what each app moves.
//!
//! Watching needs nothing: `nettop` is asked once a second, only while the settings page shows the diagram. Wiring
//! (beta, its own switch) points the system's SOCKS proxy and secure web proxy at a proxy of Figghost on 127.0.0.1
//! (`core-netcontrol`), so apps that follow the system's proxy settings connect through it and take the wire of their
//! app: the system's way, one link, or every link in turn, at most as fast as their limit and below the apps wired
//! before them to the same place. What each network service had is kept in `network-proxy.txt` in the data directory
//! until it is put back: when wiring is turned off, when the app quits, and at the next start after a crash.
//! Changing the settings may ask for an administrator's password, once for all the services.
//!
//! The beta of sharing this Mac's connection with an Android phone runs `gnirehtet autorun` when the person asks, and
//! stops it with the feature.

use super::{AttachEnv, islands, run_bounded, run_bounded_output};
use crate::{i18n, settings::model::Fault, trace};
use core_calendar::format::day_index;
use core_netcontrol::{
    anomaly,
    history::{self, History},
    monitor::{Monitor, Usage},
    owner,
    online::{self, Event, Reach},
    policy::{self, Level, Period, Quota},
    proxy::{Control, Snapshot},
    rules::{self, Priority, Rule, Target},
    sysproxy::{self, NETWORKSETUP, Saved},
    tether,
};
use macos_sys::{
    app, eventkit,
    notch::{Activity, Detail, Ear, Notch},
    style::Tone,
    workspace,
};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::{
    cell::RefCell,
    collections::{BTreeMap, HashMap, VecDeque},
    fs,
    os::unix::process::CommandExt,
    path::{Path, PathBuf},
    process::{Child, Command, Stdio},
    rc::{Rc, Weak},
    sync::{
        Arc, Mutex, PoisonError,
        atomic::{AtomicBool, Ordering::Relaxed},
    },
    thread,
    time::{Duration, Instant},
};

pub mod dns;
pub mod settings;
pub mod speedtest;
pub mod text;

/// The port the proxy asks for first, so that the system's settings stay right after a restart.
pub const DEFAULT_PORT: u16 = 47117;
const SAVED_FILE: &str = "network-proxy.txt";
const COMMAND_SECONDS: u64 = 15;
/// The password dialog waits for the person.
const ADMIN_SECONDS: u64 = 180;
/// The most apps the page is told about.
const MAX_APPS: usize = 40;
/// How often `adb devices` is asked while the page shows the phone row.
const PHONES_EVERY_SECONDS: u64 = 4;
/// How often the links are listed while the proxy is off; listing may run `networksetup`, so never on the main thread.
const LINKS_EVERY_SECONDS: u64 = 3;
/// How often each link is timed: a TCP handshake through it to a well-known address, off the main thread.
const QUALITY_EVERY_SECONDS: u64 = 5;
/// The handshakes kept a link, so that one slow answer does not paint it weak.
const QUALITY_SAMPLES: usize = 6;
/// A handshake slower than this counts as lost.
const QUALITY_TIMEOUT_MS: u64 = 1500;
/// Answered by anycast close to everyone; port 443 because some networks let little else out.
const QUALITY_TARGET: ([u8; 4], u16) = ([1, 1, 1, 1], 443);
/// Round trips (ms) up to which a link is good, and from which it is weak.
const QUALITY_GOOD_MS: u32 = 80;
const QUALITY_POOR_MS: u32 = 250;
/// Every app seen using the network stays on the diagram; this file keeps them across runs, one a line: the name, then
/// after a tab the path of its bundle when it has one (for its icon).
const APPS_FILE: &str = "network-apps.txt";
const MAX_KNOWN_APPS: usize = 80;
/// What each app moved over the days, kept in a small text file (see `core_netcontrol::history`).
const HISTORY_FILE: &str = "network-history.txt";
/// How often the history is written out while it grows.
const HISTORY_SAVE_SECONDS: u64 = 60;
/// How often the history is read while it counts, in seconds: the choices of the page.
pub const HISTORY_SECONDS: [u64; 3] = [2, 5, 10];
/// A speed test fills the line for up to a minute; this is the longest it is let run.
const SPEED_TEST_SECONDS: u64 = 75;
/// What the notch says of a quota or a cap: the slot, how long and how urgent.
const SLOT_QUOTA: &str = "network_quota";
const SLOT_METERED: &str = "network_metered";
const SLOT_UPLOAD: &str = "network_upload";
const SLOT_ONLINE: &str = "network_online";
/// How often the internet is probed while the notices are on.
const WATCH_SECONDS: u64 = 5;
/// The page every Apple device asks to find out whether a network wants a login; it answers `Success`.
const CAPTIVE_URL: &str = "http://captive.apple.com/hotspot-detect.html";
const NOTICE_SECONDS: f64 = 8.0;
const NOTICE_PRIORITY: u8 = 25;
/// The apps each range of the summary lists.
const HISTORY_TOP: usize = 12;
/// The most links kept on the diagram, the places of its nodes, and the links in the Combined circle.
pub const MAX_KNOWN_LINKS: usize = 12;
pub const MAX_PLACES: usize = 300;
pub const MAX_MERGE: usize = 4;
/// The canvas is this large, in points.
pub const CANVAS_EDGE: i32 = 6000;

/// One wire as the settings file keeps it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Wire {
    pub app: String,
    /// `system`, `merge` or `link:<interface>`.
    pub target: String,
    /// `high`, `normal` or `low`.
    pub priority: String,
    pub down_kbps: u32,
    pub up_kbps: u32,
    /// What the app may move in a period, in megabytes, 0 for no quota. Once it is reached the proxy refuses the app.
    pub quota_mb: u32,
    /// `day` or `month`.
    pub quota_period: String,
}

impl Default for Wire {
    fn default() -> Self {
        Self {
            app: String::new(),
            target: "system".into(),
            priority: "normal".into(),
            down_kbps: 0,
            up_kbps: 0,
            quota_mb: 0,
            quota_period: "month".into(),
        }
    }
}

/// The most a quota or a cap may be, in megabytes (10 terabytes).
pub const MAX_QUOTA_MB: u32 = 10_000_000;

impl Wire {
    pub fn rule(&self) -> Option<Rule> {
        Some(Rule {
            app: self.app.clone(),
            target: Target::from_id(&self.target)?,
            priority: Priority::from_id(&self.priority)?,
            down_kbps: self.down_kbps,
            up_kbps: self.up_kbps,
        })
    }

    fn from_rule(rule: &Rule) -> Self {
        Self {
            app: rule.app.clone(),
            target: rule.target.id(),
            priority: rule.priority.id().into(),
            down_kbps: rule.down_kbps,
            up_kbps: rule.up_kbps,
            ..Self::default()
        }
    }

    /// The wires as they apply: one for each app, of the right shape, within the limits, each keeping its quota.
    pub fn sanitized(wires: &[Wire]) -> Vec<Wire> {
        let kept = rules::sanitized(wires.iter().filter_map(Wire::rule).collect());
        kept.iter()
            .map(|rule| {
                let mut wire = Wire::from_rule(rule);
                if let Some(old) = wires.iter().find(|w| w.app.trim().eq_ignore_ascii_case(&rule.app)) {
                    wire.quota_mb = old.quota_mb.min(MAX_QUOTA_MB);
                    if Period::from_id(&old.quota_period).is_some() {
                        wire.quota_period.clone_from(&old.quota_period);
                    }
                }
                wire
            })
            .collect()
    }

    pub fn quota(&self) -> Option<Quota> {
        (self.quota_mb > 0).then(|| Quota {
            app: self.app.clone(),
            bytes: u64::from(self.quota_mb) * 1_000_000,
            period: Period::from_id(&self.quota_period).unwrap_or(Period::Month),
        })
    }
}

/// Saving data on a connection that is paid for by the byte.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Metered {
    /// Only the apps that are allowed use a connection that is metered, through the proxy.
    pub enabled: bool,
    /// A phone's connection is taken for metered by its name and address.
    pub auto: bool,
    /// Connections the person said are metered, and those they said are not, by interface.
    pub marked: Vec<String>,
    pub unmarked: Vec<String>,
    /// The apps let through on a metered connection, by name.
    pub allow: Vec<String>,
    /// What the metered connections may move in a period, in megabytes, 0 for no cap; at the cap every app but this one
    /// is refused.
    pub cap_mb: u32,
    pub period: String,
}

impl Default for Metered {
    fn default() -> Self {
        Self {
            enabled: false,
            auto: true,
            marked: Vec::new(),
            unmarked: Vec::new(),
            allow: Vec::new(),
            cap_mb: 0,
            period: "month".into(),
        }
    }
}

pub const MAX_ALLOWED: usize = 40;

/// App names that can be kept in a list: trimmed, of the right shape, none twice, no more than the limit.
pub fn sanitized_names(names: &[String]) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for name in names {
        let name = name.trim();
        if rules::valid_app(name) && !out.iter().any(|a| a.eq_ignore_ascii_case(name)) && out.len() < MAX_ALLOWED {
            out.push(name.to_owned());
        }
    }
    out
}

impl Metered {
    pub fn sanitized(mut self) -> Self {
        let interfaces = |list: &[String]| {
            let mut out: Vec<String> = Vec::new();
            for name in list {
                if rules::valid_interface(name) && !out.contains(name) && out.len() < MAX_KNOWN_LINKS {
                    out.push(name.clone());
                }
            }
            out
        };
        self.marked = interfaces(&self.marked);
        // A connection is marked one way or the other.
        self.unmarked = interfaces(&self.unmarked).into_iter().filter(|n| !self.marked.contains(n)).collect();
        self.allow = sanitized_names(&self.allow);
        self.cap_mb = self.cap_mb.min(MAX_QUOTA_MB);
        if Period::from_id(&self.period).is_none() {
            self.period = "month".into();
        }
        self
    }

    pub fn period(&self) -> Period {
        Period::from_id(&self.period).unwrap_or(Period::Month)
    }
}

/// A connection that stays on the diagram while it is away, by its interface and what System Settings calls it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct KnownLink {
    pub name: String,
    pub label: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    pub enabled: bool,
    /// The system's proxy points at Figghost's, so the wires apply (beta).
    pub control: bool,
    /// The port of the proxy on 127.0.0.1, 1024 to 65535.
    pub port: u16,
    pub wires: Vec<Wire>,
    /// The links in the Combined circle, made by wiring one link to another on the diagram; empty is no circle, and
    /// a wire to Combined then spreads over every link.
    pub merge: Vec<String>,
    /// The connections kept on the diagram.
    pub links: Vec<KnownLink>,
    /// Where each node of the diagram was put: `link:en0`, `merge` or `app:<name>`, to its top left corner.
    pub layout: BTreeMap<String, [i32; 2]>,
    /// What each app moves is added up over the days, also while the page is closed.
    pub history: bool,
    /// How often it is read while it counts: 2, 5 or 10 seconds.
    pub history_seconds: u64,
    pub metered: Metered,
    /// The notch says so when an app sends far more than it usually does.
    pub anomaly: bool,
    /// The apps that are never reported, such as a backup that is meant to send a lot.
    pub anomaly_ignore: Vec<String>,
    /// The notch says when the internet goes away, comes back, or the network asks for a login. Works without the rest
    /// of the page being on.
    pub watch: bool,
    /// The choice of encrypted DNS the profile is made for (`cloudflare`, `quad9`, `adguard`, `google`, `nextdns` or
    /// `custom`), the NextDNS id, and the address of a server of the person's own.
    pub dns: String,
    pub dns_nextdns: String,
    pub dns_custom: String,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            enabled: false,
            control: false,
            port: DEFAULT_PORT,
            wires: Vec::new(),
            merge: Vec::new(),
            links: Vec::new(),
            layout: BTreeMap::new(),
            history: false,
            history_seconds: 5,
            metered: Metered::default(),
            anomaly: false,
            anomaly_ignore: Vec::new(),
            watch: false,
            dns: "cloudflare".into(),
            dns_nextdns: String::new(),
            dns_custom: String::new(),
        }
    }
}

/// A label or a node id that can be kept: short and without control characters.
pub fn valid_text(text: &str, most: usize) -> bool {
    !text.trim().is_empty() && text.chars().count() <= most && !text.chars().any(char::is_control)
}

impl Config {
    /// A config file can say anything: a port below 1024 is the usual one, and a wire that is not of the right shape, a
    /// second one for the same app and any beyond the limit are dropped.
    pub fn sanitized(mut self) -> Self {
        if self.port < 1024 {
            self.port = DEFAULT_PORT;
        }
        if !HISTORY_SECONDS.contains(&self.history_seconds) {
            self.history_seconds = 5;
        }
        self.wires = Wire::sanitized(&self.wires);
        self.metered = std::mem::take(&mut self.metered).sanitized();
        self.anomaly_ignore = sanitized_names(&self.anomaly_ignore);
        if !dns::PROVIDERS.iter().any(|p| p.key == self.dns) && self.dns != "nextdns" && self.dns != "custom" {
            self.dns = "cloudflare".into();
        }
        if !dns::valid_nextdns(&self.dns_nextdns) {
            self.dns_nextdns.clear();
        }
        if !dns::valid_https_url(&self.dns_custom) {
            self.dns_custom.clear();
        }
        let mut merge: Vec<String> = Vec::new();
        for name in &self.merge {
            if rules::valid_interface(name) && !merge.contains(name) && merge.len() < MAX_MERGE {
                merge.push(name.clone());
            }
        }
        // One link is no circle.
        self.merge = if merge.len() >= 2 { merge } else { Vec::new() };
        let mut links: Vec<KnownLink> = Vec::new();
        for l in &self.links {
            if rules::valid_interface(&l.name) && valid_text(&l.label, 64) && !links.iter().any(|k| k.name == l.name) && links.len() < MAX_KNOWN_LINKS {
                links.push(l.clone());
            }
        }
        self.links = links;
        self.layout = std::mem::take(&mut self.layout)
            .into_iter()
            .filter(|(id, _)| valid_text(id, 90))
            .take(MAX_PLACES)
            .map(|(id, [x, y])| (id, [x.clamp(0, CANVAS_EDGE), y.clamp(0, CANVAS_EDGE)]))
            .collect();
        self
    }

    pub fn rules(&self) -> Vec<Rule> {
        self.wires.iter().filter_map(Wire::rule).collect()
    }
}

/// Where the system's proxy settings stand.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Status {
    Off,
    /// Being changed: the password dialog may be up.
    Starting,
    On,
    /// The password was refused or the dialog cancelled.
    Denied,
    Failed,
}

impl Status {
    pub fn id(self) -> &'static str {
        match self {
            Status::Off => "off",
            Status::Starting => "starting",
            Status::On => "on",
            Status::Denied => "denied",
            Status::Failed => "failed",
        }
    }
}

/// The phones `adb` lists, asked off the main thread.
#[derive(Default)]
struct Phones {
    ready: Vec<tether::Phone>,
    unauthorized: usize,
    asked: Option<Instant>,
    busy: bool,
}

/// The history while it counts: written to from the thread of the monitor and from a timer, and to its file about
/// once a minute and when the app quits, whole or not at all.
struct Ledger {
    history: Mutex<History>,
    path: PathBuf,
    /// Whether it counts now: the monitor also steps while the page is open with the history off.
    on: AtomicBool,
    /// The app this program is: while the proxy carries the others, what the proxy sends is theirs and not its own.
    me: String,
    skip_me: AtomicBool,
    saved: Mutex<Instant>,
    dirty: AtomicBool,
    /// What each app sent in the last few minutes, step by step, for the check of a sudden upload.
    recent: Mutex<VecDeque<(Instant, String, u64)>>,
}

impl Ledger {
    fn new(path: PathBuf, me: String) -> Self {
        let mut history = super::read_small(&path, super::MAX_FILE).map(|t| History::read(&t)).unwrap_or_default();
        history.prune(today());
        Self {
            history: Mutex::new(history),
            path,
            on: AtomicBool::new(false),
            me,
            skip_me: AtomicBool::new(false),
            saved: Mutex::new(Instant::now()),
            dirty: AtomicBool::new(false),
            recent: Mutex::default(),
        }
    }

    fn remember_sent(&self, app: &str, up: u64) {
        if up == 0 {
            return;
        }
        let mut recent = self.recent.lock().unwrap_or_else(PoisonError::into_inner);
        let now = Instant::now();
        recent.push_back((now, app.to_owned(), up));
        while recent.front().is_some_and(|(at, ..)| now.duration_since(*at).as_secs() > anomaly::WINDOW_SECONDS) {
            recent.pop_front();
        }
    }

    /// What each app sent over the last `WINDOW_SECONDS`.
    fn recent_sent(&self) -> HashMap<String, u64> {
        let now = Instant::now();
        let mut out: HashMap<String, u64> = HashMap::new();
        for (at, app, up) in self.recent.lock().unwrap_or_else(PoisonError::into_inner).iter() {
            if now.duration_since(*at).as_secs() <= anomaly::WINDOW_SECONDS {
                *out.entry(app.clone()).or_default() += up;
            }
        }
        out
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, History> {
        self.history.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// What the apps moved in one step of the monitor.
    fn step(&self, usage: &[Usage]) {
        if !self.on.load(Relaxed) {
            return;
        }
        let skip = self.skip_me.load(Relaxed);
        let day = today();
        {
            let mut history = self.lock();
            let apps: Vec<Usage> = usage.iter().filter(|u| !(skip && u.name == self.me)).cloned().collect();
            history.add_step(day, &apps);
            history.prune(day);
            for u in &apps {
                self.remember_sent(&u.name, u.delta.iter().map(|d| d.2).sum());
            }
        }
        self.dirty.store(true, Relaxed);
        self.save_when_due();
    }

    /// What went through the proxy for one app in a step: its link when it used one, else unknown.
    fn add_proxied(&self, app: &str, link: &str, down: u64, up: u64) {
        if !self.on.load(Relaxed) {
            return;
        }
        self.lock().add(today(), app, link, down, up);
        self.remember_sent(app, up);
        self.dirty.store(true, Relaxed);
    }

    fn save_when_due(&self) {
        let due = self.saved.lock().unwrap_or_else(PoisonError::into_inner).elapsed().as_secs() >= HISTORY_SAVE_SECONDS;
        if due {
            self.save();
        }
    }

    fn save(&self) {
        *self.saved.lock().unwrap_or_else(PoisonError::into_inner) = Instant::now();
        if !self.dirty.swap(false, Relaxed) {
            return;
        }
        let text = self.lock().write();
        if let Err(e) = super::write_atomically(&self.path, &text) {
            eprintln!("network: cannot keep the history: {e}");
            self.dirty.store(true, Relaxed);
        }
    }

    fn clear(&self) {
        self.lock().clear();
        self.dirty.store(false, Relaxed);
        if let Err(e) = fs::remove_file(&self.path)
            && e.kind() != std::io::ErrorKind::NotFound
        {
            eprintln!("network: cannot remove the history: {e}");
        }
    }
}

/// One look at the internet: a connection to a well-known address, then the page that tells a network that wants a
/// login from one that does not.
fn probe() -> Reach {
    let to = std::net::SocketAddr::from(QUALITY_TARGET);
    if core_netmerge::connect_by(None, to, Duration::from_millis(2500)).is_err() {
        return Reach::Fail;
    }
    // `--noproxy` so that the check does not go through this program's own proxy.
    let captive = run_bounded("/usr/bin/curl", &["-s", "--max-time", "5", "--noproxy", "*", CAPTIVE_URL], 8)
        .is_ok_and(|body| online::captive_from(&body));
    Reach::Ok { captive }
}

fn level_id(level: Level) -> &'static str {
    match level {
        Level::Ok => "ok",
        Level::Warn => "warn",
        Level::Over => "over",
    }
}

fn now_ms() -> i64 {
    eventkit::now() * 1000
}

fn today() -> i64 {
    day_index(eventkit::now(), &eventkit::utc_offset)
}

thread_local! {
    static FEATURE: RefCell<Weak<Feature>> = const { RefCell::new(Weak::new()) };
}

pub struct Feature {
    config: RefCell<Config>,
    home: PathBuf,
    monitor: Monitor,
    control: RefCell<Option<Control>>,
    status: Arc<Mutex<Status>>,
    tether: RefCell<Option<Child>>,
    phones: Arc<Mutex<Phones>>,
    /// The app this program is, left out of the watched list while the proxy carries the others: its own count would
    /// be theirs again.
    me: String,
    /// Every app seen using the network, oldest first, with the path of its bundle; kept in `network-apps.txt`.
    known_apps: RefCell<Vec<KnownApp>>,
    /// The links while the proxy is off, listed on another thread.
    idle_links: Arc<Mutex<IdleLinks>>,
    /// The last test of the phone's connection: `{ "state": "idle" }`, running, or done with its verdict.
    phone_test: Arc<Mutex<Value>>,
    /// How well each link answers, timed on another thread.
    quality: Arc<Mutex<Quality>>,
    ledger: Arc<Ledger>,
    /// The last speed test of each link, by interface: `running`, `done` or `failed`.
    speeds: Arc<Mutex<HashMap<String, Value>>>,
    /// Reads what the proxy carried for each app while the history counts.
    history_timer: RefCell<Option<(u64, app::Timer)>>,
    /// What each app had received and sent through the proxy at the last look.
    proxied_last: RefCell<HashMap<String, (u64, u64)>>,
    notch: Option<Weak<Notch>>,
    /// What the last look at the quotas and the metered connections found, for the page.
    policy: RefCell<Value>,
    /// The notices already given in this period, so that each is given once: (what, level, start of the period).
    told: RefCell<std::collections::HashSet<(String, u8, i64)>>,
    /// Whether the connection in use was metered at the last look.
    was_metered: std::cell::Cell<bool>,
    /// Stops the thread that probes the internet.
    watch_stop: RefCell<Option<Arc<AtomicBool>>>,
}

/// The last handshake times of each link, by interface: `None` for one that got no answer in time.
#[derive(Default)]
struct Quality {
    samples: HashMap<String, VecDeque<Option<u32>>>,
    asked: Option<Instant>,
    busy: bool,
}

/// What the samples of a link say: its usual round trip, the share lost, and a grade the diagram draws.
pub fn grade(samples: &VecDeque<Option<u32>>) -> (Option<u32>, f64, &'static str) {
    if samples.is_empty() {
        return (None, 0.0, "unknown");
    }
    let mut ok: Vec<u32> = samples.iter().flatten().copied().collect();
    let loss = (samples.len() - ok.len()) as f64 / samples.len() as f64;
    if ok.is_empty() {
        return (None, 1.0, "down");
    }
    ok.sort_unstable();
    let rtt = ok[ok.len() / 2];
    let grade = if rtt > QUALITY_POOR_MS || loss >= 0.34 {
        "poor"
    } else if rtt <= QUALITY_GOOD_MS && loss == 0.0 {
        "good"
    } else {
        "fair"
    };
    (Some(rtt), loss, grade)
}

/// The links of the system, listed off the main thread (naming them may run `networksetup`).
#[derive(Default)]
struct IdleLinks {
    links: Vec<core_netmerge::LinkStats>,
    asked: Option<Instant>,
    busy: bool,
}

/// An app kept on the diagram: its name, and the path of its bundle or nothing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KnownApp {
    pub name: String,
    pub bundle: String,
}

/// A path that can be shown as the icon of an app: an absolute path to a bundle, with no way up out of it.
fn valid_bundle(path: &str) -> bool {
    path.starts_with('/') && path.ends_with(".app") && !path.contains("/../") && valid_text(path, 400)
}

/// The apps of the apps file that can stand as apps.
pub fn read_known_apps(text: &str) -> Vec<KnownApp> {
    let mut out: Vec<KnownApp> = Vec::new();
    for line in text.lines() {
        let (name, bundle) = line.split_once('\t').unwrap_or((line, ""));
        let (name, bundle) = (name.trim(), bundle.trim());
        if rules::valid_app(name) && !out.iter().any(|a| a.name.eq_ignore_ascii_case(name)) && out.len() < MAX_KNOWN_APPS {
            out.push(KnownApp { name: name.to_owned(), bundle: if valid_bundle(bundle) { bundle.to_owned() } else { String::new() } });
        }
    }
    out
}

pub fn write_known_apps(apps: &[KnownApp]) -> String {
    apps.iter().map(|a| if a.bundle.is_empty() { format!("{}\n", a.name) } else { format!("{}\t{}\n", a.name, a.bundle) }).collect()
}

/// Adds the apps of `seen` (name, bundle) that are not known yet, and the bundle of one known without it, never
/// forgetting one; true when the list changed.
pub fn remember_apps(known: &mut Vec<KnownApp>, seen: impl IntoIterator<Item = (String, String)>) -> bool {
    let mut changed = false;
    for (name, bundle) in seen {
        let bundle = if valid_bundle(&bundle) { bundle } else { String::new() };
        if let Some(app) = known.iter_mut().find(|a| a.name.eq_ignore_ascii_case(&name)) {
            if app.bundle.is_empty() && !bundle.is_empty() {
                app.bundle = bundle;
                changed = true;
            }
            continue;
        }
        if known.len() < MAX_KNOWN_APPS && rules::valid_app(&name) && name != core_netcontrol::proxy::UNKNOWN_APP {
            known.push(KnownApp { name, bundle });
            changed = true;
        }
    }
    changed
}

pub fn attach(env: &AttachEnv) -> Option<Rc<Feature>> {
    let config = env.config.network.clone().sanitized();
    let me = owner::app_of(std::process::id() as i32).map(|a| a.name).unwrap_or_default();
    let ledger = Arc::new(Ledger::new(env.home.join(HISTORY_FILE), me.clone()));
    let monitor = Monitor::new();
    {
        let ledger = Arc::clone(&ledger);
        monitor.on_step(move |usage| ledger.step(usage));
    }
    let feature = Rc::new(Feature {
        config: RefCell::new(config.clone()),
        home: env.home.to_owned(),
        monitor,
        control: RefCell::default(),
        status: Arc::new(Mutex::new(Status::Off)),
        tether: RefCell::default(),
        phones: Arc::default(),
        me,
        known_apps: RefCell::new(read_known_apps(&fs::read_to_string(env.home.join(APPS_FILE)).unwrap_or_default())),
        idle_links: Arc::default(),
        quality: Arc::default(),
        phone_test: Arc::new(Mutex::new(json!({ "state": "idle" }))),
        ledger,
        speeds: Arc::default(),
        history_timer: RefCell::default(),
        proxied_last: RefCell::default(),
        notch: env.notch.map(Rc::downgrade),
        policy: RefCell::new(Value::Null),
        told: RefCell::default(),
        was_metered: std::cell::Cell::new(false),
        watch_stop: RefCell::default(),
    });
    FEATURE.with(|slot| *slot.borrow_mut() = Rc::downgrade(&feature));
    // A run that ended without putting the settings back left them here.
    if !(config.enabled && config.control) && saved_path(&feature.home).exists() {
        trace::log("network: putting back the proxy settings a previous run left");
        feature.put_back();
    }
    feature.keep(&config);
    let weak = Rc::downgrade(&feature);
    app::on_terminate(move || {
        if let Some(f) = weak.upgrade() {
            f.ledger.save();
            f.stop_tether_and(true);
            if f.control.borrow().is_some() {
                // On the way out: done here and now, with a password if it takes one.
                restore_settings(&f.home, f.config.borrow().port);
            }
        }
    });
    Some(feature)
}

fn saved_path(home: &Path) -> PathBuf {
    home.join(SAVED_FILE)
}

fn set_status(status: &Mutex<Status>, value: Status) {
    *status.lock().unwrap_or_else(PoisonError::into_inner) = value;
}

/// Runs `networksetup` with each set of arguments, then all of them again with a password when it refused to.
fn run_all(commands: &[Vec<String>]) -> Status {
    if commands.is_empty() {
        return Status::On;
    }
    let direct = commands.iter().all(|c| {
        let args: Vec<&str> = c.iter().map(String::as_str).collect();
        matches!(run_bounded_output(NETWORKSETUP, &args, &[], COMMAND_SECONDS), Ok((true, out)) if !sysproxy::failed(&out))
    });
    if direct {
        return Status::On;
    }
    let script = sysproxy::admin_script(commands);
    match run_bounded_output("/usr/bin/osascript", &["-e", &script], &[], ADMIN_SECONDS) {
        Ok((true, _)) => Status::On,
        Ok((false, _)) => Status::Denied,
        Err(_) => Status::Failed,
    }
}

fn services() -> Vec<String> {
    run_bounded(NETWORKSETUP, &["-listallnetworkservices"], COMMAND_SECONDS)
        .map(|t| sysproxy::parse_services(&t))
        .unwrap_or_default()
}

fn read_proxy(flag: &str, service: &str) -> Option<sysproxy::Proxy> {
    run_bounded(NETWORKSETUP, &[flag, service], COMMAND_SECONDS)
        .ok()
        .and_then(|t| sysproxy::parse_proxy(&t))
}

/// One change of the system's settings at a time: one that puts them back waits for one that is still asking.
static CHANGING: Mutex<()> = Mutex::new(());

/// Keeps what each service has, unless a file from before already holds it, then points them all at `port`.
fn apply_settings(home: &Path, port: u16) -> Status {
    let _one = CHANGING.lock().unwrap_or_else(PoisonError::into_inner);
    let services = services();
    if services.is_empty() {
        return Status::Failed;
    }
    let path = saved_path(home);
    if !path.exists() {
        let saved: Vec<Saved> = services
            .iter()
            .filter_map(|s| {
                Some(Saved {
                    service: s.clone(),
                    socks: read_proxy("-getsocksfirewallproxy", s)?,
                    secure: read_proxy("-getsecurewebproxy", s)?,
                })
            })
            .collect();
        if super::write_atomically(&path, &sysproxy::write_saved(&saved)).is_err() {
            return Status::Failed;
        }
    }
    run_all(&sysproxy::apply_commands(&services, port))
}

/// Whether any service points at the proxy on `port` now.
fn any_ours(port: u16) -> bool {
    services().iter().any(|s| {
        ["-getsocksfirewallproxy", "-getsecurewebproxy"]
            .iter()
            .any(|flag| read_proxy(flag, s).is_some_and(|p| sysproxy::is_ours(&p, port)))
    })
}

/// After a change that did not go through: the settings are put back only if some of them did change, so that a
/// cancelled password dialog is not followed by a second one.
fn undo_settings(home: &Path, port: u16) {
    if any_ours(port) {
        restore_settings(home, port);
    } else {
        let _one = CHANGING.lock().unwrap_or_else(PoisonError::into_inner);
        let _ = fs::remove_file(saved_path(home));
    }
}

/// Puts back what the file holds, and forgets it once that is done.
fn restore_settings(home: &Path, port: u16) -> Status {
    let _one = CHANGING.lock().unwrap_or_else(PoisonError::into_inner);
    let path = saved_path(home);
    let Some(text) = super::read_small(&path, super::MAX_FILE) else {
        return Status::Off;
    };
    let status = run_all(&sysproxy::restore_commands(
        &sysproxy::read_saved(&text),
        port,
    ));
    if status == Status::On {
        let _ = fs::remove_file(&path);
        Status::Off
    } else {
        status
    }
}

impl Feature {
    pub fn configure(&self, config: &Config) -> Result<(), Fault> {
        let config = config.clone().sanitized();
        *self.config.borrow_mut() = config.clone();
        self.keep(&config);
        Ok(())
    }

    /// Starts or stops the proxy and the system's settings to match the switches.
    fn keep(&self, config: &Config) {
        let wanted = config.enabled && config.control;
        if !config.enabled {
            self.stop_tether();
        }
        let running = self.control.borrow().is_some();
        // After a refused password, only the switch turned off and on again asks once more.
        let refused = matches!(self.status(), Status::Denied | Status::Failed);
        if wanted && !running && !refused {
            match Control::start(config.port, config.rules()) {
                Ok(control) => {
                    control.set_merge(config.merge.clone());
                    let port = control.port();
                    trace::log("network: proxy on");
                    *self.control.borrow_mut() = Some(control);
                    set_status(&self.status, Status::Starting);
                    let (home, status) = (self.home.clone(), Arc::clone(&self.status));
                    thread::spawn(move || {
                        let done = apply_settings(&home, port);
                        set_status(&status, done);
                        if done != Status::On {
                            // Nothing points at the proxy: it goes, and the settings that may have changed come back.
                            undo_settings(&home, port);
                            app::on_main(|| {
                                FEATURE.with(|f| {
                                    if let Some(f) = f.borrow().upgrade() {
                                        f.control.borrow_mut().take();
                                    }
                                });
                            });
                        }
                    });
                }
                Err(e) => {
                    eprintln!("network: the proxy did not start: {e}");
                    set_status(&self.status, Status::Failed);
                }
            }
        } else if !wanted && running {
            trace::log("network: proxy off");
            self.put_back();
        } else if let Some(control) = self.control.borrow().as_ref() {
            control.set_rules(config.rules());
            control.set_merge(config.merge.clone());
        }
        if !wanted && matches!(self.status(), Status::Denied | Status::Failed) {
            set_status(&self.status, Status::Off);
        }
        self.keep_history(config);
        self.keep_watch(config);
    }

    /// Starts or stops the probing of the internet to match the switch; it does not need the rest of the feature.
    fn keep_watch(&self, config: &Config) {
        let running = self.watch_stop.borrow().is_some();
        if config.watch && !running {
            let stop = Arc::new(AtomicBool::new(false));
            *self.watch_stop.borrow_mut() = Some(Arc::clone(&stop));
            thread::spawn(move || {
                let mut tracker = online::Tracker::default();
                while !stop.load(Relaxed) {
                    if let Some(event) = tracker.step(probe()) {
                        app::on_main(move || {
                            FEATURE.with(|f| {
                                if let Some(f) = f.borrow().upgrade() {
                                    f.online_event(event);
                                }
                            });
                        });
                    }
                    for _ in 0..WATCH_SECONDS {
                        if stop.load(Relaxed) {
                            break;
                        }
                        thread::sleep(Duration::from_secs(1));
                    }
                }
            });
        } else if !config.watch && let Some(stop) = self.watch_stop.borrow_mut().take() {
            stop.store(true, Relaxed);
        }
    }

    fn online_event(&self, event: Event) {
        if !self.config.borrow().watch {
            return;
        }
        let language = i18n::current();
        let (symbol, tone, words) = match event {
            Event::WentOffline => ("wifi.slash", Tone::Red, text::offline(language)),
            Event::BackOnline => ("wifi", Tone::Green, text::back_online(language)),
            Event::NeedsLogin => ("lock.shield.fill", Tone::Orange, text::needs_login(language)),
        };
        self.notice(SLOT_ONLINE, symbol, tone, words);
    }

    /// Starts or stops the counting of the history, and the judging of quotas and metered connections, to match the
    /// switches. Quotas and the cap of a metered connection are worked out from the history, so they make it count.
    fn keep_history(&self, config: &Config) {
        let quotas = config.wires.iter().any(|w| w.quota_mb > 0);
        let counts = config.enabled
            && (config.history || config.anomaly || quotas || (config.metered.enabled && config.metered.cap_mb > 0));
        let judges = config.enabled && (quotas || config.metered.enabled || config.anomaly);
        self.ledger.on.store(counts, Relaxed);
        self.ledger.skip_me.store(self.control.borrow().is_some(), Relaxed);
        self.monitor.background(if counts { config.history_seconds } else { 0 });
        if counts || judges {
            let every = self.history_timer.borrow().as_ref().map(|(seconds, _)| *seconds);
            if every != Some(config.history_seconds) {
                let weak = FEATURE.with(|f| f.borrow().clone());
                let timer = app::every(config.history_seconds as f64, move || {
                    if let Some(f) = weak.upgrade() {
                        f.look_at_the_proxy();
                        f.evaluate_policy();
                    }
                });
                *self.history_timer.borrow_mut() = Some((config.history_seconds, timer));
            }
        } else {
            self.history_timer.borrow_mut().take();
            self.proxied_last.borrow_mut().clear();
        }
        if !counts {
            self.ledger.save();
        }
        if judges {
            self.evaluate_policy();
        } else {
            *self.policy.borrow_mut() = Value::Null;
            self.was_metered.set(false);
            if let Some(control) = self.control.borrow().as_ref() {
                control.set_policy(&[], None);
            }
        }
    }

    /// The links with an address now: those the proxy follows, else those of the system.
    fn links_now(&self) -> Vec<(String, String, std::net::Ipv4Addr, bool)> {
        if let Some(control) = self.control.borrow().as_ref() {
            return control.snapshot().links.into_iter().map(|l| (l.name, l.label, l.ip, l.primary)).collect();
        }
        self.idle_links().into_iter().map(|l| (l.name, l.label, l.ip, l.primary)).collect()
    }

    /// Says something on the notch, for a few seconds.
    fn notice(&self, slot: &'static str, symbol: &'static str, tone: Tone, (title, subtitle): (String, String)) {
        let Some(notch) = self.notch.as_ref().and_then(Weak::upgrade) else { return };
        islands::show_for(
            &notch,
            slot,
            Activity {
                left: Ear::Symbol { name: symbol, tone, pulse: true },
                right: Ear::Empty,
                detail: Some(Detail { title, subtitle }),
                page: None,
                priority: NOTICE_PRIORITY,
            },
            Some(NOTICE_SECONDS),
        );
    }

    /// Works out who is refused: the apps whose quota has run out, and, on a metered connection, everyone but the
    /// allowed apps (no one at all once the cap is reached). The proxy is told, the notch speaks once for each level
    /// reached in a period, and the page gets what was found.
    fn evaluate_policy(&self) {
        let config = self.config.borrow().clone();
        if !config.enabled {
            return;
        }
        let today = today();
        let language = i18n::current();
        let quotas: Vec<Quota> = config.wires.iter().filter_map(Wire::quota).collect();
        let links = self.links_now();
        let m = &config.metered;
        let metered: Vec<String> = links
            .iter()
            .filter(|(name, label, ip, _)| policy::is_metered(name, label, *ip, &m.marked, &m.unmarked, m.auto))
            .map(|(name, ..)| name.clone())
            .collect();
        let primary = links.iter().find(|l| l.3);
        let metered_now = m.enabled && primary.is_some_and(|p| metered.contains(&p.0));
        let (levels, cap_used) = {
            let history = self.ledger.lock();
            let (from, to) = m.period().range(today);
            (policy::quota_levels(&history, &quotas, today), history.bytes(from, to, None, Some(&metered)))
        };
        let cap = u64::from(m.cap_mb) * 1_000_000;
        let cap_level = if metered_now { Level::of(cap_used, cap) } else { Level::Ok };

        let mut told = self.told.borrow_mut();
        if told.len() > 400 {
            told.clear();
        }
        let mut blocked = Vec::new();
        let mut quota_json = Vec::new();
        for (q, (used, level)) in quotas.iter().zip(&levels) {
            if *level == Level::Over {
                blocked.push(q.app.clone());
            }
            quota_json.push(json!({ "app": q.app, "used": used, "quota": q.bytes, "level": level_id(*level) }));
            if *level != Level::Ok && told.insert((format!("quota:{}", q.app.to_lowercase()), *level as u8, q.period.key(today))) {
                self.notice(SLOT_QUOTA, "exclamationmark.triangle.fill", if *level == Level::Over { Tone::Red } else { Tone::Orange }, text::quota(language, &q.app, *used, q.bytes, *level == Level::Over));
            }
        }
        if cap_level != Level::Ok && told.insert(("cap".to_owned(), cap_level as u8, m.period().key(today))) {
            self.notice(SLOT_QUOTA, "exclamationmark.triangle.fill", if cap_level == Level::Over { Tone::Red } else { Tone::Orange }, text::cap(language, cap_used, cap, cap_level == Level::Over));
        }
        if config.anomaly {
            let hour = eventkit::now() / 3600;
            let history = self.ledger.lock();
            for (app, sent) in self.ledger.recent_sent() {
                let ignored = config.anomaly_ignore.iter().any(|a| a.eq_ignore_ascii_case(&app));
                if !ignored && anomaly::is_sudden(&history, &app, sent, today) && told.insert((format!("up:{}", app.to_lowercase()), 1, hour)) {
                    self.notice(SLOT_UPLOAD, "arrow.up.circle.fill", Tone::Orange, text::upload(language, &app, sent));
                }
            }
        }
        drop(told);
        if metered_now && !self.was_metered.get() {
            self.notice(SLOT_METERED, "antenna.radiowaves.left.and.right", Tone::Blue, text::metered_on(language, primary.map_or("", |p| p.1.as_str())));
        }
        self.was_metered.set(metered_now);

        let only: Option<Vec<String>> = metered_now.then(|| {
            if cap_level == Level::Over {
                vec![self.me.clone()]
            } else {
                m.allow.iter().cloned().chain(std::iter::once(self.me.clone())).collect()
            }
        });
        if let Some(control) = self.control.borrow().as_ref() {
            control.set_policy(&blocked, only.as_deref());
        }
        *self.policy.borrow_mut() = json!({
            "metered_now": metered_now,
            "link": primary.map_or("", |p| p.1.as_str()),
            "cap_used": cap_used,
            "cap_level": level_id(cap_level),
            "refusing": only.is_some(),
            "quotas": quota_json,
        });
    }

    /// While the proxy carries apps, what it carried for each since the last look is theirs: nettop counts it as this
    /// program's own.
    fn look_at_the_proxy(&self) {
        self.ledger.skip_me.store(self.control.borrow().is_some(), Relaxed);
        let Some(snapshot) = self.control.borrow().as_ref().map(Control::snapshot) else {
            self.proxied_last.borrow_mut().clear();
            return;
        };
        let mut last = self.proxied_last.borrow_mut();
        for a in &snapshot.apps {
            let (was_in, was_out) = last.get(&a.name).copied().unwrap_or((a.received, a.sent));
            let link = if let [only] = a.links.as_slice() { only.as_str() } else { "?" };
            self.ledger.add_proxied(&a.name, link, a.received.saturating_sub(was_in), a.sent.saturating_sub(was_out));
            last.insert(a.name.clone(), (a.received, a.sent));
        }
        last.retain(|name, _| snapshot.apps.iter().any(|a| &a.name == name));
    }

    /// Puts the system's settings back, off the main thread, and only then lets the proxy go, so that no app is left
    /// pointed at a port where nothing answers.
    fn put_back(&self) {
        let port = self.config.borrow().port;
        let control = self.control.borrow_mut().take();
        let (home, status) = (self.home.clone(), Arc::clone(&self.status));
        set_status(&status, Status::Starting);
        thread::spawn(move || {
            let done = restore_settings(&home, port);
            set_status(
                &status,
                if done == Status::On {
                    Status::Off
                } else {
                    done
                },
            );
            drop(control);
        });
    }

    fn status(&self) -> Status {
        *self.status.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// Stops sharing: the relay on the Mac ends, and each phone is told to turn its VPN off, so that it does not stay
    /// on a VPN that leads nowhere. `wait` runs the phones' part here and now (when the app quits), else off the main
    /// thread.
    fn stop_tether_and(&self, wait: bool) {
        let Some(mut child) = self.tether.borrow_mut().take() else { return };
        macos_sys::process::kill_group(child.id());
        let _ = child.wait();
        trace::log("network: phone sharing stopped");
        self.phones_off(wait);
    }

    /// Tells each phone that is plugged in to turn the VPN of sharing off.
    fn phones_off(&self, wait: bool) {
        let phones = self.phones.lock().unwrap_or_else(PoisonError::into_inner).ready.clone();
        let (Some(gnirehtet), Some(adb)) = (tether::find("gnirehtet"), tether::find("adb")) else { return };
        let work = move || {
            for phone in phones {
                if let Some(args) = tether::stop_args(&phone.serial) {
                    let args: Vec<&str> = args.iter().map(String::as_str).collect();
                    let adb = adb.to_string_lossy().into_owned();
                    let ok = run_bounded_output(&gnirehtet.to_string_lossy(), &args, &[("ADB", adb.as_str())], 8).is_ok();
                    trace::log(&format!("network: the VPN of a phone was {}turned off", if ok { "" } else { "not " }));
                }
            }
        };
        if wait {
            work();
        } else {
            thread::spawn(work);
        }
    }

    fn stop_tether(&self) {
        self.stop_tether_and(false);
    }

    fn tether_running(&self) -> bool {
        let mut slot = self.tether.borrow_mut();
        let over = slot
            .as_mut()
            .is_some_and(|c| !matches!(c.try_wait(), Ok(None)));
        if over {
            slot.take();
        }
        slot.is_some()
    }

    fn start_tether(&self) -> Result<(), Fault> {
        if self.tether_running() {
            return Ok(());
        }
        let gnirehtet =
            tether::find("gnirehtet").ok_or_else(|| Fault::new("missing_tool", "gnirehtet"))?;
        let adb = tether::find("adb").ok_or_else(|| Fault::new("missing_tool", "adb"))?;
        let mut path = std::env::var("PATH").unwrap_or_default();
        if let Some(dir) = adb.parent() {
            path = format!("{}:{path}", dir.display());
        }
        // The phone asks the Mac's own DNS servers, through the relay: the public one gnirehtet uses by itself is often
        // not let through by a company network, and then apps could not look up a name while a browser that asks over
        // HTTPS still could.
        let scutil = run_bounded("/usr/sbin/scutil", &["--dns"], 3).unwrap_or_default();
        let dns = tether::dns_servers(&scutil);
        trace::log(&format!("network: the phone will ask {} for names", tether::autorun_args(&dns)[2]));
        let mut command = Command::new(&gnirehtet);
        command
            .args(tether::autorun_args(&dns))
            .env("PATH", path)
            .env("ADB", &adb)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .process_group(0);
        if let Some(apk) = tether::apk_near(&gnirehtet) {
            command.env("GNIREHTET_APK", apk);
        }
        let child = command
            .spawn()
            .map_err(|_| Fault::new("tether_failed", "gnirehtet"))?;
        trace::log("network: phone sharing started");
        *self.tether.borrow_mut() = Some(child);
        Ok(())
    }

    /// The links the last listing found; a new one is asked on another thread when it is due.
    fn idle_links(&self) -> Vec<core_netmerge::LinkStats> {
        let mut idle = self.idle_links.lock().unwrap_or_else(PoisonError::into_inner);
        let due = idle.asked.is_none_or(|at| at.elapsed().as_secs() >= LINKS_EVERY_SECONDS);
        if due && !idle.busy {
            idle.busy = true;
            idle.asked = Some(Instant::now());
            let shared = Arc::clone(&self.idle_links);
            thread::spawn(move || {
                let links = core_netmerge::idle_stats().links;
                let mut idle = shared.lock().unwrap_or_else(PoisonError::into_inner);
                idle.links = links;
                idle.busy = false;
            });
        }
        idle.links.clone()
    }

    /// The grade of each link from the handshakes timed so far; new ones are timed on another thread when due.
    fn quality(&self) -> HashMap<String, (Option<u32>, f64, &'static str)> {
        let mut q = self.quality.lock().unwrap_or_else(PoisonError::into_inner);
        let due = q.asked.is_none_or(|at| at.elapsed().as_secs() >= QUALITY_EVERY_SECONDS);
        if due && !q.busy {
            q.busy = true;
            q.asked = Some(Instant::now());
            let shared = Arc::clone(&self.quality);
            thread::spawn(move || {
                let links = core_netmerge::links();
                let to = std::net::SocketAddr::from(QUALITY_TARGET);
                // All links at once, so a dead one does not hold the others up.
                let times: Vec<(String, Option<u32>)> = thread::scope(|scope| {
                    let jobs: Vec<_> = links
                        .iter()
                        .map(|link| {
                            scope.spawn(move || {
                                let start = Instant::now();
                                let ok = core_netmerge::connect_by(Some(link), to, Duration::from_millis(QUALITY_TIMEOUT_MS)).is_ok();
                                let ms = start.elapsed().as_millis().min(u128::from(u32::MAX)) as u32;
                                (link.name.clone(), ok.then_some(ms.max(1)))
                            })
                        })
                        .collect();
                    jobs.into_iter().filter_map(|j| j.join().ok()).collect()
                });
                let mut q = shared.lock().unwrap_or_else(PoisonError::into_inner);
                q.samples.retain(|name, _| times.iter().any(|(n, _)| n == name));
                for (name, ms) in times {
                    let list = q.samples.entry(name).or_default();
                    list.push_back(ms);
                    while list.len() > QUALITY_SAMPLES {
                        list.pop_front();
                    }
                }
                q.busy = false;
            });
        }
        q.samples.iter().map(|(name, list)| (name.clone(), grade(list))).collect()
    }

    /// Asks `adb` for the phones now and then, off the main thread.
    fn phones(&self) -> (Vec<tether::Phone>, usize) {
        let mut phones = self.phones.lock().unwrap_or_else(PoisonError::into_inner);
        let due = phones
            .asked
            .is_none_or(|at| at.elapsed().as_secs() >= PHONES_EVERY_SECONDS);
        if due
            && !phones.busy
            && let Some(adb) = tether::find("adb")
        {
            phones.busy = true;
            phones.asked = Some(Instant::now());
            let shared = Arc::clone(&self.phones);
            thread::spawn(move || {
                let text = run_bounded(&adb.to_string_lossy(), &["devices", "-l"], 6).unwrap_or_default();
                let mut phones = shared.lock().unwrap_or_else(PoisonError::into_inner);
                phones.ready = tether::parse_devices(&text);
                phones.unauthorized = tether::count_unauthorized(&text);
                phones.busy = false;
            });
        }
        (phones.ready.clone(), phones.unauthorized)
    }

    pub fn action(&self, name: &str) -> Option<Result<(), Fault>> {
        Some(match name {
            "network_tether_start" => self.start_tether(),
            "network_tether_test" => self.test_phone(),
            "network_tether_stop" => {
                // Also when this run did not start it (a phone left on its VPN by a run that ended badly).
                if self.tether.borrow().is_some() {
                    self.stop_tether();
                } else {
                    self.phones_off(false);
                }
                Ok(())
            }
            "network_history_clear" => {
                self.ledger.clear();
                self.proxied_last.borrow_mut().clear();
                Ok(())
            }
            "network_dns_install" => self.install_dns(),
            // Where the profile is approved or taken away.
            "network_dns_settings" => {
                workspace::open_url("x-apple.systempreferences:com.apple.preferences.configurationprofiles");
                Ok(())
            }
            // The page that a network asks to log in on, opened in the browser.
            "network_open_login" => {
                workspace::open_url(CAPTIVE_URL);
                Ok(())
            }
            name if name.starts_with("network_history_export:") => self.export_history(name),
            name if name.starts_with("network_speed_test:") => self.speed_test(name["network_speed_test:".len()..].to_owned()),
            // Puts the system's settings back even with the switches off, for a person who sees the web stop.
            "network_restore" => {
                self.put_back();
                Ok(())
            }
            _ => return None,
        })
    }

    /// Facts only the running app knows, for the settings page. Cheap: what the threads measured last.
    pub fn runtime(&self) -> Value {
        let config = self.config.borrow();
        if !config.enabled {
            return json!({ "status": self.status().id(), "saved": saved_path(&self.home).exists(), "history": history_view(&self.ledger.lock(), today()) });
        }
        let snapshot = self.control.borrow().as_ref().map(Control::snapshot);
        let usage = self.monitor.usage();
        let (ready, unauthorized) = self.phones();
        let idle = if snapshot.is_none() { self.idle_links() } else { Vec::new() };
        let mut out = view(&usage, snapshot.as_ref(), &self.me, &idle);
        let quality = self.quality();
        if let Some(links) = out["links"].as_array_mut() {
            for link in links {
                let Some(&(rtt, loss, grade)) = link["name"].as_str().and_then(|n| quality.get(n)) else { continue };
                link["rtt"] = json!(rtt);
                link["loss"] = json!((loss * 100.0).round() / 100.0);
                link["quality"] = json!(grade);
            }
        }
        let speeds = self.speeds.lock().unwrap_or_else(PoisonError::into_inner).clone();
        let radio = macos_sys::wifi::read();
        if let Some(links) = out["links"].as_array_mut() {
            for link in links {
                let name = link["name"].as_str().unwrap_or_default().to_owned();
                if let Some(test) = speeds.get(&name) {
                    link["speed_test"] = test.clone();
                }
                if let Some(r) = radio.as_ref().filter(|r| r.interface == name) {
                    link["wifi"] = wifi_json(r);
                }
            }
        }
        // Every app seen stays on the diagram, also once it stops using the network.
        let seen = out["apps"]
            .as_array()
            .map(|apps| {
                apps.iter()
                    .filter_map(|a| Some((a["name"].as_str()?.to_owned(), a["bundle"].as_str().unwrap_or_default().to_owned())))
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        let grew = remember_apps(&mut self.known_apps.borrow_mut(), seen);
        if grew {
            let text = write_known_apps(&self.known_apps.borrow());
            let path = self.home.join(APPS_FILE);
            thread::spawn(move || {
                if let Err(e) = fs::write(&path, text) {
                    eprintln!("network: cannot keep the list of apps: {e}");
                }
            });
        }
        out["known_apps"] = json!(self.known_apps.borrow().iter().map(|a| json!({ "name": a.name, "bundle": a.bundle })).collect::<Vec<_>>());
        out["history"] = history_view(&self.ledger.lock(), today());
        if !self.policy.borrow().is_null() {
            out["policy"] = self.policy.borrow().clone();
        }
        out["status"] = json!(self.status().id());
        out["saved"] = json!(saved_path(&self.home).exists());
        out["port"] = json!(
            self.control
                .borrow()
                .as_ref()
                .map_or(config.port, Control::port)
        );
        out["tether"] = json!({
            "gnirehtet": tether::find("gnirehtet").is_some(),
            "adb": tether::find("adb").is_some(),
            "running": self.tether_running(),
            "phones": ready.iter().map(|p| json!({ "serial": p.serial, "model": p.model })).collect::<Vec<_>>(),
            "unauthorized": unauthorized,
            "meter": phone_meter(&usage),
            "test": self.phone_test.lock().unwrap_or_else(PoisonError::into_inner).clone(),
        });
        out
    }

    /// Measures one connection with `networkQuality -c -I <interface>`, off the main thread; one test runs at a time,
    /// since each one fills the line.
    fn speed_test(&self, interface: String) -> Result<(), Fault> {
        if !rules::valid_interface(&interface) {
            return Err(Fault::new("invalid_value", interface));
        }
        {
            let mut speeds = self.speeds.lock().unwrap_or_else(PoisonError::into_inner);
            if speeds.values().any(|v| v["state"] == "running") {
                return Ok(());
            }
            speeds.insert(interface.clone(), speedtest::running());
        }
        let shared = Arc::clone(&self.speeds);
        thread::spawn(move || {
            let result = match run_bounded_output("/usr/bin/networkQuality", &["-c", "-I", &interface], &[], SPEED_TEST_SECONDS) {
                Ok((_, text)) => match speedtest::parse(&text) {
                    Ok(speed) => speedtest::done(&speed, now_ms()),
                    Err(reason) => speedtest::failed(reason),
                },
                Err(_) => speedtest::failed("slow"),
            };
            trace::log(&format!("network: a speed test of {interface} ended: {}", result["state"]));
            shared.lock().unwrap_or_else(PoisonError::into_inner).insert(interface, result);
        });
        Ok(())
    }

    /// Writes the profile of the chosen encrypted DNS into the data folder and opens it: macOS takes the person to
    /// System Settings to approve it.
    fn install_dns(&self) -> Result<(), Fault> {
        let config = self.config.borrow();
        let (url, addresses) = dns::resolve(&config.dns, &config.dns_nextdns, &config.dns_custom)
            .ok_or_else(|| Fault::new("invalid_value", "dns"))?;
        let file = self.home.join("encrypted-dns.mobileconfig");
        fs::create_dir_all(&self.home)
            .and_then(|()| fs::write(&file, dns::mobileconfig(&url, &addresses, "Encrypted DNS (Figghost)")))
            .map_err(|e| {
                eprintln!("network: cannot write the DNS profile: {e}");
                Fault::new("export_failed", "dns")
            })?;
        trace::log("network: the encrypted DNS profile was written and opened");
        workspace::open_path(&file);
        Ok(())
    }

    /// `network_history_export:<csv|json>:<7|30|90>:<day|month>`: writes the file into the Downloads folder and shows it
    /// in Finder.
    fn export_history(&self, name: &str) -> Result<(), Fault> {
        let bad = || Fault::new("invalid_value", name);
        let parts: Vec<&str> = name.split(':').collect();
        let [_, format, days, grain] = parts[..] else { return Err(bad()) };
        let days: i64 = match days {
            "7" | "30" | "90" => days.parse().map_err(|_| bad())?,
            _ => return Err(bad()),
        };
        if !matches!(format, "csv" | "json") || !matches!(grain, "day" | "month") {
            return Err(bad());
        }
        let (to, from) = (today(), today() - days + 1);
        let rows = self.ledger.lock().rows(from, to, grain == "month");
        let text = if format == "csv" {
            history::csv(&rows)
        } else {
            serde_json::to_string_pretty(
                &rows
                    .iter()
                    .map(|r| json!({ "date": r.date, "app": r.app, "interface": r.interface, "bytes_in": r.down, "bytes_out": r.up }))
                    .collect::<Vec<_>>(),
            )
            .map_err(|_| Fault::new("export_failed", name))?
        };
        let folder = std::env::var_os("FIGGHOST_DOWNLOADS")
            .map(PathBuf::from)
            .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join("Downloads")))
            .ok_or_else(|| Fault::new("export_failed", name))?;
        let (a, b) = (history::civil(from), history::civil(to));
        let file = folder.join(format!(
            "figghost-network-{:04}{:02}{:02}-{:04}{:02}{:02}.{format}",
            a.0, a.1, a.2, b.0, b.1, b.2
        ));
        fs::create_dir_all(&folder).and_then(|()| fs::write(&file, text)).map_err(|e| {
            eprintln!("network: cannot write the export: {e}");
            Fault::new("export_failed", name)
        })?;
        workspace::reveal(&file);
        Ok(())
    }

    /// Has the phone ping the internet from its own shell, by address and then by name, off the main thread.
    fn test_phone(&self) -> Result<(), Fault> {
        let phone = self.phones.lock().unwrap_or_else(PoisonError::into_inner).ready.first().cloned();
        let phone = phone.ok_or_else(|| Fault::new("no_phone", "tether"))?;
        let adb = tether::find("adb").ok_or_else(|| Fault::new("missing_tool", "adb"))?;
        {
            let mut test = self.phone_test.lock().unwrap_or_else(PoisonError::into_inner);
            if test["state"] == "running" {
                return Ok(());
            }
            *test = json!({ "state": "running", "phone": phone.model });
        }
        let shared = Arc::clone(&self.phone_test);
        thread::spawn(move || {
            let adb = adb.to_string_lossy().into_owned();
            // Each probe is a real connection from the phone; the time is the whole round, as an app would wait it.
            let probe = |which| {
                let args = tether::probe_args(&phone.serial, which)?;
                let args: Vec<&str> = args.iter().map(String::as_str).collect();
                let started = Instant::now();
                let (_, text) = run_bounded_output(&adb, &args, &[], 15).ok()?;
                let ms = started.elapsed().as_secs_f64() * 1000.0;
                Some((tether::read_probe(&text), ms))
            };
            let address = probe(tether::Probe::Address);
            let name = probe(tether::Probe::Name);
            let result = phone_test_result(&phone.model, address, name);
            trace::log(&format!("network: the phone's connection was tested: {}", result["verdict"]));
            *shared.lock().unwrap_or_else(PoisonError::into_inner) = result;
        });
        Ok(())
    }
}

/// The radio of a Wi-Fi link as the page gets it.
pub fn wifi_json(r: &macos_sys::wifi::Radio) -> Value {
    json!({
        "rssi": r.rssi, "noise": r.noise, "snr": r.rssi - r.noise, "channel": r.channel, "band": r.band_ghz,
        "width": r.width_mhz, "rate": r.rate_mbps.round(), "standard": r.standard,
        "grade": macos_sys::wifi::grade(r.rssi, r.noise),
    })
}

fn totals_json(totals: Vec<history::Total>) -> Vec<Value> {
    totals
        .into_iter()
        .take(HISTORY_TOP)
        .map(|t| {
            json!({
                "app": t.app, "down": t.down, "up": t.up,
                "interfaces": t.interfaces.iter().map(|(n, d, u)| json!({ "name": n, "down": d, "up": u })).collect::<Vec<_>>(),
            })
        })
        .collect()
}

/// The summary the page draws: the apps of today, of the last 7 days and of the last 30 with what they moved, and the
/// last 14 days one by one.
pub fn history_view(history: &History, today: i64) -> Value {
    let daily: Vec<Value> = history
        .per_day(today - 13, today)
        .into_iter()
        .map(|(day, down, up)| {
            let (_, m, d) = history::civil(day);
            json!({ "day": format!("{d:02}/{m:02}"), "down": down, "up": up })
        })
        .collect();
    json!({
        "empty": history.is_empty(),
        "today": totals_json(history.totals(today, today)),
        "week": totals_json(history.totals(today - 6, today)),
        "month": totals_json(history.totals(today - 29, today)),
        "daily": daily,
    })
}

/// What went through the relay of Phone sharing is what the phone moved: its traffic reaches the Mac over `adb` on
/// the loopback, which is not counted, and leaves by the Mac's own links.
pub fn phone_meter(usage: &[Usage]) -> Value {
    let relay = usage.iter().filter(|u| u.name.to_ascii_lowercase().contains("gnirehtet"));
    let (mut down, mut up, mut received, mut sent) = (0, 0, 0, 0);
    for u in relay {
        down += u.down;
        up += u.up;
        received += u.received;
        sent += u.sent;
    }
    json!({ "down": down, "up": up, "received": received, "sent": sent })
}

/// The verdict of a test, from the probe by address and the one by name (each: whether the page answered, `None` when
/// the phone has no `nc`, and the milliseconds it took): `good`, `slow` (over a second and a half by name), `dns` (by
/// address only: names are not looked up), `none` (nothing answered), or `untested` (the phone cannot run the test).
pub fn phone_test_result(phone: &str, address: Option<(Option<bool>, f64)>, name: Option<(Option<bool>, f64)>) -> Value {
    let answered = |p: Option<(Option<bool>, f64)>| p.and_then(|(ok, _)| ok);
    let ms = name.filter(|(ok, _)| *ok == Some(true)).map(|(_, ms)| ms.round());
    let verdict = match (answered(address), answered(name)) {
        (None, None) => "untested",
        (_, Some(true)) if ms.is_some_and(|m| m > 1500.0) => "slow",
        (_, Some(true)) => "good",
        (Some(true), _) => "dns",
        _ => "none",
    };
    json!({ "state": "done", "phone": phone, "verdict": verdict, "loss": 0, "ms": ms })
}

/// The links and the apps for the page: what `nettop` saw, with what went through the proxy added to its app.
pub fn view(usage: &[Usage], snapshot: Option<&Snapshot>, me: &str, idle: &[core_netmerge::LinkStats]) -> Value {
    let proxied = snapshot.is_some();
    // The links: those the proxy follows, else those of the system, with what the apps received by each interface.
    let mut by_interface: HashMap<String, (u64, usize)> = HashMap::new();
    for u in usage.iter().filter(|u| !(proxied && u.name == me)) {
        for (iface, down) in &u.interfaces {
            let entry = by_interface.entry(iface.clone()).or_default();
            entry.0 += down;
            entry.1 += 1;
        }
    }
    let links: Vec<Value> = match snapshot {
        Some(s) => s
            .links
            .iter()
            .map(|l| {
                let (down, _) = by_interface.get(&l.name).copied().unwrap_or_default();
                json!({
                    "name": l.name, "label": l.label, "ip": l.ip.to_string(), "primary": l.primary,
                    "down": l.down + down, "up": l.up, "connections": l.connections,
                })
            })
            .collect(),
        None => idle
            .iter()
            .map(|l| {
                let (down, _) = by_interface.get(&l.name).copied().unwrap_or_default();
                json!({
                    "name": l.name, "label": l.label, "ip": l.ip.to_string(), "primary": l.primary,
                    "down": down, "up": 0, "connections": 0,
                })
            })
            .collect(),
    };
    let mut apps: Vec<Value> = Vec::new();
    let mut names: Vec<String> = Vec::new();
    let empty = Vec::new();
    let through = snapshot.map_or(&empty, |s| &s.apps);
    for u in usage.iter().filter(|u| !(proxied && u.name == me)) {
        let p = through.iter().find(|a| a.name == u.name);
        let mut interfaces: Vec<Value> = u
            .interfaces
            .iter()
            .map(|(n, d)| json!({ "name": n, "down": d }))
            .collect();
        if let Some(p) = p {
            for link in &p.links {
                if !link.is_empty() && !u.interfaces.iter().any(|(n, _)| n == link) {
                    interfaces.push(json!({ "name": link, "down": 0 }));
                }
            }
        }
        let mut hosts: Vec<Value> = p
            .map(|p| p.hosts.iter().map(|h| json!({ "name": h.name, "received": h.received, "sent": h.sent })).collect())
            .unwrap_or_default();
        hosts.extend(u.hosts.iter().filter(|(n, ..)| !hosts.iter().any(|h| h["name"] == *n)).take(20).map(|(n, a, b)| json!({ "name": n, "received": a, "sent": b })).collect::<Vec<_>>());
        hosts.sort_by(|a, b| {
            let n = |v: &Value| v["received"].as_u64().unwrap_or(0) + v["sent"].as_u64().unwrap_or(0);
            n(b).cmp(&n(a))
        });
        hosts.truncate(20);
        apps.push(json!({
            "name": u.name,
            "bundle": u.bundle,
            "hosts": hosts,
            "down": u.down + p.map_or(0, |p| p.down),
            "up": u.up + p.map_or(0, |p| p.up),
            "received": u.received + p.map_or(0, |p| p.received),
            "sent": u.sent + p.map_or(0, |p| p.sent),
            "interfaces": interfaces,
            "proxied": p.is_some_and(|p| p.connections > 0),
            "held_to": p.and_then(|p| p.held_to),
            "refused": p.map_or(0, |p| p.refused),
        }));
        names.push(u.name.clone());
    }
    for p in through
        .iter()
        .filter(|p| !names.contains(&p.name) && p.name != core_netcontrol::proxy::UNKNOWN_APP)
    {
        apps.push(json!({
            "name": p.name, "bundle": p.bundle, "down": p.down, "up": p.up, "received": p.received, "sent": p.sent,
            "interfaces": p.links.iter().filter(|l| !l.is_empty()).map(|l| json!({ "name": l, "down": 0 })).collect::<Vec<_>>(),
            "proxied": p.connections > 0, "held_to": p.held_to, "refused": p.refused,
            "hosts": p.hosts.iter().map(|h| json!({ "name": h.name, "received": h.received, "sent": h.sent })).collect::<Vec<_>>(),
        }));
    }
    apps.sort_by(|a, b| {
        let n = |v: &Value, k: &str| v[k].as_u64().unwrap_or(0);
        n(b, "down")
            .cmp(&n(a, "down"))
            .then(n(b, "received").cmp(&n(a, "received")))
    });
    apps.truncate(MAX_APPS);
    let total = |k: &str| apps.iter().map(|a| a[k].as_u64().unwrap_or(0)).sum::<u64>();
    json!({
        "links": links,
        "apps": apps,
        "total_down": total("down"),
        "total_up": total("up"),
        "paused": snapshot.is_some_and(|s| s.paused),
        "running": proxied,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use core_netcontrol::proxy::AppView;

    #[test]
    fn it_starts_off_and_a_config_that_says_something_odd_is_read_as_the_usual_one() {
        let c = Config::default();
        assert!(!c.enabled && !c.control && c.port == DEFAULT_PORT && c.wires.is_empty());
        let odd = Config {
            port: 80,
            wires: vec![
                Wire {
                    app: "Steam".into(),
                    target: "link:en0".into(),
                    priority: "high".into(),
                    down_kbps: 5,
                    up_kbps: 0,
                    ..Wire::default()
                },
                Wire {
                    app: "steam".into(),
                    ..Wire::default()
                },
                Wire {
                    app: "Bad".into(),
                    target: "link:../x".into(),
                    ..Wire::default()
                },
                Wire {
                    app: "Odd".into(),
                    priority: "urgent".into(),
                    ..Wire::default()
                },
            ],
            ..Config::default()
        }
        .sanitized();
        assert_eq!(odd.port, DEFAULT_PORT);
        assert_eq!(odd.wires.len(), 1);
        assert_eq!(odd.rules()[0].target, Target::Link("en0".into()));
    }

    #[test]
    fn the_phone_test_says_what_it_found_and_the_meter_counts_the_relay() {
        let ok = Some((Some(true), 180.0));
        let r = phone_test_result("Pixel", ok, ok);
        assert_eq!((r["verdict"].clone(), r["ms"].clone()), (json!("good"), json!(180.0)));
        assert_eq!(phone_test_result("Pixel", ok, Some((Some(true), 2400.0)))["verdict"], json!("slow"));
        assert_eq!(phone_test_result("Pixel", ok, Some((Some(false), 6000.0)))["verdict"], json!("dns"), "by address but not by name");
        assert_eq!(phone_test_result("Pixel", Some((Some(false), 6000.0)), Some((Some(false), 6000.0)))["verdict"], json!("none"));
        assert_eq!(phone_test_result("Pixel", Some((None, 10.0)), Some((None, 10.0)))["verdict"], json!("untested"));
        assert_eq!(phone_test_result("Pixel", None, None)["verdict"], json!("untested"), "adb could not be run");
        let relay = Usage { name: "gnirehtet".into(), down: 500, up: 40, received: 9000, sent: 700, ..Usage::default() };
        let other = usage("Safari", 100, "en0");
        assert_eq!(phone_meter(&[relay, other]), json!({ "down": 500, "up": 40, "received": 9000, "sent": 700 }));
    }

    #[test]
    fn every_app_seen_is_kept_and_none_is_forgotten() {
        let names = |k: &[KnownApp]| k.iter().map(|a| a.name.clone()).collect::<Vec<_>>();
        let mut known = read_known_apps("Safari\t/Applications/Safari.app\n\nsafari\nbad/name\nSteam\t/etc/../x.app\n");
        assert_eq!(names(&known), ["Safari", "Steam"]);
        assert_eq!((known[0].bundle.as_str(), known[1].bundle.as_str()), ("/Applications/Safari.app", ""), "a path that climbs out is no icon");
        let seen = |n: &str, b: &str| (n.to_owned(), b.to_owned());
        assert!(remember_apps(&mut known, vec![seen("Zoom", ""), seen("STEAM", "/Applications/Steam.app"), seen("?", "")]));
        assert_eq!(names(&known), ["Safari", "Steam", "Zoom"], "an app that stopped is kept, and the unknown one is not added");
        assert_eq!(known[1].bundle, "/Applications/Steam.app", "a bundle found later is kept");
        assert!(!remember_apps(&mut known, vec![seen("Zoom", "")]));
        assert_eq!(read_known_apps(&write_known_apps(&known)), known);
        for i in 0..200 {
            remember_apps(&mut known, vec![seen(&format!("app {i}"), "")]);
        }
        assert_eq!(known.len(), MAX_KNOWN_APPS);
    }

    #[test]
    fn the_diagram_keeps_its_circle_its_links_and_its_places_within_bounds() {
        let c = Config {
            merge: vec!["en0".into(), "en0".into(), "../x".into(), "en7".into()],
            links: vec![
                KnownLink { name: "en0".into(), label: "Wi-Fi".into() },
                KnownLink { name: "en0".into(), label: "again".into() },
                KnownLink { name: "bad name".into(), label: "x".into() },
            ],
            layout: [("app:Safari".to_owned(), [-5, 99_999]), ("bad\u{7}".to_owned(), [1, 1])].into_iter().collect(),
            ..Config::default()
        }
        .sanitized();
        assert_eq!(c.merge, ["en0", "en7"]);
        assert_eq!(c.links.len(), 1);
        assert_eq!(c.layout.get("app:Safari"), Some(&[0, CANVAS_EDGE]));
        assert_eq!(c.layout.len(), 1);
        let one = Config { merge: vec!["en0".into()], ..Config::default() }.sanitized();
        assert!(one.merge.is_empty(), "one link is no circle");
    }

    fn usage(name: &str, down: u64, iface: &str) -> Usage {
        Usage {
            name: name.into(),
            down,
            received: down,
            interfaces: vec![(iface.into(), down)],
            sockets: 1,
            ..Usage::default()
        }
    }

    #[test]
    fn the_page_gets_the_apps_busiest_first_with_what_went_through_the_proxy() {
        let watched = [
            usage("Steam", 500, "en0"),
            usage("Figghost", 9000, "en0"),
            usage("Safari", 100, "en0"),
        ];
        let snapshot = Snapshot {
            links: Vec::new(),
            apps: vec![AppView {
                name: "Safari".into(),
                bundle: String::new(),
                connections: 2,
                down: 1000,
                up: 10,
                received: 1000,
                sent: 10,
                links: vec!["en7".into()],
                held_to: Some(2048),
                hosts: vec![core_netcontrol::proxy::HostView { name: "example.com".into(), connections: 3, received: 900, sent: 10 }],
                refused: 0,
            }],
            paused: false,
        };
        let v = view(&watched, Some(&snapshot), "Figghost", &[]);
        let names: Vec<&str> = v["apps"]
            .as_array()
            .unwrap()
            .iter()
            .map(|a| a["name"].as_str().unwrap())
            .collect();
        assert_eq!(
            names,
            ["Safari", "Steam"],
            "Figghost's own count is the proxied apps again, so it is left out"
        );
        assert_eq!(v["apps"][0]["down"], json!(1100));
        assert_eq!(v["apps"][0]["proxied"], json!(true));
        assert_eq!(v["apps"][0]["held_to"], json!(2048));
        assert_eq!(v["apps"][0]["interfaces"][1]["name"], json!("en7"));
        assert_eq!(v["apps"][0]["hosts"][0]["name"], json!("example.com"), "the sites the proxy saw it go to");
        assert_eq!(v["total_down"], json!(1600));
        let v = view(&watched, None, "Figghost", &[]);
        assert_eq!(
            v["apps"][0]["name"],
            json!("Figghost"),
            "without the proxy Figghost is an app like the others"
        );
        assert_eq!(v["running"], json!(false));
    }

    #[test]
    fn grades_a_link_by_its_handshakes() {
        let q = |v: &[Option<u32>]| grade(&v.iter().copied().collect());
        assert_eq!(q(&[]), (None, 0.0, "unknown"));
        assert_eq!(q(&[Some(20), Some(30), Some(25)]), (Some(25), 0.0, "good"));
        assert_eq!(q(&[Some(20), Some(900), Some(30), Some(25)]).2, "good", "one slow answer is not the link");
        assert_eq!(q(&[Some(120), Some(140), Some(130)]).2, "fair");
        assert_eq!(q(&[Some(20), None, Some(30), Some(25), Some(25), Some(25)]).2, "fair", "a loss keeps it from good");
        assert_eq!(q(&[Some(400), Some(500), Some(450)]).2, "poor");
        assert_eq!(q(&[Some(20), None, None]).2, "poor");
        assert_eq!(q(&[None, None]), (None, 1.0, "down"));
    }

    fn moved(name: &str, iface: &str, down: u64, up: u64) -> Usage {
        Usage { name: name.into(), delta: vec![(iface.into(), down, up)], ..Usage::default() }
    }

    #[test]
    fn the_ledger_counts_only_while_it_is_on_and_leaves_out_what_the_proxy_carries_for_others() {
        let dir = std::env::temp_dir().join(format!("figghost-ledger-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let ledger = Ledger::new(dir.join(HISTORY_FILE), "Figghost".into());
        let step = [moved("Safari", "en0", 100, 10), moved("Figghost", "en0", 90, 9)];
        ledger.step(&step);
        assert!(ledger.lock().is_empty(), "off: nothing is counted");
        ledger.on.store(true, Relaxed);
        ledger.step(&step);
        let apps: Vec<String> = ledger.lock().totals(today(), today()).into_iter().map(|t| t.app).collect();
        assert_eq!(apps, ["Safari", "Figghost"]);
        ledger.skip_me.store(true, Relaxed);
        ledger.step(&step);
        let totals = ledger.lock().totals(today(), today());
        assert_eq!(totals[0].down, 200);
        assert_eq!(totals[1].down, 90, "the program's own count stops while the proxy carries the others");
        ledger.add_proxied("Steam", "en7", 500, 5);
        assert_eq!(ledger.lock().totals(today(), today())[0].app, "Steam");
        ledger.save();
        let again = Ledger::new(dir.join(HISTORY_FILE), String::new());
        assert_eq!(again.lock().totals(today(), today()).len(), 3, "what was saved is read back");
        let sent = ledger.recent_sent();
        assert_eq!((sent["Safari"], sent["Steam"], sent["Figghost"]), (20, 5, 9), "what went out in the last minutes, step by step");
        ledger.clear();
        assert!(ledger.lock().is_empty() && !dir.join(HISTORY_FILE).exists());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn the_summary_lists_the_busiest_apps_of_each_range_and_each_of_the_last_days() {
        let mut h = History::default();
        h.add(100, "Safari", "en0", 1000, 100);
        h.add(98, "Steam", "en7", 5000, 0);
        h.add(60, "Old", "en0", 1, 1);
        let v = history_view(&h, 100);
        assert_eq!(v["empty"], json!(false));
        assert_eq!(v["today"].as_array().unwrap().len(), 1);
        assert_eq!(v["week"][0]["app"], json!("Steam"));
        assert_eq!(v["week"][0]["interfaces"][0]["name"], json!("en7"));
        assert_eq!(v["month"].as_array().unwrap().len(), 2, "the app of 40 days ago is out of the month");
        let daily = v["daily"].as_array().unwrap();
        assert_eq!(daily.len(), 14, "quiet days are listed too");
        assert_eq!(daily[13]["down"], json!(1000));
        assert_eq!(history_view(&History::default(), 100)["empty"], json!(true));
    }
}
