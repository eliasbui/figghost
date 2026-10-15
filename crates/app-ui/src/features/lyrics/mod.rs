//! Lyrics that follow the music on the media tab.
//!
//! The tab already asks Music and Spotify what is playing, now with the position in the track. This looks the
//! track up (in the cache on disk, and online only when the person allowed it), shows the sung line with the
//! ones around it, and keeps it in step between two looks at the player with a timer that exists only while
//! synced lyrics are on screen and the music plays.

use super::AttachEnv;
use crate::{i18n, media::MediaFeature, settings::model::Fault};
use core_lyrics::{
    cache::Cache,
    lrclib::Query,
    service::{Service, Status},
};
use macos_sys::{app, media_ui::Lyrics, now_playing::Sample, status_item::Entry};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{
    cell::{Cell, RefCell},
    rc::{Rc, Weak},
    time::Instant,
};
use strings::Notice;

pub mod settings;
mod strings;

/// How often the line is looked up while the music plays. Cheap: a binary search, and the page is only
/// touched when the line changes.
const FOLLOW_SECONDS: f64 = 0.2;

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    /// Show lyrics on the media tab.
    pub enabled: bool,
    /// Ask lrclib.net for lyrics that are not on this machine yet. Sends the artist, track and album names and
    /// the length of the track.
    pub fetch: bool,
}

/// The running feature. Created by `attach`, kept alive by the app context.
pub struct Feature {
    me: Weak<Feature>,
    media: Rc<MediaFeature>,
    service: Service,
    config: RefCell<Config>,
    /// The last look at the player, from which the position is carried forward.
    sample: RefCell<Option<Sample>>,
    visible: Cell<bool>,
    follow: RefCell<Option<app::Timer>>,
}

/// Starts the feature; `None` when it cannot run (no notch to put a page in, for example).
pub fn attach(env: &AttachEnv) -> Option<Rc<Feature>> {
    let media = Rc::clone(env.media?);
    let cache = Cache::new(env.home.join("lyrics-cache"));
    let feature = Rc::new_cyclic(|me| Feature {
        me: me.clone(),
        media,
        service: Service::new(cache),
        config: RefCell::new(env.config.lyrics.clone()),
        sample: RefCell::default(),
        visible: Cell::new(false),
        follow: RefCell::default(),
    });
    let (shown, hidden) = (Rc::downgrade(&feature), Rc::downgrade(&feature));
    feature.media.observe(
        move |sample| {
            if let Some(feature) = shown.upgrade() {
                feature.visible.set(true);
                *feature.sample.borrow_mut() = sample.cloned();
                feature.render();
            }
        },
        move || {
            if let Some(feature) = hidden.upgrade() {
                feature.visible.set(false);
                feature.follow.borrow_mut().take();
            }
        },
    );
    Some(feature)
}

/// What the lyrics area should show.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Shown {
    Hidden,
    /// Lyrics are on but nothing is playing.
    Empty,
    Notice(Notice),
    Synced {
        previous: String,
        current: String,
        next: String,
    },
    Plain(String),
}

#[derive(Debug, PartialEq, Eq)]
struct Plan {
    shown: Shown,
    /// True while the sung line moves by itself: synced lyrics on screen and the music playing.
    follow: bool,
}

/// The players lyrics are looked up for. Whatever else plays (a video in a browser, a podcast app) has a title that is
/// not a song, which would only be sent to the lyrics service for nothing.
const MUSIC_APPS: [&str; 2] = ["com.apple.Music", "com.spotify.client"];

/// Decides what to show; `lookup` answers for a track, given whether the network may be used.
fn plan(
    config: &Config,
    sample: Option<&Sample>,
    now: Instant,
    lookup: impl FnOnce(&Query, bool) -> Status,
) -> Plan {
    let still = |shown| Plan {
        shown,
        follow: false,
    };
    if !config.enabled {
        return still(Shown::Hidden);
    }
    let Some(sample) = sample else {
        return still(Shown::Empty);
    };
    let track = &sample.track;
    if !MUSIC_APPS.contains(&track.app.as_str()) {
        return still(Shown::Hidden);
    }
    let Some(query) = Query::new(&track.artist, &track.title, &track.album, track.duration_ms)
    else {
        return still(Shown::Notice(Notice::Missing));
    };
    match lookup(&query, config.fetch) {
        Status::Offline => still(Shown::Notice(Notice::Off)),
        Status::Fetching => still(Shown::Notice(Notice::Fetching)),
        Status::Missing => still(Shown::Notice(Notice::Missing)),
        Status::Failed => still(Shown::Notice(Notice::Failed)),
        Status::Ready(ready) => {
            if let Some(lrc) = &ready.synced {
                let position = i64::try_from(sample.position_ms(now)).unwrap_or(0);
                let line = lrc.view(position);
                return Plan {
                    shown: Shown::Synced {
                        previous: line.previous.to_owned(),
                        current: line.current.to_owned(),
                        next: line.next.to_owned(),
                    },
                    follow: track.playing,
                };
            }
            still(match &ready.plain {
                Some(text) => Shown::Plain(text.clone()),
                None => Shown::Notice(Notice::Instrumental),
            })
        }
    }
}

