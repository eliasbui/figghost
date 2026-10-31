//! Jarvis: an assistant that answers questions in a chat on Liquid Glass. A shortcut opens the panel, the person
//! types a question, and the answer is written into the panel as the service sends it. Off until the person turns
//! it on, and a beta.
//!
//! The question goes to the service the person chose, with the key they pasted, which is kept in the Keychain and
//! never in the config file; turning the feature on is the consent to send what is typed there. The talk is kept in
//! memory while the panel is open and forgotten when it closes. Nothing is kept on disk.

use super::{
    AttachEnv,
    dictation::{self, Spoken},
};
use crate::{
    config::parse_hotkey,
    i18n::{self, Language},
    settings::model::Fault,
    trace,
};
use core_jarvis::{Cancel, Failure, Kind, Message, Source, codex};
use macos_sys::{
    MainThreadMarker, app,
    hotkey::Hotkey,
    jarvis_ui::{JarvisDisplay, JarvisPanel, Line, Stage},
    keychain,
    panel::Key,
    permissions,
    speech::{self, Heard, Listening},
    voice_out::{self, Speaker},
    workspace,
};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::{
    cell::{Cell, RefCell},
    collections::VecDeque,
    time::Instant,
    rc::{Rc, Weak},
    sync::{
        Arc, Mutex,
        atomic::Ordering,
    },
};

pub mod settings;
mod text;

/// The Keychain service of the keys; the account is the provider. Tests use a service of their own.
const SERVICE: &str = if cfg!(test) { "vn.figghost.jarvis.test" } else { "vn.figghost.jarvis" };
/// The longest a whole answer may take.
const ANSWER_SECONDS: u64 = 90;
/// How often the answer that arrived on another thread is taken up.
const LOOK_SECONDS: f64 = 0.05;
/// A silence after words after which they are sent, and how long the microphone waits for words at all.
const SILENCE_SECONDS: f64 = 2.0;
const QUIET_SECONDS: f64 = 10.0;
/// How often the microphone's silence and the voice that is speaking are looked at.
const TICK_SECONDS: f64 = 0.15;
/// The turns sent with a question; older ones are forgotten so that a long talk never grows the request.
const TURNS: usize = 20;
const SYSTEM: &str = "You are Jarvis, a helpful assistant on the user's Mac. Answer briefly and plainly, in the \
language the person writes in. Reply in plain text: no markdown, no headings, no tables.";

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Provider {
    #[default]
    Anthropic,
    /// The ChatGPT plan, through the sign-in of the Codex command line.
    Codex,
    #[serde(rename = "openai")]
    OpenAi,
    Gemini,
    #[serde(rename = "deepseek")]
    DeepSeek,
    /// A model on this machine, served by Ollama.
    Local,
}

impl Provider {
    pub const ALL: [Provider; 6] = [
        Provider::Anthropic,
        Provider::Codex,
        Provider::OpenAi,
        Provider::Gemini,
        Provider::DeepSeek,
        Provider::Local,
    ];

    pub fn id(self) -> &'static str {
        match self {
            Provider::Anthropic => "anthropic",
            Provider::Codex => "codex",
            Provider::OpenAi => "openai",
            Provider::Gemini => "gemini",
            Provider::DeepSeek => "deepseek",
            Provider::Local => "local",
        }
    }

    pub fn from_id(id: &str) -> Option<Self> {
        Self::ALL.into_iter()
            .find(|p| p.id() == id)
    }

    fn name(self) -> &'static str {
        match self {
            Provider::Anthropic => "Anthropic",
            Provider::Codex => "Codex",
            Provider::OpenAi => "OpenAI",
            Provider::Gemini => "Gemini",
            Provider::DeepSeek => "DeepSeek",
            Provider::Local => "Ollama",
        }
    }

    fn kind(self) -> Kind {
        match self {
            Provider::Anthropic => Kind::Anthropic,
            Provider::Codex => Kind::Codex,
            _ => Kind::OpenAi,
        }
    }

    fn url(self) -> &'static str {
        match self {
            Provider::Anthropic => "https://api.anthropic.com/v1/messages",
            Provider::Codex => "https://chatgpt.com/backend-api/codex/responses",
            Provider::OpenAi => "https://api.openai.com/v1/chat/completions",
            Provider::Gemini => "https://generativelanguage.googleapis.com/v1beta/openai/chat/completions",
            Provider::DeepSeek => "https://api.deepseek.com/chat/completions",
            Provider::Local => "http://127.0.0.1:11434/v1/chat/completions",
        }
    }

    /// The model asked when the person has not chosen one; the names move on, which is why the page lets them type
    /// their own.
    pub fn default_model(self) -> &'static str {
        match self {
            Provider::Anthropic => "claude-haiku-5-5",
            // What Codex lists first is read from its own cache when there is one (see `Config::source`).
            Provider::Codex => "gpt-5",
            Provider::OpenAi => "gpt-4o-mini",
            Provider::Gemini => "gemini-2.0-flash",
            Provider::DeepSeek => "deepseek-chat",
            Provider::Local => "llama3.2",
        }
    }

    /// Whether a key has to be pasted: Codex uses its own sign-in and a local model needs nothing.
    pub fn needs_key(self) -> bool {
        !matches!(self, Provider::Local | Provider::Codex)
    }
}

