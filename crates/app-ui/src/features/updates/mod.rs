//! The Updates tab of the notch: the software on this Mac that has a newer version, from Homebrew, npm, the App
//! Store, Apple and the coding agents that update themselves, with the version now in red and the new one in green.
//! Rows are ticked to update them together, or everything is updated at once, and it can update everything once a
//! day by itself. It is off until the person turns it on.
//!
//! The versions are asked of the tools already installed, run with fixed arguments and no shell; what they print is
//! read as data. The newest version of an agent is read from the npm registry with curl. Apple's updates need an
//! administrator, so updating them opens System Settings instead.

use super::{AttachEnv, Ran, prs::locate, run_bounded_output};
use crate::{
    history::{ago, now_ms},
    i18n::{self, Language},
    settings::model::Fault,
    trace,
};
use logic::{AGENTS, Source, Update};
use macos_sys::{
    app,
    notch::Notch,
    updates_ui::{PAGE_SIZE, UpdateRow, UpdatesCallbacks, UpdatesDisplay, UpdatesView},
    workspace,
};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::{
    cell::{Cell, RefCell},
    collections::BTreeSet,
    path::{Path, PathBuf},
    rc::{Rc, Weak},
};

pub mod logic;
pub mod settings;
mod text;

/// How old a look may get before it is made again.
const LOOK_EVERY_MS: i64 = 6 * 3_600_000;
const DAY_MS: i64 = 86_400_000;
/// The first look comes this long after the app starts, so that starting is not slowed.
const FIRST_LOOK_SECONDS: f64 = 20.0;
const WATCH_SECONDS: f64 = 300.0;
/// Asking a tool what is out of date; `brew update` and `softwareupdate` go to the network.
const LOOK_SECONDS: u64 = 180;
const VERSION_SECONDS: u64 = 10;
/// One update command; a big cask can take a while to download.
const UPDATE_SECONDS: u64 = 1800;
/// When everything was last updated by itself, in milliseconds since 1970, kept in the data folder.
const AUTO_FILE: &str = "updates-auto";
const SOFTWARE_UPDATE_SETTINGS: &str =
    "x-apple.systempreferences:com.apple.Software-Update-Settings.extension";
