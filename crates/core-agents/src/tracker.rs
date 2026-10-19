//! Which sessions exist and what each is doing, built from the events the hooks report.

use crate::event::{Agent, EventKind, HookEvent, Question};
use std::cmp::Reverse;

/// A session that reports nothing for this long while "working" is more likely stuck or closed than busy.
const WORKING_GOES_QUIET_MS: i64 = 15 * 60 * 1000;
/// Sessions that were not heard from for this long are forgotten.
const KEEP_MS: i64 = 6 * 60 * 60 * 1000;
const MAX_SESSIONS: usize = 40;

/// The order is the order of importance on screen.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum State {
    /// A question for the user is open.
    Waiting,
    Working,
    Failed,
    /// The last turn finished.
    Done,
    Idle,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Session {
    pub agent: Agent,
    pub id: String,
    pub cwd: String,
    pub state: State,
    /// What the session waits for, when it waits.
    pub note: Option<String>,
    pub started_ms: i64,
    pub updated_ms: i64,
}

impl Session {
    pub fn project(&self) -> String {
        project_name(&self.cwd)
    }
}

/// The folder a session works in, by its last component.
pub fn project_name(cwd: &str) -> String {
    let trimmed = cwd.trim_end_matches('/');
    match trimmed.rsplit('/').next() {
        Some(name) if !name.is_empty() => name.to_owned(),
        _ if cwd.is_empty() => "?".to_owned(),
        _ => "/".to_owned(),
    }
}

/// A command or edit that waits for the user.
#[derive(Debug, Clone, PartialEq)]
pub struct Approval {
    pub id: u64,
    pub agent: Agent,
    pub session_id: String,
    pub cwd: String,
    pub tool: String,
    pub detail: String,
    /// The questions to put to the user when the agent asked with choices.
    pub questions: Vec<Question>,
    pub requested_ms: i64,
}

impl Approval {
    pub fn project(&self) -> String {
        project_name(&self.cwd)
    }
}

/// What the app should do about an event, beyond redrawing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Effect {
    None,
    /// A new question was opened; it needs an answer.
    Approval(u64),
    Finished {
        agent: Agent,
        project: String,
    },
    Failed {
        agent: Agent,
        project: String,
    },
}

#[derive(Debug, Default)]
pub struct Tracker {
    sessions: Vec<Session>,
    approvals: Vec<Approval>,
    next_approval: u64,
}

impl Tracker {
    fn session_mut(&mut self, event: &HookEvent) -> &mut Session {
        let found = self
            .sessions
            .iter()
            .position(|s| s.agent == event.agent && s.id == event.session_id);
        let index = found.unwrap_or_else(|| {
            self.sessions.push(Session {
                agent: event.agent,
                id: event.session_id.clone(),
                cwd: String::new(),
                state: State::Idle,
                note: None,
                started_ms: event.at_ms,
                updated_ms: event.at_ms,
            });
            self.sessions.len() - 1
        });
        let session = &mut self.sessions[index];
        if !event.cwd.is_empty() {
            session.cwd.clone_from(&event.cwd);
        }
        session.updated_ms = event.at_ms;
        session
    }