/// A model name as the services write them: letters, digits and . _ - : /
pub fn valid_model(model: &str) -> bool {
    model.len() <= 80
        && model
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-' | ':' | '/'))
}

/// Where the sign-in of Codex is: its own folder, in `CODEX_HOME` or the home folder.
fn codex_home() -> Option<(std::path::PathBuf, std::path::PathBuf)> {
    let home = std::env::var_os("HOME").map(std::path::PathBuf::from)?;
    let folder = std::env::var_os("CODEX_HOME").map_or_else(|| home.join(".codex"), std::path::PathBuf::from);
    Some((folder.join("auth.json"), folder.join("models_cache.json")))
}

fn unix_now() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| i64::try_from(d.as_secs()).unwrap_or(0))
}

/// The state of the Codex sign-in for the page: `ready`, `missing`, `expired` or `unreadable`.
pub fn codex_status() -> &'static str {
    let Some((auth, _)) = codex_home() else { return "missing" };
    match codex::read(&auth, unix_now()) {
        Ok(_) => "ready",
        Err(codex::SignIn::Missing) => "missing",
        Err(codex::SignIn::Expired) => "expired",
        Err(codex::SignIn::Unreadable) => "unreadable",
    }
}

pub fn has_key(provider: Provider) -> bool {
    matches!(keychain::read(SERVICE, provider.id()), Ok(Some(key)) if !key.is_empty())
}

fn key_of(provider: Provider) -> String {
    match keychain::read(SERVICE, provider.id()) {
        Ok(Some(key)) => String::from_utf8(key).unwrap_or_default(),
        _ => String::new(),
    }
}

pub fn store_key(provider: Provider, key: &str) -> Result<(), i32> {
    keychain::write(SERVICE, provider.id(), key.as_bytes())
}

pub fn forget_key(provider: Provider) -> Result<(), i32> {
    keychain::delete(SERVICE, provider.id())
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    pub enabled: bool,
    /// Opens the panel, and closes it when it is pressed again.
    pub shortcut: String,
    pub provider: Provider,
    /// Empty is the provider's own.
    pub model: String,
    /// The panel listens when it opens and after each answer, and sends what was said after a silence.
    pub voice: bool,
    pub language: Spoken,
    /// The audio never leaves the Mac; a language this Mac cannot recognize by itself is then refused.
    pub on_device: bool,
    /// The answer is read aloud, sentence by sentence.
    pub speak: bool,
    /// A soft sound when Jarvis wakes, answers and fails.
    pub sound: bool,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            enabled: false,
            shortcut: "ctrl+opt+space".into(),
            provider: Provider::Anthropic,
            model: String::new(),
            voice: true,
            language: Spoken::Auto,
            on_device: true,
            speak: true,
            sound: true,
        }
    }
}

impl Config {
    /// A config file can say anything; whatever is not offered is read as the default.
    pub fn sanitized(mut self) -> Self {
        self.model = self.model.trim().to_owned();
        if !valid_model(&self.model) {
            self.model.clear();
        }
        if !self.shortcut.is_empty() && parse_hotkey(&self.shortcut).is_err() {
            self.shortcut = Self::default().shortcut;
        }
        self
    }

