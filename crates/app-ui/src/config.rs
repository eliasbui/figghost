use crate::audio::model::AudioConfig;
use core_mouse::Settings;
use core_window::Layout;
use macos_sys::hotkey::modifier;
use serde::{Deserialize, Serialize};
use std::{
    collections::HashMap,
    env, fs, io,
    os::unix::fs::DirBuilderExt,
    path::{Path, PathBuf},
};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct TilingConfig {
    pub enabled: bool,
    /// Shortcut per layout name (`left`, `top_right`, `maximize`, ...); an empty string turns that layout off.
    pub shortcuts: HashMap<String, String>,
}

impl Default for TilingConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            shortcuts: HashMap::new(),
        }
    }
}

impl TilingConfig {
    /// Every active layout with its shortcut: the user's override, else the built-in default.
    pub fn bindings(&self) -> Vec<(Layout, String)> {
        Layout::ALL
            .into_iter()
            .map(|layout| {
                let shortcut = self.shortcuts.get(layout.key()).map_or_else(
                    || layout.default_shortcut().to_owned(),
                    |s| s.trim().to_owned(),
                );
                (layout, shortcut)
            })
            .filter(|(_, shortcut)| !shortcut.is_empty())
            .collect()
    }
}

/// The tabs of the notch, in their default order.
pub const NOTCH_PAGES: [&str; 14] = [
    "clipboard",
    "shelf",
    "media",
    "focus",
    "agents",
    "calendar",
    "notes",
    "launcher",
    "weather",
    "prs",
    "pomodoro",
    "screentime",
    "system",
    "sound",
];

/// The notch and how it looks and behaves. Read leniently: a hand-edited value of the wrong type or an unknown
/// word falls back to its default instead of making the whole file unreadable.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(from = "serde_json::Value")]
pub struct NotchConfig {
    /// The notch panel, with its hover trigger. Off removes it from the screen entirely.
    pub enabled: bool,
    /// What the open notch is made of: `glass`, `clear` (glass with less tint), `ink` (glass, heavily tinted),
    /// `accent` (clear glass in the accent colour) or `black`.
    pub material: String,
    pub shadow: bool,
    /// How it moves: `spring`, `smooth` or `none`.
    pub motion: String,
    pub open_on_hover: bool,
    /// How long the pointer rests on the notch before it opens, 0 to 1000.
    pub hover_delay_ms: u32,
    /// The least width of the open notch in points, 0 for what each tab needs, otherwise 480 to 1000.
    pub width: u32,
    /// The least height of the open notch in points, 0 for what each tab needs, otherwise 260 to 900.
    pub height: u32,
    /// The closed notch on a screen without a hardware notch: width 120 to 400 and height 20 to 48, 0 for the default.
    pub rest_width: u32,
    pub rest_height: u32,
    /// On a screen without a notch: `notch` shows a black tab, `hidden` only a strip at the top edge.
    pub rest: String,
    /// Keeps the notch out of screenshots and screen recordings.
    pub hide_in_capture: bool,
    pub haptics: bool,
    /// Which screen: `auto` (the built-in one with a notch, else the main one), `main` or `pointer`.
    pub display: String,
    /// The tab it opens on: `last`, `activity` (what is playing or working), or a tab id.
    pub open_page: String,
    /// The tabs to show, in order; empty shows them all.
    pub pages: Vec<String>,
    /// Playing music in the closed notch: the cover and moving bars.
    pub live_media: bool,
    /// Working coding agents in the closed notch.
    pub live_agents: bool,
    /// What moves the bars: `live` (the sound the Mac plays; needs System Audio Recording) or `animated`.
    pub waveform: String,
    /// A global shortcut that opens or closes the notch; empty for none.
    pub shortcut: String,
    /// The look of the volume indicator: `notch` (black, under the cutout), `glass` (the same shape in Liquid
    /// Glass), `corner` (a capsule in the corner), `bar` (the level filling the shape), `island` (a capsule at the
    /// middle of the top), `side` (a slider on the right edge), `tile` (a large tile low on the screen), `ring`
    /// (a round dial), `equalizer` (bars that light up) or `wide` (a long thin capsule).
    pub hud: String,
    /// Where a floating indicator goes: `auto` (the place of its style) or one of the nine places of the screen.
    pub hud_position: String,
}

