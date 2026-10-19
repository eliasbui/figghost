//! Spatial 3D for headphones: the two channels become two virtual speakers around the listener's head.
//!
//! Each speaker reaches each ear through a spherical-head model: the far ear hears it later (interaural
//! time difference), duller and quieter (head shadow), and a speaker behind the head is duller still. A
//! small room reverb moves the sound out of the head. Turning the speakers around the head rotates the
//! whole sound stage, which is what the 360 degree and orbit controls do.
//!
//! This is a parametric model, not measured HRTF data: the cues are right but a real head differs.
//! ponytail: measured HRTF (SOFA) per listener if this model does not externalise well enough.

use std::f64::consts::PI;

const HEAD_RADIUS_M: f64 = 0.0875;
const SPEED_OF_SOUND_M_S: f64 = 343.0;
/// Each virtual speaker sits this far from straight ahead, like a stereo pair.
const SPEAKER_HALF_ANGLE_DEG: f64 = 30.0;
/// Longer than the largest head delay at 192 kHz, a power of two for cheap wrapping.
const LINE: usize = 256;
/// The paths are recomputed this often; the rotation is slow compared with 0.7 ms.
const UPDATE_EVERY: u32 = 32;
/// How fast the rotation follows a changed angle, per update: about 35 ms at 48 kHz.
const ANGLE_FOLLOW: f64 = 0.02;
/// Two speakers add up in each ear, so the wet signal comes down to keep the loudness.
const WET_GAIN: f64 = 0.6;
/// Reverb level inside the wet signal.
const ROOM: f64 = 0.3;

/// Ring buffer of one speaker's signal, read at a fractional delay for each ear.
struct Line {
    buf: [f64; LINE],
    pos: usize,
}

impl Line {
    fn new() -> Self {
        Self {
            buf: [0.0; LINE],
            pos: 0,
        }
    }

    fn push(&mut self, x: f64) {
        self.pos = (self.pos + 1) & (LINE - 1);
        self.buf[self.pos] = x;
    }

    fn read(&self, delay: f64) -> f64 {
        let delay = delay.clamp(0.0, (LINE - 2) as f64);
        let whole = delay as usize;
        let frac = delay - whole as f64;
        let a = self.buf[self.pos.wrapping_sub(whole) & (LINE - 1)];
        let b = self.buf[self.pos.wrapping_sub(whole + 1) & (LINE - 1)];
        a + (b - a) * frac
    }
}

/// One speaker to one ear: a delay, a gain and a one-pole low-pass.
#[derive(Clone, Copy, Default)]
struct Path {
    delay: f64,
    gain: f64,
    smooth: f64,
    state: f64,
}

impl Path {
    /// `theta` is the speaker's angle in radians, 0 straight ahead and positive to the right; `ear` is
    /// +1 for the right ear and -1 for the left.
    fn aim(&mut self, theta: f64, ear: f64, sample_rate: f64) {
        let side = theta.sin();
        let toward = side * ear;
        // How much of the head is in the way: 0 for the ear facing the speaker, 1 for the opposite one.
        let shadow = (-toward).max(0.0);
        // 0 in front of the head, 1 straight behind it.
        let behind = (-theta.cos()).max(0.0);
        self.delay = if toward < 0.0 {
            HEAD_RADIUS_M / SPEED_OF_SOUND_M_S * (side.asin().abs() + side.abs()) * sample_rate
        } else {
            0.0
        };
        self.gain = (1.0 - 0.35 * shadow) * (1.0 - 0.15 * behind);
        let cutoff =
            (18_000.0 * 0.09f64.powf(shadow) * (1.0 - 0.55 * behind)).min(sample_rate * 0.45);
        self.smooth = 1.0 - (-2.0 * PI * cutoff / sample_rate).exp();
    }

    fn run(&mut self, line: &Line) -> f64 {
        self.state += self.smooth * (line.read(self.delay) - self.state);
        self.state * self.gain
    }
}

struct Comb {
    buf: Vec<f64>,
    pos: usize,
    damp: f64,
}

impl Comb {
    fn new(len: usize) -> Self {
        Self {
            buf: vec![0.0; len.max(1)],
            pos: 0,
            damp: 0.0,
        }
    }

    fn run(&mut self, x: f64) -> f64 {
        let out = self.buf[self.pos];
        self.damp = out * 0.6 + self.damp * 0.4;
        self.buf[self.pos] = x + self.damp * 0.78;
        self.pos = (self.pos + 1) % self.buf.len();
        out
    }
}

struct Allpass {
    buf: Vec<f64>,
    pos: usize,
}

impl Allpass {
    fn new(len: usize) -> Self {
        Self {
            buf: vec![0.0; len.max(1)],
            pos: 0,
        }
    }

    fn run(&mut self, x: f64) -> f64 {
        let delayed = self.buf[self.pos];
        self.buf[self.pos] = x + delayed * 0.5;
        self.pos = (self.pos + 1) % self.buf.len();
        delayed - x * 0.5
    }
}

/// A small room for one ear: four combs and two all-passes, tuned a little differently per ear so the
/// two ears do not hear the same reflections.
struct Room {
    combs: [Comb; 4],
    allpass: [Allpass; 2],
}

