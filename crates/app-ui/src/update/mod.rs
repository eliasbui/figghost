//! Keeping the app up to date: a check every day, the choice to install what it finds without asking, and the
//! popup that says what is new. The controller lives on the main thread; the network and the disk work run on
//! a thread of their own and report back to it.
//!
//! An update replaces the app on disk while the old one keeps running. The new version starts with the next
//! launch, so a restart is all it takes.

mod install;
mod page;
mod release;

use crate::{
    config::Config as AppConfig,
    i18n,
    settings::{base64, relaunch},
    trace,
};
use install::Stage;
use macos_sys::{
    MainThreadMarker, app,
    webview::{WebWindow, WindowSpec},
    widgets::image_png,
    workspace,
};
use release::Release;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::{
    cell::{Cell, RefCell},
    fs,
    path::PathBuf,
    rc::{Rc, Weak},
    time::{SystemTime, UNIX_EPOCH},
};

/// The first check comes this long after the launch, so that the launch itself is not slowed down.
const FIRST_CHECK_SECONDS: f64 = 30.0;
/// How often the clock is looked at to see whether a check is due; the check itself is once a day.
const TICK_SECONDS: f64 = 3600.0;
const DAY_SECONDS: u64 = 24 * 60 * 60;
/// When the last answer came, in seconds since 1970, in the data folder.
const CHECKED_FILE: &str = "update-check";
const POPUP_WIDTH: f64 = 460.0;
const POPUP_HEIGHT: f64 = 540.0;
const POPUP_MIN_HEIGHT: f64 = 420.0;
const APP_ICON: &[u8] = include_bytes!("../../assets/Figghost.icns");
const ICON_POINTS: f64 = 64.0;
const ICON_SCALE: f64 = 2.0;

/// The settings of updating. Looking for a new version has none: it happens every day.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    /// Install a new version as soon as it is found instead of asking first. Either way the running app
    /// keeps its version until it is restarted.
    pub auto: bool,
}

/// Who started a check: the clock, or a press of the button.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Ask {
    Daily,
    Manual,
}

/// Where the update stands, which is what the About page and the popup show.
#[derive(Clone)]
enum Phase {
    Idle,
    Checking,
    Current,
    Failed(String),
    /// A newer version, waiting for a yes.
    Available(Release),
    Installing(Release, Stage),
    /// Installed, a restart away.
    Ready(Release),
    /// A newer version that cannot be installed from here, and why.
    Blocked(Release, String),
}

/// What the About page is told.
fn status(phase: &Phase) -> Value {
    let named = |state: &str, release: &Release| json!({ "t": "update", "state": state, "version": release.version });
    match phase {
        Phase::Idle => json!({ "t": "update", "state": "idle" }),
        Phase::Checking => json!({ "t": "update", "state": "checking" }),
        Phase::Current => json!({ "t": "update", "state": "current" }),
        Phase::Failed(detail) => json!({ "t": "update", "state": "failed", "detail": detail }),
        Phase::Available(release) => named("available", release),
        Phase::Installing(release, stage) => {
            let mut message = named("installing", release);
            message["stage"] = json!(stage.name());
            message["percent"] = json!(stage.percent());
            message
        }
        Phase::Ready(release) => named("ready", release),
        Phase::Blocked(release, detail) => {
            let mut message = named("blocked", release);
            message["detail"] = json!(detail);
            message
        }
    }
}

/// What the popup is told; nothing for the phases that are no question to the user.
fn view(phase: &Phase, language: &str, current: &str) -> Option<Value> {
    let (state, release, stage, detail) = match phase {
        Phase::Available(release) => ("available", release, None, ""),
        Phase::Installing(release, stage) => ("installing", release, Some(*stage), ""),
        Phase::Ready(release) => ("ready", release, None, ""),
        Phase::Blocked(release, detail) => ("blocked", release, None, detail.as_str()),
        _ => return None,
    };
    Some(json!({
        "t": "show",
        "lang": language,
        "state": state,
        "version": release.version,
        "current": current,
        "changes": release.changes,
        "stage": stage.map_or("", Stage::name),
        "percent": stage.and_then(Stage::percent),
        "detail": detail,
    }))
}

/// A check is due a day after the last one that got an answer. A clock that was set back counts as due,
/// or the checks would wait for it to catch up.
fn is_due(last: Option<u64>, now: u64) -> bool {
    last.is_none_or(|last| now < last || now - last >= DAY_SECONDS)
}

fn now_seconds() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_secs())
}

