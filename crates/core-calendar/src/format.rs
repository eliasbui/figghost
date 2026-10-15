//! Days, times and the short phrases of the page ("Today", "in 5 min", "now") in each language, and the
//! grouping of events by local day. The local time zone is a function from an instant to its offset from UTC
//! in seconds, so none of this reads the system.

use crate::model::Event;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Lang {
    En,
    Vi,
}

/// The offset from UTC, in seconds, that applies at an instant.
pub type Offset<'a> = &'a dyn Fn(i64) -> i32;

const DAY: i64 = 86_400;

/// The local day an instant falls on, counted from 1970-01-01.
pub fn day_index(t: i64, offset: Offset) -> i64 {
    (t + i64::from(offset(t))).div_euclid(DAY)
}

/// The instant a local day begins. The offset is looked up three times because the offset at midnight is what
/// matters and a change of offset (daylight saving) may sit between the guess and the answer.
pub fn day_start(day: i64, offset: Offset) -> i64 {
    let mut guess = day * DAY;
    for _ in 0..3 {
        guess = day * DAY - i64::from(offset(guess));
    }
    guess
}

fn civil(days: i64) -> (i64, u32, u32) {
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let month = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    (yoe + era * 400 + i64::from(month <= 2), month, day)
}

/// 0 is Sunday.
fn weekday(day: i64) -> usize {
    (day + 4).rem_euclid(7) as usize
}

/// "9:30 AM" in English, "09:30" in Vietnamese.
pub fn clock(lang: Lang, t: i64, offset: Offset) -> String {
    let secs = (t + i64::from(offset(t))).rem_euclid(DAY);
    let (hour, minute) = (secs / 3600, secs % 3600 / 60);
    match lang {
        Lang::En => {
            let twelve = if hour % 12 == 0 { 12 } else { hour % 12 };
            format!(
                "{twelve}:{minute:02} {}",
                if hour < 12 { "AM" } else { "PM" }
            )
        }
        Lang::Vi => format!("{hour:02}:{minute:02}"),
    }
}

/// "Thu, Oct 15" or "Thứ Năm, 15 tháng 10".
pub fn date(lang: Lang, day: i64) -> String {
    let (_, month, d) = civil(day);
    match lang {
        Lang::En => {
            const WEEKDAYS: [&str; 7] = ["Sun", "Mon", "Tue", "Wed", "Thu", "Fri", "Sat"];
            const MONTHS: [&str; 12] = [
                "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
            ];
            format!(
                "{}, {} {d}",
                WEEKDAYS[weekday(day)],
                MONTHS[month as usize - 1]
            )
        }
        Lang::Vi => {
            const WEEKDAYS: [&str; 7] = [
                "Chủ Nhật",
                "Thứ Hai",
                "Thứ Ba",
                "Thứ Tư",
                "Thứ Năm",
                "Thứ Sáu",
                "Thứ Bảy",
            ];
            format!("{}, {d} tháng {month}", WEEKDAYS[weekday(day)])
        }
    }
}

/// "Today", "Tomorrow", or the date for any other day.
pub fn day_label(lang: Lang, day: i64, today: i64) -> String {
    match (day - today, lang) {
        (0, Lang::En) => "Today".into(),
        (0, Lang::Vi) => "Hôm nay".into(),
        (1, Lang::En) => "Tomorrow".into(),
        (1, Lang::Vi) => "Ngày mai".into(),
        _ => date(lang, day),
    }
}

pub fn all_day(lang: Lang) -> &'static str {
    match lang {
        Lang::En => "All day",
        Lang::Vi => "Cả ngày",
    }
}

/// How far off an event is: "now" while it runs, "in 5 min", "in 1 h 20 min"; `None` once it is over.
pub fn relative(lang: Lang, start: i64, end: i64, now: i64) -> Option<String> {
    if now >= end.max(start + 1) {
        return None;
    }
    if now >= start {
        return Some(
            match lang {
                Lang::En => "now",
                Lang::Vi => "đang diễn ra",
            }
            .into(),
        );
    }
    let minutes = (start - now + 59) / 60;
    let (hours, rest) = (minutes / 60, minutes % 60);
    Some(match (lang, hours, rest) {
        (Lang::En, 0, m) => format!("in {m} min"),
        (Lang::En, h, 0) => format!("in {h} h"),
        (Lang::En, h, m) => format!("in {h} h {m} min"),
        (Lang::Vi, 0, m) => format!("sau {m} phút"),
        (Lang::Vi, h, 0) => format!("sau {h} giờ"),
        (Lang::Vi, h, m) => format!("sau {h} giờ {m} phút"),
    })
}

