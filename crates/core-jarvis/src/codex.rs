//! The ChatGPT plan through Codex, signed in by Figghost itself: the browser opens OpenAI's sign-in page for the
//! public OAuth client of the Codex command line (PKCE, a redirect to `localhost:1455`), and the tokens that come
//! back are Figghost's own, kept by the app and renewed by it. The Codex program on this Mac is never read or used.
//! The same route as app-card-english, OpenCode and ChatMock; not an official one for other programs, and OpenAI may
//! change it at any time.

use core_smart::net::quote;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{
    collections::HashMap,
    io::{Read, Write},
    net::{TcpListener, TcpStream},
    process::{Command, Stdio},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    thread,
    time::{Duration, Instant},
};

pub const CLIENT_ID: &str = "app_EMoamEEZ73f0CkXaXp7hrann";
const AUTHORIZE: &str = "https://auth.openai.com/oauth/authorize";
pub const TOKEN: &str = "https://auth.openai.com/oauth/token";
const ISSUER: &str = "https://auth.openai.com";
/// The Codex client accepts this redirect only, so the port cannot be another.
pub const PORT: u16 = 1455;
const REDIRECT: &str = "http://localhost:1455/auth/callback";
const SCOPE: &str = "openid profile email offline_access";
pub const RESPONSES: &str = "https://chatgpt.com/backend-api/codex/responses";
pub const MODELS: &str = "https://chatgpt.com/backend-api/codex/models?client_version=0.160.0";
/// Who is calling, said plainly instead of passing for the Codex command line.
pub const ORIGINATOR: &str = "figghost";
/// An access token is renewed this long before it runs out.
const REFRESH_MARGIN: i64 = 120;
/// How long the browser is waited for.
pub const SIGN_IN_SECONDS: u64 = 300;
/// What is asked when the account lists no model.
pub const FALLBACK_MODEL: &str = "gpt-5";

/// Raised from any thread to stop a sign-in that waits for the browser.
pub type Cancel = Arc<AtomicBool>;

/// The signed-in account, as the app keeps it (in the Keychain). Signing out forgets all of it.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(default)]
pub struct Session {
    pub email: String,
    pub account_id: String,
    /// `free`, `plus`, `pro`, `team`...
    pub plan: String,
    pub access_token: String,
    pub refresh_token: String,
    /// Unix seconds.
    pub expires_at: i64,
    /// The model the account lists first, read at sign-in.
    pub model: String,
}

impl Session {
    pub fn signed_in(&self) -> bool {
        !self.access_token.is_empty() || !self.refresh_token.is_empty()
    }

    pub fn to_json(&self) -> String {
        serde_json::to_string(self).unwrap_or_default()
    }

    pub fn from_json(text: &[u8]) -> Option<Session> {
        serde_json::from_slice(text).ok()
    }

    /// Whether the access token has to be renewed before it is used at `now`.
    pub fn due(&self, now: i64) -> bool {
        self.access_token.is_empty() || self.expires_at - REFRESH_MARGIN <= now
    }
}

/// Why there is no usable sign-in.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SignIn {
    /// Never signed in, or signed out.
    Missing,
    /// The session ended and could not be renewed: sign in again.
    Expired,
    /// The renewal could not reach OpenAI; the session is kept for the next try.
    Unreadable,
}

// Encoding.

const B64: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_";

/// base64url without padding.
pub fn base64url(bytes: &[u8]) -> String {
    let mut out = String::new();
    for chunk in bytes.chunks(3) {
        let n = chunk.iter().enumerate().fold(0u32, |n, (i, b)| n | (u32::from(*b) << (16 - 8 * i)));
        for i in 0..=chunk.len() {
            out.push(B64[((n >> (18 - 6 * i)) & 63) as usize] as char);
        }
    }
    out
}

