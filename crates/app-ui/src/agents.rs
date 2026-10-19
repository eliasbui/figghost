//! Coding agents in the notch: which sessions run, which command waits for an answer, and how much of the
//! usage limits is left. The agents report through hooks (see `core-agents`); this connects what they say to
//! the page, and carries the answer back.

use crate::{
    config::{AgentsConfig, NotchConfig},
    history::{ago, now_ms},
    i18n::{self, Language},
    trace,
};
use core_agents::{
    Agent, HookEvent, Reply,
    cli::CHAIN_FILE,
    install::{self, Report},
    server::{Handler, Server, Ticket},
    socket_path,
    tokens::{ClaudeLedger, CodexLedger, Summary, Totals},
    tracker::{Approval, Effect, Session, State, Tracker},
    usage::{self, CLAUDE_USAGE_FILE, Limit, Limits},
};
use macos_sys::{
    MainThreadMarker,
    agents_ui::{
        AgentsCallbacks, AgentsDisplay, AgentsView, ApprovalCardData, ChoiceData, SessionData,
        TokensData,
        UsageData,
    },
    app,
    notch::{Activity, Ear, Notch},
    style::Tone,
    workspace,
};
use std::{
    cell::{Cell, RefCell},
    collections::HashMap,
    fs,
    io::{self, Write},
    os::unix::fs::OpenOptionsExt,
    path::{Path, PathBuf},
    rc::{Rc, Weak},
    sync::{Arc, mpsc},
};

/// How often the page prunes answered questions and refreshes its ages, while there is anything to show.
const REFRESH_SECONDS: f64 = 1.0;
/// Reading the logs for the limits is cheap but not free: at most this often.
const USAGE_REFRESH_MS: i64 = 20_000;
/// How long the closed notch says that an agent finished.
const PEEK_SECONDS: f64 = 5.0;
/// The slot of the agents' activity in the closed notch.
const ACTIVITY: &str = "agents";
/// The tokens are counted again at most this often while someone looks at them. Reading what the agents appended
/// since the last count is cheap, but the first count of a week can read gigabytes.
const TOKENS_REFRESH_MS: i64 = 30_000;
const DAY_MS: i64 = 86_400_000;
/// Numbers older than this are shown as "as of ...": the percentage is probably not true any more.
const STALE_USAGE_MS: i64 = 30 * 60 * 1000;
const ASK_SOUND: &str = "Ping";
const DONE_SOUND: &str = "Glass";
const FAILED_SOUND: &str = "Basso";

thread_local! {
    /// How events from the server's threads find the feature, which only lives on the main thread.
    static FEATURE: RefCell<Weak<AgentsFeature>> = const { RefCell::new(Weak::new()) };
}

fn dispatch(event: HookEvent, ticket: Option<Ticket>) {
    let feature = FEATURE.with(|slot| slot.borrow().upgrade());
    // Without a feature the ticket is dropped here, which tells the agent to ask for itself.
    if let Some(feature) = feature {
        feature.handle(event, ticket);
    }
}

pub fn tone_of(state: State) -> Tone {
    match state {
        State::Waiting => Tone::Orange,
        State::Working => Tone::Blue,
        State::Done => Tone::Green,
        State::Failed => Tone::Red,
        State::Idle => Tone::Gray,
    }
}

fn short_name(agent: Agent) -> &'static str {
    match agent {
        Agent::Claude => "Claude",
        Agent::Codex => "Codex",
    }
}

/// How far the user is through the questions of one request: the answers so far and, for a question that takes
/// several choices, the ones ticked.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct Progress {
    pub answers: Vec<String>,
    pub ticked: Vec<usize>,
}

/// The card for a command that waits for an answer, or for the question the user is at.
pub fn approval_card(
    language: Language,
    approval: &Approval,
    progress: &Progress,
    now: i64,
) -> ApprovalCardData {
    let name = approval.agent.name();
    let detail = format!(
        "{}  -  {}",
        approval.project(),
        ago(language, now, approval.requested_ms)
    );
    if let Some(question) = approval.questions.get(progress.answers.len()) {
        let of = match approval.questions.len() {
            1 => String::new(),
            n => format!(" {}/{n}", progress.answers.len() + 1),
        };
        let heading = if question.header.is_empty() {
            format!("{name}{of}")
        } else {
            format!("{name} - {}{of}", question.header)
        };
        return ApprovalCardData {
            id: approval.id,
            heading,
            command: question.question.clone(),
            detail,
            allow: language.agents_done().to_owned(),
            deny: language.agents_terminal().to_owned(),
            choices: question
                .options
                .iter()
                .enumerate()
                .map(|(n, label)| ChoiceData {
                    label: label.clone(),
                    selected: progress.ticked.contains(&n),
                })
                .collect(),
            confirm: question.multi,
        };
    }
    ApprovalCardData {
        id: approval.id,
        heading: if approval.tool.is_empty() {
            language.agents_asks(name)
        } else {
            language.agents_wants(name, &approval.tool)
        },
        command: approval.detail.clone(),
        detail,
        allow: language.agents_allow().to_owned(),
        deny: language.agents_deny().to_owned(),
        choices: Vec::new(),
        confirm: true,
    }
}

pub fn session_row(language: Language, session: &Session, now: i64) -> SessionData {
    SessionData {
        tone: tone_of(session.state),
        agent: session.agent.name().to_owned(),
        project: session.project(),
        state: language.agents_state(session.state).to_owned(),
        age: ago(language, now, session.updated_ms),
    }
}

/// One bar of the usage section. A window that has started over since it was measured, or numbers that are
/// old, say so instead of showing a percentage nobody can rely on.
pub fn usage_row(
    language: Language,
    agent: Agent,
    limit: &Limit,
    observed_ms: i64,
    now_ms: i64,
) -> UsageData {
    let now_s = now_ms / 1000;
    let window = limit
        .window_minutes
        .map(|minutes| language.agents_window(minutes))
        .unwrap_or_default();
    let label = format!("{} {window}", short_name(agent)).trim().to_owned();
    if limit.has_reset(now_s) {
        return UsageData {
            label,
            fraction: None,
            value: "--".to_owned(),
            reset: language.agents_reset_already().to_owned(),
        };
    }
    let reset = if now_ms - observed_ms > STALE_USAGE_MS {
        language.agents_as_of(&ago(language, now_ms, observed_ms))
    } else {
        limit
            .resets_in(now_s)
            .map(|left| language.agents_resets_in(left))
            .unwrap_or_default()
    };
    UsageData {
        label,
        fraction: Some(limit.used_percent / 100.0),
        value: format!("{:.0}%", limit.used_percent),
        reset,
    }
}

