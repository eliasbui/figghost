//! The System tab of the notch: the processor core by core, the memory and the swap, the network, and a row of
//! smaller figures (graphics, startup disk, heat, fans, battery). It is off until the person turns it on, and even then
//! nothing is read while the tab is not showing: the readings start when the tab opens and stop when it closes,
//! so an idle app does no work for it.
//!
//! The exception is the figures the person chooses to keep in the menu bar (processor, memory, network, graphics, heat
//! and the startup disk, each off until chosen, each drawn as text, a label over its value, a graph or the cores): those
//! are read at the same pace for as long as one of them is chosen, and a click on them opens the tab.
//!
//! Nothing here needs a permission. The counters come from the kernel and from the power and disk services, and
//! none of them says what any app is doing.

use super::AttachEnv;
use crate::{
    features::islands::logic::human_size,
    i18n::{self, Language},
    settings::model::Fault,
    trace,
};
use macos_sys::{
    MainThreadMarker, app,
    notch::Notch,
    sensors,
    status_item::TextItem,
    style::Tone,
    sysinfo::{self, Interface, Pressure, Thermal},
    system_bar,
    system_ui::{
        CpuDisplay, HISTORY, MemoryDisplay, NetworkDisplay, Note, PAGE_SIZE, Stat, SystemDisplay,
        SystemView,
    },
};
use serde::{Deserialize, Serialize};
use std::{
    cell::{Cell, RefCell},
    rc::{Rc, Weak},
    slice,
    time::Instant,
};

use logic::{Figure, Kind};

pub(crate) mod logic;
pub mod settings;
mod text;

/// How often the tab can be read while it shows, in seconds.
pub const SECONDS: [u32; 3] = [1, 2, 5];
/// The first reading after the tab opens is followed this soon by another, so that the figures that are
/// differences do not wait a whole interval to appear.
const FIRST_SECONDS: f64 = 0.4;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    pub enabled: bool,
    /// Seconds between two readings while the tab shows; one of `SECONDS`.
    pub seconds: u32,
    pub processor: bool,
    pub memory: bool,
    pub network: bool,
    /// The row of small figures: graphics, startup disk, heat, fans and battery.
    pub details: bool,
    /// The figures kept in the menu bar, read even while the tab is closed.
    pub bar_processor: bool,
    pub bar_memory: bool,
    pub bar_network: bool,
    pub bar_gpu: bool,
    pub bar_heat: bool,
    pub bar_disk: bool,
    /// How each figure is drawn in the menu bar.
    pub bar_processor_kind: Kind,
    pub bar_memory_kind: Kind,
    pub bar_network_kind: Kind,
    pub bar_gpu_kind: Kind,
    pub bar_heat_kind: Kind,
    pub bar_disk_kind: Kind,
    /// One item of the menu bar for all the figures, or one for each, which Command-drag can move apart.
    pub bar_combined: bool,
    /// Graphs and cores in the accent instead of the colour of the menu bar's text.
    pub bar_color: bool,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            enabled: false,
            seconds: 1,
            processor: true,
            memory: true,
            network: true,
            details: true,
            bar_processor: false,
            bar_memory: false,
            bar_network: false,
            bar_gpu: false,
            bar_heat: false,
            bar_disk: false,
            bar_processor_kind: Kind::default(),
            bar_memory_kind: Kind::default(),
            bar_network_kind: Kind::default(),
            bar_gpu_kind: Kind::default(),
            bar_heat_kind: Kind::default(),
            bar_disk_kind: Kind::default(),
            bar_combined: true,
            bar_color: false,
        }
    }
}

impl Config {
    /// A config file can say anything; a number that is not one of the choices is read as the default.
    pub fn sanitized(mut self) -> Self {
        if !SECONDS.contains(&self.seconds) {
            self.seconds = Self::default().seconds;
        }
        for (figure, kind) in self.bar_kinds() {
            if !kind.fits(figure) {
                *kind = Kind::default();
            }
        }
        self
    }

    fn bar_kinds(&mut self) -> [(Figure, &mut Kind); 6] {
        [
            (Figure::Processor, &mut self.bar_processor_kind),
            (Figure::Memory, &mut self.bar_memory_kind),
            (Figure::Network, &mut self.bar_network_kind),
            (Figure::Gpu, &mut self.bar_gpu_kind),
            (Figure::Heat, &mut self.bar_heat_kind),
            (Figure::Disk, &mut self.bar_disk_kind),
        ]
    }

