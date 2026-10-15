//! The mixer's settings and every decision that needs no audio system: which apps need a tap, how the
//! volume slider maps to a gain, how the audio processes of the system group into apps, and how a tap's
//! frames reach the device's channels.

use core_audio_fx::{BANDS, MAX_BAND_DB, MAX_GAIN, Params, Preset, Processor};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// Settings of one app, by bundle identifier.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct AppAudio {
    /// 1.0 is unchanged, up to 4.0.
    pub volume: f32,
    pub muted: bool,
    /// UID of the output device to play on; empty follows the system's default output.
    pub output: String,
    /// Equalizer preset key; empty for none.
    pub eq: String,
}

impl Default for AppAudio {
    fn default() -> Self {
        Self {
            volume: 1.0,
            muted: false,
            output: String::new(),
            eq: String::new(),
        }
    }
}

/// Software volume of a device whose volume the system cannot change, by device UID.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct DeviceAudio {
    pub volume: f32,
    pub muted: bool,
}

impl Default for DeviceAudio {
    fn default() -> Self {
        Self {
            volume: 1.0,
            muted: false,
        }
    }
}

/// Effects on everything this app plays: the system output, and every app it routes.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Effects {
    /// Equalizer preset key, `custom` for `custom_eq`, empty for none.
    pub eq: String,
    pub custom_eq: [f32; BANDS],
    /// Magic Boost, 0 to 1: quiet passages get louder, loud ones do not.
    pub boost: f32,
    /// Strong compression for listening at night: loud scenes come down, dialogue comes up.
    pub night: bool,
    /// Stereo width: 0 is mono, 1 unchanged, 2 twice as wide.
    pub width: f32,
}

impl Default for Effects {
    fn default() -> Self {
        Self {
            eq: String::new(),
            custom_eq: [0.0; BANDS],
            boost: 0.0,
            night: false,
            width: 1.0,
        }
    }
}

pub const CUSTOM_EQ: &str = "custom";

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct AudioConfig {
    /// Off leaves every app's audio untouched, whatever the settings below say.
    pub enabled: bool,
    pub apps: BTreeMap<String, AppAudio>,
    pub devices: BTreeMap<String, DeviceAudio>,
    pub effects: Effects,
}

impl Default for AudioConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            apps: BTreeMap::new(),
            devices: BTreeMap::new(),
            effects: Effects::default(),
        }
    }
}

fn clamp(value: f32, min: f32, max: f32, fallback: f32) -> f32 {
    if value.is_finite() {
        value.clamp(min, max)
    } else {
        fallback
    }
}

impl AudioConfig {
    /// Clamps hand-edited values and drops entries that ask for nothing.
    pub fn sanitized(mut self) -> Self {
        for app in self.apps.values_mut() {
            app.volume = clamp(app.volume, 0.0, MAX_GAIN, 1.0);
            if !app.eq.is_empty() && Preset::from_key(&app.eq).is_none() {
                app.eq.clear();
            }
        }
        self.apps.retain(|_, app| !app.is_neutral());
        for device in self.devices.values_mut() {
            device.volume = clamp(device.volume, 0.0, MAX_GAIN, 1.0);
        }
        self.devices.retain(|_, d| *d != DeviceAudio::default());
        let e = &mut self.effects;
        e.boost = clamp(e.boost, 0.0, 1.0, 0.0);
        e.width = clamp(e.width, 0.0, 2.0, 1.0);
        for gain in &mut e.custom_eq {
            *gain = clamp(*gain, -MAX_BAND_DB, MAX_BAND_DB, 0.0);
        }
        if !e.eq.is_empty() && e.eq != CUSTOM_EQ && Preset::from_key(&e.eq).is_none() {
            e.eq.clear();
        }
        self
    }
}

impl AppAudio {
    pub fn is_neutral(&self) -> bool {
        self.volume == 1.0
            && !self.muted
            && self.output.is_empty()
            && (self.eq.is_empty() || self.eq == Preset::Flat.key())
    }
}

/// What an app's settings need from the audio system.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Need {
    /// Leave it alone.
    Nothing,
    /// Silence it; nothing has to be played.
    Mute,
    /// Take its audio, process it and play it on a device.
    Route,
}

pub fn need(app: &AppAudio) -> Need {
    if app.muted {
        Need::Mute
    } else if app.is_neutral() {
        Need::Nothing
    } else {
        Need::Route
    }
}

/// Band gains for an equalizer setting; `None` when it is off.
pub fn eq_curve(key: &str, custom: &[f32; BANDS]) -> Option<[f32; BANDS]> {
    if key == CUSTOM_EQ {
        return Some(*custom);
    }
    Preset::from_key(key)
        .filter(|p| *p != Preset::Flat)
        .map(Preset::gains)
}

fn set_eq(params: &Params, curve: Option<[f32; BANDS]>) {
    match curve {
        Some(gains) => {
            if params.eq_gains() != gains {
                params.set_eq_gains(gains);
            }
            if !params.eq_enabled() {
                params.set_eq_enabled(true);
            }
        }
        None if params.eq_enabled() => params.set_eq_enabled(false),
        None => {}
    }
}

