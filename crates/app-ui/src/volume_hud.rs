//! Replaces the system volume indicator: while enabled, the volume and mute keys are handled here and
//! shown in a notch-style HUD. Swallowing keys needs an active event tap, so this needs Accessibility.
//!
//! When the default output has no volume of its own (HDMI and DisplayPort speakers), the keys move the
//! mixer's software volume instead, even with the indicator switched off: the system could do nothing
//! with them there.

use crate::{audio::SoftVolume, trace};
use macos_sys::{
    MainThreadMarker, app,
    event_tap::{EventTap, EventView, TapError, Verdict, mask},
    hud::{self, Hud, HudPosition, HudStyle},
    system_keys::{SYSTEM_DEFINED, SystemKey},
    volume::{self, Volume},
};
use std::{
    cell::{Cell, RefCell},
    mem,
    rc::Rc,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
};

const RETRY_SECONDS: f64 = 3.0;
/// Shift and Option held together select the fine step, as in the system.
const FINE_STEP_FLAGS: u64 = 0x0002_0000 | 0x0008_0000;

pub struct VolumeHud {
    _hud: Rc<Hud>,
    tap: Rc<RefCell<Option<EventTap>>>,
    retry: Rc<RefCell<Option<app::Timer>>>,
    /// The indicator for devices with a volume of their own: the user's switch.
    hardware: Arc<AtomicBool>,
    soft: Arc<SoftVolume>,
    /// The default output has no volume of its own, so the keys belong to the software volume.
    soft_keys: Cell<bool>,
}

impl VolumeHud {
    pub fn new(mtm: MainThreadMarker, soft: Arc<SoftVolume>, style: HudStyle) -> Self {
        Self {
            _hud: Hud::with_style(mtm, style),
            tap: Rc::default(),
            retry: Rc::default(),
            hardware: Arc::default(),
            soft,
            soft_keys: Cell::new(false),
        }
    }

    pub fn is_enabled(&self) -> bool {
        self.hardware.load(Ordering::Relaxed)
    }

    /// The look of the indicator, as chosen in the settings.
    pub fn set_style(&self, style: HudStyle) {
        self._hud.set_style(style);
    }

    pub fn set_position(&self, position: HudPosition) {
        self._hud.set_position(position);
    }

    /// Shows the indicator with the volume as it is, so a style or a place can be judged without a key press.
    pub fn preview(&self) {
        let now = volume::read().unwrap_or(Volume {
            level: 0.5,
            muted: false,
        });
        self._hud.show(now.level, now.muted);
    }

    fn wanted(&self) -> bool {
        self.is_enabled() || self.soft_keys.get()
    }

    fn start_tap(&self) -> Result<EventTap, TapError> {
        let (hardware, soft) = (Arc::clone(&self.hardware), Arc::clone(&self.soft));
        let mut swallowing = false;
        EventTap::spawn(mask(SYSTEM_DEFINED), move |_, event| {
            decide(&mut swallowing, event, &hardware, &soft)
        })
    }

    /// Starts the key tap when something needs it and stops it when nothing does.
    fn sync(&self) -> Result<(), TapError> {
        if !self.wanted() {
            self.retry.borrow_mut().take();
            self.tap.borrow_mut().take();
            return Ok(());
        }
        if self.tap.borrow().is_none() {
            *self.tap.borrow_mut() = Some(self.start_tap()?);
        }
        Ok(())
    }

    /// Starts or stops the indicator. Starting fails without Accessibility access.
    pub fn set_enabled(&self, enabled: bool) -> Result<(), TapError> {
        self.retry.borrow_mut().take();
        self.hardware.store(enabled, Ordering::Relaxed);
        let result = self.sync();
        if result.is_err() {
            self.hardware.store(false, Ordering::Relaxed);
        }
        result
    }

    /// Like `set_enabled(true)`, but keeps trying every few seconds until access is granted.
    pub fn enable_when_possible(&self) {
        self.hardware.store(true, Ordering::Relaxed);
        self.keep_trying();
    }

