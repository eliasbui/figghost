//! net-merge inside one app: the app sends what it downloads through a SOCKS5 proxy on 127.0.0.1, and each connection
//! made through it leaves the computer by one of its network connections (Wi-Fi, Ethernet, a phone tethered by USB),
//! so a download that opens several connections uses all of them at once.
//!
//! This is the scheduler of net-merge (https://github.com/net-merge/net-merge, MIT licence, Rohit Mahto) without its
//! virtual adapter. A connection is pinned to its link by the interface (`IP_BOUND_IF` on macOS) and by the source
//! address, which is enough on Windows, since it sends a packet only by the interface that holds its source address.
//! So nothing needs an administrator, and only what the app sends through the proxy takes part. A new connection goes
//! to the link with the fewest busy connections for its weight, a weight follows the speed its link was seen to reach,
//! and a link that does not reach the internet gets none.
//!
//! Some networks break some sites: a Wi-Fi whose firewall replaces the certificates of video sites, an address signed
//! for the public address of another link. A program such as yt-dlp gives up at the first certificate it refuses, so a
//! link is trusted with a site only once it has been checked for it: the system's curl must reach the site over that
//! link with a certificate it accepts. Meanwhile, and always for the first busy connection of a download to a site,
//! connections take the link the system would have taken. A link on which a connection to a site ended at once with
//! almost nothing received is kept away from that site for a while, longer each time, and checked again after.
//!
//! The same file is figghost's `crates/core-netmerge/src/lib.rs` and Lanes' `src/netmerge.rs`; change both together.

use socket2::{Domain, Protocol, Socket, Type};
use std::{
    hash::{BuildHasher, Hasher, RandomState},
    io::{self, ErrorKind, Read, Write},
    net::{
        IpAddr, Ipv4Addr, Shutdown, SocketAddr, SocketAddrV4, TcpListener, TcpStream,
        ToSocketAddrs, UdpSocket,
    },
    sync::{
        Arc, Mutex, MutexGuard, PoisonError,
        atomic::{AtomicBool, AtomicU64, Ordering::Relaxed},
    },
    thread,
    time::{Duration, Instant},
};

/// How long a connection to a site may take before another link is tried.
const CONNECT_TIMEOUT: Duration = Duration::from_secs(8);
/// How long a program has to say what it wants once it has connected to the proxy.
const HANDSHAKE_TIMEOUT: Duration = Duration::from_secs(15);
/// After the site has sent everything, how long the program may keep its side open.
const LINGER: Duration = Duration::from_secs(60);
/// A link reaches the internet when a TCP connection to one of these opens from it in time.
const PROBE_TARGETS: [SocketAddrV4; 2] = [
    SocketAddrV4::new(Ipv4Addr::new(1, 1, 1, 1), 443),
    SocketAddrV4::new(Ipv4Addr::new(8, 8, 8, 8), 443),
];
const PROBE_TIMEOUT: Duration = Duration::from_secs(2);
/// A link that works is probed now and then, one that does not more often, so that it is back within seconds.
const PROBE_UP_EVERY: Duration = Duration::from_secs(15);
const PROBE_DOWN_EVERY: Duration = Duration::from_secs(4);
/// How often the links are listed again, so that a phone plugged in joins and one unplugged leaves.
const RESCAN_EVERY: Duration = Duration::from_secs(3);
const TICK: Duration = Duration::from_secs(1);
/// A link whose own connection failed (no network, no address) is left out for this long.
const LINK_COOLDOWN: Duration = Duration::from_secs(15);
/// A connection that received nothing for this many ticks is not busy: programs keep idle connections open for the next
/// request, and those should not steer new ones away from their link.
const IDLE_TICKS: u32 = 2;
/// A link counts as measured once it has carried this much a second (about 0.4 Mbit/s).
const MEASURED: f64 = 50_000.0;
/// A connection that ends within `SUSPECT_TIME` with less than this received has failed in all likelihood (a refused
/// certificate, a 403 for an address signed for another link); anything more proves the link for the site.
const SUSPECT_BYTES: u64 = 16 * 1024;
const SUSPECT_TIME: Duration = Duration::from_secs(10);
/// How long a link stays away from a site after a failure there; each failure in a row doubles it.
const AVOID_FIRST: Duration = Duration::from_secs(30);
const AVOID_MAX: Duration = Duration::from_secs(600);
/// How long a second connection to a site waits for the links that have not served it yet to be checked.
const CHECK_WAIT: Duration = Duration::from_secs(3);
/// Sites that sign the address of a video for the public address that asked for it, so that another link is refused:
/// a download's connections to them all take the system's link.
const ONE_LINK: [&str; 1] = ["googlevideo.com"];
const BUFFER: usize = 64 * 1024;

/// The replies of SOCKS5 (RFC 1928).
const REP_OK: u8 = 0;
const REP_FAILURE: u8 = 1;
const REP_NET_UNREACHABLE: u8 = 3;
const REP_HOST_UNREACHABLE: u8 = 4;
const REP_REFUSED: u8 = 5;
const REP_COMMAND: u8 = 7;
const REP_ADDRESS: u8 = 8;

/// A network connection of this computer: its interface and the IPv4 address connections leave from.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Link {
    /// The interface: `en0` on a Mac, the adapter's name on Windows.
    pub name: String,
    pub index: u32,
    pub ip: Ipv4Addr,
    /// What the system calls it: Wi-Fi, Ethernet, iPhone USB.
    pub label: String,
}

/// The links that may carry downloads: real hardware that is up, with an IPv4 address (and on Windows a gateway).
/// Tunnels and VPNs are left out; a probe later finds which of the rest reach the internet.
pub fn links() -> Vec<Link> {
    sys::links()
}

/// What a page shows of one link.
#[derive(Debug, Clone, PartialEq)]
pub struct LinkStats {
    pub name: String,
    pub label: String,
    pub ip: Ipv4Addr,
    /// `idle` while no proxy runs, then `checking`, `up` or `down` (it does not reach the internet).
    pub state: &'static str,
    /// The link the system uses by itself, which takes the first connection of each download to a site.
    pub primary: bool,
    /// Connections open through it.
    pub connections: usize,
    /// Bytes a second received through it, over the last second.
    pub speed: u64,
    /// Bytes received through it since the proxy started.
    pub received: u64,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct Stats {
    pub links: Vec<LinkStats>,
    /// The system's own route leaves by none of the links, as when a VPN carries everything: connections then take that
    /// route and nothing is merged, so that nothing goes around the VPN.
    pub paused: bool,
}

/// The links while no proxy runs, for a page that shows them before the first download.
pub fn idle_stats() -> Stats {
    let primary = primary_ip();
    let links: Vec<LinkStats> = links()
        .into_iter()
        .map(|link| LinkStats {
            primary: Some(link.ip) == primary,
            name: link.name,
            label: link.label,
            ip: link.ip,
            state: "idle",
            connections: 0,
            speed: 0,
            received: 0,
        })
        .collect();
    let paused = !links.iter().any(|l| l.primary);
    Stats { links, paused }
}

/// The proxy and its scheduler, from `start` until it is dropped; connections already open finish on their own.
pub struct Merge {
    shared: Arc<Shared>,
    port: u16,
    token: String,
}

impl Merge {
    /// Starts the proxy on a free port of 127.0.0.1, with the links of this computer.
    pub fn start() -> io::Result<Merge> {
        Self::launch(None)
    }

    /// The proxy with links that are given, never listed again and never probed: each is taken to work. A link with
    /// index 0 is pinned by its address alone.
    #[cfg(test)]
    fn with_links(links: Vec<Link>, primary: Option<&str>) -> io::Result<Merge> {
        Self::launch(Some((links, primary.map(str::to_owned))))
    }

    fn launch(fixed: Option<(Vec<Link>, Option<String>)>) -> io::Result<Merge> {
        let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0))?;
        let port = listener.local_addr()?.port();
        let token = token();
        let shared = Arc::new(Shared {
            sched: Mutex::new(Sched::new()),
            stopped: AtomicBool::new(false),
            fixed: fixed.is_some(),
        });
        {
            let mut sched = shared.sched();
            match fixed {
                Some((links, primary)) => {
                    sched.update(links, None, Instant::now());
                    for slot in &mut sched.slots {
                        slot.state = State::Up;
                    }
                    sched.primary = primary;
                }
                // Listed at once, so that the first connection is pinned already.
                None => sched.update(links(), primary_ip(), Instant::now()),
            }
        }
        {
            let (shared, token) = (Arc::clone(&shared), token.clone());
            thread::spawn(move || accept(&shared, &listener, &token));
        }
        {
            let shared = Arc::clone(&shared);
            thread::spawn(move || keep(&shared));
        }
        Ok(Merge {
            shared,
            port,
            token,
        })
    }

    /// The address of the proxy for one download, as a URL of `scheme`: `socks5h` for yt-dlp, which then leaves the
    /// names of sites to the proxy, `socks5` for ureq, which does that anyway. `job` names the download, so that its
    /// first connection to each site takes the system's own link.
    pub fn proxy_url(&self, scheme: &str, job: &str) -> String {
        format!(
            "{scheme}://{}:{}@127.0.0.1:{}",
            job_name(job),
            self.token,
            self.port
        )
    }

    pub fn stats(&self) -> Stats {
        self.shared.sched().stats()
    }

    /// How many links carry a busy connection of `job` now.
    pub fn links_used(&self, job: &str) -> usize {
        let job = job_name(job);
        let sched = self.shared.sched();
        let mut used: Vec<&str> = sched
            .open
            .iter()
            .filter(|o| o.busy() && o.flow.job == job)
            .filter_map(|o| o.flow.link.as_deref())
            .collect();
        used.sort_unstable();
        used.dedup();
        used.len()
    }
}

