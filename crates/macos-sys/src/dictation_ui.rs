//! The panel of Dictation: a microphone in the middle of the screen on Liquid Glass, a meter of the voice that moves
//! as the person speaks, the words as they are understood, and the language heard. It takes the keyboard only while it
//! listens, for Return, Escape and the arrows, and gives it back to the app being typed into when it goes.

use crate::{
    panel::{FigPanel, Key},
    style::{self, Tone, Weight, white},
    widgets::{Fit, draw_text, text_attributes, view_png},
    workspace::display_options,
};
use objc2::{DefinedClass, MainThreadMarker, MainThreadOnly, define_class, msg_send, rc::Retained};
use objc2_app_kit::{NSEvent, NSScreen, NSView};
use objc2_foundation::{NSPoint, NSRect, NSSize};
use std::{
    cell::{Cell, RefCell},
    collections::VecDeque,
    rc::Rc,
};

pub const SIZE: NSSize = NSSize::new(460.0, 176.0);
const RADIUS: f64 = 30.0;
/// The bars of the meter, the newest on the right.
const BARS: usize = 36;
const BAR_WIDTH: f64 = 5.0;
const MIC: NSPoint = NSPoint::new(62.0, 106.0);
const MIC_RADIUS: f64 = 30.0;
const LEFT: f64 = 116.0;
const RIGHT: f64 = 24.0;
/// About two lines of the words; the start of a longer text gives way to its end.
const TEXT_CHARS: usize = 96;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Stage {
    /// The microphone is on: the disc is red and the meter moves.
    #[default]
    Listening,
    /// The microphone is off and the last words are on their way.
    Finishing,
    /// Only something to say: no access, nothing heard, the words were copied.
    Note,
}

#[derive(Debug, Clone, Default)]
pub struct DictationDisplay {
    pub stage: Stage,
    /// The glyph on the disc.
    pub symbol: &'static str,
    /// The words understood so far, or what the note says.
    pub text: String,
    /// What shows while there are no words yet.
    pub placeholder: String,
    /// The language heard, on the chip at the bottom.
    pub language: String,
    /// The keys, beside the chip.
    pub hint: String,
}

/// The end of `text` in about `chars` characters, from the start of a word, after an ellipsis.
pub fn tail(text: &str, chars: usize) -> String {
    let count = text.chars().count();
    if count <= chars {
        return text.to_owned();
    }
    let rest: String = text.chars().skip(count - chars).collect();
    let rest = match rest.find(' ') {
        Some(at) if at < rest.len() / 3 => &rest[at + 1..],
        _ => rest.as_str(),
    };
    format!("...{rest}")
}

pub struct MeterIvars {
    display: RefCell<DictationDisplay>,
    levels: RefCell<VecDeque<f32>>,
    /// The level the ring around the microphone shows: it jumps up with the voice and falls back slowly.
    ring: Cell<f32>,
    /// Stands in for the glass while a snapshot is drawn, since the window server, which draws the glass, is not
    /// part of it.
    backdrop: Cell<bool>,
}

define_class!(
    // SAFETY: NSView allows subclassing; the ivars are only touched on the main thread and need no Drop.
    #[unsafe(super(NSView))]
    #[thread_kind = MainThreadOnly]
    #[name = "FigDictationMeter"]
    #[ivars = MeterIvars]
    pub struct MeterView;

    impl MeterView {
        #[unsafe(method(drawRect:))]
        fn draw_rect(&self, _dirty: NSRect) {
            self.paint();
        }
    }
);

impl MeterView {
    fn new(mtm: MainThreadMarker) -> Retained<Self> {
        let this = mtm.alloc::<Self>().set_ivars(MeterIvars {
            display: RefCell::default(),
            levels: RefCell::new(VecDeque::from(vec![0.0; BARS])),
            ring: Cell::new(0.0),
            backdrop: Cell::new(false),
        });
        // SAFETY: designated initializer with valid arguments on the main thread.
        unsafe { msg_send![super(this), initWithFrame: NSRect::new(NSPoint::ZERO, SIZE)] }
    }

