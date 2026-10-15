//! Keeps the system's audio in line with the mixer settings: a tap per app that needs one, a route on
//! the default output for the effects and the software volume, and each route running only while its
//! audio plays, so an idle Mac can still sleep.
//!
//! Taps here silence what they take for as long as they exist, and routes start within a few
//! milliseconds of the audio starting: a quieted app never blasts at its old level first. If this app
//! dies, the system drops its taps and every app plays normally again.

use super::model::{self, AppAudio, AudioConfig, DeviceAudio, Effects, Group, Member, Need, Owner};
use crate::trace;
use core_audio_fx::{Params, Processor};
use macos_sys::{
    app,
    audio_tap::{self, Listener, Muting, ObjectId, OutputDevice, Route, Source, Tap, selector},
    volume::Volume,
    workspace,
};
use std::{
    cell::{Cell, RefCell},
    collections::{BTreeMap, HashMap, hash_map::Entry},
    rc::{Rc, Weak},
    sync::{Arc, Mutex, PoisonError},
    time::Instant,
};

/// A route whose audio stopped keeps running this long, so the gap between two tracks does not stop
/// and restart the device.
const IDLE_STOP_SECONDS: f64 = 5.0;
/// Notifications come in bursts; one rebuild after the last of them is enough.
const SETTLE_SECONDS: f64 = 0.05;
/// Key presses on a software volume are saved this long after the last one.
const SAVE_DELAY_SECONDS: f64 = 1.0;

enum AppTap {
    /// A muted app: the tap alone silences it.
    Mute { tap: Tap, objects: Vec<ObjectId> },
    Route {
        route: Route,
        objects: Vec<ObjectId>,
        output: String,
        idle_since: Option<Instant>,
    },
}

impl AppTap {
    fn objects(&self) -> &[ObjectId] {
        match self {
            AppTap::Mute { objects, .. } | AppTap::Route { objects, .. } => objects,
        }
    }
}

struct DeviceRoute {
    route: Route,
    uid: String,
    excluded: Vec<ObjectId>,
    idle_since: Option<Instant>,
}

/// One app in the mixer list.
#[derive(Debug, Clone, PartialEq)]
pub struct AppRow {
    pub owner: Owner,
    pub playing: bool,
    pub settings: AppAudio,
}

/// Everything the mixer page shows.
#[derive(Debug, Clone, PartialEq)]
pub struct Snapshot {
    pub supported: bool,
    pub enabled: bool,
    pub rows: Vec<AppRow>,
    pub devices: Vec<OutputDevice>,
    pub default_output: Option<OutputDevice>,
    /// Software volume of the default output, when it has no volume of its own.
    pub device: Option<DeviceAudio>,
    pub effects: Effects,
    pub problem: Option<String>,
}

#[derive(Default)]
struct State {
    own: Option<ObjectId>,
    devices: Vec<OutputDevice>,
    default_output: Option<OutputDevice>,
    groups: Vec<Group>,
    owners: HashMap<i32, Option<Owner>>,
    pids: HashMap<ObjectId, i32>,
    app_params: HashMap<String, Arc<Params>>,
    device_params: HashMap<String, Arc<Params>>,
    apps: HashMap<String, AppTap>,
    device: Option<DeviceRoute>,
    watchers: HashMap<ObjectId, Listener>,
    system: Vec<Listener>,
    /// Apps that played since launch stay in the list while they run, so a pause does not hide them.
    seen: BTreeMap<String, Owner>,
    problem: Option<String>,
}

/// What the volume keys may change when the default output has no volume of its own. Shared with the
/// event tap thread.
#[derive(Default)]
pub struct SoftVolume {
    target: Mutex<Option<(String, Arc<Params>)>>,
}

impl SoftVolume {
    pub(crate) fn set_target(&self, target: Option<(String, Arc<Params>)>) {
        *self.target.lock().unwrap_or_else(PoisonError::into_inner) = target;
    }

    fn current(&self) -> Option<(String, Arc<Params>)> {
        self.target
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone()
    }

    /// The software volume, when the volume keys should move it instead of the system's.
    pub fn read(&self) -> Option<Volume> {
        let (_, params) = self.current()?;
        Some(Volume {
            level: params.volume(),
            muted: params.muted(),
        })
    }