    /// The figures chosen for the menu bar, in the order they stand there, each with how it is drawn.
    pub fn bar_figures(&self) -> Vec<(Figure, Kind)> {
        [
            (
                Figure::Processor,
                self.bar_processor,
                self.bar_processor_kind,
            ),
            (Figure::Memory, self.bar_memory, self.bar_memory_kind),
            (Figure::Network, self.bar_network, self.bar_network_kind),
            (Figure::Gpu, self.bar_gpu, self.bar_gpu_kind),
            (Figure::Heat, self.bar_heat, self.bar_heat_kind),
            (Figure::Disk, self.bar_disk, self.bar_disk_kind),
        ]
        .into_iter()
        .filter(|(_, on, _)| *on)
        .map(|(figure, _, kind)| (figure, kind))
        .collect()
    }

    /// How many items the figures take in the menu bar: none while the feature is off.
    fn bar_items(&self) -> usize {
        let figures = if self.enabled {
            self.bar_figures().len()
        } else {
            0
        };
        if self.bar_combined {
            figures.min(1)
        } else {
            figures
        }
    }

    fn shows_anything(&self) -> bool {
        self.processor || self.memory || self.network || self.details
    }
}

/// What the counters said at the last reading, for the differences.
struct Reading {
    at: Instant,
    ticks: Vec<[u32; 4]>,
    interfaces: Vec<Interface>,
}

pub struct Feature {
    mtm: MainThreadMarker,
    notch: Weak<Notch>,
    view: Rc<SystemView>,
    config: RefCell<Config>,
    ticker: RefCell<Option<app::Timer>>,
    last: RefCell<Option<Reading>>,
    down: RefCell<logic::History>,
    up: RefCell<logic::History>,
    showing: Cell<bool>,
    /// Bumped whenever the readings start over; a delayed reading that finds another number does nothing.
    epoch: Cell<u64>,
    /// The figures in the menu bar, with their own readings so that the tab opening and closing does not touch them.
    /// One item for all of them, or one for each in the order of `Config::bar_figures`.
    bar: RefCell<Vec<TextItem>>,
    /// The last levels of each figure, for a graph, in the order of `Figure`.
    bar_history: RefCell<[logic::History; 6]>,
    bar_ticker: RefCell<Option<app::Timer>>,
    bar_last: RefCell<Option<Reading>>,
    me: Weak<Feature>,
}

/// Adds the tab. `None` without a notch, which is where the tab lives.
pub fn attach(env: &AttachEnv) -> Option<Rc<Feature>> {
    let notch = env.notch?;
    let view = SystemView::new(env.mtm, notch.page_area(PAGE_SIZE.width, PAGE_SIZE.height));
    let feature = Rc::new_cyclic(|me| Feature {
        mtm: env.mtm,
        notch: Rc::downgrade(notch),
        view: Rc::clone(&view),
        config: RefCell::new(env.config.system.clone().sanitized()),
        ticker: RefCell::default(),
        last: RefCell::default(),
        down: RefCell::default(),
        up: RefCell::default(),
        showing: Cell::new(false),
        epoch: Cell::new(0),
        bar: RefCell::default(),
        bar_history: RefCell::default(),
        bar_ticker: RefCell::default(),
        bar_last: RefCell::default(),
        me: me.clone(),
    });
    let (shown, hidden) = (Rc::downgrade(&feature), Rc::downgrade(&feature));
    notch.add_page(view.page(
        i18n::current().system_tab().to_owned(),
        move || {
            if let Some(f) = shown.upgrade() {
                f.showing.set(true);
                f.begin();
            }
        },
        move || {
            if let Some(f) = hidden.upgrade() {
                f.showing.set(false);
                f.stop();
            }
        },
    ));
    feature.place_bar();
    Some(feature)
}

impl Feature {
    /// Applies the settings after a change; a tab that is showing starts over with them.
    pub fn configure(&self, config: &Config) -> Result<(), Fault> {
        *self.config.borrow_mut() = config.clone().sanitized();
        if self.showing.get() {
            self.begin();
        }
        self.place_bar();
        Ok(())
    }