    /// Who to ask and how, or what is missing before it can be asked.
    fn source(&self) -> Result<Source, Block> {
        let provider = self.provider;
        // For the checks, which serve a made-up service on a loopback port; a key is never sent anywhere else.
        let url = std::env::var("FIGGHOST_JARVIS_URL")
            .ok()
            .filter(|url| url.starts_with("http://127.0.0.1"))
            .unwrap_or_else(|| provider.url().to_owned());
        let (key, account, default) = if provider == Provider::Codex {
            let (auth, cache) = codex_home().ok_or(Block::SignIn(codex::SignIn::Missing))?;
            let sign = codex::read(&auth, unix_now()).map_err(Block::SignIn)?;
            let model = codex::default_model(&cache).unwrap_or_else(|| provider.default_model().to_owned());
            (sign.access_token, sign.account_id, model)
        } else {
            let key = key_of(provider);
            if provider.needs_key() && key.is_empty() {
                return Err(Block::NoKey);
            }
            (key, String::new(), provider.default_model().to_owned())
        };
        Ok(Source {
            kind: provider.kind(),
            url,
            key,
            account,
            model: if self.model.is_empty() { default } else { self.model.clone() },
        })
    }
}

/// The sentences of `buffer` that are complete, taken out of it; with `flush` the rest too. A sentence ends at a line
/// break, or at . ! ? followed by a space, so "3.5" in the middle of a line stays whole.
pub fn sentences(buffer: &mut String, flush: bool) -> Vec<String> {
    let mut out = Vec::new();
    loop {
        let chars: Vec<(usize, char)> = buffer.char_indices().collect();
        let cut = chars.iter().enumerate().find_map(|(i, (at, c))| {
            let after = chars.get(i + 1).map(|(_, n)| *n);
            // A line break always ends one. A mark ends one when a space follows it, and the last character of the
            // buffer waits for the next, since "3.5" is not two sentences.
            let ends = *c == '\n'
                || (matches!(c, '.' | '!' | '?' | '…' | '。') && after.is_some_and(char::is_whitespace));
            ends.then(|| at + c.len_utf8())
        });
        match cut {
            Some(at) => {
                let sentence = buffer[..at].trim().to_owned();
                buffer.replace_range(..at, "");
                if !sentence.is_empty() {
                    out.push(sentence);
                }
            }
            None => break,
        }
    }
    if flush {
        let rest = buffer.trim().to_owned();
        buffer.clear();
        if !rest.is_empty() {
            out.push(rest);
        }
    }
    out
}

/// What is missing before a question can be asked.
#[derive(Debug, PartialEq, Eq)]
enum Block {
    NoKey,
    SignIn(codex::SignIn),
}

/// What the thread that asks hands to the panel.
enum Update {
    Text(String),
    Done(Result<String, Failure>),
}

type Inbox = Arc<Mutex<VecDeque<Update>>>;

/// A question on its way.
struct Flight {
    cancel: Cancel,
    inbox: Inbox,
    _timer: app::Timer,
    /// Whether any words of the answer have arrived.
    started: bool,
}

/// The microphone while the person speaks.
struct Hearing {
    listening: Listening,
    words: String,
    started: Instant,
    changed: Instant,
}

pub struct Feature {
    mtm: MainThreadMarker,
    me: Weak<Feature>,
    config: RefCell<Config>,
    hotkey: RefCell<Option<Hotkey>>,
    /// Made the first time the panel opens.
    panel: RefCell<Option<Rc<JarvisPanel>>>,
    /// What the panel says, and what has been said to the service.
    lines: RefCell<Vec<Line>>,
    turns: RefCell<Vec<Message>>,
    stage: Cell<Stage>,
    status: RefCell<String>,
    flight: RefCell<Option<Flight>>,
    hearing: RefCell<Option<Hearing>>,
    speaker: Speaker,
    /// The part of the answer that has arrived and is not yet given to the voice.
    unspoken: RefCell<String>,
    /// While the panel is open: looks at the silence of the microphone and at the end of the voice.
    ticker: RefCell<Option<app::Timer>>,
    /// The voice is asking what it can do; the microphone opens when it has.
    greeting: Cell<bool>,
}

