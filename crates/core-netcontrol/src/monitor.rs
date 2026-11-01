//! Every app that uses the network, whether or not it goes through the proxy: what `nettop` says of each socket,
//! a second apart, summed by app and by interface. `nettop` needs no administrator and takes a few milliseconds.

use crate::owner::{self, App};
use std::{
    collections::HashMap,
    process::{Command, Stdio},
    sync::{
        Arc, Mutex, PoisonError,
        atomic::{AtomicBool, AtomicU64, Ordering::Relaxed},
    },
    thread,
    time::{Duration, Instant},
};

/// One socket in what `nettop` printed.
#[derive(Debug, Clone, PartialEq)]
pub struct Socket {
    /// What `nettop` names it by: protocol and the two ends.
    pub key: String,
    pub interface: String,
    pub bytes_in: u64,
    pub bytes_out: u64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Process {
    pub pid: i32,
    /// The name `nettop` gives, cut short by it; the app is found from the pid.
    pub name: String,
    pub sockets: Vec<Socket>,
}

/// Reads `nettop -L 1 -x -n -J interface,bytes_in,bytes_out`: a line for each process (`name.pid,,in,out`), then a
/// line for each of its sockets (`tcp4 a:1<->b:2,en0,in,out`). The columns are found by the header.
pub fn parse(text: &str) -> Vec<Process> {
    let mut lines = text.lines();
    let Some(header) = lines.next() else {
        return Vec::new();
    };
    let columns: Vec<&str> = header.split(',').collect();
    let at = |name: &str| columns.iter().position(|c| *c == name);
    let (Some(iface), Some(bin), Some(bout)) = (at("interface"), at("bytes_in"), at("bytes_out"))
    else {
        return Vec::new();
    };
    let mut out: Vec<Process> = Vec::new();
    for line in lines {
        // The name comes first and may hold a comma of its own: the other fields are counted from the end.
        let fields: Vec<&str> = line.split(',').collect();
        if fields.len() < columns.len() {
            continue;
        }
        let extra = fields.len() - columns.len();
        let first = fields[..=extra].join(",");
        let field = |i: usize| fields[i + extra];
        let number = |i: usize| field(i).trim().parse::<u64>().unwrap_or(0);
        if first.contains("<->") {
            if let Some(process) = out.last_mut() {
                process.sockets.push(Socket {
                    key: first,
                    interface: field(iface).to_owned(),
                    bytes_in: number(bin),
                    bytes_out: number(bout),
                });
            }
        } else if let Some((name, pid)) = first.rsplit_once('.')
            && let Ok(pid) = pid.parse::<i32>()
        {
            out.push(Process {
                pid,
                name: name.trim().to_owned(),
                sockets: Vec::new(),
            });
        }
    }
    out
}

/// What an app moved, over the last second and since watching began.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Usage {
    pub name: String,
    pub bundle: String,
    /// Bytes a second.
    pub down: u64,
    pub up: u64,
    /// Bytes since watching began.
    pub received: u64,
    pub sent: u64,
    /// Bytes a second received by interface, the busiest first.
    pub interfaces: Vec<(String, u64)>,
    pub sockets: usize,
    /// The addresses its sockets talked to since watching began, the busiest first: (address, bytes in, bytes out).
    pub hosts: Vec<(String, u64, u64)>,
    /// What moved in this one step, by interface: (interface, bytes in, bytes out). What `History` adds up.
    pub delta: Vec<(String, u64, u64)>,
}

/// Turns samples into speeds: what each socket moved since the last sample. A socket new since then counts whole,
/// since it was opened in between; on the first sample nothing is known yet.
#[derive(Default)]
pub struct Meter {
    last: HashMap<(i32, String), (u64, u64)>,
    started: bool,
    apps: HashMap<i32, App>,
    totals: HashMap<String, (u64, u64)>,
    /// What each app moved to each address, since watching began.
    remotes: HashMap<String, HashMap<String, (u64, u64)>>,
}

/// The most addresses kept for an app, and listed.
const MAX_REMOTES: usize = 60;
pub const LISTED_REMOTES: usize = 20;

/// The address at the far end of a socket, as `nettop` names it (`tcp4 10.0.0.2:5000<->1.1.1.1:443`, or with a dot before
/// the port for IPv6), without its port; `None` for a socket that talks to no one in particular (`*:*`).
pub fn remote_of(key: &str) -> Option<String> {
    let far = key.split_once("<->")?.1.trim();
    let address = if far.matches(':').count() == 1 {
        far.split_once(':')?.0
    } else {
        far.rsplit_once('.')?.0
    };
    (!address.is_empty() && address != "*" && !address.contains('*')).then(|| address.to_owned())
}