impl Feature {
    /// Applies the settings after a change. An error is shown on the settings page and the change is undone.
    pub fn configure(&self, config: &Config) -> Result<(), Fault> {
        *self.config.borrow_mut() = config.clone();
        if self.visible.get() {
            self.render();
        }
        Ok(())
    }

    /// Releases the global shortcuts while the settings page records a new one.
    pub fn suspend_hotkeys(&self) {}

    /// Registers the global shortcuts again from the config.
    pub fn resume_hotkeys(&self, _config: &Config) {}

    /// Facts only the running app knows; the page needs none.
    pub fn runtime(&self) -> Value {
        Value::Null
    }

    /// A button of the settings page; this feature has none.
    pub fn action(&self, _name: &str, _config: &mut Config) -> Option<Result<(), Fault>> {
        None
    }

    /// Entries for the menu bar item; this feature has none.
    pub fn menu_entries(_this: &Rc<Self>) -> Vec<Entry> {
        Vec::new()
    }

    /// Draws the lyrics area for the track and the moment the player was last seen at. Returns whether the line
    /// has to be followed with a timer.
    fn draw(&self, config: &Config, sample: Option<&Sample>) -> bool {
        let plan = plan(config, sample, Instant::now(), |query, online| {
            self.service.lookup(query, online)
        });
        let language = i18n::current();
        let view = &self.media.view;
        match &plan.shown {
            Shown::Hidden => view.show_lyrics(&Lyrics::Hidden),
            Shown::Empty => view.show_lyrics(&Lyrics::Message {
                title: "",
                detail: "",
            }),
            Shown::Notice(notice) => {
                let (title, detail) = language.lyrics_notice(*notice);
                view.show_lyrics(&Lyrics::Message { title, detail });
            }
            Shown::Synced {
                previous,
                current,
                next,
            } => view.show_lyrics(&Lyrics::Synced {
                previous,
                current,
                next,
            }),
            Shown::Plain(text) => view.show_lyrics(&Lyrics::Plain(text)),
        }
        plan.follow
    }

    fn render(&self) {
        let config = self.config.borrow().clone();
        let sample = self.sample.borrow().clone();
        let follow = self.draw(&config, sample.as_ref());
        self.follow_while(follow);
    }

