//! The Notes tab of the notch: the list of notes at the left and an editor at the right. Everything on it is plain
//! data from the app, so nothing here reads a file or knows a language.

use crate::{
    button::{Look, TouchButton},
    notch::Page,
    panel::Key,
    style::{self, Weight, white},
    widgets::{Fit, draw_page_note, draw_text, text_attributes},
};
use objc2::{
    DefinedClass, MainThreadMarker, MainThreadOnly, define_class, msg_send, rc::Retained,
    runtime::AnyObject,
};
use objc2_app_kit::{
    NSAutoresizingMaskOptions, NSBorderType, NSScrollView, NSScrollerStyle, NSTextField,
    NSTextView, NSView,
};
use objc2_foundation::{NSPoint, NSRect, NSSize, NSString};
use std::{cell::RefCell, rc::Rc};

/// The size of the area the tab fills below the header.
pub const PAGE_SIZE: NSSize = NSSize::new(580.0, 270.0);
/// The choices of the size of the text, in points.
pub const TEXT_SIZES: [u32; 3] = [12, 14, 16];

const MARGIN: f64 = 18.0;
const LIST_WIDTH: f64 = 178.0;
const GAP: f64 = 12.0;
const ROW_HEIGHT: f64 = 42.0;
const ROW_GAP: f64 = 2.0;
const CAPTION_HEIGHT: f64 = 30.0;
const FOOTER_HEIGHT: f64 = 30.0;
const RADIUS: f64 = 14.0;

/// One row of the list.
#[derive(Debug, Clone)]
pub struct NoteRow {
    pub title: String,
    pub detail: String,
    pub selected: bool,
}

/// What the tab shows besides the text of the open note.
#[derive(Debug, Clone, Default)]
pub struct NotesDisplay {
    pub caption: String,
    pub rows: Vec<NoteRow>,
    /// The line under the editor, such as "123 characters".
    pub footer: String,
    /// Grey words in an empty editor.
    pub placeholder: String,
    pub new_label: String,
    pub delete_label: String,
    /// Words that replace everything: the tab is off.
    pub note: Option<(String, String)>,
}

/// What the controls do; the app owns the notes.
pub struct NotesCallbacks {
    pub on_new: Box<dyn Fn()>,
    pub on_select: Box<dyn Fn(usize)>,
    pub on_delete: Box<dyn Fn()>,
    /// The editor's whole text after every change a person makes; never after one the app makes.
    pub on_edit: Box<dyn Fn(String)>,
    /// Up (-1) or down (+1) in the list, while the editor does not have the keys.
    pub on_step: Box<dyn Fn(i32)>,
}

struct Layout {
    caption: NSRect,
    add: NSRect,
    list: NSRect,
    card: NSRect,
    text: NSRect,
    count: NSRect,
    trash: NSRect,
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
            NSSize::new(LIST_WIDTH - 40.0, 14.0),
        ),
        add: NSRect::new(
            NSPoint::new(MARGIN + LIST_WIDTH - 26.0, h - 6.0 - 26.0),
            NSSize::new(26.0, 26.0),
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
        count: NSRect::new(
            NSPoint::new(card.origin.x + 14.0, card.origin.y + 8.0),
            NSSize::new(card.size.width - 60.0, 14.0),
        ),
        trash: NSRect::new(
            NSPoint::new(
                card.origin.x + card.size.width - 12.0 - 22.0,
                card.origin.y + 4.0,
            ),
            NSSize::new(22.0, 22.0),
        ),
    }
}

// -- the text view ------------------------------------------------------------------------------------------

type ChangeHandler = Rc<dyn Fn(String)>;

pub struct NoteTextIvars {
    on_change: RefCell<Option<ChangeHandler>>,
    placeholder: RefCell<String>,
}