/// The interface of this computer's own connections to itself: the proxy's, among others. Never counted.
const LOOPBACK: &str = "lo0";

impl Meter {
    /// The usage of each app, the busiest first. `app_of` names the app of a pid (`owner::app_of`, or a fixed one in
    /// tests), asked once per pid.
    pub fn step(
        &mut self,
        sample: &[Process],
        secs: f64,
        app_of: impl Fn(i32, &str) -> App,
    ) -> Vec<Usage> {
        let secs = secs.max(0.001);
        let mut next = HashMap::new();
        let mut by_app: HashMap<String, Usage> = HashMap::new();
        for process in sample {
            let app = self
                .apps
                .entry(process.pid)
                .or_insert_with(|| app_of(process.pid, &process.name))
                .clone();
            let usage = by_app.entry(app.name.clone()).or_insert_with(|| Usage {
                name: app.name.clone(),
                bundle: app.bundle.clone(),
                ..Usage::default()
            });
            for socket in &process.sockets {
                if socket.interface == LOOPBACK {
                    continue;
                }
                let id = (process.pid, socket.key.clone());
                let (was_in, was_out) = match self.last.get(&id) {
                    Some(&before) => before,
                    None if self.started => (0, 0),
                    None => (socket.bytes_in, socket.bytes_out),
                };
                next.insert(id, (socket.bytes_in, socket.bytes_out));
                let got = socket.bytes_in.saturating_sub(was_in);
                let gave = socket.bytes_out.saturating_sub(was_out);
                usage.received += got;
                usage.sent += gave;
                usage.sockets += 1;
                if (got > 0 || gave > 0)
                    && let Some(remote) = remote_of(&socket.key)
                {
                    let sites = self.remotes.entry(app.name.clone()).or_default();
                    if !sites.contains_key(&remote) && sites.len() >= MAX_REMOTES {
                        let quietest = sites.iter().min_by_key(|(_, (a, b))| a + b).map(|(n, _)| n.clone());
                        if let Some(quietest) = quietest {
                            sites.remove(&quietest);
                        }
                    }
                    let pair = sites.entry(remote).or_default();
                    pair.0 += got;
                    pair.1 += gave;
                }
                if got > 0 || gave > 0 {
                    let name = if socket.interface.is_empty() { "?" } else { socket.interface.as_str() };
                    match usage.delta.iter_mut().find(|(i, _, _)| i == name) {
                        Some((_, i, o)) => (*i, *o) = (*i + got, *o + gave),
                        None => usage.delta.push((name.to_owned(), got, gave)),
                    }
                }
                if got > 0 {
                    let iface = if socket.interface.is_empty() {
                        "?".to_owned()
                    } else {
                        socket.interface.clone()
                    };
                    match usage.interfaces.iter_mut().find(|(i, _)| *i == iface) {
                        Some((_, n)) => *n += got,
                        None => usage.interfaces.push((iface, got)),
                    }
                }
            }
        }
        self.last = next;
        self.started = true;
        let alive: Vec<i32> = sample.iter().map(|p| p.pid).collect();
        self.apps.retain(|pid, _| alive.contains(pid));
        let mut out: Vec<Usage> = by_app
            .into_values()
            .map(|mut u| {
                let total = self.totals.entry(u.name.clone()).or_default();
                total.0 += u.received;
                total.1 += u.sent;
                u.down = (u.received as f64 / secs) as u64;
                u.up = (u.sent as f64 / secs) as u64;
                for (_, n) in &mut u.interfaces {
                    *n = (*n as f64 / secs) as u64;
                }
                u.interfaces.sort_by_key(|i| std::cmp::Reverse(i.1));
                (u.received, u.sent) = *total;
                let mut hosts: Vec<(String, u64, u64)> = self
                    .remotes
                    .get(&u.name)
                    .into_iter()
                    .flatten()
                    .map(|(n, (a, b))| (n.clone(), *a, *b))
                    .collect();
                hosts.sort_by(|a, b| (b.1 + b.2).cmp(&(a.1 + a.2)).then(a.0.cmp(&b.0)));
                hosts.truncate(LISTED_REMOTES);
                u.hosts = hosts;
                u
            })
            .filter(|u| u.sockets > 0 || u.received > 0)
            .collect();
        out.sort_by(|a, b| {
            b.down
                .cmp(&a.down)
                .then(b.received.cmp(&a.received))
                .then(a.name.cmp(&b.name))
        });
        out
    }
}

