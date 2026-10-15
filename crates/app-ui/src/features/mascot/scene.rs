//! Where the mascot is at each moment, as plain data for the stage to draw (`macos_sys::mascot`). Nothing
//! here touches the screen, so the story is tested like any other code; `examples/mascot_check.rs` includes
//! this file to draw the frames, which is why it uses nothing from the rest of the app.

use macos_sys::{
    NSPoint, NSRect,
    mascot::{Chip, Frame, Guard, Ray, Shape},
};

/// The side of a frame of every sheet, in pixels (`docs/mascots/build.py` CELL).
pub const CELL: f64 = 48.0;

/// The frames of every sheet, in order (`docs/mascots/build.py` FRAMES).
pub const IDLE: usize = 0;
pub const LOOK_LEFT: usize = 2;
pub const LOOK_RIGHT: usize = 3;
pub const ALERT: usize = 4;
pub const PULL_A: usize = 5;
pub const PULL_B: usize = 6;
pub const HAPPY: usize = 7;
pub const PROUD: usize = 8;
pub const SAD: usize = 9;
pub const WAVE: usize = 10;

/// How a character moves things, beyond its picture.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Style {
    /// What reaches for a copy and holds it.
    pub ray: Ray,
    /// What strikes first, when it is something else (the wizard's bolt before his magic).
    pub strike: Ray,
    pub haul: Haul,
    pub guard: Guard,
    /// Hangs from the notch on a thread.
    pub hangs: bool,
    /// Points per pixel of the sheet.
    pub scale: f64,
}

impl Style {
    fn side(&self) -> f64 {
        CELL * self.scale
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Haul {
    /// In three pulls.
    Tugs,
    /// All at once, with an overshoot.
    Yank,
    /// Drifting on a wave.
    Float,
}

/// How a file dragged out of the shelf is seen off.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Farewell {
    /// Escorted to where it is dropped.
    Guard,
    /// Held on to until it is gone, then waved goodbye.
    Regret,
}

/// Where things are, in screen coordinates.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Place {
    /// The notch as it is drawn now: the character comes out of it and things go into it.
    pub hole: NSRect,
    /// The screen the notch is on; nothing leaves it.
    pub screen: NSRect,
}

impl Place {
    fn edge(&self) -> f64 {
        self.hole.origin.y
    }

    /// Where the character waits, just under the notch.
    fn home(&self, style: &Style) -> NSPoint {
        let mid = self.hole.origin.x + self.hole.size.width / 2.0;
        NSPoint::new(mid, self.edge() - style.side() * 0.42)
    }

    /// Up inside the notch, out of sight.
    fn inside(&self, style: &Style) -> NSPoint {
        let home = self.home(style);
        NSPoint::new(home.x, self.edge() + style.side() * 0.6)
    }