/// The heading of the usage section, which also says how the last refresh went.
fn usage_heading(language: Language, refresh: Refresh, now: i64) -> String {
    let note = match refresh {
        Refresh::Idle | Refresh::Running => return language.agents_usage().to_owned(),
        Refresh::Done(at) => language.agents_updated(&ago(language, now, at)),
        Refresh::Failed => language.agents_refresh_failed().to_owned(),
    };
    format!("{}  -  {note}", language.agents_usage())
}

/// What one agent used today and in the last seven days, for the settings page.
#[derive(Debug, Clone, PartialEq)]
pub struct Spend {
    pub agent: &'static str,
    pub today: u64,
    pub week: u64,
    /// At API list prices; `None` when no price is known (Codex).
    pub today_cost: Option<f64>,
    pub week_cost: Option<f64>,
}

/// The latest counts from the transcripts, and when they were last asked for.
#[derive(Default)]
struct Counts {
    claude: Option<Summary>,
    codex: Option<Summary>,
    asked_ms: i64,
}

/// Local midnight today and six days before it, in milliseconds since 1970: the windows of "today" and "week".
fn day_and_week(now_ms: i64) -> (i64, i64) {
    let offset =
        i64::from(macos_sys::timezone::offset_minutes("local", now_ms / 1000).unwrap_or(0))
            * 60_000;
    let local = now_ms + offset;
    let day = local - local.rem_euclid(DAY_MS) - offset;
    (day, day - 6 * DAY_MS)
}

/// The counting thread: it owns both ledgers, so what they remember between counts never crosses threads. Only the
/// newest request in the queue is answered.
fn start_counter(user_home: PathBuf) -> Option<mpsc::Sender<(i64, i64)>> {
    let (tx, rx) = mpsc::channel::<(i64, i64)>();
    std::thread::Builder::new()
        .name("fig-agent-tokens".into())
        .spawn(move || {
            macos_sys::qos::utility();
            let mut claude = ClaudeLedger::new(user_home.join(".claude/projects"));
            let mut codex = CodexLedger::new(user_home.join(".codex/sessions"));
            while let Ok(mut window) = rx.recv() {
                while let Ok(newer) = rx.try_recv() {
                    window = newer;
                }
                let (day, week) = window;
                claude.refresh(week);
                codex.refresh(week);
                let counts = (claude.summary(day, week), codex.summary(day, week));
                app::on_main(move || {
                    let feature = FEATURE.with(|slot| slot.borrow().upgrade());
                    if let Some(feature) = feature {
                        feature.took_counts(counts.0, counts.1);
                    }
                });
            }
        })
        .ok()?;
    Some(tx)
}

#[derive(Default)]
struct UsageCache {
    claude: Option<Limits>,
    codex: Option<Limits>,
    /// What Codex's server said when the refresh button last asked. It is newer than the logs until they catch up.
    codex_live: Option<Limits>,
    read_at_ms: i64,
}

impl UsageCache {
    /// Codex's limits from whichever source measured them last: its logs, or the answer of its server.
    fn codex_latest(&self) -> Option<&Limits> {
        [&self.codex, &self.codex_live]
            .into_iter()
            .flatten()
            .max_by_key(|limits| limits.observed_ms)
    }
}

/// How the last press of the refresh button went; the heading of the usage section says so.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
enum Refresh {
    #[default]
    Idle,
    Running,
    /// Done at this time, in milliseconds since 1970.
    Done(i64),
    /// Codex was asked and did not give its limits.
    Failed,
}

/// What the settings page shows about one agent.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct AgentStatus {
    /// The agent's folder exists, so there is somewhere to put the hooks.
    pub found: bool,
    pub installed: usize,
    pub wanted: usize,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct Status {
    pub claude: AgentStatus,
    pub codex: AgentStatus,
    /// Our status line program is what Claude Code runs, so its usage limits reach the notch.
    pub usage_tap: bool,
    /// Limits from Claude Code have been received at least once.
    pub usage_known: bool,
    /// What each agent used, once counted.
    pub spend: Vec<Spend>,
}

/// Where the agents' own files (`.claude`, `.codex`) are looked for. Each time it starts, the feature rewrites
/// the paths of the hooks it installed, so a private instance (`FIGGHOST_HOME` set, which is how every test
/// runs) must never reach the real files: it gets an empty folder inside its own data directory, unless a
/// test points it somewhere on purpose with `FIGGHOST_AGENTS_HOME`.
fn agents_home(
    agents: Option<std::ffi::OsString>,
    private_data: Option<&Path>,
    real: Option<std::ffi::OsString>,
) -> PathBuf {
    match (agents, private_data) {
        (Some(dir), _) => PathBuf::from(dir),
        (None, Some(data)) => data.join("agents-home"),
        (None, None) => real.map(PathBuf::from).unwrap_or_default(),
    }
}

/// The files the agents read and the ones we keep, found from two directories. Nothing here needs the
/// interface, so the install, the removal and the status are tested on real-shaped files in a temp folder.
#[derive(Debug, Clone)]
pub struct Files {
    /// The app's data directory: the socket, the usage file and the saved status line command.
    pub home: PathBuf,
    /// Where `.claude` and `.codex` are; the test harness points it elsewhere.
    pub user_home: PathBuf,
}

impl Files {
    fn hooks_file(&self, agent: Agent) -> PathBuf {
        let targets = install::targets(&self.user_home);
        match agent {
            Agent::Claude => targets.claude_settings,
            Agent::Codex => targets.codex_hooks,
        }
    }

    fn agent_dir(&self, agent: Agent) -> PathBuf {
        let targets = install::targets(&self.user_home);
        match agent {
            Agent::Claude => targets.claude_dir,
            Agent::Codex => targets.codex_dir,
        }
    }

    /// What is installed now, read from the agents' files.
    pub fn status(&self) -> Status {
        let one = |agent: Agent| match install::status_file(&self.hooks_file(agent), agent) {
            Ok(s) => AgentStatus {
                found: self.agent_dir(agent).is_dir(),
                installed: s.installed,
                wanted: s.wanted,
                error: None,
            },
            Err(e) => AgentStatus {
                found: self.agent_dir(agent).is_dir(),
                installed: 0,
                wanted: install::subscriptions(agent).len(),
                error: Some(e.to_string()),
            },
        };
        let claude_doc = install::read_json(&self.hooks_file(Agent::Claude)).ok();
        Status {
            claude: one(Agent::Claude),
            codex: one(Agent::Codex),
            usage_tap: claude_doc
                .as_ref()
                .is_some_and(install::statusline_installed),
            usage_known: self.home.join(CLAUDE_USAGE_FILE).exists(),
            spend: Vec::new(),
        }
    }

    /// Adds the hooks to the agent's file, next to whatever is there.
    pub fn install(&self, agent: Agent, exe: &str) -> Result<Report, String> {
        install::install_file(&self.hooks_file(agent), agent, exe).map_err(|e| e.to_string())
    }

    pub fn remove(&self, agent: Agent) -> Result<usize, String> {
        install::remove_file(&self.hooks_file(agent)).map_err(|e| e.to_string())
    }

