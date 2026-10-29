//! The browser: the Figghost Companion extension for the browsers built on Chromium, and the bridge it talks to. While a
//! Pomodoro work stage runs, the sites the person lists are blocked in the browser; the extension's button and menu send
//! a page or a link to the video download. It is off until the person turns it on.
//!
//! The extension talks only to a program on this Mac, through native messaging: the browser starts this app's own
//! program with the extension's origin (`host.rs`), which asks the running app over a local socket, `browser.sock` in
//! the data directory, open to this user alone. Installing the bridge writes one small file into the folder of each
//! browser the person picks, naming this program for this extension, and removing it takes that file away. The
//! extension itself is written to the folder `browser-extension` of the data directory, to be loaded by the person.
//! Remove all, which works while the feature is off too, takes the file away from every browser and the folder with it;
//! the extension loaded in a browser is removed there, by the person.
//! The extension reads no page, blocks with the browser's own rules (no access to any site is asked), and nothing
//! leaves the Mac.

use super::{AttachEnv, download, islands, pomodoro, write_atomically};
use crate::{
    i18n::{self, Language},
    settings::model::Fault,
    trace,
};
use logic::Ask;
use macos_sys::{
    app,
    notch::{Activity, Detail, Ear, Notch},
    style::Tone,
    workspace,
};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::{
    cell::RefCell,
    fs,
    io::{BufRead, BufReader, Read, Write},
    os::unix::{
        fs::PermissionsExt,
        net::{UnixListener, UnixStream},
    },
    path::{Path, PathBuf},
    rc::{Rc, Weak},
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
        mpsc,
    },
    thread,
    time::{Duration, Instant},
};

pub mod host;
pub mod logic;
pub mod settings;
mod text;

/// How often what to block is looked at again, on the main thread, while the feature is on.
const REFRESH_SECONDS: f64 = 2.0;
/// A host that asked within this long is a browser with the extension running.
const CONNECTED: Duration = Duration::from_secs(6);
const SLOT: &str = "browser";
const PRIORITY: u8 = 25;

/// The extension, as it is written to the data directory for the person to load.
const EXTENSION: [(&str, &[u8]); 8] = [
    (
        "manifest.json",
        include_bytes!("../../../assets/browser-extension/manifest.json"),
    ),
    (
        "background.js",
        include_bytes!("../../../assets/browser-extension/background.js"),
    ),
    (
        "_locales/en/messages.json",
        include_bytes!("../../../assets/browser-extension/_locales/en/messages.json"),
    ),
    (
        "_locales/vi/messages.json",
        include_bytes!("../../../assets/browser-extension/_locales/vi/messages.json"),
    ),
    (
        "icon16.png",
        include_bytes!("../../../assets/browser-extension/icon16.png"),
    ),
    (
        "icon48.png",
        include_bytes!("../../../assets/browser-extension/icon48.png"),
    ),
    (
        "icon128.png",
        include_bytes!("../../../assets/browser-extension/icon128.png"),
    ),
    (
        "README.txt",
        include_bytes!("../../../assets/browser-extension/README.txt"),
    ),
];

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    pub enabled: bool,
    /// Block the sites while a Pomodoro work stage runs.
    pub block: bool,
    pub sites: Vec<String>,
    /// The extension's button and menu send a page to the video download.
    pub download: bool,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            enabled: false,
            block: true,
            sites: logic::DEFAULT_SITES.map(String::from).to_vec(),
            download: true,
        }
    }
}

impl Config {
    /// A config file can say anything; only sites that are sites are kept, once each.
    pub fn sanitized(mut self) -> Self {
        self.sites = logic::sites(&self.sites.join("\n"));
        self
    }
}

/// What the socket answers with, kept up to date by the main thread.
#[derive(Debug, Default)]
struct Shared {
    block: bool,
    sites: Vec<String>,
    download: bool,
    asked: Option<Instant>,
}

/// The socket while it is open: the flag that stops it, and where it is.
struct Serving {
    stop: Arc<AtomicBool>,
    path: PathBuf,
}

impl Drop for Serving {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::SeqCst);
        // The thread waits in accept; a connection wakes it to see the flag.
        let _ = UnixStream::connect(&self.path);
        let _ = fs::remove_file(&self.path);
    }
}