impl Room {
    fn new(sample_rate: f64, spread: usize) -> Self {
        let scale = sample_rate / 44_100.0;
        let len = |base: usize| ((base + spread) as f64 * scale) as usize;
        Self {
            combs: [1116, 1188, 1277, 1356].map(|n| Comb::new(len(n))),
            allpass: [556, 441].map(|n| Allpass::new(len(n))),
        }
    }

    fn run(&mut self, x: f64) -> f64 {
        let mut sum = 0.0;
        for comb in &mut self.combs {
            sum += comb.run(x);
        }
        for allpass in &mut self.allpass {
            sum = allpass.run(sum);
        }
        sum * 0.25
    }
}

pub(crate) struct Spatial {
    sample_rate: f64,
    lines: [Line; 2],
    /// `[speaker][ear]`, ears left then right.
    paths: [[Path; 2]; 2],
    rooms: [Room; 2],
    angle_target: f64,
    angle: f64,
    /// Degrees added per frame by the orbit.
    orbit_step: f64,
    phase: f64,
    tick: u32,
}

impl Spatial {
    pub(crate) fn new(sample_rate: f64) -> Self {
        Self {
            sample_rate,
            lines: [Line::new(), Line::new()],
            paths: [[Path::default(); 2]; 2],
            rooms: [Room::new(sample_rate, 0), Room::new(sample_rate, 23)],
            angle_target: 0.0,
            angle: 0.0,
            orbit_step: 0.0,
            phase: 0.0,
            tick: 0,
        }
    }

    /// `angle` in degrees turns the stage around the head; `orbit` in degrees per second keeps turning it.
    pub(crate) fn configure(&mut self, angle: f32, orbit: f32) {
        self.angle_target = f64::from(angle);
        self.orbit_step = f64::from(orbit) / self.sample_rate;
    }

    fn aim(&mut self) {
        // ponytail: the angle moves straight to its target, not the short way round the circle.
        self.angle += (self.angle_target - self.angle) * ANGLE_FOLLOW;
        let stage = (self.angle + self.phase).to_radians();
        let half = SPEAKER_HALF_ANGLE_DEG.to_radians();
        for (speaker, theta) in [stage - half, stage + half].into_iter().enumerate() {
            self.paths[speaker][0].aim(theta, -1.0, self.sample_rate);
            self.paths[speaker][1].aim(theta, 1.0, self.sample_rate);
        }
    }

    /// The binaural signal for one frame. The caller mixes it with the original.
    #[inline]
    pub(crate) fn run(&mut self, left: f64, right: f64) -> (f64, f64) {
        if self.tick == 0 {
            self.aim();
        }
        self.tick = (self.tick + 1) % UPDATE_EVERY;
        self.phase = (self.phase + self.orbit_step) % 360.0;

        self.lines[0].push(left);
        self.lines[1].push(right);
        let mut ears = [0.0; 2];
        for (speaker, line) in self.lines.iter().enumerate() {
            for (ear, sum) in ears.iter_mut().enumerate() {
                *sum += self.paths[speaker][ear].run(line);
            }
        }
        let l = ears[0] * WET_GAIN;
        let r = ears[1] * WET_GAIN;
        (
            l + self.rooms[0].run(l) * ROOM,
            r + self.rooms[1].run(r) * ROOM,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const RATE: f64 = 48_000.0;

    /// Both ears' response to a click on the left channel only, over `frames` frames.
    fn click_left(angle: f32, frames: usize) -> (Vec<f64>, Vec<f64>) {
        let mut spatial = Spatial::new(RATE);
        spatial.configure(angle, 0.0);
        // Let the angle settle before the click.
        for _ in 0..(RATE as usize / 2) {
            spatial.run(0.0, 0.0);
        }
        let (mut l, mut r) = (Vec::new(), Vec::new());
        for i in 0..frames {
            let (a, b) = spatial.run(if i == 0 { 1.0 } else { 0.0 }, 0.0);
            l.push(a);
            r.push(b);
        }
        (l, r)
    }

    fn peak_index(x: &[f64]) -> usize {
        x.iter()
            .enumerate()
            .max_by(|a, b| a.1.abs().total_cmp(&b.1.abs()))
            .map(|(i, _)| i)
            .unwrap()
    }

    #[test]
    fn a_sound_on_the_left_reaches_the_left_ear_first_and_louder() {
        let (l, r) = click_left(0.0, 200);
        assert!(peak_index(&l) < peak_index(&r));
        assert!(
            l.iter().fold(0.0f64, |m, x| m.max(x.abs()))
                > r.iter().fold(0.0f64, |m, x| m.max(x.abs()))
        );
    }

    #[test]
    fn turning_the_stage_half_way_round_swaps_the_ears() {
        let (l, r) = click_left(180.0, 200);
        // The left speaker has moved to the right side, behind the head.
        assert!(peak_index(&r) <= peak_index(&l));
    }

    #[test]
    fn silence_in_gives_silence_out() {
        let mut spatial = Spatial::new(RATE);
        spatial.configure(90.0, 30.0);
        for _ in 0..10_000 {
            assert_eq!(spatial.run(0.0, 0.0), (0.0, 0.0));
        }
    }

    #[test]
    fn orbit_keeps_turning_the_stage() {
        let mut spatial = Spatial::new(RATE);
        spatial.configure(0.0, 90.0);
        for _ in 0..(RATE as usize) {
            spatial.run(0.0, 0.0);
        }
        assert!(
            (spatial.phase - 90.0).abs() < 0.01,
            "phase {}",
            spatial.phase
        );
    }
}
