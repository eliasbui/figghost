//! The system's proxy settings, through `networksetup`: what each network service had before, the commands that point
//! its SOCKS proxy and its secure web proxy at Figghost's, and those that put back what it had. Pure text in and out;
//! the app runs the commands.

pub const NETWORKSETUP: &str = "/usr/sbin/networksetup";

/// One proxy setting of one service.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Proxy {
    pub enabled: bool,
    pub server: String,
    pub port: u16,
}

/// The two settings Figghost changes, as a service had them.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Saved {
    pub service: String,
    pub socks: Proxy,
    pub secure: Proxy,
}

/// A service name that can be passed as one argument and kept on one line of the saved file.
fn valid_service(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= 128
        && !name.chars().any(char::is_control)
        && !name.starts_with('-')
}

/// The enabled services in `networksetup -listallnetworkservices`: its first line explains the asterisk that marks
/// the disabled ones.
pub fn parse_services(text: &str) -> Vec<String> {
    text.lines()
        .skip(1)
        .map(str::trim_end)
        .filter(|l| !l.starts_with('*'))
        .filter(|l| valid_service(l))
        .map(str::to_owned)
        .collect()
}

/// What `networksetup -getsocksfirewallproxy` or `-getsecurewebproxy` printed.
pub fn parse_proxy(text: &str) -> Option<Proxy> {
    let mut enabled = None;
    let (mut server, mut port) = (String::new(), 0);
    for line in text.lines() {
        let Some((key, value)) = line.split_once(':') else {
            continue;
        };
        let value = value.trim();
        match key.trim() {
            "Enabled" => enabled = Some(value.eq_ignore_ascii_case("yes")),
            "Server" => server = value.to_owned(),
            "Port" => port = value.parse().unwrap_or(0),
            _ => {}
        }
    }
    Some(Proxy {
        enabled: enabled?,
        server,
        port,
    })
}

/// Whether a setting points at Figghost's proxy on `port`.
pub fn is_ours(proxy: &Proxy, port: u16) -> bool {
    proxy.enabled
        && proxy.port == port
        && matches!(proxy.server.as_str(), "127.0.0.1" | "localhost")
}

fn args(list: &[&str]) -> Vec<String> {
    list.iter().map(|s| (*s).to_owned()).collect()
}

/// The commands that point every service at `127.0.0.1:port`.
pub fn apply_commands(services: &[String], port: u16) -> Vec<Vec<String>> {
    let port = port.to_string();
    let mut out = Vec::new();
    for service in services.iter().filter(|s| valid_service(s)) {
        out.push(args(&[
            "-setsocksfirewallproxy",
            service,
            "127.0.0.1",
            &port,
        ]));
        out.push(args(&["-setsecurewebproxy", service, "127.0.0.1", &port]));
    }
    out
}

/// The commands that put back what each service had: its own server when it had one, then whether it was on.
pub fn restore_commands(saved: &[Saved], ours: u16) -> Vec<Vec<String>> {
    let mut out = Vec::new();
    for s in saved.iter().filter(|s| valid_service(&s.service)) {
        for (proxy, set, state) in [
            (
                &s.socks,
                "-setsocksfirewallproxy",
                "-setsocksfirewallproxystate",
            ),
            (&s.secure, "-setsecurewebproxy", "-setsecurewebproxystate"),
        ] {
            // A setting that was already ours (left by a run that ended badly) is put back to off.
            let theirs = !is_ours(proxy, ours) && !proxy.server.is_empty() && proxy.port != 0;
            if theirs {
                out.push(args(&[
                    set,
                    &s.service,
                    &proxy.server,
                    &proxy.port.to_string(),
                ]));
            }
            let on = theirs && proxy.enabled;
            out.push(args(&[state, &s.service, if on { "on" } else { "off" }]));
        }
    }
    out
}

/// The saved settings as lines of text: `service`, then the two proxies as `on|off server port`, tab-separated.
pub fn write_saved(saved: &[Saved]) -> String {
    let mut out = String::new();
    for s in saved.iter().filter(|s| valid_service(&s.service)) {
        let proxy = |p: &Proxy| {
            format!(
                "{}\t{}\t{}",
                if p.enabled { "on" } else { "off" },
                p.server.replace(['\t', '\n'], ""),
                p.port
            )
        };
        out.push_str(&format!(
            "{}\t{}\t{}\n",
            s.service,
            proxy(&s.socks),
            proxy(&s.secure)
        ));
    }
    out
}

pub fn read_saved(text: &str) -> Vec<Saved> {
    text.lines()
        .filter_map(|line| {
            let f: Vec<&str> = line.split('\t').collect();
            if f.len() != 7 || !valid_service(f[0]) {
                return None;
            }
            let proxy = |at: usize| -> Option<Proxy> {
                Some(Proxy {
                    enabled: match f[at] {
                        "on" => true,
                        "off" => false,
                        _ => return None,
                    },
                    server: f[at + 1].to_owned(),
                    port: f[at + 2].parse().ok()?,
                })
            };
            Some(Saved {
                service: f[0].to_owned(),
                socks: proxy(1)?,
                secure: proxy(4)?,
            })
        })
        .collect()
}

