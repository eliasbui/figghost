//! Clipboard list UI: search field, scrolling list with thumbnails, footer hint, and the cursor popup.

use crate::{
    labels,
    panel::{FigPanel, Key},
    widgets::{Fit, PageRoot, draw_text, text_attributes},
};
use block2::RcBlock;
use objc2::{
    AnyThread, DefinedClass, MainThreadMarker, MainThreadOnly, define_class, msg_send,
    rc::Retained,
    runtime::{AnyObject, ProtocolObject},
};
use objc2_app_kit::{
    NSAutoresizingMaskOptions, NSBezierPath, NSBorderType, NSColor, NSCompositingOperation,
    NSControlTextDidChangeNotification, NSEvent, NSFocusRingType, NSFont, NSFontWeightMedium,
    NSImage, NSScreen, NSScrollView, NSScrollerStyle, NSSearchField, NSTrackingArea,
    NSTrackingAreaOptions, NSView, NSVisualEffectBlendingMode, NSVisualEffectMaterial,
    NSVisualEffectState, NSVisualEffectView, NSWindowDidResignKeyNotification,
};
use objc2_foundation::{
    NSNotification, NSNotificationCenter, NSObjectProtocol, NSPoint, NSRect, NSSize, NSString,
};
use std::{
    cell::RefCell,
    collections::HashMap,
    path::{Path, PathBuf},
    ptr::NonNull,
    rc::Rc,
};

const ROW_HEIGHT: f64 = 46.0;
const THUMB: f64 = 34.0;
const MARGIN: f64 = 12.0;
const SEARCH_HEIGHT: f64 = 28.0;
const FOOTER_HEIGHT: f64 = 24.0;
const THUMB_CACHE_LIMIT: usize = 96;
const PAGE_ROWS: isize = 7;
const KEY_LABEL_WIDTH: f64 = 30.0;

pub const POPUP_SIZE: NSSize = NSSize::new(460.0, 440.0);

#[derive(Debug, Clone)]
pub struct Row {
    pub id: i64,
    pub title: String,
    pub subtitle: String,
    pub pinned: bool,
    /// The letter that pastes this row with Command, shown at its end.
    pub pin_key: Option<char>,
    pub mono: bool,
    pub thumb: Option<PathBuf>,
}

struct ListState {
    rows: Vec<Row>,
    selected: usize,
    min_height: f64,
    empty_text: String,
}

type RowActivate = Rc<dyn Fn(i64)>;

pub struct ListIvars {
    state: RefCell<ListState>,
    thumbs: RefCell<HashMap<PathBuf, Option<Retained<NSImage>>>>,
    on_activate: RefCell<Option<RowActivate>>,
}

define_class!(
    // SAFETY: NSView allows subclassing; the ivars are only touched on the main thread and need no Drop.
    #[unsafe(super(NSView))]
    #[thread_kind = MainThreadOnly]
    #[name = "FigListView"]
    #[ivars = ListIvars]
    pub struct ListView;

    impl ListView {
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

        #[unsafe(method(mouseMoved:))]
        fn mouse_moved(&self, event: &NSEvent) {
            if let Some(row) = self.row_at(event) {
                self.select(row, false);
            }
        }

        #[unsafe(method(mouseDown:))]
        fn mouse_down(&self, event: &NSEvent) {
            if let Some(row) = self.row_at(event) {
                self.select(row, false);
                if let Some(id) = self.selected_id() {
                    let callback = self.ivars().on_activate.borrow().clone();
                    if let Some(callback) = callback {
                        callback(id);
                    }
                }
            }
        }
    }
);

