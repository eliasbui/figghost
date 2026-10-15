#![forbid(unsafe_code)]
//! What a piece of selected text means: a unit, an amount of money, a time, or plain words.

pub mod currency;
pub mod net;
pub mod number;
pub mod translate;
pub mod units;
pub mod zone;

use currency::Rates;
use number::NumberStyle;

/// Longer than this is not a quantity, an amount or a time, so nobody looks.
const MAX_CHARS: usize = 80;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Unit(units::Dim),
    Money,
    Time,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Answer {
    pub kind: Kind,
    /// What is read out and what is copied: "3.107 mi", "89.44 EUR", "6:00 AM".
    pub text: String,
    /// Money: the rate. Time: the zone, "local" for the reader's own.
    pub note: String,
    /// Time only: days after (positive) or before the day in the text.
    pub day: i32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Analysis {
    /// A quantity, an amount or a time. `input` is how it was read.
    Found { input: String, answers: Vec<Answer> },
    /// An amount of money, with no rates to convert it by.
    NoRates { input: String },
    /// Nothing that is understood: plain words.
    Text,
}

pub struct Context<'a> {
    pub style: NumberStyle,
    pub rates: Option<&'a Rates>,
    /// Currency codes to show an amount in.
    pub currencies: &'a [String],
    /// Zones to show a time in: IANA identifiers, "UTC" or "local".
    pub zones: &'a [String],
    /// Unix seconds.
    pub now: i64,
    /// Minutes east of UTC of a zone at a Unix time; "local" is the reader's own zone.
    pub offset: &'a dyn Fn(&str, i64) -> Option<i32>,
}

/// The calendar date (year, month, day) of a Unix time in UTC, with no time zone database: the civil from days
/// algorithm. Used to say which day the exchange rates are from.
pub fn civil_date(unix: i64) -> (i32, u32, u32) {
    let days = unix.div_euclid(86_400) + 719_468;
    let era = days.div_euclid(146_097);
    let day_of_era = days.rem_euclid(146_097);
    let year_of_era =
        (day_of_era - day_of_era / 1460 + day_of_era / 36_524 - day_of_era / 146_096) / 365;
    let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let shifted_month = (5 * day_of_year + 2) / 153;
    let day = (day_of_year - (153 * shifted_month + 2) / 5 + 1) as u32;
    let month = if shifted_month < 10 {
        shifted_month + 3
    } else {
        shifted_month - 9
    } as u32;
    let year = (year_of_era + era * 400 + i64::from(month <= 2)) as i32;
    (year, month, day)
}

fn answer(kind: Kind, text: String, note: String) -> Answer {
    Answer {
        kind,
        text,
        note,
        day: 0,
    }
}

