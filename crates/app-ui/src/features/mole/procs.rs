//! Programs that coding agents leave running after their session is gone: a server of the Model Context Protocol, a
//! dev server, a headless browser of a test. They are told apart from the rest by what they are and by being orphans
//! (their parent is `launchd`), and listed for the person to choose; nothing is stopped by itself, and only what the
//! last look listed, and still looks the same, can be stopped.

use macos_sys::process;
use serde_json::{Value, json};
use std::{collections::BTreeMap, process::Command, thread, time::Duration};

/// A program that ran for less than this may still be starting.
const MIN_AGE_SECONDS: u64 = 600;
/// How long a program is given to end before it is made to.
const GRACE: Duration = Duration::from_secs(5);
/// The most rows listed.
const MAX_ROWS: usize = 200;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Proc {
    pub pid: i32,
    pub ppid: i32,
    pub uid: u32,
    pub age_seconds: u64,
    pub rss_kb: u64,
    pub command: String,
}

/// `[[dd-]hh:]mm:ss`, as `ps` says how long a program has run.
pub fn parse_etime(text: &str) -> Option<u64> {
    let (days, rest) = match text.split_once('-') {
        Some((d, rest)) => (d.parse::<u64>().ok()?, rest),
        None => (0, text),
    };
    let mut seconds = 0u64;
    let parts: Vec<&str> = rest.split(':').collect();
    if parts.len() > 3 || parts.is_empty() {
        return None;
    }
    for part in &parts {
        seconds = seconds * 60 + part.parse::<u64>().ok()?;
    }
    Some(days * 86_400 + seconds)
}

/// The lines of `ps -axo pid=,ppid=,uid=,etime=,rss=,command=`.
pub fn parse_ps(text: &str) -> Vec<Proc> {
    text.lines()
        .filter_map(|line| {
            let mut fields = line.split_whitespace();
            let pid = fields.next()?.parse().ok()?;
            let ppid = fields.next()?.parse().ok()?;
            let uid = fields.next()?.parse().ok()?;
            let age_seconds = parse_etime(fields.next()?)?;
            let rss_kb = fields.next()?.parse().ok()?;
            let command = fields.collect::<Vec<_>>().join(" ");
            (!command.is_empty()).then_some(Proc {
                pid,
                ppid,
                uid,
                age_seconds,
                rss_kb,
                command,
            })
        })
        .collect()
}

/// What kind of leftover a program is, from its command line; `None` for what is not one. The word is a key of the page.
pub fn kind_of(command: &str) -> Option<&'static str> {
    let lower = command.to_lowercase();
    let has = |words: &[&str]| words.iter().any(|w| lower.contains(w));
    if has(&[
        "mcp-server",
        "server-mcp",
        "modelcontextprotocol",
        "mcp-remote",
        "mcp_server",
        "/mcp/",
        "-mcp ",
    ]) || lower.ends_with("-mcp")
    {
        return Some("mcp");
    }
    if has(&["playwright", "puppeteer"]) && has(&["chrom", "headless", "node"]) {
        return Some("browser");
    }
    if has(&[
        "vite",
        "next dev",
        "next-server",
        "webpack serve",
        "webpack-dev-server",
        "nodemon",
        "ts-node-dev",
        "storybook dev",
        "astro dev",
        "nuxt dev",
        "remix dev",
        "react-scripts start",
        "npm run dev",
        "pnpm dev",
        "yarn dev",
        "bun dev",
    ]) {
        return Some("dev");
    }
    if lower.contains(".npm/_npx") {
        return Some("npx");
    }
    None
}

/// Where a program that is part of the system or of an installed app lives: never a leftover.
fn is_system(command: &str) -> bool {
    let program = command.split_whitespace().next().unwrap_or_default();
    [
        "/System/",
        "/usr/libexec/",
        "/usr/sbin/",
        "/sbin/",
        "/Library/",
        "/Applications/",
    ]
    .iter()
    .any(|p| program.starts_with(p))
        && !program.contains("/node_modules/")
}

