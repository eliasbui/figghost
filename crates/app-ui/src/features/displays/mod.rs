//! The Displays tab of the notch: the brightness and contrast of each external display, set in the display itself
//! over DDC/CI as its own buttons would, the colour profile of every display, chosen among the ones macOS comes with
//! as System Settings offers them, and, when the person turns it on, the brightness keys for the external displays
//! with the volume indicator's look. It is off until the person turns it on.
//!
//! Every message to a display waits on it for tens of milliseconds, so all of them go through one worker thread,
//! which also keeps two messages from crossing on a display's bus; a slider that is dragged sends only its latest
//! position. The displays are looked for when the tab opens and when the keys need them, and never otherwise. No
//! permission is needed for the tab; the keys need Accessibility, as the volume indicator's do.

use super::AttachEnv;
use crate::{
    i18n::{self, Language},
    settings::model::Fault,
    trace,
};
use macos_sys::{
    MainThreadMarker, app,
    color_profiles::{self, ScreenProfiles},
    ddc::{self, Control, Level, Monitor},
    displays_ui::{DisplaysDisplay, DisplaysTexts, DisplaysView, PAGE_SIZE, Screen},
    event_tap::{EventTap, Verdict, mask},
    hud, permissions,
    system_keys::{SYSTEM_DEFINED, SystemKey},
    volume,
};
use serde::{Deserialize, Serialize};
use std::{
    cell::{Cell, RefCell},
    mem,
    rc::{Rc, Weak},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
        mpsc::{self, Receiver, Sender},
    },
    time::{Duration, Instant},
};

pub mod settings;
mod text;

/// The displays a key press finds are looked for again when the last look is older than this, so that one plugged
/// in since is found.
const STALE: Duration = Duration::from_secs(30);
/// A look asked for this soon after another is not made again.
const FRESH: Duration = Duration::from_secs(3);
/// Without Accessibility the keys are tried again this often, until access is granted.
const RETRY_SECONDS: f64 = 3.0;
/// Shift and Option held together select the fine step, as in the system; Control sends the keys to the external
/// displays while a built-in screen is lit, as it does for an Apple display.
const FINE_STEP_FLAGS: u64 = 0x0002_0000 | 0x0008_0000;
const CONTROL_FLAG: u64 = 0x0004_0000;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    pub enabled: bool,
    /// The brightness keys set the external displays. Needs Accessibility.
    pub keys: bool,
    /// Each card has a menu of the display's colour profiles, and every display gets a card.
    pub profiles: bool,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            enabled: false,
            keys: false,
            profiles: true,
        }
    }
}

// -- the worker -----------------------------------------------------------------------------------------------

enum Job {
    Look,
    Set {
        id: u64,
        control: Control,
        fraction: f64,
    },
    Step {
        up: bool,
        fine: bool,
    },
}

/// What is known of one display, sent from the worker to the main thread.
#[derive(Debug, Clone, PartialEq)]
struct Known {
    id: u64,
    name: String,
    brightness: Option<Level>,
    contrast: Option<Level>,
}

struct Attached {
    monitor: Monitor,
    known: Known,
}

/// The worker's side: the displays, opened once per look, and when they were last looked for.
struct Worker {
    displays: Vec<Attached>,
    looked: Option<Instant>,
    /// Whether any display answers, for the key tap to decide in no time whether a key is for this app.
    any: Arc<AtomicBool>,
}

impl Worker {
    fn look(&mut self) {
        if self.looked.is_some_and(|at| at.elapsed() < FRESH) {
            return;
        }
        trace::log("displays: looking");
        self.displays = ddc::monitors()
            .into_iter()
            .filter_map(|monitor| {
                let brightness = monitor.read(Control::Brightness)?;
                let contrast = monitor.read(Control::Contrast);
                let known = Known {
                    id: monitor.id,
                    name: monitor.name.clone(),
                    brightness: Some(brightness),
                    contrast,
                };
                Some(Attached { monitor, known })
            })
            .collect();
        self.looked = Some(Instant::now());
        self.any.store(!self.displays.is_empty(), Ordering::Relaxed);
        trace::log(&format!("displays: {} answer", self.displays.len()));
    }

