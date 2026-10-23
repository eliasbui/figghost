//! The Updates tab of the notch: the software that has a newer version, one row each with a box to tick, the version
//! now in red and the new one in green, and buttons to update what is ticked or everything. Everything on it is plain
//! data from the app, so nothing here runs a command or knows a language.

use crate::{
    button::{Look, TouchButton},
    notch::Page,
    style::{self, Tone, Weight, white},
    widgets::{Fit, draw_page_note, draw_text, text_attributes},
};
use objc2::{DefinedClass, MainThreadMarker, MainThreadOnly, define_class, msg_send, rc::Retained};
use objc2_app_kit::{
    NSBezierPath, NSBorderType, NSColor, NSScrollView, NSScrollerStyle, NSStringDrawing, NSView,
};
use objc2_foundation::{NSPoint, NSRect, NSSize, NSString};
use std::{cell::RefCell, rc::Rc};

/// The size of the area the tab fills below the header.
pub const PAGE_SIZE: NSSize = NSSize::new(580.0, 262.0);

const MARGIN: f64 = 18.0;
const HEADER_HEIGHT: f64 = 40.0;
const ROW_HEIGHT: f64 = 40.0;
const ROW_GAP: f64 = 2.0;
const BOX: f64 = 16.0;
const BUTTON_GAP: f64 = 8.0;
/// The widest the versions get on a row; a longer one is cut in the middle.
const MAX_VERSIONS: f64 = 230.0;

#[derive(Debug, Clone, Default)]
pub struct UpdateRow {
    pub name: String,
    /// Where it comes from: Homebrew, npm, the App Store, macOS or an agent.
    pub source: String,
    /// Empty when the version now is not known.
    pub current: String,
    pub latest: String,
    pub checked: bool,
}

