//! The strip of page icons at the top of the open notch: a capsule with one glyph per page, the current
//! page in white. It replaces a row of text tabs, so any number of pages fit in a small space.

use crate::style::{self, TEXT_SECONDARY, black, fill_capsule, white};
use objc2::{
    AnyThread, DefinedClass, MainThreadMarker, MainThreadOnly, define_class, msg_send, rc::Retained,
};
use objc2_app_kit::{NSEvent, NSTrackingArea, NSTrackingAreaOptions, NSView};
use objc2_foundation::{NSObjectProtocol, NSPoint, NSRect, NSSize, NSString};
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
};

pub const HEIGHT: f64 = 24.0;
const PAD: f64 = 2.0;
const ITEM_WIDTH: f64 = 28.0;
const GLYPH: f64 = 12.0;

/// The width that fits this many pages.
pub fn width_for(count: usize) -> f64 {
    2.0 * PAD + count as f64 * ITEM_WIDTH
}

type SelectHandler = Rc<dyn Fn(usize)>;

pub struct SwitcherIvars {
    items: RefCell<Vec<(String, String)>>,
    selected: Cell<usize>,
    /// Pages with something that needs the user, marked with a dot.
    badges: RefCell<Vec<usize>>,
    hovered: Cell<Option<usize>>,
    on_select: RefCell<Option<SelectHandler>>,
}

define_class!(
    // SAFETY: NSView allows subclassing; the ivars are only touched on the main thread and need no Drop.
    #[unsafe(super(NSView))]
    #[thread_kind = MainThreadOnly]
    #[name = "FigPageSwitcher"]
    #[ivars = SwitcherIvars]
    pub struct PageSwitcher;

    impl PageSwitcher {
        #[unsafe(method(acceptsFirstMouse:))]
        fn accepts_first_mouse(&self, _event: Option<&NSEvent>) -> bool {
            true
        }

        #[unsafe(method(drawRect:))]
        fn draw_rect(&self, _dirty: NSRect) {
            self.paint();
        }

        #[unsafe(method(mouseMoved:))]
        fn mouse_moved(&self, event: &NSEvent) {
            let index = self.index_at(event);
            if self.ivars().hovered.replace(index) != index {
                let title = index.and_then(|i| self.ivars().items.borrow().get(i).map(|(_, t)| t.clone()));
                self.setToolTip(title.as_deref().map(NSString::from_str).as_deref());
                self.setNeedsDisplay(true);
            }
        }

        #[unsafe(method(mouseExited:))]
        fn mouse_exited(&self, _event: &NSEvent) {
            if self.ivars().hovered.replace(None).is_some() {
                self.setNeedsDisplay(true);
            }
        }

        #[unsafe(method(mouseDown:))]
        fn mouse_down(&self, event: &NSEvent) {
            if let Some(index) = self.index_at(event) {
                let select = self.ivars().on_select.borrow().clone();
                if let Some(select) = select {
                    select(index);
                }
            }
        }
    }

    unsafe impl NSObjectProtocol for PageSwitcher {}
);

impl PageSwitcher {
    pub fn new(mtm: MainThreadMarker, origin: NSPoint) -> Retained<Self> {
        let ivars = SwitcherIvars {
            items: RefCell::new(Vec::new()),
            selected: Cell::new(0),
            badges: RefCell::new(Vec::new()),
            hovered: Cell::new(None),
            on_select: RefCell::new(None),
        };
        let frame = NSRect::new(origin, NSSize::new(width_for(0), HEIGHT));
        // SAFETY: designated initializer with valid arguments on the main thread.
        let this: Retained<Self> =
            unsafe { msg_send![super(mtm.alloc::<Self>().set_ivars(ivars)), initWithFrame: frame] };
        // SAFETY: valid options and owner; InVisibleRect ignores the (empty) rect.
        let area = unsafe {
            NSTrackingArea::initWithRect_options_owner_userInfo(
                NSTrackingArea::alloc(),
                NSRect::ZERO,
                NSTrackingAreaOptions::MouseMoved
                    | NSTrackingAreaOptions::MouseEnteredAndExited
                    | NSTrackingAreaOptions::ActiveAlways
                    | NSTrackingAreaOptions::InVisibleRect,
                Some(&this),
                None,
            )
        };
        this.addTrackingArea(&area);
        this
    }

    pub fn set_on_select(&self, f: impl Fn(usize) + 'static) {
        *self.ivars().on_select.borrow_mut() = Some(Rc::new(f));
    }

    /// One `(SF Symbol name, title)` per page. The strip resizes to fit them.
    pub fn set_items(&self, items: Vec<(String, String)>) {
        let width = width_for(items.len());
        *self.ivars().items.borrow_mut() = items;
        self.setFrameSize(NSSize::new(width, HEIGHT));
        self.setNeedsDisplay(true);
    }

    pub fn set_selected(&self, index: usize) {
        self.ivars().selected.set(index);
        self.setNeedsDisplay(true);
    }

    /// Marks a page icon with a dot, or clears the mark.
    pub fn set_badge(&self, index: usize, on: bool) {
        let mut badges = self.ivars().badges.borrow_mut();
        badges.retain(|&i| i != index);
        if on {
            badges.push(index);
        }
        drop(badges);
        self.setNeedsDisplay(true);
    }

    fn index_at(&self, event: &NSEvent) -> Option<usize> {
        let point = self.convertPoint_fromView(event.locationInWindow(), None);
        let relative = point.x - PAD;
        if relative < 0.0 || point.y < 0.0 || point.y > HEIGHT {
            return None;
        }
        let index = (relative / ITEM_WIDTH) as usize;
        (index < self.ivars().items.borrow().len()).then_some(index)
    }

    fn paint(&self) {
        let bounds = self.bounds();
        fill_capsule(bounds, &white(style::CONTROL));
        let items = self.ivars().items.borrow();
        let (selected, hovered) = (self.ivars().selected.get(), self.ivars().hovered.get());
        for (i, (symbol, _)) in items.iter().enumerate() {
            let cell = NSRect::new(
                NSPoint::new(PAD + i as f64 * ITEM_WIDTH, PAD),
                NSSize::new(ITEM_WIDTH, HEIGHT - 2.0 * PAD),
            );
            let on = i == selected;
            if on {
                fill_capsule(cell, &white(0.94));
            } else if hovered == Some(i) {
                fill_capsule(cell, &white(0.12));
            }
            let ink = if on {
                black(0.88)
            } else {
                white(TEXT_SECONDARY + 0.12)
            };
            style::draw_symbol(
                symbol,
                NSPoint::new(
                    cell.origin.x + cell.size.width / 2.0,
                    cell.origin.y + cell.size.height / 2.0,
                ),
                GLYPH,
                &ink,
            );
            if self.ivars().badges.borrow().contains(&i) {
                let dot = NSRect::new(
                    NSPoint::new(
                        cell.origin.x + cell.size.width - 9.0,
                        cell.origin.y + cell.size.height - 7.5,
                    ),
                    NSSize::new(6.0, 6.0),
                );
                style::fill_circle(dot, &style::Tone::Orange.color());
            }
        }
    }
}
