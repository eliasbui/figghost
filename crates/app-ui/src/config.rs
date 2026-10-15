use crate::{audio::model::AudioConfig, capture::CaptureConfig};
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

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct NotchConfig {
    /// The notch panel, with its hover trigger. Off removes it from the screen entirely.
    pub enabled: bool,
}

impl Default for NotchConfig {
    fn default() -> Self {
        Self { enabled: true }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    /// `auto` follows the system language; or a language code such as `en` or `vi`.
    pub language: String,
    /// Open the settings window every time the app is started by hand (never when started at login).
    pub show_settings_on_launch: bool,
    pub mouse: Settings,
    pub tiling: TilingConfig,
    /// Global shortcut that opens the clipboard popup, e.g. "cmd+shift+c".
    pub hotkey: String,
    /// Send Command+V to the frontmost app after choosing an item; off means copy only.
    pub auto_paste: bool,
    /// Paste text without its formatting by default; Option+Shift+Enter then pastes it with formatting.
    pub paste_plain: bool,
    pub notch: NotchConfig,
    /// Replace the system volume indicator with one in the notch. Needs Accessibility access.
    pub volume_hud: bool,
    /// Unpinned items kept in history.
    pub history_limit: usize,
    /// Bundle identifiers whose copies are never recorded, on top of the built-in password managers.
    pub ignored_apps: Vec<String>,
    /// The per-app mixer: volumes, output devices and effects.
    pub audio: AudioConfig,
    /// Screenshots: where they go, what happens after, shortcuts.
    pub capture: CaptureConfig,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            language: "auto".into(),
            show_settings_on_launch: true,
            mouse: Settings::default(),
            tiling: TilingConfig::default(),
            hotkey: "cmd+shift+c".into(),
            auto_paste: true,
            paste_plain: false,
            notch: NotchConfig::default(),
            volume_hud: false,
            history_limit: 500,
            ignored_apps: Vec::new(),
            audio: AudioConfig::default(),
            capture: CaptureConfig::default(),
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
                cfg.audio = cfg.audio.sanitized();
                cfg.capture = cfg.capture.sanitized();
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
    let is_function_key =
        (96..=122).contains(&key_code) || [64, 79, 80, 90, 105, 106, 107, 113].contains(&key_code);
    if modifiers == 0 && !is_function_key {
        return Err("needs at least one modifier".into());
    }
    Ok(HotkeySpec {
        key_code,
        modifiers,
    })
}

fn key_code(name: &str) -> Option<u32> {
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
        assert_eq!(
            parse_hotkey("f13").unwrap(),
            HotkeySpec {
                key_code: 105,
                modifiers: 0
            }
        );
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
