//! The moving sky behind the Weather tab: the sun or the moon where it is in the day, clouds, rain of every size,
//! lightning, snow, frost, wind with the leaves it carries and the dust of dirty air. The app says what the sky is like
//! as numbers (`Atmosphere`); where every drop and leaf is at each moment is worked out here by `Sim`, which knows
//! nothing of AppKit so that its rules can be tested, and `FxView` only paints it.

use objc2::{
    AnyThread, DefinedClass, MainThreadMarker, MainThreadOnly, define_class, msg_send, rc::Retained,
};
use objc2_app_kit::{NSBezierPath, NSColor, NSGradient, NSGradientDrawingOptions, NSView};
use objc2_foundation::{NSObjectProtocol, NSPoint, NSRect, NSSize};
use std::{
    cell::RefCell,
    f64::consts::{PI, TAU},
};

/// What the sky is like, every figure from 0 to 1 unless it says otherwise.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Atmosphere {
    pub day: bool,
    /// How far the sun has gone through the daylight, or the moon through the night: 0 at rise, 1 at set.
    pub progress: f64,
    pub cloud: f64,
    /// 0 is dry. Up to a quarter is a drizzle of fine drops, over three quarters a downpour of big ones.
    pub rain: f64,
    pub thunder: bool,
    pub snow: f64,
    pub fog: f64,
    pub cold: f64,
    pub wind: f64,
    /// Which way the wind blows on screen: -1 to the left, 1 to the right.
    pub wind_dir: f64,
    pub haze: f64,
    /// The colour of the leaves the wind carries, which says the season.
    pub leaf: (f64, f64, f64),
}

/// A small, fast, repeatable generator: the same sky starts the same way, which the checks rely on.
struct Rng(u64);

impl Rng {
    fn next(&mut self) -> f64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        (self.0 >> 11) as f64 / (1u64 << 53) as f64
    }

    fn range(&mut self, low: f64, high: f64) -> f64 {
        low + (high - low) * self.next()
    }
}

#[derive(Clone, Copy)]
struct Particle {
    x: f64,
    y: f64,
    /// A speed factor or a size, by the kind of particle.
    a: f64,
    b: f64,
    phase: f64,
}

pub struct Sim {
    atm: Atmosphere,
    w: f64,
    h: f64,
    t: f64,
    rng: Rng,
    drops: Vec<Particle>,
    ripples: Vec<(f64, f64, f64)>,
    flakes: Vec<Particle>,
    streaks: Vec<Particle>,
    leaves: Vec<Particle>,
    motes: Vec<Particle>,
    clouds: Vec<Particle>,
    sparkles: Vec<Particle>,
    mist: Vec<Particle>,
    stars: Vec<Particle>,
    /// 1 at the instant of a flash, fading to 0.
    flash: f64,
    next_bolt: f64,
    bolt: Vec<(f64, f64)>,
    /// Seconds until the thunder of the last bolt arrives: light is quicker than sound.
    thunder_in: Option<f64>,
    rumble: bool,
}

fn count(base: f64, scale: f64, level: f64) -> usize {
    if level <= 0.0 {
        0
    } else {
        (base + scale * level.min(1.0)) as usize
    }
}