    /// Puts the chosen figures in the menu bar and reads them from now on, or takes them away when none is chosen.
    fn place_bar(&self) {
        self.bar_ticker.borrow_mut().take();
        self.bar_last.borrow_mut().take();
        self.bar_history
            .borrow_mut()
            .iter_mut()
            .for_each(logic::History::clear);
        let config = self.config.borrow().clone();
        let wanted = config.bar_items();
        if self.bar.borrow().len() != wanted {
            let mut items = self.bar.borrow_mut();
            items.clear();
            // A new item goes to the left of those already there, so the last figure is placed first.
            items.extend((0..wanted).map(|_| self.bar_item()));
            items.reverse();
            trace::log(&if wanted == 0 {
                "system: menu bar figures removed".to_owned()
            } else {
                format!("system: menu bar figures placed in {wanted} items")
            });
        }
        if wanted == 0 {
            return;
        }
        self.read_bar();
        let weak = self.me.clone();
        app::after(FIRST_SECONDS, move || {
            if let Some(f) = weak.upgrade() {
                f.read_bar();
            }
        });
        let weak = self.me.clone();
        *self.bar_ticker.borrow_mut() = Some(app::every(f64::from(config.seconds), move || {
            if let Some(f) = weak.upgrade() {
                f.read_bar();
            }
        }));
    }

    fn bar_item(&self) -> TextItem {
        let weak = self.me.clone();
        TextItem::new(self.mtm, i18n::current().system_tab(), "", move || {
            if let Some(f) = weak.upgrade() {
                f.open_tab();
            }
        })
    }

    /// Reads only the figures the menu bar shows and draws them there.
    fn read_bar(&self) {
        let config = self.config.borrow().clone();
        let items = self.bar.borrow();
        if items.is_empty() {
            return;
        }
        let figures = config.bar_figures();
        let chosen = |figure: Figure| figures.iter().any(|(f, _)| *f == figure);
        let now = Reading {
            at: Instant::now(),
            ticks: if chosen(Figure::Processor) {
                sysinfo::cpu_ticks().unwrap_or_default()
            } else {
                Vec::new()
            },
            interfaces: if chosen(Figure::Network) {
                sysinfo::interfaces()
            } else {
                Vec::new()
            },
        };
        let (loads, rates) = match self.bar_last.borrow().as_ref() {
            Some(before) => {
                let seconds = now.at.duration_since(before.at).as_secs_f64();
                (
                    logic::core_loads(&before.ticks, &now.ticks),
                    logic::rates(&before.interfaces, &now.interfaces, seconds),
                )
            }
            None => (vec![0.0; now.ticks.len()], (0.0, 0.0)),
        };
        let reading = logic::BarReading {
            slow: if chosen(Figure::Processor) {
                sysinfo::core_kinds().map_or(0, |(slow, _)| slow)
            } else {
                0
            },
            loads,
            memory: chosen(Figure::Memory)
                .then(sysinfo::memory)
                .flatten()
                .filter(|m| m.total > 0)
                .map(|m| m.used() as f64 / m.total as f64),
            rates,
            gpu: chosen(Figure::Gpu)
                .then(sysinfo::gpu_percent)
                .flatten()
                .map(|p| f64::from(p) / 100.0),
            celsius: chosen(Figure::Heat)
                .then(sensors::processor_celsius)
                .flatten(),
            disk: chosen(Figure::Disk).then(sysinfo::disk).flatten(),
        };
        *self.bar_last.borrow_mut() = Some(now);
        trace::log("system: menu bar read");
        let labels = i18n::current().system_bar_labels();
        let mut history = self.bar_history.borrow_mut();
        let cells: Vec<system_bar::Cell> = figures
            .iter()
            .map(|&(figure, kind)| {
                let line = &mut history[figure as usize];
                line.push(reading.level(figure), system_bar::GRAPH_POINTS);
                logic::bar_cell(labels, figure, kind, &reading, &line.values())
            })
            .collect();
        if config.bar_combined {
            items[0].set_image(
                &system_bar::image(&cells, config.bar_color),
                &logic::describe(labels, &figures, &reading),
            );
        } else {
            for ((item, cell), figure) in items.iter().zip(&cells).zip(&figures) {
                item.set_image(
                    &system_bar::image(slice::from_ref(cell), config.bar_color),
                    &logic::describe(labels, slice::from_ref(figure), &reading),
                );
            }
        }
    }

