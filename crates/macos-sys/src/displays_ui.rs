//! The Displays tab of the notch: a card for each external display with a slider for its brightness and one for its
//! contrast. Everything on it is plain data from the app, which does the talking to the displays; nothing here knows
//! a language. A slider reports every move while it is dragged.

use crate::{
    ddc::Control,
    notch::Page,
    style::{self, Weight, white},
    widgets::{ActionTarget, Fit, draw_page_note, draw_text, text_attributes},
};
use objc2::{
    DefinedClass, MainThreadMarker, MainThreadOnly, define_class, msg_send, rc::Retained, sel,
};
use objc2_app_kit::{
    NSAccessibility, NSControlSize, NSFont, NSLineBreakMode, NSSlider, NSTextAlignment,
    NSTextField, NSView,
};
use objc2_foundation::{NSPoint, NSRect, NSSize, NSString};
use std::{cell::RefCell, rc::Rc};

/// The size of the area the tab fills below the header.
pub const PAGE_SIZE: NSSize = NSSize::new(580.0, 200.0);
/// The cards in a row, which is all the page has room for.
pub const MAX_SCREENS: usize = 3;

const MARGIN: f64 = 18.0;
const GAP: f64 = 10.0;
const CARD_HEIGHT: f64 = 112.0;
const PAD: f64 = 14.0;
const RADIUS: f64 = 14.0;
/// Below the top of a card: the name, then the two rows of controls.
const NAME_TOP: f64 = 12.0;
const ROW_TOPS: [f64; 2] = [48.0, 82.0];
const ICON: f64 = 22.0;
const PERCENT: f64 = 40.0;

/// One display: its name and where its settings stand, 0 to 1. A setting the display does not have is `None`
/// and has no slider.
#[derive(Debug, Clone, PartialEq)]
pub struct Screen {
    pub name: String,
    pub brightness: Option<f64>,
    pub contrast: Option<f64>,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct DisplaysDisplay {
    pub screens: Vec<Screen>,
    /// Words that replace the cards: the tab is off, or no display answers.
    pub note: Option<(String, String)>,
}

/// The names of the two sliders, for VoiceOver, in the user's language.
#[derive(Debug, Clone, Default)]
pub struct DisplaysTexts {
    pub brightness: String,
    pub contrast: String,
}

/// Told that a slider moved: which display (its place in `screens`), which setting, and where to, 0 to 1.
pub type OnChange = Rc<dyn Fn(usize, Control, f64)>;

pub struct CanvasIvars {
    display: RefCell<DisplaysDisplay>,
}

define_class!(
    // SAFETY: NSView allows subclassing; the ivars are only touched on the main thread and need no Drop.
    #[unsafe(super(NSView))]
    #[thread_kind = MainThreadOnly]
    #[name = "FigDisplaysCanvas"]
    #[ivars = CanvasIvars]
    pub struct DisplaysCanvas;

    impl DisplaysCanvas {
        #[unsafe(method(drawRect:))]
        fn draw_rect(&self, _dirty: NSRect) {
            draw(self.bounds(), &self.ivars().display.borrow());
        }
    }
);

impl DisplaysCanvas {
    fn new(mtm: MainThreadMarker, frame: NSRect) -> Retained<Self> {
        let this = mtm.alloc::<Self>().set_ivars(CanvasIvars {
            display: RefCell::default(),
        });
        // SAFETY: designated initializer with valid arguments on the main thread.
        unsafe { msg_send![super(this), initWithFrame: frame] }
    }
}

/// Where the cards go: side by side across the page, as many as there are displays.
fn cards(bounds: NSRect, count: usize) -> Vec<NSRect> {
    let n = count.clamp(1, MAX_SCREENS) as f64;
    let width = (bounds.size.width - 2.0 * MARGIN - (n - 1.0) * GAP) / n;
    // The page is as tall as the note it may show; the cards sit in its middle.
    let y = (bounds.size.height - CARD_HEIGHT) / 2.0;
    (0..count.min(MAX_SCREENS))
        .map(|i| {
            NSRect::new(
                NSPoint::new(MARGIN + i as f64 * (width + GAP), y),
                NSSize::new(width, CARD_HEIGHT),
            )
        })
        .collect()
}

/// The middle of a row of controls in a card, from the bottom of the page.
fn row_middle(card: NSRect, row: usize) -> f64 {
    card.origin.y + card.size.height - ROW_TOPS[row]
}

fn symbol_of(control: Control) -> &'static str {
    match control {
        Control::Brightness => "sun.max.fill",
        Control::Contrast => "circle.lefthalf.filled",
    }
}

