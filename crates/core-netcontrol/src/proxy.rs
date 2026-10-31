//! The proxy on 127.0.0.1 that the system's proxy settings point at: each connection is matched to its app, leaves by
//! the wire of that app, and is held to the app's limit and to what its priority leaves it.

use crate::{
    bucket::Bucket,
    handshake::{self, Host},
    owner,
    rules::{Priority, Rule, Target, priority_caps, rule_for},
};
use core_netmerge::Link;
use std::{
    collections::HashMap,
    io::{self, ErrorKind, Read, Write},
    net::{IpAddr, Ipv4Addr, Shutdown, SocketAddr, TcpListener, TcpStream, ToSocketAddrs},
    sync::{
        Arc, Mutex, MutexGuard, PoisonError,
        atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering::Relaxed},
    },
    thread,
    time::{Duration, Instant},
};

const CONNECT_TIMEOUT: Duration = Duration::from_secs(8);
const HANDSHAKE_TIMEOUT: Duration = Duration::from_secs(15);
const LINGER: Duration = Duration::from_secs(60);
const TICK: Duration = Duration::from_secs(1);
const RESCAN_EVERY: Duration = Duration::from_secs(3);
/// A link whose own connection failed (no network, no address) is left out of the merge for this long.
const LINK_COOLDOWN: Duration = Duration::from_secs(15);
/// The apps remembered, those with nothing open forgotten first.
const MAX_APPS: usize = 200;
const BUFFER: usize = 64 * 1024;
/// The name of a connection whose process could not be found.
pub const UNKNOWN_APP: &str = "?";

/// What the page shows of a link.
#[derive(Debug, Clone, PartialEq)]
pub struct LinkView {
    pub name: String,
    pub label: String,
    pub ip: Ipv4Addr,
    /// The link the system uses by itself.
    pub primary: bool,
    /// Bytes a second, over the last second.
    pub down: u64,
    pub up: u64,
    pub connections: usize,
    /// Bytes since the proxy started.
    pub received: u64,
    pub sent: u64,
}

/// What the page shows of an app that went through the proxy.
#[derive(Debug, Clone, PartialEq)]
pub struct AppView {
    pub name: String,
    pub bundle: String,
    pub connections: usize,
    pub down: u64,
    pub up: u64,
    pub received: u64,
    pub sent: u64,
    /// The links its open connections leave by.
    pub links: Vec<String>,
    /// What its priority leaves it now, in bytes a second; `None` while nothing holds it back.
    pub held_to: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct Snapshot {
    pub links: Vec<LinkView>,
    pub apps: Vec<AppView>,
    /// The system's own route leaves by none of the links, as when a VPN carries everything: every connection then
    /// takes the system's way, so that nothing goes around the VPN.
    pub paused: bool,
}

/// Counters that the copying threads add to without the lock.
#[derive(Default)]
struct Counter {
    total: AtomicU64,
    last: AtomicU64,
}

impl Counter {
    fn add(&self, n: usize) {
        self.total.fetch_add(n as u64, Relaxed);
    }