impl Default for NotchConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            material: "glass".into(),
            shadow: true,
            motion: "spring".into(),
            open_on_hover: true,
            // About what other notch apps wait, so a pointer that only passes by does not open it.
            hover_delay_ms: 300,
            width: 0,
            height: 0,
            rest_width: 0,
            rest_height: 0,
            rest: "notch".into(),
            hide_in_capture: false,
            haptics: true,
            display: "auto".into(),
            open_page: "activity".into(),
            pages: Vec::new(),
            live_media: true,
            live_agents: true,
            waveform: "live".into(),
            shortcut: String::new(),
            hud: "notch".into(),
            hud_position: "auto".into(),
        }
    }
}

/// `value` when it is one of `words`, else the default.
/// A size in points: 0 for automatic, otherwise kept between `least` and `most`.
pub fn size_or_auto(value: &serde_json::Value, least: u64, most: u64, default: u32) -> u32 {
    value.as_u64().map_or(default, |n| {
        if n == 0 {
            0
        } else {
            n.clamp(least, most) as u32
        }
    })
}

fn word(value: &serde_json::Value, words: &[&str], default: &str) -> String {
    value
        .as_str()
        .filter(|w| words.contains(w))
        .unwrap_or(default)
        .to_owned()
}

/// The screen setting: one of the fixed words, or `screen:<name>` of one of the screens (a name that is gone
/// again falls back to `auto` when the notch asks for it).
fn screen_choice(value: &serde_json::Value, default: &str) -> String {
    match value.as_str() {
        Some(text) if ["auto", "main", "pointer"].contains(&text) => text.to_owned(),
        Some(text)
            if text
                .strip_prefix("screen:")
                .is_some_and(|name| !name.is_empty()) =>
        {
            text.to_owned()
        }
        _ => default.to_owned(),
    }
}

/// The tab ids in `value` that exist, each once, in the given order.
pub fn notch_pages(value: &serde_json::Value) -> Vec<String> {
    let mut pages: Vec<String> = Vec::new();
    for id in value
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|v| v.as_str())
    {
        if NOTCH_PAGES.contains(&id) && !pages.iter().any(|p| p == id) {
            pages.push(id.to_owned());
        }
    }
    pages
}

impl From<serde_json::Value> for NotchConfig {
    fn from(v: serde_json::Value) -> Self {
        let d = Self::default();
        let flag = |key: &str, default: bool| v[key].as_bool().unwrap_or(default);
        let open_page = v["open_page"]
            .as_str()
            .filter(|w| ["last", "activity"].contains(w) || NOTCH_PAGES.contains(w))
            .unwrap_or(&d.open_page)
            .to_owned();
        Self {
            enabled: flag("enabled", d.enabled),
            material: word(
                &v["material"],
                &["glass", "clear", "ink", "accent", "black"],
                &d.material,
            ),
            shadow: flag("shadow", d.shadow),
            motion: word(&v["motion"], &["spring", "smooth", "none"], &d.motion),
            open_on_hover: flag("open_on_hover", d.open_on_hover),
            hover_delay_ms: v["hover_delay_ms"]
                .as_u64()
                .map_or(d.hover_delay_ms, |ms| ms.min(1000) as u32),
            width: size_or_auto(&v["width"], 480, 1000, d.width),
            height: size_or_auto(&v["height"], 260, 900, d.height),
            rest_width: size_or_auto(&v["rest_width"], 120, 400, d.rest_width),
            rest_height: size_or_auto(&v["rest_height"], 20, 48, d.rest_height),
            rest: word(&v["rest"], &["notch", "hidden"], &d.rest),
            hide_in_capture: flag("hide_in_capture", d.hide_in_capture),
            haptics: flag("haptics", d.haptics),
            display: screen_choice(&v["display"], &d.display),
            open_page,
            pages: notch_pages(&v["pages"]),
            live_media: flag("live_media", d.live_media),
            live_agents: flag("live_agents", d.live_agents),
            waveform: word(&v["waveform"], &["live", "animated"], &d.waveform),
            shortcut: v["shortcut"].as_str().unwrap_or("").trim().to_owned(),
            hud: word(
                &v["hud"],
                &["notch", "glass", "corner", "bar", "island", "side", "tile", "ring", "equalizer", "wide"],
                &d.hud,
            ),
            hud_position: word(
                &v["hud_position"],
                &["auto", "top-left", "top", "top-right", "left", "center", "right", "bottom-left", "bottom", "bottom-right"],
                &d.hud_position,
            ),
        }
    }
}

