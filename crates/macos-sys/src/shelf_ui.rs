//! The shelf page: a grid of file tiles. Files dragged in are reported to the app, tiles can be dragged
//! out to other apps, double-clicked to open, and removed with the keyboard.

use crate::{
    labels,
    notch::Page,
    panel::Key,
    pasteboard::file_paths,
    style,
    widgets::{ActionTarget, Fit, PageRoot, draw_text, text_attributes},
};
use objc2::{
    AnyThread, DefinedClass, MainThreadMarker, MainThreadOnly, define_class, msg_send,
    rc::Retained,
    runtime::{AnyObject, ProtocolObject},
    sel,
};
use objc2_app_kit::{
    NSAutoresizingMaskOptions, NSBezierPath, NSBorderType, NSColor, NSCompositingOperation,
    NSDragOperation, NSDraggingContext, NSDraggingDestination, NSDraggingInfo, NSDraggingItem,
    NSDraggingSession, NSDraggingSource, NSEvent, NSEventModifierFlags, NSFont, NSImage, NSMenu,
    NSMenuItem, NSPasteboardTypeFileURL, NSPasteboardWriting, NSScrollView, NSScrollerStyle,
    NSView, NSWorkspace,
};
use objc2_foundation::{NSArray, NSObjectProtocol, NSPoint, NSRect, NSSize, NSString, NSURL};
use std::{
    cell::{Cell, RefCell},
    collections::HashMap,
    path::Path,
    rc::Rc,
};

const TILE_WIDTH: f64 = 96.0;
const TILE_HEIGHT: f64 = 92.0;
const PAD: f64 = 10.0;
const ICON: f64 = 48.0;
const FOOTER_HEIGHT: f64 = 24.0;
const DRAG_THRESHOLD: f64 = 4.0;
const ICON_CACHE_LIMIT: usize = 120;

type PathHandler = Rc<dyn Fn(Vec<String>)>;
type OneHandler = Rc<dyn Fn(String)>;
type MenuHandler = Rc<dyn Fn(String, &NSEvent)>;
type DragHandler = Rc<dyn Fn(Drag)>;

/// A tile on its way out of the shelf, in screen coordinates.
#[derive(Debug, Clone, PartialEq)]
pub enum Drag {
    Began {
        path: String,
        at: NSPoint,
    },
    Moved(NSPoint),
    /// `dropped` is false when the drag was given up and the file stays where it was.
    Ended {
        at: NSPoint,
        dropped: bool,
    },
}

pub struct CanvasIvars {
    paths: RefCell<Vec<String>>,
    selected: Cell<Option<usize>>,
    icons: RefCell<HashMap<String, Retained<NSImage>>>,
    /// A drag carrying files is over the canvas.
    hovering: Cell<bool>,
    /// Where the mouse went down and on which tile, to tell a click from the start of a drag.
    press: Cell<Option<(NSPoint, usize)>>,
    min_height: Cell<f64>,
    on_drop: RefCell<Option<PathHandler>>,
    on_open: RefCell<Option<OneHandler>>,
    on_menu: RefCell<Option<MenuHandler>>,
    on_drag: RefCell<Option<DragHandler>>,
    /// The file of the drag under way.
    dragging: RefCell<Option<String>>,
}

