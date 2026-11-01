//! The speed of one connection, measured by the system's own `networkQuality` (Apple's responsiveness test) bound to
//! that interface. The text it prints is read here; running it is `Feature::speed_test`.

use serde_json::{Value, json};

/// What a finished test found.
#[derive(Debug, Clone, PartialEq)]
pub struct Speed {
    /// Bits a second.
    pub down_bps: u64,
    pub up_bps: u64,
    /// Round trips a minute under load, Apple's measure of how responsive the line stays when it is busy.
    pub rpm: Option<u32>,
    /// The round trip with nothing else going on, in milliseconds.
    pub rtt_ms: Option<u32>,
}

/// Reads the JSON of `networkQuality -c`. An error the tool reports, a throughput of nothing and text that is not that
/// JSON give `Err` with a short reason the page can word: `error`, `empty` or `unreadable`.
pub fn parse(text: &str) -> Result<Speed, &'static str> {
    let root: Value = serde_json::from_str(text.trim()).map_err(|_| "unreadable")?;
    if root
        .get("error_code")
        .is_some_and(|c| c.as_i64().is_some_and(|c| c != 0))
    {
        return Err("error");
    }
    let number = |key: &str| root[key].as_f64().filter(|n| n.is_finite() && *n >= 0.0);
    let (Some(down), Some(up)) = (number("dl_throughput"), number("ul_throughput")) else {
        return Err("unreadable");
    };
    if down < 1.0 && up < 1.0 {
        return Err("empty");
    }
    Ok(Speed {
        down_bps: down as u64,
        up_bps: up as u64,
        rpm: number("responsiveness").map(|n| n.round() as u32),
        rtt_ms: number("base_rtt").map(|n| n.round() as u32),
    })
}

/// How a line holds up under load by Apple's own bands: `low` under 200 round trips a minute, `medium` to 1000, `high`.
pub fn grade(rpm: u32) -> &'static str {
    match rpm {
        0..200 => "low",
        200..1000 => "medium",
        _ => "high",
    }
}

/// The result as the page gets it.
pub fn done(speed: &Speed, at_ms: i64) -> Value {
    json!({
        "state": "done", "down": speed.down_bps, "up": speed.up_bps, "rpm": speed.rpm,
        "rtt": speed.rtt_ms, "grade": speed.rpm.map(grade), "at": at_ms,
    })
}

pub fn failed(reason: &str) -> Value {
    json!({ "state": "failed", "reason": reason })
}

pub fn running() -> Value {
    json!({ "state": "running" })
}

#[cfg(test)]
mod tests {
    use super::*;

    const GOOD: &str = r#"{ "dl_throughput" : 93814760, "ul_throughput" : 21022520, "responsiveness" : 412.5,
        "base_rtt" : 18.4, "dl_flows" : 12, "draft_version" : 8 }"#;

    #[test]
    fn a_finished_test_is_read() {
        let s = parse(GOOD).unwrap();
        assert_eq!(
            (s.down_bps, s.up_bps, s.rpm, s.rtt_ms),
            (93_814_760, 21_022_520, Some(413), Some(18))
        );
        let v = done(&s, 5);
        assert_eq!(
            (v["state"].clone(), v["grade"].clone()),
            (json!("done"), json!("medium"))
        );
        assert!(
            parse(r#"{ "dl_throughput": 5e7, "ul_throughput": 1e6 }"#)
                .unwrap()
                .rpm
                .is_none()
        );
    }

    #[test]
    fn what_the_tool_reports_as_wrong_and_what_is_not_a_result_are_refused() {
        // What this Mac's tool printed through a network that blocks the upload: an error code and no throughput.
        let blocked = r#"{ "dl_throughput" : 0, "ul_throughput" : 0, "error_code" : -1000, "error_domain" : "NSURLErrorDomain" }"#;
        assert_eq!(parse(blocked), Err("error"));
        assert_eq!(parse(r#"{ "error_code" : -1009 }"#), Err("error"));
        assert_eq!(
            parse(r#"{ "dl_throughput" : 0, "ul_throughput" : 0 }"#),
            Err("empty")
        );
        assert_eq!(
            parse(r#"{ "dl_throughput" : -5, "ul_throughput" : 1 }"#),
            Err("unreadable")
        );
        for bad in [
            "",
            "not json",
            "[]",
            "null",
            r#"{ "dl_throughput" : "fast" }"#,
        ] {
            assert_eq!(parse(bad), Err("unreadable"), "{bad:?}");
        }
        assert_eq!(failed("error")["state"], json!("failed"));
    }

    #[test]
    fn responsiveness_is_banded_as_apple_bands_it() {
        assert_eq!(
            (grade(0), grade(199), grade(200), grade(999), grade(1000)),
            ("low", "low", "medium", "medium", "high")
        );
    }
}
