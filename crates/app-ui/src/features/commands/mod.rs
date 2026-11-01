//! The Commands tab of the notch: a few commands the person saved, each run by a press, with what it prints scrolling in
//! the tab. It is a runner and not a terminal: a command gets no keyboard and no screen to draw on, so a program that
//! asks questions or draws on the whole screen is not for it. One runs at a time; Stop ends it and what it started, and
//! one that runs longer than ten minutes is ended. The line is given to the user's shell (`zsh -lc`) with the home folder
//! as the place it runs in, exactly as typed in the settings, which are the person's own: the same trust as the
//! configuration file of the app. Nothing is kept of what a command prints. It is off until the person turns it on.

use super::AttachEnv;
use crate::{i18n, settings::model::Fault, trace};
use logic::{Cleaner, Item};
use macos_sys::{
    app,
    commands_ui::{CommandRow, CommandsCallbacks, CommandsDisplay, CommandsView, PAGE_SIZE},
    process::kill_group,
};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::{
    cell::{Cell, RefCell},
    io::Read,
    os::unix::process::CommandExt,
    path::PathBuf,
    process::{Command, Stdio},
    rc::{Rc, Weak},
    sync::{Arc, Mutex, PoisonError},
    thread,
    time::{Duration, Instant},
};

pub mod logic;
pub mod settings;
mod text;

/// The longest a command may run.
const MAX_SECONDS: u64 = 600;
/// How often what a command printed is brought to the tab, in seconds.
const LOOK_SECONDS: f64 = 0.1;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct Config {
    pub enabled: bool,
    pub items: Vec<Item>,
}

impl Config {
    /// A config file can say anything: the items that are not of the right shape, and those beyond the room, are dropped.
    pub fn sanitized(mut self) -> Self {
        self.items = logic::sanitized(&self.items);
        self
    }
}

/// How a command ended.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Ended {
    Code(i32),
    /// Ended by a signal: by Stop, or by the limit of time.
    Signal,
    TooLong,
    NotStarted,
}

/// What a running command hands to the tab: the pieces it printed, and how it ended.
#[derive(Default)]
struct Inbox {
    pieces: Vec<String>,
    ended: Option<Ended>,
    /// Stop was pressed: the end is Stop's and not the program's own.
    stopped: bool,
}

struct Run {
    name: String,
    index: usize,
    inbox: Arc<Mutex<Inbox>>,
    group: u32,
    _timer: app::Timer,
}

thread_local! {
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
    home: PathBuf,
    view: Rc<CommandsView>,
    config: RefCell<Config>,
    run: RefCell<Option<Run>>,
    /// What the last command to run printed, as far as it is kept, and how it ended.
    output: RefCell<String>,
    cleaner: RefCell<Cleaner>,
    last: RefCell<(Option<usize>, String)>,
    showing: Cell<bool>,
    me: Weak<Feature>,
}

/// Adds the tab. `None` without a notch, which is where the tab lives.
pub fn attach(env: &AttachEnv) -> Option<Rc<Feature>> {
    let notch = env.notch?;
    let feature = Rc::new_cyclic(|me: &Weak<Feature>| {
        let (run, stop, clear) = (me.clone(), me.clone(), me.clone());
        let view = CommandsView::new(
            env.mtm,
            notch.page_area(PAGE_SIZE.width, PAGE_SIZE.height),
            CommandsCallbacks {
                on_run: Box::new(move |index| {
                    if let Some(f) = run.upgrade() {
                        f.start(index);
                    }
                }),
                on_stop: Box::new(move || {
                    if let Some(f) = stop.upgrade() {
                        f.stop();
                    }
                }),
                on_clear: Box::new(move || {
                    if let Some(f) = clear.upgrade() {
                        f.clear();
                    }
                }),
            },
        );
        Feature {
            home: PathBuf::from(std::env::var_os("HOME").unwrap_or_default()),
            view,
            config: RefCell::new(env.config.commands.clone().sanitized()),
            run: RefCell::default(),
            output: RefCell::default(),
            cleaner: RefCell::default(),
            last: RefCell::default(),
            showing: Cell::new(false),
            me: me.clone(),
        }
    });
    FEATURE.with(|slot| *slot.borrow_mut() = Rc::downgrade(&feature));
    let (shown, hidden) = (Rc::downgrade(&feature), Rc::downgrade(&feature));
    notch.add_page(feature.view.page(
        text::tab(i18n::current()),
        move || {
            if let Some(f) = shown.upgrade() {
                f.showing.set(true);
                f.render();
            }
        },
        move || {
            if let Some(f) = hidden.upgrade() {
                f.showing.set(false);
            }
        },
    ));
    Some(feature)
}

impl Feature {
    pub fn configure(&self, config: &Config) -> Result<(), Fault> {
        let config = config.clone().sanitized();
        let was = self.config.replace(config.clone());
        if was.items != config.items || (was.enabled && !config.enabled) {
            // What runs was started from a list that is not this one any more.
            self.stop();
        }
        self.render();
        Ok(())
    }

