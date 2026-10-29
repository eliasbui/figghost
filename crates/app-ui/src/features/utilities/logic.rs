//! The small rules of the utilities: how long a keep-awake can be, and how time is written on an island.

/// The lengths offered for keeping the Mac awake, in minutes. 0 is "until I stop".
pub const AWAKE_MINUTES: [u32; 6] = [15, 30, 60, 120, 240, 0];
/// The lengths offered for snoozing the notch, in minutes. 0 is "until I wake it".
pub const SNOOZE_MINUTES: [u32; 6] = [5, 15, 30, 60, 120, 0];
/// The lengths of the keyboard lock the settings offer, in seconds.
pub const LOCK_SECONDS: (u32, u32) = (15, 300);

/// Held to the lengths that are offered; anything else is an hour. A mistake in a file or a message must not turn into
/// "until I stop", which keeps the Mac awake for ever.
pub fn awake_minutes(minutes: u32) -> u32 {
    if AWAKE_MINUTES.contains(&minutes) {
        minutes
    } else {
        60
    }
}

/// Held to the lengths that are offered; anything else is half an hour.
pub fn snooze_minutes(minutes: u32) -> u32 {
    if SNOOZE_MINUTES.contains(&minutes) {
        minutes
    } else {
        30
    }
}

/// The ways the link of a tab can be copied; the first is the default.
pub const LINK_FORMATS: [&str; 3] = ["markdown", "url", "title_url"];

/// Held to the formats that are offered; anything else is Markdown.
pub fn link_format(format: &str) -> &'static str {
    LINK_FORMATS
        .into_iter()
        .find(|known| *known == format)
        .unwrap_or(LINK_FORMATS[0])
}

/// The link of a tab as it is copied: `[title](address)`, the address alone, or the title and the address on two lines.
/// In Markdown the brackets of a title are escaped and the spaces and parentheses of an address are written as `%`
/// codes, so the whole stays one link; a tab with no title is named by its address.
pub fn link_text(format: &str, title: &str, url: &str) -> String {
    let title = title.trim();
    match link_format(format) {
        "url" => url.to_owned(),
        "title_url" if title.is_empty() => url.to_owned(),
        "title_url" => format!("{title}\n{url}"),
        _ => {
            let name = if title.is_empty() { url } else { title };
            let mut escaped = String::with_capacity(name.len());
            for c in name.chars() {
                if matches!(c, '\\' | '[' | ']') {
                    escaped.push('\\');
                }
                escaped.push(c);
            }
            let address = url
                .replace(' ', "%20")
                .replace('(', "%28")
                .replace(')', "%29");
            format!("[{escaped}]({address})")
        }
    }
}

/// Seconds as a clock: `0:42`, `4:05`, `1:02:09`.
pub fn clock(seconds: u64) -> String {
    let (h, m, s) = (seconds / 3600, seconds % 3600 / 60, seconds % 60);
    if h > 0 {
        format!("{h}:{m:02}:{s:02}")
    } else {
        format!("{m}:{s:02}")
    }
}

/// What is left, as it is said in a menu: `45 s`, `12 min`, `1 h 05 min`. A minute that has begun counts.
pub fn left(seconds: u64) -> String {
    if seconds < 60 {
        return format!("{seconds} s");
    }
    let minutes = seconds.div_ceil(60);
    if minutes < 60 {
        format!("{minutes} min")
    } else {
        format!("{} h {:02} min", minutes / 60, minutes % 60)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_length_that_is_not_offered_is_an_hour_and_never_until_stopped() {
        assert_eq!(awake_minutes(60), 60);
        assert_eq!(
            awake_minutes(0),
            0,
            "until I stop is offered, and chosen on purpose"
        );
        assert_eq!(awake_minutes(37), 60);
        assert_eq!(awake_minutes(100_000), 60);
        assert_eq!(awake_minutes(u32::MAX), 60);
    }

    #[test]
    fn a_snooze_that_is_not_offered_is_half_an_hour() {
        for minutes in SNOOZE_MINUTES {
            assert_eq!(snooze_minutes(minutes), minutes);
        }
        assert_eq!(snooze_minutes(7), 30);
        assert_eq!(snooze_minutes(u32::MAX), 30);
    }

    #[test]
    fn a_link_is_written_in_the_format_chosen_and_stays_one_link_in_markdown() {
        let url = "https://example.com/a";
        assert_eq!(
            link_text("markdown", "An example", url),
            "[An example](https://example.com/a)"
        );
        assert_eq!(link_text("url", "An example", url), url);
        assert_eq!(
            link_text("title_url", " An example ", url),
            "An example\nhttps://example.com/a"
        );
        assert_eq!(link_text("title_url", "", url), url);
        assert_eq!(
            link_text("nonsense", "", url),
            "[https://example.com/a](https://example.com/a)"
        );
        assert_eq!(
            link_text("markdown", "[Draft] a\\b", "https://w.org/A (b)"),
            "[\\[Draft\\] a\\\\b](https://w.org/A%20%28b%29)"
        );
        assert_eq!(link_format("url"), "url");
        assert_eq!(link_format("HTML"), "markdown");
    }

    #[test]
    fn the_clock_has_minutes_and_seconds_and_hours_when_needed() {
        assert_eq!(clock(0), "0:00");
        assert_eq!(clock(42), "0:42");
        assert_eq!(clock(245), "4:05");
        assert_eq!(clock(3729), "1:02:09");
    }

    #[test]
    fn what_is_left_rounds_up_to_the_minute_that_has_begun() {
        assert_eq!(left(45), "45 s");
        assert_eq!(left(60), "1 min");
        assert_eq!(left(61), "2 min");
        assert_eq!(left(3600), "1 h 00 min");
        assert_eq!(left(3900), "1 h 05 min");
    }
}