/// What `nettop` printed once, or nothing when it could not run.
fn sample() -> Option<String> {
    let output = Command::new("/usr/bin/nettop")
        .args(["-L", "1", "-x", "-n", "-J", "interface,bytes_in,bytes_out"])
        .stdin(Stdio::null())
        .stderr(Stdio::null())
        .output()
        .ok()?;
    output
        .status
        .success()
        .then(|| String::from_utf8_lossy(&output.stdout).into_owned())
}

/// Watches while someone looks: the thread starts with the first `usage` and stops once nobody asked for a while.
pub struct Monitor {
    shared: Arc<Watch>,
}

struct Watch {
    usage: Mutex<Vec<Usage>>,
    asked: Mutex<Instant>,
    running: AtomicBool,
    /// Seconds between steps while only the history watches; 0 when it does not.
    background: AtomicU64,
    meter: Mutex<Meter>,
    sink: Mutex<Option<Sink>>,
}

/// What is handed every step that is measured.
type Sink = Box<dyn Fn(&[Usage]) + Send + Sync>;

/// How long the thread keeps going after the last `usage`.
const KEEP_FOR: Duration = Duration::from_secs(6);
const EVERY: Duration = Duration::from_secs(1);

impl Default for Monitor {
    fn default() -> Self {
        Self::new()
    }
}

impl Monitor {
    pub fn new() -> Self {
        Self {
            shared: Arc::new(Watch {
                usage: Mutex::new(Vec::new()),
                asked: Mutex::new(Instant::now()),
                running: AtomicBool::new(false),
                background: AtomicU64::new(0),
                meter: Mutex::new(Meter::default()),
                sink: Mutex::new(None),
            }),
        }
    }

    /// The last usage measured, and the watching goes on for a few seconds more.
    pub fn usage(&self) -> Vec<Usage> {
        *self
            .shared
            .asked
            .lock()
            .unwrap_or_else(PoisonError::into_inner) = Instant::now();
        self.ensure_running();
        self.shared
            .usage
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone()
    }

    /// Hands every step to `sink` as it is measured, for as long as the watching goes on (`History`).
    pub fn on_step(&self, sink: impl Fn(&[Usage]) + Send + Sync + 'static) {
        *self.shared.sink.lock().unwrap_or_else(PoisonError::into_inner) = Some(Box::new(sink));
    }

    /// Watches every `seconds` even when nobody asks for the usage, for the history; 0 stops that.
    pub fn background(&self, seconds: u64) {
        self.shared.background.store(seconds, Relaxed);
        if seconds > 0 {
            self.ensure_running();
        }
    }

    fn ensure_running(&self) {
        if !self.shared.running.swap(true, Relaxed) {
            let shared = Arc::clone(&self.shared);
            thread::spawn(move || watch(&shared));
        }
    }
}

fn watch(shared: &Watch) {
    let mut last = Instant::now();
    loop {
        let asked = *shared.asked.lock().unwrap_or_else(PoisonError::into_inner);
        let background = shared.background.load(Relaxed);
        let watched = asked.elapsed() <= KEEP_FOR;
        if !watched && background == 0 {
            break;
        }
        if let Some(text) = sample() {
            let now = Instant::now();
            let processes = parse(&text);
            let usage = shared
                .meter
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .step(
                    &processes,
                    now.duration_since(last).as_secs_f64(),
                    |pid, name| {
                        owner::app_of(pid).unwrap_or(App {
                            name: name.to_owned(),
                            bundle: String::new(),
                        })
                    },
                );
            last = now;
            if let Some(sink) = shared.sink.lock().unwrap_or_else(PoisonError::into_inner).as_ref() {
                sink(&usage);
            }
            *shared.usage.lock().unwrap_or_else(PoisonError::into_inner) = usage;
        }
        thread::sleep(if watched { EVERY } else { Duration::from_secs(background) });
    }
    shared.running.store(false, Relaxed);
}

#[cfg(test)]
mod tests {
    use super::*;