impl Sim {
    pub fn new(atm: Atmosphere, w: f64, h: f64) -> Self {
        let mut rng = Rng(0x9E37_79B9_7F4A_7C15);
        let scatter = |n: usize, rng: &mut Rng| -> Vec<Particle> {
            (0..n)
                .map(|_| Particle {
                    x: rng.range(0.0, w),
                    y: rng.range(0.0, h),
                    a: rng.range(0.7, 1.3),
                    b: rng.range(0.0, 1.0),
                    phase: rng.range(0.0, TAU),
                })
                .collect()
        };
        let rain = if atm.rain > 0.02 { atm.rain } else { 0.0 };
        let snow = atm.snow;
        let wind = if atm.wind > 0.1 { atm.wind } else { 0.0 };
        let sim = Self {
            drops: scatter(count(24.0, 190.0, rain), &mut rng),
            flakes: scatter(count(30.0, 110.0, snow), &mut rng),
            streaks: scatter(count(2.0, 14.0, wind), &mut rng),
            // Leaves start to come at a fresh breeze and swirl in a gale.
            leaves: scatter(
                if atm.wind > 0.2 {
                    (3.0 + atm.wind * 13.0) as usize
                } else {
                    0
                },
                &mut rng,
            ),
            motes: scatter(count(24.0, 110.0, atm.haze), &mut rng),
            clouds: scatter(count(2.0, 5.0, atm.cloud), &mut rng),
            sparkles: scatter(count(6.0, 22.0, atm.cold), &mut rng),
            mist: scatter(count(5.0, 8.0, atm.cold), &mut rng),
            stars: scatter(if atm.day { 0 } else { 46 }, &mut rng),
            ripples: Vec::new(),
            atm,
            w,
            h,
            t: 0.0,
            rng,
            flash: 0.0,
            next_bolt: 1.2,
            bolt: Vec::new(),
            thunder_in: None,
            rumble: false,
        };
        let mut sim = sim;
        // A frame of a sky that has been going on for a while, not one that is only starting.
        for _ in 0..90 {
            sim.step(1.0 / 30.0);
        }
        sim
    }

    pub fn atmosphere(&self) -> Atmosphere {
        self.atm
    }

    /// Moves everything on by `dt` seconds.
    pub fn step(&mut self, dt: f64) {
        let dt = dt.clamp(0.0, 0.05);
        self.t += dt;
        let (w, h) = (self.w, self.h);
        let atm = self.atm;
        let dir = if atm.wind_dir < 0.0 { -1.0 } else { 1.0 };

        // Rain falls on a slant that the wind sets, and a drop that lands rings the water for a moment.
        let slant = dir * (30.0 + atm.wind * 360.0);
        let fall = 300.0 + 520.0 * atm.rain;
        let length = rain_length(atm.rain);
        let mut landed = Vec::new();
        for d in &mut self.drops {
            d.y += fall * d.a * dt;
            d.x += slant * dt;
            if d.y > h {
                landed.push(d.x);
                d.y = -length;
                d.x = self.rng.range(-0.3 * w, 1.1 * w);
            }
        }
        for x in landed {
            if self.rng.next() < 0.3 && self.ripples.len() < 28 && (0.0..w).contains(&x) {
                let y = self.rng.range(h - 18.0, h - 3.0);
                self.ripples.push((x, y, 0.0));
            }
        }
        for r in &mut self.ripples {
            r.2 += dt;
        }
        self.ripples.retain(|r| r.2 < 0.7);

        for f in &mut self.flakes {
            f.y += (22.0 + 36.0 * f.a) * dt;
            f.x += ((self.t * 1.3 + f.phase).sin() * 14.0 + dir * atm.wind * 140.0) * dt;
            if f.y > h + 4.0 {
                f.y = -4.0;
                f.x = self.rng.range(0.0, w);
            }
            f.x = f.x.rem_euclid(w);
        }

        // Wind streaks and leaves travel across, and come back in at the other side.
        for s in &mut self.streaks {
            s.x += dir * (150.0 + 820.0 * atm.wind) * s.a * dt;
            let len = 40.0 + 110.0 * atm.wind;
            if s.x > w + len || s.x < -len {
                s.x = if dir > 0.0 { -len } else { w + len };
                s.y = self.rng.range(14.0, h - 10.0);
            }
        }
        let swirl = swirl_of(atm.wind);
        for l in &mut self.leaves {
            l.x += dir * (50.0 + 420.0 * atm.wind) * l.a * dt;
            l.y += (8.0 + 14.0 * l.b) * dt;
            // A leaf in a gale goes round as it goes along, which is what rolls it up.
            l.phase += (1.6 + 5.5 * swirl + 2.0 * l.b) * dt;
            if l.x > w + 40.0 || l.x < -40.0 {
                l.x = if dir > 0.0 { -30.0 } else { w + 30.0 };
                l.y = self.rng.range(h * 0.25, h * 0.95);
            }
            if l.y > h + 20.0 {
                l.y = self.rng.range(-10.0, h * 0.4);
            }
        }

        for m in &mut self.motes {
            m.x += (6.0 * dir * (0.5 + atm.wind) + 8.0 * (self.t * 0.4 + m.phase).sin()) * dt;
            m.y += 5.0 * (self.t * 0.3 + m.phase * 2.0).cos() * dt;
            m.x = m.x.rem_euclid(w);
            m.y = m.y.rem_euclid(h);
        }
        for c in &mut self.clouds {
            c.x += dir * (5.0 + 55.0 * atm.wind) * c.a * dt;
            if c.x > w + 120.0 {
                c.x = -120.0;
            } else if c.x < -120.0 {
                c.x = w + 120.0;
            }
        }
        for m in &mut self.mist {
            m.y -= 9.0 * m.a * dt;
            m.x += 5.0 * dir * dt;
            if m.y < h * 0.3 {
                m.y = h + 20.0;
                m.x = self.rng.range(0.0, w);
            }
            m.x = m.x.rem_euclid(w);
        }

        // Thunder weather: a bolt now and then, bright at first and gone in a third of a second.
        self.flash = (self.flash - dt * 3.2).max(0.0);
        if atm.thunder {
            self.next_bolt -= dt;
            if self.next_bolt <= 0.0 {
                self.next_bolt = self.rng.range(2.5, 7.0);
                self.flash = 1.0;
                self.bolt = self.make_bolt();
                self.thunder_in = Some(self.rng.range(0.4, 2.2));
            }
        }
        if let Some(left) = self.thunder_in.as_mut() {
            *left -= dt;
            if *left <= 0.0 {
                self.thunder_in = None;
                self.rumble = true;
            }
        }
    }

