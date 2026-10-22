//! The Claude Code and Codex accounts the user keeps in Orca, what is left of their limits, and switching the
//! account the plain `claude` (or `codex`) command uses to one of them.
//!
//! Orca gives each account a folder of its own: a `CLAUDE_CONFIG_DIR` for Claude Code, which then keeps the
//! sign-in in the Keychain item "Claude Code-credentials-<first 8 hex digits of the SHA-256 of the folder>",
//! and a `CODEX_HOME` for Codex, which keeps it in `auth.json` there. The plain commands use
//! "Claude Code-credentials" with `~/.claude.json`, and `~/.codex/auth.json`. Switching copies the account's
//! sign-in over the plain one after keeping a private copy of what was there, which is what Restore puts back.
//! Secrets go through `Secrets`, so the Keychain itself stays in `macos-sys` and the tests use a fake one.

use crate::{
    Agent, install,
    usage::{Limit, Limits, parse_rfc3339_ms},
};
use serde_json::{Map, Value, json};
use std::{
    fs,
    io::{self, Write},
    os::unix::fs::{DirBuilderExt, OpenOptionsExt},
    path::{Path, PathBuf},
};

/// The Keychain item of the plain `claude` command.
pub const CLAUDE_SERVICE: &str = "Claude Code-credentials";
/// Where Claude Code asks for the limits of a subscription; Orca asks the same way.
pub const USAGE_URL: &str = "https://api.anthropic.com/api/oauth/usage";
pub const USAGE_BETA: &str = "oauth-2025-04-20";
/// The folder of the data directory that keeps the copies taken before a switch.
pub const BACKUP_DIR: &str = "account-backups";
const BACKUPS_KEPT: usize = 5;
/// A token this close to its end is treated as ended: it would die on the way.
const EXPIRY_MARGIN_MS: i64 = 60_000;

#[derive(Debug, Clone, PartialEq)]
pub struct Account {
    pub agent: Agent,
    /// Orca's id for it.
    pub id: String,
    pub email: String,
    /// The display name of the account, empty when unknown.
    pub name: String,
    /// The plan, as "Max 20x" or "Pro", empty when unknown.
    pub plan: String,
    /// The folder Orca gives it: `CLAUDE_CONFIG_DIR` for Claude Code, `CODEX_HOME` for Codex.
    pub home: PathBuf,
    /// Orca uses it for the agents it starts.
    pub orca: bool,
    /// The plain command uses it now.
    pub cli: bool,
}

/// Where the secrets are kept; the Keychain in the app, a map in the tests.
pub trait Secrets {
    fn read(&self, service: &str, account: &str) -> Result<Option<Vec<u8>>, String>;
    fn write(&self, service: &str, account: &str, data: &[u8]) -> Result<(), String>;
    fn delete(&self, service: &str, account: &str) -> Result<(), String>;
}

/// The files a switch touches: the user's home, the app's backup folder and the Keychain account name.
#[derive(Debug, Clone)]
pub struct Places {
    pub user_home: PathBuf,
    pub backups: PathBuf,
    pub keychain_account: String,
}

pub fn orca_dir(user_home: &Path) -> PathBuf {
    user_home.join("Library/Application Support/orca")
}

/// The account attribute Claude Code gives its Keychain items: the user name when it is plain, else a fixed one.
pub fn keychain_account(user: &str) -> String {
    let plain = !user.is_empty()
        && user
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-'));
    if plain {
        user.to_owned()
    } else {
        "claude-code-user".to_owned()
    }
}

/// The Keychain item Claude Code uses with this `CLAUDE_CONFIG_DIR`, or the plain one without.
// shortcut: the path is hashed as given, without the NFC normalisation Claude Code applies; Orca's paths are
// plain ASCII. Normalise first if accounts with accented folder names turn up.
pub fn claude_service(config_dir: Option<&Path>) -> String {
    match config_dir {
        Some(dir) => format!(
            "{CLAUDE_SERVICE}-{:08x}",
            sha256_first_word(dir.as_os_str().as_encoded_bytes())
        ),
        None => CLAUDE_SERVICE.to_owned(),
    }
}