    pub fn runtime(&self) -> Value {
        json!({ "running": self.run.borrow().is_some() })
    }

    fn render(&self) {
        let language = i18n::current();
        let config = self.config.borrow();
        let run = self.run.borrow();
        let last = self.last.borrow();
        let selected = run.as_ref().map(|r| r.index).or(last.0);
        let note = if !config.enabled {
            Some(text::off(language))
        } else if config.items.is_empty() {
            Some(text::none(language))
        } else {
            None
        };
        let footer = match run.as_ref() {
            Some(r) => text::running(language, &r.name),
            None => last.1.clone(),
        };
        self.view.show(&CommandsDisplay {
            caption: text::caption(language),
            rows: config
                .items
                .iter()
                .enumerate()
                .map(|(i, item)| CommandRow {
                    title: item.name.clone(),
                    detail: item.command.clone(),
                    selected: selected == Some(i),
                })
                .collect(),
            footer,
            stop_label: text::stop(language),
            clear_label: text::clear(language),
            running: run.is_some(),
            placeholder: text::placeholder(language),
            note,
        });
    }

    fn clear(&self) {
        self.output.borrow_mut().clear();
        self.view.set_output("");
    }

    /// Runs the command at this place of the list, if none runs: its line goes to the shell, and what it prints comes to
    /// the tab as it is printed.
    fn start(&self, index: usize) {
        let item = {
            let config = self.config.borrow();
            if !config.enabled || self.run.borrow().is_some() {
                return;
            }
            match config.items.get(index) {
                Some(item) => item.clone(),
                None => return,
            }
        };
        let language = i18n::current();
        self.clear();
        *self.cleaner.borrow_mut() = Cleaner::default();
        let inbox: Arc<Mutex<Inbox>> = Arc::default();
        let group = match spawn(&item.command, &self.home, &inbox) {
            Some(pid) => pid,
            None => {
                *self.last.borrow_mut() = (Some(index), text::failed(language, &item.name));
                self.render();
                return;
            }
        };
        trace::log("commands: one started");
        let weak = self.me.clone();
        let timer = app::every(LOOK_SECONDS, move || {
            if let Some(f) = weak.upgrade() {
                f.look();
            }
        });
        *self.run.borrow_mut() = Some(Run {
            name: item.name,
            index,
            inbox,
            group,
            _timer: timer,
        });
        self.render();
    }

    fn stop(&self) {
        let run = self.run.borrow();
        if let Some(run) = run.as_ref() {
            run.inbox
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .stopped = true;
            kill_group(run.group);
        }
    }

    /// Brings what the command printed since the last look to the tab, and ends the run when it has ended.
    fn look(&self) {
        let (pieces, ended, stopped) = {
            let run = self.run.borrow();
            let Some(run) = run.as_ref() else { return };
            let mut inbox = run.inbox.lock().unwrap_or_else(PoisonError::into_inner);
            (
                std::mem::take(&mut inbox.pieces),
                inbox.ended.clone(),
                inbox.stopped,
            )
        };
        let mut added = String::new();
        for piece in &pieces {
            added.push_str(&self.cleaner.borrow_mut().feed(piece));
        }
        if !added.is_empty() {
            let mut output = self.output.borrow_mut();
            output.push_str(&added);
            let kept = logic::tail(&output, logic::MAX_OUTPUT_CHARS).len();
            if kept < output.len() {
                let cut = output.len() - kept;
                output.drain(..cut);
                self.view.set_output(&output);
            } else {
                self.view.append(&added);
            }
        }
        if let Some(ended) = ended {
            let run = self.run.borrow_mut().take();
            if let Some(run) = run {
                let language = i18n::current();
                let words = match (ended, stopped) {
                    (Ended::TooLong, _) => text::too_long(language, &run.name),
                    (Ended::Signal, _) | (_, true) => text::stopped(language, &run.name),
                    (Ended::Code(code), false) => text::finished(language, &run.name, code),
                    (Ended::NotStarted, false) => text::failed(language, &run.name),
                };
                *self.last.borrow_mut() = (Some(run.index), words);
                trace::log("commands: one ended");
            }
            self.render();
        }
    }
}

