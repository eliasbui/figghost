//! A live level meter of what the default output plays, for the music bars in the notch: a private
//! device tap that only listens (every app keeps playing as before), read by a route whose callback
//! splits the sound into frequency bands and writes silence, so nothing audible changes.
//!
//! The bands are measured on the device's real-time IO thread and read on the main thread through
//! atomics. When the default output changes, or its sample rate does, the tap and route are rebuilt on
//! the main thread behind the same [`Meter`].

use crate::{
    app,
    audio_tap::{
        self, Error, Listener, Muting, ObjectId, Route, Source, Tap, process_object, selector,
    },
};
use std::{
    cell::{Cell, RefCell},
    f64::consts::PI,
    rc::{Rc, Weak},
    sync::{
        Arc,
        atomic::{AtomicBool, AtomicU32, Ordering},
    },
};

/// The bands cover this range, evenly spaced on a log scale.
const LOWEST_HZ: f64 = 60.0;
const HIGHEST_HZ: f64 = 12_000.0;
/// An empty bar is this far below a full one.
const RANGE_DB: f32 = 60.0;
/// A bar rises within a beat and falls more slowly, so it follows the music without flickering.
const ATTACK_SECONDS: f32 = 0.03;
const RELEASE_SECONDS: f32 = 0.25;
/// The automatic gain lifts the loudest recent band to a full bar, by at most this much, so the hiss
/// under a quiet passage stays low.
const MAX_GAIN_DB: f32 = 30.0;
/// After a loud passage the gain comes back at this rate: a quiet song after a loud one fills the bars
/// again within seconds, and the gain does not pump with every beat.
const GAIN_RECOVERY_DB_PER_SECOND: f32 = 4.0;
/// A sample above this (-100 dBFS) is sound. A tap without the permission delivers silence.
const HEARD_FLOOR: f32 = 1e-5;
/// Device notifications come in bursts; one rebuild after the last of them is enough.
const SETTLE_SECONDS: f64 = 0.05;
const NAME: &str = "Figghost level meter";
/// `kAudioHardwareBadDeviceError`.
const NO_DEVICE: i32 = i32::from_be_bytes(*b"!dev");
/// `kAudio_ParamError`.
const PARAM_ERROR: i32 = -50;

/// `bands + 1` frequencies from `LOWEST_HZ` to `HIGHEST_HZ`, evenly spaced on a log scale. The top stays
/// under 0.45 of the sample rate, so a low-rate device (a headset during a call) still gets valid filters.
fn band_edges(bands: usize, sample_rate: f64) -> Vec<f64> {
    let top = HIGHEST_HZ.min(0.45 * sample_rate);
    let step = (top / LOWEST_HZ).powf(1.0 / bands as f64);
    (0..=bands)
        .map(|i| LOWEST_HZ * step.powi(i as i32))
        .collect()
}

/// Band-pass biquad from the Audio EQ Cookbook (R. Bristow-Johnson), 0 dB at its centre, normalized so
/// a0 = 1. Its numerator is always b0 (1 - z^-2), so b0 is all it keeps of it. In f64, like the effects'
/// filters: narrow low bands put the poles close to the unit circle, where f32 rounding moves them.
#[derive(Debug, Clone, Copy, PartialEq)]
struct BandPass {
    b0: f64,
    a1: f64,
    a2: f64,
}

impl BandPass {
    fn new(sample_rate: f64, low: f64, high: f64) -> Self {
        let centre = (low * high).sqrt();
        let q = centre / (high - low);
        let w0 = 2.0 * PI * centre / sample_rate;
        let alpha = w0.sin() / (2.0 * q);
        let a0 = 1.0 + alpha;
        Self {
            b0: alpha / a0,
            a1: -2.0 * w0.cos() / a0,
            a2: (1.0 - alpha) / a0,
        }
    }
}

/// Transposed direct form II state of one filter section.
#[derive(Debug, Clone, Copy, Default)]
struct Section {
    z1: f64,
    z2: f64,
}

impl Section {
    #[inline]
    fn run(&mut self, f: &BandPass, x: f64) -> f64 {
        let y = f.b0 * x + self.z1;
        self.z1 = self.z2 - f.a1 * y;
        self.z2 = -f.b0 * x - f.a2 * y;
        y
    }
}

/// One bar.
struct Band {
    filter: BandPass,
    /// Two sections in series: with one, a bass note reaches the top of 5 bands at about -36 dB, which
    /// the 60 dB scale still shows as a bar 40 % high.
    sections: [Section; 2],
    /// Sum of squares over the current cycle.
    power: f64,
    /// What the bar shows, 0 to 1.
    level: f32,
}