pub fn attach(env: &AttachEnv) -> Option<Rc<Feature>> {
    let config = env.config.jarvis.clone().sanitized();
    let feature = Rc::new_cyclic(|me| Feature {
        mtm: env.mtm,
        me: me.clone(),
        config: RefCell::new(config.clone()),
        hotkey: RefCell::default(),
        panel: RefCell::default(),
        lines: RefCell::default(),
        turns: RefCell::default(),
        stage: Cell::new(Stage::Idle),
        status: RefCell::default(),
        flight: RefCell::default(),
        hearing: RefCell::default(),
        speaker: Speaker::new(),
        unspoken: RefCell::default(),
        ticker: RefCell::default(),
        greeting: Cell::new(false),
    });
    feature.register_hotkey(&config);
    // For the checks, which cannot press a global shortcut or type: the same as pressing it, and then typing the
    // question of FIGGHOST_JARVIS_ASK, a moment after the start.
    if config.enabled && std::env::var_os("FIGGHOST_JARVIS_START").is_some() {
        let weak = Rc::downgrade(&feature);
        app::after(1.0, move || {
            if let Some(f) = weak.upgrade() {
                f.toggle();
            }
        });
        if let Ok(question) = std::env::var("FIGGHOST_JARVIS_ASK") {
            let weak = Rc::downgrade(&feature);
            app::after(2.0, move || {
                if let Some(f) = weak.upgrade() {
                    if let Some(panel) = f.panel.borrow().as_ref() {
                        panel.set_typed(&question);
                    }
                    f.submit();
                }
            });
        }
    }
    Some(feature)
}

impl Feature {
    pub fn configure(&self, config: &Config) -> Result<(), Fault> {
        let config = config.clone().sanitized();
        *self.config.borrow_mut() = config.clone();
        self.hotkey.borrow_mut().take();
        if !config.enabled {
            self.close();
        }
        self.register_hotkey(&config);
        Ok(())
    }

    fn register_hotkey(&self, config: &Config) {
        if !config.enabled || config.shortcut.is_empty() {
            return;
        }
        let Ok(spec) = parse_hotkey(&config.shortcut) else {
            return;
        };
        let weak = self.me.clone();
        match Hotkey::register(self.mtm, spec.key_code, spec.modifiers, move || {
            if let Some(f) = weak.upgrade() {
                f.toggle();
            }
        }) {
            Ok(hotkey) => *self.hotkey.borrow_mut() = Some(hotkey),
            Err(status) => eprintln!("jarvis: cannot register its shortcut (status {status})"),
        }
    }

    pub fn suspend_hotkeys(&self) {
        self.hotkey.borrow_mut().take();
    }

    pub fn resume_hotkeys(&self, config: &Config) {
        self.register_hotkey(&config.clone().sanitized());
    }

    /// Facts only the running app knows, for the settings page: whether it is open, and the access it has.
    pub fn runtime(&self) -> Value {
        json!({
            "open": self.panel.borrow().as_ref().is_some_and(|p| p.is_visible()),
            "speech": match speech::speech_access() {
                Some(true) => "allowed",
                Some(false) => "denied",
                None => "ask",
            },
            "microphone": permissions::microphone_trusted(),
        })
    }

    /// The buttons of the settings page: ask for the access, or open where it is given.
    pub fn action(&self, name: &str) -> Option<Result<(), Fault>> {
        match name {
            "jarvis_allow" => self.ask_access(),
            "jarvis_open_speech" => permissions::open_pane(dictation::SPEECH_PANE),
            "jarvis_open_microphone" => permissions::open_pane(permissions::MICROPHONE_PANE),
            _ => return None,
        }
        Some(Ok(()))
    }

    /// Asks for what is not decided yet: Speech Recognition, then the microphone. A program that is not in its
    /// bundle has no words for the dialogs and is not asked.
    fn ask_access(&self) {
        if speech::speech_access().is_none() {
            speech::speech_request(|_| {});
        }
        if !permissions::microphone_trusted() && speech::explains("NSMicrophoneUsageDescription") {
            permissions::microphone_request();
        }
    }

    fn language(&self) -> Language {
        i18n::current()
    }

