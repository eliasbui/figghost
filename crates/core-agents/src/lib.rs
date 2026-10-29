#![forbid(unsafe_code)]
//! Coding agents (Claude Code, Codex, pi) as the notch sees them: what their hooks report, which sessions exist,
//! which commands wait for an answer, how much of the usage limits is left, and how the hooks get installed.

pub mod accounts;
pub mod cli;
pub mod event;
pub mod install;
pub mod server;
pub mod tokens;
pub mod tracker;
pub mod usage;
pub mod wire;

/// The local socket of the hooks. Windows 10 1803 and later have AF_UNIX too, but the standard library does not
/// expose it there.
mod sock {
    #[cfg(unix)]
    pub use std::os::unix::net::{UnixListener, UnixStream};
    #[cfg(windows)]
    pub use uds_windows::{UnixListener, UnixStream};
}

pub use event::{Agent, EventKind, HookEvent, parse_event};
pub use wire::Reply;

use std::path::{Path, PathBuf};

/// A unix socket path must fit in 104 bytes on macOS; a long data directory falls back to a short name in the
/// temporary directory, derived from the same text so the hook and the app always agree.
pub fn socket_path(home: &Path) -> PathBuf {
    short_socket(home, "agents.sock", "figghost")
}

/// The socket of another part of the app, `<name>.sock` in the data directory, with the same way out of a long one.
pub fn named_socket(home: &Path, name: &str) -> PathBuf {
    short_socket(home, &format!("{name}.sock"), &format!("figghost-{name}"))
}

fn short_socket(home: &Path, file: &str, prefix: &str) -> PathBuf {
    const LIMIT: usize = 100;
    let direct = home.join(file);
    if direct.as_os_str().len() <= LIMIT {
        return direct;
    }
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for byte in home.as_os_str().as_encoded_bytes() {
        hash = (hash ^ u64::from(*byte)).wrapping_mul(0x100_0000_01b3);
    }
    std::env::temp_dir().join(format!("{prefix}-{hash:016x}.sock"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_short_home_keeps_the_socket_inside_it() {
        let path = socket_path(Path::new("/Users/me/Library/Application Support/Figghost"));
        assert_eq!(
            path,
            Path::new("/Users/me/Library/Application Support/Figghost/agents.sock")
        );
    }

    #[test]
    fn another_socket_has_its_own_name_inside_the_home_or_elsewhere() {
        let home = Path::new("/Users/me/Library/Application Support/Figghost");
        assert_eq!(named_socket(home, "browser"), home.join("browser.sock"));
        let long = PathBuf::from(format!("/tmp/{}", "x".repeat(120)));
        let browser = named_socket(&long, "browser");
        assert_ne!(browser, socket_path(&long));
        assert!(
            browser
                .file_name()
                .unwrap()
                .to_string_lossy()
                .starts_with("figghost-browser-")
        );
        assert_eq!(browser, named_socket(&long, "browser"));
    }

    #[test]
    fn a_long_home_gets_a_short_stable_name_elsewhere() {
        let long = PathBuf::from(format!("/tmp/{}", "x".repeat(120)));
        let first = socket_path(&long);
        assert!(first.as_os_str().len() <= 100, "{first:?}");
        assert_eq!(first, socket_path(&long));
        assert_ne!(
            first,
            socket_path(&PathBuf::from(format!("/tmp/{}", "y".repeat(120))))
        );
    }
}
