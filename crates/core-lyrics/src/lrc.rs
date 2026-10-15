//! Time-stamped lyrics in the LRC format, and the lookup of the line that is sung at a given moment.
//!
//! The text comes from a server, so every limit here is a defence: input size, number of lines, line length,
//! and the range of a timestamp. Whatever does not fit is dropped, never an error.

const MAX_INPUT_BYTES: usize = 1 << 20;
pub const MAX_LINES: usize = 4000;
const MAX_LINE_CHARS: usize = 300;
/// A line with a word timestamp every few letters is longer than its text; past this it is cut.
const MAX_RAW_LINE_BYTES: usize = 8192;
/// A song is not a day long; a larger timestamp is a typing mistake or an attack.
const MAX_MS: i64 = 24 * 60 * 60 * 1000;
const MAX_OFFSET_MS: i64 = 10 * 60 * 1000;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Line {
    pub at_ms: u32,
    /// Empty for an instrumental break.
    pub text: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Lrc {
    lines: Vec<Line>,
}

/// The lines around a moment. `index` is `None` before the first line is reached.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct View<'a> {
    pub index: Option<usize>,
    pub previous: &'a str,
    pub current: &'a str,
    pub next: &'a str,
}

fn cut(text: &str, max: usize) -> &str {
    let mut end = text.len().min(max);
    while !text.is_char_boundary(end) {
        end -= 1;
    }
    &text[..end]
}

/// `mm:ss`, `mm:ss.f`, `mm:ss.ff` or `mm:ss.fff` (a colon before the fraction is also seen), in milliseconds.
fn timestamp(tag: &str) -> Option<i64> {
    let (minutes, rest) = tag.split_once(':')?;
    let (seconds, fraction) = match rest.split_once(['.', ':']) {
        Some((seconds, fraction)) => (seconds, Some(fraction)),
        None => (rest, None),
    };
    let digits = |s: &str, range: std::ops::RangeInclusive<usize>| {
        range.contains(&s.len()) && s.bytes().all(|b| b.is_ascii_digit())
    };
    if !digits(minutes, 1..=4) || !digits(seconds, 1..=2) {
        return None;
    }
    let fraction_ms = match fraction {
        None => 0,
        Some(f) if digits(f, 1..=3) => f.parse::<i64>().ok()? * [100, 10, 1][f.len() - 1],
        Some(_) => return None,
    };
    let total =
        (minutes.parse::<i64>().ok()? * 60 + seconds.parse::<i64>().ok()?) * 1000 + fraction_ms;
    (total <= MAX_MS).then_some(total)
}

/// The sung text of a line: word timestamps `<mm:ss.xx>` removed, control characters dropped, spaces collapsed.
fn clean(text: &str) -> String {
    let mut plain = String::with_capacity(text.len());
    let mut rest = cut(text, MAX_RAW_LINE_BYTES);
    while let Some(open) = rest.find('<') {
        plain.push_str(&rest[..open]);
        let after = &rest[open + 1..];
        match after.find('>') {
            Some(close) if timestamp(&after[..close]).is_some() => rest = &after[close + 1..],
            _ => {
                plain.push('<');
                rest = after;
            }
        }
    }
    plain.push_str(rest);

    let mut out = String::new();
    let mut chars = 0;
    let mut pending_space = false;
    for c in plain.chars() {
        if c.is_whitespace() {
            pending_space = !out.is_empty();
        } else if !c.is_control() {
            if pending_space {
                out.push(' ');
                chars += 1;
                pending_space = false;
            }
            out.push(c);
            chars += 1;
        }
        if chars >= MAX_LINE_CHARS {
            break;
        }
    }
    out
}

pub fn parse(text: &str) -> Lrc {
    let text = cut(text, MAX_INPUT_BYTES).trim_start_matches('\u{feff}');

    let mut offset_ms = 0;
    let mut lines: Vec<(i64, String)> = Vec::new();
    'input: for raw in text.split(['\n', '\r']) {
        let mut rest = raw.trim_start();
        let mut stamps: Vec<i64> = Vec::new();
        while let Some(tag_start) = rest.strip_prefix('[') {
            let Some(close) = tag_start.find(']') else {
                break;
            };
            let tag = &tag_start[..close];
            if let Some(ms) = timestamp(tag.trim()) {
                stamps.push(ms);
                rest = tag_start[close + 1..].trim_start();
            } else if stamps.is_empty() && tag.starts_with(|c: char| c.is_ascii_alphabetic()) {
                // A metadata line such as [ar:Artist]; only [offset:...] means anything to us.
                if let Some(value) = tag.strip_prefix("offset:") {
                    offset_ms = value
                        .trim()
                        .parse::<i64>()
                        .unwrap_or(0)
                        .clamp(-MAX_OFFSET_MS, MAX_OFFSET_MS);
                }
                break;
            } else {
                break;
            }
        }
        if stamps.is_empty() {
            continue;
        }
        let text = clean(rest);
        for ms in stamps {
            if lines.len() >= MAX_LINES {
                break 'input;
            }
            lines.push((ms, text.clone()));
        }
    }

    // A positive offset makes the lyrics come earlier, as the format defines it.
    let mut lines: Vec<Line> = lines
        .into_iter()
        .map(|(ms, text)| Line {
            at_ms: u32::try_from((ms - offset_ms).clamp(0, MAX_MS)).unwrap_or(0),
            text,
        })
        .collect();
    lines.sort_by_key(|l| l.at_ms);
    Lrc { lines }
}

