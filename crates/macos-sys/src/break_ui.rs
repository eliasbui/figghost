//! The screen of a break: every display blurred and dimmed, and on the one with the pointer a card of Liquid Glass with
//! what to do, the time left, and two buttons, to skip the break and to have it a few minutes later (Escape skips too).
//! Nothing here counts time or knows a language: the app says what to show and is told which button was pressed.
//!
//! The covers take the clicks that land on them, so nothing under them is pressed by mistake, but the app is never
//! brought to the front; the card takes the keyboard while it shows, so typing does not go on behind it.

use crate::{
    button::{Look, TouchButton},
    notch::{Material, glass_tint, vibrancy_of},
    panel::{FigPanel, Key},
    style::{self, Weight, white},
    widgets::{Fit, draw_text, text_attributes, view_png},
};
use objc2::{
    DefinedClass, MainThreadMarker, MainThreadOnly, define_class, msg_send, rc::Retained,
    runtime::AnyClass,
};
use objc2_app_kit::{
    NSAccessibility, NSEvent, NSGlassEffectView, NSScreen, NSView, NSVisualEffectBlendingMode,
    NSVisualEffectMaterial, NSVisualEffectState, NSVisualEffectView,
};
use objc2_foundation::{NSPoint, NSRect, NSSize, NSString};
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
};

/// The size of the card.
pub const CARD: NSSize = NSSize::new(440.0, 260.0);
const RADIUS: f64 = 26.0;
const BUTTON: NSSize = NSSize::new(150.0, 32.0);

/// What the card says.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct BreakDisplay {
    /// "Look into the distance"
    pub title: String,
    /// One or two lines of what to do.
    pub hint: String,
    /// The time left, "0:18".
    pub left: String,
    pub skip: String,
    pub later: String,
}

pub struct CardIvars {
    display: RefCell<BreakDisplay>,
    /// A dark shape standing in for the glass, which the window server draws on screen and no picture can hold.
    backdrop: Cell<bool>,
}

define_class!(
    // SAFETY: NSView allows subclassing; the ivars are only touched on the main thread and need no Drop.
    #[unsafe(super(NSView))]
    #[thread_kind = MainThreadOnly]
    #[name = "FigBreakCard"]
    #[ivars = CardIvars]
    pub struct BreakCard;

    impl BreakCard {
        #[unsafe(method(drawRect:))]
        fn draw_rect(&self, _dirty: NSRect) {
            self.paint();
        }

        #[unsafe(method(acceptsFirstMouse:))]
        fn accepts_first_mouse(&self, _event: Option<&NSEvent>) -> bool {
            true
        }
    }
);

impl BreakCard {
    fn new(mtm: MainThreadMarker, frame: NSRect) -> Retained<Self> {
        let this = mtm.alloc::<Self>().set_ivars(CardIvars {
            display: RefCell::default(),
            backdrop: Cell::new(false),
        });
        // SAFETY: designated initializer with valid arguments on the main thread.
        unsafe { msg_send![super(this), initWithFrame: frame] }
    }

    fn paint(&self) {
        let (w, h) = (CARD.width, CARD.height);
        let display = self.ivars().display.borrow();
        if self.ivars().backdrop.get() {
            style::fill_rounded(self.bounds(), RADIUS, &style::black(0.86));
        }
        let centred = |size: f64, weight: Weight, alpha: f64, fit: Fit| {
            text_attributes(&style::font(size, weight), &white(alpha), fit, true)
        };
        style::draw_symbol("eye", NSPoint::new(w / 2.0, h - 40.0), 26.0, &white(0.8));
        draw_text(
            &display.title,
            NSRect::new(NSPoint::new(24.0, h - 92.0), NSSize::new(w - 48.0, 26.0)),
            &centred(20.0, Weight::Semibold, style::TEXT, Fit::Tail),
        );
        draw_text(
            &display.hint,
            NSRect::new(NSPoint::new(32.0, h - 134.0), NSSize::new(w - 64.0, 36.0)),
            &centred(13.0, Weight::Regular, style::TEXT_SECONDARY, Fit::Words),
        );
        draw_text(
            &display.left,
            NSRect::new(NSPoint::new(24.0, 62.0), NSSize::new(w - 48.0, 48.0)),
            &text_attributes(
                &style::digits(40.0, Weight::Light),
                &white(style::TEXT),
                Fit::Clip,
                true,
            ),
        );
    }
}

pub struct BreakScreen {
    mtm: MainThreadMarker,
    covers: RefCell<Vec<Retained<FigPanel>>>,
    card_panel: Retained<FigPanel>,
    card: Retained<BreakCard>,
    skip: Retained<TouchButton>,
    later: Retained<TouchButton>,
}

