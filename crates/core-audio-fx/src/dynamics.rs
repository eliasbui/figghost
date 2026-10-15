//! Level control: the compressor behind Magic Boost and Night mode, and the output limiter.

/// Width of the soft knee around the threshold.
const KNEE_DB: f64 = 6.0;
/// Below this level the boost fades out, so hiss and room noise between sounds are not lifted.
const FLOOR_DB: f64 = -55.0;
const FLOOR_FADE_DB: f64 = 15.0;
/// The limiter keeps peaks 0.3 dB under full scale.
const CEILING: f64 = 0.966;
const LIMITER_RELEASE_S: f64 = 0.08;
/// Envelope level treated as silence, to keep the logarithm finite.
const SILENCE: f64 = 1e-9;

/// A static compressor curve with make-up gain and the speed it follows the signal at.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct Curve {
    threshold_db: f64,
    ratio: f64,
    makeup_db: f64,
    attack_s: f64,
    release_s: f64,
}

/// Make-up gain that brings a full-scale signal back to full scale: everything quieter comes out
/// louder, and nothing comes out louder than full scale.
fn makeup(threshold_db: f64, ratio: f64) -> f64 {
    -threshold_db * (1.0 - 1.0 / ratio)
}

impl Curve {
    /// The curve for a Magic Boost amount (0 to 1) and the Night mode switch; `None` leaves the level alone.
    /// Night mode is stronger and quicker than any boost: loud scenes come down, quiet dialogue comes up.
    pub(crate) fn for_settings(boost: f32, night: bool) -> Option<Self> {
        if night {
            let (threshold_db, ratio) = (-30.0, 4.0);
            return Some(Self {
                threshold_db,
                ratio,
                makeup_db: makeup(threshold_db, ratio) * 0.8,
                attack_s: 0.003,
                release_s: 0.25,
            });
        }
        if boost.is_nan() || boost <= 0.0 {
            return None;
        }
        let amount = f64::from(boost.min(1.0));
        let (threshold_db, ratio) = (-24.0 * amount, 1.0 + 2.0 * amount);
        Some(Self {
            threshold_db,
            ratio,
            makeup_db: makeup(threshold_db, ratio),
            attack_s: 0.01,
            release_s: 0.3,
        })
    }

    /// Static curve: output level for an input level, both in dB.
    fn output_db(&self, x: f64) -> f64 {
        let over = x - self.threshold_db;
        if 2.0 * over < -KNEE_DB {
            x
        } else if 2.0 * over.abs() <= KNEE_DB {
            x + (1.0 / self.ratio - 1.0) * (over + KNEE_DB / 2.0).powi(2) / (2.0 * KNEE_DB)
        } else {
            self.threshold_db + over / self.ratio
        }
    }

    /// The gain change in dB for a signal at `x` dB.
    pub(crate) fn gain_db(&self, x: f64) -> f64 {
        let gain = self.output_db(x) - x + self.makeup_db;
        if gain <= 0.0 || x >= FLOOR_DB {
            return gain;
        }
        gain * ((x - (FLOOR_DB - FLOOR_FADE_DB)) / FLOOR_FADE_DB).clamp(0.0, 1.0)
    }
}

/// One-pole smoothing coefficient for a time constant at a sample rate.
pub(crate) fn coefficient(seconds: f64, sample_rate: f64) -> f64 {
    (-1.0 / (seconds * sample_rate)).exp()
}

/// Feed-forward, stereo-linked compressor following the peak level.
pub(crate) struct Compressor {
    curve: Option<Curve>,
    attack: f64,
    release: f64,
    envelope: f64,
}

impl Compressor {
    pub(crate) fn new() -> Self {
        Self {
            curve: None,
            attack: 0.0,
            release: 0.0,
            envelope: 0.0,
        }
    }