/// Splits interleaved audio into bands and turns each band's level into a bar height. It allocates only
/// when made; `process` runs on the IO thread.
struct Analyzer {
    bands: Vec<Band>,
    sample_rate: f32,
    /// The loudest band of recent cycles, in dBFS: it follows a louder band at once and a quieter one
    /// slowly.
    loudest_db: f32,
}

impl Analyzer {
    fn new(bands: usize, sample_rate: f64) -> Self {
        let bands = band_edges(bands, sample_rate)
            .windows(2)
            .map(|edge| Band {
                filter: BandPass::new(sample_rate, edge[0], edge[1]),
                sections: [Section::default(); 2],
                power: 0.0,
                level: 0.0,
            })
            .collect();
        Self {
            bands,
            sample_rate: sample_rate as f32,
            loudest_db: f32::NEG_INFINITY,
        }
    }

    /// Measures one IO cycle of interleaved frames and moves the bars; true if the cycle held sound.
    fn process(&mut self, input: &[f32], channels: usize) -> bool {
        let channels = channels.max(1);
        let frames = input.len() / channels;
        if frames == 0 {
            return false;
        }
        let mut peak = 0.0f32;
        for frame in input.chunks_exact(channels) {
            let mut sum = 0.0;
            for &sample in frame {
                sum += f64::from(sample);
                peak = peak.max(sample.abs());
            }
            let x = sum / channels as f64;
            // One bad sample would stay in the filters' state for good.
            if !x.is_finite() {
                continue;
            }
            for band in &mut self.bands {
                let mut y = x;
                for section in &mut band.sections {
                    y = section.run(&band.filter, y);
                }
                band.power += y * y;
            }
        }
        let dt = frames as f32 / self.sample_rate;
        let db = |power: f64| (10.0 * (power / frames as f64).log10()) as f32;
        let loudest = self.bands.iter().fold(0.0, |m: f64, b| m.max(b.power));
        self.loudest_db = db(loudest).max(self.loudest_db - GAIN_RECOVERY_DB_PER_SECOND * dt);
        let gain_db = (-self.loudest_db).clamp(0.0, MAX_GAIN_DB);
        let attack = 1.0 - (-dt / ATTACK_SECONDS).exp();
        let release = 1.0 - (-dt / RELEASE_SECONDS).exp();
        for band in &mut self.bands {
            let target = ((db(band.power) + gain_db) / RANGE_DB + 1.0).clamp(0.0, 1.0);
            band.power = 0.0;
            let speed = if target > band.level { attack } else { release };
            band.level += (target - band.level) * speed;
        }
        peak > HEARD_FLOOR
    }

    fn levels(&self) -> impl Iterator<Item = f32> + '_ {
        self.bands.iter().map(|b| b.level)
    }
}

/// What the IO thread measures, read on the main thread.
struct Levels {
    /// Per band, 0 to 1, as f32 bits.
    bands: Box<[AtomicU32]>,
    heard: AtomicBool,
}

impl Levels {
    fn clear(&self) {
        for band in self.bands.iter() {
            band.store(0, Ordering::Relaxed);
        }
    }
}

/// The tap and route on the current output, and the listener for that device's sample rate.
struct Current {
    _rate: Option<Listener>,
    _route: Route,
}

struct Inner {
    me: Weak<Inner>,
    levels: Arc<Levels>,
    current: RefCell<Option<Current>>,
    pending: Cell<bool>,
}

impl Inner {
    fn build(&self) -> Result<Current, Error> {
        let output = audio_tap::default_output().ok_or(Error {
            action: "finding the default output device",
            status: NO_DEVICE,
        })?;
        let pid = std::process::id() as i32;
        let own = process_object(pid);
        let tap = Tap::new(
            Source::Device {
                uid: &output.uid,
                exclude: own.as_slice(),
            },
            Muting::Unmuted,
            NAME,
        )?;
        let levels = Arc::clone(&self.levels);
        let mut route = Route::new(tap, &output.uid, NAME, move |format| {
            let mut analyzer = Analyzer::new(levels.bands.len(), format.sample_rate);
            // The output stays as the route zeroed it: silence.
            Box::new(move |input, channels, _, _| {
                let sound = analyzer.process(input, channels);
                for (band, level) in levels.bands.iter().zip(analyzer.levels()) {
                    band.store(level.to_bits(), Ordering::Relaxed);
                }
                if sound {
                    levels.heard.store(true, Ordering::Relaxed);
                }
            })
        })?;
        // Building the route connected this process to the audio system if nothing had before. Its own
        // output stays out: the Sound feature plays apps again through it, and the tap hears them already.
        if own.is_none()
            && let Some(own) = process_object(pid)
        {
            route.tap().set_processes(&[own])?;
        }
        route.start()?;
        Ok(Current {
            _rate: self.listen(output.id, selector::NOMINAL_RATE),
            _route: route,
        })
    }

