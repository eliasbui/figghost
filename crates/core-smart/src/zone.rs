//! Clock times that name a time zone ("3pm PST", "15:30 UTC+7", "9am in Tokyo"), shown in other zones.
//!
//! The zone database stays with the system: whoever calls `convert` passes a function that says how far a
//! zone is from UTC at a given moment, which is what makes daylight saving time come out right.

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Source {
    /// No zone was written: the time is the reader's own.
    Local,
    /// An offset that the text itself fixes: "PST", "UTC+7".
    Fixed { minutes: i32, label: String },
    /// A place, as an IANA identifier.
    Named(&'static str),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Query {
    pub hour: u32,
    pub minute: u32,
    /// The text used AM and PM, so the answers do as well.
    pub twelve_hour: bool,
    pub source: Source,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Row {
    /// The target as it was asked for: an IANA identifier, "UTC" or "local".
    pub zone: String,
    pub hour: u32,
    pub minute: u32,
    /// Days after (positive) or before the date of the text.
    pub day: i32,
}

/// Abbreviations that fix an offset by themselves, in minutes. The ambiguous ones take the common reading:
/// CST is North American and IST is Indian.
#[rustfmt::skip]
const ABBREVIATIONS: &[(&str, i32)] = &[
    ("UTC", 0), ("GMT", 0), ("EST", -300), ("EDT", -240), ("CST", -360), ("CDT", -300), ("MST", -420),
    ("MDT", -360), ("PST", -480), ("PDT", -420), ("AKST", -540), ("AKDT", -480), ("HST", -600),
    ("BST", 60), ("WET", 0), ("WEST", 60), ("CET", 60), ("CEST", 120), ("EET", 120), ("EEST", 180),
    ("MSK", 180), ("IST", 330), ("PKT", 300), ("ICT", 420), ("WIB", 420), ("SGT", 480), ("HKT", 480),
    ("PHT", 480), ("JST", 540), ("KST", 540), ("AEST", 600), ("AEDT", 660), ("ACST", 570), ("AWST", 480),
    ("NZST", 720), ("NZDT", 780), ("SAST", 120), ("WAT", 60), ("CAT", 120), ("EAT", 180), ("GST", 240),
    ("IRST", 210),
];

#[rustfmt::skip]
const CITIES: &[(&str, &str)] = &[
    ("new york", "America/New_York"), ("nyc", "America/New_York"), ("los angeles", "America/Los_Angeles"),
    ("san francisco", "America/Los_Angeles"), ("seattle", "America/Los_Angeles"),
    ("chicago", "America/Chicago"), ("denver", "America/Denver"), ("toronto", "America/Toronto"),
    ("vancouver", "America/Vancouver"), ("mexico city", "America/Mexico_City"),
    ("sao paulo", "America/Sao_Paulo"), ("london", "Europe/London"), ("dublin", "Europe/Dublin"),
    ("paris", "Europe/Paris"), ("berlin", "Europe/Berlin"), ("madrid", "Europe/Madrid"),
    ("rome", "Europe/Rome"), ("amsterdam", "Europe/Amsterdam"), ("zurich", "Europe/Zurich"),
    ("stockholm", "Europe/Stockholm"), ("warsaw", "Europe/Warsaw"), ("istanbul", "Europe/Istanbul"),
    ("moscow", "Europe/Moscow"), ("dubai", "Asia/Dubai"), ("mumbai", "Asia/Kolkata"),
    ("delhi", "Asia/Kolkata"), ("bangalore", "Asia/Kolkata"), ("bangkok", "Asia/Bangkok"),
    ("hanoi", "Asia/Ho_Chi_Minh"), ("ho chi minh", "Asia/Ho_Chi_Minh"),
    ("ho chi minh city", "Asia/Ho_Chi_Minh"), ("saigon", "Asia/Ho_Chi_Minh"),
    ("singapore", "Asia/Singapore"), ("kuala lumpur", "Asia/Kuala_Lumpur"), ("jakarta", "Asia/Jakarta"),
    ("manila", "Asia/Manila"), ("hong kong", "Asia/Hong_Kong"), ("shanghai", "Asia/Shanghai"),
    ("beijing", "Asia/Shanghai"), ("taipei", "Asia/Taipei"), ("seoul", "Asia/Seoul"),
    ("tokyo", "Asia/Tokyo"), ("sydney", "Australia/Sydney"), ("melbourne", "Australia/Melbourne"),
    ("auckland", "Pacific/Auckland"), ("cairo", "Africa/Cairo"), ("johannesburg", "Africa/Johannesburg"),
    ("lagos", "Africa/Lagos"), ("nairobi", "Africa/Nairobi"),
];

/// The clock at the start of `text`: its hour and minute, whether AM or PM was written, and where it ends.
/// A bare number is not a time; it needs a colon, an "h" ("15h30") or AM/PM.
fn parse_clock(text: &str) -> Option<(u32, u32, bool, usize)> {
    let bytes = text.as_bytes();
    let digits = |from: usize, most: usize| -> usize {
        bytes[from..]
            .iter()
            .take(most)
            .take_while(|b| b.is_ascii_digit())
            .count()
    };
    let hour_len = digits(0, 2);
    if hour_len == 0 {
        return None;
    }
    let mut hour: u32 = text[..hour_len].parse().ok()?;
    let mut at = hour_len;
    let mut minute = 0;
    let mut marked = false;
    if matches!(bytes.get(at), Some(b':' | b'h' | b'H')) {
        let colon = bytes[at] == b':';
        let minutes = digits(at + 1, 2);
        if minutes == 2 {
            minute = text[at + 1..at + 3].parse().ok()?;
            at += 3;
            marked = true;
            // Seconds are allowed and ignored.
            if colon && bytes.get(at) == Some(&b':') && digits(at + 1, 2) == 2 {
                at += 3;
            }
        } else if !colon && minutes == 0 {
            // "15h"
            at += 1;
            marked = true;
        } else {
            return None;
        }
    }
    let rest = text[at..].trim_start();
    let lower = rest.to_lowercase();
    let mut twelve = false;
    let mut pm = false;
    for (word, is_pm) in [("a.m.", false), ("p.m.", true), ("am", false), ("pm", true)] {
        if lower.starts_with(word) && !lower[word.len()..].starts_with(|c: char| c.is_alphabetic())
        {
            twelve = true;
            pm = is_pm;
            at = text.len() - rest.len() + word.len();
            break;
        }
    }
    if !(marked || twelve) || minute > 59 {
        return None;
    }
    if twelve {
        if !(1..=12).contains(&hour) {
            return None;
        }
        hour = hour % 12 + if pm { 12 } else { 0 };
    } else if hour > 23 {
        return None;
    }
    Some((hour, minute, twelve, at))
}

/// "+7", "+07:00", "-0530": a sign, hours, and optional minutes.
fn parse_offset(text: &str) -> Option<i32> {
    let sign = match text.chars().next()? {
        '+' => 1,
        '-' | '\u{2212}' => -1,
        _ => return None,
    };
    let body = &text[text.chars().next()?.len_utf8()..];
    let digits: String = body.chars().filter(|c| *c != ':').collect();
    if body.matches(':').count() > 1
        || digits.is_empty()
        || digits.len() > 4
        || !digits.bytes().all(|b| b.is_ascii_digit())
    {
        return None;
    }
    let (hours, minutes) = match (body.contains(':'), digits.len()) {
        (true, _) => {
            let (h, m) = body.split_once(':')?;
            (h.parse::<i32>().ok()?, m.parse::<i32>().ok()?)
        }
        (false, 1 | 2) => (digits.parse().ok()?, 0),
        (false, 3) => (digits[..1].parse().ok()?, digits[1..].parse().ok()?),
        (false, _) => (digits[..2].parse().ok()?, digits[2..].parse().ok()?),
    };
    (hours <= 14 && minutes <= 59).then_some(sign * (hours * 60 + minutes))
}

fn offset_label(minutes: i32) -> String {
    let sign = if minutes < 0 { '-' } else { '+' };
    let (h, m) = (minutes.abs() / 60, minutes.abs() % 60);
    if m == 0 {
        format!("UTC{sign}{h}")
    } else {
        format!("UTC{sign}{h}:{m:02}")
    }
}

fn parse_zone(text: &str) -> Option<Source> {
    let name = text.trim().trim_end_matches('.').trim();
    if name.is_empty() {
        return Some(Source::Local);
    }
    let lower = name.to_lowercase();
    let place = ["in ", "at ", "@"]
        .iter()
        .find_map(|p| lower.strip_prefix(p))
        .unwrap_or(&lower)
        .trim();
    if let Some((_, id)) = CITIES.iter().find(|(city, _)| *city == place) {
        return Some(Source::Named(id));
    }
    let upper = name.to_uppercase().replace(' ', "");
    if let Some(offset) = upper
        .strip_prefix("UTC")
        .or_else(|| upper.strip_prefix("GMT"))
    {
        let minutes = if offset.is_empty() {
            0
        } else {
            parse_offset(offset)?
        };
        return Some(Source::Fixed {
            minutes,
            label: if offset.is_empty() {
                upper
            } else {
                offset_label(minutes)
            },
        });
    }
    if let Some((abbreviation, minutes)) = ABBREVIATIONS.iter().find(|(a, _)| *a == upper) {
        return Some(Source::Fixed {
            minutes: *minutes,
            label: (*abbreviation).to_owned(),
        });
    }
    let minutes = parse_offset(&upper)?;
    Some(Source::Fixed {
        minutes,
        label: offset_label(minutes),
    })
}

/// A time of day with a zone, or `None` for text that is anything else.
pub fn parse(text: &str) -> Option<Query> {
    let text = text.trim();
    let (hour, minute, twelve_hour, used) = parse_clock(text)?;
    Some(Query {
        hour,
        minute,
        twelve_hour,
        source: parse_zone(&text[used..])?,
    })
}

/// `query` in each of the `targets`, leaving out the ones that show the same clock as the source.
/// `offset` says how many minutes east of UTC a zone is at a Unix time; "local" is the reader's own zone.
pub fn convert(
    query: &Query,
    targets: &[String],
    now: i64,
    offset: &dyn Fn(&str, i64) -> Option<i32>,
) -> Vec<Row> {
    let source_id = match &query.source {
        Source::Local => Some("local"),
        Source::Named(id) => Some(*id),
        Source::Fixed { .. } => None,
    };
    let source_offset = |at: i64| match (&query.source, source_id) {
        (Source::Fixed { minutes, .. }, _) => Some(*minutes),
        (_, Some(id)) => offset(id, at),
        _ => None,
    };
    let Some(mut shift) = source_offset(now) else {
        return Vec::new();
    };
    let day0 = (now + i64::from(shift) * 60).div_euclid(86_400);
    let clock = i64::from(query.hour) * 3600 + i64::from(query.minute) * 60;
    let mut instant = day0 * 86_400 + clock - i64::from(shift) * 60;
    // The offset on that day can differ from today's across a daylight saving change.
    if let Some(on_the_day) = source_offset(instant).filter(|o| *o != shift) {
        shift = on_the_day;
        instant = day0 * 86_400 + clock - i64::from(shift) * 60;
    }
    let mut rows: Vec<Row> = Vec::new();
    for target in targets {
        let Some(there) = offset(target, instant) else {
            continue;
        };
        if there == shift || rows.iter().any(|r| r.zone == *target) {
            continue;
        }
        let wall = instant + i64::from(there) * 60;
        let seconds = wall.rem_euclid(86_400);
        rows.push(Row {
            zone: target.clone(),
            hour: (seconds / 3600) as u32,
            minute: (seconds % 3600 / 60) as u32,
            day: (wall.div_euclid(86_400) - day0) as i32,
        });
    }
    rows
}

/// "3:00 PM" or "15:00".
pub fn format_time(hour: u32, minute: u32, twelve_hour: bool) -> String {
    if twelve_hour {
        let suffix = if hour >= 12 { "PM" } else { "AM" };
        format!("{}:{minute:02} {suffix}", (hour + 11) % 12 + 1)
    } else {
        format!("{hour:02}:{minute:02}")
    }
}

/// A short name for a zone: "Ho Chi Minh" for Asia/Ho_Chi_Minh. "UTC" and "local" stay as they are.
pub fn label(zone: &str) -> String {
    zone.rsplit('/').next().unwrap_or(zone).replace('_', " ")
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 2026-10-15 03:00:00 UTC.
    const NOW: i64 = 1_792_022_400 + 3 * 3600;

    fn fixed(minutes: i32, label: &str) -> Source {
        Source::Fixed {
            minutes,
            label: label.to_owned(),
        }
    }

    #[test]
    fn clocks() {
        let q = |text: &str| parse(text).map(|q| (q.hour, q.minute, q.twelve_hour));
        assert_eq!(q("3pm"), Some((15, 0, true)));
        assert_eq!(q("3 PM"), Some((15, 0, true)));
        assert_eq!(q("3:30 pm"), Some((15, 30, true)));
        assert_eq!(q("12am"), Some((0, 0, true)));
        assert_eq!(q("12 pm"), Some((12, 0, true)));
        assert_eq!(q("9:05 a.m."), Some((9, 5, true)));
        assert_eq!(q("15:30"), Some((15, 30, false)));
        assert_eq!(q("07:05:59"), Some((7, 5, false)));
        assert_eq!(q("15h30"), Some((15, 30, false)));
        assert_eq!(q("9h"), Some((9, 0, false)));
        assert_eq!(q("0:00"), Some((0, 0, false)));
    }

    #[test]
    fn what_is_not_a_clock() {
        for text in [
            "15", "3", "25:00", "13pm", "0pm", "9:60", "3:5", "hello", "", "3 pmx", "3:30:", "1234",
        ] {
            assert_eq!(parse(text), None, "{text}");
        }
    }

    #[test]
    fn zones_by_abbreviation_offset_or_place() {
        let zone = |text: &str| parse(text).map(|q| q.source);
        assert_eq!(zone("3pm"), Some(Source::Local));
        assert_eq!(zone("3pm PST"), Some(fixed(-480, "PST")));
        assert_eq!(zone("3pm pst."), Some(fixed(-480, "PST")));
        assert_eq!(zone("15:30 UTC"), Some(fixed(0, "UTC")));
        assert_eq!(zone("15:30 UTC+7"), Some(fixed(420, "UTC+7")));
        assert_eq!(zone("15:30 UTC +7"), Some(fixed(420, "UTC+7")));
        assert_eq!(zone("15:30 GMT-5"), Some(fixed(-300, "UTC-5")));
        assert_eq!(zone("15:30 UTC+05:30"), Some(fixed(330, "UTC+5:30")));
        assert_eq!(zone("15:30 +0700"), Some(fixed(420, "UTC+7")));
        assert_eq!(zone("15:30 -08:00"), Some(fixed(-480, "UTC-8")));
        assert_eq!(zone("9am JST"), Some(fixed(540, "JST")));
        assert_eq!(zone("9am in Tokyo"), Some(Source::Named("Asia/Tokyo")));
        assert_eq!(
            zone("9am Ho Chi Minh"),
            Some(Source::Named("Asia/Ho_Chi_Minh"))
        );
        assert_eq!(
            zone("9am at New York"),
            Some(Source::Named("America/New_York"))
        );
        assert_eq!(zone("9am Atlantis"), None);
        assert_eq!(zone("9am UTC+15"), None, "no zone is that far east");
        assert_eq!(zone("9am PSTX"), None);
    }

    /// Fake zones: Vietnam as the reader's own, New York with a daylight saving period.
    fn fake(zone: &str, at: i64) -> Option<i32> {
        match zone {
            "local" | "Asia/Ho_Chi_Minh" => Some(420),
            "UTC" => Some(0),
            "Asia/Tokyo" => Some(540),
            // From 2026-03-08 to 2026-11-01 (UTC midnight, close enough for a test).
            "America/New_York" => Some(if (1_772_928_000..1_793_491_200).contains(&at) {
                -240
            } else {
                -300
            }),
            _ => None,
        }
    }

    fn targets(list: &[&str]) -> Vec<String> {
        list.iter().map(|s| (*s).to_owned()).collect()
    }

    #[test]
    fn a_fixed_zone_to_others_and_the_day_that_changes() {
        // 15:00 PST on the 14th is 23:00 UTC, 06:00 on the 15th in Vietnam, 08:00 in Tokyo.
        let q = parse("3pm PST").unwrap();
        let rows = convert(&q, &targets(&["local", "UTC", "Asia/Tokyo"]), NOW, &fake);
        let shown: Vec<(&str, u32, u32, i32)> = rows
            .iter()
            .map(|r| (r.zone.as_str(), r.hour, r.minute, r.day))
            .collect();
        assert_eq!(
            shown,
            [
                ("local", 6, 0, 1),
                ("UTC", 23, 0, 0),
                ("Asia/Tokyo", 8, 0, 1)
            ]
        );
    }

    #[test]
    fn a_time_without_a_zone_is_the_readers_and_leaves_their_own_zone_out() {
        let q = parse("9:30").unwrap();
        let rows = convert(
            &q,
            &targets(&["local", "UTC", "Asia/Ho_Chi_Minh", "America/New_York"]),
            NOW,
            &fake,
        );
        let shown: Vec<(&str, u32, i32)> = rows
            .iter()
            .map(|r| (r.zone.as_str(), r.hour, r.day))
            .collect();
        // Vietnam is the reader's zone twice over; 09:30 there is 02:30 UTC and 22:30 the evening before in New York.
        assert_eq!(shown, [("UTC", 2, 0), ("America/New_York", 22, -1)]);
        assert_eq!(rows[0].minute, 30);
    }

    #[test]
    fn a_place_follows_daylight_saving() {
        let q = parse("12:00 in New York").unwrap();
        let rows = convert(&q, &targets(&["UTC"]), NOW, &fake);
        // October 15th is still summer time there: noon is 16:00 UTC, not 17:00.
        assert_eq!((rows[0].hour, rows[0].day), (16, 0));
        let winter = NOW + 60 * 86_400;
        let rows = convert(&q, &targets(&["UTC"]), winter, &fake);
        assert_eq!(rows[0].hour, 17);
    }

    #[test]
    fn zones_that_are_not_known_or_repeat_are_skipped() {
        let q = parse("3pm UTC").unwrap();
        let rows = convert(
            &q,
            &targets(&["UTC", "Mars/Olympus", "Asia/Tokyo", "Asia/Tokyo"]),
            NOW,
            &fake,
        );
        assert_eq!(
            rows.len(),
            1,
            "UTC is the source, Mars is unknown, Tokyo once"
        );
        assert_eq!(rows[0].zone, "Asia/Tokyo");
        // If the reader's own zone cannot be found there is nothing to say.
        assert!(
            convert(&parse("3pm").unwrap(), &targets(&["UTC"]), NOW, &|_, _| {
                None
            })
            .is_empty()
        );
    }

    #[test]
    fn times_are_written_the_way_the_text_was() {
        assert_eq!(format_time(15, 0, true), "3:00 PM");
        assert_eq!(format_time(0, 5, true), "12:05 AM");
        assert_eq!(format_time(12, 30, true), "12:30 PM");
        assert_eq!(format_time(23, 59, false), "23:59");
        assert_eq!(format_time(7, 5, false), "07:05");
    }

    #[test]
    fn zone_names_are_shortened_for_reading() {
        assert_eq!(label("Asia/Ho_Chi_Minh"), "Ho Chi Minh");
        assert_eq!(label("America/New_York"), "New York");
        assert_eq!(label("UTC"), "UTC");
        assert_eq!(label("local"), "local");
    }

    #[test]
    fn every_city_has_a_plausible_identifier() {
        for (city, id) in CITIES {
            assert_eq!(*city, city.to_lowercase());
            assert!(id.contains('/') && !id.contains(' '), "{id}");
        }
    }
}