const CURL: &str = "/usr/bin/curl";
/// What the commands see besides the person's own environment: no colours, no questions, no notices, and the places
/// a Mac keeps command-line tools, since an app started from the Finder has no search path of its own.
const ENVIRONMENT: [(&str, &str); 6] = [
    ("NO_COLOR", "1"),
    ("NONINTERACTIVE", "1"),
    ("HOMEBREW_NO_ENV_HINTS", "1"),
    ("HOMEBREW_NO_AUTO_UPDATE", "1"),
    ("NO_UPDATE_NOTIFIER", "1"),
    (
        "PATH",
        "/opt/homebrew/bin:/usr/local/bin:/usr/bin:/bin:/usr/sbin:/sbin",
    ),
];

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct Config {
    pub enabled: bool,
    /// Update everything once a day, without asking. Apple's updates are left to System Settings.
    pub daily: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Work {
    Idle,
    Looking,
    Updating,
}

/// What the last look found.
struct Found {
    updates: Vec<Update>,
    at: i64,
    /// The tools that are installed but did not answer.
    failed: Vec<Source>,
}

thread_local! {
    /// Answers arrive from another thread and hop to the main one, which finds the feature here.
    static FEATURE: RefCell<Weak<Feature>> = const { RefCell::new(Weak::new()) };
}

fn with_feature(f: impl FnOnce(&Feature)) {
    FEATURE.with(|slot| {
        if let Some(feature) = slot.borrow().upgrade() {
            f(&feature);
        }
    });
}

pub struct Feature {
    notch: Weak<Notch>,
    page: usize,
    view: Rc<UpdatesView>,
    home: PathBuf,
    config: RefCell<Config>,
    found: RefCell<Option<Found>>,
    /// The keys of the ticked rows.
    chosen: RefCell<BTreeSet<String>>,
    work: Cell<Work>,
    /// How the last update went: done and failed.
    result: Cell<Option<(usize, usize)>>,
    /// Bumped by every look; an answer that finds another number is no longer wanted.
    generation: Cell<u64>,
    watcher: RefCell<Option<app::Timer>>,
    showing: Cell<bool>,
    me: Weak<Feature>,
}

/// Adds the tab. `None` without a notch, which is where the tab lives.
pub fn attach(env: &AttachEnv) -> Option<Rc<Feature>> {
    let notch = env.notch?;
    let feature = Rc::new_cyclic(|me: &Weak<Feature>| {
        let call = |f: fn(&Feature)| {
            let me = me.clone();
            Box::new(move || {
                if let Some(this) = me.upgrade() {
                    f(&this);
                }
            })
        };
        let toggle = me.clone();
        let view = UpdatesView::new(
            env.mtm,
            notch.page_area(PAGE_SIZE.width, PAGE_SIZE.height),
            UpdatesCallbacks {
                on_toggle: Box::new(move |index| {
                    if let Some(f) = toggle.upgrade() {
                        f.toggle(index);
                    }
                }),
                on_refresh: call(Feature::look),
                on_update_chosen: call(|f| f.update(false)),
                on_update_all: call(|f| f.update(true)),
            },
        );
        Feature {
            notch: Rc::downgrade(notch),
            page: notch.page_count(),
            view,
            home: env.home.to_path_buf(),
            config: RefCell::new(env.config.updates.clone()),
            found: RefCell::default(),
            chosen: RefCell::default(),
            work: Cell::new(Work::Idle),
            result: Cell::new(None),
            generation: Cell::new(0),
            watcher: RefCell::default(),
            showing: Cell::new(false),
            me: me.clone(),
        }
    });
    FEATURE.with(|slot| *slot.borrow_mut() = Rc::downgrade(&feature));
    let (shown, hidden) = (Rc::downgrade(&feature), Rc::downgrade(&feature));
    notch.add_page(feature.view.page(
        i18n::current().updates_tab().to_owned(),
        move || {
            if let Some(f) = shown.upgrade() {
                f.showing.set(true);
                f.render();
                f.look_if_due();
            }
        },
        move || {
            if let Some(f) = hidden.upgrade() {
                f.showing.set(false);
            }
        },
    ));
    if feature.config.borrow().enabled {
        feature.start();
    }
    Some(feature)
}

impl Feature {
    pub fn configure(&self, config: &Config) -> Result<(), Fault> {
        let was = self.config.replace(config.clone());
        match (was.enabled, config.enabled) {
            (false, true) => self.start(),
            (true, false) => self.stop(),
            _ => self.auto_if_due(),
        }
        self.render();
        Ok(())
    }

    pub fn runtime(&self) -> Value {
        json!({
            "count": self.found.borrow().as_ref().map_or(0, |f| f.updates.len()),
            "busy": self.work.get() != Work::Idle,
        })
    }

    pub fn action(&self, name: &str) -> Option<Result<(), Fault>> {
        if name != "updates_check" {
            return None;
        }
        self.look();
        Some(Ok(()))
    }

    fn start(&self) {
        let weak = self.me.clone();
        *self.watcher.borrow_mut() = Some(app::every(WATCH_SECONDS, move || {
            if let Some(f) = weak.upgrade() {
                f.look_if_due();
            }
        }));
        let (weak, generation) = (self.me.clone(), self.generation.get());
        app::after(FIRST_LOOK_SECONDS, move || {
            if let Some(f) = weak.upgrade()
                && f.generation.get() == generation
            {
                f.look_if_due();
            }
        });
    }

    fn stop(&self) {
        self.watcher.borrow_mut().take();
        self.generation.set(self.generation.get() + 1);
        if self.work.get() == Work::Looking {
            self.work.set(Work::Idle);
        }
        self.found.borrow_mut().take();
        self.chosen.borrow_mut().clear();
        self.mark(false);
    }

    fn look_if_due(&self) {
        let old = self
            .found
            .borrow()
            .as_ref()
            .is_none_or(|f| now_ms() - f.at >= LOOK_EVERY_MS);
        if old {
            self.look();
        } else {
            self.auto_if_due();
        }
    }

    /// Asks every tool at once. It takes up to a few minutes, so it is done off the main thread.
    fn look(&self) {
        if !self.config.borrow().enabled || self.work.get() != Work::Idle {
            return;
        }
        let generation = self.generation.get() + 1;
        self.generation.set(generation);
        self.work.set(Work::Looking);
        trace::log("updates: looking");
        self.render();
        std::thread::spawn(move || {
            let (updates, failed) = fetch_all();
            app::on_main(move || with_feature(|f| f.arrived(generation, updates, failed)));
        });
    }

    fn arrived(&self, generation: u64, updates: Vec<Update>, failed: Vec<Source>) {
        if generation != self.generation.get() {
            return;
        }
        self.work.set(Work::Idle);
        trace::log(&format!(
            "updates: {} found, {} sources did not answer",
            updates.len(),
            failed.len()
        ));
        let keys: BTreeSet<String> = updates.iter().map(Update::key).collect();
        self.chosen.borrow_mut().retain(|k| keys.contains(k));
        self.mark(!updates.is_empty());
        *self.found.borrow_mut() = Some(Found {
            updates,
            at: now_ms(),
            failed,
        });
        self.render();
        self.auto_if_due();
    }

    /// The daily update, when it is on, a day has gone by since the last one, and something can be updated by itself.
    fn auto_if_due(&self) {
        let config = self.config.borrow().clone();
        if !config.enabled || !config.daily || self.work.get() != Work::Idle {
            return;
        }
        let auto_file = self.home.join(AUTO_FILE);
        let last: i64 = std::fs::read_to_string(&auto_file)
            .ok()
            .and_then(|t| t.trim().parse().ok())
            .unwrap_or(0);
        let any = self
            .found
            .borrow()
            .as_ref()
            .is_some_and(|f| f.updates.iter().any(|u| u.source != Source::Apple));
        if any && now_ms() - last >= DAY_MS {
            let _ = std::fs::write(&auto_file, now_ms().to_string());
            trace::log("updates: the daily update");
            self.run(true, false);
        }
    }

    fn toggle(&self, index: usize) {
        let Some(key) = self
            .found
            .borrow()
            .as_ref()
            .and_then(|f| f.updates.get(index))
            .map(Update::key)
        else {
            return;
        };
        let mut chosen = self.chosen.borrow_mut();
        if !chosen.remove(&key) {
            chosen.insert(key);
        }
        drop(chosen);
        self.render();
    }

    /// A button: update what is ticked, or everything.
    fn update(&self, all: bool) {
        self.run(all, true);
    }

    /// Runs the commands one after another off the main thread, then looks again. `apple` lets the Apple ones open
    /// System Settings, which only a press of the person's should do.
    fn run(&self, all: bool, apple: bool) {
        if self.work.get() != Work::Idle {
            return;
        }
        let chosen: Vec<Update> = {
            let keys = self.chosen.borrow();
            self.found
                .borrow()
                .as_ref()
                .map(|f| {
                    f.updates
                        .iter()
                        .filter(|u| all || keys.contains(&u.key()))
                        .filter(|u| apple || u.source != Source::Apple)
                        .cloned()
                        .collect()
                })
                .unwrap_or_default()
        };
        if chosen.is_empty() {
            return;
        }
        let (commands, open_settings) = logic::plan(&chosen);
        if open_settings {
            workspace::open_url(SOFTWARE_UPDATE_SETTINGS);
        }
        if commands.is_empty() {
            return;
        }
        let counts: Vec<usize> = commands.iter().map(|c| update_count(c)).collect();
        self.work.set(Work::Updating);
        self.result.set(None);
        trace::log(&format!("updates: running {} commands", commands.len()));
        self.render();
        std::thread::spawn(move || {
            let (mut done, mut failed) = (0, 0);
            for (command, count) in commands.iter().zip(counts) {
                if run_update(command) {
                    done += count;
                } else {
                    failed += count;
                }
            }
            app::on_main(move || {
                with_feature(|f| {
                    f.work.set(Work::Idle);
                    f.result.set(Some((done, failed)));
                    f.chosen.borrow_mut().clear();
                    f.look();
                });
            });
        });
    }

    fn mark(&self, on: bool) {
        if let Some(notch) = self.notch.upgrade() {
            notch.set_page_badge(self.page, on);
        }
    }

    fn render(&self) {
        if !self.showing.get() {
            return;
        }
        self.view.show(&display(
            i18n::current(),
            &self.config.borrow(),
            self.found.borrow().as_ref(),
            &self.chosen.borrow(),
            self.work.get(),
            self.result.get(),
            now_ms(),
        ));
    }
}

/// How many updates a command of `logic::plan` takes care of: what follows `brew upgrade --cask`, `npm install -g`
/// or `mas upgrade`, or the one agent.
fn update_count(command: &[String]) -> usize {
    match command.first().map(String::as_str) {
        Some("brew" | "npm") => command.len() - 3,
        Some("mas") => command.len() - 2,
        _ => 1,
    }
}

// -- the commands ----------------------------------------------------------------------------------------------------

/// The places a Mac keeps command-line tools, and those where the agents install themselves.
fn tool_dirs() -> Vec<PathBuf> {
    let mut dirs: Vec<PathBuf> = ["/opt/homebrew/bin", "/usr/local/bin", "/usr/bin"]
        .iter()
        .map(PathBuf::from)
        .collect();
    if let Some(home) = std::env::var_os("HOME") {
        let home = PathBuf::from(home);
        for dir in [
            ".local/bin",
            ".opencode/bin",
            ".pi/agent/bin",
            ".amp/bin",
            ".factory/bin",
        ] {
            dirs.push(home.join(dir));
        }
    }
    dirs
}

fn output(tool: &Path, args: &[&str], seconds: u64) -> Result<(bool, String), Ran> {
    let program = tool.to_str().ok_or(Ran::Failed)?;
    run_bounded_output(program, args, &ENVIRONMENT, seconds)
}

/// One update command, its program found where the tools live.
fn run_update(command: &[String]) -> bool {
    let Some(tool) = command.first().and_then(|name| locate(&tool_dirs(), name)) else {
        return false;
    };
    let args: Vec<&str> = command[1..].iter().map(String::as_str).collect();
    matches!(output(&tool, &args, UPDATE_SECONDS), Ok((true, _)))
}

type Outcome = Result<Vec<Update>, Source>;

/// Every source at once, since some go to the network: what is out of date, and the tools that did not answer.
fn fetch_all() -> (Vec<Update>, Vec<Source>) {
    let outcomes: Vec<Outcome> = std::thread::scope(|scope| {
        let jobs = [
            scope.spawn(brew),
            scope.spawn(npm),
            scope.spawn(mas),
            scope.spawn(apple),
            scope.spawn(|| Ok(agents())),
        ];
        jobs.into_iter()
            .map(|job| job.join().unwrap_or(Ok(Vec::new())))
            .collect()
    });
    let mut updates = Vec::new();
    let mut failed = Vec::new();
    for outcome in outcomes {
        match outcome {
            Ok(list) => updates.extend(list),
            Err(source) => failed.push(source),
        }
    }
    (updates, failed)
}

/// A tool that is not installed has nothing out of date.
fn brew() -> Outcome {
    let Some(tool) = locate(&tool_dirs(), "brew") else {
        return Ok(Vec::new());
    };
    // What Homebrew knows of the newest versions is only as fresh as its last update.
    let _ = output(&tool, &["update", "--quiet"], LOOK_SECONDS);
    let (_, text) =
        output(&tool, &["outdated", "--json=v2"], LOOK_SECONDS).map_err(|_| Source::Formula)?;
    logic::parse_brew(&text).map_err(|_| Source::Formula)
}

fn npm() -> Outcome {
    let Some(tool) = locate(&tool_dirs(), "npm") else {
        return Ok(Vec::new());
    };
    let (_, text) =
        output(&tool, &["outdated", "-g", "--json"], LOOK_SECONDS).map_err(|_| Source::Npm)?;
    logic::parse_npm(&text).map_err(|_| Source::Npm)
}

fn mas() -> Outcome {
    let Some(tool) = locate(&tool_dirs(), "mas") else {
        return Ok(Vec::new());
    };
    let (_, text) = output(&tool, &["outdated"], LOOK_SECONDS).map_err(|_| Source::AppStore)?;
    Ok(logic::parse_mas(&text))
}

fn apple() -> Outcome {
    let (_, text) = output(
        Path::new("/usr/sbin/softwareupdate"),
        &["--list"],
        LOOK_SECONDS,
    )
    .map_err(|_| Source::Apple)?;
    let macos = output(
        Path::new("/usr/bin/sw_vers"),
        &["-productVersion"],
        VERSION_SECONDS,
    )
    .map(|(_, v)| v.trim().to_owned())
    .unwrap_or_default();
    Ok(logic::parse_softwareupdate(&text, &macos))
}

/// The agents installed on their own, each asked its version and compared with the newest on npm. One that cannot
/// be asked is left out rather than counted as a failure: it may simply not be there.
fn agents() -> Vec<Update> {
    let dirs = tool_dirs();
    AGENTS
        .iter()
        .filter_map(|agent| {
            let tool = locate(&dirs, agent.program)?;
            let resolved = std::fs::canonicalize(&tool).ok()?;
            if logic::managed_elsewhere(&resolved.to_string_lossy()) {
                return None;
            }
            let (_, said) = output(&tool, &["--version"], VERSION_SECONDS).ok()?;
            let current = logic::version_in(&said)?;
            let url = format!("https://registry.npmjs.org/{}/latest", agent.package);
            let (ok, body) = output(
                Path::new(CURL),
                &["-fsSL", "--max-time", "15", &url],
                VERSION_SECONDS + 10,
            )
            .ok()?;
            let latest = logic::npm_latest(&body).filter(|_| ok)?;
            logic::newer(&latest, &current).then(|| Update {
                source: Source::Agent,
                id: agent.program.to_owned(),
                name: agent.name.to_owned(),
                current,
                latest,
            })
        })
        .collect()
}

// -- what shows --------------------------------------------------------------------------------------------------------

fn note(symbol: &'static str, title: &str, hint: &str) -> UpdatesDisplay {
    UpdatesDisplay {
        note: Some((symbol, title.to_owned(), hint.to_owned())),
        ..UpdatesDisplay::default()
    }
}

/// What the tab shows. Pure, so that the tests drive it with a time of their own.
fn display(
    language: Language,
    config: &Config,
    found: Option<&Found>,
    chosen: &BTreeSet<String>,
    work: Work,
    result: Option<(usize, usize)>,
    now: i64,
) -> UpdatesDisplay {
    const SYMBOL: &str = "arrow.down.circle";
    if !config.enabled {
        return note(
            SYMBOL,
            language.updates_off_title(),
            language.updates_off_hint(),
        );
    }
    let Some(found) = found else {
        return note(
            SYMBOL,
            language.updates_loading_title(),
            language.updates_loading_hint(),
        );
    };
    let mut status = match work {
        Work::Looking => language.updates_looking().to_owned(),
        Work::Updating => language.updates_updating().to_owned(),
        Work::Idle => {
            let checked = language.updates_checked(&ago(language, now, found.at));
            match result {
                Some((done, failed)) => {
                    format!("{}  ·  {checked}", language.updates_result(done, failed))
                }
                None => checked,
            }
        }
    };
    for source in &found.failed {
        status.push_str("  ·  ");
        status.push_str(&language.updates_source_failed(*source));
    }
    let busy = work != Work::Idle;
    if found.updates.is_empty() {
        let mut shown = note(
            "checkmark.circle",
            language.updates_nothing_title(),
            language.updates_nothing_hint(),
        );
        shown.refresh_label = language.updates_refresh().to_owned();
        shown.status = status;
        shown.busy = busy;
        return shown;
    }
    let rows = found
        .updates
        .iter()
        .map(|u| UpdateRow {
            name: u.name.clone(),
            source: language.updates_source(u.source).to_owned(),
            current: u.current.clone(),
            latest: u.latest.clone(),
            checked: chosen.contains(&u.key()),
        })
        .collect();
    let picked = found
        .updates
        .iter()
        .filter(|u| chosen.contains(&u.key()))
        .count();
    UpdatesDisplay {
        heading: language.updates_heading(found.updates.len()),
        status,
        rows,
        refresh_label: language.updates_refresh().to_owned(),
        chosen_label: language.updates_chosen(picked),
        all_label: language.updates_all().to_owned(),
        busy,
        note: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const NOW: i64 = 1_792_470_000_000;

    fn update(source: Source, id: &str) -> Update {
        Update {
            source,
            id: id.into(),
            name: id.into(),
            current: "1.0".into(),
            latest: "2.0".into(),
        }
    }

    fn found(updates: Vec<Update>) -> Found {
        Found {
            updates,
            at: NOW - 60_000,
            failed: vec![Source::Npm],
        }
    }

    #[test]
    fn the_feature_is_off_by_default_and_says_so() {
        let config: Config = serde_json::from_str("{}").unwrap();
        assert!(!config.enabled && !config.daily);
        let shown = display(
            Language::English,
            &config,
            None,
            &BTreeSet::new(),
            Work::Idle,
            None,
            NOW,
        );
        assert!(shown.note.is_some());
    }

    #[test]
    fn rows_show_both_versions_and_which_are_ticked() {
        let on = Config {
            enabled: true,
            daily: false,
        };
        let f = found(vec![
            update(Source::Formula, "node"),
            update(Source::Agent, "claude"),
        ]);
        let chosen: BTreeSet<String> = [f.updates[1].key()].into();
        for language in [Language::English, Language::Vietnamese] {
            let shown = display(
                language,
                &on,
                Some(&f),
                &chosen,
                Work::Idle,
                Some((2, 1)),
                NOW,
            );
            assert!(shown.note.is_none());
            assert_eq!(shown.rows.len(), 2);
            assert_eq!(
                (
                    shown.rows[0].current.as_str(),
                    shown.rows[0].latest.as_str()
                ),
                ("1.0", "2.0")
            );
            assert!(!shown.rows[0].checked && shown.rows[1].checked);
            assert!(shown.chosen_label.contains('1'), "{}", shown.chosen_label);
            assert!(
                shown.status.contains("npm"),
                "a source that did not answer is named"
            );
            assert!(
                shown.status.contains('2'),
                "how the last update went: {}",
                shown.status
            );
        }
        let none = display(
            Language::English,
            &on,
            Some(&found(vec![])),
            &chosen,
            Work::Idle,
            None,
            NOW,
        );
        assert!(none.note.is_some() && !none.refresh_label.is_empty());
    }

    #[test]
    fn each_command_counts_what_it_updates() {
        let chosen = vec![
            update(Source::Formula, "a"),
            update(Source::Formula, "b"),
            update(Source::Npm, "c"),
            update(Source::Agent, "claude"),
        ];
        let (commands, _) = logic::plan(&chosen);
        let counts: Vec<usize> = commands.iter().map(|c| update_count(c)).collect();
        assert_eq!(counts, [2, 1, 1]);
    }
}
