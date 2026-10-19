//! A music player that floats on the desktop in Liquid Glass: the cover, the song and the controls of whatever
//! plays, in one of a few styles, anywhere on the screen and at any size.
//!
//! macOS gives no app a place on the lock screen, so this floats on every Space instead: under the windows
//! like a widget on the wallpaper, above them, or above everything, full-screen apps included. It is moved by
//! dragging it and resized by the grip in its corner; where it is, and how large, is kept in a file of its own
//! so that a drag does not have to go through the settings.

use super::AttachEnv;
use crate::{
    i18n::{self, Language},
    media::MediaFeature,
    settings::{base64, model::Fault},
};
use macos_sys::{
    MainThreadMarker, app, now_playing,
    system_keys::{self, SystemKey},
    widget_panel::{Glass, Layer, Place, WidgetPanel},
};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::{
    cell::{Cell, RefCell},
    path::{Path, PathBuf},
    rc::{Rc, Weak},
    time::Instant,
};

pub mod settings;

pub const STYLES: &[&str] = &["card", "cover", "bar", "vinyl", "minimal"];
pub const MATERIALS: &[&str] = &["glass", "clear", "dark", "accent"];
pub const LAYERS: &[&str] = &["desktop", "top", "always", "lockscreen"];

const PAGE: &str = include_str!("../../../web/widget.html");
const PLACE_FILE: &str = "musicwidget.json";
/// How often the player is looked at. The page moves the bar by itself in between.
const TICK_SECONDS: f64 = 0.5;
/// The position is sent again this often even when nothing else changed, to catch up with a seek.
const RESYNC_TICKS: u32 = 6;
/// A player takes a moment to answer a key press.
const AFTER_PRESS_SECONDS: f64 = 0.4;
const SAVE_AFTER_SECONDS: f64 = 0.6;
const BOTTOM_GAP: f64 = 48.0;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    pub enabled: bool,
    /// How it is laid out: one of `STYLES`.
    pub style: String,
    /// What the glass is like: one of `MATERIALS`.
    pub material: String,
    /// How far above the rest of the screen it is: one of `LAYERS`.
    pub layer: String,
    /// The blurred cover behind the content, lighting the glass with the colours of the music.
    pub ambient: bool,
    /// Hide it while nothing plays.
    pub hide_idle: bool,
    pub opacity: f64,
    /// The size asked for in the settings; 0 is the style's own.
    pub width: f64,
    pub height: f64,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            enabled: false,
            style: "card".into(),
            material: "glass".into(),
            layer: "top".into(),
            ambient: true,
            hide_idle: true,
            opacity: 1.0,
            width: 0.0,
            height: 0.0,
        }
    }
}

const WIDTH_RANGE: (f64, f64) = (180.0, 700.0);
const HEIGHT_RANGE: (f64, f64) = (56.0, 600.0);

/// `v` held to the range of the width (axis 0) or the height (axis 1).
pub fn clamp_size(v: f64, axis: usize) -> f64 {
    let (low, high) = if axis == 0 { WIDTH_RANGE } else { HEIGHT_RANGE };
    v.clamp(low, high)
}

impl Config {
    /// A config file can say anything; whatever is unusable is replaced by the default.
    pub fn sanitized(mut self) -> Self {
        let defaults = Self::default();
        let pick = |value: &mut String, words: &[&str], default: String| {
            if !words.contains(&value.as_str()) {
                *value = default;
            }
        };
        pick(&mut self.style, STYLES, defaults.style);
        pick(&mut self.material, MATERIALS, defaults.material);
        pick(&mut self.layer, LAYERS, defaults.layer);
        self.opacity = if self.opacity.is_finite() {
            self.opacity.clamp(0.3, 1.0)
        } else {
            defaults.opacity
        };
        for (value, axis) in [(&mut self.width, 0), (&mut self.height, 1)] {
            *value = if *value == 0.0 || !value.is_finite() {
                0.0
            } else {
                clamp_size(*value, axis)
            };
        }
        self
    }
}

