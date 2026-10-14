//! Replaces the system volume indicator: while enabled, the volume and mute keys are handled here and
//! shown in a notch-style HUD. Swallowing keys needs an active event tap, so this needs Accessibility.

use crate::trace;
use macos_sys::{
    MainThreadMarker, app,
    event_tap::{EventTap, EventView, TapError, Verdict, mask},
    hud::{self, Hud},
    system_keys::{SYSTEM_DEFINED, SystemKey},
    volume::{self, Volume},
};
use std::{cell::RefCell, mem, rc::Rc};

const RETRY_SECONDS: f64 = 3.0;
/// Shift and Option held together select the fine step, as in the system.
const FINE_STEP_FLAGS: u64 = 0x0002_0000 | 0x0008_0000;

pub struct VolumeHud {
    _hud: Rc<Hud>,
    tap: Rc<RefCell<Option<EventTap>>>,
    retry: Rc<RefCell<Option<app::Timer>>>,
}

impl VolumeHud {
    pub fn new(mtm: MainThreadMarker) -> Self {
        Self {
            _hud: Hud::new(mtm),
            tap: Rc::default(),
            retry: Rc::default(),
        }
    }

    pub fn is_enabled(&self) -> bool {
        self.tap.borrow().is_some()
    }

    /// Starts or stops handling the volume keys. Starting fails without Accessibility access.
    pub fn set_enabled(&self, enabled: bool) -> Result<(), TapError> {
        self.retry.borrow_mut().take();
        if !enabled {
            self.tap.borrow_mut().take();
            return Ok(());
        }
        if !self.is_enabled() {
            *self.tap.borrow_mut() = Some(start_tap()?);
        }
        Ok(())
    }

    /// Like `set_enabled(true)`, but keeps trying every few seconds until access is granted.
    pub fn enable_when_possible(&self) {
        if self.set_enabled(true).is_ok() {
            return;
        }
        let (tap, retry) = (Rc::clone(&self.tap), Rc::clone(&self.retry));
        *self.retry.borrow_mut() = Some(app::every(RETRY_SECONDS, move || {
            if let Ok(started) = start_tap() {
                *tap.borrow_mut() = Some(started);
                retry.borrow_mut().take();
            }
        }));
    }
}

fn start_tap() -> Result<EventTap, TapError> {
    let mut swallowing = false;
    EventTap::spawn(mask(SYSTEM_DEFINED), move |_, event| {
        decide(&mut swallowing, event)
    })
}

/// Runs inside the input path, so it only touches CoreAudio and hands the drawing to the main thread.
fn decide(swallowing: &mut bool, event: &EventView<'_>) -> Verdict {
    let Some(press) = event.system_key() else {
        return Verdict::Pass;
    };
    if !matches!(
        press.key,
        SystemKey::SoundUp | SystemKey::SoundDown | SystemKey::Mute
    ) {
        return Verdict::Pass;
    }
    let verdict = |swallow: bool| {
        if swallow {
            Verdict::Drop
        } else {
            Verdict::Pass
        }
    };
    // A release, or a held mute key repeating, goes the way the press that started it went.
    if !press.down {
        return verdict(mem::take(swallowing));
    }
    if press.repeat && press.key == SystemKey::Mute {
        return verdict(*swallowing);
    }
    let fine = event.flags() & FINE_STEP_FLAGS == FINE_STEP_FLAGS;
    *swallowing = false;
    // Anything this app cannot change (an HDMI speaker, say) is left to the system and its own indicator.
    let Some(next) = handle_press(press.key, fine) else {
        trace::log(&format!("volume hud: {:?} left to the system", press.key));
        return Verdict::Pass;
    };
    *swallowing = true;
    trace::log(&format!("volume hud: {:?} -> {next:?}", press.key));
    hud::show_from_any_thread(next.level, next.muted);
    Verdict::Drop
}

/// The state a key press leads to; `None` for any other key.
fn target(key: SystemKey, current: Volume, fine: bool) -> Option<Volume> {
    match key {
        SystemKey::Mute => Some(Volume {
            muted: !current.muted,
            ..current
        }),
        // A volume key also lifts a mute, as it does in the system.
        SystemKey::SoundUp | SystemKey::SoundDown => Some(Volume {
            level: volume::step_level(current.level, key == SystemKey::SoundUp, fine),
            muted: false,
        }),
        _ => None,
    }
}

fn handle_press(key: SystemKey, fine: bool) -> Option<Volume> {
    let current = volume::read()?;
    let next = target(key, current, fine)?;
    // Level first: unmuting then happens at the new level, never at the old one.
    if next.level != current.level && !volume::set_level(next.level) {
        return None;
    }
    if next.muted != current.muted && !volume::set_muted(next.muted) {
        return None;
    }
    Some(next)
}

#[cfg(test)]
mod tests {
    use super::*;

    const HALF: Volume = Volume {
        level: 0.5,
        muted: false,
    };

    #[test]
    fn volume_keys_step_the_level() {
        let up = target(SystemKey::SoundUp, HALF, false).unwrap();
        assert!((up.level - 0.5625).abs() < 1e-6 && !up.muted);
        let down = target(SystemKey::SoundDown, HALF, false).unwrap();
        assert!((down.level - 0.4375).abs() < 1e-6 && !down.muted);
    }

    #[test]
    fn a_volume_key_lifts_the_mute() {
        let muted = Volume {
            level: 0.5,
            muted: true,
        };
        assert!(!target(SystemKey::SoundUp, muted, false).unwrap().muted);
        assert!(!target(SystemKey::SoundDown, muted, false).unwrap().muted);
    }

    #[test]
    fn mute_toggles_without_touching_the_level() {
        let muted = target(SystemKey::Mute, HALF, false).unwrap();
        assert_eq!(
            muted,
            Volume {
                level: 0.5,
                muted: true
            }
        );
        assert_eq!(target(SystemKey::Mute, muted, false).unwrap(), HALF);
    }

    #[test]
    fn other_keys_are_not_ours() {
        for key in [
            SystemKey::PlayPause,
            SystemKey::Next,
            SystemKey::Previous,
            SystemKey::Other(3),
        ] {
            assert_eq!(target(key, HALF, false), None);
        }
    }
}
