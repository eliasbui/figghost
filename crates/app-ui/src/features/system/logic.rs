//! The arithmetic of the System tab: loads from counters, rates from byte counts, and how a figure is said. Plain
//! data in and out, so that tests can drive it with pretend readings.

use crate::features::islands::logic::human_size;
use macos_sys::{
    sensors::Fan,
    sysinfo::{Interface, Memory},
    system_bar::Cell,
};
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;

/// How busy one core was between two readings of its ticks (user, system, idle, nice), 0 to 1. The counters are 32
/// bits wide and wrap, so the difference is taken as a wrapping one.
fn load(before: [u32; 4], after: [u32; 4]) -> f64 {
    let step = |i: usize| u64::from(after[i].wrapping_sub(before[i]));
    let busy = step(0) + step(1) + step(3);
    let total = busy + step(2);
    if total == 0 {
        0.0
    } else {
        busy as f64 / total as f64
    }
}

/// The load of every core between two readings. A reading with a different number of cores (a machine that has just
/// woken, say) says nothing, so the answer is empty.
pub fn core_loads(before: &[[u32; 4]], after: &[[u32; 4]]) -> Vec<f64> {
    if before.len() != after.len() {
        return Vec::new();
    }
    before
        .iter()
        .zip(after)
        .map(|(b, a)| load(*b, *a))
        .collect()
}

pub fn overall(loads: &[f64]) -> f64 {
    if loads.is_empty() {
        0.0
    } else {
        loads.iter().sum::<f64>() / loads.len() as f64
    }
}

/// The fastest a Mac's network can plausibly move, in bytes a second (a ten gigabit line is 1.25 GB/s). A counter that
/// jumped by more than that in the time between two readings was reset, as when an interface goes down and up, and is
/// not traffic.
// shortcut: a counter reset from near the top of its range looks like a wrap and is still counted; tell them apart with
// the interface's own 64 bit counters if that is ever seen.
const MOST_PLAUSIBLE: f64 = 2.0e9;

/// Bytes received and sent per second, from two readings of the interfaces taken `seconds` apart. Only an interface
/// that is in both counts, and each one's counter is differenced on its own, since each wraps on its own.
pub fn rates(before: &[Interface], after: &[Interface], seconds: f64) -> (f64, f64) {
    if seconds <= 0.0 {
        return (0.0, 0.0);
    }
    let step = |now: u32, then: u32| {
        let bytes = f64::from(now.wrapping_sub(then));
        if bytes / seconds > MOST_PLAUSIBLE {
            0.0
        } else {
            bytes
        }
    };
    let (mut down, mut up) = (0.0, 0.0);
    for now in after {
        if let Some(then) = before.iter().find(|i| i.name == now.name) {
            down += step(now.received, then.received);
            up += step(now.sent, then.sent);
        }
    }
    (down / seconds, up / seconds)
}

/// The last readings of something, oldest first, up to a capacity.
#[derive(Debug, Default)]
pub struct History {
    values: VecDeque<f64>,
}

impl History {
    pub fn push(&mut self, value: f64, capacity: usize) {
        self.values.push_back(value);
        while self.values.len() > capacity {
            self.values.pop_front();
        }
    }

    pub fn clear(&mut self) {
        self.values.clear();
    }

    pub fn values(&self) -> Vec<f64> {
        self.values.iter().copied().collect()
    }
}

/// The readings held to 0 to 1 against the largest of them, or against `floor` when they are all small, so that a
/// quiet line stays low instead of filling the picture.
pub fn scaled(values: &[f64], floor: f64) -> Vec<f64> {
    let top = values.iter().copied().fold(floor, f64::max);
    values.iter().map(|v| (v / top).clamp(0.0, 1.0)).collect()
}

