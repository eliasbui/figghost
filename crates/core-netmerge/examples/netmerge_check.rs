//! The proxy on the links of this computer with real downloads through it: `cargo run -p core-netmerge --example
//! netmerge_check -- <url> [connections] [seconds]` starts the proxy, runs that many curl downloads of the link through
//! it at once as one download (so the first takes the system's link and the others spread), and prints every second
//! what each link carries, then what each curl got. Nothing is written but to /dev/null.

use std::{
    process::{Command, Stdio},
    thread,
    time::Duration,
};

fn main() {
    let mut args = std::env::args().skip(1);
    let Some(url) = args.next() else {
        eprintln!("usage: netmerge_check <url> [connections] [seconds]");
        std::process::exit(2);
    };
    let connections: usize = args.next().and_then(|n| n.parse().ok()).unwrap_or(4);
    let seconds: u64 = args.next().and_then(|n| n.parse().ok()).unwrap_or(15);
    let merge = core_netmerge::Merge::start().expect("the proxy starts");
    thread::sleep(Duration::from_secs(2));
    show(&merge.stats());
    let proxy = merge.proxy_url("socks5h", "check");
    let curls: Vec<_> = (0..connections)
        .map(|_| {
            Command::new("/usr/bin/curl")
                .args([
                    "-sL",
                    "-o",
                    "/dev/null",
                    "-w",
                    "%{http_code} %{size_download} %{speed_download}",
                ])
                // As the apps do: a failed connection is tried again, and the proxy sends it by another link.
                .args(["--retry", "5", "--retry-all-errors", "--retry-delay", "1"])
                .args([
                    "--max-time",
                    &seconds.to_string(),
                    "--proxy",
                    &proxy,
                    "--",
                    &url,
                ])
                .stdout(Stdio::piped())
                .spawn()
                .expect("curl starts")
        })
        .collect();
    for _ in 0..seconds {
        thread::sleep(Duration::from_secs(1));
        let stats = merge.stats();
        let line: Vec<String> = stats
            .links
            .iter()
            .map(|l| {
                format!(
                    "{} {} conn {:.1} MB/s",
                    l.label,
                    l.connections,
                    l.speed as f64 / 1e6
                )
            })
            .collect();
        println!(
            "{} | links used by the download: {}",
            line.join(" | "),
            merge.links_used("check")
        );
    }
    for curl in curls {
        let out = curl.wait_with_output().expect("curl ends");
        println!("curl: {}", String::from_utf8_lossy(&out.stdout));
    }
    show(&merge.stats());
}

fn show(stats: &core_netmerge::Stats) {
    println!("paused: {}", stats.paused);
    for l in &stats.links {
        println!(
            "  {} ({}, {}) {}{}: {} connections, {:.1} MB received",
            l.label,
            l.name,
            l.ip,
            l.state,
            if l.primary { ", the system's own" } else { "" },
            l.connections,
            l.received as f64 / 1e6
        );
    }
}