pub fn analyze(text: &str, ctx: &Context) -> Analysis {
    let text = text.trim();
    if text.is_empty() || text.chars().count() > MAX_CHARS || text.contains('\n') {
        return Analysis::Text;
    }
    let style = ctx.style;
    if let Some(converted) = units::convert(text, style) {
        let quantity =
            |q: &units::Quantity| format!("{} {}", number::format(q.value, style), q.symbol);
        return Analysis::Found {
            input: quantity(&converted.from),
            answers: converted
                .to
                .iter()
                .map(|q| answer(Kind::Unit(converted.dim), quantity(q), String::new()))
                .collect(),
        };
    }
    if let Some(money) = currency::parse(text, style) {
        let decimals = currency::decimals(money.code);
        let input = format!(
            "{} {}",
            number::format_fixed(money.amount, decimals, style),
            money.code
        );
        let Some(rates) = ctx.rates else {
            return Analysis::NoRates { input };
        };
        let answers = currency::convert(money, ctx.currencies, rates)
            .into_iter()
            .map(|p| {
                answer(
                    Kind::Money,
                    format!(
                        "{} {}",
                        number::format_fixed(p.value, currency::decimals(&p.code), style),
                        p.code
                    ),
                    format!(
                        "1 {} = {} {}",
                        money.code,
                        number::format(p.rate, style),
                        p.code
                    ),
                )
            })
            .collect();
        return Analysis::Found { input, answers };
    }
    if let Some(query) = zone::parse(text) {
        let clock = zone::format_time(query.hour, query.minute, query.twelve_hour);
        let input = match &query.source {
            zone::Source::Local => clock,
            zone::Source::Fixed { label, .. } => format!("{clock} {label}"),
            zone::Source::Named(id) => format!("{clock} {}", zone::label(id)),
        };
        let answers = zone::convert(&query, ctx.zones, ctx.now, ctx.offset)
            .into_iter()
            .map(|row| Answer {
                kind: Kind::Time,
                text: zone::format_time(row.hour, row.minute, query.twelve_hour),
                note: zone::label(&row.zone),
                day: row.day,
            })
            .collect();
        return Analysis::Found { input, answers };
    }
    Analysis::Text
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 2026-10-15 03:00:00 UTC.
    const NOW: i64 = 1_792_022_400 + 3 * 3600;

    fn offset(zone: &str, _: i64) -> Option<i32> {
        match zone {
            "local" => Some(420),
            "UTC" => Some(0),
            "Asia/Tokyo" => Some(540),
            _ => None,
        }
    }

    fn list(items: &[&str]) -> Vec<String> {
        items.iter().map(|s| (*s).to_owned()).collect()
    }

    fn run(text: &str, style: NumberStyle, rates: Option<&Rates>) -> Analysis {
        let currencies = list(&["USD", "EUR", "VND"]);
        let zones = list(&["local", "UTC", "Asia/Tokyo"]);
        analyze(
            text,
            &Context {
                style,
                rates,
                currencies: &currencies,
                zones: &zones,
                now: NOW,
                offset: &offset,
            },
        )
    }

    fn found(analysis: Analysis) -> (String, Vec<(String, String, i32)>) {
        match analysis {
            Analysis::Found { input, answers } => (
                input,
                answers
                    .into_iter()
                    .map(|a| (a.text, a.note, a.day))
                    .collect(),
            ),
            other => panic!("expected an answer, got {other:?}"),
        }
    }

    #[test]
    fn dates_come_out_of_unix_time() {
        assert_eq!(civil_date(0), (1970, 1, 1));
        assert_eq!(civil_date(1_792_022_551), (2026, 10, 15));
        assert_eq!(civil_date(951_782_400), (2000, 2, 29), "a leap day");
        assert_eq!(civil_date(1_709_164_799), (2024, 2, 28));
        assert_eq!(civil_date(1_709_164_800), (2024, 2, 29));
        assert_eq!(civil_date(-86_400), (1969, 12, 31));
        assert_eq!(civil_date(4_102_444_800), (2100, 1, 1));
    }

    #[test]
    fn a_unit() {
        let (input, answers) = found(run("5 km", NumberStyle::ENGLISH, None));
        assert_eq!(input, "5 km");
        assert_eq!(answers[0].0, "3.107 mi");
        assert_eq!(answers[1].0, "16,404 ft");
    }

    #[test]
    fn units_are_written_in_the_readers_style() {
        let (input, answers) = found(run("1,5 km", NumberStyle::VIETNAMESE, None));
        assert_eq!(input, "1,5 km");
        assert_eq!(answers[0].0, "4.921 ft");
        assert_eq!(answers[1].0, "0,9321 mi");
    }

    #[test]
    fn money_with_rates() {
        let rates = currency::sample_rates();
        let (input, answers) = found(run("$100", NumberStyle::ENGLISH, Some(&rates)));
        assert_eq!(input, "100.00 USD");
        assert_eq!(
            answers[0],
            ("89.44 EUR".into(), "1 USD = 0.8944 EUR".into(), 0)
        );
        assert_eq!(answers[1].0, "2,589,020 VND");
        assert_eq!(answers.len(), 2, "USD is the source");
        let (_, vi) = found(run("100 USD", NumberStyle::VIETNAMESE, Some(&rates)));
        assert_eq!(vi[1].0, "2.589.020 VND");
        assert_eq!(vi[0].1, "1 USD = 0,8944 EUR");
    }

    #[test]
    fn dong_has_no_decimals_in_the_input_either() {
        let rates = currency::sample_rates();
        let (input, answers) = found(run("5tr \u{111}", NumberStyle::VIETNAMESE, Some(&rates)));
        assert_eq!(input, "5.000.000 VND");
        assert_eq!(answers[0].0, "193,12 USD");
    }

    #[test]
    fn money_without_rates_says_so() {
        assert_eq!(
            run("$100", NumberStyle::ENGLISH, None),
            Analysis::NoRates {
                input: "100.00 USD".into()
            }
        );
    }

    #[test]
    fn a_time_in_other_zones() {
        let (input, answers) = found(run("3pm PST", NumberStyle::ENGLISH, None));
        assert_eq!(input, "3:00 PM PST");
        assert_eq!(
            answers,
            vec![
                ("6:00 AM".into(), "local".into(), 1),
                ("11:00 PM".into(), "UTC".into(), 0),
                ("8:00 AM".into(), "Tokyo".into(), 1),
            ]
        );
        let (input, answers) = found(run("9:30", NumberStyle::ENGLISH, None));
        assert_eq!(input, "09:30");
        assert_eq!(answers[0], ("02:30".into(), "UTC".into(), 0));
    }

    #[test]
    fn plain_words_and_anything_unreasonable_are_text() {
        let rates = currency::sample_rates();
        for text in [
            "hello world",
            "",
            "   ",
            "5 km\nand more",
            "5 bananas",
            "100",
            &"word ".repeat(40),
            &format!("{} km", "9".repeat(90)),
        ] {
            assert_eq!(
                run(text, NumberStyle::ENGLISH, Some(&rates)),
                Analysis::Text,
                "{text:?}"
            );
        }
    }

    #[test]
    fn a_unit_wins_over_a_currency_where_both_could_be_meant() {
        // Nothing here names a currency, so a quantity it is.
        assert!(matches!(
            run("5 m", NumberStyle::ENGLISH, None),
            Analysis::Found { .. }
        ));
        assert!(matches!(
            run("5 mi", NumberStyle::ENGLISH, None),
            Analysis::Found { .. }
        ));
    }
}
