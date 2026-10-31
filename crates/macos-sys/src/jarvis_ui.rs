//! The panel of Jarvis: a chat on Liquid Glass in the middle of the screen. At the top left is the character, a disc
//! that shows what Jarvis is doing; under it the talk as bubbles, the person's on the right, and one line to type in
//! at the bottom. The panel takes the keyboard while it is open, for Return and Escape, and gives it back to the app
//! being worked in when it goes.

use crate::{
    app,
    mascot::Sheet,
    panel::{FigPanel, Key},
    style::{self, Tone, Weight, white},
    widgets::{Fit, draw_text, text_attributes, view_png},
    workspace::display_options,
};
use objc2::{DefinedClass, MainThreadMarker, MainThreadOnly, define_class, msg_send, rc::Retained};
use objc2_app_kit::{
    NSAutoresizingMaskOptions, NSCursor, NSEvent, NSFocusRingType, NSScreen, NSStringNSExtendedStringDrawing,
    NSStringDrawingOptions, NSTextField, NSView,
};
use objc2_foundation::{NSPoint, NSRect, NSSize, NSString};
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
    time::Instant,
};

/// The size the panel opens at the first time; then it keeps the size and place the person gave it.
pub const SIZE: NSSize = NSSize::new(460.0, 540.0);
/// The smallest and the largest the panel can be made by pulling its edges.
pub const MIN_SIZE: NSSize = NSSize::new(360.0, 420.0);
pub const MAX_SIZE: NSSize = NSSize::new(1200.0, 1400.0);
/// How near an edge, in points, a press pulls that edge instead of moving the panel.
const GRIP: f64 = 8.0;
const RADIUS: f64 = 30.0;
const EDGE: f64 = 18.0;
const HEAD: f64 = 84.0;
const INPUT_HEIGHT: f64 = 40.0;
/// Under the log: the line to type in and the space around it.
const FOOT: f64 = EDGE + INPUT_HEIGHT + 12.0;
const PAD_X: f64 = 12.0;
const PAD_Y: f64 = 8.0;
const GAP: f64 = 8.0;
const TEXT_SIZE: f64 = 14.0;
/// Where the disc of the character sits in a panel of `size`.
fn disc(size: NSSize) -> NSPoint {
    NSPoint::new(EDGE + 30.0, size.height - EDGE - 30.0)
}
const DISC_RADIUS: f64 = 26.0;
/// The round button at the right end of the line to type in, which starts and stops listening.
fn mic(size: NSSize) -> NSPoint {
    NSPoint::new(size.width - EDGE - 22.0, EDGE + INPUT_HEIGHT / 2.0)
}

/// How wide a bubble is in a panel `width` wide: most of it, within what reads well.
fn bubble_width(width: f64) -> f64 {
    (width - 2.0 * EDGE - 70.0).clamp(240.0, 640.0)
}

/// Which edges a press pulls.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Edges {
    pub left: bool,
    pub right: bool,
    pub bottom: bool,
    pub top: bool,
}

impl Edges {
    pub fn any(self) -> bool {
        self.left || self.right || self.bottom || self.top
    }
}

/// The edges a press at `at`, in the panel's own points, pulls.
pub fn edges_at(at: NSPoint, size: NSSize) -> Edges {
    Edges {
        left: at.x <= GRIP,
        right: at.x >= size.width - GRIP,
        bottom: at.y <= GRIP,
        top: at.y >= size.height - GRIP,
    }
}

/// The frame after pulling `edges` of `start` by `dx`, `dy` (screen points, y up), within the smallest and largest
/// size; the edges not pulled stay where they were.
pub fn resized(start: NSRect, edges: Edges, dx: f64, dy: f64) -> NSRect {
    let (mut x, mut y, mut w, mut h) = (start.origin.x, start.origin.y, start.size.width, start.size.height);
    if edges.right {
        w += dx;
    }
    if edges.left {
        w -= dx;
    }
    if edges.top {
        h += dy;
    }
    if edges.bottom {
        h -= dy;
    }
    w = w.clamp(MIN_SIZE.width, MAX_SIZE.width);
    h = h.clamp(MIN_SIZE.height, MAX_SIZE.height);
    if edges.left {
        x = start.origin.x + start.size.width - w;
    }
    if edges.bottom {
        y = start.origin.y + start.size.height - h;
    }
    NSRect::new(NSPoint::new(x, y), NSSize::new(w, h))
}

