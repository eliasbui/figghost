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
    collections::HashMap,
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

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    pub enabled: bool,
    /// The system's proxy points at Figghost's, so the wires apply (beta).
    pub control: bool,
    /// The port of the proxy on 127.0.0.1, 1024 to 65535.
    pub port: u16,
    pub wires: Vec<Wire>,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            enabled: false,
            control: false,
            port: DEFAULT_PORT,
            wires: Vec::new(),
        }
    }
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
    ready: Vec<String>,
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
            f.stop_tether();
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

    fn stop_tether(&self) {
        if let Some(mut child) = self.tether.borrow_mut().take() {
            macos_sys::process::kill_group(child.id());
            let _ = child.wait();
            trace::log("network: phone sharing stopped");
        }
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

    /// Asks `adb` for the phones now and then, off the main thread.
    fn phones(&self) -> (Vec<String>, usize) {
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
                let text = run_bounded(&adb.to_string_lossy(), &["devices"], 6).unwrap_or_default();
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
                self.stop_tether();
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
        let mut out = view(&usage, snapshot.as_ref(), &self.me);
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
            "phones": ready,
            "unauthorized": unauthorized,
        });
        out
    }
}

/// The links and the apps for the page: what `nettop` saw, with what went through the proxy added to its app.
pub fn view(usage: &[Usage], snapshot: Option<&Snapshot>, me: &str) -> Value {
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
        None => core_netmerge::idle_stats()
            .links
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
        let v = view(&watched, Some(&snapshot), "Figghost");
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
        let v = view(&watched, None, "Figghost");
        assert_eq!(
            v["apps"][0]["name"],
            json!("Figghost"),
            "without the proxy Figghost is an app like the others"
        );
        assert_eq!(v["running"], json!(false));
    }
}
