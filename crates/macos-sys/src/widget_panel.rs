//! A free-floating Liquid Glass panel with a web page in it, for widgets that live on the desktop: it can be
//! dragged by the page, put behind the windows or above everything, and told to keep to its own size.
//!
//! A screen has no room for an app on the lock screen, so this is the nearest thing: a panel on every Space,
//! also over full-screen apps when asked to.

use crate::{
    panel::FigPanel,
    webview::WebHost,
};
use block2::RcBlock;
use objc2::{
    MainThreadMarker,
    rc::{Retained, Weak},
    runtime::ProtocolObject,
};
use objc2_app_kit::{
    NSApplication, NSColor, NSEventType, NSGlassEffectView, NSGlassEffectViewStyle, NSScreen,
    NSWindowCollectionBehavior, NSWindowDidEndLiveResizeNotification, NSWindowDidMoveNotification,
};
use objc2_foundation::{
    NSNotification, NSNotificationCenter, NSObjectProtocol, NSPoint, NSRect, NSSize,
};
use std::{cell::RefCell, ptr::NonNull, rc::Rc};

/// How far above the rest of the screen the panel is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Layer {
    /// Just above the desktop and its icons, under every window, like a widget stuck to the wallpaper.
    Desktop,
    /// Above the windows of other apps.
    Top,
    /// Above everything, full-screen apps included.
    Always,
}

impl Layer {
    pub fn named(word: &str) -> Self {
        match word {
            "desktop" => Self::Desktop,
            "always" => Self::Always,
            _ => Self::Top,
        }
    }

    fn level(self) -> isize {
        match self {
            Self::Desktop => -1,
            Self::Top => 3,
            Self::Always => 1000,
        }
    }
}

/// What the glass behind the page looks like.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Glass {
    /// Clear glass lets more of the desktop through than regular glass.
    pub clear: bool,
    /// The tint laid over the glass: red, green, blue and how strong.
    pub tint: (f64, f64, f64, f64),
}

/// Where a panel is and how large, in points from the bottom left of the screens, as the system counts.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Place {
    pub x: f64,
    pub y: f64,
    pub w: f64,
    pub h: f64,
}

impl From<NSRect> for Place {
    fn from(r: NSRect) -> Self {
        Self {
            x: r.origin.x,
            y: r.origin.y,
            w: r.size.width,
            h: r.size.height,
        }
    }
}

impl From<Place> for NSRect {
    fn from(p: Place) -> Self {
        NSRect::new(NSPoint::new(p.x, p.y), NSSize::new(p.w, p.h))
    }
}

pub struct WidgetPanel {
    pub host: WebHost,
    panel: Retained<FigPanel>,
    glass: Retained<NSGlassEffectView>,
    radius: RefCell<f64>,
    observers: RefCell<Vec<Retained<ProtocolObject<dyn NSObjectProtocol>>>>,
}

impl WidgetPanel {
    /// `on_message` hears the page; `on_frame` hears every move and every end of a resize, with the frame the
    /// panel has then.
    pub fn new(
        mtm: MainThreadMarker,
        place: Place,
        html: &str,
        on_message: impl Fn(String) + 'static,
        on_frame: impl Fn(Place) + 'static,
    ) -> Rc<Self> {
        let frame = NSRect::from(place);
        let panel = FigPanel::new(mtm, frame);
        panel.setHasShadow(true);
        panel.setMovableByWindowBackground(false);
        // Weak: the web view's controller keeps this callback, and the panel owns the web view.
        let drag_target = Weak::from_retained(&panel);
        let host = WebHost::new(
            mtm,
            frame,
            html,
            on_message,
            move |_| {
                if let (Some(panel), Some(event)) = (
                    drag_target.load(),
                    NSApplication::sharedApplication(mtm).currentEvent(),
                ) && event.r#type() == NSEventType::LeftMouseDown
                {
                    panel.performWindowDragWithEvent(&event);
                }
            },
            |_| {},
        );
        let glass = NSGlassEffectView::initWithFrame(mtm.alloc(), NSRect::new(NSPoint::ZERO, frame.size));
        glass.setStyle(NSGlassEffectViewStyle::Regular);
        glass.setContentView(Some(&host.view));
        panel.setContentView(Some(&glass));
        host.view.setFrame(glass.bounds());

        let this = Rc::new(Self {
            host,
            panel,
            glass,
            radius: RefCell::new(0.0),
            observers: RefCell::default(),
        });
        let on_frame = Rc::new(on_frame);
        let center = NSNotificationCenter::defaultCenter();
        for name in [
            // SAFETY: extern statics are process-lifetime constants.
            unsafe { NSWindowDidMoveNotification },
            unsafe { NSWindowDidEndLiveResizeNotification },
        ] {
            let (target, on_frame) = (Weak::from_retained(&this.panel), Rc::clone(&on_frame));
            let block = RcBlock::new(move |_: NonNull<NSNotification>| {
                if let Some(panel) = target.load() {
                    on_frame(panel.frame().into());
                }
            });
            // SAFETY: the block is 'static and runs on the main thread, where the notification is posted.
            let observer = unsafe {
                center.addObserverForName_object_queue_usingBlock(
                    Some(name),
                    Some(&*this.panel),
                    None,
                    &block,
                )
            };
            this.observers.borrow_mut().push(observer);
        }
        this
    }