    fn rebuild(&self) {
        // The old route stops first: only one IO thread may write the levels.
        self.current.replace(None);
        self.levels.clear();
        match self.build() {
            Ok(current) => {
                self.current.replace(Some(current));
            }
            Err(e) => eprintln!("audio meter: {e}"),
        }
    }

    fn schedule(&self) {
        if self.pending.replace(true) {
            return;
        }
        let me = self.me.clone();
        app::after(SETTLE_SECONDS, move || {
            if let Some(inner) = me.upgrade() {
                inner.pending.set(false);
                inner.rebuild();
            }
        });
    }

    /// Rebuilds the tap and route after `selector` changes on `object`.
    fn listen(&self, object: ObjectId, selector: u32) -> Option<Listener> {
        let me = self.me.clone();
        Listener::new(object, selector, move || {
            if let Some(inner) = me.upgrade() {
                inner.schedule();
            }
        })
    }
}

/// A running level meter of the default output; stops when dropped. Main thread only.
pub struct Meter {
    /// Removed first when the meter drops, so no rebuild starts after.
    _default_output: Option<Listener>,
    inner: Rc<Inner>,
}

impl Meter {
    /// Copies the level of each band, 0 to 1, lowest band first, into `out`, as many as fit.
    pub fn levels(&self, out: &mut [f32]) {
        for (out, band) in out.iter_mut().zip(self.inner.levels.bands.iter()) {
            *out = f32::from_bits(band.load(Ordering::Relaxed));
        }
    }

    /// Whether any sound has reached the meter since it started. Still false after a second of
    /// playback means the tap delivers silence: the "System Audio Recording" permission is missing.
    pub fn heard(&self) -> bool {
        self.inner.levels.heard.load(Ordering::Relaxed)
    }

    pub fn bands(&self) -> usize {
        self.inner.levels.bands.len()
    }
}