/// Where the panel opens: the frame it had, kept to the size limits, when most of it is on a screen of `screens`
/// (visible frames); else `size` in the middle of `home`.
pub fn opening_frame(saved: Option<NSRect>, screens: &[NSRect], home: NSRect) -> NSRect {
    if let Some(f) = saved {
        let size = NSSize::new(f.size.width.clamp(MIN_SIZE.width, MAX_SIZE.width), f.size.height.clamp(MIN_SIZE.height, MAX_SIZE.height));
        let f = NSRect::new(f.origin, size);
        let shown = |s: &NSRect| {
            let w = (f.origin.x + f.size.width).min(s.origin.x + s.size.width) - f.origin.x.max(s.origin.x);
            let h = (f.origin.y + f.size.height).min(s.origin.y + s.size.height) - f.origin.y.max(s.origin.y);
            w.max(0.0) * h.max(0.0)
        };
        if screens.iter().any(|s| shown(s) >= 0.5 * size.width * size.height) {
            return f;
        }
        return centred(size, home);
    }
    centred(SIZE, home)
}

fn centred(size: NSSize, area: NSRect) -> NSRect {
    let size = NSSize::new(size.width.min(area.size.width), size.height.min(area.size.height));
    NSRect::new(
        NSPoint::new(area.origin.x + (area.size.width - size.width) / 2.0, area.origin.y + (area.size.height - size.height) / 2.0),
        size,
    )
}
const MIC_RADIUS: f64 = 14.0;
/// The character in the corner while there is a talk, and in the middle of the log before there is one.
const FACE: f64 = 64.0;
/// How long the character takes to rise when the panel opens.
const WAKE_SECONDS: f64 = 0.45;
/// How long it waves hello when the panel opens.
const HELLO_SECONDS: f64 = 1.4;
/// The animation is drawn this many times a second.
const FRAME_SECONDS: f64 = 1.0 / 30.0;

/// What the character is doing; the animation of each state is drawn from it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Stage {
    #[default]
    Idle,
    /// The microphone is on: the ring follows the voice.
    Listening,
    Thinking,
    Speaking,
    Error,
}

/// The frames of the character's sheet (`assets/jarvis/jarvis.png`, drawn by agy from `docs/jarvis-art`), in order.
pub mod frame {
    pub const IDLE: usize = 0;
    pub const BLINK: usize = 1;
    pub const LOOK_LEFT: usize = 2;
    pub const LOOK_RIGHT: usize = 3;
    pub const LISTEN: usize = 4;
    pub const TALK_OPEN: usize = 5;
    pub const TALK_HALF: usize = 6;
    pub const HAPPY: usize = 7;
    pub const THINK: usize = 8;
    pub const ERROR: usize = 9;
    pub const WAVE_A: usize = 10;
    pub const WAVE_B: usize = 11;
    pub const COUNT: usize = 12;
}

