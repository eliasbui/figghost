//! A cleaner for the Mac in the settings window, after the features of Mole (github.com/tw93/Mole) but written
//! here, with nothing of Mole run or bundled: the health of the Mac, caches and logs, old build folders of
//! projects, installer files, apps with their leftovers and what takes the disk. The page scans as it opens, and
//! what is chosen in a list is removed together.

mod docker;
mod scan;
mod strings;

use super::{AttachEnv, system::logic};
use crate::settings::model::Fault;
use macos_sys::{
    sensors,
    status_item::{Entry, Item},
    sysinfo::{self, Pressure, Thermal},
    workspace,
};
use scan::{Found, Kind};
use serde_json::{Value, json};
use std::{
    collections::{BTreeMap, BTreeSet},
    path::{Path, PathBuf},
    rc::Rc,
    sync::{
        Arc, Mutex, MutexGuard,
        atomic::{AtomicU64, Ordering},
    },
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

/// How many of the busiest processes are shown, by processor and by memory.
const TOP: usize = 6;

/// The time between the two readings of the processor that its use is worked out from.
const CPU_SAMPLE: Duration = Duration::from_millis(400);

#[derive(Default)]
struct Inner {
    busy: BTreeSet<&'static str>,
    status: Value,
    lists: BTreeMap<Kind, Vec<Found>>,
    /// The folder the disk view shows, or is reading when "analyze" is busy.
    folder: Option<PathBuf>,
    /// Every folder the disk view has read, so that going back to one is at once.
    seen: BTreeMap<PathBuf, Vec<Found>>,
    /// What Docker keeps, as the page shows it; null until read.
    docker: Value,
    /// What the last removal did; `seq` tells the page it is new.
    result: Value,
    seq: u64,
}

pub struct Feature {
    home: PathBuf,
    own_id: Option<String>,
    inner: Arc<Mutex<Inner>>,
    /// Raised for every folder the disk view is asked for; a walk of an older one stops.
    walk: Arc<AtomicU64>,
}

fn lock(inner: &Mutex<Inner>) -> MutexGuard<'_, Inner> {
    inner.lock().unwrap_or_else(|e| e.into_inner())
}

pub fn attach(_env: &AttachEnv) -> Option<Rc<Feature>> {
    Some(Rc::new(Feature {
        home: PathBuf::from(std::env::var_os("HOME")?),
        own_id: workspace::own_bundle_id(),
        inner: Arc::default(),
        walk: Arc::default(),
    }))
}

impl Feature {
    pub fn runtime(&self) -> Value {
        let inner = lock(&self.inner);
        let list = |kind: Kind| {
            inner
                .lists
                .get(&kind)
                .map_or(Value::Null, |l| l.iter().map(Found::json).collect())
        };
        // While a folder is read its path is already shown, and nothing of the folder before.
        let reading = inner.busy.contains("analyze");
        let analyze = inner.folder.as_ref().map_or(Value::Null, |path| {
            match inner.lists.get(&Kind::Analyze) {
                Some(l) if !reading => json!({
                    "path": path,
                    "total": l.iter().map(|f| f.size).sum::<u64>(),
                    "entries": list(Kind::Analyze),
                }),
                _ => json!({ "path": path, "total": null, "entries": null }),
            }
        });
        json!({
            "home": self.home,
            "busy": inner.busy,
            "status": inner.status,
            "clean": list(Kind::Clean),
            "purge": list(Kind::Purge),
            "installers": list(Kind::Installers),
            "apps": list(Kind::Apps),
            "analyze": analyze,
            "docker": inner.docker,
            "result": inner.result,
        })
    }

    /// The menu bar item opens the page, which scans by itself.
    pub fn menu_entries(_this: &Rc<Self>) -> Vec<Entry> {
        let title = crate::i18n::current().mole_menu();
        vec![Entry::Item(
            Item::new(title, || crate::settings::show_page("mole")).symbol("wand.and.stars"),
        )]
    }

