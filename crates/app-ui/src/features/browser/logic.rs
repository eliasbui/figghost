//! The rules of the browser bridge with nothing of the system in them: the browsers it knows and where each looks for
//! a native messaging host, the file that names the host, the sites to block as the person wrote them, the frames of
//! native messaging, and the one-line questions the host asks the app.

use serde_json::{Value, json};
use std::{
    io::{self, Read, Write},
    path::{Path, PathBuf},
};

/// The name the extension asks Chrome for, and the name of the file that tells Chrome where the host is.
pub const HOST_NAME: &str = "vn.figghost.browser";
/// The extension's ID, fixed by the public key in its manifest, so that the file can name it before it is loaded.
pub const EXTENSION_ID: &str = "gaolignoeegofacmdaebpionmopdfpfn";
/// The most the extension may send in one message; it only ever sends an address.
pub const MAX_MESSAGE: usize = 64 * 1024;
/// The most sites that are blocked.
pub const MAX_SITES: usize = 200;
/// What a new list holds: the usual places a work session goes to.
pub const DEFAULT_SITES: [&str; 6] = [
    "youtube.com",
    "facebook.com",
    "tiktok.com",
    "instagram.com",
    "x.com",
    "reddit.com",
];

/// The browsers built on Chromium that read a host from a folder of their own in `~/Library/Application Support`: an
/// id, the name people know, and that folder. Only Chrome's and Chromium's are written in Google's documentation.
pub const BROWSERS: [(&str, &str, &str); 6] = [
    ("chrome", "Google Chrome", "Google/Chrome"),
    ("edge", "Microsoft Edge", "Microsoft Edge"),
    ("brave", "Brave", "BraveSoftware/Brave-Browser"),
    ("vivaldi", "Vivaldi", "Vivaldi"),
    ("arc", "Arc", "Arc/User Data"),
    ("chromium", "Chromium", "Chromium"),
];

/// The folder of a browser, under the person's `~/Library/Application Support`.
pub fn browser_dir(support: &Path, id: &str) -> Option<PathBuf> {
    BROWSERS
        .iter()
        .find(|b| b.0 == id)
        .map(|b| support.join(b.2))
}

/// Where that browser looks for the file of this host.
pub fn manifest_path(browser_dir: &Path) -> PathBuf {
    browser_dir
        .join("NativeMessagingHosts")
        .join(format!("{HOST_NAME}.json"))
}

/// The file that tells a browser which program to start for the extension: this one, for the extension alone.
pub fn host_manifest(program: &Path) -> String {
    let text = serde_json::to_string_pretty(&json!({
        "name": HOST_NAME,
        "description": "Figghost: the bridge of the Figghost Companion extension",
        "path": program,
        "type": "stdio",
        "allowed_origins": [format!("chrome-extension://{EXTENSION_ID}/")],
    }))
    .unwrap_or_default();
    text + "\n"
}

/// Whether a file found where the host's goes is ours and starts this program.
pub fn names_program(text: &str, program: &Path) -> bool {
    serde_json::from_str::<Value>(text)
        .is_ok_and(|v| v["name"] == HOST_NAME && v["path"].as_str() == program.to_str())
}

/// Whether Chrome started this program for the extension: its first argument is the extension's origin.
pub fn is_our_origin(argument: &str) -> bool {
    argument.trim_end_matches('/') == format!("chrome-extension://{EXTENSION_ID}")
}

/// The sites as the person wrote them, one a line or with commas, made into hosts (`https://www.YouTube.com/x` is
/// `youtube.com`); anything that is not a site, and any site twice, is left out, and the list stops at `MAX_SITES`.
pub fn sites(text: &str) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for part in text.split(['\n', ',', ' ', '\t']) {
        let part = part.trim();
        if part.is_empty() {
            continue;
        }
        let url = if part.contains("://") {
            part.to_owned()
        } else {
            format!("https://{part}")
        };
        if let Some(site) = macos_sys::browser::site_of(&url)
            && site.contains('.')
            && !out.contains(&site)
        {
            out.push(site);
        }
        if out.len() == MAX_SITES {
            break;
        }
    }
    out
}

/// Reads one message of native messaging: four bytes of length in the machine's order, then that much JSON. `None` at
/// the end of the input; an error for a message that is too long or is not JSON.
pub fn read_message(input: &mut impl Read) -> io::Result<Option<Value>> {
    let mut length = [0u8; 4];
    match input.read_exact(&mut length) {
        Ok(()) => {}
        Err(e) if e.kind() == io::ErrorKind::UnexpectedEof => return Ok(None),
        Err(e) => return Err(e),
    }
    let length = u32::from_ne_bytes(length) as usize;
    if length > MAX_MESSAGE {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "message too long",
        ));
    }
    let mut body = vec![0u8; length];
    input.read_exact(&mut body)?;
    serde_json::from_slice(&body)
        .map(Some)
        .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))
}

/// Writes one message of native messaging and sends it on at once.
pub fn write_message(output: &mut impl Write, message: &Value) -> io::Result<()> {
    let body = serde_json::to_vec(message)?;
    let length = u32::try_from(body.len())
        .map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "message too long"))?;
    output.write_all(&length.to_ne_bytes())?;
    output.write_all(&body)?;
    output.flush()
}

