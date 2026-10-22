//! Live islands: a small notice comes out of the closed notch for a moment when something happens on the Mac that
//! is worth a glance. Caps Lock turns on or off, the charger is plugged in, the battery is full or low, a drive
//! arrives or leaves, a download finishes, an app starts to use the camera or the microphone. Each can be
//! switched off on its own, and each can be shown as a sample from the settings.
//!
//! Nothing here reads what is on the screen or what is typed. The facts come from the power sources, the flags of
//! the modifier keys, the list of drives, the names and sizes of the files in the Downloads folder, and the
//! "in use" flags of the camera and microphone devices, which say that something is recording and not what.

use super::AttachEnv;
use crate::{
    i18n::{self, Language},
    menu::symbols,
    settings::model::Fault,
    trace,
};
use logic::{
    CapsWatcher, DownloadWatcher, Drives, PowerEvent, PowerWatcher, PrivacyWatcher, Reading,
};
use macos_sys::{
    app, camera, modifiers,
    notch::{Activity, Detail, Ear, Notch},
    power,
    status_item::{Entry, Item},
    style::Tone,
    volume, volumes,
};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::{
    cell::{Cell, RefCell},
    collections::HashMap,
    env,
    path::PathBuf,
    rc::{Rc, Weak},
};

pub mod logic;
pub mod settings;
mod text;

pub const FULL_RANGE: (u8, u8) = (80, 100);
pub const LOW_RANGE: (u8, u8) = (5, 30);
pub const SECONDS_RANGE: (f64, f64) = (2.0, 10.0);

/// How often each fact is looked at, in seconds. Caps Lock has to answer within a blink; a battery changes by
/// the minute.
const CAPS_SECONDS: f64 = 0.3;
const FILES_SECONDS: f64 = 2.0;
const PRIVACY_SECONDS: f64 = 1.5;
const POWER_SECONDS: f64 = 5.0;
/// A new island is brief for Caps Lock, which the person just did.
const CAPS_SHOW_SECONDS: f64 = 1.4;
/// The longest name of a file shown in an island.
const NAME_CHARS: usize = 30;
/// Up to this many drives have a place in the menu bar panel.
const EJECT_SLOTS: usize = 4;

/// Above the music and a Bluetooth device, below a coding agent that waits for an answer.
const PRIORITY_CAPS: u8 = 29;
const PRIORITY_POWER: u8 = 28;
const PRIORITY_DRIVE: u8 = 27;
const PRIORITY_DOWNLOAD: u8 = 26;
const PRIORITY_PRIVACY: u8 = 24;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    pub enabled: bool,
    pub caps_lock: bool,
    /// Plugging in, unplugging, full and low.
    pub power: bool,
    pub drives: bool,
    pub downloads: bool,
    /// An app using the camera.
    pub camera: bool,
    /// An app using the microphone.
    pub microphone: bool,
    /// The one switch of both before they had one each; read from older files and never written back.
    #[serde(skip_serializing)]
    pub privacy: Option<bool>,
    /// The charge, in percent, that is announced as full while plugged in.
    pub full_at: u8,
    /// The charge, in percent, at and under which a Mac on battery is announced as low.
    pub low_at: u8,
    /// How long a notice stays.
    pub seconds: f64,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            enabled: false,
            caps_lock: true,
            power: true,
            drives: true,
            downloads: true,
            camera: true,
            microphone: true,
            privacy: None,
            full_at: 100,
            low_at: 20,
            seconds: 3.6,
        }
    }
}

impl Config {
    /// A config file can say anything; whatever is unusable is replaced by the default.
    pub fn sanitized(mut self) -> Self {
        let defaults = Self::default();
        if self.privacy.take() == Some(false) {
            self.camera = false;
            self.microphone = false;
        }
        self.full_at = self.full_at.clamp(FULL_RANGE.0, FULL_RANGE.1);
        self.low_at = self.low_at.clamp(LOW_RANGE.0, LOW_RANGE.1);
        self.seconds = if self.seconds.is_finite() {
            self.seconds.clamp(SECONDS_RANGE.0, SECONDS_RANGE.1)
        } else {
            defaults.seconds
        };
        self
    }
}