define_class!(
    // SAFETY: NSView allows subclassing; the ivars are only touched on the main thread and need no Drop.
    #[unsafe(super(NSView))]
    #[thread_kind = MainThreadOnly]
    #[name = "FigShelfCanvas"]
    #[ivars = CanvasIvars]
    pub struct ShelfCanvas;

    impl ShelfCanvas {
        #[unsafe(method(isFlipped))]
        fn is_flipped(&self) -> bool {
            true
        }

        #[unsafe(method(acceptsFirstMouse:))]
        fn accepts_first_mouse(&self, _event: Option<&NSEvent>) -> bool {
            true
        }

        #[unsafe(method(drawRect:))]
        fn draw_rect(&self, dirty: NSRect) {
            self.paint(dirty);
        }

        #[unsafe(method(rightMouseDown:))]
        fn right_mouse_down(&self, event: &NSEvent) {
            self.show_context_menu(event);
        }

        #[unsafe(method(mouseDown:))]
        fn mouse_down(&self, event: &NSEvent) {
            // Control-click is a right click.
            if event.modifierFlags().contains(NSEventModifierFlags::Control) {
                self.show_context_menu(event);
                return;
            }
            let point = self.convertPoint_fromView(event.locationInWindow(), None);
            let tile = self.tile_at(point);
            self.ivars().selected.set(tile);
            self.ivars().press.set(tile.map(|t| (point, t)));
            self.setNeedsDisplay(true);
            if let (Some(tile), 2) = (tile, event.clickCount()) {
                let path = self.ivars().paths.borrow().get(tile).cloned();
                let open = self.ivars().on_open.borrow().clone();
                if let (Some(path), Some(open)) = (path, open) {
                    open(path);
                }
            }
        }

        #[unsafe(method(mouseDragged:))]
        fn mouse_dragged(&self, event: &NSEvent) {
            let Some((origin, tile)) = self.ivars().press.get() else {
                return;
            };
            let point = self.convertPoint_fromView(event.locationInWindow(), None);
            if (point.x - origin.x).hypot(point.y - origin.y) > DRAG_THRESHOLD {
                self.ivars().press.set(None);
                self.begin_drag(event, tile);
            }
        }

        #[unsafe(method(mouseUp:))]
        fn mouse_up(&self, _event: &NSEvent) {
            self.ivars().press.set(None);
        }
    }

    unsafe impl NSObjectProtocol for ShelfCanvas {}

    unsafe impl NSDraggingSource for ShelfCanvas {
        #[unsafe(method(draggingSession:sourceOperationMaskForDraggingContext:))]
        fn source_operation_mask(
            &self,
            _session: &NSDraggingSession,
            _context: NSDraggingContext,
        ) -> NSDragOperation {
            // Copy only: dragging a tile out must never move or delete the original file.
            NSDragOperation::Copy
        }

        #[unsafe(method(draggingSession:willBeginAtPoint:))]
        fn session_will_begin(&self, _session: &NSDraggingSession, point: NSPoint) {
            if let Some(path) = self.ivars().dragging.borrow().clone() {
                self.report_drag(Drag::Began { path, at: point });
            }
        }

        #[unsafe(method(draggingSession:movedToPoint:))]
        fn session_moved(&self, _session: &NSDraggingSession, point: NSPoint) {
            self.report_drag(Drag::Moved(point));
        }

        #[unsafe(method(draggingSession:endedAtPoint:operation:))]
        fn session_ended(&self, _session: &NSDraggingSession, point: NSPoint, operation: NSDragOperation) {
            self.ivars().dragging.borrow_mut().take();
            self.report_drag(Drag::Ended {
                at: point,
                dropped: operation != NSDragOperation::None,
            });
        }
    }

    unsafe impl NSDraggingDestination for ShelfCanvas {
        #[unsafe(method(draggingEntered:))]
        fn dragging_entered(&self, sender: &ProtocolObject<dyn NSDraggingInfo>) -> NSDragOperation {
            if self.carries_foreign_files(sender) {
                self.ivars().hovering.set(true);
                self.setNeedsDisplay(true);
                NSDragOperation::Copy
            } else {
                NSDragOperation::None
            }
        }

        #[unsafe(method(draggingExited:))]
        fn dragging_exited(&self, _sender: Option<&ProtocolObject<dyn NSDraggingInfo>>) {
            self.ivars().hovering.set(false);
            self.setNeedsDisplay(true);
        }

        #[unsafe(method(performDragOperation:))]
        fn perform_drag_operation(&self, sender: &ProtocolObject<dyn NSDraggingInfo>) -> bool {
            self.ivars().hovering.set(false);
            self.setNeedsDisplay(true);
            let handler = self.ivars().on_drop.borrow().clone();
            match handler {
                Some(handler) if self.carries_foreign_files(sender) => {
                    handler(file_paths(&sender.draggingPasteboard()));
                    true
                }
                _ => false,
            }
        }
    }
);

impl ShelfCanvas {
    fn new(mtm: MainThreadMarker, frame: NSRect) -> Retained<Self> {
        let ivars = CanvasIvars {
            paths: RefCell::new(Vec::new()),
            selected: Cell::new(None),
            icons: RefCell::new(HashMap::new()),
            hovering: Cell::new(false),
            press: Cell::new(None),
            min_height: Cell::new(frame.size.height),
            on_drop: RefCell::new(None),
            on_open: RefCell::new(None),
            on_menu: RefCell::new(None),
            on_drag: RefCell::new(None),
            dragging: RefCell::new(None),
        };
        // SAFETY: designated initializer with valid arguments on the main thread.
        let this: Retained<Self> =
            unsafe { msg_send![super(mtm.alloc::<Self>().set_ivars(ivars)), initWithFrame: frame] };
        this.setAutoresizingMask(NSAutoresizingMaskOptions::ViewWidthSizable);
        // SAFETY: extern statics are process-lifetime constants.
        let types = NSArray::from_slice(&[unsafe { NSPasteboardTypeFileURL }]);
        this.registerForDraggedTypes(&types);
        this
    }

