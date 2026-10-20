//! The day as a line: where the morning and bedtime are, where now is, how the events of today lie along it, what is on
//! and what comes next. Nothing here reads a calendar or draws; times are seconds since 1970 and the local midnight comes
//! in from outside, so the rules are tested with days and clocks of their own.

use core_calendar::model::{Event, Reminder};
use macos_sys::dayprogress_ui::{AGENDA, LANES};

/// The hours the line may begin at, and the hours it may end at (24 is midnight).
pub const WAKE: [u32; 6] = [5, 6, 7, 8, 9, 10];
pub const BED: [u32; 4] = [21, 22, 23, 24];

/// The line of the day: from the hour of waking to the hour of going to bed, in seconds since 1970.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Window {
    pub from: i64,
    pub to: i64,
}

impl Window {
    /// The line of the day that began at `midnight`. A pair of hours that is not offered, or not in order, is the usual
    /// day from 7 to 23.
    // shortcut: hours are counted from midnight, so on the two days a year the clocks change the line is an hour out
    // after the change; use wall-clock hours if that is ever noticed.
    pub fn new(midnight: i64, wake: u32, bed: u32) -> Self {
        let (wake, bed) = if WAKE.contains(&wake) && BED.contains(&bed) {
            (wake, bed)
        } else {
            (7, 23)
        };
        Self {
            from: midnight + i64::from(wake) * 3600,
            to: midnight + i64::from(bed) * 3600,
        }
    }

    pub fn length(&self) -> i64 {
        self.to - self.from
    }

    /// Where a time lies on the line, 0 at the morning and 1 at bedtime, held to that range.
    pub fn share(&self, t: i64) -> f64 {
        ((t - self.from) as f64 / self.length() as f64).clamp(0.0, 1.0)
    }
}

/// An event laid along the line: its share of it, the row it is in, and whether it is on now.
#[derive(Debug, Clone, PartialEq)]
pub struct Placed {
    pub start: f64,
    pub end: f64,
    pub lane: usize,
    pub color: u32,
    pub running: bool,
}

/// The events that are on the line, each in the first row that is free where it begins; when every row is taken the last
/// one is shared. An event that lasts all day, or lies wholly outside the line, is not on it.
pub fn place(events: &[Event], window: Window, now: i64) -> Vec<Placed> {
    let mut on: Vec<&Event> = events
        .iter()
        .filter(|e| !e.all_day && e.end > window.from && e.start < window.to && e.end > e.start)
        .collect();
    on.sort_by_key(|e| (e.start, e.end));
    let mut row_ends: Vec<i64> = Vec::new();
    on.into_iter()
        .map(|e| {
            let lane = match row_ends.iter().position(|end| *end <= e.start) {
                Some(free) => free,
                None if row_ends.len() < LANES => {
                    row_ends.push(i64::MIN);
                    row_ends.len() - 1
                }
                // All the rows are taken: share the one that frees first.
                None => row_ends
                    .iter()
                    .enumerate()
                    .min_by_key(|(_, end)| **end)
                    .map_or(0, |(i, _)| i),
            };
            row_ends[lane] = e.end;
            Placed {
                start: window.share(e.start),
                end: window.share(e.end),
                lane,
                color: e.color,
                running: e.start <= now && now < e.end,
            }
        })
        .collect()
}

/// Where each reminder that has a time of day today lies on the line, as (share, colour); those with only a date, and
/// those that are overdue from before today, have no place on a line and are counted instead.
pub fn marks(
    reminders: &[Reminder],
    window: Window,
    midnight: i64,
    next_midnight: i64,
) -> Vec<(f64, u32)> {
    reminders
        .iter()
        .filter_map(|r| {
            let due = r
                .due
                .filter(|due| !r.date_only && *due >= midnight && *due < next_midnight)?;
            Some((window.share(due), r.color))
        })
        .collect()
}

/// What is on now and what comes next, no more than `AGENDA` of them: the events that have not ended, in the order they begin.
pub fn agenda(events: &[Event], now: i64) -> Vec<&Event> {
    let mut left: Vec<&Event> = events
        .iter()
        .filter(|e| !e.all_day && e.end > now && e.end > e.start)
        .collect();
    left.sort_by_key(|e| (e.start, e.end));
    left.truncate(AGENDA);
    left
}

/// How many events have not ended yet.
pub fn events_left(events: &[Event], now: i64) -> usize {
    events
        .iter()
        .filter(|e| !e.all_day && e.end > now && e.end > e.start)
        .count()
}