impl Drop for Merge {
    fn drop(&mut self) {
        self.shared.stopped.store(true, Relaxed);
        // Wakes the thread that waits for connections, so that it sees it is to stop.
        let _ = TcpStream::connect_timeout(
            &SocketAddr::from((Ipv4Addr::LOCALHOST, self.port)),
            Duration::from_secs(1),
        );
    }
}

/// The name a download goes by in the proxy: what of `job` may stand in the user part of a URL, at most 64 characters.
fn job_name(job: &str) -> String {
    let name: String = job
        .chars()
        .filter(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_'))
        .take(64)
        .collect();
    if name.is_empty() { "app".into() } else { name }
}

/// A secret for the proxy's password, so that no other program on this computer can use it: the keys the standard
/// library takes from the system's randomness, through its keyed hash.
fn token() -> String {
    let part = || {
        let mut hasher = RandomState::new().build_hasher();
        hasher.write_u32(std::process::id());
        hasher.finish()
    };
    format!("{:016x}{:016x}", part(), part())
}

/// The part of a host name that a site is known by, so that what is learnt of a link for `rr3---sn-x.googlevideo.com`
/// holds for the next video's server: the last two labels, or three under a country's own second level
/// (`example.com.vn`, `bbc.co.uk`). An address stands for itself.
fn domain_key(host: &str) -> String {
    let host = host.trim_end_matches('.').to_ascii_lowercase();
    if host.parse::<IpAddr>().is_ok() {
        return host;
    }
    let labels: Vec<&str> = host.split('.').collect();
    let n = labels.len();
    if n <= 2 {
        return host;
    }
    let country_level = labels[n - 1].len() == 2
        && matches!(
            labels[n - 2],
            "com" | "net" | "org" | "edu" | "gov" | "co" | "ac" | "or" | "ne" | "go"
        );
    labels[n - if country_level { 3 } else { 2 }..].join(".")
}

/// The address the system sends from by its own route: connecting a UDP socket sends nothing, it only chooses.
fn primary_ip() -> Option<Ipv4Addr> {
    let socket = UdpSocket::bind((Ipv4Addr::UNSPECIFIED, 0)).ok()?;
    socket.connect((Ipv4Addr::new(1, 1, 1, 1), 53)).ok()?;
    match socket.local_addr().ok()?.ip() {
        IpAddr::V4(ip) if !ip.is_unspecified() => Some(ip),
        _ => None,
    }
}

/// A TCP connection to `to` that leaves by `link`, or by the system's own route without one.
fn connect(link: Option<&Link>, to: SocketAddr, timeout: Duration) -> io::Result<TcpStream> {
    let socket = Socket::new(Domain::for_address(to), Type::STREAM, Some(Protocol::TCP))?;
    if let Some(link) = link {
        #[cfg(target_os = "macos")]
        socket.bind_device_by_index_v4(std::num::NonZeroU32::new(link.index))?;
        socket.bind(&SocketAddr::from((link.ip, 0)).into())?;
    }
    socket.connect_timeout(&to.into(), timeout)?;
    Ok(socket.into())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum State {
    Checking,
    Up,
    Down,
}

struct Slot {
    link: Link,
    present: bool,
    state: State,
    probing: bool,
    probed: Option<Instant>,
    /// Left out until then, after a failure of the link itself.
    cooldown: Option<Instant>,
    weight: f64,
    /// The speed it was seen to reach, smoothed.
    peak: f64,
    received: Arc<AtomicU64>,
    last: u64,
    rate: f64,
}

impl Slot {
    fn new(link: Link) -> Self {
        Self {
            link,
            present: true,
            state: State::Checking,
            probing: false,
            probed: None,
            cooldown: None,
            weight: 1.0,
            peak: 0.0,
            received: Arc::default(),
            last: 0,
            rate: 0.0,
        }
    }

    /// Its tier for a new connection to a site: 0 works, 1 is not probed yet, 2 is not checked for this site yet, 3
    /// failed for this site of late, 4 does not work. Only the best tier that has a link is used, so that every link
    /// looking broken still leaves one.
    fn tier(&self, avoided: bool, unchecked: bool, now: Instant) -> u8 {
        if self.state == State::Down || self.cooldown.is_some_and(|until| now < until) {
            4
        } else if avoided {
            3
        } else if unchecked {
            2
        } else if self.state == State::Checking {
            1
        } else {
            0
        }
    }
}

/// One connection through the proxy. What it receives is counted without the lock.
struct Flow {
    id: u64,
    /// Its link, `None` when it takes the system's own route.
    link: Option<String>,
    job: String,
    domain: String,
    started: Instant,
    received: AtomicU64,
    link_received: Option<Arc<AtomicU64>>,
}

impl Flow {
    fn count(&self, n: u64) {
        self.received.fetch_add(n, Relaxed);
        if let Some(link) = &self.link_received {
            link.fetch_add(n, Relaxed);
        }
    }
}

/// A flow as the scheduler follows it.
struct Open {
    flow: Arc<Flow>,
    last: u64,
    idle: u32,
}

impl Open {
    fn busy(&self) -> bool {
        self.idle < IDLE_TICKS
    }
}

/// A link kept away from a site until `until`.
struct Avoid {
    link: String,
    domain: String,
    strikes: u32,
    until: Instant,
}

/// A link checked for a site, or being checked since `since`.
struct Trust {
    link: String,
    domain: String,
    good: bool,
    since: Instant,
}

struct Sched {
    slots: Vec<Slot>,
    /// The link the system's own route leaves by; `None` when that is none of the links.
    primary: Option<String>,
    open: Vec<Open>,
    avoid: Vec<Avoid>,
    trust: Vec<Trust>,
    next_id: u64,
    seed: u64,
    rescanned: Option<Instant>,
}

impl Sched {
    fn new() -> Self {
        Self {
            slots: Vec::new(),
            primary: None,
            open: Vec::new(),
            avoid: Vec::new(),
            trust: Vec::new(),
            next_id: 0,
            seed: RandomState::new().build_hasher().finish() | 1,
            rescanned: None,
        }
    }

    /// Takes a new list of the links: a new one is checked before it is trusted, one whose address changed is checked
    /// again from the start, and one that went away is forgotten once nothing of it is open.
    fn update(&mut self, links: Vec<Link>, primary: Option<Ipv4Addr>, now: Instant) {
        for slot in &mut self.slots {
            slot.present = false;
        }
        for link in links {
            match self.slots.iter_mut().find(|s| s.link.name == link.name) {
                Some(slot) => {
                    if slot.link.ip != link.ip {
                        slot.state = State::Checking;
                        slot.probed = None;
                        slot.cooldown = None;
                        slot.peak = 0.0;
                        slot.weight = 1.0;
                    }
                    slot.present = true;
                    slot.link = link;
                }
                None => self.slots.push(Slot::new(link)),
            }
        }
        let open = &self.open;
        self.slots.retain(|s| {
            s.present
                || open
                    .iter()
                    .any(|o| o.flow.link.as_deref() == Some(&s.link.name))
        });
        self.primary = primary.and_then(|ip| {
            self.slots
                .iter()
                .find(|s| s.present && s.link.ip == ip)
                .map(|s| s.link.name.clone())
        });
        self.rescanned = Some(now);
    }

    fn avoided(&self, link: &str, domain: &str, now: Instant) -> bool {
        self.avoid
            .iter()
            .any(|a| a.link == link && a.domain == domain && now < a.until)
    }

    fn trusted(&self, link: &str, domain: &str) -> bool {
        self.trust
            .iter()
            .any(|t| t.good && t.link == link && t.domain == domain)
    }

    /// The links to check for `domain` now, each marked as being checked: those that work, are not the system's, and
    /// have neither been checked for it nor are kept away from it.
    fn start_checks(&mut self, domain: &str, now: Instant) -> Vec<Link> {
        let mut due = Vec::new();
        for slot in &self.slots {
            let name = &slot.link.name;
            if !slot.present
                || slot.state == State::Down
                || self.primary.as_ref() == Some(name)
                || self.avoided(name, domain, now)
                || self
                    .trust
                    .iter()
                    .any(|t| &t.link == name && t.domain == domain)
            {
                continue;
            }
            due.push(slot.link.clone());
        }
        for link in &due {
            self.trust.push(Trust {
                link: link.name.clone(),
                domain: domain.to_owned(),
                good: false,
                since: now,
            });
        }
        due
    }

    /// Whether a link is still being checked for `domain`, within the time a connection waits for it.
    fn checking(&self, domain: &str, now: Instant) -> bool {
        self.trust
            .iter()
            .any(|t| !t.good && t.domain == domain && now.duration_since(t.since) < CHECK_WAIT)
    }

    /// The answer of a check: a link that passed is trusted with the site, one that failed is kept away from it.
    fn checked(&mut self, link: &str, domain: &str, good: bool, now: Instant) {
        if good {
            for t in &mut self.trust {
                if t.link == link && t.domain == domain {
                    t.good = true;
                }
            }
        } else {
            self.strike(link, domain, now);
        }
    }

    /// The link for a new connection of `job` to `domain`, leaving out the links `tried`; `None` when none is left. With
    /// `check`, a link other than the system's takes the site only once it has been checked for it.
    fn pick(
        &mut self,
        job: &str,
        domain: &str,
        check: bool,
        tried: &[String],
        now: Instant,
    ) -> Option<usize> {
        let tiers: Vec<(usize, u8)> = self
            .slots
            .iter()
            .enumerate()
            .filter(|(_, s)| s.present && !tried.contains(&s.link.name))
            .map(|(i, s)| {
                let name = &s.link.name;
                let unchecked =
                    check && self.primary.as_ref() != Some(name) && !self.trusted(name, domain);
                (i, s.tier(self.avoided(name, domain, now), unchecked, now))
            })
            .collect();
        let best = tiers.iter().map(|&(_, tier)| tier).min()?;
        let pool: Vec<usize> = tiers
            .iter()
            .filter(|&&(_, tier)| tier == best)
            .map(|&(i, _)| i)
            .collect();
        // The first busy connection of a download to a site goes the way it would have gone without net-merge: a
        // program's own requests to a site come one at a time, and a broken link must not fail them.
        let first = ONE_LINK.contains(&domain)
            || !self
                .open
                .iter()
                .any(|o| o.busy() && o.flow.job == job && o.flow.domain == domain);
        let primary = self.primary.as_deref();
        if first
            && let Some(&i) = pool
                .iter()
                .find(|&&i| Some(self.slots[i].link.name.as_str()) == primary)
        {
            return Some(i);
        }
        // The fewest busy connections for the weight, from a random start, so that ties do not follow a pattern.
        let start = self.random(pool.len());
        let mut chosen = None;
        let mut lowest = f64::INFINITY;
        for k in 0..pool.len() {
            let i = pool[(start + k) % pool.len()];
            let name = self.slots[i].link.name.as_str();
            let busy = self
                .open
                .iter()
                .filter(|o| o.busy() && o.flow.link.as_deref() == Some(name))
                .count();
            let score = (busy + 1) as f64 / self.slots[i].weight;
            if score < lowest {
                lowest = score;
                chosen = Some(i);
            }
        }
        chosen
    }

    fn random(&mut self, n: usize) -> usize {
        // xorshift: enough to break ties.
        self.seed ^= self.seed << 13;
        self.seed ^= self.seed >> 7;
        self.seed ^= self.seed << 17;
        (self.seed % n.max(1) as u64) as usize
    }

    /// A connection being made: it counts as busy for its link from now, so that the next one goes elsewhere.
    fn open_flow(
        &mut self,
        slot: Option<usize>,
        job: &str,
        domain: &str,
        now: Instant,
    ) -> Arc<Flow> {
        self.next_id += 1;
        let slot = slot.and_then(|i| self.slots.get(i));
        let flow = Arc::new(Flow {
            id: self.next_id,
            link: slot.map(|s| s.link.name.clone()),
            job: job.to_owned(),
            domain: domain.to_owned(),
            started: now,
            received: AtomicU64::new(0),
            link_received: slot.map(|s| Arc::clone(&s.received)),
        });
        self.open.push(Open {
            flow: Arc::clone(&flow),
            last: 0,
            idle: 0,
        });
        flow
    }

    /// A connection that never got going: it no longer counts, and says nothing of its link.
    fn forget(&mut self, id: u64) {
        self.open.retain(|o| o.flow.id != id);
    }

    /// A connection that is over: it no longer counts, and how it went says whether its link suits its site.
    fn close(&mut self, id: u64, now: Instant) {
        let Some(at) = self.open.iter().position(|o| o.flow.id == id) else {
            return;
        };
        let flow = self.open.swap_remove(at).flow;
        let Some(link) = flow.link.as_deref() else {
            return;
        };
        if flow.received.load(Relaxed) >= SUSPECT_BYTES {
            self.avoid
                .retain(|a| !(a.link == link && a.domain == flow.domain));
        } else if now.duration_since(flow.started) < SUSPECT_TIME {
            self.strike(link, &flow.domain, now);
        }
    }

    /// Keeps `link` away from `domain` for a while, longer for each failure in a row. The system's own link is never
    /// kept away: it is what the program would have used without net-merge.
    fn strike(&mut self, link: &str, domain: &str, now: Instant) {
        if self.primary.as_deref() == Some(link) {
            return;
        }
        // Checked again once the time is up.
        self.trust
            .retain(|t| !(t.link == link && t.domain == domain));
        match self
            .avoid
            .iter_mut()
            .find(|a| a.link == link && a.domain == domain)
        {
            Some(avoid) => {
                avoid.strikes += 1;
                avoid.until = now + backoff(avoid.strikes);
            }
            None => self.avoid.push(Avoid {
                link: link.to_owned(),
                domain: domain.to_owned(),
                strikes: 1,
                until: now + backoff(1),
            }),
        }
    }

    /// The link itself failed (no network, no address): it gets nothing for a while.
    fn cool(&mut self, link: &str, now: Instant) {
        if let Some(slot) = self.slots.iter_mut().find(|s| s.link.name == link) {
            slot.cooldown = Some(now + LINK_COOLDOWN);
        }
    }

    fn rescan_due(&self, now: Instant) -> bool {
        self.rescanned
            .is_none_or(|at| now.duration_since(at) >= RESCAN_EVERY)
    }

    /// Once a second: which connections are idle, the speed of each link and from it the weights. Gives the links that
    /// are due a probe when `probes`, marked as being probed.
    fn tick(&mut self, elapsed: Duration, now: Instant, probes: bool) -> Vec<Link> {
        let secs = elapsed.as_secs_f64().max(0.001);
        for open in &mut self.open {
            let got = open.flow.received.load(Relaxed);
            if got == open.last {
                open.idle = open.idle.saturating_add(1);
            } else {
                open.last = got;
                open.idle = 0;
            }
        }
        let mut quiet = Vec::new();
        for (i, slot) in self.slots.iter_mut().enumerate() {
            let got = slot.received.load(Relaxed);
            slot.rate = got.saturating_sub(slot.last) as f64 / secs;
            slot.last = got;
            if slot.rate >= MEASURED && slot.rate >= 0.05 * slot.peak {
                if slot.peak == 0.0 {
                    slot.peak = slot.rate;
                } else {
                    // Quick to rise and slow to fall, so that neither a burst nor a quiet second reorders the links.
                    let tau = if slot.rate < slot.peak { 30.0 } else { 2.0 };
                    slot.peak += (slot.rate - slot.peak) * (1.0 - (-secs / tau).exp());
                }
            } else {
                quiet.push(i);
            }
        }
        let live = |s: &Slot| s.present && s.state != State::Down;
        let best = self
            .slots
            .iter()
            .filter(|s| live(s))
            .map(|s| s.peak)
            .fold(0.0, f64::max);
        // A quiet link may be rated too low; it recovers, but never past half the best one only because nothing flows.
        for i in quiet {
            let slot = &mut self.slots[i];
            if live(slot) && slot.peak > 0.0 {
                slot.peak = (slot.peak * 1.03).min(slot.peak.max(0.5 * best));
            }
        }
        let count = self.slots.iter().filter(|s| live(s)).count();
        let measured = self
            .slots
            .iter()
            .filter(|s| live(s))
            .all(|s| s.peak >= MEASURED);
        for slot in self.slots.iter_mut().filter(|s| live(s)) {
            // Equal weights until every link has carried enough to be measured.
            slot.weight = if count < 2 || !measured || best <= 0.0 {
                1.0
            } else {
                (slot.peak / best).max(0.05)
            };
        }
        // An avoidance that ran out is kept a while longer, so that a link that fails again stays away longer.
        self.avoid.retain(|a| now < a.until + AVOID_MAX);
        let mut due = Vec::new();
        for slot in self.slots.iter_mut() {
            if !probes || !slot.present || slot.probing {
                continue;
            }
            let every = match slot.state {
                State::Checking => Duration::ZERO,
                State::Up => PROBE_UP_EVERY,
                State::Down => PROBE_DOWN_EVERY,
            };
            if slot.probed.is_none_or(|at| now.duration_since(at) >= every) {
                slot.probing = true;
                slot.probed = Some(now);
                due.push(slot.link.clone());
            }
        }
        due
    }

    fn probed(&mut self, link: &Link, up: bool) {
        if let Some(slot) = self
            .slots
            .iter_mut()
            .find(|s| s.link.name == link.name && s.link.ip == link.ip)
        {
            slot.probing = false;
            slot.state = if up { State::Up } else { State::Down };
            if up {
                slot.cooldown = None;
            }
        } else if let Some(slot) = self.slots.iter_mut().find(|s| s.link.name == link.name) {
            // Its address changed while it was probed: the new one is probed afresh.
            slot.probing = false;
        }
    }

    fn stats(&self) -> Stats {
        let links = self
            .slots
            .iter()
            .filter(|s| s.present)
            .map(|s| LinkStats {
                name: s.link.name.clone(),
                label: s.link.label.clone(),
                ip: s.link.ip,
                state: match s.state {
                    State::Checking => "checking",
                    State::Up => "up",
                    State::Down => "down",
                },
                primary: self.primary.as_deref() == Some(&s.link.name),
                connections: self
                    .open
                    .iter()
                    .filter(|o| o.flow.link.as_deref() == Some(&s.link.name))
                    .count(),
                speed: s.rate as u64,
                received: s.received.load(Relaxed),
            })
            .collect();
        Stats {
            links,
            paused: self.primary.is_none(),
        }
    }
}

/// How long a link stays away from a site after `strikes` failures in a row.
fn backoff(strikes: u32) -> Duration {
    (AVOID_FIRST * 2u32.pow(strikes.clamp(1, 6) - 1)).min(AVOID_MAX)
}

/// Whether `host` is a name that can stand in a URL as it is: letters, digits, dots and hyphens.
fn plain_host(host: &str) -> bool {
    !host.is_empty()
        && host.len() <= 253
        && host.parse::<IpAddr>().is_err()
        && host
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'-'))
        && !host.starts_with(['.', '-'])
}