    /// A button of the page; `None` when the name is not this feature's. What comes after the colon is checked
    /// here, since the page is not trusted: a folder must be one, and only rows the app listed can be removed.
    pub fn action(&self, name: &str) -> Option<Result<(), Fault>> {
        let (verb, arg) = name.split_once(':').unwrap_or((name, ""));
        match verb {
            // Opening the page reads the health again and scans only what was never scanned.
            "mole_open" | "mole_refresh" => {
                let all = verb == "mole_refresh";
                self.status();
                let missing: Vec<Kind> = {
                    let inner = lock(&self.inner);
                    Kind::ALL
                        .into_iter()
                        .filter(|k| all || !inner.lists.contains_key(k))
                        .collect()
                };
                for kind in missing {
                    self.scan(kind);
                }
                if all || lock(&self.inner).docker.is_null() {
                    self.docker_scan();
                }
            }
            "mole_status" => self.status(),
            "mole_scan" if arg == "docker" => self.docker_scan(),
            "mole_scan" => match Kind::parse(arg) {
                Some(kind) => self.scan(kind),
                None => return Some(Err(Fault::new("invalid_value", "unknown list"))),
            },
            "mole_analyze" => {
                let path = Path::new(arg);
                if !path.is_absolute() || !path.is_dir() {
                    return Some(Err(Fault::new("invalid_value", "not a folder")));
                }
                self.analyze(path.to_path_buf(), false);
            }
            "mole_remove" => return Some(self.remove(arg)),
            "mole_docker" => return Some(self.docker_act(arg)),
            "mole_docker_prune" => {
                if !matches!(arg, "unused" | "cache") {
                    return Some(Err(Fault::new("invalid_value", "unknown prune")));
                }
                let (home, which) = (self.home.clone(), arg.to_owned());
                self.spawn(
                    "docker_act",
                    move || {
                        let tool = docker::Tool::locate(&home);
                        let ok = tool.as_ref().is_some_and(|t| docker::prune(t, &which));
                        (
                            (usize::from(ok), usize::from(!ok)),
                            docker::scan(tool.as_ref(), &home),
                            which,
                        )
                    },
                    Self::store_docker,
                );
            }
            _ => return None,
        }
        Some(Ok(()))
    }

