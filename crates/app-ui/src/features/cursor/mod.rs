//! Custom mouse cursor: the arrow and the hand over links are drawn from a picture of the user's choosing, one
//! of the drawn rainbow arrow, a character of the clipboard mascot, or a file (a GIF, an animated PNG or any
//! picture). The window server plays the animation, so nothing runs in the app while the pointer moves. The
//! text cursor stays the system's: a text caret must stay precise.
//!
//! The pictures are shown over every other app. The app's own windows keep the system pointer while the app
//! is in front, which is how macOS lets a process draw its own cursor.

use super::{AttachEnv, mascot::Character};
use crate::{i18n::Language, settings::model::Fault};
use macos_sys::{
    MainThreadMarker,
    cursor_ui::{self, Frames, Kind},
};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::{
    cell::{Cell, RefCell},
    path::Path,
    rc::Rc,
};

pub mod settings;

pub const NEON: &str = "neon";
pub const CUSTOM: &str = "custom";
pub const DEFAULT_SIZE: f64 = 32.0;
pub const SIZE_RANGE: (f64, f64) = (16.0, 96.0);
pub const SPEED_RANGE: (f64, f64) = (0.25, 3.0);
/// Where the point that clicks is on a picture of the user's: at its top left, as on an arrow, or in the middle.
pub const TIPS: [&str; 2] = ["top", "centre"];
/// A picture file larger than this is refused (the system also refuses very large pictures).
const MAX_FILE_BYTES: u64 = 16 << 20;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    pub enabled: bool,
    /// `neon`, `custom` or the name of a character of the clipboard mascot.
    pub theme: String,
    /// Points across.
    pub size: f64,
    /// How fast the animation plays, 1 is as drawn.
    pub speed: f64,
    /// The file of the `custom` theme. Only the choose button of the page writes it.
    pub custom_path: String,
    /// One of `TIPS`, for the `custom` theme.
    pub tip: String,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            enabled: false,
            theme: NEON.into(),
            size: DEFAULT_SIZE,
            speed: 1.0,
            custom_path: String::new(),
            tip: TIPS[0].into(),
        }
    }
}

pub fn is_theme(name: &str) -> bool {
    name == NEON || name == CUSTOM || Character::parse(name).is_some()
}

impl Config {
    /// A config file can say anything; whatever is unusable is replaced by the default.
    pub fn sanitized(mut self) -> Self {
        let defaults = Self::default();
        if !is_theme(&self.theme) {
            self.theme = defaults.theme;
        }
        self.size = clamp(self.size, SIZE_RANGE, defaults.size);
        self.speed = clamp(self.speed, SPEED_RANGE, defaults.speed);
        if !TIPS.contains(&self.tip.as_str()) {
            self.tip = defaults.tip;
        }
        if self.custom_path.len() > 1024 {
            self.custom_path.clear();
        }
        self
    }
}

pub fn clamp(value: f64, (low, high): (f64, f64), default: f64) -> f64 {
    if value.is_finite() {
        value.clamp(low, high)
    } else {
        default
    }
}

/// What a picture plays on each cursor: which frames in which order, how long each stays at normal speed, and
/// where the point that clicks is (fractions of the picture from its top left).
#[derive(Debug, PartialEq)]
struct Play {
    arrow: Vec<usize>,
    link: Vec<usize>,
    arrow_seconds: f64,
    link_seconds: f64,
    hotspot: (f64, f64),
}

/// A character stands (frame 0) and blinks now and then (frame 1); over a link it beams and then looks proud
/// (frames 7 and 8 of its sheet).
fn play(theme: &str, tip: &str, frames: usize) -> Play {
    let inside = |order: Vec<usize>| {
        let kept: Vec<usize> = order.into_iter().filter(|&i| i < frames).collect();
        if kept.is_empty() { vec![0] } else { kept }
    };
    let all = || (0..frames).collect::<Vec<_>>();
    if theme == NEON {
        return Play {
            arrow: all(),
            link: all(),
            arrow_seconds: 0.08,
            link_seconds: 0.08,
            hotspot: (0.11, 0.11),
        };
    }
    if theme == CUSTOM {
        let hotspot = if tip == "centre" {
            (0.5, 0.5)
        } else {
            (0.0, 0.0)
        };
        return Play {
            arrow: all(),
            link: all(),
            arrow_seconds: 0.1,
            link_seconds: 0.1,
            hotspot,
        };
    }
    let mut stand = vec![0; 9];
    stand.push(1);
    Play {
        arrow: inside(stand),
        link: inside(vec![7, 8]),
        arrow_seconds: 0.3,
        link_seconds: 0.3,
        // Top middle, the crown of the head: the point that clicks stays in view above the character.
        hotspot: (0.5, 0.12),
    }
}