/// The folder watched for downloads. `FIGGHOST_DOWNLOADS` names another, for the checks that run the real program.
fn downloads_folder() -> PathBuf {
    env::var_os("FIGGHOST_DOWNLOADS").map_or_else(
        || {
            env::var_os("HOME")
                .map(PathBuf::from)
                .unwrap_or_default()
                .join("Downloads")
        },
        PathBuf::from,
    )
}

/// The (name, size) of each file in the folder. A folder that cannot be read is an empty one.
fn scan(folder: &std::path::Path) -> Vec<(String, u64)> {
    let Ok(entries) = std::fs::read_dir(folder) else {
        return Vec::new();
    };
    entries
        .flatten()
        .take(5000)
        .filter_map(|entry| {
            let meta = entry.metadata().ok()?;
            meta.is_file()
                .then(|| (entry.file_name().to_string_lossy().into_owned(), meta.len()))
        })
        .collect()
}

thread_local! {
    /// Which showing of each slot is the latest, so that an earlier one cannot clear a later one.
    static SERIALS: RefCell<HashMap<&'static str, u64>> = RefCell::default();
    /// Never used twice, so that a showing that was taken away cannot be mistaken for a later one.
    static NEXT_SERIAL: Cell<u64> = const { Cell::new(0) };
}

/// Puts `activity` on the notch in `slot`. With `seconds` it goes by itself after that long, unless a newer one
/// has taken its place. Every feature that speaks on the notch goes through here.
pub(crate) fn show_for(
    notch: &Rc<Notch>,
    slot: &'static str,
    activity: Activity,
    seconds: Option<f64>,
) {
    trace::log(&format!("islands: {slot} shown"));
    let serial = NEXT_SERIAL.with(|n| {
        n.set(n.get() + 1);
        n.get()
    });
    SERIALS.with(|s| s.borrow_mut().insert(slot, serial));
    notch.set_activity(slot, Some(activity));
    if let Some(seconds) = seconds {
        let weak = Rc::downgrade(notch);
        app::after(seconds, move || {
            let latest = SERIALS.with(|s| s.borrow().get(slot) == Some(&serial));
            if latest && let Some(notch) = weak.upgrade() {
                notch.set_activity(slot, None);
                SERIALS.with(|s| s.borrow_mut().remove(slot));
            }
        });
    }
}

/// Takes `slot` off the notch.
pub(crate) fn clear_slot(notch: &Notch, slot: &'static str) {
    SERIALS.with(|s| s.borrow_mut().remove(slot));
    notch.set_activity(slot, None);
}

pub struct Feature {
    notch: Weak<Notch>,
    folder: PathBuf,
    config: RefCell<Config>,
    caps: RefCell<CapsWatcher>,
    power: RefCell<PowerWatcher>,
    downloads: RefCell<DownloadWatcher>,
    privacy: RefCell<PrivacyWatcher>,
    /// The drives that are announced when they leave.
    drives: RefCell<Drives>,
    /// The drives the menu showed when it was last opened, so that a click ejects the one that was named.
    menu: RefCell<Vec<volumes::Volume>>,
    timers: RefCell<Vec<app::Timer>>,
    observer: RefCell<Option<volumes::Observer>>,
    me: Weak<Feature>,
}

/// Starts the feature; `None` without a notch, which is where the islands come out.
pub fn attach(env: &AttachEnv) -> Option<Rc<Feature>> {
    let notch = env.notch?;
    let config = env.config.islands.clone().sanitized();
    let feature = Rc::new_cyclic(|me| Feature {
        notch: Rc::downgrade(notch),
        folder: downloads_folder(),
        config: RefCell::new(config.clone()),
        caps: RefCell::default(),
        power: RefCell::default(),
        downloads: RefCell::default(),
        privacy: RefCell::default(),
        drives: RefCell::default(),
        menu: RefCell::default(),
        timers: RefCell::default(),
        observer: RefCell::default(),
        me: me.clone(),
    });
    if config.enabled {
        feature.start();
    }
    Some(feature)
}

impl Feature {
    /// Applies the settings: starts what is wanted and stops what is not.
    pub fn configure(&self, config: &Config) -> Result<(), Fault> {
        let config = config.clone().sanitized();
        *self.config.borrow_mut() = config.clone();
        self.stop();
        if config.enabled {
            self.start();
        }
        Ok(())
    }

