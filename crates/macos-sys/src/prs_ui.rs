//! The Pull requests tab of the notch: the review requests that wait for the person, one row each, and a button to
//! look again. Everything on it is plain data from the app, so nothing here runs a command or knows a language.

use crate::{
    button::{Look, TouchButton},
    notch::Page,
    style::{self, Weight, white},
    widgets::{Fit, draw_page_note, draw_text, text_attributes},
};
use objc2::{DefinedClass, MainThreadMarker, MainThreadOnly, define_class, msg_send, rc::Retained};
use objc2_app_kit::{NSBorderType, NSScrollView, NSScrollerStyle, NSStringDrawing, NSView};
use objc2_foundation::{NSPoint, NSRect, NSSize};
use std::{cell::RefCell, rc::Rc};

/// The size of the area the tab fills below the header.
pub const PAGE_SIZE: NSSize = NSSize::new(580.0, 262.0);

const MARGIN: f64 = 18.0;
const HEADER_HEIGHT: f64 = 40.0;
const ROW_HEIGHT: f64 = 46.0;
const ROW_GAP: f64 = 2.0;

#[derive(Debug, Clone)]
pub struct PrRow {
    pub title: String,
    /// What is under the title: the project, the number, who wrote it and how long ago.
    pub detail: String,
}

#[derive(Debug, Clone, Default)]
pub struct PrsDisplay {
    pub heading: String,
    /// How fresh the list is, or what is being done.
    pub status: String,
    pub rows: Vec<PrRow>,
    pub refresh_label: String,
    pub busy: bool,
    /// Words that replace the list and the heading: the tab is off, no tool is signed in, nothing waits.
    pub note: Option<(&'static str, String, String)>,
}

pub struct PrsCallbacks {
    pub on_open: Box<dyn Fn(usize)>,
    pub on_refresh: Box<dyn Fn()>,
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
    #[name = "FigPrsRoot"]
    #[ivars = RootIvars]
    pub struct PrsRoot;

    impl PrsRoot {
        #[unsafe(method(drawRect:))]
        fn draw_rect(&self, _dirty: NSRect) {
            let size = self.bounds().size;
            if let Some((symbol, title, hint)) = self.ivars().note.borrow().as_ref() {
                draw_page_note(symbol, title, hint, size);
                return;
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
            let status = self.ivars().status.borrow();
            let attributes = text_attributes(
                &style::font(10.0, Weight::Regular),
                &white(style::TEXT_TERTIARY),
                Fit::Tail,
                false,
            );
            // SAFETY: the attributes dictionary holds valid attribute objects.
            let width = unsafe {
                objc2_foundation::NSString::from_str(&status).sizeWithAttributes(Some(&attributes))
            }
            .width
            .ceil()
                + 2.0;
            let right = size.width - MARGIN - 34.0;
            draw_text(
                &status,
                NSRect::new(
                    NSPoint::new(right - width, h - 26.0),
                    NSSize::new(width, 14.0),
                ),
                &attributes,
            );
        }
    }
);

impl PrsRoot {
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
    #[name = "FigPrsList"]
    pub struct PrsList;

    impl PrsList {
        // Rows are laid out from the top, and the list scrolls from its top.
        #[unsafe(method(isFlipped))]
        fn is_flipped(&self) -> bool {
            true
        }
    }
);

impl PrsList {
    fn new(mtm: MainThreadMarker, frame: NSRect) -> Retained<Self> {
        // SAFETY: designated initializer with valid arguments on the main thread.
        unsafe { msg_send![super(mtm.alloc::<Self>().set_ivars(())), initWithFrame: frame] }
    }
}

pub struct PrsView {
    root: Retained<PrsRoot>,
    refresh: Retained<TouchButton>,
    scroll: Retained<NSScrollView>,
    list: Retained<PrsList>,
    rows: RefCell<Vec<Retained<TouchButton>>>,
    on_open: Rc<dyn Fn(usize)>,
    mtm: MainThreadMarker,
}

impl PrsView {
    /// `frame` is `Notch::page_area(PAGE_SIZE.width, PAGE_SIZE.height)`.
    pub fn new(mtm: MainThreadMarker, frame: NSRect, callbacks: PrsCallbacks) -> Rc<Self> {
        let (width, height) = (frame.size.width, frame.size.height);
        let root = PrsRoot::new(mtm, frame);
        let refresh = TouchButton::new(
            mtm,
            NSRect::new(
                NSPoint::new(width - MARGIN - 28.0, height - 34.0),
                NSSize::new(28.0, 28.0),
            ),
            Look::Round { filled: false },
        );
        refresh.set_symbol("arrow.clockwise");
        let PrsCallbacks {
            on_open,
            on_refresh,
        } = callbacks;
        refresh.set_on_click(on_refresh);
        root.addSubview(&refresh);
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
        let list = PrsList::new(mtm, NSRect::new(NSPoint::ZERO, list_frame.size));
        scroll.setDocumentView(Some(&list));
        root.addSubview(&scroll);
        Rc::new(Self {
            root,
            refresh,
            scroll,
            list,
            rows: RefCell::default(),
            on_open: Rc::new(on_open),
            mtm,
        })
    }

    pub fn show(&self, display: &PrsDisplay) {
        *self.root.ivars().heading.borrow_mut() = display.heading.clone();
        *self.root.ivars().status.borrow_mut() = display.status.clone();
        *self.root.ivars().note.borrow_mut() = display.note.clone();
        let off = display.note.is_some();
        self.refresh.set_title(&display.refresh_label);
        self.refresh.set_enabled(!display.busy);
        // The button to look again is there whenever the app offers one: not while the tab is off or still looking.
        self.refresh.setHidden(display.refresh_label.is_empty());
        self.scroll.setHidden(off);
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
                }
                None => button.setHidden(true),
            }
        }
        let height = (display.rows.len() as f64 * (ROW_HEIGHT + ROW_GAP))
            .max(self.scroll.contentSize().height);
        self.list.setFrameSize(NSSize::new(width, height));
        self.root.setNeedsDisplay(true);
    }

    /// The notch tab for this view.
    pub fn page(
        self: &Rc<Self>,
        title: String,
        on_show: impl Fn() + 'static,
        on_hide: impl Fn() + 'static,
    ) -> Page {
        Page {
            id: "prs",
            title,
            symbol: "arrow.triangle.pull",
            root: Retained::into_super(Retained::clone(&self.root)),
            on_show: Box::new(on_show),
            on_hide: Box::new(on_hide),
            on_key: Box::new(|_| false),
            accepts_files: false,
            size: None,
        }
    }
}
