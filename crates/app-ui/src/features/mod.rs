//! Features that live in a folder of their own, so that several can be built at the same time without touching
//! the same files. Shared code (the settings model and window, the menu, the composition root) reaches them
//! only through this file. A feature owns everything under `features/<name>/`, its page, strings and styles
//! under `web/features/<name>.*`, and its part of the macOS layer in `macos-sys`.

use crate::{
    clipboard::Clipboard, config::Config, media::MediaFeature, settings::model::Fault,
    shelf::ShelfFeature, tiling::Tiling,
};
use macos_sys::{MainThreadMarker, notch::Notch, status_item::Entry};
use serde_json::{Map, Value};
use std::{
    collections::HashMap,
    fs,
    io::{self, Read},
    os::unix::process::CommandExt,
    path::Path,
    process::{Command, Stdio},
    rc::{Rc, Weak},
    sync::{Arc, Mutex, PoisonError, mpsc},
    time::{Duration, Instant},
};

pub mod breaks;
pub mod dictation;
pub mod browser;
pub mod calendar;
pub mod capture;
pub mod cursor;
pub mod dayprogress;
pub mod devices;
pub mod displays;
pub mod download;
pub mod islands;
pub mod launcher;
pub mod lyrics;
pub mod mascot;
pub mod menubar;
pub mod mole;
pub mod musicwidget;
pub mod notes;
pub mod notifications;
pub mod outlook;
pub mod pin;
pub mod pomodoro;
pub mod present;
pub mod prs;
pub mod screentime;
pub mod smart;
pub mod snap;
pub mod snippets;
pub mod stack;
pub mod system;
pub mod translate;
pub mod typing;
pub mod updates;
pub mod utilities;
pub mod video;
pub mod vision;
pub mod weather;
pub mod wheel;

/// The tabs of the notch that have a place in the strip only while their feature is on, so that a tab which would only
/// say "this is off" is not there to be opened. Each follows the `enabled` of its own feature.
pub(crate) const OPT_IN_TABS: [&str; 15] = [
    "notes",
    "launcher",
    "weather",
    "prs",
    "updates",
    "pomodoro",
    "screentime",
    "dayprogress",
    "system",
    "devices",
    "displays",
    "present",
    "notifications",
    "outlook",
    "video",
];

/// Whether this tab is out of the strip for now because its feature is off.
pub fn tab_waits(id: &str, config: &Config) -> bool {
    match id {
        "notes" => !config.notes.enabled,
        "launcher" => !config.launcher.enabled,
        "weather" => !config.weather.enabled,
        "prs" => !config.prs.enabled,
        "updates" => !config.updates.enabled,
        "pomodoro" => !config.pomodoro.enabled,
        "screentime" => !config.screentime.enabled,
        "dayprogress" => !config.dayprogress.enabled,
        "system" => !config.system.enabled,
        "devices" => !config.devices.enabled,
        "displays" => !config.displays.enabled,
        "present" => !config.present.enabled,
        "notifications" => !config.notifications.enabled,
        "outlook" => !config.outlook.enabled,
        "video" => !config.video.enabled,
        _ => false,
    }
}

/// Whether a character may not stand in a name that is shown or compared: a control character, and the characters that
/// change the direction of text or take no room, with which a name such as `invoice` followed by U+202E and `gpj.exe`
/// is made to read as another.
pub(crate) fn is_hidden_char(c: char) -> bool {
    c.is_control()
        || matches!(
            c,
            '\u{200B}'
                | '\u{200E}'
                | '\u{200F}'
                | '\u{2028}'
                | '\u{2029}'
                | '\u{202A}'..='\u{202E}'
                | '\u{2060}'
                | '\u{2066}'..='\u{2069}'
                | '\u{FEFF}'
                | '\u{061C}'
        )
}

/// The most a file of ours holds that is read whole: what the tallies and the statistics keep is small, so a longer
/// file was written by something else.
pub(crate) const MAX_FILE: u64 = 1 << 20;

/// The text of a file no longer than `max` bytes; `None` for one that is missing, too long or not text.
pub(crate) fn read_small(path: &Path, max: u64) -> Option<String> {
    let mut text = String::new();
    fs::File::open(path)
        .ok()?
        .take(max + 1)
        .read_to_string(&mut text)
        .ok()?;
    (text.len() as u64 <= max).then_some(text)
}

pub(crate) use core_notes::write_atomically;

/// Why a command gave no answer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Ran {
    /// It did not finish in the time given, and was stopped.
    Slow,
    /// It is not there, or it said it failed.
    Failed,
}

/// The most a command may print before it is cut off.
const MAX_OUTPUT: usize = 1 << 20;
/// After a command has finished, how long what it printed is waited for: something it started may still hold the
/// output open, and is stopped if it does not let go.
const DRAIN: Duration = Duration::from_secs(2);

/// Runs a command and gives back what it printed, if it finishes within `seconds`; otherwise stops it, and what it
/// started with it. What it prints is read as it comes, so that a command with much to say does not wait for a reader
/// that is not there.
pub(crate) fn run_bounded(program: &str, args: &[&str], seconds: u64) -> Result<String, Ran> {
    run_bounded_with(program, args, &[], seconds)
}

/// The same with some environment variables added for the command.
pub(crate) fn run_bounded_with(
    program: &str,
    args: &[&str],
    envs: &[(&str, &str)],
    seconds: u64,
) -> Result<String, Ran> {
    match run_bounded_output(program, args, envs, seconds)? {
        (true, text) => Ok(text),
        (false, _) => Err(Ran::Failed),
    }
}

