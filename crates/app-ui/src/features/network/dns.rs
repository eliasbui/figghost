//! Encrypted DNS for the whole Mac: a configuration profile (`.mobileconfig`, payload `com.apple.dnsSettings.managed`)
//! that the person opens and approves in System Settings. macOS offers no other way to set it from a program that is
//! not signed by Apple, and it keeps the approval with the person.

/// A choice of provider: its key, and where it listens over HTTPS, with the addresses of its servers so that the
/// system can reach it before it knows its name.
pub struct Provider {
    pub key: &'static str,
    pub url: &'static str,
    pub addresses: &'static [&'static str],
}

pub const PROVIDERS: [Provider; 4] = [
    Provider {
        key: "cloudflare",
        url: "https://cloudflare-dns.com/dns-query",
        addresses: &["1.1.1.1", "1.0.0.1"],
    },
    Provider {
        key: "quad9",
        url: "https://dns.quad9.net/dns-query",
        addresses: &["9.9.9.9", "149.112.112.112"],
    },
    Provider {
        key: "adguard",
        url: "https://dns.adguard-dns.com/dns-query",
        addresses: &["94.140.14.14", "94.140.15.15"],
    },
    Provider {
        key: "google",
        url: "https://dns.google/dns-query",
        addresses: &["8.8.8.8", "8.8.4.4"],
    },
];

/// The profile and its two payloads are named by these, so that installing another replaces the one before.
const PROFILE_ID: &str = "vn.figghost.dns";
const PROFILE_UUID: &str = "6B1F0D4E-3C7A-4E21-9B5A-0F7A2C9D1E01";
const PAYLOAD_UUID: &str = "6B1F0D4E-3C7A-4E21-9B5A-0F7A2C9D1E02";

/// A NextDNS configuration id: letters and digits.
pub fn valid_nextdns(id: &str) -> bool {
    (4..=16).contains(&id.len()) && id.bytes().all(|b| b.is_ascii_alphanumeric())
}

/// An address that may be the server of DNS over HTTPS: `https`, a host of letters, digits, dots and hyphens (and an
/// optional port), and a path of plain characters.
pub fn valid_https_url(url: &str) -> bool {
    let Some(rest) = url.strip_prefix("https://") else {
        return false;
    };
    if url.len() > 200 {
        return false;
    }
    let (authority, path) = rest.split_once('/').unwrap_or((rest, ""));
    let (host, port) = authority
        .split_once(':')
        .map_or((authority, None), |(h, p)| (h, Some(p)));
    let host_ok = !host.is_empty()
        && host.contains('.')
        && !host.starts_with(['.', '-'])
        && !host.ends_with(['.', '-'])
        && host
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'.' || b == b'-');
    let port_ok =
        port.is_none_or(|p| (1..=5).contains(&p.len()) && p.bytes().all(|b| b.is_ascii_digit()));
    let path_ok = path
        .bytes()
        .all(|b| b.is_ascii_alphanumeric() || b"-._~/".contains(&b));
    host_ok && port_ok && path_ok
}

/// What to use for a choice: the server's address and, for a provider, the addresses of its servers.
pub fn resolve(choice: &str, nextdns: &str, custom: &str) -> Option<(String, Vec<&'static str>)> {
    if let Some(p) = PROVIDERS.iter().find(|p| p.key == choice) {
        return Some((p.url.to_owned(), p.addresses.to_vec()));
    }
    match choice {
        "nextdns" if valid_nextdns(nextdns) => {
            Some((format!("https://dns.nextdns.io/{nextdns}"), Vec::new()))
        }
        "custom" if valid_https_url(custom) => Some((custom.to_owned(), Vec::new())),
        _ => None,
    }
}

