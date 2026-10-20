//! A button that draws itself in the notch's style: a round glyph button, a rounded action button, or a chip.
//! One class for all three keeps every page's controls identical in size, colour and feedback.

use crate::{
    style::{self, BUTTON_RADIUS, Control, TEXT, Tone, Weight, black, white},
    widgets::{Fit, draw_text, text_attributes},
};
use objc2::{
    AnyThread, DefinedClass, MainThreadMarker, MainThreadOnly, define_class, msg_send, rc::Retained,
};
use objc2_app_kit::{
    NSAccessibilityButtonRole, NSEvent, NSImage, NSStringDrawing, NSTrackingArea,
    NSTrackingAreaOptions, NSView,
};
use objc2_foundation::{NSCopying, NSObjectProtocol, NSPoint, NSRect, NSSize, NSString};
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Look {
    /// A circle holding a glyph. Filled, it is the one thing to press on the page.
    Round { filled: bool },
    /// A rounded rectangle with a glyph and a label side by side.
    Rect,
    /// A capsule with a glyph, a label and a count; white while selected.
    Chip,
    /// A row of a list: the title over a dimmer line, with no shape of its own until it is hovered or chosen.
    Row,
    /// A tile of a grid: a picture (an app's icon, or a glyph) with its name under it.
    Tile,
}

const CHIP_PAD: f64 = 12.0;
const GLYPH_GAP: f64 = 6.0;
const COUNT_GAP: f64 = 6.0;

pub struct ButtonIvars {
    look: Cell<Look>,
    symbol: RefCell<String>,
    title: RefCell<String>,
    detail: RefCell<String>,
    selected: Cell<bool>,
    enabled: Cell<bool>,
    hovered: Cell<bool>,
    pressed: Cell<bool>,
    tone: Cell<Option<Tone>>,
    /// A picture for a tile, drawn instead of the glyph.
    image: RefCell<Option<Retained<NSImage>>>,
    on_click: RefCell<Option<Rc<dyn Fn()>>>,
}

define_class!(
    // SAFETY: NSView allows subclassing; the ivars are only touched on the main thread and need no Drop.
    #[unsafe(super(NSView))]
    #[thread_kind = MainThreadOnly]
    #[name = "FigTouchButton"]
    #[ivars = ButtonIvars]
    pub struct TouchButton;

    impl TouchButton {
        #[unsafe(method(acceptsFirstMouse:))]
        fn accepts_first_mouse(&self, _event: Option<&NSEvent>) -> bool {
            true
        }

        #[unsafe(method(drawRect:))]
        fn draw_rect(&self, _dirty: NSRect) {
            self.paint();
        }

        // The button is an element of its own for VoiceOver and for anything that drives the interface through
        // the accessibility tree, named by its label and pressable.
        #[unsafe(method(isAccessibilityElement))]
        fn is_accessibility_element(&self) -> bool {
            true
        }

        #[unsafe(method_id(accessibilityRole))]
        fn accessibility_role(&self) -> Option<Retained<NSString>> {
            // SAFETY: an extern static constant that lives as long as the process.
            Some(unsafe { NSAccessibilityButtonRole }.copy())
        }

        #[unsafe(method_id(accessibilityLabel))]
        fn accessibility_label(&self) -> Option<Retained<NSString>> {
            Some(NSString::from_str(&self.accessible_name()))
        }

        #[unsafe(method(accessibilityPerformPress))]
        fn accessibility_perform_press(&self) -> bool {
            self.press()
        }

        #[unsafe(method(mouseEntered:))]
        fn mouse_entered(&self, _event: &NSEvent) {
            self.ivars().hovered.set(true);
            self.setNeedsDisplay(true);
        }

        #[unsafe(method(mouseExited:))]
        fn mouse_exited(&self, _event: &NSEvent) {
            self.ivars().hovered.set(false);
            self.setNeedsDisplay(true);
        }

        #[unsafe(method(mouseDown:))]
        fn mouse_down(&self, _event: &NSEvent) {
            if self.ivars().enabled.get() {
                self.ivars().pressed.set(true);
                self.setNeedsDisplay(true);
            }
        }

        #[unsafe(method(mouseUp:))]
        fn mouse_up(&self, event: &NSEvent) {
            let was_pressed = self.ivars().pressed.replace(false);
            self.setNeedsDisplay(true);
            let point = self.convertPoint_fromView(event.locationInWindow(), None);
            let bounds = self.bounds();
            let inside = point.x >= 0.0
                && point.y >= 0.0
                && point.x <= bounds.size.width
                && point.y <= bounds.size.height;
            if was_pressed && inside {
                self.press();
            }
        }
    }

    unsafe impl NSObjectProtocol for TouchButton {}
);