/// The same, giving back what it printed whether or not it said it failed, with whether it succeeded: some tools,
/// such as `npm outdated`, report what they found with a failing status.
pub(crate) fn run_bounded_output(
    program: &str,
    args: &[&str],
    envs: &[(&str, &str)],
    seconds: u64,
) -> Result<(bool, String), Ran> {
    let mut child = Command::new(program)
        .args(args)
        .envs(envs.iter().copied())
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        // A group of its own, so that what it starts can be stopped with it.
        .process_group(0)
        .spawn()
        .map_err(|_| Ran::Failed)?;
    let group = child.id();
    let stop_group = move || macos_sys::process::kill_group(group);
    let mut stdout = child.stdout.take().ok_or(Ran::Failed)?;
    let text = Arc::new(Mutex::new(Vec::<u8>::new()));
    // Told, when the reading is over, whether it stopped because there was too much.
    let (over, finished) = mpsc::channel::<bool>();
    let reading = Arc::clone(&text);
    std::thread::spawn(move || {
        let mut chunk = [0u8; 8192];
        let too_much = loop {
            match stdout.read(&mut chunk) {
                Ok(0) => break false,
                Ok(n) => {
                    let mut all = reading.lock().unwrap_or_else(PoisonError::into_inner);
                    all.extend_from_slice(&chunk[..n]);
                    if all.len() > MAX_OUTPUT {
                        break true;
                    }
                }
                Err(e) if e.kind() == io::ErrorKind::Interrupted => {}
                Err(_) => break false,
            }
        };
        // Letting go of the pipe here is what makes a command that is still printing fail instead of wait.
        drop(stdout);
        let _ = over.send(too_much);
    });
    let deadline = Instant::now() + Duration::from_secs(seconds);
    loop {
        match child.try_wait() {
            Ok(Some(status)) => {
                let too_much = match finished.recv_timeout(DRAIN) {
                    Ok(too_much) => too_much,
                    Err(_) => {
                        stop_group();
                        false
                    }
                };
                let printed = text.lock().unwrap_or_else(PoisonError::into_inner).clone();
                return if too_much {
                    Err(Ran::Failed)
                } else {
                    Ok((
                        status.success(),
                        String::from_utf8_lossy(&printed).into_owned(),
                    ))
                };
            }
            Ok(None) if Instant::now() < deadline => {
                std::thread::sleep(Duration::from_millis(40));
            }
            Ok(None) => {
                stop_group();
                let _ = child.kill();
                let _ = child.wait();
                return Err(Ran::Slow);
            }
            Err(_) => return Err(Ran::Failed),
        }
    }
}

/// What a feature may need when it starts. Each feature reads the fields it needs.
#[allow(dead_code)]
pub struct AttachEnv<'a> {
    pub mtm: MainThreadMarker,
    /// `None` when the notch cannot be shown on this machine.
    pub notch: Option<&'a Rc<Notch>>,
    /// The data directory.
    pub home: &'a Path,
    pub config: &'a Config,
    pub clipboard: Option<&'a Rc<Clipboard>>,
    pub tiling: &'a Rc<Tiling>,
    /// The media tab, `None` without a notch.
    pub media: Option<&'a Rc<MediaFeature>>,
    /// The shelf tab, `None` without a notch.
    pub shelf: Option<&'a Rc<ShelfFeature>>,
}

/// The running features, kept alive by the app context.
#[derive(Default)]
pub struct Features {
    pub smart: Option<Rc<smart::Feature>>,
    pub capture: Option<Rc<capture::Feature>>,
    pub vision: Option<Rc<vision::Feature>>,
    pub lyrics: Option<Rc<lyrics::Feature>>,
    pub calendar: Option<Rc<calendar::Feature>>,
    pub snap: Option<Rc<snap::Feature>>,
    pub menubar: Option<Rc<menubar::Feature>>,
    pub stack: Option<Rc<stack::Feature>>,
    pub mascot: Option<Rc<mascot::Feature>>,
    pub wheel: Option<Rc<wheel::Feature>>,
    pub mole: Option<Rc<mole::Feature>>,
    pub translate: Option<Rc<translate::Feature>>,
    pub pin: Option<Rc<pin::Feature>>,
    pub cursor: Option<Rc<cursor::Feature>>,
    pub musicwidget: Option<Rc<musicwidget::Feature>>,
    pub islands: Option<Rc<islands::Feature>>,
    pub utilities: Option<Rc<utilities::Feature>>,
    pub breaks: Option<Rc<breaks::Feature>>,
    pub dictation: Option<Rc<dictation::Feature>>,
    pub browser: Option<Rc<browser::Feature>>,
    pub notes: Option<Rc<notes::Feature>>,
    pub launcher: Option<Rc<launcher::Feature>>,
    pub weather: Option<Rc<weather::Feature>>,
    pub pomodoro: Option<Rc<pomodoro::Feature>>,
    pub prs: Option<Rc<prs::Feature>>,
    pub updates: Option<Rc<updates::Feature>>,
    pub screentime: Option<Rc<screentime::Feature>>,
    pub dayprogress: Option<Rc<dayprogress::Feature>>,
    pub devices: Option<Rc<devices::Feature>>,
    pub displays: Option<Rc<displays::Feature>>,
    pub present: Option<Rc<present::Feature>>,
    pub notifications: Option<Rc<notifications::Feature>>,
    pub outlook: Option<Rc<outlook::Feature>>,
    pub video: Option<Rc<video::Feature>>,
    pub system: Option<Rc<system::Feature>>,
    pub typing: Option<Rc<typing::Feature>>,
    pub snippets: Option<Rc<snippets::Feature>>,
    pub download: Option<Rc<download::Feature>>,
    /// Where the tabs of the features that are off are taken out of the strip and put back.
    notch: Option<Weak<Notch>>,
}

impl Features {
    pub fn attach(env: &AttachEnv) -> Self {
        // The wheel opens images in the capture editor, so it is given the capture feature.
        let capture = capture::attach(env);
        // Snippets are typed through the keyboard that Vietnamese typing owns.
        let typing = typing::attach(env);
        let mut features = Self {
            smart: smart::attach(env),
            wheel: wheel::attach(env, capture.as_ref()),
            capture,
            vision: vision::attach(env),
            lyrics: lyrics::attach(env),
            calendar: calendar::attach(env),
            snap: snap::attach(env),
            menubar: menubar::attach(env),
            stack: stack::attach(env),
            mascot: mascot::attach(env),
            mole: mole::attach(env),
            translate: translate::attach(env),
            pin: pin::attach(env),
            cursor: cursor::attach(env),
            musicwidget: musicwidget::attach(env),
            islands: islands::attach(env),
            utilities: utilities::attach(env),
            breaks: breaks::attach(env),
            dictation: dictation::attach(env),
            browser: None,
            notes: notes::attach(env),
            launcher: launcher::attach(env),
            weather: weather::attach(env),
            pomodoro: pomodoro::attach(env),
            prs: prs::attach(env),
            updates: updates::attach(env),
            screentime: screentime::attach(env),
            dayprogress: dayprogress::attach(env),
            system: system::attach(env),
            devices: devices::attach(env),
            displays: displays::attach(env),
            present: present::attach(env),
            notifications: notifications::attach(env),
            outlook: outlook::attach(env),
            video: video::attach(env),
            snippets: snippets::attach(typing.as_ref()),
            typing,
            download: download::attach(env),
            notch: env.notch.map(Rc::downgrade),
        };
        // The browser blocks while a Pomodoro work stage runs and hands addresses to the download.
        features.browser =
            browser::attach(env, features.pomodoro.as_ref(), features.download.as_ref());
        if let Some(notch) = env.notch {
            for id in OPT_IN_TABS {
                notch.set_page_dormant(id, tab_waits(id, env.config));
            }
        }
        features
    }

