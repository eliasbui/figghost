//! The Screen Time tab of the notch: how long the apps were in front, as a total and a list with a bar for each. Everything
//! on it is plain data from the app, so nothing here measures time or knows a language.

use crate::{
    button::{Look, TouchButton},
    notch::Page,
    style::{self, Tone, Weight, white},
    widgets::{Fit, draw_page_note, draw_text, text_attributes},
    workspace,
};
use objc2::{DefinedClass, MainThreadMarker, MainThreadOnly, define_class, msg_send, rc::Retained};
use objc2_app_kit::{NSImage, NSStringDrawing, NSView};
use objc2_foundation::{NSPoint, NSRect, NSSize, NSString};
use std::{
    cell::{Cell, RefCell},
    collections::HashMap,
    path::{Path, PathBuf},
    rc::Rc,
};

/// The size of the area the tab fills below the header.
pub const PAGE_SIZE: NSSize = NSSize::new(580.0, 262.0);
/// How many rows the list has room for.
pub const MAX_ROWS: usize = 6;

const MARGIN: f64 = 18.0;
const ROW_HEIGHT: f64 = 28.0;
const LIST_TOP: f64 = 84.0;
const ICON: f64 = 20.0;
/// The size of the total, and the least it shrinks to when the chips leave it less room.
const TOTAL_SIZE: (f64, f64) = (34.0, 20.0);

/// What a row shows before its name.
#[derive(Debug, Clone)]
pub enum Icon {
    /// The icon the Finder shows for this path, such as an app.
    App(PathBuf),
    Symbol(&'static str),
    None,
}

#[derive(Debug, Clone)]
pub struct Row {
    pub icon: Icon,
    pub title: String,
    /// "1 h 20 min"
    pub value: String,
    /// How long the bar is, 0 to 1, against the longest row.
    pub fraction: f64,
}

#[derive(Debug, Clone, Default)]
pub struct ScreenTimeDisplay {
    /// "Today" or "Last 7 days".
    pub caption: String,
    /// "3 h 42 min"
    pub total: String,
    pub rows: Vec<Row>,
    /// What the list says when it has no rows.
    pub empty: String,
    pub ranges: [String; 2],
    /// By app, by group, by site; a grouping with no name has no chip.
    pub groups: [String; 3],
    /// Which of the two ranges and which of the groupings is chosen.
    pub range: usize,
    pub group: usize,
    /// Words that replace everything: the tab is off.
    pub note: Option<(String, String)>,
}

pub struct ScreenTimeCallbacks {
    pub on_range: Box<dyn Fn(usize)>,
    pub on_group: Box<dyn Fn(usize)>,
}

pub struct CanvasIvars {
    display: RefCell<ScreenTimeDisplay>,
    icons: RefCell<HashMap<PathBuf, Retained<NSImage>>>,
    /// Where the leftmost chip starts, which is as far as the total may reach.
    chips_left: Cell<f64>,
}

fn attrs(
    size: f64,
    weight: Weight,
    alpha: f64,
) -> Retained<
    objc2_foundation::NSMutableDictionary<objc2_foundation::NSString, objc2::runtime::AnyObject>,
> {
    text_attributes(&style::font(size, weight), &white(alpha), Fit::Tail, false)
}

define_class!(
    // SAFETY: NSView allows subclassing; the ivars are only touched on the main thread and need no Drop.
    #[unsafe(super(NSView))]
    #[thread_kind = MainThreadOnly]
    #[name = "FigScreenTimeCanvas"]
    #[ivars = CanvasIvars]
    pub struct ScreenTimeCanvas;

    impl ScreenTimeCanvas {
        #[unsafe(method(drawRect:))]
        fn draw_rect(&self, _dirty: NSRect) {
            self.paint();
        }
    }
);

impl ScreenTimeCanvas {
    fn new(mtm: MainThreadMarker, frame: NSRect) -> Retained<Self> {
        let this = mtm.alloc::<Self>().set_ivars(CanvasIvars {
            display: RefCell::default(),
            icons: RefCell::default(),
            chips_left: Cell::new(f64::MAX),
        });
        // SAFETY: designated initializer with valid arguments on the main thread.
        unsafe { msg_send![super(this), initWithFrame: frame] }
    }

    fn icon_of(&self, path: &Path) -> Retained<NSImage> {
        self.ivars()
            .icons
            .borrow_mut()
            .entry(path.to_path_buf())
            .or_insert_with(|| workspace::icon_of(path, ICON))
            .clone()
    }