    /// Makes our program Claude Code's status line, which runs the previous one after reading the usage
    /// limits from its input.
    pub fn install_usage_tap(&self, exe: &str) -> Result<(), String> {
        let path = self.hooks_file(Agent::Claude);
        let mut doc = install::read_json(&path).map_err(|e| e.to_string())?;
        let change = install::install_statusline(&mut doc, exe).map_err(|e| e.to_string())?;
        if !change.changed {
            return Ok(());
        }
        // The previous command is saved before the file points at ours, so it is never lost.
        write_private(&self.home.join(CHAIN_FILE), change.chain.as_bytes())
            .map_err(|e| e.to_string())?;
        install::backup_once(&path).map_err(|e| e.to_string())?;
        install::write_json(&path, &doc).map_err(|e| e.to_string())
    }

    pub fn remove_usage_tap(&self) -> Result<(), String> {
        let path = self.hooks_file(Agent::Claude);
        let mut doc = install::read_json(&path).map_err(|e| e.to_string())?;
        if install::remove_statusline(&mut doc) {
            install::backup_once(&path).map_err(|e| e.to_string())?;
            install::write_json(&path, &doc).map_err(|e| e.to_string())?;
        }
        let _ = fs::remove_file(self.home.join(CHAIN_FILE));
        Ok(())
    }
}

pub struct AgentsFeature {
    mtm: MainThreadMarker,
    files: Files,
    notch: Weak<Notch>,
    page: usize,
    view: Rc<AgentsView>,
    settings: RefCell<AgentsConfig>,
    tracker: RefCell<Tracker>,
    tickets: RefCell<HashMap<u64, Ticket>>,
    /// Where the user is in each question request that is open.
    progress: RefCell<HashMap<u64, Progress>>,
    server: RefCell<Option<Server>>,
    ticker: RefCell<Option<app::Timer>>,
    limits: RefCell<UsageCache>,
    refresh: Cell<Refresh>,
    visible: Cell<bool>,
    /// The notch was closed when something here opened it, so it may be closed again when that is over.
    opened_by_us: Cell<bool>,
    /// Show what the agents do in the closed notch.
    live: Cell<bool>,
    /// An agent just finished (true) or failed (false); the closed notch says so until the time given.
    peek: Cell<Option<(bool, i64)>>,
    counts: RefCell<Counts>,
    /// What the closed notch shows about the agents now, for the trace.
    showing: Cell<&'static str>,
    /// Where count requests go; started the first time the tokens are wanted.
    counter: RefCell<Option<mpsc::Sender<(i64, i64)>>>,
    me: Weak<AgentsFeature>,
}

impl AgentsFeature {
    pub fn attach(
        mtm: MainThreadMarker,
        notch: &Rc<Notch>,
        home: &Path,
        config: &AgentsConfig,
    ) -> Rc<Self> {
        let user_home = agents_home(
            std::env::var_os("FIGGHOST_AGENTS_HOME"),
            std::env::var_os("FIGGHOST_HOME").map(|_| home),
            std::env::var_os("HOME"),
        );
        let feature = Rc::new_cyclic(|me: &Weak<AgentsFeature>| {
            let (decider, picker, refresher) = (me.clone(), me.clone(), me.clone());
            let view = AgentsView::new(
                mtm,
                notch.page_frame(),
                AgentsCallbacks {
                    on_decide: Box::new(move |id, allow| {
                        if let Some(feature) = decider.upgrade() {
                            feature.decide(id, allow);
                        }
                    }),
                    on_pick: Box::new(move |id, choice| {
                        if let Some(feature) = picker.upgrade() {
                            feature.pick(id, choice);
                        }
                    }),
                    on_refresh: Box::new(move || {
                        if let Some(feature) = refresher.upgrade() {
                            feature.refresh_limits();
                        }
                    }),
                },
            );
            Self {
                mtm,
                files: Files {
                    home: home.to_owned(),
                    user_home,
                },
                notch: Rc::downgrade(notch),
                page: notch.page_count(),
                view,
                settings: RefCell::new(config.clone()),
                tracker: RefCell::default(),
                tickets: RefCell::default(),
                progress: RefCell::default(),
                server: RefCell::default(),
                ticker: RefCell::default(),
                limits: RefCell::default(),
                refresh: Cell::default(),
                visible: Cell::new(false),
                opened_by_us: Cell::new(false),
                live: Cell::new(true),
                peek: Cell::new(None),
                counts: RefCell::default(),
                showing: Cell::new("nothing"),
                counter: RefCell::default(),
                me: me.clone(),
            }
        });
        let (shown, hidden) = (Rc::downgrade(&feature), Rc::downgrade(&feature));
        notch.add_page(feature.view.page(
            move || {
                if let Some(feature) = shown.upgrade() {
                    feature.visible.set(true);
                    feature.render();
                }
            },
            move || {
                if let Some(feature) = hidden.upgrade() {
                    feature.visible.set(false);
                }
            },
        ));
        FEATURE.with(|slot| *slot.borrow_mut() = Rc::downgrade(&feature));
        feature.configure(config);
        feature
    }

    /// Applies the settings: starts or stops listening, and passes on the wait.
    pub fn configure(&self, config: &AgentsConfig) {
        *self.settings.borrow_mut() = config.clone();
        if config.enabled {
            if let Some(server) = self.server.borrow().as_ref() {
                server.set_wait_seconds(config.approval_wait_seconds);
                return;
            }
            self.refresh_installed_hooks();
            self.start_server(config.approval_wait_seconds);
        } else {
            self.server.borrow_mut().take();
            // Questions that are still open go back to the agents' own terminals.
            self.tickets.borrow_mut().clear();
            *self.tracker.borrow_mut() = Tracker::default();
            self.sync_ticker();
            self.update_badge();
            self.render();
        }
        self.sync_activity();
    }

    /// Applies the notch settings that concern the agents: whether they show in the closed notch.
    pub fn configure_notch(&self, notch: &NotchConfig) {
        self.live.set(notch.live_agents);
        self.sync_activity();
    }

    fn start_server(&self, wait_seconds: u32) {
        let handler: Arc<Handler> = Arc::new(|event, ticket| {
            app::on_main(move || dispatch(event, ticket));
        });
        match Server::start(&socket_path(&self.files.home), wait_seconds, handler) {
            Ok(server) => *self.server.borrow_mut() = Some(server),
            Err(e) => eprintln!("agents: cannot listen for hooks: {e}"),
        }
    }

