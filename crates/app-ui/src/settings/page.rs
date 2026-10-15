//! Assembles the settings page: one HTML string with the styles and the script inlined, so nothing is
//! read from disk or the network at run time.

use macos_sys::widgets::{image_png, symbol_png};
use serde_json::{Map, Value, json};

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
    include_str!("../../web/features/mascot.js"),
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
    include_str!("../../web/features/mascot.css"),
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
    (
        "mascot",
        include_str!("../../web/features/mascot.fixture.json"),
    ),
];

/// The SF Symbols the page draws (app.js `icon()`): page icons and button glyphs. The page's self-test fails
/// for an icon whose name is missing here.
const SYMBOLS: &[&str] = &[
    "gearshape.fill",
    "computermouse.fill",
    "doc.on.clipboard.fill",
    "rectangle.topthird.inset.filled",
    "terminal.fill",
    "text.magnifyingglass",
    "camera.viewfinder",
    "eyedropper.halffull",
    "calendar",
    "rectangle.split.2x1.fill",
    "info.circle.fill",
    "plus",
    "xmark",
    "folder",
    "arrow.clockwise",
    "trash",
    "power",
    "chevron.up",
    "chevron.down",
];
/// Symbols are drawn this many points on their longer side, for Retina screens; CSS fits them to their box.
const SYMBOL_POINTS: f64 = 16.0;
const SCALE: f64 = 2.0;
const APP_ICON: &[u8] = include_bytes!("../../assets/Figghost.icns");

/// `window.__symbols` (name to a white PNG) and `window.__appIcon`, drawn by the system since the page may
/// load nothing. A symbol this system lacks is left out and its glyph stays empty.
fn assets() -> String {
    let url = |png: Vec<u8>| Value::String(format!("data:image/png;base64,{}", base64(&png)));
    let symbols: Map<String, Value> = SYMBOLS
        .iter()
        .filter_map(|&name| Some((name.into(), url(symbol_png(name, SYMBOL_POINTS, SCALE)?))))
        .collect();
    let icon = image_png(APP_ICON, 64.0, SCALE).map(url);
    format!(
        "window.__symbols = {};\nwindow.__appIcon = {};",
        Value::Object(symbols),
        json!(icon)
    )
}

/// The same as `settings::base64`, which this file cannot reach when the preview example includes it.
fn base64(bytes: &[u8]) -> String {
    const TABLE: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        let n = chunk
            .iter()
            .enumerate()
            .fold(0u32, |n, (i, &b)| n | u32::from(b) << (16 - 8 * i));
        for i in 0..4 {
            if i <= chunk.len() {
                out.push(char::from(TABLE[(n >> (18 - 6 * i) & 63) as usize]));
            } else {
                out.push('=');
            }
        }
    }
    out
}

pub fn html() -> String {
    let script = format!("{}\n{}\n{SCRIPT}", assets(), FEATURE_SCRIPTS.join("\n"));
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
    fn every_symbol_the_page_names_exists() {
        for name in SYMBOLS {
            assert!(
                symbol_png(name, SYMBOL_POINTS, SCALE).is_some(),
                "{name} is not an SF Symbol on this system"
            );
        }
        assert!(image_png(APP_ICON, 64.0, SCALE).is_some(), "the app icon");
    }

    #[test]
    fn every_feature_has_sample_data() {
        let sample = fixture();
        for (id, _) in FEATURE_FIXTURES {
            assert!(sample[*id].is_object(), "{id} has no sample data");
        }
    }
}
