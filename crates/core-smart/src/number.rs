//! Numbers the way people type them, in either of the two common styles (1,234.5 and 1.234,5).

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NumberStyle {
    pub group: char,
    pub decimal: char,
}

impl NumberStyle {
    pub const ENGLISH: Self = Self {
        group: ',',
        decimal: '.',
    };
    pub const VIETNAMESE: Self = Self {
        group: '.',
        decimal: ',',
    };
}

/// Separators that only ever group digits. The apostrophe is not one: "6'" is six feet.
fn is_space_group(c: char) -> bool {
    matches!(c, '\u{a0}' | '\u{202f}')
}

fn is_number_char(c: char) -> bool {
    c.is_ascii_digit() || matches!(c, '.' | ',') || is_space_group(c)
}

/// The number at the start of `text` and how many bytes it takes, sign included. `None` when the text does
/// not start with one, or when its separators make no sense ("1.2.3", "1,2.3,4").
///
/// With both a point and a comma the last one is the decimal separator. With one kind that appears more than
/// once it only groups. With a single separator followed by exactly three digits ("1,234") the style decides,
/// because that is what both "one thousand two hundred" and "one and a bit" look like; anything else with a
/// single separator ("1,5") is a decimal.
pub fn parse_prefix(text: &str, style: NumberStyle) -> Option<(f64, usize)> {
    let mut chars = text.char_indices().peekable();
    let mut negative = false;
    let mut start = 0;
    if let Some(&(_, c)) = chars.peek()
        && matches!(c, '-' | '+' | '\u{2212}')
    {
        negative = c != '+';
        start = c.len_utf8();
    }
    let mut end = start;
    for (i, c) in text[start..].char_indices() {
        if !is_number_char(c) {
            break;
        }
        end = start + i + c.len_utf8();
    }
    // A separator at the end belongs to the sentence ("about 5.", "10,").
    while end > start && matches!(text[start..end].chars().last(), Some('.' | ',')) {
        end -= 1;
    }
    let run: String = text[start..end]
        .chars()
        .filter(|c| !is_space_group(*c))
        .collect();
    if run.is_empty() || !run.starts_with(|c: char| c.is_ascii_digit()) {
        return None;
    }
    let value: f64 = clean(&run, style)?.parse().ok()?;
    value
        .is_finite()
        .then_some((if negative { -value } else { value }, end))
}

/// The digits of `run` with a point for the decimal separator, or `None` when the separators are wrong.
fn clean(run: &str, style: NumberStyle) -> Option<String> {
    let dots = run.matches('.').count();
    let commas = run.matches(',').count();
    let decimal = match (dots, commas) {
        (0, 0) => None,
        (_, 0) => single_kind(run, '.', dots, style)?,
        (0, _) => single_kind(run, ',', commas, style)?,
        _ => {
            let last = run.rfind(['.', ','])?;
            let decimal = run[last..].chars().next()?;
            // The decimal separator appears once and no other kind comes after it.
            if run.matches(decimal).count() != 1 {
                return None;
            }
            Some(decimal)
        }
    };
    let mut out = String::new();
    for (i, c) in run.char_indices() {
        match c {
            c if c.is_ascii_digit() => out.push(c),
            c if Some(c) == decimal && i == run.rfind(c)? => out.push('.'),
            '.' | ',' => {}
            _ => return None,
        }
    }
    // Groups are three digits wide: "1.2.3" and "12,34,567" are not numbers.
    if !groups_are_regular(run, decimal) {
        return None;
    }
    Some(out)
}

/// Decides for a run that has only one kind of separator: the decimal one, or `None` for "all grouping".
fn single_kind(run: &str, sep: char, count: usize, style: NumberStyle) -> Option<Option<char>> {
    if count > 1 {
        return Some(None);
    }
    let at = run.find(sep)?;
    let (before, after) = (&run[..at], &run[at + 1..]);
    let digits_after = after.chars().filter(char::is_ascii_digit).count();
    let looks_grouped =
        digits_after == 3 && (1..=3).contains(&before.len()) && !before.starts_with('0');
    Some(if looks_grouped && sep == style.group {
        None
    } else {
        Some(sep)
    })
}

