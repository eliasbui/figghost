//! The strip across the top of the editor: tools, colour, thickness and fill, then the actions.

use super::{Strings, render::ns_colour};
use crate::{
    button::{Look, TouchButton},
    style::{black, fill_capsule, fill_circle, white},
};
use core_annotate::{Colour, PALETTE, Tool, WIDTHS};
use objc2::{DefinedClass, MainThreadMarker, MainThreadOnly, define_class, msg_send, rc::Retained};
use objc2_app_kit::{
    NSAccessibilityRadioGroupRole, NSAutoresizingMaskOptions, NSBezierPath, NSColor, NSEvent,
    NSView,
};
use objc2_foundation::{NSCopying, NSObjectProtocol, NSPoint, NSRect, NSSize, NSString};
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
};

pub const HEIGHT: f64 = 48.0;
const BUTTON: f64 = 32.0;
const GAP: f64 = 2.0;
const MARGIN: f64 = 12.0;
const SEPARATOR: f64 = 14.0;
const COLOUR_CELL: f64 = 22.0;
const WIDTH_CELL: f64 = 26.0;

/// The window background, a little darker than the strip.
pub const SURROUND: f64 = 0.075;
const STRIP: f64 = 0.115;

pub enum StripKind {
    Colours(Vec<Colour>),
    Widths(Vec<f64>),
}

type Pick = Rc<dyn Fn(usize)>;

pub struct StripIvars {
    kind: StripKind,
    cell: f64,
    label: String,
    selected: Cell<Option<usize>>,
    on_pick: RefCell<Option<Pick>>,
}

define_class!(
    // SAFETY: NSView allows subclassing; the ivars are only touched on the main thread and need no Drop.
    #[unsafe(super(NSView))]
    #[thread_kind = MainThreadOnly]
    #[name = "FigChoiceStrip"]
    #[ivars = StripIvars]
    pub struct ChoiceStrip;

    impl ChoiceStrip {
        #[unsafe(method(acceptsFirstMouse:))]
        fn accepts_first_mouse(&self, _event: Option<&NSEvent>) -> bool {
            true
        }

        #[unsafe(method(drawRect:))]
        fn draw_rect(&self, _dirty: NSRect) {
            self.paint();
        }

        #[unsafe(method(mouseDown:))]
        fn mouse_down(&self, event: &NSEvent) {
            let at = self.convertPoint_fromView(event.locationInWindow(), None);
            let iv = self.ivars();
            let index = (at.x / iv.cell).floor();
            let count = self.count();
            if at.x >= 0.0 && index >= 0.0 && (index as usize) < count {
                let pick = iv.on_pick.borrow().clone();
                if let Some(pick) = pick {
                    pick(index as usize);
                }
            }
        }

        #[unsafe(method(isAccessibilityElement))]
        fn is_accessibility_element(&self) -> bool {
            true
        }

        #[unsafe(method_id(accessibilityRole))]
        fn accessibility_role(&self) -> Option<Retained<NSString>> {
            // SAFETY: an extern static constant that lives as long as the process.
            Some(unsafe { NSAccessibilityRadioGroupRole }.copy())
        }

        #[unsafe(method_id(accessibilityLabel))]
        fn accessibility_label(&self) -> Option<Retained<NSString>> {
            Some(NSString::from_str(&self.ivars().label))
        }
    }

    unsafe impl NSObjectProtocol for ChoiceStrip {}
);

impl ChoiceStrip {
    fn new(mtm: MainThreadMarker, origin: NSPoint, kind: StripKind, label: &str) -> Retained<Self> {
        let (cell, count) = match &kind {
            StripKind::Colours(c) => (COLOUR_CELL, c.len()),
            StripKind::Widths(w) => (WIDTH_CELL, w.len()),
        };
        let frame = NSRect::new(origin, NSSize::new(cell * count as f64, BUTTON));
        let this = mtm.alloc::<Self>().set_ivars(StripIvars {
            kind,
            cell,
            label: label.to_owned(),
            selected: Cell::new(None),
            on_pick: RefCell::new(None),
        });
        // SAFETY: designated initializer with valid arguments on the main thread.
        let this: Retained<Self> = unsafe { msg_send![super(this), initWithFrame: frame] };
        this.setToolTip(Some(&NSString::from_str(label)));
        this
    }

    fn count(&self) -> usize {
        match &self.ivars().kind {
            StripKind::Colours(c) => c.len(),
            StripKind::Widths(w) => w.len(),
        }
    }

    fn set_selected(&self, selected: Option<usize>) {
        if self.ivars().selected.replace(selected) != selected {
            self.setNeedsDisplay(true);
        }
    }