/// The first 32 bits of the SHA-256 of `data`, which is all a Keychain item name needs.
fn sha256_first_word(data: &[u8]) -> u32 {
    const K: [u32; 64] = [
        0x428a2f98, 0x71374491, 0xb5c0fbcf, 0xe9b5dba5, 0x3956c25b, 0x59f111f1, 0x923f82a4,
        0xab1c5ed5, 0xd807aa98, 0x12835b01, 0x243185be, 0x550c7dc3, 0x72be5d74, 0x80deb1fe,
        0x9bdc06a7, 0xc19bf174, 0xe49b69c1, 0xefbe4786, 0x0fc19dc6, 0x240ca1cc, 0x2de92c6f,
        0x4a7484aa, 0x5cb0a9dc, 0x76f988da, 0x983e5152, 0xa831c66d, 0xb00327c8, 0xbf597fc7,
        0xc6e00bf3, 0xd5a79147, 0x06ca6351, 0x14292967, 0x27b70a85, 0x2e1b2138, 0x4d2c6dfc,
        0x53380d13, 0x650a7354, 0x766a0abb, 0x81c2c92e, 0x92722c85, 0xa2bfe8a1, 0xa81a664b,
        0xc24b8b70, 0xc76c51a3, 0xd192e819, 0xd6990624, 0xf40e3585, 0x106aa070, 0x19a4c116,
        0x1e376c08, 0x2748774c, 0x34b0bcb5, 0x391c0cb3, 0x4ed8aa4a, 0x5b9cca4f, 0x682e6ff3,
        0x748f82ee, 0x78a5636f, 0x84c87814, 0x8cc70208, 0x90befffa, 0xa4506ceb, 0xbef9a3f7,
        0xc67178f2,
    ];
    let mut h: [u32; 8] = [
        0x6a09e667, 0xbb67ae85, 0x3c6ef372, 0xa54ff53a, 0x510e527f, 0x9b05688c, 0x1f83d9ab,
        0x5be0cd19,
    ];
    let mut message = data.to_vec();
    message.push(0x80);
    while message.len() % 64 != 56 {
        message.push(0);
    }
    message.extend_from_slice(&(data.len() as u64 * 8).to_be_bytes());
    for block in message.chunks(64) {
        let mut w = [0u32; 64];
        for (i, word) in block.chunks(4).enumerate() {
            w[i] = u32::from_be_bytes([word[0], word[1], word[2], word[3]]);
        }
        for i in 16..64 {
            let s0 = w[i - 15].rotate_right(7) ^ w[i - 15].rotate_right(18) ^ (w[i - 15] >> 3);
            let s1 = w[i - 2].rotate_right(17) ^ w[i - 2].rotate_right(19) ^ (w[i - 2] >> 10);
            w[i] = w[i - 16]
                .wrapping_add(s0)
                .wrapping_add(w[i - 7])
                .wrapping_add(s1);
        }
        let mut v = h;
        for i in 0..64 {
            let [a, b, c, d, e, f, g, hh] = v;
            let s1 = e.rotate_right(6) ^ e.rotate_right(11) ^ e.rotate_right(25);
            let t1 = hh
                .wrapping_add(s1)
                .wrapping_add((e & f) ^ (!e & g))
                .wrapping_add(K[i])
                .wrapping_add(w[i]);
            let s0 = a.rotate_right(2) ^ a.rotate_right(13) ^ a.rotate_right(22);
            let t2 = s0.wrapping_add((a & b) ^ (a & c) ^ (b & c));
            v = [t1.wrapping_add(t2), a, b, c, d.wrapping_add(t1), e, f, g];
        }
        for (word, add) in h.iter_mut().zip(v) {
            *word = word.wrapping_add(add);
        }
    }
    h[0]
}

fn text<'a>(value: &'a Value, key: &str) -> Option<&'a str> {
    value
        .get(key)
        .and_then(Value::as_str)
        .filter(|s| !s.is_empty())
}

/// Orca's settings of its active profile. Newer Orca keeps them in the profile's SQLite file, older Orca in
/// `orca-data.json`; the file is opened read-only, so Orca's own writes are never in danger.
fn orca_settings(orca: &Path) -> Value {
    let index = install::read_json(&orca.join("orca-profile-index.json")).unwrap_or_default();
    // The id becomes part of a path, so only a plain name is taken.
    let profile = text(&index, "activeProfileId")
        .filter(|id| {
            id.chars()
                .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_'))
        })
        .unwrap_or("local-default");
    let dir = orca.join("profiles").join(profile);
    settings_from_db(&dir.join("profile-state.db"))
        .or_else(|| {
            install::read_json(&dir.join("orca-data.json"))
                .ok()?
                .get("settings")
                .cloned()
        })
        .unwrap_or(Value::Null)
}

fn settings_from_db(path: &Path) -> Option<Value> {
    use rusqlite::{Connection, OpenFlags};
    if !path.is_file() {
        return None;
    }
    let db = Connection::open_with_flags(
        path,
        OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )
    .ok()?;
    let payload: String = db
        .query_row(
            "SELECT payload FROM profile_state_documents WHERE domain = 'settings'",
            [],
            |row| row.get(0),
        )
        .ok()?;
    serde_json::from_str(&payload).ok()
}

/// The `oauthAccount` of a Claude Code config file, if it names one.
fn oauth_account(doc: &Value) -> Option<Value> {
    doc.get("oauthAccount").filter(|v| v.is_object()).cloned()
}

/// "Max 20x" from "default_claude_max_20x", "Pro" from "claude_pro".
pub fn plan_label(profile: &Value) -> String {
    let Some(raw) =
        text(profile, "organizationRateLimitTier").or(text(profile, "organizationType"))
    else {
        return String::new();
    };
    let rest = raw.strip_prefix("default_").unwrap_or(raw);
    let rest = rest.strip_prefix("claude_").unwrap_or(rest);
    let mut words: Vec<String> = rest
        .split('_')
        .filter(|w| !w.is_empty())
        .map(str::to_owned)
        .collect();
    if let Some(first) = words.first_mut() {
        let mut chars = first.chars();
        if let Some(c) = chars.next() {
            *first = c.to_uppercase().chain(chars).collect();
        }
    }
    words.join(" ")
}

/// The same Claude account: the same account id, or the same address when one side has no id.
fn same_claude(a: &Value, b: &Value) -> bool {
    match (text(a, "accountUuid"), text(b, "accountUuid")) {
        (Some(x), Some(y)) => x == y,
        _ => matches!((text(a, "emailAddress"), text(b, "emailAddress")),
            (Some(x), Some(y)) if x.eq_ignore_ascii_case(y)),
    }
}

