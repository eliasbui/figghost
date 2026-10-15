//! The editor window: a panel that takes the keyboard without activating the app, the toolbar over a
//! scrolling canvas, and the controller that joins the model to them.

use super::{
    Outcome, Request, Strings,
    canvas::{CanvasView, CenterClip, Phase, TextWatcher, Toast, cursor_for},
    render::{
        self, Source, draw_text_block, export_png, ns_colour, ns_rect, paint_document, text_font,
    },
    toolbar::{self, BarActions, BarState, Button, Toolbar},
};
use crate::{
    pasteboard::{Pasteboard, Write},
    style::{Tone, white},
};
use core_annotate::{
    Colour, Effect, Kind, Mods, Object, Point, Rect, Session, Size, Tool,
    layout::follow_resize,
    naming::{Stamp, file_name},
    shape::MIN_TEXT_WIDTH,
};
use objc2::{
    DefinedClass, MainThreadMarker, MainThreadOnly, define_class, msg_send, rc::Retained,
    runtime::ProtocolObject,
};
use objc2_app_kit::{
    NSAppearance, NSAppearanceCustomization, NSAppearanceNameDarkAqua, NSAutoresizingMaskOptions,
    NSBackingStoreType, NSBezierPath, NSBitmapImageFileType, NSColor, NSEvent,
    NSEventModifierFlags, NSEventType, NSGraphicsContext, NSImage, NSPanel, NSScreen, NSScrollView,
    NSTextInputClient, NSTextView, NSView, NSWindow, NSWindowAnimationBehavior,
    NSWindowCollectionBehavior, NSWindowDelegate, NSWindowStyleMask,
};
use objc2_core_graphics::CGContext;
use objc2_foundation::{
    NSDictionary, NSNotification, NSObject, NSObjectProtocol, NSPoint, NSRange, NSRect, NSSize,
    NSString,
};
use std::{
    cell::{Cell, RefCell},
    fs::OpenOptions,
    io::{self, Write as _},
    path::{Path, PathBuf},
    rc::{Rc, Weak},
};

/// The margin around the canvas, where a drag can start outside the picture.
pub const PAD: f64 = 28.0;
const MIN_VISIBLE_HEIGHT: f64 = 220.0;
const TOAST_SECONDS: f64 = 2.4;
const NUDGE: f64 = 1.0;
const NUDGE_FAR: f64 = 10.0;
/// Where the window goes in tests that must not touch a screen.
const OFFSCREEN_AREA: NSRect =
    NSRect::new(NSPoint::new(-30_000.0, 0.0), NSSize::new(1_920.0, 1_055.0));

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Placement {
    /// Centred on the display that has the pointer.
    PointerScreen,
    /// Far outside every screen, for checks.
    Offscreen,
}

impl Placement {
    /// Follows the variable the rest of the app uses to keep its windows off the screen in tests.
    pub fn from_env() -> Self {
        if std::env::var_os("FIGGHOST_NOTCH_OFFSCREEN").is_some() {
            Self::Offscreen
        } else {
            Self::PointerScreen
        }
    }
}

type KeyHandler = Rc<dyn Fn(&NSEvent) -> bool>;

pub struct PanelIvars {
    on_key: RefCell<Option<KeyHandler>>,
    free: Cell<bool>,
}

define_class!(
    // SAFETY: NSPanel allows subclassing; the ivars are only touched on the main thread and need no Drop.
    #[unsafe(super(NSPanel))]
    #[thread_kind = MainThreadOnly]
    #[name = "FigAnnotatePanel"]
    #[ivars = PanelIvars]
    pub struct EditorPanel;

    impl EditorPanel {
        #[unsafe(method(canBecomeKeyWindow))]
        fn can_become_key_window(&self) -> bool {
            true
        }

        #[unsafe(method(canBecomeMainWindow))]
        fn can_become_main_window(&self) -> bool {
            false
        }

        // A window placed outside every screen would be pulled back onto one.
        #[unsafe(method(constrainFrameRect:toScreen:))]
        fn constrain_frame_rect(&self, frame: NSRect, screen: Option<&NSScreen>) -> NSRect {
            if self.ivars().free.get() {
                return frame;
            }
            // SAFETY: forwards the same arguments to NSPanel's own implementation.
            unsafe { msg_send![super(self), constrainFrameRect: frame, toScreen: screen] }
        }

        #[unsafe(method(sendEvent:))]
        fn send_event(&self, event: &NSEvent) {
            if event.r#type() == NSEventType::KeyDown {
                let handler = self.ivars().on_key.borrow().clone();
                if handler.is_some_and(|h| h(event)) {
                    return;
                }
            }
            // SAFETY: forwards the same event to NSPanel's own implementation.
            let _: () = unsafe { msg_send![super(self), sendEvent: event] };
        }
    }
);

pub struct DelegateIvars {
    on_close: Box<dyn Fn()>,
}