    fn stop(&self) {
        self.timers.borrow_mut().clear();
        self.observer.take();
        if let Some(notch) = self.notch.upgrade() {
            for slot in [
                SLOT_CAPS,
                SLOT_POWER,
                SLOT_DRIVE,
                SLOT_DOWNLOAD,
                SLOT_PRIVACY,
            ] {
                clear_slot(&notch, slot);
            }
        }
    }

    fn start(&self) {
        let config = self.config.borrow().clone();
        self.caps.borrow_mut().update(modifiers::caps_lock_on());
        self.power.borrow_mut().reset();
        self.downloads.borrow_mut().reset();
        self.privacy.borrow_mut().reset();
        let mut timers = Vec::new();
        let every = |seconds: f64, tick: fn(&Feature)| {
            let me = self.me.clone();
            app::every(seconds, move || {
                if let Some(feature) = me.upgrade() {
                    tick(&feature);
                }
            })
        };
        if config.caps_lock {
            timers.push(every(CAPS_SECONDS, Feature::tick_caps));
        }
        if config.power {
            self.tick_power();
            timers.push(every(POWER_SECONDS, Feature::tick_power));
        }
        if config.downloads {
            self.tick_downloads();
            timers.push(every(FILES_SECONDS, Feature::tick_downloads));
        }
        if config.camera || config.microphone {
            self.tick_privacy();
            timers.push(every(PRIVACY_SECONDS, Feature::tick_privacy));
        }
        *self.timers.borrow_mut() = timers;
        if config.drives {
            self.drives
                .borrow_mut()
                .reset(volumes::external().into_iter().map(|v| v.path));
            let me = self.me.clone();
            *self.observer.borrow_mut() = Some(volumes::observe(move |event| {
                if let Some(feature) = me.upgrade() {
                    feature.drive_event(event);
                }
            }));
        }
    }

    fn language(&self) -> Language {
        i18n::current()
    }

    fn show(&self, slot: &'static str, activity: Activity, seconds: Option<f64>) {
        if let Some(notch) = self.notch.upgrade() {
            show_for(&notch, slot, activity, seconds);
        }
    }

    fn clear(&self, slot: &'static str) {
        if let Some(notch) = self.notch.upgrade() {
            clear_slot(&notch, slot);
        }
    }

    fn seconds(&self) -> f64 {
        self.config.borrow().seconds
    }

    // -- Caps Lock --------------------------------------------------------------------------------------------

    fn tick_caps(&self) {
        let change = self.caps.borrow_mut().update(modifiers::caps_lock_on());
        if let Some(on) = change {
            self.caps_island(on);
        }
    }

    fn caps_island(&self, on: bool) {
        let language = self.language();
        let activity = Activity {
            left: Ear::Symbol {
                name: if on { "capslock.fill" } else { "capslock" },
                tone: if on { Tone::Orange } else { Tone::Gray },
                pulse: false,
            },
            right: Ear::Empty,
            detail: Some(Detail {
                title: language.island_caps_title().to_owned(),
                subtitle: language.island_caps_state(on).to_owned(),
            }),
            page: None,
            priority: PRIORITY_CAPS,
        };
        self.show(SLOT_CAPS, activity, Some(CAPS_SHOW_SECONDS));
    }

    // -- battery ----------------------------------------------------------------------------------------------

    fn tick_power(&self) {
        let Some(battery) = power::battery() else {
            return;
        };
        let (full_at, low_at) = {
            let config = self.config.borrow();
            (config.full_at, config.low_at)
        };
        let events = self.power.borrow_mut().update(
            Reading {
                percent: battery.percent,
                on_ac: battery.on_ac,
                charged: battery.charged,
            },
            full_at,
            low_at,
        );
        // Several at once (unplugged at a low charge) are shown in turn, the one that matters most last.
        if let Some(event) = events.last() {
            self.power_island(*event);
        }
    }

