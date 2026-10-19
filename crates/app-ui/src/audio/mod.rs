//! The per-app mixer, SoundSource and Boom 3D in one: the engine that takes over apps' audio, and the
//! sound page of the notch.

mod engine;
pub mod model;
mod text;

pub use engine::{Engine, Snapshot, SoftVolume};

use crate::{
    config::Config,
    i18n::{self, Language},
};
use core_audio_fx::{MAX_ORBIT, Preset};
use macos_sys::{
    MainThreadMarker, app,
    mixer_ui::{MixerCallbacks, MixerModel, MixerRow, MixerTexts, MixerView},
    notch::Notch,
};
use model::{CUSTOM_EQ, gain_from_slider, slider_from_gain};
use std::{
    cell::{Cell, RefCell},
    path::PathBuf,
    rc::Rc,
};

/// How often an open page re-reads which apps are playing.
const REFRESH_SECONDS: f64 = 1.0;

/// Equalizer choices for the effects, in page order: off, every preset that changes something, then the
/// custom curve from the config file.
fn effects_eq_keys() -> Vec<&'static str> {
    let mut keys = vec![""];
    keys.extend(
        Preset::ALL
            .into_iter()
            .filter(|p| *p != Preset::Flat)
            .map(Preset::key),
    );
    keys.push(CUSTOM_EQ);
    keys
}

/// The same choices for one app, which has no custom curve.
fn app_eq_keys() -> Vec<&'static str> {
    let mut keys = effects_eq_keys();
    keys.pop();
    keys
}

fn index_of(keys: &[&str], key: &str) -> usize {
    keys.iter().position(|k| *k == key).unwrap_or(0)
}

fn percent(gain: f32) -> String {
    format!("{:.0}%", gain * 100.0)
}

fn eq_name(text: Language, key: &str) -> String {
    match key {
        "" => text.audio_eq_off().to_owned(),
        CUSTOM_EQ => text.audio_eq_custom().to_owned(),
        key => Preset::from_key(key)
            .map(|p| text.audio_preset(p).to_owned())
            .unwrap_or_else(|| key.to_owned()),
    }
}

/// What the page shows for a state of the engine.
pub fn mixer_model(s: &Snapshot, text: Language) -> MixerModel {
    let effects_keys = effects_eq_keys();
    let app_keys = app_eq_keys();
    let mut outputs = vec![text.audio_default_output().to_owned()];
    outputs.extend(s.devices.iter().map(|d| d.name.clone()));
    let rows: Vec<MixerRow> = s
        .rows
        .iter()
        .map(|r| MixerRow {
            key: r.owner.bundle_id.clone(),
            name: r.owner.name.clone(),
            icon_path: r.owner.path.clone(),
            playing: r.playing,
            volume: slider_from_gain(r.settings.volume),
            percent: percent(r.settings.volume),
            muted: r.settings.muted,
            output: s
                .devices
                .iter()
                .position(|d| d.uid == r.settings.output)
                .map_or(0, |i| i + 1),
            eq: index_of(&app_keys, &r.settings.eq),
        })
        .collect();
    let device_title = if !s.supported {
        text.audio_unsupported().to_owned()
    } else if let Some(problem) = &s.problem {
        text.audio_problem(problem)
    } else if !s.enabled {
        text.audio_disabled().to_owned()
    } else {
        match &s.default_output {
            Some(d) => text.audio_output(&d.name, !d.hardware_volume),
            None => text.audio_no_output().to_owned(),
        }
    };
    MixerModel {
        enabled: s.enabled,
        notice: rows
            .is_empty()
            .then(|| text.audio_nothing_playing().to_owned()),
        device_title,
        device_volume: s
            .device
            .as_ref()
            .map(|d| (slider_from_gain(d.volume), percent(d.volume))),
        outputs,
        eq_options: effects_keys.iter().map(|k| eq_name(text, k)).collect(),
        app_eq_options: app_keys.iter().map(|k| eq_name(text, k)).collect(),
        effects_eq: index_of(&effects_keys, &s.effects.eq),
        boost: f64::from(s.effects.boost),
        night: s.effects.night,
        width: f64::from(s.effects.width) / 2.0,
        spatial: f64::from(s.effects.spatial),
        angle: f64::from(s.effects.angle) / 360.0 + 0.5,
        orbit: f64::from(s.effects.orbit) / f64::from(2.0 * MAX_ORBIT) + 0.5,
        rows,
    }
}

type Refresh = Rc<RefCell<Option<Rc<dyn Fn()>>>>;