fn groups_are_regular(run: &str, decimal: Option<char>) -> bool {
    let integer = match decimal {
        Some(d) => run.rsplit_once(d).map_or(run, |(integer, _)| integer),
        None => run,
    };
    let mut parts = integer.split(['.', ',']);
    let first = parts.next().unwrap_or("");
    if integer.contains(['.', ',']) && !(1..=3).contains(&first.len()) {
        return false;
    }
    parts.all(|p| p.len() == 3)
}

/// `value` with four significant digits, trailing zeros removed and thousands grouped: 3.1069 is "3.107",
/// 16404.2 is "16,404" and 0.0035 is "0.0035".
pub fn format(value: f64, style: NumberStyle) -> String {
    if !value.is_finite() {
        return String::from("-");
    }
    let abs = value.abs();
    if abs == 0.0 {
        return String::from("0");
    }
    let magnitude = abs.log10().floor() as i32;
    let decimals = (3 - magnitude).clamp(0, 8) as usize;
    let fixed = format_fixed(value, decimals, style);
    if decimals == 0 || !fixed.contains(style.decimal) {
        return fixed;
    }
    fixed
        .trim_end_matches('0')
        .trim_end_matches(style.decimal)
        .to_owned()
}

/// `value` with exactly `decimals` decimals and thousands grouped, for money.
pub fn format_fixed(value: f64, decimals: usize, style: NumberStyle) -> String {
    // Rounded half away from zero, as people do, not half to even as the formatter does.
    let scale = 10f64.powi(decimals as i32);
    let text = format!("{:.*}", decimals, (value.abs() * scale).round() / scale);
    let (integer, fraction) = match text.split_once('.') {
        Some((i, f)) => (i, Some(f)),
        None => (text.as_str(), None),
    };
    let mut out = String::new();
    let first = integer.len() % 3;
    for (i, c) in integer.chars().enumerate() {
        if i > 0 && (i + 3 - first) % 3 == 0 {
            out.push(style.group);
        }
        out.push(c);
    }
    if let Some(fraction) = fraction {
        out.push(style.decimal);
        out.push_str(fraction);
    }
    // "-0" is not a thing people write.
    let zero = out.chars().all(|c| !c.is_ascii_digit() || c == '0');
    if value < 0.0 && !zero {
        out.insert(0, '-');
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    const EN: NumberStyle = NumberStyle::ENGLISH;
    const VI: NumberStyle = NumberStyle::VIETNAMESE;

    fn parse(text: &str, style: NumberStyle) -> Option<f64> {
        parse_prefix(text, style).map(|(v, _)| v)
    }

    #[test]
    fn plain_numbers() {
        assert_eq!(parse("5", EN), Some(5.0));
        assert_eq!(parse("5.5 km", EN), Some(5.5));
        assert_eq!(parse("-3", EN), Some(-3.0));
        assert_eq!(parse("\u{2212}3.5", EN), Some(-3.5));
        assert_eq!(parse("+7", EN), Some(7.0));
        assert_eq!(parse("0.123", EN), Some(0.123));
        assert_eq!(parse("abc", EN), None);
        assert_eq!(parse("", EN), None);
        assert_eq!(parse("-", EN), None);
        assert_eq!(parse(".5", EN), None, "a number starts with a digit");
    }

    #[test]
    fn how_many_bytes_the_number_took() {
        assert_eq!(parse_prefix("12.5km", EN), Some((12.5, 4)));
        assert_eq!(parse_prefix("-1,000 g", EN), Some((-1000.0, 6)));
        assert_eq!(
            parse_prefix("10. next", EN),
            Some((10.0, 2)),
            "a full stop ends the sentence"
        );
        assert_eq!(parse_prefix("5,", EN), Some((5.0, 1)));
    }

    #[test]
    fn both_separators_make_the_last_one_decimal() {
        assert_eq!(parse("1,234.56", EN), Some(1234.56));
        assert_eq!(parse("1.234,56", VI), Some(1234.56));
        assert_eq!(parse("1.234,56", EN), Some(1234.56), "whatever the style");
        assert_eq!(parse("1,234,567.8", EN), Some(1_234_567.8));
        assert_eq!(parse("1.2,3.4", EN), None);
    }

    #[test]
    fn one_kind_repeated_only_groups() {
        assert_eq!(parse("1.234.567", VI), Some(1_234_567.0));
        assert_eq!(parse("1,234,567", EN), Some(1_234_567.0));
        assert_eq!(parse("1.2.3", EN), None);
        assert_eq!(parse("12,34,567", EN), None, "groups are three wide");
    }

    #[test]
    fn a_single_separator_with_three_digits_follows_the_style() {
        assert_eq!(parse("1,234", EN), Some(1234.0));
        assert_eq!(parse("1.234", EN), Some(1.234));
        assert_eq!(parse("1.234", VI), Some(1234.0));
        assert_eq!(parse("1,234", VI), Some(1.234));
        assert_eq!(parse("100.000", VI), Some(100_000.0));
    }

    #[test]
    fn a_single_separator_with_other_digit_counts_is_decimal() {
        assert_eq!(parse("1,5", EN), Some(1.5));
        assert_eq!(parse("1,5", VI), Some(1.5));
        assert_eq!(parse("7.12345", VI), Some(7.12345));
        assert_eq!(
            parse("0,123", EN),
            Some(0.123),
            "a leading zero is not a group"
        );
        assert_eq!(parse("1234,5", EN), Some(1234.5));
    }

    #[test]
    fn spaces_that_group() {
        assert_eq!(parse("1\u{a0}000\u{a0}000", EN), Some(1_000_000.0));
        assert_eq!(
            parse_prefix("6'", EN),
            Some((6.0, 1)),
            "an apostrophe is not a separator"
        );
    }

    #[test]
    fn absurd_lengths_do_not_produce_infinity() {
        assert_eq!(parse(&"9".repeat(400), EN), None);
    }

    #[test]
    fn four_significant_digits() {
        assert_eq!(format(3.10685596, EN), "3.107");
        assert_eq!(format(16404.2, EN), "16,404");
        assert_eq!(format(11.811, EN), "11.81");
        assert_eq!(format(0.62137, EN), "0.6214");
        assert_eq!(format(0.0035, EN), "0.0035");
        assert_eq!(format(100.0, EN), "100");
        assert_eq!(format(5.0, EN), "5");
        assert_eq!(format(1234.5, EN), "1,235");
        assert_eq!(format(0.0, EN), "0");
        assert_eq!(format(-22.2222, EN), "-22.22");
        assert_eq!(format(9.99996, EN), "10");
        assert_eq!(format(f64::NAN, EN), "-");
        assert_eq!(format(3.10685596, VI), "3,107");
        assert_eq!(format(16404.2, VI), "16.404");
        assert_eq!(format(0.00003862, EN), "0.00003862");
    }

    #[test]
    fn fixed_decimals_group_the_thousands() {
        assert_eq!(format_fixed(1234567.891, 2, EN), "1,234,567.89");
        assert_eq!(format_fixed(25890.2015, 0, VI), "25.890");
        assert_eq!(format_fixed(999.0, 2, EN), "999.00");
        assert_eq!(format_fixed(1000.0, 2, VI), "1.000,00");
        assert_eq!(format_fixed(-0.001, 2, EN), "0.00", "no negative zero");
        assert_eq!(format_fixed(-5.5, 1, EN), "-5.5");
        assert_eq!(format_fixed(0.5, 0, EN), "1", "halves round away from zero");
        assert_eq!(format_fixed(2.5, 0, EN), "3");
        assert_eq!(format_fixed(0.125, 2, EN), "0.13");
    }

    #[test]
    fn what_is_written_can_be_read_back() {
        for style in [EN, VI] {
            for value in [0.5, 12.0, 999.0, 1234.0, 1_234_567.0, 12.34] {
                let text = format_fixed(value, 2, style);
                assert_eq!(parse(&text, style), Some(value), "{text}");
            }
        }
    }
}