/// The hours that are labelled under the line: every third one from the morning, with their share of it.
pub fn tick_hours(window: Window, wake: u32, bed: u32) -> Vec<(u32, f64)> {
    let (wake, bed) = if WAKE.contains(&wake) && BED.contains(&bed) {
        (wake, bed)
    } else {
        (7, 23)
    };
    (wake..=bed)
        .step_by(3)
        .map(|hour| {
            (
                hour % 24,
                window.share(window.from + i64::from(hour - wake) * 3600),
            )
        })
        .collect()
}

/// A length of time as whole hours and minutes: (hours, minutes), the minutes rounded up so that 59 seconds is a minute.
pub fn hours_minutes(seconds: i64) -> (i64, i64) {
    let minutes = (seconds.max(0) + 59) / 60;
    (minutes / 60, minutes % 60)
}

#[cfg(test)]
mod tests {
    use super::*;

    const MIDNIGHT: i64 = 1_000_000 * 86_400;
    const HOUR: i64 = 3600;

    fn at(hour: f64) -> i64 {
        MIDNIGHT + (hour * 3600.0) as i64
    }

    fn event(from: f64, to: f64, color: u32) -> Event {
        Event {
            id: format!("{from}-{to}"),
            title: format!("{from}-{to}"),
            start: at(from),
            end: at(to),
            all_day: false,
            calendar_id: "c".into(),
            calendar: "Work".into(),
            color,
            location: String::new(),
            notes: String::new(),
            url: String::new(),
        }
    }

    fn reminder(due: Option<i64>, date_only: bool) -> Reminder {
        Reminder {
            id: "r".into(),
            title: "t".into(),
            due,
            date_only,
            calendar: "Reminders".into(),
            color: 0xffd60a,
        }
    }

    #[test]
    fn the_line_runs_from_the_chosen_morning_to_the_chosen_bedtime() {
        let day = Window::new(MIDNIGHT, 7, 23);
        assert_eq!(
            (day.from, day.to, day.length()),
            (at(7.0), at(23.0), 16 * HOUR)
        );
        assert_eq!(Window::new(MIDNIGHT, 5, 24).length(), 19 * HOUR);
        let usual = Window::new(MIDNIGHT, 7, 23);
        for (wake, bed) in [
            (0, 23),
            (7, 0),
            (11, 23),
            (7, 20),
            (23, 7),
            (24, 24),
            (u32::MAX, 1),
        ] {
            assert_eq!(Window::new(MIDNIGHT, wake, bed), usual, "{wake} {bed}");
        }
    }

    #[test]
    fn where_a_time_lies_on_the_line_is_held_to_the_line() {
        let day = Window::new(MIDNIGHT, 7, 23);
        assert_eq!(day.share(at(7.0)), 0.0);
        assert_eq!(day.share(at(23.0)), 1.0);
        assert!((day.share(at(15.0)) - 0.5).abs() < 1e-9);
        assert_eq!(day.share(at(3.0)), 0.0, "before the morning");
        assert_eq!(day.share(at(23.9)), 1.0, "after bedtime");
    }

    #[test]
    fn events_are_put_in_the_first_row_that_is_free_and_clipped_to_the_line() {
        let day = Window::new(MIDNIGHT, 7, 23);
        let events = [
            event(9.0, 10.0, 1),
            event(9.5, 10.5, 2),
            event(9.75, 11.0, 3),
            event(10.25, 12.0, 4),
            event(6.0, 8.0, 5),
            event(22.0, 26.0, 6),
            event(1.0, 3.0, 7),
        ];
        let placed = place(&events, day, at(9.8));
        let by_color = |color: u32| placed.iter().find(|p| p.color == color).unwrap();
        assert_eq!(
            by_color(5).start,
            0.0,
            "an event that began before the morning begins at the line's start"
        );
        assert_eq!(
            by_color(6).end,
            1.0,
            "one that goes past bedtime ends at the line's end"
        );
        assert!(
            placed.iter().all(|p| p.color != 7),
            "one wholly before the line is not on it"
        );
        assert_eq!(
            (by_color(5).lane, by_color(1).lane),
            (0, 0),
            "the early one is over before the first begins"
        );
        assert_eq!((by_color(2).lane, by_color(3).lane), (1, 2));
        assert_eq!(
            by_color(4).lane,
            0,
            "the first row is free again after 10:00"
        );
        assert!(
            by_color(1).running
                && by_color(2).running
                && by_color(3).running
                && !by_color(4).running
        );
    }

