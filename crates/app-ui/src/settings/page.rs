//! Assembles the settings page: one HTML string with the styles and the script inlined, so nothing is
//! read from disk or the network at run time.

const INDEX: &str = include_str!("../../web/index.html");
const STYLE: &str = include_str!("../../web/app.css");
const SCRIPT: &str = include_str!("../../web/app.js");

pub fn html() -> String {
    INDEX
        .replace("/*__CSS__*/", STYLE)
        .replace("/*__JS__*/", SCRIPT)
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
    fn the_script_cannot_close_its_own_tag() {
        assert!(!SCRIPT.to_lowercase().contains("</script"));
    }
}