    fn power_island(&self, event: PowerEvent) {
        let language = self.language();
        let (left, right, title, subtitle) = match event {
            PowerEvent::PluggedIn(percent) => (
                Ear::Symbol {
                    name: "powerplug.fill",
                    tone: Tone::Green,
                    pulse: true,
                },
                Ear::Text {
                    text: language.island_percent(percent),
                    tone: Tone::Green,
                },
                language.island_plugged().to_owned(),
                String::new(),
            ),
            PowerEvent::Unplugged(percent) => (
                Ear::Symbol {
                    name: logic::battery_symbol(percent),
                    tone: Tone::Gray,
                    pulse: false,
                },
                Ear::Text {
                    text: language.island_percent(percent),
                    tone: Tone::Gray,
                },
                language.island_unplugged().to_owned(),
                String::new(),
            ),
            PowerEvent::Full(percent) => (
                Ear::Symbol {
                    name: "battery.100",
                    tone: Tone::Green,
                    pulse: false,
                },
                Ear::Text {
                    text: language.island_percent(percent),
                    tone: Tone::Green,
                },
                language.island_full().to_owned(),
                language.island_full_hint(percent),
            ),
            PowerEvent::Low(percent) => (
                Ear::Symbol {
                    name: logic::battery_symbol(percent),
                    tone: Tone::Red,
                    pulse: true,
                },
                Ear::Text {
                    text: language.island_percent(percent),
                    tone: Tone::Red,
                },
                language.island_low().to_owned(),
                language.island_low_hint(percent),
            ),
        };
        let detail = Some(Detail { title, subtitle });
        self.show(
            SLOT_POWER,
            Activity {
                left,
                right,
                detail,
                page: None,
                priority: PRIORITY_POWER,
            },
            Some(self.seconds()),
        );
    }

    // -- drives -----------------------------------------------------------------------------------------------

    /// A drive arrived or left, as the system says. Only a drive that is known to be one that can be ejected is told of
    /// leaving: disk images, backup volumes and shares come and go, and what this app has just ejected was told already.
    fn drive_event(&self, event: volumes::Event) {
        match &event {
            volumes::Event::Mounted(volume) => {
                self.drives.borrow_mut().arrived(volume.path.clone())
            }
            volumes::Event::Unmounted(volume) => {
                if !self.drives.borrow_mut().left(&volume.path) {
                    return;
                }
            }
        }
        self.drive(&event, false);
    }

    /// `ejected` is true when this app has just ejected the drive, which is when it is safe to unplug.
    fn drive(&self, event: &volumes::Event, ejected: bool) {
        let language = self.language();
        let (name, symbol, tone, state, mark) = match event {
            volumes::Event::Mounted(volume) => (
                volume.name.clone(),
                "externaldrive.fill",
                Tone::Blue,
                language.island_drive_connected(),
                "checkmark.circle.fill",
            ),
            volumes::Event::Unmounted(volume) => (
                volume.name.clone(),
                "externaldrive",
                Tone::Gray,
                if ejected {
                    language.island_drive_ejected()
                } else {
                    language.island_drive_removed()
                },
                if ejected {
                    "checkmark.circle.fill"
                } else {
                    "eject.fill"
                },
            ),
        };
        self.show(
            SLOT_DRIVE,
            Activity {
                left: Ear::Symbol {
                    name: symbol,
                    tone,
                    pulse: matches!(event, volumes::Event::Mounted(_)),
                },
                right: Ear::Symbol {
                    name: mark,
                    tone: if matches!(event, volumes::Event::Mounted(_)) || ejected {
                        Tone::Green
                    } else {
                        Tone::Gray
                    },
                    pulse: false,
                },
                detail: Some(Detail {
                    title: logic::shorten(&name, NAME_CHARS),
                    subtitle: state.to_owned(),
                }),
                page: None,
                priority: PRIORITY_DRIVE,
            },
            Some(self.seconds()),
        );
    }

    fn eject(&self, volume: &volumes::Volume) {
        match volumes::eject(&volume.path) {
            Ok(()) => {
                // Told here, so the system's notice of the same departure that follows is not told again.
                self.drives.borrow_mut().left(&volume.path);
                self.drive(&volumes::Event::Unmounted(volume.clone()), true);
            }
            Err(error) => {
                eprintln!("islands: cannot eject {}: {error}", volume.path);
                self.show(
                    SLOT_DRIVE,
                    Activity {
                        left: Ear::Symbol {
                            name: "exclamationmark.triangle.fill",
                            tone: Tone::Orange,
                            pulse: false,
                        },
                        right: Ear::Empty,
                        detail: Some(Detail {
                            title: logic::shorten(&volume.name, NAME_CHARS),
                            subtitle: self.language().island_eject_failed().to_owned(),
                        }),
                        page: None,
                        priority: PRIORITY_DRIVE,
                    },
                    Some(self.seconds()),
                );
            }
        }
    }