    /// Opens the notch on this tab, for a click on the figures in the menu bar.
    fn open_tab(&self) {
        let Some(notch) = self.notch.upgrade() else {
            return;
        };
        if let Some(index) = notch.page_ids().iter().position(|(id, _)| *id == "system") {
            notch.open_page(index);
        }
    }

    fn stop(&self) {
        self.epoch.set(self.epoch.get() + 1);
        if self.ticker.borrow_mut().take().is_some() {
            trace::log("system: reading stops");
        }
        self.last.borrow_mut().take();
        self.down.borrow_mut().clear();
        self.up.borrow_mut().clear();
    }

    /// Starts reading from nothing, or shows why it does not.
    fn begin(&self) {
        self.stop();
        let config = self.config.borrow().clone();
        let language = i18n::current();
        if !config.enabled || !config.shows_anything() {
            trace::log("system: nothing to read");
            self.view.show(SystemDisplay {
                note: Some(note(language, config.enabled)),
                ..SystemDisplay::default()
            });
            return;
        }
        trace::log("system: reading starts");
        self.read();
        let (epoch, weak) = (self.epoch.get(), self.me.clone());
        app::after(FIRST_SECONDS, move || {
            if let Some(f) = weak.upgrade()
                && f.epoch.get() == epoch
            {
                f.read();
            }
        });
        let weak = self.me.clone();
        *self.ticker.borrow_mut() = Some(app::every(f64::from(config.seconds), move || {
            if let Some(f) = weak.upgrade() {
                f.read();
            }
        }));
    }

    /// Reads the system, works out what changed since the last reading, and draws it.
    fn read(&self) {
        trace::log("system: read");
        let now = Reading {
            at: Instant::now(),
            ticks: sysinfo::cpu_ticks().unwrap_or_default(),
            interfaces: sysinfo::interfaces(),
        };
        let (loads, down, up) = match self.last.borrow().as_ref() {
            Some(before) => {
                let seconds = now.at.duration_since(before.at).as_secs_f64();
                let (down, up) = logic::rates(&before.interfaces, &now.interfaces, seconds);
                self.down.borrow_mut().push(down, HISTORY);
                self.up.borrow_mut().push(up, HISTORY);
                (logic::core_loads(&before.ticks, &now.ticks), down, up)
            }
            None => (vec![0.0; now.ticks.len()], 0.0, 0.0),
        };
        *self.last.borrow_mut() = Some(now);
        self.view.show(display(
            i18n::current(),
            &self.config.borrow(),
            &loads,
            (down, up),
            (&self.down.borrow().values(), &self.up.borrow().values()),
        ));
    }
}

/// The words that stand in for the figures: the feature is off, or every card is.
fn note(language: Language, enabled: bool) -> Note {
    if enabled {
        Note {
            title: language.system_empty_title().to_owned(),
            hint: language.system_empty_hint().to_owned(),
        }
    } else {
        Note {
            title: language.system_off_title().to_owned(),
            hint: language.system_off_hint().to_owned(),
        }
    }
}

/// The picture of the tab for the figures read now. A card the settings leave out is not read at all.
fn display(
    language: Language,
    config: &Config,
    loads: &[f64],
    (down, up): (f64, f64),
    (down_history, up_history): (&[f64], &[f64]),
) -> SystemDisplay {
    let cpu = config.processor.then(|| {
        let kinds = sysinfo::core_kinds();
        CpuDisplay {
            title: language.system_cpu().to_owned(),
            value: logic::percent_text(logic::overall(loads)),
            cores: loads.to_vec(),
            slow: kinds.map_or(0, |(slow, _)| slow),
            slow_label: language.system_efficiency().to_owned(),
            fast_label: language.system_performance().to_owned(),
        }
    });
    let memory = if config.memory {
        memory_card(language)
    } else {
        None
    };
    let network = config.network.then(|| NetworkDisplay {
        title: language.system_network().to_owned(),
        down: logic::rate_text(down),
        up: logic::rate_text(up),
        down_history: logic::scaled(down_history, logic::NETWORK_FLOOR),
        up_history: logic::scaled(up_history, logic::NETWORK_FLOOR),
    });
    SystemDisplay {
        cpu,
        memory,
        network,
        stats: if config.details {
            details(language)
        } else {
            Vec::new()
        },
        note: None,
    }
}

