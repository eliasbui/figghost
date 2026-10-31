//! Sharing this Mac's connection with an Android phone on USB (beta), through gnirehtet (Genymobile, Apache 2.0):
//! `adb reverse` carries the phone's traffic to a relay on the Mac, and an app on the phone acts as its VPN. Neither
//! side needs an administrator or root. Figghost only finds the tools, lists the phones and runs `gnirehtet autorun`.

use std::path::{Path, PathBuf};

/// Where Homebrew and the usual installs put the tools.
pub const TOOL_DIRS: [&str; 3] = ["/opt/homebrew/bin", "/usr/local/bin", "/usr/bin"];

/// The program of that name in the first of `dirs` that has it.
pub fn find_in(name: &str, dirs: &[&Path]) -> Option<PathBuf> {
    dirs.iter().map(|d| d.join(name)).find(|p| p.is_file())
}

pub fn find(name: &str) -> Option<PathBuf> {
    let dirs: Vec<&Path> = TOOL_DIRS.iter().map(Path::new).collect();
    find_in(name, &dirs)
}

/// The app gnirehtet installs on the phone, wherever its package put it, for `GNIREHTET_APK`.
pub fn apk_near(gnirehtet: &Path) -> Option<PathBuf> {
    let real = gnirehtet
        .canonicalize()
        .unwrap_or_else(|_| gnirehtet.to_owned());
    let dir = real.parent()?;
    [
        dir.join("gnirehtet.apk"),
        dir.join("../libexec/gnirehtet.apk"),
        dir.join("../share/gnirehtet/gnirehtet.apk"),
        dir.join("../share/gnirehtet.apk"),
    ]
    .into_iter()
    .find(|p| p.is_file())
}

/// A phone `adb` lists as ready: its serial, and the model it reports (`Pixel 8`), or the serial when it says none.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Phone {
    pub serial: String,
    pub model: String,
}

/// The phones `adb devices -l` lists as ready; those not authorised yet or offline are left out.
pub fn parse_devices(text: &str) -> Vec<Phone> {
    text.lines()
        .skip_while(|l| !l.starts_with("List of devices"))
        .skip(1)
        .filter_map(|l| {
            let mut parts = l.split_whitespace();
            let serial = parts.next()?;
            if parts.next() != Some("device") {
                return None;
            }
            let model = parts
                .find_map(|p| p.strip_prefix("model:"))
                .map(|m| m.replace('_', " "))
                .filter(|m| !m.is_empty() && m.chars().all(|c| !c.is_control()))
                .unwrap_or_else(|| serial.to_owned());
            Some(Phone { serial: serial.to_owned(), model })
        })
        .collect()
}

/// A serial that can be passed to `adb -s` and `gnirehtet stop` as one argument.
pub fn valid_serial(serial: &str) -> bool {
    !serial.is_empty()
        && serial.len() <= 64
        && !serial.starts_with('-')
        && serial.bytes().all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b':' | b'_' | b'-'))
}

/// The arguments that stop sharing on a phone: its VPN goes off and the reverse tunnel of `adb` is taken away.
pub fn stop_args(serial: &str) -> Option<Vec<String>> {
    valid_serial(serial).then(|| vec!["stop".to_owned(), serial.to_owned()])
}

/// Phones that are plugged in but have not allowed this Mac yet: they say so on their screen.
pub fn count_unauthorized(text: &str) -> usize {
    text.lines()
        .filter(|l| l.split_whitespace().nth(1) == Some("unauthorized"))
        .count()
}

/// The DNS servers of this Mac, from `scutil --dns`, for the phone to ask: the relay carries its questions to them
/// from the Mac, so names resolve as they do here (a company network often lets no other DNS server through). Only
/// IPv4, which is all the relay carries, and never a loopback or link-local address, which on the phone would stay on
/// the phone. At most three, in the order the Mac asks them.
pub fn dns_servers(scutil: &str) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for line in scutil.lines() {
        let Some((key, value)) = line.split_once(':') else { continue };
        if !key.trim().starts_with("nameserver[") {
            continue;
        }
        let Ok(ip) = value.trim().parse::<std::net::Ipv4Addr>() else { continue };
        if ip.is_loopback() || ip.is_link_local() || ip.is_unspecified() || ip.is_multicast() {
            continue;
        }
        let ip = ip.to_string();
        if !out.contains(&ip) && out.len() < 3 {
            out.push(ip);
        }
    }
    out
}

/// The DNS server the phone falls back to when the Mac's do not answer.
pub const FALLBACK_DNS: &str = "8.8.8.8";

/// The arguments of `gnirehtet autorun`: the phone asks `dns` first, then the public one.
pub fn autorun_args(dns: &[String]) -> Vec<String> {
    let mut servers: Vec<String> = dns.to_vec();
    if !servers.iter().any(|d| d == FALLBACK_DNS) {
        servers.push(FALLBACK_DNS.to_owned());
    }
    vec!["autorun".to_owned(), "-d".to_owned(), servers.join(",")]
}