/// The ChatGPT account id in a Codex `auth.json`; nothing secret is kept from it.
fn codex_account_id(path: &Path) -> Option<String> {
    let doc = install::read_json(path).ok()?;
    doc.get("tokens")?
        .get("account_id")?
        .as_str()
        .map(str::to_owned)
}

/// Every account Orca keeps that runs on this Mac, Claude Code first. A missing Orca, an unreadable file or
/// an entry of an unknown shape gives fewer accounts, never an error.
pub fn list(user_home: &Path) -> Vec<Account> {
    let settings = orca_settings(&orca_dir(user_home));
    let entries = |key: &str| {
        settings
            .get(key)
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default()
            .into_iter()
            // Accounts that live inside WSL belong to Windows.
            .filter(|e| {
                text(e, "managedAuthRuntime")
                    .or(text(e, "managedHomeRuntime"))
                    .is_none_or(|r| r == "host")
            })
    };
    let mut out = Vec::new();
    let cli = install::read_json(&user_home.join(".claude.json"))
        .ok()
        .and_then(|doc| oauth_account(&doc));
    let orca_claude = text(&settings, "activeClaudeManagedAccountId");
    for entry in entries("claudeManagedAccounts") {
        let (Some(id), Some(home)) = (text(&entry, "id"), text(&entry, "managedAuthPath")) else {
            continue;
        };
        let home = PathBuf::from(home);
        let profile = install::read_json(&home.join(".claude.json"))
            .ok()
            .and_then(|doc| oauth_account(&doc))
            .unwrap_or_default();
        out.push(Account {
            agent: Agent::Claude,
            id: id.to_owned(),
            email: text(&profile, "emailAddress")
                .or(text(&entry, "email"))
                .unwrap_or_default()
                .to_owned(),
            name: text(&profile, "displayName").unwrap_or_default().to_owned(),
            plan: plan_label(&profile),
            orca: orca_claude == Some(id),
            cli: cli
                .as_ref()
                .is_some_and(|cli| profile.is_object() && same_claude(cli, &profile)),
            home,
        });
    }
    let cli_codex = codex_account_id(&user_home.join(".codex/auth.json"));
    let orca_codex = text(&settings, "activeCodexManagedAccountId");
    for entry in entries("codexManagedAccounts") {
        let (Some(id), Some(home)) = (text(&entry, "id"), text(&entry, "managedHomePath")) else {
            continue;
        };
        let home = PathBuf::from(home);
        let own = codex_account_id(&home.join("auth.json"));
        out.push(Account {
            agent: Agent::Codex,
            id: id.to_owned(),
            email: text(&entry, "email").unwrap_or_default().to_owned(),
            name: String::new(),
            plan: String::new(),
            orca: orca_codex == Some(id),
            cli: own.is_some() && own == cli_codex,
            home,
        });
    }
    out
}

/// A short name for the account the plain `claude` command uses: its display name, else the part of its
/// address before the at sign.
pub fn cli_account_name(user_home: &Path) -> Option<String> {
    let profile = oauth_account(&install::read_json(&user_home.join(".claude.json")).ok()?)?;
    let name = text(&profile, "displayName")
        .or_else(|| text(&profile, "emailAddress").and_then(|e| e.split('@').next()))?;
    Some(name.chars().take(24).collect())
}

/// Why the limits of an account cannot be shown.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Unavailable {
    /// No sign-in is stored for it.
    NoSignIn,
    /// Its access token has run out; Orca or Claude Code renews it the next time they use the account. Renewing
    /// it here would change the refresh token under them and sign them out.
    Expired,
    /// The user did not let us read it from the Keychain.
    Refused,
    /// The request failed or the answer was not understood.
    Failed,
}

impl Unavailable {
    pub fn code(self) -> &'static str {
        match self {
            Unavailable::NoSignIn => "no_sign_in",
            Unavailable::Expired => "expired",
            Unavailable::Refused => "refused",
            Unavailable::Failed => "failed",
        }
    }
}

/// The access token in a Claude Code credential, unless it has run out.
pub fn access_token(credential: &[u8], now_ms: i64) -> Result<String, Unavailable> {
    let doc: Value = serde_json::from_slice(credential).map_err(|_| Unavailable::NoSignIn)?;
    let oauth = doc.get("claudeAiOauth").ok_or(Unavailable::NoSignIn)?;
    let token = text(oauth, "accessToken").ok_or(Unavailable::NoSignIn)?;
    match oauth.get("expiresAt").and_then(Value::as_i64) {
        Some(end) if end - EXPIRY_MARGIN_MS <= now_ms => Err(Unavailable::Expired),
        _ => Ok(token.to_owned()),
    }
}

fn usage_window(root: &Value, key: &str, minutes: u64) -> Option<Limit> {
    let object = root.get(key)?;
    let used = object
        .get("utilization")
        .or_else(|| object.get("used_percentage"))?
        .as_f64()?;
    let resets_at = match object.get("resets_at") {
        Some(Value::String(at)) => parse_rfc3339_ms(at).map(|ms| ms / 1000),
        Some(Value::Number(n)) => n.as_f64().map(|s| s as i64),
        _ => None,
    };
    Some(Limit {
        used_percent: used.clamp(0.0, 100.0),
        window_minutes: Some(minutes),
        resets_at,
    })
}

