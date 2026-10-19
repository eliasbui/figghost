//! What is new in the version that was just installed: a popup that opens by itself the first time a new version
//! runs, as a list for a few changes and as slides for many. The notes are the ones in `docs/releases`, built into
//! the app, so nothing is fetched.

use super::{UpdateFeature, icon_url, page, release};
use crate::trace;
use macos_sys::{
    app,
    webview::{WebWindow, WindowSpec},
};
use serde_json::{Value, json};
use std::fs;

include!(concat!(env!("OUT_DIR"), "/release_notes.rs"));

/// The version whose news was last shown, in the data folder.
const SEEN_FILE: &str = "whatsnew-seen";
/// The popup opens this long after the launch, once the other windows have settled.
const OPEN_AFTER_SECONDS: f64 = 1.5;
const WIDTH: f64 = 520.0;
const HEIGHT: f64 = 580.0;
const MIN_HEIGHT: f64 = 460.0;

/// The notes of a version as the lines worth showing, empty when the version has none.
fn notes_of(version: &str) -> Vec<String> {
    NOTES
        .iter()
        .find(|(v, _)| *v == version)
        .map(|(_, text)| release::changes(text))
        .unwrap_or_default()
}

/// Whether the news of `current` is to be shown by itself: only the first time a newer version runs. A first
/// install has nothing to compare with, so it shows nothing, and neither does a version that has no notes.
fn is_news(seen: Option<&str>, current: &str, has_notes: bool) -> bool {
    has_notes && seen.is_some_and(|seen| release::newer(current, seen))
}

impl UpdateFeature {
    /// Shows the news of this version once, the first time it runs after an update.
    pub(super) fn start_whats_new(&self) {
        let current = env!("CARGO_PKG_VERSION");
        let path = self.home.join(SEEN_FILE);
        let seen = fs::read_to_string(&path).ok().map(|s| s.trim().to_owned());
        if seen.as_deref() == Some(current) {
            return;
        }
        // Remembered before it opens, so that a crash while it is open does not bring it back at every launch.
        let _ = fs::write(&path, current);
        let wanted = self.config.borrow().update.whats_new;
        if wanted && is_news(seen.as_deref(), current, !notes_of(current).is_empty()) {
            let weak = self.me.clone();
            app::after(OPEN_AFTER_SECONDS, move || {
                if let Some(this) = weak.upgrade() {
                    this.show_whats_new();
                }
            });
        }
    }

    /// Opens the news of the running version, or brings it to the front: also the About page's button.
    pub fn show_whats_new(&self) {
        let existing = self.news.borrow().clone();
        if let Some(window) = existing {
            window.show(self.mtm);
            return;
        }
        let (on_message, on_close) = (self.me.clone(), self.me.clone());
        let window = WebWindow::new(
            self.mtm,
            &WindowSpec {
                title: "Figghost",
                width: WIDTH,
                height: HEIGHT,
                min_width: WIDTH,
                min_height: MIN_HEIGHT,
            },
            &page::news_html(&icon_url()),
            move |text| {
                if let Some(this) = on_message.upgrade() {
                    this.news_message(&text);
                }
            },
            move || {
                if let Some(this) = on_close.upgrade() {
                    this.news_closed();
                }
            },
        );
        *self.news.borrow_mut() = Some(std::rc::Rc::clone(&window));
        window.show(self.mtm);
        trace::log("update: the news of this version is open");
    }

    fn news_message(&self, text: &str) {
        let Ok(message) = serde_json::from_str::<Value>(text) else {
            return;
        };
        match message["t"].as_str() {
            Some("ready") => {
                if let Some(window) = self.news.borrow().as_ref() {
                    let version = env!("CARGO_PKG_VERSION");
                    window.host.post(
                        &json!({
                            "t": "show",
                            "lang": crate::i18n::current().code(),
                            "version": version,
                            "changes": notes_of(version),
                        })
                        .to_string(),
                    );
                }
            }
            Some("js_error") => eprintln!(
                "news page: {} (line {})",
                message["message"].as_str().unwrap_or_default(),
                message["line"]
            ),
            Some("action") if message["name"] == "close" => {
                if let Some(window) = self.news.borrow().as_ref() {
                    window.close();
                }
            }
            _ => {}
        }
    }

    fn news_closed(&self) {
        let weak = self.me.clone();
        // The window is still inside its own delegate call, so it is dropped on the next turn of the run loop.
        app::after(0.0, move || {
            let Some(this) = weak.upgrade() else {
                return;
            };
            let reopened = this.news.borrow().as_ref().is_some_and(|w| w.is_visible());
            if !reopened {
                this.news.borrow_mut().take();
            }
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_news_opens_only_when_a_newer_version_runs_for_the_first_time() {
        assert!(is_news(Some("0.7.0"), "0.8.0", true));
        assert!(!is_news(None, "0.8.0", true), "a first install");
        assert!(!is_news(Some("0.8.0"), "0.8.0", true), "already seen");
        assert!(!is_news(Some("0.9.0"), "0.8.0", true), "an older build");
        assert!(!is_news(Some("0.7.0"), "0.8.0", false), "no notes");
    }

    #[test]
    fn the_notes_of_this_version_are_built_in() {
        assert!(
            !notes_of(env!("CARGO_PKG_VERSION")).is_empty(),
            "docs/releases/{}.md is missing or has no list",
            env!("CARGO_PKG_VERSION")
        );
        assert!(notes_of("0.0.0").is_empty());
    }
}