/// Starts `zsh -lc <line>` in its own process group, with two threads that read what it prints and one that waits for it.
/// Returns the group, or `None` when the shell could not be started.
fn spawn(line: &str, home: &std::path::Path, inbox: &Arc<Mutex<Inbox>>) -> Option<u32> {
    let mut child = Command::new("/bin/zsh")
        .args(["-lc", line])
        .current_dir(if home.is_dir() {
            home
        } else {
            std::path::Path::new("/")
        })
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .process_group(0)
        .spawn()
        .ok()?;
    let group = child.id();
    let mut readers = Vec::new();
    let mut take = |source: Option<Box<dyn Read + Send>>| {
        if let Some(mut source) = source {
            let inbox = Arc::clone(inbox);
            readers.push(thread::spawn(move || {
                let mut buffer = [0u8; 8192];
                // A character cut in two by the end of a read is kept for the next.
                let mut held: Vec<u8> = Vec::new();
                while let Ok(n) = source.read(&mut buffer) {
                    if n == 0 {
                        break;
                    }
                    held.extend_from_slice(&buffer[..n]);
                    let piece = match std::str::from_utf8(&held) {
                        Ok(text) => {
                            let text = text.to_owned();
                            held.clear();
                            text
                        }
                        Err(e) => {
                            let good = e.valid_up_to();
                            let text = String::from_utf8_lossy(&held[..good]).into_owned();
                            // What is left is the start of a character, or bytes that are no text at all.
                            held.drain(..good);
                            if held.len() > 4 {
                                held.clear();
                            }
                            text
                        }
                    };
                    if !piece.is_empty() {
                        inbox
                            .lock()
                            .unwrap_or_else(PoisonError::into_inner)
                            .pieces
                            .push(piece);
                    }
                }
            }));
        }
    };
    take(
        child
            .stdout
            .take()
            .map(|s| Box::new(s) as Box<dyn Read + Send>),
    );
    take(
        child
            .stderr
            .take()
            .map(|s| Box::new(s) as Box<dyn Read + Send>),
    );
    let inbox = Arc::clone(inbox);
    thread::spawn(move || {
        let started = Instant::now();
        let mut too_long = false;
        let status = loop {
            match child.try_wait() {
                Ok(Some(status)) => break Some(status),
                Ok(None) if started.elapsed() >= Duration::from_secs(MAX_SECONDS) => {
                    too_long = true;
                    kill_group(group);
                    break child.wait().ok();
                }
                Ok(None) => thread::sleep(Duration::from_millis(50)),
                Err(_) => break None,
            }
        };
        for reader in readers {
            let _ = reader.join();
        }
        let ended = match (status, too_long) {
            (_, true) => Ended::TooLong,
            (Some(status), _) => status.code().map_or(Ended::Signal, Ended::Code),
            (None, _) => Ended::NotStarted,
        };
        inbox.lock().unwrap_or_else(PoisonError::into_inner).ended = Some(ended);
        app::on_main(|| with_feature(Feature::look));
    });
    Some(group)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn wait_for_end(inbox: &Arc<Mutex<Inbox>>) -> bool {
        for _ in 0..400 {
            if inbox.lock().unwrap().ended.is_some() {
                return true;
            }
            thread::sleep(Duration::from_millis(25));
        }
        false
    }

    fn printed(inbox: &Arc<Mutex<Inbox>>) -> String {
        inbox.lock().unwrap().pieces.concat()
    }

    #[test]
    fn it_is_off_by_default_and_a_config_file_keeps_only_good_items() {
        let c = Config::default();
        assert!(!c.enabled && c.items.is_empty());
        let parsed: Config = serde_json::from_str(
            r#"{ "enabled": true, "items": [{ "name": "Ok", "command": "ls" }, { "name": "", "command": "ls" }, { "command": "x" }] }"#,
        )
        .unwrap();
        assert_eq!(parsed.sanitized().items.len(), 1);
    }

    #[test]
    fn what_a_command_prints_on_both_streams_arrives_and_its_code_with_it() {
        let inbox: Arc<Mutex<Inbox>> = Arc::default();
        let group = spawn(
            "echo out; echo err 1>&2; exit 3",
            &std::env::temp_dir(),
            &inbox,
        );
        assert!(group.is_some());
        assert!(wait_for_end(&inbox), "the command never ended");
        let text = printed(&inbox);
        assert!(text.contains("out") && text.contains("err"), "{text:?}");
        assert_eq!(inbox.lock().unwrap().ended, Some(Ended::Code(3)));
    }

    #[test]
    fn a_command_that_waits_for_the_keyboard_gets_an_empty_one_and_ends() {
        let inbox: Arc<Mutex<Inbox>> = Arc::default();
        spawn("cat; echo after", &std::env::temp_dir(), &inbox).unwrap();
        assert!(wait_for_end(&inbox));
        assert!(printed(&inbox).contains("after"));
    }

    #[test]
    fn stopping_ends_the_command_and_what_it_started() {
        let inbox: Arc<Mutex<Inbox>> = Arc::default();
        let group = spawn(
            "sleep 30 & sleep 31; echo never",
            &std::env::temp_dir(),
            &inbox,
        )
        .unwrap();
        thread::sleep(Duration::from_millis(300));
        kill_group(group);
        assert!(
            wait_for_end(&inbox),
            "the command was not ended by the stop"
        );
        assert_eq!(inbox.lock().unwrap().ended, Some(Ended::Signal));
        assert!(!printed(&inbox).contains("never"));
    }

    #[test]
    fn what_is_printed_in_pieces_of_a_character_is_put_back_together() {
        let inbox: Arc<Mutex<Inbox>> = Arc::default();
        spawn(
            "printf '\\xe1\\xbb\\xa9 ok\\n'",
            &std::env::temp_dir(),
            &inbox,
        )
        .unwrap();
        assert!(wait_for_end(&inbox));
        assert_eq!(printed(&inbox), "\u{1ee9} ok\n");
    }
}
