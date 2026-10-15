//! The window Smart Select answers in: what was read, a list of answers or a block of text, and a hint line.
//! It takes the keyboard without activating the app, like the clipboard popup, and closes when it loses it.

use crate::{
    panel::{FigPanel, Key},
    style::{self, Tone, Weight},
    ui::origin_near_pointer,
    widgets::{Fit, draw_text, text_attributes},
};
use block2::RcBlock;
use objc2::{
    AnyThread, DefinedClass, MainThreadMarker, MainThreadOnly, define_class, msg_send,
    rc::Retained,
    runtime::{AnyObject, ProtocolObject},
};
use objc2_app_kit::{
    NSAccessibilityStaticTextRole, NSAutoresizingMaskOptions, NSBitmapImageFileType, NSColor,
    NSEvent, NSEventMask, NSFont, NSStringDrawingDeprecated, NSStringDrawingOptions,
    NSTrackingArea, NSTrackingAreaOptions, NSView, NSVisualEffectBlendingMode,
    NSVisualEffectMaterial, NSVisualEffectState, NSVisualEffectView,
    NSWindowDidResignKeyNotification,
};
use objc2_foundation::{
    NSCopying, NSDictionary, NSNotification, NSNotificationCenter, NSObjectProtocol, NSPoint,
    NSRect, NSSize, NSString,
};
use std::{cell::RefCell, ptr::NonNull, rc::Rc};

pub const WIDTH: f64 = 380.0;
const PAD: f64 = 10.0;
const HEADER: f64 = 40.0;
const ROW: f64 = 50.0;
const FOOTER: f64 = 30.0;
const TEXT_PAD: f64 = 6.0;
/// Longest block of text before it is cut off.
const TEXT_MAX: f64 = 220.0;
pub const MAX_ROWS: usize = 7;
const RADIUS: f64 = 14.0;