    /// Selects the tile under the pointer and hands its path to the owner, which pops up the menu.
    fn show_context_menu(&self, event: &NSEvent) {
        let point = self.convertPoint_fromView(event.locationInWindow(), None);
        let Some(tile) = self.tile_at(point) else {
            return;
        };
        self.ivars().selected.set(Some(tile));
        self.setNeedsDisplay(true);
        let path = self.ivars().paths.borrow().get(tile).cloned();
        let handler = self.ivars().on_menu.borrow().clone();
        if let (Some(path), Some(handler)) = (path, handler) {
            handler(path, event);
        }
    }

    fn report_drag(&self, drag: Drag) {
        let handler = self.ivars().on_drag.borrow().clone();
        if let Some(handler) = handler {
            handler(drag);
        }
    }

    /// Files from another app only: a tile dragged out and back in would otherwise duplicate itself.
    fn carries_foreign_files(&self, sender: &ProtocolObject<dyn NSDraggingInfo>) -> bool {
        let from_self = sender.draggingSource().is_some_and(|source| {
            std::ptr::eq(
                &*source as *const _ as *const u8,
                self as *const _ as *const u8,
            )
        });
        !from_self && !file_paths(&sender.draggingPasteboard()).is_empty()
    }

    fn columns(&self) -> usize {
        (((self.bounds().size.width - PAD) / TILE_WIDTH).floor() as usize).max(1)
    }

    fn tile_rect(&self, index: usize) -> NSRect {
        let columns = self.columns();
        NSRect::new(
            NSPoint::new(
                PAD + (index % columns) as f64 * TILE_WIDTH,
                PAD + (index / columns) as f64 * TILE_HEIGHT,
            ),
            NSSize::new(TILE_WIDTH, TILE_HEIGHT),
        )
    }

    fn tile_at(&self, point: NSPoint) -> Option<usize> {
        let count = self.ivars().paths.borrow().len();
        (0..count).find(|&i| {
            let r = self.tile_rect(i);
            point.x >= r.origin.x
                && point.x < r.origin.x + r.size.width
                && point.y >= r.origin.y
                && point.y < r.origin.y + r.size.height
        })
    }

    fn icon(&self, path: &str) -> Retained<NSImage> {
        let mut icons = self.ivars().icons.borrow_mut();
        if icons.len() >= ICON_CACHE_LIMIT {
            icons.clear();
        }
        icons
            .entry(path.to_owned())
            .or_insert_with(|| {
                NSWorkspace::sharedWorkspace().iconForFile(&NSString::from_str(path))
            })
            .clone()
    }

    fn icon_rect(&self, index: usize) -> NSRect {
        let tile = self.tile_rect(index);
        NSRect::new(
            NSPoint::new(
                tile.origin.x + (TILE_WIDTH - ICON) / 2.0,
                tile.origin.y + 6.0,
            ),
            NSSize::new(ICON, ICON),
        )
    }

    fn begin_drag(&self, event: &NSEvent, index: usize) {
        let Some(path) = self.ivars().paths.borrow().get(index).cloned() else {
            return;
        };
        let url = NSURL::fileURLWithPath(&NSString::from_str(&path));
        let writer: Retained<ProtocolObject<dyn NSPasteboardWriting>> =
            ProtocolObject::from_retained(url);
        let item = NSDraggingItem::initWithPasteboardWriter(NSDraggingItem::alloc(), &writer);
        let icon = self.icon(&path);
        // SAFETY: the contents object is an NSImage, which NSDraggingItem accepts.
        unsafe { item.setDraggingFrame_contents(self.icon_rect(index), Some(&icon)) };
        let items = NSArray::from_slice(&[&*item]);
        *self.ivars().dragging.borrow_mut() = Some(path);
        self.beginDraggingSessionWithItems_event_source(
            &items,
            event,
            ProtocolObject::from_ref(self),
        );
    }