    /// What was added since the last call.
    fn take_delta(&self) -> u64 {
        let total = self.total.load(Relaxed);
        total.saturating_sub(self.last.swap(total, Relaxed))
    }
}

struct Slot {
    link: Link,
    present: bool,
    cooldown: Option<Instant>,
    down: Arc<Counter>,
    up: Arc<Counter>,
    /// What each priority received through it, for the caps.
    tiers: Arc<[Counter; 3]>,
    rate_down: f64,
    rate_up: f64,
    /// The speed it was seen to reach: quick to rise, slow to fall.
    peak: f64,
    caps: [Option<f64>; 3],
    open: Arc<AtomicUsize>,
}

struct AppSlot {
    bundle: String,
    down: Arc<Counter>,
    up: Arc<Counter>,
    rate_down: f64,
    rate_up: f64,
    bucket_down: Arc<Mutex<Bucket>>,
    bucket_up: Arc<Mutex<Bucket>>,
    held_to: Option<f64>,
    /// The links of its open connections, one entry each.
    open: Vec<String>,
    seen: Instant,
}

impl AppSlot {
    fn new(bundle: String, now: Instant) -> Self {
        Self {
            bundle,
            down: Arc::default(),
            up: Arc::default(),
            rate_down: 0.0,
            rate_up: 0.0,
            bucket_down: Arc::new(Mutex::new(Bucket::new(now))),
            bucket_up: Arc::new(Mutex::new(Bucket::new(now))),
            held_to: None,
            open: Vec::new(),
            seen: now,
        }
    }
}

struct State {
    rules: Vec<Rule>,
    slots: Vec<Slot>,
    primary: Option<String>,
    apps: HashMap<String, AppSlot>,
    rescanned: Option<Instant>,
    /// Links given by a test: never listed again.
    fixed: bool,
}

struct Shared {
    state: Mutex<State>,
    stopped: AtomicBool,
    port: u16,
}

impl Shared {
    fn state(&self) -> MutexGuard<'_, State> {
        self.state.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

/// The proxy, from `start` until it is dropped; connections already open finish on their own.
pub struct Control {
    shared: Arc<Shared>,
}

impl Control {
    /// Starts on `port` of 127.0.0.1 when it is free, so that the system's settings stay right after a restart, else on
    /// any free port.
    pub fn start(port: u16, rules: Vec<Rule>) -> io::Result<Control> {
        Self::launch(port, rules, None)
    }

    fn launch(
        port: u16,
        rules: Vec<Rule>,
        fixed: Option<(Vec<Link>, Option<String>)>,
    ) -> io::Result<Control> {
        let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, port))
            .or_else(|_| TcpListener::bind((Ipv4Addr::LOCALHOST, 0)))?;
        let port = listener.local_addr()?.port();
        let mut state = State {
            rules,
            slots: Vec::new(),
            primary: None,
            apps: HashMap::new(),
            rescanned: None,
            fixed: fixed.is_some(),
        };
        let now = Instant::now();
        match fixed {
            Some((links, primary)) => {
                state.update(links, now);
                state.primary = primary;
            }
            None => {
                let links = core_netmerge::links();
                let elsewhere = core_netmerge::route_elsewhere(&links);
                state.update(links, now);
                state.find_primary(elsewhere);
            }
        }
        let shared = Arc::new(Shared {
            state: Mutex::new(state),
            stopped: AtomicBool::new(false),
            port,
        });
        {
            let shared = Arc::clone(&shared);
            thread::spawn(move || accept(&shared, &listener));
        }
        {
            let shared = Arc::clone(&shared);
            thread::spawn(move || keep(&shared));
        }
        Ok(Control { shared })
    }

    pub fn port(&self) -> u16 {
        self.shared.port
    }

    pub fn set_rules(&self, rules: Vec<Rule>) {
        self.shared.state().rules = rules;
    }

    pub fn snapshot(&self) -> Snapshot {
        self.shared.state().snapshot()
    }
}

impl Drop for Control {
    fn drop(&mut self) {
        self.shared.stopped.store(true, Relaxed);
        // Wakes the thread that waits for connections, so that it sees it is to stop.
        let _ = TcpStream::connect_timeout(
            &SocketAddr::from((Ipv4Addr::LOCALHOST, self.shared.port)),
            Duration::from_secs(1),
        );
    }
}

impl State {
    fn update(&mut self, links: Vec<Link>, now: Instant) {
        for slot in &mut self.slots {
            slot.present = false;
        }
        for link in links {
            match self.slots.iter_mut().find(|s| s.link.name == link.name) {
                Some(slot) => {
                    if slot.link.ip != link.ip {
                        slot.cooldown = None;
                        slot.peak = 0.0;
                    }
                    slot.present = true;
                    slot.link = link;
                }
                None => self.slots.push(Slot {
                    link,
                    present: true,
                    cooldown: None,
                    down: Arc::default(),
                    up: Arc::default(),
                    tiers: Arc::default(),
                    rate_down: 0.0,
                    rate_up: 0.0,
                    peak: 0.0,
                    caps: [None; 3],
                    open: Arc::default(),
                }),
            }
        }
        self.slots.retain(|s| s.present || s.open.load(Relaxed) > 0);
        self.rescanned = Some(now);
    }

