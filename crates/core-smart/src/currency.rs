//! Amounts of money: reading "$100", "50 EUR" or "5tr đ", and converting them with a table of rates.

use crate::number::{self, NumberStyle};
use serde_json::Value;
use std::collections::HashMap;

/// The provider's free open endpoint: rates against the dollar, updated once a day, no key. Nothing about the
/// person or their text is sent.
pub const SERVICE: &str = "https://open.er-api.com/v6/latest/USD";

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Money {
    pub amount: f64,
    pub code: &'static str,
}

/// The codes that are recognised in a text. The rates table knows more, and any of those can be a target.
#[rustfmt::skip]
const CODES: &[&str] = &[
    "USD", "EUR", "GBP", "JPY", "CNY", "VND", "KRW", "INR", "THB", "SGD", "AUD", "CAD", "CHF", "HKD",
    "NZD", "SEK", "NOK", "DKK", "PLN", "CZK", "HUF", "RUB", "TRY", "BRL", "MXN", "ARS", "CLP", "COP",
    "PEN", "ZAR", "EGP", "AED", "SAR", "ILS", "IDR", "MYR", "PHP", "TWD", "PKR", "BDT", "LKR", "NGN",
    "KES", "UAH", "RON", "BGN", "KZT", "MMK", "KHR", "LAK",
];

/// Written in front of or after an amount. The longer ones are tried first.
#[rustfmt::skip]
const SYMBOLS: &[(&str, &str)] = &[
    ("US$", "USD"), ("AU$", "AUD"), ("CA$", "CAD"), ("NZ$", "NZD"), ("HK$", "HKD"), ("A$", "AUD"),
    ("C$", "CAD"), ("S$", "SGD"), ("R$", "BRL"), ("$", "USD"), ("\u{20ac}", "EUR"), ("\u{a3}", "GBP"),
    ("\u{a5}", "JPY"), ("\u{ffe5}", "JPY"), ("\u{20ab}", "VND"), ("\u{20a9}", "KRW"), ("\u{20b9}", "INR"),
    ("\u{e3f}", "THB"), ("\u{20bd}", "RUB"), ("\u{20b1}", "PHP"), ("\u{20ba}", "TRY"), ("\u{20aa}", "ILS"),
    ("\u{20b4}", "UAH"), ("z\u{142}", "PLN"),
];

/// Words that name a currency after an amount, lowercase. Vietnamese writes "100.000đ" and "5tr đồng".
#[rustfmt::skip]
const WORDS: &[(&str, &str)] = &[
    ("\u{111}", "VND"), ("\u{111}\u{1ed3}ng", "VND"), ("dong", "VND"), ("vn\u{111}", "VND"),
    ("dollar", "USD"), ("dollars", "USD"), ("euro", "EUR"), ("euros", "EUR"), ("yen", "JPY"),
    ("yuan", "CNY"), ("baht", "THB"), ("won", "KRW"),
];

/// Multipliers written after an amount: 5k, 2M, 5tr (triệu, a million), 2 tỷ (a billion).
#[rustfmt::skip]
const MAGNITUDES: &[(&str, f64)] = &[
    ("tri\u{1ec7}u", 1e6), ("ngh\u{ec}n", 1e3), ("ng\u{e0}n", 1e3), ("t\u{1ef7}", 1e9), ("t\u{1ec9}", 1e9),
    ("tr", 1e6), ("mn", 1e6), ("bn", 1e9), ("k", 1e3), ("m", 1e6), ("b", 1e9),
];

const ZERO_DECIMALS: &[&str] = &[
    "VND", "JPY", "KRW", "IDR", "CLP", "PYG", "ISK", "UGX", "XAF", "XOF",
];

/// How many decimals an amount of this currency is written with.
pub fn decimals(code: &str) -> usize {
    if ZERO_DECIMALS.contains(&code) { 0 } else { 2 }
}

/// Three capital letters: the shape of a currency code.
pub fn is_code(text: &str) -> bool {
    text.len() == 3 && text.bytes().all(|b| b.is_ascii_uppercase())
}

fn known_code(text: &str) -> Option<&'static str> {
    let upper = text.to_uppercase();
    CODES.iter().find(|c| **c == upper).copied()
}