    /// Applies a volume key at once, then lets the main thread build the route and save the setting.
    pub fn write(&self, volume: Volume) -> bool {
        let Some((uid, params)) = self.current() else {
            return false;
        };
        params.set_volume(volume.level);
        params.set_muted(volume.muted);
        app::on_main(move || {
            ENGINE.with(|engine| {
                if let Some(engine) = engine.borrow().upgrade() {
                    engine.update_device(&uid, |d| {
                        d.volume = volume.level;
                        d.muted = volume.muted;
                    });
                }
            });
        });
        true
    }
}

type SoftHook = Box<dyn Fn(bool)>;

thread_local! {
    static ENGINE: RefCell<Weak<Engine>> = const { RefCell::new(Weak::new()) };
}

pub struct Engine {
    me: Weak<Engine>,
    settings: RefCell<AudioConfig>,
    state: RefCell<State>,
    soft: Arc<SoftVolume>,
    pending: Cell<bool>,
    save_pending: Cell<bool>,
    on_save: Box<dyn Fn(&AudioConfig)>,
    on_update: RefCell<Option<Box<dyn Fn()>>>,
    on_soft: RefCell<Option<SoftHook>>,
    soft_present: Cell<bool>,
}

fn owner_of(pid: i32) -> Option<Owner> {
    let responsible = audio_tap::responsible_pid(pid).unwrap_or(pid);
    let app = workspace::running_app(responsible).or_else(|| workspace::running_app(pid))?;
    if app.pid == std::process::id() as i32 {
        return None;
    }
    Some(Owner {
        path: workspace::app_path(&app.bundle_id).map(|p| p.to_string_lossy().into_owned()),
        bundle_id: app.bundle_id,
        name: app.name,
    })
}

impl Engine {
    /// Starts following the audio system. `on_save` is called with the settings after every change the
    /// user makes, to write them to the config file.
    pub fn start(settings: AudioConfig, on_save: impl Fn(&AudioConfig) + 'static) -> Rc<Self> {
        let engine = Rc::new_cyclic(|me| Self {
            me: me.clone(),
            settings: RefCell::new(settings),
            state: RefCell::default(),
            soft: Arc::default(),
            pending: Cell::new(false),
            save_pending: Cell::new(false),
            on_save: Box::new(on_save),
            on_update: RefCell::new(None),
            on_soft: RefCell::new(None),
            soft_present: Cell::new(false),
        });
        ENGINE.with(|slot| *slot.borrow_mut() = Rc::downgrade(&engine));
        if audio_tap::taps_supported() {
            let listeners: Vec<Listener> = [
                selector::PROCESSES,
                selector::DEVICES,
                selector::DEFAULT_OUTPUT,
            ]
            .into_iter()
            .filter_map(|sel| {
                let weak = engine.me.clone();
                Listener::new(audio_tap::SYSTEM, sel, move || {
                    if let Some(engine) = weak.upgrade() {
                        engine.schedule();
                    }
                })
            })
            .collect();
            engine.state.borrow_mut().system = listeners;
        }
        engine.reconcile();
        engine
    }

    pub fn soft_volume(&self) -> Arc<SoftVolume> {
        Arc::clone(&self.soft)
    }

    /// Called after the state changed on its own (an app started playing, a device appeared).
    pub fn set_on_update(&self, on_update: impl Fn() + 'static) {
        *self.on_update.borrow_mut() = Some(Box::new(on_update));
    }

    /// Called with true when the volume keys should move the software volume (the default output has no
    /// volume of its own), and with false when they should go back to the system. Called once right away.
    pub fn set_on_soft(&self, on_soft: impl Fn(bool) + 'static) {
        on_soft(self.soft_present.get());
        *self.on_soft.borrow_mut() = Some(Box::new(on_soft));
    }

    pub fn snapshot(&self) -> Snapshot {
        let settings = self.settings.borrow();
        let st = self.state.borrow();
        let rows = st
            .groups
            .iter()
            .filter(|g| {
                g.playing
                    || settings.apps.contains_key(&g.owner.bundle_id)
                    || st.seen.contains_key(&g.owner.bundle_id)
            })
            .map(|g| AppRow {
                owner: g.owner.clone(),
                playing: g.playing,
                settings: settings
                    .apps
                    .get(&g.owner.bundle_id)
                    .cloned()
                    .unwrap_or_default(),
            })
            .collect();
        let device = st
            .default_output
            .as_ref()
            .filter(|d| !d.hardware_volume)
            .map(|d| settings.devices.get(&d.uid).cloned().unwrap_or_default());
        Snapshot {
            supported: audio_tap::taps_supported(),
            enabled: settings.enabled,
            rows,
            devices: st.devices.clone(),
            default_output: st.default_output.clone(),
            device,
            effects: settings.effects.clone(),
            problem: st.problem.clone(),
        }
    }