/// Whether `host` answers over `link` with a certificate the system's curl accepts. A firewall that replaces the
/// certificates of some sites on one network fails this, as it would fail the program. Any answer of HTTP will do.
fn reaches_securely(link: &Link, host: &str) -> bool {
    #[cfg(windows)]
    let (curl, null, interface) = (
        std::path::PathBuf::from(
            std::env::var_os("SystemRoot").unwrap_or_else(|| "C:\\Windows".into()),
        )
        .join(r"System32\curl.exe"),
        "NUL",
        link.ip.to_string(),
    );
    #[cfg(not(windows))]
    let (curl, null, interface) = ("/usr/bin/curl", "/dev/null", link.name.clone());
    let mut command = std::process::Command::new(curl);
    command
        .args([
            "-sSI",
            "--proto",
            "=https",
            "--connect-timeout",
            "3",
            "--max-time",
            "5",
        ])
        .args(["--interface", &interface, "-o", null])
        .arg(format!("https://{host}/"))
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        // No console window.
        command.creation_flags(0x0800_0000);
    }
    command.status().is_ok_and(|status| status.success())
}

struct Shared {
    sched: Mutex<Sched>,
    stopped: AtomicBool,
    /// The links were given: they are neither listed again nor probed.
    fixed: bool,
}

