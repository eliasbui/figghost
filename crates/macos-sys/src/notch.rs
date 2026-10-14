//! The notch surface: a black shape that grows out of the notch cutout on hover and hosts the clipboard list.

use crate::{
    app, labels,
    panel::{FigPanel, Key},
    pasteboard::file_paths,
    ui::{AppCallbacks, Callbacks, ClipUi, ClipView, handle_key},
    widgets::ActionTarget,
};
use block2::RcBlock;
use objc2::{
    AnyThread, DefinedClass, MainThreadMarker, MainThreadOnly, define_class, msg_send,
    rc::Retained,
    runtime::{AnyObject, ProtocolObject},
    sel,
};
use objc2_app_kit::{
    NSApplicationDidChangeScreenParametersNotification, NSBezierPath, NSColor, NSDragOperation,
    NSDraggingDestination, NSDraggingInfo, NSEvent, NSEventMask, NSPasteboardTypeFileURL, NSScreen,
    NSSegmentSwitchTracking, NSSegmentedControl, NSStatusBar, NSTrackingArea,
    NSTrackingAreaOptions, NSView, NSWindowDidResignKeyNotification,
};
use objc2_foundation::{
    NSArray, NSNotification, NSNotificationCenter, NSObjectProtocol, NSPoint, NSRect, NSSize,
    NSString,
};
use std::{
    cell::{Cell, RefCell},
    ptr::NonNull,
    rc::Rc,
};

pub const EXPANDED_SIZE: NSSize = NSSize::new(500.0, 440.0);
/// Height reserved above the pages for the tab bar.
pub const TAB_BAR_HEIGHT: f64 = 34.0;
const TAB_WIDTH: f64 = 330.0;
const PILL_WIDTH: f64 = 200.0;
const HOT_ZONE_HEIGHT: f64 = 4.0;
const TRIGGER_MIN_HEIGHT: f64 = 8.0;
const TRIGGER_SIDE_SLACK: f64 = 10.0;
const CORNER_RADIUS: f64 = 20.0;
const HOVER_OPEN_DELAY: f64 = 0.12;
const HOVER_CLOSE_DELAY: f64 = 0.35;

/// Notch position in AppKit screen coordinates (origin bottom-left).
#[derive(Debug, Clone, Copy)]
pub struct NotchGeometry {
    pub screen: NSRect,
    /// Hardware notch size, or the pill size on displays without one.
    pub notch: NSSize,
    pub hardware: bool,
}

impl NotchGeometry {
    /// Prefers the notched built-in display; falls back to the menu-bar screen with a pill-sized notch.
    pub fn detect(mtm: MainThreadMarker) -> Option<Self> {
        let screens = NSScreen::screens(mtm);
        if let Some(s) = screens.iter().find(|s| s.safeAreaInsets().top > 0.0) {
            let (left, right) = (s.auxiliaryTopLeftArea(), s.auxiliaryTopRightArea());
            let width = right.origin.x - (left.origin.x + left.size.width);
            let notch = NSSize::new(width, s.safeAreaInsets().top);
            return Some(Self {
                screen: s.frame(),
                notch,
                hardware: true,
            });
        }
        let s = screens.firstObject()?;
        let notch = NSSize::new(PILL_WIDTH, NSStatusBar::systemStatusBar().thickness());
        Some(Self {
            screen: s.frame(),
            notch,
            hardware: false,
        })
    }

    /// A rect of the given size hugging the top edge, horizontally centered on the screen.
    pub fn frame(&self, width: f64, height: f64) -> NSRect {
        let (s, size) = (self.screen, NSSize::new(width, height));
        let origin = NSPoint::new(
            s.origin.x + (s.size.width - width) / 2.0,
            s.origin.y + s.size.height - height,
        );
        NSRect::new(origin, size)
    }

    /// The resting shape: exactly the notch cutout, or a thin hot zone on screens without one.
    pub fn collapsed_frame(&self) -> NSRect {
        if self.hardware {
            self.frame(self.notch.width, self.notch.height)
        } else {
            self.frame(PILL_WIDTH, HOT_ZONE_HEIGHT)
        }
    }

