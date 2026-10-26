//! The media tab: transport buttons that press the real media keys, which every player and browser honours, a bar
//! that moves what plays to where it is dragged (through the Now Playing adapter), and what plays in whichever app
//! plays it. Lyrics (see `features/lyrics`) hang on it through `observe`, so the player
//! is asked once for everything, and the rest of the app can follow what plays through `watch` and `current`.
//!
//! The watcher's stream of the system's Now Playing costs nothing while nothing changes, so it runs from the start
//! and `watch` hears about music that starts while the notch is closed. When the stream cannot run the watcher polls
//! Music and Spotify instead, and that only while the tab is on screen.

use macos_sys::{
    MainThreadMarker, app,
    media_ui::{MediaCallbacks, MediaView},
    notch::Notch,
    now_playing::{self, Sample, Watcher},
    system_keys::{self, SystemKey},
};
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
};

/// Moves the progress bar and the times along while the tab is visible; changes arrive by themselves.
const REFRESH_SECONDS: f64 = 0.5;
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
    watcher: Rc<Watcher>,
    observer: Rc<RefCell<Option<Observer>>>,
    listener: Rc<RefCell<Option<Refreshed>>>,
}

impl MediaFeature {
    pub fn attach(mtm: MainThreadMarker, notch: &Notch) -> Rc<Self> {
        let watcher = Rc::new(Watcher::start());
        // Weak: the buttons belong to the view, which the watcher's listener reaches.
        let press = |key: SystemKey| {
            let watcher = Rc::downgrade(&watcher);
            move || {
                system_keys::post(key);
                let watcher = watcher.clone();
                app::after(REFRESH_AFTER_PRESS_SECONDS, move || {
                    if let Some(watcher) = watcher.upgrade() {
                        watcher.poll_now();
                    }
                });
            }
        };
        let view = MediaView::new(
            mtm,
            notch.page_frame(),
            MediaCallbacks {
                on_previous: Box::new(press(SystemKey::Previous)),
                on_play_pause: Box::new(press(SystemKey::PlayPause)),
                on_next: Box::new(press(SystemKey::Next)),
                on_seek: Box::new({
                    let watcher = Rc::downgrade(&watcher);
                    move |ms| {
                        if !now_playing::seek(ms as f64 / 1000.0) {
                            eprintln!("media: cannot seek, the Now Playing adapter is missing");
                            return;
                        }
                        let watcher = watcher.clone();
                        app::after(REFRESH_AFTER_PRESS_SECONDS, move || {
                            if let Some(watcher) = watcher.upgrade() {
                                watcher.poll_now();
                            }
                        });
                    }
                }),
            },
        );

        notch.set_on_rest_step({
            let watcher = Rc::downgrade(&watcher);
            let (previous, next) = (press(SystemKey::Previous), press(SystemKey::Next));
            move |delta| {
                let playing = watcher.upgrade().is_some_and(|w| w.sample().is_some());
                if playing {
                    if delta < 0 { previous() } else { next() }
                }
                playing
            }
        });

        let observer: Rc<RefCell<Option<Observer>>> = Rc::default();
        let listener: Rc<RefCell<Option<Refreshed>>> = Rc::default();
        let visible = Rc::new(Cell::new(false));
        let refresh = {
            let (view, observer) = (Rc::clone(&view), Rc::clone(&observer));
            Rc::new(move |sample: Option<&Sample>| {
                view.show(sample);
                if let Some(observer) = observer.borrow().as_ref() {
                    (observer.refreshed)(sample);
                }
            })
        };
        {
            // Weak: the watcher keeps this closure until it is dropped.
            let watcher_ref = Rc::downgrade(&watcher);
            let (refresh, visible, listener) = (
                Rc::clone(&refresh),
                Rc::clone(&visible),
                Rc::clone(&listener),
            );
            watcher.set_on_change(move || {
                let Some(watcher) = watcher_ref.upgrade() else {
                    return;
                };
                if !visible.get() && !watcher.streams() {
                    // The stream gave way to polling while the tab was away: poll only while it shows.
                    watcher.set_active(false);
                }
                let sample = watcher.sample();
                if visible.get() {
                    refresh(sample.as_ref());
                }
                if let Some(listener) = listener.borrow().as_ref() {
                    listener(sample.as_ref());
                }
            });
        }
        if watcher.streams() {
            watcher.set_active(true);
        }
        let timer: Rc<RefCell<Option<app::Timer>>> = Rc::default();
        let on_show = {
            let (watcher, refresh, timer, visible) = (
                Rc::clone(&watcher),
                Rc::clone(&refresh),
                Rc::clone(&timer),
                Rc::clone(&visible),
            );
            move || {
                visible.set(true);
                watcher.set_active(true);
                refresh(watcher.sample().as_ref());
                let (watcher, refresh) = (Rc::clone(&watcher), Rc::clone(&refresh));
                *timer.borrow_mut() = Some(app::every(REFRESH_SECONDS, move || {
                    refresh(watcher.sample().as_ref())
                }));
            }
        };
        let on_hide = {
            let (watcher, timer, observer, visible) = (
                Rc::clone(&watcher),
                Rc::clone(&timer),
                Rc::clone(&observer),
                Rc::clone(&visible),
            );
            move || {
                visible.set(false);
                if !watcher.streams() {
                    watcher.set_active(false);
                }
                timer.borrow_mut().take();
                if let Some(observer) = observer.borrow().as_ref() {
                    (observer.hidden)();
                }
            }
        };
        notch.add_page(view.page(on_show, on_hide));
        Rc::new(Self {
            view,
            watcher,
            observer,
            listener,
        })
    }

    /// What plays now, as last heard; `None` when nothing does or nothing was heard yet.
    pub fn current(&self) -> Option<Sample> {
        self.watcher.sample()
    }

    /// `f` runs on the main thread each time what plays changes, whether or not the tab is visible. One listener at
    /// a time. Without the stream (it could not run here) changes are only seen while the tab is visible.
    pub fn watch(&self, f: impl Fn(Option<&Sample>) + 'static) {
        *self.listener.borrow_mut() = Some(Box::new(f));
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