/// Puts an app's settings into the parameters its route reads.
pub fn apply_app(app: &AppAudio, params: &Params) {
    params.set_volume(app.volume);
    params.set_muted(app.muted);
    set_eq(params, eq_curve(&app.eq, &[0.0; BANDS]));
}

/// Puts the effects and a device's software volume into the parameters every route on that device reads.
pub fn apply_device(effects: &Effects, device: &DeviceAudio, params: &Params) {
    params.set_volume(device.volume);
    params.set_muted(device.muted);
    params.set_width(effects.width);
    set_eq(params, eq_curve(&effects.eq, &effects.custom_eq));
    if params.boost() != effects.boost {
        params.set_boost(effects.boost);
    }
    if params.night() != effects.night {
        params.set_night(effects.night);
    }
}

/// Slider position (0 to 1) for a gain: 100% sits in the middle, the top end is `MAX_GAIN`.
pub fn slider_from_gain(gain: f32) -> f64 {
    let gain = f64::from(clamp(gain, 0.0, MAX_GAIN, 1.0));
    if gain <= 1.0 {
        gain / 2.0
    } else {
        0.5 + (gain - 1.0) / (f64::from(MAX_GAIN) - 1.0) / 2.0
    }
}

/// The gain at a slider position; positions close to the middle snap to exactly 100%.
pub fn gain_from_slider(position: f64) -> f32 {
    const DETENT: f64 = 0.015;
    let t = if position.is_finite() {
        position.clamp(0.0, 1.0)
    } else {
        0.5
    };
    if (t - 0.5).abs() < DETENT {
        return 1.0;
    }
    let gain = if t <= 0.5 {
        t * 2.0
    } else {
        1.0 + (t - 0.5) * 2.0 * (f64::from(MAX_GAIN) - 1.0)
    };
    gain as f32
}

/// The app an audio process belongs to.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Owner {
    pub bundle_id: String,
    pub name: String,
    /// Where the app is installed, for its icon.
    pub path: Option<String>,
}

/// One process connected to the audio system.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Member {
    pub object: u32,
    /// `None` for processes that are not an app the user can see (daemons, command-line tools).
    pub owner: Option<Owner>,
    pub playing: bool,
}

/// The audio processes of one app: the app itself and its helpers (a browser's audio service, a WebKit
/// media process, a game's launcher).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Group {
    pub owner: Owner,
    pub objects: Vec<u32>,
    pub playing: bool,
}

/// Groups processes by app, playing apps first, then by name.
pub fn group(members: &[Member]) -> Vec<Group> {
    let mut groups: BTreeMap<String, Group> = BTreeMap::new();
    for member in members {
        let Some(owner) = &member.owner else {
            continue;
        };
        let group = groups
            .entry(owner.bundle_id.clone())
            .or_insert_with(|| Group {
                owner: owner.clone(),
                objects: Vec::new(),
                playing: false,
            });
        group.objects.push(member.object);
        group.playing |= member.playing;
    }
    let mut list: Vec<Group> = groups.into_values().collect();
    for group in &mut list {
        group.objects.sort_unstable();
    }
    list.sort_by(|a, b| {
        b.playing.cmp(&a.playing).then_with(|| {
            a.owner
                .name
                .to_lowercase()
                .cmp(&b.owner.name.to_lowercase())
        })
    });
    list
}