/// Told every change of the update, in the form the About page takes.
type Listener = Box<dyn Fn(&Value)>;

thread_local! {
    static FEATURE: RefCell<Weak<UpdateFeature>> = const { RefCell::new(Weak::new()) };
}

/// Runs `f` on the controller, if it is still there, from a callback of another thread that hopped to the
/// main one.
fn with_feature(f: impl FnOnce(&UpdateFeature)) {
    FEATURE.with(|slot| {
        if let Some(feature) = slot.borrow().upgrade() {
            f(&feature);
        }
    });
}

pub struct UpdateFeature {
    mtm: MainThreadMarker,
    home: PathBuf,
    config: Rc<RefCell<AppConfig>>,
    me: Weak<UpdateFeature>,
    phase: RefCell<Phase>,
    popup: RefCell<Option<Rc<WebWindow>>>,
    /// The popup's page has loaded and can be told things.
    popup_ready: Cell<bool>,
    listeners: RefCell<Vec<Listener>>,
    ticker: RefCell<Option<app::Timer>>,
}

impl UpdateFeature {
    pub fn new(mtm: MainThreadMarker, home: PathBuf, config: Rc<RefCell<AppConfig>>) -> Rc<Self> {
        let feature = Rc::new_cyclic(|me| Self {
            mtm,
            home,
            config,
            me: me.clone(),
            phase: RefCell::new(Phase::Idle),
            popup: RefCell::default(),
            popup_ready: Cell::new(false),
            listeners: RefCell::default(),
            ticker: RefCell::default(),
        });
        FEATURE.with(|slot| *slot.borrow_mut() = Rc::downgrade(&feature));
        feature
    }

    /// Starts the daily checks. A program that is not an installed app, such as a build folder or a test
    /// instance, never checks by itself: there is nothing there to update, and a test should stay off the network.
    pub fn start(&self) {
        let Some(bundle) = install::running_bundle() else {
            return;
        };
        std::thread::spawn(move || install::tidy(&bundle));
        let first = self.me.clone();
        app::after(FIRST_CHECK_SECONDS, move || {
            if let Some(this) = first.upgrade() {
                this.check_if_due();
            }
        });
        let weak = self.me.clone();
        *self.ticker.borrow_mut() = Some(app::every(TICK_SECONDS, move || {
            if let Some(this) = weak.upgrade() {
                this.check_if_due();
            }
        }));
    }

    /// Hears every change of the update's state, in the form the About page takes.
    pub fn on_status(&self, listener: impl Fn(&Value) + 'static) {
        self.listeners.borrow_mut().push(Box::new(listener));
    }

    pub fn status(&self) -> Value {
        status(&self.phase.borrow())
    }

    /// The button: look now, and install what is found without asking, since pressing it was the asking.
    pub fn check_now(&self) {
        self.check(Ask::Manual);
    }

    /// The yes to an offered version.
    pub fn install_available(&self) {
        let release = match &*self.phase.borrow() {
            Phase::Available(release) => release.clone(),
            _ => return,
        };
        self.install(release);
    }

    /// Opens the page of the release that is being talked about, for a person to download from.
    pub fn open_release(&self) {
        let url = match &*self.phase.borrow() {
            Phase::Available(r)
            | Phase::Installing(r, _)
            | Phase::Ready(r)
            | Phase::Blocked(r, _) => Some(r.url.clone()),
            _ => None,
        };
        if let Some(url) = url {
            workspace::open_url(&url);
        }
    }

    fn check_if_due(&self) {
        if is_due(self.last_check(), now_seconds()) {
            self.check(Ask::Daily);
        }
    }

    fn last_check(&self) -> Option<u64> {
        fs::read_to_string(self.home.join(CHECKED_FILE))
            .ok()?
            .trim()
            .parse()
            .ok()
    }

    fn remember_check(&self) {
        // Without the file the next tick asks again, which costs one request.
        let _ = fs::write(self.home.join(CHECKED_FILE), now_seconds().to_string());
    }

    fn check(&self, ask: Ask) {
        let phase = self.phase.borrow().clone();
        match phase {
            Phase::Checking | Phase::Installing(..) => return,
            // Installed and waiting for a restart: another look would only say the same again.
            Phase::Ready(_) => return self.notify(),
            _ => {}
        }
        self.set(Phase::Checking);
        std::thread::spawn(move || {
            let found = release::latest();
            app::on_main(move || with_feature(|this| this.checked(ask, found)));
        });
    }

