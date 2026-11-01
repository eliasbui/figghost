//! The Commands tab of the notch: the commands the person saved in a list at the left, and at the right what the one that
//! runs prints, scrolling, with a button to stop it and one to clear the text. Everything on it is plain data from the app,
//! so nothing here runs a program or knows a language.

use crate::{
    button::{Look, TouchButton},
    notch::Page,
    style::{self, Weight, white},
    widgets::{Fit, draw_page_note, draw_text, text_attributes},
};
use objc2::{DefinedClass, MainThreadMarker, MainThreadOnly, define_class, msg_send, rc::Retained};
use objc2_app_kit::{
    NSAutoresizingMaskOptions, NSBorderType, NSFont, NSScrollView, NSScrollerStyle, NSTextField,
    NSTextView, NSView,
};
use objc2_foundation::{NSPoint, NSRange, NSRect, NSSize, NSString};
use std::{cell::RefCell, rc::Rc};

/// The size of the area the tab fills below the header.
pub const PAGE_SIZE: NSSize = NSSize::new(580.0, 270.0);
/// The most commands the list has room for.
pub const MAX_ROWS: usize = 8;

const MARGIN: f64 = 18.0;
const LIST_WIDTH: f64 = 178.0;
const GAP: f64 = 12.0;
const ROW_HEIGHT: f64 = 42.0;
const ROW_GAP: f64 = 2.0;
const CAPTION_HEIGHT: f64 = 30.0;
const FOOTER_HEIGHT: f64 = 30.0;
const RADIUS: f64 = 14.0;

#[derive(Debug, Clone)]
pub struct CommandRow {
    pub title: String,
    pub detail: String,
    /// The command whose output is shown.
    pub selected: bool,
}

#[derive(Debug, Clone, Default)]
pub struct CommandsDisplay {
    pub caption: String,
    pub rows: Vec<CommandRow>,
    /// The line under the output: that it runs, or how it ended.
    pub footer: String,
    pub stop_label: String,
    pub clear_label: String,
    /// A command runs now: its button to stop is on.
    pub running: bool,
    /// Grey words in an empty output.
    pub placeholder: String,
    /// Words that replace everything: the tab is off, or there is no command yet.
    pub note: Option<(String, String)>,
}

pub struct CommandsCallbacks {
    /// The command at this place in the list was pressed.
    pub on_run: Box<dyn Fn(usize)>,
    pub on_stop: Box<dyn Fn()>,
    pub on_clear: Box<dyn Fn()>,
}

struct Layout {
    caption: NSRect,
    list: NSRect,
    card: NSRect,
    text: NSRect,
    status: NSRect,
    stop: NSRect,
    clear: NSRect,
}

fn layout(size: NSSize) -> Layout {
    let (w, h) = (size.width, size.height);
    let card_x = MARGIN + LIST_WIDTH + GAP;
    let card = NSRect::new(
        NSPoint::new(card_x, 14.0),
        NSSize::new(w - MARGIN - card_x, h - 14.0 - 6.0),
    );
    Layout {
        caption: NSRect::new(
            NSPoint::new(MARGIN + 4.0, h - 6.0 - 22.0),
            NSSize::new(LIST_WIDTH - 8.0, 14.0),
        ),
        list: NSRect::new(
            NSPoint::new(MARGIN, 14.0),
            NSSize::new(LIST_WIDTH, h - 14.0 - 6.0 - CAPTION_HEIGHT),
        ),
        card,
        text: NSRect::new(
            NSPoint::new(card.origin.x + 4.0, card.origin.y + FOOTER_HEIGHT),
            NSSize::new(
                card.size.width - 8.0,
                card.size.height - FOOTER_HEIGHT - 6.0,
            ),
        ),
        status: NSRect::new(
            NSPoint::new(card.origin.x + 14.0, card.origin.y + 8.0),
            NSSize::new(card.size.width - 100.0, 14.0),
        ),
        stop: NSRect::new(
            NSPoint::new(
                card.origin.x + card.size.width - 12.0 - 22.0 - 28.0,
                card.origin.y + 4.0,
            ),
            NSSize::new(22.0, 22.0),
        ),
        clear: NSRect::new(
            NSPoint::new(
                card.origin.x + card.size.width - 12.0 - 22.0,
                card.origin.y + 4.0,
            ),
            NSSize::new(22.0, 22.0),
        ),
    }
}

define_class!(
    // SAFETY: NSView allows subclassing; this class has no ivars.
    #[unsafe(super(NSView))]
    #[thread_kind = MainThreadOnly]
    #[name = "FigCommandList"]
    pub struct CommandList;

    impl CommandList {
        // Rows are laid out from the top, and the list scrolls from its top.
        #[unsafe(method(isFlipped))]
        fn is_flipped(&self) -> bool {
            true
        }
    }
);