fn draw(bounds: NSRect, display: &DisplaysDisplay) {
    if let Some((title, hint)) = &display.note {
        draw_page_note("display", title, hint, bounds.size);
        return;
    }
    let attrs = text_attributes(
        &style::font(13.0, Weight::Semibold),
        &white(style::TEXT),
        Fit::Clip,
        false,
    );
    for (card, screen) in cards(bounds, display.screens.len())
        .into_iter()
        .zip(&display.screens)
    {
        style::card(card, RADIUS);
        let top = card.origin.y + card.size.height;
        draw_text(
            &screen.name,
            NSRect::new(
                NSPoint::new(card.origin.x + PAD, top - NAME_TOP - 17.0),
                NSSize::new(card.size.width - 2.0 * PAD, 17.0),
            ),
            &attrs,
        );
        let rows = [
            (Control::Brightness, screen.brightness),
            (Control::Contrast, screen.contrast),
        ];
        for (row, (control, level)) in rows.into_iter().enumerate() {
            if level.is_some() {
                style::draw_symbol(
                    symbol_of(control),
                    NSPoint::new(
                        card.origin.x + PAD + ICON / 2.0 - 2.0,
                        row_middle(card, row),
                    ),
                    13.0,
                    &white(style::TEXT_SECONDARY),
                );
            }
        }
    }
}

fn percent_text(fraction: f64) -> String {
    format!("{:.0}%", (fraction * 100.0).clamp(0.0, 100.0))
}

/// A slider and the figure beside it, which its action reads once both are made.
type Controls = (Retained<NSSlider>, Retained<NSTextField>);

/// The controls of one setting of one display.
struct Row {
    slider: Retained<NSSlider>,
    percent: Retained<NSTextField>,
    _target: Retained<ActionTarget>,
}

pub struct DisplaysView {
    mtm: MainThreadMarker,
    canvas: Retained<DisplaysCanvas>,
    texts: DisplaysTexts,
    on_change: OnChange,
    /// For each display, its brightness and contrast rows, either of which it may not have.
    rows: RefCell<Vec<[Option<Row>; 2]>>,
}

impl DisplaysView {
    /// `frame` is `Notch::page_area(PAGE_SIZE.width, PAGE_SIZE.height)`.
    pub fn new(
        mtm: MainThreadMarker,
        frame: NSRect,
        texts: DisplaysTexts,
        on_change: OnChange,
    ) -> Rc<Self> {
        Rc::new(Self {
            mtm,
            canvas: DisplaysCanvas::new(mtm, frame),
            texts,
            on_change,
            rows: RefCell::default(),
        })
    }

    /// Shows the displays. The sliders are made again only when the displays or their settings change; otherwise
    /// they are moved, so that a slider in the hand is not taken away from under it.
    pub fn show(&self, display: DisplaysDisplay) {
        let shape = |d: &DisplaysDisplay| -> Vec<(String, bool, bool)> {
            if d.note.is_some() {
                return Vec::new();
            }
            d.screens
                .iter()
                .take(MAX_SCREENS)
                .map(|s| (s.name.clone(), s.brightness.is_some(), s.contrast.is_some()))
                .collect()
        };
        let same = shape(&self.canvas.ivars().display.borrow()) == shape(&display)
            && self.rows.borrow().len() == shape(&display).len();
        if !same {
            self.build(&display);
        }
        for (rows, screen) in self.rows.borrow().iter().zip(&display.screens) {
            for (row, level) in rows.iter().zip([screen.brightness, screen.contrast]) {
                if let (Some(row), Some(level)) = (row, level) {
                    row.slider.setDoubleValue(level);
                    row.percent
                        .setStringValue(&NSString::from_str(&percent_text(level)));
                }
            }
        }
        *self.canvas.ivars().display.borrow_mut() = display;
        self.canvas.setNeedsDisplay(true);
    }

    fn build(&self, display: &DisplaysDisplay) {
        for rows in self.rows.borrow_mut().drain(..) {
            for row in rows.into_iter().flatten() {
                row.slider.removeFromSuperview();
                row.percent.removeFromSuperview();
            }
        }
        if display.note.is_some() {
            return;
        }
        let bounds = self.canvas.bounds();
        let mut built = Vec::new();
        for (index, (card, screen)) in cards(bounds, display.screens.len())
            .into_iter()
            .zip(&display.screens)
            .enumerate()
        {
            let mut rows: [Option<Row>; 2] = [None, None];
            let controls = [
                (
                    Control::Brightness,
                    screen.brightness,
                    &self.texts.brightness,
                ),
                (Control::Contrast, screen.contrast, &self.texts.contrast),
            ];
            for (place, (control, level, name)) in controls.into_iter().enumerate() {
                if level.is_some() {
                    rows[place] = Some(self.row(card, place, index, control, &screen.name, name));
                }
            }
            built.push(rows);
        }
        *self.rows.borrow_mut() = built;
    }