    /// The link the system's route leaves by, as `core_netmerge` finds it; none while a VPN carries everything.
    fn find_primary(&mut self, elsewhere: bool) {
        self.primary = if elsewhere {
            None
        } else {
            let ip = primary_ip();
            self.slots
                .iter()
                .find(|s| s.present && Some(s.link.ip) == ip)
                .map(|s| s.link.name.clone())
        };
    }

    /// The links a new connection of `target` tries, in order; empty is the system's own way.
    fn plan(&mut self, target: &Target, now: Instant) -> Vec<usize> {
        if self.primary.is_none() {
            return Vec::new();
        }
        match target {
            Target::System => Vec::new(),
            Target::Link(name) => self
                .slots
                .iter()
                .position(|s| s.present && &s.link.name == name)
                .into_iter()
                .collect(),
            Target::Merge => {
                let best = self
                    .slots
                    .iter()
                    .filter(|s| s.present)
                    .map(|s| s.peak)
                    .fold(0.0, f64::max);
                let mut pool: Vec<(usize, f64)> = self
                    .slots
                    .iter()
                    .enumerate()
                    .filter(|(_, s)| s.present && s.cooldown.is_none_or(|until| now >= until))
                    .map(|(i, s)| {
                        let weight = if best > 0.0 && s.peak > 0.0 {
                            (s.peak / best).max(0.05)
                        } else {
                            1.0
                        };
                        (i, (s.open.load(Relaxed) + 1) as f64 / weight)
                    })
                    .collect();
                pool.sort_by(|a, b| a.1.total_cmp(&b.1));
                pool.into_iter().map(|(i, _)| i).collect()
            }
        }
    }

    fn tick(&mut self, elapsed: Duration, now: Instant) {
        let secs = elapsed.as_secs_f64().max(0.001);
        for slot in &mut self.slots {
            slot.rate_down = slot.down.take_delta() as f64 / secs;
            slot.rate_up = slot.up.take_delta() as f64 / secs;
            let used = [0, 1, 2].map(|t| slot.tiers[t].take_delta() as f64 / secs);
            slot.peak = if slot.rate_down > slot.peak {
                slot.rate_down
            } else {
                slot.peak + (slot.rate_down - slot.peak) * (1.0 - (-secs / 60.0).exp())
            };
            slot.caps = priority_caps(slot.peak, used);
        }
        let rules = self.rules.clone();
        for (name, app) in &mut self.apps {
            app.rate_down = app.down.take_delta() as f64 / secs;
            app.rate_up = app.up.take_delta() as f64 / secs;
            let rule = rule_for(&rules, name);
            let priority = rule.map_or(Priority::Normal, |r| r.priority);
            let held = app
                .open
                .iter()
                .filter_map(|link| self.slots.iter().find(|s| &s.link.name == link))
                .filter_map(|s| s.caps[priority.index()])
                .fold(None, |low: Option<f64>, cap| {
                    Some(low.map_or(cap, |l| l.min(cap)))
                });
            app.held_to = held;
            let limit = |kbps: u32| (kbps > 0).then(|| f64::from(kbps) * 1024.0);
            let down = match (rule.and_then(|r| limit(r.down_kbps)), held) {
                (Some(a), Some(b)) => Some(a.min(b)),
                (a, b) => a.or(b),
            };
            app.bucket_down
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .set_rate(down, now);
            let up = rule.and_then(|r| limit(r.up_kbps));
            app.bucket_up
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .set_rate(up, now);
        }
        if self.apps.len() > MAX_APPS {
            let mut idle: Vec<(Instant, String)> = self
                .apps
                .iter()
                .filter(|(_, a)| a.open.is_empty())
                .map(|(n, a)| (a.seen, n.clone()))
                .collect();
            idle.sort();
            for (_, name) in idle.into_iter().take(self.apps.len() - MAX_APPS) {
                self.apps.remove(&name);
            }
        }
    }

