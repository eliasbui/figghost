//! The Video tab of the notch (beta): one stream or video file played in the open notch, muted. It is off until the
//! person turns it on and gives an address.
//!
//! Playing is AVPlayer's own: HLS, MP4 and MOV over https, or a local file. The player is made when the tab shows and
//! dropped when it hides, so a closed notch fetches and decodes nothing. Nothing is kept on disk but the address.

use super::AttachEnv;
use crate::{i18n, settings::model::Fault, trace};
use macos_sys::{
    app,
    video_ui::{PAGE_SIZE, VideoView},
};
use serde::{Deserialize, Serialize};
use std::{
    cell::{Cell, RefCell},
    rc::{Rc, Weak},
};

pub mod settings;
mod text;

/// While the tab shows, whether the stream broke is looked at this often, in seconds.
const WATCH_SECONDS: f64 = 2.0;
const MAX_URL: usize = 2048;

#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    pub enabled: bool,
    /// An http, https or file address; empty is none.
    pub url: String,
}

/// An address AVPlayer can be given: a web or file address in one piece.
pub fn valid_url(url: &str) -> bool {
    url.len() <= MAX_URL
        && ["https://", "http://", "file://"]
            .iter()
            .any(|scheme| url.starts_with(scheme))
        && !url.chars().any(|c| c.is_whitespace() || c.is_control())
}

impl Config {
    /// A config file can say anything: an address that cannot play is none.
    pub fn sanitized(mut self) -> Self {
        self.url = self.url.trim().to_owned();
        if !valid_url(&self.url) {
            self.url.clear();
        }
        self
    }
}

pub struct Feature {
    view: Rc<VideoView>,
    config: RefCell<Config>,
    watcher: RefCell<Option<app::Timer>>,
    showing: Cell<bool>,
    me: Weak<Feature>,
}

/// Adds the tab. `None` without a notch, which is where the tab lives.
pub fn attach(env: &AttachEnv) -> Option<Rc<Feature>> {
    let notch = env.notch?;
    let view = VideoView::new(env.mtm, notch.page_area(PAGE_SIZE.width, PAGE_SIZE.height));
    let feature = Rc::new_cyclic(|me| Feature {
        view: Rc::clone(&view),
        config: RefCell::new(env.config.video.clone().sanitized()),
        watcher: RefCell::default(),
        showing: Cell::new(false),
        me: me.clone(),
    });
    let (shown, hidden) = (Rc::downgrade(&feature), Rc::downgrade(&feature));
    notch.add_page(view.page(
        i18n::current().video_tab().to_owned(),
        move || {
            if let Some(f) = shown.upgrade() {
                f.showing.set(true);
                f.begin();
            }
        },
        move || {
            if let Some(f) = hidden.upgrade() {
                f.showing.set(false);
                f.end();
                trace::log("video: closed");
            }
        },
    ));
    Some(feature)
}

fn note((title, hint): (&str, &str)) -> Option<(String, String)> {
    Some((title.to_owned(), hint.to_owned()))
}

impl Feature {
    pub fn configure(&self, config: &Config) -> Result<(), Fault> {
        *self.config.borrow_mut() = config.clone().sanitized();
        if self.showing.get() {
            self.begin();
        } else {
            self.end();
        }
        Ok(())
    }

    /// The tab opened, or its address changed while it shows: start over from the address.
    fn begin(&self) {
        self.end();
        let config = self.config.borrow().clone();
        let language = i18n::current();
        if !config.enabled || config.url.is_empty() {
            self.view.note(note(language.video_no_url()));
            return;
        }
        if !self.view.play(&config.url) {
            self.view.note(note(language.video_failed()));
            return;
        }
        self.view.note(None);
        trace::log("video: playing");
        let weak = self.me.clone();
        *self.watcher.borrow_mut() = Some(app::every(WATCH_SECONDS, move || {
            if let Some(f) = weak.upgrade()
                && f.view.failed()
            {
                trace::log("video: failed");
                f.end();
                f.view.note(note(i18n::current().video_failed()));
            }
        }));
    }

    /// Nothing plays and nothing is watched.
    fn end(&self) {
        self.watcher.borrow_mut().take();
        self.view.stop();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_a_web_or_file_address_in_one_piece_is_valid() {
        assert!(valid_url("https://example.com/live.m3u8"));
        assert!(valid_url("http://192.168.1.5:8080/stream.mp4"));
        assert!(valid_url("file:///Users/me/clip.mov"));
        assert!(!valid_url("rtsp://camera/stream"));
        assert!(!valid_url("https://a b"));
        assert!(!valid_url("https://a\n"));
        assert!(!valid_url(&format!("https://{}", "a".repeat(MAX_URL))));
        assert!(!valid_url(""));
    }

    #[test]
    fn a_config_file_with_a_bad_address_has_none() {
        let config = Config {
            enabled: true,
            url: " javascript:alert(1) ".into(),
        };
        assert_eq!(config.sanitized().url, "");
        let config = Config {
            enabled: true,
            url: " https://example.com/a.mp4 ".into(),
        };
        assert_eq!(config.sanitized().url, "https://example.com/a.mp4");
    }
}