    fn set(&mut self, id: u64, control: Control, fraction: f64) {
        let Some(display) = self.displays.iter_mut().find(|d| d.known.id == id) else {
            return;
        };
        let slot = match control {
            Control::Brightness => &mut display.known.brightness,
            Control::Contrast => &mut display.known.contrast,
        };
        let Some(level) = slot.as_mut() else {
            return;
        };
        let value = level.at(fraction);
        if display.monitor.write_level(control, value) {
            level.current = value;
        }
    }

    /// A brightness key: every display that answers takes the same step. Returns the level the indicator shows.
    fn step(&mut self, up: bool, fine: bool) -> Option<f64> {
        if self.looked.is_none_or(|at| at.elapsed() > STALE) {
            self.look();
        }
        let mut shown = None;
        let mut lost = false;
        for display in &mut self.displays {
            let Some(level) = display.known.brightness.as_mut() else {
                continue;
            };
            let next = volume::step_level(level.fraction() as f32, up, fine);
            let value = level.at(f64::from(next));
            if display.monitor.write_level(Control::Brightness, value) {
                level.current = value;
                shown.get_or_insert(level.fraction());
            } else {
                lost = true;
            }
        }
        if lost {
            // A display that took no message was unplugged; the next key finds what is there now.
            self.looked = None;
        }
        shown
    }

    fn known(&self) -> Vec<Known> {
        self.displays.iter().map(|d| d.known.clone()).collect()
    }
}

/// Runs the jobs in order. The latest position of a slider stands for every earlier one still waiting.
fn work(jobs: Receiver<Job>, any: Arc<AtomicBool>) {
    let mut worker = Worker {
        displays: Vec::new(),
        looked: None,
        any,
    };
    while let Ok(first) = jobs.recv() {
        let mut waiting: Vec<Job> = Vec::from([first]);
        waiting.extend(jobs.try_iter());
        let mut changed = false;
        for (i, job) in waiting.iter().enumerate() {
            match *job {
                Job::Look => {
                    worker.look();
                    changed = true;
                }
                Job::Set {
                    id,
                    control,
                    fraction,
                } => {
                    let later = waiting[i + 1..].iter().any(|j| {
                        matches!(j, Job::Set { id: other, control: c, .. } if *other == id && *c == control)
                    });
                    if !later {
                        worker.set(id, control, fraction);
                    }
                }
                Job::Step { up, fine } => {
                    if let Some(level) = worker.step(up, fine) {
                        hud::show_brightness_from_any_thread(level as f32);
                    }
                    changed = true;
                }
            }
        }
        if changed {
            let known = worker.known();
            app::on_main(move || {
                FEATURE.with(|slot| {
                    if let Some(f) = slot.borrow().upgrade() {
                        f.arrived(known);
                    }
                });
            });
        }
    }
}

// -- the keys -------------------------------------------------------------------------------------------------

/// Runs inside the input path, so it only reads flags and hands the work to the worker.
fn decide(
    swallowing: &mut bool,
    press: macos_sys::system_keys::KeyPress,
    flags: u64,
    any: &AtomicBool,
    jobs: &Sender<Job>,
) -> Verdict {
    let up = match press.key {
        SystemKey::BrightnessUp => true,
        SystemKey::BrightnessDown => false,
        _ => return Verdict::Pass,
    };
    if !press.down {
        return if mem::take(swallowing) {
            Verdict::Drop
        } else {
            Verdict::Pass
        };
    }
    *swallowing = false;
    // A lit built-in screen keeps its keys, unless Control is held.
    if flags & CONTROL_FLAG == 0 && ddc::built_in_screen_on() {
        return Verdict::Pass;
    }
    if !any.load(Ordering::Relaxed) {
        // Nothing found yet: the system has the key, and the next one finds what is plugged in.
        let _ = jobs.send(Job::Look);
        return Verdict::Pass;
    }
    let fine = flags & FINE_STEP_FLAGS == FINE_STEP_FLAGS;
    if jobs.send(Job::Step { up, fine }).is_err() {
        return Verdict::Pass;
    }
    *swallowing = true;
    Verdict::Drop
}