    fn snapshot(&self) -> Snapshot {
        let links = self
            .slots
            .iter()
            .filter(|s| s.present)
            .map(|s| LinkView {
                name: s.link.name.clone(),
                label: s.link.label.clone(),
                ip: s.link.ip,
                primary: self.primary.as_deref() == Some(&s.link.name),
                down: s.rate_down as u64,
                up: s.rate_up as u64,
                connections: s.open.load(Relaxed),
                received: s.down.total.load(Relaxed),
                sent: s.up.total.load(Relaxed),
            })
            .collect();
        let mut apps: Vec<AppView> = self
            .apps
            .iter()
            .map(|(name, a)| {
                let mut links = a.open.clone();
                links.sort();
                links.dedup();
                AppView {
                    name: name.clone(),
                    bundle: a.bundle.clone(),
                    connections: a.open.len(),
                    down: a.rate_down as u64,
                    up: a.rate_up as u64,
                    received: a.down.total.load(Relaxed),
                    sent: a.up.total.load(Relaxed),
                    links,
                    held_to: a.held_to.map(|h| h as u64),
                }
            })
            .collect();
        apps.sort_by(|a, b| {
            b.down
                .cmp(&a.down)
                .then(b.received.cmp(&a.received))
                .then(a.name.cmp(&b.name))
        });
        Snapshot {
            links,
            apps,
            paused: self.primary.is_none() && !self.slots.is_empty(),
        }
    }
}

/// The address the system sends from by its own route: connecting a UDP socket sends nothing, it only chooses.
fn primary_ip() -> Option<Ipv4Addr> {
    let socket = std::net::UdpSocket::bind((Ipv4Addr::UNSPECIFIED, 0)).ok()?;
    socket.connect((Ipv4Addr::new(1, 1, 1, 1), 53)).ok()?;
    match socket.local_addr().ok()?.ip() {
        IpAddr::V4(ip) if !ip.is_unspecified() => Some(ip),
        _ => None,
    }
}

fn keep(shared: &Arc<Shared>) {
    let mut last = Instant::now();
    while !shared.stopped.load(Relaxed) {
        thread::sleep(TICK);
        let now = Instant::now();
        let due = {
            let state = shared.state();
            !state.fixed
                && state
                    .rescanned
                    .is_none_or(|at| now.duration_since(at) >= RESCAN_EVERY)
        };
        if due {
            // Outside the lock: naming the links may run a program.
            let links = core_netmerge::links();
            let elsewhere = core_netmerge::route_elsewhere(&links);
            let mut state = shared.state();
            state.update(links, now);
            state.find_primary(elsewhere);
        }
        shared.state().tick(now - last, now);
        last = now;
    }
}

fn accept(shared: &Arc<Shared>, listener: &TcpListener) {
    for stream in listener.incoming() {
        if shared.stopped.load(Relaxed) {
            return;
        }
        match stream {
            Ok(stream) => {
                let shared = Arc::clone(shared);
                thread::spawn(move || serve(&shared, stream));
            }
            Err(_) => thread::sleep(Duration::from_millis(50)),
        }
    }
}

/// Everything a connection needs to be copied, counted and held to its limits.
struct Lane {
    app: String,
    link: Option<String>,
    priority: Priority,
    app_down: Arc<Counter>,
    app_up: Arc<Counter>,
    link_down: Option<Arc<Counter>>,
    link_up: Option<Arc<Counter>>,
    link_tiers: Option<Arc<[Counter; 3]>>,
    link_open: Option<Arc<AtomicUsize>>,
    bucket_down: Arc<Mutex<Bucket>>,
    bucket_up: Arc<Mutex<Bucket>>,
}

