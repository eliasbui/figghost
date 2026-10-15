//! What the page and the scheduler need to know about an event or a reminder.

use crate::links::{self, Link};

/// One occurrence of an event, as EventKit expanded it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Event {
    /// The identifier of the event; every occurrence of a recurring event shares it.
    pub id: String,
    pub title: String,
    pub start: i64,
    pub end: i64,
    pub all_day: bool,
    pub calendar_id: String,
    pub calendar: String,
    /// 0xRRGGBB.
    pub color: u32,
    pub location: String,
    pub notes: String,
    pub url: String,
}

impl Event {
    /// Identifies this occurrence: a moved event is a new key, which is how it alerts again at its new time.
    pub fn key(&self) -> String {
        format!("{}@{}", self.id, self.start)
    }

    /// The meeting link, preferring the URL field, then the location, then the notes.
    pub fn link(&self) -> Option<Link> {
        links::find(&[&self.url, &self.location, &self.notes])
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Reminder {
    pub id: String,
    pub title: String,
    /// `None` for a reminder with no date.
    pub due: Option<i64>,
    /// The due date has no time of day.
    pub date_only: bool,
    pub calendar: String,
    pub color: u32,
}
