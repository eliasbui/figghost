//! Which meeting alerts fire and when. The scheduler holds no clock and no timer: the caller says what time it
//! is and arms one timer for `next_wake`, so the same code runs under test with a pretend clock.

use crate::model::Event;
use std::collections::HashSet;

/// An alert that was missed by more than this (the Mac was asleep, the app was off) is not shown late.
const GRACE: i64 = 60;
/// A fired alert stays pending, and keeps the tab marked, until this long after the event begins.
const LINGER: i64 = 120;

pub const MIN_LEAD: u32 = 1;
pub const MAX_LEAD: u32 = 60;

#[derive(Debug, Clone, Default)]
pub struct Scheduler {
    lead: i64,
    fired: HashSet<String>,
    dismissed: HashSet<String>,
}

impl Scheduler {
    pub fn new(lead_minutes: u32) -> Self {
        let mut this = Self::default();
        this.set_lead(lead_minutes);
        this
    }

    /// Changes how long before an event the alert comes. An alert that already fired does not fire again.
    pub fn set_lead(&mut self, minutes: u32) {
        self.lead = i64::from(minutes.clamp(MIN_LEAD, MAX_LEAD)) * 60;
    }

    fn alert_time(&self, event: &Event) -> i64 {
        event.start - self.lead
    }

    /// An all-day event has no moment to be reminded at.
    fn counts(event: &Event) -> bool {
        !event.all_day
    }

    /// Marks the alerts that are due now as fired and returns their positions in `events`, earliest first. Two
    /// events that overlap both fire. An event moved to another time is a new occurrence and fires again.
    pub fn tick(&mut self, events: &[Event], now: i64) -> Vec<usize> {
        let mut due: Vec<usize> = events
            .iter()
            .enumerate()
            .filter(|(_, e)| {
                Self::counts(e)
                    && self.alert_time(e) <= now
                    && now < e.start.max(self.alert_time(e)) + GRACE
                    && !self.fired.contains(&e.key())
            })
            .map(|(i, _)| i)
            .collect();
        due.sort_by_key(|&i| (events[i].start, events[i].key()));
        for &i in &due {
            self.fired.insert(events[i].key());
        }
        // Forget what can no longer matter, so the sets do not grow for as long as the app runs.
        let keys: HashSet<String> = events.iter().map(Event::key).collect();
        self.fired.retain(|k| keys.contains(k));
        self.dismissed.retain(|k| keys.contains(k));
        due
    }

    /// Events whose alert fired and has not been dismissed, while they are about to begin or just have.
    pub fn pending(&self, events: &[Event], now: i64) -> Vec<usize> {
        let mut list: Vec<usize> = events
            .iter()
            .enumerate()
            .filter(|(_, e)| {
                let key = e.key();
                self.fired.contains(&key)
                    && !self.dismissed.contains(&key)
                    && now < e.start + LINGER
            })
            .map(|(i, _)| i)
            .collect();
        list.sort_by_key(|&i| (events[i].start, events[i].key()));
        list
    }

    pub fn dismiss(&mut self, event: &Event) {
        self.dismissed.insert(event.key());
    }