    /// Facts only the running app knows, by feature id, for the settings state.
    pub fn runtime(&self) -> HashMap<&'static str, Value> {
        let mut out = HashMap::new();
        if let Some(f) = &self.smart {
            out.insert("smart", f.runtime());
        }
        if let Some(f) = &self.capture {
            out.insert("capture", f.runtime());
        }
        if let Some(f) = &self.vision {
            out.insert("vision", f.runtime());
        }
        if let Some(f) = &self.lyrics {
            out.insert("lyrics", f.runtime());
        }
        if let Some(f) = &self.calendar {
            out.insert("calendar", f.runtime());
        }
        if let Some(f) = &self.snap {
            out.insert("snap", f.runtime());
        }
        if let Some(f) = &self.menubar {
            out.insert("menubar", f.runtime());
        }
        if let Some(f) = &self.stack {
            out.insert("stack", f.runtime());
        }
        if let Some(f) = &self.mascot {
            out.insert("mascot", f.runtime());
        }
        if let Some(f) = &self.wheel {
            out.insert("wheel", f.runtime());
        }
        if let Some(f) = &self.mole {
            out.insert("mole", f.runtime());
        }
        if let Some(f) = &self.translate {
            out.insert("translate", f.runtime());
        }
        if let Some(f) = &self.pin {
            out.insert("pin", f.runtime());
        }
        if let Some(f) = &self.cursor {
            out.insert("cursor", f.runtime());
        }
        if let Some(f) = &self.musicwidget {
            out.insert("musicwidget", f.runtime());
        }
        if let Some(f) = &self.islands {
            out.insert("islands", f.runtime());
        }
        if let Some(f) = &self.utilities {
            out.insert("utilities", f.runtime());
        }
        if let Some(f) = &self.dictation {
            out.insert("dictation", f.runtime());
        }
        if let Some(f) = &self.breaks {
            out.insert("breaks", f.runtime());
        }
        if let Some(f) = &self.browser {
            out.insert("browser", f.runtime());
        }
        if let Some(f) = &self.notes {
            out.insert("notes", f.runtime());
        }
        if let Some(f) = &self.launcher {
            out.insert("launcher", f.runtime());
        }
        if let Some(f) = &self.weather {
            out.insert("weather", f.runtime());
        }
        if let Some(f) = &self.pomodoro {
            out.insert("pomodoro", f.runtime());
        }
        if let Some(f) = &self.prs {
            out.insert("prs", f.runtime());
        }
        if let Some(f) = &self.updates {
            out.insert("updates", f.runtime());
        }
        if let Some(f) = &self.screentime {
            out.insert("screentime", f.runtime());
        }
        if let Some(f) = &self.present {
            out.insert("present", f.runtime());
        }
        if let Some(f) = &self.notifications {
            out.insert("notifications", f.runtime());
        }
        if let Some(f) = &self.outlook {
            out.insert("outlook", f.runtime());
        }
        if let Some(f) = &self.typing {
            out.insert("typing", f.runtime());
        }
        if let Some(f) = &self.snippets {
            out.insert("snippets", f.runtime());
        }
        if let Some(f) = &self.download {
            out.insert("download", f.runtime());
        }
        out
    }

    /// Applies the settings of one feature after a change.
    pub fn configure(&self, id: &str, config: &Config) -> Result<(), Fault> {
        let result = self.configure_feature(id, config);
        if OPT_IN_TABS.contains(&id)
            && let Some(notch) = self.notch.as_ref().and_then(Weak::upgrade)
        {
            notch.set_page_dormant(id, tab_waits(id, config));
        }
        result
    }

    fn configure_feature(&self, id: &str, config: &Config) -> Result<(), Fault> {
        match id {
            "smart" => self
                .smart
                .as_ref()
                .map_or(Ok(()), |f| f.configure(&config.smart)),
            "capture" => self
                .capture
                .as_ref()
                .map_or(Ok(()), |f| f.configure(&config.capture)),
            "vision" => self
                .vision
                .as_ref()
                .map_or(Ok(()), |f| f.configure(&config.vision)),
            "lyrics" => self
                .lyrics
                .as_ref()
                .map_or(Ok(()), |f| f.configure(&config.lyrics)),
            "calendar" => {
                // The Day progress tab leaves out the calendars that this one leaves out.
                if let Some(f) = &self.dayprogress {
                    f.set_excluded(&config.calendar.excluded);
                }
                self.calendar
                    .as_ref()
                    .map_or(Ok(()), |f| f.configure(&config.calendar))
            }
            "snap" => self
                .snap
                .as_ref()
                .map_or(Ok(()), |f| f.configure(&config.snap)),
            "menubar" => self
                .menubar
                .as_ref()
                .map_or(Ok(()), |f| f.configure(&config.menubar)),
            "stack" => self
                .stack
                .as_ref()
                .map_or(Ok(()), |f| f.configure(&config.stack)),
            "mascot" => self
                .mascot
                .as_ref()
                .map_or(Ok(()), |f| f.configure(&config.mascot)),
            "wheel" => self
                .wheel
                .as_ref()
                .map_or(Ok(()), |f| f.configure(&config.wheel)),
            "translate" => self
                .translate
                .as_ref()
                .map_or(Ok(()), |f| f.configure(&config.translate)),
            "pin" => self
                .pin
                .as_ref()
                .map_or(Ok(()), |f| f.configure(&config.pin)),
            "cursor" => self
                .cursor
                .as_ref()
                .map_or(Ok(()), |f| f.configure(&config.cursor)),
            "musicwidget" => self
                .musicwidget
                .as_ref()
                .map_or(Ok(()), |f| f.configure(&config.musicwidget)),
            "islands" => self
                .islands
                .as_ref()
                .map_or(Ok(()), |f| f.configure(&config.islands)),
            "utilities" => self
                .utilities
                .as_ref()
                .map_or(Ok(()), |f| f.configure(&config.utilities)),
            "breaks" => self
                .breaks
                .as_ref()
                .map_or(Ok(()), |f| f.configure(&config.breaks)),
            "dictation" => self
                .dictation
                .as_ref()
                .map_or(Ok(()), |f| f.configure(&config.dictation)),
            "browser" => self
                .browser
                .as_ref()
                .map_or(Ok(()), |f| f.configure(&config.browser)),
            "notes" => self
                .notes
                .as_ref()
                .map_or(Ok(()), |f| f.configure(&config.notes)),
            "launcher" => self
                .launcher
                .as_ref()
                .map_or(Ok(()), |f| f.configure(&config.launcher)),
            "weather" => self
                .weather
                .as_ref()
                .map_or(Ok(()), |f| f.configure(&config.weather)),
            "pomodoro" => self
                .pomodoro
                .as_ref()
                .map_or(Ok(()), |f| f.configure(&config.pomodoro)),
            "prs" => self
                .prs
                .as_ref()
                .map_or(Ok(()), |f| f.configure(&config.prs)),
            "updates" => self
                .updates
                .as_ref()
                .map_or(Ok(()), |f| f.configure(&config.updates)),
            "screentime" => self
                .screentime
                .as_ref()
                .map_or(Ok(()), |f| f.configure(&config.screentime)),
            "dayprogress" => self
                .dayprogress
                .as_ref()
                .map_or(Ok(()), |f| f.configure(&config.dayprogress)),
            "devices" => self
                .devices
                .as_ref()
                .map_or(Ok(()), |f| f.configure(&config.devices)),
            "displays" => self
                .displays
                .as_ref()
                .map_or(Ok(()), |f| f.configure(&config.displays)),
            "present" => self
                .present
                .as_ref()
                .map_or(Ok(()), |f| f.configure(&config.present)),
            "notifications" => self
                .notifications
                .as_ref()
                .map_or(Ok(()), |f| f.configure(&config.notifications)),
            "outlook" => self
                .outlook
                .as_ref()
                .map_or(Ok(()), |f| f.configure(&config.outlook)),
            "video" => self
                .video
                .as_ref()
                .map_or(Ok(()), |f| f.configure(&config.video)),
            "system" => self
                .system
                .as_ref()
                .map_or(Ok(()), |f| f.configure(&config.system)),
            "typing" => self
                .typing
                .as_ref()
                .map_or(Ok(()), |f| f.configure(&config.typing)),
            "snippets" => self
                .snippets
                .as_ref()
                .map_or(Ok(()), |f| f.configure(&config.snippets)),
            "download" => self
                .download
                .as_ref()
                .map_or(Ok(()), |f| f.configure(&config.download)),
            _ => Ok(()),
        }
    }

    /// While the settings page records a shortcut, no global shortcut may fire.
    pub fn suspend_hotkeys(&self) {
        if let Some(f) = &self.smart {
            f.suspend_hotkeys();
        }
        if let Some(f) = &self.capture {
            f.suspend_hotkeys();
        }
        if let Some(f) = &self.vision {
            f.suspend_hotkeys();
        }
        if let Some(f) = &self.lyrics {
            f.suspend_hotkeys();
        }
        if let Some(f) = &self.calendar {
            f.suspend_hotkeys();
        }
        if let Some(f) = &self.snap {
            f.suspend_hotkeys();
        }
        if let Some(f) = &self.menubar {
            f.suspend_hotkeys();
        }
        if let Some(f) = &self.stack {
            f.suspend_hotkeys();
        }
        if let Some(f) = &self.mascot {
            f.suspend_hotkeys();
        }
        if let Some(f) = &self.wheel {
            f.suspend_hotkeys();
        }
        if let Some(f) = &self.translate {
            f.suspend_hotkeys();
        }
        if let Some(f) = &self.pin {
            f.suspend_hotkeys();
        }
        if let Some(f) = &self.utilities {
            f.suspend_hotkeys();
        }
        if let Some(f) = &self.breaks {
            f.suspend_hotkeys();
        }
        if let Some(f) = &self.dictation {
            f.suspend_hotkeys();
        }
        if let Some(f) = &self.pomodoro {
            f.suspend_hotkeys();
        }
        if let Some(f) = &self.present {
            f.suspend_hotkeys();
        }
        if let Some(f) = &self.typing {
            f.suspend_hotkeys();
        }
    }

    pub fn resume_hotkeys(&self, config: &Config) {
        if let Some(f) = &self.smart {
            f.resume_hotkeys(&config.smart);
        }
        if let Some(f) = &self.capture {
            f.resume_hotkeys(&config.capture);
        }
        if let Some(f) = &self.vision {
            f.resume_hotkeys(&config.vision);
        }
        if let Some(f) = &self.lyrics {
            f.resume_hotkeys(&config.lyrics);
        }
        if let Some(f) = &self.calendar {
            f.resume_hotkeys(&config.calendar);
        }
        if let Some(f) = &self.snap {
            f.resume_hotkeys(&config.snap);
        }
        if let Some(f) = &self.menubar {
            f.resume_hotkeys(&config.menubar);
        }
        if let Some(f) = &self.stack {
            f.resume_hotkeys(&config.stack);
        }
        if let Some(f) = &self.mascot {
            f.resume_hotkeys(&config.mascot);
        }
        if let Some(f) = &self.wheel {
            f.resume_hotkeys(&config.wheel);
        }
        if let Some(f) = &self.translate {
            f.resume_hotkeys(&config.translate);
        }
        if let Some(f) = &self.pin {
            f.resume_hotkeys(&config.pin);
        }
        if let Some(f) = &self.utilities {
            f.resume_hotkeys(&config.utilities);
        }
        if let Some(f) = &self.breaks {
            f.resume_hotkeys(&config.breaks);
        }
        if let Some(f) = &self.dictation {
            f.resume_hotkeys(&config.dictation);
        }
        if let Some(f) = &self.pomodoro {
            f.resume_hotkeys(&config.pomodoro);
        }
        if let Some(f) = &self.present {
            f.resume_hotkeys(&config.present);
        }
        if let Some(f) = &self.typing {
            f.resume_hotkeys(&config.typing);
        }
    }

    /// A button of the settings page; `None` when no feature knows the name.
    pub fn action(&self, name: &str, config: &mut Config) -> Option<Result<(), Fault>> {
        if let Some(f) = &self.smart
            && let Some(result) = f.action(name, &mut config.smart)
        {
            return Some(result);
        }
        if let Some(f) = &self.capture
            && let Some(result) = f.action(name, &mut config.capture)
        {
            return Some(result);
        }
        if let Some(f) = &self.vision
            && let Some(result) = f.action(name, &mut config.vision)
        {
            return Some(result);
        }
        if let Some(f) = &self.lyrics
            && let Some(result) = f.action(name, &mut config.lyrics)
        {
            return Some(result);
        }
        if let Some(f) = &self.calendar
            && let Some(result) = f.action(name, &mut config.calendar)
        {
            return Some(result);
        }
        if let Some(f) = &self.snap
            && let Some(result) = f.action(name, &mut config.snap)
        {
            return Some(result);
        }
        if let Some(f) = &self.menubar
            && let Some(result) = f.action(name, &mut config.menubar)
        {
            return Some(result);
        }
        if let Some(f) = &self.stack
            && let Some(result) = f.action(name, &mut config.stack)
        {
            return Some(result);
        }
        if let Some(f) = &self.mascot
            && let Some(result) = f.action(name, &mut config.mascot)
        {
            return Some(result);
        }
        if let Some(f) = &self.wheel
            && let Some(result) = f.action(name, &mut config.wheel)
        {
            return Some(result);
        }
        if let Some(f) = &self.translate
            && let Some(result) = f.action(name, &mut config.translate)
        {
            return Some(result);
        }
        if let Some(f) = &self.pin
            && let Some(result) = f.action(name, &mut config.pin)
        {
            return Some(result);
        }
        if let Some(f) = &self.musicwidget
            && let Some(result) = f.action(name, &mut config.musicwidget)
        {
            return Some(result);
        }
        if let Some(f) = &self.islands
            && let Some(result) = f.action(name, &mut config.islands)
        {
            return Some(result);
        }
        if let Some(f) = &self.utilities
            && let Some(result) = f.action(name, &mut config.utilities)
        {
            return Some(result);
        }
        if let Some(f) = &self.breaks
            && let Some(result) = f.action(name)
        {
            return Some(result);
        }
        if let Some(f) = &self.browser
            && let Some(result) = f.action(name)
        {
            return Some(result);
        }
        if let Some(f) = &self.notes
            && let Some(result) = f.action(name)
        {
            return Some(result);
        }
        if let Some(f) = &self.launcher
            && let Some(result) = f.action(name, &mut config.launcher)
        {
            return Some(result);
        }
        if let Some(f) = &self.weather
            && let Some(result) = f.action(name, &mut config.weather)
        {
            return Some(result);
        }
        if let Some(f) = &self.pomodoro
            && let Some(result) = f.action(name)
        {
            return Some(result);
        }
        if let Some(f) = &self.dictation
            && let Some(result) = f.action(name)
        {
            return Some(result);
        }
        if let Some(f) = &self.present
            && let Some(result) = f.action(name)
        {
            return Some(result);
        }
        if let Some(f) = &self.notifications
            && let Some(result) = f.action(name)
        {
            return Some(result);
        }
        if let Some(f) = &self.outlook
            && let Some(result) = f.action(name)
        {
            return Some(result);
        }
        if let Some(f) = &self.prs
            && let Some(result) = f.action(name)
        {
            return Some(result);
        }
        if let Some(f) = &self.updates
            && let Some(result) = f.action(name)
        {
            return Some(result);
        }
        if let Some(f) = &self.screentime
            && let Some(result) = f.action(name)
        {
            return Some(result);
        }
        if let Some(f) = &self.cursor
            && let Some(result) = f.action(name, &mut config.cursor)
        {
            return Some(result);
        }
        if let Some(f) = &self.mole
            && let Some(result) = f.action(name)
        {
            return Some(result);
        }
        if let Some(f) = &self.typing
            && let Some(result) = f.action(name, &mut config.typing)
        {
            return Some(result);
        }
        if let Some(f) = &self.download
            && let Some(result) = f.action(name)
        {
            return Some(result);
        }
        None
    }

    /// Entries for the menu bar item.
    pub fn menu_entries(&self) -> Vec<Entry> {
        let mut entries = Vec::new();
        if let Some(f) = &self.smart {
            entries.extend(smart::Feature::menu_entries(f));
        }
        if let Some(f) = &self.capture {
            entries.extend(capture::Feature::menu_entries(f));
        }
        if let Some(f) = &self.vision {
            entries.extend(vision::Feature::menu_entries(f));
        }
        if let Some(f) = &self.lyrics {
            entries.extend(lyrics::Feature::menu_entries(f));
        }
        if let Some(f) = &self.calendar {
            entries.extend(calendar::Feature::menu_entries(f));
        }
        if let Some(f) = &self.snap {
            entries.extend(snap::Feature::menu_entries(f));
        }
        if let Some(f) = &self.menubar {
            entries.extend(menubar::Feature::menu_entries(f));
        }
        if let Some(f) = &self.stack {
            entries.extend(stack::Feature::menu_entries(f));
        }
        if let Some(f) = &self.mascot {
            entries.extend(mascot::Feature::menu_entries(f));
        }
        if let Some(f) = &self.wheel {
            entries.extend(wheel::Feature::menu_entries(f));
        }
        if let Some(f) = &self.translate {
            entries.extend(translate::Feature::menu_entries(f));
        }
        if let Some(f) = &self.pin {
            entries.extend(pin::Feature::menu_entries(f));
        }
        if let Some(f) = &self.mole {
            entries.extend(mole::Feature::menu_entries(f));
        }
        if let Some(f) = &self.islands {
            entries.extend(islands::Feature::menu_entries(f));
        }
        if let Some(f) = &self.utilities {
            entries.extend(utilities::Feature::menu_entries(f));
        }
        if let Some(f) = &self.breaks {
            entries.extend(breaks::Feature::menu_entries(f));
        }
        if let Some(f) = &self.pomodoro {
            entries.extend(pomodoro::Feature::menu_entries(f));
        }
        if let Some(f) = &self.present {
            entries.extend(present::Feature::menu_entries(f));
        }
        if let Some(f) = &self.typing {
            entries.extend(typing::Feature::menu_entries(f));
        }
        if let Some(f) = &self.download {
            entries.extend(download::Feature::menu_entries(f));
        }
        entries
    }
}