    fn panel(&self) -> Rc<JarvisPanel> {
        let mut slot = self.panel.borrow_mut();
        let panel = slot.get_or_insert_with(|| {
            let weak = self.me.clone();
            let panel = JarvisPanel::new(self.mtm, {
                let weak = weak.clone();
                move |key| weak.upgrade().is_some_and(|f| f.key(key))
            });
            panel.set_on_mic(move || {
                if let Some(f) = weak.upgrade() {
                    f.mic();
                }
            });
            panel
        });
        Rc::clone(panel)
    }

    fn display(&self) -> JarvisDisplay {
        let language = self.language();
        let stage = self.stage.get();
        JarvisDisplay {
            stage,
            symbol: match stage {
                Stage::Error => "exclamationmark.triangle.fill",
                _ => "sparkles",
            },
            name: "Jarvis".into(),
            beta: language.jarvis_beta().into(),
            status: self.status.borrow().clone(),
            lines: self.lines.borrow().clone(),
            placeholder: language.jarvis_placeholder().into(),
            greeting: language.jarvis_greeting().into(),
        }
    }

    fn paint(&self) {
        if let Some(panel) = self.panel.borrow().as_ref() {
            panel.set(&self.display());
        }
    }

    fn set_stage(&self, stage: Stage, status: &str) {
        self.stage.set(stage);
        *self.status.borrow_mut() = status.to_owned();
    }

    /// Back to waiting, to be spoken to or typed to.
    fn rest(&self) {
        self.set_stage(Stage::Idle, self.language().jarvis_status_ready());
    }

    fn sound(&self, name: &str) {
        if self.config.borrow().sound {
            workspace::play_sound(name);
        }
    }

    fn is_open(&self) -> bool {
        self.panel.borrow().as_ref().is_some_and(|p| p.is_visible())
    }

    /// The shortcut: the panel opens, or, open, it closes.
    pub fn toggle(&self) {
        if self.is_open() {
            self.close();
        } else {
            self.open();
        }
    }

    fn open(&self) {
        if !self.config.borrow().enabled {
            return;
        }
        let language = self.language();
        self.rest();
        self.sound("Pop");
        trace::log("jarvis: the panel opens");
        self.panel().show(&self.display());
        let weak = self.me.clone();
        *self.ticker.borrow_mut() = Some(app::every(TICK_SECONDS, move || {
            if let Some(f) = weak.upgrade() {
                f.tick();
            }
        }));
        // Jarvis asks what it can do, out loud, and then listens; with no voice it only listens.
        let config = self.config.borrow().clone();
        if config.speak && config.voice {
            self.greeting.set(true);
            self.say(language.jarvis_ask(), &language);
        } else if config.voice {
            self.listen();
        }
    }

    /// The panel goes and the talk with it.
    fn close(&self) {
        self.stop();
        self.hear_stop(true);
        self.speaker.stop();
        self.greeting.set(false);
        self.ticker.borrow_mut().take();
        self.unspoken.borrow_mut().clear();
        self.lines.borrow_mut().clear();
        self.turns.borrow_mut().clear();
        if let Some(panel) = self.panel.borrow().as_ref() {
            panel.hide();
        }
    }

    /// Queues `text` for the voice, in the language it is written in.
    fn say(&self, text: &str, _language: &Language) {
        self.speaker.say(text, voice_out::language_of(text));
    }

    /// Cuts the answer short. True when there was one.
    fn stop(&self) -> bool {
        let Some(flight) = self.flight.borrow_mut().take() else {
            return false;
        };
        flight.cancel.store(true, Ordering::SeqCst);
        true
    }

    fn key(&self, key: Key) -> bool {
        match key {
            Key::Enter => {
                self.submit();
                true
            }
            Key::Escape => {
                // The first Escape stops what is going on, the answer, the voice or the microphone; the next one
                // closes the panel.
                let busy = self.stop() | self.speaker.is_speaking() | self.hearing.borrow().is_some();
                if busy {
                    self.speaker.stop();
                    self.greeting.set(false);
                    self.unspoken.borrow_mut().clear();
                    self.hear_stop(true);
                    self.rest();
                    self.paint();
                } else {
                    self.close();
                }
                true
            }
            _ => false,
        }
    }

    /// The button beside the line: starts listening, or sends what has been said so far.
    fn mic(&self) {
        if self.hearing.borrow().is_some() {
            self.hear_finish();
        } else {
            self.speaker.stop();
            self.greeting.set(false);
            self.stop();
            self.listen();
        }
    }