thread_local! {
    /// An address to download arrives on the socket's thread and hops to the main one, which finds the feature here.
    static FEATURE: RefCell<Weak<Feature>> = const { RefCell::new(Weak::new()) };
}

pub struct Feature {
    home: PathBuf,
    notch: Option<Weak<Notch>>,
    config: RefCell<Config>,
    pomodoro: Option<Weak<pomodoro::Feature>>,
    download: Option<Weak<download::Feature>>,
    shared: Arc<Mutex<Shared>>,
    serving: RefCell<Option<Serving>>,
    refresh: RefCell<Option<app::Timer>>,
    me: Weak<Feature>,
}

/// Starts the feature, with the Pomodoro whose work stages block and the download that takes the addresses.
pub fn attach(
    env: &AttachEnv,
    pomodoro: Option<&Rc<pomodoro::Feature>>,
    download: Option<&Rc<download::Feature>>,
) -> Option<Rc<Feature>> {
    let feature = Rc::new_cyclic(|me| Feature {
        home: env.home.to_path_buf(),
        notch: env.notch.map(Rc::downgrade),
        config: RefCell::new(env.config.browser.clone().sanitized()),
        pomodoro: pomodoro.map(Rc::downgrade),
        download: download.map(Rc::downgrade),
        shared: Arc::default(),
        serving: RefCell::default(),
        refresh: RefCell::default(),
        me: me.clone(),
    });
    FEATURE.with(|slot| *slot.borrow_mut() = Rc::downgrade(&feature));
    if feature.config.borrow().enabled {
        feature.start();
    }
    Some(feature)
}

fn with_feature<R>(f: impl FnOnce(&Feature) -> R) -> Option<R> {
    FEATURE.with(|slot| slot.borrow().upgrade().map(|feature| f(&feature)))
}

/// The person's `~/Library/Application Support`, where the browsers keep their folders.
fn support_dir() -> PathBuf {
    std::env::var_os("HOME")
        .map(PathBuf::from)
        .unwrap_or_default()
        .join("Library/Application Support")
}

/// This program, as a browser is to start it.
fn program() -> Option<PathBuf> {
    std::env::current_exe()
        .ok()
        .and_then(|p| p.canonicalize().ok())
}

impl Feature {
    pub fn configure(&self, config: &Config) -> Result<(), Fault> {
        let config = config.clone().sanitized();
        let was = self.config.borrow().enabled;
        *self.config.borrow_mut() = config.clone();
        match (was, config.enabled) {
            (false, true) => self.start(),
            (true, false) => self.stop(),
            (true, true) => self.refresh(),
            _ => {}
        }
        Ok(())
    }

    fn start(&self) {
        let path = core_agents::named_socket(&self.home, "browser");
        let _ = fs::remove_file(&path);
        if let Some(dir) = path.parent() {
            let _ = fs::create_dir_all(dir);
        }
        match UnixListener::bind(&path) {
            Ok(listener) => {
                let _ = fs::set_permissions(&path, fs::Permissions::from_mode(0o600));
                let stop = Arc::new(AtomicBool::new(false));
                let (flag, shared) = (Arc::clone(&stop), Arc::clone(&self.shared));
                thread::spawn(move || serve(&listener, &flag, &shared));
                *self.serving.borrow_mut() = Some(Serving { stop, path });
                trace::log("browser: listening");
            }
            Err(e) => eprintln!("browser: cannot open its socket: {e}"),
        }
        self.refresh();
        let weak = self.me.clone();
        *self.refresh.borrow_mut() = Some(app::every(REFRESH_SECONDS, move || {
            if let Some(f) = weak.upgrade() {
                f.refresh();
            }
        }));
    }

    fn stop(&self) {
        self.refresh.borrow_mut().take();
        self.serving.borrow_mut().take();
        *self.shared.lock().unwrap_or_else(|e| e.into_inner()) = Shared::default();
        trace::log("browser: stopped");
    }

    /// What the socket says from now on: the sites while a work stage runs, and whether addresses are taken.
    fn refresh(&self) {
        let config = self.config.borrow();
        let working = self
            .pomodoro
            .as_ref()
            .and_then(Weak::upgrade)
            .is_some_and(|p| p.is_working());
        let mut shared = self.shared.lock().unwrap_or_else(|e| e.into_inner());
        shared.block = config.enabled && config.block && working;
        shared.sites.clone_from(&config.sites);
        shared.download = config.enabled && config.download;
    }