/// The symbol or code the text starts with, and what follows it.
fn leading_currency(text: &str) -> Option<(&'static str, &str)> {
    let symbol = SYMBOLS
        .iter()
        .filter(|(s, _)| text.starts_with(s))
        .max_by_key(|(s, _)| s.len());
    if let Some((s, code)) = symbol {
        return Some((code, &text[s.len()..]));
    }
    let head = text.get(..3)?;
    let code = known_code(head).filter(|_| head.bytes().all(|b| b.is_ascii_alphabetic()))?;
    let rest = &text[3..];
    // "USDX" is a word, not a code.
    (!rest.starts_with(|c: char| c.is_alphabetic())).then_some((code, rest))
}

/// A multiplier at the start of `text`, and what follows it. A word like "mi" is not an "m".
fn magnitude(text: &str) -> (f64, &str) {
    let rest = text.trim_start();
    let lower = rest.to_lowercase();
    for (word, factor) in MAGNITUDES {
        if lower.starts_with(word) {
            let after = &rest[word.len()..];
            if !after.starts_with(|c: char| c.is_alphabetic()) {
                return (*factor, after);
            }
        }
    }
    (1.0, text)
}

/// The currency a text that follows an amount names, if it is nothing else.
fn trailing_currency(text: &str) -> Option<&'static str> {
    let name = text.trim().trim_end_matches('.').trim();
    if name.is_empty() {
        return None;
    }
    let lower = name.to_lowercase();
    if let Some((_, code)) = WORDS.iter().find(|(w, _)| *w == lower) {
        return Some(code);
    }
    if let Some((_, code)) = SYMBOLS.iter().find(|(s, _)| *s == name) {
        return Some(code);
    }
    known_code(name)
}

/// An amount of money in a text that is nothing else, or `None`. A bare number, or a multiplier with no
/// currency ("5k"), is not money.
pub fn parse(text: &str, style: NumberStyle) -> Option<Money> {
    let text = text.trim();
    if let Some((code, rest)) = leading_currency(text) {
        let rest = rest.trim_start();
        let (amount, used) = number::parse_prefix(rest, style)?;
        let (factor, after) = magnitude(&rest[used..]);
        let tail = after.trim().trim_end_matches('.').trim();
        return tail.is_empty().then_some(Money {
            amount: amount * factor,
            code,
        });
    }
    let (amount, used) = number::parse_prefix(text, style)?;
    let (factor, after) = magnitude(&text[used..]);
    let code = trailing_currency(after)?;
    Some(Money {
        amount: amount * factor,
        code,
    })
}

/// Exchange rates against one base currency, from the free open endpoint of exchangerate-api.com.
#[derive(Debug, Clone, PartialEq)]
pub struct Rates {
    table: HashMap<String, f64>,
    /// Unix seconds of the provider's last update and of the next one it announces.
    pub updated: i64,
    pub next_update: i64,
}

impl Rates {
    /// The provider's JSON, or `None` for anything that is not a plausible answer.
    pub fn parse(body: &str) -> Option<Self> {
        let value: Value = serde_json::from_str(body).ok()?;
        if value.get("result")?.as_str()? != "success" {
            return None;
        }
        let mut table = HashMap::new();
        for (code, rate) in value.get("rates")?.as_object()? {
            let rate = rate.as_f64()?;
            if is_code(code) && rate.is_finite() && rate > 0.0 {
                table.insert(code.clone(), rate);
            }
        }
        // A real table has well over a hundred currencies; a handful means something is wrong.
        if table.len() < 20 {
            return None;
        }
        let updated = value
            .get("time_last_update_unix")
            .and_then(Value::as_i64)
            .unwrap_or(0);
        let next_update = value
            .get("time_next_update_unix")
            .and_then(Value::as_i64)
            .filter(|n| *n > updated)
            .unwrap_or(updated + 86_400);
        Some(Self {
            table,
            updated,
            next_update,
        })
    }

    pub fn knows(&self, code: &str) -> bool {
        self.table.contains_key(code)
    }

    /// What one `from` is worth in `to`.
    pub fn rate(&self, from: &str, to: &str) -> Option<f64> {
        Some(self.table.get(to)? / self.table.get(from)?)
    }