    fn paint(&self) {
        let iv = self.ivars();
        let mid = self.bounds().size.height / 2.0;
        for i in 0..self.count() {
            let centre = NSPoint::new(iv.cell * (i as f64 + 0.5), mid);
            let chosen = iv.selected.get() == Some(i);
            match &iv.kind {
                StripKind::Colours(colours) => {
                    let dot = |d: f64| {
                        NSRect::new(
                            NSPoint::new(centre.x - d / 2.0, centre.y - d / 2.0),
                            NSSize::new(d, d),
                        )
                    };
                    fill_circle(dot(15.0), &ns_colour(colours[i], 1.0));
                    // A hairline keeps the darkest and the lightest swatch apart from the strip.
                    white(0.30).setStroke();
                    let edge = NSBezierPath::bezierPathWithOvalInRect(dot(14.5));
                    edge.setLineWidth(1.0);
                    edge.stroke();
                    if chosen {
                        white(0.95).setStroke();
                        let ring = NSBezierPath::bezierPathWithOvalInRect(dot(21.0));
                        ring.setLineWidth(2.0);
                        ring.stroke();
                    }
                }
                StripKind::Widths(widths) => {
                    let cell = NSRect::new(
                        NSPoint::new(
                            centre.x - iv.cell / 2.0 + 1.0,
                            centre.y - BUTTON / 2.0 + 3.0,
                        ),
                        NSSize::new(iv.cell - 2.0, BUTTON - 6.0),
                    );
                    if chosen {
                        fill_capsule(cell, &white(0.93));
                    }
                    let d = (widths[i] * 0.9 + 2.0).min(15.0);
                    let ink = if chosen { black(0.88) } else { white(0.72) };
                    fill_circle(
                        NSRect::new(
                            NSPoint::new(centre.x - d / 2.0, centre.y - d / 2.0),
                            NSSize::new(d, d),
                        ),
                        &ink,
                    );
                }
            }
        }
    }
}

pub struct BarIvars {
    left_separators: RefCell<Vec<f64>>,
    /// Distances from the right edge, for the separators that move with it.
    right_separators: RefCell<Vec<f64>>,
}

define_class!(
    // SAFETY: NSView allows subclassing; the ivars are only touched on the main thread and need no Drop.
    #[unsafe(super(NSView))]
    #[thread_kind = MainThreadOnly]
    #[name = "FigAnnotateBar"]
    #[ivars = BarIvars]
    pub struct BarView;

    impl BarView {
        #[unsafe(method(drawRect:))]
        fn draw_rect(&self, _dirty: NSRect) {
            let bounds = self.bounds();
            NSColor::colorWithWhite_alpha(STRIP, 1.0).setFill();
            objc2_app_kit::NSRectFill(bounds);
            white(0.08).setFill();
            objc2_app_kit::NSRectFill(NSRect::new(NSPoint::ZERO, NSSize::new(bounds.size.width, 1.0)));
            white(0.14).setFill();
            let iv = self.ivars();
            let rights = iv.right_separators.borrow().iter().map(|d| bounds.size.width - d).collect::<Vec<_>>();
            for x in iv.left_separators.borrow().iter().copied().chain(rights) {
                objc2_app_kit::NSRectFill(NSRect::new(
                    NSPoint::new(x.round(), 12.0),
                    NSSize::new(1.0, bounds.size.height - 24.0),
                ));
            }
        }
    }
);

/// What the strip shows besides its fixed buttons.
#[derive(Debug, Clone, Copy)]
pub struct BarState {
    pub tool: Tool,
    pub colour: Colour,
    pub width: f64,
    pub fill: bool,
    pub can_undo: bool,
    pub can_redo: bool,
    pub can_delete: bool,
}

/// What the buttons do. Each closure runs on the main thread.
pub struct BarActions {
    pub tool: Rc<dyn Fn(Tool)>,
    pub colour: Rc<dyn Fn(Colour)>,
    pub width: Rc<dyn Fn(f64)>,
    pub fill: Rc<dyn Fn()>,
    pub undo: Rc<dyn Fn()>,
    pub redo: Rc<dyn Fn()>,
    pub delete: Rc<dyn Fn()>,
    pub copy: Rc<dyn Fn()>,
    pub save: Rc<dyn Fn()>,
    pub done: Rc<dyn Fn()>,
}

/// Everything on the strip that can be clicked.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Button {
    Tool(Tool),
    Colour(usize),
    Width(usize),
    Fill,
    Undo,
    Redo,
    Delete,
    Copy,
    Save,
    Done,
}