    /// Whether the thunder of a bolt has arrived since this was last asked.
    pub fn take_rumble(&mut self) -> bool {
        std::mem::take(&mut self.rumble)
    }

    fn make_bolt(&mut self) -> Vec<(f64, f64)> {
        let (w, h) = (self.w, self.h);
        let mut x = self.rng.range(w * 0.15, w * 0.85);
        let mut y = 0.0;
        let mut points = vec![(x, y)];
        while y < h * 0.8 {
            y += self.rng.range(14.0, 30.0);
            x += self.rng.range(-22.0, 22.0);
            points.push((x, y));
        }
        points
    }

    pub fn flash(&self) -> f64 {
        self.flash
    }
}

/// How much of a gale's turning a wind has: none below a strong wind, all of it in a storm.
fn swirl_of(wind: f64) -> f64 {
    ((wind - 0.55) / 0.35).clamp(0.0, 1.0)
}

fn rain_length(rain: f64) -> f64 {
    6.0 + 16.0 * rain
}

fn rgba(r: f64, g: f64, b: f64, a: f64) -> Retained<NSColor> {
    NSColor::colorWithSRGBRed_green_blue_alpha(r, g, b, a.clamp(0.0, 1.0))
}

fn pt(x: f64, y: f64, h: f64) -> NSPoint {
    NSPoint::new(x, h - y)
}

/// A soft round light: `rgb` at `alpha` in the middle, nothing at `radius`.
fn glow(x: f64, y: f64, radius: f64, rgb: (f64, f64, f64), alpha: f64, h: f64) {
    if alpha <= 0.002 {
        return;
    }
    let Some(gradient) = NSGradient::initWithStartingColor_endingColor(
        NSGradient::alloc(),
        &rgba(rgb.0, rgb.1, rgb.2, alpha),
        &rgba(rgb.0, rgb.1, rgb.2, 0.0),
    ) else {
        return;
    };
    let centre = pt(x, y, h);
    gradient.drawFromCenter_radius_toCenter_radius_options(
        centre,
        0.0,
        centre,
        radius,
        NSGradientDrawingOptions::empty(),
    );
}

fn dot(path: &NSBezierPath, x: f64, y: f64, r: f64, h: f64) {
    path.appendBezierPathWithOvalInRect(NSRect::new(
        NSPoint::new(x - r, h - y - r),
        NSSize::new(r * 2.0, r * 2.0),
    ));
}

fn lerp(a: f64, b: f64, t: f64) -> f64 {
    a + (b - a) * t.clamp(0.0, 1.0)
}

