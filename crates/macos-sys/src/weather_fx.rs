//! The moving sky behind the Weather tab: the sun or the moon where it is in the day, clouds, rain of every size,
//! lightning, snow, frost, wind with the leaves it carries and the dust of dirty air. The app says what the sky is like
//! as numbers (`Atmosphere`); the GPU paints the air itself (`weather_gpu`), and `Sim` works out what has a body of its
//! own: where each leaf is, and when lightning strikes and its thunder arrives. `Sim` knows nothing of AppKit, so its
//! rules can be tested.

use crate::weather_gpu::{Gpu, MAX_LEAVES, Uniforms};
use objc2::{
    AnyThread, DefinedClass, MainThreadMarker, MainThreadOnly, define_class, msg_send, rc::Retained,
};
use objc2_app_kit::{NSBitmapImageRep, NSDeviceRGBColorSpace, NSView};
use objc2_foundation::{NSObjectProtocol, NSPoint, NSRect, NSSize};
use objc2_quartz_core::CAMetalLayer;
use std::{
    cell::{Cell, RefCell},
    f64::consts::TAU,
    time::{Duration, Instant},
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
struct Leaf {
    x: f64,
    y: f64,
    /// A speed factor, a size and a phase, each from its own draw.
    speed: f64,
    size: f64,
    phase: f64,
}

pub struct Sim {
    atm: Atmosphere,
    w: f64,
    h: f64,
    t: f64,
    rng: Rng,
    leaves: Vec<Leaf>,
    /// 1 at the instant of a flash, fading to 0.
    flash: f64,
    next_bolt: f64,
    /// Where the last bolt fell, from 0 at the left edge to 1 at the right.
    bolt_x: f64,
    /// Seconds until the thunder of the last bolt arrives: light is quicker than sound.
    thunder_in: Option<f64>,
    rumble: bool,
}

impl Sim {
    pub fn new(atm: Atmosphere, w: f64, h: f64) -> Self {
        let mut rng = Rng(0x9E37_79B9_7F4A_7C15);
        // Leaves start to come at a fresh breeze and swirl in a gale.
        let count = if atm.wind > 0.2 {
            ((3.0 + atm.wind * 13.0) as usize).min(MAX_LEAVES)
        } else {
            0
        };
        let leaves = (0..count)
            .map(|_| Leaf {
                x: rng.range(0.0, w),
                y: rng.range(0.0, h),
                speed: rng.range(0.7, 1.3),
                size: rng.range(0.0, 1.0),
                phase: rng.range(0.0, TAU),
            })
            .collect();
        let mut sim = Self {
            leaves,
            atm,
            w,
            h,
            t: 0.0,
            rng,
            flash: 0.0,
            next_bolt: 1.2,
            bolt_x: 0.5,
            thunder_in: None,
            rumble: false,
        };
        // A sky that has been going on for a while, not one that is only starting.
        for _ in 0..90 {
            sim.step(1.0 / 30.0);
        }
        sim
    }

    pub fn atmosphere(&self) -> Atmosphere {
        self.atm
    }

    pub fn time(&self) -> f64 {
        self.t
    }

    /// Moves everything on by `dt` seconds.
    pub fn step(&mut self, dt: f64) {
        let dt = dt.clamp(0.0, 0.05);
        self.t += dt;
        let (w, h) = (self.w, self.h);
        let atm = self.atm;
        let dir = if atm.wind_dir < 0.0 { -1.0 } else { 1.0 };
        let swirl = swirl_of(atm.wind);
        for l in &mut self.leaves {
            l.x += dir * (50.0 + 420.0 * atm.wind) * l.speed * dt;
            l.y += (8.0 + 14.0 * l.size) * dt;
            // A leaf in a gale goes round as it goes along, which is what rolls it up.
            l.phase += (1.6 + 5.5 * swirl + 2.0 * l.size) * dt;
            if l.x > w + 40.0 || l.x < -40.0 {
                l.x = if dir > 0.0 { -30.0 } else { w + 30.0 };
                l.y = self.rng.range(h * 0.25, h * 0.95);
            }
            if l.y > h + 20.0 {
                l.y = self.rng.range(-10.0, h * 0.4);
            }
        }

        // Thunder weather: a bolt now and then, bright at first and gone in a third of a second.
        self.flash = (self.flash - dt * 3.2).max(0.0);
        if atm.thunder {
            self.next_bolt -= dt;
            if self.next_bolt <= 0.0 {
                self.next_bolt = self.rng.range(2.5, 7.0);
                self.flash = 1.0;
                self.bolt_x = self.rng.range(0.15, 0.85);
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

    pub fn flash(&self) -> f64 {
        self.flash
    }

    /// The leaves for the shader: where each is across and down the area from 0 to 1, how far it has turned, and its
    /// size as a part of the height.
    fn leaf_data(&self) -> Vec<[f32; 4]> {
        let swirl = swirl_of(self.atm.wind);
        self.leaves
            .iter()
            .map(|l| {
                // Going round in the wind: the wider the circle, the stronger the gale.
                let radius = 6.0 * l.size + swirl * (16.0 + 22.0 * l.size);
                let x = l.x + radius * l.phase.cos();
                let y = l.y + radius * 0.7 * l.phase.sin() + 10.0 * (l.phase * 0.5).sin();
                [
                    (x / self.w) as f32,
                    (y / self.h) as f32,
                    (l.phase * 1.4 + l.size * TAU) as f32,
                    ((5.0 + 3.0 * l.size) / self.h * 1.15) as f32,
                ]
            })
            .collect()
    }

    /// What the shader needs for this moment, for an area drawn at `scale` pixels to a point.
    fn uniforms(&self, scale: f64) -> Uniforms {
        let a = &self.atm;
        let f = |v: f64| v as f32;
        let flag = |on: bool| if on { 1.0 } else { 0.0 };
        Uniforms([
            [f(self.w * scale), f(self.h * scale), f(self.t), flag(a.day)],
            [f(a.progress), f(a.cloud), f(a.rain), flag(a.thunder)],
            [f(a.snow), f(a.fog), f(a.cold), f(a.wind)],
            [
                f(if a.wind_dir < 0.0 { -1.0 } else { 1.0 }),
                f(a.haze),
                f(self.flash),
                f(self.bolt_x),
            ],
            [
                f(a.leaf.0),
                f(a.leaf.1),
                f(a.leaf.2),
                self.leaves.len() as f32,
            ],
            [f(scale), 0.0, 0.0, 0.0],
        ])
    }
}

/// How much of a gale's turning a wind has: none below a strong wind, all of it in a storm.
fn swirl_of(wind: f64) -> f64 {
    ((wind - 0.55) / 0.35).clamp(0.0, 1.0)
}

/// One picture of a sky `seconds` into its life, `width` by `height` points at `scale` pixels to a point, as BGRA with
/// the top row first, for the checks. `None` where there is no GPU.
pub fn render_image(
    atm: Atmosphere,
    seconds: f64,
    width: f64,
    height: f64,
    scale: f64,
) -> Option<Vec<u8>> {
    render_with(&Gpu::new().ok()?, atm, seconds, width, height, scale)
}

fn render_with(
    gpu: &Gpu,
    atm: Atmosphere,
    seconds: f64,
    width: f64,
    height: f64,
    scale: f64,
) -> Option<Vec<u8>> {
    let mut sim = Sim::new(atm, width, height);
    while sim.time() < seconds {
        sim.step(1.0 / 30.0);
    }
    gpu.render(
        &sim.uniforms(scale),
        &sim.leaf_data(),
        (width * scale) as usize,
        (height * scale) as usize,
    )
}

pub struct FxIvars {
    sim: RefCell<Option<Sim>>,
    /// The GPU, and the layer it draws in on a real screen. Made when a sky is first shown, not when the app starts: most
    /// people never turn the Weather tab on, and a GPU costs memory and time. None on a Mac that cannot run the shader,
    /// which leaves the tab plain.
    gpu: RefCell<Option<Gpu>>,
    layer: RefCell<Option<Retained<CAMetalLayer>>>,
    asked_for_gpu: Cell<bool>,
    /// Drawing is put off until then after the window server held a frame back: a hidden window does not give frames
    /// back, and the main thread must not wait for them.
    resume_at: Cell<Option<Instant>>,
    /// For the checks that cannot look at a screen (`FIGGHOST_NOTCH_OFFSCREEN`): the sky as a picture the view draws.
    still: RefCell<Option<Retained<NSBitmapImageRep>>>,
    offscreen: bool,
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
            if let Some(rep) = self.ivars().still.borrow().as_ref() {
                rep.drawInRect(self.bounds());
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
        let offscreen = std::env::var_os("FIGGHOST_NOTCH_OFFSCREEN").is_some();
        let this = mtm.alloc::<Self>().set_ivars(FxIvars {
            sim: RefCell::new(None),
            gpu: RefCell::new(None),
            layer: RefCell::new(None),
            asked_for_gpu: Cell::new(false),
            resume_at: Cell::new(None),
            still: RefCell::new(None),
            offscreen,
        });
        // SAFETY: designated initializer with valid arguments on the main thread.
        unsafe { msg_send![super(this), initWithFrame: frame] }
    }

    /// Makes the GPU the first time it is wanted.
    fn ensure_gpu(&self) {
        let ivars = self.ivars();
        if ivars.asked_for_gpu.replace(true) {
            return;
        }
        let Ok(gpu) = Gpu::new() else {
            return;
        };
        if !ivars.offscreen {
            let layer = gpu.layer();
            // The view hosts the layer: the GPU draws in it, and AppKit keeps it the size of the view.
            self.setLayer(Some(&layer));
            self.setWantsLayer(true);
            *ivars.layer.borrow_mut() = Some(layer);
        }
        *ivars.gpu.borrow_mut() = Some(gpu);
    }

    fn scale(&self) -> f64 {
        self.window().map_or(2.0, |w| w.backingScaleFactor())
    }

    /// Draws the sky as it is now.
    fn paint(&self) {
        if self.ivars().offscreen {
            self.paint_still();
            return;
        }
        let slot = self.ivars().sim.borrow();
        let (gpu, layer) = (self.ivars().gpu.borrow(), self.ivars().layer.borrow());
        let (Some(sim), Some(gpu), Some(layer)) = (slot.as_ref(), gpu.as_ref(), layer.as_ref())
        else {
            return;
        };
        // Nothing is drawn for a window that is not on screen: its frames are not given back.
        if self.isHiddenOrHasHiddenAncestor() || !self.window().is_some_and(|w| w.isVisible()) {
            return;
        }
        if self
            .ivars()
            .resume_at
            .get()
            .is_some_and(|at| Instant::now() < at)
        {
            return;
        }
        let scale = self.scale();
        let size = self.bounds().size;
        layer.setContentsScale(scale);
        layer.setDrawableSize(NSSize::new(size.width * scale, size.height * scale));
        let mut uniforms = sim.uniforms(scale);
        uniforms.0[0][0] = (size.width * scale) as f32;
        uniforms.0[0][1] = (size.height * scale) as f32;
        if !gpu.draw(layer, &uniforms, &sim.leaf_data()) {
            self.ivars()
                .resume_at
                .set(Some(Instant::now() + Duration::from_secs(3)));
        }
    }

    fn paint_still(&self) {
        let size = self.bounds().size;
        let atmosphere = self
            .ivars()
            .sim
            .borrow()
            .as_ref()
            .map(|s| (s.atmosphere(), s.time()));
        let rep = atmosphere.and_then(|(atm, seconds)| {
            let bgra = render_with(
                self.ivars().gpu.borrow().as_ref()?,
                atm,
                seconds,
                size.width,
                size.height,
                2.0,
            )?;
            let (w, h) = ((size.width * 2.0) as isize, (size.height * 2.0) as isize);
            // SAFETY: a plain RGBA bitmap with valid dimensions, which is then filled to its last byte.
            let rep = unsafe {
                NSBitmapImageRep::initWithBitmapDataPlanes_pixelsWide_pixelsHigh_bitsPerSample_samplesPerPixel_hasAlpha_isPlanar_colorSpaceName_bytesPerRow_bitsPerPixel(
                    NSBitmapImageRep::alloc(), std::ptr::null_mut(), w, h, 8, 4, true, false, NSDeviceRGBColorSpace, 0, 0,
                )
            }?;
            let out = rep.bitmapData();
            if out.is_null() || rep.bytesPerRow() as usize != bgra.len() / h as usize {
                return None;
            }
            for (i, px) in bgra.chunks_exact(4).enumerate() {
                // SAFETY: `i` is below the number of pixels, which the bitmap holds.
                unsafe { std::slice::from_raw_parts_mut(out.add(i * 4), 4) }
                    .copy_from_slice(&[px[2], px[1], px[0], px[3]]);
            }
            rep.setSize(size);
            Some(rep)
        });
        *self.ivars().still.borrow_mut() = rep;
        self.setNeedsDisplay(true);
    }

    /// Changes the sky. The same one again keeps what is in flight, so a refresh of the forecast does not restart it.
    pub fn set(&self, atmosphere: Option<Atmosphere>) {
        if atmosphere.is_some() {
            self.ensure_gpu();
        }
        let size = self.bounds().size;
        {
            let mut slot = self.ivars().sim.borrow_mut();
            match atmosphere {
                None => *slot = None,
                Some(atm) => {
                    if slot.as_ref().map(Sim::atmosphere) != Some(atm) {
                        *slot = Some(Sim::new(atm, size.width, size.height));
                    }
                }
            }
        }
        self.setHidden(atmosphere.is_none());
        self.paint();
    }

    /// Draws the sky again as it is, which is all a still sky needs.
    pub fn repaint(&self) {
        if !self.ivars().offscreen {
            self.paint();
        }
    }

    /// One frame of `dt` seconds. True when the thunder of a bolt has just arrived.
    pub fn tick(&self, dt: f64) -> bool {
        let rumble = {
            let mut slot = self.ivars().sim.borrow_mut();
            let Some(sim) = slot.as_mut() else {
                return false;
            };
            sim.step(dt);
            sim.take_rumble()
        };
        // The still of a check is drawn when the sky is set, not on every frame.
        if !self.ivars().offscreen {
            self.paint();
        }
        rumble
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
    fn a_calm_sky_has_no_leaf_and_wind_brings_them_in_numbers_that_grow() {
        let make = |wind: f64| Sim::new(Atmosphere { wind, ..storm() }, 580.0, 250.0);
        assert!(make(0.1).leaves.is_empty(), "a light breeze lifts no leaf");
        assert!(make(0.5).leaves.len() < make(1.0).leaves.len());
        assert!(make(1.0).leaves.len() <= MAX_LEAVES);
        assert_eq!(swirl_of(0.3), 0.0);
        assert_eq!(swirl_of(1.0), 1.0);
    }

    #[test]
    fn leaves_stay_finite_while_time_runs_and_lightning_strikes_only_in_thunder() {
        let mut sim = Sim::new(storm(), 580.0, 250.0);
        let (mut flashed, mut rumbles) = (false, 0);
        for _ in 0..3600 {
            sim.step(1.0 / 30.0);
            flashed |= sim.flash() > 0.9;
            rumbles += usize::from(sim.take_rumble());
            assert!(sim.leaf_data().iter().flatten().all(|v| v.is_finite()));
            assert!((0.0..=1.0).contains(&sim.bolt_x));
        }
        assert!(flashed, "two minutes of thunder weather without a bolt");
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
            assert!(!dry.take_rumble());
        }
    }

    #[test]
    fn the_shader_compiles_and_a_sky_comes_out_lit_and_with_its_corners_cut() {
        // On a machine with no GPU there is nothing to look at.
        let Some(sunny) = render_image(
            Atmosphere {
                day: true,
                progress: 0.5,
                cloud: 0.2,
                ..Atmosphere::default()
            },
            5.0,
            290.0,
            125.0,
            1.0,
        ) else {
            return;
        };
        assert_eq!(sunny.len(), 290 * 125 * 4);
        let pixel = |x: usize, y: usize| &sunny[(y * 290 + x) * 4..][..4];
        assert_eq!(pixel(0, 0)[3], 0, "the corner is cut");
        assert_eq!(pixel(145, 60)[3], 255, "the middle is solid");
        let night = render_image(
            Atmosphere {
                day: false,
                progress: 0.4,
                ..Atmosphere::default()
            },
            5.0,
            290.0,
            125.0,
            1.0,
        )
        .unwrap();
        let brightness = |image: &[u8]| image.iter().map(|b| u64::from(*b)).sum::<u64>();
        assert!(
            brightness(&sunny) > 2 * brightness(&night),
            "a day is brighter than a night"
        );
    }
}