/// The frame the character shows: `opened` seconds after the panel opened, `t` seconds on a running clock. Without
/// movement each stage has one still frame.
pub fn frame_for(stage: Stage, opened: f64, t: f64, animate: bool) -> usize {
    use frame::*;
    if !animate {
        return match stage {
            Stage::Idle => IDLE,
            Stage::Listening => LISTEN,
            Stage::Thinking => THINK,
            Stage::Speaking => TALK_OPEN,
            Stage::Error => ERROR,
        };
    }
    if opened < HELLO_SECONDS && matches!(stage, Stage::Idle | Stage::Speaking) {
        return if (opened / 0.2) as usize % 2 == 0 { WAVE_A } else { WAVE_B };
    }
    match stage {
        Stage::Idle => {
            // A blink every four seconds, and a look aside now and then.
            let beat = t % 8.0;
            if beat % 4.0 < 0.14 {
                BLINK
            } else if (5.0..5.8).contains(&beat) {
                LOOK_LEFT
            } else if (5.8..6.6).contains(&beat) {
                LOOK_RIGHT
            } else {
                IDLE
            }
        }
        Stage::Listening => {
            if t % 3.0 < 0.12 { BLINK } else { LISTEN }
        }
        Stage::Thinking => [THINK, LOOK_LEFT, THINK, LOOK_RIGHT][(t / 0.45) as usize % 4],
        Stage::Speaking => [TALK_OPEN, TALK_HALF, IDLE, TALK_HALF][(t / 0.11) as usize % 4],
        Stage::Error => ERROR,
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Line {
    /// The person's own words.
    pub mine: bool,
    pub text: String,
}

#[derive(Debug, Clone, Default)]
pub struct JarvisDisplay {
    pub stage: Stage,
    /// The glyph on the disc.
    pub symbol: &'static str,
    pub name: String,
    /// The small mark next to the name.
    pub beta: String,
    /// What Jarvis is doing, in words, under the name.
    pub status: String,
    pub lines: Vec<Line>,
    /// What the empty line says, and the log says while there is no talk yet.
    pub placeholder: String,
    pub greeting: String,
}

fn attributes(color_alpha: f64, size: f64, weight: Weight) -> Retained<objc2_foundation::NSMutableDictionary<NSString, objc2::runtime::AnyObject>> {
    text_attributes(&style::font(size, weight), &white(color_alpha), Fit::Words, false)
}

/// The height `text` takes in a column of `width`.
fn text_height(text: &str, width: f64) -> f64 {
    let attrs = attributes(1.0, TEXT_SIZE, Weight::Regular);
    // SAFETY: plain measuring of a string with valid attributes.
    let rect = unsafe {
        NSString::from_str(text).boundingRectWithSize_options_attributes_context(
            NSSize::new(width, 100_000.0),
            NSStringDrawingOptions::UsesLineFragmentOrigin,
            Some(&attrs),
            None,
        )
    };
    rect.size.height.ceil()
}

/// The size of a bubble `width` wide that holds `text`.
fn bubble_size(text: &str, width: f64) -> NSSize {
    let inner = width - 2.0 * PAD_X;
    NSSize::new(width, text_height(text, inner) + 2.0 * PAD_Y)
}

/// Which lines fit above `room` points from the bottom in bubbles `width` wide, newest last, with the height each
/// takes.
fn fitting(lines: &[Line], room: f64, width: f64) -> Vec<(&Line, f64)> {
    let mut used = 0.0;
    let mut out = Vec::new();
    for line in lines.iter().rev() {
        let height = bubble_size(&line.text, width).height;
        if !out.is_empty() && used + height > room {
            break;
        }
        used += height + GAP;
        out.push((line, height));
    }
    out.reverse();
    out
}

pub struct JarvisIvars {
    display: RefCell<JarvisDisplay>,
    backdrop: Cell<bool>,
    /// The loudness of the voice now, from 0 to 1: the ring of the listening character.
    level: Cell<f32>,
    /// When the panel opened, and when the stage last changed, for the movement of the character.
    opened: Cell<Instant>,
    since: Cell<Instant>,
    started: Instant,
    /// Whether the character moves; not under Reduce motion.
    animate: Cell<bool>,
    on_mic: RefCell<Option<Rc<dyn Fn()>>>,
    /// While an edge is pulled: which, where the pointer was on the screen, and the frame then.
    pulling: Cell<Option<(Edges, NSPoint, NSRect)>>,
    /// The drawn character; without it a disc stands in.
    sheet: RefCell<Option<Sheet>>,
}

define_class!(
    // SAFETY: NSView allows subclassing; the ivars are only touched on the main thread and need no Drop.
    #[unsafe(super(NSView))]
    #[thread_kind = MainThreadOnly]
    #[name = "FigJarvisView"]
    #[ivars = JarvisIvars]
    pub struct JarvisView;

    impl JarvisView {
        #[unsafe(method(drawRect:))]
        fn draw_rect(&self, _dirty: NSRect) {
            self.paint();
        }

        // A press on the microphone button presses it; near an edge it pulls that edge; anywhere else it moves the
        // panel, as the title bar of a window does.
        #[unsafe(method(mouseDown:))]
        fn mouse_down(&self, event: &NSEvent) {
            let size = self.bounds().size;
            let at = self.convertPoint_fromView(event.locationInWindow(), None);
            let button = mic(size);
            let (dx, dy) = (at.x - button.x, at.y - button.y);
            if dx * dx + dy * dy <= (MIC_RADIUS + 4.0) * (MIC_RADIUS + 4.0) {
                let handler = self.ivars().on_mic.borrow().clone();
                if let Some(handler) = handler {
                    handler();
                }
                return;
            }
            let Some(window) = self.window() else { return };
            let edges = edges_at(at, size);
            if edges.any() {
                self.ivars().pulling.set(Some((edges, NSEvent::mouseLocation(), window.frame())));
            } else {
                window.performWindowDragWithEvent(event);
            }
        }

        #[unsafe(method(mouseDragged:))]
        fn mouse_dragged(&self, _event: &NSEvent) {
            let Some((edges, from, start)) = self.ivars().pulling.get() else { return };
            let Some(window) = self.window() else { return };
            let now = NSEvent::mouseLocation();
            window.setFrame_display(resized(start, edges, now.x - from.x, now.y - from.y), true);
            self.setNeedsDisplay(true);
        }

        #[unsafe(method(mouseUp:))]
        fn mouse_up(&self, _event: &NSEvent) {
            if self.ivars().pulling.take().is_some()
                && let Some(window) = self.window()
                && window.hasShadow()
            {
                window.invalidateShadow();
            }
        }

        // The pointer says which edges can be pulled.
        #[unsafe(method(resetCursorRects))]
        fn reset_cursor_rects(&self) {
            let b = self.bounds();
            let (w, h) = (b.size.width, b.size.height);
            #[allow(deprecated)]
            let (across, up) = (NSCursor::resizeLeftRightCursor(), NSCursor::resizeUpDownCursor());
            self.addCursorRect_cursor(NSRect::new(NSPoint::ZERO, NSSize::new(GRIP, h)), &across);
            self.addCursorRect_cursor(NSRect::new(NSPoint::new(w - GRIP, 0.0), NSSize::new(GRIP, h)), &across);
            self.addCursorRect_cursor(NSRect::new(NSPoint::ZERO, NSSize::new(w, GRIP)), &up);
            self.addCursorRect_cursor(NSRect::new(NSPoint::new(0.0, h - GRIP), NSSize::new(w, GRIP)), &up);
        }
    }
);

impl JarvisView {
    fn new(mtm: MainThreadMarker) -> Retained<Self> {
        let this = mtm.alloc::<Self>().set_ivars(JarvisIvars {
            display: RefCell::default(),
            backdrop: Cell::new(false),
            level: Cell::new(0.0),
            opened: Cell::new(Instant::now()),
            since: Cell::new(Instant::now()),
            started: Instant::now(),
            animate: Cell::new(true),
            on_mic: RefCell::new(None),
            pulling: Cell::new(None),
            sheet: RefCell::new(None),
        });
        // SAFETY: designated initializer with valid arguments on the main thread.
        unsafe { msg_send![super(this), initWithFrame: NSRect::new(NSPoint::ZERO, SIZE)] }
    }

    /// The drawn character, `size` points tall, with its middle at `centre`: it floats, rises when the panel opens,
    /// and shows what Jarvis is doing. False when there is no sheet to draw from.
    fn paint_face(&self, display: &JarvisDisplay, centre: NSPoint, size: f64) -> bool {
        let ivars = self.ivars();
        let sheet = ivars.sheet.borrow();
        let Some(sheet) = sheet.as_ref().filter(|s| s.frames() >= frame::COUNT) else { return false };
        let animate = ivars.animate.get();
        let t = ivars.started.elapsed().as_secs_f64();
        let opened = ivars.opened.get().elapsed().as_secs_f64();
        let age = ivars.since.get().elapsed().as_secs_f64();
        let wake = if animate { style::ease_out((opened / WAKE_SECONDS).min(1.0)) } else { 1.0 };
        let float = if animate { size * 0.03 * (t * 2.2).sin() } else { 0.0 };
        let shake = if animate && display.stage == Stage::Error { size * 0.08 * (-6.0 * age).exp() * (age * 46.0).sin() } else { 0.0 };
        let side = size * (0.6 + 0.4 * wake);
        let middle = NSPoint::new(centre.x + shake, centre.y + float - (1.0 - wake) * size * 0.2);
        if display.stage == Stage::Listening {
            // A ring behind the head that follows the voice.
            let level = if animate { f64::from(ivars.level.get()) } else { 0.0 };
            let r = side * (0.42 + 0.14 * level);
            style::fill_circle(
                NSRect::new(NSPoint::new(middle.x - r, middle.y - r), NSSize::new(2.0 * r, 2.0 * r)),
                &Tone::Blue.color().colorWithAlphaComponent(0.18 + 0.25 * level),
            );
        }
        let at = frame_for(display.stage, opened, t, animate);
        sheet.draw(at, NSRect::new(NSPoint::new(middle.x - side / 2.0, middle.y - side / 2.0), NSSize::new(side, side)));
        true
    }

    /// The character: a disc that rises when the panel opens and then shows what Jarvis is doing.
    fn paint_character(&self, display: &JarvisDisplay) {
        let ivars = self.ivars();
        let animate = ivars.animate.get();
        let t = ivars.started.elapsed().as_secs_f64();
        let age = ivars.since.get().elapsed().as_secs_f64();
        let wake = if animate {
            style::ease_out((ivars.opened.get().elapsed().as_secs_f64() / WAKE_SECONDS).min(1.0))
        } else {
            1.0
        };
        let level = f64::from(ivars.level.get());
        let shake = if animate && display.stage == Stage::Error { 7.0 * (-6.0 * age).exp() * (age * 46.0).sin() } else { 0.0 };
        let spot = disc(self.bounds().size);
        let centre = NSPoint::new(spot.x + shake, spot.y);
        let radius = DISC_RADIUS * (0.55 + 0.45 * wake);
        let around = |r: f64| NSRect::new(NSPoint::new(centre.x - r, centre.y - r), NSSize::new(2.0 * r, 2.0 * r));

        // What goes on around the disc.
        match display.stage {
            Stage::Idle => {
                let breath = if animate { 0.5 + 0.5 * (t * 1.5).sin() } else { 0.0 };
                style::fill_circle(around(radius + 3.0 + 3.0 * breath), &white(0.05 + 0.04 * breath));
            }
            Stage::Listening => {
                let ring = if animate { level } else { 0.0 };
                style::fill_circle(
                    around(radius + 3.0 + 10.0 * ring),
                    &Tone::Blue.color().colorWithAlphaComponent(0.16 + 0.22 * ring),
                );
            }
            Stage::Thinking if animate => {
                // A dot that goes round, with a tail.
                for step in 0..6 {
                    let angle = t * 3.6 - f64::from(step) * 0.34;
                    let dot = NSPoint::new(centre.x + (radius + 9.0) * angle.cos(), centre.y + (radius + 9.0) * angle.sin());
                    let size = 3.6 - 0.4 * f64::from(step);
                    style::fill_circle(
                        NSRect::new(NSPoint::new(dot.x - size, dot.y - size), NSSize::new(2.0 * size, 2.0 * size)),
                        &white(0.9 - 0.15 * f64::from(step)),
                    );
                }
            }
            Stage::Speaking => {
                let pulse = if animate { 0.5 + 0.5 * (t * 7.0).sin() } else { 0.5 };
                style::fill_circle(around(radius + 4.0 + 8.0 * pulse), &white(0.07 + 0.07 * pulse));
            }
            _ => {}
        }

        let fill = match display.stage {
            Stage::Error => Tone::Red.color(),
            Stage::Listening => Tone::Blue.color(),
            _ => white(0.16),
        };
        style::fill_circle(around(radius), &fill);

        // What is on the disc: a glyph, or while it speaks the bars of a voice.
        if display.stage == Stage::Speaking {
            for i in 0..5 {
                let wave = if animate { (t * 9.0 + f64::from(i) * 1.1).sin().abs() } else { 0.6 };
                let height = (5.0 + 15.0 * wave) * wake;
                let x = centre.x + (f64::from(i) - 2.0) * 6.5;
                style::fill_capsule(
                    NSRect::new(NSPoint::new(x - 2.0, centre.y - height / 2.0), NSSize::new(4.0, height)),
                    &white(0.95),
                );
            }
        } else {
            let symbol = if display.stage == Stage::Listening { "mic.fill" } else { display.symbol };
            style::draw_symbol(symbol, centre, 24.0 * (0.6 + 0.4 * wake), &white(0.95));
        }
    }

    fn paint_mic(&self, display: &JarvisDisplay) {
        let listening = display.stage == Stage::Listening;
        let button = mic(self.bounds().size);
        let disc = NSRect::new(
            NSPoint::new(button.x - MIC_RADIUS, button.y - MIC_RADIUS),
            NSSize::new(2.0 * MIC_RADIUS, 2.0 * MIC_RADIUS),
        );
        let tone = if listening { Tone::Blue.color() } else { white(0.14) };
        style::fill_circle(disc, &tone);
        style::draw_symbol(if listening { "stop.fill" } else { "mic.fill" }, button, 13.0, &white(0.95));
    }

    fn paint(&self) {
        let ivars = self.ivars();
        let size = self.bounds().size;
        let bubble = bubble_width(size.width);
        let big_face = (size.width.min(size.height - HEAD - FOOT) * 0.4).clamp(110.0, 280.0);
        let display = ivars.display.borrow();
        if ivars.backdrop.get() {
            style::fill_rounded(self.bounds(), RADIUS, &style::black(0.82));
        }

        // Before there is a talk the character is large in the middle; once there is one it sits in the corner.
        let alone = display.lines.is_empty();
        let corner = NSPoint::new(EDGE + FACE / 2.0 - 4.0, size.height - EDGE - FACE / 2.0 + 6.0);
        if alone {
            let middle = NSPoint::new(size.width / 2.0, FOOT + (size.height - HEAD - EDGE - FOOT) * 0.62);
            if !self.paint_face(&display, middle, big_face) {
                self.paint_character(&display);
            }
        } else if !self.paint_face(&display, corner, FACE) {
            self.paint_character(&display);
        }

        // The name with its mark, and what is going on.
        let left = disc(size).x + DISC_RADIUS + 12.0;
        let name_font = style::font(16.0, Weight::Semibold);
        draw_text(
            &display.name,
            NSRect::new(NSPoint::new(left, size.height - EDGE - 30.0), NSSize::new(200.0, 20.0)),
            &text_attributes(&name_font, &white(0.97), Fit::Tail, false),
        );
        if !display.beta.is_empty() {
            let x = left + 12.0 + display.name.chars().count() as f64 * 9.0;
            let width = 16.0 + display.beta.chars().count() as f64 * 7.0;
            let pill = NSRect::new(NSPoint::new(x, size.height - EDGE - 28.0), NSSize::new(width, 17.0));
            style::fill_capsule(pill, &Tone::Orange.color().colorWithAlphaComponent(0.28));
            draw_text(
                &display.beta,
                NSRect::new(NSPoint::new(pill.origin.x, pill.origin.y + 1.0), NSSize::new(width, 14.0)),
                &text_attributes(&style::font(10.5, Weight::Bold), &white(0.92), Fit::Clip, true),
            );
        }
        draw_text(
            &display.status,
            NSRect::new(
                NSPoint::new(left, size.height - EDGE - 50.0),
                NSSize::new(size.width - left - EDGE, 16.0),
            ),
            &text_attributes(&style::font(12.5, Weight::Regular), &white(0.62), Fit::Tail, false),
        );

        // The talk, newest at the bottom, the oldest that do not fit left out.
        let top = size.height - HEAD - EDGE;
        let room = top - FOOT;
        if alone {
            let has_face = self.ivars().sheet.borrow().is_some();
            let height = if has_face { room * 0.62 - big_face / 2.0 - 8.0 } else { room };
            draw_text(
                &display.greeting,
                NSRect::new(NSPoint::new(EDGE + 6.0, FOOT), NSSize::new(size.width - 2.0 * EDGE - 12.0, height)),
                &text_attributes(&style::font(14.0, Weight::Regular), &white(0.62), Fit::Words, has_face),
            );
        }
        let shown = fitting(&display.lines, room, bubble);
        let mut y = FOOT;
        for (line, height) in shown.into_iter().rev() {
            let width = bubble;
            let x = if line.mine { size.width - EDGE - width } else { EDGE };
            let rect = NSRect::new(NSPoint::new(x, y), NSSize::new(width, height));
            style::fill_rounded(rect, 16.0, &white(if line.mine { 0.18 } else { 0.08 }));
            draw_text(
                &line.text,
                NSRect::new(
                    NSPoint::new(x + PAD_X, y + PAD_Y),
                    NSSize::new(width - 2.0 * PAD_X, height - 2.0 * PAD_Y),
                ),
                &attributes(0.96, TEXT_SIZE, Weight::Regular),
            );
            y += height + GAP;
        }

        // The well the line to type in sits in.
        style::well(NSRect::new(NSPoint::new(EDGE, EDGE), NSSize::new(size.width - 2.0 * EDGE, INPUT_HEIGHT)), 20.0);
        self.paint_mic(&display);
    }
}

pub struct JarvisPanel {
    mtm: MainThreadMarker,
    panel: Retained<FigPanel>,
    view: Retained<JarvisView>,
    field: Retained<NSTextField>,
    solid: Cell<Option<bool>>,
    /// Where the panel was left, to open there again.
    saved: Cell<Option<NSRect>>,
    /// Redraws the character while the panel is open.
    frames: RefCell<Option<app::Timer>>,
}

impl JarvisPanel {
    /// `on_key` hears Return, Escape and the other keys of the panel and returns whether it took them.
    /// `face` is the PNG of the character's frames, a row of squares; `None` draws a disc instead.
    pub fn new(mtm: MainThreadMarker, face: Option<&[u8]>, on_key: impl Fn(Key) -> bool + 'static) -> Rc<Self> {
        let panel = FigPanel::new(mtm, NSRect::new(NSPoint::ZERO, SIZE));
        panel.setMinSize(MIN_SIZE);
        panel.setMaxSize(MAX_SIZE);
        panel.set_key_handler(on_key);
        let view = JarvisView::new(mtm);
        *view.ivars().sheet.borrow_mut() = face.and_then(Sheet::from_png);
        let field = NSTextField::initWithFrame(
            mtm.alloc(),
            NSRect::new(
                NSPoint::new(EDGE + 16.0, EDGE + (INPUT_HEIGHT - 20.0) / 2.0),
                NSSize::new(SIZE.width - 2.0 * EDGE - 32.0 - 36.0, 20.0),
            ),
        );
        field.setBezeled(false);
        field.setDrawsBackground(false);
        field.setFocusRingType(NSFocusRingType::None);
        field.setFont(Some(&style::font(14.0, Weight::Regular)));
        field.setTextColor(Some(&white(style::TEXT)));
        field.setUsesSingleLineMode(true);
        if let Some(cell) = field.cell() {
            cell.setScrollable(true);
        }
        // The line keeps to the bottom and grows with the panel's width.
        field.setAutoresizingMask(NSAutoresizingMaskOptions::ViewWidthSizable | NSAutoresizingMaskOptions::ViewMaxYMargin);
        view.addSubview(&field);
        Rc::new(Self { mtm, panel, view, field, solid: Cell::new(None), saved: Cell::new(None), frames: RefCell::new(None) })
    }

    /// Shows `display` in the middle of the screen the pointer is on and gives the line to type in the keyboard.
    pub fn show(&self, display: &JarvisDisplay) {
        let opening = !self.panel.isVisible();
        if opening {
            self.place();
        }
        self.set(display);
        let options = display_options();
        let solid = options.reduce_transparency || options.increase_contrast;
        if self.solid.replace(Some(solid)) != Some(solid) {
            self.panel.set_glass_content(self.mtm, &self.view, RADIUS);
            // Liquid Glass draws its own rim. A shadow of the window is cast by its rectangle, which showed as dark
            // corners outside the rounded glass; only the stand-ins without glass keep one.
            self.panel.setHasShadow(!self.panel.is_glass());
        }
        if opening {
            let ivars = self.view.ivars();
            ivars.opened.set(Instant::now());
            ivars.level.set(0.0);
            ivars.animate.set(!display_options().reduce_motion);
            self.panel.makeKeyAndOrderFront(None);
            self.panel.makeFirstResponder(Some(&self.field));
            if self.panel.hasShadow() {
                self.panel.invalidateShadow();
            }
            if ivars.animate.get() {
                let view = self.view.clone();
                *self.frames.borrow_mut() = Some(app::every(FRAME_SECONDS, move || {
                    // The corner while there is a talk; the whole log while the large character is in it.
                    let b = view.bounds();
                    let area = if view.ivars().display.borrow().lines.is_empty() {
                        b
                    } else {
                        NSRect::new(NSPoint::new(0.0, b.size.height - HEAD - EDGE), NSSize::new(b.size.width, HEAD + EDGE))
                    };
                    view.setNeedsDisplayInRect(area);
                }));
            }
        }
    }

    /// Changes what is shown while the panel stays where it is.
    pub fn set(&self, display: &JarvisDisplay) {
        self.field
            .setPlaceholderString(Some(&NSString::from_str(&display.placeholder)));
        let ivars = self.view.ivars();
        if ivars.display.borrow().stage != display.stage {
            ivars.since.set(Instant::now());
        }
        *ivars.display.borrow_mut() = display.clone();
        self.view.setNeedsDisplay(true);
    }

    /// The loudness of the voice now, from 0 to 1.
    pub fn level(&self, level: f32) {
        let ivars = self.view.ivars();
        ivars.level.set(level.clamp(0.0, 1.0).max(ivars.level.get() * 0.86));
    }

    /// What the button at the right end of the line does when it is pressed.
    pub fn set_on_mic(&self, handler: impl Fn() + 'static) {
        *self.view.ivars().on_mic.borrow_mut() = Some(Rc::new(handler));
    }

    /// Puts words in the line to type in, as the voice is understood.
    pub fn set_typed(&self, text: &str) {
        self.field.setStringValue(&NSString::from_str(text));
    }

    /// Where the panel opens: where the person left it, else in the middle of the screen the pointer is on.
    fn place(&self) {
        let pointer = NSEvent::mouseLocation();
        let screens = NSScreen::screens(self.mtm);
        let areas: Vec<NSRect> = screens.iter().map(|s| s.visibleFrame()).collect();
        let home = screens
            .iter()
            .find(|s| {
                let f = s.frame();
                pointer.x >= f.origin.x
                    && pointer.x < f.origin.x + f.size.width
                    && pointer.y >= f.origin.y
                    && pointer.y < f.origin.y + f.size.height
            })
            .or_else(|| screens.firstObject())
            .map(|s| s.visibleFrame());
        let Some(home) = home else { return };
        self.panel.setFrame_display(opening_frame(self.saved.get(), &areas, home), false);
    }

    /// The frame to open at next time, from where the panel was left (kept by the feature across runs).
    pub fn set_saved_frame(&self, frame: Option<NSRect>) {
        self.saved.set(frame);
    }

    /// Where the panel is now, on the screen.
    pub fn frame(&self) -> NSRect {
        self.panel.frame()
    }

    /// What is typed, trimmed.
    pub fn typed(&self) -> String {
        self.field.stringValue().to_string().trim().to_owned()
    }

    pub fn clear(&self) {
        self.field.setStringValue(&NSString::new());
    }

    pub fn hide(&self) {
        self.saved.set(Some(self.panel.frame()));
        self.frames.borrow_mut().take();
        self.clear();
        self.panel.orderOut(None);
    }

    pub fn is_visible(&self) -> bool {
        self.panel.isVisible()
    }

    /// The same drawn at another size, as after the person pulled the panel's edges.
    pub fn png_sized(&self, display: &JarvisDisplay, size: NSSize) -> Option<Vec<u8>> {
        let before = self.view.frame();
        self.view.setFrame(NSRect::new(NSPoint::ZERO, size));
        let png = self.png(display);
        self.view.setFrame(before);
        png
    }

    /// The content drawn into PNG bytes without the glass behind it, for looking at the design and for checks.
    pub fn png(&self, display: &JarvisDisplay) -> Option<Vec<u8>> {
        *self.view.ivars().display.borrow_mut() = display.clone();
        self.view.ivars().backdrop.set(true);
        let png = view_png(&self.view);
        self.view.ivars().backdrop.set(false);
        png
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn line(text: &str) -> Line {
        Line { mine: false, text: text.into() }
    }

    #[test]
    fn the_newest_lines_that_fit_are_kept() {
        let lines: Vec<Line> = (0..30).map(|n| line(&format!("line {n}"))).collect();
        let shown = fitting(&lines, 200.0, 340.0);
        assert!(shown.len() < lines.len() && !shown.is_empty());
        assert_eq!(shown.last().unwrap().0.text, "line 29");
        let used: f64 = shown.iter().map(|(_, h)| h + GAP).sum();
        assert!(used - GAP <= 200.0 + 1.0);
    }

    #[test]
    fn the_character_waves_then_shows_what_jarvis_does() {
        use frame::*;
        assert!(matches!(frame_for(Stage::Idle, 0.1, 10.0, true), WAVE_A | WAVE_B), "it waves hello when the panel opens");
        assert_eq!(frame_for(Stage::Idle, 5.0, 1.0, true), IDLE);
        assert_eq!(frame_for(Stage::Idle, 5.0, 4.05, true), BLINK);
        assert_eq!(frame_for(Stage::Listening, 0.1, 1.0, true), LISTEN, "listening at once is not hidden by the wave");
        assert_eq!(frame_for(Stage::Error, 5.0, 1.0, true), ERROR);
        let talking: std::collections::HashSet<usize> = (0..40).map(|i| frame_for(Stage::Speaking, 5.0, f64::from(i) * 0.05, true)).collect();
        assert!(talking.contains(&TALK_OPEN) && talking.contains(&TALK_HALF), "the mouth moves while it speaks");
        let thinking: std::collections::HashSet<usize> = (0..40).map(|i| frame_for(Stage::Thinking, 5.0, f64::from(i) * 0.1, true)).collect();
        assert!(thinking.len() > 1);
        for stage in [Stage::Idle, Stage::Listening, Stage::Thinking, Stage::Speaking, Stage::Error] {
            let still = frame_for(stage, 0.0, 0.0, false);
            assert!((0..20).all(|i| frame_for(stage, f64::from(i), f64::from(i) * 0.37, false) == still), "Reduce motion keeps it still");
            assert!(still < COUNT);
        }
    }

    fn rect(x: f64, y: f64, w: f64, h: f64) -> NSRect {
        NSRect::new(NSPoint::new(x, y), NSSize::new(w, h))
    }

    #[test]
    fn pulling_an_edge_moves_only_that_edge_within_the_limits() {
        let start = rect(100.0, 100.0, 460.0, 540.0);
        let right = Edges { right: true, ..Edges::default() };
        assert_eq!(resized(start, right, 40.0, 0.0), rect(100.0, 100.0, 500.0, 540.0));
        let left = Edges { left: true, ..Edges::default() };
        assert_eq!(resized(start, left, 30.0, 0.0), rect(130.0, 100.0, 430.0, 540.0), "the right edge stays");
        assert_eq!(resized(start, left, 400.0, 0.0), rect(200.0, 100.0, MIN_SIZE.width, 540.0), "never under the smallest size");
        let corner = Edges { bottom: true, right: true, ..Edges::default() };
        assert_eq!(resized(start, corner, 20.0, -60.0), rect(100.0, 40.0, 480.0, 600.0), "the top stays when the bottom is pulled down");
        let top = Edges { top: true, ..Edges::default() };
        assert_eq!(resized(start, top, 0.0, 5000.0).size.height, MAX_SIZE.height);
        let size = NSSize::new(460.0, 540.0);
        assert_eq!(edges_at(NSPoint::new(2.0, 2.0), size), Edges { left: true, bottom: true, ..Edges::default() });
        assert!(!edges_at(NSPoint::new(200.0, 200.0), size).any(), "the middle moves the panel");
    }

    #[test]
    fn the_panel_opens_where_it_was_left_while_that_is_on_a_screen() {
        let screen = rect(0.0, 0.0, 1440.0, 875.0);
        let left = rect(900.0, 100.0, 600.0, 700.0);
        assert_eq!(opening_frame(Some(left), &[screen], screen), left);
        let gone = rect(3000.0, 100.0, 600.0, 700.0);
        let back = opening_frame(Some(gone), &[screen], screen);
        assert_eq!(back.size, NSSize::new(600.0, 700.0), "a screen that is gone keeps the size");
        assert_eq!(back.origin, NSPoint::new(420.0, 87.5), "and comes back to the middle");
        assert_eq!(opening_frame(None, &[screen], screen).size, SIZE);
        let tiny = opening_frame(Some(rect(10.0, 10.0, 50.0, 50.0)), &[screen], screen);
        assert_eq!(tiny.size, MIN_SIZE);
        assert!((bubble_width(460.0) - 354.0).abs() < 0.01 && bubble_width(5000.0) == 640.0);
    }

    #[test]
    fn a_line_taller_than_the_room_is_still_shown() {
        let long = "word ".repeat(400);
        let lines = [line(&long)];
        let shown = fitting(&lines, 100.0, 340.0);
        assert_eq!(shown.len(), 1, "the last thing said is never hidden");
    }
}