fn serve(shared: &Arc<Shared>, mut client: TcpStream) {
    let _ = client.set_read_timeout(Some(HANDSHAKE_TIMEOUT));
    let _ = client.set_nodelay(true);
    let Some(request) = handshake::read(&mut client) else {
        return;
    };
    let app = client
        .peer_addr()
        .ok()
        .filter(|a| a.ip().is_loopback())
        .and_then(|a| owner::pid_of_client(a.port(), shared.port))
        .and_then(owner::app_of)
        .unwrap_or(owner::App {
            name: UNKNOWN_APP.into(),
            bundle: String::new(),
        });
    let Some((server, lane)) = dial(shared, &app, &request.host, request.port) else {
        let _ = handshake::answer(&mut client, request.kind, false);
        return;
    };
    if !handshake::answer(&mut client, request.kind, true) || client.set_read_timeout(None).is_err()
    {
        close(shared, &lane);
        return;
    }
    relay(client, server, &lane);
    close(shared, &lane);
}

fn resolve(host: &Host, port: u16) -> Option<Vec<SocketAddr>> {
    match host {
        Host::Ip(ip) => Some(vec![SocketAddr::new(*ip, port)]),
        Host::Name(name) => (name.as_str(), port)
            .to_socket_addrs()
            .ok()
            .map(Iterator::collect)
            .filter(|a: &Vec<SocketAddr>| !a.is_empty()),
    }
}

/// Connects to the site by the app's wire, and opens the app's lane.
fn dial(
    shared: &Arc<Shared>,
    app: &owner::App,
    host: &Host,
    port: u16,
) -> Option<(TcpStream, Lane)> {
    let addrs = resolve(host, port)?;
    let v4 = addrs.iter().find(|a| a.is_ipv4()).copied();
    let (target, priority) = {
        let state = shared.state();
        rule_for(&state.rules, &app.name).map_or((Target::System, Priority::Normal), |r| {
            (r.target.clone(), r.priority)
        })
    };
    let plan = match v4 {
        // A site with IPv6 only takes the system's way: the links are pinned by their IPv4 address.
        Some(_) => shared.state().plan(&target, Instant::now()),
        None => Vec::new(),
    };
    for i in plan {
        let link = match shared.state().slots.get(i) {
            Some(slot) => slot.link.clone(),
            None => continue,
        };
        let lane = open_lane(shared, app, Some(&link.name), priority);
        match core_netmerge::connect_by(Some(&link), v4?, CONNECT_TIMEOUT) {
            Ok(server) => return Some((server, lane)),
            Err(error) => {
                close(shared, &lane);
                if matches!(
                    error.kind(),
                    ErrorKind::NetworkUnreachable
                        | ErrorKind::NetworkDown
                        | ErrorKind::AddrNotAvailable
                ) && let Some(slot) = shared
                    .state()
                    .slots
                    .iter_mut()
                    .find(|s| s.link.name == link.name)
                {
                    slot.cooldown = Some(Instant::now() + LINK_COOLDOWN);
                }
                if !matches!(
                    error.kind(),
                    ErrorKind::NetworkUnreachable
                        | ErrorKind::NetworkDown
                        | ErrorKind::AddrNotAvailable
                        | ErrorKind::HostUnreachable
                        | ErrorKind::TimedOut
                ) {
                    return None;
                }
            }
        }
    }
    // The system's way: what the app asked for, when it is wired so, or when its links could not reach the site.
    let primary = shared.state().primary.clone();
    let lane = open_lane(shared, app, primary.as_deref(), priority);
    let to = v4.unwrap_or(addrs[0]);
    match core_netmerge::connect_by(None, to, CONNECT_TIMEOUT) {
        Ok(server) => Some((server, lane)),
        Err(_) => {
            close(shared, &lane);
            None
        }
    }
}

