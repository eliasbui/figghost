//! The Pull requests tab of the notch: the review requests that wait for the person on GitHub and GitLab and the person's
//! own open ones, newest first,
//! with a dot on the tab's icon while there are some. Pressing one opens it in the browser. It is off until the person turns
//! it on.
//!
//! Nothing here holds a password or a token. The requests are asked of the command-line tools the person already signed
//! in with, `gh` for GitHub and `glab` for GitLab, run with fixed arguments and no shell, and what they print is read
//! as data: only a request that has a title, a project, a number, a time and an https address is kept, and only that address
//! is ever opened. Nothing of it is written to disk. A tool that is not installed or not signed in is said so and left alone.

use super::{AttachEnv, Ran, run_bounded_with};
use crate::{
    history::{ago, now_ms},
    i18n::{self, Language},
    settings::model::Fault,
    trace,
};
use logic::{Host, Request, gitlab_user_id, merged, parse_github, parse_gitlab};
use macos_sys::{
    app,
    notch::Notch,
    prs_ui::{PAGE_SIZE, PrRow, PrsCallbacks, PrsDisplay, PrsView},
    workspace,
};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::{
    cell::{Cell, RefCell},
    os::unix::fs::PermissionsExt,
    path::{Path, PathBuf},
    rc::{Rc, Weak},
};

pub mod logic;
pub mod settings;
mod text;

/// How often the requests are asked for while the feature is on, in minutes.
pub const MINUTES: [u32; 4] = [5, 10, 30, 60];
/// A command is stopped if it has not answered after this many seconds.
const COMMAND_SECONDS: u64 = 20;
const CHECK_SECONDS: u64 = 10;
/// The first look comes this long after the app starts, so that starting is not slowed.
const FIRST_LOOK_SECONDS: f64 = 5.0;
/// Whether it is time to look again is asked this often.
const WATCH_SECONDS: f64 = 60.0;
/// What the commands see of the environment besides the person's own: no colours, no questions, no notices about updates,
/// and the places a Mac keeps command-line tools, since an app started from the Finder has no search path of its own.
const ENVIRONMENT: [(&str, &str); 4] = [
    ("NO_COLOR", "1"),
    ("GH_PROMPT_DISABLED", "1"),
    ("GH_NO_UPDATE_NOTIFIER", "1"),
    ("PATH", "/opt/homebrew/bin:/usr/local/bin:/usr/bin:/bin"),
];

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    pub enabled: bool,
    pub github: bool,
    pub gitlab: bool,
    /// Draft requests are listed too.
    pub drafts: bool,
    /// How often to look, in minutes, one of `MINUTES`.
    pub minutes: u32,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            enabled: false,
            github: true,
            gitlab: true,
            drafts: false,
            minutes: 10,
        }
    }
}

impl Config {
    /// A config file can say anything; an interval that is not offered is read as the usual one.
    pub fn sanitized(mut self) -> Self {
        if !MINUTES.contains(&self.minutes) {
            self.minutes = Self::default().minutes;
        }
        self
    }

    fn sources(&self) -> Vec<Host> {
        let mut hosts = Vec::new();
        if self.github {
            hosts.push(Host::GitHub);
        }
        if self.gitlab {
            hosts.push(Host::GitLab);
        }
        hosts
    }
}

/// Why a source gave no requests.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Problem {
    /// The tool is not installed.
    Missing,
    /// The tool failed, most often because it is not signed in.
    Failed,
    Slow,
    /// What it printed is not what was asked for.
    Unreadable,
}

type Outcome = (Host, Result<Vec<Request>, Problem>);

/// What the last look found.
struct Found {
    requests: Vec<Request>,
    /// When it was made, in milliseconds since 1970.
    at: i64,
    problems: Vec<(Host, Problem)>,
    /// How many sources answered.
    answered: usize,
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

/// What the settings page is told about a tool.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Tools {
    github: &'static str,
    gitlab: &'static str,
    checking: bool,
}

pub struct Feature {
    notch: Weak<Notch>,
    /// The number of this tab in the notch.
    page: usize,
    view: Rc<PrsView>,
    config: RefCell<Config>,
    found: RefCell<Option<Found>>,
    busy: Cell<bool>,
    /// Bumped by every look; an answer that finds another number is for a look that is no longer wanted.
    generation: Cell<u64>,
    tools: Cell<Tools>,
    watcher: RefCell<Option<app::Timer>>,
    showing: Cell<bool>,
    me: Weak<Feature>,
}