fn paint_sun(sim: &Sim, h: f64) {
    let atm = sim.atm;
    let (w, t) = (sim.w, sim.t);
    let p = atm.progress.clamp(0.0, 1.0);
    // The sun climbs from the left and goes down at the right; how high it is sets its colour and its strength.
    let height = (PI * p).sin();
    let (x, y) = (w * (0.10 + 0.80 * p), h * (0.86 - 0.58 * height));
    // The sky hides it, and so does the smoke of dirty air, which also turns what is left of it red.
    let hidden = (atm.cloud * 0.75 + atm.rain * 0.6 + atm.fog * 0.5 + atm.snow * 0.3).min(0.95);
    let strength = (1.0 - hidden) * (1.0 - atm.haze * 0.35);
    let low = (1.0 - height * 3.0).clamp(0.0, 1.0);
    let smoke = atm.haze * 0.6;
    let rgb = (
        1.0,
        lerp(lerp(0.86, 0.5, low), 0.45, smoke),
        lerp(lerp(0.40, 0.18, low), 0.2, smoke),
    );
    glow(x, y, 150.0, rgb, 0.30 * strength, h);
    glow(x, y, 62.0, rgb, 0.55 * strength, h);
    if strength < 0.12 {
        return;
    }
    // Rays turn slowly round a bright disc.
    let rays = NSBezierPath::bezierPath();
    for i in 0..14 {
        let a = t * 0.18 + f64::from(i) * TAU / 14.0;
        let (s, c) = a.sin_cos();
        let (inner, outer) = (21.0, 29.0 + 7.0 * (t * 1.4 + f64::from(i) * 1.7).sin());
        rays.moveToPoint(pt(x + c * inner, y + s * inner, h));
        rays.lineToPoint(pt(x + c * outer, y + s * outer, h));
    }
    rays.setLineWidth(1.6);
    rgba(rgb.0, rgb.1, rgb.2, 0.55 * strength).setStroke();
    rays.stroke();
    rgba(
        1.0,
        lerp(0.95, 0.75, low),
        lerp(0.7, 0.4, low),
        0.95 * strength,
    )
    .setFill();
    let disc = NSBezierPath::bezierPath();
    dot(&disc, x, y, 15.0, h);
    disc.fill();
}

fn paint_moon(sim: &Sim, h: f64) {
    let atm = sim.atm;
    let (w, t) = (sim.w, sim.t);
    let veiled = (atm.cloud * 0.7 + atm.rain * 0.6 + atm.fog * 0.5).min(0.9);
    // Stars that twinkle, where the sky is clear enough to see them.
    let stars = NSBezierPath::bezierPath();
    for s in &sim.stars {
        let twinkle = 0.55 + 0.45 * (t * (1.2 + s.b * 2.5) + s.phase).sin();
        if s.y < h * 0.7 {
            dot(&stars, s.x, s.y, 0.6 + s.b * 0.9 * twinkle, h);
        }
    }
    rgba(0.85, 0.9, 1.0, 0.75 * (1.0 - veiled)).setFill();
    stars.fill();
    let p = atm.progress.clamp(0.0, 1.0);
    let height = (PI * p).sin();
    let (x, y) = (w * (0.10 + 0.80 * p), h * (0.80 - 0.50 * height));
    let strength = 1.0 - veiled;
    glow(x, y, 120.0, (0.7, 0.78, 1.0), 0.26 * strength, h);
    glow(x, y, 48.0, (0.85, 0.9, 1.0), 0.40 * strength, h);
    rgba(0.93, 0.95, 1.0, 0.95 * strength).setFill();
    let disc = NSBezierPath::bezierPath();
    dot(&disc, x, y, 13.0, h);
    disc.fill();
    rgba(0.62, 0.68, 0.85, 0.35 * strength).setFill();
    let craters = NSBezierPath::bezierPath();
    dot(&craters, x - 4.0, y - 3.0, 3.2, h);
    dot(&craters, x + 5.0, y + 3.0, 2.4, h);
    dot(&craters, x + 1.0, y - 7.0, 1.6, h);
    craters.fill();
}