define_class!(
    // SAFETY: NSObject allows subclassing; the ivars are only touched on the main thread and need no Drop.
    #[unsafe(super(NSObject))]
    #[thread_kind = MainThreadOnly]
    #[name = "FigAnnotateDelegate"]
    #[ivars = DelegateIvars]
    pub struct EditorDelegate;

    unsafe impl NSObjectProtocol for EditorDelegate {}

    unsafe impl NSWindowDelegate for EditorDelegate {
        #[unsafe(method(windowWillClose:))]
        fn window_will_close(&self, _notification: &NSNotification) {
            (self.ivars().on_close)();
        }
    }
);

struct Overlay {
    view: Retained<NSTextView>,
    watcher: Retained<TextWatcher>,
}

pub struct Shared {
    mtm: MainThreadMarker,
    me: Weak<Shared>,
    pub(super) panel: Retained<EditorPanel>,
    delegate: Retained<EditorDelegate>,
    toolbar: Toolbar,
    scroll: Retained<NSScrollView>,
    clip: Retained<CenterClip>,
    pub(super) canvas: Retained<CanvasView>,
    toast: Retained<Toast>,
    session: RefCell<Session>,
    source: RefCell<Option<Rc<Source>>>,
    overlay: RefCell<Option<Overlay>>,
    last_canvas: Cell<Rect>,
    area: NSRect,
    titlebar: f64,
    strings: Strings,
    save_dir: PathBuf,
    pasteboard: Pasteboard,
    on_finish: Box<dyn Fn(Outcome)>,
    outcome: RefCell<Outcome>,
    finished: Cell<bool>,
    toast_count: Cell<u32>,
}

fn work_area(mtm: MainThreadMarker, placement: Placement) -> NSRect {
    if placement == Placement::Offscreen {
        return OFFSCREEN_AREA;
    }
    let mouse = NSEvent::mouseLocation();
    let screens = NSScreen::screens(mtm);
    let under_pointer = screens.iter().find(|s| {
        let f = s.frame();
        mouse.x >= f.origin.x
            && mouse.x < f.origin.x + f.size.width
            && mouse.y >= f.origin.y
            && mouse.y < f.origin.y + f.size.height
    });
    under_pointer
        .or_else(|| screens.firstObject())
        .map_or(OFFSCREEN_AREA, |s| s.visibleFrame())
}

fn local_stamp() -> Stamp {
    // SAFETY: time and localtime_r are given valid pointers to values of their own types.
    let tm = unsafe {
        let now = libc::time(std::ptr::null_mut());
        let mut tm: libc::tm = std::mem::zeroed();
        libc::localtime_r(&now, &mut tm);
        tm
    };
    Stamp {
        year: tm.tm_year + 1900,
        month: tm.tm_mon as u32 + 1,
        day: tm.tm_mday as u32,
        hour: tm.tm_hour as u32,
        minute: tm.tm_min as u32,
        second: tm.tm_sec as u32,
    }
}

/// Writes `png` into `dir` under a name that is not taken, never replacing a file. A file that could not be
/// written to the end is removed rather than left half done.
pub fn write_unique(dir: &Path, stamp: Stamp, png: &[u8]) -> io::Result<PathBuf> {
    std::fs::create_dir_all(dir)?;
    for attempt in 0..1000 {
        let path = dir.join(file_name(stamp, attempt));
        match OpenOptions::new().write(true).create_new(true).open(&path) {
            Ok(mut file) => {
                if let Err(error) = file.write_all(png).and_then(|()| file.sync_all()) {
                    drop(file);
                    let _ = std::fs::remove_file(&path);
                    return Err(error);
                }
                return Ok(path);
            }
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => continue,
            Err(error) => return Err(error),
        }
    }
    Err(io::Error::new(
        io::ErrorKind::AlreadyExists,
        "every name is taken",
    ))
}

fn is_plain(flags: NSEventModifierFlags) -> bool {
    !flags.intersects(
        NSEventModifierFlags::Command
            | NSEventModifierFlags::Option
            | NSEventModifierFlags::Control
            | NSEventModifierFlags::Shift,
    )
}

pub struct Setup {
    pub request: Request,
    pub placement: Placement,
    pub pasteboard: Pasteboard,
}

