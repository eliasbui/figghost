//! The pages of the app's windows and panels. `app-tauri` writes them (`pnpm build:embed` there): one HTML file each in
//! `crates/app-ui/web`, with the script, the styles and the fonts inline, so nothing is read from disk or the network when
//! a page opens. What only the system can draw (the app's icon, the SF Symbols) is written into a page here, before its own
//! script runs.

use serde_json::Value;

/// Where a page has room for what the app tells it before it runs.
const PRELUDE: &str = "<!--__PRELUDE__-->";

/// `page` with each of `values` set as `window.__<name>`. A name is letters only, and a value cannot close the script.
pub fn with(page: &str, values: &[(&str, Value)]) -> String {
    let mut script = String::new();
    for (name, value) in values {
        assert!(
            name.chars().all(|c| c.is_ascii_alphabetic()),
            "a prelude name is letters only: {name}"
        );
        // `</` and `<!--` are what can end or hide a script; in a string, `\/` and `\!` mean the same characters.
        let text = value
            .to_string()
            .replace("</", "<\\/")
            .replace("<!--", "<\\!--");
        script.push_str(&format!("window.__{name} = {text};\n"));
    }
    let script = if script.is_empty() {
        String::new()
    } else {
        format!("<script>\n{script}</script>")
    };
    page.replacen(PRELUDE, &script, 1)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn a_value_reaches_the_page_as_a_global_and_cannot_close_the_script() {
        let page = format!("<body>{PRELUDE}<script>run()</script></body>");
        let out = with(
            &page,
            &[
                ("icon", json!("data:x</script><b>")),
                ("canvas", json!("network")),
            ],
        );
        assert!(!out.contains(PRELUDE));
        assert!(out.contains("window.__canvas = \"network\";"));
        assert!(out.contains("window.__icon = \"data:x<\\/script><b>\";"));
        assert_eq!(
            out.matches("</script").count(),
            2,
            "the page's own and the prelude's"
        );
    }

    #[test]
    #[should_panic(expected = "letters only")]
    fn a_name_that_is_not_letters_is_refused() {
        with("<!--__PRELUDE__-->", &[("a;b", json!(1))]);
    }
}