// -- the feature ----------------------------------------------------------------------------------------------

thread_local! {
    /// What the worker learns hops to the main thread, which finds the feature here.
    static FEATURE: RefCell<Weak<Feature>> = const { RefCell::new(Weak::new()) };
}

pub struct Feature {
    mtm: MainThreadMarker,
    view: Rc<DisplaysView>,
    config: RefCell<Config>,
    known: RefCell<Option<Vec<Known>>>,
    /// The displays the system shows with their colour profiles, read when the tab opens.
    color: RefCell<Vec<ScreenProfiles>>,
    /// What each card on the tab stands for, in the order shown.
    cards: RefCell<Vec<Card>>,
    jobs: RefCell<Option<Sender<Job>>>,
    any: Arc<AtomicBool>,
    tap: RefCell<Option<EventTap>>,
    retry: RefCell<Option<app::Timer>>,
    showing: Cell<bool>,
    me: Weak<Feature>,
}

/// Adds the tab. `None` without a notch, which is where the tab lives.
pub fn attach(env: &AttachEnv) -> Option<Rc<Feature>> {
    let notch = env.notch?;
    let language = i18n::current();
    let texts = DisplaysTexts {
        brightness: language.displays_brightness().to_owned(),
        contrast: language.displays_contrast().to_owned(),
        profile: language.displays_profile().to_owned(),
    };
    let feature = Rc::new_cyclic(|me: &Weak<Feature>| {
        let (weak, chooser) = (me.clone(), me.clone());
        let view = DisplaysView::new(
            env.mtm,
            notch.page_area(PAGE_SIZE.width, PAGE_SIZE.height),
            texts,
            Rc::new(move |index, control, fraction| {
                if let Some(f) = weak.upgrade() {
                    f.moved(index, control, fraction);
                }
            }),
            Rc::new(move |index, choice| {
                if let Some(f) = chooser.upgrade() {
                    f.chosen(index, choice);
                }
            }),
        );
        Feature {
            mtm: env.mtm,
            view,
            config: RefCell::new(env.config.displays.clone()),
            known: RefCell::default(),
            color: RefCell::default(),
            cards: RefCell::default(),
            jobs: RefCell::default(),
            any: Arc::default(),
            tap: RefCell::default(),
            retry: RefCell::default(),
            showing: Cell::new(false),
            me: me.clone(),
        }
    });
    FEATURE.with(|slot| *slot.borrow_mut() = Rc::downgrade(&feature));
    let (shown, hidden) = (Rc::downgrade(&feature), Rc::downgrade(&feature));
    notch.add_page(feature.view.page(
        language.displays_tab().to_owned(),
        move || {
            if let Some(f) = shown.upgrade() {
                f.showing.set(true);
                f.read_profiles();
                f.render();
                if f.config.borrow().enabled {
                    f.send(Job::Look);
                }
            }
        },
        move || {
            if let Some(f) = hidden.upgrade() {
                f.showing.set(false);
            }
        },
    ));
    if feature.wants_keys() {
        feature.keep_trying();
    }
    feature.render();
    Some(feature)
}