impl NotchConfig {
    /// What the notch itself needs to know.
    pub fn style(&self) -> macos_sys::notch::Style {
        use macos_sys::notch::{Display, Material, Motion, OpenPage, Rest, Style};
        Style {
            material: match self.material.as_str() {
                "clear" => Material::ClearGlass,
                "ink" => Material::Ink,
                "accent" => Material::Accent,
                "black" => Material::Black,
                _ => Material::Glass,
            },
            shadow: self.shadow,
            motion: match self.motion.as_str() {
                "smooth" => Motion::Smooth,
                "none" => Motion::Instant,
                _ => Motion::Spring,
            },
            open_on_hover: self.open_on_hover,
            hover_delay: f64::from(self.hover_delay_ms.min(1000)) / 1000.0,
            rest: if self.rest == "hidden" {
                Rest::Hidden
            } else {
                Rest::Notch
            },
            hide_in_capture: self.hide_in_capture,
            haptics: self.haptics,
            min_width: f64::from(self.width),
            min_height: f64::from(self.height),
            rest_width: f64::from(self.rest_width),
            rest_height: f64::from(self.rest_height),
            display: match self.display.strip_prefix("screen:") {
                Some(name) if !name.is_empty() => Display::Screen(name.to_owned()),
                _ => match self.display.as_str() {
                    "main" => Display::Main,
                    "pointer" => Display::Pointer,
                    _ => Display::Auto,
                },
            },
            open_page: match self.open_page.as_str() {
                "last" => OpenPage::Last,
                "activity" => OpenPage::Activity,
                page => OpenPage::Page(page.to_owned()),
            },
            pages: self.pages.clone(),
        }
    }

    pub fn hud_position(&self) -> macos_sys::hud::HudPosition {
        macos_sys::hud::HudPosition::named(&self.hud_position)
    }

    /// What the volume indicator needs to know.
    pub fn hud_style(&self) -> macos_sys::hud::HudStyle {
        macos_sys::hud::HudStyle::named(&self.hud)
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct McpConfig {
    /// Let an AI assistant search the clipboard history through `Figghost --mcp`. Off by default: what
    /// was copied can include secrets.
    pub enabled: bool,
}

/// The seconds a command may wait for an answer in the notch. Below the lower bound nobody could answer;
/// above the upper one the agent would give up on the hook first.
pub const AGENT_WAIT_RANGE: std::ops::RangeInclusive<u32> = 5..=100;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct AgentsConfig {
    /// Listen for the hooks of Claude Code and Codex and show them in the notch. Off by default.
    pub enabled: bool,
    /// How long a command waits for an answer in the notch before the agent asks in its own terminal.
    pub approval_wait_seconds: u32,
    /// A sound when a command needs an answer or an agent finishes.
    pub sound: bool,
    /// Show for a moment in the closed notch that an agent finished.
    pub peek_when_done: bool,
    /// Open the notch when a command needs an answer; off shows it in the closed notch instead.
    pub open_on_ask: bool,
    /// Count the tokens and estimate the cost from the agents' own transcripts on this Mac.
    pub tokens: bool,
}

impl Default for AgentsConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            approval_wait_seconds: 30,
            sound: true,
            peek_when_done: true,
            open_on_ask: false,
            tokens: true,
        }
    }
}

