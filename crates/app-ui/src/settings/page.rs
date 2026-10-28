//! Assembles the settings page: one HTML string with the styles and the script inlined, so nothing is
//! read from disk or the network at run time.

use macos_sys::widgets::{base64, image_png, symbol_png};
use serde_json::{Map, Value, json};

const INDEX: &str = include_str!("../../web/index.html");
const STYLE: &str = include_str!("../../web/app.css");
const SCRIPT: &str = include_str!("../../web/app.js");
/// What each feature brings to the page: scripts that register a page and its strings (they run before
/// `app.js` reads them), styles, and the sample data of its part of the state.
const FEATURE_SCRIPTS: &[&str] = &[
    include_str!("../../web/features/smart.js"),
    include_str!("../../web/features/translate.js"),
    include_str!("../../web/features/capture.js"),
    include_str!("../../web/features/vision.js"),
    include_str!("../../web/features/lyrics.js"),
    include_str!("../../web/features/calendar.js"),
    include_str!("../../web/features/snap.js"),
    include_str!("../../web/features/pin.js"),
    include_str!("../../web/features/cursor.js"),
    include_str!("../../web/features/musicwidget.js"),
    include_str!("../../web/features/menubar.js"),
    include_str!("../../web/features/stack.js"),
    include_str!("../../web/features/mascot.js"),
    include_str!("../../web/features/wheel.js"),
    include_str!("../../web/features/mole.js"),
    include_str!("../../web/features/islands.js"),
    include_str!("../../web/features/utilities.js"),
    include_str!("../../web/features/notes.js"),
    include_str!("../../web/features/launcher.js"),
    include_str!("../../web/features/weather.js"),
    include_str!("../../web/features/prs.js"),
    include_str!("../../web/features/updates.js"),
    include_str!("../../web/features/pomodoro.js"),
    include_str!("../../web/features/screentime.js"),
    include_str!("../../web/features/dayprogress.js"),
    include_str!("../../web/features/devices.js"),
    include_str!("../../web/features/displays.js"),
    include_str!("../../web/features/present.js"),
    include_str!("../../web/features/notifications.js"),
    include_str!("../../web/features/outlook.js"),
    include_str!("../../web/features/video.js"),
    include_str!("../../web/features/system.js"),
    include_str!("../../web/features/typing.js"),
    include_str!("../../web/features/snippets.js"),
    include_str!("../../web/features/download.js"),
];
const FEATURE_STYLES: &[&str] = &[
    include_str!("../../web/features/smart.css"),
    include_str!("../../web/features/translate.css"),
    include_str!("../../web/features/capture.css"),
    include_str!("../../web/features/vision.css"),
    include_str!("../../web/features/lyrics.css"),
    include_str!("../../web/features/calendar.css"),
    include_str!("../../web/features/snap.css"),
    include_str!("../../web/features/pin.css"),
    include_str!("../../web/features/cursor.css"),
    include_str!("../../web/features/musicwidget.css"),
    include_str!("../../web/features/menubar.css"),
    include_str!("../../web/features/stack.css"),
    include_str!("../../web/features/mascot.css"),
    include_str!("../../web/features/wheel.css"),
    include_str!("../../web/features/mole.css"),
    include_str!("../../web/features/islands.css"),
    include_str!("../../web/features/utilities.css"),
    include_str!("../../web/features/weather.css"),
    include_str!("../../web/features/launcher.css"),
    include_str!("../../web/features/prs.css"),
    include_str!("../../web/features/updates.css"),
    include_str!("../../web/features/snippets.css"),
    include_str!("../../web/features/download.css"),
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
    ("pin", include_str!("../../web/features/pin.fixture.json")),
    (
        "cursor",
        include_str!("../../web/features/cursor.fixture.json"),
    ),
    (
        "musicwidget",
        include_str!("../../web/features/musicwidget.fixture.json"),
    ),
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
    (
        "wheel",
        include_str!("../../web/features/wheel.fixture.json"),
    ),
    ("mole", include_str!("../../web/features/mole.fixture.json")),
    (
        "translate",
        include_str!("../../web/features/translate.fixture.json"),
    ),
    (
        "islands",
        include_str!("../../web/features/islands.fixture.json"),
    ),
    (
        "utilities",
        include_str!("../../web/features/utilities.fixture.json"),
    ),
    (
        "notes",
        include_str!("../../web/features/notes.fixture.json"),
    ),
    (
        "launcher",
        include_str!("../../web/features/launcher.fixture.json"),
    ),
    (
        "weather",
        include_str!("../../web/features/weather.fixture.json"),
    ),
    ("prs", include_str!("../../web/features/prs.fixture.json")),
    (
        "updates",
        include_str!("../../web/features/updates.fixture.json"),
    ),
    (
        "pomodoro",
        include_str!("../../web/features/pomodoro.fixture.json"),
    ),
    (
        "screentime",
        include_str!("../../web/features/screentime.fixture.json"),
    ),
    (
        "dayprogress",
        include_str!("../../web/features/dayprogress.fixture.json"),
    ),
    (
        "devices",
        include_str!("../../web/features/devices.fixture.json"),
    ),
    (
        "displays",
        include_str!("../../web/features/displays.fixture.json"),
    ),
    (
        "present",
        include_str!("../../web/features/present.fixture.json"),
    ),
    (
        "notifications",
        include_str!("../../web/features/notifications.fixture.json"),
    ),
    (
        "outlook",
        include_str!("../../web/features/outlook.fixture.json"),
    ),
    (
        "video",
        include_str!("../../web/features/video.fixture.json"),
    ),
    (
        "system",
        include_str!("../../web/features/system.fixture.json"),
    ),
    (
        "typing",
        include_str!("../../web/features/typing.fixture.json"),
    ),
    (
        "snippets",
        include_str!("../../web/features/snippets.fixture.json"),
    ),
    (
        "download",
        include_str!("../../web/features/download.fixture.json"),
    ),
];