/// Runs one IO cycle: the tap's frames through the processor onto the device's channels. The first two
/// channels carry the processed stereo; a mono device gets their average; any further channels get the
/// tap's own channel of that number, or silence.
pub fn render(
    processor: &mut Processor,
    input: &[f32],
    in_channels: usize,
    output: &mut [f32],
    out_channels: usize,
) {
    if in_channels == 0 || out_channels == 0 {
        return;
    }
    processor.begin();
    let frames = (input.len() / in_channels).min(output.len() / out_channels);
    for frame in 0..frames {
        let source = &input[frame * in_channels..(frame + 1) * in_channels];
        let target = &mut output[frame * out_channels..(frame + 1) * out_channels];
        let left = source[0];
        let right = if in_channels > 1 { source[1] } else { left };
        let (l, r) = processor.frame(left, right);
        if out_channels == 1 {
            target[0] = (l + r) * 0.5;
            continue;
        }
        target[0] = l;
        target[1] = r;
        for (channel, sample) in target.iter_mut().enumerate().skip(2) {
            *sample = source.get(channel).copied().unwrap_or(0.0);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    fn owner(id: &str, name: &str) -> Option<Owner> {
        Some(Owner {
            bundle_id: id.into(),
            name: name.into(),
            path: None,
        })
    }

    #[test]
    fn the_slider_puts_full_volume_in_the_middle_and_round_trips() {
        assert_eq!(slider_from_gain(1.0), 0.5);
        assert_eq!(slider_from_gain(0.0), 0.0);
        assert_eq!(slider_from_gain(MAX_GAIN), 1.0);
        assert_eq!(gain_from_slider(1.0), MAX_GAIN);
        assert_eq!(gain_from_slider(0.0), 0.0);
        for gain in [0.1, 0.25, 0.8, 1.5, 2.0, 3.5] {
            let back = gain_from_slider(slider_from_gain(gain));
            assert!((back - gain).abs() < 1e-5, "{gain} -> {back}");
        }
    }

    #[test]
    fn the_slider_snaps_to_full_volume_near_the_middle() {
        assert_eq!(gain_from_slider(0.51), 1.0);
        assert_eq!(gain_from_slider(0.49), 1.0);
        assert!(gain_from_slider(0.45) < 1.0);
        assert_eq!(gain_from_slider(f64::NAN), 1.0);
    }

    #[test]
    fn only_settings_that_change_something_need_a_tap() {
        let mut app = AppAudio::default();
        assert_eq!(need(&app), Need::Nothing);
        app.eq = "flat".into();
        assert_eq!(need(&app), Need::Nothing);
        app.eq = "rock".into();
        assert_eq!(need(&app), Need::Route);
        app.eq.clear();
        app.volume = 0.5;
        assert_eq!(need(&app), Need::Route);
        app.muted = true;
        assert_eq!(need(&app), Need::Mute);
        let routed = AppAudio {
            output: "BuiltInSpeakerDevice".into(),
            ..AppAudio::default()
        };
        assert_eq!(need(&routed), Need::Route);
    }

    #[test]
    fn hand_edited_audio_settings_are_cleaned_up() {
        let text = r#"{"apps": {"a": {"volume": 99}, "b": {"eq": "nonsense"}, "c": {"muted": true}},
                       "devices": {"d": {"volume": 1.0}}, "effects": {"boost": 3, "width": -1, "eq": "x"}}"#;
        let config: AudioConfig = serde_json::from_str::<AudioConfig>(text)
            .unwrap()
            .sanitized();
        assert!(config.enabled);
        assert_eq!(config.apps["a"].volume, MAX_GAIN);
        assert!(!config.apps.contains_key("b"), "neutral after cleaning");
        assert!(config.apps["c"].muted);
        assert!(config.devices.is_empty());
        assert_eq!((config.effects.boost, config.effects.width), (1.0, 0.0));
        assert!(config.effects.eq.is_empty());
    }

    #[test]
    fn effects_and_device_volume_reach_the_parameters() {
        let params = Params::default();
        let effects = Effects {
            eq: "bass_boost".into(),
            boost: 0.5,
            ..Effects::default()
        };
        apply_device(
            &effects,
            &DeviceAudio {
                volume: 0.3,
                muted: false,
            },
            &params,
        );
        assert!(params.eq_enabled());
        assert_eq!(params.eq_gains(), Preset::BassBoost.gains());
        assert_eq!((params.volume(), params.boost()), (0.3, 0.5));
        apply_device(&Effects::default(), &DeviceAudio::default(), &params);
        assert!(params.is_neutral());
        let custom = Effects {
            eq: CUSTOM_EQ.into(),
            custom_eq: [2.0; BANDS],
            ..Effects::default()
        };
        apply_device(&custom, &DeviceAudio::default(), &params);
        assert_eq!(params.eq_gains(), [2.0; BANDS]);
    }

    #[test]
    fn processes_group_into_apps_with_playing_ones_first() {
        let members = [
            Member {
                object: 30,
                owner: owner("com.google.Chrome", "Google Chrome"),
                playing: false,
            },
            Member {
                object: 12,
                owner: owner("com.spotify.client", "Spotify"),
                playing: false,
            },
            Member {
                object: 31,
                owner: owner("com.google.Chrome", "Google Chrome"),
                playing: true,
            },
            Member {
                object: 5,
                owner: None,
                playing: true,
            },
            Member {
                object: 8,
                owner: owner("com.apple.Music", "Music"),
                playing: false,
            },
        ];
        let groups = group(&members);
        let names: Vec<&str> = groups.iter().map(|g| g.owner.name.as_str()).collect();
        assert_eq!(names, ["Google Chrome", "Music", "Spotify"]);
        assert_eq!(groups[0].objects, [30, 31]);
        assert!(groups[0].playing && !groups[1].playing);
    }

    #[test]
    fn render_maps_stereo_onto_mono_stereo_and_wider_devices() {
        let neutral = Arc::new(Params::default());
        let mut processor = Processor::new(48_000.0, None, Arc::clone(&neutral));
        let input = [0.5f32, -0.25, 0.1, 0.3];
        let mut mono = [9.0f32; 2];
        render(&mut processor, &input, 2, &mut mono, 1);
        assert_eq!(mono, [0.125, 0.2]);
        let mut stereo = [9.0f32; 4];
        render(&mut processor, &input, 2, &mut stereo, 2);
        assert_eq!(stereo, input);
        let mut wide = [9.0f32; 8];
        render(&mut processor, &input, 2, &mut wide, 4);
        assert_eq!(wide, [0.5, -0.25, 0.0, 0.0, 0.1, 0.3, 0.0, 0.0]);
        let mut short = [9.0f32; 2];
        render(&mut processor, &input, 2, &mut short, 2);
        assert_eq!(short, [0.5, -0.25]);
    }
}