    /// Hooks we installed point at this program by its path; after the app moved they would point nowhere,
    /// so each start brings them up to date. Nothing is installed that was not already there.
    fn refresh_installed_hooks(&self) {
        let status = self.files.status();
        for (agent, agent_status) in [
            (Agent::Claude, &status.claude),
            (Agent::Codex, &status.codex),
        ] {
            if agent_status.installed > 0
                && let Err(e) = self.install(agent)
            {
                eprintln!("agents: cannot update the {} hooks: {e}", agent.name());
            }
        }
        if status.usage_tap {
            // Same for the status line: only its path can have changed.
            let _ = Self::exe().and_then(|exe| self.files.install_usage_tap(&exe));
        }
    }

    fn handle(&self, event: HookEvent, ticket: Option<Ticket>) {
        trace::log(&format!(
            "agents: {:?} from {} ({}){}",
            event.kind,
            event.agent.id(),
            core_agents::tracker::project_name(&event.cwd),
            if ticket.is_some() {
                ", waiting for an answer"
            } else {
                ""
            }
        ));
        let effect = self.tracker.borrow_mut().apply(&event);
        let settings = self.settings.borrow().clone();
        match effect {
            Effect::Approval(id) => {
                match ticket {
                    Some(ticket) => {
                        self.tickets.borrow_mut().insert(id, ticket);
                    }
                    None => {
                        self.tracker.borrow_mut().end_approval(id);
                    }
                }
                if settings.sound {
                    workspace::play_sound(ASK_SOUND);
                }
                // By default the question waits in the closed notch, which opens on it under the pointer; the whole
                // notch opening by itself while the user types elsewhere is what this avoids.
                if settings.open_on_ask || !self.live.get() {
                    self.open_page();
                }
            }
            Effect::Finished { .. } | Effect::Failed { .. } => {
                let done = matches!(effect, Effect::Finished { .. });
                if settings.sound {
                    workspace::play_sound(if done { DONE_SOUND } else { FAILED_SOUND });
                }
                if settings.peek_when_done {
                    self.peek
                        .set(Some((done, now_ms() + (PEEK_SECONDS * 1000.0) as i64)));
                    let weak = self.me.clone();
                    app::after(PEEK_SECONDS + 0.05, move || {
                        if let Some(feature) = weak.upgrade() {
                            feature.sync_activity();
                        }
                    });
                }
            }
            Effect::None => {}
        }
        // A session that ended or finished takes its open questions with it.
        let open: Vec<u64> = self
            .tracker
            .borrow()
            .approvals()
            .iter()
            .map(|a| a.id)
            .collect();
        self.tickets.borrow_mut().retain(|id, _| open.contains(id));
        self.progress.borrow_mut().retain(|id, _| open.contains(id));
        self.sync_ticker();
        self.update_badge();
        self.render();
        self.sync_activity();
    }

    /// What the closed notch shows about the agents: a question that waits (it breathes until answered), an agent
    /// that just finished or failed, or agents at work with the share of the five-hour limit used so far.
    fn sync_activity(&self) {
        let Some(notch) = self.notch.upgrade() else {
            return;
        };
        if !self.live.get() || !self.settings.borrow().enabled {
            self.show_activity(&notch, "nothing", None);
            return;
        }
        let now = now_ms();
        let (waiting, working) = {
            let tracker = self.tracker.borrow();
            let sessions = tracker.sessions();
            (
                !tracker.approvals().is_empty()
                    || sessions.iter().any(|s| s.state == State::Waiting),
                sessions.iter().any(|s| s.state == State::Working),
            )
        };
        let peek = self.peek.get().filter(|(_, until)| *until > now);
        if peek.is_none() {
            self.peek.set(None);
        }
        let language = i18n::current();
        let page = Some(self.page);
        let terminal = |tone| Ear::Symbol {
            name: "terminal",
            tone,
            pulse: false,
        };
        let (what, activity) = if waiting {
            (
                "a waiting question",
                Some(Activity {
                    detail: None,
                    left: terminal(Tone::Orange),
                    right: Ear::Symbol {
                        name: "hand.raised.fill",
                        tone: Tone::Orange,
                        pulse: true,
                    },
                    page,
                    priority: 30,
                }),
            )
        } else if let Some((done, _)) = peek {
            let (state, symbol, tone) = if done {
                (State::Done, "checkmark.circle.fill", Tone::Green)
            } else {
                (State::Failed, "xmark.octagon.fill", Tone::Red)
            };
            let what = if done {
                "a finished agent"
            } else {
                "a failed agent"
            };
            (
                what,
                Some(Activity {
                    detail: None,
                    left: Ear::Symbol {
                        name: symbol,
                        tone,
                        pulse: false,
                    },
                    right: Ear::Text {
                        text: language.agents_state(state).to_owned(),
                        tone,
                    },
                    page,
                    priority: 25,
                }),
            )
        } else if working {
            let right = match self.five_hour_used(now) {
                Some(used) => Ear::Text {
                    text: format!("{used:.0}%"),
                    tone: if used >= 90.0 {
                        Tone::Red
                    } else if used >= 70.0 {
                        Tone::Orange
                    } else {
                        Tone::Gray
                    },
                },
                None => Ear::Symbol {
                    name: "ellipsis",
                    tone: Tone::Gray,
                    pulse: true,
                },
            };
            (
                "a working agent",
                Some(Activity {
                    detail: None,
                    left: terminal(Tone::Blue),
                    right,
                    page,
                    priority: 10,
                }),
            )
        } else {
            ("nothing", None)
        };
        self.show_activity(&notch, what, activity);
    }

    fn show_activity(&self, notch: &Notch, what: &'static str, activity: Option<Activity>) {
        if self.showing.replace(what) != what {
            trace::log(&format!("agents: the closed notch shows {what}"));
        }
        notch.set_activity(ACTIVITY, activity);
    }

    /// The share of Claude Code's five-hour limit used, when it is known and still current.
    fn five_hour_used(&self, now: i64) -> Option<f64> {
        self.refresh_usage(now);
        let cache = self.limits.borrow();
        let limits = cache.claude.as_ref()?;
        let five = limits.primary.as_ref()?;
        let fresh = now - limits.observed_ms <= STALE_USAGE_MS && !five.has_reset(now / 1000);
        fresh.then_some(five.used_percent)
    }

    /// Opens the notch on this page, remembering whether it was closed before.
    fn open_page(&self) {
        let Some(notch) = self.notch.upgrade() else {
            return;
        };
        if !notch.is_expanded() {
            self.opened_by_us.set(true);
        }
        notch.open_page(self.page);
    }

    fn close_later(&self, seconds: f64) {
        let weak = self.me.clone();
        app::after(seconds, move || {
            if let Some(feature) = weak.upgrade() {
                feature.close_if_ours();
            }
        });
    }

    /// Closes the notch if this page opened it, nothing waits for an answer and the pointer is elsewhere.
    fn close_if_ours(&self) {
        let Some(notch) = self.notch.upgrade() else {
            return;
        };
        if self.opened_by_us.get()
            && self.tracker.borrow().approvals().is_empty()
            && notch.is_expanded()
            && !notch.pointer_inside()
        {
            notch.close();
        }
        if self.tracker.borrow().approvals().is_empty() {
            self.opened_by_us.set(false);
        }
    }

