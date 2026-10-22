//! The Accounts section of the AI agents settings page: the accounts Orca keeps, what is left of their limits,
//! and switching the account of the plain `claude` and `codex` commands (see `core_agents::accounts`).
//!
//! The limits are asked from Anthropic with each account's own sign-in, which is read from the Keychain. macOS
//! asks the user before it lets the app read an item Claude Code made, so nothing is read until the refresh
//! button is pressed; after that the limits are asked again at most every five minutes while the page is on
//! the AI agents page. Everything that may wait for the user or the network runs on a thread of its own.

use crate::{agents::usage_row, history::ago, i18n::Language};
use core_agents::{
    Agent,
    accounts::{self, Account, Places, Secrets, Unavailable},
    usage::Limits,
};
use core_smart::net;
use macos_sys::keychain;
use serde_json::{Value, json};
use std::{
    cell::{Cell, RefCell},
    collections::HashMap,
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
};

/// The list is read again at most this often while the page shows it.
const LIST_MS: i64 = 10_000;
/// The limits are asked again at most this often by themselves.
const LIMITS_MS: i64 = 5 * 60_000;
const FETCH_SECONDS: u64 = 15;

struct KeychainSecrets;

fn keychain_error(status: i32) -> String {
    if keychain::REFUSED.contains(&status) {
        "the Keychain did not let Figghost in".to_owned()
    } else {
        format!("the Keychain refused (status {status})")
    }
}

impl Secrets for KeychainSecrets {
    fn read(&self, service: &str, account: &str) -> Result<Option<Vec<u8>>, String> {
        keychain::read(service, account).map_err(keychain_error)
    }
    fn write(&self, service: &str, account: &str, data: &[u8]) -> Result<(), String> {
        keychain::write(service, account, data).map_err(keychain_error)
    }
    fn delete(&self, service: &str, account: &str) -> Result<(), String> {
        keychain::delete(service, account).map_err(keychain_error)
    }
}

/// What the threads leave for the page.
#[derive(Default)]
struct Shared {
    limits: HashMap<String, Result<Limits, Unavailable>>,
    asked_ms: i64,
    /// Limits are being asked for, or a switch or a restore runs.
    busy: bool,
    /// How the last switch or restore went: what was done, or why it was not.
    outcome: Option<Result<&'static str, String>>,
}

#[derive(Default)]
struct Listing {
    read_ms: i64,
    accounts: Vec<Account>,
    /// The email of the account the plain `claude` command uses.
    cli: String,
    restore_claude: bool,
    restore_codex: bool,
}

pub struct Accounts {
    places: Places,
    /// A private instance (`FIGGHOST_HOME` set, how every test runs) never reads or writes the real Keychain.
    keychain: bool,
    shared: Arc<Mutex<Shared>>,
    listing: RefCell<Listing>,
    /// The settings page is on the AI agents page.
    showing: Cell<bool>,
    /// The refresh button was pressed once, so the user expects macOS to ask about the Keychain.
    wanted: Cell<bool>,
}

impl Accounts {
    pub fn new(data_home: &Path) -> Self {
        let user_home = crate::agents::agents_home(
            std::env::var_os("FIGGHOST_AGENTS_HOME"),
            std::env::var_os("FIGGHOST_HOME").map(|_| data_home),
            std::env::var_os("HOME"),
        );
        Accounts {
            places: Places {
                user_home,
                backups: data_home.join(accounts::BACKUP_DIR),
                keychain_account: accounts::keychain_account(
                    &std::env::var("USER").unwrap_or_default(),
                ),
            },
            keychain: std::env::var_os("FIGGHOST_HOME").is_none(),
            shared: Arc::default(),
            listing: RefCell::default(),
            showing: Cell::new(false),
            wanted: Cell::new(false),
        }
    }

    /// The page the settings window shows; only the AI agents page reads the accounts.
    pub fn set_page(&self, id: &str) {
        self.showing.set(id == "agents");
    }