/// The size a style has when nothing asks for another.
fn default_size(style: &str) -> (f64, f64) {
    match style {
        "cover" => (240.0, 240.0),
        "bar" => (380.0, 64.0),
        "vinyl" => (240.0, 310.0),
        "minimal" => (280.0, 84.0),
        _ => (340.0, 132.0),
    }
}

/// The radius of the corners; a bar is a capsule.
fn radius(style: &str, height: f64) -> f64 {
    if style == "bar" {
        height / 2.0
    } else {
        26.0
    }
}

fn glass(material: &str) -> Glass {
    match material {
        "clear" => Glass {
            clear: true,
            tint: (0.0, 0.0, 0.0, 0.2),
        },
        "dark" => Glass {
            clear: false,
            tint: (0.0, 0.0, 0.0, 0.6),
        },
        "accent" => Glass {
            clear: false,
            tint: (0.16, 0.42, 1.0, 0.44),
        },
        _ => Glass {
            clear: false,
            tint: (0.0, 0.0, 0.0, 0.28),
        },
    }
}

fn labels(language: Language) -> Value {
    match language {
        Language::English => json!({
            "idle": "Nothing is playing", "play": "Play or pause", "prev": "Previous", "next": "Next",
        }),
        Language::Vietnamese => json!({
            "idle": "Chưa phát gì", "play": "Phát hoặc tạm dừng", "prev": "Bài trước", "next": "Bài sau",
        }),
    }
}

/// Where the widget was left: kept in a file of its own, next to the settings.
fn load_place(home: &Path) -> Option<Place> {
    let text = std::fs::read_to_string(home.join(PLACE_FILE)).ok()?;
    let v: Value = serde_json::from_str(&text).ok()?;
    let n = |k: &str| v.get(k).and_then(Value::as_f64).filter(|x| x.is_finite());
    let (w, h) = (n("w")?, n("h")?);
    (w >= WIDTH_RANGE.0 && w <= WIDTH_RANGE.1 && h >= HEIGHT_RANGE.0 && h <= HEIGHT_RANGE.1)
        .then(|| Place {
            x: n("x").unwrap_or(0.0),
            y: n("y").unwrap_or(0.0),
            w,
            h,
        })
}

fn art_url(bytes: &[u8]) -> String {
    let mime = if bytes.starts_with(&[0x89, b'P', b'N', b'G']) {
        "png"
    } else {
        "jpeg"
    };
    format!("data:image/{mime};base64,{}", base64(bytes))
}

pub struct Feature {
    mtm: MainThreadMarker,
    media: Rc<MediaFeature>,
    home: PathBuf,
    config: RefCell<Config>,
    panel: RefCell<Option<Rc<WidgetPanel>>>,
    timer: RefCell<Option<app::Timer>>,
    place: Cell<Option<Place>>,
    /// The cover the page has, so that it is sent again only when it changes.
    sent_art: Cell<u64>,
    /// What the page was last told, apart from the position.
    last: RefCell<String>,
    ticks: Cell<u32>,
    save_generation: Cell<u64>,
    me: Weak<Feature>,
}

/// Starts the feature; `None` when it cannot run (no notch, so no media watcher to ask).
pub fn attach(env: &AttachEnv) -> Option<Rc<Feature>> {
    let media = Rc::clone(env.media?);
    let config = env.config.musicwidget.clone().sanitized();
    let feature = Rc::new_cyclic(|me| Feature {
        mtm: env.mtm,
        media,
        home: env.home.to_owned(),
        config: RefCell::new(config.clone()),
        panel: RefCell::default(),
        timer: RefCell::default(),
        place: Cell::new(load_place(env.home)),
        sent_art: Cell::new(0),
        last: RefCell::default(),
        ticks: Cell::new(0),
        save_generation: Cell::new(0),
        me: me.clone(),
    });
    if config.enabled {
        feature.start(false);
    }
    Some(feature)
}