impl Shared {
    pub fn build(mtm: MainThreadMarker, source: Source, setup: Setup) -> Rc<Shared> {
        let Setup {
            request,
            placement,
            pasteboard,
        } = setup;
        let (px_w, px_h) = source.size_pixels();
        let mut session = Session::new(px_w, px_h, request.scale);
        session.set_text_measure(Box::new(render::measure_text));
        let canvas_rect = session.doc().canvas();
        let area = work_area(mtm, placement);
        let strings = request.strings;
        let title = if request.title.is_empty() {
            strings.window_title.clone()
        } else {
            request.title.clone()
        };

        let shared = Rc::new_cyclic(|me: &Weak<Shared>| {
            let call = |f: fn(&Shared)| -> Rc<dyn Fn()> {
                let me = me.clone();
                Rc::new(move || {
                    if let Some(shared) = me.upgrade() {
                        f(&shared);
                    }
                })
            };
            let actions = BarActions {
                tool: {
                    let me = me.clone();
                    Rc::new(move |tool| {
                        if let Some(s) = me.upgrade() {
                            s.choose_tool(tool);
                        }
                    })
                },
                colour: {
                    let me = me.clone();
                    Rc::new(move |colour| {
                        if let Some(s) = me.upgrade() {
                            s.choose_colour(colour);
                        }
                    })
                },
                width: {
                    let me = me.clone();
                    Rc::new(move |width| {
                        if let Some(s) = me.upgrade() {
                            s.choose_width(width);
                        }
                    })
                },
                fill: call(Shared::toggle_fill),
                undo: call(Shared::undo),
                redo: call(Shared::redo),
                delete: call(Shared::delete),
                copy: call(|s| {
                    s.copy();
                }),
                save: call(Shared::save),
                done: call(Shared::done),
            };
            let toolbar = Toolbar::new(mtm, 1.0e3, &strings, actions);

            let style = NSWindowStyleMask::Titled
                | NSWindowStyleMask::Closable
                | NSWindowStyleMask::Resizable
                | NSWindowStyleMask::NonactivatingPanel;
            let probe = NSRect::new(NSPoint::ZERO, NSSize::new(100.0, 100.0));
            let titlebar = NSWindow::frameRectForContentRect_styleMask(probe, style, mtm)
                .size
                .height
                - probe.size.height;
            let max = Self::max_visible(area, titlebar);
            let min = Size::new(toolbar.min_width(), MIN_VISIBLE_HEIGHT);
            let want = Size::new(canvas_rect.w + 2.0 * PAD, canvas_rect.h + 2.0 * PAD);
            let visible = Size::new(
                want.w.max(min.w).min(max.w.max(min.w)),
                want.h.max(min.h).min(max.h.max(min.h)),
            );
            let content = NSRect::new(
                NSPoint::ZERO,
                NSSize::new(visible.w, visible.h + toolbar::HEIGHT),
            );

            // SAFETY: designated initializer with valid arguments on the main thread.
            let panel: Retained<EditorPanel> = unsafe {
                msg_send![
                    super(mtm.alloc::<EditorPanel>().set_ivars(PanelIvars {
                        on_key: RefCell::new(None),
                        free: Cell::new(placement == Placement::Offscreen),
                    })),
                    initWithContentRect: content,
                    styleMask: style,
                    backing: NSBackingStoreType::Buffered,
                    defer: false
                ]
            };
            // The Rust side owns the panel; AppKit must not release it on close.
            // SAFETY: false is the only value compatible with a window kept alive by `Retained`.
            unsafe { panel.setReleasedWhenClosed(false) };
            panel.setTitle(&NSString::from_str(&title));
            panel.setTitlebarAppearsTransparent(true);
            panel.setFloatingPanel(true);
            panel.setHidesOnDeactivate(false);
            panel.setBecomesKeyOnlyIfNeeded(false);
            panel.setAnimationBehavior(NSWindowAnimationBehavior::None);
            panel.setCollectionBehavior(
                NSWindowCollectionBehavior::MoveToActiveSpace
                    | NSWindowCollectionBehavior::FullScreenAuxiliary,
            );
            panel.setBackgroundColor(Some(&NSColor::colorWithWhite_alpha(toolbar::SURROUND, 1.0)));
            // SAFETY: extern statics are process-lifetime constants.
            let dark = unsafe { NSAppearance::appearanceNamed(NSAppearanceNameDarkAqua) };
            panel.setAppearance(dark.as_deref());
            panel.setContentMinSize(NSSize::new(min.w, min.h + toolbar::HEIGHT));

            let root = NSView::initWithFrame(mtm.alloc(), content);
            let scroll_frame = NSRect::new(NSPoint::ZERO, NSSize::new(visible.w, visible.h));
            let scroll = NSScrollView::initWithFrame(mtm.alloc(), scroll_frame);
            let clip = CenterClip::new(mtm);
            scroll.setContentView(&clip);
            scroll.setHasVerticalScroller(true);
            scroll.setHasHorizontalScroller(true);
            scroll.setAutohidesScrollers(true);
            scroll.setDrawsBackground(true);
            let surround = NSColor::colorWithWhite_alpha(toolbar::SURROUND, 1.0);
            scroll.setBackgroundColor(&surround);
            clip.setBackgroundColor(&surround);
            scroll.setAutoresizingMask(
                NSAutoresizingMaskOptions::ViewWidthSizable
                    | NSAutoresizingMaskOptions::ViewHeightSizable,
            );
            let canvas = CanvasView::new(
                mtm,
                NSRect::new(
                    NSPoint::ZERO,
                    NSSize::new(canvas_rect.w + 2.0 * PAD, canvas_rect.h + 2.0 * PAD),
                ),
            );
            scroll.setDocumentView(Some(&canvas));
            root.addSubview(&scroll);

            toolbar.view.setFrame(NSRect::new(
                NSPoint::new(0.0, visible.h),
                NSSize::new(visible.w, toolbar::HEIGHT),
            ));
            root.addSubview(&toolbar.view);
            let toast = Toast::new(mtm);
            toast.setAutoresizingMask(
                NSAutoresizingMaskOptions::ViewMinXMargin
                    | NSAutoresizingMaskOptions::ViewMaxXMargin
                    | NSAutoresizingMaskOptions::ViewMaxYMargin,
            );
            root.addSubview(&toast);
            panel.setContentView(Some(&root));

            let closing = me.clone();
            let delegate = {
                let this = mtm.alloc::<EditorDelegate>().set_ivars(DelegateIvars {
                    on_close: Box::new(move || {
                        if let Some(shared) = closing.upgrade() {
                            shared.finish();
                        }
                    }),
                });
                // SAFETY: NSObject's designated initializer.
                let delegate: Retained<EditorDelegate> = unsafe { msg_send![super(this), init] };
                delegate
            };
            panel.setDelegate(Some(ProtocolObject::from_ref(&*delegate)));
            let keys = me.clone();
            *panel.ivars().on_key.borrow_mut() = Some(Rc::new(move |event: &NSEvent| {
                keys.upgrade()
                    .is_some_and(|shared| shared.handle_key(event))
            }));

            let frame = panel.frame();
            panel.setFrameOrigin(NSPoint::new(
                area.origin.x + (area.size.width - frame.size.width) / 2.0,
                area.origin.y + (area.size.height - frame.size.height) / 2.0,
            ));

            Shared {
                mtm,
                me: me.clone(),
                panel,
                delegate,
                toolbar,
                scroll,
                clip,
                canvas,
                toast,
                session: RefCell::new(session),
                source: RefCell::new(Some(Rc::new(source))),
                overlay: RefCell::new(None),
                last_canvas: Cell::new(canvas_rect),
                area,
                titlebar,
                strings,
                save_dir: request.save_dir,
                pasteboard,
                on_finish: request.on_finish,
                outcome: RefCell::new(Outcome::Closed),
                finished: Cell::new(false),
                toast_count: Cell::new(0),
            }
        });
        shared.canvas.set_host(Rc::downgrade(&shared));
        shared.after_change();
        shared
    }