pub struct Feature {
    mtm: MainThreadMarker,
    config: RefCell<Config>,
    /// The decoded pictures of the theme in use, by what they were made from, so that dragging the size does not
    /// read the file again at every step.
    pictures: RefCell<Option<(String, Rc<Frames>)>>,
    /// The cursors are replaced now.
    applied: Cell<bool>,
    /// The file of the `custom` theme could not be read.
    unreadable: Cell<bool>,
}

/// Starts the feature; the cursors are replaced when it is on.
pub fn attach(env: &AttachEnv) -> Option<Rc<Feature>> {
    let config = env.config.cursor.clone().sanitized();
    let feature = Rc::new(Feature {
        mtm: env.mtm,
        config: RefCell::new(Config::default()),
        pictures: RefCell::new(None),
        applied: Cell::new(false),
        unreadable: Cell::new(false),
    });
    if let Err(fault) = feature.configure(&config) {
        eprintln!("cursor: {}", fault.detail);
    }
    Some(feature)
}

fn choose_prompt(language: Language) -> &'static str {
    match language {
        Language::English => {
            "Choose a picture for the cursor: a GIF, an animated PNG or any image."
        }
        Language::Vietnamese => {
            "Ch\u{1ecd}n h\u{ec}nh cho con tr\u{1ecf}: GIF, PNG \u{111}\u{1ed9}ng ho\u{1eb7}c \u{1ea3}nh b\u{1ea5}t k\u{1ef3}."
        }
    }
}

fn fault(detail: &str) -> Fault {
    Fault::new("cursor_failed", detail)
}

impl Feature {
    /// Applies the settings: replaces the cursors, or gives them back when the feature has just been turned off.
    pub fn configure(&self, config: &Config) -> Result<(), Fault> {
        let was_on = self.config.replace(config.clone()).enabled;
        self.unreadable.set(false);
        if !config.enabled {
            if was_on || self.applied.get() {
                cursor_ui::restore();
            }
            self.applied.set(false);
            return Ok(());
        }
        let result = self.picture(config).and_then(|frames| {
            let shape = play(&config.theme, &config.tip, frames.len());
            let seconds = |base: f64| base / config.speed;
            let arrow = cursor_ui::apply(
                Kind::Arrow,
                &frames,
                &shape.arrow,
                config.size,
                shape.hotspot,
                seconds(shape.arrow_seconds),
            );
            let link = cursor_ui::apply(
                Kind::Link,
                &frames,
                &shape.link,
                config.size,
                shape.hotspot,
                seconds(shape.link_seconds),
            );
            if arrow && link {
                Ok(())
            } else {
                Err(fault("the system refused to change the cursor"))
            }
        });
        self.applied.set(result.is_ok());
        if result.is_err() && was_on {
            cursor_ui::restore();
        }
        result
    }

    /// The pictures the config asks for, decoded now or kept from the last time.
    fn picture(&self, config: &Config) -> Result<Rc<Frames>, Fault> {
        let key = if config.theme == CUSTOM {
            format!("{CUSTOM}:{}", config.custom_path)
        } else {
            config.theme.clone()
        };
        if let Some((kept, frames)) = &*self.pictures.borrow()
            && *kept == key
        {
            return Ok(Rc::clone(frames));
        }
        let frames = if config.theme == NEON {
            Some(Frames::neon())
        } else if let Some(character) = Character::parse(&config.theme) {
            Frames::from_sheet(character.png())
        } else {
            match read_picture(Path::new(&config.custom_path)) {
                Some(frames) => Some(frames),
                None => {
                    self.unreadable.set(true);
                    return Err(fault("the picture cannot be read"));
                }
            }
        };
        let frames = Rc::new(
            frames
                .filter(|f| !f.is_empty())
                .ok_or_else(|| fault("no picture"))?,
        );
        *self.pictures.borrow_mut() = Some((key, Rc::clone(&frames)));
        Ok(frames)
    }