/// One answer: what it is, the text that is copied, and a line saying more.
#[derive(Debug, Clone, PartialEq)]
pub struct SmartRow {
    pub symbol: &'static str,
    pub tone: Tone,
    pub title: String,
    pub detail: String,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Body {
    Rows(Vec<SmartRow>),
    /// A block of text. Muted text is a message, not an answer: it is dim and cannot be copied.
    Text {
        text: String,
        muted: bool,
    },
}

#[derive(Debug, Clone, PartialEq)]
pub struct Content {
    pub header: String,
    pub body: Body,
    pub footer: String,
}

struct State {
    content: Content,
    selected: usize,
    /// While drawing an image of the view for a test: a background of its own, since the blur behind the real
    /// window is not part of the view.
    backdrop: bool,
}

type CopyHandler = Rc<dyn Fn(String)>;

pub struct ViewIvars {
    state: RefCell<State>,
    on_copy: RefCell<Option<CopyHandler>>,
}

fn body_font() -> Retained<NSFont> {
    style::font(14.0, Weight::Regular)
}

/// The height a block of text needs at `width`.
fn text_height(text: &str, width: f64) -> f64 {
    let attrs = text_attributes(&body_font(), &NSColor::whiteColor(), Fit::Words, false);
    // SAFETY: the attributes are a font, a colour and a paragraph style under their own keys.
    let bounds = unsafe {
        NSString::from_str(text).boundingRectWithSize_options_attributes(
            NSSize::new(width, 10_000.0),
            NSStringDrawingOptions::UsesLineFragmentOrigin,
            Some(&attrs),
        )
    };
    bounds.size.height.ceil()
}

fn height_of(content: &Content) -> f64 {
    let body = match &content.body {
        Body::Rows(rows) => rows.len().clamp(1, MAX_ROWS) as f64 * ROW + 6.0,
        Body::Text { text, .. } => {
            text_height(text, WIDTH - 2.0 * (PAD + TEXT_PAD)).min(TEXT_MAX) + 16.0
        }
    };
    HEADER + body + FOOTER
}

fn spoken(content: &Content) -> String {
    match &content.body {
        Body::Rows(rows) => rows
            .iter()
            .map(|r| {
                if r.detail.is_empty() {
                    r.title.clone()
                } else {
                    format!("{}, {}", r.title, r.detail)
                }
            })
            .collect::<Vec<_>>()
            .join("\n"),
        Body::Text { text, .. } => text.clone(),
    }
}

define_class!(
    // SAFETY: NSView allows subclassing; the ivars are only touched on the main thread and need no Drop.
    #[unsafe(super(NSView))]
    #[thread_kind = MainThreadOnly]
    #[name = "FigSmartView"]
    #[ivars = ViewIvars]
    pub struct SmartView;

    impl SmartView {
        #[unsafe(method(isFlipped))]
        fn is_flipped(&self) -> bool {
            true
        }

        #[unsafe(method(acceptsFirstMouse:))]
        fn accepts_first_mouse(&self, _event: Option<&NSEvent>) -> bool {
            true
        }

        #[unsafe(method(drawRect:))]
        fn draw_rect(&self, _dirty: NSRect) {
            self.paint();
        }

        #[unsafe(method(mouseMoved:))]
        fn mouse_moved(&self, event: &NSEvent) {
            if let Some(row) = self.row_at(event) {
                self.select(row);
            }
        }

        #[unsafe(method(mouseDown:))]
        fn mouse_down(&self, event: &NSEvent) {
            if let Some(row) = self.row_at(event) {
                self.select(row);
                self.activate(Some(row));
            }
        }

        // What is on screen is also what a screen reader and the accessibility tree report.
        #[unsafe(method(isAccessibilityElement))]
        fn is_accessibility_element(&self) -> bool {
            true
        }

        #[unsafe(method_id(accessibilityRole))]
        fn accessibility_role(&self) -> Option<Retained<NSString>> {
            // SAFETY: an extern static constant that lives as long as the process.
            Some(unsafe { NSAccessibilityStaticTextRole }.copy())
        }

        #[unsafe(method_id(accessibilityLabel))]
        fn accessibility_label(&self) -> Option<Retained<NSString>> {
            Some(NSString::from_str(&self.ivars().state.borrow().content.header))
        }

        #[unsafe(method_id(accessibilityValue))]
        fn accessibility_value(&self) -> Option<Retained<AnyObject>> {
            let text = spoken(&self.ivars().state.borrow().content);
            // SAFETY: an NSString is an object, which is what an accessibility value may be.
            Some(unsafe { Retained::cast_unchecked(NSString::from_str(&text)) })
        }
    }
);

impl SmartView {
    fn new(mtm: MainThreadMarker, frame: NSRect) -> Retained<Self> {
        let ivars = ViewIvars {
            state: RefCell::new(State {
                content: Content {
                    header: String::new(),
                    body: Body::Rows(Vec::new()),
                    footer: String::new(),
                },
                selected: 0,
                backdrop: false,
            }),
            on_copy: RefCell::new(None),
        };
        // SAFETY: designated initializer with valid arguments on the main thread.
        let this: Retained<Self> =
            unsafe { msg_send![super(mtm.alloc::<Self>().set_ivars(ivars)), initWithFrame: frame] };
        this.setAutoresizingMask(
            NSAutoresizingMaskOptions::ViewWidthSizable
                | NSAutoresizingMaskOptions::ViewHeightSizable,
        );
        // Hover selection works while the app is inactive, which a non-activating panel always is.
        // SAFETY: valid options and owner; InVisibleRect ignores the (empty) rect.
        let area = unsafe {
            NSTrackingArea::initWithRect_options_owner_userInfo(
                NSTrackingArea::alloc(),
                NSRect::ZERO,
                NSTrackingAreaOptions::MouseMoved
                    | NSTrackingAreaOptions::ActiveAlways
                    | NSTrackingAreaOptions::InVisibleRect,
                Some(&this),
                None,
            )
        };
        this.addTrackingArea(&area);
        this
    }

    fn rows_len(&self) -> usize {
        match &self.ivars().state.borrow().content.body {
            Body::Rows(rows) => rows.len().min(MAX_ROWS),
            Body::Text { .. } => 0,
        }
    }

    fn row_at(&self, event: &NSEvent) -> Option<usize> {
        let point = self.convertPoint_fromView(event.locationInWindow(), None);
        if point.y < HEADER {
            return None;
        }
        let row = ((point.y - HEADER) / ROW) as usize;
        (row < self.rows_len()).then_some(row)
    }

    fn select(&self, row: usize) {
        {
            let mut state = self.ivars().state.borrow_mut();
            if state.selected == row {
                return;
            }
            state.selected = row;
        }
        self.setNeedsDisplay(true);
    }

