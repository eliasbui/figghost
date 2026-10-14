//! Scroll engine wiring: shared settings, frontmost-app tracking, and the thread that starts the event
//! tap as soon as Accessibility access exists.

use core_mouse::{Control, Engine, Error, Settings};
use macos_sys::{permissions, workspace};
use std::{sync::Arc, thread, time::Duration};

pub struct Mouse {
    pub control: Arc<Control>,
    _frontmost: workspace::Observer,
}

impl Mouse {
    pub fn start(settings: Settings) -> Self {
        let control = Control::new(settings);
        if let Some(front) = workspace::frontmost_app() {
            control.set_frontmost(front.bundle_id);
        }
        let frontmost = {
            let control = Arc::clone(&control);
            workspace::observe_frontmost(move |front| control.set_frontmost(front.bundle_id))
        };
        spawn_engine(Arc::clone(&control));
        Self {
            control,
            _frontmost: frontmost,
        }
    }
}

/// Keeps the engine alive for the process; without access it retries every two seconds.
fn spawn_engine(control: Arc<Control>) {
    thread::spawn(move || {
        let mut asked = false;
        loop {
            match Engine::start(Arc::clone(&control)) {
                Ok(_engine) => loop {
                    thread::park();
                },
                Err(Error::NotTrusted) => {
                    if !asked {
                        asked = true;
                        permissions::accessibility_trusted(true);
                        eprintln!(
                            "Accessibility access is off. Enable this app (or its terminal) under System Settings > \
                             Privacy & Security > Accessibility; scrolling starts once granted."
                        );
                    }
                    thread::sleep(Duration::from_secs(2));
                }
                Err(e) => {
                    eprintln!("scroll engine failed: {e:?}");
                    return;
                }
            }
        }
    });
}