impl Shared {
    fn sched(&self) -> MutexGuard<'_, Sched> {
        self.sched.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

/// Lists the links again, measures them and probes them, until the proxy stops.
fn keep(shared: &Arc<Shared>) {
    let mut last = Instant::now();
    while !shared.stopped.load(Relaxed) {
        thread::sleep(TICK);
        let now = Instant::now();
        if !shared.fixed && shared.sched().rescan_due(now) {
            // Outside the lock: naming the links may run a program.
            let (links, primary) = (links(), primary_ip());
            shared.sched().update(links, primary, now);
        }
        let due = shared.sched().tick(now - last, now, !shared.fixed);
        last = now;
        for link in due {
            let shared = Arc::clone(shared);
            thread::spawn(move || {
                let up = PROBE_TARGETS
                    .iter()
                    .any(|to| connect(Some(&link), SocketAddr::V4(*to), PROBE_TIMEOUT).is_ok());
                shared.sched().probed(&link, up);
            });
        }
    }
}

fn accept(shared: &Arc<Shared>, listener: &TcpListener, token: &str) {
    for stream in listener.incoming() {
        if shared.stopped.load(Relaxed) {
            return;
        }
        match stream {
            Ok(stream) => {
                let (shared, token) = (Arc::clone(shared), token.to_owned());
                thread::spawn(move || serve(&shared, stream, &token));
            }
            // Out of files for a moment: waiting beats spinning.
            Err(_) => thread::sleep(Duration::from_millis(50)),
        }
    }
}

/// Where a program asked to go.
enum Host {
    Ip(IpAddr),
    Name(String),
}

impl Host {
    fn resolve(&self, port: u16) -> Option<Vec<SocketAddr>> {
        match self {
            Host::Ip(ip) => Some(vec![SocketAddr::new(*ip, port)]),
            Host::Name(name) => (name.as_str(), port)
                .to_socket_addrs()
                .ok()
                .map(Iterator::collect)
                .filter(|addrs: &Vec<SocketAddr>| !addrs.is_empty()),
        }
    }