fn paint_clouds(sim: &Sim, h: f64) {
    let atm = sim.atm;
    let dark = (atm.rain * 0.6 + if atm.thunder { 0.5 } else { 0.0 }).min(1.0);
    let rgb = (
        lerp(0.92, 0.42, dark),
        lerp(0.94, 0.46, dark),
        lerp(1.0, 0.58, dark),
    );
    let alpha = 0.05 + 0.10 * atm.cloud.max(atm.rain) + 0.06 * dark;
    for c in &sim.clouds {
        let scale = 0.8 + c.b * 0.7;
        let y = 16.0 + c.b * h * 0.34;
        for (dx, dy, r) in [
            (-34.0, 6.0, 30.0),
            (0.0, -4.0, 40.0),
            (34.0, 5.0, 32.0),
            (62.0, 10.0, 22.0),
        ] {
            glow(
                c.x + dx * scale,
                y + dy * scale,
                r * scale * 1.5,
                rgb,
                alpha,
                h,
            );
        }
    }
}

fn paint_rain(sim: &Sim, h: f64) {
    let atm = sim.atm;
    if sim.drops.is_empty() {
        return;
    }
    let dir = if atm.wind_dir < 0.0 { -1.0 } else { 1.0 };
    let slant = dir * (30.0 + atm.wind * 360.0) / (300.0 + 520.0 * atm.rain);
    let len = rain_length(atm.rain);
    let path = NSBezierPath::bezierPath();
    for d in &sim.drops {
        path.moveToPoint(pt(d.x, d.y, h));
        path.lineToPoint(pt(d.x - slant * len, d.y - len * d.a, h));
    }
    // A drizzle is fine and faint, a downpour thick and bright.
    path.setLineWidth(0.7 + 1.2 * atm.rain);
    rgba(0.66, 0.82, 1.0, 0.30 + 0.35 * atm.rain).setStroke();
    path.stroke();
    let rings = NSBezierPath::bezierPath();
    for &(x, y, age) in &sim.ripples {
        let (rx, ry) = (3.0 + age * 34.0, 1.0 + age * 8.0);
        rings.appendBezierPathWithOvalInRect(NSRect::new(
            NSPoint::new(x - rx, h - y - ry),
            NSSize::new(rx * 2.0, ry * 2.0),
        ));
    }
    rings.setLineWidth(0.8);
    rgba(0.7, 0.85, 1.0, 0.35).setStroke();
    rings.stroke();
}

fn paint_snow(sim: &Sim, h: f64) {
    if sim.flakes.is_empty() {
        return;
    }
    let path = NSBezierPath::bezierPath();
    for f in &sim.flakes {
        dot(&path, f.x, f.y, 1.1 + f.b * 2.0, h);
    }
    rgba(0.96, 0.98, 1.0, 0.85).setFill();
    path.fill();
}

fn paint_wind(sim: &Sim, h: f64) {
    let atm = sim.atm;
    let dir = if atm.wind_dir < 0.0 { -1.0 } else { 1.0 };
    let len = 40.0 + 110.0 * atm.wind;
    let lines = NSBezierPath::bezierPath();
    for s in &sim.streaks {
        // A long thin ribbon that waves a little, drawn from its tail to its head.
        for k in 0..=6 {
            let f = f64::from(k) / 6.0;
            let x = s.x - dir * len * (1.0 - f);
            let y = s.y + (4.0 + 8.0 * atm.wind) * (sim.t * 3.0 + s.phase + f * 3.0).sin();
            if k == 0 {
                lines.moveToPoint(pt(x, y, h));
            } else {
                lines.lineToPoint(pt(x, y, h));
            }
        }
    }
    lines.setLineWidth(1.1);
    rgba(0.9, 0.96, 1.0, 0.10 + 0.20 * atm.wind).setStroke();
    lines.stroke();

    let swirl = swirl_of(atm.wind);
    for l in &sim.leaves {
        // Going round in the wind: the wider the circle, the stronger the gale.
        let radius = 6.0 * l.b + swirl * (16.0 + 22.0 * l.b);
        let x = l.x + radius * l.phase.cos();
        let y = l.y + radius * 0.7 * l.phase.sin() + 10.0 * (l.phase * 0.5).sin();
        let turn = l.phase * 1.4 + l.b * TAU;
        paint_leaf(x, y, turn, 5.0 + 3.0 * l.b, atm.leaf, 0.85, h);
    }
}