    fn checked(&self, ask: Ask, found: Result<Release, String>) {
        let release = match found {
            Ok(release) => release,
            Err(detail) => {
                trace::log(&format!("update: the check failed: {detail}"));
                // Nobody asked for a check that ran by itself, so nobody needs to hear that it failed.
                return self.set(if ask == Ask::Manual {
                    Phase::Failed(detail)
                } else {
                    Phase::Idle
                });
            }
        };
        self.remember_check();
        if !release::newer(&release.version, env!("CARGO_PKG_VERSION")) {
            return self.set(Phase::Current);
        }
        trace::log(&format!("update: {} is out", release.version));
        if ask == Ask::Manual || self.config.borrow().update.auto {
            self.install(release);
        } else {
            self.set(Phase::Available(release));
            self.show_popup();
        }
    }

    fn install(&self, release: Release) {
        let Some(bundle) = install::running_bundle() else {
            return self.set(Phase::Blocked(
                release,
                "this copy of Figghost is not an installed app".into(),
            ));
        };
        self.set(Phase::Installing(release.clone(), Stage::Downloading(0)));
        std::thread::spawn(move || {
            let result = install::install(&release, &bundle, |stage| {
                app::on_main(move || with_feature(|this| this.staged(stage)));
            });
            app::on_main(move || with_feature(|this| this.installed(release, result)));
        });
    }

    fn staged(&self, stage: Stage) {
        let release = match &*self.phase.borrow() {
            Phase::Installing(release, _) => release.clone(),
            _ => return,
        };
        self.set(Phase::Installing(release, stage));
    }

    fn installed(&self, release: Release, result: Result<(), String>) {
        trace::log(&format!(
            "update: installing {}: {result:?}",
            release.version
        ));
        match result {
            Ok(()) => {
                self.set(Phase::Ready(release));
                self.show_popup();
            }
            Err(detail) => self.set(Phase::Blocked(release, detail)),
        }
    }

    fn set(&self, phase: Phase) {
        *self.phase.borrow_mut() = phase;
        self.notify();
    }

    fn notify(&self) {
        let status = self.status();
        for listener in self.listeners.borrow().iter() {
            listener(&status);
        }
        self.refresh_popup();
    }

    // -- the popup ----------------------------------------------------------------------------------------

    /// Opens the popup, or brings it to the front. It shows whatever the phase says once its page is ready.
    fn show_popup(&self) {
        let existing = self.popup.borrow().clone();
        if let Some(window) = existing {
            window.show(self.mtm);
            return;
        }
        let (on_message, on_close) = (self.me.clone(), self.me.clone());
        let window = WebWindow::new(
            self.mtm,
            &WindowSpec {
                title: "Figghost",
                width: POPUP_WIDTH,
                height: POPUP_HEIGHT,
                min_width: POPUP_WIDTH,
                min_height: POPUP_MIN_HEIGHT,
            },
            &page::html(&icon_url()),
            move |text| {
                if let Some(this) = on_message.upgrade() {
                    this.popup_message(&text);
                }
            },
            move || {
                if let Some(this) = on_close.upgrade() {
                    this.popup_closed();
                }
            },
        );
        *self.popup.borrow_mut() = Some(Rc::clone(&window));
        window.show(self.mtm);
        trace::log("update: the popup is open");
    }

    fn refresh_popup(&self) {
        if !self.popup_ready.get() {
            return;
        }
        let Some(window) = self.popup.borrow().clone() else {
            return;
        };
        let phase = self.phase.borrow().clone();
        if let Some(message) = view(&phase, i18n::current().code(), env!("CARGO_PKG_VERSION")) {
            window.host.post(&message.to_string());
        }
    }

    fn popup_message(&self, text: &str) {
        let Ok(message) = serde_json::from_str::<Value>(text) else {
            return;
        };
        if message["t"] == "action" {
            trace::log(&format!("update: the popup asks for {}", message["name"]));
        }
        match message["t"].as_str() {
            Some("ready") => {
                self.popup_ready.set(true);
                self.refresh_popup();
            }
            Some("js_error") => eprintln!(
                "update page: {} (line {})",
                message["message"].as_str().unwrap_or_default(),
                message["line"]
            ),
            Some("action") => match message["name"].as_str() {
                Some("install") => self.install_available(),
                Some("restart") => relaunch(self.mtm, "--background"),
                Some("download") => self.open_release(),
                Some("later") => {
                    if let Some(window) = self.popup.borrow().as_ref() {
                        window.close();
                    }
                }
                _ => {}
            },
            _ => {}
        }
    }

