//! The panel of Jarvis: a chat on Liquid Glass in the middle of the screen. At the top left is the character, a disc
//! that shows what Jarvis is doing; under it the talk as bubbles, the person's on the right, and one line to type in
//! at the bottom. The panel takes the keyboard while it is open, for Return and Escape, and gives it back to the app
//! being worked in when it goes.

use crate::{
    app,
    panel::{FigPanel, Key},
    style::{self, Tone, Weight, white},
    widgets::{Fit, draw_text, text_attributes, view_png},
    workspace::display_options,
};
use objc2::{DefinedClass, MainThreadMarker, MainThreadOnly, define_class, msg_send, rc::Retained};
use objc2_app_kit::{
    NSEvent, NSFocusRingType, NSScreen, NSStringNSExtendedStringDrawing, NSStringDrawingOptions, NSTextField, NSView,
};
use objc2_foundation::{NSPoint, NSRect, NSSize, NSString};
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
    time::Instant,
};

pub const SIZE: NSSize = NSSize::new(460.0, 540.0);
const RADIUS: f64 = 30.0;
const EDGE: f64 = 18.0;
const HEAD: f64 = 84.0;
const INPUT_HEIGHT: f64 = 40.0;
/// Under the log: the line to type in and the space around it.
const FOOT: f64 = EDGE + INPUT_HEIGHT + 12.0;
const BUBBLE_WIDTH: f64 = 340.0;
const PAD_X: f64 = 12.0;
const PAD_Y: f64 = 8.0;
const GAP: f64 = 8.0;
const TEXT_SIZE: f64 = 14.0;
const DISC: NSPoint = NSPoint::new(EDGE + 30.0, SIZE.height - EDGE - 30.0);
const DISC_RADIUS: f64 = 26.0;
/// The round button at the right end of the line to type in, which starts and stops listening.
const MIC: NSPoint = NSPoint::new(SIZE.width - EDGE - 22.0, EDGE + INPUT_HEIGHT / 2.0);
const MIC_RADIUS: f64 = 14.0;
/// How long the character takes to rise when the panel opens.
const WAKE_SECONDS: f64 = 0.45;
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

/// The size of a bubble that holds `text`.
fn bubble_size(text: &str) -> NSSize {
    let inner = BUBBLE_WIDTH - 2.0 * PAD_X;
    NSSize::new(BUBBLE_WIDTH, text_height(text, inner) + 2.0 * PAD_Y)
}

/// Which lines fit above `room` points from the bottom, newest last, with the height each takes.
fn fitting(lines: &[Line], room: f64) -> Vec<(&Line, f64)> {
    let mut used = 0.0;
    let mut out = Vec::new();
    for line in lines.iter().rev() {
        let height = bubble_size(&line.text).height;
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

        #[unsafe(method(mouseDown:))]
        fn mouse_down(&self, event: &NSEvent) {
            let at = self.convertPoint_fromView(event.locationInWindow(), None);
            let (dx, dy) = (at.x - MIC.x, at.y - MIC.y);
            if dx * dx + dy * dy <= (MIC_RADIUS + 4.0) * (MIC_RADIUS + 4.0) {
                let handler = self.ivars().on_mic.borrow().clone();
                if let Some(handler) = handler {
                    handler();
                }
            }
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
        });
        // SAFETY: designated initializer with valid arguments on the main thread.
        unsafe { msg_send![super(this), initWithFrame: NSRect::new(NSPoint::ZERO, SIZE)] }
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
        let centre = NSPoint::new(DISC.x + shake, DISC.y);
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
        let disc = NSRect::new(
            NSPoint::new(MIC.x - MIC_RADIUS, MIC.y - MIC_RADIUS),
            NSSize::new(2.0 * MIC_RADIUS, 2.0 * MIC_RADIUS),
        );
        let tone = if listening { Tone::Blue.color() } else { white(0.14) };
        style::fill_circle(disc, &tone);
        style::draw_symbol(if listening { "stop.fill" } else { "mic.fill" }, MIC, 13.0, &white(0.95));
    }

    fn paint(&self) {
        let ivars = self.ivars();
        let display = ivars.display.borrow();
        if ivars.backdrop.get() {
            style::fill_rounded(self.bounds(), RADIUS, &style::black(0.82));
        }

        self.paint_character(&display);

        // The name with its mark, and what is going on.
        let left = DISC.x + DISC_RADIUS + 12.0;
        let name_font = style::font(16.0, Weight::Semibold);
        draw_text(
            &display.name,
            NSRect::new(NSPoint::new(left, SIZE.height - EDGE - 30.0), NSSize::new(200.0, 20.0)),
            &text_attributes(&name_font, &white(0.97), Fit::Tail, false),
        );
        if !display.beta.is_empty() {
            let x = left + 12.0 + display.name.chars().count() as f64 * 9.0;
            let width = 16.0 + display.beta.chars().count() as f64 * 7.0;
            let pill = NSRect::new(NSPoint::new(x, SIZE.height - EDGE - 28.0), NSSize::new(width, 17.0));
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
                NSPoint::new(left, SIZE.height - EDGE - 50.0),
                NSSize::new(SIZE.width - left - EDGE, 16.0),
            ),
            &text_attributes(&style::font(12.5, Weight::Regular), &white(0.62), Fit::Tail, false),
        );

        // The talk, newest at the bottom, the oldest that do not fit left out.
        let top = SIZE.height - HEAD - EDGE;
        let room = top - FOOT;
        if display.lines.is_empty() {
            draw_text(
                &display.greeting,
                NSRect::new(NSPoint::new(EDGE + 6.0, FOOT), NSSize::new(SIZE.width - 2.0 * EDGE - 12.0, room)),
                &attributes(0.55, 14.0, Weight::Regular),
            );
        }
        let shown = fitting(&display.lines, room);
        let mut y = FOOT;
        for (line, height) in shown.into_iter().rev() {
            let width = BUBBLE_WIDTH;
            let x = if line.mine { SIZE.width - EDGE - width } else { EDGE };
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
        style::well(NSRect::new(NSPoint::new(EDGE, EDGE), NSSize::new(SIZE.width - 2.0 * EDGE, INPUT_HEIGHT)), 20.0);
        self.paint_mic(&display);
    }
}