/// What the host may ask the app, one JSON line each way over the local socket.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Ask {
    /// What to block now.
    State,
    /// Download the video at this address.
    Download(String),
}

impl Ask {
    pub fn line(&self) -> String {
        match self {
            Ask::State => json!({ "ask": "state" }),
            Ask::Download(url) => json!({ "ask": "download", "url": url }),
        }
        .to_string()
    }

    /// A line from the socket, which anything of this user can write to: only the two questions, and an address of
    /// http or https that is not too long.
    pub fn parse(line: &str) -> Option<Self> {
        let value: Value = serde_json::from_str(line.trim()).ok()?;
        match value["ask"].as_str()? {
            "state" => Some(Ask::State),
            "download" => {
                let url = value["url"].as_str()?.trim();
                let lower = url.to_ascii_lowercase();
                let web = lower.starts_with("https://") || lower.starts_with("http://");
                (web && url.len() <= 4096 && !url.chars().any(char::is_control))
                    .then(|| Ask::Download(url.to_owned()))
            }
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    #[test]
    fn each_browser_has_its_folder_and_the_file_names_this_program_for_this_extension() {
        let support = Path::new("/Users/me/Library/Application Support");
        let edge = browser_dir(support, "edge").unwrap();
        assert_eq!(
            manifest_path(&edge),
            Path::new(
                "/Users/me/Library/Application Support/Microsoft Edge/NativeMessagingHosts/vn.figghost.browser.json"
            )
        );
        assert!(browser_dir(support, "firefox").is_none());
        let program = Path::new("/Applications/Figghost.app/Contents/MacOS/Figghost");
        let text = host_manifest(program);
        let value: Value = serde_json::from_str(&text).unwrap();
        assert_eq!(value["type"], "stdio");
        assert_eq!(
            value["allowed_origins"],
            json!(["chrome-extension://gaolignoeegofacmdaebpionmopdfpfn/"])
        );
        assert!(names_program(&text, program));
        assert!(!names_program(&text, Path::new("/tmp/other")));
        assert!(!names_program("not json", program));
    }

    #[test]
    fn only_the_origin_of_this_extension_is_ours() {
        assert!(is_our_origin(
            "chrome-extension://gaolignoeegofacmdaebpionmopdfpfn/"
        ));
        assert!(is_our_origin(
            "chrome-extension://gaolignoeegofacmdaebpionmopdfpfn"
        ));
        assert!(!is_our_origin(
            "chrome-extension://aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa/"
        ));
        assert!(!is_our_origin("--figghost-hook"));
    }

    #[test]
    fn sites_are_hosts_once_each_whatever_way_they_were_written() {
        assert_eq!(
            sites(
                "youtube.com\nhttps://www.Facebook.com/feed, x.com\n\n  YOUTUBE.com  \nnot a site\nlocalhost\nfile:///etc"
            ),
            ["youtube.com", "facebook.com", "x.com"]
        );
        let many: String = (0..500).map(|i| format!("s{i}.com\n")).collect();
        assert_eq!(sites(&many).len(), MAX_SITES);
        assert!(sites("").is_empty());
    }

    #[test]
    fn a_message_goes_through_the_frames_and_back() {
        let mut buffer = Vec::new();
        write_message(&mut buffer, &json!({ "type": "state", "block": true })).unwrap();
        write_message(&mut buffer, &json!({ "type": "download", "ok": false })).unwrap();
        let mut input = Cursor::new(buffer);
        assert_eq!(
            read_message(&mut input).unwrap(),
            Some(json!({ "type": "state", "block": true }))
        );
        assert_eq!(
            read_message(&mut input).unwrap().unwrap()["type"],
            "download"
        );
        assert_eq!(
            read_message(&mut input).unwrap(),
            None,
            "the end of the input"
        );
    }

    #[test]
    fn a_message_too_long_or_not_json_is_refused() {
        let mut long = (MAX_MESSAGE as u32 + 1).to_ne_bytes().to_vec();
        long.extend(vec![b' '; 16]);
        assert!(read_message(&mut Cursor::new(long)).is_err());
        let mut bad = 3u32.to_ne_bytes().to_vec();
        bad.extend(b"{x}");
        assert!(read_message(&mut Cursor::new(bad)).is_err());
        let mut cut = 10u32.to_ne_bytes().to_vec();
        cut.extend(b"{}");
        assert!(read_message(&mut Cursor::new(cut)).is_err());
    }

    #[test]
    fn only_the_two_questions_and_a_web_address_are_taken_from_the_socket() {
        assert_eq!(Ask::parse(&Ask::State.line()), Some(Ask::State));
        let download = Ask::Download("https://www.youtube.com/watch?v=1".into());
        assert_eq!(Ask::parse(&download.line()), Some(download));
        for bad in [
            r#"{"ask":"download","url":"file:///etc/passwd"}"#,
            r#"{"ask":"download","url":"-o /tmp/x"}"#,
            r#"{"ask":"download","url":"https://a.com/\nx"}"#,
            r#"{"ask":"download"}"#,
            r#"{"ask":"quit"}"#,
            "not json",
            "",
        ] {
            assert!(Ask::parse(bad).is_none(), "{bad}");
        }
        let long = format!(
            r#"{{"ask":"download","url":"https://a.com/{}"}}"#,
            "x".repeat(5000)
        );
        assert!(Ask::parse(&long).is_none());
    }
}