    /// Runs `work` on its own thread and `store`s what it gives back; a job already running is not started twice.
    fn spawn<R: Send + 'static>(
        &self,
        key: &'static str,
        work: impl FnOnce() -> R + Send + 'static,
        store: impl FnOnce(&mut Inner, R) + Send + 'static,
    ) {
        if !lock(&self.inner).busy.insert(key) {
            return;
        }
        let shared = Arc::clone(&self.inner);
        std::thread::spawn(move || {
            let out = work();
            let mut inner = lock(&shared);
            inner.busy.remove(key);
            store(&mut inner, out);
        });
    }

    fn status(&self) {
        self.spawn("status", health, |inner, v| inner.status = v);
    }

    /// Lists one kind again; the disk view reads its folder again.
    fn scan(&self, kind: Kind) {
        if kind == Kind::Analyze {
            let folder = lock(&self.inner).folder.clone();
            self.analyze(folder.unwrap_or_else(|| self.home.clone()), true);
            return;
        }
        let home = self.home.clone();
        let own_id = self.own_id.clone();
        self.spawn(
            kind.label(),
            move || scan::list(kind, &home, own_id.as_deref()),
            move |inner, list| {
                inner.lists.insert(kind, list);
            },
        );
    }

    /// Shows a folder in the disk view: one read before comes back at once unless `fresh`, and a new one is
    /// walked while any older walk is stopped, so a click is never lost behind a slow scan of the home folder.
    fn analyze(&self, folder: PathBuf, fresh: bool) {
        let mut inner = lock(&self.inner);
        // Raised under the lock, so a walk that checks under the lock knows for sure whether it is the newest.
        let walk = self.walk.fetch_add(1, Ordering::SeqCst) + 1;
        inner.folder = Some(folder.clone());
        if !fresh && let Some(list) = inner.seen.get(&folder).cloned() {
            inner.busy.remove("analyze");
            inner.lists.insert(Kind::Analyze, list);
            return;
        }
        inner.busy.insert("analyze");
        drop(inner);
        let shared = Arc::clone(&self.inner);
        let newest = Arc::clone(&self.walk);
        std::thread::spawn(move || {
            let stale = || newest.load(Ordering::SeqCst) != walk;
            let list = scan::analyze(&folder, &stale);
            let mut inner = lock(&shared);
            if stale() {
                return;
            }
            inner.busy.remove("analyze");
            inner.seen.insert(folder, list.clone());
            inner.lists.insert(Kind::Analyze, list);
        });
    }

    fn docker_scan(&self) {
        let home = self.home.clone();
        self.spawn(
            "docker",
            move || docker::scan(docker::Tool::locate(&home).as_ref(), &home),
            |inner, v| inner.docker = v,
        );
    }

    /// `arg` is a verb of `docker::VERBS` and then ids, one per line; only ids of the list that verb works on, as
    /// the last reading listed them, are passed to Docker.
    fn docker_act(&self, arg: &str) -> Result<(), Fault> {
        let mut lines = arg.lines();
        let verb = lines.next().unwrap_or_default();
        let list = docker::VERBS
            .iter()
            .find(|(v, _)| *v == verb)
            .map(|(_, list)| *list)
            .ok_or_else(|| Fault::new("invalid_value", "unknown docker action"))?;
        let wanted: BTreeSet<&str> = lines.collect();
        let ids: Vec<String> = {
            let inner = lock(&self.inner);
            inner.docker[list]
                .as_array()
                .into_iter()
                .flatten()
                .filter_map(|row| row["path"].as_str())
                .filter(|id| wanted.contains(id))
                .map(str::to_owned)
                .collect()
        };
        if ids.is_empty() {
            return Err(Fault::new("invalid_value", "nothing to do"));
        }
        let (home, verb) = (self.home.clone(), verb.to_owned());
        self.spawn(
            "docker_act",
            move || {
                let tool = docker::Tool::locate(&home);
                let done = tool
                    .as_ref()
                    .map_or((0, ids.len()), |t| docker::act(t, &verb, &ids));
                (done, docker::scan(tool.as_ref(), &home), verb)
            },
            Self::store_docker,
        );
        Ok(())
    }

    fn store_docker(
        inner: &mut Inner,
        ((done, failed), state, verb): ((usize, usize), Value, String),
    ) {
        inner.docker = state;
        inner.seq += 1;
        inner.result =
            json!({ "seq": inner.seq, "docker": verb, "removed": done, "failed": failed });
    }

    /// `arg` is the kind of list and then the paths of its rows, one per line.
    fn remove(&self, arg: &str) -> Result<(), Fault> {
        let mut lines = arg.lines();
        let kind = lines
            .next()
            .and_then(Kind::parse)
            .ok_or_else(|| Fault::new("invalid_value", "unknown list"))?;
        let wanted: BTreeSet<&Path> = lines.map(Path::new).collect();
        let chosen: Vec<Found> = {
            let inner = lock(&self.inner);
            inner
                .lists
                .get(&kind)
                .into_iter()
                .flatten()
                .filter(|f| wanted.contains(f.path.as_path()))
                // The disk view can show any folder of the Mac; what it sends to the Trash is the user's own.
                .filter(|f| kind != Kind::Analyze || f.path.starts_with(&self.home))
                .cloned()
                .collect()
        };
        if chosen.is_empty() {
            return Err(Fault::new("invalid_value", "nothing to remove"));
        }
        self.spawn(
            "remove",
            move || {
                let (gone, failed): (Vec<Found>, Vec<Found>) = chosen
                    .into_iter()
                    .partition(|f| scan::remove(kind, f).is_ok());
                (gone, failed.len(), health())
            },
            move |inner, (gone, failed, status)| {
                let paths: BTreeSet<&PathBuf> = gone.iter().map(|f| &f.path).collect();
                if let Some(list) = inner.lists.get_mut(&kind) {
                    list.retain(|f| !paths.contains(&f.path));
                }
                // What is left of every folder read before is now smaller.
                inner.seen.clear();
                inner.seq += 1;
                inner.result = json!({
                    "seq": inner.seq, "kind": kind.label(), "removed": gone.len(), "failed": failed,
                    "freed": gone.iter().map(|f| f.size).sum::<u64>(),
                });
                inner.status = status;
            },
        );
        Ok(())
    }
}