    fn domain(&self) -> String {
        match self {
            Host::Ip(ip) => ip.to_string(),
            Host::Name(name) => domain_key(name),
        }
    }
}

fn serve(shared: &Arc<Shared>, mut client: TcpStream, token: &str) {
    let _ = client.set_read_timeout(Some(HANDSHAKE_TIMEOUT));
    let Some((job, host, port)) = handshake(&mut client, token) else {
        return;
    };
    let (server, flow) = match dial(shared, &job, &host, port) {
        Ok(pair) => pair,
        Err(rep) => {
            let _ = client.write_all(&reply(rep));
            return;
        }
    };
    if client.write_all(&reply(REP_OK)).is_err() || client.set_read_timeout(None).is_err() {
        shared.sched().forget(flow.id);
        return;
    }
    relay(client, server, &flow, || {
        shared.sched().close(flow.id, Instant::now());
    });
}

/// Reads the greeting, the user name and password (RFC 1929) and the CONNECT request. The user name is the job, and
/// the password must be the proxy's token.
fn handshake(client: &mut TcpStream, token: &str) -> Option<(String, Host, u16)> {
    let [version, count] = read_array(client)?;
    let methods = read_vec(client, count.into())?;
    if version != 5 {
        return None;
    }
    if !methods.contains(&2) {
        let _ = client.write_all(&[5, 0xFF]);
        return None;
    }
    client.write_all(&[5, 2]).ok()?;
    let [auth, length] = read_array(client)?;
    let user = read_vec(client, length.into())?;
    let [length] = read_array(client)?;
    let password = read_vec(client, length.into())?;
    if auth != 1 || password != token.as_bytes() {
        let _ = client.write_all(&[1, 1]);
        return None;
    }
    client.write_all(&[1, 0]).ok()?;
    let [version, command, _, kind] = read_array(client)?;
    if version != 5 {
        return None;
    }
    let host = match kind {
        1 => Host::Ip(IpAddr::from(read_array::<4>(client)?)),
        3 => {
            let [length] = read_array(client)?;
            let name = String::from_utf8(read_vec(client, length.into())?).ok()?;
            if name.is_empty() {
                return None;
            }
            Host::Name(name)
        }
        4 => Host::Ip(IpAddr::from(read_array::<16>(client)?)),
        _ => {
            let _ = client.write_all(&reply(REP_ADDRESS));
            return None;
        }
    };
    let port = u16::from_be_bytes(read_array(client)?);
    if command != 1 {
        let _ = client.write_all(&reply(REP_COMMAND));
        return None;
    }
    Some((job_name(&String::from_utf8_lossy(&user)), host, port))
}

fn read_array<const N: usize>(from: &mut TcpStream) -> Option<[u8; N]> {
    let mut bytes = [0; N];
    from.read_exact(&mut bytes).ok()?;
    Some(bytes)
}

fn read_vec(from: &mut TcpStream, length: usize) -> Option<Vec<u8>> {
    let mut bytes = vec![0; length];
    from.read_exact(&mut bytes).ok()?;
    Some(bytes)
}

/// A reply without the address the proxy connected from, which no program here needs.
fn reply(rep: u8) -> [u8; 10] {
    [5, rep, 0, 1, 0, 0, 0, 0, 0, 0]
}

struct Class {
    rep: u8,
    /// Another link may do better.
    retry: bool,
    /// The link itself is broken.
    link_down: bool,
}

fn classify(error: &io::Error) -> Class {
    let (rep, retry, link_down) = match error.kind() {
        ErrorKind::NetworkUnreachable | ErrorKind::NetworkDown | ErrorKind::AddrNotAvailable => {
            (REP_NET_UNREACHABLE, true, true)
        }
        ErrorKind::HostUnreachable | ErrorKind::TimedOut => (REP_HOST_UNREACHABLE, true, false),
        ErrorKind::ConnectionRefused => (REP_REFUSED, false, false),
        _ => (REP_FAILURE, false, false),
    };
    Class {
        rep,
        retry,
        link_down,
    }
}

/// Checks the links that have not served `host` yet, and makes a connection that is not the first of its download to
/// the site wait a moment for the checks. Only names on port 443 are checked: a certificate is what goes wrong there.
fn check(shared: &Arc<Shared>, job: &str, host: &Host, domain: &str, port: u16) -> bool {
    let Host::Name(name) = host else {
        return false;
    };
    // A site kept on one link never needs another checked.
    if port != 443 || shared.fixed || !plain_host(name) || ONE_LINK.contains(&domain) {
        return false;
    }
    let now = Instant::now();
    let (due, first) = {
        let mut sched = shared.sched();
        let first = !sched
            .open
            .iter()
            .any(|o| o.busy() && o.flow.job == job && o.flow.domain == domain);
        (sched.start_checks(domain, now), first)
    };
    for link in due {
        let (shared, name, domain) = (Arc::clone(shared), name.clone(), domain.to_owned());
        thread::spawn(move || {
            let good = reaches_securely(&link, &name);
            shared
                .sched()
                .checked(&link.name, &domain, good, Instant::now());
        });
    }
    if !first {
        while Instant::now() < now + CHECK_WAIT && shared.sched().checking(domain, Instant::now()) {
            thread::sleep(Duration::from_millis(50));
        }
    }
    true
}

/// Connects to the site by a link the scheduler picks, trying the next link when one cannot reach it.
fn dial(
    shared: &Arc<Shared>,
    job: &str,
    host: &Host,
    port: u16,
) -> Result<(TcpStream, Arc<Flow>), u8> {
    let domain = host.domain();
    let addrs = host.resolve(port).ok_or(REP_HOST_UNREACHABLE)?;
    let pinned = shared.sched().primary.is_some();
    // shortcut: the first IPv4 address only; another address of the site is tried by the program's own retry.
    let Some(to) = addrs
        .iter()
        .find(|a| a.is_ipv4())
        .copied()
        .filter(|_| pinned)
    else {
        // A site with IPv6 only, or a VPN that carries everything: the system's own route, as without net-merge.
        let to = addrs[0];
        let flow = shared.sched().open_flow(None, job, &domain, Instant::now());
        return match connect(None, to, CONNECT_TIMEOUT) {
            Ok(server) => Ok((server, flow)),
            Err(error) => {
                shared.sched().forget(flow.id);
                Err(classify(&error).rep)
            }
        };
    };
    let checked = check(shared, job, host, &domain, port);
    let mut tried: Vec<String> = Vec::new();
    let mut rep = REP_NET_UNREACHABLE;
    loop {
        let (link, flow) = {
            let mut sched = shared.sched();
            let now = Instant::now();
            let Some(i) = sched.pick(job, &domain, checked, &tried, now) else {
                return Err(rep);
            };
            let link = sched.slots[i].link.clone();
            (link, sched.open_flow(Some(i), job, &domain, now))
        };
        tried.push(link.name.clone());
        match connect(Some(&link), to, CONNECT_TIMEOUT) {
            Ok(server) => return Ok((server, flow)),
            Err(error) => {
                let class = classify(&error);
                let mut sched = shared.sched();
                let now = Instant::now();
                sched.forget(flow.id);
                if class.link_down {
                    sched.cool(&link.name, now);
                }
                if class.retry {
                    sched.strike(&link.name, &domain, now);
                }
                rep = class.rep;
                if !class.retry {
                    return Err(rep);
                }
            }
        }
    }
}

/// Copies both ways until both sides are done, counting what the site sends. `done` runs once, when the site has
/// finished sending or either side has failed, which is when the connection stops counting for its link.
fn relay(client: TcpStream, server: TcpStream, flow: &Flow, done: impl FnOnce()) {
    let (Ok(mut from_client), Ok(mut to_server)) = (client.try_clone(), server.try_clone()) else {
        done();
        return;
    };
    let up = thread::spawn(move || {
        copy(&mut from_client, &mut to_server, None);
        // shortcut: the program's end closes the site's side too; the HTTP clients that use this never half-close.
        let _ = to_server.shutdown(Shutdown::Both);
    });
    let (mut from_server, mut to_client) = (server, client);
    let whole = copy(&mut from_server, &mut to_client, Some(flow));
    done();
    if whole {
        let _ = to_client.shutdown(Shutdown::Write);
        // A program that keeps its side open after the end is given a while.
        let _ = to_client.set_read_timeout(Some(LINGER));
    } else {
        let _ = to_client.shutdown(Shutdown::Both);
        let _ = from_server.shutdown(Shutdown::Both);
    }
    let _ = up.join();
}

/// Copies until the end, counting into `flow`; false when it stopped on an error.
fn copy(from: &mut TcpStream, to: &mut TcpStream, flow: Option<&Flow>) -> bool {
    let mut buffer = vec![0; BUFFER];
    loop {
        match from.read(&mut buffer) {
            Ok(0) => return true,
            Ok(n) => {
                if let Some(flow) = flow {
                    flow.count(n as u64);
                }
                if to.write_all(&buffer[..n]).is_err() {
                    return false;
                }
            }
            Err(e) if e.kind() == ErrorKind::Interrupted => {}
            Err(_) => return false,
        }
    }
}

#[cfg(target_os = "macos")]
mod sys {
    use super::Link;
    use std::{
        ffi::CStr,
        net::Ipv4Addr,
        process::Command,
        sync::{Mutex, PoisonError},
        time::{Duration, Instant},
    };

    pub fn links() -> Vec<Link> {
        let mut list: *mut libc::ifaddrs = std::ptr::null_mut();
        // SAFETY: getifaddrs fills `list`, which is released below.
        if unsafe { libc::getifaddrs(&mut list) } != 0 {
            return Vec::new();
        }
        let mut found: Vec<Link> = Vec::new();
        let mut entry = list;
        while !entry.is_null() {
            // SAFETY: each entry of the list is valid until freeifaddrs.
            let e = unsafe { &*entry };
            entry = e.ifa_next;
            let flags = e.ifa_flags as i32;
            if e.ifa_addr.is_null()
                || flags & libc::IFF_UP == 0
                || flags & libc::IFF_RUNNING == 0
                || flags & libc::IFF_LOOPBACK != 0
                // SAFETY: a non-null address of an entry is a sockaddr.
                || i32::from(unsafe { (*e.ifa_addr).sa_family }) != libc::AF_INET
            {
                continue;
            }
            // SAFETY: the name of an entry is a C string.
            let name = unsafe { CStr::from_ptr(e.ifa_name) }
                .to_string_lossy()
                .into_owned();
            if !hardware(&name) || found.iter().any(|l| l.name == name) {
                continue;
            }
            // SAFETY: an address of the family AF_INET is a sockaddr_in.
            let sin = unsafe { &*e.ifa_addr.cast::<libc::sockaddr_in>() };
            let ip = Ipv4Addr::from(u32::from_be(sin.sin_addr.s_addr));
            // SAFETY: the name is a C string.
            let index = unsafe { libc::if_nametoindex(e.ifa_name) };
            if ip.is_link_local() || ip.is_unspecified() || index == 0 {
                continue;
            }
            found.push(Link {
                label: label(&name),
                name,
                index,
                ip,
            });
        }
        // SAFETY: `list` came from getifaddrs and is released once.
        unsafe { libc::freeifaddrs(list) };
        found
    }

    /// `enN` is Wi-Fi, Ethernet, a phone on USB or a Thunderbolt adapter, and `fethN` an Android phone through TetherKit;
    /// the rest (utun, ipsec, ppp, bridge, awdl, llw) are tunnels, VPNs and Apple's own plumbing.
    fn hardware(name: &str) -> bool {
        ["en", "feth"].iter().any(|prefix| {
            name.strip_prefix(prefix)
                .is_some_and(|n| !n.is_empty() && n.bytes().all(|b| b.is_ascii_digit()))
        })
    }

    /// When networksetup was last asked, and each device it named with its port.
    type Ports = (Option<Instant>, Vec<(String, String)>);

    /// What System Settings calls an interface, from networksetup, which is asked again only for an interface it has not
    /// named, and then at most every ten seconds.
    fn label(name: &str) -> String {
        static PORTS: Mutex<Ports> = Mutex::new((None, Vec::new()));
        let mut ports = PORTS.lock().unwrap_or_else(PoisonError::into_inner);
        let named = |ports: &[(String, String)]| {
            ports
                .iter()
                .find(|(device, _)| device == name)
                .map(|(_, port)| port.clone())
        };
        if let Some(port) = named(&ports.1) {
            return port;
        }
        if ports
            .0
            .is_none_or(|at| at.elapsed() >= Duration::from_secs(10))
        {
            let listed = Command::new("/usr/sbin/networksetup")
                .arg("-listallhardwareports")
                .output()
                .map(|out| super::parse_ports(&String::from_utf8_lossy(&out.stdout)))
                .unwrap_or_default();
            *ports = (Some(Instant::now()), listed);
        }
        named(&ports.1).unwrap_or_else(|| name.to_owned())
    }
}

/// The ports of `networksetup -listallhardwareports`: a `Hardware Port:` line, then the `Device:` it is.
#[cfg(any(target_os = "macos", test))]
fn parse_ports(text: &str) -> Vec<(String, String)> {
    let mut ports = Vec::new();
    let mut port = None;
    for line in text.lines() {
        if let Some(name) = line.strip_prefix("Hardware Port: ") {
            port = Some(name.trim().to_owned());
        } else if let (Some(device), Some(name)) = (line.strip_prefix("Device: "), port.take()) {
            ports.push((device.trim().to_owned(), name));
        }
    }
    ports
}

#[cfg(windows)]
mod sys {
    use super::Link;
    use std::net::Ipv4Addr;
    use windows_sys::Win32::{
        Foundation::{ERROR_BUFFER_OVERFLOW, NO_ERROR},
        NetworkManagement::{
            IpHelper::{
                GAA_FLAG_INCLUDE_GATEWAYS, GAA_FLAG_SKIP_ANYCAST, GAA_FLAG_SKIP_DNS_SERVER,
                GAA_FLAG_SKIP_MULTICAST, GetAdaptersAddresses, IP_ADAPTER_ADDRESSES_LH,
                IP_ADAPTER_UNICAST_ADDRESS_LH,
            },
            Ndis::IfOperStatusUp,
        },
        Networking::WinSock::{AF_INET, SOCKADDR_IN},
    };