    /// The largest the scrolling area may be on a screen of this size: the window keeps a margin to the edges.
    fn max_visible(area: NSRect, titlebar: f64) -> Size {
        Size::new(
            (area.size.width - 40.0).max(0.0),
            (area.size.height - 40.0 - titlebar - toolbar::HEIGHT).max(MIN_VISIBLE_HEIGHT),
        )
    }

    pub fn show(&self) {
        self.panel.orderFrontRegardless();
        self.panel.makeKeyWindow();
        self.panel.makeFirstResponder(Some(&self.canvas));
    }

    // Geometry

    pub fn to_document(&self, view: Point) -> Point {
        let canvas = self.session.borrow().doc().canvas();
        Point::new(view.x - PAD + canvas.x, view.y - PAD + canvas.y)
    }

    pub fn to_view(&self, doc: Point) -> Point {
        let canvas = self.session.borrow().doc().canvas();
        Point::new(doc.x - canvas.x + PAD, doc.y - canvas.y + PAD)
    }

    pub fn cursor(&self) -> Retained<objc2_app_kit::NSCursor> {
        let tool = self.session.borrow().tool();
        cursor_for(
            !matches!(tool, Tool::Select | Tool::Text),
            tool == Tool::Text,
        )
    }

    // Painting

    pub fn paint(&self, bounds: NSRect) {
        NSColor::colorWithWhite_alpha(toolbar::SURROUND, 1.0).setFill();
        objc2_app_kit::NSRectFill(bounds);
        let Some(source) = self.source.borrow().clone() else {
            return;
        };
        let session = self.session.borrow();
        let doc = session.doc();
        let canvas = doc.canvas();
        let Some(context) = NSGraphicsContext::currentContext() else {
            return;
        };
        NSGraphicsContext::saveGraphicsState_class();
        CGContext::translate_ctm(Some(&context.CGContext()), PAD - canvas.x, PAD - canvas.y);
        checkerboard().setFill();
        objc2_app_kit::NSRectFill(ns_rect(canvas));
        paint_document(doc, &source, false, session.editing());
        white(0.14).setStroke();
        let edge = NSBezierPath::bezierPathWithRect(ns_rect(canvas.inflate(0.5)));
        edge.setLineWidth(1.0);
        edge.stroke();
        let accent = Tone::Blue.color();
        if let Some(id) = session.editing() {
            if let Some(object) = doc.object(id) {
                let empty = self
                    .overlay
                    .borrow()
                    .as_ref()
                    .is_some_and(|o| o.view.string().length() == 0);
                paint_editing(
                    object,
                    &accent,
                    empty.then_some(self.strings.text_placeholder.as_str()),
                );
            }
        } else if let Some(object) = session.selected_object() {
            paint_selection(object, &accent, !session.gesture_active());
        }
        if let Some(region) = session.crop_preview() {
            paint_crop(canvas, region);
        }
        NSGraphicsContext::restoreGraphicsState_class();
    }

    // Changes

    fn bar_state(&self) -> BarState {
        let s = self.session.borrow();
        let style = s.selected_object().map_or(s.style(), |o| o.style);
        BarState {
            tool: s.tool(),
            colour: style.colour,
            width: style.width,
            fill: style.fill,
            can_undo: s.can_undo(),
            can_redo: s.can_redo(),
            can_delete: s.selection().is_some(),
        }
    }