fn escape(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

/// The text of the profile for a server address and the addresses of its servers.
pub fn mobileconfig(url: &str, addresses: &[&str], name: &str) -> String {
    let servers = if addresses.is_empty() {
        String::new()
    } else {
        format!(
            "\t\t\t\t<key>ServerAddresses</key>\n\t\t\t\t<array>\n{}\t\t\t\t</array>\n",
            addresses
                .iter()
                .map(|a| format!("\t\t\t\t\t<string>{}</string>\n", escape(a)))
                .collect::<String>()
        )
    };
    let name = escape(name);
    format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n\
<!DOCTYPE plist PUBLIC \"-//Apple//DTD PLIST 1.0//EN\" \"http://www.apple.com/DTDs/PropertyList-1.0.dtd\">\n\
<plist version=\"1.0\">\n<dict>\n\
\t<key>PayloadContent</key>\n\t<array>\n\t\t<dict>\n\
\t\t\t<key>PayloadType</key>\n\t\t\t<string>com.apple.dnsSettings.managed</string>\n\
\t\t\t<key>PayloadVersion</key>\n\t\t\t<integer>1</integer>\n\
\t\t\t<key>PayloadIdentifier</key>\n\t\t\t<string>{PROFILE_ID}.settings</string>\n\
\t\t\t<key>PayloadUUID</key>\n\t\t\t<string>{PAYLOAD_UUID}</string>\n\
\t\t\t<key>PayloadDisplayName</key>\n\t\t\t<string>{name}</string>\n\
\t\t\t<key>DNSSettings</key>\n\t\t\t<dict>\n\
\t\t\t\t<key>DNSProtocol</key>\n\t\t\t\t<string>HTTPS</string>\n\
\t\t\t\t<key>ServerURL</key>\n\t\t\t\t<string>{}</string>\n{servers}\t\t\t</dict>\n\
\t\t</dict>\n\t</array>\n\
\t<key>PayloadDisplayName</key>\n\t<string>{name}</string>\n\
\t<key>PayloadIdentifier</key>\n\t<string>{PROFILE_ID}</string>\n\
\t<key>PayloadType</key>\n\t<string>Configuration</string>\n\
\t<key>PayloadUUID</key>\n\t<string>{PROFILE_UUID}</string>\n\
\t<key>PayloadVersion</key>\n\t<integer>1</integer>\n\
</dict>\n</plist>\n",
        escape(url)
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_an_https_address_of_plain_characters_is_a_server() {
        for good in [
            "https://dns.example.com/dns-query",
            "https://dns.example.com:8443/dns-query",
            "https://resolver.example.org",
            "https://dns.nextdns.io/abc123",
        ] {
            assert!(valid_https_url(good), "{good}");
        }
        for bad in [
            "http://dns.example.com/dns-query",
            "https://",
            "https://localhost/dns-query",
            "https://-bad.example.com/x",
            "https://dns.example.com./x",
            "https://dns.example.com:abc/x",
            "https://dns.example.com/dns query",
            "https://dns.example.com/<x>",
            "https://dns.example.com/\"x\"",
            "https://user@dns.example.com/x",
            "https://dns.example.com/x?a=b",
            "ftp://dns.example.com/x",
            "",
        ] {
            assert!(!valid_https_url(bad), "{bad}");
        }
        assert!(!valid_https_url(&format!(
            "https://dns.example.com/{}",
            "a".repeat(200)
        )));
    }

    #[test]
    fn a_nextdns_id_is_letters_and_digits() {
        assert!(valid_nextdns("a1b2c3") && valid_nextdns("ABCD1234abcd1234"));
        for bad in [
            "",
            "abc",
            "a b c d",
            "abc/def",
            "abcdefghijklmnopq",
            "abc-123",
        ] {
            assert!(!valid_nextdns(bad), "{bad}");
        }
    }

    #[test]
    fn a_choice_gives_its_server_and_a_wrong_one_gives_nothing() {
        let (url, addresses) = resolve("quad9", "", "").unwrap();
        assert_eq!(
            (url.as_str(), addresses.len()),
            ("https://dns.quad9.net/dns-query", 2)
        );
        assert_eq!(
            resolve("nextdns", "abc123", "").unwrap().0,
            "https://dns.nextdns.io/abc123"
        );
        assert!(resolve("nextdns", "no good", "").is_none());
        assert_eq!(
            resolve("custom", "", "https://dns.example.com/q")
                .unwrap()
                .0,
            "https://dns.example.com/q"
        );
        assert!(resolve("custom", "", "http://dns.example.com/q").is_none());
        assert!(resolve("unknown", "abc123", "https://dns.example.com/q").is_none());
        assert!(
            PROVIDERS
                .iter()
                .all(|p| valid_https_url(p.url) && resolve(p.key, "", "").is_some())
        );
    }

    #[test]
    fn the_profile_names_the_server_and_escapes_what_it_is_given() {
        let text = mobileconfig(
            "https://dns.example.com/q?a=1&b=2",
            &["9.9.9.9"],
            "Encrypted DNS <Figghost>",
        );
        assert!(text.contains("<string>com.apple.dnsSettings.managed</string>"));
        assert!(text.contains("<string>HTTPS</string>"));
        assert!(text.contains("q?a=1&amp;b=2"), "the ampersand is escaped");
        assert!(text.contains("Encrypted DNS &lt;Figghost&gt;"));
        assert!(
            text.contains("<key>ServerAddresses</key>")
                && text.contains("<string>9.9.9.9</string>")
        );
        assert!(!mobileconfig("https://dns.example.com/q", &[], "x").contains("ServerAddresses"));
        assert!(text.starts_with("<?xml") && text.trim_end().ends_with("</plist>"));
        assert_eq!(
            text.matches("<dict>").count(),
            text.matches("</dict>").count()
        );
        assert_eq!(
            text.matches("<array>").count(),
            text.matches("</array>").count()
        );
    }

    /// `plutil -lint` is the system's own check that a property list is well formed.
    #[test]
    fn the_system_reads_the_profile_as_a_property_list() {
        let dir = std::env::temp_dir().join(format!("figghost-dns-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let file = dir.join("dns.mobileconfig");
        std::fs::write(
            &file,
            mobileconfig(
                "https://dns.quad9.net/dns-query",
                &["9.9.9.9", "149.112.112.112"],
                "Encrypted DNS",
            ),
        )
        .unwrap();
        let out = std::process::Command::new("/usr/bin/plutil")
            .arg("-lint")
            .arg(&file)
            .output()
            .unwrap();
        let said = String::from_utf8_lossy(&out.stdout).into_owned()
            + &String::from_utf8_lossy(&out.stderr);
        let _ = std::fs::remove_dir_all(&dir);
        assert!(out.status.success(), "{said}");
    }
}