    fn paint(&self) {
        let size = self.bounds().size;
        let h = size.height;
        let display = self.ivars().display.borrow();
        if let Some((title, hint)) = &display.note {
            draw_page_note("hourglass", title, hint, size);
            return;
        }
        // Under the chips: what the figure is, and the figure.
        draw_text(
            &display.caption.to_uppercase(),
            NSRect::new(
                NSPoint::new(MARGIN + 2.0, h - 22.0),
                NSSize::new(200.0, 12.0),
            ),
            &attrs(9.5, Weight::Semibold, style::TEXT_TERTIARY),
        );
        let room = (self.ivars().chips_left.get() - MARGIN - 14.0).min(240.0);
        let total = |size: f64| {
            text_attributes(
                &style::digits(size, Weight::Light),
                &white(style::TEXT),
                Fit::Tail,
                false,
            )
        };
        // SAFETY: the attributes dictionary holds valid attribute objects.
        let wide = unsafe {
            NSString::from_str(&display.total).sizeWithAttributes(Some(&total(TOTAL_SIZE.0)))
        }
        .width;
        let fitted = if wide > room {
            (TOTAL_SIZE.0 * room / wide).max(TOTAL_SIZE.1)
        } else {
            TOTAL_SIZE.0
        };
        draw_text(
            &display.total,
            NSRect::new(
                NSPoint::new(MARGIN + 2.0, h - 66.0),
                NSSize::new(room, 42.0),
            ),
            &total(fitted),
        );
        if display.rows.is_empty() {
            draw_text(
                &display.empty,
                NSRect::new(
                    NSPoint::new(MARGIN + 2.0, h - LIST_TOP - 36.0),
                    NSSize::new(size.width - 2.0 * MARGIN, 18.0),
                ),
                &attrs(12.0, Weight::Regular, style::TEXT_SECONDARY),
            );
            return;
        }
        let bar_left = 222.0;
        let bar_width = size.width - MARGIN - 96.0 - bar_left;
        for (i, row) in display.rows.iter().take(MAX_ROWS).enumerate() {
            let top = h - LIST_TOP - i as f64 * ROW_HEIGHT;
            let mid = top - ROW_HEIGHT / 2.0;
            match &row.icon {
                Icon::App(path) => {
                    self.icon_of(path).drawInRect(NSRect::new(
                        NSPoint::new(MARGIN, mid - ICON / 2.0),
                        NSSize::new(ICON, ICON),
                    ));
                }
                Icon::Symbol(name) => style::draw_symbol(
                    name,
                    NSPoint::new(MARGIN + ICON / 2.0, mid),
                    15.0,
                    &white(0.78),
                ),
                Icon::None => {}
            }
            draw_text(
                &row.title,
                NSRect::new(
                    NSPoint::new(MARGIN + ICON + 10.0, mid - 8.0),
                    NSSize::new(bar_left - MARGIN - ICON - 22.0, 16.0),
                ),
                &attrs(12.0, Weight::Medium, style::TEXT),
            );
            let track = NSRect::new(
                NSPoint::new(bar_left, mid - 3.0),
                NSSize::new(bar_width, 6.0),
            );
            style::fill_capsule(track, &white(0.10));
            let fraction = row.fraction.clamp(0.0, 1.0);
            if fraction > 0.0 {
                style::fill_capsule(
                    NSRect::new(
                        track.origin,
                        NSSize::new((bar_width * fraction).max(6.0), 6.0),
                    ),
                    &Tone::Blue.color(),
                );
            }
            draw_text(
                &row.value,
                NSRect::new(
                    NSPoint::new(size.width - MARGIN - 86.0, mid - 7.0),
                    NSSize::new(86.0, 14.0),
                ),
                &text_attributes(
                    &style::digits(11.0, Weight::Medium),
                    &white(style::TEXT_SECONDARY),
                    Fit::Tail,
                    false,
                ),
            );
        }
    }
}

pub struct ScreenTimeView {
    canvas: Retained<ScreenTimeCanvas>,
    ranges: [Retained<TouchButton>; 2],
    groups: [Retained<TouchButton>; 3],
}

impl ScreenTimeView {
    /// `frame` is `Notch::page_area(PAGE_SIZE.width, PAGE_SIZE.height)`.
    pub fn new(mtm: MainThreadMarker, frame: NSRect, callbacks: ScreenTimeCallbacks) -> Rc<Self> {
        let canvas = ScreenTimeCanvas::new(mtm, frame);
        let callbacks = Rc::new(callbacks);
        let chip = |index: usize, range: bool| {
            let button = TouchButton::new(mtm, NSRect::ZERO, Look::Chip);
            let shared = Rc::clone(&callbacks);
            button.set_on_click(move || {
                if range {
                    (shared.on_range)(index);
                } else {
                    (shared.on_group)(index);
                }
            });
            canvas.addSubview(&button);
            button
        };
        Rc::new(Self {
            ranges: [chip(0, true), chip(1, true)],
            groups: [chip(0, false), chip(1, false), chip(2, false)],
            canvas,
        })
    }

    pub fn show(&self, display: ScreenTimeDisplay) {
        let off = display.note.is_some();
        let frame = self.canvas.frame().size;
        // The chips sit at the top right: the two ranges, then the groupings.
        let mut right = frame.width - MARGIN;
        let top = frame.height - 10.0 - 28.0;
        for (buttons, labels, chosen) in [
            (&self.groups[..], &display.groups[..], display.group),
            (&self.ranges[..], &display.ranges[..], display.range),
        ] {
            for (i, button) in buttons.iter().enumerate().rev() {
                button.set_title(&labels[i]);
                button.set_selected(i == chosen);
                button.setHidden(off || labels[i].is_empty());
                if labels[i].is_empty() {
                    continue;
                }
                let width = button.fit_width();
                right -= width;
                button.setFrame(NSRect::new(
                    NSPoint::new(right, top),
                    NSSize::new(width, 28.0),
                ));
                right -= 6.0;
            }
            right -= 10.0;
        }
        self.canvas.ivars().chips_left.set(right + 10.0);
        *self.canvas.ivars().display.borrow_mut() = display;
        self.canvas.setNeedsDisplay(true);
    }

    /// The notch tab for this view.
    pub fn page(
        self: &Rc<Self>,
        title: String,
        on_show: impl Fn() + 'static,
        on_hide: impl Fn() + 'static,
    ) -> Page {
        Page {
            id: "screentime",
            title,
            symbol: "hourglass",
            root: Retained::into_super(Retained::clone(&self.canvas)),
            on_show: Box::new(on_show),
            on_hide: Box::new(on_hide),
            on_key: Box::new(|_| false),
            accepts_files: false,
            size: None,
        }
    }
}
