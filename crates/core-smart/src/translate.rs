//! Translation through the public MyMemory service. This is the only part of Smart Select that sends the
//! selected text anywhere, so it is only used when the user has turned the internet on for it.

use serde_json::Value;

pub const SERVICE: &str = "https://api.mymemory.translated.net/get";

/// What the service accepts in one request.
pub const MAX_CHARS: usize = 450;

/// The longest translation shown.
const MAX_SHOWN: usize = 2000;

pub struct Language {
    pub code: &'static str,
    /// Written in the language itself, which is how people look for it.
    pub name: &'static str,
}

#[rustfmt::skip]
pub const LANGUAGES: &[Language] = &[
    Language { code: "en", name: "English" },
    Language { code: "vi", name: "Ti\u{1ebf}ng Vi\u{1ec7}t" },
    Language { code: "ja", name: "\u{65e5}\u{672c}\u{8a9e}" },
    Language { code: "ko", name: "\u{d55c}\u{ad6d}\u{c5b4}" },
    Language { code: "zh-CN", name: "\u{7b80}\u{4f53}\u{4e2d}\u{6587}" },
    Language { code: "zh-TW", name: "\u{7e41}\u{9ad4}\u{4e2d}\u{6587}" },
    Language { code: "fr", name: "Fran\u{e7}ais" },
    Language { code: "de", name: "Deutsch" },
    Language { code: "es", name: "Espa\u{f1}ol" },
    Language { code: "pt", name: "Portugu\u{ea}s" },
    Language { code: "it", name: "Italiano" },
    Language { code: "ru", name: "\u{420}\u{443}\u{441}\u{441}\u{43a}\u{438}\u{439}" },
    Language { code: "th", name: "\u{e44}\u{e17}\u{e22}" },
    Language { code: "id", name: "Bahasa Indonesia" },
];

pub fn is_supported(code: &str) -> bool {
    LANGUAGES.iter().any(|l| l.code == code)
}