define_class!(
    // SAFETY: NSTextView allows subclassing; the ivars are only touched on the main thread and need no Drop.
    #[unsafe(super(NSTextView))]
    #[thread_kind = MainThreadOnly]
    #[name = "FigNoteText"]
    #[ivars = NoteTextIvars]
    pub struct NoteText;

    impl NoteText {
        // Called after a person changes the text; setting the text from the app does not call it.
        #[unsafe(method(didChangeText))]
        fn did_change_text(&self) {
            // SAFETY: forwards the same message to NSTextView's own implementation.
            let _: () = unsafe { msg_send![super(self), didChangeText] };
            let change = self.ivars().on_change.borrow().clone();
            if let Some(change) = change {
                change(self.string().to_string());
            }
        }

        #[unsafe(method(drawRect:))]
        fn draw_rect(&self, dirty: NSRect) {
            // SAFETY: forwards the same message to NSTextView's own implementation.
            let _: () = unsafe { msg_send![super(self), drawRect: dirty] };
            if self.string().length() > 0 {
                return;
            }
            let origin = self.textContainerOrigin();
            let width = (self.bounds().size.width - origin.x * 2.0).max(0.0);
            draw_text(
                &self.ivars().placeholder.borrow(),
                NSRect::new(NSPoint::new(origin.x + 5.0, origin.y), NSSize::new(width, 20.0)),
                &text_attributes(
                    &style::font(14.0, Weight::Regular),
                    &white(style::TEXT_TERTIARY),
                    Fit::Tail,
                    false,
                ),
            );
        }
    }
);

impl NoteText {
    fn new(mtm: MainThreadMarker, frame: NSRect) -> Retained<Self> {
        let this = mtm.alloc::<Self>().set_ivars(NoteTextIvars {
            on_change: RefCell::new(None),
            placeholder: RefCell::new(String::new()),
        });
        // SAFETY: designated initializer with valid arguments on the main thread.
        unsafe { msg_send![super(this), initWithFrame: frame] }
    }
}

// -- the list's container and the page's root ----------------------------------------------------------------------

define_class!(
    // SAFETY: NSView allows subclassing; this class has no ivars.
    #[unsafe(super(NSView))]
    #[thread_kind = MainThreadOnly]
    #[name = "FigNoteList"]
    pub struct NoteList;

    impl NoteList {
        // Rows are laid out from the top, and the list scrolls from its top.
        #[unsafe(method(isFlipped))]
        fn is_flipped(&self) -> bool {
            true
        }
    }
);

impl NoteList {
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
    #[name = "FigNotesRoot"]
    #[ivars = RootIvars]
    pub struct NotesRoot;

    impl NotesRoot {
        #[unsafe(method(drawRect:))]
        fn draw_rect(&self, _dirty: NSRect) {
            let size = self.bounds().size;
            if let Some((title, hint)) = self.ivars().note.borrow().as_ref() {
                draw_page_note("note.text", title, hint, size);
                return;
            }
            let parts = layout(size);
            style::card(parts.card, RADIUS);
            draw_text(
                &self.ivars().caption.borrow().to_uppercase(),
                parts.caption,
                &text_attributes(
                    &style::font(9.5, Weight::Semibold),
                    &white(style::TEXT_TERTIARY),
                    Fit::Tail,
                    false,
                ),
            );
        }
    }
);

impl NotesRoot {
    fn new(mtm: MainThreadMarker, frame: NSRect) -> Retained<Self> {
        let this = mtm.alloc::<Self>().set_ivars(RootIvars {
            caption: RefCell::new(String::new()),
            note: RefCell::new(None),
        });
        // SAFETY: designated initializer with valid arguments on the main thread.
        unsafe { msg_send![super(this), initWithFrame: frame] }
    }
}

// -- the view ------------------------------------------------------------------------------------------------------

pub struct NotesView {
    root: Retained<NotesRoot>,
    add: Retained<TouchButton>,
    trash: Retained<TouchButton>,
    list_scroll: Retained<NSScrollView>,
    list: Retained<NoteList>,
    rows: RefCell<Vec<Retained<TouchButton>>>,
    editor_scroll: Retained<NSScrollView>,
    editor: Retained<NoteText>,
    count: Retained<NSTextField>,
    callbacks: Rc<NotesCallbacks>,
    mtm: MainThreadMarker,
}

