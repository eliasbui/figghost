//! Drives a real drag and drop between two shelf pages of this process with synthetic mouse events,
//! so both halves of the shelf (drag out of one, drop into the other) go through the window server.
//! The pointer moves for about two seconds and is put back afterwards; no other app is touched.
//! `cargo run -p macos-sys --example dnd_check`

use macos_sys::{
    MainThreadMarker, app,
    panel::FigPanel,
    shelf_ui::{ShelfCallbacks, ShelfView},
};
use objc2::rc::Retained;
use objc2_app_kit::{NSBox, NSBoxType, NSColor, NSScreen};
use objc2_foundation::{NSPoint, NSRect, NSSize};
use std::{cell::RefCell, env, ffi::c_void, fs, process, rc::Rc};

#[repr(C)]
#[derive(Clone, Copy)]
struct CGPoint {
    x: f64,
    y: f64,
}

#[link(name = "CoreGraphics", kind = "framework")]
unsafe extern "C" {
    fn CGEventCreate(source: *mut c_void) -> *mut c_void;
    fn CGEventGetLocation(event: *mut c_void) -> CGPoint;
    fn CGEventCreateMouseEvent(
        source: *mut c_void,
        kind: u32,
        at: CGPoint,
        button: u32,
    ) -> *mut c_void;
    fn CGEventSetIntegerValueField(event: *mut c_void, field: u32, value: i64);
    fn CGEventPost(tap: u32, event: *mut c_void);
}

#[link(name = "CoreFoundation", kind = "framework")]
unsafe extern "C" {
    fn CFRelease(cf: *const c_void);
}

const MOUSE_MOVED: u32 = 5;
const LEFT_DOWN: u32 = 1;
const LEFT_UP: u32 = 2;
const LEFT_DRAGGED: u32 = 6;
const CLICK_STATE: u32 = 1;
const STEPS: u32 = 16;

fn pointer() -> CGPoint {
    // SAFETY: a throwaway event created and released here, used only to read the pointer position.
    unsafe {
        let event = CGEventCreate(std::ptr::null_mut());
        let at = CGEventGetLocation(event);
        CFRelease(event);
        at
    }
}

fn post(kind: u32, at: CGPoint) {
    // SAFETY: the event is created, configured, posted and released within this call.
    unsafe {
        let event = CGEventCreateMouseEvent(std::ptr::null_mut(), kind, at, 0);
        if kind == LEFT_DOWN {
            CGEventSetIntegerValueField(event, CLICK_STATE, 1);
        }
        CGEventPost(0, event);
        CFRelease(event);
    }
}

/// Like the notch, the panel has an opaque black body: the window server treats fully transparent pixels as
/// not part of a window, so a transparent panel would never be offered a drop.
fn panel(mtm: MainThreadMarker, origin: NSPoint, view: &ShelfView) -> Retained<FigPanel> {
    let size = NSSize::new(420.0, 260.0);
    let panel = FigPanel::new(mtm, NSRect::new(origin, size));
    let body = NSBox::new(mtm);
    body.setBoxType(NSBoxType::Custom);
    body.setBorderWidth(0.0);
    body.setFillColor(&NSColor::blackColor());
    body.setContentViewMargins(NSSize::ZERO);
    body.setFrame(NSRect::new(NSPoint::ZERO, size));
    body.setContentView(Some(&view.root));
    panel.setContentView(Some(&body));
    panel.orderFrontRegardless();
    panel
}

fn main() {
    let mtm = MainThreadMarker::new().expect("main thread");
    app::run_accessory(mtm, move || {
        let file = env::temp_dir().join(format!("figghost-dnd-{}.txt", process::id()));
        fs::write(&file, "drag me").expect("temp file");
        let file_path = file.to_string_lossy().into_owned();

        let dropped: Rc<RefCell<Vec<String>>> = Rc::default();
        let quiet = || ShelfCallbacks {
            on_drop: Box::new(|_| {}),
            on_remove: Box::new(|_| {}),
            on_open: Box::new(|_| {}),
            actions_for: Box::new(|_| Vec::new()),
            on_action: Box::new(|_, _| {}),
        };
        let frame = NSRect::new(NSPoint::ZERO, NSSize::new(420.0, 260.0));
        let source = ShelfView::new(mtm, frame, quiet());
        source.set_paths(vec![file_path.clone()]);
        let sink = {
            let dropped = Rc::clone(&dropped);
            ShelfView::new(
                mtm,
                frame,
                ShelfCallbacks {
                    on_drop: Box::new(move |paths| dropped.borrow_mut().extend(paths)),
                    ..quiet()
                },
            )
        };
        let panels = (
            panel(mtm, NSPoint::new(120.0, 420.0), &source),
            panel(mtm, NSPoint::new(620.0, 420.0), &sink),
        );

        // The pointer path is worked out once the panels are on screen, then replayed on a timer so the
        // application event loop keeps running while the drag is in flight.
        let path: Rc<RefCell<Option<(CGPoint, CGPoint, CGPoint)>>> = Rc::default();
        {
            let (path, source, sink) = (Rc::clone(&path), Rc::clone(&source), Rc::clone(&sink));
            app::after(0.8, move || {
                let primary = NSScreen::screens(mtm)
                    .firstObject()
                    .expect("a screen")
                    .frame()
                    .size
                    .height;
                let to_cg = |p: NSPoint| CGPoint {
                    x: p.x,
                    y: primary - p.y,
                };
                let from = to_cg(source.tile_center_on_screen(0).expect("tile on screen"));
                let to = to_cg(sink.center_on_screen().expect("sink on screen"));
                *path.borrow_mut() = Some((pointer(), from, to));
                post(MOUSE_MOVED, from);
            });
        }
        let step = |delay: f64, act: Box<dyn Fn((CGPoint, CGPoint, CGPoint))>| {
            let path = Rc::clone(&path);
            app::after(delay, move || {
                if let Some(p) = *path.borrow() {
                    act(p);
                }
            });
        };
        step(1.0, Box::new(|(_, from, _)| post(LEFT_DOWN, from)));
        for i in 1..=STEPS {
            let t = f64::from(i) / f64::from(STEPS);
            step(
                1.1 + f64::from(i) * 0.05,
                Box::new(move |(_, from, to)| {
                    post(
                        LEFT_DRAGGED,
                        CGPoint {
                            x: from.x + (to.x - from.x) * t,
                            y: from.y + (to.y - from.y) * t,
                        },
                    );
                }),
            );
        }
        step(2.2, Box::new(|(_, _, to)| post(LEFT_UP, to)));
        step(
            3.0,
            Box::new(|(original, _, _)| post(MOUSE_MOVED, original)),
        );
        {
            let dropped = Rc::clone(&dropped);
            app::after(3.3, move || {
                let got = dropped.borrow().clone();
                let _ = fs::remove_file(&file);
                println!("sent     {file_path}");
                println!("received {got:?}");
                let ok = got == [file_path.clone()];
                println!(
                    "{}",
                    if ok {
                        "drag and drop check: pass"
                    } else {
                        "drag and drop check: FAIL"
                    }
                );
                process::exit(i32::from(!ok));
            });
        }
        (panels, source, sink)
    });
}