    pub fn runtime(&self) -> Value {
        json!({ "applied": self.applied.get(), "unreadable": self.unreadable.get() })
    }

    /// The choose button of the settings page: the user picks a picture, which becomes the `custom` theme.
    pub fn action(&self, name: &str, config: &mut Config) -> Option<Result<(), Fault>> {
        if name != "cursor_choose" {
            return None;
        }
        let prompt = choose_prompt(crate::i18n::current());
        let Some(path) = cursor_ui::choose_file(self.mtm, prompt) else {
            return Some(Ok(()));
        };
        config.custom_path = path.to_string_lossy().into_owned();
        config.theme = CUSTOM.into();
        let result = if config.enabled {
            self.configure(config)
        } else {
            Ok(())
        };
        Some(result)
    }
}

/// A picture file the system can read, at most `MAX_FILE_BYTES` large.
fn read_picture(path: &Path) -> Option<Frames> {
    let size = std::fs::metadata(path).ok()?.len();
    if size == 0 || size > MAX_FILE_BYTES {
        return None;
    }
    Frames::from_file(&std::fs::read(path).ok()?)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_config_file_cannot_hold_an_unusable_value() {
        let config = Config {
            theme: "klingon".into(),
            size: f64::NAN,
            speed: 99.0,
            tip: "left".into(),
            custom_path: "x".repeat(2000),
            ..Config::default()
        }
        .sanitized();
        assert_eq!(
            config,
            Config {
                speed: SPEED_RANGE.1,
                ..Config::default()
            }
        );
        let tiny = Config {
            size: 2.0,
            ..Config::default()
        }
        .sanitized();
        assert_eq!(tiny.size, SIZE_RANGE.0);
    }

    #[test]
    fn every_theme_of_the_page_is_known() {
        assert!(is_theme("neon") && is_theme("custom"));
        for c in Character::ALL {
            assert!(is_theme(c.as_str()), "{}", c.as_str());
        }
        assert!(!is_theme("") && !is_theme("Neon"));
    }

    #[test]
    fn a_character_blinks_on_the_arrow_and_beams_on_the_hand() {
        let shape = play("ghost", "top", 12);
        assert_eq!(shape.arrow.len(), 10);
        assert_eq!(shape.arrow.iter().filter(|&&i| i == 1).count(), 1);
        assert_eq!(shape.link, vec![7, 8]);
        // A sheet too short for those frames still plays what it has.
        let short = play("ghost", "top", 2);
        assert!(short.arrow.iter().all(|&i| i < 2));
        assert_eq!(short.link, vec![0]);
    }

    #[test]
    fn the_other_themes_play_every_frame_and_the_tip_follows_the_choice() {
        assert_eq!(play("neon", "top", 12).arrow, (0..12).collect::<Vec<_>>());
        let still = play("custom", "top", 1);
        assert_eq!((still.arrow, still.hotspot), (vec![0], (0.0, 0.0)));
        assert_eq!(play("custom", "centre", 3).hotspot, (0.5, 0.5));
    }

    #[test]
    fn every_character_sheet_can_be_a_cursor() {
        for c in Character::ALL {
            let frames = Frames::from_sheet(c.png()).expect("a sheet");
            assert_eq!(frames.len(), 12, "{}", c.as_str());
        }
    }

    #[test]
    fn a_missing_or_empty_file_is_no_picture() {
        assert!(read_picture(Path::new("/nonexistent/cursor.gif")).is_none());
        let dir = std::env::temp_dir().join(format!("figghost-cursor-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let empty = dir.join("empty.gif");
        std::fs::write(&empty, b"").unwrap();
        let text = dir.join("text.png");
        std::fs::write(&text, b"not a picture").unwrap();
        assert!(read_picture(&empty).is_none());
        assert!(read_picture(&text).is_none());
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