impl Feature {
    pub fn configure(&self, config: &Config) -> Result<(), Fault> {
        let was = self.config.replace(config.clone());
        if !config.enabled {
            *self.known.borrow_mut() = None;
        }
        if self.wants_keys() {
            if self.tap.borrow().is_none() {
                self.retry.borrow_mut().take();
                let tap = self.start_tap().map_err(|e| {
                    eprintln!(
                        "displays: the keys cannot start ({e:?}); Accessibility access is needed"
                    );
                    permissions::accessibility_trusted(true);
                    Fault::new("accessibility_needed", "displays")
                })?;
                *self.tap.borrow_mut() = Some(tap);
            }
        } else {
            self.retry.borrow_mut().take();
            self.tap.borrow_mut().take();
        }
        if config.enabled && !was.enabled && self.showing.get() {
            self.send(Job::Look);
        }
        if self.showing.get() {
            self.read_profiles();
        }
        self.render();
        Ok(())
    }

    /// Reads the displays and their colour profiles, while the tab can show them.
    fn read_profiles(&self) {
        let config = self.config.borrow();
        *self.color.borrow_mut() = if config.enabled && config.profiles {
            color_profiles::screens(self.mtm)
        } else {
            Vec::new()
        };
    }

    /// A profile was chosen in a card's menu: the display takes it, as in System Settings. The display's own
    /// profile takes the choice away instead, which puts it back.
    fn chosen(&self, index: usize, choice: usize) {
        let card = self.cards.borrow().get(index).copied();
        let Some(at) = card.and_then(|c| c.color) else {
            return;
        };
        let Some((display, name, path, own)) = self.color.borrow().get(at).and_then(|screen| {
            let profile = screen.profiles.get(choice)?;
            Some((
                screen.display,
                profile.name.clone(),
                profile.path.clone(),
                choice < screen.own,
            ))
        }) else {
            return;
        };
        let done = color_profiles::set(display, (!own).then_some(path.as_path()));
        trace::log(&format!(
            "displays: profile {name} for display {display} {}",
            if done { "set" } else { "refused" }
        ));
        // ColorSync may take a moment to report the change, so what was chosen is shown until the tab opens again.
        if done && let Some(screen) = self.color.borrow_mut().get_mut(at) {
            screen.current = Some(choice);
        }
        self.render();
    }

    fn wants_keys(&self) -> bool {
        let config = self.config.borrow();
        config.enabled && config.keys
    }

    /// Hands a job to the worker, which starts with the first one.
    fn send(&self, job: Job) {
        let mut jobs = self.jobs.borrow_mut();
        let sender = jobs.get_or_insert_with(|| {
            let (sender, receiver) = mpsc::channel();
            let any = Arc::clone(&self.any);
            std::thread::Builder::new()
                .name("fig-displays".into())
                .spawn(move || work(receiver, any))
                .expect("a thread for the displays");
            sender
        });
        let _ = sender.send(job);
    }

    /// The way to the worker for the key tap, which starts the worker with a look so that the first key finds the
    /// displays.
    fn sender(&self) -> Sender<Job> {
        self.send(Job::Look);
        self.jobs.borrow().clone().expect("made by send")
    }

    fn start_tap(&self) -> Result<EventTap, macos_sys::event_tap::TapError> {
        let (jobs, any) = (self.sender(), Arc::clone(&self.any));
        let mut swallowing = false;
        EventTap::spawn(mask(SYSTEM_DEFINED), move |_, event| {
            match event.system_key() {
                Some(press) => decide(&mut swallowing, press, event.flags(), &any, &jobs),
                None => Verdict::Pass,
            }
        })
    }

    /// Starts the keys, or tries again every few seconds until Accessibility is granted. For the start of the app,
    /// where there is no page to tell.
    fn keep_trying(&self) {
        if let Ok(tap) = self.start_tap() {
            *self.tap.borrow_mut() = Some(tap);
            return;
        }
        let weak = self.me.clone();
        *self.retry.borrow_mut() = Some(app::every(RETRY_SECONDS, move || {
            if let Some(f) = weak.upgrade()
                && f.wants_keys()
                && f.tap.borrow().is_none()
                && let Ok(tap) = f.start_tap()
            {
                *f.tap.borrow_mut() = Some(tap);
                f.retry.borrow_mut().take();
            }
        }));
    }