fn base64url_decode(text: &str) -> Option<Vec<u8>> {
    let mut bits = 0u32;
    let mut held = 0u32;
    let mut out = Vec::new();
    for c in text.trim_end_matches('=').bytes() {
        let v = match c {
            b'A'..=b'Z' => c - b'A',
            b'a'..=b'z' => c - b'a' + 26,
            b'0'..=b'9' => c - b'0' + 52,
            b'-' | b'+' => 62,
            b'_' | b'/' => 63,
            _ => return None,
        };
        bits = (bits << 6) | u32::from(v);
        held += 6;
        if held >= 8 {
            held -= 8;
            out.push((bits >> held) as u8);
            bits &= (1 << held) - 1;
        }
    }
    Some(out)
}

/// SHA-256 (FIPS 180-4), for the PKCE challenge; no crate in the workspace has it.
pub fn sha256(data: &[u8]) -> [u8; 32] {
    const K: [u32; 64] = [
        0x428a2f98, 0x71374491, 0xb5c0fbcf, 0xe9b5dba5, 0x3956c25b, 0x59f111f1, 0x923f82a4, 0xab1c5ed5, 0xd807aa98,
        0x12835b01, 0x243185be, 0x550c7dc3, 0x72be5d74, 0x80deb1fe, 0x9bdc06a7, 0xc19bf174, 0xe49b69c1, 0xefbe4786,
        0x0fc19dc6, 0x240ca1cc, 0x2de92c6f, 0x4a7484aa, 0x5cb0a9dc, 0x76f988da, 0x983e5152, 0xa831c66d, 0xb00327c8,
        0xbf597fc7, 0xc6e00bf3, 0xd5a79147, 0x06ca6351, 0x14292967, 0x27b70a85, 0x2e1b2138, 0x4d2c6dfc, 0x53380d13,
        0x650a7354, 0x766a0abb, 0x81c2c92e, 0x92722c85, 0xa2bfe8a1, 0xa81a664b, 0xc24b8b70, 0xc76c51a3, 0xd192e819,
        0xd6990624, 0xf40e3585, 0x106aa070, 0x19a4c116, 0x1e376c08, 0x2748774c, 0x34b0bcb5, 0x391c0cb3, 0x4ed8aa4a,
        0x5b9cca4f, 0x682e6ff3, 0x748f82ee, 0x78a5636f, 0x84c87814, 0x8cc70208, 0x90befffa, 0xa4506ceb, 0xbef9a3f7,
        0xc67178f2,
    ];
    let mut h: [u32; 8] =
        [0x6a09e667, 0xbb67ae85, 0x3c6ef372, 0xa54ff53a, 0x510e527f, 0x9b05688c, 0x1f83d9ab, 0x5be0cd19];
    let mut message = data.to_vec();
    message.push(0x80);
    while message.len() % 64 != 56 {
        message.push(0);
    }
    message.extend_from_slice(&((data.len() as u64) * 8).to_be_bytes());
    for block in message.chunks(64) {
        let mut w = [0u32; 64];
        for (i, word) in block.chunks(4).enumerate() {
            w[i] = u32::from_be_bytes([word[0], word[1], word[2], word[3]]);
        }
        for i in 16..64 {
            let s0 = w[i - 15].rotate_right(7) ^ w[i - 15].rotate_right(18) ^ (w[i - 15] >> 3);
            let s1 = w[i - 2].rotate_right(17) ^ w[i - 2].rotate_right(19) ^ (w[i - 2] >> 10);
            w[i] = w[i - 16].wrapping_add(s0).wrapping_add(w[i - 7]).wrapping_add(s1);
        }
        let mut v = h;
        for i in 0..64 {
            let s1 = v[4].rotate_right(6) ^ v[4].rotate_right(11) ^ v[4].rotate_right(25);
            let ch = (v[4] & v[5]) ^ (!v[4] & v[6]);
            let t1 = v[7].wrapping_add(s1).wrapping_add(ch).wrapping_add(K[i]).wrapping_add(w[i]);
            let s0 = v[0].rotate_right(2) ^ v[0].rotate_right(13) ^ v[0].rotate_right(22);
            let maj = (v[0] & v[1]) ^ (v[0] & v[2]) ^ (v[1] & v[2]);
            let t2 = s0.wrapping_add(maj);
            v = [t1.wrapping_add(t2), v[0], v[1], v[2], v[3].wrapping_add(t1), v[4], v[5], v[6]];
        }
        for (a, b) in h.iter_mut().zip(v) {
            *a = a.wrapping_add(b);
        }
    }
    let mut out = [0u8; 32];
    for (i, word) in h.iter().enumerate() {
        out[4 * i..4 * i + 4].copy_from_slice(&word.to_be_bytes());
    }
    out
}