fn open_lane(
    shared: &Arc<Shared>,
    app: &owner::App,
    link: Option<&str>,
    priority: Priority,
) -> Lane {
    let now = Instant::now();
    let mut state = shared.state();
    let slot = link.and_then(|name| state.slots.iter().find(|s| s.link.name == name));
    let (link_down, link_up, link_tiers, link_open) = match slot {
        Some(s) => {
            s.open.fetch_add(1, Relaxed);
            (
                Some(Arc::clone(&s.down)),
                Some(Arc::clone(&s.up)),
                Some(Arc::clone(&s.tiers)),
                Some(Arc::clone(&s.open)),
            )
        }
        None => (None, None, None, None),
    };
    let link = slot.map(|s| s.link.name.clone());
    let entry = state
        .apps
        .entry(app.name.clone())
        .or_insert_with(|| AppSlot::new(app.bundle.clone(), now));
    entry.seen = now;
    if let Some(name) = &link {
        entry.open.push(name.clone());
    } else {
        entry.open.push(String::new());
    }
    Lane {
        app: app.name.clone(),
        link,
        priority,
        app_down: Arc::clone(&entry.down),
        app_up: Arc::clone(&entry.up),
        link_down,
        link_up,
        link_tiers,
        link_open,
        bucket_down: Arc::clone(&entry.bucket_down),
        bucket_up: Arc::clone(&entry.bucket_up),
    }
}

fn close(shared: &Arc<Shared>, lane: &Lane) {
    if let Some(open) = &lane.link_open {
        open.fetch_sub(1, Relaxed);
    }
    let mut state = shared.state();
    if let Some(app) = state.apps.get_mut(&lane.app) {
        let link = lane.link.clone().unwrap_or_default();
        if let Some(at) = app.open.iter().position(|l| *l == link) {
            app.open.swap_remove(at);
        }
        app.seen = Instant::now();
    }
}

fn relay(client: TcpStream, server: TcpStream, lane: &Lane) {
    let (Ok(mut from_client), Ok(mut to_server)) = (client.try_clone(), server.try_clone()) else {
        return;
    };
    let up_counters = (
        Arc::clone(&lane.app_up),
        lane.link_up.clone(),
        Arc::clone(&lane.bucket_up),
    );
    let up = thread::spawn(move || {
        let (app, link, bucket) = up_counters;
        copy(&mut from_client, &mut to_server, &bucket, |n| {
            app.add(n);
            if let Some(link) = &link {
                link.add(n);
            }
        });
        let _ = to_server.shutdown(Shutdown::Both);
    });
    let (mut from_server, mut to_client) = (server, client);
    let tier = lane.priority.index();
    let whole = copy(&mut from_server, &mut to_client, &lane.bucket_down, |n| {
        lane.app_down.add(n);
        if let Some(link) = &lane.link_down {
            link.add(n);
        }
        if let Some(tiers) = &lane.link_tiers {
            tiers[tier].add(n);
        }
    });
    if whole {
        let _ = to_client.shutdown(Shutdown::Write);
        let _ = to_client.set_read_timeout(Some(LINGER));
    } else {
        let _ = to_client.shutdown(Shutdown::Both);
        let _ = from_server.shutdown(Shutdown::Both);
    }
    let _ = up.join();
}

