//! What a program asks of the proxy: SOCKS5 (RFC 1928, no password, since the system's own proxy settings give
//! none), SOCKS4 and 4a, or an HTTP CONNECT, which is how the system's secure web proxy is used.

use std::{
    io::{Read, Write},
    net::{IpAddr, Ipv4Addr},
};

/// Where a program asked to go.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Host {
    Ip(IpAddr),
    Name(String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Socks5,
    Socks4,
    Connect,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Request {
    pub kind: Kind,
    pub host: Host,
    pub port: u16,
}

/// The longest header of an HTTP CONNECT that is read.
const MAX_HEADER: usize = 8 * 1024;

/// Reads a request; `None` for anything else, after telling the program so where its protocol has a way to.
pub fn read<S: Read + Write>(stream: &mut S) -> Option<Request> {
    let [first] = array(stream)?;
    match first {
        5 => socks5(stream),
        4 => socks4(stream),
        b'C' => connect(stream),
        _ => None,
    }
}

/// Tells the program whether the connection to the site is made.
pub fn answer<S: Write>(stream: &mut S, kind: Kind, ok: bool) -> bool {
    let bytes: &[u8] = match (kind, ok) {
        (Kind::Socks5, true) => &[5, 0, 0, 1, 0, 0, 0, 0, 0, 0],
        (Kind::Socks5, false) => &[5, 4, 0, 1, 0, 0, 0, 0, 0, 0],
        (Kind::Socks4, true) => &[0, 0x5A, 0, 0, 0, 0, 0, 0],
        (Kind::Socks4, false) => &[0, 0x5B, 0, 0, 0, 0, 0, 0],
        (Kind::Connect, true) => b"HTTP/1.1 200 Connection Established\r\n\r\n",
        (Kind::Connect, false) => {
            b"HTTP/1.1 502 Bad Gateway\r\nContent-Length: 0\r\nConnection: close\r\n\r\n"
        }
    };
    stream
        .write_all(bytes)
        .and_then(|()| stream.flush())
        .is_ok()
}

fn array<const N: usize, S: Read>(from: &mut S) -> Option<[u8; N]> {
    let mut bytes = [0; N];
    from.read_exact(&mut bytes).ok()?;
    Some(bytes)
}

fn vec<S: Read>(from: &mut S, length: usize) -> Option<Vec<u8>> {
    let mut bytes = vec![0; length];
    from.read_exact(&mut bytes).ok()?;
    Some(bytes)
}

fn name(bytes: Vec<u8>) -> Option<Host> {
    let name = String::from_utf8(bytes).ok()?;
    if name.is_empty() || name.len() > 253 || name.bytes().any(|b| b <= b' ' || b == 0x7F) {
        return None;
    }
    Some(match name.parse::<IpAddr>() {
        Ok(ip) => Host::Ip(ip),
        Err(_) => Host::Name(name),
    })
}

fn socks5<S: Read + Write>(s: &mut S) -> Option<Request> {
    let [count] = array(s)?;
    let methods = vec(s, count.into())?;
    if !methods.contains(&0) {
        let _ = s.write_all(&[5, 0xFF]);
        return None;
    }
    s.write_all(&[5, 0]).ok()?;
    let [version, command, _, kind] = array(s)?;
    if version != 5 {
        return None;
    }
    let host = match kind {
        1 => Host::Ip(IpAddr::from(array::<4, _>(s)?)),
        3 => {
            let [length] = array(s)?;
            name(vec(s, length.into())?)?
        }
        4 => Host::Ip(IpAddr::from(array::<16, _>(s)?)),
        _ => {
            let _ = s.write_all(&[5, 8, 0, 1, 0, 0, 0, 0, 0, 0]);
            return None;
        }
    };
    let port = u16::from_be_bytes(array(s)?);
    if command != 1 {
        let _ = s.write_all(&[5, 7, 0, 1, 0, 0, 0, 0, 0, 0]);
        return None;
    }
    Some(Request {
        kind: Kind::Socks5,
        host,
        port,
    })
}

/// A string ended by a zero byte, at most 255 bytes.
fn zero_ended<S: Read>(s: &mut S) -> Option<Vec<u8>> {
    let mut out = Vec::new();
    loop {
        let [b] = array(s)?;
        if b == 0 {
            return Some(out);
        }
        if out.len() == 255 {
            return None;
        }
        out.push(b);
    }
}

fn socks4<S: Read + Write>(s: &mut S) -> Option<Request> {
    let [command] = array(s)?;
    let port = u16::from_be_bytes(array(s)?);
    let ip = Ipv4Addr::from(array::<4, _>(s)?);
    zero_ended(s)?;
    if command != 1 {
        let _ = answer(s, Kind::Socks4, false);
        return None;
    }
    // 4a: an address 0.0.0.x with x not 0 says that the name of the site follows.
    let octets = ip.octets();
    let host = if octets[..3] == [0, 0, 0] && octets[3] != 0 {
        name(zero_ended(s)?)?
    } else {
        Host::Ip(IpAddr::V4(ip))
    };
    Some(Request {
        kind: Kind::Socks4,
        host,
        port,
    })
}

fn connect<S: Read + Write>(s: &mut S) -> Option<Request> {
    let mut head = vec![b'C'];
    while !head.ends_with(b"\r\n\r\n") {
        if head.len() >= MAX_HEADER {
            return None;
        }
        let [b] = array(s)?;
        head.push(b);
    }
    let text = std::str::from_utf8(&head).ok()?;
    let line = text.lines().next()?;
    let mut parts = line.split(' ');
    let (method, target, version) = (parts.next()?, parts.next()?, parts.next()?);
    if method != "CONNECT" || !version.starts_with("HTTP/1.") || parts.next().is_some() {
        let _ = answer(s, Kind::Connect, false);
        return None;
    }
    let (host, port) = target.rsplit_once(':')?;
    let host = host.trim_start_matches('[').trim_end_matches(']');
    let port: u16 = port.parse().ok().filter(|p| *p != 0)?;
    Some(Request {
        kind: Kind::Connect,
        host: name(host.as_bytes().to_vec())?,
        port,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    /// A stream that reads what is given and keeps what is written.
    struct Duplex {
        input: Cursor<Vec<u8>>,
        output: Vec<u8>,
    }

    impl Read for Duplex {
        fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
            self.input.read(buf)
        }
    }

    impl Write for Duplex {
        fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
            self.output.extend_from_slice(buf);
            Ok(buf.len())
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }

    fn duplex(bytes: &[u8]) -> Duplex {
        Duplex {
            input: Cursor::new(bytes.to_vec()),
            output: Vec::new(),
        }
    }

    #[test]
    fn socks5_by_name_and_by_address() {
        let mut s = duplex(&[
            5, 1, 0, 5, 1, 0, 3, 11, b'e', b'x', b'a', b'm', b'p', b'l', b'e', b'.', b'c', b'o',
            b'm', 1, 187,
        ]);
        let r = read(&mut s).unwrap();
        assert_eq!(
            r,
            Request {
                kind: Kind::Socks5,
                host: Host::Name("example.com".into()),
                port: 443
            }
        );
        assert_eq!(s.output, [5, 0]);
        let mut s = duplex(&[5, 2, 2, 0, 5, 1, 0, 1, 1, 1, 1, 1, 0, 80]);
        assert_eq!(
            read(&mut s).unwrap().host,
            Host::Ip("1.1.1.1".parse().unwrap())
        );
    }

    #[test]
    fn socks5_without_the_open_method_or_with_another_command_is_refused() {
        let mut s = duplex(&[5, 1, 2]);
        assert!(read(&mut s).is_none());
        assert_eq!(s.output, [5, 0xFF]);
        let mut s = duplex(&[5, 1, 0, 5, 3, 0, 1, 1, 1, 1, 1, 0, 53]);
        assert!(read(&mut s).is_none(), "UDP ASSOCIATE is not offered");
        assert_eq!(s.output[2..4], [5, 7]);
    }

    #[test]
    fn socks4_and_4a() {
        let mut s = duplex(&[4, 1, 0, 80, 93, 184, 216, 34, b'u', 0]);
        assert_eq!(
            read(&mut s).unwrap().host,
            Host::Ip("93.184.216.34".parse().unwrap())
        );
        let mut s = duplex(&[4, 1, 1, 187, 0, 0, 0, 1, 0, b'a', b'.', b'v', b'n', 0]);
        let r = read(&mut s).unwrap();
        assert_eq!(
            (r.kind, r.host, r.port),
            (Kind::Socks4, Host::Name("a.vn".into()), 443)
        );
    }

    #[test]
    fn an_http_connect_and_what_is_not_one() {
        let mut s = duplex(b"CONNECT github.com:443 HTTP/1.1\r\nHost: github.com:443\r\n\r\n");
        let r = read(&mut s).unwrap();
        assert_eq!(
            (r.kind, r.host, r.port),
            (Kind::Connect, Host::Name("github.com".into()), 443)
        );
        let mut s = duplex(b"CONNECT [2001:db8::1]:443 HTTP/1.1\r\n\r\n");
        assert_eq!(
            read(&mut s).unwrap().host,
            Host::Ip("2001:db8::1".parse().unwrap())
        );
        let mut s = duplex(b"COPY / HTTP/1.1\r\n\r\n");
        assert!(read(&mut s).is_none());
        assert!(s.output.starts_with(b"HTTP/1.1 502"));
        let mut s = duplex(b"CONNECT a b:443 HTTP/1.1\r\n\r\n");
        assert!(read(&mut s).is_none(), "a space in the target");
        let mut long = b"CONNECT a:1 HTTP/1.1\r\n".to_vec();
        long.extend(std::iter::repeat_n(b'x', MAX_HEADER));
        assert!(
            read(&mut duplex(&long)).is_none(),
            "a header that never ends"
        );
        assert!(read(&mut duplex(b"GET / HTTP/1.1\r\n\r\n")).is_none());
    }
}
