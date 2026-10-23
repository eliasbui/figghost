//! What Figghost itself is using, for the About page: processor, GPU, memory, disk, network and the frames its
//! animations draw. Read only while the settings window is open, each time the page is refreshed, as rates since the
//! reading before.
//!
//! The network is counted with `nettop` for this process and the tools it is running at that moment (curl, gh,
//! brew): the app itself opens no connection of its own. A tool that starts and finishes between two readings is
//! not seen.

use macos_sys::sysinfo::{self, OwnUsage};
use serde_json::{Value, json};
use std::{
    collections::{HashMap, HashSet},
    sync::{
        Arc, Mutex, PoisonError,
        atomic::{AtomicBool, Ordering},
    },
    time::Instant,
};

const NETTOP: &str = "/usr/bin/nettop";
const NETTOP_SECONDS: u64 = 5;

struct Reading {
    at: Instant,
    own: OwnUsage,
    frames: u64,
    net: u64,
}

/// Bytes seen for each process, the most each has had; their sum only grows while the processes come and go.
#[derive(Default)]
struct Network {
    by_pid: HashMap<i32, u64>,
}

impl Network {
    fn total(&self) -> u64 {
        self.by_pid.values().sum()
    }
}

#[derive(Default)]
pub struct Sampler {
    last: Option<Reading>,
    net: Arc<Mutex<Network>>,
    asking: Arc<AtomicBool>,
}

impl Sampler {
    /// The rates since the last call; on the first, only the memory, the rest waiting for a second reading.
    pub fn sample(&mut self) -> Value {
        self.ask_network();
        let Some(own) = sysinfo::own_usage() else {
            return Value::Null;
        };
        let now = Reading {
            at: Instant::now(),
            own,
            frames: sysinfo::frames(),
            net: self
                .net
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .total(),
        };
        let value = match &self.last {
            Some(before) => rates(before, &now),
            None => json!({ "memory": own.memory }),
        };
        self.last = Some(now);
        value
    }

    /// Asks `nettop` off the main thread; the answer is there for the next reading.
    fn ask_network(&self) {
        if self.asking.swap(true, Ordering::AcqRel) {
            return;
        }
        let (net, asking) = (Arc::clone(&self.net), Arc::clone(&self.asking));
        std::thread::spawn(move || {
            let mut ours: HashSet<i32> = sysinfo::own_children().into_iter().collect();
            ours.insert(std::process::id() as i32);
            if let Ok(text) = crate::features::run_bounded(
                NETTOP,
                &["-P", "-L", "1", "-n", "-x", "-J", "bytes_in,bytes_out"],
                NETTOP_SECONDS,
            ) {
                let mut net = net.lock().unwrap_or_else(PoisonError::into_inner);
                for (pid, bytes) in parse_nettop(&text) {
                    if ours.contains(&pid) {
                        let seen = net.by_pid.entry(pid).or_default();
                        *seen = (*seen).max(bytes);
                    }
                }
            }
            asking.store(false, Ordering::Release);
        });
    }
}

fn rates(before: &Reading, now: &Reading) -> Value {
    let seconds = now.at.duration_since(before.at).as_secs_f64().max(0.001);
    let per_second = |a: u64, b: u64| b.saturating_sub(a) as f64 / seconds;
    let percent = |a: u64, b: u64| per_second(a, b) / 1e9 * 100.0;
    json!({
        "cpu": percent(before.own.cpu_ns, now.own.cpu_ns),
        "gpu": percent(before.own.gpu_ns, now.own.gpu_ns),
        "memory": now.own.memory,
        "disk_read": per_second(before.own.disk_read, now.own.disk_read),
        "disk_write": per_second(before.own.disk_written, now.own.disk_written),
        "net": per_second(before.net, now.net),
        "fps": per_second(before.frames, now.frames),
    })
}

/// `name.pid,bytes_in,bytes_out,` on each line of what `nettop -P -x -J bytes_in,bytes_out` prints.
fn parse_nettop(text: &str) -> Vec<(i32, u64)> {
    text.lines()
        .filter_map(|line| {
            let mut fields = line.split(',');
            let (_, pid) = fields.next()?.rsplit_once('.')?;
            let pid = pid.parse().ok()?;
            let bytes_in: u64 = fields.next()?.parse().ok()?;
            let bytes_out: u64 = fields.next()?.parse().ok()?;
            Some((pid, bytes_in + bytes_out))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn nettop_lines_are_read_by_process() {
        let text =
            ",bytes_in,bytes_out,\nsyslogd.395,0,40648,\ncom.apple.Web.Content.12,5,7,\nnonsense\n";
        assert_eq!(parse_nettop(text), [(395, 40648), (12, 12)]);
    }

    #[test]
    fn rates_are_per_second_and_the_processor_per_core() {
        let at = Instant::now();
        let reading = |at: Instant, cpu_ns: u64, frames: u64, net: u64| Reading {
            at,
            own: OwnUsage {
                cpu_ns,
                gpu_ns: cpu_ns / 2,
                memory: 50 << 20,
                disk_read: cpu_ns,
                disk_written: 0,
            },
            frames,
            net,
        };
        let r = rates(
            &reading(at, 0, 0, 1000),
            &reading(at + Duration::from_secs(2), 1_000_000_000, 120, 3000),
        );
        assert!((r["cpu"].as_f64().unwrap() - 50.0).abs() < 0.01);
        assert!((r["gpu"].as_f64().unwrap() - 25.0).abs() < 0.01);
        assert!((r["fps"].as_f64().unwrap() - 60.0).abs() < 0.01);
        assert!((r["net"].as_f64().unwrap() - 1000.0).abs() < 0.01);
        assert_eq!(r["memory"], 50 << 20);
    }

    #[test]
    fn a_second_reading_gives_rates() {
        let mut sampler = Sampler::default();
        assert!(sampler.sample()["memory"].as_u64().unwrap() > 0);
        std::thread::sleep(Duration::from_millis(50));
        let r = sampler.sample();
        assert!(r["cpu"].is_number() && r["fps"].is_number(), "{r}");
    }
}