    fn relist(&self, now: i64, force: bool) {
        if !force && now - self.listing.borrow().read_ms < LIST_MS {
            return;
        }
        let home = &self.places.user_home;
        let cli = core_agents::install::read_json(&home.join(".claude.json"))
            .ok()
            .and_then(|doc| {
                doc.get("oauthAccount")?
                    .get("emailAddress")?
                    .as_str()
                    .map(str::to_owned)
            })
            .unwrap_or_default();
        *self.listing.borrow_mut() = Listing {
            read_ms: now,
            accounts: accounts::list(home),
            cli,
            restore_claude: accounts::can_restore(&self.places, Agent::Claude),
            restore_codex: accounts::can_restore(&self.places, Agent::Codex),
        };
    }

    /// The section's state for the page.
    pub fn snapshot(&self, language: Language, now: i64) -> Value {
        if self.showing.get() {
            self.relist(now, false);
            if self.wanted.get() {
                self.fetch(now, false);
            }
        }
        let shared = self.shared.lock().unwrap_or_else(|e| e.into_inner());
        to_json(
            &self.listing.borrow(),
            &shared,
            self.keychain,
            language,
            now,
        )
    }

    /// Asks for the limits of every Claude account on a thread, unless that is running or was done lately.
    /// An account whose Keychain item the user refused is left alone until they press refresh.
    fn fetch(&self, now: i64, force: bool) {
        if !self.keychain {
            return;
        }
        let mut shared = self.shared.lock().unwrap_or_else(|e| e.into_inner());
        if shared.busy || (!force && now - shared.asked_ms < LIMITS_MS) {
            return;
        }
        let targets: Vec<(String, PathBuf)> = self
            .listing
            .borrow()
            .accounts
            .iter()
            .filter(|a| a.agent == Agent::Claude)
            .filter(|a| force || shared.limits.get(&a.id) != Some(&Err(Unavailable::Refused)))
            .map(|a| (a.id.clone(), a.home.clone()))
            .collect();
        shared.busy = true;
        shared.asked_ms = now;
        drop(shared);
        let (store, user) = (
            Arc::clone(&self.shared),
            self.places.keychain_account.clone(),
        );
        self.spawn(move || {
            for (id, home) in targets {
                let result = limits_of(&home, &user);
                store
                    .lock()
                    .unwrap_or_else(|e| e.into_inner())
                    .limits
                    .insert(id, result);
            }
        });
    }

    /// Runs `work` on a thread with the busy flag up, and puts it down afterwards.
    fn spawn(&self, work: impl FnOnce() + Send + 'static) {
        let shared = Arc::clone(&self.shared);
        let started = std::thread::Builder::new()
            .name("fig-accounts".into())
            .spawn(move || {
                macos_sys::qos::utility();
                work();
                shared.lock().unwrap_or_else(|e| e.into_inner()).busy = false;
            });
        if started.is_err() {
            self.shared.lock().unwrap_or_else(|e| e.into_inner()).busy = false;
        }
    }

