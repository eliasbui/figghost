//! The media tab: transport buttons that press the real media keys, and a track display for Music and
//! Spotify that only polls while the tab is on screen. Lyrics (see `features/lyrics`) hang on it through
//! `observe`, so the player is asked once for everything.

use macos_sys::{
    MainThreadMarker, app,
    media_ui::{MediaCallbacks, MediaView},
    notch::Notch,
    now_playing::{Sample, Watcher},
    system_keys::{self, SystemKey},
};
use std::{cell::RefCell, rc::Rc};

const REFRESH_SECONDS: f64 = 0.7;
/// Players take a moment to update after a key press.
const REFRESH_AFTER_PRESS_SECONDS: f64 = 0.5;

/// What the tab tells a listener: a fresh look at the player while the tab is visible, and when it goes away.
type Refreshed = Box<dyn Fn(Option<&Sample>)>;

struct Observer {
    refreshed: Refreshed,
    hidden: Box<dyn Fn()>,
}

/// Keeps the tab and the background watcher alive.
pub struct MediaFeature {
    pub view: Rc<MediaView>,
    _watcher: Rc<Watcher>,
    observer: Rc<RefCell<Option<Observer>>>,
}

impl MediaFeature {
    pub fn attach(mtm: MainThreadMarker, notch: &Notch) -> Rc<Self> {
        let watcher = Rc::new(Watcher::start());
        let press = |key: SystemKey| {
            let watcher = Rc::clone(&watcher);
            move || {
                system_keys::post(key);
                let watcher = Rc::clone(&watcher);
                app::after(REFRESH_AFTER_PRESS_SECONDS, move || watcher.poll_now());
            }
        };
        let view = MediaView::new(
            mtm,
            notch.page_frame(),
            MediaCallbacks {
                on_previous: Box::new(press(SystemKey::Previous)),
                on_play_pause: Box::new(press(SystemKey::PlayPause)),
                on_next: Box::new(press(SystemKey::Next)),
            },
        );

        let observer: Rc<RefCell<Option<Observer>>> = Rc::default();
        let refresh = {
            let (watcher, view, observer) =
                (Rc::clone(&watcher), Rc::clone(&view), Rc::clone(&observer));
            Rc::new(move || {
                let sample = watcher.sample();
                view.show(sample.as_ref().map(|s| &s.track));
                if let Some(observer) = observer.borrow().as_ref() {
                    (observer.refreshed)(sample.as_ref());
                }
            })
        };
        let timer: Rc<RefCell<Option<app::Timer>>> = Rc::default();
        let on_show = {
            let (watcher, refresh, timer) =
                (Rc::clone(&watcher), Rc::clone(&refresh), Rc::clone(&timer));
            move || {
                watcher.set_active(true);
                refresh();
                let refresh = Rc::clone(&refresh);
                *timer.borrow_mut() = Some(app::every(REFRESH_SECONDS, move || refresh()));
            }
        };
        let on_hide = {
            let (watcher, timer, observer) =
                (Rc::clone(&watcher), Rc::clone(&timer), Rc::clone(&observer));
            move || {
                watcher.set_active(false);
                timer.borrow_mut().take();
                if let Some(observer) = observer.borrow().as_ref() {
                    (observer.hidden)();
                }
            }
        };
        notch.add_page(view.page(on_show, on_hide));
        Rc::new(Self {
            view,
            _watcher: watcher,
            observer,
        })
    }

    /// `refreshed` runs each time the tab looks at the player, which is only while the tab is visible;
    /// `hidden` runs when it goes away. One listener at a time.
    pub fn observe(
        &self,
        refreshed: impl Fn(Option<&Sample>) + 'static,
        hidden: impl Fn() + 'static,
    ) {
        *self.observer.borrow_mut() = Some(Observer {
            refreshed: Box::new(refreshed),
            hidden: Box::new(hidden),
        });
    }
}
