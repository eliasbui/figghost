//! The Notifications tab of the notch: the notifications other apps sent, newest first, one row each with the app's
//! icon, and a button beside the heading (clear the list, or open the settings that give access). Everything on it is
//! plain data from the app, so nothing here reads a notification or knows a language.

use crate::{
    button::{Look, TouchButton},
    notch::Page,
    style::{self, Weight, white},
    widgets::{Fit, draw_page_note, draw_text, text_attributes},
};
use objc2::{DefinedClass, MainThreadMarker, MainThreadOnly, define_class, msg_send, rc::Retained};
use objc2_app_kit::{NSBorderType, NSScrollView, NSScrollerStyle, NSView};
use objc2_foundation::{NSPoint, NSRect, NSSize};
use std::{cell::RefCell, path::PathBuf, rc::Rc};

/// The size of the area the tab fills below the header.
pub const PAGE_SIZE: NSSize = NSSize::new(580.0, 262.0);

const MARGIN: f64 = 18.0;
const HEADER_HEIGHT: f64 = 40.0;
const ROW_HEIGHT: f64 = 46.0;
const ROW_GAP: f64 = 2.0;

#[derive(Debug, Clone)]
pub struct NoticeRow {
    pub title: String,
    /// The app, how long ago, and what it says.
    pub detail: String,
    /// The app, whose icon the row shows.
    pub app: Option<PathBuf>,
}