/// Adds the tab. `None` without a notch, which is where the tab lives.
pub fn attach(env: &AttachEnv) -> Option<Rc<Feature>> {
    let notch = env.notch?;
    let config = env.config.prs.clone().sanitized();
    let feature = Rc::new_cyclic(|me: &Weak<Feature>| {
        let (open, refresh) = (me.clone(), me.clone());
        let view = PrsView::new(
            env.mtm,
            notch.page_area(PAGE_SIZE.width, PAGE_SIZE.height),
            PrsCallbacks {
                on_open: Box::new(move |index| {
                    if let Some(f) = open.upgrade() {
                        f.open(index);
                    }
                }),
                on_refresh: Box::new(move || {
                    if let Some(f) = refresh.upgrade() {
                        f.look();
                    }
                }),
            },
        );
        Feature {
            notch: Rc::downgrade(notch),
            page: notch.page_count(),
            view,
            config: RefCell::new(config),
            found: RefCell::default(),
            busy: Cell::new(false),
            generation: Cell::new(0),
            tools: Cell::new(Tools {
                github: "unknown",
                gitlab: "unknown",
                checking: false,
            }),
            watcher: RefCell::default(),
            showing: Cell::new(false),
            me: me.clone(),
        }
    });
    FEATURE.with(|slot| *slot.borrow_mut() = Rc::downgrade(&feature));
    let (shown, hidden) = (Rc::downgrade(&feature), Rc::downgrade(&feature));
    notch.add_page(feature.view.page(
        i18n::current().prs_tab().to_owned(),
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
        let config = config.clone().sanitized();
        let was = self.config.replace(config.clone());
        match (was.enabled, config.enabled) {
            (false, true) => self.start(),
            (true, false) => self.stop(),
            (true, true) if was.sources() != config.sources() || was.drafts != config.drafts => {
                // What is asked for changed, so what is known is no longer an answer, and neither is one that is still on
                // its way: it was asked for the old sources.
                self.found.borrow_mut().take();
                self.generation.set(self.generation.get() + 1);
                self.busy.set(false);
                self.look();
            }
            _ => {}
        }
        self.render();
        Ok(())
    }

    /// Facts only the running app knows, for the settings page.
    pub fn runtime(&self) -> Value {
        let tools = self.tools.get();
        json!({
            "github_tool": tools.github,
            "gitlab_tool": tools.gitlab,
            "checking": tools.checking,
            "count": self.found.borrow().as_ref().map_or(0, |f| f.requests.len()),
        })
    }

    /// The buttons of the settings page. `None` when the name is not this feature's.
    pub fn action(&self, name: &str) -> Option<Result<(), Fault>> {
        if name != "prs_check" {
            return None;
        }
        self.check_tools();
        Some(Ok(()))
    }

    /// Starts watching: a first look soon, then one whenever the interval has gone by.
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
                && f.config.borrow().enabled
            {
                f.look_if_due();
                f.check_tools();
            }
        });
    }

    /// Stops watching and lets go of what was found, with the dot on the tab.
    fn stop(&self) {
        self.watcher.borrow_mut().take();
        // An answer that is still on its way is for a feature that is off.
        self.generation.set(self.generation.get() + 1);
        self.busy.set(false);
        self.found.borrow_mut().take();
        self.mark(false);
    }

    fn look_if_due(&self) {
        let config = self.config.borrow();
        if !config.enabled || self.busy.get() {
            return;
        }
        let old = self
            .found
            .borrow()
            .as_ref()
            .is_none_or(|f| now_ms() - f.at >= i64::from(config.minutes) * 60_000);
        drop(config);
        if old {
            self.look();
        }
    }

    /// Asks the tools. It takes seconds, so it is done off the main thread.
    fn look(&self) {
        let config = self.config.borrow().clone();
        if !config.enabled || self.busy.get() {
            return;
        }
        let sources = config.sources();
        let generation = self.generation.get() + 1;
        self.generation.set(generation);
        if sources.is_empty() {
            self.found.borrow_mut().take();
            self.mark(false);
            self.render();
            return;
        }
        self.busy.set(true);
        trace::log("prs: asking");
        self.render();
        std::thread::spawn(move || {
            let outcomes: Vec<Outcome> = sources
                .into_iter()
                .map(|host| (host, fetch(host)))
                .collect();
            app::on_main(move || with_feature(|f| f.arrived(generation, outcomes)));
        });
    }

    fn arrived(&self, generation: u64, outcomes: Vec<Outcome>) {
        if generation != self.generation.get() {
            return;
        }
        self.busy.set(false);
        let drafts = self.config.borrow().drafts;
        let mut lists = Vec::new();
        let mut problems = Vec::new();
        for (host, outcome) in outcomes {
            match outcome {
                Ok(list) => lists.push(list),
                Err(problem) => problems.push((host, problem)),
            }
        }
        let answered = lists.len();
        let requests = merged(lists, drafts);
        trace::log(&format!(
            "prs: {} requests, {answered} sources answered",
            requests.len()
        ));
        self.mark(!requests.is_empty());
        *self.found.borrow_mut() = Some(Found {
            requests,
            at: now_ms(),
            problems,
            answered,
        });
        self.render();
    }

    /// The dot on the icon of the tab.
    fn mark(&self, on: bool) {
        if let Some(notch) = self.notch.upgrade() {
            notch.set_page_badge(self.page, on);
        }
    }

    fn render(&self) {
        if !self.showing.get() {
            return;
        }
        let config = self.config.borrow();
        self.view.show(&display(
            i18n::current(),
            &config,
            self.found.borrow().as_ref(),
            self.busy.get(),
            now_ms(),
        ));
    }

    /// A row was pressed: open the request in the browser, and put the notch away.
    fn open(&self, index: usize) {
        let url = self
            .found
            .borrow()
            .as_ref()
            .and_then(|f| f.requests.get(index))
            .map(|r| r.url.clone());
        let Some(url) = url else {
            return;
        };
        trace::log("prs: opening a request");
        workspace::open_url(&url);
        if let Some(notch) = self.notch.upgrade() {
            notch.close();
        }
    }

    /// Asks each tool whether it is there and signed in, for the settings page.
    fn check_tools(&self) {
        let mut tools = self.tools.get();
        if tools.checking {
            return;
        }
        tools.checking = true;
        self.tools.set(tools);
        std::thread::spawn(|| {
            let (github, gitlab) = (check("gh"), check("glab"));
            app::on_main(move || {
                with_feature(|f| {
                    f.tools.set(Tools {
                        github,
                        gitlab,
                        checking: false,
                    });
                });
            });
        });
    }
}