    fn paint(&self) {
        let ivars = self.ivars();
        let display = ivars.display.borrow();
        let listening = display.stage == Stage::Listening;
        let size = self.bounds().size;
        if ivars.backdrop.get() {
            style::fill_rounded(self.bounds(), RADIUS, &style::black(0.82));
        }

        // The disc, with a ring that grows with the voice.
        let ring = f64::from(ivars.ring.get());
        if listening && ring > 0.02 {
            let r = MIC_RADIUS + 4.0 + 16.0 * ring;
            style::fill_circle(
                NSRect::new(
                    NSPoint::new(MIC.x - r, MIC.y - r),
                    NSSize::new(2.0 * r, 2.0 * r),
                ),
                &Tone::Red.color().colorWithAlphaComponent(0.18 + 0.2 * ring),
            );
        }
        let disc = NSRect::new(
            NSPoint::new(MIC.x - MIC_RADIUS, MIC.y - MIC_RADIUS),
            NSSize::new(2.0 * MIC_RADIUS, 2.0 * MIC_RADIUS),
        );
        if listening {
            style::fill_circle(disc, &Tone::Red.color());
        } else {
            style::fill_circle(disc, &white(0.16));
        }
        style::draw_symbol(display.symbol, MIC, 26.0, &white(0.95));

        // The meter: the newest level on the right, older ones moving left.
        if display.stage != Stage::Note {
            let span = size.width - LEFT - RIGHT;
            let gap = (span - BARS as f64 * BAR_WIDTH) / (BARS as f64 - 1.0);
            let shade = if listening { 0.85 } else { 0.32 };
            for (i, level) in ivars.levels.borrow().iter().enumerate() {
                let height = 4.0 + 38.0 * f64::from(*level);
                let x = LEFT + i as f64 * (BAR_WIDTH + gap);
                style::fill_rounded(
                    NSRect::new(
                        NSPoint::new(x, 132.0 - height / 2.0),
                        NSSize::new(BAR_WIDTH, height),
                    ),
                    BAR_WIDTH / 2.0,
                    &white(shade),
                );
            }
        }

        // The words, or what shows while there are none.
        let (words, shade) = if display.text.is_empty() {
            (display.placeholder.as_str(), 0.5)
        } else {
            (display.text.as_str(), 0.96)
        };
        // A note is said whole; only the words heard give their start up to their end.
        let (words, top) = if display.stage == Stage::Note {
            (words.to_owned(), 132.0)
        } else {
            (tail(words, TEXT_CHARS), 104.0)
        };
        draw_text(
            &words,
            NSRect::new(
                NSPoint::new(LEFT, 44.0),
                NSSize::new(size.width - LEFT - RIGHT, top - 44.0),
            ),
            &text_attributes(
                &style::font(15.0, Weight::Medium),
                &white(shade),
                Fit::Words,
                false,
            ),
        );

        // The language on a chip, and the keys beside it.
        let chip_font = style::font(12.0, Weight::Semibold);
        let chip_width = 20.0 + text_width(&display.language, 12.0);
        if !display.language.is_empty() {
            let chip = NSRect::new(NSPoint::new(22.0, 12.0), NSSize::new(chip_width, 22.0));
            style::fill_rounded(chip, 11.0, &white(0.12));
            draw_text(
                &display.language,
                NSRect::new(
                    NSPoint::new(chip.origin.x, 15.0),
                    NSSize::new(chip_width, 16.0),
                ),
                &text_attributes(&chip_font, &white(0.85), Fit::Tail, true),
            );
        }
        let after = 22.0 + chip_width + 12.0;
        draw_text(
            &display.hint,
            NSRect::new(
                NSPoint::new(after, 16.0),
                NSSize::new((size.width - after - RIGHT).max(0.0), 16.0),
            ),
            &text_attributes(
                &style::font(11.5, Weight::Regular),
                &white(0.5),
                Fit::Tail,
                false,
            ),
        );
    }
}