    fn move_selection(&self, delta: isize) {
        let len = self.rows_len();
        if len == 0 {
            return;
        }
        let current = self.ivars().state.borrow().selected as isize;
        self.select((current + delta).clamp(0, len as isize - 1) as usize);
    }

    /// The text a row (or, for a block of text, the whole block) puts on the clipboard.
    fn copy_text(&self, row: Option<usize>) -> Option<String> {
        let state = self.ivars().state.borrow();
        match &state.content.body {
            Body::Rows(rows) => rows
                .get(row.unwrap_or(state.selected))
                .map(|r| r.title.clone()),
            Body::Text { text, muted: false } if row.is_none() => Some(text.clone()),
            Body::Text { .. } => None,
        }
    }

    fn activate(&self, row: Option<usize>) {
        let Some(text) = self.copy_text(row) else {
            return;
        };
        let handler = self.ivars().on_copy.borrow().clone();
        if let Some(handler) = handler {
            handler(text);
        }
    }

    fn set_content(&self, content: Content, reset_selection: bool) {
        let mut state = self.ivars().state.borrow_mut();
        let rows = match &content.body {
            Body::Rows(rows) => rows.len().min(MAX_ROWS),
            Body::Text { .. } => 0,
        };
        state.selected = if reset_selection {
            0
        } else {
            state.selected.min(rows.saturating_sub(1))
        };
        state.content = content;
        drop(state);
        self.setNeedsDisplay(true);
    }