    fn paint(&self, dirty: NSRect) {
        let paths = self.ivars().paths.borrow();
        let bounds = self.bounds();
        if paths.is_empty() {
            let height = self.ivars().min_height.get();
            // A dashed outline says "drop here" without a word.
            let zone = NSRect::new(
                NSPoint::new(16.0, 8.0),
                NSSize::new(bounds.size.width - 32.0, height - 24.0),
            );
            let outline = NSBezierPath::bezierPathWithRoundedRect_xRadius_yRadius(
                zone,
                style::CARD_RADIUS,
                style::CARD_RADIUS,
            );
            style::white(0.22).setStroke();
            outline.setLineWidth(1.5);
            let dash = [7.0_f64, 6.0];
            // SAFETY: the pattern pointer is valid for the two entries named by the count.
            unsafe { outline.setLineDash_count_phase(dash.as_ptr().cast_mut(), 2, 0.0) };
            outline.stroke();
            let centre = NSPoint::new(bounds.size.width / 2.0, height / 2.0 + 8.0);
            style::draw_symbol("arrow.down.doc", centre, 30.0, &style::white(0.5));
            draw_text(
                &labels::get().shelf_empty,
                NSRect::new(
                    NSPoint::new(24.0, centre.y + 26.0),
                    NSSize::new(bounds.size.width - 48.0, 20.0),
                ),
                &text_attributes(
                    &style::font(13.0, style::Weight::Medium),
                    &style::white(style::TEXT_SECONDARY),
                    Fit::Clip,
                    true,
                ),
            );
        }
        let selected = self.ivars().selected.get();
        let name_attrs = |white: f64| {
            text_attributes(
                &NSFont::systemFontOfSize(11.0),
                &NSColor::colorWithWhite_alpha(1.0, white),
                Fit::Middle,
                true,
            )
        };
        for (index, path) in paths.iter().enumerate() {
            let tile = self.tile_rect(index);
            if tile.origin.y + tile.size.height < dirty.origin.y
                || tile.origin.y > dirty.origin.y + dirty.size.height
            {
                continue;
            }
            let card = NSRect::new(
                NSPoint::new(tile.origin.x + 3.0, tile.origin.y + 2.0),
                NSSize::new(tile.size.width - 6.0, tile.size.height - 4.0),
            );
            if selected == Some(index) {
                style::card_selected(card, 14.0, &style::Tone::Blue);
            } else {
                style::card(card, 14.0);
            }
            let icon = self.icon(path);
            // SAFETY: called from drawRect: with a valid graphics context; a zero source rect means the whole image.
            unsafe {
                icon.drawInRect_fromRect_operation_fraction_respectFlipped_hints(
                    self.icon_rect(index),
                    NSRect::ZERO,
                    NSCompositingOperation::SourceOver,
                    1.0,
                    true,
                    None,
                );
            }
            let name = Path::new(path)
                .file_name()
                .map_or_else(|| path.clone(), |n| n.to_string_lossy().into_owned());
            let white = if selected == Some(index) { 1.0 } else { 0.85 };
            draw_text(
                &name,
                NSRect::new(
                    NSPoint::new(tile.origin.x + 4.0, tile.origin.y + 6.0 + ICON + 8.0),
                    NSSize::new(tile.size.width - 8.0, 16.0),
                ),
                &name_attrs(white),
            );
        }
        if self.ivars().hovering.get() {
            // Something is being dragged over the page: it will be taken in.
            let zone = NSRect::new(
                NSPoint::new(8.0, 6.0),
                NSSize::new(bounds.size.width - 16.0, bounds.size.height - 12.0),
            );
            style::card(zone, style::CARD_RADIUS);
            style::stroke_rounded(zone, style::CARD_RADIUS, &style::white(0.7), 2.0);
        }
    }

    fn set_paths(&self, paths: Vec<String>) {
        let count = paths.len();
        *self.ivars().paths.borrow_mut() = paths;
        let selected = self.ivars().selected.get().filter(|&i| i < count);
        self.ivars().selected.set(selected);
        let rows = count.div_ceil(self.columns());
        let height = (2.0 * PAD + rows as f64 * TILE_HEIGHT).max(self.ivars().min_height.get());
        self.setFrameSize(NSSize::new(self.frame().size.width, height));
        self.setNeedsDisplay(true);
    }

