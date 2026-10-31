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

use super::{AttachEnv, run_bounded, run_bounded_output};
use crate::{settings::model::Fault, trace};
use core_netcontrol::{
    monitor::{Monitor, Usage},
    owner,
    proxy::{Control, Snapshot},
    rules::{self, Priority, Rule, Target},
    sysproxy::{self, NETWORKSETUP, Saved},
    tether,
};
use macos_sys::app;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::{
    cell::RefCell,
    collections::{BTreeMap, HashMap},
    fs,
    os::unix::process::CommandExt,
    path::{Path, PathBuf},
    process::{Child, Command, Stdio},
    rc::{Rc, Weak},
    sync::{Arc, Mutex, PoisonError},
    thread,
    time::Instant,
};

pub mod settings;

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
/// Every app seen using the network stays on the diagram; this file keeps them across runs, one a line: the name, then
/// after a tab the path of its bundle when it has one (for its icon).
const APPS_FILE: &str = "network-apps.txt";
const MAX_KNOWN_APPS: usize = 80;
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
}

impl Default for Wire {
    fn default() -> Self {
        Self {
            app: String::new(),
            target: "system".into(),
            priority: "normal".into(),
            down_kbps: 0,
            up_kbps: 0,
        }
    }
}

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
        }
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
        let kept = rules::sanitized(self.wires.iter().filter_map(Wire::rule).collect());
        self.wires = kept.iter().map(Wire::from_rule).collect();
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
    let feature = Rc::new(Feature {
        config: RefCell::new(config.clone()),
        home: env.home.to_owned(),
        monitor: Monitor::new(),
        control: RefCell::default(),
        status: Arc::new(Mutex::new(Status::Off)),
        tether: RefCell::default(),
        phones: Arc::default(),
        me: owner::app_of(std::process::id() as i32)
            .map(|a| a.name)
            .unwrap_or_default(),
        known_apps: RefCell::new(read_known_apps(&fs::read_to_string(env.home.join(APPS_FILE)).unwrap_or_default())),
        idle_links: Arc::default(),
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
        let mut command = Command::new(&gnirehtet);
        command
            .arg("autorun")
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
            "network_tether_stop" => {
                // Also when this run did not start it (a phone left on its VPN by a run that ended badly).
                if self.tether.borrow().is_some() {
                    self.stop_tether();
                } else {
                    self.phones_off(false);
                }
                Ok(())
            }
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
            return json!({ "status": self.status().id(), "saved": saved_path(&self.home).exists() });
        }
        let snapshot = self.control.borrow().as_ref().map(Control::snapshot);
        let usage = self.monitor.usage();
        let (ready, unauthorized) = self.phones();
        let idle = if snapshot.is_none() { self.idle_links() } else { Vec::new() };
        let mut out = view(&usage, snapshot.as_ref(), &self.me, &idle);
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
        });
        out
    }
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
        apps.push(json!({
            "name": u.name,
            "bundle": u.bundle,
            "down": u.down + p.map_or(0, |p| p.down),
            "up": u.up + p.map_or(0, |p| p.up),
            "received": u.received + p.map_or(0, |p| p.received),
            "sent": u.sent + p.map_or(0, |p| p.sent),
            "interfaces": interfaces,
            "proxied": p.is_some_and(|p| p.connections > 0),
            "held_to": p.and_then(|p| p.held_to),
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
            "proxied": p.connections > 0, "held_to": p.held_to,
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
        assert_eq!(v["total_down"], json!(1600));
        let v = view(&watched, None, "Figghost", &[]);
        assert_eq!(
            v["apps"][0]["name"],
            json!("Figghost"),
            "without the proxy Figghost is an app like the others"
        );
        assert_eq!(v["running"], json!(false));
    }
}