fn paint_leaf(x: f64, y: f64, turn: f64, size: f64, rgb: (f64, f64, f64), alpha: f64, h: f64) {
    let (s, c) = turn.sin_cos();
    // The outline in the leaf's own frame, from the stalk to the tip and back; then turned and placed.
    let outline = [
        (-1.0, 0.0),
        (-0.4, 0.55),
        (0.35, 0.6),
        (1.0, 0.0),
        (0.35, -0.6),
        (-0.4, -0.55),
    ];
    let at = |(lx, ly): (f64, f64)| {
        let (lx, ly) = (lx * size, ly * size);
        pt(x + lx * c - ly * s, y + lx * s + ly * c, h)
    };
    let path = NSBezierPath::bezierPath();
    path.moveToPoint(at(outline[0]));
    for p in &outline[1..] {
        path.lineToPoint(at(*p));
    }
    path.closePath();
    rgba(rgb.0, rgb.1, rgb.2, alpha).setFill();
    path.fill();
    let rib = NSBezierPath::bezierPath();
    rib.moveToPoint(at((-1.2, 0.0)));
    rib.lineToPoint(at((0.8, 0.0)));
    rib.setLineWidth(0.7);
    rgba(rgb.0 * 0.6, rgb.1 * 0.6, rgb.2 * 0.6, alpha).setStroke();
    rib.stroke();
}

fn paint_cold(sim: &Sim, h: f64) {
    let atm = sim.atm;
    let w = sim.w;
    if atm.cold <= 0.0 {
        return;
    }
    // Frost creeps in from the edges, the colder the further.
    if let Some(gradient) = NSGradient::initWithStartingColor_endingColor(
        NSGradient::alloc(),
        &rgba(0.7, 0.88, 1.0, 0.0),
        &rgba(0.7, 0.88, 1.0, 0.10 + 0.34 * atm.cold),
    ) {
        let centre = NSPoint::new(w / 2.0, h / 2.0);
        // The colour past the outer circle is drawn out to the corners.
        gradient.drawFromCenter_radius_toCenter_radius_options(
            centre,
            h * (0.62 - 0.22 * atm.cold),
            centre,
            w * 0.62,
            NSGradientDrawingOptions::DrawsAfterEndingLocation,
        );
    }
    for m in &sim.mist {
        glow(
            m.x,
            m.y,
            44.0 + 30.0 * m.b,
            (0.85, 0.94, 1.0),
            0.05 + 0.09 * atm.cold,
            h,
        );
    }
    // Ice that glints: a small cross that swells and fades.
    let glints = NSBezierPath::bezierPath();
    for s in &sim.sparkles {
        let k = (sim.t * 1.6 + s.phase).sin().max(0.0).powi(3);
        let r = 1.0 + 4.5 * k * (0.6 + s.b * 0.6);
        glints.moveToPoint(pt(s.x - r, s.y, h));
        glints.lineToPoint(pt(s.x + r, s.y, h));
        glints.moveToPoint(pt(s.x, s.y - r, h));
        glints.lineToPoint(pt(s.x, s.y + r, h));
    }
    glints.setLineWidth(0.9);
    rgba(0.85, 0.96, 1.0, 0.8).setStroke();
    glints.stroke();
}

fn paint_haze(sim: &Sim, h: f64, rect: NSRect) {
    let atm = sim.atm;
    if atm.haze <= 0.0 {
        return;
    }
    rgba(0.66, 0.55, 0.38, atm.haze * 0.26).setFill();
    NSBezierPath::bezierPathWithRect(rect).fill();
    let dust = NSBezierPath::bezierPath();
    for m in &sim.motes {
        dot(&dust, m.x, m.y, 0.8 + m.b * 1.5, h);
    }
    rgba(0.82, 0.72, 0.54, 0.16 + 0.22 * atm.haze).setFill();
    dust.fill();
}

