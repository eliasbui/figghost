//! Turns events, reminders and the permission state into what the notch page draws. Pure data in, pure data
//! out, so the wording and the choices (which row says "now", when the note replaces the list) are tested
//! without a screen.

use super::Config;
use crate::i18n::Language;
use core_calendar::{
    format::{self, Offset},
    links::Link,
    model::{Event, Reminder},
};
use macos_sys::{
    calendar_ui::{AlertData, CalendarDisplay, DayData, EventData, NoteData, ReminderData},
    eventkit::Access,
    style::Tone,
};

/// What the page needs to know to be drawn.
pub struct Input<'a> {
    pub language: Language,
    pub now: i64,
    pub offset: Offset<'a>,
    pub config: &'a Config,
    pub events: &'a [Event],
    pub reminders: &'a [Reminder],
    /// Positions in `events` of the alerts that wait for the person.
    pub pending: &'a [usize],
    pub events_access: Access,
    pub reminders_access: Access,
    pub can_prompt: bool,
}

/// A row is "soon" when it begins within this many seconds.
const SOON: i64 = 15 * 60;

pub fn build(input: &Input) -> CalendarDisplay {
    let language = input.language;
    let mut display = CalendarDisplay {
        reminders_heading: language.calendar_reminders().into(),
        more: language.calendar_more().into(),
        ..CalendarDisplay::default()
    };
    if !input.config.enabled {
        display.note = Some(NoteData {
            symbol: "calendar",
            title: language.calendar_off_title().into(),
            hint: language.calendar_off_hint().into(),
            action: None,
        });
        return display;
    }
    let events_ok = input.events_access == Access::Granted;
    let reminders_ok = input.config.reminders && input.reminders_access == Access::Granted;
    if !events_ok && !reminders_ok {
        display.note = Some(access_note(input));
        return display;
    }

    let lang = language.calendar_lang();
    let today = format::day_index(input.now, input.offset);
    let events = if events_ok { input.events } else { &[] };
    for day in format::group(events, input.now, input.offset) {
        display.days.push(DayData {
            heading: format::day_label(lang, day.index, today),
            date: format::date(lang, day.index),
            events: day
                .events
                .iter()
                .map(|&i| event_row(input, &events[i], day.index == today))
                .collect(),
        });
    }
    for &i in input.pending.iter().filter(|&&i| i < events.len()) {
        display.alerts.push(alert(input, &events[i]));
    }
    if display.alerts.len() > 1 {
        display.more_alerts = language.calendar_more_alerts(display.alerts.len() - 1);
        display.alerts.truncate(1);
    }
    if reminders_ok {
        let mut sorted: Vec<&Reminder> = input.reminders.iter().collect();
        sorted.sort_by(|a, b| (a.due, &a.title).cmp(&(b.due, &b.title)));
        display.reminders = sorted.into_iter().map(|r| reminder_row(input, r)).collect();
    }
    if display.days.is_empty() && display.reminders.is_empty() {
        display.note = Some(NoteData {
            symbol: "calendar",
            title: language.calendar_empty_title().into(),
            hint: language.calendar_empty_hint().into(),
            action: None,
        });
    }
    display
}

/// Why there is nothing to show, and what could be done about it.
fn access_note(input: &Input) -> NoteData {
    let language = input.language;
    let (symbol, title, hint, action) = match input.events_access {
        Access::NotDetermined if input.can_prompt => (
            "calendar.badge.plus",
            language.calendar_ask_title(),
            language.calendar_ask_hint(),
            Some(language.calendar_ask_action()),
        ),
        Access::NotDetermined => (
            "calendar.badge.exclamationmark",
            language.calendar_denied_title(),
            language.calendar_no_prompt_hint(),
            None,
        ),
        Access::Restricted => (
            "lock",
            language.calendar_restricted_title(),
            language.calendar_restricted_hint(),
            None,
        ),
        Access::WriteOnly => (
            "calendar.badge.exclamationmark",
            language.calendar_denied_title(),
            language.calendar_write_only_hint(),
            Some(language.calendar_open_settings()),
        ),
        Access::Denied | Access::Granted => (
            "calendar.badge.exclamationmark",
            language.calendar_denied_title(),
            language.calendar_denied_hint(),
            Some(language.calendar_open_settings()),
        ),
    };
    NoteData {
        symbol,
        title: title.into(),
        hint: hint.into(),
        action: action.map(Into::into),
    }
}