/// A word for the shell, between single quotes.
pub fn shell_quote(word: &str) -> String {
    format!("'{}'", word.replace('\'', "'\\''"))
}

/// The AppleScript that runs the commands with an administrator's password, asked once for all of them.
pub fn admin_script(commands: &[Vec<String>]) -> String {
    let line = commands
        .iter()
        .map(|c| {
            std::iter::once(NETWORKSETUP.to_owned())
                .chain(c.iter().map(|a| shell_quote(a)))
                .collect::<Vec<_>>()
                .join(" ")
        })
        .collect::<Vec<_>>()
        .join(" && ");
    let escaped = line.replace('\\', "\\\\").replace('"', "\\\"");
    format!("do shell script \"{escaped}\" with administrator privileges")
}

/// Whether what `networksetup` printed says it failed: it often leaves with success and an error in words.
pub fn failed(output: &str) -> bool {
    let lower = output.to_ascii_lowercase();
    lower.contains("error")
        || lower.contains("requires admin")
        || lower.contains("not a recognized")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_enabled_services_are_listed() {
        let text = "An asterisk (*) denotes that a network service is disabled.\nWi-Fi\n*Bluetooth PAN\nUSB 10/100/1000 LAN\nThunderbolt Bridge\n";
        assert_eq!(
            parse_services(text),
            ["Wi-Fi", "USB 10/100/1000 LAN", "Thunderbolt Bridge"]
        );
    }

    #[test]
    fn a_proxy_setting_is_read() {
        let p = parse_proxy(
            "Enabled: Yes\nServer: 10.0.0.1\nPort: 1080\nAuthenticated Proxy Enabled: 0\n",
        )
        .unwrap();
        assert_eq!(
            p,
            Proxy {
                enabled: true,
                server: "10.0.0.1".into(),
                port: 1080
            }
        );
        let p = parse_proxy("Enabled: No\nServer: \nPort: 0\n").unwrap();
        assert!(!p.enabled && p.server.is_empty());
        assert!(parse_proxy("** Error: The parameters were not valid.").is_none());
        assert!(is_ours(
            &Proxy {
                enabled: true,
                server: "127.0.0.1".into(),
                port: 47117
            },
            47117
        ));
        assert!(!is_ours(
            &Proxy {
                enabled: false,
                server: "127.0.0.1".into(),
                port: 47117
            },
            47117
        ));
    }

    #[test]
    fn every_service_is_pointed_at_the_proxy_and_given_back_what_it_had() {
        let services = vec!["Wi-Fi".to_owned(), "-bad".to_owned()];
        let apply = apply_commands(&services, 47117);
        assert_eq!(
            apply.len(),
            2,
            "a name that reads as an option is left alone"
        );
        assert_eq!(
            apply[0],
            ["-setsocksfirewallproxy", "Wi-Fi", "127.0.0.1", "47117"]
        );
        let saved = vec![Saved {
            service: "Wi-Fi".into(),
            socks: Proxy {
                enabled: true,
                server: "10.0.0.1".into(),
                port: 1080,
            },
            secure: Proxy {
                enabled: false,
                server: String::new(),
                port: 0,
            },
        }];
        let restore = restore_commands(&saved, 47117);
        assert_eq!(
            restore,
            [
                vec!["-setsocksfirewallproxy", "Wi-Fi", "10.0.0.1", "1080"],
                vec!["-setsocksfirewallproxystate", "Wi-Fi", "on"],
                vec!["-setsecurewebproxystate", "Wi-Fi", "off"],
            ]
        );
        let left = vec![Saved {
            service: "Wi-Fi".into(),
            socks: Proxy {
                enabled: true,
                server: "127.0.0.1".into(),
                port: 47117,
            },
            secure: Proxy {
                enabled: true,
                server: "127.0.0.1".into(),
                port: 47117,
            },
        }];
        assert!(
            restore_commands(&left, 47117)
                .iter()
                .all(|c| c.last().unwrap() == "off"),
            "ours is never put back"
        );
    }

    #[test]
    fn the_saved_file_goes_and_comes_back() {
        let saved = vec![Saved {
            service: "USB 10/100/1000 LAN".into(),
            socks: Proxy {
                enabled: false,
                server: String::new(),
                port: 0,
            },
            secure: Proxy {
                enabled: true,
                server: "proxy.corp".into(),
                port: 8443,
            },
        }];
        assert_eq!(read_saved(&write_saved(&saved)), saved);
        assert!(read_saved("Wi-Fi\tmaybe\tx\t1\toff\t\t0\nshort\n").is_empty());
    }

    #[test]
    fn the_password_is_asked_once_for_commands_quoted_for_the_shell() {
        let script = admin_script(&[vec![
            "-setsocksfirewallproxy".into(),
            "Bob's \"Wi-Fi\"".into(),
        ]]);
        assert_eq!(
            script,
            "do shell script \"/usr/sbin/networksetup '-setsocksfirewallproxy' 'Bob'\\\\''s \\\"Wi-Fi\\\"'\" with administrator privileges"
        );
        assert!(failed("** Error: Unable to commit changes") && !failed(""));
    }
}