impl TouchButton {
    pub fn new(mtm: MainThreadMarker, frame: NSRect, look: Look) -> Retained<Self> {
        let ivars = ButtonIvars {
            look: Cell::new(look),
            symbol: RefCell::new(String::new()),
            title: RefCell::new(String::new()),
            detail: RefCell::new(String::new()),
            selected: Cell::new(false),
            enabled: Cell::new(true),
            hovered: Cell::new(false),
            pressed: Cell::new(false),
            tone: Cell::new(None),
            image: RefCell::new(None),
            on_click: RefCell::new(None),
        };
        // SAFETY: designated initializer with valid arguments on the main thread.
        let this: Retained<Self> =
            unsafe { msg_send![super(mtm.alloc::<Self>().set_ivars(ivars)), initWithFrame: frame] };
        // SAFETY: valid options and owner; InVisibleRect ignores the (empty) rect.
        let area = unsafe {
            NSTrackingArea::initWithRect_options_owner_userInfo(
                NSTrackingArea::alloc(),
                NSRect::ZERO,
                NSTrackingAreaOptions::MouseEnteredAndExited
                    | NSTrackingAreaOptions::ActiveAlways
                    | NSTrackingAreaOptions::InVisibleRect,
                Some(&this),
                None,
            )
        };
        this.addTrackingArea(&area);
        this
    }

    /// What a person or an assistive tool calls the button: its label, or its glyph's name when it has none.
    fn accessible_name(&self) -> String {
        let iv = self.ivars();
        let title = iv.title.borrow();
        if title.is_empty() {
            iv.symbol.borrow().replace('.', " ")
        } else {
            title.clone()
        }
    }

    /// Runs the click action; false when the button is off or has none.
    fn press(&self) -> bool {
        if !self.ivars().enabled.get() {
            return false;
        }
        let click = self.ivars().on_click.borrow().clone();
        match click {
            Some(click) => {
                click();
                true
            }
            None => false,
        }
    }

    pub fn set_on_click(&self, f: impl Fn() + 'static) {
        *self.ivars().on_click.borrow_mut() = Some(Rc::new(f));
    }

    pub fn set_symbol(&self, name: &str) {
        name.clone_into(&mut self.ivars().symbol.borrow_mut());
        self.setNeedsDisplay(true);
    }

    /// The label. A round button does not draw it, but it names the button as a tooltip.
    pub fn set_title(&self, title: &str) {
        title.clone_into(&mut self.ivars().title.borrow_mut());
        if matches!(self.ivars().look.get(), Look::Round { .. }) {
            self.setToolTip(Some(&NSString::from_str(title)));
        }
        self.setNeedsDisplay(true);
    }

    /// The dimmer text after the label, a count on a chip.
    pub fn set_detail(&self, detail: &str) {
        detail.clone_into(&mut self.ivars().detail.borrow_mut());
        self.setNeedsDisplay(true);
    }

    pub fn set_selected(&self, selected: bool) {
        self.ivars().selected.set(selected);
        self.setNeedsDisplay(true);
    }

    pub fn set_enabled(&self, enabled: bool) {
        self.ivars().enabled.set(enabled);
        self.setNeedsDisplay(true);
    }

    /// Colours the glyph, for a button whose meaning is a state (stop in red, approve in green).
    pub fn set_tone(&self, tone: Option<Tone>) {
        self.ivars().tone.set(tone);
        self.setNeedsDisplay(true);
    }

    /// The picture of a tile, such as an app's icon.
    pub fn set_image(&self, image: Option<Retained<NSImage>>) {
        *self.ivars().image.borrow_mut() = image;
        self.setNeedsDisplay(true);
    }

    pub fn set_look(&self, look: Look) {
        self.ivars().look.set(look);
        self.setNeedsDisplay(true);
    }

    pub fn is_selected(&self) -> bool {
        self.ivars().selected.get()
    }