pub struct Toolbar {
    pub view: Retained<BarView>,
    tools: Vec<(Tool, Retained<TouchButton>)>,
    copy: Retained<TouchButton>,
    save: Retained<TouchButton>,
    done: Retained<TouchButton>,
    colours: Retained<ChoiceStrip>,
    widths: Retained<ChoiceStrip>,
    fill: Retained<TouchButton>,
    undo: Retained<TouchButton>,
    redo: Retained<TouchButton>,
    delete: Retained<TouchButton>,
    min_width: f64,
}

impl Toolbar {
    pub fn new(mtm: MainThreadMarker, width: f64, strings: &Strings, actions: BarActions) -> Self {
        let frame = NSRect::new(NSPoint::ZERO, NSSize::new(width, HEIGHT));
        let view: Retained<BarView> = {
            let this = mtm.alloc::<BarView>().set_ivars(BarIvars {
                left_separators: RefCell::default(),
                right_separators: RefCell::default(),
            });
            // SAFETY: designated initializer with valid arguments on the main thread.
            unsafe { msg_send![super(this), initWithFrame: frame] }
        };
        let top = (HEIGHT - BUTTON) / 2.0;
        let button = |x: f64, symbol: &str, title: &str, click: Rc<dyn Fn()>| {
            let b = TouchButton::new(
                mtm,
                NSRect::new(NSPoint::new(x, top), NSSize::new(BUTTON, BUTTON)),
                Look::Round { filled: false },
            );
            b.set_symbol(symbol);
            b.set_title(title);
            b.set_on_click(move || click());
            b
        };
        let mut separators = Vec::new();
        let mut x = MARGIN;

        let mut tools = Vec::new();
        for (tool, symbol, title) in [
            (Tool::Select, "cursorarrow", &strings.select),
            (Tool::Arrow, "arrow.up.right", &strings.arrow),
            (Tool::Line, "line.diagonal", &strings.line),
            (Tool::Rectangle, "rectangle", &strings.rectangle),
            (Tool::Ellipse, "circle", &strings.ellipse),
            (Tool::Text, "textformat", &strings.text),
            (Tool::Pen, "pencil.tip", &strings.pen),
            (Tool::Highlighter, "highlighter", &strings.highlighter),
            (Tool::Blur, "squareshape.split.3x3", &strings.blur),
            (Tool::Crop, "crop", &strings.crop),
        ] {
            let pick = Rc::clone(&actions.tool);
            let b = button(x, symbol, title, Rc::new(move || pick(tool)));
            view.addSubview(&b);
            tools.push((tool, b));
            x += BUTTON + GAP;
        }
        x += SEPARATOR - GAP;
        separators.push(x - SEPARATOR / 2.0);

        let colours = ChoiceStrip::new(
            mtm,
            NSPoint::new(x, top),
            StripKind::Colours(PALETTE.to_vec()),
            &strings.colour,
        );
        let pick = Rc::clone(&actions.colour);
        colours
            .ivars()
            .on_pick
            .replace(Some(Rc::new(move |i| pick(PALETTE[i]))));
        view.addSubview(&colours);
        x += colours.frame().size.width + SEPARATOR;
        separators.push(x - SEPARATOR / 2.0);

        let widths = ChoiceStrip::new(
            mtm,
            NSPoint::new(x, top),
            StripKind::Widths(WIDTHS.to_vec()),
            &strings.thickness,
        );
        let pick = Rc::clone(&actions.width);
        widths
            .ivars()
            .on_pick
            .replace(Some(Rc::new(move |i| pick(WIDTHS[i]))));
        view.addSubview(&widths);
        x += widths.frame().size.width + GAP;

        let fill = button(
            x,
            "circle.lefthalf.filled",
            &strings.fill,
            Rc::clone(&actions.fill),
        );
        view.addSubview(&fill);
        x += BUTTON;
        let left_end = x;

        // The right group is laid out from the right edge and stays there when the window is resized.
        let done = TouchButton::new(
            mtm,
            NSRect::new(NSPoint::ZERO, NSSize::new(0.0, BUTTON)),
            Look::Rect,
        );
        done.set_symbol("checkmark");
        done.set_title(&strings.done);
        done.set_selected(true);
        let finish = Rc::clone(&actions.done);
        done.set_on_click(move || finish());
        let done_width = done.fit_width() + 8.0;
        let mut from_right = MARGIN + done_width;
        done.setFrame(NSRect::new(
            NSPoint::new(width - from_right, top),
            NSSize::new(done_width, BUTTON),
        ));
        done.setAutoresizingMask(NSAutoresizingMaskOptions::ViewMinXMargin);
        view.addSubview(&done);
        let mut right_separators = vec![from_right + SEPARATOR / 2.0];
        from_right += SEPARATOR;
        let mut right_buttons = Vec::new();
        for (symbol, title, click) in [
            (
                "square.and.arrow.down",
                &strings.save,
                Rc::clone(&actions.save),
            ),
            ("doc.on.doc", &strings.copy, Rc::clone(&actions.copy)),
        ] {
            let b = button(width - from_right - BUTTON, symbol, title, click);
            b.setAutoresizingMask(NSAutoresizingMaskOptions::ViewMinXMargin);
            view.addSubview(&b);
            right_buttons.push(b);
            from_right += BUTTON + GAP;
        }
        let (copy, save) = (right_buttons.pop().unwrap(), right_buttons.pop().unwrap());
        from_right += SEPARATOR - GAP;
        right_separators.push(from_right - SEPARATOR / 2.0);
        let mut history = Vec::new();
        for (symbol, title, click) in [
            ("trash", &strings.delete, Rc::clone(&actions.delete)),
            (
                "arrow.uturn.forward",
                &strings.redo,
                Rc::clone(&actions.redo),
            ),
            (
                "arrow.uturn.backward",
                &strings.undo,
                Rc::clone(&actions.undo),
            ),
        ] {
            let b = button(width - from_right - BUTTON, symbol, title, click);
            b.setAutoresizingMask(NSAutoresizingMaskOptions::ViewMinXMargin);
            view.addSubview(&b);
            history.push(b);
            from_right += BUTTON + GAP;
        }
        let (undo, redo, delete) = (
            history.pop().unwrap(),
            history.pop().unwrap(),
            history.pop().unwrap(),
        );
        let min_width = left_end + SEPARATOR + from_right - GAP;
        *view.ivars().left_separators.borrow_mut() = separators;
        *view.ivars().right_separators.borrow_mut() = right_separators;
        view.setAutoresizingMask(
            NSAutoresizingMaskOptions::ViewWidthSizable | NSAutoresizingMaskOptions::ViewMinYMargin,
        );
        Self {
            view,
            tools,
            copy,
            save,
            done,
            colours,
            widths,
            fill,
            undo,
            redo,
            delete,
            min_width,
        }
    }