fn memory_card(language: Language) -> Option<MemoryDisplay> {
    let m = sysinfo::memory()?;
    let pressure = sysinfo::memory_pressure();
    let swap = sysinfo::swap().map_or_else(String::new, |(_, used)| logic::memory_text(used));
    Some(MemoryDisplay {
        title: language.system_memory().to_owned(),
        used: logic::memory_text(m.used()),
        of: format!("/ {}", logic::memory_text(m.total)),
        shares: logic::shares(&m),
        legend: [
            language.system_apps(&logic::memory_text(m.app)),
            language.system_wired(&logic::memory_text(m.wired)),
            language.system_compressed(&logic::memory_text(m.compressed)),
        ],
        pressure: language.system_pressure(pressure).to_owned(),
        pressure_tone: match pressure {
            Pressure::Normal => Tone::Green,
            Pressure::Warning => Tone::Orange,
            Pressure::Critical => Tone::Red,
        },
        swap_title: language.system_swap().to_uppercase(),
        swap,
    })
}

/// The row of small cards. Each is left out when this Mac has nothing to say for it, the battery most of all.
fn details(language: Language) -> Vec<Stat> {
    let mut stats = Vec::new();
    if let Some(gpu) = sysinfo::gpu_percent() {
        stats.push(Stat {
            title: language.system_gpu().to_owned(),
            value: format!("{gpu:.0}%"),
            detail: String::new(),
            fraction: Some(f64::from(gpu) / 100.0),
            tone: Tone::Blue,
        });
    }
    if let Some((free, total)) = sysinfo::disk() {
        stats.push(Stat {
            title: language.system_disk().to_owned(),
            value: human_size(free),
            detail: language.system_disk_free(&human_size(total)),
            fraction: Some(1.0 - free as f64 / total as f64),
            tone: Tone::Blue,
        });
    }
    let thermal = sysinfo::thermal();
    let state = language.system_thermal_state(thermal).to_owned();
    // With a reading of the die, the figure is the heat and the state goes under it.
    let (value, detail) = match sensors::processor_celsius() {
        Some(celsius) => (logic::celsius_text(celsius), state),
        None => (state, String::new()),
    };
    stats.push(Stat {
        title: language.system_thermal().to_owned(),
        value,
        detail,
        fraction: None,
        tone: match thermal {
            Thermal::Nominal => Tone::Green,
            Thermal::Fair => Tone::Orange,
            Thermal::Serious | Thermal::Critical => Tone::Red,
        },
    });
    let fans = sensors::fans();
    if let Some((fan, share)) = logic::fastest(&fans) {
        stats.push(Stat {
            title: language.system_fan().to_owned(),
            value: logic::rpm_text(fan.rpm),
            detail: language.system_fan_detail(fans.len(), &logic::rpm_text(fan.max)),
            fraction: Some(share),
            tone: Tone::Blue,
        });
    }
    if let Some(wear) = sysinfo::battery_wear() {
        stats.push(Stat {
            title: language.system_battery().to_owned(),
            value: format!("{}%", wear.health),
            detail: language.system_cycles(wear.cycles),
            fraction: Some(f64::from(wear.health) / 100.0),
            tone: if wear.health >= 80 {
                Tone::Green
            } else {
                Tone::Orange
            },
        });
    }
    stats
}

#[cfg(test)]
mod tests {
    use super::*;

    const LANGUAGES: [Language; 2] = [Language::English, Language::Vietnamese];

    fn everything(language: Language, config: &Config) -> SystemDisplay {
        // What the tab shows is built from what this Mac says right now, so the check reads the real system: it
        // must come out whole whatever machine runs it.
        let cores = vec![0.5; sysinfo::cpu_ticks().expect("the kernel answers").len()];
        display(
            language,
            config,
            &cores,
            (1_200_000.0, 80_000.0),
            (&[1.0, 2.0], &[0.5]),
        )
    }