/// Copies until the end, as fast as the bucket lets it, counting what moves; false when it stopped on an error.
fn copy(
    from: &mut TcpStream,
    to: &mut TcpStream,
    bucket: &Mutex<Bucket>,
    count: impl Fn(usize),
) -> bool {
    let mut buffer = vec![0; BUFFER];
    loop {
        let allowed = loop {
            let took = bucket
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .take(BUFFER, Instant::now());
            match took {
                Ok(n) => break n,
                Err(wait) => thread::sleep(wait),
            }
        };
        match from.read(&mut buffer[..allowed]) {
            Ok(0) => return true,
            Ok(n) => {
                if n < allowed {
                    bucket
                        .lock()
                        .unwrap_or_else(PoisonError::into_inner)
                        .refund(allowed - n);
                }
                count(n);
                if to.write_all(&buffer[..n]).is_err() {
                    return false;
                }
            }
            Err(e) if e.kind() == ErrorKind::Interrupted => {}
            Err(_) => return false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rules::Rule;

    /// A site on 127.0.0.1 that sends `size` bytes to each connection, then closes.
    fn site(size: usize) -> SocketAddr {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        thread::spawn(move || {
            for stream in listener.incoming().flatten() {
                thread::spawn(move || {
                    let mut stream = stream;
                    let chunk = vec![7u8; 16 * 1024];
                    let mut left = size;
                    while left > 0 {
                        let n = left.min(chunk.len());
                        if stream.write_all(&chunk[..n]).is_err() {
                            return;
                        }
                        left -= n;
                    }
                });
            }
        });
        addr
    }

    fn through(control: &Control, site: SocketAddr) -> (usize, Duration) {
        let started = Instant::now();
        let mut s = TcpStream::connect(("127.0.0.1", control.port())).unwrap();
        let port = site.port().to_be_bytes();
        s.write_all(&[5, 1, 0, 5, 1, 0, 1, 127, 0, 0, 1, port[0], port[1]])
            .unwrap();
        let mut head = [0u8; 12];
        s.read_exact(&mut head).unwrap();
        assert_eq!(head[..2], [5, 0], "the method is accepted");
        assert_eq!(head[2..4], [5, 0], "the connection is made");
        let mut got = Vec::new();
        s.read_to_end(&mut got).unwrap();
        (got.len(), started.elapsed())
    }

    fn me() -> String {
        owner::app_of(std::process::id() as i32).unwrap().name
    }

    #[test]
    fn a_connection_is_carried_whole_and_counted_for_its_app() {
        let control = Control::launch(0, Vec::new(), Some((Vec::new(), None))).unwrap();
        let (got, _) = through(&control, site(300_000));
        assert_eq!(got, 300_000);
        thread::sleep(Duration::from_millis(100));
        let snap = control.snapshot();
        let app = snap
            .apps
            .iter()
            .find(|a| a.name == me())
            .expect("the app of the test is named");
        assert_eq!(app.received, 300_000);
        assert_eq!(app.connections, 0, "the connection is over");
    }

    #[test]
    fn the_limit_of_an_app_holds_it_to_that_speed() {
        let rule = Rule {
            app: me(),
            target: Target::System,
            priority: Priority::Normal,
            down_kbps: 200,
            up_kbps: 0,
        };
        let control = Control::launch(0, vec![rule], Some((Vec::new(), None))).unwrap();
        // The bucket takes its rate at the first tick, once the app is known: one connection to learn it first.
        let _ = through(&control, site(1000));
        thread::sleep(Duration::from_millis(1300));
        let (got, took) = through(&control, site(400 * 1024));
        assert_eq!(got, 400 * 1024);
        assert!(
            took >= Duration::from_millis(1500),
            "400 KB at 200 KB/s takes about two seconds, not {took:?}"
        );
    }

    #[test]
    fn a_wire_to_a_link_that_is_not_there_takes_the_systems_way() {
        let rule = Rule {
            app: me(),
            target: Target::Link("en99".into()),
            priority: Priority::High,
            down_kbps: 0,
            up_kbps: 0,
        };
        let control = Control::launch(0, vec![rule], Some((Vec::new(), None))).unwrap();
        let (got, _) = through(&control, site(10_000));
        assert_eq!(got, 10_000);
    }

    #[test]
    fn the_merge_takes_the_least_busy_link_first() {
        let link = |name: &str, last: u8| Link {
            name: name.into(),
            index: 0,
            ip: Ipv4Addr::new(10, 0, 0, last),
            label: name.into(),
        };
        let now = Instant::now();
        let mut state = State {
            rules: Vec::new(),
            slots: Vec::new(),
            primary: Some("en0".into()),
            apps: HashMap::new(),
            rescanned: None,
            fixed: true,
        };
        state.update(vec![link("en0", 1), link("en7", 2)], now);
        state.slots[0].open.store(3, Relaxed);
        assert_eq!(state.plan(&Target::Merge, now), [1, 0]);
        state.slots[1].cooldown = Some(now + LINK_COOLDOWN);
        assert_eq!(
            state.plan(&Target::Merge, now),
            [0],
            "a link that just failed sits out"
        );
        assert_eq!(
            state.plan(&Target::Link("en7".into()), now),
            [1],
            "a wire to one link asks for it alone"
        );
        assert!(state.plan(&Target::System, now).is_empty());
        state.primary = None;
        assert!(
            state.plan(&Target::Merge, now).is_empty(),
            "behind a VPN that carries everything, nothing is pinned"
        );
    }
}