/// The state under the key "mole". `web/features/mole.fixture.json` must have exactly the same keys.
pub mod settings {
    use serde_json::{Value, json};

    pub fn state(runtime: &Value) -> Value {
        let object = |key: &str| {
            if runtime[key].is_object() {
                runtime[key].clone()
            } else {
                Value::Null
            }
        };
        let array = |key: &str| {
            if runtime[key].is_array() {
                runtime[key].clone()
            } else {
                Value::Null
            }
        };
        json!({
            "home": runtime["home"].as_str().unwrap_or(""),
            "busy": runtime["busy"].as_array().cloned().unwrap_or_default(),
            "status": object("status"),
            "clean": array("clean"),
            "purge": array("purge"),
            "installers": array("installers"),
            "apps": array("apps"),
            "analyze": object("analyze"),
            "docker": object("docker"),
            "result": object("result"),
        })
    }
}

/// The share of the processors in use between two readings of their ticks: user, system, idle, nice.
fn cpu_percent(a: &[[u32; 4]], b: &[[u32; 4]]) -> f64 {
    let (mut busy, mut all) = (0u64, 0u64);
    for (x, y) in a.iter().zip(b) {
        let d: Vec<u64> = (0..4).map(|i| u64::from(y[i].wrapping_sub(x[i]))).collect();
        busy += d[0] + d[1] + d[3];
        all += d.iter().sum::<u64>();
    }
    if all == 0 {
        0.0
    } else {
        busy as f64 * 100.0 / all as f64
    }
}

/// Out of 100: what is taken off for a busy processor, full memory, a full disk and heavy swapping.
fn score(cpu: f64, memory: f64, disk: f64, swap: f64) -> u32 {
    let mut score = 100.0;
    for (value, warn, bad, weight) in [
        (cpu, 70.0, 90.0, 15.0),
        (memory, 80.0, 92.0, 20.0),
        (disk, 80.0, 92.0, 30.0),
        (swap, 50.0, 80.0, 10.0),
    ] {
        if value >= bad {
            score -= weight;
        } else if value >= warn {
            score -= weight / 2.0;
        }
    }
    score as u32
}

/// The busiest processes between two readings taken `seconds` apart, by processor (in percent of one core, as
/// Activity Monitor counts it) and by memory. A process that is in only one reading has no processor figure.
fn busiest(
    before: &[sysinfo::Process],
    after: &[sysinfo::Process],
    seconds: f64,
) -> (Vec<Value>, Vec<Value>) {
    let then: BTreeMap<i32, u64> = before.iter().map(|p| (p.pid, p.cpu_ns)).collect();
    let mut rows: Vec<(f64, &sysinfo::Process)> = after
        .iter()
        .map(|p| {
            let used = then.get(&p.pid).map_or(0, |t| p.cpu_ns.saturating_sub(*t));
            (used as f64 / 1e9 / seconds.max(1e-3) * 100.0, p)
        })
        .collect();
    let row = |(cpu, p): &(f64, &sysinfo::Process)| json!({ "pid": p.pid, "name": p.name, "cpu": cpu, "memory": p.memory });
    rows.sort_by(|a, b| b.0.total_cmp(&a.0));
    let by_cpu = rows
        .iter()
        .take(TOP)
        .filter(|r| r.0 > 0.0)
        .map(row)
        .collect();
    rows.sort_by_key(|r| std::cmp::Reverse(r.1.memory));
    let by_memory = rows.iter().take(TOP).map(row).collect();
    (by_cpu, by_memory)
}