    fn decide(&self, id: u64, allow: bool) {
        let asks = self
            .tracker
            .borrow()
            .approval(id)
            .is_some_and(|a| !a.questions.is_empty());
        if asks {
            // Allow is "Done" on a question that takes several choices; deny is leaving it to the terminal.
            return if allow {
                self.confirm(id)
            } else {
                self.finish(id, Reply::Pass, "left to the terminal")
            };
        }
        let reply = if allow {
            Reply::Allow
        } else {
            Reply::Deny {
                message: i18n::current().agents_denied_message().to_owned(),
            }
        };
        self.finish(id, reply, if allow { "allowed" } else { "denied" });
    }

    /// A choice of the question the user is at: it answers a single-choice question, and ticks or unticks one
    /// of a question that takes several.
    fn pick(&self, id: u64, choice: usize) {
        let (label, multi) = {
            let tracker = self.tracker.borrow();
            let Some(approval) = tracker.approval(id) else {
                return;
            };
            let step = self.progress.borrow().get(&id).map_or(0, |p| p.answers.len());
            let Some(question) = approval.questions.get(step) else {
                return;
            };
            let Some(label) = question.options.get(choice) else {
                return;
            };
            (label.clone(), question.multi)
        };
        if multi {
            let mut progress = self.progress.borrow_mut();
            let ticked = &mut progress.entry(id).or_default().ticked;
            match ticked.iter().position(|&n| n == choice) {
                Some(at) => {
                    ticked.remove(at);
                }
                None => ticked.push(choice),
            }
            drop(progress);
            self.render();
        } else {
            self.answer(id, label);
        }
    }

    /// "Done" on a question that takes several choices: the ticked ones, in the order of the question.
    fn confirm(&self, id: u64) {
        let labels: Vec<String> = {
            let tracker = self.tracker.borrow();
            let progress = self.progress.borrow();
            let Some(progress) = progress.get(&id) else {
                return;
            };
            let Some(question) = tracker
                .approval(id)
                .and_then(|a| a.questions.get(progress.answers.len()))
            else {
                return;
            };
            let mut ticked = progress.ticked.clone();
            ticked.sort_unstable();
            ticked.iter().filter_map(|&n| question.options.get(n).cloned()).collect()
        };
        if !labels.is_empty() {
            self.answer(id, labels.join(", "));
        }
    }

    /// Takes the answer to the question the user is at, and sends them all once the last one is in.
    fn answer(&self, id: u64, answer: String) {
        let (done, total) = {
            let total = match self.tracker.borrow().approval(id) {
                Some(a) => a.questions.len(),
                None => return,
            };
            let mut progress = self.progress.borrow_mut();
            let progress = progress.entry(id).or_default();
            progress.answers.push(answer);
            progress.ticked.clear();
            (progress.answers.clone(), total)
        };
        if done.len() >= total {
            self.finish(id, Reply::Answers { answers: done }, "answered");
        } else {
            self.render();
        }
    }

    /// Gives the agent the reply for a request and takes the request off the page.
    fn finish(&self, id: u64, reply: Reply, what: &str) {
        let ticket = self.tickets.borrow_mut().remove(&id);
        self.progress.borrow_mut().remove(&id);
        let delivered = ticket.is_some_and(|ticket| ticket.answer(reply));
        trace::log(&format!(
            "agents: question {id} {what} ({})",
            if delivered {
                "delivered"
            } else {
                "nobody was waiting"
            }
        ));
        self.tracker.borrow_mut().end_approval(id);
        self.update_badge();
        self.render();
        self.sync_activity();
        if self.tracker.borrow().approvals().is_empty() {
            self.close_later(0.6);
        }
    }

    /// Runs while there is anything to show, and only then, so an idle app does no work for this page.
    fn sync_ticker(&self) {
        let busy = !self.tracker.borrow().is_empty();
        let mut ticker = self.ticker.borrow_mut();
        match (busy, ticker.is_some()) {
            (true, false) => {
                let weak = self.me.clone();
                *ticker = Some(app::every(REFRESH_SECONDS, move || {
                    if let Some(feature) = weak.upgrade() {
                        feature.tick();
                    }
                }));
            }
            (false, true) => {
                ticker.take();
            }
            _ => {}
        }
    }

    fn tick(&self) {
        // Questions nobody waits for any more: answered in the terminal, the hook was stopped, or the wait ran out.
        let gone: Vec<u64> = self
            .tickets
            .borrow()
            .iter()
            .filter(|(_, ticket)| !ticket.is_open())
            .map(|(id, _)| *id)
            .collect();
        for id in &gone {
            trace::log(&format!("agents: question {id} closed without an answer"));
            self.tickets.borrow_mut().remove(id);
            self.tracker.borrow_mut().end_approval(*id);
        }
        self.tracker.borrow_mut().sweep(now_ms());
        if !gone.is_empty() && self.tracker.borrow().approvals().is_empty() {
            self.close_later(0.6);
        }
        self.sync_ticker();
        self.update_badge();
        self.render();
        self.sync_activity();
    }

    fn update_badge(&self) {
        if let Some(notch) = self.notch.upgrade() {
            notch.set_page_badge(self.page, !self.tracker.borrow().approvals().is_empty());
        }
    }

    fn refresh_usage(&self, now: i64) {
        let mut cache = self.limits.borrow_mut();
        if now - cache.read_at_ms < USAGE_REFRESH_MS {
            return;
        }
        cache.read_at_ms = now;
        cache.claude = usage::read_limits(&self.files.home.join(CLAUDE_USAGE_FILE));
        cache.codex = usage::codex_limits(&self.files.user_home.join(".codex/sessions"));
    }

    /// The refresh button: reads the limits again at once, and asks Codex for them as its server has them, which is
    /// newer than its session logs. Claude Code cannot be asked, so its numbers are as new as its last status line.
    fn refresh_limits(&self) {
        if self.refresh.get() == Refresh::Running {
            return;
        }
        self.limits.borrow_mut().read_at_ms = 0;
        let Some(codex) = usage::find_codex(&self.files.user_home) else {
            self.refresh.set(Refresh::Done(now_ms()));
            self.render();
            return;
        };
        self.refresh.set(Refresh::Running);
        let home = self.files.user_home.clone();
        let started = std::thread::Builder::new()
            .name("fig-agent-limits".into())
            .spawn(move || {
                macos_sys::qos::utility();
                let answer = usage::codex_limits_live(&codex, &home, now_ms(), usage::LIVE_TIMEOUT);
                app::on_main(move || {
                    let feature = FEATURE.with(|slot| slot.borrow().upgrade());
                    if let Some(feature) = feature {
                        feature.took_live_limits(answer);
                    }
                });
            });
        if started.is_err() {
            self.refresh.set(Refresh::Failed);
        }
        self.render();
    }