fn callbacks(
    engine: &Rc<Engine>,
    last: &Rc<RefCell<Snapshot>>,
    refresh: &Refresh,
) -> MixerCallbacks {
    // Every change re-reads the state so the percent labels and check marks follow at once.
    let after = |refresh: &Refresh| {
        let refresh = Rc::clone(refresh);
        move || {
            let f = refresh.borrow().clone();
            if let Some(f) = f {
                f();
            }
        }
    };
    let e = || Rc::clone(engine);
    MixerCallbacks {
        on_enabled: {
            let (engine, done) = (e(), after(refresh));
            Box::new(move |on| {
                engine.set_enabled(on);
                done();
            })
        },
        on_device_volume: {
            let (engine, last, done) = (e(), Rc::clone(last), after(refresh));
            Box::new(move |position| {
                let uid = last.borrow().default_output.as_ref().map(|d| d.uid.clone());
                if let Some(uid) = uid {
                    engine.update_device(&uid, |d| d.volume = gain_from_slider(position));
                    done();
                }
            })
        },
        on_effects_eq: {
            let (engine, done) = (e(), after(refresh));
            Box::new(move |index| {
                let key = effects_eq_keys().get(index).copied().unwrap_or("");
                engine.update_effects(|fx| fx.eq = key.to_owned());
                done();
            })
        },
        on_boost: {
            let (engine, done) = (e(), after(refresh));
            Box::new(move |position| {
                engine.update_effects(|fx| fx.boost = position as f32);
                done();
            })
        },
        on_night: {
            let (engine, done) = (e(), after(refresh));
            Box::new(move |on| {
                engine.update_effects(|fx| fx.night = on);
                done();
            })
        },
        on_width: {
            let (engine, done) = (e(), after(refresh));
            Box::new(move |position| {
                // The middle of the slider is exactly unchanged.
                let width = if (position - 0.5).abs() < 0.02 {
                    1.0
                } else {
                    (position * 2.0) as f32
                };
                engine.update_effects(|fx| fx.width = width);
                done();
            })
        },
        on_spatial: {
            let (engine, done) = (e(), after(refresh));
            Box::new(move |position| {
                engine.update_effects(|fx| fx.spatial = position as f32);
                done();
            })
        },
        on_angle: {
            let (engine, done) = (e(), after(refresh));
            Box::new(move |position| {
                let angle = if (position - 0.5).abs() < 0.02 {
                    0.0
                } else {
                    ((position - 0.5) * 360.0) as f32
                };
                engine.update_effects(|fx| fx.angle = angle);
                done();
            })
        },
        on_orbit: {
            let (engine, done) = (e(), after(refresh));
            Box::new(move |position| {
                let orbit = if (position - 0.5).abs() < 0.02 {
                    0.0
                } else {
                    ((position - 0.5) * 2.0) as f32 * MAX_ORBIT
                };
                engine.update_effects(|fx| fx.orbit = orbit);
                done();
            })
        },
        on_volume: {
            let (engine, done) = (e(), after(refresh));
            Box::new(move |key, position| {
                engine.update_app(key, |a| a.volume = gain_from_slider(position));
                done();
            })
        },
        on_mute: {
            let (engine, done) = (e(), after(refresh));
            Box::new(move |key, muted| {
                engine.update_app(key, |a| a.muted = muted);
                done();
            })
        },
        on_output: {
            let (engine, last, done) = (e(), Rc::clone(last), after(refresh));
            Box::new(move |key, index| {
                let uid = match index {
                    0 => Some(String::new()),
                    i => last.borrow().devices.get(i - 1).map(|d| d.uid.clone()),
                };
                if let Some(uid) = uid {
                    engine.update_app(key, |a| a.output = uid);
                    done();
                }
            })
        },
        on_eq: {
            let (engine, done) = (e(), after(refresh));
            Box::new(move |key, index| {
                let eq = app_eq_keys().get(index).copied().unwrap_or("");
                engine.update_app(key, |a| a.eq = eq.to_owned());
                done();
            })
        },
    }
}

/// Keeps the engine and the page alive.
pub struct AudioFeature {
    pub engine: Rc<Engine>,
    _view: Option<Rc<MixerView>>,
    _timer: Rc<RefCell<Option<app::Timer>>>,
}