    /// Runs the timer that keeps the line in step exactly while it has work to do.
    fn follow_while(&self, needed: bool) {
        let mut follow = self.follow.borrow_mut();
        match (needed, follow.is_some()) {
            (true, false) => {
                let me = self.me.clone();
                *follow = Some(app::every(FOLLOW_SECONDS, move || {
                    if let Some(feature) = me.upgrade() {
                        feature.render();
                    }
                }));
            }
            (false, true) => {
                // Dropped after the borrow ends: this may run inside the timer's own callback.
                let timer = follow.take();
                drop(follow);
                drop(timer);
            }
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use core_lyrics::{lrc, service::Ready};
    use macos_sys::now_playing::Track;
    use std::{sync::Arc, time::Duration};

    fn sample(playing: bool, position_ms: u64, artist: &str) -> (Sample, Instant) {
        let at = Instant::now();
        let track = Track {
            app: "com.apple.Music".into(),
            app_name: "Music".into(),
            title: "Song".into(),
            artist: artist.into(),
            album: "Album".into(),
            playing,
            rate: if playing { 1.0 } else { 0.0 },
            position_ms,
            duration_ms: 200_000,
            artwork: None,
        };
        (Sample { track, at }, at)
    }

    fn synced() -> Status {
        Status::Ready(Arc::new(Ready {
            synced: Some(lrc::parse("[00:01.00]one\n[00:05.00]two\n[00:09.00]three")),
            plain: Some("one\ntwo\nthree".into()),
            instrumental: false,
        }))
    }

    fn on() -> Config {
        Config {
            enabled: true,
            fetch: true,
        }
    }

    #[test]
    fn off_means_hidden_and_nobody_is_asked() {
        let (s, at) = sample(true, 0, "Band");
        let p = plan(&Config::default(), Some(&s), at, |_, _| {
            panic!("a lookup while lyrics are off")
        });
        assert_eq!(
            p,
            Plan {
                shown: Shown::Hidden,
                follow: false
            }
        );
    }

    #[test]
    fn nothing_playing_or_no_artist_is_an_honest_empty_state() {
        let p = plan(&on(), None, Instant::now(), |_, _| panic!("no track"));
        assert_eq!(p.shown, Shown::Empty);
        let (s, at) = sample(true, 0, "  ");
        let p = plan(&on(), Some(&s), at, |_, _| {
            panic!("no artist, nothing to ask")
        });
        assert_eq!(p.shown, Shown::Notice(Notice::Missing));
    }

    #[test]
    fn what_plays_in_a_browser_is_never_looked_up() {
        let (mut s, at) = sample(true, 0, "Channel");
        s.track.app = "com.microsoft.edgemac".into();
        let p = plan(&on(), Some(&s), at, |_, _| {
            panic!("a video title must not reach the lyrics service")
        });
        assert_eq!(p.shown, Shown::Hidden);
    }

    #[test]
    fn the_online_switch_reaches_the_lookup() {
        let (s, at) = sample(true, 0, "Band");
        for fetch in [false, true] {
            let config = Config {
                enabled: true,
                fetch,
            };
            let mut seen = None;
            plan(&config, Some(&s), at, |q, online| {
                seen = Some((q.artist.clone(), q.duration_secs, online));
                Status::Offline
            });
            assert_eq!(seen, Some(("Band".into(), Some(200), fetch)));
        }
    }

    #[test]
    fn every_status_has_its_notice() {
        let (s, at) = sample(true, 0, "Band");
        for (status, notice) in [
            (Status::Offline, Notice::Off),
            (Status::Fetching, Notice::Fetching),
            (Status::Missing, Notice::Missing),
            (Status::Failed, Notice::Failed),
        ] {
            let p = plan(&on(), Some(&s), at, |_, _| status.clone());
            assert_eq!(
                p,
                Plan {
                    shown: Shown::Notice(notice),
                    follow: false
                }
            );
        }
        let instrumental = Status::Ready(Arc::new(Ready {
            synced: None,
            plain: None,
            instrumental: true,
        }));
        let p = plan(&on(), Some(&s), at, |_, _| instrumental);
        assert_eq!(p.shown, Shown::Notice(Notice::Instrumental));
    }

    #[test]
    fn the_line_follows_the_position_carried_forward_while_playing() {
        let (s, at) = sample(true, 4_500, "Band");
        let line = |now| match plan(&on(), Some(&s), now, |_, _| synced()).shown {
            Shown::Synced { current, .. } => current,
            other => panic!("{other:?}"),
        };
        assert_eq!(line(at), "one");
        assert_eq!(line(at + Duration::from_millis(600)), "two");
        let p = plan(&on(), Some(&s), at, |_, _| synced());
        assert_eq!(
            p.shown,
            Shown::Synced {
                previous: String::new(),
                current: "one".into(),
                next: "two".into()
            }
        );
        assert!(p.follow);
    }

    #[test]
    fn a_paused_track_stays_on_its_line_and_needs_no_timer() {
        let (s, at) = sample(false, 4_500, "Band");
        let p = plan(&on(), Some(&s), at + Duration::from_secs(30), |_, _| {
            synced()
        });
        assert!(matches!(&p.shown, Shown::Synced { current, .. } if current == "one"));
        assert!(!p.follow);
    }

    #[test]
    fn lyrics_without_timing_are_plain_text_and_need_no_timer() {
        let (s, at) = sample(true, 0, "Band");
        let plain = Status::Ready(Arc::new(Ready {
            synced: None,
            plain: Some("la la".into()),
            instrumental: false,
        }));
        let p = plan(&on(), Some(&s), at, |_, _| plain);
        assert_eq!(
            p,
            Plan {
                shown: Shown::Plain("la la".into()),
                follow: false
            }
        );
    }
}
