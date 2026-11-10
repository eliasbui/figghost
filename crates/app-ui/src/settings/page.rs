//! The page of the settings window, and of the windows that show one canvas of it. `app-tauri` builds it (`pnpm
//! build:embed` there): one HTML file with its script, styles, fonts and pictures inline, so nothing is read from disk or
//! the network at run time.

use crate::web;
use serde_json::json;

const PAGE: &str = include_str!("../../web/settings.html");

pub fn html() -> String {
    web::with(PAGE, &[])
}

/// The same page for a window that shows only the canvas of one feature (`network`).
#[allow(dead_code)]
pub fn html_canvas(id: &str) -> String {
    let id: String = id.chars().filter(char::is_ascii_lowercase).collect();
    web::with(PAGE, &[("canvas", json!(id))])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_canvas_window_is_the_same_page_told_which_canvas_to_show() {
        let page = html_canvas("network");
        assert!(page.contains("window.__canvas = \"network\";"));
        assert!(!html().contains("window.__canvas ="));
        assert!(
            html_canvas("x'; alert(1)//").contains("window.__canvas = \"xalert\";"),
            "only letters reach the script"
        );
    }

    /// The page with what is inside its script and style elements taken out: the elements themselves are what can fetch.
    fn bare(page: &str) -> String {
        let mut out = String::new();
        let mut rest = page;
        while let Some(start) = rest.find("<script").or_else(|| rest.find("<style")) {
            let (head, tail) = rest.split_at(start);
            out.push_str(head);
            let close = if tail.starts_with("<script") {
                "</script>"
            } else {
                "</style>"
            };
            rest = tail
                .find(close)
                .map_or("", |end| &tail[end + close.len()..]);
        }
        out.push_str(rest);
        out
    }

    #[test]
    fn the_page_is_one_self_contained_document() {
        let page = html();
        assert!(page.contains("Content-Security-Policy"));
        // Nothing may be fetched: no external scripts, styles, images or fonts.
        for forbidden in ["src=\"http", "href=\"http", "url(http", "@import"] {
            assert!(!page.contains(forbidden), "found {forbidden}");
        }
        assert!(!bare(&page).contains("<link "), "an element that fetches");
    }

    #[test]
    fn the_page_has_one_script_that_cannot_close_early() {
        // The build writes `<\/script` and `<\!--` inside the script; the only real ones are the page's own.
        assert_eq!(PAGE.matches("</script").count(), 1);
        assert_eq!(
            PAGE.matches("<!--").count(),
            1,
            "only the place for the prelude"
        );
        assert_eq!(html_canvas("network").matches("</script").count(), 2);
    }

    #[test]
    fn the_licence_on_the_about_page_is_the_licence_file() {
        let shown = include_str!("../../../../app-tauri/src/assets/LICENSE.txt");
        let file = include_str!("../../../../LICENSE");
        assert_eq!(
            shown.trim_end(),
            file.trim_end(),
            "app-tauri/src/assets/LICENSE.txt is not LICENSE"
        );
        assert!(
            PAGE.contains("Permission is hereby granted, free of charge"),
            "the build has no licence in it"
        );
    }
}
