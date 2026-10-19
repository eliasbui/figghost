//! Clipboard list UI: search field, scrolling list with thumbnails, footer hint, and the cursor popup.

use crate::{
    button::{Look, TouchButton},
    labels,
    panel::{FigPanel, Key},
    style::{self, Tone, Weight},
    widgets::{ActionTarget, Fit, PageRoot, draw_text, text_attributes},
};
use block2::RcBlock;
use objc2::{
    AnyThread, DefinedClass, MainThreadMarker, MainThreadOnly, define_class, msg_send,
    rc::Retained,
    runtime::{AnyObject, ProtocolObject},
};
use objc2_app_kit::{
    NSAutoresizingMaskOptions, NSBezierPath, NSBorderType, NSColor, NSCompositingOperation,
    NSControlStateValueOn, NSControlTextDidChangeNotification, NSEvent, NSFocusRingType, NSFont,
    NSFontWeightMedium, NSGraphicsContext, NSImage, NSMenu, NSMenuItem, NSScreen, NSScrollView,
    NSScrollerStyle, NSSearchField, NSTrackingArea, NSTrackingAreaOptions, NSView,
    NSVisualEffectBlendingMode, NSVisualEffectMaterial, NSVisualEffectState, NSVisualEffectView,
    NSWindowDidResignKeyNotification,
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
const CARD_ROW_HEIGHT: f64 = 60.0;
const CARD_THUMB: f64 = 38.0;
const CHIP_HEIGHT: f64 = 26.0;
const CHIP_GAP: f64 = 6.0;
/// What a hidden row shows in place of its text: dots, always the same count so the length is not given away.
const HIDDEN_TEXT: &str = "\u{2022}\u{2022}\u{2022}\u{2022}\u{2022}\u{2022}\u{2022}\u{2022}\u{2022}\u{2022}\u{2022}\u{2022}";

/// A card of the strip: its width, the gap between two, and the room around the first and last.
const TILE_WIDTH: f64 = 132.0;
const TILE_GAP: f64 = 10.0;
const TILE_MARGIN: f64 = 12.0;
/// The room above and below the cards inside the scrolling area.
const TILE_INSET: f64 = 4.0;
const TILE_RADIUS: f64 = 16.0;
/// How many cards a Page Up or Page Down moves over.
const STRIP_PAGE: isize = 4;
/// Where the search field of the strip starts: after the three dots of the notch's header.
const STRIP_SEARCH_X: f64 = 70.0;

/// How the rows look: the flat list of the cursor popup, the vertical cards of the notch, or the notch's strip of
/// cards that scrolls sideways.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ListStyle {
    Plain,
    Cards,
    Strip,
}

/// The words in a search that are filters (`@link`) and the one filter a chip stands for.
const FILTERS: [&str; 6] = ["@pin", "@link", "@code", "@image", "@file", "@text"];

/// A board that has a chip, and the word that searches it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BoardChip {
    pub id: i64,
    pub name: String,
    /// `@` and the board's search word, as it is typed in the search field.
    pub filter: String,
}

fn is_filter_word(word: &str, boards: &[BoardChip]) -> bool {
    FILTERS
        .iter()
        .copied()
        .chain(boards.iter().map(|b| b.filter.as_str()))
        .any(|f| f.eq_ignore_ascii_case(word))
}

/// The first filter in a query, as the chip that is selected: one of the fixed filters or a board's.
pub fn filter_of(query: &str, boards: &[BoardChip]) -> Option<String> {
    query
        .split_whitespace()
        .find(|word| is_filter_word(word, boards))
        .map(|word| {
            FILTERS
                .iter()
                .copied()
                .chain(boards.iter().map(|b| b.filter.as_str()))
                .find(|f| f.eq_ignore_ascii_case(word))
                .unwrap_or(word)
                .to_owned()
        })
}

/// The query with its filters replaced by `filter`, or removed when there is none; the other words stay.
pub fn with_filter(query: &str, filter: Option<&str>, boards: &[BoardChip]) -> String {
    let rest: Vec<&str> = query
        .split_whitespace()
        .filter(|word| !is_filter_word(word, boards))
        .collect();
    match filter {
        Some(filter) if rest.is_empty() => format!("{filter} "),
        Some(filter) => format!("{filter} {}", rest.join(" ")),
        None => rest.join(" "),
    }
}

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

/// What a row is beyond its text. It is supplied next to the rows rather than inside `Row`, so a surface
/// that builds plain rows needs nothing from it.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Mark {
    /// A likely secret: drawn with a lock.
    pub sensitive: bool,
    /// Its text is not shown, and activating the row reveals it first instead of pasting it.
    pub hidden: bool,
    /// The board a pinned item is filed in.
    pub board: Option<i64>,
}

struct ListState {
    rows: Vec<Row>,
    marks: HashMap<i64, Mark>,
    selected: usize,
    min_height: f64,
    /// The width of the area the strip scrolls in; the strip is never narrower.
    min_width: f64,
    empty_text: String,
}

type RowActivate = Rc<dyn Fn(i64)>;
type RowMenu = Rc<dyn Fn(i64, &NSEvent)>;

pub struct ListIvars {
    style: ListStyle,
    state: RefCell<ListState>,
    thumbs: RefCell<HashMap<PathBuf, Option<Retained<NSImage>>>>,
    on_activate: RefCell<Option<RowActivate>>,
    /// Opens the board menu for a row; set by the surface that owns the list.
    on_menu: RefCell<Option<RowMenu>>,
    /// The menu being shown and the targets of its items, kept alive until it closes.
    menu_targets: RefCell<Vec<Retained<ActionTarget>>>,
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