    pub fn place(&self) -> Place {
        self.panel.frame().into()
    }

    pub fn set_place(&self, place: Place) {
        self.panel.setFrame_display(place.into(), true);
        self.host.view.setFrame(self.glass.bounds());
    }

    /// The glass: its look, its tint and the radius of its corners.
    pub fn set_glass(&self, glass: Glass, radius: f64) {
        self.glass.setStyle(if glass.clear {
            NSGlassEffectViewStyle::Clear
        } else {
            NSGlassEffectViewStyle::Regular
        });
        let (r, g, b, a) = glass.tint;
        self.glass
            .setTintColor(Some(&NSColor::colorWithSRGBRed_green_blue_alpha(r, g, b, a)));
        self.glass.setCornerRadius(radius);
        *self.radius.borrow_mut() = radius;
    }

    pub fn set_layer(&self, layer: Layer) {
        self.panel.setLevel(layer.level());
        let stay = match layer {
            Layer::Desktop => NSWindowCollectionBehavior::Stationary,
            _ => NSWindowCollectionBehavior::FullScreenAuxiliary,
        };
        self.panel.setCollectionBehavior(
            NSWindowCollectionBehavior::CanJoinAllSpaces
                | NSWindowCollectionBehavior::IgnoresCycle
                | stay,
        );
    }

    /// How see-through the whole panel is, 0.3 to 1.
    pub fn set_opacity(&self, opacity: f64) {
        self.panel.setAlphaValue(opacity.clamp(0.3, 1.0));
    }

    pub fn show(&self) {
        self.panel.orderFrontRegardless();
    }

    pub fn hide(&self) {
        self.panel.orderOut(None);
    }

    pub fn is_visible(&self) -> bool {
        self.panel.isVisible()
    }

    /// The area of the main screen that windows may use.
    pub fn work_area(mtm: MainThreadMarker) -> Place {
        NSScreen::mainScreen(mtm)
            .map_or(Place { x: 0.0, y: 0.0, w: 1440.0, h: 900.0 }, |s| s.visibleFrame().into())
    }

    /// Whether most of a panel of this place is on some screen, so a place kept from a screen that is gone is
    /// not used.
    pub fn on_a_screen(mtm: MainThreadMarker, place: Place) -> bool {
        let centre = NSPoint::new(place.x + place.w / 2.0, place.y + place.h / 2.0);
        NSScreen::screens(mtm).iter().any(|s| {
            let f = s.frame();
            centre.x >= f.origin.x
                && centre.x <= f.origin.x + f.size.width
                && centre.y >= f.origin.y
                && centre.y <= f.origin.y + f.size.height
        })
    }
}

impl Drop for WidgetPanel {
    fn drop(&mut self) {
        let center = NSNotificationCenter::defaultCenter();
        for observer in self.observers.borrow_mut().drain(..) {
            // SAFETY: each observer came from `addObserverForName`.
            unsafe { center.removeObserver(observer.as_ref()) };
        }
        self.panel.orderOut(None);
        self.panel.close();
    }
}
