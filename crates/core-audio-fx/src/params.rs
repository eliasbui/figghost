//! Settings shared between the interface and the audio thread. Every field is an atomic, so the audio
//! thread reads them without locks; values are clamped on the way in, so it never sees a bad one.

use crate::eq::{BANDS, MAX_BAND_DB};
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};

/// Highest volume: four times, +12 dB, for audio that is too quiet even at full system volume.
pub const MAX_GAIN: f32 = 4.0;
const MAX_WIDTH: f32 = 2.0;

struct AtomicF32(AtomicU32);

impl AtomicF32 {
    const fn new(value: f32) -> Self {
        Self(AtomicU32::new(value.to_bits()))
    }

    fn get(&self) -> f32 {
        f32::from_bits(self.0.load(Ordering::Relaxed))
    }

    fn set(&self, value: f32) {
        self.0.store(value.to_bits(), Ordering::Relaxed);
    }
}

/// `value` limited to `min..=max`; not-a-number becomes `fallback`.
fn sane(value: f32, min: f32, max: f32, fallback: f32) -> f32 {
    if value.is_nan() {
        fallback
    } else {
        value.clamp(min, max)
    }
}

/// One set of effects: the settings of one app, or of one output device.
pub struct Params {
    volume: AtomicF32,
    muted: AtomicBool,
    /// -1 is left only, 1 is right only.
    balance: AtomicF32,
    /// 0 is mono, 1 unchanged, 2 twice as wide.
    width: AtomicF32,
    eq_enabled: AtomicBool,
    eq_gains: [AtomicF32; BANDS],
    /// Magic Boost amount, 0 to 1.
    boost: AtomicF32,
    night: AtomicBool,
    /// Bumped after every change that needs filters or the compressor rebuilt.
    revision: AtomicU32,
}

impl Default for Params {
    fn default() -> Self {
        Self {
            volume: AtomicF32::new(1.0),
            muted: AtomicBool::new(false),
            balance: AtomicF32::new(0.0),
            width: AtomicF32::new(1.0),
            eq_enabled: AtomicBool::new(false),
            eq_gains: std::array::from_fn(|_| AtomicF32::new(0.0)),
            boost: AtomicF32::new(0.0),
            night: AtomicBool::new(false),
            revision: AtomicU32::new(0),
        }
    }
}

impl Params {
    fn changed(&self) {
        self.revision.fetch_add(1, Ordering::Release);
    }

    pub(crate) fn revision(&self) -> u32 {
        self.revision.load(Ordering::Acquire)
    }

    pub fn volume(&self) -> f32 {
        self.volume.get()
    }

    /// 1.0 is unchanged; up to `MAX_GAIN`.
    pub fn set_volume(&self, volume: f32) {
        self.volume.set(sane(volume, 0.0, MAX_GAIN, 1.0));
    }

    pub fn muted(&self) -> bool {
        self.muted.load(Ordering::Relaxed)
    }

    pub fn set_muted(&self, muted: bool) {
        self.muted.store(muted, Ordering::Relaxed);
    }

    pub fn balance(&self) -> f32 {
        self.balance.get()
    }

    pub fn set_balance(&self, balance: f32) {
        self.balance.set(sane(balance, -1.0, 1.0, 0.0));
    }

    pub fn width(&self) -> f32 {
        self.width.get()
    }

    pub fn set_width(&self, width: f32) {
        self.width.set(sane(width, 0.0, MAX_WIDTH, 1.0));
    }

    pub fn eq_enabled(&self) -> bool {
        self.eq_enabled.load(Ordering::Relaxed)
    }

    pub fn set_eq_enabled(&self, enabled: bool) {
        self.eq_enabled.store(enabled, Ordering::Relaxed);
        self.changed();
    }

    pub fn eq_gains(&self) -> [f32; BANDS] {
        std::array::from_fn(|band| self.eq_gains[band].get())
    }

    pub fn set_eq_gains(&self, gains: [f32; BANDS]) {
        for (slot, gain) in self.eq_gains.iter().zip(gains) {
            slot.set(sane(gain, -MAX_BAND_DB, MAX_BAND_DB, 0.0));
        }
        self.changed();
    }

    pub fn boost(&self) -> f32 {
        self.boost.get()
    }

    pub fn set_boost(&self, boost: f32) {
        self.boost.set(sane(boost, 0.0, 1.0, 0.0));
        self.changed();
    }

    pub fn night(&self) -> bool {
        self.night.load(Ordering::Relaxed)
    }

    pub fn set_night(&self, night: bool) {
        self.night.store(night, Ordering::Relaxed);
        self.changed();
    }

    /// True when these settings leave the audio exactly as it was, so nothing needs to process it.
    pub fn is_neutral(&self) -> bool {
        self.volume() == 1.0
            && !self.muted()
            && self.balance() == 0.0
            && self.width() == 1.0
            && (!self.eq_enabled() || self.eq_gains() == [0.0; BANDS])
            && self.boost() == 0.0
            && !self.night()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_are_neutral() {
        assert!(Params::default().is_neutral());
    }

    #[test]
    fn bad_values_are_clamped_or_replaced() {
        let p = Params::default();
        p.set_volume(f32::NAN);
        assert_eq!(p.volume(), 1.0);
        p.set_volume(100.0);
        assert_eq!(p.volume(), MAX_GAIN);
        p.set_volume(-1.0);
        assert_eq!(p.volume(), 0.0);
        p.set_balance(f32::INFINITY);
        assert_eq!(p.balance(), 1.0);
        p.set_width(f32::NAN);
        assert_eq!(p.width(), 1.0);
        p.set_boost(7.0);
        assert_eq!(p.boost(), 1.0);
        let mut gains = [0.0; BANDS];
        gains[0] = 99.0;
        gains[1] = f32::NAN;
        gains[2] = -99.0;
        p.set_eq_gains(gains);
        assert_eq!(&p.eq_gains()[..3], &[MAX_BAND_DB, 0.0, -MAX_BAND_DB]);
    }

    #[test]
    fn a_disabled_equalizer_is_still_neutral_whatever_its_curve() {
        let p = Params::default();
        p.set_eq_gains([3.0; BANDS]);
        assert!(p.is_neutral());
        p.set_eq_enabled(true);
        assert!(!p.is_neutral());
    }

    #[test]
    fn filter_changes_bump_the_revision_and_volume_changes_do_not() {
        let p = Params::default();
        let start = p.revision();
        p.set_volume(0.5);
        p.set_muted(true);
        assert_eq!(p.revision(), start);
        p.set_eq_gains([1.0; BANDS]);
        p.set_boost(0.5);
        p.set_night(true);
        p.set_eq_enabled(true);
        assert_eq!(p.revision(), start + 4);
    }
}