/// The version of the file's layout this build writes; older files are brought up to it once, when they load.
pub const CONFIG_VERSION: u32 = 2;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    /// The layout of this file; 0 for files written before it was recorded (hence its own default).
    #[serde(default)]
    pub version: u32,
    /// `auto` follows the system language; or a language code such as `en` or `vi`.
    pub language: String,
    /// Open the settings window every time the app is started by hand (never when started at login).
    pub show_settings_on_launch: bool,
    /// Each time the app is started by hand, its ghost comes out of the notch and blinks (never at login, never
    /// when started in the background, and only with the notch on).
    pub launch_effect: bool,
    /// The keys of the actions in the notch and the clipboard popup.
    pub panel_keys: PanelKeys,
    /// The icon in the menu bar. Without it, opening the app again (Finder, Spotlight) shows the settings.
    pub menu_icon: bool,
    /// The settings window's appearance: `auto` follows the system, or `light` or `dark`.
    pub theme: String,
    pub mouse: Settings,
    pub tiling: TilingConfig,
    /// Global shortcut that opens the clipboard popup, e.g. "cmd+shift+c".
    pub hotkey: String,
    /// Send Command+V to the frontmost app after choosing an item; off means copy only.
    pub auto_paste: bool,
    /// Paste text without its formatting by default; Option+Shift+Enter then pastes it with formatting.
    pub paste_plain: bool,
    pub mcp: McpConfig,
    pub agents: AgentsConfig,
    pub notch: NotchConfig,
    pub smart: crate::features::smart::Config,
    pub capture: crate::features::capture::Config,
    pub vision: crate::features::vision::Config,
    pub lyrics: crate::features::lyrics::Config,
    pub calendar: crate::features::calendar::Config,
    pub snap: crate::features::snap::Config,
    pub menubar: crate::features::menubar::Config,
    pub stack: crate::features::stack::Config,
    pub mascot: crate::features::mascot::Config,
    pub update: crate::update::Config,
    pub wheel: crate::features::wheel::Config,
    pub translate: crate::features::translate::Config,
    pub pin: crate::features::pin::Config,
    pub cursor: crate::features::cursor::Config,
    pub musicwidget: crate::features::musicwidget::Config,
    pub islands: crate::features::islands::Config,
    pub utilities: crate::features::utilities::Config,
    pub notes: crate::features::notes::Config,
    pub launcher: crate::features::launcher::Config,
    pub weather: crate::features::weather::Config,
    pub prs: crate::features::prs::Config,
    pub pomodoro: crate::features::pomodoro::Config,
    pub screentime: crate::features::screentime::Config,
    pub system: crate::features::system::Config,
    /// Replace the system volume indicator with one in the notch. Needs Accessibility access.
    pub volume_hud: bool,
    /// Unpinned items kept in history.
    pub history_limit: usize,
    /// Bundle identifiers whose copies are never recorded, on top of the built-in password managers.
    pub ignored_apps: Vec<String>,
    /// The per-app mixer: volumes, output devices and effects.
    pub audio: AudioConfig,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            version: CONFIG_VERSION,
            language: "auto".into(),
            show_settings_on_launch: true,
            launch_effect: true,
            panel_keys: PanelKeys::default(),
            menu_icon: true,
            theme: "auto".into(),
            mouse: Settings::default(),
            tiling: TilingConfig::default(),
            hotkey: "cmd+shift+c".into(),
            auto_paste: true,
            paste_plain: false,
            mcp: McpConfig::default(),
            agents: AgentsConfig::default(),
            notch: NotchConfig::default(),
            smart: crate::features::smart::Config::default(),
            capture: crate::features::capture::Config::default(),
            vision: crate::features::vision::Config::default(),
            lyrics: crate::features::lyrics::Config::default(),
            calendar: crate::features::calendar::Config::default(),
            snap: crate::features::snap::Config::default(),
            menubar: crate::features::menubar::Config::default(),
            stack: crate::features::stack::Config::default(),
            mascot: crate::features::mascot::Config::default(),
            update: crate::update::Config::default(),
            wheel: crate::features::wheel::Config::default(),
            translate: crate::features::translate::Config::default(),
            pin: crate::features::pin::Config::default(),
            cursor: crate::features::cursor::Config::default(),
            musicwidget: crate::features::musicwidget::Config::default(),
            islands: crate::features::islands::Config::default(),
            utilities: crate::features::utilities::Config::default(),
            notes: crate::features::notes::Config::default(),
            launcher: crate::features::launcher::Config::default(),
            weather: crate::features::weather::Config::default(),
            prs: crate::features::prs::Config::default(),
            pomodoro: crate::features::pomodoro::Config::default(),
            screentime: crate::features::screentime::Config::default(),
            system: crate::features::system::Config::default(),
            volume_hud: false,
            history_limit: 500,
            ignored_apps: Vec::new(),
            audio: AudioConfig::default(),
        }
    }
}

/// `FIGGHOST_HOME` overrides the data directory (used by tests and for running a second profile).
pub fn home_dir() -> PathBuf {
    if let Some(dir) = env::var_os("FIGGHOST_HOME") {
        return PathBuf::from(dir);
    }
    let home = env::var_os("HOME").map(PathBuf::from).unwrap_or_default();
    home.join("Library/Application Support/Figghost")
}

impl Config {
    fn path(dir: &Path) -> PathBuf {
        dir.join("config.json")
    }

