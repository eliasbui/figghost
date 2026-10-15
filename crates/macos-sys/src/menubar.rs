//! The two status items that hide other apps' menu bar icons: an arrow and a divider to its left. Making the
//! divider very long pushes every icon to its left out of view; making it short again gives them back. The person
//! chooses which icons hide by dragging them (Command-drag) to the left of the divider. Dropping this value takes
//! both items off the menu bar, which gives every icon back at once, and so does the process ending.
//!
//! The system's side of this changes between versions, so what is decided lives in the app (`features/menubar`);
//! this file only creates the items, sets a length and reports where the two windows are.

use crate::style;
use block2::RcBlock;
use objc2::{
    AnyThread, DefinedClass, MainThreadMarker, MainThreadOnly, define_class, msg_send,
    rc::Retained,
    runtime::{AnyObject, ProtocolObject},
    sel,
};
use objc2_app_kit::{
    NSAccessibility, NSApplicationDidChangeScreenParametersNotification, NSScreen, NSStatusBar,
    NSStatusItem, NSTrackingArea, NSTrackingAreaOptions, NSVariableStatusItemLength,
};
use objc2_foundation::{
    NSNotification, NSNotificationCenter, NSObject, NSObjectProtocol, NSRect, NSString,
};
use std::ptr::NonNull;

/// Names under which the system remembers where the person put each item, so that the arrangement survives a
/// restart. The system keeps them in this app's own preferences; nothing else reads them.
const ARROW_NAME: &str = "figghost.menubar.arrow";
const DIVIDER_NAME: &str = "figghost.menubar.divider";

/// The words the items carry for assistive technology and as tooltips.
pub struct Labels {
    pub show: String,
    pub hide: String,
}

pub struct TargetIvars {
    on_click: Box<dyn Fn()>,
    on_hover: Box<dyn Fn()>,
}

define_class!(
    // SAFETY: NSObject allows subclassing; the ivars are only touched on the main thread and need no Drop.
    #[unsafe(super(NSObject))]
    #[thread_kind = MainThreadOnly]
    #[name = "FigHiderTarget"]
    #[ivars = TargetIvars]
    struct HiderTarget;

    unsafe impl NSObjectProtocol for HiderTarget {}

    impl HiderTarget {
        #[unsafe(method(clicked:))]
        fn clicked(&self, _sender: Option<&AnyObject>) {
            (self.ivars().on_click)();
        }

        #[unsafe(method(mouseEntered:))]
        fn mouse_entered(&self, _event: Option<&AnyObject>) {
            (self.ivars().on_hover)();
        }
    }
);

impl HiderTarget {
    fn new(
        mtm: MainThreadMarker,
        on_click: impl Fn() + 'static,
        on_hover: impl Fn() + 'static,
    ) -> Retained<Self> {
        let this = mtm.alloc::<Self>().set_ivars(TargetIvars {
            on_click: Box::new(on_click),
            on_hover: Box::new(on_hover),
        });
        // SAFETY: NSObject's designated initializer.
        unsafe { msg_send![super(this), init] }
    }
}

type Observer = Retained<ProtocolObject<dyn NSObjectProtocol>>;

pub struct Hider {
    arrow: Retained<NSStatusItem>,
    divider: Retained<NSStatusItem>,
    labels: Labels,
    _target: Retained<HiderTarget>,
    _area: Retained<NSTrackingArea>,
    screens: Observer,
}