impl ListView {
    fn new(mtm: MainThreadMarker, frame: NSRect) -> Retained<Self> {
        let ivars = ListIvars {
            state: RefCell::new(ListState {
                rows: Vec::new(),
                selected: 0,
                min_height: frame.size.height,
                empty_text: String::new(),
            }),
            thumbs: RefCell::new(HashMap::new()),
            on_activate: RefCell::new(None),
        };
        // SAFETY: designated initializer with valid arguments on the main thread.
        let this: Retained<Self> =
            unsafe { msg_send![super(mtm.alloc::<Self>().set_ivars(ivars)), initWithFrame: frame] };
        this.setAutoresizingMask(NSAutoresizingMaskOptions::ViewWidthSizable);
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

    fn set_on_activate(&self, f: impl Fn(i64) + 'static) {
        *self.ivars().on_activate.borrow_mut() = Some(Rc::new(f));
    }

    /// Replaces the rows, selects the first and scrolls back to the top.
    pub fn set_rows(&self, rows: Vec<Row>, empty_text: &str) {
        let width = self.frame().size.width;
        {
            let mut state = self.ivars().state.borrow_mut();
            let height = (rows.len() as f64 * ROW_HEIGHT).max(state.min_height);
            self.setFrameSize(NSSize::new(width, height));
            state.rows = rows;
            state.selected = 0;
            state.empty_text = empty_text.to_owned();
        }
        self.scrollPoint(NSPoint::ZERO);
        self.setNeedsDisplay(true);
    }

    fn set_min_height(&self, height: f64) {
        self.ivars().state.borrow_mut().min_height = height;
    }

    /// The row pinned with this letter.
    pub fn id_for_pin_key(&self, letter: char) -> Option<i64> {
        let state = self.ivars().state.borrow();
        state
            .rows
            .iter()
            .find(|r| r.pin_key == Some(letter))
            .map(|r| r.id)
    }

    pub fn selected_id(&self) -> Option<i64> {
        let state = self.ivars().state.borrow();
        state.rows.get(state.selected).map(|r| r.id)
    }

    /// Keeps the selection near where it was after the rows were rebuilt (pin, delete).
    pub fn keep_selection(&self, index: usize) {
        let mut state = self.ivars().state.borrow_mut();
        state.selected = index.min(state.rows.len().saturating_sub(1));
        drop(state);
        self.setNeedsDisplay(true);
    }

    /// Moves the selection to the row with this id, if it is still listed.
    pub fn select_id(&self, id: i64) {
        let index = self
            .ivars()
            .state
            .borrow()
            .rows
            .iter()
            .position(|r| r.id == id);
        if let Some(index) = index {
            self.ivars().state.borrow_mut().selected = usize::MAX;
            self.select(index, true);
        }
    }

    pub fn selected_index(&self) -> usize {
        self.ivars().state.borrow().selected
    }

    pub fn move_selection(&self, delta: isize) {
        let len = self.ivars().state.borrow().rows.len();
        if len == 0 {
            return;
        }
        let current = self.ivars().state.borrow().selected as isize;
        let next = (current + delta).clamp(0, len as isize - 1) as usize;
        self.select(next, true);
    }

    fn select(&self, row: usize, reveal: bool) {
        {
            let mut state = self.ivars().state.borrow_mut();
            if state.selected == row {
                return;
            }
            state.selected = row;
        }
        if reveal {
            let rect = NSRect::new(
                NSPoint::new(0.0, row as f64 * ROW_HEIGHT),
                NSSize::new(self.bounds().size.width, ROW_HEIGHT),
            );
            self.scrollRectToVisible(rect);
        }
        self.setNeedsDisplay(true);
    }

    fn row_at(&self, event: &NSEvent) -> Option<usize> {
        let point = self.convertPoint_fromView(event.locationInWindow(), None);
        if point.y < 0.0 {
            return None;
        }
        let row = (point.y / ROW_HEIGHT) as usize;
        (row < self.ivars().state.borrow().rows.len()).then_some(row)
    }

    fn thumbnail(&self, path: &Path) -> Option<Retained<NSImage>> {
        let mut cache = self.ivars().thumbs.borrow_mut();
        if cache.len() >= THUMB_CACHE_LIMIT {
            cache.clear();
        }
        cache
            .entry(path.to_path_buf())
            .or_insert_with(|| {
                let file = NSString::from_str(&path.to_string_lossy());
                NSImage::initWithContentsOfFile(NSImage::alloc(), &file)
            })
            .clone()
    }

    fn paint(&self, dirty: NSRect) {
        let state = self.ivars().state.borrow();
        let width = self.bounds().size.width;
        if state.rows.is_empty() {
            let attrs = text_attributes(
                &NSFont::systemFontOfSize(13.0),
                &NSColor::colorWithWhite_alpha(1.0, 0.5),
                Fit::Clip,
                true,
            );
            let rect = NSRect::new(
                NSPoint::new(0.0, state.min_height / 2.0 - 10.0),
                NSSize::new(width, 20.0),
            );
            draw_text(&state.empty_text, rect, &attrs);
            return;
        }
        let first = (dirty.origin.y / ROW_HEIGHT).floor().max(0.0) as usize;
        let last = (((dirty.origin.y + dirty.size.height) / ROW_HEIGHT).ceil() as usize)
            .min(state.rows.len());
        for index in first..last {
            self.paint_row(&state.rows[index], index, index == state.selected, width);
        }
    }

    fn paint_row(&self, row: &Row, index: usize, selected: bool, width: f64) {
        let top = index as f64 * ROW_HEIGHT;
        if selected {
            let rect = NSRect::new(
                NSPoint::new(6.0, top + 2.0),
                NSSize::new(width - 12.0, ROW_HEIGHT - 4.0),
            );
            NSColor::selectedContentBackgroundColor().setFill();
            NSBezierPath::bezierPathWithRoundedRect_xRadius_yRadius(rect, 8.0, 8.0).fill();
        }
        if row.pinned {
            let bar = NSRect::new(
                NSPoint::new(10.0, top + 12.0),
                NSSize::new(3.0, ROW_HEIGHT - 24.0),
            );
            let color = if selected {
                NSColor::whiteColor()
            } else {
                NSColor::controlAccentColor()
            };
            color.setFill();
            NSBezierPath::bezierPathWithRoundedRect_xRadius_yRadius(bar, 1.5, 1.5).fill();
        }

        let mut left = 20.0;
        if let Some(image) = row.thumb.as_ref().and_then(|p| self.thumbnail(p)) {
            let size = image.size();
            let scale = (THUMB / size.width.max(1.0)).min(THUMB / size.height.max(1.0));
            let (w, h) = (size.width * scale, size.height * scale);
            let dst = NSRect::new(
                NSPoint::new(left + (THUMB - w) / 2.0, top + (ROW_HEIGHT - h) / 2.0),
                NSSize::new(w, h),
            );
            // Flipped view: plain drawInRect would render the image upside down.
            // SAFETY: called from drawRect: with a valid graphics context; a zero source rect means the whole image.
            unsafe {
                image.drawInRect_fromRect_operation_fraction_respectFlipped_hints(
                    dst,
                    NSRect::ZERO,
                    NSCompositingOperation::SourceOver,
                    1.0,
                    true,
                    None,
                );
            }
            left += THUMB + 10.0;
        }

        // Every surface is dark. Semantic label colours are vibrancy based and fade out on the blurred popup.
        let (title_color, subtitle_color) = if selected {
            (
                NSColor::whiteColor(),
                NSColor::colorWithWhite_alpha(1.0, 0.78),
            )
        } else {
            (
                NSColor::colorWithWhite_alpha(1.0, 0.92),
                NSColor::colorWithWhite_alpha(1.0, 0.52),
            )
        };
        let title_font = if row.mono {
            NSFont::userFixedPitchFontOfSize(12.0).unwrap_or_else(|| NSFont::systemFontOfSize(13.0))
        } else {
            NSFont::systemFontOfSize(13.0)
        };
        let key_label = row
            .pin_key
            .map(|c| format!("\u{2318}{}", c.to_ascii_uppercase()));
        if let Some(label) = &key_label {
            let color = if selected {
                NSColor::colorWithWhite_alpha(1.0, 0.8)
            } else {
                NSColor::colorWithWhite_alpha(1.0, 0.45)
            };
            let attrs = text_attributes(
                // SAFETY: extern static weight constant.
                &NSFont::monospacedSystemFontOfSize_weight(11.0, unsafe { NSFontWeightMedium }),
                &color,
                Fit::Clip,
                false,
            );
            draw_text(
                label,
                NSRect::new(
                    NSPoint::new(width - 16.0 - KEY_LABEL_WIDTH, top + 15.0),
                    NSSize::new(KEY_LABEL_WIDTH, 16.0),
                ),
                &attrs,
            );
        }
        let reserved = if key_label.is_some() {
            KEY_LABEL_WIDTH + 6.0
        } else {
            0.0
        };
        let text_width = (width - left - 16.0 - reserved).max(10.0);
        draw_text(
            &row.title,
            NSRect::new(NSPoint::new(left, top + 7.0), NSSize::new(text_width, 18.0)),
            &text_attributes(&title_font, &title_color, Fit::Tail, false),
        );
        draw_text(
            &row.subtitle,
            NSRect::new(
                NSPoint::new(left, top + 26.0),
                NSSize::new(text_width, 14.0),
            ),
            &text_attributes(
                &NSFont::systemFontOfSize(11.0),
                &subtitle_color,
                Fit::Tail,
                false,
            ),
        );
    }
}

/// Handles to the pieces of one clipboard surface; cheap to clone, passed to every callback.
#[derive(Clone)]
pub struct ClipView {
    pub root: Retained<PageRoot>,
    pub search: Retained<NSSearchField>,
    pub list: Retained<ListView>,
}

impl ClipView {
    pub fn query(&self) -> String {
        self.search.stringValue().to_string()
    }