impl BreakScreen {
    pub fn new(
        mtm: MainThreadMarker,
        on_skip: impl Fn() + 'static,
        on_later: impl Fn() + 'static,
    ) -> Rc<Self> {
        let frame = NSRect::new(NSPoint::ZERO, CARD);
        let card_panel = FigPanel::new(mtm, frame);
        let card = BreakCard::new(mtm, frame);
        let button = |x: f64| {
            let b = TouchButton::new(mtm, NSRect::new(NSPoint::new(x, 18.0), BUTTON), Look::Rect);
            card.addSubview(&b);
            b
        };
        let gap = 12.0;
        let left = (CARD.width - 2.0 * BUTTON.width - gap) / 2.0;
        let skip = button(left);
        let later = button(left + BUTTON.width + gap);
        skip.set_on_click(on_skip);
        later.set_on_click(on_later);
        // A card of Liquid Glass with the notch's tint; before macOS 26 the vibrancy of the same darkness.
        let bed: Retained<NSView> = match glass_tint(Material::Glass) {
            Some((look, tint)) if AnyClass::get(c"NSGlassEffectView").is_some() => {
                let glass = NSGlassEffectView::initWithFrame(mtm.alloc(), frame);
                glass.setStyle(look);
                glass.setTintColor(Some(&tint));
                glass.setCornerRadius(RADIUS);
                glass.setContentView(Some(&card));
                Retained::into_super(glass)
            }
            _ => {
                let blur = NSVisualEffectView::initWithFrame(mtm.alloc(), frame);
                blur.setMaterial(vibrancy_of(Material::Glass));
                blur.setBlendingMode(NSVisualEffectBlendingMode::BehindWindow);
                blur.setState(NSVisualEffectState::Active);
                blur.setWantsLayer(true);
                if let Some(layer) = blur.layer() {
                    layer.setCornerRadius(RADIUS);
                    layer.setMasksToBounds(true);
                }
                blur.addSubview(&card);
                Retained::into_super(blur)
            }
        };
        card_panel.setContentView(Some(&bed));
        card_panel.setHasShadow(true);
        let screen = Rc::new(Self {
            mtm,
            covers: RefCell::default(),
            card_panel,
            card,
            skip,
            later,
        });
        let weak = Rc::downgrade(&screen);
        screen.card_panel.set_key_handler(move |key| {
            key == Key::Escape && weak.upgrade().is_some_and(|screen| screen.press("skip"))
        });
        screen
    }

    pub fn is_visible(&self) -> bool {
        self.card_panel.isVisible()
    }

    /// Puts up the covers and the card, or only draws the card again when they are up.
    pub fn show(&self, display: &BreakDisplay) {
        self.set(display);
        if self.is_visible() {
            return;
        }
        let screens = NSScreen::screens(self.mtm);
        let covers: Vec<Retained<FigPanel>> = screens
            .iter()
            .map(|screen| {
                let frame = screen.frame();
                let panel = FigPanel::new(self.mtm, frame);
                let blur = NSVisualEffectView::initWithFrame(
                    self.mtm.alloc(),
                    NSRect::new(NSPoint::ZERO, frame.size),
                );
                blur.setMaterial(NSVisualEffectMaterial::FullScreenUI);
                blur.setBlendingMode(NSVisualEffectBlendingMode::BehindWindow);
                blur.setState(NSVisualEffectState::Active);
                panel.setContentView(Some(&blur));
                panel.orderFrontRegardless();
                panel
            })
            .collect();
        *self.covers.borrow_mut() = covers;
        // The card goes on the screen with the pointer, in its middle.
        let pointer = NSEvent::mouseLocation();
        let screen = screens
            .iter()
            .find(|s| {
                let f = s.frame();
                pointer.x >= f.origin.x
                    && pointer.x <= f.origin.x + f.size.width
                    && pointer.y >= f.origin.y
                    && pointer.y <= f.origin.y + f.size.height
            })
            .or_else(|| screens.firstObject());
        if let Some(screen) = screen {
            let f = screen.frame();
            self.card_panel.setFrame_display(
                NSRect::new(
                    NSPoint::new(
                        f.origin.x + (f.size.width - CARD.width) / 2.0,
                        f.origin.y + (f.size.height - CARD.height) / 2.0,
                    ),
                    CARD,
                ),
                true,
            );
        }
        self.card_panel.makeKeyAndOrderFront(None);
    }

    /// Takes the covers and the card down.
    pub fn hide(&self) {
        for cover in self.covers.borrow_mut().drain(..) {
            cover.orderOut(None);
        }
        self.card_panel.orderOut(None);
    }

    fn set(&self, display: &BreakDisplay) {
        self.skip.set_title(&display.skip);
        self.later.set_title(&display.later);
        let label = format!("{}. {} {}", display.title, display.hint, display.left);
        self.card
            .setAccessibilityLabel(Some(&NSString::from_str(&label)));
        *self.card.ivars().display.borrow_mut() = display.clone();
        self.card.setNeedsDisplay(true);
    }

    /// The card alone as PNG bytes, without showing anything, for the pictures of the documentation.
    pub fn card_png(&self, display: &BreakDisplay) -> Option<Vec<u8>> {
        self.set(display);
        self.card.ivars().backdrop.set(true);
        let png = view_png(&self.card);
        self.card.ivars().backdrop.set(false);
        png
    }

    /// Presses a button the way an assistive tool does: "skip" or "later". False for any other name.
    pub fn press(&self, what: &str) -> bool {
        let button = match what {
            "skip" => &self.skip,
            "later" => &self.later,
            _ => return false,
        };
        // SAFETY: accessibilityPerformPress takes no arguments and returns whether it pressed.
        unsafe { msg_send![&**button, accessibilityPerformPress] }
    }
}
