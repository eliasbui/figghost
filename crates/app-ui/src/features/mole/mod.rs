//! A cleaner for the Mac in the settings window, after the features of Mole (github.com/tw93/Mole) but written
//! here, with nothing of Mole run or bundled: the health of the Mac, caches and logs, old build folders of
//! projects, installer files, apps with their leftovers and what takes the disk. The page scans as it opens, and
//! what is chosen in a list is removed together.

mod scan;
mod strings;

use super::AttachEnv;
use crate::settings::model::Fault;
use macos_sys::{
    status_item::{Entry, Item},
    sysinfo, workspace,
};
use scan::{Found, Kind};
use serde_json::{Value, json};
use std::{
    collections::{BTreeMap, BTreeSet},
    path::{Path, PathBuf},
    rc::Rc,
    sync::{Arc, Mutex, MutexGuard},
    time::Duration,
};

/// The time between the two readings of the processor that its use is worked out from.
const CPU_SAMPLE: Duration = Duration::from_millis(400);

#[derive(Default)]
struct Inner {
    busy: BTreeSet<&'static str>,
    status: Value,
    lists: BTreeMap<Kind, Vec<Found>>,
    /// The folder the disk view shows.
    folder: Option<PathBuf>,
    /// What the last removal did; `seq` tells the page it is new.
    result: Value,
    seq: u64,
}

pub struct Feature {
    home: PathBuf,
    own_id: Option<String>,
    inner: Arc<Mutex<Inner>>,
}

fn lock(inner: &Mutex<Inner>) -> MutexGuard<'_, Inner> {
    inner.lock().unwrap_or_else(|e| e.into_inner())
}

pub fn attach(_env: &AttachEnv) -> Option<Rc<Feature>> {
    Some(Rc::new(Feature {
        home: PathBuf::from(std::env::var_os("HOME")?),
        own_id: workspace::own_bundle_id(),
        inner: Arc::default(),
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
        let analyze = inner.lists.get(&Kind::Analyze).map_or(Value::Null, |l| {
            json!({
                "path": inner.folder,
                "total": l.iter().map(|f| f.size).sum::<u64>(),
                "entries": list(Kind::Analyze),
            })
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
            "result": inner.result,
        })
    }

    /// The menu bar item opens the page, which scans by itself.
    pub fn menu_entries(_this: &Rc<Self>) -> Vec<Entry> {
        let title = crate::i18n::current().mole_menu();
        vec![Entry::Item(
            Item::new(title, || crate::settings::show_page("mole")).symbol("sparkles"),
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
                    self.scan(kind, None);
                }
            }
            "mole_status" => self.status(),
            "mole_scan" => match Kind::parse(arg) {
                Some(kind) => self.scan(kind, None),
                None => return Some(Err(Fault::new("invalid_value", "unknown list"))),
            },
            "mole_analyze" => {
                let path = Path::new(arg);
                if !path.is_absolute() || !path.is_dir() {
                    return Some(Err(Fault::new("invalid_value", "not a folder")));
                }
                self.scan(Kind::Analyze, Some(path.to_path_buf()));
            }
            "mole_remove" => return Some(self.remove(arg)),
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

    /// Lists one kind again; the disk view keeps its folder unless given another.
    fn scan(&self, kind: Kind, folder: Option<PathBuf>) {
        let home = self.home.clone();
        let own_id = self.own_id.clone();
        let folder = folder
            .or_else(|| lock(&self.inner).folder.clone())
            .unwrap_or_else(|| home.clone());
        self.spawn(
            kind.label(),
            move || {
                let list = if kind == Kind::Analyze {
                    scan::analyze(&folder)
                } else {
                    scan::list(kind, &home, own_id.as_deref())
                };
                (list, folder)
            },
            move |inner, (list, folder)| {
                if kind == Kind::Analyze {
                    inner.folder = Some(folder);
                }
                inner.lists.insert(kind, list);
            },
        );
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

/// The numbers the page shows; it takes a moment to read the processor twice.
fn health() -> Value {
    let first = sysinfo::cpu_ticks();
    std::thread::sleep(CPU_SAMPLE);
    let cpu = match (first, sysinfo::cpu_ticks()) {
        (Some(a), Some(b)) => cpu_percent(&a, &b),
        _ => 0.0,
    };
    let percent = |used: u64, total: u64| {
        if total == 0 {
            0.0
        } else {
            used as f64 * 100.0 / total as f64
        }
    };
    let (mem_used, mem_total) = sysinfo::memory().map_or((0, 0), |m| (m.used(), m.total));
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
    fn only_listed_rows_can_be_removed() {
        let feature = Feature {
            home: PathBuf::from("/nonexistent-home"),
            own_id: None,
            inner: Arc::default(),
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
