//! The 10-band graphic equalizer and its presets.

use crate::biquad::{Biquad, Coefficients};

pub const BANDS: usize = 10;
/// Octave-spaced centre frequencies, as on hardware graphic equalizers.
pub const BAND_HZ: [f64; BANDS] = [
    31.25, 62.5, 125.0, 250.0, 500.0, 1000.0, 2000.0, 4000.0, 8000.0, 16000.0,
];
/// Each band moves at most this far either way.
pub const MAX_BAND_DB: f32 = 12.0;
/// One octave of bandwidth per band.
const Q: f64 = 1.41;
/// Part of the largest boost taken off before the bands, so a boosted curve keeps some headroom and
/// leans less on the limiter. Cuts never raise the level.
const PREAMP_SHARE: f64 = 0.5;
/// A band this close to Nyquist cannot be designed; it is left out at that sample rate.
const MAX_FREQ_SHARE: f64 = 0.45;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Preset {
    Flat,
    BassBoost,
    BassReducer,
    TrebleBoost,
    TrebleReducer,
    Vocal,
    Loudness,
    Rock,
    Pop,
    Electronic,
    HipHop,
    Jazz,
    Classical,
    Acoustic,
    Spoken,
    Movie,
    Gaming,
}

impl Preset {
    pub const ALL: [Preset; 17] = [
        Preset::Flat,
        Preset::BassBoost,
        Preset::BassReducer,
        Preset::TrebleBoost,
        Preset::TrebleReducer,
        Preset::Vocal,
        Preset::Loudness,
        Preset::Rock,
        Preset::Pop,
        Preset::Electronic,
        Preset::HipHop,
        Preset::Jazz,
        Preset::Classical,
        Preset::Acoustic,
        Preset::Spoken,
        Preset::Movie,
        Preset::Gaming,
    ];

    /// Name in the config file.
    pub const fn key(self) -> &'static str {
        match self {
            Preset::Flat => "flat",
            Preset::BassBoost => "bass_boost",
            Preset::BassReducer => "bass_reducer",
            Preset::TrebleBoost => "treble_boost",
            Preset::TrebleReducer => "treble_reducer",
            Preset::Vocal => "vocal",
            Preset::Loudness => "loudness",
            Preset::Rock => "rock",
            Preset::Pop => "pop",
            Preset::Electronic => "electronic",
            Preset::HipHop => "hip_hop",
            Preset::Jazz => "jazz",
            Preset::Classical => "classical",
            Preset::Acoustic => "acoustic",
            Preset::Spoken => "spoken",
            Preset::Movie => "movie",
            Preset::Gaming => "gaming",
        }
    }

    pub fn from_key(key: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|p| p.key() == key)
    }

    /// Band gains in dB, lowest band first.
    pub const fn gains(self) -> [f32; BANDS] {
        match self {
            Preset::Flat => [0.0; BANDS],
            Preset::BassBoost => [6.0, 5.0, 4.0, 2.0, 0.5, 0.0, 0.0, 0.0, 0.0, 0.0],
            Preset::BassReducer => [-6.0, -5.0, -4.0, -2.0, -0.5, 0.0, 0.0, 0.0, 0.0, 0.0],
            Preset::TrebleBoost => [0.0, 0.0, 0.0, 0.0, 0.0, 0.5, 2.0, 4.0, 5.0, 6.0],
            Preset::TrebleReducer => [0.0, 0.0, 0.0, 0.0, 0.0, -0.5, -2.0, -4.0, -5.0, -6.0],
            Preset::Vocal => [-2.0, -2.0, -1.0, 1.0, 3.0, 4.0, 3.0, 1.0, 0.0, -1.0],
            Preset::Loudness => [5.0, 4.0, 2.0, 0.0, -1.0, -1.0, 0.0, 2.0, 4.0, 5.0],
            Preset::Rock => [5.0, 4.0, 2.0, -1.0, -2.0, -1.0, 1.0, 3.0, 4.0, 5.0],
            Preset::Pop => [-1.0, 1.0, 3.0, 4.0, 3.0, 1.0, -1.0, -1.0, 1.0, 2.0],
            Preset::Electronic => [5.0, 4.0, 1.0, 0.0, -2.0, 1.0, 0.0, 1.0, 4.0, 5.0],
            Preset::HipHop => [5.0, 4.0, 2.0, 3.0, -1.0, -1.0, 1.0, -1.0, 2.0, 3.0],
            Preset::Jazz => [3.0, 2.0, 1.0, 2.0, -1.0, -1.0, 0.0, 1.0, 2.0, 3.0],
            Preset::Classical => [4.0, 3.0, 2.0, 1.0, -1.0, -1.0, 0.0, 2.0, 3.0, 4.0],
            Preset::Acoustic => [4.0, 3.0, 2.0, 1.0, 1.0, 1.0, 2.0, 2.0, 3.0, 2.0],
            Preset::Spoken => [-4.0, -3.0, -1.0, 1.0, 3.0, 4.0, 4.0, 2.0, 0.0, -2.0],
            Preset::Movie => [4.0, 3.0, 1.0, 0.0, -1.0, 1.0, 2.0, 3.0, 2.0, 1.0],
            Preset::Gaming => [3.0, 2.0, 0.0, -1.0, 0.0, 2.0, 3.0, 4.0, 3.0, 2.0],
        }
    }

    /// The preset whose curve is exactly `gains`, so a saved curve shows its preset name again.
    pub fn matching(gains: &[f32; BANDS]) -> Option<Self> {
        Self::ALL.into_iter().find(|p| p.gains() == *gains)
    }
}