    pub(crate) fn configure(&mut self, sample_rate: f64, curve: Option<Curve>) {
        if let Some(c) = &curve {
            self.attack = coefficient(c.attack_s, sample_rate);
            self.release = coefficient(c.release_s, sample_rate);
        }
        self.curve = curve;
    }

    /// The gain to apply now, given the current peak of both channels.
    #[inline]
    pub(crate) fn gain(&mut self, peak: f64) -> f64 {
        let Some(curve) = &self.curve else {
            return 1.0;
        };
        let speed = if peak > self.envelope {
            self.attack
        } else {
            self.release
        };
        self.envelope = peak + speed * (self.envelope - peak);
        let level = 20.0 * self.envelope.max(SILENCE).log10();
        10f64.powf(curve.gain_db(level) / 20.0)
    }
}

/// Instant-attack peak limiter with an exponential release, then a hard ceiling at full scale. A safety
/// net for volumes past 100%, boosted equalizer curves and Magic Boost; quiet audio passes untouched.
pub(crate) struct Limiter {
    gain: f64,
    release: f64,
}

impl Limiter {
    pub(crate) fn new(sample_rate: f64) -> Self {
        Self {
            gain: 1.0,
            release: coefficient(LIMITER_RELEASE_S, sample_rate),
        }
    }

    #[inline]
    pub(crate) fn run(&mut self, left: f64, right: f64) -> (f64, f64) {
        let peak = left.abs().max(right.abs());
        let limit = if peak > CEILING { CEILING / peak } else { 1.0 };
        self.gain = if limit < self.gain {
            limit
        } else {
            limit + self.release * (self.gain - limit)
        };
        (
            (left * self.gain).clamp(-1.0, 1.0),
            (right * self.gain).clamp(-1.0, 1.0),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn magic_boost_lifts_quiet_audio_and_leaves_full_scale_alone() {
        let curve = Curve::for_settings(1.0, false).unwrap();
        assert!(curve.gain_db(0.0).abs() < 1e-9);
        assert!((curve.gain_db(-40.0) - 16.0).abs() < 1e-9);
        // Between the two, the boost shrinks steadily as the level rises.
        let mut last = f64::INFINITY;
        for level in (-40..=0).map(f64::from) {
            let gain = curve.gain_db(level);
            assert!(gain <= last + 1e-9, "{level}: {gain} > {last}");
            last = gain;
        }
    }

    #[test]
    fn the_noise_floor_is_not_lifted() {
        let curve = Curve::for_settings(1.0, false).unwrap();
        assert_eq!(curve.gain_db(-80.0), 0.0);
        assert!(curve.gain_db(-62.5) > 0.0 && curve.gain_db(-62.5) < curve.gain_db(-50.0));
    }

    #[test]
    fn night_mode_brings_loud_audio_down() {
        let curve = Curve::for_settings(0.0, true).unwrap();
        assert!(curve.gain_db(0.0) < -3.0);
        assert!(curve.gain_db(-40.0) > 15.0);
    }

    #[test]
    fn no_boost_means_no_compressor() {
        assert_eq!(Curve::for_settings(0.0, false), None);
        assert_eq!(Curve::for_settings(f32::NAN, false), None);
        let mut compressor = Compressor::new();
        compressor.configure(48_000.0, None);
        assert_eq!(compressor.gain(0.8), 1.0);
    }

    #[test]
    fn the_limiter_never_lets_a_sample_past_full_scale() {
        let mut limiter = Limiter::new(48_000.0);
        for i in 0..48_000 {
            let x = 4.0 * (i as f64 * 0.05).sin();
            let (l, r) = limiter.run(x, -x);
            assert!(l.abs() <= CEILING + 1e-12 && r.abs() <= CEILING + 1e-12);
        }
    }

    #[test]
    fn the_limiter_leaves_quiet_audio_untouched() {
        let mut limiter = Limiter::new(48_000.0);
        for x in [0.1, -0.5, 0.9, -0.96] {
            assert_eq!(limiter.run(x, x), (x, x));
        }
    }
}