impl Feature {
    /// Applies the settings: shows, changes or takes away the widget.
    pub fn configure(&self, config: &Config) -> Result<(), Fault> {
        let config = config.clone().sanitized();
        let old = self.config.replace(config.clone());
        if !config.enabled {
            self.timer.take();
            if let Some(panel) = self.panel.take() {
                panel.hide();
            }
            return Ok(());
        }
        let resized = old.style != config.style
            || (old.width, old.height) != (config.width, config.height);
        self.start(resized);
        Ok(())
    }

    /// Builds the panel when there is none, then dresses and places it. `resize` puts it at the size the settings
    /// ask for, keeping its top left corner where it is.
    fn start(&self, resize: bool) {
        let config = self.config.borrow().clone();
        let want = {
            let (dw, dh) = default_size(&config.style);
            (
                if config.width > 0.0 { config.width } else { dw },
                if config.height > 0.0 { config.height } else { dh },
            )
        };
        let mut place = self.place.get().filter(|p| WidgetPanel::on_a_screen(self.mtm, *p));
        if place.is_none() {
            let work = WidgetPanel::work_area(self.mtm);
            place = Some(Place {
                x: work.x + (work.w - want.0) / 2.0,
                y: work.y + BOTTOM_GAP,
                w: want.0,
                h: want.1,
            });
        } else if resize && let Some(p) = place.as_mut() {
            p.y += p.h - want.1;
            (p.w, p.h) = want;
        }
        let place = place.expect("a place was chosen above");
        self.place.set(Some(place));
        let existing = self.panel.borrow().clone();
        let panel = existing.unwrap_or_else(|| {
            let (message, moved) = (self.me.clone(), self.me.clone());
            let panel = WidgetPanel::new(
                self.mtm,
                place,
                PAGE,
                move |text| {
                    if let Some(feature) = message.upgrade() {
                        feature.message(&text);
                    }
                },
                move |frame| {
                    if let Some(feature) = moved.upgrade() {
                        feature.moved(frame);
                    }
                },
            );
            *self.panel.borrow_mut() = Some(Rc::clone(&panel));
            panel
        });
        panel.set_glass(glass(&config.material), radius(&config.style, place.h));
        panel.set_layer(Layer::named(&config.layer));
        panel.set_opacity(config.opacity);
        panel.set_place(place);
        self.last.borrow_mut().clear();
        self.push(true);
        if self.timer.borrow().is_none() {
            let weak = self.me.clone();
            *self.timer.borrow_mut() = Some(app::every(TICK_SECONDS, move || {
                if let Some(feature) = weak.upgrade() {
                    feature.tick();
                }
            }));
        }
    }

    fn tick(&self) {
        let ticks = self.ticks.get() + 1;
        self.ticks.set(ticks % RESYNC_TICKS);
        self.push(ticks.is_multiple_of(RESYNC_TICKS));
    }

    /// Tells the page what plays. A message goes only when something but the position changed, or when
    /// `force` asks for it.
    fn push(&self, force: bool) {
        let Some(panel) = self.panel.borrow().clone() else {
            return;
        };
        let config = self.config.borrow().clone();
        let sample = self.media.current();
        if sample.is_none() && config.hide_idle {
            panel.hide();
            return;
        }
        if !panel.is_visible() {
            panel.show();
        }
        let now = Instant::now();
        let art = sample.as_ref().and_then(|s| s.track.artwork.clone());
        let art_id = art.as_ref().map_or(0, |a| a.id);
        let height = self.place.get().map_or(0.0, |p| p.h);
        let track = sample.as_ref().map(|s| {
            json!({
                "title": s.track.title, "artist": s.track.artist, "album": s.track.album,
                "app": s.track.app_name, "playing": s.track.playing,
                "pos": s.position_ms(now), "dur": s.track.duration_ms,
            })
        });
        let signature = format!(
            "{}|{}|{}|{}|{}",
            config.style,
            config.ambient,
            radius(&config.style, height),
            art_id,
            track.as_ref().map_or(String::new(), |t| {
                format!(
                    "{}|{}|{}|{}|{}",
                    t["title"], t["artist"], t["album"], t["playing"], t["dur"]
                )
            })
        );
        if !force && *self.last.borrow() == signature {
            return;
        }
        *self.last.borrow_mut() = signature;
        let mut message = json!({
            "t": "state", "style": config.style, "ambient": config.ambient,
            "radius": radius(&config.style, height),
            "labels": labels(i18n::current()), "track": track,
        });
        if force || art_id != self.sent_art.get() {
            message["art"] = art.map_or(Value::Null, |a| Value::String(art_url(&a.bytes)));
            self.sent_art.set(art_id);
        }
        panel.host.post(&message.to_string());
    }