/// The numbers the page shows; it takes a moment to read the processor and the network twice.
fn health() -> Value {
    let (first, net, procs) = (
        sysinfo::cpu_ticks(),
        sysinfo::interfaces(),
        sysinfo::processes(),
    );
    let start = Instant::now();
    std::thread::sleep(CPU_SAMPLE);
    let (second, net_after, procs_after) = (
        sysinfo::cpu_ticks(),
        sysinfo::interfaces(),
        sysinfo::processes(),
    );
    let seconds = start.elapsed().as_secs_f64();
    let (down, up) = logic::rates(&net, &net_after, seconds);
    let (by_cpu, by_memory) = busiest(&procs, &procs_after, seconds);
    let (cpu, cores) = match (first, second) {
        (Some(a), Some(b)) => (cpu_percent(&a, &b), logic::core_loads(&a, &b)),
        _ => (0.0, Vec::new()),
    };
    let percent = |used: u64, total: u64| {
        if total == 0 {
            0.0
        } else {
            used as f64 * 100.0 / total as f64
        }
    };
    let mem = sysinfo::memory();
    let (mem_used, mem_total) = mem.map_or((0, 0), |m| (m.used(), m.total));
    let (swap_total, swap_used) = sysinfo::swap().unwrap_or((0, 0));
    let (free, disk_total) = sysinfo::disk().unwrap_or((0, 0));
    let disk_used = disk_total.saturating_sub(free);
    let (memory, disk) = (percent(mem_used, mem_total), percent(disk_used, disk_total));
    json!({
        "score": score(cpu, memory, disk, percent(swap_used, swap_total)),
        "cpu": cpu,
        "memory": memory, "memory_used": mem_used, "memory_total": mem_total,
        "swap_used": swap_used, "swap_total": swap_total,
        "disk": disk, "disk_used": disk_used, "disk_total": disk_total,
        "at": SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |d| d.as_millis() as u64),
        "cores": cores.iter().map(|l| l * 100.0).collect::<Vec<_>>(),
        // The slow cores come first in the list, then the fast ones.
        "kinds": sysinfo::core_kinds().map(|(slow, fast)| [slow, fast]),
        "memory_app": mem.map(|m| m.app), "memory_wired": mem.map(|m| m.wired),
        "memory_compressed": mem.map(|m| m.compressed),
        "pressure": match sysinfo::memory_pressure() {
            Pressure::Normal => "normal",
            Pressure::Warning => "warning",
            Pressure::Critical => "critical",
        },
        "net_down": down, "net_up": up,
        "gpu": sysinfo::gpu_percent(),
        "celsius": sensors::processor_celsius(),
        "thermal": match sysinfo::thermal() {
            Thermal::Nominal => "nominal",
            Thermal::Fair => "fair",
            Thermal::Serious => "serious",
            Thermal::Critical => "critical",
        },
        "fans": sensors::fans().iter().map(|f| json!({ "rpm": f.rpm, "max": f.max })).collect::<Vec<_>>(),
        "top_cpu": by_cpu, "top_memory": by_memory,
        "battery": sysinfo::battery_wear().map(|w| json!({ "health": w.health, "cycles": w.cycles })),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_processor_share_counts_every_state_but_idle() {
        let a = [[100, 50, 800, 50]];
        let b = [[200, 100, 1000, 100]];
        assert!((cpu_percent(&a, &b) - 50.0).abs() < 1e-9);
        assert_eq!(cpu_percent(&a, &a), 0.0);
        let wrapped = [[u32::MAX, 0, 0, 0]];
        let after = [[9, 0, 10, 0]];
        assert!((cpu_percent(&wrapped, &after) - 50.0).abs() < 1e-9);
    }

    #[test]
    fn a_full_disk_costs_more_than_a_busy_processor() {
        assert_eq!(score(10.0, 50.0, 50.0, 0.0), 100);
        assert_eq!(score(95.0, 50.0, 50.0, 0.0), 85);
        assert_eq!(score(10.0, 50.0, 95.0, 0.0), 70);
    }

    #[test]
    fn the_busiest_processes_are_counted_over_the_time_between_readings() {
        let p = |pid, cpu_ns, memory| sysinfo::Process {
            pid,
            name: format!("p{pid}"),
            cpu_ns,
            memory,
        };
        let before = [p(1, 0, 10), p(2, 1_000_000_000, 500)];
        let after = [
            p(1, 500_000_000, 10),
            p(2, 1_000_000_000, 500),
            p(3, 9_000_000_000, 1),
        ];
        let (cpu, memory) = busiest(&before, &after, 1.0);
        assert_eq!(
            cpu.len(),
            1,
            "idle and new processes have no processor figure: {cpu:?}"
        );
        assert_eq!(cpu[0]["pid"], 1);
        assert!((cpu[0]["cpu"].as_f64().unwrap() - 50.0).abs() < 1e-9);
        assert_eq!(memory[0]["pid"], 2);
        assert_eq!(memory.len(), 3);
    }

    #[test]
    fn only_listed_rows_can_be_removed() {
        let feature = Feature {
            home: PathBuf::from("/nonexistent-home"),
            own_id: None,
            inner: Arc::default(),
            walk: Arc::default(),
        };
        assert!(
            feature
                .action("mole_remove:clean\n/etc/hosts")
                .unwrap()
                .is_err()
        );
        assert!(
            feature
                .action("mole_remove:nope\n/etc/hosts")
                .unwrap()
                .is_err()
        );
        assert!(
            feature
                .action("mole_analyze:relative/path")
                .unwrap()
                .is_err()
        );
        assert!(feature.action("mole_other").is_none());
        lock(&feature.inner).docker =
            json!({ "ok": true, "containers": [{ "path": "abc" }], "images": [] });
        for bad in [
            "mole_docker:nope\nabc",
            "mole_docker:rmi\nabc",
            "mole_docker:rm\n--help",
            "mole_docker_prune:all",
        ] {
            assert!(feature.action(bad).unwrap().is_err(), "{bad}");
        }
    }

    #[test]
    fn the_newest_folder_asked_for_wins_and_one_read_before_comes_back_at_once() {
        let root = std::env::temp_dir().join(format!("figghost-mole-walk-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        for (dir, file) in [("a", "in-a"), ("b", "in-b")] {
            std::fs::create_dir_all(root.join(dir)).unwrap();
            std::fs::write(root.join(dir).join(file), "x").unwrap();
        }
        let feature = Feature {
            home: root.clone(),
            own_id: None,
            inner: Arc::default(),
            walk: Arc::default(),
        };
        let names = |f: &Feature| {
            let inner = lock(&f.inner);
            inner.lists[&Kind::Analyze]
                .iter()
                .map(|x| x.name.clone())
                .collect::<Vec<_>>()
        };
        let settle = |f: &Feature| {
            for _ in 0..500 {
                if !lock(&f.inner).busy.contains("analyze") {
                    return;
                }
                std::thread::sleep(Duration::from_millis(10));
            }
            panic!("the walk never ended");
        };
        feature.analyze(root.join("a"), false);
        feature.analyze(root.join("b"), false);
        settle(&feature);
        assert_eq!(lock(&feature.inner).folder, Some(root.join("b")));
        assert_eq!(names(&feature), ["in-b"]);
        feature.analyze(root.join("a"), false);
        settle(&feature);
        feature.analyze(root.join("b"), false);
        assert!(
            !lock(&feature.inner).busy.contains("analyze"),
            "a folder read before is not read again"
        );
        assert_eq!(names(&feature), ["in-b"]);
        std::fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn the_state_has_the_keys_of_the_fixture() {
        let fixture: Value =
            serde_json::from_str(include_str!("../../../web/features/mole.fixture.json")).unwrap();
        let state = settings::state(&Value::Null);
        let keys = |v: &Value| v.as_object().unwrap().keys().cloned().collect::<Vec<_>>();
        assert_eq!(keys(&state), keys(&fixture));
    }
}