    /// Brings the toolbar, the window and the canvas in line with the model after anything changed it.
    pub fn after_change(&self) {
        self.toolbar.sync(self.bar_state());
        let canvas = self.session.borrow().doc().canvas();
        let old = self.last_canvas.replace(canvas);
        if old != canvas {
            self.relayout(old, canvas);
        }
        self.canvas.setNeedsDisplay(true);
        self.panel.invalidateCursorRectsForView(&self.canvas);
    }

    fn relayout(&self, old: Rect, new: Rect) {
        let visible = self.scroll.contentSize();
        let scroll = self.clip.bounds().origin;
        let max = Self::max_visible(self.area, self.titlebar);
        let min = Size::new(self.toolbar.min_width(), MIN_VISIBLE_HEIGHT);
        let follow = follow_resize(
            old,
            new,
            PAD,
            Size::new(visible.width, visible.height),
            Point::new(scroll.x, scroll.y),
            min,
            max,
        );
        self.canvas
            .setFrameSize(NSSize::new(new.w + 2.0 * PAD, new.h + 2.0 * PAD));
        let mut frame = self.panel.frame();
        let (dw, dh) = (
            follow.visible.w - visible.width,
            follow.visible.h - visible.height,
        );
        frame.origin.x -= follow.left;
        frame.size.width += dw;
        frame.size.height += dh;
        frame.origin.y += follow.top - dh;
        // Stay on the display the window opened on.
        let area = self.area;
        frame.origin.x = frame.origin.x.clamp(
            area.origin.x,
            (area.origin.x + area.size.width - frame.size.width).max(area.origin.x),
        );
        frame.origin.y = frame.origin.y.clamp(
            area.origin.y,
            (area.origin.y + area.size.height - frame.size.height).max(area.origin.y),
        );
        self.panel.setFrame_display(frame, true);
        // scrollToPoint does not apply the clip view's rules, so a small canvas is centred by asking it to.
        let wanted = NSRect::new(
            NSPoint::new(follow.scroll.x, follow.scroll.y),
            self.clip.bounds().size,
        );
        self.clip
            .scrollToPoint(self.clip.constrainBoundsRect(wanted).origin);
        self.scroll.reflectScrolledClipView(&self.clip);
    }

    pub fn pointer(&self, phase: Phase, at: Point, shift: bool, clicks: isize) {
        if phase == Phase::Down {
            self.commit_text();
        }
        let mods = Mods { shift };
        let effect = {
            let mut session = self.session.borrow_mut();
            match phase {
                Phase::Down if clicks >= 2 => session.double_click(at),
                Phase::Down => session.pointer_down(at),
                Phase::Drag => {
                    session.pointer_drag(at, mods);
                    Effect::None
                }
                Phase::Up => session.pointer_up(at, mods),
            }
        };
        self.after_change();
        if let Effect::EditText(id) = effect {
            self.begin_text(id);
        }
    }

    /// Runs `change` on the model, then updates everything that shows it.
    pub fn edit<R>(&self, change: impl FnOnce(&mut Session) -> R) -> R {
        let result = change(&mut self.session.borrow_mut());
        self.after_change();
        result
    }

    fn choose_tool(&self, tool: Tool) {
        self.commit_text();
        self.edit(|s| s.set_tool(tool));
    }

    fn choose_colour(&self, colour: Colour) {
        self.commit_text();
        self.edit(|s| s.set_colour(colour));
    }

    fn choose_width(&self, width: f64) {
        self.commit_text();
        self.edit(|s| s.set_width(width));
    }

    fn toggle_fill(&self) {
        self.commit_text();
        let fill = !self.bar_state().fill;
        self.edit(|s| s.set_fill(fill));
    }

    fn undo(&self) {
        self.commit_text();
        self.edit(|s| s.undo());
    }

    fn redo(&self) {
        self.commit_text();
        self.edit(|s| s.redo());
    }

    fn delete(&self) {
        self.commit_text();
        self.edit(|s| s.delete_selected());
    }

    fn duplicate(&self) {
        self.commit_text();
        self.edit(|s| s.duplicate_selected());
    }

    // Text