impl AudioFeature {
    /// Starts the engine with the saved settings; the page is added to the notch when there is one.
    pub fn start(
        mtm: MainThreadMarker,
        config: &Rc<RefCell<Config>>,
        home: PathBuf,
        notch: Option<&Notch>,
    ) -> Self {
        let settings = config.borrow().audio.clone();
        let engine = {
            let config = Rc::clone(config);
            Engine::start(settings, move |audio| {
                config.borrow_mut().audio = audio.clone();
                if let Err(e) = config.borrow().save(&home) {
                    eprintln!("config: cannot save: {e}");
                }
            })
        };
        let timer: Rc<RefCell<Option<app::Timer>>> = Rc::default();
        let Some(notch) = notch else {
            return Self {
                engine,
                _view: None,
                _timer: timer,
            };
        };
        let text = i18n::current();
        let texts = MixerTexts {
            tab: text.audio_tab().into(),
            hint: text.audio_hint().into(),
            enabled: text.audio_enabled().into(),
            device_volume: text.audio_device_volume().into(),
            boost: text.audio_boost().into(),
            night: text.audio_night().into(),
            width: text.audio_width().into(),
            spatial: text.audio_spatial().into(),
            angle: text.audio_angle().into(),
            orbit: text.audio_orbit().into(),
            mute: text.audio_mute().into(),
            app_eq: text.audio_app_eq().into(),
        };
        let last = Rc::new(RefCell::new(engine.snapshot()));
        let refresh: Refresh = Rc::default();
        let view = MixerView::new(
            mtm,
            notch.page_frame(),
            texts,
            callbacks(&engine, &last, &refresh),
        );
        let show: Rc<dyn Fn()> = {
            let (engine, view, last) = (Rc::clone(&engine), Rc::clone(&view), Rc::clone(&last));
            Rc::new(move || {
                let snapshot = engine.snapshot();
                view.show(&mixer_model(&snapshot, text));
                *last.borrow_mut() = snapshot;
            })
        };
        *refresh.borrow_mut() = Some(Rc::clone(&show));
        let visible = Rc::new(Cell::new(false));
        {
            let (show, visible) = (Rc::clone(&show), Rc::clone(&visible));
            engine.set_on_update(move || {
                if visible.get() {
                    show();
                }
            });
        }
        let on_show = {
            let (show, visible, timer) = (Rc::clone(&show), Rc::clone(&visible), Rc::clone(&timer));
            move || {
                visible.set(true);
                show();
                let show = Rc::clone(&show);
                *timer.borrow_mut() = Some(app::every(REFRESH_SECONDS, move || show()));
            }
        };
        let on_hide = {
            let (visible, timer) = (Rc::clone(&visible), Rc::clone(&timer));
            move || {
                visible.set(false);
                timer.borrow_mut().take();
            }
        };
        notch.add_page(view.page(on_show, on_hide));
        show();
        Self {
            engine,
            _view: Some(view),
            _timer: timer,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use engine::AppRow;
    use macos_sys::audio_tap::{OutputDevice, Transport};
    use model::{AppAudio, Effects, Owner};

    fn device(uid: &str, name: &str, hardware_volume: bool) -> OutputDevice {
        OutputDevice {
            id: 1,
            uid: uid.into(),
            name: name.into(),
            transport: Transport::BuiltIn,
            hardware_volume,
        }
    }

    fn snapshot() -> Snapshot {
        let hdmi = device("hdmi-1", "S24i-30", false);
        Snapshot {
            supported: true,
            enabled: true,
            rows: vec![AppRow {
                owner: Owner {
                    bundle_id: "com.spotify.client".into(),
                    name: "Spotify".into(),
                    path: None,
                },
                playing: true,
                settings: AppAudio {
                    volume: 2.0,
                    output: "hdmi-1".into(),
                    eq: "rock".into(),
                    ..AppAudio::default()
                },
            }],
            devices: vec![device("speakers", "Mac mini Speakers", true), hdmi.clone()],
            default_output: Some(hdmi),
            device: Some(model::DeviceAudio {
                volume: 0.5,
                muted: false,
            }),
            effects: Effects {
                eq: "bass_boost".into(),
                width: 1.5,
                ..Effects::default()
            },
            problem: None,
        }
    }

    #[test]
    fn the_page_model_follows_the_engine_state() {
        let m = mixer_model(&snapshot(), Language::English);
        assert_eq!(m.outputs, ["Default", "Mac mini Speakers", "S24i-30"]);
        let row = &m.rows[0];
        assert_eq!((row.output, row.percent.as_str()), (2, "200%"));
        assert_eq!(m.app_eq_options[row.eq], "Rock");
        assert_eq!(m.eq_options[m.effects_eq], "Bass boost");
        assert_eq!(m.device_volume, Some((0.25, "50%".to_owned())));
        assert_eq!(m.width, 0.75);
        assert!(m.device_title.contains("no hardware volume"));
        assert_eq!(m.notice, None);
    }

    #[test]
    fn choices_start_with_off_and_only_effects_offer_the_custom_curve() {
        let (fx, app) = (effects_eq_keys(), app_eq_keys());
        assert_eq!((fx[0], app[0]), ("", ""));
        assert_eq!(fx.last(), Some(&CUSTOM_EQ));
        assert!(!app.contains(&CUSTOM_EQ) && !app.contains(&"flat"));
        assert_eq!(fx.len(), Preset::ALL.len() + 1);
        assert_eq!(index_of(&app, "flat"), 0);
    }

    #[test]
    fn problems_replace_the_output_line_and_an_empty_list_says_so() {
        let mut s = snapshot();
        s.rows.clear();
        s.problem = Some("starting the route failed ('nope')".into());
        let m = mixer_model(&s, Language::Vietnamese);
        assert!(m.device_title.starts_with("Lỗi âm thanh"));
        assert_eq!(m.notice.as_deref(), Some("Chưa có app nào phát âm thanh."));
    }
}