/// A rate as a person reads it: `0 B/s`, `84 KB/s`, `1.2 MB/s`, `48 MB/s`.
pub fn rate_text(bytes_per_second: f64) -> String {
    let value = bytes_per_second.max(0.0);
    if value < 1000.0 {
        return format!("{:.0} B/s", value);
    }
    let (unit, scale) = if value < 1e6 {
        ("KB/s", 1e3)
    } else if value < 1e9 {
        ("MB/s", 1e6)
    } else {
        ("GB/s", 1e9)
    };
    let n = value / scale;
    // Three figures are enough: 250 MB/s, 84 KB/s, 48.2 MB/s, 1.2 MB/s. Kilobytes are whole from ten up, since a
    // tenth of a kilobyte means nothing.
    if n >= 100.0 || (n >= 10.0 && unit == "KB/s") {
        format!("{n:.0} {unit}")
    } else {
        format!("{n:.1} {unit}")
    }
}

/// An amount of memory as Activity Monitor says it: binary units, labelled GB and MB.
pub fn memory_text(bytes: u64) -> String {
    let gib = bytes as f64 / (1u64 << 30) as f64;
    if gib >= 1.0 {
        format!("{gib:.1} GB")
    } else {
        format!("{:.0} MB", bytes as f64 / (1u64 << 20) as f64)
    }
}

pub fn percent_text(fraction: f64) -> String {
    format!("{:.0}%", (fraction * 100.0).clamp(0.0, 100.0))
}

/// A line of the network never rises above this much until traffic does, so that a quiet network stays low.
pub const NETWORK_FLOOR: f64 = 100_000.0;
/// What a cell says when this Mac does not tell the figure.
const MISSING: &str = "–";
/// The widest value of each figure, which sets the width of its cell.
const WIDEST_PERCENT: &str = "100%";
const WIDEST_RATE: &str = "88.8 MB/s";
const WIDEST_HEAT: &str = "100°";
const WIDEST_SIZE: &str = "88.8 GB";

/// A figure that can stay in the menu bar, in the order the page lists them and `system_bar_labels` names them.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Figure {
    Processor,
    Memory,
    Network,
    Gpu,
    Heat,
    Disk,
}

/// How a figure is drawn in the menu bar. For the network, `Stacked` is what is sent over what is received. A kind
/// this version does not know (from a later one) is read as `Stacked`, so that it cannot make the whole file unreadable.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Kind {
    Text,
    Graph,
    Cores,
    // Last, as serde requires of the variant that takes what is not known.
    #[default]
    #[serde(other)]
    Stacked,
}

impl Kind {
    /// Only the processor has cores to draw.
    pub fn fits(self, figure: Figure) -> bool {
        self != Kind::Cores || figure == Figure::Processor
    }
}

/// What was read for the menu bar. A figure that is not chosen, or that this Mac does not tell, is empty.
#[derive(Debug, Clone, Default)]
pub struct BarReading {
    pub loads: Vec<f64>,
    /// How many of `loads` are frugal cores, first.
    pub slow: usize,
    /// The share of the memory in use, 0 to 1.
    pub memory: Option<f64>,
    /// Bytes a second received and sent.
    pub rates: (f64, f64),
    /// How busy the graphics processor is, 0 to 1.
    pub gpu: Option<f64>,
    pub celsius: Option<f64>,
    /// Bytes free and in all on the startup disk.
    pub disk: Option<(u64, u64)>,
}

impl BarReading {
    /// The figure as a point of its graph: 0 to 1, except the network in bytes a second, which is scaled against
    /// its own recent readings when drawn.
    pub fn level(&self, figure: Figure) -> f64 {
        match figure {
            Figure::Processor => overall(&self.loads),
            Figure::Memory => self.memory.unwrap_or(0.0),
            Figure::Network => self.rates.0 + self.rates.1,
            Figure::Gpu => self.gpu.unwrap_or(0.0),
            Figure::Heat => self.celsius.map_or(0.0, |c| c / 100.0),
            Figure::Disk => self
                .disk
                .filter(|(_, total)| *total > 0)
                .map_or(0.0, |(free, total)| 1.0 - free as f64 / total as f64),
        }
    }