    pub fn set_enabled(&self, enabled: bool) {
        self.settings.borrow_mut().enabled = enabled;
        self.changed();
    }

    pub fn update_app(&self, bundle_id: &str, edit: impl FnOnce(&mut AppAudio)) {
        {
            let mut settings = self.settings.borrow_mut();
            let app = settings.apps.entry(bundle_id.to_owned()).or_default();
            edit(app);
            if app.is_neutral() {
                settings.apps.remove(bundle_id);
            }
        }
        self.changed();
    }

    pub fn update_effects(&self, edit: impl FnOnce(&mut Effects)) {
        edit(&mut self.settings.borrow_mut().effects);
        self.changed();
    }

    pub fn update_device(&self, uid: &str, edit: impl FnOnce(&mut DeviceAudio)) {
        {
            let mut settings = self.settings.borrow_mut();
            let device = settings.devices.entry(uid.to_owned()).or_default();
            edit(device);
            if *device == DeviceAudio::default() {
                settings.devices.remove(uid);
            }
        }
        self.changed();
    }

    /// A settings change: parameters first, so a running route follows at once; then the taps.
    fn changed(&self) {
        self.apply_params();
        self.reconcile();
        if !self.save_pending.replace(true) {
            let weak = self.me.clone();
            app::after(SAVE_DELAY_SECONDS, move || {
                if let Some(engine) = weak.upgrade() {
                    engine.save_pending.set(false);
                    (engine.on_save)(&engine.settings.borrow());
                }
            });
        }
    }

    fn schedule(&self) {
        if self.pending.replace(true) {
            return;
        }
        let weak = self.me.clone();
        app::after(SETTLE_SECONDS, move || {
            if let Some(engine) = weak.upgrade() {
                engine.pending.set(false);
                engine.reconcile();
                engine.notify();
            }
        });
    }

    fn notify(&self) {
        if let Some(on_update) = self.on_update.borrow().as_ref() {
            on_update();
        }
    }

    fn app_params(&self, st: &mut State, bundle_id: &str) -> Arc<Params> {
        Arc::clone(st.app_params.entry(bundle_id.to_owned()).or_default())
    }

    fn device_params(&self, st: &mut State, uid: &str) -> Arc<Params> {
        Arc::clone(st.device_params.entry(uid.to_owned()).or_default())
    }

    /// Writes the settings into every parameter set, which running routes read on their next cycle.
    fn apply_params(&self) {
        let settings = self.settings.borrow();
        let st = self.state.borrow();
        for (bundle_id, params) in &st.app_params {
            model::apply_app(
                &settings.apps.get(bundle_id).cloned().unwrap_or_default(),
                params,
            );
        }
        for (uid, params) in &st.device_params {
            let device = settings.devices.get(uid).cloned().unwrap_or_default();
            model::apply_device(&settings.effects, &device, params);
        }
    }

    fn reconcile(&self) {
        let mut st = self.state.borrow_mut();
        if !audio_tap::taps_supported() {
            st.problem = Some("taps".into());
            return;
        }
        st.problem = None;
        st.devices = audio_tap::output_devices();
        st.default_output = audio_tap::default_output();

        let me = std::process::id() as i32;
        let processes = audio_tap::processes();
        let mut members = Vec::with_capacity(processes.len());
        let mut live = HashMap::new();
        for p in &processes {
            live.insert(p.object, p.pid);
            if p.pid == me {
                st.own = Some(p.object);
                continue;
            }
            let owner = st
                .owners
                .entry(p.pid)
                .or_insert_with(|| owner_of(p.pid))
                .clone();
            members.push(Member {
                object: p.object,
                owner,
                playing: p.playing,
            });
        }
        st.owners.retain(|pid, _| live.values().any(|p| p == pid));
        st.watchers.retain(|object, _| live.contains_key(object));
        for &object in live.keys() {
            if let Entry::Vacant(slot) = st.watchers.entry(object) {
                let weak = self.me.clone();
                if let Some(listener) = Listener::new(object, selector::RUNNING_OUTPUT, move || {
                    if let Some(engine) = weak.upgrade() {
                        engine.update_running();
                        engine.notify();
                    }
                }) {
                    slot.insert(listener);
                }
            }
        }
        st.pids = live;
        st.groups = model::group(&members);
        let playing: Vec<Owner> = st
            .groups
            .iter()
            .filter(|g| g.playing)
            .map(|g| g.owner.clone())
            .collect();
        for owner in playing {
            st.seen.insert(owner.bundle_id.clone(), owner);
        }

        let settings = self.settings.borrow().clone();
        self.reconcile_apps(&mut st, &settings);
        self.reconcile_device(&mut st, &settings);
        drop(st);
        self.apply_params();
        self.update_running();
        let present = self.soft.read().is_some();
        if self.soft_present.replace(present) != present
            && let Some(on_soft) = self.on_soft.borrow().as_ref()
        {
            on_soft(present);
        }
    }