    fn paint(&self) {
        let state = self.ivars().state.borrow();
        let bounds = self.bounds();
        let width = bounds.size.width;
        if state.backdrop {
            style::fill_rounded(
                bounds,
                RADIUS,
                &NSColor::colorWithSRGBRed_green_blue_alpha(0.15, 0.15, 0.17, 1.0),
            );
        }
        let header = text_attributes(
            &style::font(14.0, Weight::Semibold),
            &style::white(0.95),
            Fit::Tail,
            false,
        );
        draw_text(
            &state.content.header,
            NSRect::new(
                NSPoint::new(PAD + TEXT_PAD, 12.0),
                NSSize::new(width - 2.0 * (PAD + TEXT_PAD), 20.0),
            ),
            &header,
        );
        match &state.content.body {
            Body::Rows(rows) => {
                for (index, row) in rows.iter().take(MAX_ROWS).enumerate() {
                    paint_row(row, index, index == state.selected, width);
                }
            }
            Body::Text { text, muted } => {
                let attrs = text_attributes(
                    &body_font(),
                    &style::white(if *muted { 0.58 } else { 0.95 }),
                    Fit::Words,
                    false,
                );
                let rect = NSRect::new(
                    NSPoint::new(PAD + TEXT_PAD, HEADER + 2.0),
                    NSSize::new(
                        width - 2.0 * (PAD + TEXT_PAD),
                        (bounds.size.height - HEADER - FOOTER - 6.0).max(0.0),
                    ),
                );
                draw_text(text, rect, &attrs);
            }
        }
        draw_text(
            &state.content.footer,
            NSRect::new(
                NSPoint::new(PAD + TEXT_PAD, bounds.size.height - FOOTER + 8.0),
                NSSize::new(width - 2.0 * (PAD + TEXT_PAD), 14.0),
            ),
            &text_attributes(
                &style::font(11.0, Weight::Regular),
                &style::white(0.42),
                Fit::Tail,
                false,
            ),
        );
    }
}

fn paint_row(row: &SmartRow, index: usize, selected: bool, width: f64) {
    let top = HEADER + index as f64 * ROW;
    let card = NSRect::new(
        NSPoint::new(PAD, top + 2.0),
        NSSize::new(width - 2.0 * PAD, ROW - 4.0),
    );
    if selected {
        style::fill_rounded(card, 12.0, &style::white(0.17));
        style::stroke_rounded(card, 12.0, &style::white(0.34), 1.5);
    }
    let middle = top + ROW / 2.0;
    let tile = NSRect::new(
        NSPoint::new(card.origin.x + 10.0, middle - 15.0),
        NSSize::new(30.0, 30.0),
    );
    let tint = row.tone.color();
    style::fill_circle(tile, &tint.colorWithAlphaComponent(0.22));
    style::draw_symbol(
        row.symbol,
        NSPoint::new(tile.origin.x + 15.0, middle),
        14.0,
        &tint,
    );
    let left = tile.origin.x + 30.0 + 12.0;
    let text_width = (card.origin.x + card.size.width - 12.0 - left).max(10.0);
    draw_text(
        &row.title,
        NSRect::new(NSPoint::new(left, top + 8.0), NSSize::new(text_width, 20.0)),
        &text_attributes(
            &style::font(15.0, Weight::Medium),
            &style::white(0.96),
            Fit::Tail,
            false,
        ),
    );
    draw_text(
        &row.detail,
        NSRect::new(
            NSPoint::new(left, top + 29.0),
            NSSize::new(text_width, 14.0),
        ),
        &text_attributes(
            &style::font(11.0, Weight::Regular),
            &style::white(0.54),
            Fit::Tail,
            false,
        ),
    );
}

/// The token of a global monitor, shared between the window and the monitor's own handler.
type MonitorSlot = Rc<RefCell<Option<Retained<AnyObject>>>>;

/// The answer window. Create it once; `show` it for each selection.
pub struct SmartPopup {
    panel: Retained<FigPanel>,
    view: Retained<SmartView>,
    resign: Retained<ProtocolObject<dyn NSObjectProtocol>>,
    /// Watches for a click in another app while the window is open: if the window did not manage to take the
    /// keyboard it never learns that the person moved on, and would stay until Escape could reach it.
    outside: MonitorSlot,
}

impl Drop for SmartPopup {
    fn drop(&mut self) {
        // SAFETY: `resign` was returned by the default center's addObserverForName.
        unsafe {
            NSNotificationCenter::defaultCenter()
                .removeObserver(AsRef::<AnyObject>::as_ref(&*self.resign));
        }
        if let Some(token) = self.outside.borrow_mut().take() {
            // SAFETY: the token came from addGlobalMonitorForEventsMatchingMask and is removed once.
            unsafe { NSEvent::removeMonitor(&token) };
        }
    }
}

impl SmartPopup {
    /// `on_copy` gets the text the person chose; the window is closed first.
    pub fn new(mtm: MainThreadMarker, on_copy: impl Fn(String) + 'static) -> Self {
        let frame = NSRect::new(NSPoint::ZERO, NSSize::new(WIDTH, HEADER + FOOTER + ROW));
        let panel = FigPanel::new(mtm, frame);
        panel.setHasShadow(true);
        // The window has no text field that would need the keyboard, so it has to ask for it: Return, Escape
        // and the arrows are only delivered to the key window.
        panel.setBecomesKeyOnlyIfNeeded(false);

        let effect = NSVisualEffectView::initWithFrame(mtm.alloc(), frame);
        effect.setMaterial(NSVisualEffectMaterial::HUDWindow);
        effect.setBlendingMode(NSVisualEffectBlendingMode::BehindWindow);
        effect.setState(NSVisualEffectState::Active);
        effect.setWantsLayer(true);
        if let Some(layer) = effect.layer() {
            // SAFETY: plain property setters on the view's backing layer.
            unsafe {
                let _: () = msg_send![&*layer, setCornerRadius: RADIUS];
                let _: () = msg_send![&*layer, setMasksToBounds: true];
            }
        }
        panel.setContentView(Some(&effect));

        let view = SmartView::new(mtm, frame);
        effect.addSubview(&view);
        let hide = |view: &SmartView| {
            if let Some(window) = view.window() {
                window.orderOut(None);
            }
        };
        {
            let view = view.clone();
            *view.ivars().on_copy.borrow_mut() = Some(Rc::new({
                let view = view.clone();
                move |text| {
                    // Close first so the keyboard goes back to the app the text came from.
                    hide(&view);
                    on_copy(text);
                }
            }));
        }
        {
            let view = view.clone();
            panel.set_key_handler(move |key| match key {
                Key::Up => {
                    view.move_selection(-1);
                    true
                }
                Key::Down => {
                    view.move_selection(1);
                    true
                }
                Key::Enter | Key::EnterAlt => {
                    view.activate(None);
                    true
                }
                Key::Tab(n) => {
                    view.activate(Some(usize::from(n).saturating_sub(1)));
                    true
                }
                Key::Escape => {
                    hide(&view);
                    true
                }
                _ => false,
            });
        }
        let closing = view.clone();
        let block = RcBlock::new(move |_: NonNull<NSNotification>| hide(&closing));
        let window: &AnyObject = &panel;
        // SAFETY: the block is 'static and runs on the main thread, where the notification is posted.
        let resign = unsafe {
            NSNotificationCenter::defaultCenter().addObserverForName_object_queue_usingBlock(
                Some(NSWindowDidResignKeyNotification),
                Some(window),
                None,
                &block,
            )
        };
        Self {
            panel,
            view,
            resign,
            outside: Rc::default(),
        }
    }

