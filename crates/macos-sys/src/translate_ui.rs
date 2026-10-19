//! The window Translate answers in: Liquid Glass like the notch, the selected text shimmering while it is being
//! translated, then the translation rising into its place. Return or a click copies it, Escape closes it.

use crate::{
    app::{self, Timer},
    notch::{Material, glass_tint, vibrancy_of},
    panel::{FigPanel, Key},
    smart_ui::text_height,
    style::{self, Weight},
    ui::origin_near_pointer,
    widgets::{Fit, draw_text, text_attributes},
    workspace::display_options,
};
use block2::RcBlock;
use objc2::{
    DefinedClass, MainThreadMarker, MainThreadOnly, define_class, msg_send,
    rc::Retained,
    runtime::{AnyClass, AnyObject, ProtocolObject},
};
use objc2_app_kit::{
    NSAutoresizingMaskOptions, NSColor, NSEvent, NSEventMask, NSFont, NSGlassEffectView,
    NSGraphicsContext, NSRectClip, NSView, NSVisualEffectBlendingMode, NSVisualEffectState,
    NSVisualEffectView, NSWindowDidResignKeyNotification,
};
use objc2_foundation::{
    NSNotification, NSNotificationCenter, NSObjectProtocol, NSPoint, NSRect, NSSize,
};
use std::{cell::RefCell, ptr::NonNull, rc::Rc, time::Instant};

const WIDTH: f64 = 380.0;
const PAD: f64 = 20.0;
const HEADER: f64 = 46.0;
const FOOTER: f64 = 32.0;
/// Longest block of text before it is cut off.
const TEXT_MAX: f64 = 240.0;
const RADIUS: f64 = 22.0;
/// How long the translation takes to rise into place.
const REVEAL: f64 = 0.55;
/// How long the light takes to cross the text once while it waits.
const SWEEP: f64 = 1.3;
/// The bands of light, widest first: each is drawn over the last, so the light is brightest in the middle.
const BANDS: [(f64, f64); 3] = [(150.0, 0.26), (96.0, 0.28), (44.0, 0.32)];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Phase {
    /// The selection waits for its translation.
    Working,
    /// The translation has come.
    Done,
    /// A message instead of a translation: it is dim and cannot be copied.
    Message,
}

struct State {
    header: String,
    footer: String,
    source: String,
    /// The translation, or the message.
    result: String,
    phase: Phase,
    since: Instant,
    /// False when the person asked for less motion.
    motion: bool,
}

impl State {
    fn elapsed(&self) -> f64 {
        self.since.elapsed().as_secs_f64()
    }

    fn animating(&self) -> bool {
        match self.phase {
            Phase::Working => self.motion,
            Phase::Done => self.motion && self.elapsed() < REVEAL,
            Phase::Message => false,
        }
    }

    /// The text the window is about to show, which sets its height.
    fn shown(&self) -> &str {
        match self.phase {
            Phase::Working => &self.source,
            _ => &self.result,
        }
    }
}

type CopyHandler = Rc<dyn Fn(String)>;

pub struct ViewIvars {
    state: RefCell<State>,
    on_copy: RefCell<Option<CopyHandler>>,
}

fn body_font() -> Retained<NSFont> {
    style::font(14.0, Weight::Regular)
}

fn text_rect(bounds: NSRect) -> NSRect {
    NSRect::new(
        NSPoint::new(PAD, HEADER),
        NSSize::new(
            bounds.size.width - 2.0 * PAD,
            (bounds.size.height - HEADER - FOOTER).max(0.0),
        ),
    )
}

fn height_of(text: &str) -> f64 {
    HEADER + text_height(text, WIDTH - 2.0 * PAD).min(TEXT_MAX) + 8.0 + FOOTER
}

/// `text` drawn in `rect`, moved down by `lift` and at this brightness.
fn text_at(text: &str, rect: NSRect, lift: f64, alpha: f64) {
    let attrs = text_attributes(&body_font(), &style::white(alpha), Fit::Words, false);
    let moved = NSRect::new(NSPoint::new(rect.origin.x, rect.origin.y + lift), rect.size);
    draw_text(text, moved, &attrs);
}