    /// `accounts_refresh`, `accounts_switch:<id>` and `accounts_restore:<claude|codex>` from the page.
    pub fn action(&self, name: &str, now: i64) -> Result<(), String> {
        let (verb, arg) = name.split_once(':').unwrap_or((name, ""));
        match verb {
            "accounts_refresh" => {
                self.wanted.set(true);
                self.relist(now, true);
                self.fetch(now, true);
                Ok(())
            }
            "accounts_switch" | "accounts_restore" => {
                let account = (verb == "accounts_switch")
                    .then(|| {
                        self.listing
                            .borrow()
                            .accounts
                            .iter()
                            .find(|a| a.id == arg)
                            .cloned()
                    })
                    .flatten();
                let agent = match &account {
                    Some(account) => account.agent,
                    None => Agent::from_id(arg).ok_or("unknown account")?,
                };
                if agent == Agent::Claude && !self.keychain {
                    return Err("a private instance does not use the Keychain".into());
                }
                let mut shared = self.shared.lock().unwrap_or_else(|e| e.into_inner());
                if shared.busy {
                    return Err("busy, try again in a moment".into());
                }
                shared.busy = true;
                shared.outcome = None;
                drop(shared);
                let (store, places) = (Arc::clone(&self.shared), self.places.clone());
                let restoring = account.is_none();
                self.listing.borrow_mut().read_ms = 0;
                self.spawn(move || {
                    let result = match &account {
                        Some(account) => accounts::switch(
                            &places,
                            account,
                            &KeychainSecrets,
                            crate::history::now_ms(),
                        )
                        .map(|()| "switched"),
                        None => {
                            accounts::restore(&places, agent, &KeychainSecrets).map(|()| "restored")
                        }
                    };
                    if let Err(e) = &result {
                        eprintln!(
                            "accounts: {} failed: {e}",
                            if restoring { "restore" } else { "switch" }
                        );
                    }
                    store.lock().unwrap_or_else(|e| e.into_inner()).outcome = Some(result);
                });
                Ok(())
            }
            _ => Err(format!("unknown action {name}")),
        }
    }
}

/// The limits of one Claude account, asked with its own access token. A token that has run out is not renewed.
fn limits_of(home: &Path, user: &str) -> Result<Limits, Unavailable> {
    let credential = match keychain::read(&accounts::claude_service(Some(home)), user) {
        Ok(Some(credential)) => credential,
        Ok(None) => return Err(Unavailable::NoSignIn),
        Err(status) if keychain::REFUSED.contains(&status) => return Err(Unavailable::Refused),
        Err(_) => return Err(Unavailable::Failed),
    };
    let token = accounts::access_token(&credential, crate::history::now_ms())?;
    let bearer = format!("Bearer {token}");
    // The same headers Orca sends; curl reads them from its standard input, so the token is never in `ps`.
    let headers = [
        ("Authorization", bearer.as_str()),
        ("anthropic-beta", accounts::USAGE_BETA),
        ("User-Agent", "claude-code/2.1.0"),
    ];
    let body = net::get_with_headers(accounts::USAGE_URL, &[], &headers, FETCH_SECONDS)
        .map_err(|_| Unavailable::Failed)?;
    accounts::limits_from_usage(&body, crate::history::now_ms()).ok_or(Unavailable::Failed)
}

fn to_json(
    listing: &Listing,
    shared: &Shared,
    keychain: bool,
    language: Language,
    now: i64,
) -> Value {
    let entry = |account: &Account| {
        let (limits, note) = match shared.limits.get(&account.id) {
            Some(Ok(limits)) => {
                let rows = [&limits.primary, &limits.secondary].into_iter().flatten().map(|limit| {
                    let row = usage_row(language, account.agent, limit, limits.observed_ms, now);
                    let label = limit.window_minutes.map(|m| language.agents_window(m)).unwrap_or(row.label);
                    json!({ "label": label, "fraction": row.fraction, "value": row.value, "reset": row.reset })
                });
                (rows.collect::<Vec<_>>(), "")
            }
            Some(Err(why)) => (Vec::new(), why.code()),
            None => (Vec::new(), ""),
        };
        json!({
            "agent": account.agent.id(),
            "id": account.id,
            "email": account.email,
            "name": account.name,
            "plan": account.plan,
            "orca": account.orca,
            "cli": account.cli,
            "limits": limits,
            "note": note,
        })
    };
    json!({
        "list": listing.accounts.iter().map(entry).collect::<Vec<_>>(),
        "cli": listing.cli,
        "restore": { "claude": listing.restore_claude, "codex": listing.restore_codex },
        "busy": shared.busy,
        "updated": if shared.asked_ms > 0 { ago(language, now, shared.asked_ms) } else { String::new() },
        "outcome": match &shared.outcome {
            Some(Ok(done)) => json!({ "ok": true, "what": done, "detail": "" }),
            Some(Err(why)) => json!({ "ok": false, "what": "failed", "detail": why }),
            None => Value::Null,
        },
        "keychain": keychain,
    })
}