    fn selected_path(&self) -> Option<String> {
        let index = self.ivars().selected.get()?;
        self.ivars().paths.borrow().get(index).cloned()
    }

    /// Moves the selection by whole tiles; starts at the first tile when nothing is selected.
    fn move_selection(&self, columns_delta: isize, rows_delta: isize) {
        let count = self.ivars().paths.borrow().len();
        if count == 0 {
            return;
        }
        let step = columns_delta + rows_delta * self.columns() as isize;
        let next = match self.ivars().selected.get() {
            None => 0,
            Some(current) => (current as isize + step).clamp(0, count as isize - 1) as usize,
        };
        self.ivars().selected.set(Some(next));
        self.scrollRectToVisible(self.tile_rect(next));
        self.setNeedsDisplay(true);
    }
}

/// One entry of the menu a tile offers; an empty `id` is a separator.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ShelfAction {
    pub id: String,
    pub title: String,
}

impl ShelfAction {
    pub fn new(id: &str, title: &str) -> Self {
        Self {
            id: id.into(),
            title: title.into(),
        }
    }

    pub fn separator() -> Self {
        Self::new("", "")
    }
}

/// The menu a file offers, and what happens when an entry is chosen.
pub type MenuFor = Box<dyn Fn(&str) -> Vec<ShelfAction>>;
pub type MenuChoice = Box<dyn Fn(&str, &str)>;

/// What the app decides about the shelf contents.
pub struct ShelfCallbacks {
    pub on_drop: Box<dyn Fn(Vec<String>)>,
    pub on_remove: Box<dyn Fn(String)>,
    pub on_open: Box<dyn Fn(String)>,
    /// The menu for a file; the app knows what can be done with it.
    pub actions_for: MenuFor,
    /// Called with the file and the id of the entry the user chose.
    pub on_action: MenuChoice,
}

/// A context menu for `actions`, with the targets that must stay alive while it is open.
pub fn build_menu(
    mtm: MainThreadMarker,
    actions: &[ShelfAction],
    on_pick: &Rc<dyn Fn(&str)>,
) -> (Retained<NSMenu>, Vec<Retained<ActionTarget>>) {
    let menu = NSMenu::new(mtm);
    menu.setAutoenablesItems(false);
    let mut targets = Vec::new();
    for action in actions {
        if action.id.is_empty() {
            menu.addItem(&NSMenuItem::separatorItem(mtm));
            continue;
        }
        let (pick, id) = (Rc::clone(on_pick), action.id.clone());
        let target = ActionTarget::new(mtm, move || pick(&id));
        // SAFETY: `fire:` is implemented by ActionTarget; the target is returned to the caller to outlive the menu.
        let item = unsafe {
            NSMenuItem::initWithTitle_action_keyEquivalent(
                NSMenuItem::alloc(mtm),
                &NSString::from_str(&action.title),
                Some(sel!(fire:)),
                &NSString::from_str(""),
            )
        };
        let target_object: &AnyObject = &target;
        // SAFETY: the target outlives the menu (see above).
        unsafe { item.setTarget(Some(target_object)) };
        menu.addItem(&item);
        targets.push(target);
    }
    (menu, targets)
}

pub struct ShelfView {
    pub root: Retained<PageRoot>,
    canvas: Retained<ShelfCanvas>,
    callbacks: ShelfCallbacks,
}

impl ShelfView {
    pub fn new(mtm: MainThreadMarker, frame: NSRect, callbacks: ShelfCallbacks) -> Rc<Self> {
        let root = PageRoot::new(mtm, frame, &labels::get().shelf_hint);
        let scroll_frame = NSRect::new(
            NSPoint::new(0.0, FOOTER_HEIGHT),
            NSSize::new(frame.size.width, frame.size.height - FOOTER_HEIGHT - 8.0),
        );
        let scroll = NSScrollView::initWithFrame(mtm.alloc(), scroll_frame);
        scroll.setHasVerticalScroller(true);
        scroll.setAutohidesScrollers(true);
        scroll.setScrollerStyle(NSScrollerStyle::Overlay);
        scroll.setDrawsBackground(false);
        scroll.setBorderType(NSBorderType::NoBorder);
        let canvas = ShelfCanvas::new(mtm, NSRect::new(NSPoint::ZERO, scroll_frame.size));
        scroll.setDocumentView(Some(&canvas));
        root.addSubview(&scroll);

        let this = Rc::new(Self {
            root,
            canvas,
            callbacks,
        });
        let (drop, open) = (Rc::clone(&this), Rc::clone(&this));
        *this.canvas.ivars().on_drop.borrow_mut() =
            Some(Rc::new(move |paths| (drop.callbacks.on_drop)(paths)));
        *this.canvas.ivars().on_open.borrow_mut() =
            Some(Rc::new(move |path| (open.callbacks.on_open)(path)));
        let menu_owner = Rc::downgrade(&this);
        *this.canvas.ivars().on_menu.borrow_mut() = Some(Rc::new(move |path, event| {
            if let Some(view) = menu_owner.upgrade() {
                view.pop_up_menu(mtm, &path, event);
            }
        }));
        this
    }

