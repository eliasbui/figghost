//! Where the data lives and the settings of the beta. The keys are the ones the macOS app uses for the same things,
//! so a config can be read by either.

use serde::{Deserialize, Serialize};
use std::{
    env, fs, io,
    path::{Path, PathBuf},
};

/// `FIGGHOST_HOME` overrides the data directory (tests, a second profile).
pub fn home_dir() -> PathBuf {
    if let Some(dir) = env::var_os("FIGGHOST_HOME") {
        return PathBuf::from(dir);
    }
    let home = || {
        env::var_os("HOME")
            .or_else(|| env::var_os("USERPROFILE"))
            .map(PathBuf::from)
            .unwrap_or_default()
    };
    if cfg!(windows) {
        env::var_os("APPDATA")
            .map(PathBuf::from)
            .unwrap_or_else(|| home().join("AppData/Roaming"))
            .join("Figghost")
    } else if cfg!(target_os = "macos") {
        // Only for working on the beta on a Mac: kept apart from the history of the real app.
        home().join("Library/Application Support/Figghost Desktop")
    } else {
        env::var_os("XDG_DATA_HOME")
            .map(PathBuf::from)
            .filter(|p| p.is_absolute())
            .unwrap_or_else(|| home().join(".local/share"))
            .join("figghost")
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    /// `auto`, `en` or `vi`.
    pub language: String,
    /// `auto`, `light` or `dark`.
    pub theme: String,
    /// Opens the clipboard history from anywhere, in the form `ctrl+alt+KeyV`.
    pub hotkey: String,
    pub history_limit: usize,
    /// What happens to a copy that looks like a password or a key: `keep`, `mask` or `skip`.
    pub secrets: String,
    pub mcp: Mcp,
    pub agents: Agents,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Mcp {
    /// Off by default: what was copied can include secrets.
    pub enabled: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Agents {
    pub enabled: bool,
    pub approval_wait_seconds: u32,
}

impl Default for Agents {
    fn default() -> Self {
        Self {
            enabled: false,
            approval_wait_seconds: 30,
        }
    }
}

impl Default for Config {
    fn default() -> Self {
        Self {
            language: "auto".into(),
            theme: "auto".into(),
            hotkey: "ctrl+alt+KeyV".into(),
            history_limit: 500,
            secrets: "mask".into(),
            mcp: Mcp::default(),
            agents: Agents::default(),
        }
    }
}

pub const HISTORY_LIMITS: std::ops::RangeInclusive<usize> = 10..=100_000;
pub const WAIT_SECONDS: std::ops::RangeInclusive<u32> = 5..=100;
/// How long a masked secret stays before it is deleted.
pub const MASK_MINUTES: u32 = 60;

impl Config {
    fn path(dir: &Path) -> PathBuf {
        dir.join("config.json")
    }

    /// A missing file gives the defaults. A file that cannot be read as a config is put aside as
    /// `config.json.bad`, never overwritten, and the defaults are used.
    pub fn load(dir: &Path) -> Config {
        let path = Self::path(dir);
        let Ok(text) = fs::read_to_string(&path) else {
            return Config::default();
        };
        match serde_json::from_str::<Config>(&text) {
            Ok(config) => config.clamped(),
            Err(e) => {
                eprintln!("config: {e}; the file is kept as config.json.bad");
                let _ = fs::rename(&path, dir.join("config.json.bad"));
                Config::default()
            }
        }
    }

    fn clamped(mut self) -> Config {
        self.history_limit = self
            .history_limit
            .clamp(*HISTORY_LIMITS.start(), *HISTORY_LIMITS.end());
        self.agents.approval_wait_seconds = self
            .agents
            .approval_wait_seconds
            .clamp(*WAIT_SECONDS.start(), *WAIT_SECONDS.end());
        self
    }

    /// Keeps the keys this app does not know (the macOS app writes many more) and replaces its own.
    pub fn save(&self, dir: &Path) -> io::Result<()> {
        let path = Self::path(dir);
        let mut doc = fs::read_to_string(&path)
            .ok()
            .and_then(|t| serde_json::from_str::<serde_json::Value>(&t).ok())
            .filter(serde_json::Value::is_object)
            .unwrap_or_else(|| serde_json::json!({}));
        let ours = serde_json::to_value(self).map_err(io::Error::other)?;
        for (key, value) in ours.as_object().into_iter().flatten() {
            doc[key] = value.clone();
        }
        let text = serde_json::to_string_pretty(&doc).map_err(io::Error::other)?;
        core_notes::write_atomically(&path, &text)
    }

    /// Applies one change from the settings page. Gives an error for a value that is not allowed.
    pub fn set(&mut self, key: &str, value: &serde_json::Value) -> Result<(), String> {
        let text = || value.as_str().map(str::to_owned).ok_or("a text is needed");
        let one_of = |words: &[&str]| {
            let word = text()?;
            if words.contains(&word.as_str()) {
                Ok(word)
            } else {
                Err(format!("{word} is not one of {}", words.join(", ")))
            }
        };
        match key {
            "language" => self.language = one_of(&["auto", "en", "vi"])?,
            "theme" => self.theme = one_of(&["auto", "light", "dark"])?,
            "secrets" => self.secrets = one_of(&["keep", "mask", "skip"])?,
            "hotkey" => self.hotkey = text()?,
            "history_limit" => {
                let n = value.as_u64().ok_or("a number is needed")? as usize;
                if !HISTORY_LIMITS.contains(&n) {
                    return Err("out of range".into());
                }
                self.history_limit = n;
            }
            "mcp.enabled" => self.mcp.enabled = value.as_bool().ok_or("true or false")?,
            "agents.enabled" => self.agents.enabled = value.as_bool().ok_or("true or false")?,
            "agents.approval_wait_seconds" => {
                let n = value.as_u64().ok_or("a number is needed")? as u32;
                if !WAIT_SECONDS.contains(&n) {
                    return Err("out of range".into());
                }
                self.agents.approval_wait_seconds = n;
            }
            _ => return Err(format!("unknown setting {key}")),
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn scratch(tag: &str) -> PathBuf {
        let dir = env::temp_dir().join(format!("figghost-desktop-{tag}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn saving_keeps_the_keys_of_the_macos_app() {
        let dir = scratch("keep");
        fs::write(
            dir.join("config.json"),
            r#"{"notch": {"enabled": true}, "language": "en"}"#,
        )
        .unwrap();
        let mut config = Config::load(&dir);
        assert_eq!(config.language, "en");
        config.set("language", &json!("vi")).unwrap();
        config.save(&dir).unwrap();
        let doc: serde_json::Value =
            serde_json::from_str(&fs::read_to_string(dir.join("config.json")).unwrap()).unwrap();
        assert_eq!(doc["notch"]["enabled"], true);
        assert_eq!(Config::load(&dir).language, "vi");
    }

    #[test]
    fn a_broken_file_is_put_aside_and_the_defaults_are_used() {
        let dir = scratch("bad");
        fs::write(dir.join("config.json"), "{ not json").unwrap();
        assert_eq!(Config::load(&dir), Config::default());
        assert!(dir.join("config.json.bad").exists());
    }

    #[test]
    fn values_out_of_range_are_refused_or_clamped() {
        let mut config = Config::default();
        assert!(config.set("history_limit", &json!(5)).is_err());
        assert!(config.set("theme", &json!("blue")).is_err());
        assert!(config.set("nope", &json!(1)).is_err());
        assert!(config.set("agents.approval_wait_seconds", &json!(40)).is_ok());
        let loaded: Config = serde_json::from_value(json!({"history_limit": 1})).unwrap();
        assert_eq!(loaded.clamped().history_limit, 10);
    }
}