impl CommandList {
    fn new(mtm: MainThreadMarker, frame: NSRect) -> Retained<Self> {
        // SAFETY: designated initializer with valid arguments on the main thread.
        unsafe { msg_send![super(mtm.alloc::<Self>().set_ivars(())), initWithFrame: frame] }
    }
}

pub struct RootIvars {
    caption: RefCell<String>,
    note: RefCell<Option<(String, String)>>,
}

define_class!(
    // SAFETY: NSView allows subclassing; the ivars are only touched on the main thread and need no Drop.
    #[unsafe(super(NSView))]
    #[thread_kind = MainThreadOnly]
    #[name = "FigCommandsRoot"]
    #[ivars = RootIvars]
    pub struct CommandsRoot;

    impl CommandsRoot {
        #[unsafe(method(drawRect:))]
        fn draw_rect(&self, _dirty: NSRect) {
            let size = self.bounds().size;
            if let Some((title, hint)) = self.ivars().note.borrow().as_ref() {
                draw_page_note("terminal", title, hint, size);
                return;
            }
            let parts = layout(size);
            style::card(parts.card, RADIUS);
            draw_text(
                &self.ivars().caption.borrow().to_uppercase(),
                parts.caption,
                &text_attributes(&style::font(9.5, Weight::Semibold), &white(style::TEXT_TERTIARY), Fit::Tail, false),
            );
        }
    }
);

impl CommandsRoot {
    fn new(mtm: MainThreadMarker, frame: NSRect) -> Retained<Self> {
        let this = mtm.alloc::<Self>().set_ivars(RootIvars {
            caption: RefCell::new(String::new()),
            note: RefCell::new(None),
        });
        // SAFETY: designated initializer with valid arguments on the main thread.
        unsafe { msg_send![super(this), initWithFrame: frame] }
    }
}

pub struct CommandsView {
    root: Retained<CommandsRoot>,
    stop: Retained<TouchButton>,
    clear: Retained<TouchButton>,
    list_scroll: Retained<NSScrollView>,
    list: Retained<CommandList>,
    rows: RefCell<Vec<Retained<TouchButton>>>,
    output_scroll: Retained<NSScrollView>,
    output: Retained<NSTextView>,
    status: Retained<NSTextField>,
    callbacks: Rc<CommandsCallbacks>,
    mtm: MainThreadMarker,
}

impl CommandsView {
    pub fn new(mtm: MainThreadMarker, frame: NSRect, callbacks: CommandsCallbacks) -> Rc<Self> {
        let parts = layout(frame.size);
        let callbacks = Rc::new(callbacks);
        let root = CommandsRoot::new(mtm, frame);

        let list_scroll = NSScrollView::initWithFrame(mtm.alloc(), parts.list);
        list_scroll.setHasVerticalScroller(true);
        list_scroll.setHasHorizontalScroller(false);
        list_scroll.setAutohidesScrollers(true);
        list_scroll.setScrollerStyle(NSScrollerStyle::Overlay);
        list_scroll.setDrawsBackground(false);
        list_scroll.setBorderType(NSBorderType::NoBorder);
        let list = CommandList::new(mtm, NSRect::new(NSPoint::ZERO, parts.list.size));
        list_scroll.setDocumentView(Some(&list));
        root.addSubview(&list_scroll);

        let output_scroll = NSScrollView::initWithFrame(mtm.alloc(), parts.text);
        output_scroll.setHasVerticalScroller(true);
        output_scroll.setHasHorizontalScroller(false);
        output_scroll.setAutohidesScrollers(true);
        output_scroll.setScrollerStyle(NSScrollerStyle::Overlay);
        output_scroll.setDrawsBackground(false);
        output_scroll.setBorderType(NSBorderType::NoBorder);
        let output =
            NSTextView::initWithFrame(mtm.alloc(), NSRect::new(NSPoint::ZERO, parts.text.size));
        output.setMinSize(NSSize::new(0.0, parts.text.size.height));
        output.setMaxSize(NSSize::new(f64::MAX, f64::MAX));
        output.setVerticallyResizable(true);
        output.setHorizontallyResizable(false);
        output.setAutoresizingMask(NSAutoresizingMaskOptions::ViewWidthSizable);
        // SAFETY: the container belongs to the text view just made and is used on the main thread.
        if let Some(container) = unsafe { output.textContainer() } {
            container.setContainerSize(NSSize::new(parts.text.size.width, f64::MAX));
            container.setWidthTracksTextView(true);
        }
        output.setEditable(false);
        output.setSelectable(true);
        output.setRichText(false);
        output.setDrawsBackground(false);
        output.setTextColor(Some(&white(style::TEXT)));
        output.setTextContainerInset(NSSize::new(6.0, 8.0));
        output.setFont(Some(&NSFont::monospacedSystemFontOfSize_weight(11.5, 0.0)));
        output_scroll.setDocumentView(Some(&output));
        root.addSubview(&output_scroll);

        let status = NSTextField::labelWithString(&NSString::from_str(""), mtm);
        status.setFont(Some(&style::font(10.0, Weight::Regular)));
        status.setTextColor(Some(&white(style::TEXT_TERTIARY)));
        status.setFrame(parts.status);
        root.addSubview(&status);

        let stop = TouchButton::new(mtm, parts.stop, Look::Round { filled: false });
        stop.set_symbol("stop.fill");
        let shared = Rc::clone(&callbacks);
        stop.set_on_click(move || (shared.on_stop)());
        root.addSubview(&stop);
        let clear = TouchButton::new(mtm, parts.clear, Look::Round { filled: false });
        clear.set_symbol("trash");
        let shared = Rc::clone(&callbacks);
        clear.set_on_click(move || (shared.on_clear)());
        root.addSubview(&clear);

        Rc::new(Self {
            root,
            stop,
            clear,
            list_scroll,
            list,
            rows: RefCell::default(),
            output_scroll,
            output,
            status,
            callbacks,
            mtm,
        })
    }