    pub fn links() -> Vec<Link> {
        let flags = GAA_FLAG_INCLUDE_GATEWAYS
            | GAA_FLAG_SKIP_ANYCAST
            | GAA_FLAG_SKIP_MULTICAST
            | GAA_FLAG_SKIP_DNS_SERVER;
        let mut size: u32 = 32 * 1024;
        for _ in 0..4 {
            // Words of eight bytes, so that the list is aligned as its structures need.
            let mut buffer = vec![0u64; size as usize / 8 + 1];
            let first = buffer.as_mut_ptr().cast::<IP_ADAPTER_ADDRESSES_LH>();
            // SAFETY: the buffer holds `size` bytes and outlives the reading of the list.
            let status = unsafe {
                GetAdaptersAddresses(
                    u32::from(AF_INET),
                    flags,
                    std::ptr::null(),
                    first,
                    &mut size,
                )
            };
            if status == ERROR_BUFFER_OVERFLOW {
                continue;
            }
            if status != NO_ERROR {
                return Vec::new();
            }
            let mut found = Vec::new();
            let mut at = first.cast_const();
            while !at.is_null() {
                // SAFETY: each adapter of the list, and what it points to, is valid while `buffer` is.
                let adapter = unsafe { &*at };
                at = adapter.Next;
                let name = wide(adapter.FriendlyName);
                if adapter.OperStatus != IfOperStatusUp
                    || !physical(adapter.IfType)
                    || adapter.FirstGatewayAddress.is_null()
                    || is_virtual(&name)
                    || is_virtual(&wide(adapter.Description))
                {
                    continue;
                }
                let Some(ip) = first_ipv4(adapter.FirstUnicastAddress) else {
                    continue;
                };
                // SAFETY: the index is the second word of the union in every version of the structure.
                let index = unsafe { adapter.Anonymous1.Anonymous.IfIndex };
                found.push(Link {
                    label: name.clone(),
                    name,
                    index,
                    ip,
                });
            }
            return found;
        }
        Vec::new()
    }

    /// Ethernet, Wi-Fi and mobile broadband by their IANA types; tunnels, PPP and most VPN adapters are of others.
    fn physical(kind: u32) -> bool {
        matches!(kind, 6 | 62 | 69 | 117 | 71 | 243 | 244)
    }

    /// The virtual adapters that call themselves Ethernet: switches of Hyper-V and WSL, VPN taps, virtual machines.
    fn is_virtual(text: &str) -> bool {
        let text = text.to_lowercase();
        [
            "virtual",
            "vpn",
            "tap-",
            "tunnel",
            "wintun",
            "wireguard",
            "tailscale",
            "zerotier",
            "openvpn",
            "anyconnect",
            "fortinet",
            "vmware",
            "virtualbox",
            "hyper-v",
            "vethernet",
            "wsl",
            "npcap",
            "miniport",
            "loopback",
            "docker",
            "pseudo",
        ]
        .iter()
        .any(|word| text.contains(word))
    }

    fn first_ipv4(mut at: *mut IP_ADAPTER_UNICAST_ADDRESS_LH) -> Option<Ipv4Addr> {
        while !at.is_null() {
            // SAFETY: each address of the list is valid while the list is.
            let unicast = unsafe { &*at };
            at = unicast.Next;
            let address = unicast.Address.lpSockaddr;
            // SAFETY: a non-null address is a SOCKADDR, and one of the family AF_INET a SOCKADDR_IN.
            if address.is_null() || unsafe { (*address).sa_family } != AF_INET {
                continue;
            }
            let ip = Ipv4Addr::from(u32::from_be(unsafe {
                (*address.cast::<SOCKADDR_IN>()).sin_addr.S_un.S_addr
            }));
            if !ip.is_link_local() {
                return Some(ip);
            }
        }
        None
    }