#[derive(Debug, Clone, Default)]
pub struct NotificationsDisplay {
    pub heading: String,
    /// How many there are, or how they arrive.
    pub status: String,
    pub rows: Vec<NoticeRow>,
    /// The button beside the heading, its glyph and its name; none when the name is empty.
    pub button: (&'static str, String),
    /// Words that replace the list: the tab is off, access is missing, nothing has arrived.
    pub note: Option<(&'static str, String, String)>,
}

pub struct NotificationsCallbacks {
    pub on_open: Box<dyn Fn(usize)>,
    pub on_button: Box<dyn Fn()>,
}

pub struct RootIvars {
    heading: RefCell<String>,
    status: RefCell<String>,
    note: RefCell<Option<(&'static str, String, String)>>,
}

define_class!(
    // SAFETY: NSView allows subclassing; the ivars are only touched on the main thread and need no Drop.
    #[unsafe(super(NSView))]
    #[thread_kind = MainThreadOnly]
    #[name = "FigNotificationsRoot"]
    #[ivars = RootIvars]
    struct NotificationsRoot;

    impl NotificationsRoot {
        #[unsafe(method(drawRect:))]
        fn draw_rect(&self, _dirty: NSRect) {
            let size = self.bounds().size;
            if let Some((symbol, title, hint)) = self.ivars().note.borrow().as_ref() {
                draw_page_note(symbol, title, hint, size);
            }
            let h = size.height;
            draw_text(
                &self.ivars().heading.borrow(),
                NSRect::new(
                    NSPoint::new(MARGIN + 2.0, h - 28.0),
                    NSSize::new(size.width * 0.5, 18.0),
                ),
                &text_attributes(
                    &style::font(14.0, Weight::Semibold),
                    &white(style::TEXT),
                    Fit::Tail,
                    false,
                ),
            );
            draw_text(
                &self.ivars().status.borrow(),
                NSRect::new(
                    NSPoint::new(size.width * 0.5, h - 26.0),
                    NSSize::new(size.width * 0.5 - MARGIN - 40.0, 14.0),
                ),
                &text_attributes(
                    &style::font(10.0, Weight::Regular),
                    &white(style::TEXT_TERTIARY),
                    Fit::Tail,
                    false,
                ),
            );
        }
    }
);

impl NotificationsRoot {
    fn new(mtm: MainThreadMarker, frame: NSRect) -> Retained<Self> {
        let this = mtm.alloc::<Self>().set_ivars(RootIvars {
            heading: RefCell::default(),
            status: RefCell::default(),
            note: RefCell::new(None),
        });
        // SAFETY: designated initializer with valid arguments on the main thread.
        unsafe { msg_send![super(this), initWithFrame: frame] }
    }
}

define_class!(
    // SAFETY: NSView allows subclassing; this class has no ivars.
    #[unsafe(super(NSView))]
    #[thread_kind = MainThreadOnly]
    #[name = "FigNotificationsList"]
    struct NotificationsList;

    impl NotificationsList {
        // Rows are laid out from the top, and the list scrolls from its top.
        #[unsafe(method(isFlipped))]
        fn is_flipped(&self) -> bool {
            true
        }
    }
);

impl NotificationsList {
    fn new(mtm: MainThreadMarker, frame: NSRect) -> Retained<Self> {
        // SAFETY: designated initializer with valid arguments on the main thread.
        unsafe { msg_send![super(mtm.alloc::<Self>().set_ivars(())), initWithFrame: frame] }
    }
}

pub struct NotificationsView {
    root: Retained<NotificationsRoot>,
    button: Retained<TouchButton>,
    scroll: Retained<NSScrollView>,
    list: Retained<NotificationsList>,
    rows: RefCell<Vec<Retained<TouchButton>>>,
    on_open: Rc<dyn Fn(usize)>,
    mtm: MainThreadMarker,
}

impl NotificationsView {
    /// `frame` is `Notch::page_area(PAGE_SIZE.width, PAGE_SIZE.height)`.
    pub fn new(
        mtm: MainThreadMarker,
        frame: NSRect,
        callbacks: NotificationsCallbacks,
    ) -> Rc<Self> {
        let (width, height) = (frame.size.width, frame.size.height);
        let root = NotificationsRoot::new(mtm, frame);
        let button = TouchButton::new(
            mtm,
            NSRect::new(
                NSPoint::new(width - MARGIN - 28.0, height - 34.0),
                NSSize::new(28.0, 28.0),
            ),
            Look::Round { filled: false },
        );
        let NotificationsCallbacks { on_open, on_button } = callbacks;
        button.set_on_click(on_button);
        root.addSubview(&button);
        let list_frame = NSRect::new(
            NSPoint::new(MARGIN, 8.0),
            NSSize::new(width - 2.0 * MARGIN, height - HEADER_HEIGHT - 8.0),
        );
        let scroll = NSScrollView::initWithFrame(mtm.alloc(), list_frame);
        scroll.setHasVerticalScroller(true);
        scroll.setHasHorizontalScroller(false);
        scroll.setAutohidesScrollers(true);
        scroll.setScrollerStyle(NSScrollerStyle::Overlay);
        scroll.setDrawsBackground(false);
        scroll.setBorderType(NSBorderType::NoBorder);
        let list = NotificationsList::new(mtm, NSRect::new(NSPoint::ZERO, list_frame.size));
        scroll.setDocumentView(Some(&list));
        root.addSubview(&scroll);
        Rc::new(Self {
            root,
            button,
            scroll,
            list,
            rows: RefCell::default(),
            on_open: Rc::new(on_open),
            mtm,
        })
    }

    pub fn show(&self, display: &NotificationsDisplay) {
        let ivars = self.root.ivars();
        *ivars.heading.borrow_mut() = display.heading.clone();
        *ivars.status.borrow_mut() = display.status.clone();
        *ivars.note.borrow_mut() = display.note.clone();
        let (symbol, name) = &display.button;
        self.button.set_symbol(symbol);
        self.button.set_title(name);
        self.button.setHidden(name.is_empty());
        self.scroll.setHidden(display.note.is_some());
        let width = self.scroll.contentSize().width;
        let mut pool = self.rows.borrow_mut();
        while pool.len() < display.rows.len() {
            let index = pool.len();
            let button = TouchButton::new(self.mtm, NSRect::ZERO, Look::Row);
            let open = Rc::clone(&self.on_open);
            button.set_on_click(move || open(index));
            self.list.addSubview(&button);
            pool.push(button);
        }
        for (i, button) in pool.iter().enumerate() {
            match display.rows.get(i) {
                Some(row) => {
                    button.setHidden(false);
                    button.setFrame(NSRect::new(
                        NSPoint::new(0.0, i as f64 * (ROW_HEIGHT + ROW_GAP)),
                        NSSize::new(width, ROW_HEIGHT),
                    ));
                    button.set_title(&row.title);
                    button.set_detail(&row.detail);
                    button.set_image(
                        row.app
                            .as_deref()
                            .map(|path| crate::workspace::icon_of(path, 28.0)),
                    );
                }
                None => button.setHidden(true),
            }
        }
        let height = (display.rows.len() as f64 * (ROW_HEIGHT + ROW_GAP))
            .max(self.scroll.contentSize().height);
        self.list.setFrameSize(NSSize::new(width, height));
        self.root.setNeedsDisplay(true);
    }

    /// The notch tab for this view, under the id and the SF Symbol of the feature that shows it.
    pub fn page(
        self: &Rc<Self>,
        id: &'static str,
        symbol: &'static str,
        title: String,
        on_show: impl Fn() + 'static,
        on_hide: impl Fn() + 'static,
    ) -> Page {
        Page {
            id,
            title,
            symbol,
            root: Retained::into_super(Retained::clone(&self.root)),
            on_show: Box::new(on_show),
            on_hide: Box::new(on_hide),
            on_key: Box::new(|_| false),
            accepts_files: false,
            size: None,
        }
    }

    /// Presses a row, or the button beside the heading with `None`, as an assistive tool does, for the checks.
    pub fn press(&self, row: Option<usize>) -> bool {
        let target = match row {
            Some(i) => self.rows.borrow().get(i).filter(|b| !b.isHidden()).cloned(),
            None => Some(self.button.clone()).filter(|b| !b.isHidden()),
        };
        // SAFETY: accessibilityPerformPress takes no arguments and returns whether it pressed.
        target.is_some_and(|b| unsafe { msg_send![&*b, accessibilityPerformPress] })
    }

    /// The page drawn into a PNG, rows included, for the pictures of the documentation and the checks.
    pub fn snapshot_png(&self) -> Option<Vec<u8>> {
        crate::widgets::view_png(&self.root)
    }
}
