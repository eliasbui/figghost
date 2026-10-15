//! Assembles the settings page: one HTML string with the styles and the script inlined, so nothing is
//! read from disk or the network at run time.

use serde_json::Value;

const INDEX: &str = include_str!("../../web/index.html");
const STYLE: &str = include_str!("../../web/app.css");
const SCRIPT: &str = include_str!("../../web/app.js");
/// What each feature brings to the page: scripts that register a page and its strings (they run before
/// `app.js` reads them), styles, and the sample data of its part of the state.
const FEATURE_SCRIPTS: &[&str] = &[
    include_str!("../../web/features/smart.js"),
    include_str!("../../web/features/capture.js"),
    include_str!("../../web/features/vision.js"),
    include_str!("../../web/features/lyrics.js"),
    include_str!("../../web/features/calendar.js"),
    include_str!("../../web/features/snap.js"),
    include_str!("../../web/features/menubar.js"),
    include_str!("../../web/features/stack.js"),
];
const FEATURE_STYLES: &[&str] = &[
    include_str!("../../web/features/smart.css"),
    include_str!("../../web/features/capture.css"),
    include_str!("../../web/features/vision.css"),
    include_str!("../../web/features/lyrics.css"),
    include_str!("../../web/features/calendar.css"),
    include_str!("../../web/features/snap.css"),
    include_str!("../../web/features/menubar.css"),
    include_str!("../../web/features/stack.css"),
];
const FEATURE_FIXTURES: &[(&str, &str)] = &[
    (
        "smart",
        include_str!("../../web/features/smart.fixture.json"),
    ),
    (
        "capture",
        include_str!("../../web/features/capture.fixture.json"),
    ),
    (
        "vision",
        include_str!("../../web/features/vision.fixture.json"),
    ),
    (
        "lyrics",
        include_str!("../../web/features/lyrics.fixture.json"),
    ),
    (
        "calendar",
        include_str!("../../web/features/calendar.fixture.json"),
    ),
    ("snap", include_str!("../../web/features/snap.fixture.json")),
    (
        "menubar",
        include_str!("../../web/features/menubar.fixture.json"),
    ),
    (
        "stack",
        include_str!("../../web/features/stack.fixture.json"),
    ),
];

pub fn html() -> String {
    let script = format!("{}\n{SCRIPT}", FEATURE_SCRIPTS.join("\n"));
    let style = format!("{STYLE}\n{}", FEATURE_STYLES.join("\n"));
    INDEX
        .replace("/*__CSS__*/", &style)
        .replace("/*__JS__*/", &script)
}

/// The sample state for previews and for the page's own checks: `fixture.json` with each feature's part
/// under its id. A unit test keeps it in step with the real state.
#[allow(dead_code)]
pub fn fixture() -> Value {
    let mut state: Value =
        serde_json::from_str(include_str!("../../web/fixture.json")).expect("fixture is JSON");
    for (id, text) in FEATURE_FIXTURES {
        let part: Value =
            serde_json::from_str(text).unwrap_or_else(|e| panic!("{id} fixture is not JSON: {e}"));
        state[*id] = part;
    }
    state
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_page_is_one_self_contained_document() {
        let page = html();
        assert!(!page.contains("/*__CSS__*/") && !page.contains("/*__JS__*/"));
        assert!(page.contains("Content-Security-Policy"));
        // Nothing may be fetched: no external scripts, styles, images or fonts.
        for forbidden in ["src=\"http", "href=\"http", "url(http", "@import", "<link "] {
            assert!(!page.contains(forbidden), "found {forbidden}");
        }
    }

    #[test]
    fn no_script_can_close_its_own_tag() {
        for script in std::iter::once(&SCRIPT).chain(FEATURE_SCRIPTS) {
            assert!(!script.to_lowercase().contains("</script"));
        }
    }

    #[test]
    fn every_feature_has_sample_data() {
        let sample = fixture();
        for (id, _) in FEATURE_FIXTURES {
            assert!(sample[*id].is_object(), "{id} has no sample data");
        }
    }
}