    pub fn clear_query(&self) {
        self.search.setStringValue(&NSString::from_str(""));
    }

    pub fn set_rows(&self, rows: Vec<Row>, empty_text: &str) {
        self.list.set_rows(rows, empty_text);
    }

    /// Hides the window this surface lives in.
    pub fn hide_window(&self) {
        if let Some(window) = self.root.window() {
            window.orderOut(None);
        }
    }
}

/// How an item is pasted: the usual way, or the other way (with formatting when plain is the default, and
/// the reverse).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Activate {
    Normal,
    Alternate,
}

pub type QueryHandler = Box<dyn Fn(&ClipView, &str)>;
pub type ItemHandler = Box<dyn Fn(&ClipView, i64)>;
pub type ActivateCallback = Box<dyn Fn(&ClipView, i64, Activate)>;
pub type CloseHandler = Box<dyn Fn(&ClipView)>;

/// What the application decides; closing the surface is each surface's own business.
pub struct AppCallbacks {
    pub on_query: QueryHandler,
    pub on_activate: ActivateCallback,
    pub on_pin: ItemHandler,
    pub on_delete: ItemHandler,
}

pub struct Callbacks {
    pub on_query: QueryHandler,
    pub on_activate: ActivateCallback,
    pub on_pin: ItemHandler,
    pub on_delete: ItemHandler,
    pub on_close: CloseHandler,
}

