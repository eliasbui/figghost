//! The page of the update popup: one HTML string with the styles and the script inlined, so nothing is read
//! from disk or the network at run time.

const PAGE: &str = include_str!("../../web/update.html");

const NEWS_PAGE: &str = include_str!("../../web/whatsnew.html");

/// The page that says what is new in the version that was just installed, with the same `icon`.
pub fn news_html(icon: &str) -> String {
    NEWS_PAGE.replace("/*__ICON__*/", icon)
}

/// `icon` is the app's icon as a data URL, or empty for none.
pub fn html(icon: &str) -> String {
    PAGE.replace("/*__ICON__*/", icon)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_page_is_one_self_contained_document() {
        let page = html("data:image/png;base64,AAAA");
        assert!(!page.contains("/*__ICON__*/"));
        assert!(page.contains("data:image/png;base64,AAAA"));
        assert!(page.contains("Content-Security-Policy"));
        // Nothing may be fetched: no external scripts, styles, images or fonts.
        for forbidden in ["src=\"http", "href=\"http", "url(http", "@import", "<link "] {
            assert!(!page.contains(forbidden), "found {forbidden}");
        }
    }

    #[test]
    fn the_news_page_is_one_self_contained_document() {
        let page = news_html("data:image/png;base64,AAAA");
        assert!(!page.contains("/*__ICON__*/"));
        assert!(page.contains("Content-Security-Policy"));
        for forbidden in ["src=\"http", "href=\"http", "url(http", "@import", "<link "] {
            assert!(!page.contains(forbidden), "found {forbidden}");
        }
        assert_eq!(NEWS_PAGE.to_lowercase().matches("</script").count(), 2);
    }

    #[test]
    fn no_script_can_close_its_own_tag() {
        // The two scripts of the page, and no more: a third closing tag would be inside one of them.
        assert_eq!(PAGE.to_lowercase().matches("</script").count(), 2);
    }
}