    fn wide(text: *const u16) -> String {
        if text.is_null() {
            return String::new();
        }
        let mut length = 0;
        // SAFETY: the text ends with a zero.
        while unsafe { *text.add(length) } != 0 {
            length += 1;
        }
        // SAFETY: the `length` characters before the zero.
        String::from_utf16_lossy(unsafe { std::slice::from_raw_parts(text, length) })
    }
}

#[cfg(not(any(target_os = "macos", windows)))]
mod sys {
    /// shortcut: Linux pins an interface only with CAP_NET_RAW, so it has no links and the proxy takes the system's
    /// route; add policy routing if the desktop app ever downloads.
    pub fn links() -> Vec<super::Link> {
        Vec::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn link(name: &str) -> Link {
        Link {
            name: name.into(),
            index: 0,
            ip: Ipv4Addr::LOCALHOST,
            label: name.to_uppercase(),
        }
    }

    /// Two links that work, `a` the system's own.
    fn two() -> Sched {
        let mut sched = Sched::new();
        sched.update(vec![link("a"), link("b")], None, Instant::now());
        for slot in &mut sched.slots {
            slot.state = State::Up;
        }
        sched.primary = Some("a".into());
        sched
    }

    fn name(sched: &Sched, i: Option<usize>) -> &str {
        &sched.slots[i.expect("a link is picked")].link.name
    }

    fn open(sched: &mut Sched, job: &str, domain: &str) -> (String, u64) {
        let now = Instant::now();
        let i = sched.pick(job, domain, false, &[], now);
        let picked = name(sched, i).to_owned();
        (picked, sched.open_flow(i, job, domain, now).id)
    }

    #[test]
    fn a_site_is_known_by_its_last_labels_and_an_address_by_itself() {
        for (host, key) in [
            ("rr1---sn-8pxuuxa-i5o6k.googlevideo.com", "googlevideo.com"),
            ("video.xx.fbcdn.net", "fbcdn.net"),
            ("GitHub.com.", "github.com"),
            ("objects.githubusercontent.com", "githubusercontent.com"),
            ("cdn.example.com.vn", "example.com.vn"),
            ("www.bbc.co.uk", "bbc.co.uk"),
            ("a.b.c.example.de", "example.de"),
            ("localhost", "localhost"),
            ("10.0.0.1", "10.0.0.1"),
            ("2001:db8::1", "2001:db8::1"),
        ] {
            assert_eq!(domain_key(host), key, "{host}");
        }
    }

    #[test]
    fn the_names_of_the_ports_are_read_from_networksetup() {
        let text = "\nHardware Port: Ethernet\nDevice: en0\nEthernet Address: 1\n\nHardware Port: Wi-Fi\nDevice: en1\n\nHardware Port: iPhone USB\nDevice: en8\nEthernet Address: N/A\nVLAN Configurations\n";
        assert_eq!(
            parse_ports(text),
            [
                ("en0".to_owned(), "Ethernet".to_owned()),
                ("en1".to_owned(), "Wi-Fi".to_owned()),
                ("en8".to_owned(), "iPhone USB".to_owned()),
            ]
        );
        assert!(
            parse_ports("Device: en0\n").is_empty(),
            "a device needs its port"
        );
    }

    #[test]
    fn a_job_name_keeps_only_what_a_url_allows() {
        assert_eq!(job_name("dl7"), "dl7");
        assert_eq!(job_name("a:b@c/d e"), "abcde");
        assert_eq!(job_name(""), "app");
        assert_eq!(job_name(&"x".repeat(100)).len(), 64);
        let one = token();
        assert_eq!(one.len(), 32);
        assert!(one.bytes().all(|b| b.is_ascii_hexdigit()));
        assert_ne!(one, token(), "each proxy has a secret of its own");
    }

    #[test]
    fn the_first_connection_of_a_download_to_a_site_takes_the_system_link_and_the_next_ones_spread()
    {
        let mut sched = two();
        assert_eq!(open(&mut sched, "j", "x.com").0, "a");
        assert_eq!(
            open(&mut sched, "j", "x.com").0,
            "b",
            "the second goes to the idle link"
        );
        // Another download starts on the system's link, whatever the first one has open.
        assert_eq!(open(&mut sched, "k", "x.com").0, "a");
        // From then on the busy counts decide: a has two, b one.
        assert_eq!(open(&mut sched, "j", "x.com").0, "b");
        let (mut on_a, mut on_b) = (0i32, 0i32);
        for _ in 0..40 {
            match open(&mut sched, "j", "x.com").0.as_str() {
                "a" => on_a += 1,
                _ => on_b += 1,
            }
        }
        assert!((on_a - on_b).abs() <= 1, "equal links share: {on_a} {on_b}");
    }

    #[test]
    fn idle_connections_do_not_count_and_closed_ones_free_their_link() {
        let mut sched = two();
        let (_, first) = open(&mut sched, "j", "x.com");
        let (_, second) = open(&mut sched, "j", "x.com");
        // Both go quiet: a program keeps them for its next request, and they are not busy any more.
        for _ in 0..IDLE_TICKS {
            sched.tick(TICK, Instant::now(), false);
        }
        assert_eq!(
            open(&mut sched, "j", "x.com").0,
            "a",
            "with nothing busy, the next one is a first again"
        );
        sched.forget(first);
        sched.forget(second);
        assert_eq!(sched.open.len(), 1);
    }

    #[test]
    fn a_link_that_fails_a_site_is_kept_away_from_it_and_one_that_works_is_trusted_again() {
        let mut sched = two();
        let now = Instant::now();
        let (_, busy) = open(&mut sched, "j", "x.com");
        let (link, failed) = open(&mut sched, "j", "x.com");
        assert_eq!(link, "b");
        // The connection on b ends at once with nothing received: a refused certificate, or a 403.
        sched.close(failed, now);
        assert!(sched.avoided("b", "x.com", now));
        for _ in 0..5 {
            assert_eq!(
                open(&mut sched, "j", "x.com").0,
                "a",
                "b is kept away from x.com"
            );
        }
        assert_eq!(
            open(&mut sched, "j", "y.com").0,
            "a",
            "a first to another site"
        );
        assert_eq!(
            open(&mut sched, "j", "y.com").0,
            "b",
            "and b still serves other sites"
        );
        // Each failure in a row keeps it away longer, up to ten minutes.
        assert_eq!(backoff(1), AVOID_FIRST);
        assert_eq!(backoff(2), AVOID_FIRST * 2);
        assert_eq!(backoff(9), AVOID_MAX);
        sched.strike("b", "x.com", now);
        let avoid = sched.avoid.iter().find(|a| a.domain == "x.com").unwrap();
        assert_eq!((avoid.strikes, avoid.until), (2, now + AVOID_FIRST * 2));
        // Once it is past, a connection that works there clears it.
        let later = now + AVOID_MAX;
        assert!(!sched.avoided("b", "x.com", later));
        let i = sched.slots.iter().position(|s| s.link.name == "b");
        let flow = sched.open_flow(i, "j", "x.com", later);
        flow.count(SUSPECT_BYTES);
        sched.close(flow.id, later);
        assert!(sched.avoid.iter().all(|a| a.domain != "x.com"));
        // The system's own link is never kept away: it is what the program would have used anyway.
        let flow = sched.open_flow(Some(0), "j", "z.com", now);
        sched.close(flow.id, now);
        assert!(sched.avoid.iter().all(|a| a.link != "a"));
        sched.forget(busy);
    }

    #[test]
    fn a_link_serves_a_site_over_https_only_once_it_was_checked_for_it() {
        let mut sched = two();
        let now = Instant::now();
        let pick = |sched: &mut Sched| {
            let i = sched.pick("j", "x.com", true, &[], now);
            let picked = name(sched, i).to_owned();
            sched.open_flow(i, "j", "x.com", now);
            picked
        };
        assert_eq!(pick(&mut sched), "a");
        assert_eq!(pick(&mut sched), "a", "b is not checked for x.com yet");
        let due = sched.start_checks("x.com", now);
        let names: Vec<&str> = due.iter().map(|l| l.name.as_str()).collect();
        assert_eq!(names, ["b"], "only the other link is checked");
        assert!(sched.start_checks("x.com", now).is_empty(), "and once");
        assert!(sched.checking("x.com", now));
        assert!(
            !sched.checking("x.com", now + CHECK_WAIT),
            "a connection waits only so long"
        );
        assert_eq!(pick(&mut sched), "a", "still being checked");
        sched.checked("b", "x.com", true, now);
        assert!(!sched.checking("x.com", now));
        assert_eq!(pick(&mut sched), "b", "checked, it takes its share");
        // A site b fails is kept away from it, and checked again once that time is up.
        let due = sched.start_checks("y.com", now);
        sched.checked(&due[0].name, "y.com", false, now);
        assert!(sched.avoided("b", "y.com", now));
        assert!(sched.start_checks("y.com", now).is_empty());
        assert_eq!(sched.start_checks("y.com", now + AVOID_FIRST).len(), 1);
        // A connection that fails on a checked link takes the trust back.
        sched.strike("b", "x.com", now);
        assert!(!sched.trusted("b", "x.com"));
    }

    #[test]
    fn a_site_that_signs_its_addresses_for_one_public_address_keeps_a_download_on_one_link() {
        let mut sched = two();
        for job in ["j", "j", "j", "k", "j"] {
            assert_eq!(open(&mut sched, job, "googlevideo.com").0, "a");
        }
        assert_eq!(
            domain_key("rr4---sn-8pxuuxa-i5o6k.googlevideo.com"),
            "googlevideo.com"
        );
    }

    #[test]
    fn only_a_plain_name_goes_into_the_address_a_check_asks_for() {
        assert!(plain_host("rr1---sn-8pxuuxa-i5o6k.googlevideo.com"));
        assert!(plain_host("github.com"));
        for bad in [
            "",
            "-x.com",
            ".x.com",
            "a b.com",
            "evil.com/@x",
            "user@x.com",
            "1.2.3.4",
            "x.com:443",
            "[::1]",
        ] {
            assert!(!plain_host(bad), "{bad}");
        }
    }

    #[test]
    fn a_link_without_internet_gets_nothing_unless_every_link_is_down() {
        let mut sched = two();
        let now = Instant::now();
        sched.slots[1].state = State::Down;
        for _ in 0..5 {
            assert_eq!(open(&mut sched, "j", "x.com").0, "a");
        }
        sched.slots[1].state = State::Up;
        sched.cool("b", now);
        assert_eq!(open(&mut sched, "j", "x.com").0, "a", "b cools down");
        sched.slots[1].cooldown = None;
        sched.slots[0].state = State::Down;
        assert_eq!(
            open(&mut sched, "k", "x.com").0,
            "b",
            "the system's link is down: a first goes elsewhere"
        );
        sched.slots[1].state = State::Down;
        assert!(
            sched.pick("j", "x.com", false, &[], now).is_some(),
            "with every link down one is still used"
        );
        assert!(
            sched
                .pick("j", "x.com", false, &["a".into(), "b".into()], now)
                .is_none(),
            "every link tried"
        );
    }

    #[test]
    fn the_faster_link_gets_more_once_both_are_measured() {
        let mut sched = two();
        let now = Instant::now();
        for _ in 0..8 {
            sched.slots[0].received.fetch_add(1_000_000, Relaxed);
            sched.slots[1].received.fetch_add(10_000_000, Relaxed);
            sched.tick(TICK, now, false);
        }
        assert_eq!(sched.slots[1].weight, 1.0);
        assert!(
            (sched.slots[0].weight - 0.1).abs() < 0.03,
            "{}",
            sched.slots[0].weight
        );
        // Three busy on b still beat none on a: (3 + 1) / 1 against (0 + 1) / 0.1.
        let b = Some(1);
        for _ in 0..3 {
            sched.open_flow(b, "other", "x.com", now);
        }
        open(&mut sched, "j", "x.com");
        assert_eq!(open(&mut sched, "j", "x.com").0, "b");
        // A link with nothing measured keeps the weights equal.
        let mut fresh = two();
        fresh.slots[1].received.fetch_add(10_000_000, Relaxed);
        fresh.tick(TICK, now, false);
        assert_eq!((fresh.slots[0].weight, fresh.slots[1].weight), (1.0, 1.0));
    }

    #[test]
    fn a_new_link_is_checked_one_that_changed_address_again_and_one_that_left_is_dropped() {
        let mut sched = Sched::new();
        let now = Instant::now();
        sched.update(vec![link("a")], Some(Ipv4Addr::LOCALHOST), now);
        assert_eq!(sched.slots[0].state, State::Checking);
        assert_eq!(sched.primary.as_deref(), Some("a"));
        let due = sched.tick(TICK, now, true);
        assert_eq!(due.len(), 1, "a new link is probed at once");
        assert!(
            sched.tick(TICK, now, true).is_empty(),
            "and not twice at the same time"
        );
        sched.probed(&due[0], true);
        assert_eq!(sched.slots[0].state, State::Up);
        let mut moved = link("a");
        moved.ip = Ipv4Addr::new(10, 0, 0, 2);
        sched.update(vec![moved], Some(Ipv4Addr::new(10, 0, 0, 3)), now);
        assert_eq!(sched.slots[0].state, State::Checking);
        assert_eq!(sched.primary, None, "the system's route is none of them");
        assert!(sched.stats().paused);
        sched.update(Vec::new(), None, now);
        assert!(sched.slots.is_empty());
    }

    /// A site on this computer: it reads a length of four bytes and sends that many.
    fn site() -> u16 {
        let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).unwrap();
        let port = listener.local_addr().unwrap().port();
        thread::spawn(move || {
            for stream in listener.incoming().flatten() {
                thread::spawn(move || {
                    let mut stream = stream;
                    let mut length = [0; 4];
                    if stream.read_exact(&mut length).is_ok() {
                        let body: Vec<u8> =
                            (0..u32::from_be_bytes(length)).map(|i| i as u8).collect();
                        let _ = stream.write_all(&body);
                    }
                });
            }
        });
        port
    }

