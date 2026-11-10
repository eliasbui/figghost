//! The pages of the update popup and of the news of a version: one HTML string each, built by `app-tauri`, with the styles
//! and the script inlined, so nothing is read from disk or the network at run time.

use crate::web;
use serde_json::json;

const PAGE: &str = include_str!("../../web/update.html");

const NEWS_PAGE: &str = include_str!("../../web/whatsnew.html");

/// The page that says what is new in the version that was just installed, with the same `icon`.
pub fn news_html(icon: &str) -> String {
    web::with(NEWS_PAGE, &[("icon", json!(icon))])
}

/// `icon` is the app's icon as a data URL, or empty for none.
pub fn html(icon: &str) -> String {
    web::with(PAGE, &[("icon", json!(icon))])
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A page is one self-contained document: nothing may be fetched, so no external scripts, styles, images or fonts.
    fn assert_self_contained(page: &str) {
        assert!(!page.contains("<!--__PRELUDE__-->"));
        assert!(page.contains("data:image/png;base64,AAAA"));
        assert!(page.contains("Content-Security-Policy"));
        for forbidden in ["src=\"http", "href=\"http", "url(http", "@import", "<link "] {
            assert!(!page.contains(forbidden), "found {forbidden}");
        }
    }

    #[test]
    fn the_popup_is_one_self_contained_document() {
        assert_self_contained(&html("data:image/png;base64,AAAA"));
    }

    #[test]
    fn the_news_page_is_one_self_contained_document() {
        assert_self_contained(&news_html("data:image/png;base64,AAAA"));
    }

    #[test]
    fn no_script_can_close_its_own_tag() {
        // The page's own script, and the prelude's once the app has written it: a third closing tag would be inside one.
        for page in [PAGE, NEWS_PAGE] {
            assert_eq!(page.to_lowercase().matches("</script").count(), 1);
            assert_eq!(
                page.matches("<!--").count(),
                1,
                "only the place for the prelude"
            );
        }
        assert_eq!(html("x").to_lowercase().matches("</script").count(), 2);
    }
}