    /// The figure as words, with the widest it can say.
    fn value(&self, figure: Figure) -> (String, &'static str) {
        let or_missing = |value: Option<String>| value.unwrap_or_else(|| MISSING.to_owned());
        match figure {
            Figure::Processor => (percent_text(overall(&self.loads)), WIDEST_PERCENT),
            Figure::Memory => (or_missing(self.memory.map(percent_text)), WIDEST_PERCENT),
            Figure::Network => (rate_text(self.rates.0 + self.rates.1), WIDEST_RATE),
            Figure::Gpu => (or_missing(self.gpu.map(percent_text)), WIDEST_PERCENT),
            Figure::Heat => (
                or_missing(self.celsius.map(|c| format!("{:.0}°", c.max(0.0)))),
                WIDEST_HEAT,
            ),
            Figure::Disk => (
                or_missing(self.disk.map(|(free, _)| human_size(free))),
                WIDEST_SIZE,
            ),
        }
    }
}

/// The cell of one figure in the menu bar. `history` is the figure's last levels, oldest first.
pub fn bar_cell(
    labels: [&str; 6],
    figure: Figure,
    kind: Kind,
    reading: &BarReading,
    history: &[f64],
) -> Cell {
    let label = labels[figure as usize];
    let (value, widest) = reading.value(figure);
    match kind {
        Kind::Text => Cell::Text {
            label: label.to_owned(),
            value,
            widest: widest.to_owned(),
        },
        Kind::Stacked if figure == Figure::Network => Cell::Rates {
            up: rate_text(reading.rates.1),
            down: rate_text(reading.rates.0),
            widest: widest.to_owned(),
        },
        Kind::Graph if figure == Figure::Network => Cell::Graph {
            history: scaled(history, NETWORK_FLOOR),
        },
        Kind::Graph => Cell::Graph {
            history: history.iter().map(|v| v.clamp(0.0, 1.0)).collect(),
        },
        Kind::Cores if figure == Figure::Processor => Cell::Cores {
            loads: reading.loads.clone(),
            slow: reading.slow,
        },
        Kind::Stacked | Kind::Cores => Cell::Stacked {
            label: label.to_uppercase(),
            value,
            widest: widest.to_owned(),
        },
    }
}

/// The figures as words, for assistive technology, which cannot read the picture: `CPU 12%, Mem 61%`.
pub fn describe(labels: [&str; 6], figures: &[(Figure, Kind)], reading: &BarReading) -> String {
    figures
        .iter()
        .map(|&(figure, _)| format!("{} {}", labels[figure as usize], reading.value(figure).0))
        .collect::<Vec<_>>()
        .join(", ")
}

/// The fan card: the fastest fan, as a share of the fastest it can turn. `None` for a Mac without fans.
pub fn fastest(fans: &[Fan]) -> Option<(Fan, f64)> {
    let fan = *fans.iter().max_by(|a, b| a.rpm.total_cmp(&b.rpm))?;
    Some((fan, (fan.rpm / fan.max).clamp(0.0, 1.0)))
}

pub fn rpm_text(rpm: f64) -> String {
    format!("{:.0} rpm", rpm.max(0.0))
}

pub fn celsius_text(celsius: f64) -> String {
    format!("{celsius:.0} °C")
}