/// The query parameters of a request, or `None` when the text is empty or too long or the language is
/// unknown. The source language is detected by the service.
pub fn query(text: &str, target: &str) -> Option<Vec<(&'static str, String)>> {
    let text = text.trim();
    if text.is_empty() || text.chars().count() > MAX_CHARS || !is_supported(target) {
        return None;
    }
    Some(vec![
        ("q", text.to_owned()),
        ("langpair", format!("Autodetect|{target}")),
    ])
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Failure {
    /// The free allowance for today is used up.
    Quota,
    /// The text is already in the target language.
    SameLanguage,
    /// The service said no, with this status.
    Service(i64),
    /// Something came back that is not an answer.
    Unreadable,
}

/// Character references that the service leaves in its answers: `&#39;` and the five named ones.
fn decode_entities(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(at) = rest.find('&') {
        out.push_str(&rest[..at]);
        rest = &rest[at..];
        let decoded = rest.find(';').filter(|end| *end <= 8).and_then(|end| {
            let name = &rest[1..end];
            let c = match name {
                "amp" => '&',
                "lt" => '<',
                "gt" => '>',
                "quot" => '"',
                "apos" => '\'',
                _ => {
                    let number = name.strip_prefix('#')?;
                    let code = match number.strip_prefix(['x', 'X']) {
                        Some(hex) => u32::from_str_radix(hex, 16).ok()?,
                        None => number.parse().ok()?,
                    };
                    char::from_u32(code)?
                }
            };
            Some((c, end + 1))
        });
        match decoded {
            Some((c, used)) => {
                out.push(c);
                rest = &rest[used..];
            }
            None => {
                out.push('&');
                rest = &rest[1..];
            }
        }
    }
    out.push_str(rest);
    out
}

/// What is shown of a translation: entities decoded, control characters gone, and not unreasonably long.
fn clean(text: &str) -> String {
    let decoded = decode_entities(text);
    let printable: String = decoded
        .chars()
        .filter(|c| *c == '\n' || !c.is_control())
        .collect();
    printable.trim().chars().take(MAX_SHOWN).collect()
}

pub fn parse_response(body: &str) -> Result<String, Failure> {
    let value: Value = serde_json::from_str(body).map_err(|_| Failure::Unreadable)?;
    let translated = value
        .pointer("/responseData/translatedText")
        .and_then(Value::as_str)
        .unwrap_or("");
    let status = match value.get("responseStatus") {
        Some(Value::Number(n)) => n.as_i64(),
        Some(Value::String(s)) => s.trim().parse().ok(),
        _ => None,
    };
    let said = |needle: &str| {
        let details = value
            .get("responseDetails")
            .and_then(Value::as_str)
            .unwrap_or("");
        translated.to_uppercase().contains(needle) || details.to_uppercase().contains(needle)
    };
    if value.get("quotaFinished").and_then(Value::as_bool) == Some(true)
        || translated.starts_with("MYMEMORY WARNING")
    {
        return Err(Failure::Quota);
    }
    match status {
        Some(200) => {}
        Some(403) if said("DISTINCT LANGUAGES") => return Err(Failure::SameLanguage),
        Some(other) => return Err(Failure::Service(other)),
        None => return Err(Failure::Unreadable),
    }
    let text = clean(translated);
    if text.is_empty() {
        Err(Failure::Unreadable)
    } else {
        Ok(text)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_request_carries_the_text_and_the_target() {
        let q = query("  good morning ", "vi").unwrap();
        assert_eq!(q[0], ("q", "good morning".to_owned()));
        assert_eq!(q[1], ("langpair", "Autodetect|vi".to_owned()));
    }

    #[test]
    fn what_cannot_be_sent_is_not() {
        assert!(query("", "vi").is_none());
        assert!(query("   ", "vi").is_none());
        assert!(query("hello", "xx").is_none());
        assert!(query("hello", "").is_none());
        assert!(query(&"a".repeat(MAX_CHARS + 1), "vi").is_none());
        assert!(query(&"a".repeat(MAX_CHARS), "vi").is_some());
        // Characters, not bytes: Vietnamese and Japanese take several bytes each.
        assert!(query(&"\u{1ec7}".repeat(MAX_CHARS), "en").is_some());
    }

    #[test]
    fn the_real_answer_is_read() {
        let body = r#"{"responseData":{"translatedText":"xin ch\u00e0o","match":0.99},"quotaFinished":false,"responseDetails":"","responseStatus":200,"matches":[{"translation":"[object HTMLTextAreaElement]"}]}"#;
        assert_eq!(parse_response(body), Ok("xin ch\u{e0}o".to_owned()));
        let as_text = r#"{"responseData":{"translatedText":"ok"},"responseStatus":"200"}"#;
        assert_eq!(parse_response(as_text), Ok("ok".to_owned()));
    }

    #[test]
    fn the_failures_are_told_apart() {
        let same = r#"{"responseData":{"translatedText":"PLEASE SELECT TWO DISTINCT LANGUAGES"},"responseStatus":403}"#;
        assert_eq!(parse_response(same), Err(Failure::SameLanguage));
        let quota = r#"{"responseData":{"translatedText":"MYMEMORY WARNING: YOU USED ALL AVAILABLE FREE TRANSLATIONS FOR TODAY."},"quotaFinished":true,"responseStatus":429}"#;
        assert_eq!(parse_response(quota), Err(Failure::Quota));
        let warning_only = r#"{"responseData":{"translatedText":"MYMEMORY WARNING: NEXT AVAILABLE IN 3 HOURS"},"responseStatus":200}"#;
        assert_eq!(parse_response(warning_only), Err(Failure::Quota));
        let other = r#"{"responseData":{"translatedText":""},"responseStatus":500}"#;
        assert_eq!(parse_response(other), Err(Failure::Service(500)));
        for body in [
            "",
            "<html>",
            "[]",
            "{}",
            r#"{"responseStatus":200}"#,
            r#"{"responseData":{"translatedText":"  "},"responseStatus":200}"#,
        ] {
            assert_eq!(parse_response(body), Err(Failure::Unreadable), "{body}");
        }
    }

    #[test]
    fn entities_are_decoded_and_odd_ones_left_alone() {
        assert_eq!(
            decode_entities("it&#39;s &amp; &lt;b&gt; &quot;x&quot;"),
            "it's & <b> \"x\""
        );
        assert_eq!(decode_entities("&#x41;&#66;"), "AB");
        assert_eq!(decode_entities("fish & chips"), "fish & chips");
        assert_eq!(
            decode_entities("&unknown; &#99999999999; &"),
            "&unknown; &#99999999999; &"
        );
        assert_eq!(decode_entities("a&amp"), "a&amp");
    }

    #[test]
    fn the_shown_text_is_plain_and_bounded() {
        assert_eq!(clean("  a\u{7}b\u{1b}[31mc\n d  "), "ab[31mc\n d");
        let long = "x".repeat(10_000);
        assert_eq!(clean(&long).chars().count(), MAX_SHOWN);
    }

    #[test]
    fn every_language_has_a_name_and_a_unique_code() {
        let mut codes = std::collections::HashSet::new();
        for language in LANGUAGES {
            assert!(!language.name.is_empty());
            assert!(codes.insert(language.code));
        }
    }
}