    fn took_live_limits(&self, answer: Result<Limits, String>) {
        match answer {
            Ok(limits) => {
                trace::log("agents: Codex limits refreshed");
                self.limits.borrow_mut().codex_live = Some(limits);
                self.refresh.set(Refresh::Done(now_ms()));
            }
            Err(e) => {
                trace::log(&format!("agents: Codex limits: {e}"));
                self.refresh.set(Refresh::Failed);
            }
        }
        self.render();
    }

    fn usage_rows(&self, language: Language, now: i64) -> Vec<UsageData> {
        self.refresh_usage(now);
        let cache = self.limits.borrow();
        let mut rows = Vec::new();
        for (agent, limits) in [
            (Agent::Claude, cache.claude.as_ref()),
            (Agent::Codex, cache.codex_latest()),
        ] {
            let Some(limits) = limits else {
                continue;
            };
            for limit in [&limits.primary, &limits.secondary].into_iter().flatten() {
                rows.push(usage_row(language, agent, limit, limits.observed_ms, now));
            }
        }
        rows
    }

    /// Redraws the page, if it is the one on screen.
    fn render(&self) {
        if !self.visible.get() {
            return;
        }
        let (language, now) = (i18n::current(), now_ms());
        self.want_counts(now);
        let (approval, more_waiting, sessions) = {
            let tracker = self.tracker.borrow();
            let waiting = tracker.approvals();
            (
                waiting.first().map(|a| {
                    let progress = self.progress.borrow().get(&a.id).cloned().unwrap_or_default();
                    approval_card(language, a, &progress, now)
                }),
                match waiting.len() {
                    0 | 1 => String::new(),
                    n => language.agents_more_waiting(n - 1),
                },
                tracker
                    .sessions()
                    .into_iter()
                    .map(|s| session_row(language, s, now))
                    .collect::<Vec<_>>(),
            )
        };
        let usage = self.usage_rows(language, now);
        // With no numbers yet the button stays when Codex can be asked: that is how the first ones come.
        let refresh_label = (!usage.is_empty()
            || usage::find_codex(&self.files.user_home).is_some())
        .then(|| language.agents_refresh().to_owned());
        self.view.show(
            self.mtm,
            &AgentsDisplay {
                approval,
                more_waiting,
                sessions_heading: language.agents_sessions().to_owned(),
                sessions,
                usage_heading: usage_heading(language, self.refresh.get(), now),
                usage,
                refresh_label,
                refreshing: self.refresh.get() == Refresh::Running,
                tokens_heading: language.agents_tokens().to_owned(),
                tokens: self.token_rows(language),
                empty_title: language.agents_empty_title().to_owned(),
                empty_hint: language.agents_empty_hint().to_owned(),
            },
        );
    }

    fn exe() -> Result<String, String> {
        std::env::current_exe()
            .map(|p| p.to_string_lossy().into_owned())
            .map_err(|e| e.to_string())
    }

    pub fn status(&self) -> Status {
        self.want_counts(now_ms());
        Status {
            spend: self.spend(),
            ..self.files.status()
        }
    }

    /// Asks the counting thread for fresh numbers, at most every so often; they arrive later on the main thread.
    fn want_counts(&self, now: i64) {
        if !self.settings.borrow().tokens {
            return;
        }
        {
            let mut counts = self.counts.borrow_mut();
            if now - counts.asked_ms < TOKENS_REFRESH_MS {
                return;
            }
            counts.asked_ms = now;
        }
        let mut counter = self.counter.borrow_mut();
        if counter.is_none() {
            *counter = start_counter(self.files.user_home.clone());
        }
        if let Some(tx) = counter.as_ref()
            && tx.send(day_and_week(now)).is_err()
        {
            counter.take();
        }
    }

    fn took_counts(&self, claude: Summary, codex: Summary) {
        {
            let mut counts = self.counts.borrow_mut();
            counts.claude = Some(claude);
            counts.codex = Some(codex);
        }
        self.render();
    }

    /// Per agent: today and the week, with the cost when there is a price. Agents with nothing this week are left out.
    fn spend(&self) -> Vec<Spend> {
        if !self.settings.borrow().tokens {
            return Vec::new();
        }
        let counts = self.counts.borrow();
        let cost = |t: &Totals| (t.messages > t.unpriced).then_some(t.cost_usd);
        [("claude", &counts.claude), ("codex", &counts.codex)]
            .into_iter()
            .filter_map(|(agent, summary)| {
                let s = summary.as_ref()?;
                (s.week.tokens() > 0).then(|| Spend {
                    agent,
                    today: s.today.tokens(),
                    week: s.week.tokens(),
                    today_cost: cost(&s.today),
                    week_cost: cost(&s.week),
                })
            })
            .collect()
    }

    fn token_rows(&self, language: Language) -> Vec<TokensData> {
        self.spend()
            .into_iter()
            .map(|spend| TokensData {
                label: if spend.agent == "claude" {
                    "Claude"
                } else {
                    "Codex"
                }
                .to_owned(),
                today: language.tokens_today(spend.today, spend.today_cost),
                week: language.tokens_week(spend.week, spend.week_cost),
            })
            .collect()
    }

    pub fn install(&self, agent: Agent) -> Result<Report, String> {
        self.files.install(agent, &Self::exe()?)
    }

    pub fn remove(&self, agent: Agent) -> Result<usize, String> {
        self.files.remove(agent)
    }

    pub fn install_usage_tap(&self) -> Result<(), String> {
        self.files.install_usage_tap(&Self::exe()?)
    }

    pub fn remove_usage_tap(&self) -> Result<(), String> {
        self.files.remove_usage_tap()
    }
}

fn write_private(path: &Path, bytes: &[u8]) -> io::Result<()> {
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir)?;
    }
    let mut file = fs::OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .mode(0o600)
        .open(path)?;
    file.write_all(bytes)
}

/// `Figghost --figghost-hook claude|codex`: what an agent runs for each event. Returns the exit status.
pub fn run_hook_cli(home: &Path, agent: Option<&str>) -> i32 {
    let Some(agent) = agent.and_then(Agent::from_id) else {
        return 0;
    };
    core_agents::cli::run_hook(
        agent,
        io::stdin().lock(),
        &mut io::stdout().lock(),
        &socket_path(home),
        now_ms(),
    )
}

/// `Figghost --figghost-statusline`: Claude Code's status line while the usage tap is on.
pub fn run_statusline_cli(home: &Path) -> i32 {
    core_agents::cli::run_statusline(io::stdin().lock(), &mut io::stdout().lock(), home, now_ms())
}

#[cfg(test)]
mod tests {
    use super::*;
    use core_agents::{event::EventKind, tracker::Tracker};