        // The wheel of a mouse only turns up and down: over the strip it moves the cards sideways.
        #[unsafe(method(scrollWheel:))]
        fn scroll_wheel(&self, event: &NSEvent) {
            let (dx, dy) = (event.scrollingDeltaX(), event.scrollingDeltaY());
            if self.is_strip()
                && dy.abs() > dx.abs()
                && let Some(scroll) = self.enclosingScrollView()
            {
                let lines = if event.hasPreciseScrollingDeltas() { 1.0 } else { 12.0 };
                let clip = scroll.contentView();
                let mut origin = clip.bounds().origin;
                let most = (self.frame().size.width - clip.bounds().size.width).max(0.0);
                origin.x = (origin.x - dy * lines).clamp(0.0, most);
                clip.scrollToPoint(origin);
                scroll.reflectScrolledClipView(&clip);
                return;
            }
            // SAFETY: forwards the same selector and argument to the superclass.
            let _: () = unsafe { msg_send![super(self), scrollWheel: event] };
        }

        #[unsafe(method(mouseMoved:))]
        fn mouse_moved(&self, event: &NSEvent) {
            if let Some(row) = self.row_at(event) {
                self.select(row, false);
            }
        }

        #[unsafe(method(rightMouseDown:))]
        fn right_mouse_down(&self, event: &NSEvent) {
            if let Some(row) = self.row_at(event) {
                self.select(row, false);
                let id = self.selected_id();
                let callback = self.ivars().on_menu.borrow().clone();
                if let (Some(id), Some(callback)) = (id, callback) {
                    callback(id, event);
                }
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
    fn new(mtm: MainThreadMarker, frame: NSRect, style: ListStyle) -> Retained<Self> {
        let ivars = ListIvars {
            style,
            state: RefCell::new(ListState {
                rows: Vec::new(),
                marks: HashMap::new(),
                selected: 0,
                min_height: frame.size.height,
                min_width: frame.size.width,
                empty_text: String::new(),
            }),
            thumbs: RefCell::new(HashMap::new()),
            on_activate: RefCell::new(None),
            on_menu: RefCell::new(None),
            menu_targets: RefCell::new(Vec::new()),
        };
        // SAFETY: designated initializer with valid arguments on the main thread.
        let this: Retained<Self> =
            unsafe { msg_send![super(mtm.alloc::<Self>().set_ivars(ivars)), initWithFrame: frame] };
        if style != ListStyle::Strip {
            this.setAutoresizingMask(NSAutoresizingMaskOptions::ViewWidthSizable);
        }
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

    fn row_height(&self) -> f64 {
        match self.ivars().style {
            ListStyle::Plain => ROW_HEIGHT,
            ListStyle::Cards | ListStyle::Strip => CARD_ROW_HEIGHT,
        }
    }

    pub fn is_strip(&self) -> bool {
        self.ivars().style == ListStyle::Strip
    }

    /// How many rows Page Up and Page Down move over.
    fn page(&self) -> isize {
        if self.is_strip() {
            STRIP_PAGE
        } else {
            PAGE_ROWS
        }
    }

    /// The distance from one card of the strip to the next.
    fn tile_step() -> f64 {
        TILE_WIDTH + TILE_GAP
    }

    /// The size of the document: as many rows as there are (down, or across for the strip), and at least what the
    /// scrolling area shows.
    fn document_size(&self, rows: usize, min_width: f64, min_height: f64, width: f64) -> NSSize {
        if self.is_strip() {
            let across = if rows == 0 {
                0.0
            } else {
                2.0 * TILE_MARGIN + rows as f64 * Self::tile_step() - TILE_GAP
            };
            NSSize::new(across.max(min_width), min_height)
        } else {
            NSSize::new(width, (rows as f64 * self.row_height()).max(min_height))
        }
    }

    fn set_on_activate(&self, f: impl Fn(i64) + 'static) {
        *self.ivars().on_activate.borrow_mut() = Some(Rc::new(f));
    }

    /// Replaces the rows, selects the first and scrolls back to the top.
    pub fn set_rows(&self, rows: Vec<Row>, empty_text: &str) {
        self.set_rows_marked(rows, HashMap::new(), empty_text);
    }

    /// `set_rows`, with what is known about some of the rows by their ids.
    pub fn set_rows_marked(&self, rows: Vec<Row>, marks: HashMap<i64, Mark>, empty_text: &str) {
        let width = self.frame().size.width;
        {
            let mut state = self.ivars().state.borrow_mut();
            let size = self.document_size(rows.len(), state.min_width, state.min_height, width);
            self.setFrameSize(size);
            state.rows = rows;
            state.marks = marks;
            state.selected = 0;
            state.empty_text = empty_text.to_owned();
        }
        self.scrollPoint(NSPoint::ZERO);
        self.setNeedsDisplay(true);
    }

    /// Tells the list how big the area it scrolls in is: the strip is as wide as that, or wider.
    pub fn set_viewport(&self, width: f64, height: f64) {
        let (rows, min_width) = {
            let mut state = self.ivars().state.borrow_mut();
            state.min_height = height;
            if self.is_strip() {
                state.min_width = width;
            }
            (state.rows.len(), state.min_width)
        };
        let size = self.document_size(rows, min_width, height, self.frame().size.width);
        self.setFrameSize(size);
        self.setNeedsDisplay(true);
    }

    pub fn mark_of(&self, id: i64) -> Mark {
        self.ivars()
            .state
            .borrow()
            .marks
            .get(&id)
            .copied()
            .unwrap_or_default()
    }

    /// Shows the text of a hidden row, until the rows are replaced. Activating it again pastes it.
    pub fn reveal(&self, id: i64, text: &str) {
        {
            let mut state = self.ivars().state.borrow_mut();
            if let Some(row) = state.rows.iter_mut().find(|r| r.id == id) {
                row.title = text.to_owned();
            }
            if let Some(mark) = state.marks.get_mut(&id) {
                mark.hidden = false;
            }
        }
        self.setNeedsDisplay(true);
    }

    fn set_on_menu(&self, f: impl Fn(i64, &NSEvent) + 'static) {
        *self.ivars().on_menu.borrow_mut() = Some(Rc::new(f));
    }

    /// Shows a menu at the pointer; the targets of its items are kept until the next one.
    fn pop_up(&self, menu: &NSMenu, targets: Vec<Retained<ActionTarget>>, event: &NSEvent) {
        *self.ivars().menu_targets.borrow_mut() = targets;
        NSMenu::popUpContextMenu_withEvent_forView(menu, event, self);
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

    pub fn id_at(&self, index: usize) -> Option<i64> {
        self.ivars().state.borrow().rows.get(index).map(|r| r.id)
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
            let rect = if self.is_strip() {
                NSRect::new(
                    NSPoint::new(TILE_MARGIN + row as f64 * Self::tile_step() - TILE_GAP, 0.0),
                    NSSize::new(TILE_WIDTH + 2.0 * TILE_GAP, self.bounds().size.height),
                )
            } else {
                NSRect::new(
                    NSPoint::new(0.0, row as f64 * self.row_height()),
                    NSSize::new(self.bounds().size.width, self.row_height()),
                )
            };
            self.scrollRectToVisible(rect);
        }
        self.setNeedsDisplay(true);
    }

    fn row_at(&self, event: &NSEvent) -> Option<usize> {
        let point = self.convertPoint_fromView(event.locationInWindow(), None);
        if self.is_strip() {
            let across = point.x - TILE_MARGIN;
            if across < 0.0 || point.y < 0.0 {
                return None;
            }
            // The gap between two cards belongs to neither.
            let (row, within) = (
                (across / Self::tile_step()) as usize,
                across % Self::tile_step(),
            );
            return (within <= TILE_WIDTH && row < self.ivars().state.borrow().rows.len())
                .then_some(row);
        }
        if point.y < 0.0 {
            return None;
        }
        let row = (point.y / self.row_height()) as usize;
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
            let mut y = state.min_height / 2.0 - 10.0;
            if self.ivars().style != ListStyle::Plain {
                let centre = NSPoint::new(width / 2.0, state.min_height / 2.0 - 14.0);
                style::draw_symbol("doc.on.clipboard", centre, 30.0, &style::white(0.32));
                y += 30.0;
            }
            let rect = NSRect::new(NSPoint::new(0.0, y), NSSize::new(width, 20.0));
            draw_text(&state.empty_text, rect, &attrs);
            return;
        }
        let (first, last) = if self.is_strip() {
            let step = Self::tile_step();
            (
                ((dirty.origin.x - TILE_MARGIN) / step).floor().max(0.0) as usize,
                ((((dirty.origin.x + dirty.size.width - TILE_MARGIN) / step).ceil()) as usize)
                    .min(state.rows.len()),
            )
        } else {
            let height = self.row_height();
            (
                (dirty.origin.y / height).floor().max(0.0) as usize,
                (((dirty.origin.y + dirty.size.height) / height).ceil() as usize)
                    .min(state.rows.len()),
            )
        };
        for index in first..last {
            let row = &state.rows[index];
            let mark = state.marks.get(&row.id).copied().unwrap_or_default();
            self.paint_row(row, mark, index, index == state.selected, width);
        }
    }

    fn paint_row(&self, row: &Row, mark: Mark, index: usize, selected: bool, width: f64) {
        match self.ivars().style {
            ListStyle::Plain => self.paint_plain_row(row, mark, index, selected, width),
            ListStyle::Cards => self.paint_card(row, mark, index, selected, width),
            ListStyle::Strip => self.paint_tile(row, mark, index, selected),
        }
    }

    /// One card of the strip: a picture or the first lines of the text, who copied it and when under it, and a pin
    /// or the letter that pastes it.
    fn paint_tile(&self, row: &Row, mark: Mark, index: usize, selected: bool) {
        let card = NSRect::new(
            NSPoint::new(TILE_MARGIN + index as f64 * Self::tile_step(), TILE_INSET),
            NSSize::new(TILE_WIDTH, self.bounds().size.height - 2.0 * TILE_INSET),
        );
        if selected {
            style::card_selected(card, TILE_RADIUS, &Tone::Blue);
        } else {
            style::card(card, TILE_RADIUS);
        }
        let pad = 10.0;
        let inner = card.size.width - 2.0 * pad;
        let footer_top = card.origin.y + card.size.height - 25.0;
        let ink = |alpha: f64, selected_alpha: f64| {
            style::white(if selected { selected_alpha } else { alpha })
        };

        // The footer: a lock for a secret, who and when, and at the right a pin or the letter that pastes it.
        let mut left = card.origin.x + pad;
        if mark.sensitive {
            style::draw_symbol(
                "lock.fill",
                NSPoint::new(left + 4.0, footer_top + 7.0),
                9.0,
                &ink(0.6, 0.85),
            );
            left += 13.0;
        }
        let mut right = card.origin.x + card.size.width - pad;
        if let Some(letter) = row.pin_key {
            let chip = NSRect::new(
                NSPoint::new(right - 30.0, footer_top - 1.0),
                NSSize::new(30.0, 16.0),
            );
            style::control(chip, 8.0, style::Control::Rest);
            draw_text(
                &format!("\u{2318}{}", letter.to_ascii_uppercase()),
                NSRect::new(
                    NSPoint::new(chip.origin.x, chip.origin.y + 1.5),
                    NSSize::new(30.0, 13.0),
                ),
                &text_attributes(
                    // SAFETY: extern static weight constant.
                    &NSFont::monospacedSystemFontOfSize_weight(10.0, unsafe { NSFontWeightMedium }),
                    &ink(0.7, 0.9),
                    Fit::Clip,
                    true,
                ),
            );
            right -= 34.0;
        } else if row.pinned {
            style::draw_symbol(
                "pin.fill",
                NSPoint::new(right - 5.0, footer_top + 7.0),
                9.0,
                &Tone::Orange.color(),
            );
            right -= 14.0;
        }
        draw_text(
            &row.subtitle,
            NSRect::new(
                NSPoint::new(left, footer_top),
                NSSize::new((right - left).max(10.0), 14.0),
            ),
            &text_attributes(
                &style::font(10.0, Weight::Regular),
                &ink(0.5, 0.78),
                Fit::Tail,
                false,
            ),
        );

        // The body: a picture with its name under it, or the text.
        let top = card.origin.y + pad;
        let bottom = footer_top - 6.0;
        let picture = row
            .thumb
            .as_ref()
            .filter(|_| !mark.hidden)
            .and_then(|p| self.thumbnail(p));
        if let Some(image) = picture {
            let height = ((bottom - top) * 0.74).floor();
            let dst = NSRect::new(
                NSPoint::new(card.origin.x + pad, top),
                NSSize::new(inner, height),
            );
            let natural = image.size();
            // The picture fills its frame and loses what overflows, like a cover.
            let scale = (dst.size.width / natural.width.max(1.0))
                .max(dst.size.height / natural.height.max(1.0));
            let (sw, sh) = (dst.size.width / scale, dst.size.height / scale);
            let source = NSRect::new(
                NSPoint::new((natural.width - sw) / 2.0, (natural.height - sh) / 2.0),
                NSSize::new(sw, sh),
            );
            NSGraphicsContext::saveGraphicsState_class();
            NSBezierPath::bezierPathWithRoundedRect_xRadius_yRadius(dst, 10.0, 10.0).addClip();
            // Flipped view: plain drawInRect would render the image upside down.
            // SAFETY: called from drawRect: with a valid graphics context.
            unsafe {
                image.drawInRect_fromRect_operation_fraction_respectFlipped_hints(
                    dst,
                    source,
                    NSCompositingOperation::SourceOver,
                    1.0,
                    true,
                    None,
                );
            }
            NSGraphicsContext::restoreGraphicsState_class();
            draw_text(
                &row.title,
                NSRect::new(
                    NSPoint::new(dst.origin.x, dst.origin.y + height + 4.0),
                    NSSize::new(inner, (bottom - top - height - 4.0).max(12.0)),
                ),
                &text_attributes(
                    &style::font(10.5, Weight::Medium),
                    &ink(0.85, 1.0),
                    Fit::Tail,
                    false,
                ),
            );
        } else {
            let font = if row.mono && !mark.hidden {
                NSFont::userFixedPitchFontOfSize(11.0)
                    .unwrap_or_else(|| NSFont::systemFontOfSize(12.0))
            } else {
                style::font(12.0, Weight::Medium)
            };
            draw_text(
                if mark.hidden { HIDDEN_TEXT } else { &row.title },
                NSRect::new(
                    NSPoint::new(card.origin.x + pad, top),
                    NSSize::new(inner, bottom - top),
                ),
                &text_attributes(&font, &ink(0.92, 1.0), Fit::Words, false),
            );
        }
    }

    /// One row as a rounded card: a pin on the left, the picture or the text, the shortcut letter on the right.
    fn paint_card(&self, row: &Row, mark: Mark, index: usize, selected: bool, width: f64) {
        let top = index as f64 * CARD_ROW_HEIGHT;
        let card = NSRect::new(
            NSPoint::new(10.0, top + 3.0),
            NSSize::new(width - 20.0, CARD_ROW_HEIGHT - 6.0),
        );
        if selected {
            style::card_selected(card, 16.0, &Tone::Blue);
        } else {
            style::card(card, 16.0);
        }
        let middle = top + CARD_ROW_HEIGHT / 2.0;
        let mut left = card.origin.x + 14.0;
        if row.pinned {
            style::draw_symbol(
                "pin.fill",
                NSPoint::new(left + 5.0, middle),
                11.0,
                &Tone::Orange.color(),
            );
            left += 20.0;
        }
        if mark.sensitive {
            style::draw_symbol(
                "lock.fill",
                NSPoint::new(left + 5.0, middle),
                12.0,
                &style::white(0.7),
            );
            left += 20.0;
        }
        if let Some(image) = row.thumb.as_ref().and_then(|p| self.thumbnail(p)) {
            let size = image.size();
            let scale = (CARD_THUMB / size.width.max(1.0)).min(CARD_THUMB / size.height.max(1.0));
            let (w, h) = (size.width * scale, size.height * scale);
            let dst = NSRect::new(
                NSPoint::new(left + (CARD_THUMB - w) / 2.0, middle - h / 2.0),
                NSSize::new(w, h),
            );
            NSGraphicsContext::saveGraphicsState_class();
            NSBezierPath::bezierPathWithRoundedRect_xRadius_yRadius(dst, 8.0, 8.0).addClip();
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
            NSGraphicsContext::restoreGraphicsState_class();
            left += CARD_THUMB + 12.0;
        }
        let mut right = card.origin.x + card.size.width - 14.0;
        if let Some(letter) = row.pin_key {
            let label = format!("\u{2318}{}", letter.to_ascii_uppercase());
            let key = NSRect::new(
                NSPoint::new(right - 36.0, middle - 10.0),
                NSSize::new(36.0, 20.0),
            );
            style::control(key, 10.0, style::Control::Rest);
            draw_text(
                &label,
                NSRect::new(
                    NSPoint::new(key.origin.x, key.origin.y + 3.0),
                    NSSize::new(36.0, 14.0),
                ),
                &text_attributes(
                    &NSFont::monospacedSystemFontOfSize_weight(11.0, unsafe { NSFontWeightMedium }),
                    &style::white(0.7),
                    Fit::Clip,
                    true,
                ),
            );
            right -= 36.0 + 10.0;
        }
        let text_width = (right - left).max(10.0);
        let title_font = if row.mono && !mark.hidden {
            NSFont::userFixedPitchFontOfSize(12.0).unwrap_or_else(|| NSFont::systemFontOfSize(13.0))
        } else {
            style::font(13.0, Weight::Medium)
        };
        draw_text(
            if mark.hidden { HIDDEN_TEXT } else { &row.title },
            NSRect::new(
                NSPoint::new(left, top + 12.0),
                NSSize::new(text_width, 18.0),
            ),
            &text_attributes(&title_font, &style::white(0.95), Fit::Tail, false),
        );
        draw_text(
            &row.subtitle,
            NSRect::new(
                NSPoint::new(left, top + 32.0),
                NSSize::new(text_width, 14.0),
            ),
            &text_attributes(
                &style::font(11.0, Weight::Regular),
                &style::white(0.52),
                Fit::Tail,
                false,
            ),
        );
    }

    fn paint_plain_row(&self, row: &Row, mark: Mark, index: usize, selected: bool, width: f64) {
        let top = index as f64 * ROW_HEIGHT;
        if selected {
            let rect = NSRect::new(
                NSPoint::new(6.0, top + 2.0),
                NSSize::new(width - 12.0, ROW_HEIGHT - 4.0),
            );
            style::card_selected_color(rect, 10.0, &NSColor::selectedContentBackgroundColor());
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
        if mark.sensitive {
            let tone = if selected { 0.9 } else { 0.6 };
            style::draw_symbol(
                "lock.fill",
                NSPoint::new(left + 5.0, top + ROW_HEIGHT / 2.0),
                12.0,
                &style::white(tone),
            );
            left += 20.0;
        }
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
        let title_font = if row.mono && !mark.hidden {
            NSFont::userFixedPitchFontOfSize(12.0).unwrap_or_else(|| NSFont::systemFontOfSize(13.0))
        } else {
            NSFont::systemFontOfSize(13.0)
        };
        // Command+1..9 paste the first nine rows; a pinned row shows its own letter instead.
        let key_label = row
            .pin_key
            .map(|c| format!("\u{2318}{}", c.to_ascii_uppercase()))
            .or_else(|| (index < 9).then(|| format!("\u{2318}{}", index + 1)));
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
            if mark.hidden { HIDDEN_TEXT } else { &row.title },
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
    scroll: Retained<NSScrollView>,
    mtm: MainThreadMarker,
    geometry: Rc<Geometry>,
    /// The filter chips (empty on the cursor popup) and then the board chips.
    chips: Rc<RefCell<Vec<Chip>>>,
    boards: Rc<RefCell<Vec<BoardChip>>>,
    extras: Rc<RefCell<Option<Rc<Extras>>>>,
    callbacks: Rc<Callbacks>,
}

struct Chip {
    /// The filter the chip stands for; `None` is "All".
    filter: Option<String>,
    button: Retained<TouchButton>,
    board: bool,
}

/// Where things sit in the surface, from its size.
struct Geometry {
    size: NSSize,
    search_top: f64,
}

/// What the application adds after the surface is built: what to do with a hidden row and with a board,
/// and the words of the board menu.
pub struct Extras {
    /// A hidden row was activated: show its text so the next activation can paste it.
    pub on_reveal: ItemHandler,
    /// File an item in a board, or take it out of its board (`None`).
    pub on_board: BoardHandler,
    pub menu_title: String,
    pub no_board_title: String,
}

impl ClipView {
    pub fn query(&self) -> String {
        self.search.stringValue().to_string()
    }

    pub fn clear_query(&self) {
        self.search.setStringValue(&NSString::from_str(""));
        self.sync_chips();
    }

    /// Selects the chip of the filter that is in the search text, or "All" when there is none.
    pub fn sync_chips(&self) {
        let current = filter_of(&self.query(), &self.boards.borrow());
        for chip in self.chips.borrow().iter() {
            chip.button
                .set_selected(chip.filter.as_deref() == current.as_deref());
        }
    }

    pub fn set_rows(&self, rows: Vec<Row>, empty_text: &str) {
        self.list.set_rows(rows, empty_text);
    }

    /// `set_rows`, with what is known about some of the rows by their ids.
    pub fn set_rows_marked(&self, rows: Vec<Row>, marks: HashMap<i64, Mark>, empty_text: &str) {
        self.list.set_rows_marked(rows, marks, empty_text);
    }

    pub fn set_extras(&self, extras: Extras) {
        *self.extras.borrow_mut() = Some(Rc::new(extras));
    }

    /// Replaces the board chips, which sit below the filter chips (or alone, on the cursor popup).
    pub fn set_boards(&self, boards: Vec<BoardChip>) {
        if *self.boards.borrow() == boards {
            return;
        }
        {
            let mut chips = self.chips.borrow_mut();
            chips.retain(|chip| {
                if chip.board {
                    chip.button.removeFromSuperview();
                }
                !chip.board
            });
            for board in &boards {
                let button = TouchButton::new(self.mtm, NSRect::ZERO, Look::Chip);
                button.set_title(&chip_title(&board.name));
                self.root.addSubview(&button);
                let (view, filter) = (self.clone(), board.filter.clone());
                button.set_on_click(move || view.choose_filter(Some(&filter)));
                chips.push(Chip {
                    filter: Some(board.filter.clone()),
                    button,
                    board: true,
                });
            }
        }
        *self.boards.borrow_mut() = boards;
        self.relayout();
        self.sync_chips();
    }

    fn choose_filter(&self, filter: Option<&str>) {
        let text = with_filter(&self.query(), filter, &self.boards.borrow());
        self.search.setStringValue(&NSString::from_str(&text));
        self.sync_chips();
        (self.callbacks.on_query)(self, &text);
    }

    /// Places the chips and sizes the list below them. The filter chips share one row, shrinking their gaps
    /// to fit; board chips wrap onto rows of their own.
    fn relayout(&self) {
        let chips = self.chips.borrow();
        let room = self.geometry.size.width - 2.0 * MARGIN;
        let top = self.geometry.search_top - SEARCH_HEIGHT - 8.0;
        let mut rows = 0usize;
        let place = |chip: &Chip, x: f64, row: usize, width: f64| {
            let y = top - CHIP_HEIGHT - row as f64 * (CHIP_HEIGHT + CHIP_GAP);
            chip.button.setFrame(NSRect::new(
                NSPoint::new(x, y),
                NSSize::new(width, CHIP_HEIGHT),
            ));
        };

        let filters: Vec<&Chip> = chips.iter().filter(|c| !c.board).collect();
        if !filters.is_empty() {
            let widths: Vec<f64> = filters.iter().map(|c| c.button.fit_width()).collect();
            let gap = CHIP_GAP
                .min(((room - widths.iter().sum::<f64>()) / (widths.len() as f64 - 1.0)).max(2.0));
            let mut x = MARGIN;
            for (chip, width) in filters.iter().zip(&widths) {
                place(chip, x, 0, *width);
                x += width + gap;
            }
            rows = 1;
        }

        let mut x = MARGIN;
        let mut started = false;
        for chip in chips.iter().filter(|c| c.board) {
            let width = chip.button.fit_width().min(room);
            if started && x + width > MARGIN + room {
                rows += 1;
                x = MARGIN;
            } else if !started {
                started = true;
                // Boards start a row of their own below the filters.
                rows += 1;
            }
            place(chip, x, rows - 1, width);
            x += width + CHIP_GAP;
        }

        let block = if rows == 0 {
            0.0
        } else {
            rows as f64 * CHIP_HEIGHT + (rows - 1) as f64 * CHIP_GAP + 10.0
        };
        let list_height = top - block - FOOTER_HEIGHT;
        self.scroll.setFrame(NSRect::new(
            NSPoint::new(0.0, FOOTER_HEIGHT),
            NSSize::new(self.geometry.size.width, list_height),
        ));
        self.list
            .set_viewport(self.geometry.size.width, list_height);
    }

    /// The board after `current` in the order they are shown; after the last comes none.
    fn next_board(&self, current: Option<i64>) -> Option<Option<i64>> {
        let boards = self.boards.borrow();
        let first = boards.first()?;
        Some(
            match current.and_then(|c| boards.iter().position(|b| b.id == c)) {
                None => Some(first.id),
                Some(at) => boards.get(at + 1).map(|b| b.id),
            },
        )
    }

    /// The menu for moving a row to a board, and the targets its items need kept alive.
    pub fn board_menu(&self, id: i64) -> Option<(Retained<NSMenu>, Vec<Retained<ActionTarget>>)> {
        let extras = self.extras.borrow().clone()?;
        let boards = self.boards.borrow().clone();
        if boards.is_empty() {
            return None;
        }
        let current = self.list.mark_of(id).board;
        let menu = NSMenu::initWithTitle(
            NSMenu::alloc(self.mtm),
            &NSString::from_str(&extras.menu_title),
        );
        let header = NSMenuItem::new(self.mtm);
        header.setTitle(&NSString::from_str(&extras.menu_title));
        header.setEnabled(false);
        menu.addItem(&header);
        let mut targets = Vec::new();
        let choices = std::iter::once((None, extras.no_board_title.clone()))
            .chain(boards.iter().map(|b| (Some(b.id), b.name.clone())));
        for (target_board, title) in choices {
            let (view, extras) = (self.clone(), Rc::clone(&extras));
            let target = ActionTarget::new(self.mtm, move || {
                (extras.on_board)(&view, id, target_board);
            });
            // SAFETY: `fire:` is implemented by ActionTarget; the caller keeps the target alive with the menu.
            let item = unsafe {
                NSMenuItem::initWithTitle_action_keyEquivalent(
                    NSMenuItem::alloc(self.mtm),
                    &NSString::from_str(&title),
                    Some(objc2::sel!(fire:)),
                    &NSString::from_str(""),
                )
            };
            // SAFETY: as above.
            unsafe { item.setTarget(Some(&target)) };
            if target_board == current {
                item.setState(NSControlStateValueOn);
            }
            menu.addItem(&item);
            targets.push(target);
        }
        Some((menu, targets))
    }

    /// Activates a row: a hidden one is shown first, and only a shown one is pasted.
    fn activate_row(&self, id: i64, mode: Activate) {
        if self.list.mark_of(id).hidden {
            let extras = self.extras.borrow().clone();
            if let Some(extras) = extras {
                (extras.on_reveal)(self, id);
            }
            return;
        }
        (self.callbacks.on_activate)(self, id, mode);
    }

    /// Hides the window this surface lives in.
    pub fn hide_window(&self) {
        if let Some(window) = self.root.window() {
            window.orderOut(None);
        }
    }
}

/// A chip shows the start of a long name.
fn chip_title(name: &str) -> String {
    const SHOWN: usize = 14;
    if name.chars().count() <= SHOWN {
        name.to_owned()
    } else {
        format!("{}...", name.chars().take(SHOWN - 1).collect::<String>())
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
/// An item and the board it goes to, or `None` for out of its board.
pub type BoardHandler = Box<dyn Fn(&ClipView, i64, Option<i64>)>;
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
        Key::PageUp => view.list.move_selection(-view.list.page()),
        Key::PageDown => view.list.move_selection(view.list.page()),
        Key::Escape => (callbacks.on_close)(view),
        Key::Enter | Key::EnterAlt => {
            let mode = if key == Key::EnterAlt {
                Activate::Alternate
            } else {
                Activate::Normal
            };
            if let Some(id) = view.list.selected_id() {
                view.activate_row(id, mode);
            }
        }
        Key::Shortcut(letter) => {
            // A letter nothing is pinned to is an ordinary shortcut (Command+A, Command+C) for the text field.
            let Some(id) = view.list.id_for_pin_key(letter) else {
                return false;
            };
            view.activate_row(id, Activate::Normal);
        }
        Key::Pin => {
            if let Some(id) = view.list.selected_id() {
                (callbacks.on_pin)(view, id);
            }
        }
        Key::Board => {
            let extras = view.extras.borrow().clone();
            if let (Some(id), Some(extras)) = (view.list.selected_id(), extras)
                && let Some(next) = view.next_board(view.list.mark_of(id).board)
            {
                (extras.on_board)(view, id, next);
            }
        }
        Key::Delete => {
            if let Some(id) = view.list.selected_id() {
                (callbacks.on_delete)(view, id);
            }
        }
        // The notch takes Command+digit for its pages before a key reaches here, so this is the popup's.
        Key::Tab(n) => {
            let Some(id) = view.list.id_at(usize::from(n).saturating_sub(1)) else {
                return false;
            };
            view.activate_row(id, Activate::Normal);
        }
        // Along the strip the arrows move from card to card while there is nothing typed to move a caret in.
        Key::Left if view.list.is_strip() && view.query().is_empty() => {
            view.list.move_selection(-1)
        }
        Key::Right if view.list.is_strip() && view.query().is_empty() => {
            view.list.move_selection(1)
        }
        // Caret movement belongs to the search field.
        Key::Left | Key::Right => return false,
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
        Self::with_style(mtm, size, top_inset, callbacks, ListStyle::Plain)
    }

    /// The same surface with the look of the notch (`Cards`) or of the cursor popup (`Plain`).
    pub fn with_style(
        mtm: MainThreadMarker,
        size: NSSize,
        top_inset: f64,
        callbacks: Callbacks,
        list_style: ListStyle,
    ) -> Self {
        let callbacks = Rc::new(callbacks);
        let frame = NSRect::new(NSPoint::ZERO, size);
        let hint = if list_style == ListStyle::Strip {
            &labels::get().clipboard_hint_strip
        } else {
            &labels::get().clipboard_hint
        };
        let root = PageRoot::new(mtm, frame, hint);

        let strip = list_style == ListStyle::Strip;
        // The strip's search field shares the header row of the notch: after the three dots, before the page icons.
        let search_top = if strip {
            size.height - top_inset - 2.0
        } else {
            size.height - top_inset - MARGIN
        };
        let search_frame = if strip {
            let tools = 16.0 + crate::switcher::width_for(8) + 8.0;
            NSRect::new(
                NSPoint::new(STRIP_SEARCH_X, search_top - SEARCH_HEIGHT),
                NSSize::new(size.width - STRIP_SEARCH_X - tools, SEARCH_HEIGHT),
            )
        } else {
            NSRect::new(
                NSPoint::new(MARGIN, search_top - SEARCH_HEIGHT),
                NSSize::new(size.width - 2.0 * MARGIN, SEARCH_HEIGHT),
            )
        };
        let search = NSSearchField::initWithFrame(mtm.alloc(), search_frame);
        let placeholder = if list_style != ListStyle::Plain {
            &labels::get().search_placeholder_short
        } else {
            &labels::get().search_placeholder
        };
        search.setPlaceholderString(Some(&NSString::from_str(placeholder)));
        search.setFocusRingType(NSFocusRingType::None);

        let cards = list_style != ListStyle::Plain;
        let chips: Vec<Chip> = if cards {
            let t = labels::get();
            [
                (None, &t.chip_all),
                (Some("@pin"), &t.chip_pinned),
                (Some("@link"), &t.chip_links),
                (Some("@code"), &t.chip_code),
                (Some("@image"), &t.chip_images),
                (Some("@file"), &t.chip_files),
                (Some("@text"), &t.chip_text),
            ]
            .into_iter()
            .map(|(filter, title)| {
                let button = TouchButton::new(mtm, NSRect::ZERO, Look::Chip);
                button.set_title(title);
                root.addSubview(&button);
                Chip {
                    filter: filter.map(str::to_owned),
                    button,
                    board: false,
                }
            })
            .collect()
        } else {
            Vec::new()
        };

        let scroll = NSScrollView::initWithFrame(mtm.alloc(), NSRect::ZERO);
        scroll.setHasVerticalScroller(!strip);
        // The strip has no scroller: with the legacy ones (a mouse is connected) it would take a strip of the cards'
        // height. It still scrolls, with a trackpad, the wheel (see `scrollWheel:`) and the arrow keys.
        scroll.setHasHorizontalScroller(false);
        scroll.setAutohidesScrollers(true);
        scroll.setScrollerStyle(NSScrollerStyle::Overlay);
        scroll.setDrawsBackground(false);
        scroll.setBorderType(NSBorderType::NoBorder);
        let list = ListView::new(mtm, NSRect::ZERO, list_style);
        scroll.setDocumentView(Some(&list));
        root.addSubview(&scroll);
        root.addSubview(&search);

        let view = ClipView {
            root,
            search,
            list,
            scroll,
            mtm,
            geometry: Rc::new(Geometry { size, search_top }),
            chips: Rc::new(RefCell::new(chips)),
            boards: Rc::default(),
            extras: Rc::default(),
            callbacks: Rc::clone(&callbacks),
        };
        view.relayout();
        let list_frame = view.scroll.frame().size;
        view.list.setFrameSize(list_frame);
        view.sync_chips();
        for chip in view.chips.borrow().iter().filter(|c| !c.board) {
            let (view, filter) = (view.clone(), chip.filter.clone());
            chip.button
                .set_on_click(move || view.choose_filter(filter.as_deref()));
        }
        {
            let view = view.clone();
            view.list
                .clone()
                .set_on_activate(move |id| view.activate_row(id, Activate::Normal));
        }
        {
            let view = view.clone();
            view.list.clone().set_on_menu(move |id, event| {
                if let Some((menu, targets)) = view.board_menu(id) {
                    view.list.pop_up(&menu, targets, event);
                }
            });
        }

        let (observed, callbacks_for_block) = (view.clone(), Rc::clone(&callbacks));
        let block = RcBlock::new(move |_: NonNull<NSNotification>| {
            observed.sync_chips();
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
        self.panel
            .setFrameOrigin(origin_near_pointer(mtm, POPUP_SIZE));
        self.panel.makeKeyAndOrderFront(None);
        self.ui.focus_search();
    }
}

/// The bottom-left corner for a window of `size` that opens with its top-left corner just beside the pointer,
/// kept inside the visible area of the screen the pointer is on. With `FIGGHOST_NOTCH_OFFSCREEN` set (for
/// tests that must not put anything on the person's screen) it is far outside every screen.
pub(crate) fn origin_near_pointer(mtm: MainThreadMarker, size: NSSize) -> NSPoint {
    if std::env::var_os("FIGGHOST_NOTCH_OFFSCREEN").is_some() {
        return NSPoint::new(-30_000.0, 200.0);
    }
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
    origin
}

#[cfg(test)]
mod tests {
    use super::*;

    fn board(id: i64, name: &str) -> BoardChip {
        BoardChip {
            id,
            name: name.into(),
            filter: format!("@{}", name.to_lowercase()),
        }
    }

    fn filter(query: &str) -> Option<String> {
        filter_of(query, &[])
    }

    #[test]
    fn the_filter_in_a_query_is_found_wherever_it_is() {
        assert_eq!(filter("@link").as_deref(), Some("@link"));
        assert_eq!(filter("hello @code world").as_deref(), Some("@code"));
        assert_eq!(filter("@IMAGE logo").as_deref(), Some("@image"));
        assert_eq!(filter("hello"), None);
        assert_eq!(filter("@unknown thing"), None);
        assert_eq!(filter(""), None);
    }

    #[test]
    fn choosing_a_chip_replaces_the_filter_and_keeps_the_words() {
        let with = |query, chosen| with_filter(query, chosen, &[]);
        assert_eq!(with("", Some("@link")), "@link ");
        assert_eq!(with("hello world", Some("@code")), "@code hello world");
        assert_eq!(with("@link hello", Some("@code")), "@code hello");
        assert_eq!(
            with("hello @pin world", Some("@image")),
            "@image hello world"
        );
        assert_eq!(with("@link hello", None), "hello");
        assert_eq!(with("@link", None), "");
        // Two filters collapse to the one that was chosen.
        assert_eq!(with("@link @code x", Some("@file")), "@file x");
    }

    #[test]
    fn a_board_is_a_filter_like_the_others() {
        let boards = [board(1, "Work"), board(2, "Snippets")];
        assert_eq!(filter_of("@work", &boards).as_deref(), Some("@work"));
        assert_eq!(filter_of("notes @WORK", &boards).as_deref(), Some("@work"));
        assert_eq!(filter_of("@work", &[]), None, "no chip, no filter");
        assert_eq!(
            filter_of("@snippets x", &boards).as_deref(),
            Some("@snippets")
        );
        // Choosing a board replaces any filter, and choosing a filter replaces the board.
        assert_eq!(
            with_filter("@link hello", Some("@work"), &boards),
            "@work hello"
        );
        assert_eq!(
            with_filter("@work hello", Some("@code"), &boards),
            "@code hello"
        );
        assert_eq!(with_filter("@work hello", None, &boards), "hello");
        assert_eq!(
            with_filter("@work", Some("@snippets"), &boards),
            "@snippets "
        );
    }

    #[test]
    fn long_board_names_are_shortened_on_their_chip() {
        assert_eq!(chip_title("Work"), "Work");
        assert_eq!(chip_title("exactly fourteen"), "exactly fourt...");
        assert_eq!(
            chip_title("Công việc hằng ngày của tôi"),
            "Công việc hằn..."
        );
        assert_eq!(chip_title("12345678901234"), "12345678901234");
    }
}