#[derive(Debug, Clone, Default)]
pub struct UpdatesDisplay {
    pub heading: String,
    /// How fresh the list is, or what is being done.
    pub status: String,
    pub rows: Vec<UpdateRow>,
    pub refresh_label: String,
    pub chosen_label: String,
    pub all_label: String,
    /// Looking or updating: the buttons wait.
    pub busy: bool,
    /// Words that replace the list and the heading: the tab is off, it is looking for the first time, all is current.
    pub note: Option<(&'static str, String, String)>,
}

pub struct UpdatesCallbacks {
    pub on_toggle: Box<dyn Fn(usize)>,
    pub on_refresh: Box<dyn Fn()>,
    pub on_update_chosen: Box<dyn Fn()>,
    pub on_update_all: Box<dyn Fn()>,
}

pub struct RootIvars {
    heading: RefCell<String>,
    status: RefCell<String>,
    /// Where the buttons start, so that the status ends before them.
    buttons_left: RefCell<f64>,
    note: RefCell<Option<(&'static str, String, String)>>,
}

define_class!(
    // SAFETY: NSView allows subclassing; the ivars are only touched on the main thread and need no Drop.
    #[unsafe(super(NSView))]
    #[thread_kind = MainThreadOnly]
    #[name = "FigUpdatesRoot"]
    #[ivars = RootIvars]
    pub struct UpdatesRoot;

    impl UpdatesRoot {
        #[unsafe(method(drawRect:))]
        fn draw_rect(&self, _dirty: NSRect) {
            let size = self.bounds().size;
            if let Some((symbol, title, hint)) = self.ivars().note.borrow().as_ref() {
                draw_page_note(symbol, title, hint, size);
                return;
            }
            let h = size.height;
            let heading_attrs = text_attributes(
                &style::font(14.0, Weight::Semibold),
                &white(style::TEXT),
                Fit::Tail,
                false,
            );
            let heading = self.ivars().heading.borrow();
            let heading_width = text_width(&heading, &heading_attrs);
            draw_text(
                &heading,
                NSRect::new(
                    NSPoint::new(MARGIN + 2.0, h - 28.0),
                    NSSize::new(heading_width, 18.0),
                ),
                &heading_attrs,
            );
            let left = MARGIN + 2.0 + heading_width + 10.0;
            let right = *self.ivars().buttons_left.borrow() - BUTTON_GAP;
            draw_text(
                &self.ivars().status.borrow(),
                NSRect::new(
                    NSPoint::new(left, h - 26.0),
                    NSSize::new((right - left).max(0.0), 14.0),
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

impl UpdatesRoot {
    fn new(mtm: MainThreadMarker, frame: NSRect) -> Retained<Self> {
        let this = mtm.alloc::<Self>().set_ivars(RootIvars {
            heading: RefCell::default(),
            status: RefCell::default(),
            buttons_left: RefCell::new(frame.size.width),
            note: RefCell::new(None),
        });
        // SAFETY: designated initializer with valid arguments on the main thread.
        unsafe { msg_send![super(this), initWithFrame: frame] }
    }
}

fn text_width(
    text: &str,
    attrs: &objc2_foundation::NSMutableDictionary<NSString, objc2::runtime::AnyObject>,
) -> f64 {
    if text.is_empty() {
        return 0.0;
    }
    // SAFETY: the attributes dictionary holds valid attribute objects.
    unsafe { NSString::from_str(text).sizeWithAttributes(Some(attrs)) }
        .width
        .ceil()
}

pub struct ListIvars {
    rows: RefCell<Vec<UpdateRow>>,
}

define_class!(
    // SAFETY: NSView allows subclassing; the ivars are only touched on the main thread and need no Drop.
    #[unsafe(super(NSView))]
    #[thread_kind = MainThreadOnly]
    #[name = "FigUpdatesList"]
    #[ivars = ListIvars]
    pub struct UpdatesList;

    impl UpdatesList {
        // Rows are laid out from the top, and the list scrolls from its top.
        #[unsafe(method(isFlipped))]
        fn is_flipped(&self) -> bool {
            true
        }

        #[unsafe(method(drawRect:))]
        fn draw_rect(&self, _dirty: NSRect) {
            let width = self.bounds().size.width;
            for (i, row) in self.ivars().rows.borrow().iter().enumerate() {
                paint_row(row, i as f64 * (ROW_HEIGHT + ROW_GAP), width);
            }
        }
    }
);

impl UpdatesList {
    fn new(mtm: MainThreadMarker, frame: NSRect) -> Retained<Self> {
        let this = mtm.alloc::<Self>().set_ivars(ListIvars {
            rows: RefCell::default(),
        });
        // SAFETY: designated initializer with valid arguments on the main thread.
        unsafe { msg_send![super(this), initWithFrame: frame] }
    }
}

/// One row in the flipped list, `top` being its upper edge: the box, the name over the source, and the versions at
/// the right, the one now in red and the new one in green.
fn paint_row(row: &UpdateRow, top: f64, width: f64) {
    let mid = top + ROW_HEIGHT / 2.0;
    let pad = 10.0;
    let rect = NSRect::new(NSPoint::new(pad, mid - BOX / 2.0), NSSize::new(BOX, BOX));
    if row.checked {
        style::fill_circle(rect, &Tone::Blue.color());
        let path = NSBezierPath::bezierPath();
        // The tick, in the flipped coordinates of the list.
        path.moveToPoint(NSPoint::new(rect.origin.x + 4.5, mid + 0.5));
        path.lineToPoint(NSPoint::new(rect.origin.x + 7.0, mid + 3.0));
        path.lineToPoint(NSPoint::new(rect.origin.x + 11.5, mid - 3.0));
        path.setLineWidth(1.8);
        white(1.0).set();
        path.stroke();
    } else {
        style::stroke_rounded(rect, BOX / 2.0, &white(0.45), 1.2);
    }
    let regular = |size: f64, color: &NSColor, fit: Fit| {
        text_attributes(&style::font(size, Weight::Regular), color, fit, false)
    };
    let new_attrs = text_attributes(
        &style::digits(11.5, Weight::Semibold),
        &Tone::Green.color(),
        Fit::Middle,
        false,
    );
    let old_attrs = text_attributes(
        &style::digits(11.5, Weight::Regular),
        &Tone::Red.color(),
        Fit::Middle,
        false,
    );
    let arrow_attrs = regular(11.5, &white(style::TEXT_TERTIARY), Fit::Clip);
    let arrow = "  ->  ";
    let latest_w = text_width(&row.latest, &new_attrs).min(MAX_VERSIONS / 2.0);
    let (current_w, arrow_w) = if row.current.is_empty() {
        (0.0, 0.0)
    } else {
        (
            text_width(&row.current, &old_attrs).min(MAX_VERSIONS / 2.0),
            text_width(arrow, &arrow_attrs),
        )
    };
    let right = width - pad;
    let mut x = right - latest_w;
    draw_text(
        &row.latest,
        NSRect::new(NSPoint::new(x, mid - 8.0), NSSize::new(latest_w, 16.0)),
        &new_attrs,
    );
    if current_w > 0.0 {
        x -= arrow_w;
        draw_text(
            arrow,
            NSRect::new(NSPoint::new(x, mid - 8.0), NSSize::new(arrow_w, 16.0)),
            &arrow_attrs,
        );
        x -= current_w;
        draw_text(
            &row.current,
            NSRect::new(NSPoint::new(x, mid - 8.0), NSSize::new(current_w, 16.0)),
            &old_attrs,
        );
    }
    let text_left = pad + BOX + 10.0;
    let text_width = (x - 12.0 - text_left).max(0.0);
    draw_text(
        &row.name,
        NSRect::new(
            NSPoint::new(text_left, mid - 16.0),
            NSSize::new(text_width, 16.0),
        ),
        &text_attributes(
            &style::font(12.0, Weight::Medium),
            &white(style::TEXT),
            Fit::Tail,
            false,
        ),
    );
    draw_text(
        &row.source,
        NSRect::new(NSPoint::new(text_left, mid), NSSize::new(text_width, 14.0)),
        &regular(10.5, &white(style::TEXT_SECONDARY), Fit::Tail),
    );
}

pub struct UpdatesView {
    root: Retained<UpdatesRoot>,
    refresh: Retained<TouchButton>,
    chosen: Retained<TouchButton>,
    all: Retained<TouchButton>,
    scroll: Retained<NSScrollView>,
    list: Retained<UpdatesList>,
    rows: RefCell<Vec<Retained<TouchButton>>>,
    on_toggle: Rc<dyn Fn(usize)>,
    mtm: MainThreadMarker,
}

impl UpdatesView {
    /// `frame` is `Notch::page_area(PAGE_SIZE.width, PAGE_SIZE.height)`.
    pub fn new(mtm: MainThreadMarker, frame: NSRect, callbacks: UpdatesCallbacks) -> Rc<Self> {
        let (width, height) = (frame.size.width, frame.size.height);
        let root = UpdatesRoot::new(mtm, frame);
        let UpdatesCallbacks {
            on_toggle,
            on_refresh,
            on_update_chosen,
            on_update_all,
        } = callbacks;
        let refresh = TouchButton::new(
            mtm,
            NSRect::new(
                NSPoint::new(width - MARGIN - 28.0, height - 34.0),
                NSSize::new(28.0, 28.0),
            ),
            Look::Round { filled: false },
        );
        refresh.set_symbol("arrow.clockwise");
        refresh.set_on_click(on_refresh);
        let chosen = TouchButton::new(mtm, NSRect::ZERO, Look::Rect);
        chosen.set_on_click(on_update_chosen);
        let all = TouchButton::new(mtm, NSRect::ZERO, Look::Rect);
        all.set_symbol("arrow.down.circle");
        all.set_on_click(on_update_all);
        for button in [&refresh, &chosen, &all] {
            root.addSubview(button);
        }
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
        let list = UpdatesList::new(mtm, NSRect::new(NSPoint::ZERO, list_frame.size));
        scroll.setDocumentView(Some(&list));
        root.addSubview(&scroll);
        Rc::new(Self {
            root,
            refresh,
            chosen,
            all,
            scroll,
            list,
            rows: RefCell::default(),
            on_toggle: Rc::new(on_toggle),
            mtm,
        })
    }

    pub fn show(&self, display: &UpdatesDisplay) {
        let iv = self.root.ivars();
        *iv.heading.borrow_mut() = display.heading.clone();
        *iv.status.borrow_mut() = display.status.clone();
        *iv.note.borrow_mut() = display.note.clone();
        let listing = display.note.is_none();
        self.refresh.set_title(&display.refresh_label);
        self.refresh.set_enabled(!display.busy);
        self.refresh.setHidden(display.refresh_label.is_empty());
        // The two update buttons sit left of the refresh one, the widest last so the one used most is nearest.
        let mut right = self.refresh.frame().origin.x - BUTTON_GAP;
        let any_checked = display.rows.iter().any(|r| r.checked);
        for (button, label, enabled) in [
            (&self.all, &display.all_label, !display.rows.is_empty()),
            (&self.chosen, &display.chosen_label, any_checked),
        ] {
            button.set_title(label);
            button.set_enabled(enabled && !display.busy);
            button.setHidden(!listing || label.is_empty());
            let w = button.fit_width();
            right -= w;
            button.setFrame(NSRect::new(
                NSPoint::new(right, self.refresh.frame().origin.y),
                NSSize::new(w, 28.0),
            ));
            right -= BUTTON_GAP;
        }
        *iv.buttons_left.borrow_mut() = right + BUTTON_GAP;
        self.scroll.setHidden(!listing);
        let width = self.scroll.contentSize().width;
        *self.list.ivars().rows.borrow_mut() = display.rows.clone();
        let mut pool = self.rows.borrow_mut();
        while pool.len() < display.rows.len() {
            let index = pool.len();
            let button = TouchButton::new(self.mtm, NSRect::ZERO, Look::Overlay);
            let toggle = Rc::clone(&self.on_toggle);
            button.set_on_click(move || toggle(index));
            self.list.addSubview(&button);
            pool.push(button);
        }
        for (i, button) in pool.iter().enumerate() {
            match display.rows.get(i) {
                Some(row) => {
                    button.setHidden(false);
                    button.set_enabled(!display.busy);
                    button.setFrame(NSRect::new(
                        NSPoint::new(0.0, i as f64 * (ROW_HEIGHT + ROW_GAP)),
                        NSSize::new(width, ROW_HEIGHT),
                    ));
                    // The label names the row for VoiceOver, which cannot see what the list draws.
                    button.set_title(&format!(
                        "{}, {} {} {}",
                        row.name, row.source, row.current, row.latest
                    ));
                    button.set_selected(row.checked);
                }
                None => button.setHidden(true),
            }
        }
        let height = (display.rows.len() as f64 * (ROW_HEIGHT + ROW_GAP))
            .max(self.scroll.contentSize().height);
        self.list.setFrameSize(NSSize::new(width, height));
        self.list.setNeedsDisplay(true);
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
            id: "updates",
            title,
            symbol: "arrow.down.circle",
            root: Retained::into_super(Retained::clone(&self.root)),
            on_show: Box::new(on_show),
            on_hide: Box::new(on_hide),
            on_key: Box::new(|_| false),
            accepts_files: false,
            size: None,
        }
    }
}