    pub fn show(&self, display: &CommandsDisplay) {
        *self.root.ivars().caption.borrow_mut() = display.caption.clone();
        *self.root.ivars().note.borrow_mut() = display.note.clone();
        let off = display.note.is_some();
        for view in [
            &*self.stop as &NSView,
            &self.clear,
            &self.list_scroll,
            &self.output_scroll,
            &self.status,
        ] {
            view.setHidden(off);
        }
        self.stop.set_title(&display.stop_label);
        self.clear.set_title(&display.clear_label);
        self.stop.set_enabled(display.running);
        self.status
            .setStringValue(&NSString::from_str(&display.footer));
        self.sync_rows(&display.rows);
        self.root.setNeedsDisplay(true);
    }

    fn sync_rows(&self, rows: &[CommandRow]) {
        let width = self.list_scroll.contentSize().width;
        let mut pool = self.rows.borrow_mut();
        while pool.len() < rows.len() {
            let index = pool.len();
            let button = TouchButton::new(self.mtm, NSRect::ZERO, Look::Row);
            let shared = Rc::clone(&self.callbacks);
            button.set_on_click(move || (shared.on_run)(index));
            self.list.addSubview(&button);
            pool.push(button);
        }
        for (i, button) in pool.iter().enumerate() {
            match rows.get(i) {
                Some(row) => {
                    button.setHidden(false);
                    button.setFrame(NSRect::new(
                        NSPoint::new(0.0, i as f64 * (ROW_HEIGHT + ROW_GAP)),
                        NSSize::new(width, ROW_HEIGHT),
                    ));
                    button.set_title(&row.title);
                    button.set_detail(&row.detail);
                    button.set_selected(row.selected);
                }
                None => button.setHidden(true),
            }
        }
        let height =
            (rows.len() as f64 * (ROW_HEIGHT + ROW_GAP)).max(self.list_scroll.contentSize().height);
        self.list.setFrameSize(NSSize::new(width, height));
    }

    /// Adds to the end of the output, which scrolls to show it.
    pub fn append(&self, text: &str) {
        let end = NSRange::new(self.output.string().length(), 0);
        self.output
            .replaceCharactersInRange_withString(end, &NSString::from_str(text));
        self.output
            .scrollRangeToVisible(NSRange::new(self.output.string().length(), 0));
    }

    /// Replaces the whole output; empty clears it.
    pub fn set_output(&self, text: &str) {
        self.output.setString(&NSString::from_str(text));
        self.output.scrollRangeToVisible(NSRange::new(0, 0));
    }

    pub fn page(
        self: &Rc<Self>,
        title: String,
        on_show: impl Fn() + 'static,
        on_hide: impl Fn() + 'static,
    ) -> Page {
        Page {
            id: "commands",
            title,
            symbol: "terminal",
            root: Retained::into_super(Retained::clone(&self.root)),
            on_show: Box::new(on_show),
            on_hide: Box::new(on_hide),
            on_key: Box::new(|_| false),
            accepts_files: false,
            size: None,
        }
    }
}