/// Starts metering the default output in `bands` bands. Call it on the main thread, whose run loop must
/// run for the meter to follow a change of output. Fails before macOS 14.2 and without an output device.
pub fn start(bands: usize) -> Result<Meter, Error> {
    if bands == 0 {
        return Err(Error {
            action: "metering zero bands",
            status: PARAM_ERROR,
        });
    }
    let inner = Rc::new_cyclic(|me| Inner {
        me: me.clone(),
        levels: Arc::new(Levels {
            bands: (0..bands).map(|_| AtomicU32::new(0)).collect(),
            heard: AtomicBool::new(false),
        }),
        current: RefCell::new(None),
        pending: Cell::new(false),
    });
    let current = inner.build()?;
    inner.current.replace(Some(current));
    Ok(Meter {
        _default_output: inner.listen(audio_tap::SYSTEM, selector::DEFAULT_OUTPUT),
        inner,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const BLOCK: usize = 512;

    /// Gain of a filter at `freq`, as a plain factor.
    fn magnitude(f: &BandPass, sample_rate: f64, freq: f64) -> f64 {
        let w = 2.0 * PI * freq / sample_rate;
        let numerator = f.b0 * 2.0 * w.sin().abs();
        let re = 1.0 + f.a1 * w.cos() + f.a2 * (2.0 * w).cos();
        let im = f.a1 * w.sin() + f.a2 * (2.0 * w).sin();
        numerator / (re * re + im * im).sqrt()
    }

    /// Feeds `seconds` of a stereo sine (amplitude 0 for silence) in 512-frame cycles, as the IO thread
    /// would, and returns the levels after the last cycle.
    fn feed(
        analyzer: &mut Analyzer,
        sample_rate: f64,
        freq: f64,
        amplitude: f64,
        seconds: f64,
    ) -> Vec<f32> {
        let mut cycle = [0.0f32; 2 * BLOCK];
        for start in (0..(seconds * sample_rate) as usize).step_by(BLOCK) {
            for (i, frame) in cycle.chunks_exact_mut(2).enumerate() {
                let t = (start + i) as f64 / sample_rate;
                frame.fill((amplitude * (2.0 * PI * freq * t).sin()) as f32);
            }
            analyzer.process(&cycle, 2);
        }
        analyzer.levels().collect()
    }

    #[test]
    fn bands_are_log_spaced_from_60_hz_to_12_khz() {
        let edges = band_edges(5, 48_000.0);
        assert_eq!(edges.len(), 6);
        assert!((edges[0] - 60.0).abs() < 1e-9 && (edges[5] - 12_000.0).abs() < 1e-6);
        let step = edges[1] / edges[0];
        for pair in edges.windows(2) {
            assert!((pair[1] / pair[0] - step).abs() < 1e-9, "{edges:?}");
        }
        // A headset in a call runs at 16 kHz: its top band must stay below 8 kHz.
        assert!(band_edges(5, 16_000.0)[5] < 8_000.0);
    }

    #[test]
    fn filters_are_stable_and_pass_their_centre_at_44_1_and_48_khz() {
        for rate in [44_100.0, 48_000.0] {
            for bands in [1, 5, 32] {
                for edge in band_edges(bands, rate).windows(2) {
                    let f = BandPass::new(rate, edge[0], edge[1]);
                    assert!(f.b0.is_finite() && f.a1.is_finite() && f.a2.is_finite());
                    // Both poles inside the unit circle.
                    assert!(
                        f.a2.abs() < 1.0 && f.a1.abs() < 1.0 + f.a2,
                        "{rate} {edge:?}"
                    );
                    let centre = (edge[0] * edge[1]).sqrt();
                    assert!((magnitude(&f, rate, centre) - 1.0).abs() < 1e-9);
                    if bands == 5 {
                        assert!(magnitude(&f, rate, centre / 8.0) < 0.2);
                        assert!(magnitude(&f, rate, (centre * 8.0).min(rate / 2.2)) < 0.4);
                    }
                }
            }
        }
    }

    #[test]
    fn a_low_sine_lights_the_low_band_and_not_the_top_one() {
        for rate in [44_100.0, 48_000.0] {
            let levels = feed(&mut Analyzer::new(5, rate), rate, 100.0, 0.1, 1.0);
            assert!(levels[0] > 0.9, "{rate}: {levels:?}");
            assert!(levels[4] < 0.05, "{rate}: {levels:?}");
        }
    }

    #[test]
    fn a_high_sine_lights_the_top_band_and_not_the_low_one() {
        for rate in [44_100.0, 48_000.0] {
            let levels = feed(&mut Analyzer::new(5, rate), rate, 5_000.0, 0.1, 1.0);
            assert!(levels[4] > 0.9, "{rate}: {levels:?}");
            assert!(levels[0] < 0.05, "{rate}: {levels:?}");
        }
    }

    #[test]
    fn bars_rise_fast_fall_slower_and_silence_decays_to_zero() {
        let rate = 48_000.0;
        let mut analyzer = Analyzer::new(5, rate);
        assert!(feed(&mut analyzer, rate, 100.0, 0.1, 0.06)[0] > 0.6);
        feed(&mut analyzer, rate, 100.0, 0.1, 1.0);
        assert!(feed(&mut analyzer, rate, 0.0, 0.0, 0.1)[0] > 0.5);
        let levels = feed(&mut analyzer, rate, 0.0, 0.0, 2.0);
        assert!(levels.iter().all(|&l| l < 0.01), "{levels:?}");
    }

    #[test]
    fn quiet_and_loud_material_both_move_the_bars() {
        let rate = 48_000.0;
        for amplitude in [0.003, 0.5] {
            let levels = feed(&mut Analyzer::new(5, rate), rate, 100.0, amplitude, 1.0);
            assert!(levels[0] > 0.5, "{amplitude}: {levels:?}");
        }
        // A quiet passage right after a loud one gets the gain back within seconds.
        let mut analyzer = Analyzer::new(5, rate);
        feed(&mut analyzer, rate, 100.0, 0.5, 1.0);
        let at_once = feed(&mut analyzer, rate, 100.0, 0.005, 1.0)[0];
        let later = feed(&mut analyzer, rate, 100.0, 0.005, 10.0)[0];
        assert!(later > at_once + 0.2, "{at_once} then {later}");
    }

    #[test]
    fn only_sound_counts_as_heard_and_bad_samples_do_not_stick() {
        let rate = 48_000.0;
        let mut analyzer = Analyzer::new(5, rate);
        assert!(!analyzer.process(&[0.0; 2 * BLOCK], 2));
        assert!(!analyzer.process(&[1e-7; 2 * BLOCK], 2));
        assert!(analyzer.process(&[f32::NAN, f32::INFINITY, 0.01, 0.01], 2));
        let levels = feed(&mut analyzer, rate, 100.0, 0.1, 0.5);
        assert!(
            levels.iter().all(|l| l.is_finite()) && levels[0] > 0.9,
            "{levels:?}"
        );
        // An empty cycle and a zero channel count are harmless.
        assert!(!analyzer.process(&[], 2));
        assert!(analyzer.process(&[0.5; 7], 0));
    }
}