fn paint_fog(sim: &Sim, h: f64) {
    if sim.atm.fog <= 0.0 {
        return;
    }
    for k in 0..7 {
        let drift = (sim.t * (3.0 + f64::from(k % 3)) + f64::from(k) * 90.0)
            .rem_euclid(sim.w * 1.4)
            - sim.w * 0.2;
        let y = h * (0.3 + 0.12 * f64::from(k % 4));
        glow(drift, y, 110.0, (0.82, 0.85, 0.9), 0.20 * sim.atm.fog, h);
    }
}

/// The whole sky into `rect`, whose height is `h`.
fn paint(sim: &Sim, rect: NSRect) {
    let h = rect.size.height;
    let atm = sim.atm;
    // Nothing falls or glows outside the page, nor past the rounded corners of the notch under it.
    NSBezierPath::bezierPathWithRoundedRect_xRadius_yRadius(rect, 18.0, 18.0).addClip();
    // The colour of the air: blue by day, gold when the sun is low, deep blue at night, and grey in a storm.
    let height = (PI * atm.progress.clamp(0.0, 1.0)).sin();
    let clear = 1.0 - atm.cloud * 0.6;
    let (top, bottom) = if atm.day {
        let low = (1.0 - height * 3.0).clamp(0.0, 1.0);
        (
            rgba(0.25, 0.55, 1.0, 0.20 * clear),
            rgba(
                1.0,
                lerp(0.8, 0.5, low),
                lerp(0.55, 0.25, low),
                (0.04 + 0.20 * low) * clear,
            ),
        )
    } else {
        (rgba(0.05, 0.08, 0.30, 0.36), rgba(0.10, 0.12, 0.32, 0.08))
    };
    if let Some(gradient) =
        NSGradient::initWithStartingColor_endingColor(NSGradient::alloc(), &top, &bottom)
    {
        gradient.drawInRect_angle(rect, -90.0);
    }
    let gloom = (atm.rain * 0.8 + if atm.thunder { 0.4 } else { 0.0 }).min(1.0);
    if gloom > 0.0 {
        rgba(0.08, 0.10, 0.16, 0.34 * gloom).setFill();
        NSBezierPath::bezierPathWithRect(rect).fill();
    }
    if atm.day {
        paint_sun(sim, h);
    } else {
        paint_moon(sim, h);
    }
    paint_clouds(sim, h);
    paint_fog(sim, h);
    paint_haze(sim, h, rect);
    paint_wind(sim, h);
    paint_rain(sim, h);
    paint_snow(sim, h);
    paint_cold(sim, h);
    let flash = sim.flash();
    if flash > 0.0 {
        rgba(0.9, 0.93, 1.0, 0.32 * flash).setFill();
        NSBezierPath::bezierPathWithRect(rect).fill();
        if flash > 0.45 && sim.bolt.len() > 1 {
            for (width, alpha) in [(7.0, 0.20), (2.2, 0.95)] {
                let bolt = NSBezierPath::bezierPath();
                for (i, &(x, y)) in sim.bolt.iter().enumerate() {
                    if i == 0 {
                        bolt.moveToPoint(pt(x, y, h));
                    } else {
                        bolt.lineToPoint(pt(x, y, h));
                    }
                }
                bolt.setLineWidth(width);
                rgba(0.85, 0.9, 1.0, alpha * flash).setStroke();
                bolt.stroke();
            }
        }
    }
}

pub struct FxIvars {
    sim: RefCell<Option<Sim>>,
}

define_class!(
    // SAFETY: NSView allows subclassing; the ivars are only touched on the main thread and need no Drop.
    #[unsafe(super(NSView))]
    #[thread_kind = MainThreadOnly]
    #[name = "FigWeatherFx"]
    #[ivars = FxIvars]
    pub struct FxView;

    impl FxView {
        #[unsafe(method(drawRect:))]
        fn draw_rect(&self, _dirty: NSRect) {
            if let Some(sim) = self.ivars().sim.borrow().as_ref() {
                paint(sim, self.bounds());
            }
        }

        // The sky takes no clicks: they go through to the tab on top of it.
        #[unsafe(method(hitTest:))]
        fn hit_test(&self, _point: NSPoint) -> *mut NSView {
            std::ptr::null_mut()
        }
    }

    unsafe impl NSObjectProtocol for FxView {}
);