    /// Missing file means defaults. An unreadable file is moved aside (so a later save cannot destroy
    /// the user's hand edits) and defaults are used; the returned message says what happened.
    pub fn load(dir: &Path) -> (Self, Option<String>) {
        let path = Self::path(dir);
        let text = match fs::read_to_string(&path) {
            Ok(t) => t,
            Err(e) if e.kind() == io::ErrorKind::NotFound => return (Self::default(), None),
            Err(e) => {
                return (
                    Self::default(),
                    Some(format!("cannot read {}: {e}", path.display())),
                );
            }
        };
        match serde_json::from_str::<Config>(&text) {
            Ok(mut cfg) => {
                cfg.mouse = cfg.mouse.sanitized();
                cfg.history_limit = cfg.history_limit.clamp(10, 100_000);
                cfg.agents.approval_wait_seconds = cfg
                    .agents
                    .approval_wait_seconds
                    .clamp(*AGENT_WAIT_RANGE.start(), *AGENT_WAIT_RANGE.end());
                cfg.audio = cfg.audio.sanitized();
                cfg.capture = cfg.capture.sanitized();
                cfg.islands = cfg.islands.sanitized();
                cfg.utilities = cfg.utilities.sanitized();
                cfg.notes = cfg.notes.sanitized();
                cfg.launcher = cfg.launcher.sanitized();
                cfg.weather = cfg.weather.sanitized();
                cfg.prs = cfg.prs.sanitized();
                cfg.pomodoro = cfg.pomodoro.sanitized();
                cfg.screentime = cfg.screentime.sanitized();
                cfg.system = cfg.system.sanitized();
                if !["auto", "light", "dark"].contains(&cfg.theme.as_str()) {
                    cfg.theme = "auto".into();
                }
                cfg.upgrade();
                (cfg, None)
            }
            Err(e) => {
                let backup = path.with_extension("json.bad");
                let moved = fs::rename(&path, &backup).is_ok();
                let note = format!(
                    "{} is invalid ({e}); {}",
                    path.display(),
                    if moved {
                        format!("moved to {}", backup.display())
                    } else {
                        "left in place".into()
                    }
                );
                (Self::default(), Some(note))
            }
        }
    }

    /// Brings a file written by an older version up to date, once: what it changes is saved with the next save,
    /// and from then on the user's own choices stand.
    fn upgrade(&mut self) {
        if self.version < 2 {
            // Dragging a window onto the notch used to be off by default, and the settings window wrote that default
            // into every file, so a file cannot tell a choice from a default: it is turned on once, as it now is
            // for everyone.
            self.snap.enabled = true;
        }
        self.version = CONFIG_VERSION;
    }

    pub fn save(&self, dir: &Path) -> io::Result<()> {
        fs::DirBuilder::new()
            .recursive(true)
            .mode(0o700)
            .create(dir)?;
        let tmp = dir.join("config.json.tmp");
        fs::write(
            &tmp,
            serde_json::to_vec_pretty(self).map_err(io::Error::other)?,
        )?;
        fs::rename(tmp, Self::path(dir))
    }
}

/// The keys of the actions in the notch and the clipboard popup, as shortcuts ("opt+p"). Arrows, Return, Escape,
/// Page Up and Down, Command with a number or a letter, and Command+Delete keep the meaning they have everywhere
/// on the Mac.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct PanelKeys {
    /// Pins or unpins the selected item.
    pub pin: String,
    /// Files the selected item in the next board.
    pub board: String,
    /// Pastes the other way, with or without its formatting.
    pub paste_other: String,
    /// Deletes the selected item, refuses an agent's request, dismisses an alert.
    pub delete: String,
}

impl Default for PanelKeys {
    fn default() -> Self {
        Self {
            pin: "opt+p".into(),
            board: "opt+b".into(),
            paste_other: "opt+shift+return".into(),
            delete: "opt+delete".into(),
        }
    }
}