    /// Where the pointer must be to open the notch. Taller than the resting shape on screens without a
    /// notch, and checked against raw pointer positions: the topmost rows of the screen never deliver
    /// events to a window, so a tracking area alone would miss a pointer pushed against the edge.
    pub fn trigger_rect(&self) -> NSRect {
        let rest = self.collapsed_frame();
        let height = rest.size.height.max(TRIGGER_MIN_HEIGHT);
        self.frame(rest.size.width + 2.0 * TRIGGER_SIDE_SLACK, height)
    }

    pub fn expanded_frame(&self) -> NSRect {
        self.frame(EXPANDED_SIZE.width, EXPANDED_SIZE.height)
    }

    /// Room the content leaves free at the top so it clears the cutout.
    pub fn top_inset(&self) -> f64 {
        if self.hardware {
            self.notch.height + 6.0
        } else {
            10.0
        }
    }
}

type Signal = Rc<dyn Fn()>;
type DragSignal = Rc<dyn Fn(bool)>;

pub struct ShapeIvars {
    expanded: Cell<bool>,
    hardware: Cell<bool>,
    /// Called when the pointer enters or leaves the window.
    on_pointer: RefCell<Option<Signal>>,
    on_click: RefCell<Option<Signal>>,
    /// Called with true when a drag carrying files enters the shape and false when it leaves.
    on_drag: RefCell<Option<DragSignal>>,
}

define_class!(
    // SAFETY: NSView allows subclassing; the ivars are only touched on the main thread and need no Drop.
    #[unsafe(super(NSView))]
    #[thread_kind = MainThreadOnly]
    #[name = "FigNotchShape"]
    #[ivars = ShapeIvars]
    struct ShapeView;

    impl ShapeView {
        #[unsafe(method(acceptsFirstMouse:))]
        fn accepts_first_mouse(&self, _event: Option<&NSEvent>) -> bool {
            true
        }

        #[unsafe(method(drawRect:))]
        fn draw_rect(&self, _dirty: NSRect) {
            let ivars = self.ivars();
            // On screens without a notch the resting shape must stay invisible but still catch the pointer,
            // and fully transparent pixels would let it pass through.
            let alpha = if ivars.expanded.get() || ivars.hardware.get() { 1.0 } else { 0.01 };
            NSColor::colorWithWhite_alpha(0.0, alpha).setFill();
            let bounds = self.bounds();
            // Overshoot the top edge so only the bottom corners are rounded, like the hardware notch.
            let shape = NSRect::new(
                bounds.origin,
                NSSize::new(bounds.size.width, bounds.size.height + CORNER_RADIUS),
            );
            NSBezierPath::bezierPathWithRoundedRect_xRadius_yRadius(shape, CORNER_RADIUS, CORNER_RADIUS)
                .fill();
        }

        // Everything inside lives in one container laid out for the expanded size and pinned to the top
        // centre here. Autoresizing masks misplace views wider than a shrunken parent, and views that are
        // added while the shape is collapsed.
        #[unsafe(method(setFrameSize:))]
        fn set_frame_size(&self, size: NSSize) {
            // SAFETY: forwards the same selector and argument to the superclass.
            let _: () = unsafe { msg_send![super(self), setFrameSize: size] };
            if let Some(content) = self.subviews().firstObject() {
                content.setFrameOrigin(NSPoint::new(
                    (size.width - EXPANDED_SIZE.width) / 2.0,
                    size.height - EXPANDED_SIZE.height,
                ));
            }
        }

        #[unsafe(method(mouseEntered:))]
        fn mouse_entered(&self, _event: &NSEvent) {
            self.fire(&self.ivars().on_pointer);
        }

        #[unsafe(method(mouseExited:))]
        fn mouse_exited(&self, _event: &NSEvent) {
            self.fire(&self.ivars().on_pointer);
        }

        #[unsafe(method(mouseDown:))]
        fn mouse_down(&self, _event: &NSEvent) {
            if !self.ivars().expanded.get() {
                self.fire(&self.ivars().on_click);
            }
        }
    }

    unsafe impl NSObjectProtocol for ShapeView {}

    // The shape is a drag destination only to notice files arriving: it opens the notch on the shelf. The
    // shelf page itself accepts the drop, so this never claims the operation.
    unsafe impl NSDraggingDestination for ShapeView {
        #[unsafe(method(draggingEntered:))]
        fn dragging_entered(&self, sender: &ProtocolObject<dyn NSDraggingInfo>) -> NSDragOperation {
            if !file_paths(&sender.draggingPasteboard()).is_empty() {
                self.signal_drag(true);
            }
            NSDragOperation::None
        }

        #[unsafe(method(draggingExited:))]
        fn dragging_exited(&self, _sender: Option<&ProtocolObject<dyn NSDraggingInfo>>) {
            self.signal_drag(false);
        }
    }
);