    fn row(
        &self,
        card: NSRect,
        place: usize,
        index: usize,
        control: Control,
        screen: &str,
        name: &str,
    ) -> Row {
        let mtm = self.mtm;
        let middle = row_middle(card, place);
        let left = card.origin.x + PAD + ICON;
        let right = card.origin.x + card.size.width - PAD;
        let percent = NSTextField::labelWithString(&NSString::new(), mtm);
        percent.setFont(Some(&NSFont::monospacedDigitSystemFontOfSize_weight(
            11.0,
            // SAFETY: extern static weight constant.
            unsafe { objc2_app_kit::NSFontWeightMedium },
        )));
        percent.setTextColor(Some(&white(style::TEXT_SECONDARY)));
        percent.setAlignment(NSTextAlignment::Right);
        percent.setLineBreakMode(NSLineBreakMode::ByClipping);
        percent.setFrame(NSRect::new(
            NSPoint::new(right - PERCENT, middle - 8.0),
            NSSize::new(PERCENT, 16.0),
        ));
        let slot: Rc<RefCell<Option<Controls>>> = Rc::default();
        let (reader, on_change) = (Rc::clone(&slot), Rc::clone(&self.on_change));
        let target = ActionTarget::new(mtm, move || {
            if let Some((slider, percent)) = reader.borrow().as_ref() {
                let value = slider.doubleValue();
                percent.setStringValue(&NSString::from_str(&percent_text(value)));
                on_change(index, control, value);
            }
        });
        let target_object: &objc2::runtime::AnyObject = &target;
        // SAFETY: `fire:` takes the sender as its only argument; the row keeps the target alive.
        let slider = unsafe {
            NSSlider::sliderWithValue_minValue_maxValue_target_action(
                0.5,
                0.0,
                1.0,
                Some(target_object),
                Some(sel!(fire:)),
                mtm,
            )
        };
        slider.setControlSize(NSControlSize::Small);
        slider.setContinuous(true);
        slider.setTrackFillColor(Some(&white(style::TEXT)));
        slider.setFrame(NSRect::new(
            NSPoint::new(left, middle - 9.0),
            NSSize::new(right - PERCENT - 6.0 - left, 18.0),
        ));
        slider.setAccessibilityLabel(Some(&NSString::from_str(&format!("{name}, {screen}"))));
        *slot.borrow_mut() = Some((slider.clone(), percent.clone()));
        self.canvas.addSubview(&slider);
        self.canvas.addSubview(&percent);
        Row {
            slider,
            percent,
            _target: target,
        }
    }

    /// The notch tab for this view.
    pub fn page(
        self: &Rc<Self>,
        title: String,
        on_show: impl Fn() + 'static,
        on_hide: impl Fn() + 'static,
    ) -> Page {
        Page {
            id: "displays",
            title,
            symbol: "display",
            root: Retained::into_super(Retained::clone(&self.canvas)),
            on_show: Box::new(on_show),
            on_hide: Box::new(on_hide),
            on_key: Box::new(|_| false),
            accepts_files: false,
            size: None,
        }
    }

    /// The page drawn into a PNG, sliders included, for the pictures of the documentation and the checks.
    pub fn snapshot_png(&self) -> Option<Vec<u8>> {
        crate::widgets::view_png(&self.canvas)
    }

    /// Moves the slider of a setting as a person would, for the checks: it reports the move as a drag does.
    pub fn press(&self, index: usize, control: Control, value: f64) -> bool {
        let rows = self.rows.borrow();
        let place = match control {
            Control::Brightness => 0,
            Control::Contrast => 1,
        };
        let Some(Some(row)) = rows.get(index).map(|r| r[place].as_ref()) else {
            return false;
        };
        row.slider.setDoubleValue(value);
        // SAFETY: the target's action takes the sender, which may be nil.
        let _: () = unsafe { msg_send![&*row._target, fire: Option::<&NSView>::None] };
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_cards_share_the_width_and_stay_on_the_page() {
        let bounds = NSRect::new(NSPoint::ZERO, PAGE_SIZE);
        for count in 1..=MAX_SCREENS {
            let cards = cards(bounds, count);
            assert_eq!(cards.len(), count);
            let last = cards.last().unwrap();
            assert!((last.origin.x + last.size.width - (PAGE_SIZE.width - MARGIN)).abs() < 1e-9);
            assert!(cards.iter().all(|c| c.origin.y >= 0.0));
        }
        assert_eq!(cards(bounds, 5).len(), MAX_SCREENS, "the rest have no room");
        assert!(cards(bounds, 0).is_empty());
    }

    #[test]
    fn percentages_are_whole_and_held_to_the_range() {
        assert_eq!(percent_text(0.756), "76%");
        assert_eq!(percent_text(2.0), "100%");
        assert_eq!(percent_text(-0.5), "0%");
    }
}
