//! Runs the proxy of the Network page against the real internet and the real links of this Mac, with `curl` as the app,
//! and prints what each wire does: the system's way, one link, every link in turn, Block, a quota that ran out and a
//! metered link. It starts its own proxy on a free port and does not touch the system's proxy settings.
//!
//! `cargo run -p core-netcontrol --example proxy_check [url]`

use core_netcontrol::{
    proxy::Control,
    rules::{Priority, Rule, Target},
};
use std::{process::Command, time::Instant};

fn rule(target: Target) -> Rule {
    Rule {
        app: "curl".into(),
        target,
        priority: Priority::Normal,
        down_kbps: 0,
        up_kbps: 0,
    }
}

/// One request through the proxy: (curl's exit code, HTTP status, bytes, seconds).
fn fetch(port: u16, url: &str, http: bool) -> (i32, String, String, f64) {
    let started = Instant::now();
    let out = Command::new("/usr/bin/curl")
        .args([
            "-s",
            "-o",
            "/dev/null",
            "--max-time",
            "15",
            "-w",
            "%{http_code} %{size_download}",
        ])
        // The way the system's settings reach it: a SOCKS proxy, or the secure web proxy (HTTP CONNECT).
        .args(
            if http {
                ["-x", "http://127.0.0.1"]
            } else {
                ["--socks5-hostname", "127.0.0.1"]
            }
            .map(|a| {
                if a == "http://127.0.0.1" {
                    format!("http://127.0.0.1:{port}")
                } else if a == "127.0.0.1" {
                    format!("127.0.0.1:{port}")
                } else {
                    a.to_owned()
                }
            }),
        )
        .arg(url)
        .output()
        .expect("curl runs");
    let text = String::from_utf8_lossy(&out.stdout).into_owned();
    let mut parts = text.split_whitespace();
    (
        out.status.code().unwrap_or(-1),
        parts.next().unwrap_or("-").to_owned(),
        parts.next().unwrap_or("-").to_owned(),
        started.elapsed().as_secs_f64(),
    )
}

fn main() {
    let url = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "http://captive.apple.com/hotspot-detect.html".into());
    let control = Control::start(0, Vec::new()).expect("the proxy starts");
    let port = control.port();
    println!("proxy on 127.0.0.1:{port}, asking {url}");
    let snapshot = control.snapshot();
    for l in &snapshot.links {
        println!(
            "link {} ({}) {} primary={}",
            l.name, l.label, l.ip, l.primary
        );
    }
    let mut cases: Vec<(String, Target)> = vec![
        ("the system's way".into(), Target::System),
        ("every link in turn".into(), Target::Merge),
        ("Block".into(), Target::Block),
    ];
    for l in &snapshot.links {
        cases.push((format!("only {}", l.name), Target::Link(l.name.clone())));
    }
    for via_http in [false, true] {
        println!("-- by {}", if via_http { "HTTP CONNECT" } else { "SOCKS5" });
        for (name, target) in &cases {
            control.set_rules(vec![rule(target.clone())]);
            let (code, http, bytes, secs) = fetch(port, &url, via_http);
            let seen = control
                .snapshot()
                .apps
                .iter()
                .find(|a| a.name == "curl")
                .map(|a| (a.refused, a.hosts.len()))
                .unwrap_or((0, 0));
            println!(
                "{name:<24} curl exit {code:<3} http {http:<4} {bytes:>6} bytes {secs:>5.2}s   refused so far {} sites {}",
                seen.0, seen.1
            );
        }
    }
    control.set_rules(Vec::new());
    for (name, blocked, only) in [
        ("quota ran out", vec!["Curl".to_owned()], None),
        (
            "metered, curl not allowed",
            vec![],
            Some(vec!["Mail".to_owned()]),
        ),
        (
            "metered, curl allowed",
            vec![],
            Some(vec!["CURL".to_owned()]),
        ),
        ("policy cleared", vec![], None),
    ] {
        control.set_policy(&blocked, only.as_deref());
        let (code, http, bytes, secs) = fetch(port, &url, true);
        println!(
            "{name:<24} curl exit {code:<3} http {http:<4} {bytes:>6} bytes {secs:>5.2}s (HTTP CONNECT)"
        );
    }
}