    /// Closes the window at the next click in another app. Global monitors never see clicks in this app's
    /// own windows, so a click on an answer is not one of them. The monitor removes itself when it fires.
    fn close_on_outside_click(&self) {
        if self.outside.borrow().is_some() {
            return;
        }
        let (view, slot) = (self.view.clone(), Rc::clone(&self.outside));
        let on_click = RcBlock::new(move |_: NonNull<NSEvent>| {
            if let Some(window) = view.window() {
                window.orderOut(None);
            }
            let slot = Rc::clone(&slot);
            // Not from inside the handler it belongs to.
            crate::app::after(0.0, move || {
                if let Some(token) = slot.borrow_mut().take() {
                    // SAFETY: the token came from addGlobalMonitorForEventsMatchingMask and is removed once.
                    unsafe { NSEvent::removeMonitor(&token) };
                }
            });
        });
        let token = NSEvent::addGlobalMonitorForEventsMatchingMask_handler(
            NSEventMask::LeftMouseDown | NSEventMask::RightMouseDown | NSEventMask::OtherMouseDown,
            &on_click,
        );
        *self.outside.borrow_mut() = token;
    }

    pub fn is_visible(&self) -> bool {
        self.panel.isVisible()
    }

    pub fn hide(&self) {
        self.panel.orderOut(None);
    }

    /// Opens next to the pointer with this content.
    pub fn show(&self, mtm: MainThreadMarker, content: Content) {
        self.resize(content, true);
        let size = self.panel.frame().size;
        self.panel.setFrameOrigin(origin_near_pointer(mtm, size));
        self.panel.makeKeyAndOrderFront(None);
        self.close_on_outside_click();
    }

    /// Replaces the content of the open window, keeping its place and the highlighted row.
    pub fn update(&self, content: Content) {
        self.resize(content, false);
    }

    fn resize(&self, content: Content, reset_selection: bool) {
        let height = height_of(&content);
        self.view.set_content(content, reset_selection);
        // The top edge stays where it is.
        let frame = self.panel.frame();
        let top = frame.origin.y + frame.size.height;
        let mut origin = NSPoint::new(frame.origin.x, top - height);
        if let Some(screen) = self.panel.screen() {
            let area = screen.visibleFrame();
            origin.y = origin.y.max(area.origin.y);
        }
        self.panel.setFrame_display(
            NSRect::new(origin, NSSize::new(WIDTH, height)),
            self.panel.isVisible(),
        );
    }

    /// The window drawn into PNG bytes without showing it, with a backdrop in place of the blur. For checking
    /// the design.
    pub fn snapshot_png(&self) -> Option<Vec<u8>> {
        self.view.ivars().state.borrow_mut().backdrop = true;
        let bounds = self.view.bounds();
        let png = self
            .view
            .bitmapImageRepForCachingDisplayInRect(bounds)
            .and_then(|rep| {
                self.view.cacheDisplayInRect_toBitmapImageRep(bounds, &rep);
                // SAFETY: an empty properties dictionary is valid for PNG output.
                unsafe {
                    rep.representationUsingType_properties(
                        NSBitmapImageFileType::PNG,
                        &NSDictionary::new(),
                    )
                }
            })
            .map(|data| data.to_vec());
        self.view.ivars().state.borrow_mut().backdrop = false;
        png
    }

    /// Hands an event to the window as the system would. For tests, which cannot use the keyboard.
    pub fn send_event(&self, event: &NSEvent) {
        self.panel.sendEvent(event);
    }

    /// Presses the highlighted row, as Return does. For tests.
    pub fn activate_selected(&self) {
        self.view.activate(None);
    }
}