    pub fn apply(&mut self, event: &HookEvent) -> Effect {
        match event.kind {
            EventKind::Other => Effect::None,
            EventKind::SessionEnd => {
                self.sessions
                    .retain(|s| !(s.agent == event.agent && s.id == event.session_id));
                self.approvals
                    .retain(|a| !(a.agent == event.agent && a.session_id == event.session_id));
                Effect::None
            }
            EventKind::PermissionRequest | EventKind::Question => {
                let session = self.session_mut(event);
                session.state = State::Waiting;
                session.note.clone_from(&event.detail);
                let cwd = session.cwd.clone();
                self.next_approval += 1;
                let id = self.next_approval;
                self.approvals.push(Approval {
                    id,
                    agent: event.agent,
                    session_id: event.session_id.clone(),
                    cwd,
                    tool: event.tool.clone().unwrap_or_default(),
                    detail: event.detail.clone().unwrap_or_default(),
                    questions: event.questions.clone(),
                    requested_ms: event.at_ms,
                });
                self.trim();
                Effect::Approval(id)
            }
            EventKind::SessionStart => {
                let session = self.session_mut(event);
                session.state = State::Idle;
                session.note = None;
                self.trim();
                Effect::None
            }
            EventKind::PromptSubmit | EventKind::ToolDone => {
                let session = self.session_mut(event);
                session.state = State::Working;
                session.note = None;
                self.trim();
                Effect::None
            }
            EventKind::Interrupt => {
                let session = self.session_mut(event);
                session.state = State::Idle;
                session.note = None;
                Effect::None
            }
            EventKind::Notification => {
                let kind = event.tool.as_deref();
                let message = event.message.clone();
                let waiting_for_input = kind == Some("idle_prompt")
                    || message
                        .as_deref()
                        .is_some_and(|m| m.to_lowercase().contains("waiting for your input"));
                let session = self.session_mut(event);
                if kind == Some("auth_success") {
                    return Effect::None;
                }
                if waiting_for_input {
                    // The turn is over and the agent just reminds the user: not a question.
                    if session.state == State::Working {
                        session.state = State::Idle;
                    }
                } else {
                    session.state = State::Waiting;
                    session.note = message;
                }
                Effect::None
            }
            EventKind::Stop => {
                let session = self.session_mut(event);
                session.state = State::Done;
                session.note = None;
                let (agent, project) = (session.agent, session.project());
                self.approvals
                    .retain(|a| !(a.agent == agent && a.session_id == event.session_id));
                Effect::Finished { agent, project }
            }
            EventKind::StopFailure => {
                let session = self.session_mut(event);
                session.state = State::Failed;
                session.note = None;
                let (agent, project) = (session.agent, session.project());
                Effect::Failed { agent, project }
            }
        }
    }

    /// Closes a question, whoever answered it. The session goes back to work unless it has another question.
    pub fn end_approval(&mut self, id: u64) -> Option<Approval> {
        let index = self.approvals.iter().position(|a| a.id == id)?;
        let approval = self.approvals.remove(index);
        let still_asking = self
            .approvals
            .iter()
            .any(|a| a.agent == approval.agent && a.session_id == approval.session_id);
        if !still_asking
            && let Some(session) = self
                .sessions
                .iter_mut()
                .find(|s| s.agent == approval.agent && s.id == approval.session_id)
            && session.state == State::Waiting
        {
            session.state = State::Working;
            session.note = None;
        }
        Some(approval)
    }

    pub fn approvals(&self) -> &[Approval] {
        &self.approvals
    }

    pub fn approval(&self, id: u64) -> Option<&Approval> {
        self.approvals.iter().find(|a| a.id == id)
    }

    /// Most important first, then most recent.
    pub fn sessions(&self) -> Vec<&Session> {
        let mut sessions: Vec<&Session> = self.sessions.iter().collect();
        sessions.sort_by_key(|s| (s.state, Reverse(s.updated_ms)));
        sessions
    }

    pub fn is_empty(&self) -> bool {
        self.sessions.is_empty() && self.approvals.is_empty()
    }

    /// Lets old sessions fade: quiet ones stop claiming to work, very old ones go away.
    pub fn sweep(&mut self, now_ms: i64) {
        for session in &mut self.sessions {
            if session.state == State::Working
                && now_ms - session.updated_ms > WORKING_GOES_QUIET_MS
            {
                session.state = State::Idle;
            }
        }
        let asking: Vec<(Agent, String)> = self
            .approvals
            .iter()
            .map(|a| (a.agent, a.session_id.clone()))
            .collect();
        self.sessions.retain(|s| {
            now_ms - s.updated_ms <= KEEP_MS
                || asking.iter().any(|(a, id)| *a == s.agent && *id == s.id)
        });
    }