/// When a reminder is due, and whether that is already past.
pub fn reminder_when(
    lang: Lang,
    due: i64,
    date_only: bool,
    now: i64,
    offset: Offset,
) -> (String, bool) {
    let (today, day) = (day_index(now, offset), day_index(due, offset));
    let overdue = if date_only { day < today } else { due < now };
    let word = |en: &str, vi: &str| match lang {
        Lang::En => en.to_owned(),
        Lang::Vi => vi.to_owned(),
    };
    let time = || clock(lang, due, offset);
    let text = match (overdue, date_only, day - today) {
        (true, true, _) => word("Overdue", "Quá hạn"),
        (true, false, 0) => format!("{} {}", word("Overdue", "Quá hạn"), time()),
        (true, false, _) => format!("{} {}", word("Overdue", "Quá hạn"), date(lang, day)),
        (false, true, 0) => day_label(lang, day, today),
        (false, true, _) => date(lang, day),
        (false, false, 0) => time(),
        (false, false, _) => format!("{} {}", day_label(lang, day, today), time()),
    };
    (text, overdue)
}

/// The events of one local day, in the order the page shows them.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Day {
    pub index: i64,
    /// Positions in the event list given to `group`.
    pub events: Vec<usize>,
}

/// Today's and tomorrow's events. A timed event that is over is left out; one that began earlier and still runs
/// belongs to today; an all-day event belongs to every day it covers. All-day events come first.
pub fn group(events: &[Event], now: i64, offset: Offset) -> Vec<Day> {
    let today = day_index(now, offset);
    let mut days = Vec::new();
    for index in [today, today + 1] {
        let mut here: Vec<usize> = events
            .iter()
            .enumerate()
            .filter(|(_, e)| {
                let last = e.end.max(e.start + 1) - 1;
                (e.all_day || e.end > now)
                    && day_index(e.start, offset) <= index
                    && day_index(last, offset) >= index
            })
            .map(|(i, _)| i)
            .collect();
        here.sort_by(|&a, &b| {
            let (a, b) = (&events[a], &events[b]);
            (!a.all_day, a.start, &a.title, &a.id).cmp(&(!b.all_day, b.start, &b.title, &b.id))
        });
        if !here.is_empty() {
            days.push(Day {
                index,
                events: here,
            });
        }
    }
    days
}

#[cfg(test)]
mod tests {
    use super::*;

    const UTC: &dyn Fn(i64) -> i32 = &|_| 0;
    /// 2026-10-15 00:00:00 UTC.
    const OCT15: i64 = 1_792_022_400;

    fn event(id: &str, title: &str, start: i64, end: i64, all_day: bool) -> Event {
        Event {
            id: id.into(),
            title: title.into(),
            start,
            end,
            all_day,
            calendar_id: "c".into(),
            calendar: "Work".into(),
            color: 0x3478f6,
            location: String::new(),
            notes: String::new(),
            url: String::new(),
        }
    }

    #[test]
    fn dates_come_out_right_in_both_languages() {
        assert_eq!(date(Lang::En, OCT15 / DAY), "Thu, Oct 15");
        assert_eq!(date(Lang::Vi, OCT15 / DAY), "Thứ Năm, 15 tháng 10");
        assert_eq!(date(Lang::En, 0), "Thu, Jan 1");
        assert_eq!(date(Lang::En, 11_016), "Tue, Feb 29");
    }

    #[test]
    fn day_labels() {
        let today = OCT15 / DAY;
        assert_eq!(day_label(Lang::En, today, today), "Today");
        assert_eq!(day_label(Lang::En, today + 1, today), "Tomorrow");
        assert_eq!(day_label(Lang::Vi, today, today), "Hôm nay");
        assert_eq!(day_label(Lang::Vi, today + 1, today), "Ngày mai");
        assert_eq!(day_label(Lang::En, today + 2, today), "Sat, Oct 17");
    }

    #[test]
    fn the_clock_follows_the_language_and_the_zone() {
        assert_eq!(clock(Lang::En, OCT15 + 9 * 3600 + 30 * 60, UTC), "9:30 AM");
        assert_eq!(clock(Lang::En, OCT15, UTC), "12:00 AM");
        assert_eq!(clock(Lang::En, OCT15 + 12 * 3600, UTC), "12:00 PM");
        assert_eq!(clock(Lang::En, OCT15 + 21 * 3600 + 5 * 60, UTC), "9:05 PM");
        assert_eq!(clock(Lang::Vi, OCT15 + 9 * 3600 + 5 * 60, UTC), "09:05");
        let plus7: &dyn Fn(i64) -> i32 = &|_| 7 * 3600;
        assert_eq!(clock(Lang::Vi, OCT15 + 20 * 3600, plus7), "03:00");
        assert_eq!(day_index(OCT15 + 20 * 3600, plus7), OCT15 / DAY + 1);
    }

    #[test]
    fn relative_times() {
        let start = OCT15 + 600;
        let end = start + 1800;
        let say = |lang, now| relative(lang, start, end, now);
        assert_eq!(say(Lang::En, start - 300).unwrap(), "in 5 min");
        assert_eq!(say(Lang::En, start - 301).unwrap(), "in 6 min");
        assert_eq!(say(Lang::En, start - 1).unwrap(), "in 1 min");
        assert_eq!(say(Lang::En, start).unwrap(), "now");
        assert_eq!(say(Lang::En, end - 1).unwrap(), "now");
        assert_eq!(say(Lang::En, end), None);
        assert_eq!(say(Lang::En, start - 3600).unwrap(), "in 1 h");
        assert_eq!(say(Lang::En, start - 4800).unwrap(), "in 1 h 20 min");
        assert_eq!(say(Lang::Vi, start - 300).unwrap(), "sau 5 phút");
        assert_eq!(say(Lang::Vi, start - 4800).unwrap(), "sau 1 giờ 20 phút");
        assert_eq!(say(Lang::Vi, start).unwrap(), "đang diễn ra");
        assert_eq!(relative(Lang::En, start, start, start).unwrap(), "now");
    }