fn first_line(text: &str) -> &str {
    text.lines().next().unwrap_or("").trim()
}

fn title_of(language: Language, event: &Event) -> String {
    match first_line(&event.title) {
        "" => language.calendar_untitled().to_owned(),
        title => title.to_owned(),
    }
}

/// The place of an event, or the meeting service when the place is the link itself, or the calendar.
fn detail_of(event: &Event, link: Option<&Link>) -> String {
    let place = first_line(&event.location);
    if !place.is_empty() && !place.to_ascii_lowercase().starts_with("http") {
        return place.to_owned();
    }
    link.map_or_else(|| event.calendar.clone(), |l| l.service.name().to_owned())
}

fn event_row(input: &Input, event: &Event, today: bool) -> EventData {
    let (language, lang) = (input.language, input.language.calendar_lang());
    let link = event.link();
    let (time, until) = if event.all_day {
        (format::all_day(lang).to_owned(), String::new())
    } else {
        (
            format::clock(lang, event.start, input.offset),
            if event.end > event.start {
                format::clock(lang, event.end, input.offset)
            } else {
                String::new()
            },
        )
    };
    let tag = if today && !event.all_day {
        format::relative(lang, event.start, event.end, input.now)
    } else {
        None
    };
    let tone = match (&tag, input.now >= event.start) {
        (None, _) => None,
        (Some(_), true) => Some(Tone::Green),
        (Some(_), false) if event.start - input.now <= SOON => Some(Tone::Blue),
        _ => None,
    };
    EventData {
        key: event.key(),
        color: event.color,
        time,
        until,
        title: title_of(language, event),
        detail: detail_of(event, link.as_ref()),
        tag: tag.unwrap_or_default(),
        tag_tone: tone,
        join: link.map(|_| language.calendar_join().to_owned()),
    }
}

fn alert(input: &Input, event: &Event) -> AlertData {
    let (language, lang) = (input.language, input.language.calendar_lang());
    let link = event.link();
    let started = input.now >= event.start;
    let when = format::relative(lang, event.start, event.end, input.now).unwrap_or_default();
    let span = if event.end > event.start {
        format!(
            "{} - {}",
            format::clock(lang, event.start, input.offset),
            format::clock(lang, event.end, input.offset)
        )
    } else {
        format::clock(lang, event.start, input.offset)
    };
    AlertData {
        key: event.key(),
        heading: if started {
            language.calendar_started()
        } else {
            language.calendar_starting_soon()
        }
        .into(),
        when,
        title: title_of(language, event),
        detail: format!("{span}  -  {}", detail_of(event, link.as_ref())),
        join: link.map(|l| language.calendar_join_service(l.service.name())),
        dismiss: language.calendar_dismiss().into(),
    }
}