impl ShapeView {
    fn new(mtm: MainThreadMarker, frame: NSRect, hardware: bool) -> Retained<Self> {
        let ivars = ShapeIvars {
            expanded: Cell::new(false),
            hardware: Cell::new(hardware),
            on_pointer: RefCell::new(None),
            on_click: RefCell::new(None),
            on_drag: RefCell::new(None),
        };
        // SAFETY: designated initializer with valid arguments on the main thread.
        let this: Retained<Self> =
            unsafe { msg_send![super(mtm.alloc::<Self>().set_ivars(ivars)), initWithFrame: frame] };
        // SAFETY: extern statics are process-lifetime constants.
        this.registerForDraggedTypes(&NSArray::from_slice(&[unsafe { NSPasteboardTypeFileURL }]));
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

    fn signal_drag(&self, entered: bool) {
        let signal = self.ivars().on_drag.borrow().clone();
        if let Some(signal) = signal {
            signal(entered);
        }
    }

    fn fire(&self, which: &RefCell<Option<Signal>>) {
        let signal = which.borrow().clone();
        if let Some(signal) = signal {
            signal();
        }
    }

    fn set_expanded(&self, expanded: bool) {
        self.ivars().expanded.set(expanded);
        self.setNeedsDisplay(true);
    }

    fn set_hardware(&self, hardware: bool) {
        self.ivars().hardware.set(hardware);
        self.setNeedsDisplay(true);
    }
}

/// One tab of the expanded notch.
pub struct Page {
    pub title: String,
    /// Fills `Notch::page_frame()`.
    pub root: Retained<NSView>,
    /// Runs whenever the page becomes visible, to refresh what it shows.
    pub on_show: Box<dyn Fn()>,
    /// Runs when the page stops being visible: another tab was chosen, or the notch closed.
    pub on_hide: Box<dyn Fn()>,
    /// Returns true when the key was consumed.
    pub on_key: Box<dyn Fn(Key) -> bool>,
    /// The notch switches to this page when files are dragged onto it.
    pub accepts_files: bool,
}

struct Inner {
    panel: Retained<FigPanel>,
    shape: Retained<ShapeView>,
    /// Holds the tab bar and every page at their expanded-size positions.
    content: Retained<NSView>,
    view: ClipView,
    tabs: Retained<NSSegmentedControl>,
    pages: RefCell<Vec<Page>>,
    active: Cell<usize>,
    /// Set while files are being dragged over the notch, to open on the page that takes them.
    files_hint: Cell<bool>,
    geometry: Cell<NotchGeometry>,
    inside: Cell<bool>,
    expanded: Cell<bool>,
    /// Off removes the panel from the screen and ignores the pointer.
    enabled: Cell<bool>,
}

impl Inner {
    fn files_page(&self) -> Option<usize> {
        self.pages.borrow().iter().position(|p| p.accepts_files)
    }

    fn select_page(&self, index: usize) {
        {
            let pages = self.pages.borrow();
            let Some(page) = pages.get(index) else {
                return;
            };
            let previous = self.active.get();
            if previous != index
                && let Some(old) = pages.get(previous)
            {
                (old.on_hide)();
            }
            for (i, p) in pages.iter().enumerate() {
                p.root.setHidden(i != index);
            }
            self.active.set(index);
            self.tabs.setSelectedSegment(index as isize);
            (page.on_show)();
        }
        self.settle_focus();
    }

    /// The search field only has focus on the clipboard page; other pages work from the keys the panel forwards.
    fn settle_focus(&self) {
        if self.panel.isKeyWindow() {
            if self.active.get() == 0 {
                self.panel.makeFirstResponder(Some(&self.view.search));
            } else {
                self.panel.makeFirstResponder(None);
            }
        }
    }