/// The SF Symbols the page draws (app.js `icon()`): page icons and button glyphs. The page's self-test fails
/// for an icon whose name is missing here.
const SYMBOLS: &[&str] = &[
    "gearshape.fill",
    "gearshape.2.fill",
    "wand.and.stars",
    "paintbrush.fill",
    "slider.horizontal.3",
    "wrench.and.screwdriver.fill",
    "switch.2",
    "scroll.fill",
    "laptopcomputer",
    "menubar.dock.rectangle",
    "square.stack.3d.up.fill",
    "list.clipboard.fill",
    "square.split.2x2.fill",
    "rectangle.lefthalf.inset.filled",
    "keyboard",
    "keyboard.fill",
    "computermouse.fill",
    "magicmouse.fill",
    "doc.on.clipboard.fill",
    "rectangle.topthird.inset.filled",
    "terminal.fill",
    "magnifyingglass",
    "text.magnifyingglass",
    "character.bubble",
    "character.bubble.fill",
    "pin",
    "pin.fill",
    "cursorarrow.rays",
    "cursorarrow.motionlines",
    "music.note",
    "music.quarternote.3",
    "camera.viewfinder",
    "camera.fill",
    "eyedropper.halffull",
    "calendar",
    "calendar.badge.clock",
    "rectangle.split.2x1.fill",
    "info.circle.fill",
    "sparkles",
    "bell.badge.fill",
    "waveform.circle.fill",
    "waveform.and.mic",
    "plus",
    "xmark",
    "folder",
    "folder.fill",
    "arrow.clockwise",
    "trash",
    "trash.fill",
    "power",
    "chevron.up",
    "chevron.down",
    "chevron.left",
    "chevron.right",
    "chart.pie.fill",
    "checkmark.shield",
    "checkmark.shield.fill",
    "cpu",
    "cpu.fill",
    "note.text",
    "cloud.sun.fill",
    "square.grid.2x2.fill",
    "hourglass",
    "sun.horizon.fill",
    "sun.max.fill",
    "headphones",
    "airpodspro",
    "display",
    "display.2",
    "rectangle.on.rectangle",
    "play.rectangle.fill",
    "timer",
    "arrow.triangle.pull",
    "arrow.down.circle",
    "arrow.down.circle.fill",
    "arrow.down.app.fill",
    "shippingbox.fill",
    "text.cursor",
    "text.quote",
    "pencil",
];
/// Symbols are drawn this many points on their longer side, for Retina screens; CSS fits them to their box.
const SYMBOL_POINTS: f64 = 20.0;
const SCALE: f64 = 2.0;
const APP_ICON: &[u8] = include_bytes!("../../assets/Figghost.icns");

/// Inter, the page's font: two subsets of one variable file, `wght` 100..900, for the Latin and the
/// Vietnamese of the interface. Bundled under the SIL Open Font License (`assets/Inter-OFL.txt`), because
/// the page fetches nothing at run time.
const FONTS: &[(&[u8], &str)] = &[
    (
        include_bytes!("../../assets/Inter-latin.woff2"),
        "U+0000-00FF,U+0131,U+0152-0153,U+02BB-02BC,U+02C6,U+02DA,U+02DC,U+0304,U+0308,U+0329,U+2000-206F,U+20AC,U+2122,U+2191,U+2193,U+2212,U+2215,U+FEFF,U+FFFD",
    ),
    (
        include_bytes!("../../assets/Inter-vietnamese.woff2"),
        "U+0102-0103,U+0110-0111,U+0128-0129,U+0168-0169,U+01A0-01A1,U+01AF-01B0,U+0300-0301,U+0303-0304,U+0308-0309,U+0323,U+0329,U+1EA0-1EF9,U+20AB",
    ),
];

/// The `@font-face` rules of one family, with the fonts themselves inline. The ranges are the ones the subsets
/// were cut for, so a character outside them (a key symbol such as `⌥`) comes from the system font instead.
fn faces(family: &str, weight: &str, fonts: &[(&[u8], &str)]) -> String {
    fonts
        .iter()
        .map(|(bytes, range)| {
            format!(
                "@font-face {{ font-family: \"{family}\"; font-style: normal; font-weight: {weight}; \
                 src: url(data:font/woff2;base64,{}) format('woff2'); unicode-range: {range}; }}\n",
                base64(bytes)
            )
        })
        .collect()
}

fn font_faces() -> String {
    faces("Inter", "100 900", FONTS)
}

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

pub fn html() -> String {
    let script = format!("{}\n{}\n{SCRIPT}", assets(), FEATURE_SCRIPTS.join("\n"));
    let style = format!("{}{STYLE}\n{}", font_faces(), FEATURE_STYLES.join("\n"));
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
    fn the_licence_on_the_about_page_is_the_licence_file_line_for_line() {
        let licence = include_str!("../../../../LICENSE");
        let lines: Vec<&str> = licence.lines().filter(|l| !l.trim().is_empty()).collect();
        assert!(lines.len() > 10 && lines[0] == "MIT License");
        for line in lines {
            assert!(
                SCRIPT.contains(&format!("'{line}'")),
                "the About page does not have this line of LICENSE: {line}"
            );
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