    fn output_for(st: &State, app: &AppAudio) -> Option<String> {
        if !app.output.is_empty() && st.devices.iter().any(|d| d.uid == app.output) {
            return Some(app.output.clone());
        }
        st.default_output.as_ref().map(|d| d.uid.clone())
    }

    fn reconcile_apps(&self, st: &mut State, settings: &AudioConfig) {
        let mut wanted: HashMap<String, (Need, Vec<ObjectId>, Option<String>, String)> =
            HashMap::new();
        for group in &st.groups {
            let Some(app) = settings.apps.get(&group.owner.bundle_id) else {
                continue;
            };
            let need = if settings.enabled {
                model::need(app)
            } else {
                Need::Nothing
            };
            if need != Need::Nothing {
                wanted.insert(
                    group.owner.bundle_id.clone(),
                    (
                        need,
                        group.objects.clone(),
                        Self::output_for(st, app),
                        group.owner.name.clone(),
                    ),
                );
            }
        }
        // Drop what is no longer wanted, or is the wrong kind, or plays on another device now.
        st.apps
            .retain(|bundle_id, tap| match (wanted.get(bundle_id), tap) {
                (Some((Need::Mute, ..)), AppTap::Mute { .. }) => true,
                (
                    Some((Need::Route, _, output, _)),
                    AppTap::Route {
                        output: current, ..
                    },
                ) => output.as_deref() == Some(current.as_str()),
                _ => false,
            });
        for (bundle_id, (need, objects, output, name)) in wanted {
            if let Some(existing) = st.apps.get_mut(&bundle_id) {
                if existing.objects() != objects.as_slice() {
                    let result = match existing {
                        AppTap::Mute { tap, objects: o } => {
                            *o = objects.clone();
                            tap.set_processes(&objects)
                        }
                        AppTap::Route {
                            route, objects: o, ..
                        } => {
                            *o = objects.clone();
                            route.tap().set_processes(&objects)
                        }
                    };
                    if let Err(e) = result {
                        st.problem = Some(e.to_string());
                    }
                }
                continue;
            }
            let label = format!("Figghost {name}");
            let made = match need {
                Need::Mute => {
                    Tap::new(Source::Processes(&objects), Muting::Muted, &label).map(|tap| {
                        AppTap::Mute {
                            tap,
                            objects: objects.clone(),
                        }
                    })
                }
                Need::Route => {
                    let Some(output) = output else { continue };
                    let app = self.app_params(st, &bundle_id);
                    let device = self.device_params(st, &output);
                    Tap::new(Source::Processes(&objects), Muting::Muted, &label)
                        .and_then(|tap| {
                            Route::new(tap, &output, &label, move |format| {
                                let mut processor =
                                    Processor::new(format.sample_rate, Some(app), device);
                                Box::new(move |input, in_ch, out, out_ch| {
                                    model::render(&mut processor, input, in_ch, out, out_ch);
                                })
                            })
                        })
                        .map(|route| AppTap::Route {
                            route,
                            objects: objects.clone(),
                            output,
                            idle_since: None,
                        })
                }
                Need::Nothing => continue,
            };
            match made {
                Ok(tap) => {
                    trace::log(&format!("audio: tapping {bundle_id} ({objects:?})"));
                    st.apps.insert(bundle_id, tap);
                }
                Err(e) => {
                    eprintln!("audio: cannot take over {bundle_id}: {e}");
                    st.problem = Some(e.to_string());
                }
            }
        }
    }