    fn expand(&self, focus: bool) {
        if !self.enabled.get() {
            return;
        }
        let first = !self.expanded.replace(true);
        if first {
            self.shape.set_expanded(true);
            self.content.setHidden(false);
            self.tabs.setHidden(false);
            self.panel
                .setFrame_display_animate(self.geometry.get().expanded_frame(), true, true);
        }
        let page = match self.files_page() {
            Some(files) if self.files_hint.replace(false) => files,
            _ => self.active.get(),
        };
        if first || page != self.active.get() {
            self.select_page(page);
        }
        if focus {
            self.panel.makeKeyAndOrderFront(None);
            self.settle_focus();
        }
    }

    fn collapse(&self) {
        if !self.expanded.replace(false) {
            return;
        }
        self.view.clear_query();
        {
            let pages = self.pages.borrow();
            if let Some(shown) = pages.get(self.active.get()) {
                (shown.on_hide)();
            }
            for page in pages.iter() {
                page.root.setHidden(true);
            }
        }
        self.active.set(0);
        self.tabs.setHidden(true);
        self.content.setHidden(true);
        self.shape.set_expanded(false);
        // A key window cannot be told to resign directly: take it off screen and bring it back unfocused.
        self.panel.orderOut(None);
        self.panel
            .setFrame_display(self.geometry.get().collapsed_frame(), true);
        self.panel.orderFrontRegardless();
    }

    fn relayout(&self, geometry: NotchGeometry) {
        self.geometry.set(geometry);
        self.shape.set_hardware(geometry.hardware);
        let frame = if self.expanded.get() {
            geometry.expanded_frame()
        } else {
            geometry.collapsed_frame()
        };
        self.panel.setFrame_display(frame, true);
    }

