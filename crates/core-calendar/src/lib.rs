#![forbid(unsafe_code)]
//! Calendar logic with no interface and no EventKit: meeting links, which alert fires when, which events
//! belong to which day, and how times are written in each language. Times are seconds since the Unix epoch;
//! the local time zone is passed in as a function from an instant to its offset from UTC, so the rules around
//! midnight and daylight saving changes are tested without touching the system.

pub mod format;
pub mod links;
pub mod model;
pub mod schedule;