/// Adds each feature's part to the state of the settings page, under its id.
pub fn state(
    config: &Config,
    runtime: &HashMap<&'static str, Value>,
    root: &mut Map<String, Value>,
) {
    let none = Value::Null;
    root.insert(
        "smart".into(),
        smart::settings::state(&config.smart, runtime.get("smart").unwrap_or(&none)),
    );
    root.insert(
        "capture".into(),
        capture::settings::state(&config.capture, runtime.get("capture").unwrap_or(&none)),
    );
    root.insert(
        "vision".into(),
        vision::settings::state(&config.vision, runtime.get("vision").unwrap_or(&none)),
    );
    root.insert(
        "lyrics".into(),
        lyrics::settings::state(&config.lyrics, runtime.get("lyrics").unwrap_or(&none)),
    );
    root.insert(
        "calendar".into(),
        calendar::settings::state(&config.calendar, runtime.get("calendar").unwrap_or(&none)),
    );
    root.insert(
        "snap".into(),
        snap::settings::state(&config.snap, runtime.get("snap").unwrap_or(&none)),
    );
    root.insert(
        "menubar".into(),
        menubar::settings::state(&config.menubar, runtime.get("menubar").unwrap_or(&none)),
    );
    root.insert(
        "stack".into(),
        stack::settings::state(&config.stack, runtime.get("stack").unwrap_or(&none)),
    );
    root.insert(
        "mascot".into(),
        mascot::settings::state(&config.mascot, runtime.get("mascot").unwrap_or(&none)),
    );
    root.insert(
        "wheel".into(),
        wheel::settings::state(&config.wheel, runtime.get("wheel").unwrap_or(&none)),
    );
    root.insert(
        "translate".into(),
        translate::settings::state(&config.translate, runtime.get("translate").unwrap_or(&none)),
    );
    root.insert(
        "pin".into(),
        pin::settings::state(&config.pin, runtime.get("pin").unwrap_or(&none)),
    );
    root.insert(
        "cursor".into(),
        cursor::settings::state(&config.cursor, runtime.get("cursor").unwrap_or(&none)),
    );
    root.insert(
        "musicwidget".into(),
        musicwidget::settings::state(
            &config.musicwidget,
            runtime.get("musicwidget").unwrap_or(&none),
        ),
    );
    root.insert(
        "islands".into(),
        islands::settings::state(&config.islands, runtime.get("islands").unwrap_or(&none)),
    );
    root.insert(
        "utilities".into(),
        utilities::settings::state(&config.utilities, runtime.get("utilities").unwrap_or(&none)),
    );
    root.insert(
        "breaks".into(),
        breaks::settings::state(&config.breaks, runtime.get("breaks").unwrap_or(&none)),
    );
    root.insert(
        "browser".into(),
        browser::settings::state(&config.browser, runtime.get("browser").unwrap_or(&none)),
    );
    root.insert(
        "notes".into(),
        notes::settings::state(&config.notes, runtime.get("notes").unwrap_or(&none)),
    );
    root.insert(
        "launcher".into(),
        launcher::settings::state(&config.launcher, runtime.get("launcher").unwrap_or(&none)),
    );
    root.insert(
        "weather".into(),
        weather::settings::state(&config.weather, runtime.get("weather").unwrap_or(&none)),
    );
    root.insert(
        "prs".into(),
        prs::settings::state(&config.prs, runtime.get("prs").unwrap_or(&none)),
    );
    root.insert(
        "updates".into(),
        updates::settings::state(&config.updates, runtime.get("updates").unwrap_or(&none)),
    );
    root.insert(
        "pomodoro".into(),
        pomodoro::settings::state(&config.pomodoro, runtime.get("pomodoro").unwrap_or(&none)),
    );
    root.insert(
        "dictation".into(),
        dictation::settings::state(&config.dictation, runtime.get("dictation").unwrap_or(&none)),
    );
    root.insert(
        "screentime".into(),
        screentime::settings::state(
            &config.screentime,
            runtime.get("screentime").unwrap_or(&none),
        ),
    );
    root.insert("system".into(), system::settings::state(&config.system));
    root.insert(
        "dayprogress".into(),
        dayprogress::settings::state(&config.dayprogress),
    );
    root.insert("devices".into(), devices::settings::state(&config.devices));
    root.insert(
        "displays".into(),
        displays::settings::state(&config.displays),
    );
    root.insert(
        "present".into(),
        present::settings::state(&config.present, runtime.get("present").unwrap_or(&none)),
    );
    root.insert(
        "notifications".into(),
        notifications::settings::state(
            &config.notifications,
            runtime.get("notifications").unwrap_or(&none),
        ),
    );
    root.insert(
        "outlook".into(),
        outlook::settings::state(&config.outlook, runtime.get("outlook").unwrap_or(&none)),
    );
    root.insert("video".into(), video::settings::state(&config.video));
    root.insert(
        "mole".into(),
        mole::settings::state(runtime.get("mole").unwrap_or(&none)),
    );
    root.insert(
        "typing".into(),
        typing::settings::state(&config.typing, runtime.get("typing").unwrap_or(&none)),
    );
    root.insert(
        "snippets".into(),
        snippets::settings::state(&config.snippets, runtime.get("snippets").unwrap_or(&none)),
    );
    root.insert(
        "download".into(),
        download::settings::state(&config.download, runtime.get("download").unwrap_or(&none)),
    );
}