/// A band of light across `text`: the text again, brighter, inside a window that crosses it from left to
/// right as `progress` goes from 0 to 1.
fn shimmer(text: &str, rect: NSRect, lift: f64, progress: f64, strength: f64) {
    let reach = BANDS[0].0;
    let centre = rect.origin.x - reach / 2.0 + (rect.size.width + reach) * progress;
    for (width, alpha) in BANDS {
        NSGraphicsContext::saveGraphicsState_class();
        NSRectClip(NSRect::new(
            NSPoint::new(centre - width / 2.0, rect.origin.y - 4.0),
            NSSize::new(width, rect.size.height + 8.0),
        ));
        text_at(text, rect, lift, alpha * strength);
        NSGraphicsContext::restoreGraphicsState_class();
    }
}

define_class!(
    // SAFETY: NSView allows subclassing; the ivars are only touched on the main thread and need no Drop.
    #[unsafe(super(NSView))]
    #[thread_kind = MainThreadOnly]
    #[name = "FigTranslateView"]
    #[ivars = ViewIvars]
    pub struct TranslateView;

    impl TranslateView {
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

        #[unsafe(method(mouseDown:))]
        fn mouse_down(&self, _event: &NSEvent) {
            self.copy();
        }
    }
);

impl TranslateView {
    fn new(mtm: MainThreadMarker, frame: NSRect) -> Retained<Self> {
        let ivars = ViewIvars {
            state: RefCell::new(State {
                header: String::new(),
                footer: String::new(),
                source: String::new(),
                result: String::new(),
                phase: Phase::Message,
                since: Instant::now(),
                motion: true,
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
        this
    }

    fn animating(&self) -> bool {
        self.window().is_some_and(|w| w.isVisible()) && self.ivars().state.borrow().animating()
    }

    fn copy(&self) {
        let text = {
            let state = self.ivars().state.borrow();
            (state.phase == Phase::Done).then(|| state.result.clone())
        };
        let handler = self.ivars().on_copy.borrow().clone();
        if let (Some(text), Some(handler)) = (text, handler) {
            handler(text);
        }
    }

    fn set(&self, f: impl FnOnce(&mut State)) {
        f(&mut self.ivars().state.borrow_mut());
        self.setNeedsDisplay(true);
    }

    fn paint(&self) {
        let state = self.ivars().state.borrow();
        let bounds = self.bounds();
        let width = bounds.size.width - 2.0 * PAD;
        style::draw_symbol(
            "globe",
            NSPoint::new(PAD + 8.0, 23.0),
            15.0,
            &style::Tone::Blue.color(),
        );
        draw_text(
            &state.header,
            NSRect::new(
                NSPoint::new(PAD + 24.0, 14.0),
                NSSize::new(width - 24.0, 18.0),
            ),
            &text_attributes(
                &style::font(13.0, Weight::Semibold),
                &style::white(0.92),
                Fit::Tail,
                false,
            ),
        );
        let rect = text_rect(bounds);
        let t = state.elapsed();
        match state.phase {
            Phase::Working => {
                text_at(&state.source, rect, 0.0, 0.42);
                if state.motion {
                    shimmer(&state.source, rect, 0.0, (t % SWEEP) / SWEEP, 1.0);
                } else {
                    text_at(&state.source, rect, 0.0, 0.2);
                }
            }
            Phase::Done => {
                let e = if state.motion {
                    style::ease_out(t / REVEAL)
                } else {
                    1.0
                };
                if e < 1.0 {
                    // The text that was waiting lifts away while the translation rises from below.
                    text_at(&state.source, rect, -10.0 * e, 0.42 * (1.0 - e));
                }
                text_at(&state.result, rect, 12.0 * (1.0 - e), 0.96 * e);
                if e < 1.0 {
                    shimmer(&state.result, rect, 12.0 * (1.0 - e), e, 1.0 - e);
                }
            }
            Phase::Message => text_at(&state.result, rect, 0.0, 0.6),
        }
        draw_text(
            &state.footer,
            NSRect::new(
                NSPoint::new(PAD, bounds.size.height - FOOTER + 8.0),
                NSSize::new(width, 14.0),
            ),
            &text_attributes(
                &style::font(11.0, Weight::Regular),
                &style::white(style::TEXT_TERTIARY),
                Fit::Tail,
                false,
            ),
        );
    }
}

/// The token of a global monitor, shared between the window and the monitor's own handler.
type MonitorSlot = Rc<RefCell<Option<Retained<AnyObject>>>>;

/// The translation window. Create it once; `show` it for each selection.
pub struct TranslatePopup {
    mtm: MainThreadMarker,
    panel: Retained<FigPanel>,
    view: Retained<TranslateView>,
    resign: Retained<ProtocolObject<dyn NSObjectProtocol>>,
    /// Watches for a click in another app while the window is open: a window that did not manage to take the
    /// keyboard never learns that the person moved on.
    outside: MonitorSlot,
    timer: Rc<RefCell<Option<Timer>>>,
}

impl Drop for TranslatePopup {
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

impl TranslatePopup {
    /// `on_copy` gets the translation the person chose to copy; the window is closed first.
    pub fn new(mtm: MainThreadMarker, on_copy: impl Fn(String) + 'static) -> Self {
        let frame = NSRect::new(NSPoint::ZERO, NSSize::new(WIDTH, HEADER + FOOTER + 40.0));
        let panel = FigPanel::new(mtm, frame);
        // The window's own shadow: one drawn under the glass would turn it grey.
        panel.setHasShadow(true);
        // Return and Escape are only delivered to the key window.
        panel.setBecomesKeyOnlyIfNeeded(false);
        let view = TranslateView::new(mtm, frame);
        let hide = |view: &TranslateView| {
            if let Some(window) = view.window() {
                window.orderOut(None);
            }
        };
        {
            let closing = view.clone();
            *view.ivars().on_copy.borrow_mut() = Some(Rc::new(move |text| {
                // Close first so the keyboard goes back to the app the text came from.
                hide(&closing);
                on_copy(text);
            }));
        }
        {
            let view = view.clone();
            panel.set_key_handler(move |key| match key {
                Key::Enter | Key::EnterAlt => {
                    view.copy();
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
        let this = Self {
            mtm,
            panel,
            view,
            resign,
            outside: Rc::default(),
            timer: Rc::default(),
        };
        this.dress();
        this
    }

    /// Puts the glass behind the view. Under Increase Contrast or Reduce Transparency the desktop must not
    /// show through, so the window is an opaque dark shape; before macOS 26 there is no Liquid Glass and a dark
    /// vibrancy material stands in.
    fn dress(&self) {
        let options = display_options();
        let frame = self.panel.frame();
        let size = NSRect::new(NSPoint::ZERO, frame.size);
        let solid = options.reduce_transparency || options.increase_contrast;
        let glass = glass_tint(Material::Glass)
            .filter(|_| !solid && AnyClass::get(c"NSGlassEffectView").is_some());
        if let Some((look, tint)) = glass {
            let glass = NSGlassEffectView::initWithFrame(self.mtm.alloc(), size);
            glass.setStyle(look);
            glass.setTintColor(Some(&tint));
            glass.setCornerRadius(RADIUS);
            glass.setContentView(Some(&self.view));
            self.panel.setContentView(Some(&glass));
            return;
        }
        let base: Retained<NSView> = if solid {
            let plain = NSView::initWithFrame(self.mtm.alloc(), size);
            plain.setWantsLayer(true);
            if let Some(layer) = plain.layer() {
                let fill = NSColor::colorWithSRGBRed_green_blue_alpha(0.11, 0.11, 0.13, 1.0);
                // SAFETY: plain property setters on the view's backing layer.
                unsafe {
                    let _: () = msg_send![&*layer, setBackgroundColor: &*fill.CGColor()];
                }
            }
            plain
        } else {
            let effect = NSVisualEffectView::initWithFrame(self.mtm.alloc(), size);
            effect.setMaterial(vibrancy_of(Material::Ink));
            effect.setBlendingMode(NSVisualEffectBlendingMode::BehindWindow);
            effect.setState(NSVisualEffectState::Active);
            effect.setWantsLayer(true);
            Retained::into_super(effect)
        };
        if let Some(layer) = base.layer() {
            // SAFETY: plain property setters on the view's backing layer.
            unsafe {
                let _: () = msg_send![&*layer, setCornerRadius: RADIUS];
                let _: () = msg_send![&*layer, setMasksToBounds: true];
            }
        }
        self.view.setFrame(size);
        base.addSubview(&self.view);
        self.panel.setContentView(Some(&base));
    }

    /// Closes the window at the next click in another app. Global monitors never see clicks in this app's
    /// own windows. The monitor removes itself when it fires.
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
            app::after(0.0, move || {
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

    /// Opens next to the pointer with `source` waiting to be translated.
    pub fn show_working(&self, header: &str, source: &str, footer: &str) {
        self.begin(Phase::Working, header, source, "", footer);
    }

    /// Opens next to the pointer with a message instead of a translation.
    pub fn show_message(&self, header: &str, message: &str, footer: &str) {
        self.begin(Phase::Message, header, "", message, footer);
    }

    fn begin(&self, phase: Phase, header: &str, source: &str, result: &str, footer: &str) {
        self.dress();
        self.view.set(|s| {
            s.header = header.to_owned();
            s.footer = footer.to_owned();
            s.source = source.to_owned();
            s.result = result.to_owned();
            s.phase = phase;
            s.since = Instant::now();
            s.motion = !display_options().reduce_motion;
        });
        self.resize(false);
        let size = self.panel.frame().size;
        self.panel
            .setFrameOrigin(origin_near_pointer(self.mtm, size));
        self.panel.makeKeyAndOrderFront(None);
        // A window outside every screen (tests) has no clicks to wait for.
        if std::env::var_os("FIGGHOST_NOTCH_OFFSCREEN").is_none() {
            self.close_on_outside_click();
        }
        self.animate();
    }

    /// The translation has come: it takes the place of the waiting text.
    pub fn finish(&self, translation: &str, footer: &str) {
        self.view.set(|s| {
            s.result = translation.to_owned();
            s.footer = footer.to_owned();
            s.phase = Phase::Done;
            s.since = Instant::now();
        });
        self.resize(true);
        self.animate();
    }

    /// The translation did not come: a message takes the place of the waiting text.
    pub fn fail(&self, header: &str, message: &str, footer: &str) {
        self.view.set(|s| {
            s.header = header.to_owned();
            s.result = message.to_owned();
            s.footer = footer.to_owned();
            s.phase = Phase::Message;
        });
        self.resize(true);
    }

    fn resize(&self, animate: bool) {
        let height = height_of(self.view.ivars().state.borrow().shown());
        // The top edge stays where it is.
        let frame = self.panel.frame();
        let top = frame.origin.y + frame.size.height;
        let mut origin = NSPoint::new(frame.origin.x, top - height);
        if let Some(screen) = self.panel.screen() {
            origin.y = origin.y.max(screen.visibleFrame().origin.y);
        }
        let target = NSRect::new(origin, NSSize::new(WIDTH, height));
        let motion = self.view.ivars().state.borrow().motion;
        self.panel
            .setFrame_display_animate(target, self.panel.isVisible(), animate && motion);
    }

    /// Redraws about sixty times a second while something moves.
    fn animate(&self) {
        if self.timer.borrow().is_some() {
            return;
        }
        let (view, slot) = (self.view.clone(), Rc::clone(&self.timer));
        let timer = app::every(1.0 / 60.0, move || {
            view.setNeedsDisplay(true);
            if !view.animating() {
                slot.borrow_mut().take();
            }
        });
        *self.timer.borrow_mut() = Some(timer);
    }

    /// Presses the copy action, as Return does. For tests.
    pub fn copy(&self) {
        self.view.copy();
    }
}