/// The five-hour and seven-day windows in the answer of the usage endpoint, true as of `now_ms`.
pub fn limits_from_usage(body: &str, now_ms: i64) -> Option<Limits> {
    let root: Value = serde_json::from_str(body).ok()?;
    let (primary, secondary) = (
        usage_window(&root, "five_hour", 300),
        usage_window(&root, "seven_day", 10_080),
    );
    if primary.is_none() && secondary.is_none() {
        return None;
    }
    Some(Limits {
        primary,
        secondary,
        observed_ms: now_ms,
        plan: None,
    })
}

/// Writes a file only the user can read, in one step.
fn write_private(path: &Path, bytes: &[u8]) -> io::Result<()> {
    let mut name = path.as_os_str().to_owned();
    name.push(format!(".figghost-{}.tmp", std::process::id()));
    let temp = PathBuf::from(name);
    let written = fs::OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .mode(0o600)
        .open(&temp)
        .and_then(|mut file| file.write_all(bytes))
        .and_then(|()| fs::rename(&temp, path));
    if written.is_err() {
        let _ = fs::remove_file(&temp);
    }
    written
}

/// Keeps what is there now before a switch, and the last few such copies only.
fn save_backup(dir: &Path, agent: Agent, now_ms: i64, content: &Value) -> Result<(), String> {
    fs::DirBuilder::new()
        .recursive(true)
        .mode(0o700)
        .create(dir)
        .map_err(|e| format!("cannot make the backup folder: {e}"))?;
    let path = dir.join(format!("{}-{now_ms:013}.json", agent.id()));
    write_private(&path, content.to_string().as_bytes())
        .map_err(|e| format!("cannot save the backup: {e}"))?;
    let mut all = backups(dir, agent);
    while all.len() > BACKUPS_KEPT {
        let _ = fs::remove_file(all.remove(0));
    }
    Ok(())
}

/// The backups of one agent, oldest first.
fn backups(dir: &Path, agent: Agent) -> Vec<PathBuf> {
    let prefix = format!("{}-", agent.id());
    let mut found: Vec<PathBuf> = fs::read_dir(dir)
        .into_iter()
        .flatten()
        .flatten()
        .map(|e| e.path())
        .filter(|p| {
            p.file_name()
                .and_then(|n| n.to_str())
                .is_some_and(|n| n.starts_with(&prefix) && n.ends_with(".json"))
        })
        .collect();
    found.sort();
    found
}

pub fn can_restore(places: &Places, agent: Agent) -> bool {
    !backups(&places.backups, agent).is_empty()
}

fn set_key(doc: &mut Value, key: &str, value: Option<Value>) -> Result<(), String> {
    let root: &mut Map<String, Value> = doc
        .as_object_mut()
        .ok_or("~/.claude.json is not an object")?;
    match value {
        // An existing key keeps its place in the file.
        Some(value) => {
            root.insert(key.to_owned(), value);
        }
        None => {
            root.shift_remove(key);
        }
    }
    Ok(())
}

/// Makes the plain command use `account`, after keeping what it used before.
pub fn switch(
    places: &Places,
    account: &Account,
    secrets: &dyn Secrets,
    now_ms: i64,
) -> Result<(), String> {
    match account.agent {
        Agent::Claude => switch_claude(places, account, secrets, now_ms),
        Agent::Codex => switch_codex(places, account, now_ms),
        // Orca keeps no pi accounts, so none is ever listed.
        Agent::Pi => Err("pi has no accounts to switch".into()),
    }
}

fn switch_claude(
    places: &Places,
    account: &Account,
    secrets: &dyn Secrets,
    now_ms: i64,
) -> Result<(), String> {
    let user = &places.keychain_account;
    let credential = secrets
        .read(&claude_service(Some(&account.home)), user)?
        .ok_or("this account has no sign-in in the Keychain; sign in to it in Orca first")?;
    let valid = serde_json::from_slice::<Value>(&credential)
        .ok()
        .is_some_and(|doc| doc.get("claudeAiOauth").is_some_and(Value::is_object));
    if !valid {
        return Err("the sign-in of this account is not one Claude Code wrote".into());
    }
    let profile = install::read_json(&account.home.join(".claude.json"))
        .ok()
        .and_then(|doc| oauth_account(&doc))
        .ok_or("the account folder does not say which account it is")?;
    let config = places.user_home.join(".claude.json");
    // An unreadable file stops the switch before anything changes, and is never overwritten.
    let mut doc = install::read_json(&config).map_err(|e| format!("~/.claude.json: {e}"))?;
    let current = secrets.read(CLAUDE_SERVICE, user)?;
    let current = current
        .map(String::from_utf8)
        .transpose()
        .map_err(|_| "the current sign-in is not text")?;
    save_backup(
        &places.backups,
        Agent::Claude,
        now_ms,
        &json!({ "credential": current, "oauthAccount": oauth_account(&doc) }),
    )?;
    secrets.write(CLAUDE_SERVICE, user, &credential)?;
    let written = set_key(&mut doc, "oauthAccount", Some(profile)).and_then(|()| {
        install::write_json(&config, &doc).map_err(|e| format!("~/.claude.json: {e}"))
    });
    if let Err(e) = written {
        // The two must name the same account, so the sign-in goes back as it was.
        let _ = match &current {
            Some(previous) => secrets.write(CLAUDE_SERVICE, user, previous.as_bytes()),
            None => secrets.delete(CLAUDE_SERVICE, user),
        };
        return Err(e);
    }
    Ok(())
}