/// A change of a setting whose path starts with a feature id. `None` when no feature owns the path; the
/// success value is the id of the feature to configure.
pub fn set(config: &mut Config, path: &str, value: &Value) -> Option<Result<&'static str, Fault>> {
    let (head, rest) = path.split_once('.')?;
    match head {
        "smart" => {
            smart::settings::set(&mut config.smart, rest, value).map(|r| r.map(|()| "smart"))
        }
        "capture" => {
            capture::settings::set(&mut config.capture, rest, value).map(|r| r.map(|()| "capture"))
        }
        "vision" => {
            vision::settings::set(&mut config.vision, rest, value).map(|r| r.map(|()| "vision"))
        }
        "lyrics" => {
            lyrics::settings::set(&mut config.lyrics, rest, value).map(|r| r.map(|()| "lyrics"))
        }
        "calendar" => calendar::settings::set(&mut config.calendar, rest, value)
            .map(|r| r.map(|()| "calendar")),
        "snap" => snap::settings::set(&mut config.snap, rest, value).map(|r| r.map(|()| "snap")),
        "menubar" => {
            menubar::settings::set(&mut config.menubar, rest, value).map(|r| r.map(|()| "menubar"))
        }
        "stack" => {
            stack::settings::set(&mut config.stack, rest, value).map(|r| r.map(|()| "stack"))
        }
        "mascot" => {
            mascot::settings::set(&mut config.mascot, rest, value).map(|r| r.map(|()| "mascot"))
        }
        "wheel" => {
            wheel::settings::set(&mut config.wheel, rest, value).map(|r| r.map(|()| "wheel"))
        }
        "translate" => translate::settings::set(&mut config.translate, rest, value)
            .map(|r| r.map(|()| "translate")),
        "pin" => pin::settings::set(&mut config.pin, rest, value).map(|r| r.map(|()| "pin")),
        "cursor" => {
            cursor::settings::set(&mut config.cursor, rest, value).map(|r| r.map(|()| "cursor"))
        }
        "musicwidget" => musicwidget::settings::set(&mut config.musicwidget, rest, value)
            .map(|r| r.map(|()| "musicwidget")),
        "islands" => {
            islands::settings::set(&mut config.islands, rest, value).map(|r| r.map(|()| "islands"))
        }
        "utilities" => utilities::settings::set(&mut config.utilities, rest, value)
            .map(|r| r.map(|()| "utilities")),
        "breaks" => {
            breaks::settings::set(&mut config.breaks, rest, value).map(|r| r.map(|()| "breaks"))
        }
        "browser" => {
            browser::settings::set(&mut config.browser, rest, value).map(|r| r.map(|()| "browser"))
        }
        "notes" => {
            notes::settings::set(&mut config.notes, rest, value).map(|r| r.map(|()| "notes"))
        }
        "launcher" => launcher::settings::set(&mut config.launcher, rest, value)
            .map(|r| r.map(|()| "launcher")),
        "weather" => {
            weather::settings::set(&mut config.weather, rest, value).map(|r| r.map(|()| "weather"))
        }
        "prs" => prs::settings::set(&mut config.prs, rest, value).map(|r| r.map(|()| "prs")),
        "updates" => {
            updates::settings::set(&mut config.updates, rest, value).map(|r| r.map(|()| "updates"))
        }
        "pomodoro" => pomodoro::settings::set(&mut config.pomodoro, rest, value)
            .map(|r| r.map(|()| "pomodoro")),
        "dictation" => dictation::settings::set(&mut config.dictation, rest, value)
            .map(|r| r.map(|()| "dictation")),
        "screentime" => screentime::settings::set(&mut config.screentime, rest, value)
            .map(|r| r.map(|()| "screentime")),
        "system" => {
            system::settings::set(&mut config.system, rest, value).map(|r| r.map(|()| "system"))
        }
        "dayprogress" => dayprogress::settings::set(&mut config.dayprogress, rest, value)
            .map(|r| r.map(|()| "dayprogress")),
        "devices" => {
            devices::settings::set(&mut config.devices, rest, value).map(|r| r.map(|()| "devices"))
        }
        "displays" => displays::settings::set(&mut config.displays, rest, value)
            .map(|r| r.map(|()| "displays")),
        "present" => {
            present::settings::set(&mut config.present, rest, value).map(|r| r.map(|()| "present"))
        }
        "notifications" => notifications::settings::set(&mut config.notifications, rest, value)
            .map(|r| r.map(|()| "notifications")),
        "outlook" => {
            outlook::settings::set(&mut config.outlook, rest, value).map(|r| r.map(|()| "outlook"))
        }
        "video" => {
            video::settings::set(&mut config.video, rest, value).map(|r| r.map(|()| "video"))
        }
        "typing" => {
            typing::settings::set(&mut config.typing, rest, value).map(|r| r.map(|()| "typing"))
        }
        "snippets" => snippets::settings::set(&mut config.snippets, rest, value)
            .map(|r| r.map(|()| "snippets")),
        "download" => download::settings::set(&mut config.download, rest, value)
            .map(|r| r.map(|()| "download")),
        _ => None,
    }
}