    fn begin_text(&self, id: u64) {
        let (object, canvas) = {
            let s = self.session.borrow();
            (s.doc().object(id).cloned(), s.doc().canvas())
        };
        let Some(Object {
            kind: Kind::Text { rect, text },
            style,
            ..
        }) = object
        else {
            return;
        };
        let size = style.font_size();
        let frame = NSRect::new(
            NSPoint::new(rect.x - canvas.x + PAD, rect.y - canvas.y + PAD),
            NSSize::new(rect.w.max(MIN_TEXT_WIDTH), rect.h),
        );
        let view = NSTextView::initWithFrame(self.mtm.alloc(), frame);
        view.setDrawsBackground(false);
        view.setRichText(false);
        view.setImportsGraphics(false);
        view.setAllowsUndo(true);
        view.setAutomaticQuoteSubstitutionEnabled(false);
        view.setAutomaticDashSubstitutionEnabled(false);
        view.setAutomaticTextReplacementEnabled(false);
        view.setAutomaticSpellingCorrectionEnabled(false);
        view.setHorizontallyResizable(false);
        view.setVerticallyResizable(true);
        view.setMinSize(NSSize::new(frame.size.width, 1.0));
        view.setMaxSize(NSSize::new(frame.size.width, 1.0e7));
        view.setTextContainerInset(NSSize::ZERO);
        // SAFETY: a plain property read on the view just created.
        if let Some(container) = unsafe { view.textContainer() } {
            container.setLineFragmentPadding(0.0);
            container.setWidthTracksTextView(true);
        }
        view.setFont(Some(&text_font(size)));
        let colour = ns_colour(style.colour, 1.0);
        view.setTextColor(Some(&colour));
        view.setInsertionPointColor(Some(&colour));
        view.setString(&NSString::from_str(&text));
        view.setSelectedRange(NSRange::new(text.encode_utf16().count(), 0));
        let me = self.me.clone();
        let watcher = TextWatcher::new(self.mtm, move || {
            if let Some(shared) = me.upgrade() {
                shared.text_changed();
            }
        });
        view.setDelegate(Some(ProtocolObject::from_ref(&*watcher)));
        self.canvas.addSubview(&view);
        self.panel.makeFirstResponder(Some(&view));
        self.canvas.scrollRectToVisible(frame);
        *self.overlay.borrow_mut() = Some(Overlay { view, watcher });
        self.canvas.setNeedsDisplay(true);
    }

    /// Keeps the editor as tall as its text, as the finished object will be.
    fn text_changed(&self) {
        let overlay = self.overlay.borrow();
        let Some(overlay) = overlay.as_ref() else {
            return;
        };
        let size = {
            let s = self.session.borrow();
            s.editing()
                .and_then(|id| s.doc().object(id))
                .map(|o| o.style.font_size())
        };
        if let Some(size) = size {
            let mut frame = overlay.view.frame();
            frame.size.height =
                render::measure_text(&overlay.view.string().to_string(), size, frame.size.width)
                    .max(size * 1.25);
            overlay.view.setFrame(frame);
        }
        self.canvas.setNeedsDisplay(true);
    }

    /// Ends the text editor, if there is one, and gives the model the text.
    pub fn commit_text(&self) {
        let Some(overlay) = self.overlay.borrow_mut().take() else {
            return;
        };
        // Syllables still being composed are kept as they stand.
        // SAFETY: unmarkText takes no arguments and is part of NSTextInputClient, which NSTextView adopts.
        let _: () = unsafe { msg_send![&*overlay.view, unmarkText] };
        let text = overlay.view.string().to_string();
        overlay.view.setDelegate(None);
        self.panel.makeFirstResponder(Some(&self.canvas));
        overlay.view.removeFromSuperview();
        drop(overlay.watcher);
        self.session.borrow_mut().finish_text(&text);
        self.after_change();
    }

    // Keys

    fn composing(&self) -> bool {
        self.overlay
            .borrow()
            .as_ref()
            // SAFETY: hasMarkedText is part of NSTextInputClient and takes no arguments.
            .is_some_and(|o| unsafe { msg_send![&*o.view, hasMarkedText] })
    }

    fn handle_key(&self, event: &NSEvent) -> bool {
        if self.finished.get() {
            return false;
        }
        let flags = event.modifierFlags();
        let code = event.keyCode();
        if self.overlay.borrow().is_some() {
            // The text editor has the keyboard; only Escape is the editor's own.
            if code == 53 && !self.composing() {
                self.commit_text();
                return true;
            }
            return false;
        }
        let command = flags.contains(NSEventModifierFlags::Command)
            && !flags.intersects(NSEventModifierFlags::Option | NSEventModifierFlags::Control);
        if command {
            let shift = flags.contains(NSEventModifierFlags::Shift);
            let letter = event
                .charactersIgnoringModifiers()
                .map(|s| s.to_string().to_lowercase())
                .unwrap_or_default();
            return match letter.as_str() {
                "z" if shift => {
                    self.redo();
                    true
                }
                "z" => {
                    self.undo();
                    true
                }
                "c" => {
                    self.copy();
                    true
                }
                "s" => {
                    self.save();
                    true
                }
                "d" => {
                    self.duplicate();
                    true
                }
                "w" => {
                    self.close();
                    true
                }
                _ => false,
            };
        }
        let step = if flags.contains(NSEventModifierFlags::Shift) {
            NUDGE_FAR
        } else {
            NUDGE
        };
        let plain_or_shift = !flags.intersects(
            NSEventModifierFlags::Command
                | NSEventModifierFlags::Option
                | NSEventModifierFlags::Control,
        );
        match code {
            53 if is_plain(flags) => {
                let dragging = self.session.borrow().gesture_active();
                if dragging {
                    self.edit(|s| s.cancel_gesture());
                } else {
                    self.close();
                }
                true
            }
            51 | 117 if is_plain(flags) => {
                let had = self.session.borrow().selection().is_some();
                self.delete();
                had
            }
            123..=126 if plain_or_shift => {
                let (dx, dy) = match code {
                    123 => (-step, 0.0),
                    124 => (step, 0.0),
                    125 => (0.0, step),
                    _ => (0.0, -step),
                };
                self.edit(|s| s.nudge(dx, dy))
            }
            _ => false,
        }
    }