    /// A slider moved: the worker sets the display, and what is known follows without drawing the page again.
    fn moved(&self, index: usize, control: Control, fraction: f64) {
        let Some(at) = self.cards.borrow().get(index).and_then(|c| c.ddc) else {
            return;
        };
        let mut known = self.known.borrow_mut();
        let Some(display) = known.as_mut().and_then(|k| k.get_mut(at)) else {
            return;
        };
        let slot = match control {
            Control::Brightness => &mut display.brightness,
            Control::Contrast => &mut display.contrast,
        };
        if let Some(level) = slot.as_mut() {
            level.current = level.at(fraction);
        }
        let id = display.id;
        drop(known);
        self.send(Job::Set {
            id,
            control,
            fraction,
        });
    }

    fn arrived(&self, known: Vec<Known>) {
        if !self.config.borrow().enabled {
            return;
        }
        *self.known.borrow_mut() = Some(known);
        self.render();
    }

    fn render(&self) {
        let (shown, cards) = display(
            i18n::current(),
            self.config.borrow().enabled,
            self.known.borrow().as_deref(),
            &self.color.borrow(),
        );
        *self.cards.borrow_mut() = cards;
        self.view.show(shown);
    }
}

/// What a card stands for: a display the system shows, by its place in the colour list, the display of the same
/// name that answers over DDC/CI, by its place in what is known, or both.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Card {
    color: Option<usize>,
    ddc: Option<usize>,
}

/// A name without the " (2)" macOS adds when two displays have the same one.
fn base_name(name: &str) -> &str {
    match name.rsplit_once(" (") {
        Some((base, n))
            if n.strip_suffix(')')
                .is_some_and(|n| n.parse::<u32>().is_ok()) =>
        {
            base
        }
        _ => name,
    }
}

/// One card for each display the system shows, joined to the first display of the same name that answers over
/// DDC/CI; one that answers and has no such twin gets a card of its own after them.
// ponytail: joined by name, as nothing else ties a DDC port to a display; two displays of one model pair in order.
fn join(screens: &[ScreenProfiles], known: &[Known]) -> Vec<Card> {
    let mut used = vec![false; known.len()];
    let mut cards: Vec<Card> = screens
        .iter()
        .enumerate()
        .map(|(n, screen)| {
            let ddc = known.iter().enumerate().position(|(k, display)| {
                !used[k] && !display.name.is_empty() && display.name == base_name(&screen.name)
            });
            if let Some(k) = ddc {
                used[k] = true;
            }
            Card {
                color: Some(n),
                ddc,
            }
        })
        .collect();
    cards.extend((0..known.len()).filter(|&k| !used[k]).map(|k| Card {
        color: None,
        ddc: Some(k),
    }));
    cards
}