    fn reconcile_device(&self, st: &mut State, settings: &AudioConfig) {
        let Some(default) = st.default_output.clone() else {
            st.device = None;
            self.soft.set_target(None);
            return;
        };
        let params = self.device_params(st, &default.uid);
        let device = settings
            .devices
            .get(&default.uid)
            .cloned()
            .unwrap_or_default();
        model::apply_device(&settings.effects, &device, &params);
        self.soft.set_target(
            (settings.enabled && !default.hardware_volume)
                .then(|| (default.uid.clone(), Arc::clone(&params))),
        );
        if !settings.enabled || params.is_neutral() {
            st.device = None;
            return;
        }
        let mut excluded: Vec<ObjectId> = st
            .apps
            .values()
            .flat_map(|tap| tap.objects().iter().copied())
            .chain(st.own)
            .collect();
        excluded.sort_unstable();
        if let Some(current) = &mut st.device
            && current.uid == default.uid
        {
            if current.excluded != excluded {
                match current.route.tap().set_processes(&excluded) {
                    Ok(()) => current.excluded = excluded,
                    Err(e) => st.problem = Some(e.to_string()),
                }
            }
            return;
        }
        st.device = None;
        let made = Tap::new(
            Source::Device {
                uid: &default.uid,
                exclude: &excluded,
            },
            Muting::Muted,
            "Figghost output",
        )
        .and_then(|tap| {
            let params = Arc::clone(&params);
            Route::new(tap, &default.uid, "Figghost output", move |format| {
                let mut processor = Processor::new(format.sample_rate, None, params);
                Box::new(move |input, in_ch, out, out_ch| {
                    model::render(&mut processor, input, in_ch, out, out_ch);
                })
            })
        });
        let route = match made {
            Ok(route) => route,
            Err(e) => {
                eprintln!("audio: cannot take over {}: {e}", default.name);
                st.problem = Some(e.to_string());
                return;
            }
        };
        // Building the route connected this process to the audio system, if nothing had before: its own
        // output must be left out, or it would hear itself forever.
        if st.own.is_none() {
            st.own = audio_tap::process_object(std::process::id() as i32);
        }
        let Some(own) = st.own else {
            st.problem = Some("own audio process unknown".into());
            return;
        };
        if !excluded.contains(&own) {
            excluded.push(own);
            excluded.sort_unstable();
            if let Err(e) = route.tap().set_processes(&excluded) {
                st.problem = Some(e.to_string());
                return;
            }
        }
        trace::log(&format!("audio: taking over {}", default.name));
        st.device = Some(DeviceRoute {
            route,
            uid: default.uid,
            excluded,
            idle_since: None,
        });
    }

    /// Starts routes whose audio plays and stops ones that have been silent for a while.
    fn update_running(&self) {
        let now = Instant::now();
        let mut recheck = false;
        let mut st = self.state.borrow_mut();
        let mut problem = None;
        let mut decide = |route: &mut Route, playing: bool, idle_since: &mut Option<Instant>| {
            if playing {
                *idle_since = None;
                if !route.is_running()
                    && let Err(e) = route.start()
                {
                    problem = Some(e.to_string());
                }
            } else if route.is_running() {
                match idle_since {
                    Some(since)
                        if now.duration_since(*since).as_secs_f64() >= IDLE_STOP_SECONDS =>
                    {
                        route.stop();
                        *idle_since = None;
                    }
                    Some(_) => recheck = true,
                    None => {
                        *idle_since = Some(now);
                        recheck = true;
                    }
                }
            }
        };
        for tap in st.apps.values_mut() {
            if let AppTap::Route {
                route,
                objects,
                idle_since,
                ..
            } = tap
            {
                let playing = objects.iter().any(|&o| audio_tap::is_playing(o));
                decide(route, playing, idle_since);
            }
        }
        let others: Vec<ObjectId> = st.pids.keys().copied().collect();
        if let Some(device) = &mut st.device {
            let playing = others
                .iter()
                .filter(|o| !device.excluded.contains(o))
                .any(|&o| audio_tap::is_playing(o));
            decide(&mut device.route, playing, &mut device.idle_since);
        }
        if problem.is_some() {
            st.problem = problem;
        }
        drop(st);
        if recheck {
            let weak = self.me.clone();
            app::after(IDLE_STOP_SECONDS + 0.1, move || {
                if let Some(engine) = weak.upgrade() {
                    engine.update_running();
                }
            });
        }
    }
}