// -- the commands ----------------------------------------------------------------------------------------------------

/// The places a Mac keeps command-line tools.
fn tool_dirs() -> Vec<PathBuf> {
    let mut dirs: Vec<PathBuf> = ["/opt/homebrew/bin", "/usr/local/bin", "/usr/bin"]
        .iter()
        .map(PathBuf::from)
        .collect();
    if let Some(home) = std::env::var_os("HOME") {
        dirs.push(PathBuf::from(home).join(".local/bin"));
    }
    dirs
}

pub(crate) fn is_executable(path: &Path) -> bool {
    std::fs::metadata(path).is_ok_and(|m| m.is_file() && m.permissions().mode() & 0o111 != 0)
}

/// The first of `dirs` that holds an executable file called `name`.
pub(crate) fn locate(dirs: &[PathBuf], name: &str) -> Option<PathBuf> {
    dirs.iter()
        .map(|dir| dir.join(name))
        .find(|path| is_executable(path))
}

fn run(tool: &Path, args: &[&str], seconds: u64) -> Result<String, Problem> {
    let program = tool.to_str().ok_or(Problem::Failed)?;
    run_bounded_with(program, args, &ENVIRONMENT, seconds).map_err(|e| match e {
        Ran::Slow => Problem::Slow,
        Ran::Failed => Problem::Failed,
    })
}