/// What apps hold, what is wired and what is compressed, each as a share of all the memory.
pub fn shares(memory: &Memory) -> [f64; 3] {
    if memory.total == 0 {
        return [0.0; 3];
    }
    let share = |bytes: u64| bytes as f64 / memory.total as f64;
    [
        share(memory.app),
        share(memory.wired),
        share(memory.compressed),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    fn interface(name: &str, received: u32, sent: u32) -> Interface {
        Interface {
            name: name.into(),
            received,
            sent,
        }
    }

    #[test]
    fn a_core_that_spent_the_time_busy_is_full_and_one_that_idled_is_empty() {
        let before = [[100, 50, 1000, 0], [100, 50, 1000, 0]];
        let after = [[200, 150, 1000, 0], [100, 50, 1200, 0]];
        let loads = core_loads(&before, &after);
        assert!((loads[0] - 1.0).abs() < 1e-9);
        assert!(loads[1].abs() < 1e-9);
        assert!((overall(&loads) - 0.5).abs() < 1e-9);
    }

    #[test]
    fn nice_time_counts_as_busy_and_idle_time_does_not() {
        let loads = core_loads(&[[0, 0, 0, 0]], &[[10, 10, 60, 20]]);
        assert!((loads[0] - 0.4).abs() < 1e-9, "{}", loads[0]);
    }

    #[test]
    fn counters_that_wrap_around_still_give_the_difference() {
        let before = [[u32::MAX - 5, 0, u32::MAX - 5, 0]];
        let after = [[14, 0, 14, 0]];
        let loads = core_loads(&before, &after);
        assert!((loads[0] - 0.5).abs() < 1e-9, "{}", loads[0]);
    }

    #[test]
    fn nothing_happening_is_no_load_and_not_a_division_by_zero() {
        assert_eq!(core_loads(&[[1, 2, 3, 4]], &[[1, 2, 3, 4]]), vec![0.0]);
        assert_eq!(overall(&[]), 0.0);
    }

    #[test]
    fn a_different_number_of_cores_says_nothing() {
        assert!(core_loads(&[[0; 4]], &[[0; 4], [0; 4]]).is_empty());
    }

    #[test]
    fn rates_are_the_difference_of_each_interface_over_the_time_between() {
        let before = [interface("en0", 1000, 500), interface("en1", 0, 0)];
        let after = [interface("en0", 3000, 1500), interface("en1", 0, 0)];
        assert_eq!(rates(&before, &after, 2.0), (1000.0, 500.0));
    }

    #[test]
    fn a_counter_that_wraps_is_counted_from_where_it_wrapped() {
        let before = [interface("en0", u32::MAX - 99, 0)];
        let after = [interface("en0", 100, 0)];
        assert_eq!(rates(&before, &after, 1.0).0, 200.0);
    }

    #[test]
    fn a_counter_that_was_reset_is_not_taken_for_traffic() {
        // The interface went down and up: the counter fell from 5,000 to 10, which as a wrap would be 4 GB.
        let before = [interface("en0", 5_000, 5_000), interface("en1", 100, 100)];
        let after = [interface("en0", 10, 10), interface("en1", 600, 700)];
        assert_eq!(
            rates(&before, &after, 1.0),
            (500.0, 600.0),
            "only the interface that kept counting counts"
        );
        // A real transfer, even a fast one, is traffic.
        let fast = rates(
            &[interface("en0", 0, 0)],
            &[interface("en0", 1_500_000_000, 0)],
            1.0,
        );
        assert_eq!(fast.0, 1.5e9);
    }

    #[test]
    fn an_interface_that_came_or_went_between_the_readings_is_left_out() {
        let before = [interface("en0", 0, 0)];
        let after = [interface("en0", 10, 0), interface("en5", 999_999, 999_999)];
        assert_eq!(rates(&before, &after, 1.0), (10.0, 0.0));
        assert_eq!(rates(&before, &after, 0.0), (0.0, 0.0));
    }

    #[test]
    fn the_history_keeps_the_newest_readings() {
        let mut history = History::default();
        for i in 0..10 {
            history.push(f64::from(i), 4);
        }
        assert_eq!(history.values(), vec![6.0, 7.0, 8.0, 9.0]);
        history.clear();
        assert!(history.values().is_empty());
    }

    #[test]
    fn a_quiet_line_stays_low_and_a_busy_one_fills_the_picture() {
        assert_eq!(scaled(&[0.0, 50.0, 100.0], 1000.0), vec![0.0, 0.05, 0.1]);
        assert_eq!(scaled(&[0.0, 50.0, 4000.0], 1000.0), vec![0.0, 0.0125, 1.0]);
    }

    #[test]
    fn a_rate_is_said_in_the_unit_that_keeps_it_short() {
        assert_eq!(rate_text(0.0), "0 B/s");
        assert_eq!(rate_text(840.0), "840 B/s");
        assert_eq!(rate_text(1_200.0), "1.2 KB/s");
        assert_eq!(rate_text(84_000.0), "84 KB/s");
        assert_eq!(rate_text(1_200_000.0), "1.2 MB/s");
        assert_eq!(rate_text(48_200_000.0), "48.2 MB/s");
        assert_eq!(rate_text(250_000_000.0), "250 MB/s");
        assert_eq!(rate_text(1_100_000_000.0), "1.1 GB/s");
        assert_eq!(rate_text(-5.0), "0 B/s");
    }

    #[test]
    fn memory_is_said_in_binary_units_as_activity_monitor_says_it() {
        assert_eq!(memory_text(16 << 30), "16.0 GB");
        assert_eq!(memory_text((12 << 30) + (700 << 20)), "12.7 GB");
        assert_eq!(memory_text(850 << 20), "850 MB");
    }

    #[test]
    fn percentages_are_whole_and_held_to_a_hundred() {
        assert_eq!(percent_text(0.374), "37%");
        assert_eq!(percent_text(1.7), "100%");
        assert_eq!(percent_text(-1.0), "0%");
    }

    #[test]
    fn the_fan_card_follows_the_fastest_fan() {
        let fan = |rpm| Fan {
            rpm,
            min: 1000.0,
            max: 5000.0,
        };
        assert!(fastest(&[]).is_none());
        let (top, share) = fastest(&[fan(1200.0), fan(2500.0)]).unwrap();
        assert_eq!((top.rpm, share), (2500.0, 0.5));
        assert_eq!(fastest(&[fan(9000.0)]).unwrap().1, 1.0);
        assert_eq!(rpm_text(1501.6), "1502 rpm");
        assert_eq!(rpm_text(-3.0), "0 rpm");
        assert_eq!(celsius_text(82.94), "83 °C");
    }

    #[test]
    fn the_shares_of_the_memory_add_up_to_what_is_used() {
        let memory = Memory {
            total: 16 << 30,
            app: 4 << 30,
            wired: 2 << 30,
            compressed: 6 << 30,
        };
        let shares = shares(&memory);
        assert!((shares.iter().sum::<f64>() - 0.75).abs() < 1e-9);
        assert_eq!(super::shares(&Memory { total: 0, ..memory }), [0.0; 3]);
    }

    const LABELS: [&str; 6] = ["CPU", "RAM", "Mạng", "GPU", "Nhiệt", "Ổ đĩa"];

    fn busy() -> BarReading {
        BarReading {
            loads: vec![0.2, 0.4, 0.9, 0.5],
            slow: 2,
            memory: Some(0.61),
            rates: (1_234_000.0, 80_000.0),
            gpu: Some(0.07),
            celsius: Some(62.4),
            disk: Some((227_000_000_000, 460_000_000_000)),
        }
    }

    #[test]
    fn each_kind_draws_its_own_cell() {
        let r = busy();
        assert_eq!(
            bar_cell(LABELS, Figure::Processor, Kind::Text, &r, &[]),
            Cell::Text {
                label: "CPU".into(),
                value: "50%".into(),
                widest: "100%".into()
            }
        );
        assert_eq!(
            bar_cell(LABELS, Figure::Heat, Kind::Stacked, &r, &[]),
            Cell::Stacked {
                label: "NHIỆT".into(),
                value: "62°".into(),
                widest: "100°".into()
            }
        );
        assert_eq!(
            bar_cell(LABELS, Figure::Network, Kind::Stacked, &r, &[]),
            Cell::Rates {
                up: "80 KB/s".into(),
                down: "1.2 MB/s".into(),
                widest: "88.8 MB/s".into()
            }
        );
        assert_eq!(
            bar_cell(LABELS, Figure::Processor, Kind::Cores, &r, &[]),
            Cell::Cores {
                loads: vec![0.2, 0.4, 0.9, 0.5],
                slow: 2
            }
        );
        assert_eq!(
            bar_cell(LABELS, Figure::Disk, Kind::Text, &r, &[]),
            Cell::Text {
                label: "Ổ đĩa".into(),
                value: "227 GB".into(),
                widest: "88.8 GB".into()
            }
        );
        // A file that asks for the cores of something else still gets a figure.
        assert!(matches!(
            bar_cell(LABELS, Figure::Memory, Kind::Cores, &r, &[]),
            Cell::Stacked { .. }
        ));
        assert!(Kind::Cores.fits(Figure::Processor) && !Kind::Cores.fits(Figure::Disk));
        assert!(Kind::Graph.fits(Figure::Network));
    }

    #[test]
    fn a_graph_holds_its_readings_to_the_picture() {
        let r = busy();
        assert_eq!(
            bar_cell(LABELS, Figure::Memory, Kind::Graph, &r, &[0.5, 1.4, -1.0]),
            Cell::Graph {
                history: vec![0.5, 1.0, 0.0]
            }
        );
        // The network against its busiest reading, or against the floor while it is quiet.
        assert_eq!(
            bar_cell(
                LABELS,
                Figure::Network,
                Kind::Graph,
                &r,
                &[1_000.0, 400_000.0, 200_000.0]
            ),
            Cell::Graph {
                history: vec![0.0025, 1.0, 0.5]
            }
        );
        assert_eq!(r.level(Figure::Network), 1_314_000.0);
        assert_eq!(r.level(Figure::Processor), 0.5);
        assert!((r.level(Figure::Disk) - 0.5065).abs() < 1e-3);
        assert!((r.level(Figure::Heat) - 0.624).abs() < 1e-9);
    }

    #[test]
    fn what_this_mac_does_not_tell_is_a_dash_and_no_point() {
        let r = BarReading::default();
        for figure in [Figure::Memory, Figure::Gpu, Figure::Heat, Figure::Disk] {
            assert_eq!(r.value(figure).0, "–", "{figure:?}");
            assert_eq!(r.level(figure), 0.0, "{figure:?}");
        }
        assert_eq!(r.value(Figure::Processor).0, "0%");
        assert_eq!(r.value(Figure::Network).0, "0 B/s");
    }

    #[test]
    fn no_value_is_wider_than_the_widest_its_cell_is_made_for() {
        // Digits keep one width, so counting characters is enough; the units are among the widest letters.
        let mut rate = 1.0;
        while rate < MOST_PLAUSIBLE {
            assert!(
                rate_text(rate).chars().count() <= WIDEST_RATE.chars().count(),
                "{rate}"
            );
            rate *= 1.37;
        }
        for share in [0.0, 0.5, 1.0, 7.0] {
            assert!(percent_text(share).len() <= WIDEST_PERCENT.len());
        }
        let mut bytes = 1u64;
        while bytes < 100_000_000_000_000 {
            assert!(
                human_size(bytes).chars().count() <= WIDEST_SIZE.chars().count(),
                "{bytes}"
            );
            bytes = bytes * 3 + 7;
        }
    }

    #[test]
    fn the_description_names_each_figure_chosen() {
        let figures = [
            (Figure::Processor, Kind::Cores),
            (Figure::Network, Kind::Stacked),
            (Figure::Heat, Kind::Graph),
        ];
        assert_eq!(
            describe(LABELS, &figures, &busy()),
            "CPU 50%, Mạng 1.3 MB/s, Nhiệt 62°"
        );
        assert_eq!(describe(LABELS, &[], &busy()), "");
    }
}