fn reminder_row(input: &Input, reminder: &Reminder) -> ReminderData {
    let lang = input.language.calendar_lang();
    let (when, overdue) = reminder.due.map_or((String::new(), false), |due| {
        format::reminder_when(lang, due, reminder.date_only, input.now, input.offset)
    });
    ReminderData {
        key: reminder.id.clone(),
        color: reminder.color,
        title: match first_line(&reminder.title) {
            "" => input.language.calendar_untitled().to_owned(),
            title => title.to_owned(),
        },
        when,
        overdue,
        done: input.language.calendar_complete().into(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const UTC: &dyn Fn(i64) -> i32 = &|_| 0;
    /// 2026-10-15 00:00 UTC.
    const DAY: i64 = 1_792_022_400;
    const NOW: i64 = DAY + 10 * 3600;

    fn event(id: &str, title: &str, start: i64, minutes: i64) -> Event {
        Event {
            id: id.into(),
            title: title.into(),
            start,
            end: start + minutes * 60,
            all_day: false,
            calendar_id: "c".into(),
            calendar: "Work".into(),
            color: 0x3478f6,
            location: String::new(),
            notes: String::new(),
            url: String::new(),
        }
    }

    fn reminder(id: &str, title: &str, due: Option<i64>) -> Reminder {
        Reminder {
            id: id.into(),
            title: title.into(),
            due,
            date_only: false,
            calendar: "Home".into(),
            color: 0xff9f0a,
        }
    }

    fn enabled() -> Config {
        Config {
            enabled: true,
            ..Config::default()
        }
    }

    fn input<'a>(
        language: Language,
        config: &'a Config,
        events: &'a [Event],
        reminders: &'a [Reminder],
        pending: &'a [usize],
    ) -> Input<'a> {
        Input {
            language,
            now: NOW,
            offset: UTC,
            config,
            events,
            reminders,
            pending,
            events_access: Access::Granted,
            reminders_access: Access::Granted,
            can_prompt: true,
        }
    }

    #[test]
    fn events_are_grouped_under_today_and_tomorrow_in_both_languages() {
        let events = [
            event("a", "Standup", NOW + 300, 15),
            event("b", "Dentist", NOW + 86_400, 60),
        ];
        let config = enabled();
        let en = build(&input(Language::English, &config, &events, &[], &[]));
        assert_eq!(en.days.len(), 2);
        assert_eq!(
            (en.days[0].heading.as_str(), en.days[1].heading.as_str()),
            ("Today", "Tomorrow")
        );
        assert_eq!(en.days[0].date, "Thu, Oct 15");
        assert_eq!(en.days[0].events[0].time, "10:05 AM");
        assert_eq!(en.days[0].events[0].until, "10:20 AM");
        assert_eq!(en.days[0].events[0].tag, "in 5 min");
        assert_eq!(en.days[0].events[0].tag_tone, Some(Tone::Blue));
        assert_eq!(en.days[1].events[0].tag, "");
        let vi = build(&input(Language::Vietnamese, &config, &events, &[], &[]));
        assert_eq!(
            (vi.days[0].heading.as_str(), vi.days[1].heading.as_str()),
            ("Hôm nay", "Ngày mai")
        );
        assert_eq!(vi.days[0].events[0].time, "10:05");
        assert_eq!(vi.days[0].events[0].tag, "sau 5 phút");
    }

    #[test]
    fn a_running_event_says_now_and_a_finished_one_is_gone() {
        let events = [
            event("over", "Over", NOW - 7200, 60),
            event("run", "Running", NOW - 600, 60),
        ];
        let config = enabled();
        let d = build(&input(Language::English, &config, &events, &[], &[]));
        assert_eq!(d.days[0].events.len(), 1);
        assert_eq!(d.days[0].events[0].tag, "now");
        assert_eq!(d.days[0].events[0].tag_tone, Some(Tone::Green));
    }

    #[test]
    fn a_meeting_link_gives_the_row_and_the_alert_a_join_button() {
        let mut meeting = event("m", "Sync", NOW + 240, 30);
        meeting.notes = "Join: https://acme.zoom.us/j/81234567890?pwd=abc&utm_source=cal".into();
        meeting.location = "https://acme.zoom.us/j/81234567890".into();
        let plain = event("p", "Lunch", NOW + 4000, 30);
        let events = [meeting, plain];
        let config = enabled();
        let d = build(&input(Language::English, &config, &events, &[], &[0]));
        assert_eq!(d.days[0].events[0].join.as_deref(), Some("Join"));
        assert_eq!(d.days[0].events[0].detail, "Zoom");
        assert_eq!(d.days[0].events[1].join, None);
        assert_eq!(d.alerts[0].join.as_deref(), Some("Join Zoom"));
        assert_eq!(d.alerts[0].heading, "Starting soon");
        assert_eq!(d.alerts[0].when, "in 4 min");
        let vi = build(&input(Language::Vietnamese, &config, &events, &[], &[0]));
        assert_eq!(vi.alerts[0].join.as_deref(), Some("Tham gia Zoom"));
    }

    #[test]
    fn an_alert_without_a_link_has_only_dismiss_and_more_alerts_are_counted() {
        let events = [
            event("a", "One", NOW + 100, 30),
            event("b", "Two", NOW + 200, 30),
            event("c", "Three", NOW + 300, 30),
        ];
        let config = enabled();
        let d = build(&input(Language::English, &config, &events, &[], &[0, 1, 2]));
        assert_eq!(d.alerts.len(), 1);
        assert_eq!(d.alerts[0].join, None);
        assert_eq!(d.more_alerts, "2 more alerts");
        let started = build(&input(
            Language::English,
            &config,
            &[event("s", "S", NOW - 30, 30)],
            &[],
            &[0],
        ));
        assert_eq!(started.alerts[0].heading, "Meeting started");
        assert_eq!(started.alerts[0].when, "now");
    }

    #[test]
    fn titles_and_places_are_cleaned() {
        let mut odd = event("o", "  \nSecond line", NOW + 100, 30);
        odd.location = "Room 4\nBuilding B".into();
        let blank = event("b", "", NOW + 200, 30);
        let events = [odd, blank];
        let config = enabled();
        let d = build(&input(Language::English, &config, &events, &[], &[]));
        assert_eq!(d.days[0].events[0].detail, "Room 4");
        assert_eq!(d.days[0].events[1].title, "Untitled");
        assert_eq!(d.days[0].events[1].detail, "Work");
    }

    #[test]
    fn reminders_are_ordered_and_marked_late() {
        let reminders = [
            reminder("b", "Soon", Some(NOW + 3600)),
            reminder("a", "Late", Some(NOW - 3600)),
        ];
        let config = enabled();
        let d = build(&input(Language::English, &config, &[], &reminders, &[]));
        assert_eq!(d.reminders[0].title, "Late");
        assert!(d.reminders[0].overdue);
        assert_eq!(d.reminders[0].when, "Overdue 9:00 AM");
        assert_eq!(d.reminders[1].when, "11:00 AM");
        assert!(d.note.is_none());
        let off = Config {
            reminders: false,
            ..enabled()
        };
        let d = build(&input(Language::English, &off, &[], &reminders, &[]));
        assert!(d.reminders.is_empty());
        assert_eq!(d.note.unwrap().title, "Nothing planned");
    }

    #[test]
    fn a_page_with_nothing_on_it_says_why() {
        let config = enabled();
        let mut i = input(Language::English, &config, &[], &[], &[]);
        assert_eq!(build(&i).note.unwrap().title, "Nothing planned");

        i.events_access = Access::NotDetermined;
        i.reminders_access = Access::NotDetermined;
        let note = build(&i).note.unwrap();
        assert_eq!(note.title, "Allow access to your calendar");
        assert_eq!(note.action.as_deref(), Some("Allow access"));

        i.can_prompt = false;
        assert_eq!(build(&i).note.unwrap().action, None);
        i.can_prompt = true;

        i.events_access = Access::Denied;
        let note = build(&i).note.unwrap();
        assert_eq!(note.action.as_deref(), Some("Open System Settings"));
        i.language = Language::Vietnamese;
        assert_eq!(build(&i).note.unwrap().title, "Chưa có quyền truy cập lịch");
        i.language = Language::English;

        i.events_access = Access::Restricted;
        assert_eq!(build(&i).note.unwrap().action, None);
        i.events_access = Access::WriteOnly;
        assert!(build(&i).note.unwrap().hint.contains("full access"));

        let off = Config::default();
        let d = build(&input(Language::English, &off, &[], &[], &[]));
        assert_eq!(d.note.unwrap().title, "Calendar is off");
    }

    #[test]
    fn reminders_alone_are_shown_when_events_are_not_allowed() {
        let config = enabled();
        let reminders = [reminder("a", "Call", Some(NOW + 60))];
        let mut i = input(Language::English, &config, &[], &reminders, &[]);
        i.events_access = Access::Denied;
        let d = build(&i);
        assert!(d.note.is_none());
        assert_eq!(d.reminders.len(), 1);
    }
}