    /// The token and the port of a proxy URL.
    fn parts(url: &str) -> (String, u16) {
        let rest = url.split_once(':').unwrap().1.trim_start_matches("//");
        let (_, rest) = rest.split_once(':').unwrap();
        let (token, rest) = rest.split_once('@').unwrap();
        (
            token.to_owned(),
            rest.rsplit(':').next().unwrap().parse().unwrap(),
        )
    }

    /// A SOCKS5 client: the greeting with `methods`, the user and password, and a CONNECT to `target`. Gives the stream
    /// and the reply that ended the exchange.
    fn socks(
        port: u16,
        methods: &[u8],
        user: &str,
        password: &str,
        target: &[u8],
    ) -> (TcpStream, Vec<u8>) {
        let mut stream = TcpStream::connect((Ipv4Addr::LOCALHOST, port)).unwrap();
        stream
            .set_read_timeout(Some(Duration::from_secs(10)))
            .unwrap();
        let mut hello = vec![5, methods.len() as u8];
        hello.extend_from_slice(methods);
        stream.write_all(&hello).unwrap();
        let mut answer = [0; 2];
        stream.read_exact(&mut answer).unwrap();
        if answer != [5, 2] {
            return (stream, answer.to_vec());
        }
        let mut auth = vec![1, user.len() as u8];
        auth.extend_from_slice(user.as_bytes());
        auth.push(password.len() as u8);
        auth.extend_from_slice(password.as_bytes());
        stream.write_all(&auth).unwrap();
        stream.read_exact(&mut answer).unwrap();
        if answer != [1, 0] {
            return (stream, answer.to_vec());
        }
        let mut request = vec![5, 1, 0];
        request.extend_from_slice(target);
        stream.write_all(&request).unwrap();
        let mut reply = [0; 10];
        stream.read_exact(&mut reply).unwrap();
        (stream, reply.to_vec())
    }

    fn ipv4_target(port: u16) -> Vec<u8> {
        let mut target = vec![1, 127, 0, 0, 1];
        target.extend_from_slice(&port.to_be_bytes());
        target
    }

    fn fetch(stream: &mut TcpStream, n: u32) -> Vec<u8> {
        stream.write_all(&n.to_be_bytes()).unwrap();
        let mut body = Vec::new();
        stream.read_to_end(&mut body).unwrap();
        body
    }

    fn wait_until(what: &str, done: impl Fn() -> bool) {
        let until = Instant::now() + Duration::from_secs(10);
        while !done() {
            assert!(Instant::now() < until, "waited too long: {what}");
            thread::sleep(Duration::from_millis(20));
        }
    }

    #[test]
    fn downloads_through_the_proxy_are_spread_over_the_links_and_counted_on_them() {
        let site = site();
        let merge = Merge::with_links(vec![link("a"), link("b")], Some("a")).unwrap();
        let url = merge.proxy_url("socks5", "job:1");
        assert!(url.starts_with("socks5://job1:"), "{url}");
        let (token, port) = parts(&url);
        let (mut first, reply) = socks(port, &[0, 2], "job1", &token, &ipv4_target(site));
        assert_eq!(reply[..2], [5, REP_OK]);
        let (mut second, reply) = socks(port, &[2], "job1", &token, &ipv4_target(site));
        assert_eq!(reply[..2], [5, REP_OK]);
        let on = |name: &str| {
            merge
                .stats()
                .links
                .iter()
                .find(|l| l.name == name)
                .map_or(0, |l| l.connections)
        };
        assert_eq!((on("a"), on("b")), (1, 1), "one connection on each link");
        assert_eq!(merge.links_used("job1"), 2);
        assert_eq!(merge.links_used("job2"), 0);
        let expected: Vec<u8> = (0..300_000u32).map(|i| i as u8).collect();
        assert_eq!(fetch(&mut first, 300_000), expected);
        assert_eq!(fetch(&mut second, 1_000)[..], expected[..1_000]);
        wait_until("both connections are closed", || {
            merge.stats().links.iter().all(|l| l.connections == 0)
        });
        let stats = merge.stats();
        assert!(!stats.paused);
        let received: Vec<u64> = stats.links.iter().map(|l| l.received).collect();
        assert_eq!(
            received,
            [300_000, 1_000],
            "each byte counted on its own link"
        );

        // A name is resolved by the proxy.
        let mut target = vec![3, 9];
        target.extend_from_slice(b"localhost");
        target.extend_from_slice(&site.to_be_bytes());
        let (mut named, reply) = socks(port, &[2], "job2", &token, &target);
        assert_eq!(reply[..2], [5, REP_OK]);
        assert_eq!(fetch(&mut named, 10)[..], expected[..10]);
    }

    #[test]
    fn the_proxy_refuses_other_programs_other_commands_and_closed_ports() {
        let merge = Merge::with_links(vec![link("a")], Some("a")).unwrap();
        let (token, port) = parts(&merge.proxy_url("socks5", "j"));
        let (_, reply) = socks(port, &[0], "j", &token, &[]);
        assert_eq!(reply, [5, 0xFF], "no password, no proxy");
        let (_, reply) = socks(port, &[2], "j", "guess", &[]);
        assert_eq!(reply, [1, 1], "a wrong password is refused");
        // A port nobody listens on: refused, and no other link is tried for it.
        let closed = TcpListener::bind((Ipv4Addr::LOCALHOST, 0))
            .unwrap()
            .local_addr()
            .unwrap()
            .port();
        let (_, reply) = socks(port, &[2], "j", &token, &ipv4_target(closed));
        assert_eq!(reply[..2], [5, REP_REFUSED]);
        // BIND is not offered.
        let mut stream = TcpStream::connect((Ipv4Addr::LOCALHOST, port)).unwrap();
        stream.write_all(&[5, 1, 2]).unwrap();
        let mut answer = [0; 2];
        stream.read_exact(&mut answer).unwrap();
        let mut auth = vec![1, 1, b'j', token.len() as u8];
        auth.extend_from_slice(token.as_bytes());
        stream.write_all(&auth).unwrap();
        stream.read_exact(&mut answer).unwrap();
        let mut bind = vec![5, 2, 0];
        bind.extend_from_slice(&ipv4_target(closed));
        stream.write_all(&bind).unwrap();
        let mut reply = [0; 10];
        stream.read_exact(&mut reply).unwrap();
        assert_eq!(reply[..2], [5, REP_COMMAND]);
        assert!(merge.stats().links.iter().all(|l| l.connections == 0));
    }

    #[test]
    fn behind_a_vpn_the_proxy_takes_the_system_route_and_merges_nothing() {
        let site = site();
        let merge = Merge::with_links(vec![link("a"), link("b")], None).unwrap();
        assert!(merge.stats().paused);
        let (token, port) = parts(&merge.proxy_url("socks5h", "j"));
        let (mut stream, reply) = socks(port, &[2], "j", &token, &ipv4_target(site));
        assert_eq!(reply[..2], [5, REP_OK]);
        assert_eq!(fetch(&mut stream, 100).len(), 100);
        assert!(merge.stats().links.iter().all(|l| l.received == 0));
    }

    #[test]
    fn a_stopped_proxy_takes_no_more_connections() {
        let merge = Merge::with_links(vec![link("a")], Some("a")).unwrap();
        let (_, port) = parts(&merge.proxy_url("socks5", "j"));
        drop(merge);
        wait_until("the proxy is gone", || {
            TcpStream::connect((Ipv4Addr::LOCALHOST, port))
                .and_then(|mut s| {
                    s.set_read_timeout(Some(Duration::from_millis(500)))?;
                    s.write_all(&[5, 1, 2])?;
                    let mut answer = [0; 2];
                    s.read_exact(&mut answer)
                })
                .is_err()
        });
    }

    /// The links of this computer, as the proxy sees them, with a real probe of each: `cargo test -p core-netmerge --
    /// --ignored --nocapture links_of_this_computer`.
    #[test]
    #[ignore = "reads the network of this computer"]
    fn links_of_this_computer() {
        let found = links();
        println!("primary: {:?}", primary_ip());
        for link in &found {
            let up = PROBE_TARGETS
                .iter()
                .any(|to| connect(Some(link), SocketAddr::V4(*to), PROBE_TIMEOUT).is_ok());
            println!("{link:?} reaches the internet: {up}");
            for site in ["github.com", "dl.google.com"] {
                println!(
                    "  {site} with a certificate curl accepts: {}",
                    reaches_securely(link, site)
                );
            }
        }
        assert!(!found.is_empty());
    }
}