    const ONE: &str = ",interface,bytes_in,bytes_out,
syslogd.390,,0,118785,
udp4 *:56076<->*:*,lo0,0,118785,
Microsoft Edge .96287,,5000,700,
tcp4 10.0.0.2:5000<->1.1.1.1:443,en0,4000,500,
tcp4 10.0.0.3:5001<->1.1.1.1:443,en7,1000,200,
Microsoft Edge Helper (R.96300,,0,0,
weird, name.42,,10,10,
tcp4 10.0.0.2:6000<->8.8.8.8:443,en0,10,10,
";

    #[test]
    fn processes_and_their_sockets_are_read() {
        let p = parse(ONE);
        assert_eq!(p.len(), 4);
        assert_eq!((p[1].pid, p[1].name.as_str()), (96287, "Microsoft Edge"));
        assert_eq!(p[1].sockets.len(), 2);
        assert_eq!(p[1].sockets[1].interface, "en7");
        assert_eq!(p[3].name, "weird, name", "a comma in the name");
        assert_eq!(p[3].sockets[0].bytes_in, 10);
        assert!(parse("").is_empty() && parse("no,header\nx.1,,1,1").is_empty());
    }

    fn edge(pid: i32, name: &str) -> App {
        let name = if pid == 96287 || pid == 96300 {
            "Microsoft Edge"
        } else {
            name
        };
        App {
            name: name.into(),
            bundle: String::new(),
        }
    }

    #[test]
    fn speeds_come_from_what_each_socket_moved_since_the_last_look() {
        let mut meter = Meter::default();
        let first = meter.step(&parse(ONE), 1.0, edge);
        assert!(
            first.iter().all(|u| u.down == 0 && u.received == 0),
            "the first look only sets where counting starts"
        );
        let two = ONE.replace("en0,4000,500", "en0,14000,1500").replace(
            "Microsoft Edge Helper (R.96300,,0,0,",
            "Microsoft Edge Helper (R.96300,,3000,0,\ntcp4 10.0.0.2:7000<->9.9.9.9:443,en7,3000,0,",
        );
        let usage = meter.step(&parse(&two), 2.0, edge);
        let e = usage.iter().find(|u| u.name == "Microsoft Edge").unwrap();
        assert_eq!(
            e.received, 13_000,
            "10 000 more on one socket and a new socket of a helper with 3000"
        );
        assert_eq!(e.down, 6500, "over two seconds");
        assert_eq!(
            e.interfaces,
            vec![("en0".to_owned(), 5000), ("en7".to_owned(), 1500)]
        );
        assert_eq!(usage[0].name, "Microsoft Edge", "the busiest first");
        let s = usage.iter().find(|u| u.name == "syslogd");
        assert!(
            s.is_none_or(|s| s.sent == 0),
            "what goes over lo0 is never counted"
        );
    }

    #[test]
    fn the_far_end_of_a_socket_is_its_address_without_the_port() {
        assert_eq!(remote_of("tcp4 10.0.0.2:5000<->1.1.1.1:443").as_deref(), Some("1.1.1.1"));
        assert_eq!(remote_of("tcp6 2001:db8::2.5000<->2606:4700::1111.443").as_deref(), Some("2606:4700::1111"));
        assert_eq!(remote_of("udp4 *:56076<->*:*"), None);
        assert_eq!(remote_of("udp4 10.0.0.2:5353<->*:*"), None, "no one in particular");
        assert_eq!(remote_of("no arrow"), None);
    }

    #[test]
    fn the_addresses_an_app_talked_to_are_added_up_busiest_first() {
        let mut meter = Meter::default();
        meter.step(&parse(ONE), 1.0, edge);
        let two = ONE
            .replace("en0,4000,500", "en0,14000,1500")
            .replace("en7,1000,200", "en7,1100,200")
            .replace("8.8.8.8:443,en0,10,10", "8.8.8.8:443,en0,50,10");
        let usage = meter.step(&parse(&two), 1.0, edge);
        let e = usage.iter().find(|u| u.name == "Microsoft Edge").unwrap();
        assert_eq!(e.hosts[0], ("1.1.1.1".to_owned(), 10_100, 1000), "both sockets of Edge to 1.1.1.1");
        assert!(e.hosts.len() == 1);
    }

    #[test]
    fn a_socket_that_closed_takes_nothing_away() {
        let mut meter = Meter::default();
        meter.step(&parse(ONE), 1.0, edge);
        let closed = ONE.replace("tcp4 10.0.0.2:5000<->1.1.1.1:443,en0,4000,500,\n", "");
        let usage = meter.step(&parse(&closed), 1.0, edge);
        let e = usage.iter().find(|u| u.name == "Microsoft Edge").unwrap();
        assert_eq!((e.down, e.received), (0, 0));
    }
}