    /// A point brought onto the screen, away from its edges.
    fn clamp(&self, p: NSPoint) -> NSPoint {
        let s = self.screen;
        let pad = 24.0;
        NSPoint::new(
            p.x.clamp(s.origin.x + pad, s.origin.x + s.size.width - pad),
            p.y.clamp(s.origin.y + pad, s.origin.y + s.size.height - pad),
        )
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Ease {
    Linear,
    In,
    Out,
    InOut,
    /// Past the mark and back.
    Spring,
    /// Stays, then jumps at the key.
    Step,
}

fn ease(e: Ease, f: f64) -> f64 {
    let f = f.clamp(0.0, 1.0);
    match e {
        Ease::Linear => f,
        Ease::In => f * f * f,
        Ease::Out => 1.0 - (1.0 - f).powi(3),
        Ease::InOut if f < 0.5 => 4.0 * f * f * f,
        Ease::InOut => 1.0 - (2.0 - 2.0 * f).powi(3) / 2.0,
        Ease::Spring if f >= 1.0 => 1.0,
        Ease::Spring => 1.0 - (-6.0 * f).exp() * (9.0 * f).cos(),
        Ease::Step if f >= 1.0 => 1.0,
        Ease::Step => 0.0,
    }
}

trait Mix: Copy {
    fn mix(a: Self, b: Self, f: f64) -> Self;
}

impl Mix for f64 {
    fn mix(a: f64, b: f64, f: f64) -> f64 {
        a + (b - a) * f
    }
}

impl Mix for NSPoint {
    fn mix(a: NSPoint, b: NSPoint, f: f64) -> NSPoint {
        NSPoint::new(a.x + (b.x - a.x) * f, a.y + (b.y - a.y) * f)
    }
}

/// A value over time: it gets to each key's value at the key's time, moving the way the key says.
#[derive(Debug, Clone)]
struct Track<T> {
    keys: Vec<(f64, T, Ease)>,
}

impl<T: Mix> Track<T> {
    fn new(v: T) -> Self {
        Self {
            keys: vec![(0.0, v, Ease::Step)],
        }
    }

    fn to(mut self, t: f64, v: T, e: Ease) -> Self {
        self.keys.push((t, v, e));
        self
    }

    fn at(&self, t: f64) -> T {
        let mut prev = self.keys[0];
        for &(kt, kv, ke) in &self.keys[1..] {
            if t < kt {
                let span = kt - prev.0;
                let f = if span <= 0.0 {
                    1.0
                } else {
                    (t - prev.0) / span
                };
                return T::mix(prev.1, kv, ease(ke, f));
            }
            prev = (kt, kv, ke);
        }
        prev.1
    }
}

/// The frame of the sprite at `t`: the last one that started by then.
fn frame_at(frames: &[(f64, usize)], t: f64) -> usize {
    frames
        .iter()
        .take_while(|(start, _)| *start <= t)
        .last()
        .map_or(IDLE, |(_, f)| *f)
}

/// The spider's thread, from the notch straight down to the top of its body.
fn thread(place: &Place, at: NSPoint, style: &Style) -> Shape {
    Shape::Ray {
        kind: Ray::Silk,
        from: NSPoint::new(at.x, place.edge()),
        to: NSPoint::new(at.x, at.y + style.side() * 0.22),
        width: 0.0,
        alpha: 1.0,
    }
}

fn sprite(frame: usize, at: NSPoint, style: &Style) -> Shape {
    Shape::Sprite {
        frame,
        at,
        scale: style.scale,
    }
}

/// Half the width of what a chip shows, for the far end of a beam.
fn half_width(chip: &Chip) -> f64 {
    match chip {
        Chip::Text => 23.0,
        Chip::Picture => 20.0,
        Chip::File(_) => 18.0,
    }
}

/// A short animation, worked out once and read at any moment.
pub struct Script {
    pub seconds: f64,
    at: Box<dyn Fn(f64) -> Frame>,
}

impl Script {
    pub fn at(&self, t: f64) -> Frame {
        (self.at)(t)
    }
}

/// Out of the notch, a ray at `target`, a copy brought back and into the notch, a happy moment, and back in.
pub fn fetch(style: Style, place: Place, target: NSPoint, chip: Chip) -> Script {
    let home = place.home(&style);
    let inside = place.inside(&style);
    let target = place.clamp(target);
    let side = style.side();
    let hold = NSPoint::new(home.x, home.y - side * 0.3);
    let into = NSPoint::new(home.x, place.edge() + 20.0);
    let along = |f: f64| NSPoint::mix(target, hold, f);
    // The ray goes out over 0.32 to `reached` (a flick for a tongue), sticks, and from 0.52 brings the copy
    // back until 0.96; the copy goes into the notch by 1.12.
    let reached = if style.ray == Ray::Tongue { 0.38 } else { 0.46 };
    let haul: Vec<(f64, f64, Ease)> = match style.haul {
        Haul::Tugs => vec![
            (0.60, 0.4, Ease::Out),
            (0.70, 0.4, Ease::Step),
            (0.78, 0.75, Ease::Out),
            (0.88, 0.75, Ease::Step),
            (0.96, 1.0, Ease::Out),
        ],
        Haul::Yank => vec![(0.64, 1.0, Ease::Spring), (0.96, 1.0, Ease::Step)],
        Haul::Float => (1..=5)
            .map(|i| (0.52 + 0.088 * f64::from(i), f64::from(i) / 5.0, Ease::InOut))
            .collect(),
    };
    let wave = |i: usize, f: f64| {
        if style.haul == Haul::Float && f < 1.0 {
            if i % 2 == 0 { 10.0 } else { -10.0 }
        } else {
            0.0
        }
    };
    let mut path = Track::new(target).to(0.52, target, Ease::Step);
    for (i, &(t, f, e)) in haul.iter().enumerate() {
        let p = along(f);
        path = path.to(t, NSPoint::new(p.x + wave(i, f), p.y), e);
    }
    let chip_at = path.clone().to(1.12, into, Ease::InOut);
    let chip_scale = Track::new(1.0)
        .to(0.52, 1.0, Ease::Step)
        .to(0.96, 0.55, Ease::Linear)
        .to(1.12, 0.35, Ease::Linear);
    let chip_alpha = Track::new(0.0)
        .to(0.52, 1.0, Ease::Step)
        .to(1.13, 0.0, Ease::Step);
    let ray_out = Track::new(0.0)
        .to(0.32, 0.0, Ease::Step)
        .to(reached, 1.0, Ease::Out)
        .to(0.97, 0.0, Ease::Step);
    let let_go = if style.haul == Haul::Yank { 0.62 } else { 0.97 };
    let stick = Track::new(0.0)
        .to(reached, 1.0, Ease::Step)
        .to(let_go, 0.0, Ease::Step);
    let actor = Track::new(inside)
        .to(0.2, home, Ease::Out)
        .to(1.4, home, Ease::Step)
        .to(1.42, NSPoint::new(home.x, home.y + 6.0), Ease::Out)
        .to(1.66, inside, Ease::In);
    let look = if target.x < home.x - 40.0 {
        LOOK_LEFT
    } else if target.x > home.x + 40.0 {
        LOOK_RIGHT
    } else {
        ALERT
    };
    let mut frames = vec![(0.0, ALERT), (0.2, look), (0.3, ALERT), (0.52, PULL_A)];
    for (i, &(t, _, _)) in haul.iter().enumerate().filter(|(_, k)| k.2 != Ease::Step) {
        if t < 0.96 {
            frames.push((t, if i % 2 == 0 { PULL_B } else { PULL_A }));
        }
    }
    frames.extend([(0.98, ALERT), (1.12, HAPPY), (1.5, IDLE)]);
    let width = half_width(&chip) + 6.0;
    let strike_ends = if style.strike == style.ray { 0.0 } else { 0.52 };
    Script {
        seconds: 1.66,
        at: Box::new(move |t| {
            let a = actor.at(t);
            let mut shapes = Vec::new();
            if style.hangs {
                shapes.push(thread(&place, a, &style));
            }
            let out = ray_out.at(t);
            let end = path.at(t);
            if out > 0.0 {
                let kind = if t < strike_ends {
                    style.strike
                } else {
                    style.ray
                };
                shapes.push(Shape::Ray {
                    kind,
                    from: a,
                    to: NSPoint::mix(a, end, out),
                    width,
                    alpha: 1.0,
                });
            }
            let alpha = chip_alpha.at(t);
            if alpha > 0.0 {
                shapes.push(Shape::Chip {
                    chip: chip.clone(),
                    at: chip_at.at(t),
                    scale: chip_scale.at(t),
                    alpha,
                });
            }
            if stick.at(t) > 0.0 && style.ray != Ray::Beam {
                shapes.push(Shape::Spark {
                    kind: if t < strike_ends {
                        style.strike
                    } else {
                        style.ray
                    },
                    at: end,
                });
            }
            shapes.push(sprite(frame_at(&frames, t), a, &style));
            Frame {
                shapes,
                hole: Some(place.hole),
            }
        }),
    }
}

/// A file being dragged out of the shelf, followed for as long as the drag lasts.
pub struct Follow {
    style: Style,
    place: Place,
    farewell: Farewell,
    actor: NSPoint,
    pointer: NSPoint,
    last: f64,
}

impl Follow {
    pub fn new(style: Style, place: Place, farewell: Farewell, pointer: NSPoint) -> Self {
        Self {
            actor: place.inside(&style),
            style,
            place,
            farewell,
            pointer,
            last: 0.0,
        }
    }

    pub fn moved(&mut self, pointer: NSPoint) {
        self.pointer = pointer;
    }

    /// The notch may close or open while the drag goes on.
    pub fn set_place(&mut self, place: Place) {
        self.place = place;
    }

    /// Where the character wants to be: beside the file, or above it under a beam, or held back near the notch.
    fn goal(&self) -> NSPoint {
        let p = self.place.clamp(self.pointer);
        match (self.farewell, self.style.guard) {
            (Farewell::Guard, Guard::Cone) => NSPoint::new(p.x, p.y + 70.0),
            (Farewell::Guard, _) => NSPoint::new(p.x - 64.0, p.y + 6.0),
            (Farewell::Regret, _) => {
                let home = self.place.home(&self.style);
                let (dx, dy) = (p.x - home.x, p.y - home.y);
                let pull = (dx.hypot(dy) * 0.08).min(26.0);
                let len = dx.hypot(dy).max(1.0);
                NSPoint::new(home.x + dx / len * pull, home.y + dy / len * pull)
            }
        }
    }

    /// The frame at `t` seconds into the drag. It moves the character, so it is called once per frame.
    pub fn frame(&mut self, t: f64) -> Frame {
        let dt = (t - self.last).clamp(0.0, 0.1);
        self.last = t;
        // Follows with a little lag, which reads as effort rather than as being glued to the pointer.
        let k = 1.0 - (-dt * 10.0).exp();
        self.actor = NSPoint::mix(self.actor, self.goal(), k);
        let (style, a, p) = (&self.style, self.actor, self.pointer);
        let mut shapes = Vec::new();
        if style.hangs {
            shapes.push(thread(&self.place, a, style));
        }
        let frame = match self.farewell {
            Farewell::Guard => {
                shapes.push(Shape::Guard {
                    kind: style.guard,
                    at: p,
                    size: 30.0,
                    turn: t * 40.0,
                });
                if t < 0.3 {
                    ALERT
                } else if (t / 0.35) as i64 % 2 == 0 {
                    LOOK_LEFT
                } else {
                    LOOK_RIGHT
                }
            }
            Farewell::Regret => {
                shapes.push(Shape::Ray {
                    kind: style.ray,
                    from: a,
                    to: p,
                    width: 16.0,
                    alpha: 1.0,
                });
                if style.ray != Ray::Beam {
                    shapes.push(Shape::Spark {
                        kind: style.ray,
                        at: p,
                    });
                }
                if (t / 0.16) as i64 % 2 == 0 {
                    PULL_A
                } else {
                    PULL_B
                }
            }
        };
        shapes.push(sprite(frame, a, style));
        Frame {
            shapes,
            hole: Some(self.place.hole),
        }
    }

    /// What happens once the drag is over: the file landed (`dropped`) or went back to the shelf.
    pub fn finale(&self, dropped: bool) -> Script {
        let (style, place) = (self.style, self.place);
        let (start, p) = (self.actor, self.pointer);
        let home = place.home(&style);
        let inside = place.inside(&style);
        match (self.farewell, dropped) {
            (Farewell::Guard, true) => {
                let actor = Track::new(start)
                    .to(0.7, start, Ease::Step)
                    .to(1.15, home, Ease::InOut)
                    .to(1.4, inside, Ease::In);
                let frames = [(0.0, PROUD), (0.7, HAPPY), (1.15, IDLE)];
                let shield = Track::new(1.0).to(0.25, 0.0, Ease::Linear);
                Script {
                    seconds: 1.4,
                    at: Box::new(move |t| {
                        let a = actor.at(t);
                        let mut shapes = Vec::new();
                        if style.hangs {
                            shapes.push(thread(&place, a, &style));
                        }
                        if shield.at(t) > 0.0 {
                            shapes.push(Shape::Guard {
                                kind: style.guard,
                                at: p,
                                size: 30.0 * (0.6 + 0.4 * shield.at(t)),
                                turn: t * 40.0,
                            });
                        }
                        shapes.push(sprite(frame_at(&frames, t), a, &style));
                        Frame {
                            shapes,
                            hole: Some(place.hole),
                        }
                    }),
                }
            }
            (Farewell::Regret, true) => {
                // The hold gives: two ends spring apart, the character bounces home, sad, and waves.
                let actor = Track::new(start)
                    .to(0.14, NSPoint::new(home.x, home.y + 10.0), Ease::Out)
                    .to(0.4, home, Ease::Spring)
                    .to(1.3, home, Ease::Step)
                    .to(1.55, inside, Ease::In);
                let frames = [
                    (0.0, ALERT),
                    (0.2, SAD),
                    (0.45, WAVE),
                    (0.62, SAD),
                    (0.8, WAVE),
                    (0.97, SAD),
                ];
                let recoil = Track::new(0.35).to(0.18, 0.0, Ease::Out);
                let tear = |t: f64, from: f64| -> Option<(f64, f64)> {
                    let f = (t - from) / 0.45;
                    (0.0..1.0).contains(&f).then_some((f * 22.0, 1.0 - f))
                };
                let side = style.side();
                Script {
                    seconds: 1.55,
                    at: Box::new(move |t| {
                        let a = actor.at(t);
                        let mut shapes = Vec::new();
                        if style.hangs {
                            shapes.push(thread(&place, a, &style));
                        }
                        let r = recoil.at(t);
                        if r > 0.0 {
                            shapes.push(Shape::Ray {
                                kind: style.ray,
                                from: a,
                                to: NSPoint::mix(a, p, r),
                                width: 16.0,
                                alpha: r / 0.35,
                            });
                            shapes.push(Shape::Ray {
                                kind: style.ray,
                                from: p,
                                to: NSPoint::mix(p, a, r),
                                width: 16.0,
                                alpha: r / 0.35,
                            });
                        }
                        shapes.push(sprite(frame_at(&frames, t), a, &style));
                        for from in [0.25, 0.75] {
                            if let Some((fall, alpha)) = tear(t, from) {
                                let eye = NSPoint::new(a.x - side * 0.1, a.y + side * 0.02 - fall);
                                shapes.push(Shape::Tear { at: eye, alpha });
                            }
                        }
                        Frame {
                            shapes,
                            hole: Some(place.hole),
                        }
                    }),
                }
            }
            (_, false) => {
                // The drop did not happen and the file stays on the shelf: relief, and home.
                let actor = Track::new(start)
                    .to(0.45, home, Ease::InOut)
                    .to(0.7, inside, Ease::In);
                let frames = [(0.0, ALERT), (0.15, HAPPY)];
                Script {
                    seconds: 0.7,
                    at: Box::new(move |t| {
                        let a = actor.at(t);
                        let mut shapes = Vec::new();
                        if style.hangs {
                            shapes.push(thread(&place, a, &style));
                        }
                        shapes.push(sprite(frame_at(&frames, t), a, &style));
                        Frame {
                            shapes,
                            hole: Some(place.hole),
                        }
                    }),
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use macos_sys::NSSize;

    fn spider() -> Style {
        Style {
            ray: Ray::Silk,
            strike: Ray::Silk,
            haul: Haul::Tugs,
            guard: Guard::Box,
            hangs: true,
            scale: 2.0,
        }
    }

    fn wizard() -> Style {
        Style {
            ray: Ray::Magic,
            strike: Ray::Bolt,
            haul: Haul::Float,
            guard: Guard::Ring,
            hangs: false,
            scale: 1.5,
        }
    }

    fn place() -> Place {
        Place {
            hole: NSRect::new(NSPoint::new(856.0, 1084.0), NSSize::new(208.0, 32.0)),
            screen: NSRect::new(NSPoint::new(0.0, 0.0), NSSize::new(1920.0, 1116.0)),
        }
    }

    fn sprite_of(frame: &Frame) -> (usize, NSPoint) {
        frame
            .shapes
            .iter()
            .find_map(|s| match s {
                Shape::Sprite { frame, at, .. } => Some((*frame, *at)),
                _ => None,
            })
            .expect("every frame shows the character")
    }

    fn chip_of(frame: &Frame) -> Option<NSPoint> {
        frame.shapes.iter().find_map(|s| match s {
            Shape::Chip { at, .. } => Some(*at),
            _ => None,
        })
    }

    #[test]
    fn a_fetch_starts_and_ends_inside_the_notch() {
        let script = fetch(spider(), place(), NSPoint::new(400.0, 500.0), Chip::Text);
        for t in [0.0, script.seconds] {
            let (_, at) = sprite_of(&script.at(t));
            assert!(
                at.y > place().hole.origin.y,
                "hidden behind the notch at {t}"
            );
        }
        let (_, waiting) = sprite_of(&script.at(0.3));
        assert!(
            waiting.y < place().hole.origin.y,
            "under the notch while it works"
        );
        assert!(script.at(0.0).hole == Some(place().hole));
    }

    #[test]
    fn the_copy_travels_from_the_target_into_the_notch() {
        let target = NSPoint::new(400.0, 500.0);
        let script = fetch(spider(), place(), target, Chip::Text);
        assert!(
            chip_of(&script.at(0.4)).is_none(),
            "nothing is carried before the ray sticks"
        );
        let start = chip_of(&script.at(0.52)).unwrap();
        assert!(
            (start.x - target.x).abs() < 1.0 && (start.y - target.y).abs() < 1.0,
            "{start:?}"
        );
        let middle = chip_of(&script.at(0.8)).unwrap();
        assert!(
            middle.y > target.y && middle.x > target.x,
            "on its way up toward the notch"
        );
        let last = chip_of(&script.at(1.11)).unwrap();
        assert!(
            last.y > place().hole.origin.y - 20.0,
            "at the notch at the end"
        );
        assert!(chip_of(&script.at(1.2)).is_none(), "gone into the notch");
    }

    #[test]
    fn a_target_off_the_screen_is_brought_onto_it() {
        let script = fetch(
            spider(),
            place(),
            NSPoint::new(-500.0, 5000.0),
            Chip::Picture,
        );
        let start = chip_of(&script.at(0.53)).unwrap();
        assert!(start.x >= 0.0 && start.y <= place().screen.size.height);
    }

    #[test]
    fn the_wizard_strikes_with_a_bolt_then_holds_with_magic() {
        let script = fetch(wizard(), place(), NSPoint::new(1500.0, 300.0), Chip::Text);
        let ray_at = |t: f64| {
            script.at(t).shapes.iter().find_map(|s| match s {
                Shape::Ray { kind, .. } => Some(*kind),
                _ => None,
            })
        };
        assert_eq!(ray_at(0.4), Some(Ray::Bolt));
        assert_eq!(ray_at(0.7), Some(Ray::Magic));
        assert_eq!(ray_at(1.2), None, "the ray is gone once the copy is home");
        assert_eq!(
            sprite_of(&script.at(0.25)).0,
            LOOK_RIGHT,
            "it looks where it shoots"
        );
    }

    #[test]
    fn only_the_spider_hangs_on_a_thread() {
        let threads = |style: Style| {
            fetch(style, place(), NSPoint::new(400.0, 500.0), Chip::Text)
                .at(0.3)
                .shapes
                .iter()
                .filter(
                    |s| matches!(s, Shape::Ray { kind: Ray::Silk, from, to, .. } if from.x == to.x),
                )
                .count()
        };
        assert_eq!(threads(spider()), 1);
        assert_eq!(threads(wizard()), 0);
    }

    #[test]
    fn a_guard_stays_beside_the_file_and_a_regret_stays_near_the_notch() {
        let mut guard = Follow::new(
            spider(),
            place(),
            Farewell::Guard,
            NSPoint::new(900.0, 900.0),
        );
        let mut regret = Follow::new(
            spider(),
            place(),
            Farewell::Regret,
            NSPoint::new(900.0, 900.0),
        );
        let file = NSPoint::new(600.0, 300.0);
        guard.moved(file);
        regret.moved(file);
        let (mut g, mut r) = (NSPoint::new(0.0, 0.0), NSPoint::new(0.0, 0.0));
        for i in 0..240 {
            let t = f64::from(i) / 60.0;
            g = sprite_of(&guard.frame(t)).1;
            r = sprite_of(&regret.frame(t)).1;
        }
        assert!(
            (g.x - (file.x - 64.0)).abs() < 2.0 && (g.y - (file.y + 6.0)).abs() < 2.0,
            "{g:?}"
        );
        let home = place().home(&spider());
        assert!(
            (r.x - home.x).hypot(r.y - home.y) <= 26.5,
            "held back near the notch: {r:?}"
        );
        assert!(r.x < home.x && r.y < home.y, "pulled toward the file");
        let tether = regret
            .frame(4.1)
            .shapes
            .iter()
            .any(|s| matches!(s, Shape::Ray { to, .. } if *to == file));
        assert!(tether, "a regret holds the file by its ray");
    }

    #[test]
    fn every_finale_ends_inside_the_notch() {
        for farewell in [Farewell::Guard, Farewell::Regret] {
            for dropped in [true, false] {
                let mut follow =
                    Follow::new(wizard(), place(), farewell, NSPoint::new(700.0, 400.0));
                follow.frame(0.5);
                let script = follow.finale(dropped);
                let (_, at) = sprite_of(&script.at(script.seconds));
                assert!(at.y > place().hole.origin.y, "{farewell:?} {dropped}");
            }
        }
        let mut follow = Follow::new(
            spider(),
            place(),
            Farewell::Regret,
            NSPoint::new(700.0, 400.0),
        );
        follow.frame(0.5);
        let script = follow.finale(true);
        assert!(
            script
                .at(0.3)
                .shapes
                .iter()
                .any(|s| matches!(s, Shape::Tear { .. })),
            "a tear falls"
        );
        assert_eq!(sprite_of(&script.at(0.5)).0, WAVE, "it waves goodbye");
        let proud = Follow::new(
            spider(),
            place(),
            Farewell::Guard,
            NSPoint::new(700.0, 400.0),
        )
        .finale(true);
        assert_eq!(
            sprite_of(&proud.at(0.1)).0,
            PROUD,
            "a guard salutes when the file lands"
        );
    }

    #[test]
    fn eases_start_at_zero_and_end_at_one() {
        for e in [
            Ease::Linear,
            Ease::In,
            Ease::Out,
            Ease::InOut,
            Ease::Spring,
            Ease::Step,
        ] {
            assert_eq!(ease(e, 0.0), 0.0, "{e:?}");
            assert!((ease(e, 1.0) - 1.0).abs() < 1e-9, "{e:?}");
        }
        assert!(ease(Ease::Spring, 0.35) > 1.0, "a spring overshoots");
    }
}