impl NotesView {
    pub fn new(mtm: MainThreadMarker, frame: NSRect, callbacks: NotesCallbacks) -> Rc<Self> {
        let parts = layout(frame.size);
        let callbacks = Rc::new(callbacks);
        let root = NotesRoot::new(mtm, frame);

        let add = TouchButton::new(mtm, parts.add, Look::Round { filled: false });
        add.set_symbol("plus");
        let shared = Rc::clone(&callbacks);
        add.set_on_click(move || (shared.on_new)());
        root.addSubview(&add);

        let list_scroll = NSScrollView::initWithFrame(mtm.alloc(), parts.list);
        list_scroll.setHasVerticalScroller(true);
        list_scroll.setHasHorizontalScroller(false);
        list_scroll.setAutohidesScrollers(true);
        list_scroll.setScrollerStyle(NSScrollerStyle::Overlay);
        list_scroll.setDrawsBackground(false);
        list_scroll.setBorderType(NSBorderType::NoBorder);
        let list = NoteList::new(mtm, NSRect::new(NSPoint::ZERO, parts.list.size));
        list_scroll.setDocumentView(Some(&list));
        root.addSubview(&list_scroll);

        let editor_scroll = NSScrollView::initWithFrame(mtm.alloc(), parts.text);
        editor_scroll.setHasVerticalScroller(true);
        editor_scroll.setHasHorizontalScroller(false);
        editor_scroll.setAutohidesScrollers(true);
        editor_scroll.setScrollerStyle(NSScrollerStyle::Overlay);
        editor_scroll.setDrawsBackground(false);
        editor_scroll.setBorderType(NSBorderType::NoBorder);
        let editor = NoteText::new(mtm, NSRect::new(NSPoint::ZERO, parts.text.size));
        editor.setMinSize(NSSize::new(0.0, parts.text.size.height));
        editor.setMaxSize(NSSize::new(f64::MAX, f64::MAX));
        editor.setVerticallyResizable(true);
        editor.setHorizontallyResizable(false);
        editor.setAutoresizingMask(NSAutoresizingMaskOptions::ViewWidthSizable);
        // SAFETY: the container belongs to the text view just made and is used on the main thread.
        if let Some(container) = unsafe { editor.textContainer() } {
            container.setContainerSize(NSSize::new(parts.text.size.width, f64::MAX));
            container.setWidthTracksTextView(true);
        }
        editor.setRichText(false);
        editor.setImportsGraphics(false);
        editor.setAllowsUndo(true);
        editor.setDrawsBackground(false);
        editor.setTextColor(Some(&white(style::TEXT)));
        editor.setInsertionPointColor(Some(&white(0.95)));
        editor.setTextContainerInset(NSSize::new(6.0, 8.0));
        editor.setAutomaticQuoteSubstitutionEnabled(false);
        editor.setAutomaticDashSubstitutionEnabled(false);
        editor.setAutomaticTextReplacementEnabled(false);
        editor.setAutomaticSpellingCorrectionEnabled(false);
        editor.setAutomaticLinkDetectionEnabled(false);
        editor.setFont(Some(&style::font(14.0, Weight::Regular)));
        let shared = Rc::clone(&callbacks);
        *editor.ivars().on_change.borrow_mut() = Some(Rc::new(move |text| (shared.on_edit)(text)));
        editor_scroll.setDocumentView(Some(&editor));
        root.addSubview(&editor_scroll);

        let count = NSTextField::labelWithString(&NSString::from_str(""), mtm);
        count.setFont(Some(&style::font(10.0, Weight::Regular)));
        count.setTextColor(Some(&white(style::TEXT_TERTIARY)));
        count.setFrame(parts.count);
        root.addSubview(&count);

        let trash = TouchButton::new(mtm, parts.trash, Look::Round { filled: false });
        trash.set_symbol("trash");
        let shared = Rc::clone(&callbacks);
        trash.set_on_click(move || (shared.on_delete)());
        root.addSubview(&trash);

        Rc::new(Self {
            root,
            add,
            trash,
            list_scroll,
            list,
            rows: RefCell::default(),
            editor_scroll,
            editor,
            count,
            callbacks,
            mtm,
        })
    }

