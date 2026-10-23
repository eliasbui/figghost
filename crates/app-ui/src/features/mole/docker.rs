//! Docker, through its own command: what it keeps (containers with their logs, images, volumes, networks and the
//! build cache) and the commands that start, stop, clean and remove them. Nothing runs when Docker is not installed.
//! The logs live in Docker's virtual machine, not on the Mac, so they are read and emptied by a throwaway busybox
//! container that mounts the folder of the containers.

use crate::features::{Ran, run_bounded_with};
use serde_json::{Value, json};
use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
};

const SCAN_SECONDS: u64 = 90;
const ACT_SECONDS: u64 = 120;
const PRUNE_SECONDS: u64 = 900;
/// Where Docker Desktop, OrbStack, Homebrew and Rancher Desktop put the command.
const TOOL_DIRS: [&str; 5] = [
    "/usr/local/bin",
    "/opt/homebrew/bin",
    "/Applications/Docker.app/Contents/Resources/bin",
    ".docker/bin",
    ".orbstack/bin",
];
/// The disk of Docker Desktop's virtual machine on the Mac, a sparse file.
const DESKTOP_DISK: &str = "Library/Containers/com.docker.docker/Data/vms/0/data/Docker.raw";
const LOGS_DIR: &str = "/var/lib/docker/containers:/c";
/// The networks Docker makes itself and never lets go of.
const BUILT_IN: [&str; 3] = ["bridge", "host", "none"];

/// What can be done to the rows of one list, and which list each verb takes its rows from.
pub const VERBS: [(&str, &str); 8] = [
    ("start", "containers"),
    ("stop", "containers"),
    ("restart", "containers"),
    ("logs", "containers"),
    ("rm", "containers"),
    ("rmi", "images"),
    ("volume_rm", "volumes"),
    ("network_rm", "networks"),
];

pub struct Tool {
    path: PathBuf,
    path_env: String,
}

impl Tool {
    /// The docker command, if installed; its own folder goes first in the search path so that its helpers (the
    /// credential helper, the buildx plugin) are found by an app started from the Finder.
    pub fn locate(home: &Path) -> Option<Tool> {
        let dirs: Vec<PathBuf> = TOOL_DIRS.iter().map(|d| home.join(d)).collect();
        let path = crate::features::download::logic::find_tool("docker", &dirs)?;
        let dir = path.parent()?.display();
        Some(Tool {
            path_env: format!("{dir}:/usr/local/bin:/opt/homebrew/bin:/usr/bin:/bin"),
            path,
        })
    }

    fn run(&self, args: &[&str], seconds: u64) -> Result<String, Ran> {
        let program = self.path.to_str().ok_or(Ran::Failed)?;
        run_bounded_with(program, args, &[("PATH", &self.path_env)], seconds)
    }
}

/// A size as Docker prints it, in powers of 1000: `0B`, `77.8kB`, `857MB`, `1.24GB`.
pub fn parse_size(text: &str) -> u64 {
    let text = text.trim();
    let split = text
        .find(|c: char| !(c.is_ascii_digit() || c == '.'))
        .unwrap_or(text.len());
    let (number, unit) = text.split_at(split);
    let scale = match unit.trim().to_ascii_lowercase().as_str() {
        "b" | "" => 1.0,
        "kb" => 1e3,
        "mb" => 1e6,
        "gb" => 1e9,
        "tb" => 1e12,
        _ => return 0,
    };
    number.parse::<f64>().map_or(0, |n| (n * scale) as u64)
}

/// A container id as Docker writes it in full; only such ids are put into the path of a log.
fn is_full_id(id: &str) -> bool {
    id.len() == 64 && id.bytes().all(|b| b.is_ascii_hexdigit())
}

/// The bytes of logs of each container, from the lines of `du -sk /c/<id>/...`.
fn log_sizes(du: &str) -> BTreeMap<String, u64> {
    let mut out = BTreeMap::new();
    for line in du.lines() {
        let Some((kb, path)) = line.split_once(char::is_whitespace) else {
            continue;
        };
        let Some(id) = path
            .trim()
            .strip_prefix("/c/")
            .and_then(|p| p.split('/').next())
        else {
            continue;
        };
        *out.entry(id.to_owned()).or_default() += kb.trim().parse::<u64>().unwrap_or(0) * 1024;
    }
    out
}