    fn handle_key(&self, key: Key) -> bool {
        if let Key::Tab(n) = key {
            let index = usize::from(n).saturating_sub(1);
            if index < self.pages.borrow().len() {
                self.select_page(index);
            }
            return true;
        }
        let pages = self.pages.borrow();
        pages
            .get(self.active.get())
            .is_some_and(|page| (page.on_key)(key))
    }
}

/// Edges are inclusive: a pointer pushed against the top of the screen sits exactly on the boundary.
fn contains(rect: NSRect, p: NSPoint) -> bool {
    p.x >= rect.origin.x
        && p.x <= rect.origin.x + rect.size.width
        && p.y >= rect.origin.y
        && p.y <= rect.origin.y + rect.size.height
}

/// Hover logic: opens shortly after the pointer enters the trigger area, closes shortly after it leaves
/// the open shape unless the search field has focus.
fn pointer_moved(inner: &Rc<Inner>, p: NSPoint) {
    if !inner.enabled.get() {
        return;
    }
    let geometry = inner.geometry.get();
    let area = if inner.expanded.get() {
        geometry.expanded_frame()
    } else {
        geometry.trigger_rect()
    };
    let inside = contains(area, p);
    if inner.inside.replace(inside) == inside {
        return;
    }
    let inner = Rc::clone(inner);
    if inside {
        app::after(HOVER_OPEN_DELAY, move || {
            if inner.inside.get() && !inner.expanded.get() {
                inner.expand(false);
            }
        });
    } else {
        inner.files_hint.set(false);
        app::after(HOVER_CLOSE_DELAY, move || {
            // A focused search field means the user is typing: leave it open.
            if !inner.inside.get() && inner.expanded.get() && !inner.panel.isKeyWindow() {
                inner.collapse();
            }
        });
    }
}

type Observer = Retained<ProtocolObject<dyn NSObjectProtocol>>;

pub struct Notch {
    inner: Rc<Inner>,
    ui: ClipUi,
    _tab_target: Retained<ActionTarget>,
    observers: Vec<Observer>,
    /// Global pointer monitor; sees movement over other apps' windows, which tracking areas cannot.
    monitor: Option<Retained<AnyObject>>,
}

impl Drop for Notch {
    fn drop(&mut self) {
        let center = NSNotificationCenter::defaultCenter();
        for observer in &self.observers {
            // SAFETY: each token was returned by the default center's addObserverForName.
            unsafe { center.removeObserver(AsRef::<AnyObject>::as_ref(&**observer)) };
        }
        if let Some(monitor) = &self.monitor {
            // SAFETY: the token came from addGlobalMonitorForEventsMatchingMask and is removed once.
            unsafe { NSEvent::removeMonitor(monitor) };
        }
    }
}

impl Notch {
    /// The notch starts with the clipboard page; `on_expand` refreshes its rows each time it is shown.
    pub fn new(
        mtm: MainThreadMarker,
        geometry: NotchGeometry,
        app: AppCallbacks,
        on_expand: impl Fn(&ClipView) + 'static,
    ) -> Self {
        let slot: Rc<RefCell<Option<Rc<Inner>>>> = Rc::default();
        let collapse = {
            let slot = Rc::clone(&slot);
            move || {
                let inner = slot.borrow().clone();
                if let Some(inner) = inner {
                    inner.collapse();
                }
            }
        };
        let on_activate = app.on_activate;
        let close_after_activate = collapse.clone();
        let callbacks = Callbacks {
            on_query: app.on_query,
            on_activate: Box::new(move |view, id, mode| {
                close_after_activate();
                on_activate(view, id, mode);
            }),
            on_pin: app.on_pin,
            on_delete: app.on_delete,
            on_close: Box::new(move |_| collapse()),
        };

        let content_top = geometry.top_inset() + TAB_BAR_HEIGHT;
        let ui = ClipUi::new(mtm, EXPANDED_SIZE, content_top, callbacks);
        let panel = FigPanel::new(mtm, geometry.expanded_frame());
        let shape = ShapeView::new(
            mtm,
            NSRect::new(NSPoint::ZERO, EXPANDED_SIZE),
            geometry.hardware,
        );
        panel.setContentView(Some(&shape));
        // Fixed-size content: the shape grows around it instead of stretching it (see `set_frame_size`).
        let content = NSView::initWithFrame(mtm.alloc(), NSRect::new(NSPoint::ZERO, EXPANDED_SIZE));
        // Hidden while collapsed, so clicks on the resting shape reach the shape itself.
        content.setHidden(true);
        shape.addSubview(&content);
        content.addSubview(&ui.view.root);

        let tabs = NSSegmentedControl::initWithFrame(
            mtm.alloc(),
            NSRect::new(
                NSPoint::new(
                    (EXPANDED_SIZE.width - TAB_WIDTH) / 2.0,
                    EXPANDED_SIZE.height - geometry.top_inset() - 6.0 - 24.0,
                ),
                NSSize::new(TAB_WIDTH, 24.0),
            ),
        );
        tabs.setTrackingMode(NSSegmentSwitchTracking::SelectOne);
        content.addSubview(&tabs);

        panel.setFrame_display(geometry.collapsed_frame(), false);
        ui.view.root.setHidden(true);
        tabs.setHidden(true);

        let inner = Rc::new(Inner {
            panel: Retained::clone(&panel),
            shape: Retained::clone(&shape),
            content,
            view: ui.view.clone(),
            tabs: Retained::clone(&tabs),
            pages: RefCell::new(Vec::new()),
            active: Cell::new(0),
            files_hint: Cell::new(false),
            geometry: Cell::new(geometry),
            inside: Cell::new(false),
            expanded: Cell::new(false),
            enabled: Cell::new(true),
        });
        *slot.borrow_mut() = Some(Rc::clone(&inner));

        let clipboard_page = {
            let (view, callbacks) = (ui.view.clone(), Rc::clone(&ui.callbacks));
            let key_view = view.clone();
            Page {
                title: labels::get().clipboard_tab.clone(),
                root: Retained::into_super(Retained::clone(&ui.view.root)),
                on_show: Box::new(move || on_expand(&view)),
                on_hide: Box::new(|| {}),
                on_key: Box::new(move |key| handle_key(&key_view, &callbacks, key)),
                accepts_files: false,
            }
        };
        inner.pages.borrow_mut().push(clipboard_page);

        let tab_target = {
            let slot = Rc::clone(&slot);
            ActionTarget::new(mtm, move || {
                let inner = slot.borrow().clone();
                if let Some(inner) = inner {
                    inner.select_page(inner.tabs.selectedSegment().max(0) as usize);
                }
            })
        };
        // SAFETY: `fire:` takes the sender as its only argument; the target is kept alive by the Notch.
        unsafe {
            tabs.setTarget(Some(&tab_target));
            tabs.setAction(Some(sel!(fire:)));
        }
        let mut this = Self {
            inner: Rc::clone(&inner),
            ui,
            _tab_target: tab_target,
            observers: Vec::new(),
            monitor: None,
        };
        this.refresh_tabs();

        {
            let key_inner = Rc::clone(&inner);
            panel.set_key_handler(move |key| key_inner.handle_key(key));
            let on_pointer = Rc::clone(&inner);
            *shape.ivars().on_pointer.borrow_mut() = Some(Rc::new(move || {
                pointer_moved(&on_pointer, NSEvent::mouseLocation());
            }));
            let on_drag = Rc::clone(&inner);
            *shape.ivars().on_drag.borrow_mut() = Some(Rc::new(move |entered| {
                if entered {
                    // Files are arriving: open straight onto the page that takes them.
                    on_drag.files_hint.set(true);
                    on_drag.inside.set(true);
                    on_drag.expand(false);
                } else {
                    // Leaving the shape for a page inside it, or for good: ask where the pointer is now.
                    pointer_moved(&on_drag, NSEvent::mouseLocation());
                }
            }));
            let on_click = Rc::clone(&inner);
            *shape.ivars().on_click.borrow_mut() = Some(Rc::new(move || on_click.expand(true)));
        }

        let center = NSNotificationCenter::defaultCenter();
        let window: &AnyObject = &panel;
        let resigned = Rc::clone(&inner);
        let on_resign = RcBlock::new(move |_: NonNull<NSNotification>| {
            if !resigned.inside.get() {
                resigned.collapse();
            }
        });
        let relaid = Rc::clone(&inner);
        let on_screens = RcBlock::new(move |_: NonNull<NSNotification>| {
            if let Some(geometry) = NotchGeometry::detect(mtm) {
                relaid.relayout(geometry);
            }
        });
        // SAFETY: the blocks are 'static and run on the main thread, where both notifications are posted.
        let observers = unsafe {
            vec![
                center.addObserverForName_object_queue_usingBlock(
                    Some(NSWindowDidResignKeyNotification),
                    Some(window),
                    None,
                    &on_resign,
                ),
                center.addObserverForName_object_queue_usingBlock(
                    Some(NSApplicationDidChangeScreenParametersNotification),
                    None,
                    None,
                    &on_screens,
                ),
            ]
        };

        let moved = Rc::clone(&inner);
        let on_move = RcBlock::new(move |_: NonNull<NSEvent>| {
            pointer_moved(&moved, NSEvent::mouseLocation());
        });
        let monitor = NSEvent::addGlobalMonitorForEventsMatchingMask_handler(
            NSEventMask::MouseMoved,
            &on_move,
        );

        panel.orderFrontRegardless();
        this.observers = observers;
        this.monitor = monitor;
        this
    }