    // Output

    fn toast(&self, text: &str, error: bool) {
        let width = self
            .panel
            .contentView()
            .map_or(0.0, |v| v.frame().size.width);
        self.toast.show(text, error, width);
        let count = self.toast_count.get().wrapping_add(1);
        self.toast_count.set(count);
        let me = self.me.clone();
        crate::app::after(TOAST_SECONDS, move || {
            if let Some(shared) = me.upgrade()
                && shared.toast_count.get() == count
            {
                shared.toast.hide();
            }
        });
    }

    fn render(&self) -> Option<Vec<u8>> {
        let source = self.source.borrow().clone()?;
        export_png(self.session.borrow().doc(), &source)
    }

    pub fn export(&self) -> Option<Vec<u8>> {
        self.render()
    }

    /// Copies the finished picture to the clipboard. A failure is shown and the window stays.
    pub fn copy(&self) -> bool {
        self.commit_text();
        match self.render() {
            Some(png) => {
                self.pasteboard.write(Write::Png(&png));
                *self.outcome.borrow_mut() = Outcome::Copied;
                self.toast(&self.strings.copied, false);
                true
            }
            None => {
                self.toast(&self.strings.copy_failed, true);
                false
            }
        }
    }

    pub fn save(&self) {
        self.commit_text();
        let saved = self
            .render()
            .ok_or_else(|| io::Error::other("the picture could not be made"))
            .and_then(|png| write_unique(&self.save_dir, local_stamp(), &png));
        match saved {
            Ok(path) => {
                *self.outcome.borrow_mut() = Outcome::Saved(path);
                self.toast(&self.strings.saved, false);
            }
            Err(_) => self.toast(&self.strings.save_failed, true),
        }
    }

    fn done(&self) {
        if self.copy() {
            self.close();
        }
    }

    pub fn close(&self) {
        self.panel.close();
    }

    /// The middle of a toolbar control, in window coordinates.
    pub fn control_centre(&self, which: Button) -> Option<NSPoint> {
        self.toolbar.centre_in_window(which)
    }

    pub fn is_finished(&self) -> bool {
        self.finished.get()
    }

    /// Runs once, when the window closes: lets go of the picture and the model, then reports.
    fn finish(&self) {
        if self.finished.replace(true) {
            return;
        }
        if let Some(overlay) = self.overlay.borrow_mut().take() {
            overlay.view.setDelegate(None);
            overlay.view.removeFromSuperview();
        }
        self.source.borrow_mut().take();
        *self.session.borrow_mut() = Session::new(1, 1, 1.0);
        let outcome = self.outcome.borrow().clone();
        (self.on_finish)(outcome);
    }

    /// The content of the window as a PNG, for checks.
    pub fn snapshot_png(&self) -> Option<Vec<u8>> {
        let view = self.panel.contentView()?;
        let bounds = view.bounds();
        let rep = view.bitmapImageRepForCachingDisplayInRect(bounds)?;
        view.cacheDisplayInRect_toBitmapImageRep(bounds, &rep);
        // SAFETY: an empty properties dictionary is valid for PNG output.
        let png = unsafe {
            rep.representationUsingType_properties(NSBitmapImageFileType::PNG, &NSDictionary::new())
        }?;
        Some(png.to_vec())
    }

    pub fn text_overlay_frame(&self) -> Option<NSRect> {
        self.overlay.borrow().as_ref().map(|o| o.view.frame())
    }

    pub fn type_text(&self, text: &str) {
        if let Some(overlay) = self.overlay.borrow().as_ref() {
            // SAFETY: the argument is a string, which insertText accepts, and the range means the selection.
            unsafe {
                overlay.view.insertText_replacementRange(
                    &NSString::from_str(text),
                    NSRange::new(objc2_foundation::NSNotFound as usize, 0),
                );
            }
        }
    }
}

impl Drop for Shared {
    fn drop(&mut self) {
        self.panel.setDelegate(None);
        // The owner may drop this inside the close callback, while AppKit is still using the window: its
        // last reference is released when the current pool drains instead.
        let _ = Retained::autorelease_ptr(self.panel.clone());
        let _ = &self.delegate;
    }
}

thread_local! {
    static CHECKER: Retained<NSColor> = make_checker();
}

fn make_checker() -> Retained<NSColor> {
    let tile = 10.0;
    let draw = block2::RcBlock::new(move |rect: NSRect| {
        NSColor::colorWithWhite_alpha(0.20, 1.0).setFill();
        objc2_app_kit::NSRectFill(rect);
        NSColor::colorWithWhite_alpha(0.26, 1.0).setFill();
        for (x, y) in [(0.0, 0.0), (tile, tile)] {
            objc2_app_kit::NSRectFill(NSRect::new(NSPoint::new(x, y), NSSize::new(tile, tile)));
        }
        objc2::runtime::Bool::YES
    });
    let image = NSImage::imageWithSize_flipped_drawingHandler(
        NSSize::new(2.0 * tile, 2.0 * tile),
        true,
        &draw,
    );
    NSColor::colorWithPatternImage(&image)
}