    /// Pops up the menu for one file. The chosen entry runs on the next turn of the run loop, when the menu
    /// is certainly gone, so an action that changes the shelf never does it under the menu.
    fn pop_up_menu(self: &Rc<Self>, mtm: MainThreadMarker, path: &str, event: &NSEvent) {
        let actions = (self.callbacks.actions_for)(path);
        if actions.is_empty() {
            return;
        }
        let (view, file) = (Rc::clone(self), path.to_owned());
        let on_pick: Rc<dyn Fn(&str)> = Rc::new(move |id| {
            let (view, file, id) = (Rc::clone(&view), file.clone(), id.to_owned());
            crate::app::after(0.0, move || (view.callbacks.on_action)(&file, &id));
        });
        let (menu, _targets) = build_menu(mtm, &actions, &on_pick);
        NSMenu::popUpContextMenu_withEvent_forView(&menu, event, &self.canvas);
    }

    pub fn set_paths(&self, paths: Vec<String>) {
        self.canvas.set_paths(paths);
    }

    /// Told where a tile dragged out of the shelf is, from the start of the drag to its end.
    pub fn set_on_drag(&self, on_drag: impl Fn(Drag) + 'static) {
        *self.canvas.ivars().on_drag.borrow_mut() = Some(Rc::new(on_drag));
    }

    /// The notch tab for this view; `on_show` refreshes the files each time the tab opens.
    pub fn page(self: &Rc<Self>, on_show: impl Fn() + 'static) -> Page {
        let keys = Rc::clone(self);
        Page {
            id: "shelf",
            title: labels::get().shelf_tab.clone(),
            symbol: "tray.full.fill",
            root: Retained::into_super(Retained::clone(&self.root)),
            on_show: Box::new(on_show),
            on_hide: Box::new(|| {}),
            on_key: Box::new(move |key| keys.handle_key(key)),
            accepts_files: true,
        }
    }

    pub fn selected_path(&self) -> Option<String> {
        self.canvas.selected_path()
    }

    /// Centre of a tile in Cocoa screen coordinates (origin bottom-left), for driving the view with
    /// synthetic mouse events in checks.
    pub fn tile_center_on_screen(&self, index: usize) -> Option<NSPoint> {
        let tile = self.canvas.tile_rect(index);
        let center = NSPoint::new(
            tile.origin.x + tile.size.width / 2.0,
            tile.origin.y + tile.size.height / 2.0,
        );
        let window = self.canvas.window()?;
        Some(window.convertPointToScreen(self.canvas.convertPoint_toView(center, None)))
    }

    /// Centre of the whole page in Cocoa screen coordinates.
    pub fn center_on_screen(&self) -> Option<NSPoint> {
        let bounds = self.root.bounds();
        let center = NSPoint::new(bounds.size.width / 2.0, bounds.size.height / 2.0);
        let window = self.root.window()?;
        Some(window.convertPointToScreen(self.root.convertPoint_toView(center, None)))
    }

    /// Returns true when the key was consumed.
    pub fn handle_key(&self, key: Key) -> bool {
        match key {
            Key::Left => self.canvas.move_selection(-1, 0),
            Key::Right => self.canvas.move_selection(1, 0),
            Key::Up => self.canvas.move_selection(0, -1),
            Key::Down => self.canvas.move_selection(0, 1),
            Key::Enter => {
                if let Some(path) = self.selected_path() {
                    (self.callbacks.on_open)(path);
                }
            }
            Key::Delete | Key::Pin => {
                if let Some(path) = self.selected_path() {
                    (self.callbacks.on_remove)(path);
                }
            }
            _ => return false,
        }
        true
    }
}