/// The id of the feature a shortcut target belongs to: "capture" or "capture:region" belong to `capture`.
pub fn shortcut_owner(target: &str) -> Option<&'static str> {
    let head = target.split(':').next()?;
    [
        "smart",
        "capture",
        "vision",
        "lyrics",
        "calendar",
        "snap",
        "menubar",
        "stack",
        "mascot",
        "wheel",
        "translate",
        "pin",
        "utilities",
        "breaks",
        "pomodoro",
        "present",
        "typing",
        "dictation",
    ]
    .into_iter()
    .find(|id| *id == head)
}

/// Every shortcut of every feature as (target, shortcut).
pub fn shortcuts(config: &Config) -> Vec<(String, String)> {
    let mut all = Vec::new();
    all.extend(smart::settings::shortcuts(&config.smart));
    all.extend(capture::settings::shortcuts(&config.capture));
    all.extend(vision::settings::shortcuts(&config.vision));
    all.extend(lyrics::settings::shortcuts(&config.lyrics));
    all.extend(calendar::settings::shortcuts(&config.calendar));
    all.extend(snap::settings::shortcuts(&config.snap));
    all.extend(menubar::settings::shortcuts(&config.menubar));
    all.extend(stack::settings::shortcuts(&config.stack));
    all.extend(mascot::settings::shortcuts(&config.mascot));
    all.extend(wheel::settings::shortcuts(&config.wheel));
    all.extend(translate::settings::shortcuts(&config.translate));
    all.extend(pin::settings::shortcuts(&config.pin));
    all.extend(utilities::settings::shortcuts(&config.utilities));
    all.extend(breaks::settings::shortcuts(&config.breaks));
    all.extend(pomodoro::settings::shortcuts(&config.pomodoro));
    all.extend(present::settings::shortcuts(&config.present));
    all.extend(typing::settings::shortcuts(&config.typing));
    all.extend(dictation::settings::shortcuts(&config.dictation));
    all
}