/// The backdrop of a canvas: where nothing is drawn, a pattern shows that it is empty.
fn checkerboard() -> Retained<NSColor> {
    CHECKER.with(Clone::clone)
}

fn square(centre: Point, side: f64) -> NSRect {
    NSRect::new(
        NSPoint::new(centre.x - side / 2.0, centre.y - side / 2.0),
        NSSize::new(side, side),
    )
}

fn paint_selection(object: &Object, accent: &NSColor, with_handles: bool) {
    accent.setStroke();
    let line_like = matches!(object.kind, Kind::Arrow { .. } | Kind::Line { .. });
    if !line_like {
        let frame = match object.kind {
            Kind::Pen(_) | Kind::Highlight(_) => object.bounds(),
            _ => object.frame(),
        };
        let outline = NSBezierPath::bezierPathWithRect(ns_rect(frame));
        outline.setLineWidth(1.0);
        outline.stroke();
    }
    if !with_handles {
        return;
    }
    for (_, at) in object.handles() {
        let rect = square(at, 8.0);
        let path = if line_like {
            NSBezierPath::bezierPathWithOvalInRect(square(at, 10.0))
        } else {
            NSBezierPath::bezierPathWithRect(rect)
        };
        white(1.0).setFill();
        path.fill();
        accent.setStroke();
        path.setLineWidth(1.5);
        path.stroke();
    }
}

/// A dashed frame around the text being typed, and a hint while it is empty.
fn paint_editing(object: &Object, accent: &NSColor, placeholder: Option<&str>) {
    let Kind::Text { rect, .. } = object.kind else {
        return;
    };
    let frame = NSBezierPath::bezierPathWithRect(ns_rect(rect.inflate(2.0)));
    frame.setLineWidth(1.0);
    let dashes = [4.0f64, 3.0];
    // SAFETY: the pointer is to two doubles, as the count says, and is only read during the call.
    unsafe { frame.setLineDash_count_phase(dashes.as_ptr(), 2, 0.0) };
    accent.setStroke();
    frame.stroke();
    if let Some(hint) = placeholder {
        draw_text_block(
            hint,
            object.style.font_size(),
            &ns_colour(object.style.colour, 0.45),
            rect,
        );
    }
}

fn paint_crop(canvas: Rect, region: Rect) {
    let dim = NSBezierPath::bezierPathWithRect(ns_rect(canvas));
    dim.appendBezierPathWithRect(ns_rect(region));
    dim.setWindingRule(objc2_app_kit::NSWindingRule::EvenOdd);
    NSColor::colorWithWhite_alpha(0.0, 0.55).setFill();
    dim.fill();
    let outline = NSBezierPath::bezierPathWithRect(ns_rect(region));
    outline.setLineWidth(1.0);
    white(0.95).setStroke();
    outline.stroke();
}

#[cfg(test)]
mod tests {
    use super::*;

    const STAMP: Stamp = Stamp {
        year: 2026,
        month: 10,
        day: 15,
        hour: 9,
        minute: 30,
        second: 0,
    };

    fn scratch(name: &str) -> PathBuf {
        let dir =
            std::env::temp_dir().join(format!("figghost-annotate-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        dir
    }

    #[test]
    fn saving_twice_in_one_second_makes_two_files_and_replaces_nothing() {
        let dir = scratch("unique");
        let first = write_unique(&dir, STAMP, b"one").unwrap();
        let second = write_unique(&dir, STAMP, b"two").unwrap();
        assert_ne!(first, second);
        assert_eq!(std::fs::read(&first).unwrap(), b"one");
        assert_eq!(std::fs::read(&second).unwrap(), b"two");
        assert!(
            second
                .file_name()
                .unwrap()
                .to_string_lossy()
                .ends_with(" 2.png")
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_file_that_is_in_the_way_is_never_overwritten() {
        let dir = scratch("inway");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join(file_name(STAMP, 0)), b"keep").unwrap();
        std::fs::write(dir.join(file_name(STAMP, 1)), b"keep too").unwrap();
        let path = write_unique(&dir, STAMP, b"new").unwrap();
        assert!(
            path.file_name()
                .unwrap()
                .to_string_lossy()
                .ends_with(" 3.png")
        );
        assert_eq!(
            std::fs::read(dir.join(file_name(STAMP, 0))).unwrap(),
            b"keep"
        );
        assert_eq!(
            std::fs::read(dir.join(file_name(STAMP, 1))).unwrap(),
            b"keep too"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_missing_folder_is_made_and_one_that_cannot_be_is_an_error() {
        let dir = scratch("made").join("a").join("b");
        assert!(write_unique(&dir, STAMP, b"x").unwrap().exists());
        let _ = std::fs::remove_dir_all(scratch("made"));
        let file = scratch("blocked");
        std::fs::write(&file, b"i am a file").unwrap();
        assert!(write_unique(&file.join("sub"), STAMP, b"x").is_err());
        assert_eq!(std::fs::read(&file).unwrap(), b"i am a file");
        let _ = std::fs::remove_file(&file);
    }
}
