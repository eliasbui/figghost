//! Coding agents on the beta: the hooks socket, what each session is doing, and the commands that wait for an answer.
//! The rules are those of `core-agents`, as on macOS; this only keeps them for the window.

use core_agents::{
    Agent, HookEvent, Reply, install,
    server::{Server, Ticket},
    socket_path,
    tracker::{Effect, State, Tracker},
};
use serde_json::{Value, json};
use std::{
    collections::HashMap,
    path::{Path, PathBuf},
    sync::Arc,
};

/// pi's extension reaches the socket from Node, which on Windows only knows named pipes.
pub fn agents() -> &'static [Agent] {
    if cfg!(windows) {
        &[Agent::Claude, Agent::Codex]
    } else {
        &[Agent::Claude, Agent::Codex, Agent::Pi]
    }
}

pub struct Agents {
    home: PathBuf,
    user_home: PathBuf,
    server: Option<Server>,
    tracker: Tracker,
    tickets: HashMap<u64, Ticket>,
}

impl Agents {
    pub fn new(home: &Path) -> Self {
        let user_home = std::env::var_os("HOME")
            .or_else(|| std::env::var_os("USERPROFILE"))
            .map(PathBuf::from)
            .unwrap_or_default();
        Self {
            home: home.to_owned(),
            user_home,
            server: None,
            tracker: Tracker::default(),
            tickets: HashMap::new(),
        }
    }

    /// Starts or stops listening to the hooks. `send` is called on the server's threads.
    pub fn set_enabled(
        &mut self,
        on: bool,
        wait_seconds: u32,
        send: impl Fn(HookEvent, Option<Ticket>) + Send + Sync + 'static,
    ) -> Result<(), String> {
        if !on {
            self.server = None;
            self.tickets.clear();
            self.tracker = Tracker::default();
            return Ok(());
        }
        if let Some(server) = &self.server {
            server.set_wait_seconds(wait_seconds);
            return Ok(());
        }
        let server = Server::start(&socket_path(&self.home), wait_seconds, Arc::new(send))
            .map_err(|e| e.to_string())?;
        self.server = Some(server);
        Ok(())
    }

    /// Takes an event in; true when it opened a question for the user.
    pub fn receive(&mut self, event: &HookEvent, ticket: Option<Ticket>) -> bool {
        let effect = self.tracker.apply(event);
        match (effect, ticket) {
            (Effect::Approval(id), Some(ticket)) => {
                self.tickets.insert(id, ticket);
                true
            }
            // A ticket that is dropped lets the agent ask in its terminal.
            _ => false,
        }
    }

    /// Forgets old sessions and the questions whose hook is gone or whose wait ran out.
    pub fn sweep(&mut self, now_ms: i64) {
        self.tracker.sweep(now_ms);
        let closed: Vec<u64> = self
            .tickets
            .iter()
            .filter(|(_, t)| !t.is_open())
            .map(|(id, _)| *id)
            .collect();
        for id in closed {
            self.tickets.remove(&id);
            self.tracker.end_approval(id);
        }
    }

    pub fn waiting(&self) -> usize {
        self.tracker.approvals().len()
    }

    /// `allow` for a command; a question is passed back to the terminal whatever is chosen.
    pub fn answer(&mut self, id: u64, allow: bool) {
        let Some(ticket) = self.tickets.remove(&id) else {
            return;
        };
        let is_question = self
            .tracker
            .approval(id)
            .is_some_and(|a| !a.questions.is_empty());
        let reply = match (is_question, allow) {
            (true, _) => Reply::Pass,
            (false, true) => Reply::Allow,
            (false, false) => Reply::Deny {
                message: "Denied from Figghost.".into(),
            },
        };
        ticket.answer(reply);
        self.tracker.end_approval(id);
    }

    fn hooks_file(&self, agent: Agent) -> PathBuf {
        let targets = install::targets(&self.user_home);
        match agent {
            Agent::Claude => targets.claude_settings,
            Agent::Codex => targets.codex_hooks,
            Agent::Pi => targets.pi_extension,
            Agent::Agy => targets.agy_hooks,
        }
    }

    pub fn install(&self, agent: Agent, exe: &str) -> Result<(), String> {
        let path = self.hooks_file(agent);
        if agent == Agent::Pi {
            return install::install_pi(&path, &socket_path(&self.home))
                .map(drop)
                .map_err(|e| e.to_string());
        }
        install::install_file(&path, agent, exe)
            .map(drop)
            .map_err(|e| e.to_string())
    }

    pub fn remove(&self, agent: Agent) -> Result<(), String> {
        let path = self.hooks_file(agent);
        let removed = if agent == Agent::Pi {
            install::remove_pi(&path)
        } else {
            install::remove_file(&path)
        };
        removed.map(drop).map_err(|e| e.to_string())
    }

    fn installed(&self, agent: Agent) -> bool {
        let path = self.hooks_file(agent);
        if agent == Agent::Pi {
            return install::pi_installed(&path);
        }
        install::status_file(&path, agent).is_ok_and(|s| s.installed > 0)
    }

    pub fn state(&self) -> Value {
        let state_name = |state: State| match state {
            State::Waiting => "waiting",
            State::Working => "working",
            State::Failed => "failed",
            State::Done => "done",
            State::Idle => "idle",
        };
        json!({
            "listening": self.server.is_some(),
            "agents": agents().iter().map(|a| json!({
                "id": a.id(),
                "name": a.name(),
                "installed": self.installed(*a),
            })).collect::<Vec<_>>(),
            "sessions": self.tracker.sessions().iter().map(|s| json!({
                "agent": s.agent.name(),
                "project": s.project(),
                "state": state_name(s.state),
                "note": s.note,
                "updated": s.updated_ms,
            })).collect::<Vec<_>>(),
            "approvals": self.tracker.approvals().iter().map(|a| json!({
                "id": a.id,
                "agent": a.agent.name(),
                "project": a.project(),
                "tool": a.tool,
                "detail": a.detail,
                "question": a.questions.first().map(|q| q.question.clone()),
                "requested": a.requested_ms,
            })).collect::<Vec<_>>(),
        })
    }
}