fn text(v: &Value, key: &str) -> String {
    v[key].as_str().unwrap_or_default().to_owned()
}

/// Everything shown on the page. `{"missing": true}` without the command, `{"down": true}` when it does not answer.
pub fn scan(tool: Option<&Tool>, home: &Path) -> Value {
    let Some(tool) = tool else {
        return json!({ "missing": true });
    };
    let Ok(verbose) = tool.run(&["system", "df", "-v", "--format", "json"], SCAN_SECONDS) else {
        return json!({ "down": true });
    };
    let df: Value = serde_json::from_str(&verbose).unwrap_or_default();
    let totals: Vec<Value> = tool
        .run(&["system", "df", "--format", "json"], SCAN_SECONDS)
        .unwrap_or_default()
        .lines()
        .filter_map(|l| serde_json::from_str::<Value>(l).ok())
        .map(|r| {
            let reclaim = text(&r, "Reclaimable");
            json!({
                "type": text(&r, "Type"), "count": text(&r, "TotalCount").parse::<u64>().unwrap_or(0),
                "size": parse_size(&text(&r, "Size")),
                "reclaimable": parse_size(reclaim.split(' ').next().unwrap_or_default()),
            })
        })
        .collect();
    let list = |key: &str| df[key].as_array().cloned().unwrap_or_default();
    // shortcut: an empty log size when busybox cannot be pulled (offline, first time); the rest still shows.
    let logs = log_sizes(
        &tool
            .run(
                &[
                    "run",
                    "--rm",
                    "-v",
                    &format!("{LOGS_DIR}:ro"),
                    "busybox",
                    "sh",
                    "-c",
                    "du -sk /c/*/local-logs /c/*/*-json.log* 2>/dev/null",
                ],
                SCAN_SECONDS,
            )
            .unwrap_or_default(),
    );
    let containers: Vec<Value> = list("Containers")
        .iter()
        .map(|c| {
            let id = text(c, "ID");
            let log = logs.get(&id).copied().unwrap_or(0);
            json!({
                "path": id, "name": text(c, "Names"), "note": text(c, "Image"), "status": text(c, "Status"),
                "state": text(c, "State"), "logs": log, "size": parse_size(&text(c, "Size")) + log, "dir": false,
            })
        })
        .collect();
    let images: Vec<Value> = list("Images")
        .iter()
        .map(|i| {
            let (repo, tag) = (text(i, "Repository"), text(i, "Tag"));
            let id = text(i, "ID");
            let name = if repo == "<none>" {
                id.trim_start_matches("sha256:").chars().take(12).collect()
            } else {
                format!("{repo}:{tag}")
            };
            json!({
                "path": id, "name": name, "note": text(i, "CreatedSince"),
                "used": text(i, "Containers").parse::<u64>().unwrap_or(0),
                // What removing it frees: the layers no other image shares.
                "size": parse_size(&text(i, "UniqueSize")), "dir": false,
            })
        })
        .collect();
    let volumes: Vec<Value> = list("Volumes")
        .iter()
        .map(|v| {
            let name = text(v, "Name");
            let anonymous = text(v, "Labels").contains("com.docker.volume.anonymous");
            json!({
                "path": name, "name": if anonymous { name.chars().take(12).collect() } else { name.clone() },
                "anonymous": anonymous, "used": text(v, "Links").parse::<u64>().unwrap_or(0),
                "size": parse_size(&text(v, "Size")), "note": "", "dir": false,
            })
        })
        .collect();
    let networks: Vec<Value> = tool
        .run(&["network", "ls", "--format", "json"], SCAN_SECONDS)
        .unwrap_or_default()
        .lines()
        .filter_map(|l| serde_json::from_str::<Value>(l).ok())
        .filter(|n| !BUILT_IN.contains(&n["Name"].as_str().unwrap_or_default()))
        .map(|n| json!({ "path": text(&n, "ID"), "name": text(&n, "Name"), "note": text(&n, "Driver"), "size": 0, "dir": false }))
        .collect();
    let disk = home.join(DESKTOP_DISK);
    json!({
        "ok": true,
        "totals": totals,
        "logs": logs.values().sum::<u64>(),
        "disk": disk.is_file().then(|| super::scan::size_of(&disk)),
        "containers": containers, "images": images, "volumes": volumes, "networks": networks,
    })
}