/// Asks one service for the requests that wait for the person.
fn fetch(host: Host) -> Result<Vec<Request>, Problem> {
    match host {
        Host::GitHub => {
            let tool = locate(&tool_dirs(), "gh").ok_or(Problem::Missing)?;
            // The requests waiting for the person's review, then the person's own open ones; `merged` drops repeats.
            let mut all = Vec::new();
            for who in ["--review-requested=@me", "--author=@me"] {
                let text = run(
                    &tool,
                    &[
                        "search",
                        "prs",
                        who,
                        "--state=open",
                        "--limit",
                        "20",
                        "--json",
                        "number,title,url,repository,author,updatedAt,isDraft",
                    ],
                    COMMAND_SECONDS,
                )?;
                all.extend(parse_github(&text).map_err(|_| Problem::Unreadable)?);
            }
            Ok(all)
        }
        Host::GitLab => {
            let tool = locate(&tool_dirs(), "glab").ok_or(Problem::Missing)?;
            let me = run(&tool, &["api", "user"], COMMAND_SECONDS)?;
            let id = gitlab_user_id(&me).ok_or(Problem::Unreadable)?;
            let mut all = Vec::new();
            for who in ["reviewer_id", "author_id"] {
                let query =
                    format!("merge_requests?state=opened&scope=all&{who}={id}&per_page=20");
                let text = run(&tool, &["api", &query], COMMAND_SECONDS)?;
                all.extend(parse_gitlab(&text).map_err(|_| Problem::Unreadable)?);
            }
            Ok(all)
        }
    }
}

/// "ready", "signed_out", "missing" or "slow" for a tool, by asking it who is signed in.
fn check(name: &str) -> &'static str {
    let Some(tool) = locate(&tool_dirs(), name) else {
        return "missing";
    };
    match run(&tool, &["auth", "status"], CHECK_SECONDS) {
        Ok(_) => "ready",
        Err(Problem::Slow) => "slow",
        Err(_) => "signed_out",
    }
}

// -- what shows --------------------------------------------------------------------------------------------------------

fn note(symbol: &'static str, title: &str, hint: &str) -> PrsDisplay {
    PrsDisplay {
        note: Some((symbol, title.to_owned(), hint.to_owned())),
        ..PrsDisplay::default()
    }
}