    /// An address from the extension, on the main thread: the video download takes it, or says why not on the notch.
    fn download(&self, url: &str) -> Result<(), String> {
        let result = match self.download.as_ref().and_then(Weak::upgrade) {
            Some(download) => download
                .submit(url)
                .map(|_| ())
                .map_err(|f| f.code.to_string()),
            None => Err("download_off".to_owned()),
        };
        if let Err(code) = &result {
            let language = i18n::current();
            self.say(
                language.browser_download_failed(),
                language.browser_why(code),
            );
        }
        result
    }

    fn say(&self, title: &str, hint: &str) {
        if let Some(notch) = self.notch.as_ref().and_then(Weak::upgrade) {
            islands::show_for(
                &notch,
                SLOT,
                Activity {
                    left: Ear::Symbol {
                        name: "safari",
                        tone: Tone::Orange,
                        pulse: false,
                    },
                    right: Ear::Empty,
                    detail: Some(Detail {
                        title: title.to_owned(),
                        subtitle: hint.to_owned(),
                    }),
                    page: None,
                    priority: PRIORITY,
                },
                Some(4.0),
            );
        }
    }

    /// Facts only the running app knows, for the settings page.
    pub fn runtime(&self) -> Value {
        let support = support_dir();
        let program = program();
        let browsers: Vec<Value> = logic::BROWSERS
            .iter()
            .filter_map(|(id, name, _)| {
                let dir = logic::browser_dir(&support, id)?;
                let installed = program.as_ref().is_some_and(|p| {
                    fs::read_to_string(logic::manifest_path(&dir))
                        .is_ok_and(|text| logic::names_program(&text, p))
                });
                Some(json!({ "id": id, "name": name, "found": dir.is_dir(), "installed": installed }))
            })
            .collect();
        let shared = self.shared.lock().unwrap_or_else(|e| e.into_inner());
        json!({
            "browsers": browsers,
            "connected": shared.asked.is_some_and(|at| at.elapsed() < CONNECTED),
            "blocking": shared.block,
            "folder": self.extension_dir(),
        })
    }

    fn extension_dir(&self) -> PathBuf {
        self.home.join("browser-extension")
    }

    /// Writes the extension, as this version of the app has it, into its folder.
    fn write_extension(&self) -> Result<PathBuf, Fault> {
        let dir = self.extension_dir();
        for (name, bytes) in EXTENSION {
            let path = dir.join(name);
            if let Some(parent) = path.parent() {
                fs::create_dir_all(parent)
                    .map_err(|e| Fault::new("write_failed", e.to_string()))?;
            }
            fs::write(&path, bytes).map_err(|e| Fault::new("write_failed", e.to_string()))?;
        }
        Ok(dir)
    }

    /// The buttons of the settings page; the browser after the colon is checked against the list.
    pub fn action(&self, name: &str) -> Option<Result<(), Fault>> {
        let (verb, id) = name.split_once(':').unwrap_or((name, ""));
        Some(match verb {
            "browser_folder" => self.write_extension().map(|dir| workspace::reveal(&dir)),
            "browser_install" => self.install(id, true),
            "browser_remove" => self.install(id, false),
            "browser_remove_all" => self.remove_all(),
            _ => return None,
        })
    }

    /// Takes the bridge away from every browser, and the folder of the extension: all the app put outside itself.
    fn remove_all(&self) -> Result<(), Fault> {
        let failed = remove_everywhere(&support_dir(), &self.extension_dir());
        trace::log("browser: removed everywhere");
        if failed.is_empty() {
            Ok(())
        } else {
            Err(Fault::new("write_failed", failed.join("; ")))
        }
    }

    /// Writes or removes the file that names this program as the bridge, in one browser's folder.
    fn install(&self, id: &str, on: bool) -> Result<(), Fault> {
        let dir = logic::browser_dir(&support_dir(), id)
            .filter(|d| d.is_dir())
            .ok_or_else(|| Fault::new("invalid_value", id))?;
        let path = logic::manifest_path(&dir);
        if !on {
            return match fs::remove_file(&path) {
                Err(e) if e.kind() != std::io::ErrorKind::NotFound => {
                    Err(Fault::new("write_failed", e.to_string()))
                }
                _ => Ok(()),
            };
        }
        let program = program().ok_or_else(|| Fault::new("write_failed", "no program"))?;
        write_atomically(&path, &logic::host_manifest(&program))
            .map_err(|e| Fault::new("write_failed", e.to_string()))?;
        trace::log("browser: bridge installed");
        Ok(())
    }
}

