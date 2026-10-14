//! Second-order filter sections. Coefficients and state are f64: at 48 kHz the 31 Hz band puts its poles
//! within 0.3% of the unit circle, where f32 rounding audibly shifts the response.

use std::f64::consts::PI;

/// Normalized biquad coefficients (a0 = 1).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Coefficients {
    b0: f64,
    b1: f64,
    b2: f64,
    a1: f64,
    a2: f64,
}

impl Coefficients {
    pub const IDENTITY: Self = Self {
        b0: 1.0,
        b1: 0.0,
        b2: 0.0,
        a1: 0.0,
        a2: 0.0,
    };

    /// Peaking equalizer from the Audio EQ Cookbook (R. Bristow-Johnson): `gain_db` at `freq`, falling
    /// back to 0 dB away from it with bandwidth set by `q`.
    pub fn peaking(sample_rate: f64, freq: f64, q: f64, gain_db: f64) -> Self {
        let a = 10f64.powf(gain_db / 40.0);
        let w0 = 2.0 * PI * freq / sample_rate;
        let alpha = w0.sin() / (2.0 * q);
        let cos = w0.cos();
        let a0 = 1.0 + alpha / a;
        Self {
            b0: (1.0 + alpha * a) / a0,
            b1: -2.0 * cos / a0,
            b2: (1.0 - alpha * a) / a0,
            a1: -2.0 * cos / a0,
            a2: (1.0 - alpha / a) / a0,
        }
    }

    /// Gain of the filter at `freq`, as a plain factor (1.0 = unchanged).
    pub fn magnitude(&self, sample_rate: f64, freq: f64) -> f64 {
        let w = 2.0 * PI * freq / sample_rate;
        let (c1, s1, c2, s2) = (w.cos(), w.sin(), (2.0 * w).cos(), (2.0 * w).sin());
        let num_re = self.b0 + self.b1 * c1 + self.b2 * c2;
        let num_im = self.b1 * s1 + self.b2 * s2;
        let den_re = 1.0 + self.a1 * c1 + self.a2 * c2;
        let den_im = self.a1 * s1 + self.a2 * s2;
        ((num_re * num_re + num_im * num_im) / (den_re * den_re + den_im * den_im)).sqrt()
    }
}

/// Transposed direct form II state of one channel.
#[derive(Debug, Clone, Copy, Default)]
pub(crate) struct Biquad {
    z1: f64,
    z2: f64,
}

impl Biquad {
    #[inline]
    pub(crate) fn run(&mut self, c: &Coefficients, x: f64) -> f64 {
        let y = c.b0 * x + self.z1;
        self.z1 = c.b1 * x - c.a1 * y + self.z2;
        self.z2 = c.b2 * x - c.a2 * y;
        y
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const RATE: f64 = 48_000.0;

    fn db(factor: f64) -> f64 {
        20.0 * factor.log10()
    }

    #[test]
    fn peaking_reaches_its_gain_at_the_centre_and_none_far_away() {
        for gain in [-12.0, -3.0, 6.0, 12.0] {
            let c = Coefficients::peaking(RATE, 1000.0, 1.41, gain);
            assert!(
                (db(c.magnitude(RATE, 1000.0)) - gain).abs() < 1e-6,
                "{gain}"
            );
            assert!(db(c.magnitude(RATE, 20.0)).abs() < 0.05, "{gain} at 20 Hz");
            assert!(
                db(c.magnitude(RATE, 20_000.0)).abs() < 0.2,
                "{gain} at 20 kHz"
            );
        }
    }

    #[test]
    fn zero_gain_is_an_identity() {
        let c = Coefficients::peaking(RATE, 31.25, 1.41, 0.0);
        let mut state = Biquad::default();
        for x in [0.5, -0.25, 1.0, 0.0, -1.0] {
            assert!((state.run(&c, x) - x).abs() < 1e-12);
        }
    }

    #[test]
    fn a_sine_through_the_filter_comes_out_at_the_predicted_level() {
        let (freq, gain) = (31.25, 9.0);
        let c = Coefficients::peaking(RATE, freq, 1.41, gain);
        let mut state = Biquad::default();
        let (mut sum_in, mut sum_out) = (0.0, 0.0);
        let n = RATE as usize * 2;
        for i in 0..n {
            let x = (2.0 * PI * freq * i as f64 / RATE).sin();
            let y = state.run(&c, x);
            // Skip the first half second while the filter settles.
            if i > n / 4 {
                sum_in += x * x;
                sum_out += y * y;
            }
        }
        assert!((db((sum_out / sum_in).sqrt()) - gain).abs() < 0.05);
    }
}