/// The picture of the tab, a card for each display or why there is none, and what each card stands for. Without
/// colour profiles only the displays that answer over DDC/CI have a card.
fn display(
    language: Language,
    enabled: bool,
    known: Option<&[Known]>,
    color: &[ScreenProfiles],
) -> (DisplaysDisplay, Vec<Card>) {
    let note = |title: &str, hint: &str| {
        let shown = DisplaysDisplay {
            screens: Vec::new(),
            note: Some((title.to_owned(), hint.to_owned())),
        };
        (shown, Vec::new())
    };
    if !enabled {
        return note(language.displays_off_title(), language.displays_off_hint());
    }
    if color.is_empty() {
        match known {
            None => return note(language.displays_looking(), ""),
            Some([]) => {
                return note(
                    language.displays_none_title(),
                    language.displays_none_hint(),
                );
            }
            Some(_) => {}
        }
    }
    let known = known.unwrap_or_default();
    let cards = join(color, known);
    let screens = cards
        .iter()
        .enumerate()
        .map(|(i, card)| {
            let ddc = card.ddc.and_then(|k| known.get(k));
            let profiles = card.color.and_then(|n| color.get(n));
            let name = profiles
                .map(|p| p.name.clone())
                .or_else(|| ddc.map(|k| k.name.clone()))
                .filter(|name| !name.is_empty())
                .unwrap_or_else(|| language.displays_unnamed(i + 1));
            Screen {
                name,
                brightness: ddc.and_then(|k| k.brightness).map(Level::fraction),
                contrast: ddc.and_then(|k| k.contrast).map(Level::fraction),
                profiles: profiles.map_or_else(Vec::new, |p| {
                    p.profiles
                        .iter()
                        .map(|profile| profile.name.clone())
                        .collect()
                }),
                profile: profiles.and_then(|p| p.current),
            }
        })
        .collect();
    (
        DisplaysDisplay {
            screens,
            note: None,
        },
        cards,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use macos_sys::system_keys::KeyPress;

    const LANGUAGES: [Language; 2] = [Language::English, Language::Vietnamese];

    fn known(name: &str, contrast: bool) -> Known {
        Known {
            id: 7,
            name: name.into(),
            brightness: Some(Level {
                current: 40,
                max: 80,
            }),
            contrast: contrast.then_some(Level {
                current: 75,
                max: 100,
            }),
        }
    }

    #[test]
    fn the_feature_is_off_by_default_and_old_files_load() {
        let config: Config = serde_json::from_str("{}").unwrap();
        assert_eq!(config, Config::default());
        assert!(!config.enabled && !config.keys);
        assert!(config.profiles, "the profiles show once the tab is on");
        let partial: Config = serde_json::from_str(r#"{ "enabled": true, "future": 1 }"#).unwrap();
        assert!(partial.enabled && !partial.keys && partial.profiles);
    }

    fn screen(name: &str, current: Option<usize>) -> ScreenProfiles {
        ScreenProfiles {
            display: 1,
            name: name.into(),
            profiles: ["Own", "Display P3", "sRGB IEC61966-2.1"]
                .map(|n| color_profiles::Profile {
                    name: n.into(),
                    path: format!("/p/{n}.icc").into(),
                })
                .to_vec(),
            current,
            own: 1,
        }
    }

    #[test]
    fn every_display_gets_a_card_with_its_profiles_and_the_sliders_of_its_twin() {
        let color = [
            screen("Built-in Retina Display", Some(0)),
            screen("LG HDR 4K (1)", Some(2)),
            screen("LG HDR 4K (2)", None),
        ];
        let known = [
            known("LG HDR 4K", true),
            known("Hidden", false),
            known("LG HDR 4K", false),
        ];
        let (shown, cards) = display(Language::English, true, Some(&known), &color);
        assert_eq!(
            cards,
            [
                Card {
                    color: Some(0),
                    ddc: None
                },
                Card {
                    color: Some(1),
                    ddc: Some(0)
                },
                Card {
                    color: Some(2),
                    ddc: Some(2)
                },
                Card {
                    color: None,
                    ddc: Some(1)
                },
            ],
            "the two of one model pair in order; one no screen names gets a card after them"
        );
        let built_in = &shown.screens[0];
        assert!(built_in.brightness.is_none() && built_in.profiles.len() == 3);
        assert_eq!(built_in.profile, Some(0));
        let lg = &shown.screens[1];
        assert_eq!((lg.name.as_str(), lg.profile), ("LG HDR 4K (1)", Some(2)));
        assert!(lg.brightness.is_some() && lg.contrast.is_some());
        assert!(
            shown.screens[2].contrast.is_none(),
            "the second LG has its own twin"
        );
        assert!(shown.screens[3].profiles.is_empty());
        // While nothing has answered yet, the cards are there with their profiles.
        let (waiting, _) = display(Language::English, true, None, &color);
        assert!(waiting.note.is_none() && waiting.screens.len() == 3);
    }

    #[test]
    fn a_number_macos_adds_is_not_part_of_the_name() {
        assert_eq!(base_name("LG HDR 4K (2)"), "LG HDR 4K");
        assert_eq!(base_name("Studio (Left)"), "Studio (Left)");
        assert_eq!(base_name("S24i-30"), "S24i-30");
    }

    #[test]
    fn each_display_is_a_card_with_the_settings_it_has() {
        for language in LANGUAGES {
            let (shown, _) = display(
                language,
                true,
                Some(&[known("S24i-30", true), known("", false)]),
                &[],
            );
            assert!(shown.note.is_none());
            assert_eq!(shown.screens.len(), 2);
            assert_eq!(shown.screens[0].name, "S24i-30");
            assert_eq!(shown.screens[0].brightness, Some(0.5));
            assert_eq!(shown.screens[0].contrast, Some(0.75));
            assert_eq!(shown.screens[1].name, language.displays_unnamed(2));
            assert!(shown.screens[1].contrast.is_none());
        }
    }

    #[test]
    fn off_looking_and_none_each_say_something_different() {
        for language in LANGUAGES {
            let off = display(language, false, Some(&[known("A", true)]), &[])
                .0
                .note
                .unwrap();
            let looking = display(language, true, None, &[]).0.note.unwrap();
            let none = display(language, true, Some(&[]), &[]).0.note.unwrap();
            assert!(!off.0.is_empty() && !off.1.is_empty() && !none.1.is_empty());
            assert_ne!(off.0, looking.0);
            assert_ne!(looking.0, none.0);
        }
        assert_ne!(
            display(Language::English, false, None, &[]).0.note,
            display(Language::Vietnamese, false, None, &[]).0.note
        );
    }

    fn press(key: SystemKey, down: bool) -> KeyPress {
        KeyPress {
            key,
            down,
            repeat: false,
        }
    }

    #[test]
    fn the_keys_go_to_the_displays_only_when_one_answers() {
        let (jobs, received) = mpsc::channel();
        let any = AtomicBool::new(false);
        let mut swallowing = false;
        let built_in = ddc::built_in_screen_on();
        let flags = if built_in { CONTROL_FLAG } else { 0 };
        // Nothing found yet: the system keeps the key, and a look is asked for.
        let first = decide(
            &mut swallowing,
            press(SystemKey::BrightnessUp, true),
            flags,
            &any,
            &jobs,
        );
        assert!(matches!(first, Verdict::Pass));
        assert!(matches!(received.try_recv(), Ok(Job::Look)));
        // Found: the press and its release are taken, and the step is the worker's.
        any.store(true, Ordering::Relaxed);
        let down = decide(
            &mut swallowing,
            press(SystemKey::BrightnessDown, true),
            flags,
            &any,
            &jobs,
        );
        assert!(matches!(down, Verdict::Drop));
        assert!(matches!(
            received.try_recv(),
            Ok(Job::Step {
                up: false,
                fine: false
            })
        ));
        let up = decide(
            &mut swallowing,
            press(SystemKey::BrightnessDown, false),
            flags,
            &any,
            &jobs,
        );
        assert!(matches!(up, Verdict::Drop));
        // The fine step, as the system has it.
        let fine = flags | FINE_STEP_FLAGS;
        let _ = decide(
            &mut swallowing,
            press(SystemKey::BrightnessUp, true),
            fine,
            &any,
            &jobs,
        );
        assert!(matches!(
            received.try_recv(),
            Ok(Job::Step {
                up: true,
                fine: true
            })
        ));
        // Other keys are never touched.
        for key in [
            SystemKey::SoundUp,
            SystemKey::PlayPause,
            SystemKey::Other(21),
        ] {
            assert!(matches!(
                decide(&mut swallowing, press(key, true), flags, &any, &jobs),
                Verdict::Pass
            ));
        }
        assert!(received.try_recv().is_err());
    }
}