    pub fn view(&self) -> &ClipView {
        &self.ui.view
    }

    /// Area every page must fill: the shape below the tab bar.
    pub fn page_frame(&self) -> NSRect {
        let top = self.inner.geometry.get().top_inset() + TAB_BAR_HEIGHT;
        NSRect::new(
            NSPoint::ZERO,
            NSSize::new(EXPANDED_SIZE.width, EXPANDED_SIZE.height - top),
        )
    }

    /// Adds a tab after the existing ones; Command+its number selects it.
    pub fn add_page(&self, page: Page) {
        page.root.setHidden(true);
        self.inner.content.addSubview(&page.root);
        self.inner.pages.borrow_mut().push(page);
        self.refresh_tabs();
    }

    fn refresh_tabs(&self) {
        let pages = self.inner.pages.borrow();
        let tabs = &self.inner.tabs;
        tabs.setSegmentCount(pages.len() as isize);
        for (i, page) in pages.iter().enumerate() {
            tabs.setLabel_forSegment(&NSString::from_str(&page.title), i as isize);
            tabs.setWidth_forSegment(TAB_WIDTH / pages.len() as f64, i as isize);
        }
        tabs.setSelectedSegment(self.inner.active.get() as isize);
    }

    pub fn is_expanded(&self) -> bool {
        self.inner.expanded.get()
    }