/// The programs that look like what an agent left: of this user, orphans, old enough, of a known kind.
pub fn leftovers(all: &[Proc], uid: u32, own_pid: i32) -> Vec<(Proc, &'static str)> {
    let mut found: Vec<(Proc, &'static str)> = all
        .iter()
        .filter(|p| {
            p.uid == uid
                && p.ppid == 1
                && p.pid != own_pid
                && p.age_seconds >= MIN_AGE_SECONDS
                && !is_system(&p.command)
        })
        .filter_map(|p| kind_of(&p.command).map(|k| (p.clone(), k)))
        .collect();
    found.sort_by(|a, b| b.0.rss_kb.cmp(&a.0.rss_kb).then(a.0.pid.cmp(&b.0.pid)));
    found.truncate(MAX_ROWS);
    found
}

/// Every program under `root`, with its own children, found from the parent of each.
pub fn tree(all: &[Proc], root: i32) -> Vec<i32> {
    let mut by_parent: BTreeMap<i32, Vec<i32>> = BTreeMap::new();
    for p in all {
        by_parent.entry(p.ppid).or_default().push(p.pid);
    }
    let mut out = vec![root];
    let mut at = 0;
    while at < out.len() && out.len() < 500 {
        if let Some(children) = by_parent.get(&out[at]) {
            for child in children {
                if !out.contains(child) {
                    out.push(*child);
                }
            }
        }
        at += 1;
    }
    out
}

/// The ports each program listens on, from `lsof -F pn`.
pub fn parse_listening(text: &str) -> BTreeMap<i32, Vec<u16>> {
    let mut out: BTreeMap<i32, Vec<u16>> = BTreeMap::new();
    let mut pid = None;
    for line in text.lines() {
        if let Some(p) = line.strip_prefix('p') {
            pid = p.parse::<i32>().ok();
        } else if let (Some(name), Some(pid)) = (line.strip_prefix('n'), pid)
            && let Some((_, port)) = name.rsplit_once(':')
            && let Ok(port) = port.parse::<u16>()
        {
            let ports = out.entry(pid).or_default();
            if !ports.contains(&port) {
                ports.push(port);
            }
        }
    }
    out
}

fn ps() -> Vec<Proc> {
    let out = Command::new("/bin/ps")
        .args(["-axo", "pid=,ppid=,uid=,etime=,rss=,command="])
        .output();
    out.ok()
        .filter(|o| o.status.success())
        .map(|o| parse_ps(&String::from_utf8_lossy(&o.stdout)))
        .unwrap_or_default()
}

fn listening() -> BTreeMap<i32, Vec<u16>> {
    let out = Command::new("/usr/sbin/lsof")
        .args(["-nP", "-iTCP", "-sTCP:LISTEN", "-Fpn"])
        .output();
    out.ok()
        .map(|o| parse_listening(&String::from_utf8_lossy(&o.stdout)))
        .unwrap_or_default()
}

fn own_uid() -> u32 {
    process::user_id()
}

fn own_pid() -> i32 {
    std::process::id() as i32
}

/// A short name for the row: the program and what it was given, cut to a length that fits.
fn name_of(command: &str) -> String {
    let words: Vec<&str> = command.split_whitespace().collect();
    let program = words
        .first()
        .and_then(|w| w.rsplit('/').next())
        .unwrap_or_default();
    let rest: Vec<&str> = words
        .iter()
        .skip(1)
        .copied()
        .filter(|w| !w.starts_with('-') && !w.bytes().all(|b| b.is_ascii_digit()))
        .take(2)
        .map(|w| w.rsplit('/').next().unwrap_or(w))
        .collect();
    let text = if rest.is_empty() {
        program.to_owned()
    } else {
        format!("{program} {}", rest.join(" "))
    };
    text.chars().take(80).collect()
}

/// What the page lists.
pub fn scan() -> Value {
    let all = ps();
    let ports = listening();
    let items: Vec<Value> = leftovers(&all, own_uid(), own_pid())
        .into_iter()
        .map(|(p, kind)| {
            let ports = ports.get(&p.pid).cloned().unwrap_or_default();
            json!({
                "path": p.pid.to_string(), "name": name_of(&p.command), "note": kind, "size": p.rss_kb * 1024, "dir": false,
                "age": p.age_seconds / 86_400, "extra": 0, "pid": p.pid, "kind": kind, "ports": ports, "minutes": p.age_seconds / 60,
                "command": p.command.chars().take(300).collect::<String>(),
            })
        })
        .collect();
    json!({ "ok": true, "items": items })
}

/// Stops the programs with these pids, each only if it is still a leftover of the same kind it was listed as: a pid
/// that was given to another program since then is left alone. Each gets a request to end, and a program, and the ones
/// it started, that is still there after a few seconds is made to. Returns how many were stopped and how many not.
pub fn stop(pids: &[i32]) -> (usize, usize) {
    let all = ps();
    let leftovers = leftovers(&all, own_uid(), own_pid());
    let (valid, refused): (Vec<i32>, Vec<i32>) = pids
        .iter()
        .partition(|pid| leftovers.iter().any(|(p, _)| p.pid == **pid));
    let mut doomed: Vec<i32> = valid.iter().flat_map(|pid| tree(&all, *pid)).collect();
    doomed.sort_unstable();
    doomed.dedup();
    doomed.retain(|pid| *pid > 1 && *pid != own_pid());
    for pid in &doomed {
        process::terminate(*pid, false);
    }
    let mut waited = Duration::ZERO;
    while waited < GRACE && doomed.iter().any(|pid| process::is_alive(*pid)) {
        thread::sleep(Duration::from_millis(250));
        waited += Duration::from_millis(250);
    }
    for pid in &doomed {
        if process::is_alive(*pid) {
            process::terminate(*pid, true);
        }
    }
    let stopped = valid.iter().filter(|pid| !process::is_alive(**pid)).count();
    (stopped, refused.len() + valid.len() - stopped)
}

#[cfg(test)]
mod tests {
    use super::*;

    const PS: &str = "  101     1   501 02:10:00  81234 /Users/me/.nvm/versions/node/v22/bin/node /Users/me/proj/node_modules/.bin/vite --port 5173
  102   101   501 02:09:59  20000 /usr/bin/esbuild --service
  103     1   501    05:00   9000 node /Users/me/.npm/_npx/abc/node_modules/.bin/mcp-server-filesystem /Users
  104     1   501 3-04:05:06 150000 node /opt/x/node_modules/@modelcontextprotocol/server-github/dist/index.js
  105     1   502 04:00:00   5000 node /Users/other/vite.js
  106     1   501 01:00:00   3000 /usr/sbin/cfprefsd agent
  107   500   501 05:00:00   4000 node /Users/me/proj/node_modules/.bin/vite
  108     1   501 01:30:00  70000 /Users/me/.cache/ms-playwright/chromium-1/chrome --headless --remote-debugging-pipe
  109     1   501 02:00:00   1000 /Applications/Visual Studio Code.app/Contents/Frameworks/Code Helper (Renderer) vite
";

    #[test]
    fn ps_output_is_read_with_its_ages_and_a_line_that_is_not_one_is_left_out() {
        let all = parse_ps(&format!("{PS}garbage\n  x y z\n"));
        assert_eq!(all.len(), 9);
        assert_eq!(
            (
                all[0].pid,
                all[0].ppid,
                all[0].uid,
                all[0].age_seconds,
                all[0].rss_kb
            ),
            (101, 1, 501, 7800, 81234)
        );
        assert!(all[0].command.ends_with("vite --port 5173"));
        assert_eq!(all[3].age_seconds, 3 * 86_400 + 4 * 3600 + 5 * 60 + 6);
    }

    #[test]
    fn how_long_a_program_has_run_is_read_in_every_form() {
        assert_eq!(parse_etime("05:00"), Some(300));
        assert_eq!(parse_etime("01:02:03"), Some(3723));
        assert_eq!(parse_etime("2-00:00:01"), Some(172_801));
        assert_eq!(parse_etime("45"), Some(45));
        for bad in ["", "a:b", "1:2:3:4", "-1", "1-"] {
            assert_eq!(parse_etime(bad), None, "{bad:?}");
        }
    }

    #[test]
    fn what_an_agent_leaves_is_told_by_its_command_line() {
        assert_eq!(
            kind_of("node /x/node_modules/@modelcontextprotocol/server-github/dist/index.js"),
            Some("mcp")
        );
        assert_eq!(kind_of("node /x/mcp-server-filesystem /Users"), Some("mcp"));
        assert_eq!(kind_of("npx -y chrome-devtools-mcp"), Some("mcp"));
        assert_eq!(
            kind_of("/x/ms-playwright/chromium-1/chrome --headless"),
            Some("browser")
        );
        assert_eq!(kind_of("node /x/.bin/vite --port 5173"), Some("dev"));
        assert_eq!(kind_of("node /x/.bin/next dev"), Some("dev"));
        assert_eq!(
            kind_of("node /Users/me/.npm/_npx/abc/bin/tool"),
            Some("npx")
        );
        for no in [
            "/usr/bin/ssh-agent",
            "node /x/server.js",
            "python3 -m http.server",
            "/bin/zsh -l",
        ] {
            assert_eq!(kind_of(no), None, "{no}");
        }
    }

    #[test]
    fn only_orphans_of_this_user_that_are_old_enough_and_not_of_the_system_are_listed() {
        let all = parse_ps(PS);
        let listed: Vec<i32> = leftovers(&all, 501, 9999)
            .into_iter()
            .map(|(p, _)| p.pid)
            .collect();
        // The biggest first: 104 (150000), 101 (81234), 108 (70000). 103 is too young, 105 is another user's, 106 is
        // not of a kind, 107 has a parent, 109 is inside an app, and 102 is a child of 101.
        assert_eq!(listed, [104, 101, 108]);
        assert!(
            leftovers(&all, 501, 101).iter().all(|(p, _)| p.pid != 101),
            "this program is never listed"
        );
    }

    #[test]
    fn what_a_program_started_goes_with_it_and_a_loop_of_parents_ends() {
        let all = parse_ps(PS);
        assert_eq!(tree(&all, 101), [101, 102]);
        assert_eq!(tree(&all, 104), [104]);
        let looped = vec![
            Proc {
                pid: 5,
                ppid: 6,
                uid: 1,
                age_seconds: 1,
                rss_kb: 1,
                command: "a".into(),
            },
            Proc {
                pid: 6,
                ppid: 5,
                uid: 1,
                age_seconds: 1,
                rss_kb: 1,
                command: "b".into(),
            },
        ];
        assert_eq!(tree(&looped, 5), [5, 6]);
    }

    #[test]
    fn the_ports_a_program_listens_on_are_read_from_lsof() {
        let text =
            "p101\nn*:5173\nn[::1]:5173\np104\nn127.0.0.1:3000\nn127.0.0.1:3001\np7\nnnot a port\n";
        let ports = parse_listening(text);
        assert_eq!(ports[&101], [5173]);
        assert_eq!(ports[&104], [3000, 3001]);
        assert!(!ports.contains_key(&7));
    }

    #[test]
    fn a_row_is_named_by_its_program_and_what_it_was_given() {
        assert_eq!(
            name_of("/Users/me/.nvm/bin/node /Users/me/proj/node_modules/.bin/vite --port 5173"),
            "node vite"
        );
        assert_eq!(name_of("/x/chrome --headless"), "chrome");
        assert!(
            name_of(&format!("node {}", "a".repeat(200)))
                .chars()
                .count()
                <= 80
        );
    }

    #[test]
    fn a_program_that_is_gone_or_never_listed_is_not_stopped_and_the_count_says_so() {
        // 2^30 is no pid of this Mac; nothing is signalled.
        let (stopped, failed) = stop(&[1 << 30]);
        assert_eq!((stopped, failed), (0, 1));
        assert_eq!(stop(&[]), (0, 0));
    }
}
