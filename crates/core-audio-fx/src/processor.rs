//! The effects chain of one audio stream: the app's settings (when the stream carries one app), then
//! the output device's, then the limiter.

use crate::{
    dynamics::{Compressor, Curve, Limiter, coefficient},
    eq::{BANDS, Equalizer},
    params::Params,
};
use std::sync::Arc;

/// Time constant for volume, width and balance changes: dragging a slider does not crackle, and the
/// change still feels immediate.
const SMOOTHING_S: f64 = 0.015;
/// Close enough to the target to stop moving, so a settled value is exact again.
const SNAP: f64 = 1e-6;

#[derive(Debug, Clone, Copy)]
struct Smoothed {
    value: f64,
    target: f64,
}

impl Smoothed {
    fn at(value: f64) -> Self {
        Self {
            value,
            target: value,
        }
    }

    #[inline]
    fn next(&mut self, coef: f64) -> f64 {
        if self.value != self.target {
            self.value = self.target + coef * (self.value - self.target);
            if (self.value - self.target).abs() < SNAP {
                self.value = self.target;
            }
        }
        self.value
    }
}

/// One set of settings, applied in order: equalizer, dynamics, width and balance, then the volume. The
/// volume comes last so turning a stream down does not make the compressor lift it back up.
struct Stage {
    params: Arc<Params>,
    seen: Option<u32>,
    eq: Equalizer,
    compressor: Compressor,
    gain: Smoothed,
    width: Smoothed,
    balance: Smoothed,
}

impl Stage {
    fn new(params: Arc<Params>, sample_rate: f64) -> Self {
        let mut stage = Self {
            params,
            seen: None,
            eq: Equalizer::new(),
            compressor: Compressor::new(),
            gain: Smoothed::at(1.0),
            width: Smoothed::at(1.0),
            balance: Smoothed::at(0.0),
        };
        stage.begin(sample_rate);
        // A new stream starts where its settings are, without a ramp.
        stage.gain.value = stage.gain.target;
        stage.width.value = stage.width.target;
        stage.balance.value = stage.balance.target;
        stage
    }

    fn begin(&mut self, sample_rate: f64) {
        let p = &self.params;
        let revision = p.revision();
        if self.seen != Some(revision) {
            self.seen = Some(revision);
            let gains = if p.eq_enabled() {
                p.eq_gains()
            } else {
                [0.0; BANDS]
            };
            self.eq.configure(sample_rate, &gains);
            self.compressor
                .configure(sample_rate, Curve::for_settings(p.boost(), p.night()));
        }
        self.gain.target = if p.muted() {
            0.0
        } else {
            f64::from(p.volume())
        };
        self.width.target = f64::from(p.width());
        self.balance.target = f64::from(p.balance());
    }

    #[inline]
    fn run(&mut self, coef: f64, left: f64, right: f64) -> (f64, f64) {
        let (mut l, mut r) = if self.eq.is_active() {
            self.eq.run(left, right)
        } else {
            (left, right)
        };
        let dynamics = self.compressor.gain(l.abs().max(r.abs()));
        l *= dynamics;
        r *= dynamics;

        let width = self.width.next(coef);
        if width != 1.0 {
            let mid = (l + r) * 0.5;
            let side = (l - r) * 0.5 * width;
            l = mid + side;
            r = mid - side;
        }
        let balance = self.balance.next(coef);
        if balance > 0.0 {
            l *= 1.0 - balance;
        } else if balance < 0.0 {
            r *= 1.0 + balance;
        }
        let gain = self.gain.next(coef);
        (l * gain, r * gain)
    }
}

/// Processes one stereo stream. Owned by the audio thread; `begin` once per IO cycle, then `frame` for
/// every sample frame. Neither allocates nor locks.
pub struct Processor {
    sample_rate: f64,
    coef: f64,
    app: Option<Stage>,
    device: Stage,
    limiter: Limiter,
}

