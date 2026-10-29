//! The arithmetic of the System tab: loads from counters, rates from byte counts, and how a figure is said. Plain
//! data in and out, so that tests can drive it with pretend readings.

use macos_sys::{
    sensors::Fan,
    sysinfo::{Interface, Memory},
};
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

/// The figures of the menu bar, each with its label when it is chosen: `CPU 12%  Mem 61%  Net 1.2 MB/s`. The load
/// and the memory are fractions, the network is bytes a second down and up together.
pub fn bar_text(
    labels: [&str; 3],
    load: Option<f64>,
    memory: Option<f64>,
    network: Option<f64>,
) -> String {
    [
        (labels[0], load.map(percent_text)),
        (labels[1], memory.map(percent_text)),
        (labels[2], network.map(rate_text)),
    ]
    .into_iter()
    .filter_map(|(label, value)| Some(format!("{label} {}", value?)))
    .collect::<Vec<_>>()
    .join("  ")
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

    #[test]
    fn the_menu_bar_says_only_the_figures_chosen_each_with_its_label() {
        let labels = ["CPU", "RAM", "Mạng"];
        assert_eq!(
            bar_text(labels, Some(0.123), Some(0.61), Some(1_234_000.0)),
            "CPU 12%  RAM 61%  Mạng 1.2 MB/s"
        );
        assert_eq!(bar_text(labels, None, Some(0.5), None), "RAM 50%");
        assert_eq!(
            bar_text(labels, Some(1.7), None, Some(-3.0)),
            "CPU 100%  Mạng 0 B/s"
        );
        assert_eq!(bar_text(labels, None, None, None), "");
    }
}