    /// Lets the volume keys move the software volume; the mixer calls this whenever the default output
    /// switches between a device with a volume of its own and one without.
    pub fn set_soft_keys(&self, on: bool) {
        self.soft_keys.set(on);
        if on {
            self.keep_trying();
        } else {
            let _ = self.sync();
        }
    }

    fn keep_trying(&self) {
        if self.sync().is_ok() {
            return;
        }
        if self.retry.borrow().is_some() {
            return;
        }
        let (tap, retry) = (Rc::clone(&self.tap), Rc::clone(&self.retry));
        let (hardware, soft) = (Arc::clone(&self.hardware), Arc::clone(&self.soft));
        *self.retry.borrow_mut() = Some(app::every(RETRY_SECONDS, move || {
            let (hardware, soft) = (Arc::clone(&hardware), Arc::clone(&soft));
            let mut swallowing = false;
            if let Ok(started) = EventTap::spawn(mask(SYSTEM_DEFINED), move |_, event| {
                decide(&mut swallowing, event, &hardware, &soft)
            }) {
                *tap.borrow_mut() = Some(started);
                retry.borrow_mut().take();
            }
        }));
    }
}

/// Runs inside the input path, so it only touches CoreAudio and hands the drawing to the main thread.
fn decide(
    swallowing: &mut bool,
    event: &EventView<'_>,
    hardware: &AtomicBool,
    soft: &SoftVolume,
) -> Verdict {
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
    let Some(next) = handle_press(press.key, fine, hardware.load(Ordering::Relaxed), soft) else {
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

fn handle_press(key: SystemKey, fine: bool, hardware: bool, soft: &SoftVolume) -> Option<Volume> {
    let Some(current) = volume::read() else {
        return handle_soft_press(key, fine, soft);
    };
    if !hardware {
        return None;
    }
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

/// A device without a volume of its own: the mixer's software volume moves instead. A level the mixer
/// boosted past 100% is kept by Volume Up rather than pulled back to it.
fn handle_soft_press(key: SystemKey, fine: bool, soft: &SoftVolume) -> Option<Volume> {
    let current = soft.read()?;
    let next = if key == SystemKey::SoundUp && current.level >= 1.0 {
        Volume {
            muted: false,
            ..current
        }
    } else {
        target(key, current, fine)?
    };
    soft.write(next).then_some(next)
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

    fn soft_at(level: f32) -> (SoftVolume, Arc<core_audio_fx::Params>) {
        let soft = SoftVolume::default();
        let params = Arc::new(core_audio_fx::Params::default());
        params.set_volume(level);
        soft.set_target(Some(("hdmi".into(), Arc::clone(&params))));
        (soft, params)
    }

    #[test]
    fn keys_move_the_software_volume_and_keep_a_boost_on_volume_up() {
        let (soft, params) = soft_at(0.5);
        let next = handle_soft_press(SystemKey::SoundDown, false, &soft).unwrap();
        assert_eq!(next.level, 0.4375);
        assert_eq!(params.volume(), 0.4375);
        let (soft, params) = soft_at(1.5);
        let next = handle_soft_press(SystemKey::SoundUp, false, &soft).unwrap();
        assert_eq!((next.level, params.volume()), (1.5, 1.5));
        handle_soft_press(SystemKey::Mute, false, &soft).unwrap();
        assert!(params.muted());
    }

    #[test]
    fn without_a_software_device_the_keys_stay_with_the_system() {
        assert_eq!(
            handle_soft_press(SystemKey::SoundUp, false, &SoftVolume::default()),
            None
        );
    }

    #[test]
    fn other_keys_are_not_ours() {
        for key in [
            SystemKey::PlayPause,
            SystemKey::Next,
            SystemKey::Previous,
            SystemKey::BrightnessDown,
        ] {
            assert_eq!(target(key, HALF, false), None);
        }
    }
}