/// What the tab shows for what the last look found. Pure, so that the tests drive it with a time of their own.
fn display(
    language: Language,
    config: &Config,
    found: Option<&Found>,
    busy: bool,
    now: i64,
) -> PrsDisplay {
    const SYMBOL: &str = "arrow.triangle.pull";
    if !config.enabled {
        return note(SYMBOL, language.prs_off_title(), language.prs_off_hint());
    }
    if config.sources().is_empty() {
        return note(
            SYMBOL,
            language.prs_no_source_title(),
            language.prs_no_source_hint(),
        );
    }
    let Some(found) = found else {
        return note(
            SYMBOL,
            language.prs_loading_title(),
            language.prs_loading_hint(),
        );
    };
    if found.answered == 0 {
        let all_missing = found.problems.iter().all(|(_, p)| *p == Problem::Missing);
        let any_slow = found.problems.iter().any(|(_, p)| *p == Problem::Slow);
        let (title, hint) = if all_missing {
            (language.prs_missing_title(), language.prs_missing_hint())
        } else if any_slow {
            (language.prs_slow_title(), language.prs_slow_hint())
        } else {
            (language.prs_failed_title(), language.prs_failed_hint())
        };
        let mut shown = note(SYMBOL, title, hint);
        shown.refresh_label = language.prs_refresh().to_owned();
        shown.busy = busy;
        return shown;
    }
    let mut status = if busy {
        language.prs_checking().to_owned()
    } else {
        language.prs_updated(&ago(language, now, found.at))
    };
    for (host, _) in &found.problems {
        status.push_str("  ·  ");
        status.push_str(&language.prs_source_problem(*host));
    }
    if found.requests.is_empty() {
        let mut shown = note(
            "checkmark.circle",
            language.prs_nothing_title(),
            language.prs_nothing_hint(),
        );
        shown.refresh_label = language.prs_refresh().to_owned();
        shown.status = status;
        shown.busy = busy;
        return shown;
    }
    let rows = found
        .requests
        .iter()
        .map(|r| {
            let mut parts: Vec<String> = Vec::new();
            if r.draft {
                parts.push(language.prs_draft().to_owned());
            }
            parts.push(format!("{} #{}", r.repo, r.number));
            if !r.author.is_empty() {
                parts.push(r.author.clone());
            }
            parts.push(ago(language, now, r.updated * 1000));
            PrRow {
                title: r.title.clone(),
                detail: parts.join("  ·  "),
            }
        })
        .collect();
    PrsDisplay {
        heading: language.prs_heading(found.requests.len()),
        status,
        rows,
        refresh_label: language.prs_refresh().to_owned(),
        busy,
        note: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{fs, os::unix::fs::PermissionsExt};

    const LANGUAGES: [Language; 2] = [Language::English, Language::Vietnamese];
    const NOW: i64 = 1_792_470_000_000;

    fn on() -> Config {
        Config {
            enabled: true,
            ..Config::default()
        }
    }

    fn request(number: u64, draft: bool, hours_ago: i64) -> Request {
        Request {
            host: Host::GitHub,
            repo: "cli/cli".into(),
            number,
            title: format!("Request {number}"),
            author: "babakks".into(),
            updated: NOW / 1000 - hours_ago * 3600,
            draft,
            url: format!("https://github.com/cli/cli/pull/{number}"),
        }
    }

    fn found(requests: Vec<Request>, problems: Vec<(Host, Problem)>, answered: usize) -> Found {
        Found {
            requests,
            at: NOW - 3 * 60_000,
            problems,
            answered,
        }
    }

    #[test]
    fn the_feature_is_off_by_default_and_old_files_load() {
        let config: Config = serde_json::from_str("{}").unwrap();
        assert_eq!(config, Config::default());
        assert!(!config.enabled && config.github && config.gitlab && !config.drafts);
        assert_eq!(config.minutes, 10);
        let partial: Config =
            serde_json::from_str(r#"{ "enabled": true, "gitlab": false, "future_key": 1 }"#)
                .unwrap();
        assert!(partial.enabled && !partial.gitlab && partial.github);
        assert_eq!(partial.sources(), [Host::GitHub]);
        assert_eq!(
            Config {
                github: false,
                gitlab: false,
                ..Config::default()
            }
            .sources(),
            Vec::<Host>::new()
        );
    }

    #[test]
    fn an_interval_that_is_not_offered_is_read_as_the_usual_one() {
        for minutes in MINUTES {
            assert_eq!(
                Config {
                    minutes,
                    ..Config::default()
                }
                .sanitized()
                .minutes,
                minutes
            );
        }
        for minutes in [0, 7, 15, u32::MAX] {
            assert_eq!(
                Config {
                    minutes,
                    ..Config::default()
                }
                .sanitized()
                .minutes,
                10,
                "{minutes}"
            );
        }
    }

    #[test]
    fn a_tool_is_looked_for_in_the_places_given_and_must_be_an_executable_file() {
        let root = std::env::temp_dir().join(format!("figghost-prs-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        let (first, second) = (root.join("first"), root.join("second"));
        fs::create_dir_all(&first).unwrap();
        fs::create_dir_all(&second).unwrap();
        let make = |dir: &Path, name: &str, mode: u32| {
            let path = dir.join(name);
            fs::write(&path, "#!/bin/sh\n").unwrap();
            fs::set_permissions(&path, fs::Permissions::from_mode(mode)).unwrap();
        };
        make(&first, "plain", 0o644);
        make(&second, "plain", 0o755);
        make(&first, "tool", 0o755);
        make(&second, "tool", 0o755);
        fs::create_dir_all(second.join("adir")).unwrap();
        let dirs = [first.clone(), second.clone()];
        assert_eq!(
            locate(&dirs, "tool"),
            Some(first.join("tool")),
            "the first place that has it"
        );
        assert_eq!(
            locate(&dirs, "plain"),
            Some(second.join("plain")),
            "a file that cannot be run is passed over"
        );
        assert_eq!(locate(&dirs, "adir"), None, "a folder is not a tool");
        assert_eq!(locate(&dirs, "absent"), None);
        assert_eq!(locate(&[], "tool"), None);
        fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn the_tab_says_why_it_has_nothing_to_show() {
        for language in LANGUAGES {
            let off = display(language, &Config::default(), None, false, NOW);
            assert_eq!(off.note.as_ref().unwrap().1, language.prs_off_title());
            let none = Config {
                github: false,
                gitlab: false,
                ..on()
            };
            assert_eq!(
                display(language, &none, None, false, NOW).note.unwrap().1,
                language.prs_no_source_title()
            );
            assert_eq!(
                display(language, &on(), None, true, NOW).note.unwrap().1,
                language.prs_loading_title()
            );
        }
        let missing = found(
            vec![],
            vec![
                (Host::GitHub, Problem::Missing),
                (Host::GitLab, Problem::Missing),
            ],
            0,
        );
        assert_eq!(
            display(Language::English, &on(), Some(&missing), false, NOW)
                .note
                .unwrap()
                .1,
            "No command-line tool found"
        );
        let signed_out = found(
            vec![],
            vec![
                (Host::GitHub, Problem::Failed),
                (Host::GitLab, Problem::Missing),
            ],
            0,
        );
        assert_eq!(
            display(Language::English, &on(), Some(&signed_out), false, NOW)
                .note
                .unwrap()
                .1,
            "Could not read your review requests"
        );
        let slow = found(vec![], vec![(Host::GitHub, Problem::Slow)], 0);
        assert_eq!(
            display(Language::Vietnamese, &on(), Some(&slow), false, NOW)
                .note
                .unwrap()
                .1,
            "Không trả lời kịp"
        );
        let shown = display(Language::English, &on(), Some(&slow), false, NOW);
        assert_eq!(
            shown.refresh_label, "Look again",
            "a look can be asked for again"
        );
    }

    #[test]
    fn nothing_waiting_is_good_news_and_says_how_fresh_it_is() {
        let none = found(vec![], vec![], 2);
        let shown = display(Language::English, &on(), Some(&none), false, NOW);
        let (symbol, title, _) = shown.note.unwrap();
        assert_eq!(
            (symbol, title.as_str()),
            ("checkmark.circle", "Nothing waits for your review")
        );
        assert_eq!(shown.status, "Updated 3 minutes ago");
    }

    #[test]
    fn the_requests_are_listed_with_where_who_and_how_long_ago() {
        let list = found(vec![request(5, false, 2), request(6, true, 30)], vec![], 1);
        let shown = display(Language::English, &on(), Some(&list), false, NOW);
        assert!(shown.note.is_none());
        assert_eq!(shown.heading, "2 requests wait for your review");
        assert_eq!(shown.rows[0].title, "Request 5");
        assert_eq!(
            shown.rows[0].detail,
            "cli/cli #5  ·  babakks  ·  2 hours ago"
        );
        assert_eq!(
            shown.rows[1].detail,
            "Draft  ·  cli/cli #6  ·  babakks  ·  1 day ago"
        );
        let one = found(vec![request(5, false, 2)], vec![], 1);
        assert_eq!(
            display(Language::English, &on(), Some(&one), false, NOW).heading,
            "1 request waits for your review"
        );
        let vi = display(Language::Vietnamese, &on(), Some(&list), false, NOW);
        assert_eq!(vi.heading, "2 yêu cầu đang chờ bạn duyệt");
        assert!(vi.rows[1].detail.starts_with("Bản nháp"));
    }

    #[test]
    fn a_source_that_could_not_be_read_is_named_beside_the_answer_of_the_other() {
        let partial = found(
            vec![request(5, false, 2)],
            vec![(Host::GitLab, Problem::Failed)],
            1,
        );
        let shown = display(Language::English, &on(), Some(&partial), false, NOW);
        assert_eq!(
            shown.status,
            "Updated 3 minutes ago  ·  GitLab could not be read"
        );
        assert_eq!(shown.rows.len(), 1);
        let looking = display(Language::English, &on(), Some(&partial), true, NOW);
        assert!(looking.status.starts_with("Looking..."));
        assert!(looking.busy);
    }

    #[test]
    fn every_word_is_there_in_both_languages_and_the_symbols_are_ones_the_system_knows() {
        for language in LANGUAGES {
            for text in [
                language.prs_tab(),
                language.prs_draft(),
                language.prs_checking(),
                language.prs_refresh(),
                language.prs_off_title(),
                language.prs_off_hint(),
                language.prs_nothing_title(),
                language.prs_nothing_hint(),
                language.prs_loading_title(),
                language.prs_loading_hint(),
                language.prs_no_source_title(),
                language.prs_no_source_hint(),
                language.prs_missing_title(),
                language.prs_missing_hint(),
                language.prs_failed_title(),
                language.prs_failed_hint(),
                language.prs_slow_title(),
                language.prs_slow_hint(),
                language.prs_host(Host::GitHub),
                language.prs_host(Host::GitLab),
            ] {
                assert!(!text.is_empty());
            }
            assert!(
                language.prs_heading(3).contains('3') && language.prs_updated("x").contains('x')
            );
            assert!(language.prs_source_problem(Host::GitLab).contains("GitLab"));
        }
        for symbol in ["arrow.triangle.pull", "checkmark.circle", "arrow.clockwise"] {
            assert!(
                macos_sys::widgets::symbol_png(symbol, 16.0, 1.0).is_some(),
                "{symbol}"
            );
        }
    }
}
