//! The media tab: transport buttons that press the real media keys, and a track display for Music and
//! Spotify that only polls while the tab is on screen.

use macos_sys::{
    MainThreadMarker, app,
    media_ui::{MediaCallbacks, MediaView},
    notch::Notch,
    now_playing::Watcher,
    system_keys::{self, SystemKey},
};
use std::{cell::RefCell, rc::Rc};

const REFRESH_SECONDS: f64 = 0.7;
/// Players take a moment to update after a key press.
const REFRESH_AFTER_PRESS_SECONDS: f64 = 0.5;

/// Keeps the tab and the background watcher alive.
pub struct MediaFeature {
    _view: Rc<MediaView>,
    _watcher: Rc<Watcher>,
}

impl MediaFeature {
    pub fn attach(mtm: MainThreadMarker, notch: &Notch) -> Self {
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

        let timer: Rc<RefCell<Option<app::Timer>>> = Rc::default();
        let on_show = {
            let (watcher, view, timer) = (Rc::clone(&watcher), Rc::clone(&view), Rc::clone(&timer));
            move || {
                watcher.set_active(true);
                view.show(watcher.latest().as_ref());
                let (watcher, view) = (Rc::clone(&watcher), Rc::clone(&view));
                *timer.borrow_mut() = Some(app::every(REFRESH_SECONDS, move || {
                    view.show(watcher.latest().as_ref());
                }));
            }
        };
        let on_hide = {
            let (watcher, timer) = (Rc::clone(&watcher), Rc::clone(&timer));
            move || {
                watcher.set_active(false);
                timer.borrow_mut().take();
            }
        };
        notch.add_page(view.page(on_show, on_hide));
        Self {
            _view: view,
            _watcher: watcher,
        }
    }
}