    /// Whether the person has allowed what listening needs; asks for what is not decided yet.
    fn may_listen(&self) -> bool {
        match speech::speech_access() {
            Some(true) if permissions::microphone_trusted() => true,
            Some(false) => self.note(self.language().jarvis_no_speech_access()),
            None => {
                self.ask_access();
                self.note(self.language().jarvis_asking())
            }
            Some(true) => {
                self.ask_access();
                self.note(self.language().jarvis_no_microphone())
            }
        }
    }

    /// A line of Jarvis's own in the talk, for what must be told; always false.
    fn note(&self, text: &str) -> bool {
        self.lines.borrow_mut().push(Line { mine: false, text: text.to_owned() });
        self.rest();
        self.paint();
        false
    }

    /// Opens the microphone, if the panel is open and the access is there.
    fn listen(&self) {
        if !self.is_open() || self.hearing.borrow().is_some() || !self.may_listen() {
            return;
        }
        let language = self.language();
        let config = self.config.borrow().clone();
        let spoken = config.language.code(language);
        let weak = self.me.clone();
        let started = Listening::start(dictation::locale(spoken), config.on_device, move |heard| {
            if let Some(f) = weak.upgrade() {
                f.heard(heard);
            }
        });
        match started {
            Ok(listening) => {
                let now = Instant::now();
                *self.hearing.borrow_mut() = Some(Hearing { listening, words: String::new(), started: now, changed: now });
                self.set_stage(Stage::Listening, language.jarvis_status_listening());
                self.paint();
            }
            Err(reason) => {
                eprintln!("jarvis: cannot listen: {reason}");
                let says = if config.on_device && !speech::model(dictation::locale(spoken)).on_device {
                    language.jarvis_not_on_device()
                } else {
                    language.jarvis_cannot_listen()
                };
                self.note(says);
            }
        }
    }

    fn heard(&self, heard: Heard) {
        match heard {
            Heard::Level(level) => {
                if let Some(panel) = self.panel.borrow().as_ref() {
                    panel.level(level);
                }
            }
            Heard::Words { text, last } => {
                let finishing = {
                    let mut slot = self.hearing.borrow_mut();
                    let Some(hearing) = slot.as_mut() else { return };
                    if hearing.words != text {
                        hearing.words = text.clone();
                        hearing.changed = Instant::now();
                    }
                    last
                };
                if let Some(panel) = self.panel.borrow().as_ref() {
                    panel.set_typed(&text);
                }
                if finishing {
                    self.hear_stop(false);
                    self.submit();
                }
            }
            Heard::Failed(reason) => {
                eprintln!("jarvis: the recognizer stopped: {reason}");
                let any = self.hearing.borrow().as_ref().is_some_and(|h| !h.words.is_empty());
                self.hear_stop(false);
                if any {
                    self.submit();
                } else {
                    self.rest();
                    self.paint();
                }
            }
        }
    }

    /// The person is done: the microphone stops and the last words come in, then they are sent.
    fn hear_finish(&self) {
        if let Some(hearing) = self.hearing.borrow().as_ref() {
            hearing.listening.finish();
        }
    }

    /// The microphone is let go. With `drop_words` what was heard is thrown away too.
    fn hear_stop(&self, drop_words: bool) {
        let Some(hearing) = self.hearing.borrow_mut().take() else {
            return;
        };
        hearing.listening.cancel();
        if drop_words && let Some(panel) = self.panel.borrow().as_ref() {
            panel.clear();
        }
    }

    /// While the panel is open: a silence after words sends them, a long one with none gives up, and when the voice
    /// has said everything the microphone opens again.
    fn tick(&self) {
        let (quiet, any, long) = match self.hearing.borrow().as_ref() {
            Some(h) => (h.changed.elapsed().as_secs_f64(), !h.words.is_empty(), h.started.elapsed().as_secs_f64()),
            None => (0.0, false, 0.0),
        };
        if self.hearing.borrow().is_some() {
            if any && quiet >= SILENCE_SECONDS {
                self.hear_finish();
            } else if !any && long >= QUIET_SECONDS {
                self.hear_stop(true);
                self.rest();
                self.paint();
            }
            return;
        }
        // Nothing is being heard: once the answer is written and the voice is done, listen for the next question.
        let config = self.config.borrow().clone();
        if self.flight.borrow().is_none()
            && !self.speaker.is_speaking()
            && self.unspoken.borrow().is_empty()
            && matches!(self.stage.get(), Stage::Speaking)
        {
            self.rest();
            self.paint();
            if config.voice {
                self.listen();
            }
        } else if self.greeting.get() && !self.speaker.is_speaking() {
            // The spoken greeting is over: listen.
            self.greeting.set(false);
            if config.voice {
                self.listen();
            }
        }
    }