/// 32 random bytes from the system, as base64url: the PKCE verifier and the state.
pub fn random_token() -> Result<String, String> {
    let mut bytes = [0u8; 32];
    std::fs::File::open("/dev/urandom")
        .and_then(|mut f| f.read_exact(&mut bytes))
        .map_err(|e| format!("no random bytes: {e}"))?;
    Ok(base64url(&bytes))
}

/// The PKCE challenge of a verifier (S256).
pub fn challenge(verifier: &str) -> String {
    base64url(&sha256(verifier.as_bytes()))
}

fn percent_encode(text: &str) -> String {
    let mut out = String::new();
    for b in text.bytes() {
        if b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'.' | b'~') {
            out.push(b as char);
        } else {
            out.push_str(&format!("%{b:02X}"));
        }
    }
    out
}

fn percent_decode(text: &str) -> String {
    let bytes = text.as_bytes();
    let mut out = Vec::new();
    let mut i = 0;
    while i < bytes.len() {
        match bytes[i] {
            b'+' => out.push(b' '),
            b'%' if i + 2 < bytes.len() => {
                match std::str::from_utf8(&bytes[i + 1..i + 3]).ok().and_then(|h| u8::from_str_radix(h, 16).ok()).ok_or(()) {
                    Ok(v) => {
                        out.push(v);
                        i += 2;
                    }
                    Err(_) => out.push(b'%'),
                }
            }
            b => out.push(b),
        }
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

pub fn form(pairs: &[(&str, &str)]) -> String {
    pairs.iter().map(|(k, v)| format!("{}={}", percent_encode(k), percent_encode(v))).collect::<Vec<_>>().join("&")
}

fn parse_query(query: &str) -> HashMap<String, String> {
    query
        .split('&')
        .filter_map(|pair| {
            let (k, v) = pair.split_once('=').unwrap_or((pair, ""));
            (!k.is_empty()).then(|| (percent_decode(k), percent_decode(v)))
        })
        .collect()
}

/// The address of OpenAI's sign-in page.
pub fn authorize_url(state: &str, challenge: &str) -> String {
    let params = [
        ("response_type", "code"),
        ("client_id", CLIENT_ID),
        ("redirect_uri", REDIRECT),
        ("scope", SCOPE),
        ("code_challenge", challenge),
        ("code_challenge_method", "S256"),
        ("id_token_add_organizations", "true"),
        ("codex_cli_simplified_flow", "true"),
        ("state", state),
        ("originator", ORIGINATOR),
    ];
    format!("{AUTHORIZE}?{}", form(&params))
}

/// What the browser brought back to the callback: the code, or why there is none. `None` for a request that is not
/// the callback (a favicon) or carries another state, which is never taken for an answer.
pub fn read_callback(request_line: &str, state: &str) -> Option<Result<String, String>> {
    let mut parts = request_line.split(' ');
    let (method, target) = (parts.next()?, parts.next()?);
    let (path, query) = target.split_once('?').unwrap_or((target, ""));
    if method != "GET" || path != "/auth/callback" {
        return None;
    }
    let params = parse_query(query);
    if params.get("state").map(String::as_str) != Some(state) {
        return None;
    }
    if let Some(error) = params.get("error") {
        return Some(Err(error.clone()));
    }
    match params.get("code").filter(|c| !c.is_empty()) {
        Some(code) => Some(Ok(code.clone())),
        None => Some(Err("no code".into())),
    }
}

fn answer_browser(stream: &mut TcpStream, ok: bool) {
    let (status, text) = if ok {
        ("200 OK", "Figghost is signed in to ChatGPT. You can close this tab. / Figghost đã đăng nhập ChatGPT, bạn có thể đóng tab này.")
    } else {
        ("400 Bad Request", "The sign-in did not finish. Go back to Figghost and try again. / Đăng nhập chưa xong, hãy quay lại Figghost và thử lại.")
    };
    let body = format!("<!doctype html><meta charset=utf-8><title>Figghost</title><body style=\"font:16px -apple-system;margin:4em\"><p>{text}</p>");
    let _ = write!(
        stream,
        "HTTP/1.1 {status}\r\nContent-Type: text/html; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
        body.len()
    );
}

fn request_line(stream: &mut TcpStream) -> Option<String> {
    stream.set_read_timeout(Some(Duration::from_secs(5))).ok()?;
    let mut head = Vec::new();
    let mut byte = [0u8; 1];
    while !head.ends_with(b"\r\n\r\n") && head.len() < 8192 {
        if stream.read(&mut byte).ok()? == 0 {
            break;
        }
        head.push(byte[0]);
    }
    let text = String::from_utf8_lossy(&head).into_owned();
    text.lines().next().map(str::to_owned)
}

/// Waits on `listeners` for the browser to come back with `state`, until `seconds` pass or `cancel` is raised.
pub fn wait_for_code(listeners: &[TcpListener], state: &str, seconds: u64, cancel: &Cancel) -> Result<String, String> {
    for l in listeners {
        l.set_nonblocking(true).map_err(|e| e.to_string())?;
    }
    let deadline = Instant::now() + Duration::from_secs(seconds);
    while Instant::now() < deadline {
        if cancel.load(Ordering::SeqCst) {
            return Err("cancelled".into());
        }
        for l in listeners {
            let Ok((mut stream, _)) = l.accept() else { continue };
            let _ = stream.set_nonblocking(false);
            let Some(line) = request_line(&mut stream) else { continue };
            match read_callback(&line, state) {
                Some(result) => {
                    answer_browser(&mut stream, result.is_ok());
                    return result;
                }
                None => {
                    let _ = write!(stream, "HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\nConnection: close\r\n\r\n");
                }
            }
        }
        thread::sleep(Duration::from_millis(50));
    }
    Err("timed out".into())
}

// Tokens.

/// The payload of a JWT, unchecked: the expiry and the account are read from it, and the server checks the token
/// on every request.
pub fn claims(token: &str) -> Option<Value> {
    serde_json::from_slice(&base64url_decode(token.split('.').nth(1)?)?).ok()
}

/// A session from a token response. `base` is the session being renewed, whose fields stay when a refresh does not
/// repeat them.
pub fn from_token(token: &Value, base: Session, now: i64) -> Result<Session, String> {
    let text = |key: &str| token[key].as_str().unwrap_or_default().to_owned();
    let mut s = base;
    s.access_token = text("access_token");
    if s.access_token.is_empty() {
        return Err("no access token".into());
    }
    let refresh = text("refresh_token");
    if !refresh.is_empty() {
        s.refresh_token = refresh;
    }
    let access = claims(&s.access_token).unwrap_or_default();
    s.expires_at = access["exp"].as_i64().or(token["expires_in"].as_i64().map(|e| now + e)).unwrap_or(now + 3600);
    let id_token = text("id_token");
    if !id_token.is_empty() {
        let id = claims(&id_token).ok_or("an id token that cannot be read")?;
        let audience = match &id["aud"] {
            Value::String(aud) => aud == CLIENT_ID,
            Value::Array(list) => list.iter().any(|a| a == CLIENT_ID),
            _ => false,
        };
        if id["iss"] != ISSUER || !audience {
            return Err("an id token for someone else".into());
        }
        let auth = &id["https://api.openai.com/auth"];
        let account = auth["chatgpt_account_id"].as_str().unwrap_or_default();
        if !s.account_id.is_empty() && !account.is_empty() && account != s.account_id {
            return Err("the sign-in changed to another account".into());
        }
        if !account.is_empty() {
            s.account_id = account.to_owned();
        }
        if let Some(email) = id["email"].as_str() {
            s.email = email.to_owned();
        }
        let plan =
            auth["chatgpt_plan_type"].as_str().or(access["https://api.openai.com/auth"]["chatgpt_plan_type"].as_str());
        if let Some(plan) = plan {
            s.plan = plan.to_owned();
        }
    }
    if s.account_id.is_empty() {
        if let Some(account) = access["https://api.openai.com/auth"]["chatgpt_account_id"].as_str() {
            s.account_id = account.to_owned();
        }
    }
    if s.account_id.is_empty() {
        return Err("no ChatGPT account in the sign-in".into());
    }
    Ok(s)
}

/// What a call through curl gave back: the HTTP status and the body.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Reply {
    pub status: u16,
    pub body: String,
}

fn allowed(url: &str) -> bool {
    url.starts_with("https://") || url.starts_with("http://127.0.0.1")
}

/// One request through the system's curl, with the headers and body on its standard input so no token shows in the
/// list of processes.
pub fn call(url: &str, headers: &[(&str, String)], body: Option<&str>, seconds: u64) -> Result<Reply, String> {
    if !allowed(url) {
        return Err("only https addresses are used".into());
    }
    let mut config = format!("url = {}\nwrite-out = \"\\n%{{http_code}}\"\n", quote(url));
    for (name, value) in headers {
        config.push_str(&format!("header = {}\n", quote(&format!("{name}: {value}"))));
    }
    if let Some(body) = body {
        config.push_str(&format!("request = \"POST\"\ndata = {}\n", quote(body)));
    }
    let proto = if url.starts_with("https://") { "=https" } else { "=http" };
    let mut child = Command::new("/usr/bin/curl")
        .args(["--silent", "--max-time", &seconds.to_string(), "--proto", proto, "--config", "-"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|e| e.to_string())?;
    if let Some(mut stdin) = child.stdin.take() {
        let _ = stdin.write_all(config.as_bytes());
    }
    let out = child.wait_with_output().map_err(|e| e.to_string())?;
    let text = String::from_utf8_lossy(&out.stdout).into_owned();
    let (body, status) = text.rsplit_once('\n').unwrap_or(("", text.as_str()));
    match status.trim().parse::<u16>() {
        Ok(status) if status != 0 => Ok(Reply { status, body: body.to_owned() }),
        _ => Err("unreachable".into()),
    }
}

fn form_headers() -> Vec<(&'static str, String)> {
    vec![("Content-Type", "application/x-www-form-urlencoded".into()), ("Accept", "application/json".into())]
}

/// Trades the code of the browser for tokens at `token_url`.
pub fn exchange(token_url: &str, code: &str, verifier: &str, now: i64) -> Result<Session, String> {
    let body = form(&[
        ("grant_type", "authorization_code"),
        ("code", code),
        ("redirect_uri", REDIRECT),
        ("client_id", CLIENT_ID),
        ("code_verifier", verifier),
    ]);
    let reply = call(token_url, &form_headers(), Some(&body), 30)?;
    if reply.status != 200 {
        return Err(oauth_error(&reply));
    }
    let token: Value = serde_json::from_str(&reply.body).map_err(|e| e.to_string())?;
    from_token(&token, Session::default(), now)
}

/// Why a renewal did not give a session.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Renew {
    /// OpenAI refused the refresh token: the session is over.
    Ended(String),
    /// OpenAI could not be reached or failed: the session is kept for the next try.
    Later(String),
}

/// Renews the access token with the refresh token. Refresh tokens rotate, so the caller keeps the session this
/// returns and never runs two renewals at once.
pub fn refresh(token_url: &str, session: Session, now: i64) -> Result<Session, Renew> {
    if session.refresh_token.is_empty() {
        return Err(Renew::Ended("no refresh token".into()));
    }
    let body = form(&[("grant_type", "refresh_token"), ("refresh_token", &session.refresh_token), ("client_id", CLIENT_ID)]);
    let reply = call(token_url, &form_headers(), Some(&body), 30).map_err(Renew::Later)?;
    match reply.status {
        200 => {
            let token: Value = serde_json::from_str(&reply.body).map_err(|e| Renew::Later(e.to_string()))?;
            from_token(&token, session, now).map_err(Renew::Ended)
        }
        400 | 401 => Err(Renew::Ended(oauth_error(&reply))),
        status => Err(Renew::Later(format!("status {status}"))),
    }
}

fn oauth_error(reply: &Reply) -> String {
    let json: Value = serde_json::from_str(&reply.body).unwrap_or_default();
    let mut out = json["error"].as_str().or(json["error"]["code"].as_str()).unwrap_or("unknown").to_owned();
    if let Some(text) = json["error_description"].as_str().filter(|t| !t.is_empty()) {
        out.push_str(&format!(", {text}"));
    }
    format!("{} ({out})", reply.status)
}

/// The models of the account, in the order the server recommends them.
pub fn parse_models(json: &Value) -> Vec<String> {
    let mut listed: Vec<&Value> = json["models"]
        .as_array()
        .into_iter()
        .flatten()
        .filter(|m| m["visibility"] == "list" && m["slug"].as_str().is_some_and(|s| !s.is_empty()))
        .collect();
    listed.sort_by_key(|m| m["priority"].as_i64().unwrap_or(i64::MAX));
    listed.into_iter().filter_map(|m| m["slug"].as_str().map(str::to_owned)).collect()
}

/// The model the account lists first, or `None` when the list cannot be had.
pub fn first_model(session: &Session) -> Option<String> {
    let headers = [
        ("Authorization", format!("Bearer {}", session.access_token)),
        ("ChatGPT-Account-ID", session.account_id.clone()),
        ("Accept", "application/json".into()),
        ("originator", ORIGINATOR.into()),
    ];
    let reply = call(MODELS, &headers, None, 20).ok().filter(|r| r.status == 200)?;
    parse_models(&serde_json::from_str(&reply.body).ok()?).into_iter().next()
}

/// The whole sign-in: listens on the port of the redirect, has `open` show OpenAI's page in the browser, waits for
/// the code and trades it for tokens. `now` gives the time when the tokens arrive.
pub fn sign_in(open: &dyn Fn(&str), cancel: &Cancel, now: &dyn Fn() -> i64) -> Result<Session, String> {
    // `localhost` may come in over IPv4 or IPv6; the IPv4 one is enough when the other is not there.
    let v4 = TcpListener::bind(("127.0.0.1", PORT)).map_err(|_| "port 1455 is busy".to_owned())?;
    let mut listeners = vec![v4];
    if let Ok(v6) = TcpListener::bind(("::1", PORT)) {
        listeners.push(v6);
    }
    let (state, verifier) = (random_token()?, random_token()?);
    open(&authorize_url(&state, &challenge(&verifier)));
    let code = wait_for_code(&listeners, &state, SIGN_IN_SECONDS, cancel)?;
    drop(listeners);
    let mut session = exchange(TOKEN, &code, &verifier, now())?;
    session.model = first_model(&session).unwrap_or_default();
    Ok(session)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn jwt(claims: &Value) -> String {
        format!("e30.{}.sig", base64url(claims.to_string().as_bytes()))
    }

    fn hex(bytes: &[u8]) -> String {
        bytes.iter().map(|b| format!("{b:02x}")).collect()
    }

    #[test]
    fn sha256_and_the_pkce_challenge_match_the_standards() {
        assert_eq!(hex(&sha256(b"")), "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855");
        assert_eq!(hex(&sha256(b"abc")), "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad");
        let long = "a".repeat(1000);
        assert_eq!(hex(&sha256(long.as_bytes())), "41edece42d63e8d9bf515a9ba6932e1c20cbc9f5a5d134645adb5db1b9737ea3");
        // RFC 7636, appendix B.
        assert_eq!(challenge("dBjftJeZ4CVP-mB92K27uhbUJU1p1r_wW1gFWFOEjXk"), "E9Melhoa2OwvFrEMTJguCHaoeK1t8URWbuGJSstw-cM");
        let a = random_token().unwrap();
        assert_eq!(a.len(), 43);
        assert_ne!(a, random_token().unwrap());
    }

    #[test]
    fn the_sign_in_address_has_what_the_codex_client_needs() {
        let url = authorize_url("st", "ch");
        for part in [
            "https://auth.openai.com/oauth/authorize?",
            "client_id=app_EMoamEEZ73f0CkXaXp7hrann",
            "redirect_uri=http%3A%2F%2Flocalhost%3A1455%2Fauth%2Fcallback",
            "scope=openid%20profile%20email%20offline_access",
            "code_challenge=ch",
            "code_challenge_method=S256",
            "codex_cli_simplified_flow=true",
            "state=st",
            "originator=figghost",
        ] {
            assert!(url.contains(part), "{part} in {url}");
        }
    }

    #[test]
    fn only_the_callback_with_our_state_is_taken() {
        assert_eq!(read_callback("GET /auth/callback?code=a%2Bb&state=s HTTP/1.1", "s"), Some(Ok("a+b".into())));
        assert_eq!(read_callback("GET /auth/callback?error=access_denied&state=s HTTP/1.1", "s"), Some(Err("access_denied".into())));
        assert_eq!(read_callback("GET /auth/callback?code=x&state=other HTTP/1.1", "s"), None, "another state");
        assert_eq!(read_callback("GET /favicon.ico HTTP/1.1", "s"), None);
        assert_eq!(read_callback("POST /auth/callback?code=x&state=s HTTP/1.1", "s"), None);
        assert_eq!(parse_query("a=1+2&b=%7E%")["a"], "1 2");
        assert_eq!(parse_query("b=%7E%")["b"], "~%");
    }

    #[test]
    fn the_browser_is_waited_for_and_answered() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        let client = thread::spawn(move || {
            thread::sleep(Duration::from_millis(100));
            let mut other = TcpStream::connect(("127.0.0.1", port)).unwrap();
            other.write_all(b"GET /favicon.ico HTTP/1.1\r\n\r\n").unwrap();
            let mut s = TcpStream::connect(("127.0.0.1", port)).unwrap();
            s.write_all(b"GET /auth/callback?code=the-code&state=st HTTP/1.1\r\nHost: localhost\r\n\r\n").unwrap();
            let mut page = String::new();
            s.read_to_string(&mut page).unwrap();
            page
        });
        let code = wait_for_code(&[listener], "st", 10, &Cancel::default());
        assert_eq!(code, Ok("the-code".into()));
        assert!(client.join().unwrap().starts_with("HTTP/1.1 200 OK"));

        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let cancel = Cancel::default();
        cancel.store(true, Ordering::SeqCst);
        assert_eq!(wait_for_code(&[listener], "st", 10, &cancel), Err("cancelled".into()));
    }

    #[test]
    fn a_token_response_gives_the_account_plan_and_expiry() {
        let id = jwt(&json!({ "iss": ISSUER, "aud": CLIENT_ID, "sub": "u1", "email": "a@b.c",
            "https://api.openai.com/auth": { "chatgpt_account_id": "acc-1", "chatgpt_plan_type": "plus" } }));
        let token = json!({ "access_token": jwt(&json!({ "exp": 5000 })), "refresh_token": "rt", "id_token": id });
        let s = from_token(&token, Session::default(), 1000).unwrap();
        assert_eq!((s.email.as_str(), s.account_id.as_str(), s.plan.as_str(), s.expires_at), ("a@b.c", "acc-1", "plus", 5000));
        assert!(s.signed_in() && !s.due(1000) && s.due(4900));
        let renewed = from_token(&json!({ "access_token": jwt(&json!({ "exp": 9000 })), "refresh_token": "rt2" }), s.clone(), 4000).unwrap();
        assert_eq!((renewed.account_id.as_str(), renewed.refresh_token.as_str(), renewed.expires_at), ("acc-1", "rt2", 9000));
        assert_eq!(Session::from_json(renewed.to_json().as_bytes()), Some(renewed));
        let other = jwt(&json!({ "iss": ISSUER, "aud": "other", "sub": "u1" }));
        assert!(from_token(&json!({ "access_token": "x", "id_token": other }), Session::default(), 0).is_err());
        assert!(from_token(&json!({ "access_token": "x" }), Session::default(), 0).is_err(), "no account");
        assert!(from_token(&json!({}), Session::default(), 0).is_err());
    }

    /// Serves one request on a loopback port with `status` and `body`; hands back the request body.
    fn serve(status: u16, body: &'static str) -> (String, thread::JoinHandle<String>) {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let url = format!("http://127.0.0.1:{}/oauth/token", listener.local_addr().unwrap().port());
        let handle = thread::spawn(move || {
            let (mut s, _) = listener.accept().unwrap();
            let mut got = Vec::new();
            let mut buf = [0u8; 4096];
            loop {
                let n = s.read(&mut buf).unwrap();
                got.extend_from_slice(&buf[..n]);
                let text = String::from_utf8_lossy(&got).into_owned();
                if let Some((head, rest)) = text.split_once("\r\n\r\n") {
                    let length: usize = head
                        .lines()
                        .find_map(|l| l.to_ascii_lowercase().strip_prefix("content-length:").map(|v| v.trim().parse().unwrap()))
                        .unwrap_or(0);
                    if rest.len() >= length {
                        write!(s, "HTTP/1.1 {status} X\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}", body.len()).unwrap();
                        return rest.to_owned();
                    }
                }
            }
        });
        (url, handle)
    }

    #[test]
    fn a_refresh_rotates_the_token_and_a_refused_one_ends_the_session() {
        let session = Session { account_id: "acc".into(), access_token: "old".into(), refresh_token: "r1".into(), ..Session::default() };
        let (url, server) = serve(200, r#"{"access_token":"e30.eyJleHAiOjkwMDB9.s","refresh_token":"r2"}"#);
        let renewed = refresh(&url, session.clone(), 100).unwrap();
        assert_eq!((renewed.refresh_token.as_str(), renewed.expires_at), ("r2", 9000));
        let sent = server.join().unwrap();
        assert!(sent.contains("grant_type=refresh_token") && sent.contains("refresh_token=r1") && sent.contains(CLIENT_ID), "{sent}");

        let (url, server) = serve(400, r#"{"error":"invalid_grant"}"#);
        assert!(matches!(refresh(&url, session.clone(), 100), Err(Renew::Ended(why)) if why.contains("invalid_grant")));
        server.join().unwrap();
        let (url, server) = serve(503, "");
        assert!(matches!(refresh(&url, session.clone(), 100), Err(Renew::Later(_))), "a failing server keeps the session");
        server.join().unwrap();
        assert!(matches!(refresh("http://127.0.0.1:1/x", session, 100), Err(Renew::Later(_))), "nothing listening");
    }

    #[test]
    fn models_keep_the_listed_ones_by_priority() {
        let json = json!({ "models": [
            { "slug": "b", "visibility": "list", "priority": 2 },
            { "slug": "hidden", "visibility": "hide", "priority": 0 },
            { "slug": "a", "visibility": "list", "priority": 1 } ] });
        assert_eq!(parse_models(&json), ["a", "b"]);
        assert!(parse_models(&json!({})).is_empty());
    }
}