    /// When the caller must look again: the next alert, or the moment a pending alert stops being pending.
    pub fn next_wake(&self, events: &[Event], now: i64) -> Option<i64> {
        events
            .iter()
            .filter(|e| Self::counts(e))
            .map(|e| {
                if self.fired.contains(&e.key()) {
                    e.start + LINGER
                } else {
                    self.alert_time(e)
                }
            })
            .filter(|&t| t > now)
            .min()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const T0: i64 = 1_792_000_000;

    fn event(id: &str, start: i64, minutes: i64) -> Event {
        Event {
            id: id.into(),
            title: id.into(),
            start,
            end: start + minutes * 60,
            all_day: false,
            calendar_id: "c".into(),
            calendar: "Work".into(),
            color: 0,
            location: String::new(),
            notes: String::new(),
            url: String::new(),
        }
    }

    #[test]
    fn an_alert_fires_once_at_the_lead_time() {
        let events = [event("standup", T0 + 3600, 15)];
        let mut s = Scheduler::new(5);
        assert_eq!(s.next_wake(&events, T0), Some(T0 + 3600 - 300));
        assert!(s.tick(&events, T0 + 3600 - 301).is_empty());
        assert_eq!(s.tick(&events, T0 + 3600 - 300), [0]);
        assert!(s.tick(&events, T0 + 3600 - 299).is_empty());
        assert_eq!(s.pending(&events, T0 + 3600 - 299), [0]);
    }

    #[test]
    fn the_next_alert_is_the_earliest_of_several() {
        let events = [
            event("late", T0 + 7200, 30),
            event("soon", T0 + 1800, 30),
            event("past", T0 - 7200, 30),
        ];
        let s = Scheduler::new(10);
        assert_eq!(s.next_wake(&events, T0), Some(T0 + 1800 - 600));
    }

    #[test]
    fn two_events_that_overlap_both_alert_in_the_order_they_begin() {
        let events = [event("b", T0 + 1800, 60), event("a", T0 + 1500, 60)];
        let mut s = Scheduler::new(5);
        assert_eq!(s.tick(&events, T0 + 1500 - 300), [1]);
        assert_eq!(s.tick(&events, T0 + 1800 - 300), [0]);
        assert_eq!(s.pending(&events, T0 + 1800 - 300), [1, 0]);
        // A Mac that was asleep wakes with both due at once.
        let mut woke = Scheduler::new(5);
        assert_eq!(woke.tick(&events, T0 + 1500), [1, 0]);
    }

    #[test]
    fn an_alert_that_was_slept_through_is_not_shown_late() {
        let events = [event("e", T0 + 600, 30)];
        let mut s = Scheduler::new(5);
        assert!(s.tick(&events, T0 + 600 + GRACE).is_empty());
        assert_eq!(s.next_wake(&events, T0 + 600 + GRACE), None);
        let mut slept = Scheduler::new(5);
        assert_eq!(slept.tick(&events, T0 + 600 + GRACE - 1), [0]);
    }

    #[test]
    fn changing_the_lead_time_moves_alerts_that_have_not_fired_and_keeps_those_that_have() {
        let events = [event("e", T0 + 3600, 30), event("f", T0 + 7200, 30)];
        let mut s = Scheduler::new(5);
        assert_eq!(s.tick(&events, T0 + 3600 - 300), [0]);
        s.set_lead(30);
        // Already fired: not again. Not yet fired: its time moved with the lead.
        assert!(s.tick(&events, T0 + 3600 - 200).is_empty());
        assert_eq!(s.next_wake(&events, T0 + 3800), Some(T0 + 7200 - 1800));
        s.set_lead(1);
        assert_eq!(s.next_wake(&events, T0 + 3800), Some(T0 + 7200 - 60));
        s.set_lead(0);
        assert_eq!(s.next_wake(&events, T0 + 3800), Some(T0 + 7200 - 60));
        s.set_lead(9999);
        assert_eq!(s.tick(&events, T0 + 3800), [1]);
    }

    #[test]
    fn a_moved_event_alerts_again_at_its_new_time() {
        let mut s = Scheduler::new(5);
        let before = [event("e", T0 + 600, 30)];
        assert_eq!(s.tick(&before, T0 + 300), [0]);
        let after = [event("e", T0 + 5000, 30)];
        assert!(s.pending(&after, T0 + 400).is_empty());
        assert_eq!(s.next_wake(&after, T0 + 400), Some(T0 + 5000 - 300));
        assert_eq!(s.tick(&after, T0 + 5000 - 300), [0]);
    }

    #[test]
    fn a_deleted_event_takes_its_alert_with_it() {
        let mut s = Scheduler::new(5);
        let events = [event("e", T0 + 600, 30)];
        assert_eq!(s.tick(&events, T0 + 300), [0]);
        assert!(s.pending(&[], T0 + 301).is_empty());
        s.tick(&[], T0 + 302);
        assert!(s.fired.is_empty());
    }

    #[test]
    fn dismissing_clears_the_pending_alert_and_a_late_one_expires() {
        let events = [event("e", T0 + 600, 30)];
        let mut s = Scheduler::new(5);
        s.tick(&events, T0 + 300);
        assert_eq!(s.next_wake(&events, T0 + 300), Some(T0 + 600 + LINGER));
        assert_eq!(s.pending(&events, T0 + 600 + LINGER - 1), [0]);
        assert!(s.pending(&events, T0 + 600 + LINGER).is_empty());
        s.dismiss(&events[0]);
        assert!(s.pending(&events, T0 + 400).is_empty());
    }

    #[test]
    fn all_day_events_never_alert() {
        let mut all_day = event("holiday", T0 - 3600, 24 * 60);
        all_day.all_day = true;
        let mut s = Scheduler::new(5);
        assert!(s.tick(&[all_day.clone()], T0).is_empty());
        assert_eq!(s.next_wake(&[all_day], T0 - 7200), None);
    }

    #[test]
    fn an_occurrence_of_a_recurring_event_is_its_own_alert() {
        let mut s = Scheduler::new(5);
        let week = 7 * 24 * 3600;
        let events = [
            event("weekly", T0 + 600, 30),
            event("weekly", T0 + 600 + week, 30),
        ];
        assert_eq!(s.tick(&events, T0 + 300), [0]);
        assert_eq!(s.next_wake(&events, T0 + 300), Some(T0 + 600 + LINGER));
        assert_eq!(s.tick(&events, T0 + 600 + week - 300), [1]);
    }
}