    // -- downloads --------------------------------------------------------------------------------------------

    fn tick_downloads(&self) {
        let finished = self.downloads.borrow_mut().update(&scan(&self.folder));
        let Some(last) = finished.last() else {
            return;
        };
        self.download_island(&last.name, last.bytes, finished.len() - 1);
    }

    fn download_island(&self, name: &str, bytes: u64, extra: usize) {
        let language = self.language();
        let subtitle = if extra > 0 {
            language.island_download_more(extra)
        } else {
            language.island_download_done(&logic::human_size(bytes))
        };
        self.show(
            SLOT_DOWNLOAD,
            Activity {
                left: Ear::Symbol {
                    name: "arrow.down.circle.fill",
                    tone: Tone::Blue,
                    pulse: false,
                },
                right: Ear::Symbol {
                    name: "checkmark.circle.fill",
                    tone: Tone::Green,
                    pulse: false,
                },
                detail: Some(Detail {
                    title: logic::shorten(name, NAME_CHARS),
                    subtitle,
                }),
                page: None,
                priority: PRIORITY_DOWNLOAD,
            },
            Some(self.seconds()),
        );
    }

    // -- camera and microphone --------------------------------------------------------------------------------

    fn tick_privacy(&self) {
        let (show_camera, show_mic) = {
            let config = self.config.borrow();
            (config.camera, config.microphone)
        };
        // A device whose notice is off is not looked at, so its use never shows.
        let change = self.privacy.borrow_mut().update(
            show_camera && camera::in_use(),
            show_mic && volume::microphone_in_use(),
        );
        if let Some((camera, mic)) = change {
            self.privacy_island(camera, mic, false);
        }
    }