/// Removes the file of the bridge from the folder of every browser under `support`, and the folder of the extension;
/// what could not be removed is said, and what was not there is no fault. Nothing else in those folders is touched.
fn remove_everywhere(support: &Path, extension: &Path) -> Vec<String> {
    let mut failed = Vec::new();
    for (id, _, _) in logic::BROWSERS {
        if let Some(dir) = logic::browser_dir(support, id)
            && let Err(e) = fs::remove_file(logic::manifest_path(&dir))
            && e.kind() != std::io::ErrorKind::NotFound
        {
            failed.push(format!("{id}: {e}"));
        }
    }
    if let Err(e) = fs::remove_dir_all(extension)
        && e.kind() != std::io::ErrorKind::NotFound
    {
        failed.push(format!("extension: {e}"));
    }
    failed
}

/// The socket's thread: one question at a time, each held to a short wait, until the flag is set.
fn serve(listener: &UnixListener, stop: &AtomicBool, shared: &Mutex<Shared>) {
    for stream in listener.incoming() {
        if stop.load(Ordering::SeqCst) {
            break;
        }
        if let Ok(stream) = stream {
            answer(stream, shared);
        }
    }
}

fn answer(stream: UnixStream, shared: &Mutex<Shared>) {
    let _ = stream.set_read_timeout(Some(Duration::from_secs(2)));
    let _ = stream.set_write_timeout(Some(Duration::from_secs(2)));
    let mut line = String::new();
    if BufReader::new((&stream).take(logic::MAX_MESSAGE as u64))
        .read_line(&mut line)
        .is_err()
    {
        return;
    }
    let reply = match Ask::parse(&line) {
        Some(Ask::State) => {
            let mut s = shared.lock().unwrap_or_else(|e| e.into_inner());
            s.asked = Some(Instant::now());
            json!({ "block": s.block, "sites": s.sites })
        }
        Some(Ask::Download(url)) => {
            if shared.lock().unwrap_or_else(|e| e.into_inner()).download {
                // The download lives on the main thread; its answer is waited for briefly.
                let (tx, rx) = mpsc::channel();
                app::on_main(move || {
                    let result = with_feature(|f| f.download(&url))
                        .unwrap_or_else(|| Err("download_off".to_owned()));
                    let _ = tx.send(result);
                });
                match rx.recv_timeout(Duration::from_secs(2)) {
                    Ok(Ok(())) => json!({ "ok": true }),
                    Ok(Err(code)) => json!({ "ok": false, "error": code }),
                    Err(_) => json!({ "ok": false, "error": "busy" }),
                }
            } else {
                json!({ "ok": false, "error": "off" })
            }
        }
        None => json!({ "error": "unknown" }),
    };
    let _ = writeln!(&stream, "{reply}");
}