/// Returns true when the key was consumed.
pub fn handle_key(view: &ClipView, callbacks: &Callbacks, key: Key) -> bool {
    match key {
        Key::Up => view.list.move_selection(-1),
        Key::Down => view.list.move_selection(1),
        Key::PageUp => view.list.move_selection(-PAGE_ROWS),
        Key::PageDown => view.list.move_selection(PAGE_ROWS),
        Key::Escape => (callbacks.on_close)(view),
        Key::Enter | Key::EnterAlt => {
            let mode = if key == Key::EnterAlt {
                Activate::Alternate
            } else {
                Activate::Normal
            };
            if let Some(id) = view.list.selected_id() {
                (callbacks.on_activate)(view, id, mode);
            }
        }
        Key::Shortcut(letter) => {
            // A letter nothing is pinned to is an ordinary shortcut (Command+A, Command+C) for the text field.
            let Some(id) = view.list.id_for_pin_key(letter) else {
                return false;
            };
            (callbacks.on_activate)(view, id, Activate::Normal);
        }
        Key::Pin => {
            if let Some(id) = view.list.selected_id() {
                (callbacks.on_pin)(view, id);
            }
        }
        Key::Delete => {
            if let Some(id) = view.list.selected_id() {
                (callbacks.on_delete)(view, id);
            }
        }
        // Caret movement belongs to the search field; tab switching belongs to the notch.
        Key::Left | Key::Right | Key::Tab(_) => return false,
    }
    true
}