/// Does `verb` to each of `ids` side by side; how many worked and how many did not.
pub fn act(tool: &Tool, verb: &str, ids: &[String]) -> (usize, usize) {
    if verb == "logs" {
        let ids: Vec<&str> = ids
            .iter()
            .map(String::as_str)
            .filter(|i| is_full_id(i))
            .collect();
        // The ids are positional arguments of the script, never a part of it.
        let script = r#"for id; do d=/c/$id; [ -d "$d" ] || continue
for f in "$d/local-logs/container.log" "$d/$id-json.log"; do [ -f "$f" ] && : > "$f"; done
rm -f "$d"/local-logs/container.log.* "$d/$id-json.log".*; done"#;
        let mut args = vec![
            "run", "--rm", "-v", LOGS_DIR, "busybox", "sh", "-c", script, "sh",
        ];
        args.extend(&ids);
        return match tool.run(&args, ACT_SECONDS) {
            Ok(_) => (ids.len(), 0),
            Err(_) => (0, ids.len()),
        };
    }
    let head: &[&str] = match verb {
        "start" => &["start"],
        "stop" => &["stop"],
        "restart" => &["restart"],
        "rm" => &["rm", "-f"],
        "rmi" => &["rmi"],
        "volume_rm" => &["volume", "rm"],
        "network_rm" => &["network", "rm"],
        _ => return (0, ids.len()),
    };
    let ok = std::thread::scope(|s| {
        let jobs: Vec<_> = ids
            .iter()
            .map(|id| {
                s.spawn(move || {
                    // After `--` an id is never read as an option.
                    let mut args = head.to_vec();
                    args.extend(["--", id.as_str()]);
                    tool.run(&args, ACT_SECONDS).is_ok()
                })
            })
            .collect();
        jobs.into_iter()
            .filter_map(|j| j.join().ok())
            .filter(|ok| *ok)
            .count()
    });
    (ok, ids.len() - ok)
}

/// `unused`: stopped containers, unused networks, dangling images and dangling build cache, never volumes.
/// `cache`: the whole build cache that nothing uses.
pub fn prune(tool: &Tool, which: &str) -> bool {
    let args: &[&str] = match which {
        "unused" => &["system", "prune", "-f"],
        "cache" => &["builder", "prune", "-af"],
        _ => return false,
    };
    tool.run(args, PRUNE_SECONDS).is_ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sizes_are_read_the_way_docker_prints_them() {
        assert_eq!(parse_size("0B"), 0);
        assert_eq!(parse_size("77.8kB"), 77_800);
        assert_eq!(parse_size("857MB"), 857_000_000);
        assert_eq!(parse_size("1.24GB"), 1_240_000_000);
        assert_eq!(parse_size("N/A"), 0);
    }

    #[test]
    fn logs_are_added_up_by_container_and_only_full_ids_reach_a_path() {
        let id = "a".repeat(64);
        let du = format!(
            "8\t/c/{id}/local-logs\n4\t/c/{id}/{id}-json.log\nnonsense\n2\t/c/other/local-logs\n"
        );
        let sizes = log_sizes(&du);
        assert_eq!(sizes[&id], 12 * 1024);
        assert_eq!(sizes["other"], 2048);
        assert!(is_full_id(&id));
        assert!(!is_full_id("../etc") && !is_full_id(&"g".repeat(64)) && !is_full_id("abc"));
    }

    #[test]
    fn without_docker_the_page_is_told_so() {
        assert_eq!(scan(None, Path::new("/nonexistent"))["missing"], true);
    }
}
