//! Which app made a connection: the process that holds the other end of a socket on 127.0.0.1, and the app that
//! process belongs to. Helpers (`Microsoft Edge Helper (Renderer)`) count for the app whose bundle holds them.

/// An app as the diagram shows it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct App {
    /// The name of its bundle without `.app`, or of its program when it has none.
    pub name: String,
    /// The bundle, for its icon; empty for a program outside one.
    pub bundle: String,
}

/// The app a program at `path` belongs to: the outermost bundle on the path, since a helper sits inside its app.
pub fn app_from_path(path: &str) -> Option<App> {
    let mut at = 0;
    for part in path.split('/') {
        at += part.len() + 1;
        if let Some(stem) = part.strip_suffix(".app")
            && !stem.is_empty()
        {
            return Some(App {
                name: stem.to_owned(),
                bundle: path[..at - 1].to_owned(),
            });
        }
    }
    let name = path.rsplit('/').next().filter(|n| !n.is_empty())?;
    Some(App {
        name: name.to_owned(),
        bundle: String::new(),
    })
}

#[cfg(target_os = "macos")]
mod sys {
    use super::{App, app_from_path};
    use std::{
        ffi::c_void,
        mem::size_of,
        sync::{Mutex, PoisonError},
    };

    const PROC_PIDFDSOCKETINFO: i32 = 3;
    const SOCKINFO_IN: i32 = 1;
    const SOCKINFO_TCP: i32 = 2;
    // Offsets in `struct socket_fdinfo` (sys/proc_info.h): a `proc_fileinfo` of 24 bytes, then `socket_info`, whose
    // `vinfo_stat` takes 136 bytes and whose union of protocols starts at 240; `in_sockinfo` opens with the foreign
    // and the local port, each an int that holds the port in network order.
    const AT_KIND: usize = 24 + 232;
    const AT_FOREIGN_PORT: usize = 24 + 240;
    const AT_LOCAL_PORT: usize = 24 + 240 + 4;
    /// Room for the whole structure, which is under 800 bytes.
    const INFO_BYTES: usize = 2048;

    /// The processes that held a connection of late, looked at first: most connections come from a few apps.
    static RECENT: Mutex<Vec<i32>> = Mutex::new(Vec::new());

    pub fn path_of(pid: i32) -> Option<String> {
        let mut buffer = vec![0u8; libc::PROC_PIDPATHINFO_MAXSIZE as usize];
        // SAFETY: the buffer is as long as the size given.
        let n = unsafe { libc::proc_pidpath(pid, buffer.as_mut_ptr().cast(), buffer.len() as u32) };
        if n <= 0 {
            return None;
        }
        buffer.truncate(n as usize);
        String::from_utf8(buffer).ok()
    }

    pub fn app_of(pid: i32) -> Option<App> {
        app_from_path(&path_of(pid)?)
    }

    fn pids() -> Vec<i32> {
        // SAFETY: a null buffer asks for the count.
        let count = unsafe { libc::proc_listallpids(std::ptr::null_mut(), 0) };
        if count <= 0 {
            return Vec::new();
        }
        let mut pids = vec![0i32; count as usize + 64];
        let bytes = (pids.len() * size_of::<i32>()) as i32;
        // SAFETY: the buffer is as long as the size given.
        let n = unsafe { libc::proc_listallpids(pids.as_mut_ptr().cast(), bytes) };
        pids.truncate(n.max(0) as usize);
        pids.retain(|&p| p > 0);
        pids
    }

    fn sockets(pid: i32) -> Vec<i32> {
        // SAFETY: a null buffer asks for the size.
        let size =
            unsafe { libc::proc_pidinfo(pid, libc::PROC_PIDLISTFDS, 0, std::ptr::null_mut(), 0) };
        if size <= 0 {
            return Vec::new();
        }
        let mut fds = vec![
            libc::proc_fdinfo {
                proc_fd: 0,
                proc_fdtype: 0
            };
            size as usize / size_of::<libc::proc_fdinfo>() + 16
        ];
        let bytes = (fds.len() * size_of::<libc::proc_fdinfo>()) as i32;
        // SAFETY: the buffer is as long as the size given.
        let n = unsafe {
            libc::proc_pidinfo(
                pid,
                libc::PROC_PIDLISTFDS,
                0,
                fds.as_mut_ptr().cast(),
                bytes,
            )
        };
        fds.truncate(n.max(0) as usize / size_of::<libc::proc_fdinfo>());
        fds.iter()
            .filter(|f| f.proc_fdtype == libc::PROX_FDTYPE_SOCKET as u32)
            .map(|f| f.proc_fd)
            .collect()
    }