impl Processor {
    /// `app` holds the settings of the one app this stream carries, if it carries a single app;
    /// `device` holds the output device's settings, which apply to everything played on it.
    pub fn new(sample_rate: f64, app: Option<Arc<Params>>, device: Arc<Params>) -> Self {
        Self {
            sample_rate,
            coef: coefficient(SMOOTHING_S, sample_rate),
            app: app.map(|p| Stage::new(p, sample_rate)),
            device: Stage::new(device, sample_rate),
            limiter: Limiter::new(sample_rate),
        }
    }

    pub fn sample_rate(&self) -> f64 {
        self.sample_rate
    }

    /// Picks up settings changed since the last cycle.
    pub fn begin(&mut self) {
        if let Some(app) = &mut self.app {
            app.begin(self.sample_rate);
        }
        self.device.begin(self.sample_rate);
    }

    #[inline]
    pub fn frame(&mut self, left: f32, right: f32) -> (f32, f32) {
        let (mut l, mut r) = (f64::from(left), f64::from(right));
        if let Some(app) = &mut self.app {
            (l, r) = app.run(self.coef, l, r);
        }
        (l, r) = self.device.run(self.coef, l, r);
        let (l, r) = self.limiter.run(l, r);
        (l as f32, r as f32)
    }

    /// One cycle over interleaved stereo samples, in place.
    pub fn run_interleaved(&mut self, samples: &mut [f32]) {
        self.begin();
        for frame in samples.chunks_exact_mut(2) {
            (frame[0], frame[1]) = self.frame(frame[0], frame[1]);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::eq::{BAND_HZ, Preset};
    use std::f64::consts::PI;

    const RATE: f64 = 48_000.0;

    fn sine(freq: f64, amplitude: f64, seconds: f64) -> Vec<f32> {
        let frames = (RATE * seconds) as usize;
        (0..frames)
            .flat_map(|i| {
                let x = (amplitude * (2.0 * PI * freq * i as f64 / RATE).sin()) as f32;
                [x, x]
            })
            .collect()
    }

    /// RMS of the left channel over the second half, after the effects have settled.
    fn settled_rms(samples: &[f32]) -> f64 {
        let half = &samples[samples.len() / 2..];
        let sum: f64 = half.iter().step_by(2).map(|&x| f64::from(x).powi(2)).sum();
        (sum / (half.len() / 2) as f64).sqrt()
    }

    fn db(ratio: f64) -> f64 {
        20.0 * ratio.log10()
    }

    fn run(app: Option<&Arc<Params>>, device: &Arc<Params>, input: &[f32]) -> Vec<f32> {
        let mut processor = Processor::new(RATE, app.cloned(), Arc::clone(device));
        let mut out = input.to_vec();
        // Cycles the size a device typically uses.
        for block in out.chunks_mut(512 * 2) {
            processor.run_interleaved(block);
        }
        out
    }

    #[test]
    fn neutral_settings_pass_audio_through_bit_for_bit() {
        let input = sine(440.0, 0.5, 0.2);
        let neutral = Arc::new(Params::default());
        assert_eq!(run(Some(&neutral), &neutral, &input), input);
    }

    #[test]
    fn volume_scales_the_level() {
        let input = sine(440.0, 0.5, 0.5);
        let app = Arc::new(Params::default());
        app.set_volume(0.25);
        let out = run(Some(&app), &Arc::new(Params::default()), &input);
        assert!((db(settled_rms(&out) / settled_rms(&input)) - db(0.25)).abs() < 0.01);
    }

    #[test]
    fn a_volume_change_ramps_instead_of_jumping() {
        let app = Arc::new(Params::default());
        let device = Arc::new(Params::default());
        let mut processor = Processor::new(RATE, Some(Arc::clone(&app)), device);
        app.set_volume(0.0);
        processor.begin();
        // A constant input shows the gain directly; no step between frames may exceed a small fraction.
        let mut last = 1.0f32;
        for _ in 0..(RATE as usize / 10) {
            let (l, _) = processor.frame(0.5, 0.5);
            let gain = l / 0.5;
            assert!((last - gain).abs() < 0.01, "jump from {last} to {gain}");
            last = gain;
        }
        assert!(last < 0.01, "still at {last} after 100 ms");
    }

    #[test]
    fn mute_silences_and_unmute_restores() {
        let app = Arc::new(Params::default());
        app.set_muted(true);
        let out = run(
            Some(&app),
            &Arc::new(Params::default()),
            &sine(440.0, 0.5, 0.3),
        );
        assert!(settled_rms(&out) < 1e-6);
        app.set_muted(false);
        let input = sine(440.0, 0.5, 0.3);
        let out = run(Some(&app), &Arc::new(Params::default()), &input);
        assert_eq!(out, input);
    }

    #[test]
    fn a_boost_past_full_volume_lifts_quiet_audio_and_never_clips() {
        let device = Arc::new(Params::default());
        device.set_volume(crate::MAX_GAIN);
        let quiet = sine(100.0, 0.1, 0.5);
        let lifted = run(None, &device, &quiet);
        assert!((db(settled_rms(&lifted) / settled_rms(&quiet)) - 12.04).abs() < 0.05);
        // Full scale cannot get louder without distortion: it is held just under the ceiling instead.
        let out = run(None, &device, &sine(100.0, 1.0, 0.5));
        assert!(out.iter().all(|x| x.abs() <= 1.0));
        assert!(settled_rms(&out) > 0.65);
    }

    #[test]
    fn an_equalizer_band_changes_its_frequency_and_not_others() {
        let device = Arc::new(Params::default());
        let mut gains = [0.0; BANDS];
        gains[5] = 6.0;
        device.set_eq_gains(gains);
        device.set_eq_enabled(true);
        let preamp = crate::eq::preamp_db(&gains);
        let at = |freq: f64| {
            let input = sine(freq, 0.25, 1.0);
            db(settled_rms(&run(None, &device, &input)) / settled_rms(&input))
        };
        assert!((at(BAND_HZ[5]) - (6.0 + preamp)).abs() < 0.1);
        assert!((at(40.0) - preamp).abs() < 0.1);
    }

    #[test]
    fn presets_reach_the_audio_thread_on_the_next_cycle() {
        let device = Arc::new(Params::default());
        let mut processor = Processor::new(RATE, None, Arc::clone(&device));
        let mut quiet = sine(62.5, 0.1, 0.5);
        let reference = quiet.clone();
        device.set_eq_gains(Preset::BassBoost.gains());
        device.set_eq_enabled(true);
        for block in quiet.chunks_mut(1024) {
            processor.run_interleaved(block);
        }
        assert!(db(settled_rms(&quiet) / settled_rms(&reference)) > 1.5);
    }

    #[test]
    fn magic_boost_lifts_quiet_passages_more_than_loud_ones() {
        let device = Arc::new(Params::default());
        device.set_boost(1.0);
        let lift = |amplitude: f64| {
            let input = sine(440.0, amplitude, 1.0);
            db(settled_rms(&run(None, &device, &input)) / settled_rms(&input))
        };
        let (quiet, loud) = (lift(0.01), lift(0.9));
        assert!(quiet > 12.0, "quiet lifted by {quiet} dB");
        assert!(loud < 1.5, "loud lifted by {loud} dB");
    }

    #[test]
    fn zero_width_is_mono_and_full_balance_silences_one_side() {
        let device = Arc::new(Params::default());
        device.set_width(0.0);
        let mut processor = Processor::new(RATE, None, Arc::clone(&device));
        processor.begin();
        let (l, r) = processor.frame(0.6, -0.2);
        assert!((l - r).abs() < 1e-6 && (l - 0.2).abs() < 1e-6);

        let device = Arc::new(Params::default());
        device.set_balance(-1.0);
        let mut processor = Processor::new(RATE, None, device);
        processor.begin();
        assert_eq!(processor.frame(0.5, 0.5), (0.5, 0.0));
    }
}
