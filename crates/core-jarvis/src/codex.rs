//! The sign-in of the Codex command line, used to ask the model of the ChatGPT plan the person already has. The
//! file `auth.json` of Codex is only read: a token is never renewed here, because Codex rotates its refresh tokens
//! and a renewal made behind its back would sign it out. When the access token has run out the person is told to
//! use Codex once, which renews it. Not an official way for other programs to call that service, and OpenAI may
//! change it at any time.

use serde_json::Value;
use std::{
    fs,
    path::{Path, PathBuf},
};

/// What a request needs from the sign-in.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Sign {
    pub access_token: String,
    pub account_id: String,
}

/// Why there is no usable sign-in.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SignIn {
    /// There is no `auth.json`, or it holds no ChatGPT sign-in (an API key is not one).
    Missing,
    /// The access token has run out.
    Expired,
    Unreadable,
}

/// Where Codex keeps its sign-in: `CODEX_HOME` if it is set, else `.codex` in the home folder.
pub fn auth_path(codex_home: Option<&Path>, home: &Path) -> PathBuf {
    codex_home.map_or_else(|| home.join(".codex"), Path::to_path_buf).join("auth.json")
}

/// The part of a JWT between its first and second dot, decoded.
fn claims(token: &str) -> Option<Value> {
    let payload = token.split('.').nth(1)?;
    let mut bits = 0u32;
    let mut held = 0u32;
    let mut out = Vec::new();
    for c in payload.trim_end_matches('=').bytes() {
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
    serde_json::from_slice(&out).ok()
}

/// The sign-in in `auth.json`, if it is there, readable and not out of date at `now` (Unix seconds).
pub fn read(path: &Path, now: i64) -> Result<Sign, SignIn> {
    let text = match fs::read_to_string(path) {
        Ok(text) => text,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Err(SignIn::Missing),
        Err(_) => return Err(SignIn::Unreadable),
    };
    let json: Value = serde_json::from_str(&text).map_err(|_| SignIn::Unreadable)?;
    let tokens = &json["tokens"];
    let access = tokens["access_token"].as_str().filter(|t| !t.is_empty()).ok_or(SignIn::Missing)?;
    let account = tokens["account_id"]
        .as_str()
        .filter(|a| !a.is_empty())
        .map(str::to_owned)
        .or_else(|| {
            let id = claims(tokens["id_token"].as_str()?)?;
            id["https://api.openai.com/auth"]["chatgpt_account_id"].as_str().map(str::to_owned)
        })
        .ok_or(SignIn::Missing)?;
    // A token with no expiry is tried; the service says if it is refused.
    if let Some(exp) = claims(access).and_then(|c| c["exp"].as_i64())
        && exp <= now + 30
    {
        return Err(SignIn::Expired);
    }
    Ok(Sign { access_token: access.to_owned(), account_id: account })
}

/// The model Codex lists first, from the cache it keeps next to its sign-in (`models_cache.json`).
pub fn default_model(cache: &Path) -> Option<String> {
    let json: Value = serde_json::from_str(&fs::read_to_string(cache).ok()?).ok()?;
    let mut listed: Vec<&Value> = json["models"]
        .as_array()?
        .iter()
        .filter(|m| m["visibility"] == "list" && m["slug"].as_str().is_some_and(|s| !s.is_empty()))
        .collect();
    listed.sort_by_key(|m| m["priority"].as_i64().unwrap_or(i64::MAX));
    listed.first()?["slug"].as_str().map(str::to_owned)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn jwt(claims: &Value) -> String {
        // base64url of the claims, written out here so the test does not lean on the decoder it checks.
        const ABC: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_";
        let bytes = claims.to_string().into_bytes();
        let mut out = String::from("e30.");
        for chunk in bytes.chunks(3) {
            let n = chunk.iter().enumerate().fold(0u32, |n, (i, b)| n | (u32::from(*b) << (16 - 8 * i)));
            for i in 0..=chunk.len() {
                out.push(ABC[((n >> (18 - 6 * i)) & 63) as usize] as char);
            }
        }
        out.push_str(".sig");
        out
    }

    fn dir() -> PathBuf {
        let dir = std::env::temp_dir().join(format!("jarvis-codex-{}-{:?}", std::process::id(), std::thread::current().id()));
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn write(dir: &Path, name: &str, json: &Value) -> PathBuf {
        let path = dir.join(name);
        fs::write(&path, json.to_string()).unwrap();
        path
    }

    #[test]
    fn the_sign_in_is_where_codex_keeps_it() {
        assert_eq!(auth_path(None, Path::new("/Users/a")), Path::new("/Users/a/.codex/auth.json"));
        assert_eq!(auth_path(Some(Path::new("/x/home")), Path::new("/Users/a")), Path::new("/x/home/auth.json"));
    }

    #[test]
    fn a_token_that_has_not_run_out_is_taken_with_its_account() {
        let dir = dir();
        let token = jwt(&json!({ "exp": 5000 }));
        let path = write(&dir, "a.json", &json!({ "tokens": { "access_token": token, "account_id": "acct-1", "refresh_token": "never read" } }));
        assert_eq!(read(&path, 1000), Ok(Sign { access_token: token.clone(), account_id: "acct-1".into() }));
        assert_eq!(read(&path, 4990), Err(SignIn::Expired), "within thirty seconds of the end counts as out");
        assert_eq!(read(&path, 9000), Err(SignIn::Expired));
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn the_account_may_come_from_the_id_token_and_a_token_without_expiry_is_tried() {
        let dir = dir();
        let id = jwt(&json!({ "https://api.openai.com/auth": { "chatgpt_account_id": "acct-2" } }));
        let path = write(&dir, "b.json", &json!({ "tokens": { "access_token": "opaque", "id_token": id } }));
        assert_eq!(read(&path, 1000).unwrap().account_id, "acct-2");
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn what_is_not_a_chatgpt_sign_in_is_said_so() {
        let dir = dir();
        assert_eq!(read(&dir.join("none.json"), 0), Err(SignIn::Missing));
        let api_key = write(&dir, "key.json", &json!({ "OPENAI_API_KEY": "sk-x", "tokens": null }));
        assert_eq!(read(&api_key, 0), Err(SignIn::Missing), "an API key is not the plan's sign-in");
        let bad = dir.join("bad.json");
        fs::write(&bad, "not json").unwrap();
        assert_eq!(read(&bad, 0), Err(SignIn::Unreadable));
        let no_account = write(&dir, "n.json", &json!({ "tokens": { "access_token": "t" } }));
        assert_eq!(read(&no_account, 0), Err(SignIn::Missing));
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn the_default_model_is_the_first_listed_by_priority() {
        let dir = dir();
        let cache = write(&dir, "models_cache.json", &json!({ "models": [
            { "slug": "hidden", "visibility": "hide", "priority": 1 },
            { "slug": "second", "visibility": "list", "priority": 9 },
            { "slug": "first", "visibility": "list", "priority": 5 } ] }));
        assert_eq!(default_model(&cache).as_deref(), Some("first"));
        assert_eq!(default_model(&dir.join("missing.json")), None);
        fs::remove_dir_all(&dir).unwrap();
    }
}