/// A built surface plus the text-change observer that must stay registered with it.
pub struct ClipUi {
    pub view: ClipView,
    pub callbacks: Rc<Callbacks>,
    center: Retained<NSNotificationCenter>,
    token: Retained<ProtocolObject<dyn NSObjectProtocol>>,
}

impl Drop for ClipUi {
    fn drop(&mut self) {
        // SAFETY: `token` was returned by this center's addObserverForName.
        unsafe {
            self.center
                .removeObserver(AsRef::<AnyObject>::as_ref(&*self.token))
        };
    }
}

impl ClipUi {
    /// `top_inset` reserves room above the search field (the notch cutout).
    pub fn new(mtm: MainThreadMarker, size: NSSize, top_inset: f64, callbacks: Callbacks) -> Self {
        let callbacks = Rc::new(callbacks);
        let frame = NSRect::new(NSPoint::ZERO, size);
        let root = PageRoot::new(mtm, frame, &labels::get().clipboard_hint);

        let search_top = size.height - top_inset - MARGIN;
        let search = NSSearchField::initWithFrame(
            mtm.alloc(),
            NSRect::new(
                NSPoint::new(MARGIN, search_top - SEARCH_HEIGHT),
                NSSize::new(size.width - 2.0 * MARGIN, SEARCH_HEIGHT),
            ),
        );
        search.setPlaceholderString(Some(&NSString::from_str(&labels::get().search_placeholder)));
        search.setFocusRingType(NSFocusRingType::None);

        let list_height = search_top - SEARCH_HEIGHT - 8.0 - FOOTER_HEIGHT;
        let scroll = NSScrollView::initWithFrame(
            mtm.alloc(),
            NSRect::new(
                NSPoint::new(0.0, FOOTER_HEIGHT),
                NSSize::new(size.width, list_height),
            ),
        );
        scroll.setHasVerticalScroller(true);
        scroll.setAutohidesScrollers(true);
        scroll.setScrollerStyle(NSScrollerStyle::Overlay);
        scroll.setDrawsBackground(false);
        scroll.setBorderType(NSBorderType::NoBorder);
        let list = ListView::new(
            mtm,
            NSRect::new(NSPoint::ZERO, NSSize::new(size.width, list_height)),
        );
        list.set_min_height(list_height);
        scroll.setDocumentView(Some(&list));
        root.addSubview(&scroll);
        root.addSubview(&search);

        let view = ClipView { root, search, list };
        {
            let (view, callbacks) = (view.clone(), Rc::clone(&callbacks));
            view.list
                .clone()
                .set_on_activate(move |id| (callbacks.on_activate)(&view, id, Activate::Normal));
        }

        let (observed, callbacks_for_block) = (view.clone(), Rc::clone(&callbacks));
        let block = RcBlock::new(move |_: NonNull<NSNotification>| {
            (callbacks_for_block.on_query)(&observed, &observed.query());
        });
        let center = NSNotificationCenter::defaultCenter();
        let field: &AnyObject = &view.search;
        // SAFETY: the block is 'static and runs on the main thread, where text editing posts the notification.
        let token = unsafe {
            center.addObserverForName_object_queue_usingBlock(
                Some(NSControlTextDidChangeNotification),
                Some(field),
                None,
                &block,
            )
        };
        Self {
            view,
            callbacks,
            center,
            token,
        }
    }