    /// Shows or removes the notch panel. While it is off, hovering the top edge does nothing.
    pub fn set_enabled(&self, enabled: bool) {
        if self.inner.enabled.replace(enabled) == enabled {
            return;
        }
        if enabled {
            self.inner.panel.orderFrontRegardless();
        } else {
            self.inner.collapse();
            self.inner.panel.orderOut(None);
        }
    }

    /// Opens without taking keyboard focus, as hovering does.
    pub fn open(&self) {
        self.inner.expand(false);
    }

    /// Opens and focuses the search field, as a click on the notch does.
    pub fn open_focused(&self) {
        self.inner.expand(true);
    }

    /// Opens on the given page (0 is the clipboard).
    pub fn open_page(&self, index: usize) {
        self.inner.expand(false);
        self.inner.select_page(index);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn geometry(hardware: bool) -> NotchGeometry {
        NotchGeometry {
            screen: NSRect::new(NSPoint::new(0.0, 0.0), NSSize::new(1512.0, 982.0)),
            notch: NSSize::new(200.0, 32.0),
            hardware,
        }
    }

    #[test]
    fn frame_hugs_top_edge_and_is_centered() {
        let f = geometry(true).frame(200.0, 32.0);
        assert_eq!((f.origin.x, f.origin.y), (656.0, 950.0));
        assert_eq!(f.origin.x, 1512.0 - (f.origin.x + f.size.width));
        assert_eq!(f.origin.y + f.size.height, 982.0);
    }

    #[test]
    fn collapsed_shape_is_the_cutout_or_a_thin_hot_zone() {
        let hardware = geometry(true).collapsed_frame();
        assert_eq!((hardware.size.width, hardware.size.height), (200.0, 32.0));
        let plain = geometry(false).collapsed_frame();
        assert_eq!(
            (plain.size.width, plain.size.height),
            (PILL_WIDTH, HOT_ZONE_HEIGHT)
        );
        // Both stay glued to the top edge so the pointer reaches them by moving up.
        for f in [hardware, plain] {
            assert_eq!(f.origin.y + f.size.height, 982.0);
        }
    }

    #[test]
    fn trigger_area_is_taller_than_the_invisible_resting_shape() {
        let plain = geometry(false);
        let trigger = plain.trigger_rect();
        assert!(trigger.size.height >= TRIGGER_MIN_HEIGHT);
        assert!(trigger.size.width > plain.collapsed_frame().size.width);
        // The cutout is already tall enough on a real notch.
        let hardware = geometry(true);
        assert_eq!(
            hardware.trigger_rect().size.height,
            hardware.collapsed_frame().size.height
        );
    }

    #[test]
    fn pointer_pushed_against_the_top_edge_counts_as_inside() {
        let g = geometry(false);
        let trigger = g.trigger_rect();
        let top = g.screen.origin.y + g.screen.size.height;
        let centre = g.screen.size.width / 2.0;
        assert!(
            contains(trigger, NSPoint::new(centre, top)),
            "exact top edge"
        );
        assert!(contains(trigger, NSPoint::new(centre, top - 1.0)));
        assert!(!contains(
            trigger,
            NSPoint::new(centre, top - TRIGGER_MIN_HEIGHT - 1.0)
        ));
        assert!(!contains(trigger, NSPoint::new(centre + 200.0, top)));
    }

    #[test]
    fn expanded_shape_grows_down_from_the_same_top_edge() {
        let g = geometry(true);
        let (collapsed, expanded) = (g.collapsed_frame(), g.expanded_frame());
        assert_eq!(
            expanded.origin.y + expanded.size.height,
            collapsed.origin.y + collapsed.size.height
        );
        assert_eq!(
            expanded.origin.x + expanded.size.width / 2.0,
            collapsed.origin.x + collapsed.size.width / 2.0
        );
        assert!(
            g.top_inset() > g.notch.height,
            "content must clear the cutout"
        );
    }
}