impl FxView {
    pub fn new(mtm: MainThreadMarker, frame: NSRect) -> Retained<Self> {
        let this = mtm.alloc::<Self>().set_ivars(FxIvars {
            sim: RefCell::new(None),
        });
        // SAFETY: designated initializer with valid arguments on the main thread.
        unsafe { msg_send![super(this), initWithFrame: frame] }
    }

    /// Changes the sky. The same one again keeps what is in flight, so a refresh of the forecast does not restart it.
    pub fn set(&self, atmosphere: Option<Atmosphere>) {
        let size = self.bounds().size;
        let mut slot = self.ivars().sim.borrow_mut();
        match atmosphere {
            None => *slot = None,
            Some(atm) => {
                if slot.as_ref().map(Sim::atmosphere) != Some(atm) {
                    *slot = Some(Sim::new(atm, size.width, size.height));
                }
            }
        }
        drop(slot);
        self.setNeedsDisplay(true);
    }

    /// One frame of `dt` seconds. True when the thunder of a bolt has just arrived.
    pub fn tick(&self, dt: f64) -> bool {
        let mut slot = self.ivars().sim.borrow_mut();
        let Some(sim) = slot.as_mut() else {
            return false;
        };
        sim.step(dt);
        self.setNeedsDisplay(true);
        sim.take_rumble()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn storm() -> Atmosphere {
        Atmosphere {
            day: true,
            progress: 0.5,
            cloud: 1.0,
            rain: 0.9,
            thunder: true,
            wind: 0.7,
            wind_dir: 1.0,
            leaf: (0.3, 0.7, 0.3),
            ..Atmosphere::default()
        }
    }

    #[test]
    fn a_clear_calm_sky_has_nothing_falling_or_blowing() {
        let sim = Sim::new(
            Atmosphere {
                day: true,
                progress: 0.5,
                ..Atmosphere::default()
            },
            580.0,
            250.0,
        );
        assert!(sim.drops.is_empty() && sim.flakes.is_empty() && sim.leaves.is_empty());
        assert!(sim.streaks.is_empty() && sim.motes.is_empty() && sim.clouds.is_empty());
    }

    #[test]
    fn a_drizzle_is_fewer_and_finer_than_a_downpour_and_wind_brings_leaves() {
        let make = |rain: f64, wind: f64| {
            Sim::new(
                Atmosphere {
                    rain,
                    wind,
                    ..storm()
                },
                580.0,
                250.0,
            )
        };
        let (fine, heavy) = (make(0.15, 0.0), make(0.9, 0.0));
        assert!(fine.drops.len() < heavy.drops.len());
        assert!(rain_length(0.15) < rain_length(0.9));
        assert!(
            make(0.0, 0.1).leaves.is_empty(),
            "a light breeze lifts no leaf"
        );
        assert!(make(0.0, 0.5).leaves.len() < make(0.0, 1.0).leaves.len());
        assert_eq!(swirl_of(0.3), 0.0);
        assert_eq!(swirl_of(1.0), 1.0);
    }

    #[test]
    fn everything_stays_in_the_area_while_time_runs_and_lightning_strikes_only_in_thunder() {
        let mut sim = Sim::new(storm(), 580.0, 250.0);
        let mut flashed = false;
        for _ in 0..1800 {
            sim.step(1.0 / 30.0);
            flashed |= sim.flash() > 0.9;
            assert!(
                sim.leaves
                    .iter()
                    .all(|l| l.x.is_finite() && l.y.is_finite())
            );
            assert!(sim.drops.iter().all(|d| d.y <= 250.0 + 1.0));
            assert!(sim.ripples.len() <= 28);
        }
        assert!(flashed, "a minute of thunder weather without a bolt");
        let mut rumbles = 0;
        for _ in 0..1800 {
            sim.step(1.0 / 30.0);
            rumbles += usize::from(sim.take_rumble());
        }
        assert!(rumbles > 0, "every bolt is followed by its thunder");
        let mut dry = Sim::new(
            Atmosphere {
                thunder: false,
                ..storm()
            },
            580.0,
            250.0,
        );
        for _ in 0..1800 {
            dry.step(1.0 / 30.0);
            assert_eq!(dry.flash(), 0.0);
        }
    }
}