    /// Never grows without bound: the oldest quiet sessions go first.
    fn trim(&mut self) {
        while self.sessions.len() > MAX_SESSIONS {
            let victim = self
                .sessions
                .iter()
                .enumerate()
                .filter(|(_, s)| s.state != State::Waiting)
                .min_by_key(|(_, s)| s.updated_ms)
                .map(|(i, _)| i);
            match victim {
                Some(i) => {
                    self.sessions.remove(i);
                }
                None => break,
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn event(kind: EventKind, session: &str, at_ms: i64) -> HookEvent {
        HookEvent {
            agent: Agent::Claude,
            kind,
            session_id: session.to_owned(),
            cwd: "/Users/me/projects/shop".to_owned(),
            tool: None,
            detail: None,
            message: None,
            questions: Vec::new(),
            at_ms,
        }
    }

    #[test]
    fn a_question_with_choices_waits_like_a_command_and_keeps_its_choices() {
        let mut t = Tracker::default();
        let effect = t.apply(&HookEvent {
            questions: vec![Question {
                header: "Pick".into(),
                question: "Which?".into(),
                options: vec!["A".into(), "B".into()],
                multi: false,
            }],
            detail: Some("Which?".into()),
            ..event(EventKind::Question, "q", 1)
        });
        let Effect::Approval(id) = effect else {
            panic!()
        };
        assert_eq!(t.approval(id).unwrap().questions[0].options, ["A", "B"]);
        assert_eq!(state_of(&t, "q"), State::Waiting);
    }

    fn request(session: &str, command: &str, at_ms: i64) -> HookEvent {
        HookEvent {
            tool: Some("Bash".into()),
            detail: Some(command.into()),
            ..event(EventKind::PermissionRequest, session, at_ms)
        }
    }

    fn state_of(tracker: &Tracker, session: &str) -> State {
        tracker
            .sessions()
            .iter()
            .find(|s| s.id == session)
            .map(|s| s.state)
            .expect("the session exists")
    }

    #[test]
    fn a_turn_goes_from_working_to_done_and_says_so_once() {
        let mut t = Tracker::default();
        assert_eq!(
            t.apply(&event(EventKind::SessionStart, "a", 1)),
            Effect::None
        );
        assert_eq!(state_of(&t, "a"), State::Idle);
        t.apply(&event(EventKind::PromptSubmit, "a", 2));
        assert_eq!(state_of(&t, "a"), State::Working);
        let effect = t.apply(&event(EventKind::Stop, "a", 3));
        assert_eq!(
            effect,
            Effect::Finished {
                agent: Agent::Claude,
                project: "shop".into()
            }
        );
        assert_eq!(state_of(&t, "a"), State::Done);
    }

    #[test]
    fn a_question_blocks_the_session_until_it_is_closed() {
        let mut t = Tracker::default();
        t.apply(&event(EventKind::PromptSubmit, "a", 1));
        let Effect::Approval(id) = t.apply(&request("a", "rm -rf build", 2)) else {
            panic!("a request opens a question");
        };
        assert_eq!(state_of(&t, "a"), State::Waiting);
        assert_eq!(t.approval(id).unwrap().detail, "rm -rf build");
        assert_eq!(t.approval(id).unwrap().project(), "shop");
        assert!(t.end_approval(id).is_some());
        assert_eq!(state_of(&t, "a"), State::Working);
        assert!(t.end_approval(id).is_none(), "a question closes once");
    }

    #[test]
    fn two_questions_keep_the_session_waiting_until_both_are_closed() {
        let mut t = Tracker::default();
        let Effect::Approval(first) = t.apply(&request("a", "ls", 1)) else {
            panic!()
        };
        let Effect::Approval(second) = t.apply(&request("a", "pwd", 2)) else {
            panic!()
        };
        t.end_approval(first);
        assert_eq!(state_of(&t, "a"), State::Waiting);
        t.end_approval(second);
        assert_eq!(state_of(&t, "a"), State::Working);
    }

    #[test]
    fn a_finished_tool_means_the_question_was_answered_elsewhere() {
        let mut t = Tracker::default();
        t.apply(&request("a", "ls", 1));
        t.apply(&event(EventKind::ToolDone, "a", 2));
        assert_eq!(state_of(&t, "a"), State::Working);
    }

    #[test]
    fn the_end_of_a_session_removes_it_and_its_questions() {
        let mut t = Tracker::default();
        t.apply(&request("a", "ls", 1));
        t.apply(&request("b", "ls", 2));
        t.apply(&event(EventKind::SessionEnd, "a", 3));
        assert_eq!(t.sessions().len(), 1);
        assert_eq!(t.approvals().len(), 1);
        assert_eq!(t.approvals()[0].session_id, "b");
    }

    #[test]
    fn a_stop_drops_questions_nobody_answered() {
        let mut t = Tracker::default();
        t.apply(&request("a", "ls", 1));
        t.apply(&event(EventKind::Stop, "a", 2));
        assert!(t.approvals().is_empty());
    }

    #[test]
    fn the_same_session_id_of_another_agent_is_another_session() {
        let mut t = Tracker::default();
        t.apply(&event(EventKind::PromptSubmit, "same", 1));
        t.apply(&HookEvent {
            agent: Agent::Codex,
            ..event(EventKind::Stop, "same", 2)
        });
        assert_eq!(t.sessions().len(), 2);
    }

    #[test]
    fn notifications_distinguish_a_question_from_a_reminder() {
        let mut t = Tracker::default();
        t.apply(&event(EventKind::PromptSubmit, "a", 1));
        t.apply(&HookEvent {
            tool: Some("permission_prompt".into()),
            message: Some("Claude needs your permission to use Bash".into()),
            ..event(EventKind::Notification, "a", 2)
        });
        assert_eq!(state_of(&t, "a"), State::Waiting);
        t.apply(&event(EventKind::Stop, "a", 3));
        t.apply(&HookEvent {
            tool: Some("idle_prompt".into()),
            message: Some("Claude is waiting for your input".into()),
            ..event(EventKind::Notification, "a", 4)
        });
        assert_eq!(
            state_of(&t, "a"),
            State::Done,
            "a reminder does not reopen a finished turn"
        );
        t.apply(&HookEvent {
            tool: Some("auth_success".into()),
            ..event(EventKind::Notification, "a", 5)
        });
        assert_eq!(state_of(&t, "a"), State::Done);
    }

    #[test]
    fn sessions_list_the_ones_that_need_you_first_then_the_most_recent() {
        let mut t = Tracker::default();
        t.apply(&event(EventKind::Stop, "done-old", 1));
        t.apply(&event(EventKind::Stop, "done-new", 5));
        t.apply(&event(EventKind::PromptSubmit, "busy", 3));
        t.apply(&request("asking", "ls", 2));
        let order: Vec<&str> = t.sessions().iter().map(|s| s.id.as_str()).collect();
        assert_eq!(order, ["asking", "busy", "done-new", "done-old"]);
    }

    #[test]
    fn quiet_sessions_stop_claiming_to_work_and_old_ones_disappear() {
        let mut t = Tracker::default();
        t.apply(&event(EventKind::PromptSubmit, "busy", 0));
        t.apply(&event(EventKind::Stop, "done", 0));
        t.sweep(WORKING_GOES_QUIET_MS + 1);
        assert_eq!(state_of(&t, "busy"), State::Idle);
        assert_eq!(t.sessions().len(), 2);
        t.sweep(KEEP_MS + 1);
        assert!(t.is_empty());
    }

    #[test]
    fn a_session_with_an_open_question_is_never_swept_away() {
        let mut t = Tracker::default();
        t.apply(&request("a", "ls", 0));
        t.sweep(KEEP_MS * 3);
        assert_eq!(t.sessions().len(), 1);
    }

    #[test]
    fn the_list_is_bounded() {
        let mut t = Tracker::default();
        for n in 0..(MAX_SESSIONS as i64 + 10) {
            t.apply(&event(EventKind::SessionStart, &format!("s{n}"), n));
        }
        assert_eq!(t.sessions().len(), MAX_SESSIONS);
        assert!(
            t.sessions().iter().all(|s| s.started_ms >= 10),
            "the oldest went first"
        );
    }

    #[test]
    fn project_names_come_from_the_folder() {
        assert_eq!(project_name("/Users/me/projects/shop"), "shop");
        assert_eq!(project_name("/Users/me/projects/shop/"), "shop");
        assert_eq!(project_name("/"), "/");
        assert_eq!(project_name(""), "?");
    }
}