/// Stores a shortcut for a target that `shortcut_owner` recognised.
pub fn set_shortcut(
    config: &mut Config,
    target: &str,
    value: &str,
) -> Option<Result<&'static str, Fault>> {
    match shortcut_owner(target)? {
        "smart" => smart::settings::set_shortcut(&mut config.smart, target, value)
            .map(|r| r.map(|()| "smart")),
        "capture" => capture::settings::set_shortcut(&mut config.capture, target, value)
            .map(|r| r.map(|()| "capture")),
        "vision" => vision::settings::set_shortcut(&mut config.vision, target, value)
            .map(|r| r.map(|()| "vision")),
        "lyrics" => lyrics::settings::set_shortcut(&mut config.lyrics, target, value)
            .map(|r| r.map(|()| "lyrics")),
        "calendar" => calendar::settings::set_shortcut(&mut config.calendar, target, value)
            .map(|r| r.map(|()| "calendar")),
        "snap" => snap::settings::set_shortcut(&mut config.snap, target, value)
            .map(|r| r.map(|()| "snap")),
        "menubar" => menubar::settings::set_shortcut(&mut config.menubar, target, value)
            .map(|r| r.map(|()| "menubar")),
        "stack" => stack::settings::set_shortcut(&mut config.stack, target, value)
            .map(|r| r.map(|()| "stack")),
        "mascot" => mascot::settings::set_shortcut(&mut config.mascot, target, value)
            .map(|r| r.map(|()| "mascot")),
        "wheel" => wheel::settings::set_shortcut(&mut config.wheel, target, value)
            .map(|r| r.map(|()| "wheel")),
        "translate" => translate::settings::set_shortcut(&mut config.translate, target, value)
            .map(|r| r.map(|()| "translate")),
        "pin" => {
            pin::settings::set_shortcut(&mut config.pin, target, value).map(|r| r.map(|()| "pin"))
        }
        "utilities" => utilities::settings::set_shortcut(&mut config.utilities, target, value)
            .map(|r| r.map(|()| "utilities")),
        "breaks" => breaks::settings::set_shortcut(&mut config.breaks, target, value)
            .map(|r| r.map(|()| "breaks")),
        "pomodoro" => pomodoro::settings::set_shortcut(&mut config.pomodoro, target, value)
            .map(|r| r.map(|()| "pomodoro")),
        "present" => present::settings::set_shortcut(&mut config.present, target, value)
            .map(|r| r.map(|()| "present")),
        "typing" => typing::settings::set_shortcut(&mut config.typing, target, value)
            .map(|r| r.map(|()| "typing")),
        "dictation" => dictation::settings::set_shortcut(&mut config.dictation, target, value)
            .map(|r| r.map(|()| "dictation")),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::parse_hotkey;

    #[test]
    fn what_a_command_leaves_running_does_not_hold_up_its_answer_and_is_stopped() {
        let started = Instant::now();
        // The shell prints and leaves, but the sleep it started still holds the output open.
        let answer = run_bounded("/bin/sh", &["-c", "sleep 31 & echo hi"], 20);
        assert_eq!(answer, Ok("hi\n".to_owned()));
        assert!(
            started.elapsed() < Duration::from_secs(8),
            "its answer was not waited for until the sleep ended"
        );
        let running = || {
            Command::new("/usr/bin/pgrep")
                .args(["-f", "sleep 31"])
                .output()
                .is_ok_and(|out| out.status.success())
        };
        let gone = (0..20).any(|_| {
            std::thread::sleep(Duration::from_millis(100));
            !running()
        });
        assert!(gone, "what the command started was stopped with it");
    }

    #[test]
    fn a_file_is_written_whole_over_the_old_one_and_a_link_at_the_working_name_is_not_followed() {
        let dir = std::env::temp_dir().join(format!("figghost-atomic-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        let file = dir.join("deeper/tally.json");
        write_atomically(&file, "first").unwrap();
        assert_eq!(
            fs::read_to_string(&file).unwrap(),
            "first",
            "the folders are made"
        );
        write_atomically(&file, "second").unwrap();
        assert_eq!(
            fs::read_to_string(&file).unwrap(),
            "second",
            "it replaces what was there"
        );
        assert!(
            !dir.join("deeper/tally.json.part").exists(),
            "no half-written file is left"
        );
        // A link left where the working file goes would send the write somewhere else if it were followed.
        let elsewhere = dir.join("elsewhere.txt");
        fs::write(&elsewhere, "untouched").unwrap();
        std::os::unix::fs::symlink(&elsewhere, dir.join("deeper/tally.json.part")).unwrap();
        write_atomically(&file, "third").unwrap();
        assert_eq!(fs::read_to_string(&file).unwrap(), "third");
        assert_eq!(fs::read_to_string(&elsewhere).unwrap(), "untouched");
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn a_file_is_read_whole_only_when_it_is_small_text() {
        let dir = std::env::temp_dir().join(format!("figghost-small-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join("small"), "hello").unwrap();
        fs::write(dir.join("exact"), "x".repeat(100)).unwrap();
        fs::write(dir.join("long"), "x".repeat(101)).unwrap();
        fs::write(dir.join("binary"), [0xff, 0xfe, 0x00]).unwrap();
        assert_eq!(
            read_small(&dir.join("small"), 100).as_deref(),
            Some("hello")
        );
        assert_eq!(
            read_small(&dir.join("exact"), 100).map(|t| t.len()),
            Some(100)
        );
        assert_eq!(
            read_small(&dir.join("long"), 100),
            None,
            "one byte over is too long"
        );
        assert_eq!(read_small(&dir.join("binary"), 100), None, "not text");
        assert_eq!(read_small(&dir.join("missing"), 100), None);
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn characters_that_make_a_name_read_as_another_are_hidden_and_ordinary_ones_are_not() {
        for hidden in [
            '\u{202E}', '\u{202A}', '\u{2066}', '\u{2069}', '\u{200B}', '\u{200E}', '\u{FEFF}',
            '\u{2028}', '\u{7}', '\n',
        ] {
            assert!(is_hidden_char(hidden), "{hidden:?}");
        }
        for shown in ['a', ' ', 'ệ', '-', '日', '\u{200D}', '\u{200C}'] {
            assert!(!is_hidden_char(shown), "{shown:?}");
        }
    }

    #[test]
    fn the_tab_of_a_feature_waits_for_it_to_be_switched_on() {
        let mut config = Config::default();
        for id in OPT_IN_TABS {
            assert!(
                tab_waits(id, &config),
                "{id} is off by default, so its tab waits"
            );
        }
        config.weather.enabled = true;
        assert!(!tab_waits("weather", &config), "an on feature has its tab");
        assert!(tab_waits("system", &config), "and the others still wait");
        for id in [
            "clipboard",
            "shelf",
            "media",
            "agents",
            "calendar",
            "sound",
            "nonsense",
        ] {
            assert!(!tab_waits(id, &config), "{id} is not an opt-in tab");
        }
    }

    #[test]
    fn every_opt_in_tab_is_a_tab_of_the_notch_and_none_is_listed_twice() {
        for (i, id) in OPT_IN_TABS.iter().enumerate() {
            assert!(crate::config::NOTCH_PAGES.contains(id), "{id} is not a tab");
            assert!(!OPT_IN_TABS[i + 1..].contains(id), "{id} is listed twice");
        }
    }

    #[test]
    fn a_command_is_run_for_what_it_prints_and_stopped_when_it_is_slow() {
        assert_eq!(
            run_bounded("/bin/echo", &["hello"], 5),
            Ok("hello\n".to_owned())
        );
        assert_eq!(run_bounded("/usr/bin/false", &[], 5), Err(Ran::Failed));
        assert_eq!(
            run_bounded("/this/is/not/a/program", &[], 5),
            Err(Ran::Failed)
        );
        let started = Instant::now();
        assert_eq!(run_bounded("/bin/sleep", &["30"], 1), Err(Ran::Slow));
        assert!(
            started.elapsed() < Duration::from_secs(5),
            "it was stopped, not waited for"
        );
        // More than a megabyte is too much to read: the command is cut off and counts as failed, not waited for.
        assert_eq!(run_bounded("/usr/bin/yes", &[], 5), Err(Ran::Failed));
        // One with much to say, within that, is read as it comes.
        let long = run_bounded("/usr/bin/seq", &["1", "20000"], 5).unwrap();
        assert_eq!(long.lines().count(), 20_000, "all of it came");
    }

    /// Every shortcut that is on by default, across the clipboard, the window layouts and the features, is a
    /// different key combination: two tasks must never claim the same one.
    #[test]
    fn default_shortcuts_never_clash() {
        let config = Config::default();
        let mut all: Vec<(String, String)> = vec![("clipboard".into(), config.hotkey.clone())];
        all.extend(
            config
                .tiling
                .bindings()
                .into_iter()
                .map(|(layout, shortcut)| (format!("tiling:{}", layout.key()), shortcut)),
        );
        all.extend(shortcuts(&config));
        let mut seen: Vec<(crate::config::HotkeySpec, String)> = Vec::new();
        for (target, shortcut) in all {
            let spec = parse_hotkey(&shortcut)
                .unwrap_or_else(|e| panic!("{target} has the shortcut {shortcut:?}: {e}"));
            if let Some((_, other)) = seen.iter().find(|(known, _)| *known == spec) {
                panic!("{target} and {other} both use {shortcut}");
            }
            seen.push((spec, target));
        }
    }
}