    /// Makes the search field the first responder of its (key) window.
    pub fn focus_search(&self) {
        if let Some(window) = self.view.root.window() {
            window.makeFirstResponder(Some(&self.view.search));
        }
    }
}

/// The Maccy-style popup: a blurred rounded panel that opens next to the pointer.
pub struct Popup {
    pub panel: Retained<FigPanel>,
    pub ui: ClipUi,
    resign: Retained<ProtocolObject<dyn NSObjectProtocol>>,
}

impl Drop for Popup {
    fn drop(&mut self) {
        // SAFETY: `resign` was returned by the default center's addObserverForName.
        unsafe {
            NSNotificationCenter::defaultCenter()
                .removeObserver(AsRef::<AnyObject>::as_ref(&*self.resign));
        }
    }
}

impl Popup {
    pub fn new(mtm: MainThreadMarker, app: AppCallbacks) -> Self {
        let on_activate = app.on_activate;
        let callbacks = Callbacks {
            on_query: app.on_query,
            // Close first so keyboard focus returns to the app that will receive the paste.
            on_activate: Box::new(move |view, id, mode| {
                view.hide_window();
                on_activate(view, id, mode);
            }),
            on_pin: app.on_pin,
            on_delete: app.on_delete,
            on_close: Box::new(|view| view.hide_window()),
        };
        let frame = NSRect::new(NSPoint::ZERO, POPUP_SIZE);
        let panel = FigPanel::new(mtm, frame);
        panel.setHasShadow(true);

        let effect = NSVisualEffectView::initWithFrame(mtm.alloc(), frame);
        effect.setMaterial(NSVisualEffectMaterial::HUDWindow);
        effect.setBlendingMode(NSVisualEffectBlendingMode::BehindWindow);
        effect.setState(NSVisualEffectState::Active);
        effect.setWantsLayer(true);
        if let Some(layer) = effect.layer() {
            // SAFETY: plain property setters on the view's backing layer.
            unsafe {
                let _: () = msg_send![&*layer, setCornerRadius: 14.0_f64];
                let _: () = msg_send![&*layer, setMasksToBounds: true];
            }
        }
        panel.setContentView(Some(&effect));

        let ui = ClipUi::new(mtm, POPUP_SIZE, 0.0, callbacks);
        effect.addSubview(&ui.view.root);

        {
            let (view, callbacks) = (ui.view.clone(), Rc::clone(&ui.callbacks));
            panel.set_key_handler(move |key| handle_key(&view, &callbacks, key));
        }
        let (view, callbacks) = (ui.view.clone(), Rc::clone(&ui.callbacks));
        let block = RcBlock::new(move |_: NonNull<NSNotification>| (callbacks.on_close)(&view));
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
        Self { panel, ui, resign }
    }

    pub fn is_visible(&self) -> bool {
        self.panel.isVisible()
    }

    /// Opens with the top-left corner near the pointer, kept inside the visible area of its screen.
    pub fn show_at_cursor(&self, mtm: MainThreadMarker) {
        let mouse = NSEvent::mouseLocation();
        let screens = NSScreen::screens(mtm);
        let screen = screens
            .iter()
            .find(|s| {
                let f = s.frame();
                mouse.x >= f.origin.x
                    && mouse.x < f.origin.x + f.size.width
                    && mouse.y >= f.origin.y
                    && mouse.y < f.origin.y + f.size.height
            })
            .or_else(|| screens.firstObject());
        let size = POPUP_SIZE;
        let mut origin = NSPoint::new(mouse.x - 24.0, mouse.y - size.height + 12.0);
        if let Some(screen) = screen {
            let area = screen.visibleFrame();
            origin.x = origin.x.clamp(
                area.origin.x,
                (area.origin.x + area.size.width - size.width).max(area.origin.x),
            );
            origin.y = origin.y.clamp(
                area.origin.y,
                (area.origin.y + area.size.height - size.height).max(area.origin.y),
            );
        }
        self.panel.setFrameOrigin(origin);
        self.panel.makeKeyAndOrderFront(None);
        self.ui.focus_search();
    }
}