    #[test]
    fn reminders_say_when_and_whether_they_are_late() {
        let now = OCT15 + 10 * 3600;
        let when = |due, date_only, lang| reminder_when(lang, due, date_only, now, UTC);
        assert_eq!(
            when(now + 3600, false, Lang::En),
            ("11:00 AM".into(), false)
        );
        assert_eq!(
            when(now - 3600, false, Lang::En),
            ("Overdue 9:00 AM".into(), true)
        );
        assert_eq!(
            when(now - DAY, false, Lang::En),
            ("Overdue Wed, Oct 14".into(), true)
        );
        assert_eq!(when(OCT15, true, Lang::En), ("Today".into(), false));
        assert_eq!(when(OCT15 - DAY, true, Lang::Vi), ("Quá hạn".into(), true));
        assert_eq!(
            when(now + DAY, false, Lang::Vi),
            ("Ngày mai 10:00".into(), false)
        );
        assert_eq!(
            when(OCT15 + DAY, true, Lang::En),
            ("Fri, Oct 16".into(), false)
        );
    }

    #[test]
    fn events_are_grouped_by_day_and_the_finished_ones_dropped() {
        let now = OCT15 + 10 * 3600;
        let events = vec![
            event("a", "Over", OCT15 + 8 * 3600, OCT15 + 9 * 3600, false),
            event("b", "Later", OCT15 + 15 * 3600, OCT15 + 16 * 3600, false),
            event("c", "Now", OCT15 + 9 * 3600, OCT15 + 11 * 3600, false),
            event("d", "Holiday", OCT15, OCT15 + DAY - 1, true),
            event(
                "e",
                "Tomorrow",
                OCT15 + DAY + 3600,
                OCT15 + DAY + 7200,
                false,
            ),
            event(
                "f",
                "Next week",
                OCT15 + 5 * DAY,
                OCT15 + 5 * DAY + 60,
                false,
            ),
            event("g", "Overnight", OCT15 - 3600, OCT15 + 3 * DAY, false),
        ];
        let days = group(&events, now, UTC);
        assert_eq!(days.len(), 2);
        assert_eq!(days[0].index, OCT15 / DAY);
        let titles = |d: &Day| {
            d.events
                .iter()
                .map(|&i| events[i].title.as_str())
                .collect::<Vec<_>>()
        };
        assert_eq!(titles(&days[0]), ["Holiday", "Overnight", "Now", "Later"]);
        assert_eq!(titles(&days[1]), ["Overnight", "Tomorrow"]);
    }

    #[test]
    fn an_all_day_event_covers_each_day_it_spans() {
        let now = OCT15 + 3600;
        let trip = event("t", "Trip", OCT15, OCT15 + 2 * DAY - 1, true);
        let one = event("o", "One day", OCT15 + DAY, OCT15 + 2 * DAY - 1, true);
        let days = group(&[trip, one], now, UTC);
        assert_eq!(days[0].events, [0]);
        assert_eq!(days[1].events, [0, 1]);
    }

    #[test]
    fn midnight_is_found_across_a_change_of_offset() {
        // A zone that moves forward an hour at 02:00 local time on day 20_000 (offset 0 before, 3600 after).
        let change = 20_000 * DAY + 2 * 3600;
        let zone: &dyn Fn(i64) -> i32 = &|t| if t >= change { 3600 } else { 0 };
        assert_eq!(day_start(19_999, zone), 19_999 * DAY);
        assert_eq!(day_start(20_000, zone), 20_000 * DAY);
        assert_eq!(day_start(20_001, zone), 20_001 * DAY - 3600);
        assert_eq!(day_index(day_start(20_001, zone), zone), 20_001);
        assert_eq!(day_index(day_start(20_001, zone) - 1, zone), 20_000);
    }

    #[test]
    fn an_event_in_another_zone_lands_on_the_local_day() {
        // 23:30 in New York (UTC-4) is 03:30 UTC the next day; for a viewer at UTC-4 it is still the same day.
        let local: &dyn Fn(i64) -> i32 = &|_| -4 * 3600;
        let start = OCT15 + DAY + 3 * 3600 + 1800;
        let now = OCT15 + 12 * 3600;
        let events = [event("n", "Late call", start, start + 1200, false)];
        let days = group(&events, now, local);
        assert_eq!(days.len(), 1);
        assert_eq!(days[0].index, OCT15 / DAY);
        assert_eq!(clock(Lang::En, start, local), "11:30 PM");
    }
}