    fn submit(&self) {
        let Some(panel) = self.panel.borrow().clone() else {
            return;
        };
        let question = panel.typed();
        if question.is_empty() || self.flight.borrow().is_some() {
            return;
        }
        self.hear_stop(false);
        self.speaker.stop();
        self.greeting.set(false);
        panel.clear();
        let language = self.language();
        let config = self.config.borrow().clone();
        self.lines.borrow_mut().push(Line { mine: true, text: question.clone() });
        let source = match config.source() {
            Ok(source) => source,
            Err(block) => {
                let says = match block {
                    Block::NoKey => language.jarvis_no_key(config.provider.name()),
                    Block::SignIn(why) => language.jarvis_codex(why).to_owned(),
                };
                self.lines.borrow_mut().push(Line { mine: false, text: says });
                self.set_stage(Stage::Error, "");
                self.sound("Basso");
                return self.paint();
            }
        };
        {
            let mut turns = self.turns.borrow_mut();
            turns.push(Message::user(question));
            let extra = turns.len().saturating_sub(TURNS);
            turns.drain(..extra);
        }
        self.lines
            .borrow_mut()
            .push(Line { mine: false, text: language.jarvis_thinking_line().into() });
        self.unspoken.borrow_mut().clear();
        self.set_stage(Stage::Thinking, language.jarvis_status_thinking());
        self.paint();

        trace::log(&format!("jarvis: asking {} ({})", config.provider.id(), source.model));
        let cancel = Cancel::default();
        let inbox: Inbox = Arc::default();
        let turns = self.turns.borrow().clone();
        {
            let (cancel, inbox) = (cancel.clone(), inbox.clone());
            std::thread::spawn(move || {
                let pieces = inbox.clone();
                let result = core_jarvis::stream(&source, SYSTEM, &turns, ANSWER_SECONDS, &cancel, &mut |piece| {
                    pieces.lock().unwrap().push_back(Update::Text(piece.to_owned()));
                });
                inbox.lock().unwrap().push_back(Update::Done(result));
            });
        }
        let weak = self.me.clone();
        let timer = app::every(LOOK_SECONDS, move || {
            if let Some(f) = weak.upgrade() {
                f.take_up();
            }
        });
        *self.flight.borrow_mut() = Some(Flight { cancel, inbox, _timer: timer, started: false });
    }

    /// Gives the voice the sentences of the answer that are complete.
    fn give_to_voice(&self, flush: bool) {
        if !self.config.borrow().speak {
            self.unspoken.borrow_mut().clear();
            return;
        }
        let ready = sentences(&mut self.unspoken.borrow_mut(), flush);
        for sentence in ready {
            self.say(&sentence, &self.language());
        }
    }