/// The level change the equalizer applies before its bands, in dB.
pub(crate) fn preamp_db(gains: &[f32; BANDS]) -> f64 {
    let loudest = gains.iter().fold(0.0f32, |m, &g| m.max(g));
    -f64::from(loudest) * PREAMP_SHARE
}

/// Ten peaking filters per channel. Bands at 0 dB, and bands too close to Nyquist, are skipped.
pub(crate) struct Equalizer {
    coefficients: [Coefficients; BANDS],
    /// Indices of the bands that do something, in order; only the first `active` entries count.
    order: [usize; BANDS],
    active: usize,
    state: [[Biquad; BANDS]; 2],
    preamp: f64,
}

impl Equalizer {
    pub(crate) fn new() -> Self {
        Self {
            coefficients: [Coefficients::IDENTITY; BANDS],
            order: [0; BANDS],
            active: 0,
            state: [[Biquad::default(); BANDS]; 2],
            preamp: 1.0,
        }
    }

    /// Designs the bands for `gains`. Filter state is kept so a change while audio plays does not click,
    /// except for a band that was off: it starts from silence.
    pub(crate) fn configure(&mut self, sample_rate: f64, gains: &[f32; BANDS]) {
        let mut active = 0;
        for (band, &gain) in gains.iter().enumerate() {
            let freq = BAND_HZ[band];
            if gain == 0.0 || freq >= sample_rate * MAX_FREQ_SHARE {
                continue;
            }
            if !self.order[..self.active].contains(&band) {
                for channel in &mut self.state {
                    channel[band] = Biquad::default();
                }
            }
            self.coefficients[band] = Coefficients::peaking(sample_rate, freq, Q, f64::from(gain));
            self.order[active] = band;
            active += 1;
        }
        self.active = active;
        self.preamp = 10f64.powf(preamp_db(gains) / 20.0);
    }

    pub(crate) fn is_active(&self) -> bool {
        self.active > 0
    }

    #[inline]
    pub(crate) fn run(&mut self, left: f64, right: f64) -> (f64, f64) {
        let (mut l, mut r) = (left * self.preamp, right * self.preamp);
        let [state_l, state_r] = &mut self.state;
        for &band in &self.order[..self.active] {
            let c = &self.coefficients[band];
            l = state_l[band].run(c, l);
            r = state_r[band].run(c, r);
        }
        (l, r)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn preset_keys_round_trip_and_curves_stay_in_range() {
        for preset in Preset::ALL {
            assert_eq!(Preset::from_key(preset.key()), Some(preset));
            assert!(
                preset
                    .gains()
                    .iter()
                    .all(|g| g.abs() <= MAX_BAND_DB && g.is_finite()),
                "{preset:?}"
            );
        }
        assert_eq!(Preset::from_key("nonsense"), None);
    }

    #[test]
    fn every_preset_has_a_distinct_curve() {
        for (i, a) in Preset::ALL.iter().enumerate() {
            for b in &Preset::ALL[i + 1..] {
                assert_ne!(a.gains(), b.gains(), "{a:?} and {b:?}");
            }
        }
        assert_eq!(Preset::matching(&Preset::Rock.gains()), Some(Preset::Rock));
        assert_eq!(Preset::matching(&[1.5; BANDS]), None);
    }

    #[test]
    fn flat_curve_does_nothing() {
        let mut eq = Equalizer::new();
        eq.configure(48_000.0, &[0.0; BANDS]);
        assert!(!eq.is_active());
        assert_eq!(eq.run(0.3, -0.7), (0.3, -0.7));
    }

    #[test]
    fn boosts_take_headroom_and_cuts_do_not_add_any() {
        assert_eq!(preamp_db(&[0.0; BANDS]), 0.0);
        assert_eq!(preamp_db(&Preset::BassBoost.gains()), -3.0);
        assert_eq!(preamp_db(&Preset::BassReducer.gains()), 0.0);
    }

    #[test]
    fn bands_past_nyquist_are_left_out() {
        let mut eq = Equalizer::new();
        let mut gains = [0.0; BANDS];
        gains[BANDS - 1] = 6.0;
        eq.configure(22_050.0, &gains);
        assert!(!eq.is_active());
        eq.configure(48_000.0, &gains);
        assert!(eq.is_active());
        let (l, r) = eq.run(0.5, 0.5);
        assert!(l.is_finite() && r.is_finite());
    }
}