    /// Stays while either is in use; `sample` makes it go by itself, for the button of the settings page.
    fn privacy_island(&self, camera: bool, mic: bool, sample: bool) {
        if !camera && !mic {
            self.clear(SLOT_PRIVACY);
            return;
        }
        let language = self.language();
        let symbol = |name: &'static str, tone: Tone| Ear::Symbol {
            name,
            tone,
            pulse: true,
        };
        let (left, right) = match (camera, mic) {
            (true, true) => (
                symbol("video.fill", Tone::Green),
                symbol("mic.fill", Tone::Orange),
            ),
            (true, false) => (symbol("video.fill", Tone::Green), Ear::Empty),
            _ => (symbol("mic.fill", Tone::Orange), Ear::Empty),
        };
        let title = match (camera, mic) {
            (true, true) => language.island_both(),
            (true, false) => language.island_camera(),
            _ => language.island_mic(),
        };
        self.show(
            SLOT_PRIVACY,
            Activity {
                left,
                right,
                detail: Some(Detail {
                    title: title.to_owned(),
                    subtitle: String::new(),
                }),
                page: None,
                priority: PRIORITY_PRIVACY,
            },
            sample.then(|| self.seconds()),
        );
    }

    // -- the settings page and the menu -----------------------------------------------------------------------

    /// Facts for the page: whether this Mac has a battery to speak of.
    pub fn runtime(&self) -> Value {
        json!({ "has_battery": power::battery().is_some() })
    }

    /// The buttons of the settings page: a sample of each island, whether or not the feature is switched on.
    pub fn action(&self, name: &str, _config: &mut Config) -> Option<Result<(), Fault>> {
        let sample = name.strip_prefix("islands_sample:")?;
        let language = self.language();
        match sample {
            "caps" => self.caps_island(true),
            "plug" => self.power_island(PowerEvent::PluggedIn(64)),
            "full" => self.power_island(PowerEvent::Full(100)),
            "low" => self.power_island(PowerEvent::Low(15)),
            "drive" => self.drive(
                &volumes::Event::Mounted(volumes::Volume {
                    name: language.island_sample_drive().to_owned(),
                    path: "/Volumes/Sample".into(),
                }),
                false,
            ),
            "download" => self.download_island("Figghost-1.5.0.dmg", 48_300_000, 0),
            "camera" | "mic" => {
                self.privacy_island(sample == "camera", sample == "mic", true);
                // When the sample has gone by, what is really going on comes back: a camera that is in use must not
                // lose its island to a button.
                let me = self.me.clone();
                app::after(self.seconds() + 0.3, move || {
                    if let Some(feature) = me.upgrade() {
                        let config = feature.config.borrow().clone();
                        if config.enabled && (config.camera || config.microphone) {
                            let (camera, mic) = feature.privacy.borrow().current();
                            feature.privacy_island(camera, mic, false);
                        }
                    }
                });
            }
            _ => return None,
        }
        Some(Ok(()))
    }

    /// "Eject" for each drive that is plugged in, in the menu bar panel. The titles and what is visible follow the
    /// drives each time the menu opens.
    pub fn menu_entries(this: &Rc<Self>) -> Vec<Entry> {
        (0..EJECT_SLOTS)
            .map(|slot| {
                let (act, shown, title) = (this.me.clone(), this.me.clone(), this.me.clone());
                Entry::Item(
                    Item::new("", move || {
                        let Some(feature) = act.upgrade() else {
                            return;
                        };
                        // The drive the menu named, not whichever stands in that place now.
                        let volume = feature.menu.borrow().get(slot).cloned();
                        if let Some(volume) = volume {
                            feature.eject(&volume);
                        }
                    })
                    .symbol(symbols::EJECT)
                    .titled(move || {
                        title
                            .upgrade()
                            .and_then(|feature| {
                                feature
                                    .menu_volume(slot)
                                    .map(|volume| feature.language().island_eject(&volume.name))
                            })
                            .unwrap_or_default()
                    })
                    .visible_when(move || {
                        shown.upgrade().is_some_and(|f| {
                            let wanted = {
                                let config = f.config.borrow();
                                config.enabled && config.drives
                            };
                            wanted && f.menu_volume(slot).is_some()
                        })
                    }),
                )
            })
            .collect()
    }

    /// The drive in this place of the menu, as the menu named it when it opened. The first place is where the refreshing
    /// of the menu begins: it looks at the drives, and the others read what it saw.
    fn menu_volume(&self, slot: usize) -> Option<volumes::Volume> {
        if slot == 0 {
            *self.menu.borrow_mut() = volumes::external();
        }
        self.menu.borrow().get(slot).cloned()
    }
}

const SLOT_CAPS: &str = "island-caps";
const SLOT_POWER: &str = "island-power";
const SLOT_DRIVE: &str = "island-drive";
const SLOT_DOWNLOAD: &str = "island-download";
const SLOT_PRIVACY: &str = "island-privacy";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_config_file_cannot_hold_an_unusable_level() {
        let config = Config {
            full_at: 3,
            low_at: 90,
            seconds: f64::NAN,
            ..Config::default()
        }
        .sanitized();
        assert_eq!(config.full_at, FULL_RANGE.0);
        assert_eq!(config.low_at, LOW_RANGE.1);
        assert!((config.seconds - Config::default().seconds).abs() < f64::EPSILON);
    }

    #[test]
    fn the_folder_can_be_named_for_a_check_and_is_downloads_otherwise() {
        // Reading a folder that does not exist is an empty folder, not a failure.
        assert!(scan(std::path::Path::new("/nonexistent/figghost")).is_empty());
        assert!(
            downloads_folder().ends_with("Downloads")
                || env::var_os("FIGGHOST_DOWNLOADS").is_some()
        );
    }

    #[test]
    fn scanning_lists_files_and_skips_folders() {
        let dir = env::temp_dir().join(format!("figghost-islands-{}", std::process::id()));
        std::fs::create_dir_all(dir.join("sub")).unwrap();
        std::fs::write(dir.join("a.txt"), b"hello").unwrap();
        let found = scan(&dir);
        std::fs::remove_dir_all(&dir).unwrap();
        assert_eq!(found, vec![("a.txt".to_owned(), 5)]);
    }
}