    fn approval() -> Approval {
        let mut tracker = Tracker::default();
        let effect = tracker.apply(&HookEvent {
            agent: Agent::Claude,
            kind: EventKind::PermissionRequest,
            session_id: "s".into(),
            cwd: "/Users/me/shop".into(),
            tool: Some("Bash".into()),
            detail: Some("rm -rf build".into()),
            message: None,
            questions: Vec::new(),
            at_ms: 1_000,
        });
        let Effect::Approval(id) = effect else {
            panic!()
        };
        tracker.approval(id).unwrap().clone()
    }

    #[test]
    fn a_card_names_the_agent_the_tool_the_command_and_where() {
        let card = approval_card(Language::English, &approval(), &Progress::default(), 1_000 + 120_000);
        assert_eq!(card.heading, "Claude Code wants to use Bash");
        assert_eq!(card.command, "rm -rf build");
        assert_eq!(card.detail, "shop  -  2 minutes ago");
        assert_eq!((card.allow.as_str(), card.deny.as_str()), ("Allow", "Deny"));
        let vi = approval_card(Language::Vietnamese, &approval(), &Progress::default(), 1_000);
        assert_eq!(vi.heading, "Claude Code muốn dùng Bash");
        assert_eq!(vi.allow, "Cho phép");
    }

    #[test]
    fn a_question_card_has_a_button_per_choice_and_follows_the_progress() {
        let mut a = approval();
        a.questions = ["First?", "Second?"]
            .map(|question| core_agents::event::Question {
                header: "Pick".into(),
                question: question.into(),
                options: vec!["A".into(), "B".into()],
                multi: false,
            })
            .to_vec();
        let first = approval_card(Language::English, &a, &Progress::default(), 1_000);
        assert_eq!(first.heading, "Claude Code - Pick 1/2");
        assert_eq!(first.command, "First?");
        assert_eq!(first.choices.len(), 2);
        assert!(!first.confirm, "a single choice answers by itself");
        let seen = Progress {
            answers: vec!["A".into()],
            ticked: vec![1],
        };
        let second = approval_card(Language::English, &a, &seen, 1_000);
        assert_eq!(second.command, "Second?");
        assert!(second.choices[1].selected && !second.choices[0].selected);
    }

    #[test]
    fn a_request_without_a_tool_still_has_a_heading() {
        let mut a = approval();
        a.tool.clear();
        assert_eq!(
            approval_card(Language::English, &a, &Progress::default(), 1_000).heading,
            "Claude Code is asking for permission"
        );
    }

    fn limit(used: f64, resets_at: Option<i64>) -> Limit {
        Limit {
            used_percent: used,
            window_minutes: Some(300),
            resets_at,
        }
    }

    fn limits_at(used: f64, observed_ms: i64) -> Limits {
        Limits {
            primary: Some(limit(used, None)),
            secondary: None,
            observed_ms,
            plan: None,
        }
    }

    #[test]
    fn what_the_server_said_stands_until_the_logs_are_newer() {
        let mut cache = UsageCache::default();
        assert!(cache.codex_latest().is_none());
        cache.codex = Some(limits_at(10.0, 1_000));
        assert_eq!(cache.codex_latest().unwrap().observed_ms, 1_000);
        // The server's answer is newer than the logs, and the logs being read again every few seconds must not
        // put the older numbers back.
        cache.codex_live = Some(limits_at(20.0, 2_000));
        assert_eq!(cache.codex_latest().unwrap().observed_ms, 2_000);
        // Once Codex has worked since, its log is the newest.
        cache.codex = Some(limits_at(30.0, 3_000));
        assert_eq!(cache.codex_latest().unwrap().observed_ms, 3_000);
        cache.codex = None;
        assert_eq!(cache.codex_latest().unwrap().observed_ms, 2_000);
    }

    #[test]
    fn the_heading_says_how_the_last_refresh_went() {
        let (en, vi) = (Language::English, Language::Vietnamese);
        assert_eq!(usage_heading(en, Refresh::Idle, 0), "Usage limits");
        assert_eq!(usage_heading(en, Refresh::Running, 0), "Usage limits");
        assert_eq!(
            usage_heading(en, Refresh::Done(1_000), 1_000 + 120_000),
            "Usage limits  -  updated 2 minutes ago"
        );
        assert_eq!(
            usage_heading(en, Refresh::Failed, 0),
            "Usage limits  -  Codex limits unavailable"
        );
        assert_eq!(
            usage_heading(vi, Refresh::Done(0), 0),
            "Hạn mức sử dụng  -  cập nhật vừa xong"
        );
    }

    #[test]
    fn a_fresh_limit_shows_its_percentage_and_when_it_resets() {
        let now = 1_792_000_000_000;
        let row = usage_row(
            Language::English,
            Agent::Codex,
            &limit(54.0, Some(1_792_000_000 + 7_800)),
            now - 60_000,
            now,
        );
        assert_eq!(row.label, "Codex 5 h");
        assert_eq!(row.value, "54%");
        assert_eq!(row.fraction, Some(0.54));
        assert_eq!(row.reset, "resets in 2 h 10 min");
    }

    #[test]
    fn a_limit_whose_window_has_started_over_shows_no_percentage() {
        let now = 1_792_000_000_000;
        let row = usage_row(
            Language::English,
            Agent::Claude,
            &limit(90.0, Some(1_792_000_000 - 5)),
            now - 60_000,
            now,
        );
        assert_eq!(row.fraction, None);
        assert_eq!(row.value, "--");
        assert_eq!(row.reset, "has reset");
    }

    #[test]
    fn old_numbers_say_how_old_they_are() {
        let now = 1_792_000_000_000;
        let two_days = 2 * 86_400 * 1000;
        let row = usage_row(
            Language::English,
            Agent::Claude,
            &limit(40.0, Some(1_792_000_000 + 3_600)),
            now - two_days,
            now,
        );
        assert_eq!(row.fraction, Some(0.40));
        assert_eq!(row.reset, "as of 2 days ago");
    }

    #[test]
    fn a_limit_of_a_month_is_labelled_in_days() {
        let now = 1_792_000_000_000;
        let month = Limit {
            window_minutes: Some(43_200),
            ..limit(54.0, Some(1_792_000_000 + 86_400 * 10))
        };
        let row = usage_row(Language::English, Agent::Codex, &month, now, now);
        assert_eq!(row.label, "Codex 30 d");
        assert_eq!(row.reset, "resets in 10 d");
    }

    #[test]
    fn every_state_has_a_distinct_colour_for_the_states_that_matter() {
        assert_ne!(tone_of(State::Waiting), tone_of(State::Working));
        assert_ne!(tone_of(State::Done), tone_of(State::Failed));
        assert_eq!(tone_of(State::Idle), Tone::Gray);
    }