    #[test]
    fn when_every_row_is_taken_the_one_that_frees_first_is_shared() {
        let day = Window::new(MIDNIGHT, 7, 23);
        let events = [
            event(9.0, 10.0, 1),
            event(9.0, 11.0, 2),
            event(9.0, 12.0, 3),
            event(9.0, 13.0, 4),
        ];
        let placed = place(&events, day, at(8.0));
        assert_eq!(placed.len(), 4);
        assert!(placed.iter().all(|p| p.lane < LANES));
        assert_eq!(placed[3].lane, 0, "the first to end gives up its row");
    }

    #[test]
    fn an_all_day_event_and_one_that_lasts_no_time_are_not_on_the_line() {
        let day = Window::new(MIDNIGHT, 7, 23);
        let mut all_day = event(0.0, 24.0, 1);
        all_day.all_day = true;
        let none = event(10.0, 10.0, 2);
        assert!(place(&[all_day.clone(), none.clone()], day, at(12.0)).is_empty());
        assert!(agenda(&[all_day, none], at(8.0)).is_empty());
    }

    #[test]
    fn what_is_on_and_what_comes_next_are_the_events_that_have_not_ended_in_the_order_they_begin() {
        let events = [
            event(8.0, 9.0, 1),
            event(13.0, 14.0, 2),
            event(11.0, 15.0, 3),
            event(16.0, 17.0, 4),
            event(18.0, 19.0, 5),
            event(20.0, 21.0, 6),
        ];
        let list = agenda(&events, at(13.5));
        assert_eq!(
            list.iter().map(|e| e.color).collect::<Vec<_>>(),
            [3, 2, 4],
            "the long one that is on, then the one that is on, then what is next"
        );
        assert_eq!(events_left(&events, at(13.5)), 5);
        assert_eq!(events_left(&events, at(23.0)), 0);
        assert!(agenda(&events, at(23.0)).is_empty());
        assert_eq!(
            agenda(&events, at(7.0)).len(),
            AGENDA,
            "no more than the list has room for"
        );
    }

    #[test]
    fn a_reminder_with_a_time_today_is_marked_where_it_is_due_and_the_rest_are_not() {
        let day = Window::new(MIDNIGHT, 7, 23);
        let next = MIDNIGHT + 24 * HOUR;
        let list = [
            reminder(Some(at(15.0)), false),
            reminder(Some(at(1.0)), false),
            reminder(Some(at(23.9)), false),
            reminder(Some(at(12.0)), true),
            reminder(Some(MIDNIGHT - HOUR), false),
            reminder(Some(next + HOUR), false),
            reminder(None, false),
        ];
        let marked = marks(&list, day, MIDNIGHT, next);
        assert_eq!(marked.len(), 3);
        assert!((marked[0].0 - 0.5).abs() < 1e-9);
        assert_eq!(marked[1].0, 0.0, "before the morning sits at the start");
        assert_eq!(marked[2].0, 1.0, "after bedtime sits at the end");
    }

    #[test]
    fn every_third_hour_is_labelled_from_the_morning() {
        let day = Window::new(MIDNIGHT, 7, 23);
        let ticks = tick_hours(day, 7, 23);
        assert_eq!(
            ticks.iter().map(|t| t.0).collect::<Vec<_>>(),
            [7, 10, 13, 16, 19, 22]
        );
        assert_eq!(ticks[0].1, 0.0);
        assert!((ticks[2].1 - 0.375).abs() < 1e-9);
        let late = tick_hours(Window::new(MIDNIGHT, 6, 24), 6, 24);
        assert_eq!(
            late.iter().map(|t| t.0).collect::<Vec<_>>(),
            [6, 9, 12, 15, 18, 21, 0],
            "midnight is 0"
        );
        assert_eq!(late.last().unwrap().1, 1.0);
    }

    #[test]
    fn a_length_of_time_rounds_up_to_the_minute_that_has_begun() {
        assert_eq!(hours_minutes(0), (0, 0));
        assert_eq!(hours_minutes(1), (0, 1));
        assert_eq!(hours_minutes(60), (0, 1));
        assert_eq!(hours_minutes(61), (0, 2));
        assert_eq!(hours_minutes(8 * HOUR + 40 * 60), (8, 40));
        assert_eq!(hours_minutes(-500), (0, 0));
    }
}