    /// Draws what the tab is now to show. The text of the editor is not touched; `set_text` does that.
    pub fn show(&self, display: &NotesDisplay) {
        *self.root.ivars().caption.borrow_mut() = display.caption.clone();
        *self.root.ivars().note.borrow_mut() = display.note.clone();
        let off = display.note.is_some();
        for view in [
            &*self.add as &NSView,
            &self.trash,
            &self.list_scroll,
            &self.editor_scroll,
            &self.count,
        ] {
            view.setHidden(off);
        }
        *self.editor.ivars().placeholder.borrow_mut() = display.placeholder.clone();
        self.editor.setNeedsDisplay(true);
        self.add.set_title(&display.new_label);
        self.trash.set_title(&display.delete_label);
        self.trash.set_enabled(!display.rows.is_empty());
        self.count
            .setStringValue(&NSString::from_str(&display.footer));
        self.sync_rows(&display.rows);
        self.root.setNeedsDisplay(true);
    }

    fn sync_rows(&self, rows: &[NoteRow]) {
        let width = self.list_scroll.contentSize().width;
        let mut pool = self.rows.borrow_mut();
        while pool.len() < rows.len() {
            let index = pool.len();
            let button = TouchButton::new(self.mtm, NSRect::ZERO, Look::Row);
            let shared = Rc::clone(&self.callbacks);
            button.set_on_click(move || (shared.on_select)(index));
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

    /// Puts a note's text in the editor, as the app's doing and not the person's: the edit callback stays quiet.
    pub fn set_text(&self, text: &str) {
        self.editor.setString(&NSString::from_str(text));
        self.editor
            .scrollRangeToVisible(objc2_foundation::NSRange::new(0, 0));
        self.editor.setNeedsDisplay(true);
    }

    pub fn text(&self) -> String {
        self.editor.string().to_string()
    }

    /// The size of the text in the editor, in points.
    pub fn set_text_size(&self, points: u32) {
        self.editor
            .setFont(Some(&style::font(f64::from(points), Weight::Regular)));
        self.editor.setNeedsDisplay(true);
    }

    /// Gives the editor the keyboard: the notch takes it, as a click on its search field does.
    pub fn focus_editor(&self) {
        if let Some(window) = self.root.window() {
            window.makeKeyAndOrderFront(None);
            window.makeFirstResponder(Some(&self.editor));
        }
    }

    pub fn editor_focused(&self) -> bool {
        self.root
            .window()
            .and_then(|window| window.firstResponder())
            .is_some_and(|responder| {
                std::ptr::eq(
                    Retained::as_ptr(&responder).cast::<AnyObject>(),
                    Retained::as_ptr(&self.editor).cast::<AnyObject>(),
                )
            })
    }

    /// Takes the keyboard back from the editor, so that the arrows move in the list again.
    pub fn leave_editor(&self) {
        if let Some(window) = self.root.window() {
            window.makeFirstResponder(None);
        }
    }

    /// Keys while the tab is shown. While the editor has the keyboard the keys are its own, except Escape, which
    /// leaves it; otherwise the arrows move in the list and Return opens the editor. True when consumed.
    pub fn handle_key(&self, key: Key) -> bool {
        if self.root.ivars().note.borrow().is_some() {
            return false;
        }
        if self.editor_focused() {
            if key == Key::Escape {
                self.leave_editor();
                return true;
            }
            return false;
        }
        match key {
            Key::Up => (self.callbacks.on_step)(-1),
            Key::Down => (self.callbacks.on_step)(1),
            Key::Enter => self.focus_editor(),
            _ => return false,
        }
        true
    }

    /// The notch tab for this view.
    pub fn page(
        self: &Rc<Self>,
        title: String,
        on_show: impl Fn() + 'static,
        on_hide: impl Fn() + 'static,
    ) -> Page {
        let keys = Rc::clone(self);
        let hide = Rc::clone(self);
        Page {
            id: "notes",
            title,
            symbol: "note.text",
            root: Retained::into_super(Retained::clone(&self.root)),
            on_show: Box::new(on_show),
            on_hide: Box::new(move || {
                hide.leave_editor();
                on_hide();
            }),
            on_key: Box::new(move |key| keys.handle_key(key)),
            accepts_files: false,
            size: None,
        }
    }
}