fn text_width(text: &str, points: f64) -> f64 {
    // ponytail: an estimate of the width from the number of characters; measure the string if a chip ever clips.
    text.chars().count() as f64 * points * 0.58
}

pub struct DictationPanel {
    mtm: MainThreadMarker,
    panel: Retained<FigPanel>,
    view: Retained<MeterView>,
    solid: Cell<Option<bool>>,
}

impl DictationPanel {
    /// `on_key` hears the keys while the panel listens and returns whether it took them.
    pub fn new(mtm: MainThreadMarker, on_key: impl Fn(Key) -> bool + 'static) -> Rc<Self> {
        let panel = FigPanel::new(mtm, NSRect::new(NSPoint::ZERO, SIZE));
        panel.setHasShadow(true);
        panel.set_key_handler(on_key);
        Rc::new(Self {
            mtm,
            panel,
            view: MeterView::new(mtm),
            solid: Cell::new(None),
        })
    }

    /// Shows `display` in the middle of the screen the pointer is on. With `take_keys` the panel takes the keyboard
    /// for Return, Escape and the arrows; the app in front stays the app in front.
    pub fn show(&self, display: &DictationDisplay, take_keys: bool) {
        if !self.panel.isVisible() {
            *self.view.ivars().levels.borrow_mut() = VecDeque::from(vec![0.0; BARS]);
            self.view.ivars().ring.set(0.0);
            self.place();
        }
        *self.view.ivars().display.borrow_mut() = display.clone();
        // The glass is made again only when Increase Contrast or Reduce Transparency changed, not for every word.
        let options = display_options();
        let solid = options.reduce_transparency || options.increase_contrast;
        if self.solid.replace(Some(solid)) != Some(solid) {
            self.panel.set_glass_content(self.mtm, &self.view, RADIUS);
        }
        self.view.setNeedsDisplay(true);
        if take_keys {
            self.panel.makeKeyAndOrderFront(None);
        } else {
            self.panel.orderFrontRegardless();
        }
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
        self.panel
            .setFrame_display(NSRect::new(origin, SIZE), false);
    }

    /// The loudness of the voice now, from 0 to 1: one more bar on the right of the meter.
    pub fn level(&self, level: f32) {
        let ivars = self.view.ivars();
        {
            let mut levels = ivars.levels.borrow_mut();
            levels.pop_front();
            levels.push_back(level.clamp(0.0, 1.0));
        }
        ivars.ring.set(level.max(ivars.ring.get() * 0.86));
        self.view.setNeedsDisplay(true);
    }

    pub fn hide(&self) {
        self.panel.orderOut(None);
    }

    pub fn is_visible(&self) -> bool {
        self.panel.isVisible()
    }

    /// The content drawn into PNG bytes without the glass behind it, for looking at the design and for checks.
    pub fn png(&self, display: &DictationDisplay, levels: &[f32]) -> Option<Vec<u8>> {
        *self.view.ivars().display.borrow_mut() = display.clone();
        for level in levels {
            self.level(*level);
        }
        self.view.ivars().backdrop.set(true);
        let png = view_png(&self.view);
        self.view.ivars().backdrop.set(false);
        png
    }

    pub fn view(&self) -> &NSView {
        &self.view
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_long_text_shows_its_end_from_the_start_of_a_word() {
        assert_eq!(tail("short", 10), "short");
        assert_eq!(tail("one two three four", 10), "...three four");
        assert_eq!(tail("một hai ba bốn năm sáu bảy tám", 12), "...sáu bảy tám");
        assert_eq!(
            tail("abcdefghijklmnop", 5),
            "...lmnop",
            "a text with no spaces is cut where it must be"
        );
    }
}