    /// The shapes found in real files: hooks of other tools, a status line of the user's own, other keys.
    const USER_SETTINGS: &str = r#"{
      "model": "opus",
      "statusLine": { "type": "command", "command": "'/Users/me/bin/status'", "padding": 2 },
      "hooks": {
        "Notification": [ { "matcher": "*", "hooks": [ { "type": "command", "command": "'/Users/me/other/hook'", "timeout": 45 } ] } ],
        "PermissionRequest": [ { "matcher": "*", "hooks": [ { "type": "command", "command": "'/Users/me/other/hook'", "timeout": 3600 } ] } ]
      },
      "theme": "dark"
    }"#;

    struct Sandbox {
        root: PathBuf,
        files: Files,
    }

    impl Sandbox {
        fn new(name: &str, with_agents: bool) -> Self {
            let root =
                std::env::temp_dir().join(format!("figghost-agents-{name}-{}", std::process::id()));
            let _ = fs::remove_dir_all(&root);
            let (home, user_home) = (root.join("data"), root.join("user"));
            fs::create_dir_all(&home).unwrap();
            fs::create_dir_all(&user_home).unwrap();
            if with_agents {
                fs::create_dir_all(user_home.join(".claude")).unwrap();
                fs::create_dir_all(user_home.join(".codex")).unwrap();
                fs::write(user_home.join(".claude/settings.json"), USER_SETTINGS).unwrap();
            }
            Self {
                root,
                files: Files { home, user_home },
            }
        }

        fn settings(&self) -> serde_json::Value {
            serde_json::from_str(
                &fs::read_to_string(self.files.user_home.join(".claude/settings.json")).unwrap(),
            )
            .unwrap()
        }
    }

    impl Drop for Sandbox {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.root);
        }
    }

    const EXE: &str = "/Applications/Figghost.app/Contents/MacOS/Figghost";

    #[test]
    fn a_private_instance_never_reaches_the_real_agent_files() {
        let data = Path::new("/tmp/private/data");
        let real = || Some(std::ffi::OsString::from("/Users/me"));
        assert_eq!(agents_home(None, None, real()), PathBuf::from("/Users/me"));
        assert_eq!(
            agents_home(None, Some(data), real()),
            PathBuf::from("/tmp/private/data/agents-home"),
            "a private instance gets a folder of its own, whatever HOME is"
        );
        assert_eq!(
            agents_home(Some("/tmp/pretend".into()), Some(data), real()),
            PathBuf::from("/tmp/pretend"),
            "unless a test points it somewhere on purpose"
        );
        assert_eq!(agents_home(None, None, None), PathBuf::new());
    }

    #[test]
    fn without_the_agents_installed_nothing_is_found_and_nothing_is_created() {
        let sandbox = Sandbox::new("absent", false);
        let status = sandbox.files.status();
        assert!(!status.claude.found && !status.codex.found);
        assert_eq!((status.claude.installed, status.codex.installed), (0, 0));
        assert!(sandbox.files.install(Agent::Codex, EXE).is_err());
        assert!(
            !sandbox.files.user_home.join(".codex").exists(),
            "no folder is made for an agent that is not there"
        );
    }

    #[test]
    fn installing_and_removing_leaves_the_users_file_exactly_as_it_was() {
        let sandbox = Sandbox::new("roundtrip", true);
        let original = sandbox.settings();
        assert_eq!(
            sandbox.files.status().claude,
            AgentStatus {
                found: true,
                installed: 0,
                wanted: 9,
                error: None
            }
        );
        let report = sandbox.files.install(Agent::Claude, EXE).unwrap();
        assert_eq!(report.added, 9);
        let status = sandbox.files.status().claude;
        assert_eq!((status.installed, status.wanted), (9, 9));
        // The other tools' hooks and the rest of the file are still there.
        let now = sandbox.settings();
        assert_eq!(now["model"], original["model"]);
        assert_eq!(now["statusLine"], original["statusLine"]);
        assert_eq!(
            now["hooks"]["PermissionRequest"][0],
            original["hooks"]["PermissionRequest"][0]
        );
        assert_eq!(sandbox.files.remove(Agent::Claude).unwrap(), 9);
        assert_eq!(sandbox.settings(), original);
        assert_eq!(sandbox.files.status().claude.installed, 0);
    }

    #[test]
    fn codex_is_installed_in_its_own_file_and_does_not_disturb_claude() {
        let sandbox = Sandbox::new("codex", true);
        let before = sandbox.settings();
        sandbox.files.install(Agent::Codex, EXE).unwrap();
        let status = sandbox.files.status();
        assert_eq!((status.codex.installed, status.codex.wanted), (7, 7));
        assert_eq!(status.claude.installed, 0);
        assert_eq!(sandbox.settings(), before);
        assert!(sandbox.files.user_home.join(".codex/hooks.json").exists());
    }

    #[test]
    fn the_usage_tap_wraps_the_status_line_keeps_it_and_gives_it_back() {
        use std::os::unix::fs::PermissionsExt;
        let sandbox = Sandbox::new("tap", true);
        let original = sandbox.settings();
        assert!(!sandbox.files.status().usage_tap);
        sandbox.files.install_usage_tap(EXE).unwrap();
        assert!(sandbox.files.status().usage_tap);
        let chain = sandbox.files.home.join(CHAIN_FILE);
        assert_eq!(
            fs::read_to_string(&chain).unwrap(),
            "'/Users/me/bin/status'"
        );
        assert_eq!(
            fs::metadata(&chain).unwrap().permissions().mode() & 0o777,
            0o600
        );
        assert!(
            sandbox.settings()["statusLine"]["command"]
                .as_str()
                .unwrap()
                .contains("--figghost-statusline")
        );
        // Turning it on twice must not lose the real status line.
        sandbox.files.install_usage_tap(EXE).unwrap();
        assert_eq!(
            fs::read_to_string(&chain).unwrap(),
            "'/Users/me/bin/status'"
        );
        sandbox.files.remove_usage_tap().unwrap();
        assert_eq!(sandbox.settings(), original);
        assert!(!chain.exists());
        assert!(!sandbox.files.status().usage_tap);
    }

    #[test]
    fn received_limits_are_noticed() {
        let sandbox = Sandbox::new("known", true);
        assert!(!sandbox.files.status().usage_known);
        fs::write(sandbox.files.home.join(CLAUDE_USAGE_FILE), "{}").unwrap();
        assert!(sandbox.files.status().usage_known);
    }

    #[test]
    fn a_file_that_cannot_be_read_is_reported_and_never_overwritten() {
        let sandbox = Sandbox::new("broken", true);
        let path = sandbox.files.user_home.join(".claude/settings.json");
        fs::write(&path, "{ not json").unwrap();
        let status = sandbox.files.status().claude;
        assert!(status.error.is_some() && status.installed == 0);
        assert!(sandbox.files.install(Agent::Claude, EXE).is_err());
        assert!(sandbox.files.install_usage_tap(EXE).is_err());
        assert_eq!(fs::read_to_string(&path).unwrap(), "{ not json");
    }
}