impl Hider {
    /// Adds the arrow, then the divider: the system places a newer item to the left of an older one, which is the
    /// order wanted. `on_click` runs when the arrow is clicked, `on_hover` when the pointer rests on it, and
    /// `on_screens` when the screens change.
    pub fn new(
        mtm: MainThreadMarker,
        labels: Labels,
        divider_length: f64,
        on_click: impl Fn() + 'static,
        on_hover: impl Fn() + 'static,
        on_screens: impl Fn() + 'static,
    ) -> Self {
        let bar = NSStatusBar::systemStatusBar();
        let arrow = bar.statusItemWithLength(NSVariableStatusItemLength);
        let divider = bar.statusItemWithLength(divider_length);
        arrow.setAutosaveName(Some(&NSString::from_str(ARROW_NAME)));
        divider.setAutosaveName(Some(&NSString::from_str(DIVIDER_NAME)));

        let target = HiderTarget::new(mtm, on_click, on_hover);
        let button = arrow
            .button(mtm)
            .expect("a status item has a button on this system");
        // SAFETY: `clicked:` is implemented by HiderTarget; the target is kept alive by `Hider`.
        unsafe {
            button.setTarget(Some(&target));
            button.setAction(Some(sel!(clicked:)));
        }
        // SAFETY: valid options and owner; InVisibleRect ignores the (empty) rect.
        let area = unsafe {
            NSTrackingArea::initWithRect_options_owner_userInfo(
                NSTrackingArea::alloc(),
                NSRect::ZERO,
                NSTrackingAreaOptions::MouseEnteredAndExited
                    | NSTrackingAreaOptions::ActiveAlways
                    | NSTrackingAreaOptions::InVisibleRect,
                Some(&target),
                None,
            )
        };
        button.addTrackingArea(&area);

        let block = RcBlock::new(move |_: NonNull<NSNotification>| on_screens());
        // SAFETY: the block is 'static and runs on the main thread, where the notification is posted.
        let screens = unsafe {
            NSNotificationCenter::defaultCenter().addObserverForName_object_queue_usingBlock(
                Some(NSApplicationDidChangeScreenParametersNotification),
                None,
                None,
                &block,
            )
        };
        let this = Self {
            arrow,
            divider,
            labels,
            _target: target,
            _area: area,
            screens,
        };
        this.look(mtm, false);
        this
    }

    /// Sets the length of the divider.
    pub fn set_length(&self, points: f64) {
        self.divider.setLength(points);
    }

    /// The arrow points at what a click will do; the divider is drawn only while the icons show, since a
    /// collapsed divider is long and its mark would sit in the middle of what it hides.
    pub fn look(&self, mtm: MainThreadMarker, collapsed: bool) {
        let label = if collapsed {
            &self.labels.show
        } else {
            &self.labels.hide
        };
        if let Some(button) = self.arrow.button(mtm) {
            let symbol = if collapsed {
                "chevron.left"
            } else {
                "chevron.right"
            };
            let image = style::symbol(symbol, 12.0, &style::white(1.0));
            // A template image is tinted by the menu bar itself, light or dark.
            if let Some(image) = image {
                image.setTemplate(true);
                button.setImage(Some(&image));
            }
            button.setToolTip(Some(&NSString::from_str(label)));
            button.setAccessibilityLabel(Some(&NSString::from_str(label)));
        }
        if let Some(button) = self.divider.button(mtm) {
            button.setTitle(&NSString::from_str(if collapsed { "" } else { "|" }));
        }
    }

    /// The distance from the right edge of the divider's window to the left edge of the arrow's, or `None` when a
    /// window is not available. A divider the system lays out keeps this small whatever its length; one it leaves
    /// out of the layout jumps away by about its length.
    pub fn gap(&self, mtm: MainThreadMarker) -> Option<f64> {
        let divider = self.divider.button(mtm)?.window()?.frame();
        let arrow = self.arrow.button(mtm)?.window()?.frame();
        Some(arrow.origin.x - (divider.origin.x + divider.size.width))
    }
}

/// The width of the widest screen: every display shows the menu bar, so the widest asks the most of the divider.
pub fn widest_screen(mtm: MainThreadMarker) -> f64 {
    NSScreen::screens(mtm)
        .iter()
        .map(|s| s.frame().size.width)
        .fold(0.0, f64::max)
}

impl Drop for Hider {
    fn drop(&mut self) {
        // SAFETY: the token was returned by the default center's addObserverForName.
        unsafe {
            NSNotificationCenter::defaultCenter()
                .removeObserver(AsRef::<AnyObject>::as_ref(&*self.screens));
        }
        let bar = NSStatusBar::systemStatusBar();
        bar.removeStatusItem(&self.divider);
        bar.removeStatusItem(&self.arrow);
    }
}