    /// The local and foreign port of a TCP socket of `pid`.
    fn ports(pid: i32, fd: i32) -> Option<(u16, u16)> {
        let mut info = vec![0u64; INFO_BYTES / 8];
        // SAFETY: the buffer is as long as the size given.
        let n = unsafe {
            libc::proc_pidfdinfo(
                pid,
                fd,
                PROC_PIDFDSOCKETINFO,
                info.as_mut_ptr() as *mut c_void,
                INFO_BYTES as i32,
            )
        };
        if n <= AT_LOCAL_PORT as i32 + 4 {
            return None;
        }
        // SAFETY: the vector holds INFO_BYTES bytes, and n of them were written.
        let bytes = unsafe { std::slice::from_raw_parts(info.as_ptr().cast::<u8>(), INFO_BYTES) };
        let int = |at: usize| i32::from_ne_bytes(bytes[at..at + 4].try_into().unwrap());
        let kind = int(AT_KIND);
        if kind != SOCKINFO_TCP && kind != SOCKINFO_IN {
            return None;
        }
        let port = |at: usize| u16::from_be_bytes([bytes[at], bytes[at + 1]]);
        Some((port(AT_LOCAL_PORT), port(AT_FOREIGN_PORT)))
    }

    fn holds(pid: i32, local: u16, foreign: u16) -> bool {
        sockets(pid)
            .into_iter()
            .any(|fd| ports(pid, fd) == Some((local, foreign)))
    }

    /// The process whose socket goes from port `local` to port `foreign` of this computer, leaving out `except`.
    pub fn pid_of(local: u16, foreign: u16, except: i32) -> Option<i32> {
        let recent = RECENT
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone();
        let found = recent
            .iter()
            .copied()
            .find(|&p| p != except && holds(p, local, foreign))
            .or_else(|| {
                pids()
                    .into_iter()
                    .filter(|p| *p != except && !recent.contains(p))
                    .find(|&p| holds(p, local, foreign))
            })?;
        let mut recent = RECENT.lock().unwrap_or_else(PoisonError::into_inner);
        recent.retain(|&p| p != found);
        recent.insert(0, found);
        recent.truncate(16);
        Some(found)
    }
}

#[cfg(not(target_os = "macos"))]
mod sys {
    use super::App;
    pub fn app_of(_pid: i32) -> Option<App> {
        None
    }
    pub fn pid_of(_local: u16, _foreign: u16, _except: i32) -> Option<i32> {
        None
    }
}

/// The app of a process.
pub fn app_of(pid: i32) -> Option<App> {
    sys::app_of(pid)
}

/// The process that connected from port `local` of 127.0.0.1 to the proxy on port `proxy`. The proxy's own end of the
/// connection has the two ports the other way round, so it is never taken for the client.
pub fn pid_of_client(local: u16, proxy: u16) -> Option<i32> {
    sys::pid_of(local, proxy, 0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_helper_counts_for_the_app_that_holds_it() {
        let edge = "/Applications/Microsoft Edge.app/Contents/Frameworks/Microsoft Edge Framework.framework/Versions/1/Helpers/Microsoft Edge Helper (Renderer).app/Contents/MacOS/Microsoft Edge Helper (Renderer)";
        assert_eq!(
            app_from_path(edge),
            Some(App {
                name: "Microsoft Edge".into(),
                bundle: "/Applications/Microsoft Edge.app".into()
            })
        );
        assert_eq!(
            app_from_path("/usr/bin/curl"),
            Some(App {
                name: "curl".into(),
                bundle: String::new()
            })
        );
        assert_eq!(
            app_from_path("/opt/.app/x"),
            Some(App {
                name: "x".into(),
                bundle: String::new()
            }),
            "a folder named .app alone is no bundle"
        );
        assert_eq!(app_from_path(""), None);
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn the_process_of_a_connection_is_found_by_its_ports() {
        use std::net::{TcpListener, TcpStream};
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let client = TcpStream::connect(listener.local_addr().unwrap()).unwrap();
        let (_server, _) = listener.accept().unwrap();
        let local = client.local_addr().unwrap().port();
        let proxy = listener.local_addr().unwrap().port();
        let me = std::process::id() as i32;
        assert_eq!(
            sys::pid_of(local, proxy, 0),
            Some(me),
            "this test holds the client end"
        );
        assert_eq!(
            sys::pid_of(local, proxy, me),
            None,
            "and leaving it out finds no one"
        );
        assert!(app_of(me).is_some_and(|a| !a.name.is_empty()));
    }
}
