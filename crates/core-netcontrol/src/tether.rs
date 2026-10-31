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

/// Where the phone pings to see whether it reaches the internet, by address and then by name (which needs DNS).
pub const PING_ADDRESS: &str = "1.1.1.1";
pub const PING_NAME: &str = "google.com";

/// The arguments that make `phone` ping `host` `count` times from its own shell, through `adb`.
pub fn ping_args(serial: &str, host: &str, count: u32) -> Option<Vec<String>> {
    let host_ok = !host.is_empty() && host.bytes().all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'-'));
    (valid_serial(serial) && host_ok).then(|| {
        ["-s", serial, "shell", "ping", "-c", &count.to_string(), "-W", "2", host].iter().map(|s| (*s).to_owned()).collect()
    })
}

/// What a ping said: how many answers came back of those sent, and the average time of one, in milliseconds.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Ping {
    pub sent: u32,
    pub received: u32,
    pub average_ms: Option<f64>,
}

/// Reads the summary of `ping` as Android's toybox and BSD print it.
pub fn parse_ping(text: &str) -> Option<Ping> {
    let mut out = None;
    for line in text.lines() {
        if let Some((sent, rest)) = line.split_once(" packets transmitted, ") {
            let sent = sent.trim().parse().ok()?;
            let received = rest.split_whitespace().next()?.parse().ok()?;
            out = Some(Ping { sent, received, average_ms: None });
        }
        if let (Some(ping), Some((_, values))) = (out.as_mut(), line.split_once(" = ")) {
            if line.contains("min/avg/max") {
                ping.average_ms = values.split('/').nth(1).and_then(|v| v.trim().parse().ok());
            }
        }
    }
    out
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
    fn a_ping_from_the_phone_is_asked_and_read() {
        assert_eq!(
            ping_args("R58", PING_ADDRESS, 4).unwrap(),
            ["-s", "R58", "shell", "ping", "-c", "4", "-W", "2", "1.1.1.1"]
        );
        assert!(ping_args("R58", "a;reboot", 1).is_none() && ping_args("-x", "1.1.1.1", 1).is_none());
        let android = "PING 1.1.1.1 (1.1.1.1) 56(84) bytes of data.\n64 bytes from 1.1.1.1: icmp_seq=1 ttl=57 time=21.4 ms\n\n--- 1.1.1.1 ping statistics ---\n4 packets transmitted, 3 received, 25% packet loss, time 3004ms\nrtt min/avg/max/mdev = 18.120/22.517/27.301/3.402 ms\n";
        assert_eq!(parse_ping(android), Some(Ping { sent: 4, received: 3, average_ms: Some(22.517) }));
        let none = "PING google.com (142.250.1.1) 56(84) bytes of data.\n\n--- google.com ping statistics ---\n2 packets transmitted, 0 received, 100% packet loss, time 1001ms\n";
        assert_eq!(parse_ping(none), Some(Ping { sent: 2, received: 0, average_ms: None }));
        let bsd = "--- 1.1.1.1 ping statistics ---\n3 packets transmitted, 3 packets received, 0.0% packet loss\nround-trip min/avg/max/stddev = 9.1/10.5/12.0/1.2 ms\n";
        assert_eq!(parse_ping(bsd).unwrap().average_ms, Some(10.5));
        assert_eq!(parse_ping("ping: unknown host google.com"), None);
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