    pub fn min_width(&self) -> f64 {
        self.min_width
    }

    pub fn sync(&self, state: BarState) {
        for (tool, button) in &self.tools {
            // A round button is solid white while it is the active one.
            button.set_look(Look::Round {
                filled: *tool == state.tool,
            });
        }
        self.colours
            .set_selected(PALETTE.iter().position(|c| *c == state.colour));
        self.widths
            .set_selected(WIDTHS.iter().position(|w| (*w - state.width).abs() < 1e-9));
        self.fill.set_look(Look::Round { filled: state.fill });
        self.undo.set_enabled(state.can_undo);
        self.redo.set_enabled(state.can_redo);
        self.delete.set_enabled(state.can_delete);
    }

    /// The middle of a clickable thing, in window coordinates.
    pub fn centre_in_window(&self, which: Button) -> Option<NSPoint> {
        let (view, local): (&NSView, NSPoint) = match which {
            Button::Tool(tool) => {
                let b = &self.tools.iter().find(|(t, _)| *t == tool)?.1;
                (b, middle(b))
            }
            Button::Colour(i) => {
                let n = PALETTE.len();
                (&self.colours, strip_point(&self.colours, i, n))
            }
            Button::Width(i) => {
                let n = WIDTHS.len();
                (&self.widths, strip_point(&self.widths, i, n))
            }
            Button::Fill => (&self.fill, middle(&self.fill)),
            Button::Undo => (&self.undo, middle(&self.undo)),
            Button::Redo => (&self.redo, middle(&self.redo)),
            Button::Delete => (&self.delete, middle(&self.delete)),
            Button::Copy => (&self.copy, middle(&self.copy)),
            Button::Save => (&self.save, middle(&self.save)),
            Button::Done => (&self.done, middle(&self.done)),
        };
        Some(view.convertPoint_toView(local, None))
    }
}

fn middle(view: &NSView) -> NSPoint {
    let b = view.bounds();
    NSPoint::new(b.size.width / 2.0, b.size.height / 2.0)
}

fn strip_point(strip: &ChoiceStrip, index: usize, count: usize) -> NSPoint {
    let b = strip.bounds();
    NSPoint::new(
        b.size.width / count as f64 * (index as f64 + 0.5),
        b.size.height / 2.0,
    )
}