    #[test]
    fn the_feature_is_off_by_default_and_old_files_load() {
        let config: Config = serde_json::from_str("{}").unwrap();
        assert_eq!(config, Config::default());
        assert!(!config.enabled);
        assert!(config.processor && config.memory && config.network && config.details);
        let partial: Config =
            serde_json::from_str(r#"{ "enabled": true, "future_key": 1 }"#).unwrap();
        assert!(partial.enabled && partial.processor && partial.seconds == 1);
    }

    #[test]
    fn an_interval_that_is_not_offered_is_read_as_the_default() {
        for seconds in SECONDS {
            let config = Config {
                seconds,
                ..Config::default()
            };
            assert_eq!(config.sanitized().seconds, seconds);
        }
        for seconds in [0, 3, 4, 60, u32::MAX] {
            let config = Config {
                seconds,
                ..Config::default()
            };
            assert_eq!(config.sanitized().seconds, 1, "{seconds}");
        }
    }

    #[test]
    fn the_real_readings_make_a_whole_picture_in_both_languages() {
        for language in LANGUAGES {
            let shown = everything(language, &Config::default());
            assert!(shown.note.is_none());
            let cpu = shown.cpu.as_ref().expect("the processor");
            assert_eq!(cpu.value, "50%");
            assert!(cpu.slow < cpu.cores.len());
            let memory = shown.memory.as_ref().expect("the memory");
            assert!(memory.used.ends_with("GB") && memory.of.starts_with("/ "));
            assert!(memory.shares.iter().sum::<f64>() <= 1.0 + 1e-9);
            let network = shown.network.as_ref().expect("the network");
            assert_eq!(
                (network.down.as_str(), network.up.as_str()),
                ("1.2 MB/s", "80 KB/s")
            );
            assert_eq!(network.down_history.len(), 2);
            let titles: Vec<&str> = shown.stats.iter().map(|s| s.title.as_str()).collect();
            assert!(titles.contains(&language.system_thermal()), "{titles:?}");
            assert!(titles.contains(&language.system_disk()), "{titles:?}");
            for stat in &shown.stats {
                assert!(!stat.title.is_empty() && !stat.value.is_empty());
                assert!(
                    stat.fraction.is_none_or(|f| (0.0..=1.0).contains(&f)),
                    "{}",
                    stat.title
                );
            }
        }
    }

    #[test]
    fn a_card_that_is_switched_off_is_left_out() {
        let config = Config {
            processor: false,
            network: false,
            ..Config::default()
        };
        let shown = everything(Language::English, &config);
        assert!(shown.cpu.is_none() && shown.network.is_none());
        assert!(shown.memory.is_some() && !shown.stats.is_empty());
        let config = Config {
            memory: false,
            details: false,
            ..Config::default()
        };
        let shown = everything(Language::English, &config);
        assert!(shown.memory.is_none() && shown.stats.is_empty());
        assert!(shown.cpu.is_some() && shown.network.is_some());
    }

    #[test]
    fn with_every_card_off_there_is_nothing_to_show() {
        let none = Config {
            enabled: true,
            processor: false,
            memory: false,
            network: false,
            details: false,
            ..Config::default()
        };
        assert!(!none.shows_anything());
        assert!(Config::default().shows_anything());
    }

    #[test]
    fn the_two_notes_say_different_things_in_both_languages() {
        for language in LANGUAGES {
            let (off, empty) = (note(language, false), note(language, true));
            for text in [&off.title, &off.hint, &empty.title, &empty.hint] {
                assert!(!text.is_empty());
            }
            assert_ne!(off.title, empty.title);
            assert_ne!(off.hint, empty.hint);
        }
        assert_ne!(
            note(Language::English, false).title,
            note(Language::Vietnamese, false).title
        );
    }

    #[test]
    fn the_first_reading_after_the_tab_opens_has_no_differences_to_show() {
        let shown = display(
            Language::English,
            &Config::default(),
            &[0.0, 0.0],
            (0.0, 0.0),
            (&[], &[]),
        );
        assert_eq!(shown.cpu.unwrap().value, "0%");
        let network = shown.network.unwrap();
        assert_eq!(
            (network.down.as_str(), network.up.as_str()),
            ("0 B/s", "0 B/s")
        );
        assert!(network.down_history.is_empty());
    }
}