/// The section as the page gets it for one account with limits, for the preview data's shape check.
#[cfg(test)]
pub fn sample_json() -> Value {
    use core_agents::usage::Limit;
    let account = Account {
        agent: Agent::Claude,
        id: "a1".into(),
        email: "one@example.com".into(),
        name: "one".into(),
        plan: "Max 20x".into(),
        home: PathBuf::from("/tmp/a1"),
        orca: true,
        cli: false,
    };
    let limit = Limit {
        used_percent: 40.0,
        window_minutes: Some(300),
        resets_at: Some(4_000),
    };
    let shared = Shared {
        limits: HashMap::from([(
            "a1".to_owned(),
            Ok(Limits {
                primary: Some(limit),
                secondary: None,
                observed_ms: 1_000_000,
                plan: None,
            }),
        )]),
        asked_ms: 1_000_000,
        busy: false,
        outcome: Some(Ok("switched")),
    };
    let listing = Listing {
        read_ms: 0,
        accounts: vec![account],
        cli: "x@example.com".into(),
        restore_claude: true,
        restore_codex: false,
    };
    to_json(&listing, &shared, true, Language::English, 1_060_000)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_account_shows_its_windows_or_why_they_are_missing() {
        let sample = sample_json();
        let entry = &sample["list"][0];
        assert_eq!(entry["limits"][0]["label"], "5 h");
        assert_eq!(entry["limits"][0]["value"], "40%");
        assert_eq!(
            (entry["orca"].as_bool(), entry["cli"].as_bool()),
            (Some(true), Some(false))
        );
        assert_eq!(sample["restore"]["claude"], true);
        assert_eq!(sample["outcome"]["what"], "switched");
        assert_eq!(sample["updated"], "1 minute ago");

        let account = Account {
            id: "b".into(),
            ..blank_account()
        };
        let shared = Shared {
            limits: HashMap::from([("b".to_owned(), Err(Unavailable::Expired))]),
            ..Shared::default()
        };
        let listing = Listing {
            accounts: vec![account],
            ..Listing::default()
        };
        let value = to_json(&listing, &shared, true, Language::English, 0);
        assert_eq!(value["list"][0]["note"], "expired");
        assert_eq!(value["list"][0]["limits"].as_array().map(Vec::len), Some(0));
        assert_eq!(value["updated"], "");
        assert!(value["outcome"].is_null());
    }

    fn blank_account() -> Account {
        Account {
            agent: Agent::Claude,
            id: String::new(),
            email: String::new(),
            name: String::new(),
            plan: String::new(),
            home: PathBuf::new(),
            orca: false,
            cli: false,
        }
    }

    #[test]
    fn without_the_keychain_nothing_is_asked_and_unknown_actions_are_refused() {
        let root = std::env::temp_dir().join(format!("app-ui-accounts-{}", std::process::id()));
        let accounts = Accounts {
            places: Places {
                user_home: root.join("home"),
                backups: root.join(accounts::BACKUP_DIR),
                keychain_account: "me".into(),
            },
            keychain: false,
            shared: Arc::default(),
            listing: RefCell::default(),
            showing: Cell::new(true),
            wanted: Cell::new(false),
        };
        assert!(accounts.action("accounts_restore:claude", 0).is_err());
        assert!(accounts.action("accounts_switch:nope", 0).is_err());
        assert!(accounts.action("accounts_bogus", 0).is_err());
        accounts.action("accounts_refresh", 0).unwrap();
        assert!(
            !accounts.shared.lock().unwrap().busy,
            "nothing is asked without the Keychain"
        );
        assert_eq!(
            accounts.snapshot(Language::English, 0)["list"]
                .as_array()
                .map(Vec::len),
            Some(0)
        );
    }
}