/// What a test of the phone asks. The relay carries TCP and UDP, not the ICMP of `ping`, so the test opens a real
/// connection from the phone, as an app would: by address (does it reach the internet) and by name (can it look names
/// up). The page asked is the one Android itself uses to tell whether a network reaches the internet.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Probe {
    Address,
    Name,
}

/// By address: Cloudflare's, which answers any request on port 80 and does not move.
pub const PROBE_ADDRESS: &str = "1.1.1.1";
pub const PROBE_NAME: &str = "connectivitycheck.gstatic.com";

/// The arguments for `adb` that make the phone ask the page with the `nc` of its toybox and print the answer.
pub fn probe_args(serial: &str, probe: Probe) -> Option<Vec<String>> {
    if !valid_serial(serial) {
        return None;
    }
    let host = match probe {
        Probe::Address => PROBE_ADDRESS,
        Probe::Name => PROBE_NAME,
    };
    let script = format!(
        "printf 'GET /generate_204 HTTP/1.0\\r\\nHost: {PROBE_NAME}\\r\\n\\r\\n' | toybox nc -w 6 {host} 80 2>&1 | head -c 200"
    );
    Some(vec!["-s".into(), serial.into(), "shell".into(), script])
}

/// What the phone printed: the page answered (`Some(true)`), it did not (`Some(false)`), or the phone has no `nc` to
/// ask with (`None`).
pub fn read_probe(text: &str) -> Option<bool> {
    let lower = text.to_ascii_lowercase();
    if lower.contains("not found") && lower.contains("nc") {
        return None;
    }
    Some(text.starts_with("HTTP/") || text.lines().any(|l| l.starts_with("HTTP/1.")))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ready_phones_are_listed_and_the_others_counted() {
        let text = "* daemon started successfully\nList of devices attached\nR58M12ABC              device usb:1-1 product:o1s model:SM_G991B device:o1s transport_id:2\nemulator-5554\toffline\nXYZ\tunauthorized\nPLAIN\tdevice\n\n";
        assert_eq!(
            parse_devices(text),
            [
                Phone { serial: "R58M12ABC".into(), model: "SM G991B".into() },
                Phone { serial: "PLAIN".into(), model: "PLAIN".into() }
            ]
        );
        assert_eq!(stop_args("R58M12ABC"), Some(vec!["stop".to_owned(), "R58M12ABC".to_owned()]));
        assert_eq!(stop_args("192.168.1.5:5555").map(|a| a.len()), Some(2));
        assert_eq!(stop_args("-rf"), None);
        assert_eq!(stop_args("a b"), None);
        assert_eq!(count_unauthorized(text), 1);
        assert!(parse_devices("").is_empty());
    }

    #[test]
    fn the_phone_asks_the_dns_of_the_mac_first() {
        let scutil = "DNS configuration\n\nresolver #1\n  search domain[0] : corp.vn\n  nameserver[0] : 10.161.70.200\n  nameserver[1] : 127.0.0.1\n  nameserver[2] : fe80::1%en0\n  nameserver[3] : 10.161.70.200\n  flags    : Request A records\n\nresolver #2\n  nameserver[0] : 169.254.1.1\n  nameserver[1] : 1.0.0.1\n";
        assert_eq!(dns_servers(scutil), ["10.161.70.200", "1.0.0.1"]);
        assert_eq!(autorun_args(&dns_servers(scutil)), ["autorun", "-d", "10.161.70.200,1.0.0.1,8.8.8.8"]);
        assert_eq!(autorun_args(&[]), ["autorun", "-d", "8.8.8.8"]);
    }

    #[test]
    fn the_phone_opens_a_real_connection_to_test() {
        let args = probe_args("R58", Probe::Name).unwrap();
        assert_eq!(&args[..3], ["-s", "R58", "shell"]);
        assert!(args[3].contains("toybox nc -w 6 connectivitycheck.gstatic.com 80") && args[3].contains("generate_204"));
        assert!(probe_args("R58", Probe::Address).unwrap()[3].contains(PROBE_ADDRESS));
        assert!(probe_args("-x", Probe::Name).is_none());
        assert_eq!(read_probe("HTTP/1.0 204 No Content\r\nContent-Length: 0\r\n"), Some(true));
        assert_eq!(read_probe(""), Some(false), "no answer in time");
        assert_eq!(read_probe("nc: bad address 'connectivitycheck.gstatic.com'"), Some(false), "a name that does not resolve");
        assert_eq!(read_probe("/system/bin/sh: toybox nc: not found"), None);
    }

    #[test]
    fn a_tool_is_found_in_the_first_folder_that_has_it() {
        let dir = std::env::temp_dir().join(format!("figghost-tether-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("adb"), "").unwrap();
        assert_eq!(
            find_in("adb", &[Path::new("/nowhere"), &dir]),
            Some(dir.join("adb"))
        );
        assert_eq!(find_in("gnirehtet", &[&dir]), None);
        std::fs::write(dir.join("gnirehtet.apk"), "").unwrap();
        assert_eq!(
            apk_near(&dir.join("adb")).map(|p| p.file_name().unwrap().to_owned()),
            Some("gnirehtet.apk".into())
        );
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