impl PanelKeys {
    /// The actions, by the names their targets carry ("keys:pin").
    pub const ACTIONS: [&'static str; 4] = ["pin", "board", "paste_other", "delete"];

    pub fn get(&self, action: &str) -> Option<&String> {
        match action {
            "pin" => Some(&self.pin),
            "board" => Some(&self.board),
            "paste_other" => Some(&self.paste_other),
            "delete" => Some(&self.delete),
            _ => None,
        }
    }

    pub fn get_mut(&mut self, action: &str) -> Option<&mut String> {
        match action {
            "pin" => Some(&mut self.pin),
            "board" => Some(&mut self.board),
            "paste_other" => Some(&mut self.paste_other),
            "delete" => Some(&mut self.delete),
            _ => None,
        }
    }

    /// The keys as the panels take them. A shortcut that does not read (a hand-edited file) falls back to the
    /// action's default key.
    pub fn action_keys(&self) -> macos_sys::panel::ActionKeys {
        let default = Self::default();
        let combo = |text: &str, fallback: &str| {
            let spec = parse_hotkey(text).or_else(|_| parse_hotkey(fallback));
            spec.ok().map(|spec| macos_sys::panel::Combo {
                key_code: spec.key_code as u16,
                command: spec.modifiers & modifier::COMMAND != 0,
                option: spec.modifiers & modifier::OPTION != 0,
                shift: spec.modifiers & modifier::SHIFT != 0,
                control: spec.modifiers & modifier::CONTROL != 0,
            })
        };
        macos_sys::panel::ActionKeys {
            pin: combo(&self.pin, &default.pin),
            board: combo(&self.board, &default.board),
            paste_other: combo(&self.paste_other, &default.paste_other),
            delete: combo(&self.delete, &default.delete),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HotkeySpec {
    pub key_code: u32,
    pub modifiers: u32,
}

/// Parses "cmd+shift+c" style shortcuts (ANSI virtual key codes). A shortcut needs a modifier unless
/// it is a function key, so a config typo cannot capture a plain letter system-wide.
pub fn parse_hotkey(text: &str) -> Result<HotkeySpec, String> {
    let mut modifiers = 0;
    let mut key = None;
    for part in text.split('+').map(|p| p.trim().to_lowercase()) {
        match part.as_str() {
            "cmd" | "command" => modifiers |= modifier::COMMAND,
            "shift" => modifiers |= modifier::SHIFT,
            "opt" | "option" | "alt" => modifiers |= modifier::OPTION,
            "ctrl" | "control" => modifiers |= modifier::CONTROL,
            other => {
                if key
                    .replace(key_code(other).ok_or_else(|| format!("unknown key '{other}'"))?)
                    .is_some()
                {
                    return Err("more than one key".into());
                }
            }
        }
    }
    let key_code = key.ok_or("no key given")?;
    if modifiers == 0 {
        return Err("needs at least one modifier".into());
    }
    Ok(HotkeySpec {
        key_code,
        modifiers,
    })
}

/// The virtual key code of a key by the name shortcuts and the drag wheel write ("a", "f19", "space").
pub(crate) fn key_code(name: &str) -> Option<u32> {
    Some(match name {
        "a" => 0,
        "s" => 1,
        "d" => 2,
        "f" => 3,
        "h" => 4,
        "g" => 5,
        "z" => 6,
        "x" => 7,
        "c" => 8,
        "v" => 9,
        "b" => 11,
        "q" => 12,
        "w" => 13,
        "e" => 14,
        "r" => 15,
        "y" => 16,
        "t" => 17,
        "o" => 31,
        "u" => 32,
        "i" => 34,
        "p" => 35,
        "l" => 37,
        "j" => 38,
        "k" => 40,
        "n" => 45,
        "m" => 46,
        "1" => 18,
        "2" => 19,
        "3" => 20,
        "4" => 21,
        "5" => 23,
        "6" => 22,
        "7" => 26,
        "8" => 28,
        "9" => 25,
        "0" => 29,
        "space" => 49,
        "return" | "enter" => 36,
        "tab" => 48,
        "escape" | "esc" => 53,
        "delete" | "backspace" => 51,
        "left" => 123,
        "right" => 124,
        "down" => 125,
        "up" => 126,
        "f1" => 122,
        "f2" => 120,
        "f3" => 99,
        "f4" => 118,
        "f5" => 96,
        "f6" => 97,
        "f7" => 98,
        "f8" => 100,
        "f9" => 101,
        "f10" => 109,
        "f11" => 103,
        "f12" => 111,
        "f13" => 105,
        "f14" => 107,
        "f15" => 113,
        "f16" => 106,
        "f17" => 64,
        "f18" => 79,
        "f19" => 80,
        "f20" => 90,
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};

    fn temp_dir() -> PathBuf {
        static N: AtomicUsize = AtomicUsize::new(0);
        let dir = env::temp_dir().join(format!(
            "figghost-config-test-{}-{}",
            std::process::id(),
            N.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn parses_shortcuts() {
        let spec = parse_hotkey("Cmd+Shift+C").unwrap();
        assert_eq!(
            spec,
            HotkeySpec {
                key_code: 8,
                modifiers: modifier::COMMAND | modifier::SHIFT
            }
        );
        assert_eq!(parse_hotkey(" ctrl + opt + space ").unwrap().key_code, 49);
        assert!(parse_hotkey("f13").is_err());
        assert_eq!(parse_hotkey("opt+f13").unwrap().key_code, 105);
        assert!(parse_hotkey("c").is_err());
        assert!(parse_hotkey("cmd+").is_err());
        assert!(parse_hotkey("cmd+a+b").is_err());
        assert!(parse_hotkey("cmd+nonsense").is_err());
    }

    #[test]
    fn tiling_shortcuts_default_override_and_disable() {
        let mut tiling = TilingConfig::default();
        assert_eq!(tiling.bindings().len(), Layout::ALL.len());
        tiling
            .shortcuts
            .insert("left".into(), "cmd+opt+left".into());
        tiling.shortcuts.insert("maximize".into(), "".into());
        let bindings = tiling.bindings();
        assert_eq!(bindings.len(), Layout::ALL.len() - 1);
        assert!(bindings.contains(&(Layout::Left, "cmd+opt+left".to_owned())));
        assert!(bindings.contains(&(Layout::Grid1, "cmd+1".to_owned())));
        assert!(bindings.contains(&(Layout::Grid9, "cmd+9".to_owned())));
        assert!(bindings.iter().all(|(l, _)| *l != Layout::Maximize));
        // Every built-in shortcut must parse, or the feature would silently lose keys.
        for (layout, shortcut) in TilingConfig::default().bindings() {
            assert!(
                parse_hotkey(&shortcut).is_ok(),
                "{} -> {shortcut}",
                layout.key()
            );
        }
    }

    #[test]
    fn missing_file_gives_defaults_and_save_round_trips() {
        let dir = temp_dir();
        let (cfg, note) = Config::load(&dir);
        assert!(note.is_none());
        assert_eq!(cfg.hotkey, "cmd+shift+c");
        let mut edited = cfg;
        edited.mouse.reverse = true;
        edited.history_limit = 123;
        edited.save(&dir).unwrap();
        let (again, note) = Config::load(&dir);
        assert!(note.is_none());
        assert!(again.mouse.reverse);
        assert_eq!(again.history_limit, 123);
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn broken_file_is_moved_aside_not_overwritten() {
        let dir = temp_dir();
        fs::write(dir.join("config.json"), "{ not json").unwrap();
        let (cfg, note) = Config::load(&dir);
        assert!(note.unwrap().contains("invalid"));
        assert_eq!(cfg.history_limit, 500);
        assert_eq!(
            fs::read_to_string(dir.join("config.json.bad")).unwrap(),
            "{ not json"
        );
        assert!(!dir.join("config.json").exists());
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn language_defaults_to_auto_and_can_be_set() {
        let dir = temp_dir();
        assert_eq!(Config::load(&dir).0.language, "auto");
        fs::write(dir.join("config.json"), r#"{"language": "vi"}"#).unwrap();
        assert_eq!(Config::load(&dir).0.language, "vi");
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn new_settings_have_safe_defaults_and_old_files_still_load() {
        let dir = temp_dir();
        // A file written by an older version has none of the newer keys.
        fs::write(
            dir.join("config.json"),
            r#"{"hotkey": "cmd+shift+v", "auto_paste": false, "mouse": {"reverse": true}}"#,
        )
        .unwrap();
        let (cfg, note) = Config::load(&dir);
        assert!(note.is_none());
        assert!(cfg.show_settings_on_launch && cfg.notch.enabled);
        assert!(!cfg.paste_plain && !cfg.volume_hud);
        assert_eq!(cfg.hotkey, "cmd+shift+v");
        assert!(cfg.mouse.reverse && cfg.mouse.shift_horizontal && !cfg.mouse.command_zoom);
        assert!(!dir.join("config.json.bad").exists());
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn the_volume_hud_is_off_unless_asked_for() {
        let dir = temp_dir();
        assert!(!Config::load(&dir).0.volume_hud);
        fs::write(dir.join("config.json"), r#"{"volume_hud": true}"#).unwrap();
        assert!(Config::load(&dir).0.volume_hud);
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn agents_are_off_by_default_and_their_wait_stays_in_range() {
        let dir = temp_dir();
        let (cfg, _) = Config::load(&dir);
        assert!(!cfg.agents.enabled && cfg.agents.sound && cfg.agents.peek_when_done);
        assert_eq!(cfg.agents.approval_wait_seconds, 30);
        fs::write(
            dir.join("config.json"),
            r#"{"agents": {"enabled": true, "approval_wait_seconds": 100000}}"#,
        )
        .unwrap();
        let (cfg, _) = Config::load(&dir);
        assert!(cfg.agents.enabled);
        assert_eq!(cfg.agents.approval_wait_seconds, 100);
        fs::write(
            dir.join("config.json"),
            r#"{"agents": {"approval_wait_seconds": 0}}"#,
        )
        .unwrap();
        assert_eq!(Config::load(&dir).0.agents.approval_wait_seconds, 5);
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn an_older_file_turns_window_snapping_on_once_and_then_keeps_the_choice() {
        let dir = temp_dir();
        fs::write(
            dir.join("config.json"),
            r#"{"snap": {"enabled": false, "zones": ["left"]}}"#,
        )
        .unwrap();
        let (cfg, _) = Config::load(&dir);
        assert!(cfg.snap.enabled);
        assert_eq!(cfg.version, CONFIG_VERSION);
        let mut off = cfg;
        off.snap.enabled = false;
        off.save(&dir).unwrap();
        assert!(
            !Config::load(&dir).0.snap.enabled,
            "a choice made since stands"
        );
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn the_notch_can_be_pinned_to_a_named_screen() {
        let json = serde_json::json!({ "notch": { "display": "screen:LG FHD" } });
        let notch: NotchConfig = serde_json::from_value(json["notch"].clone()).unwrap();
        assert_eq!(notch.display, "screen:LG FHD");
        assert_eq!(
            notch.style().display,
            macos_sys::notch::Display::Screen("LG FHD".into())
        );
        // A screen name that is really a word of the setting, and an empty name, are the default.
        for bad in ["screen:", "screen", "LGFHD", ""] {
            let json = serde_json::json!({ "display": bad });
            let notch: NotchConfig = serde_json::from_value(json).unwrap();
            assert_eq!(notch.display, "auto", "{bad} was taken as a screen");
        }
        // What is saved reads back the same.
        let again: NotchConfig =
            serde_json::from_value(serde_json::to_value(&notch).unwrap()).unwrap();
        assert_eq!(again, notch);
    }

    #[test]
    fn notch_settings_survive_hand_edits_of_the_wrong_kind() {
        let dir = temp_dir();
        fs::write(
            dir.join("config.json"),
            r#"{"hotkey": "cmd+shift+v", "notch": {"enabled": "yes", "material": 7, "motion": "bouncy",
                "hover_delay_ms": 99999, "pages": ["media", "nope", "media", 3, "clipboard"],
                "open_page": "agents", "shortcut": "  ctrl+opt+n "}}"#,
        )
        .unwrap();
        let (cfg, note) = Config::load(&dir);
        assert!(note.is_none(), "the file stays readable");
        assert_eq!(cfg.hotkey, "cmd+shift+v");
        let notch = cfg.notch;
        assert!(notch.enabled && notch.material == "glass" && notch.motion == "spring");
        assert_eq!(notch.hover_delay_ms, 1000);
        assert_eq!(notch.pages, ["media", "clipboard"]);
        assert_eq!(notch.open_page, "agents");
        assert_eq!(notch.shortcut, "ctrl+opt+n");
        let style = notch.style();
        assert_eq!(style.hover_delay, 1.0);
        assert_eq!(
            style.open_page,
            macos_sys::notch::OpenPage::Page("agents".into())
        );
        // What is saved reads back the same.
        let again: NotchConfig =
            serde_json::from_value(serde_json::to_value(&notch).unwrap()).unwrap();
        assert_eq!(again, notch);
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn every_look_a_setting_names_reaches_the_panel_it_is_for() {
        use macos_sys::hud::HudStyle;
        use macos_sys::notch::Material;
        let mut notch = NotchConfig::default();
        for (word, material) in [
            ("glass", Material::Glass),
            ("clear", Material::ClearGlass),
            ("ink", Material::Ink),
            ("accent", Material::Accent),
            ("black", Material::Black),
        ] {
            notch.material = word.into();
            assert_eq!(notch.style().material, material, "{word}");
        }
        for (word, style) in [
            ("notch", HudStyle::Notch),
            ("glass", HudStyle::Glass),
            ("corner", HudStyle::Corner),
            ("bar", HudStyle::Bar),
            ("island", HudStyle::Island),
            ("side", HudStyle::Side),
            ("tile", HudStyle::Tile),
            ("ring", HudStyle::Ring),
            ("equalizer", HudStyle::Equalizer),
            ("wide", HudStyle::Wide),
        ] {
            notch.hud = word.into();
            assert_eq!(notch.hud_style(), style, "{word}");
        }
        // A word nothing knows falls back to the default rather than to nothing.
        notch.material = "wood".into();
        notch.hud = "bolt".into();
        assert_eq!(notch.style().material, Material::Glass);
        assert_eq!(notch.hud_style(), HudStyle::Notch);
        notch.hud_position = "bottom-left".into();
        assert_eq!(notch.hud_position(), macos_sys::hud::HudPosition::BottomLeft);
        notch.hud_position = "somewhere".into();
        assert_eq!(notch.hud_position(), macos_sys::hud::HudPosition::Auto);
    }

    #[test]
    fn hand_edited_values_are_clamped() {
        let dir = temp_dir();
        fs::write(
            dir.join("config.json"),
            r#"{"history_limit": 1, "mouse": {"fps": 9999}}"#,
        )
        .unwrap();
        let (cfg, _) = Config::load(&dir);
        assert_eq!((cfg.history_limit, cfg.mouse.fps), (10, 240.0));
        fs::remove_dir_all(dir).unwrap();
    }
}