    pub fn is_due(&self, now: i64) -> bool {
        now >= self.next_update
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Priced {
    pub code: String,
    pub value: f64,
    /// What one unit of the source currency is worth in `code`.
    pub rate: f64,
}

/// `money` in each of the `targets` the rates know, leaving out its own currency.
pub fn convert(money: Money, targets: &[String], rates: &Rates) -> Vec<Priced> {
    targets
        .iter()
        .filter(|code| code.as_str() != money.code)
        .filter_map(|code| {
            let rate = rates.rate(money.code, code)?;
            Some(Priced {
                code: code.clone(),
                value: money.amount * rate,
                rate,
            })
        })
        .collect()
}

#[cfg(test)]
pub(crate) fn sample_rates() -> Rates {
    // Twenty-odd currencies against USD, enough to look like a real table.
    let mut table = HashMap::new();
    for (code, rate) in [
        ("USD", 1.0),
        ("EUR", 0.894_425),
        ("VND", 25_890.2),
        ("JPY", 158.3),
        ("GBP", 0.756_165),
        ("KRW", 1339.8),
        ("CNY", 6.7),
        ("THB", 33.6),
    ] {
        table.insert(code.to_owned(), rate);
    }
    Rates {
        table,
        updated: 1_792_022_551,
        next_update: 1_792_109_261,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const EN: NumberStyle = NumberStyle::ENGLISH;
    const VI: NumberStyle = NumberStyle::VIETNAMESE;

    fn money(text: &str, style: NumberStyle) -> Option<(f64, &'static str)> {
        parse(text, style).map(|m| (m.amount, m.code))
    }

    #[test]
    fn a_symbol_in_front() {
        assert_eq!(money("$100", EN), Some((100.0, "USD")));
        assert_eq!(money("$1,234.50", EN), Some((1234.5, "USD")));
        assert_eq!(money("\u{20ac} 50", EN), Some((50.0, "EUR")));
        assert_eq!(money("\u{a3}9.99", EN), Some((9.99, "GBP")));
        assert_eq!(money("\u{a5}5000", EN), Some((5000.0, "JPY")));
        assert_eq!(money("US$ 5", EN), Some((5.0, "USD")));
        assert_eq!(money("A$12", EN), Some((12.0, "AUD")));
        assert_eq!(money("HK$100", EN), Some((100.0, "HKD")));
    }

    #[test]
    fn a_code_in_front_or_behind() {
        assert_eq!(money("USD 100", EN), Some((100.0, "USD")));
        assert_eq!(money("EUR100", EN), Some((100.0, "EUR")));
        assert_eq!(money("50 EUR", EN), Some((50.0, "EUR")));
        assert_eq!(money("50eur", EN), Some((50.0, "EUR")));
        assert_eq!(money("1,000 jpy", EN), Some((1000.0, "JPY")));
        assert_eq!(money("100 USD.", EN), Some((100.0, "USD")));
    }

    #[test]
    fn a_symbol_or_word_behind() {
        assert_eq!(money("100$", EN), Some((100.0, "USD")));
        assert_eq!(money("50\u{20ac}", EN), Some((50.0, "EUR")));
        assert_eq!(money("100.000\u{20ab}", VI), Some((100_000.0, "VND")));
        assert_eq!(money("100.000\u{111}", VI), Some((100_000.0, "VND")));
        assert_eq!(money("100.000 VN\u{110}", VI), Some((100_000.0, "VND")));
        assert_eq!(money("2 dollars", EN), Some((2.0, "USD")));
        assert_eq!(money("1.500 \u{111}\u{1ed3}ng", VI), Some((1500.0, "VND")));
    }

    #[test]
    fn multipliers_need_a_currency() {
        assert_eq!(money("5k USD", EN), Some((5000.0, "USD")));
        assert_eq!(money("$2M", EN), Some((2_000_000.0, "USD")));
        assert_eq!(money("$1.5k", EN), Some((1500.0, "USD")));
        assert_eq!(money("5tr \u{111}", VI), Some((5_000_000.0, "VND")));
        assert_eq!(
            money("2,5 t\u{1ef7} VND", VI),
            Some((2_500_000_000.0, "VND"))
        );
        assert_eq!(
            money("300 ngh\u{ec}n \u{111}", VI),
            Some((300_000.0, "VND"))
        );
        assert_eq!(money("5k", EN), None);
        assert_eq!(money("5 m", EN), None, "metres are not millions");
        assert_eq!(money("5 mi", EN), None);
        assert_eq!(money("5 MB", EN), None);
    }

    #[test]
    fn what_is_not_money_is_left_alone() {
        for text in [
            "100",
            "USD",
            "$",
            "hello",
            "USDX 5",
            "5 km",
            "$5 and more",
            "5 euros per day",
            "",
        ] {
            assert_eq!(money(text, EN), None, "{text}");
        }
    }

    #[test]
    fn the_real_provider_answer_is_read() {
        let mut rates = String::from(
            r#"{"result":"success","provider":"https://www.exchangerate-api.com","time_last_update_unix":1792022551,"time_next_update_unix":1792109261,"base_code":"USD","rates":{"USD":1,"EUR":0.894425,"VND":25890.201508"#,
        );
        for i in 0..30 {
            rates.push_str(&format!(
                r#","A{}{}":{}"#,
                (b'A' + i / 26) as char,
                (b'A' + i % 26) as char,
                i + 1
            ));
        }
        rates.push_str("}}");
        let rates = Rates::parse(&rates).unwrap();
        assert_eq!(rates.updated, 1_792_022_551);
        assert_eq!(rates.next_update, 1_792_109_261);
        assert!(rates.knows("VND") && !rates.knows("XXX"));
        assert!((rates.rate("USD", "VND").unwrap() - 25_890.201_508).abs() < 1e-6);
        assert!(!rates.is_due(1_792_100_000) && rates.is_due(1_792_109_261));
    }

    #[test]
    fn a_bad_answer_is_refused() {
        for body in [
            "",
            "not json",
            "{}",
            r#"{"result":"error","error-type":"quota-reached"}"#,
            r#"{"result":"success","rates":{"USD":1,"EUR":0.9}}"#,
            r#"{"result":"success","rates":"nope"}"#,
        ] {
            assert!(Rates::parse(body).is_none(), "{body}");
        }
    }

    #[test]
    fn nonsense_rates_are_refused() {
        let mut body = String::from(r#"{"result":"success","rates":{"USD":1"#);
        for i in 0..30 {
            body.push_str(&format!(
                r#","Q{}{}":{}"#,
                (b'A' + i / 26) as char,
                (b'A' + i % 26) as char,
                if i == 5 { "-3" } else { "2" }
            ));
        }
        body.push_str("}}");
        let rates = Rates::parse(&body).unwrap();
        assert!(
            !rates.knows("QAF"),
            "a negative rate is dropped, the rest is kept"
        );
        assert!(rates.knows("QAG"));
    }

    #[test]
    fn money_is_converted_to_the_targets_but_not_to_itself() {
        let rates = sample_rates();
        let targets: Vec<String> = ["USD", "EUR", "VND", "XXX"].map(String::from).to_vec();
        let priced = convert(
            Money {
                amount: 100.0,
                code: "USD",
            },
            &targets,
            &rates,
        );
        let codes: Vec<&str> = priced.iter().map(|p| p.code.as_str()).collect();
        assert_eq!(
            codes,
            ["EUR", "VND"],
            "not USD to USD, and a currency nobody knows is skipped"
        );
        assert!((priced[0].value - 89.4425).abs() < 1e-9);
        assert!((priced[1].value - 2_589_020.0).abs() < 1e-6);
        // Across two currencies that are neither the base.
        let eur = convert(
            Money {
                amount: 10.0,
                code: "EUR",
            },
            &["VND".into()],
            &rates,
        );
        assert!((eur[0].value - 10.0 * 25_890.2 / 0.894_425).abs() < 1e-6);
        assert!((eur[0].rate * 0.894_425 - 25_890.2).abs() < 1e-6);
    }

    #[test]
    fn zero_decimals_currencies() {
        assert_eq!(decimals("VND"), 0);
        assert_eq!(decimals("JPY"), 0);
        assert_eq!(decimals("USD"), 2);
        assert!(is_code("USD") && !is_code("usd") && !is_code("US") && !is_code("USDD"));
    }
}