impl Language {
    fn browser_why(self, code: &str) -> &'static str {
        match code {
            "download_missing" => self.browser_why_missing(),
            "download_bad_url" => self.browser_why_url(),
            _ => self.browser_why_off(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_feature_is_off_by_default_with_a_list_to_start_from_and_a_file_keeps_only_sites() {
        let config: Config = serde_json::from_str("{}").unwrap();
        assert_eq!(config, Config::default());
        assert!(!config.enabled && config.block && config.download);
        assert_eq!(config.sites.len(), logic::DEFAULT_SITES.len());
        let file: Config = serde_json::from_str(
            r#"{ "sites": ["https://www.YouTube.com/x", "nonsense", "youtube.com"] }"#,
        )
        .unwrap();
        assert_eq!(file.sanitized().sites, ["youtube.com"]);
    }

    #[test]
    fn every_file_of_the_extension_is_there_and_the_manifest_names_the_fixed_id() {
        let manifest: Value = serde_json::from_slice(EXTENSION[0].1).unwrap();
        assert_eq!(manifest["manifest_version"], 3);
        let key = manifest["key"].as_str().unwrap();
        // The ID Chrome gives an extension is the first half of the SHA-256 of its public key, in the letters a to p.
        let der = base64_decode(key);
        let digest = sha256(&der);
        let id: String = digest[..16]
            .iter()
            .flat_map(|b| [b >> 4, b & 15])
            .map(|n| char::from(b'a' + n))
            .collect();
        assert_eq!(id, logic::EXTENSION_ID);
        let permissions = manifest["permissions"].as_array().unwrap();
        assert!(permissions.iter().any(|p| p == "nativeMessaging"));
        assert!(
            manifest.get("host_permissions").is_none(),
            "no access to any site is asked"
        );
        for (name, bytes) in EXTENSION {
            assert!(!bytes.is_empty(), "{name}");
        }
    }

    #[test]
    fn the_socket_answers_the_state_and_refuses_what_it_does_not_know() {
        let dir = std::env::temp_dir().join(format!("figghost-browser-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        let path = dir.join("b.sock");
        let listener = UnixListener::bind(&path).unwrap();
        let shared = Arc::new(Mutex::new(Shared {
            block: true,
            sites: vec!["x.com".into()],
            download: false,
            asked: None,
        }));
        let stop = Arc::new(AtomicBool::new(false));
        let serving = Serving {
            stop: Arc::clone(&stop),
            path: path.clone(),
        };
        let (flag, state) = (Arc::clone(&stop), Arc::clone(&shared));
        let thread = thread::spawn(move || serve(&listener, &flag, &state));
        let answer = host::ask(&path, &Ask::State).unwrap();
        assert_eq!(answer, json!({ "block": true, "sites": ["x.com"] }));
        assert!(
            shared.lock().unwrap().asked.is_some(),
            "a host that asks is seen"
        );
        let refused = host::ask(&path, &Ask::Download("https://a.com/v".into())).unwrap();
        assert_eq!(refused, json!({ "ok": false, "error": "off" }));
        drop(serving);
        thread.join().unwrap();
        assert!(!path.exists(), "the socket goes with it");
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn removing_everywhere_takes_only_the_bridge_and_the_extension_and_minds_nothing_missing() {
        let root = std::env::temp_dir().join(format!("figghost-remove-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        let support = root.join("Application Support");
        let edge = logic::browser_dir(&support, "edge").unwrap();
        let ours = logic::manifest_path(&edge);
        fs::create_dir_all(ours.parent().unwrap()).unwrap();
        fs::write(&ours, logic::host_manifest(Path::new("/x"))).unwrap();
        let theirs = ours.with_file_name("com.other.host.json");
        fs::write(&theirs, "{}").unwrap();
        let extension = root.join("browser-extension");
        fs::create_dir_all(extension.join("_locales/en")).unwrap();
        fs::write(extension.join("manifest.json"), "{}").unwrap();
        assert!(remove_everywhere(&support, &extension).is_empty());
        assert!(!ours.exists() && !extension.exists());
        assert!(theirs.exists(), "another app's host is left alone");
        assert!(
            remove_everywhere(&support, &extension).is_empty(),
            "twice is no fault"
        );
        fs::remove_dir_all(&root).unwrap();
    }

    fn base64_decode(text: &str) -> Vec<u8> {
        let value = |c: u8| match c {
            b'A'..=b'Z' => c - b'A',
            b'a'..=b'z' => c - b'a' + 26,
            b'0'..=b'9' => c - b'0' + 52,
            b'+' => 62,
            _ => 63,
        };
        let clean: Vec<u8> = text.bytes().filter(|c| *c != b'=').collect();
        clean
            .chunks(4)
            .flat_map(|chunk| {
                let n = chunk
                    .iter()
                    .enumerate()
                    .fold(0u32, |n, (i, c)| n | u32::from(value(*c)) << (18 - 6 * i));
                let bytes = [(n >> 16) as u8, (n >> 8) as u8, n as u8];
                bytes[..chunk.len() - 1].to_vec()
            })
            .collect()
    }

    fn sha256(data: &[u8]) -> Vec<u8> {
        let out = std::process::Command::new("/usr/bin/shasum")
            .args(["-a", "256"])
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .spawn()
            .and_then(|mut child| {
                child.stdin.take().unwrap().write_all(data)?;
                child.wait_with_output()
            })
            .unwrap();
        let hex = String::from_utf8(out.stdout).unwrap();
        (0..32)
            .map(|i| u8::from_str_radix(&hex[2 * i..2 * i + 2], 16).unwrap())
            .collect()
    }
}