    /// Takes what arrived from the thread that asks into the panel.
    fn take_up(&self) {
        let updates: Vec<Update> = match self.flight.borrow().as_ref() {
            Some(flight) => flight.inbox.lock().unwrap().drain(..).collect(),
            None => return,
        };
        let language = self.language();
        for update in updates {
            match update {
                Update::Text(piece) => {
                    let first = {
                        let mut slot = self.flight.borrow_mut();
                        let Some(flight) = slot.as_mut() else { return };
                        !std::mem::replace(&mut flight.started, true)
                    };
                    if let Some(last) = self.lines.borrow_mut().last_mut() {
                        if first {
                            last.text.clear();
                        }
                        last.text.push_str(&piece);
                    }
                    self.unspoken.borrow_mut().push_str(&piece);
                    if first {
                        self.set_stage(Stage::Speaking, language.jarvis_status_answering());
                    }
                    self.give_to_voice(false);
                }
                Update::Done(result) => {
                    self.flight.borrow_mut().take();
                    match result {
                        Ok(answer) => {
                            trace::log(&format!("jarvis: answered with {} characters", answer.chars().count()));
                            self.turns.borrow_mut().push(Message::assistant(answer));
                            self.give_to_voice(true);
                            self.sound("Tink");
                            // Still Speaking while the voice goes on; `tick` rests and listens when it ends.
                        }
                        Err(failure) => {
                            trace::log(&format!("jarvis: failed with {failure:?}"));
                            self.turns.borrow_mut().pop();
                            self.unspoken.borrow_mut().clear();
                            if let Some(last) = self.lines.borrow_mut().last_mut() {
                                last.text = language.jarvis_failure(&failure);
                            }
                            if failure == Failure::Cancelled {
                                self.rest();
                            } else {
                                self.set_stage(Stage::Error, "");
                                self.sound("Basso");
                            }
                        }
                    }
                }
            }
        }
        self.paint();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn it_is_off_by_default_and_a_config_file_can_say_anything() {
        assert!(!Config::default().enabled);
        let bad = Config { shortcut: "nonsense".into(), model: "a b;c".into(), ..Config::default() }.sanitized();
        assert_eq!(bad.shortcut, "ctrl+opt+space");
        assert_eq!(bad.model, "");
        let none = Config { shortcut: String::new(), ..Config::default() }.sanitized();
        assert!(none.shortcut.is_empty(), "no shortcut is a choice");
    }

    #[test]
    fn a_sentence_is_given_to_the_voice_as_soon_as_it_is_complete() {
        let mut buffer = String::from("Xin chào bạn. Hôm nay trời");
        assert_eq!(sentences(&mut buffer, false), ["Xin chào bạn."]);
        assert_eq!(buffer, " Hôm nay trời");
        buffer.push_str(" đẹp! Bạn khỏe không?");
        assert_eq!(sentences(&mut buffer, false), ["Hôm nay trời đẹp!"], "the last mark waits for what follows it");
        assert_eq!(sentences(&mut buffer, true), ["Bạn khỏe không?"], "the end of the answer gives up the rest");
        assert!(buffer.is_empty());
    }

    #[test]
    fn a_number_and_a_line_break_do_not_split_a_sentence_wrongly() {
        let mut buffer = String::from("It costs 3.5 dollars. Next\nline two. ");
        assert_eq!(sentences(&mut buffer, false), ["It costs 3.5 dollars.", "Next", "line two."]);
        assert!(sentences(&mut String::new(), true).is_empty());
        let mut plain = String::from("no end yet");
        assert!(sentences(&mut plain, false).is_empty());
        assert_eq!(plain, "no end yet");
    }

    #[test]
    fn a_provider_is_known_by_its_id_and_only_a_local_model_needs_no_key() {
        for p in Provider::ALL {
            assert_eq!(Provider::from_id(p.id()), Some(p));
            assert_eq!(p.needs_key(), !matches!(p, Provider::Local | Provider::Codex), "Codex has its own sign-in");
            assert!(p.url().starts_with("https://") || p.url().starts_with("http://127.0.0.1"));
            assert!(valid_model(p.default_model()));
        }
        assert_eq!(Provider::from_id("bing"), None);
        let json = serde_json::to_string(&Config { provider: Provider::OpenAi, ..Config::default() }).unwrap();
        assert!(json.contains("\"openai\""));
    }

    #[test]
    fn the_source_asks_the_chosen_model_or_the_providers_own() {
        let mut config = Config { provider: Provider::Local, ..Config::default() };
        let s = config.source().unwrap();
        assert_eq!(s.model, "llama3.2");
        assert_eq!(s.kind, Kind::OpenAi);
        assert!(s.key.is_empty());
        config.model = "qwen2.5:7b".into();
        assert_eq!(config.source().unwrap().model, "qwen2.5:7b");
        // A service that needs a key is not asked without one (the Keychain of the tests holds none for it).
        assert_eq!(Config::default().source().err(), Some(Block::NoKey));
        assert_eq!(Provider::Anthropic.kind(), Kind::Anthropic);
        assert_eq!(Provider::Codex.kind(), Kind::Codex);
    }
}
