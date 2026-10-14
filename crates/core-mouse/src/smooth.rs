//! Exponential-ease scroll interpolation. A pure state machine: the caller supplies the time steps.

const SETTLE_PX: f64 = 0.5;
const MAX_PENDING_PX: f64 = 20_000.0;

/// The time constant for a glide of `duration_ms`: a quarter of it, so by the end the glide has covered
/// about 98% of its distance.
fn time_constant(duration_ms: f64) -> f64 {
    (duration_ms / 1000.0 / 4.0).max(0.005)
}

/// One point of a glide: how long after the push, how much of its distance has been covered (0 to 1) and
/// the speed then, as a multiple of distance per second.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GlidePoint {
    pub t_ms: f64,
    pub covered: f64,
    pub speed: f64,
}

/// The shape of one push, for drawing: `points + 1` samples from the push to 1.6 times the duration.
pub fn glide_profile(duration_ms: f64, points: usize) -> Vec<GlidePoint> {
    let tau = time_constant(duration_ms);
    let horizon = duration_ms.max(50.0) / 1000.0 * 1.6;
    (0..=points.max(1))
        .map(|i| {
            let t = horizon * i as f64 / points.max(1) as f64;
            let decay = (-t / tau).exp();
            GlidePoint {
                t_ms: t * 1000.0,
                covered: 1.0 - decay,
                speed: decay / tau,
            }
        })
        .collect()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Phase {
    Began,
    Changed,
    Ended,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Frame {
    pub dx: i32,
    pub dy: i32,
    pub phase: Phase,
}

#[derive(Debug, Clone, Copy)]
enum State {
    Idle,
    Moving { began: bool },
    Ending { began: bool },
}

pub struct Smoother {
    tau: f64,
    /// Remaining distance in pixels, `[x, y]`.
    pending: [f64; 2],
    /// Sub-pixel remainder carried between frames so integer output loses nothing.
    carry: [f64; 2],
    state: State,
}

impl Smoother {
    pub fn new(duration_ms: f64) -> Self {
        let mut s = Self {
            tau: 0.0,
            pending: [0.0; 2],
            carry: [0.0; 2],
            state: State::Idle,
        };
        s.set_duration(duration_ms);
        s
    }

    /// The time constant is a quarter of the duration: by then the glide has covered about 98% of its distance.
    pub fn set_duration(&mut self, duration_ms: f64) {
        self.tau = time_constant(duration_ms);
    }

    pub fn is_idle(&self) -> bool {
        matches!(self.state, State::Idle)
    }

    /// Adds distance in pixels. Pushing against the current direction drops the remaining glide,
    /// so reversing the wheel takes effect immediately.
    pub fn push(&mut self, dx: f64, dy: f64) {
        if dx == 0.0 && dy == 0.0 {
            return;
        }
        for (axis, delta) in [dx, dy].into_iter().enumerate() {
            if delta == 0.0 {
                continue;
            }
            if self.pending[axis] * delta < 0.0 {
                self.pending[axis] = 0.0;
                self.carry[axis] = 0.0;
            }
            self.pending[axis] =
                (self.pending[axis] + delta).clamp(-MAX_PENDING_PX, MAX_PENDING_PX);
        }
        self.state = match self.state {
            State::Idle => State::Moving { began: false },
            State::Ending { began } => State::Moving { began },
            moving => moving,
        };
    }

    /// Advances by `dt` seconds. `None` means there is nothing to post this tick; keep ticking until idle.
    pub fn tick(&mut self, dt: f64) -> Option<Frame> {
        match self.state {
            State::Idle => None,
            State::Ending { began } => {
                self.state = State::Idle;
                began.then_some(Frame {
                    dx: 0,
                    dy: 0,
                    phase: Phase::Ended,
                })
            }
            State::Moving { began } => {
                let k = 1.0 - (-dt.max(0.0) / self.tau).exp();
                let mut out = [0i32; 2];
                for (axis, slot) in out.iter_mut().enumerate() {
                    let step = self.pending[axis] * k;
                    self.pending[axis] -= step;
                    let total = self.carry[axis] + step;
                    let whole = total.trunc();
                    self.carry[axis] = total - whole;
                    *slot = whole as i32;
                }
                let settled = self.pending[0].abs() + self.pending[1].abs() < SETTLE_PX;
                if settled {
                    for (axis, slot) in out.iter_mut().enumerate() {
                        *slot += (self.pending[axis] + self.carry[axis]).round() as i32;
                        self.pending[axis] = 0.0;
                        self.carry[axis] = 0.0;
                    }
                }
                let moved = out != [0, 0];
                let phase = if began { Phase::Changed } else { Phase::Began };
                let began = began || moved;
                self.state = if settled {
                    State::Ending { began }
                } else {
                    State::Moving { began }
                };
                moved.then_some(Frame {
                    dx: out[0],
                    dy: out[1],
                    phase,
                })
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const DT: f64 = 1.0 / 120.0;

    /// Ticks until idle and returns every posted frame; fails if it never settles.
    fn drain(s: &mut Smoother) -> Vec<Frame> {
        let mut frames = Vec::new();
        for _ in 0..1000 {
            frames.extend(s.tick(DT));
            if s.is_idle() {
                return frames;
            }
        }
        panic!("smoother did not settle");
    }

    fn total(frames: &[Frame]) -> (i32, i32) {
        frames.iter().fold((0, 0), |(x, y), f| (x + f.dx, y + f.dy))
    }

    #[test]
    fn glide_covers_exactly_the_pushed_distance_with_clean_phases() {
        let mut s = Smoother::new(360.0);
        s.push(0.0, 108.0);
        let frames = drain(&mut s);
        assert_eq!(total(&frames), (0, 108));
        assert_eq!(frames.first().unwrap().phase, Phase::Began);
        assert_eq!(frames.last().unwrap().phase, Phase::Ended);
        assert!(
            frames[1..frames.len() - 1]
                .iter()
                .all(|f| f.phase == Phase::Changed)
        );
        assert!(frames.iter().all(|f| f.dy >= 0), "never moves backwards");
        // Ease-out: the first step is the largest, and the whole glide ends within about half a second.
        assert!(frames[0].dy >= frames[5].dy);
        assert!(frames.len() as f64 * DT < 0.6, "{} frames", frames.len());
    }

    #[test]
    fn the_drawn_profile_matches_what_the_smoother_really_does() {
        for duration in [80.0, 200.0, 360.0, 900.0] {
            let profile = glide_profile(duration, 400);
            let mut s = Smoother::new(duration);
            let distance = 1000.0;
            s.push(0.0, distance);
            let mut travelled = 0.0;
            for frame in 1..=60 {
                travelled += s.tick(DT).map_or(0.0, |f| f64::from(f.dy));
                let t_ms = frame as f64 * DT * 1000.0;
                // Interpolate the profile at this moment.
                let next = profile.iter().position(|p| p.t_ms >= t_ms);
                let Some(i) = next.filter(|&i| i > 0) else {
                    continue;
                };
                let (a, b) = (profile[i - 1], profile[i]);
                let covered =
                    a.covered + (b.covered - a.covered) * (t_ms - a.t_ms) / (b.t_ms - a.t_ms);
                assert!(
                    (travelled / distance - covered).abs() < 0.004,
                    "duration {duration} ms at {t_ms:.1} ms: smoother {:.4} vs profile {covered:.4}",
                    travelled / distance
                );
            }
        }
    }

    #[test]
    fn the_profile_starts_at_rest_ends_nearly_done_and_never_goes_backwards() {
        let profile = glide_profile(360.0, 100);
        assert_eq!(profile.len(), 101);
        assert_eq!((profile[0].t_ms, profile[0].covered), (0.0, 0.0));
        assert!(profile.last().unwrap().covered > 0.98);
        assert!(profile.windows(2).all(|w| w[1].covered >= w[0].covered));
        assert!(profile.windows(2).all(|w| w[1].speed <= w[0].speed));
        assert_eq!(glide_profile(360.0, 0).len(), 2);
    }

    #[test]
    fn fast_spinning_accumulates_distance() {
        let mut s = Smoother::new(360.0);
        let mut frames = Vec::new();
        for _ in 0..10 {
            s.push(0.0, -36.0);
            frames.extend(s.tick(DT));
        }
        frames.extend(drain(&mut s));
        assert_eq!(total(&frames), (0, -360));
    }

    #[test]
    fn horizontal_axis_is_independent() {
        let mut s = Smoother::new(360.0);
        s.push(50.0, -20.0);
        assert_eq!(total(&drain(&mut s)), (50, -20));
    }

    #[test]
    fn reversing_direction_drops_the_remaining_glide() {
        let mut s = Smoother::new(360.0);
        s.push(0.0, 200.0);
        let mut frames: Vec<Frame> = (0..6).filter_map(|_| s.tick(DT)).collect();
        let moved = total(&frames).1;
        assert!(moved > 0 && moved < 200);
        s.push(0.0, -50.0);
        frames.extend(drain(&mut s));
        assert_eq!(total(&frames).1, moved - 50);
        assert_eq!(frames.iter().filter(|f| f.phase == Phase::Began).count(), 1);
    }

    #[test]
    fn sub_pixel_input_posts_nothing_and_returns_to_idle() {
        let mut s = Smoother::new(360.0);
        s.push(0.0, 0.3);
        assert!(drain(&mut s).is_empty());
        assert!(s.is_idle());
        assert_eq!(s.tick(DT), None);
    }

    #[test]
    fn pushing_while_ending_continues_the_same_gesture() {
        let mut s = Smoother::new(60.0);
        s.push(0.0, 5.0);
        let mut frames = Vec::new();
        while !matches!(s.state, State::Ending { .. }) {
            frames.extend(s.tick(DT));
        }
        s.push(0.0, 5.0);
        frames.extend(drain(&mut s));
        assert_eq!(total(&frames).1, 10);
        assert_eq!(frames.iter().filter(|f| f.phase == Phase::Began).count(), 1);
        assert_eq!(frames.iter().filter(|f| f.phase == Phase::Ended).count(), 1);
    }

    #[test]
    fn irregular_frame_times_still_conserve_distance() {
        let mut s = Smoother::new(300.0);
        s.push(33.0, 977.0);
        let mut frames = Vec::new();
        let mut dt = 0.004;
        for _ in 0..2000 {
            frames.extend(s.tick(dt));
            dt = if dt > 0.03 { 0.002 } else { dt * 1.7 };
            if s.is_idle() {
                break;
            }
        }
        assert_eq!(total(&frames), (33, 977));
    }
}