    fn glyph_size(&self) -> f64 {
        match self.ivars().look.get() {
            Look::Round { .. } => {
                let b = self.bounds().size;
                (b.width.min(b.height) * 0.42).round()
            }
            Look::Rect => 15.0,
            Look::Chip => 12.0,
            Look::Row => 14.0,
            Look::Tile => 28.0,
        }
    }

    fn title_font(&self) -> Retained<objc2_app_kit::NSFont> {
        match self.ivars().look.get() {
            Look::Rect => style::font(13.0, Weight::Semibold),
            _ => style::font(12.0, Weight::Semibold),
        }
    }

    fn text_width(text: &str, font: &objc2_app_kit::NSFont) -> f64 {
        if text.is_empty() {
            return 0.0;
        }
        let attrs = text_attributes(font, &white(1.0), Fit::Clip, false);
        // SAFETY: the attributes dictionary holds valid attribute objects.
        unsafe { NSString::from_str(text).sizeWithAttributes(Some(&attrs)) }
            .width
            .ceil()
    }

    /// The width that fits the glyph, label and count, for laying a row of chips out.
    pub fn fit_width(&self) -> f64 {
        let iv = self.ivars();
        let font = self.title_font();
        let mut width = Self::text_width(&iv.title.borrow(), &font);
        if !iv.symbol.borrow().is_empty() {
            width += self.glyph_size() + if width > 0.0 { GLYPH_GAP } else { 0.0 };
        }
        let detail = Self::text_width(&iv.detail.borrow(), &style::font(12.0, Weight::Regular));
        if detail > 0.0 {
            width += COUNT_GAP + detail;
        }
        width + 2.0 * CHIP_PAD
    }

    /// A list row: a faint panel while hovered, a brighter one while chosen, and the title over its detail.
    fn paint_row(&self) {
        let iv = self.ivars();
        let bounds = self.bounds();
        let shade = if !iv.enabled.get() {
            0.0
        } else if iv.pressed.get() {
            0.16
        } else if iv.selected.get() {
            0.13
        } else if iv.hovered.get() {
            0.07
        } else {
            0.0
        };
        if shade > 0.0 {
            style::fill_rounded(bounds, 9.0, &white(shade));
        }
        let (title, detail) = (iv.title.borrow(), iv.detail.borrow());
        let pad = 10.0;
        let width = (bounds.size.width - 2.0 * pad).max(0.0);
        let mid = bounds.size.height / 2.0;
        let title_attrs = text_attributes(
            &style::font(12.0, Weight::Medium),
            &white(TEXT),
            Fit::Tail,
            false,
        );
        if detail.is_empty() {
            draw_text(
                &title,
                NSRect::new(NSPoint::new(pad, mid - 8.0), NSSize::new(width, 16.0)),
                &title_attrs,
            );
            return;
        }
        draw_text(
            &title,
            NSRect::new(NSPoint::new(pad, mid + 0.5), NSSize::new(width, 16.0)),
            &title_attrs,
        );
        draw_text(
            &detail,
            NSRect::new(NSPoint::new(pad, mid - 15.5), NSSize::new(width, 14.0)),
            &text_attributes(
                &style::font(10.5, Weight::Regular),
                &white(style::TEXT_SECONDARY),
                Fit::Tail,
                false,
            ),
        );
    }

    /// A tile: a faint panel while hovered, the picture above and the name under it.
    fn paint_tile(&self) {
        let iv = self.ivars();
        let bounds = self.bounds();
        let shade = if !iv.enabled.get() {
            0.0
        } else if iv.pressed.get() {
            0.16
        } else if iv.hovered.get() {
            0.09
        } else {
            0.0
        };
        if shade > 0.0 {
            style::fill_rounded(bounds, 12.0, &white(shade));
        }
        let side = 40.0;
        let picture = NSRect::new(
            NSPoint::new(
                (bounds.size.width - side) / 2.0,
                bounds.size.height - 10.0 - side,
            ),
            NSSize::new(side, side),
        );
        if let Some(image) = iv.image.borrow().as_ref() {
            image.drawInRect(picture);
        } else if !iv.symbol.borrow().is_empty() {
            let tone = iv.tone.get().map_or_else(|| white(0.9), Tone::color);
            style::draw_symbol(
                &iv.symbol.borrow(),
                NSPoint::new(picture.origin.x + side / 2.0, picture.origin.y + side / 2.0),
                28.0,
                &tone,
            );
        }
        draw_text(
            &iv.title.borrow(),
            NSRect::new(
                NSPoint::new(4.0, 6.0),
                NSSize::new((bounds.size.width - 8.0).max(0.0), 28.0),
            ),
            &text_attributes(
                &style::font(10.5, Weight::Medium),
                &white(TEXT),
                Fit::Words,
                true,
            ),
        );
    }