    fn popup_closed(&self) {
        self.popup_ready.set(false);
        let weak = self.me.clone();
        // The window is still inside its own delegate call, so it is dropped on the next turn of the run loop.
        app::after(0.0, move || {
            let Some(this) = weak.upgrade() else {
                return;
            };
            let reopened = this.popup.borrow().as_ref().is_some_and(|w| w.is_visible());
            if !reopened {
                this.popup.borrow_mut().take();
            }
        });
    }
}

fn icon_url() -> String {
    image_png(APP_ICON, ICON_POINTS, ICON_SCALE).map_or_else(String::new, |png| {
        format!("data:image/png;base64,{}", base64(&png))
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn release(version: &str) -> Release {
        Release {
            version: version.into(),
            url: format!("https://github.com/eliasbui/figghost/releases/tag/v{version}"),
            asset: None,
            changes: vec!["Add a thing".into()],
        }
    }

    #[test]
    fn a_check_is_due_a_day_after_the_last_answer() {
        assert!(is_due(None, 1_000_000), "never checked");
        assert!(!is_due(Some(1_000_000), 1_000_000 + DAY_SECONDS - 1));
        assert!(is_due(Some(1_000_000), 1_000_000 + DAY_SECONDS));
        assert!(is_due(Some(2_000_000), 1_000_000), "the clock was set back");
    }

    #[test]
    fn updating_by_itself_is_off_until_it_is_turned_on() {
        assert!(!Config::default().auto);
        let on: Config = serde_json::from_str(r#"{ "auto": true }"#).unwrap();
        assert!(on.auto);
        let none: Config = serde_json::from_str("{}").unwrap();
        assert_eq!(none, Config::default(), "a config file from before updates");
    }

    #[test]
    fn the_about_page_hears_each_phase() {
        let new = release("0.6.0");
        let state = |phase: &Phase| status(phase)["state"].as_str().unwrap().to_owned();
        assert_eq!(state(&Phase::Idle), "idle");
        assert_eq!(state(&Phase::Checking), "checking");
        assert_eq!(state(&Phase::Current), "current");
        assert_eq!(state(&Phase::Failed("offline".into())), "failed");
        assert_eq!(state(&Phase::Available(new.clone())), "available");
        assert_eq!(state(&Phase::Ready(new.clone())), "ready");

        let installing = status(&Phase::Installing(new.clone(), Stage::Downloading(42)));
        assert_eq!(installing["state"], "installing");
        assert_eq!(installing["stage"], "downloading");
        assert_eq!(installing["percent"], 42);
        assert_eq!(installing["version"], "0.6.0");
        let checking = status(&Phase::Installing(new.clone(), Stage::Checking));
        assert_eq!(checking["stage"], "checking");
        assert!(checking["percent"].is_null());

        let blocked = status(&Phase::Blocked(new, "no permission".into()));
        assert_eq!(blocked["state"], "blocked");
        assert_eq!(blocked["detail"], "no permission");

        let failed = status(&Phase::Failed("offline".into()));
        assert_eq!(failed["detail"], "offline");
    }

    #[test]
    fn the_popup_shows_a_version_that_waits_for_a_decision_and_nothing_else() {
        let new = release("0.6.0");
        for phase in [
            Phase::Idle,
            Phase::Checking,
            Phase::Current,
            Phase::Failed("offline".into()),
        ] {
            assert!(view(&phase, "en", "0.5.0").is_none());
        }
        let shown = view(&Phase::Available(new.clone()), "vi", "0.5.0").unwrap();
        assert_eq!(shown["t"], "show");
        assert_eq!(shown["lang"], "vi");
        assert_eq!(shown["state"], "available");
        assert_eq!(shown["version"], "0.6.0");
        assert_eq!(shown["current"], "0.5.0");
        assert_eq!(shown["changes"][0], "Add a thing");

        let blocked = view(&Phase::Blocked(new.clone(), "why".into()), "en", "0.5.0").unwrap();
        assert_eq!(blocked["detail"], "why");
        let checking = view(
            &Phase::Installing(new.clone(), Stage::Checking),
            "en",
            "0.5.0",
        )
        .unwrap();
        assert_eq!(checking["stage"], "checking");
        let downloading = view(
            &Phase::Installing(new, Stage::Downloading(7)),
            "en",
            "0.5.0",
        )
        .unwrap();
        assert_eq!(downloading["stage"], "downloading");
        assert_eq!(downloading["percent"], 7);
    }
}