pub struct JarvisPanel {
    mtm: MainThreadMarker,
    panel: Retained<FigPanel>,
    view: Retained<JarvisView>,
    field: Retained<NSTextField>,
    solid: Cell<Option<bool>>,
    /// Redraws the character while the panel is open.
    frames: RefCell<Option<app::Timer>>,
}

impl JarvisPanel {
    /// `on_key` hears Return, Escape and the other keys of the panel and returns whether it took them.
    pub fn new(mtm: MainThreadMarker, on_key: impl Fn(Key) -> bool + 'static) -> Rc<Self> {
        let panel = FigPanel::new(mtm, NSRect::new(NSPoint::ZERO, SIZE));
        panel.setHasShadow(true);
        panel.set_key_handler(on_key);
        let view = JarvisView::new(mtm);
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
        view.addSubview(&field);
        Rc::new(Self { mtm, panel, view, field, solid: Cell::new(None), frames: RefCell::new(None) })
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
        }
        if opening {
            let ivars = self.view.ivars();
            ivars.opened.set(Instant::now());
            ivars.level.set(0.0);
            ivars.animate.set(!display_options().reduce_motion);
            self.panel.makeKeyAndOrderFront(None);
            self.panel.makeFirstResponder(Some(&self.field));
            if ivars.animate.get() {
                let view = self.view.clone();
                *self.frames.borrow_mut() = Some(app::every(FRAME_SECONDS, move || {
                    let area = NSRect::new(
                        NSPoint::new(DISC.x - 60.0, DISC.y - 60.0),
                        NSSize::new(120.0, 120.0),
                    );
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

    fn place(&self) {
        let pointer = NSEvent::mouseLocation();
        let screens = NSScreen::screens(self.mtm);
        let Some(screen) = screens
            .iter()
            .find(|s| {
                let f = s.frame();
                pointer.x >= f.origin.x
                    && pointer.x < f.origin.x + f.size.width
                    && pointer.y >= f.origin.y
                    && pointer.y < f.origin.y + f.size.height
            })
            .or_else(|| screens.firstObject())
        else {
            return;
        };
        let area = screen.visibleFrame();
        let origin = NSPoint::new(
            area.origin.x + (area.size.width - SIZE.width) / 2.0,
            area.origin.y + (area.size.height - SIZE.height) / 2.0,
        );
        self.panel.setFrame_display(NSRect::new(origin, SIZE), false);
    }

    /// What is typed, trimmed.
    pub fn typed(&self) -> String {
        self.field.stringValue().to_string().trim().to_owned()
    }

    pub fn clear(&self) {
        self.field.setStringValue(&NSString::new());
    }

    pub fn hide(&self) {
        self.frames.borrow_mut().take();
        self.clear();
        self.panel.orderOut(None);
    }

    pub fn is_visible(&self) -> bool {
        self.panel.isVisible()
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
        let shown = fitting(&lines, 200.0);
        assert!(shown.len() < lines.len() && !shown.is_empty());
        assert_eq!(shown.last().unwrap().0.text, "line 29");
        let used: f64 = shown.iter().map(|(_, h)| h + GAP).sum();
        assert!(used - GAP <= 200.0 + 1.0);
    }

    #[test]
    fn a_line_taller_than_the_room_is_still_shown() {
        let long = "word ".repeat(400);
        let lines = [line(&long)];
        let shown = fitting(&lines, 100.0);
        assert_eq!(shown.len(), 1, "the last thing said is never hidden");
    }
}