impl Lrc {
    pub fn is_empty(&self) -> bool {
        self.lines.is_empty()
    }

    pub fn len(&self) -> usize {
        self.lines.len()
    }

    pub fn lines(&self) -> &[Line] {
        &self.lines
    }

    /// The index of the line that is current at `position_ms`; `None` before the first one.
    pub fn locate(&self, position_ms: i64) -> Option<usize> {
        self.lines
            .partition_point(|l| i64::from(l.at_ms) <= position_ms)
            .checked_sub(1)
    }

    pub fn view(&self, position_ms: i64) -> View<'_> {
        let text = |i: Option<usize>| i.and_then(|i| self.lines.get(i)).map_or("", |l| &*l.text);
        let index = self.locate(position_ms);
        let (previous, next) = match index {
            Some(i) => (i.checked_sub(1), Some(i + 1)),
            None => (None, Some(0)),
        };
        View {
            index,
            previous: text(previous),
            current: text(index),
            next: text(next),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Instant;

    const SAMPLE: &str = "[ti:Blue in Green]\n[ar:Miles Davis]\n[al:Kind of Blue]\n[by:someone]\n[length: 05:37]\n\
        [00:12.50]Slow and quiet\n[00:20.5]Second line\n[00:31.250]Third line\n[00:40.00]\n[00:45]Back again\n";

    fn pairs(lrc: &Lrc) -> Vec<(u32, &str)> {
        lrc.lines()
            .iter()
            .map(|l| (l.at_ms, l.text.as_str()))
            .collect()
    }

    #[test]
    fn a_realistic_file_gives_lines_in_time_order() {
        assert_eq!(
            pairs(&parse(SAMPLE)),
            vec![
                (12_500, "Slow and quiet"),
                (20_500, "Second line"),
                (31_250, "Third line"),
                (40_000, ""),
                (45_000, "Back again"),
            ]
        );
    }

    #[test]
    fn fractions_of_one_to_three_digits_and_no_fraction() {
        assert_eq!(timestamp("01:02.3"), Some(62_300));
        assert_eq!(timestamp("01:02.34"), Some(62_340));
        assert_eq!(timestamp("01:02.345"), Some(62_345));
        assert_eq!(timestamp("01:02"), Some(62_000));
        assert_eq!(timestamp("100:00.00"), Some(6_000_000));
        assert_eq!(timestamp("01:02:34"), Some(62_340));
        for bad in [
            "",
            "ab:cd",
            "1:2.3456",
            "00:00.",
            "-1:00",
            "1:234",
            "12345:00",
            "99999:99.999",
            "ar:Someone",
            "1:2.x",
        ] {
            assert_eq!(timestamp(bad), None, "{bad}");
        }
    }

    #[test]
    fn several_stamps_on_one_line_repeat_the_text() {
        let lrc = parse("[00:10.00][01:10.00] [02:10.00]Chorus line\n[00:30.00]Verse\n");
        assert_eq!(
            pairs(&lrc),
            vec![
                (10_000, "Chorus line"),
                (30_000, "Verse"),
                (70_000, "Chorus line"),
                (130_000, "Chorus line")
            ]
        );
    }

    #[test]
    fn a_positive_offset_brings_the_lyrics_earlier() {
        let lrc = parse("[offset:+500]\n[00:10.00]a\n[00:00.20]b\n");
        assert_eq!(pairs(&lrc), vec![(0, "b"), (9_500, "a")]);
        let later = parse("[offset: -1500]\n[00:10.00]a\n");
        assert_eq!(later.lines()[0].at_ms, 11_500);
        let silly = parse("[offset:99999999999999999999]\n[00:10.00]a\n");
        assert_eq!(silly.lines()[0].at_ms, 10_000);
        let after = parse("[00:10.00]a\n[offset:+1000]\n");
        assert_eq!(after.lines()[0].at_ms, 9_000);
    }

    #[test]
    fn word_timestamps_are_removed_from_the_text() {
        let lrc =
            parse("[00:01.00]<00:01.00>Hel<00:01.40>lo <00:02.00>world\n[00:05.00]a <b> c < d\n");
        assert_eq!(lrc.lines()[0].text, "Hello world");
        assert_eq!(lrc.lines()[1].text, "a <b> c < d");
    }

    #[test]
    fn metadata_and_untimed_lines_are_ignored() {
        let lrc = parse("[ar:A]\n[Chorus]\nplain words\n[00:01.00]sung\n[ve:1.0]\n");
        assert_eq!(pairs(&lrc), vec![(1_000, "sung")]);
        assert!(parse("").is_empty());
        assert!(parse("just\nplain\nlyrics\n").is_empty());
    }

    #[test]
    fn windows_and_old_mac_line_ends_and_a_bom_are_fine() {
        let lrc = parse("\u{feff}[00:01.00]one\r\n[00:02.00]two\r[00:03.00]three");
        let texts: Vec<&str> = lrc.lines().iter().map(|l| l.text.as_str()).collect();
        assert_eq!(texts, ["one", "two", "three"]);
    }

    #[test]
    fn the_current_and_neighbouring_lines_follow_the_position() {
        let lrc = parse(SAMPLE);
        let before = lrc.view(0);
        assert_eq!(
            (before.index, before.previous, before.current, before.next),
            (None, "", "", "Slow and quiet")
        );
        let first = lrc.view(12_500);
        assert_eq!(
            (first.index, first.previous, first.current, first.next),
            (Some(0), "", "Slow and quiet", "Second line")
        );
        let middle = lrc.view(33_000);
        assert_eq!(
            (middle.previous, middle.current, middle.next),
            ("Second line", "Third line", "")
        );
        let last = lrc.view(10_000_000);
        assert_eq!(
            (last.index, last.current, last.next),
            (Some(4), "Back again", "")
        );
        assert_eq!(lrc.view(-5).index, None);
        assert_eq!(lrc.locate(12_499), None);
        assert_eq!(lrc.locate(12_500), Some(0));
        assert_eq!(parse("").view(1000).index, None);
    }

    #[test]
    fn lookup_matches_a_linear_scan() {
        let mut text = String::new();
        for i in 0..500 {
            text.push_str(&format!(
                "[{:02}:{:02}.{:02}]line {i}\n",
                i / 60,
                i % 60,
                i % 100
            ));
        }
        let lrc = parse(&text);
        for ms in (0..40_000).step_by(37) {
            let expected = lrc.lines().iter().rposition(|l| i64::from(l.at_ms) <= ms);
            assert_eq!(lrc.locate(ms), expected, "at {ms}");
        }
    }

    #[test]
    fn a_huge_file_is_capped_and_quick() {
        let mut text = String::new();
        while text.len() < 5_000_000 {
            text.push_str("[00:01.00][00:02.00][00:03.00]some words to sing\n");
        }
        let started = Instant::now();
        let lrc = parse(&text);
        assert_eq!(lrc.len(), MAX_LINES);
        assert!(started.elapsed().as_secs() < 2);
    }

    #[test]
    fn hostile_text_is_survived() {
        let started = Instant::now();
        // Absurd timestamps are dropped.
        assert!(parse("[99999:99.999]x\n[9999999999999999999:00]y\n[1440:01.00]z\n").is_empty());
        // No newline at all, and nothing that looks like a tag.
        assert!(parse(&"x".repeat(3_000_000)).is_empty());
        // One enormous line of stamps.
        assert_eq!(parse(&"[00:01.00]".repeat(200_000)).len(), MAX_LINES);
        // Brackets that never close, and angle brackets everywhere.
        assert!(parse(&"[".repeat(500_000)).is_empty());
        let angles = parse(&format!("[00:01.00]{}", "<".repeat(300_000)));
        assert!(angles.lines()[0].text.chars().count() <= MAX_LINE_CHARS);
        // Control characters and very long lyrics are cleaned and cut.
        let lrc = parse(&format!(
            "[00:01.00]a\u{0}b\u{7}\u{1b}[31mc\t\td {}\n",
            "w".repeat(10_000)
        ));
        let text = &lrc.lines()[0].text;
        assert!(text.starts_with("ab[31mc d w") && text.chars().count() == MAX_LINE_CHARS);
        // A multibyte character on the byte limit does not split.
        let wide = format!("[00:01.00]x\n{}", "\u{1ec7}".repeat(MAX_INPUT_BYTES));
        assert_eq!(parse(&wide).len(), 1);
        assert!(started.elapsed().as_secs() < 5);
    }
}