    /// What the page sends: its buttons, the bar, and the grip in its corner.
    fn message(&self, text: &str) {
        let Ok(m) = serde_json::from_str::<Value>(text) else {
            return;
        };
        match m["t"].as_str() {
            Some("ready") => self.push(true),
            Some("cmd") => {
                let key = match m["name"].as_str() {
                    Some("prev") => SystemKey::Previous,
                    Some("next") => SystemKey::Next,
                    _ => SystemKey::PlayPause,
                };
                system_keys::post(key);
                self.look_soon();
            }
            Some("seek") => {
                if let Some(ms) = m["ms"].as_f64()
                    && now_playing::seek(ms / 1000.0)
                {
                    self.look_soon();
                }
            }
            Some("grip") => {
                if let (Some(w), Some(h), Some(old)) =
                    (m["w"].as_f64(), m["h"].as_f64(), self.place.get())
                {
                    let (w, h) = (clamp_size(w, 0), clamp_size(h, 1));
                    let place = Place {
                        x: old.x,
                        y: old.y + old.h - h,
                        w,
                        h,
                    };
                    if let Some(panel) = self.panel.borrow().as_ref() {
                        panel.set_place(place);
                    }
                    self.moved(place);
                    self.push(true);
                }
            }
            _ => {}
        }
    }

    fn look_soon(&self) {
        let weak = self.me.clone();
        app::after(AFTER_PRESS_SECONDS, move || {
            if let Some(feature) = weak.upgrade() {
                feature.push(true);
            }
        });
    }

    /// The panel moved or changed size: remember it, and write it down once it stops changing.
    fn moved(&self, place: Place) {
        self.place.set(Some(place));
        let generation = self.save_generation.get() + 1;
        self.save_generation.set(generation);
        let weak = self.me.clone();
        app::after(SAVE_AFTER_SECONDS, move || {
            if let Some(feature) = weak.upgrade()
                && feature.save_generation.get() == generation
                && let Some(place) = feature.place.get()
            {
                let text = json!({ "x": place.x, "y": place.y, "w": place.w, "h": place.h });
                let _ = std::fs::write(feature.home.join(PLACE_FILE), text.to_string());
            }
        });
    }

    /// The size on screen, for the settings page.
    pub fn runtime(&self) -> Value {
        match self.place.get().filter(|_| self.panel.borrow().is_some()) {
            Some(p) => json!({ "width": p.w.round(), "height": p.h.round() }),
            None => Value::Null,
        }
    }

    /// The reset button: the widget goes back to the foot of the screen at the size of its style.
    pub fn action(&self, name: &str, config: &mut Config) -> Option<Result<(), Fault>> {
        if name != "musicwidget_reset" {
            return None;
        }
        config.width = 0.0;
        config.height = 0.0;
        self.place.set(None);
        let _ = std::fs::remove_file(self.home.join(PLACE_FILE));
        self.config.borrow_mut().width = 0.0;
        self.config.borrow_mut().height = 0.0;
        if config.enabled {
            self.start(true);
        }
        Some(Ok(()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn panel_slot_lazy_initialization_does_not_double_borrow() {
        let slot: RefCell<Option<String>> = RefCell::default();
        let existing = slot.borrow().clone();
        let val = existing.unwrap_or_else(|| {
            *slot.borrow_mut() = Some("ready".into());
            "ready".into()
        });
        assert_eq!(val, "ready");
        assert_eq!(*slot.borrow(), Some("ready".into()));
    }
}