    fn paint(&self) {
        let iv = self.ivars();
        let bounds = self.bounds();
        let (look, enabled) = (iv.look.get(), iv.enabled.get());
        if look == Look::Row {
            self.paint_row();
            return;
        }
        if look == Look::Tile {
            self.paint_tile();
            return;
        }
        let solid = matches!(look, Look::Round { filled: true })
            || (matches!(look, Look::Chip | Look::Rect) && iv.selected.get());
        // What separates the looks of a control is only how it is being used.
        let state = if !enabled {
            Control::Disabled
        } else if iv.pressed.get() {
            Control::Pressed
        } else if iv.hovered.get() {
            Control::Hover
        } else if iv.selected.get() {
            Control::Chosen
        } else {
            Control::Rest
        };
        let ink = if solid { black(0.88) } else { white(TEXT) };
        let dim = match state {
            Control::Pressed => 0.80,
            Control::Hover => 1.0,
            Control::Disabled => 0.62,
            _ => 0.95,
        };
        // Every control is glass: a shape lit from above with an edge that holds it together. A solid one is the
        // bright glass the light goes through; the rest stand on the surface.
        let shape = |rect: NSRect, radius: f64| {
            if solid {
                style::card_bright_alpha(rect, radius, dim);
            } else {
                style::control(rect, radius, state);
            }
        };
        match look {
            Look::Round { .. } => {
                let side = bounds.size.width.min(bounds.size.height);
                let circle = NSRect::new(
                    NSPoint::new(
                        (bounds.size.width - side) / 2.0,
                        (bounds.size.height - side) / 2.0,
                    ),
                    NSSize::new(side, side),
                );
                shape(circle, side / 2.0);
            }
            Look::Rect => shape(bounds, BUTTON_RADIUS),
            Look::Chip => shape(bounds, bounds.size.height / 2.0),
            // Drawn by `paint_row` and `paint_tile` above, which returned already.
            Look::Row | Look::Tile => {}
        }

        let glyph_color = match (iv.tone.get(), solid) {
            (Some(tone), false) => tone.color(),
            _ => ink.clone(),
        };
        let glyph = self.glyph_size();
        let symbol = iv.symbol.borrow();
        let title = iv.title.borrow();
        let detail = iv.detail.borrow();
        let show_text = !matches!(look, Look::Round { .. });
        let font = self.title_font();
        let title_width = if show_text {
            Self::text_width(&title, &font)
        } else {
            0.0
        };
        let detail_font = style::font(12.0, Weight::Regular);
        let detail_width = if show_text {
            Self::text_width(&detail, &detail_font)
        } else {
            0.0
        };
        let has_glyph = !symbol.is_empty();
        let mut total = 0.0;
        if has_glyph {
            total += glyph;
        }
        if title_width > 0.0 {
            total += if has_glyph { GLYPH_GAP } else { 0.0 } + title_width;
        }
        if detail_width > 0.0 {
            total += COUNT_GAP + detail_width;
        }
        let mut x = (bounds.size.width - total) / 2.0;
        let mid = bounds.size.height / 2.0;
        if has_glyph {
            style::draw_symbol(
                &symbol,
                NSPoint::new(x + glyph / 2.0, mid),
                glyph,
                &glyph_color,
            );
            x += glyph + if title_width > 0.0 { GLYPH_GAP } else { 0.0 };
        }
        let line = font.ascender() - font.descender() + font.leading();
        if title_width > 0.0 {
            draw_text(
                &title,
                NSRect::new(
                    NSPoint::new(x, mid - line / 2.0),
                    NSSize::new(title_width + 2.0, line),
                ),
                &text_attributes(&font, &ink, Fit::Clip, false),
            );
            x += title_width;
        }
        if detail_width > 0.0 {
            let dim = if solid { black(0.45) } else { white(0.45) };
            draw_text(
                &detail,
                NSRect::new(
                    NSPoint::new(x + COUNT_GAP, mid - line / 2.0),
                    NSSize::new(detail_width + 2.0, line),
                ),
                &text_attributes(&detail_font, &dim, Fit::Clip, false),
            );
        }
    }
}