fn switch_codex(places: &Places, account: &Account, now_ms: i64) -> Result<(), String> {
    let source = fs::read(account.home.join("auth.json"))
        .map_err(|_| "this account has no auth.json; sign in to it in Orca first")?;
    if !serde_json::from_slice::<Value>(&source).is_ok_and(|doc| doc.is_object()) {
        return Err("the auth.json of this account is not valid".into());
    }
    let dir = places.user_home.join(".codex");
    if !dir.is_dir() {
        return Err("~/.codex does not exist: is Codex installed?".into());
    }
    let target = dir.join("auth.json");
    let current = match fs::read_to_string(&target) {
        Ok(text) => Some(text),
        Err(e) if e.kind() == io::ErrorKind::NotFound => None,
        Err(e) => return Err(format!("~/.codex/auth.json: {e}")),
    };
    save_backup(
        &places.backups,
        Agent::Codex,
        now_ms,
        &json!({ "auth": current }),
    )?;
    write_private(&target, &source).map_err(|e| format!("~/.codex/auth.json: {e}"))
}

/// Puts back what the newest backup kept, and drops that backup, so the next restore goes one further back.
pub fn restore(places: &Places, agent: Agent, secrets: &dyn Secrets) -> Result<(), String> {
    let path = backups(&places.backups, agent)
        .pop()
        .ok_or("there is nothing to restore")?;
    let backup: Value = fs::read(&path)
        .ok()
        .and_then(|bytes| serde_json::from_slice(&bytes).ok())
        .ok_or("the backup cannot be read")?;
    match agent {
        Agent::Claude => {
            let config = places.user_home.join(".claude.json");
            let mut doc =
                install::read_json(&config).map_err(|e| format!("~/.claude.json: {e}"))?;
            let user = &places.keychain_account;
            match backup.get("credential").and_then(Value::as_str) {
                Some(previous) => secrets.write(CLAUDE_SERVICE, user, previous.as_bytes())?,
                None => secrets.delete(CLAUDE_SERVICE, user)?,
            }
            set_key(
                &mut doc,
                "oauthAccount",
                backup
                    .get("oauthAccount")
                    .filter(|v| v.is_object())
                    .cloned(),
            )?;
            install::write_json(&config, &doc).map_err(|e| format!("~/.claude.json: {e}"))?;
        }
        Agent::Codex => {
            let target = places.user_home.join(".codex/auth.json");
            match backup.get("auth").and_then(Value::as_str) {
                Some(previous) => write_private(&target, previous.as_bytes()),
                None => fs::remove_file(&target).or_else(|e| match e.kind() {
                    io::ErrorKind::NotFound => Ok(()),
                    _ => Err(e),
                }),
            }
            .map_err(|e| format!("~/.codex/auth.json: {e}"))?;
        }
        Agent::Pi => return Err("pi has no accounts to restore".into()),
    }
    let _ = fs::remove_file(&path);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{cell::RefCell, collections::HashMap, os::unix::fs::PermissionsExt};

    #[derive(Default)]
    struct FakeKeychain(RefCell<HashMap<String, Vec<u8>>>);

    impl Secrets for FakeKeychain {
        fn read(&self, service: &str, account: &str) -> Result<Option<Vec<u8>>, String> {
            Ok(self
                .0
                .borrow()
                .get(&format!("{service}/{account}"))
                .cloned())
        }
        fn write(&self, service: &str, account: &str, data: &[u8]) -> Result<(), String> {
            self.0
                .borrow_mut()
                .insert(format!("{service}/{account}"), data.to_vec());
            Ok(())
        }
        fn delete(&self, service: &str, account: &str) -> Result<(), String> {
            self.0.borrow_mut().remove(&format!("{service}/{account}"));
            Ok(())
        }
    }

    struct Sandbox {
        root: PathBuf,
        places: Places,
    }

    impl Sandbox {
        fn new(name: &str) -> Self {
            let root = std::env::temp_dir().join(format!(
                "core-agents-accounts-{name}-{}",
                std::process::id()
            ));
            let _ = fs::remove_dir_all(&root);
            let home = root.join("home");
            fs::create_dir_all(&home).unwrap();
            Sandbox {
                places: Places {
                    user_home: home,
                    backups: root.join("data").join(BACKUP_DIR),
                    keychain_account: "me".into(),
                },
                root,
            }
        }

        fn home(&self) -> &Path {
            &self.places.user_home
        }

        fn write(&self, relative: &str, value: &Value) -> PathBuf {
            let path = self.home().join(relative);
            fs::create_dir_all(path.parent().unwrap()).unwrap();
            fs::write(&path, serde_json::to_string_pretty(value).unwrap()).unwrap();
            path
        }

        /// An Orca account the way Orca lays it out: its entry in the settings and a folder with `.claude.json`.
        fn claude_account(&self, id: &str, email: &str, uuid: &str) -> Value {
            let home = self.home().join(format!("orca/claude-profiles/{id}/home"));
            fs::create_dir_all(&home).unwrap();
            fs::write(
                home.join(".claude.json"),
                json!({ "numStartups": 3, "oauthAccount": profile(email, uuid) }).to_string(),
            )
            .unwrap();
            json!({ "id": id, "email": email, "managedAuthPath": home, "managedAuthRuntime": "host", "authMethod": "subscription-oauth" })
        }
    }

    impl Drop for Sandbox {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.root);
        }
    }

    fn profile(email: &str, uuid: &str) -> Value {
        json!({
            "accountUuid": uuid, "emailAddress": email, "displayName": email.split('@').next().unwrap(),
            "organizationType": "claude_max", "organizationRateLimitTier": "default_claude_max_20x"
        })
    }

    fn credential(token: &str, expires: i64) -> Vec<u8> {
        json!({ "claudeAiOauth": { "accessToken": token, "refreshToken": "r", "expiresAt": expires, "scopes": [] } })
            .to_string()
            .into_bytes()
    }

    /// Orca's settings in `orca-data.json`, the way older Orca keeps them.
    fn orca_json(sandbox: &Sandbox, settings: Value) {
        let orca = "Library/Application Support/orca";
        sandbox.write(
            &format!("{orca}/orca-profile-index.json"),
            &json!({ "activeProfileId": "p1" }),
        );
        sandbox.write(
            &format!("{orca}/profiles/p1/orca-data.json"),
            &json!({ "repos": [], "settings": settings }),
        );
    }

    #[test]
    fn the_hash_matches_sha256_and_the_names_claude_code_gives_its_items() {
        assert_eq!(sha256_first_word(b""), 0xe3b0c442);
        assert_eq!(sha256_first_word(b"abc"), 0xba7816bf);
        // Two blocks.
        assert_eq!(
            sha256_first_word(b"abcdbcdecdefdefgefghfghighijhijkijkljklmklmnlmnomnopnopq"),
            0x248d6a61
        );
        // An Orca folder, and the name `shasum -a 256` gives (checked against the real items of a Mac).
        let dir = Path::new(
            "/Users/me/Library/Application Support/orca/claude-profiles/c0d6f999-f483-4d20-9236-ad5e91a6ceaa/home",
        );
        assert_eq!(
            claude_service(Some(dir)),
            "Claude Code-credentials-8180b995"
        );
        assert_eq!(claude_service(None), "Claude Code-credentials");
        assert_eq!(keychain_account("bdthai"), "bdthai");
        assert_eq!(keychain_account("CTS\\me"), "claude-code-user");
        assert_eq!(keychain_account(""), "claude-code-user");
    }

    /// `cargo test -p core-agents -- --ignored --nocapture this_macs_orca`: the accounts of the real Orca and the
    /// Keychain items they name. It only reads files, and no secret.
    #[test]
    #[ignore]
    fn this_macs_orca_accounts() {
        let home = PathBuf::from(std::env::var_os("HOME").unwrap());
        for a in list(&home) {
            println!("{a:?} {}", claude_service(Some(&a.home)));
        }
        println!("claude uses {:?}", cli_account_name(&home));
    }

    #[test]
    fn plans_are_worded_short() {
        assert_eq!(plan_label(&profile("a@b", "u")), "Max 20x");
        assert_eq!(
            plan_label(&json!({ "organizationType": "claude_pro" })),
            "Pro"
        );
        assert_eq!(plan_label(&json!({})), "");
    }

    #[test]
    fn the_accounts_come_from_orca_with_who_uses_them() {
        let sandbox = Sandbox::new("list");
        let one = sandbox.claude_account("a1", "one@x.com", "u1");
        let two = sandbox.claude_account("a2", "two@x.com", "u2");
        let wsl = json!({ "id": "a3", "email": "w@x.com", "managedAuthPath": "/w", "managedAuthRuntime": "wsl" });
        let codex_home = sandbox.home().join("orca/codex-accounts/c1/home");
        sandbox.write(
            "orca/codex-accounts/c1/home/auth.json",
            &json!({ "tokens": { "account_id": "acct-1" } }),
        );
        sandbox.write(
            ".codex/auth.json",
            &json!({ "tokens": { "account_id": "acct-1" } }),
        );
        orca_json(
            &sandbox,
            json!({
                "claudeManagedAccounts": [one, { "broken": true }, two, wsl],
                "activeClaudeManagedAccountId": "a2",
                "codexManagedAccounts": [{ "id": "c1", "email": "c@x.com", "managedHomePath": codex_home }],
                "activeCodexManagedAccountId": null
            }),
        );
        sandbox.write(
            ".claude.json",
            &json!({ "oauthAccount": profile("ONE@x.com", "u1") }),
        );
        let accounts = list(sandbox.home());
        let summary: Vec<_> = accounts
            .iter()
            .map(|a| {
                (
                    a.agent,
                    a.id.as_str(),
                    a.email.as_str(),
                    a.name.as_str(),
                    a.plan.as_str(),
                    a.orca,
                    a.cli,
                )
            })
            .collect();
        assert_eq!(
            summary,
            [
                (
                    Agent::Claude,
                    "a1",
                    "one@x.com",
                    "one",
                    "Max 20x",
                    false,
                    true
                ),
                (
                    Agent::Claude,
                    "a2",
                    "two@x.com",
                    "two",
                    "Max 20x",
                    true,
                    false
                ),
                (Agent::Codex, "c1", "c@x.com", "", "", false, true),
            ]
        );
        assert_eq!(cli_account_name(sandbox.home()).as_deref(), Some("ONE"));
    }

    #[test]
    fn newer_orca_keeps_its_settings_in_sqlite_and_that_is_read_first() {
        let sandbox = Sandbox::new("sqlite");
        let one = sandbox.claude_account("a1", "one@x.com", "u1");
        orca_json(&sandbox, json!({ "claudeManagedAccounts": [] }));
        let db = rusqlite::Connection::open(
            sandbox
                .home()
                .join("Library/Application Support/orca/profiles/p1/profile-state.db"),
        )
        .unwrap();
        db.execute_batch(
            "CREATE TABLE profile_state_documents (domain TEXT PRIMARY KEY, payload TEXT NOT NULL)",
        )
        .unwrap();
        db.execute(
            "INSERT INTO profile_state_documents VALUES ('settings', ?1)",
            [json!({ "claudeManagedAccounts": [one] }).to_string()],
        )
        .unwrap();
        drop(db);
        let accounts = list(sandbox.home());
        assert_eq!(accounts.len(), 1);
        assert_eq!(accounts[0].email, "one@x.com");
    }

    #[test]
    fn no_orca_and_odd_files_give_no_accounts_and_no_error() {
        let sandbox = Sandbox::new("empty");
        assert!(list(sandbox.home()).is_empty());
        orca_json(
            &sandbox,
            json!({ "claudeManagedAccounts": "nope", "codexManagedAccounts": [1, null] }),
        );
        assert!(list(sandbox.home()).is_empty());
        sandbox.write(
            "Library/Application Support/orca/orca-profile-index.json",
            &json!({ "activeProfileId": "../../x" }),
        );
        assert!(list(sandbox.home()).is_empty());
        assert_eq!(cli_account_name(sandbox.home()), None);
    }

    #[test]
    fn a_token_is_used_only_while_it_is_valid() {
        assert_eq!(
            access_token(&credential("t", 10_000_000), 0),
            Ok("t".into())
        );
        assert_eq!(
            access_token(&credential("t", 10_000), 0),
            Err(Unavailable::Expired)
        );
        assert_eq!(access_token(b"{}", 0), Err(Unavailable::NoSignIn));
        assert_eq!(access_token(b"not json", 0), Err(Unavailable::NoSignIn));
    }

    #[test]
    fn the_usage_answer_gives_both_windows() {
        // The shape of the endpoint's answer, as Orca and Claude Code read it.
        let body = r#"{"five_hour":{"utilization":23.0,"resets_at":"2026-10-22T08:00:00.611316+00:00"},
            "seven_day":{"utilization":140,"resets_at":null},"seven_day_opus":null,"extra_usage":{"is_enabled":false}}"#;
        let limits = limits_from_usage(body, 5).unwrap();
        let five = limits.primary.unwrap();
        assert_eq!((five.used_percent, five.window_minutes), (23.0, Some(300)));
        assert_eq!(
            five.resets_at,
            parse_rfc3339_ms("2026-10-22T08:00:00Z").map(|ms| ms / 1000)
        );
        let week = limits.secondary.unwrap();
        assert_eq!(
            (week.used_percent, week.window_minutes, week.resets_at),
            (100.0, Some(10_080), None)
        );
        assert_eq!(limits.observed_ms, 5);
        assert!(limits_from_usage(r#"{"five_hour":null}"#, 0).is_none());
        assert!(limits_from_usage("<html>", 0).is_none());
    }

    #[test]
    fn a_switch_moves_the_sign_in_and_the_account_and_restore_puts_them_back() {
        let sandbox = Sandbox::new("switch");
        let entry = sandbox.claude_account("a1", "one@x.com", "u1");
        orca_json(&sandbox, json!({ "claudeManagedAccounts": [entry] }));
        let config = sandbox.write(
            ".claude.json",
            &json!({ "numStartups": 9, "oauthAccount": profile("old@x.com", "u0"), "projects": {} }),
        );
        fs::set_permissions(&config, fs::Permissions::from_mode(0o600)).unwrap();
        let keychain = FakeKeychain::default();
        let account = list(sandbox.home()).remove(0);
        keychain
            .write(
                &claude_service(Some(&account.home)),
                "me",
                &credential("new", 1),
            )
            .unwrap();
        keychain
            .write(CLAUDE_SERVICE, "me", &credential("old", 1))
            .unwrap();
        assert!(!can_restore(&sandbox.places, Agent::Claude));

        switch(&sandbox.places, &account, &keychain, 1_000).unwrap();
        assert_eq!(
            keychain.read(CLAUDE_SERVICE, "me").unwrap(),
            Some(credential("new", 1))
        );
        let doc = install::read_json(&config).unwrap();
        assert_eq!(doc["oauthAccount"]["emailAddress"], "one@x.com");
        let keys: Vec<_> = doc.as_object().unwrap().keys().cloned().collect();
        assert_eq!(
            keys,
            ["numStartups", "oauthAccount", "projects"],
            "every key stays, in its place"
        );
        assert_eq!(
            fs::metadata(&config).unwrap().permissions().mode() & 0o777,
            0o600
        );
        assert!(list(sandbox.home())[0].cli);
        let saved = backups(&sandbox.places.backups, Agent::Claude);
        assert_eq!(saved.len(), 1);
        assert_eq!(
            fs::metadata(&saved[0]).unwrap().permissions().mode() & 0o777,
            0o600
        );
        assert_eq!(
            fs::metadata(&sandbox.places.backups)
                .unwrap()
                .permissions()
                .mode()
                & 0o777,
            0o700
        );

        restore(&sandbox.places, Agent::Claude, &keychain).unwrap();
        assert_eq!(
            keychain.read(CLAUDE_SERVICE, "me").unwrap(),
            Some(credential("old", 1))
        );
        assert_eq!(
            install::read_json(&config).unwrap()["oauthAccount"]["emailAddress"],
            "old@x.com"
        );
        assert!(!can_restore(&sandbox.places, Agent::Claude));
        assert!(restore(&sandbox.places, Agent::Claude, &keychain).is_err());
    }

    #[test]
    fn a_switch_without_a_sign_in_or_with_a_broken_file_changes_nothing() {
        let sandbox = Sandbox::new("refuse");
        let entry = sandbox.claude_account("a1", "one@x.com", "u1");
        orca_json(&sandbox, json!({ "claudeManagedAccounts": [entry] }));
        let account = list(sandbox.home()).remove(0);
        let keychain = FakeKeychain::default();
        keychain
            .write(CLAUDE_SERVICE, "me", &credential("old", 1))
            .unwrap();
        assert!(
            switch(&sandbox.places, &account, &keychain, 1).is_err(),
            "no sign-in for the account"
        );
        keychain
            .write(
                &claude_service(Some(&account.home)),
                "me",
                &credential("new", 1),
            )
            .unwrap();
        let config = sandbox.home().join(".claude.json");
        fs::write(&config, "{ broken").unwrap();
        assert!(
            switch(&sandbox.places, &account, &keychain, 1).is_err(),
            "an unreadable ~/.claude.json"
        );
        assert_eq!(fs::read_to_string(&config).unwrap(), "{ broken");
        assert_eq!(
            keychain.read(CLAUDE_SERVICE, "me").unwrap(),
            Some(credential("old", 1))
        );
        assert!(!can_restore(&sandbox.places, Agent::Claude));
    }

    #[test]
    fn a_first_switch_with_nothing_before_restores_to_nothing() {
        let sandbox = Sandbox::new("fresh");
        let entry = sandbox.claude_account("a1", "one@x.com", "u1");
        orca_json(&sandbox, json!({ "claudeManagedAccounts": [entry] }));
        let account = list(sandbox.home()).remove(0);
        let keychain = FakeKeychain::default();
        keychain
            .write(
                &claude_service(Some(&account.home)),
                "me",
                &credential("new", 1),
            )
            .unwrap();
        switch(&sandbox.places, &account, &keychain, 1).unwrap();
        restore(&sandbox.places, Agent::Claude, &keychain).unwrap();
        assert_eq!(keychain.read(CLAUDE_SERVICE, "me").unwrap(), None);
        assert!(
            install::read_json(&sandbox.home().join(".claude.json"))
                .unwrap()
                .get("oauthAccount")
                .is_none()
        );
    }

    #[test]
    fn only_the_last_few_backups_are_kept() {
        let sandbox = Sandbox::new("keep");
        for n in 0..8 {
            save_backup(
                &sandbox.places.backups,
                Agent::Claude,
                n,
                &json!({ "credential": null }),
            )
            .unwrap();
        }
        save_backup(
            &sandbox.places.backups,
            Agent::Codex,
            99,
            &json!({ "auth": null }),
        )
        .unwrap();
        let kept = backups(&sandbox.places.backups, Agent::Claude);
        assert_eq!(kept.len(), BACKUPS_KEPT);
        assert!(kept[0].ends_with("claude-0000000000003.json"));
        assert_eq!(backups(&sandbox.places.backups, Agent::Codex).len(), 1);
    }

    #[test]
    fn a_codex_switch_copies_auth_json_and_restore_puts_the_old_one_back() {
        let sandbox = Sandbox::new("codex");
        let codex_home = sandbox.home().join("orca/codex-accounts/c1/home");
        sandbox.write(
            "orca/codex-accounts/c1/home/auth.json",
            &json!({ "tokens": { "account_id": "acct-1" } }),
        );
        orca_json(
            &sandbox,
            json!({ "codexManagedAccounts": [{ "id": "c1", "managedHomePath": codex_home }] }),
        );
        let account = list(sandbox.home()).remove(0);
        let keychain = FakeKeychain::default();
        assert!(
            switch(&sandbox.places, &account, &keychain, 1).is_err(),
            "no ~/.codex"
        );
        let target = sandbox.write(
            ".codex/auth.json",
            &json!({ "tokens": { "account_id": "acct-0" } }),
        );
        let before = fs::read_to_string(&target).unwrap();
        switch(&sandbox.places, &account, &keychain, 2).unwrap();
        assert!(list(sandbox.home())[0].cli);
        assert_eq!(
            fs::metadata(&target).unwrap().permissions().mode() & 0o777,
            0o600
        );
        restore(&sandbox.places, Agent::Codex, &keychain).unwrap();
        assert_eq!(fs::read_to_string(&target).unwrap(), before);
    }
}
