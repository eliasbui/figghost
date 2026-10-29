//! Opens the real settings window on screen with sample data, so its Liquid Glass can be seen (glass is never drawn
//! offscreen), over a picture in a window of its own so the glass has something to show, captures the two together
//! and checks that the window laid glass under the page. Exits with status 1 when it did not.
//!
//!   settings_window <page> <out.png> [--dark] [--size 1000x700] [--wait seconds] [--behind picture]

use macos_sys::{
    MainThreadMarker, app,
    webview::{WebWindow, WindowSpec},
};
use objc2_app_kit::{
    NSApplication, NSBackingStoreType, NSBitmapImageFileType, NSBitmapImageRep, NSGlassEffectView,
    NSImage, NSImageScaling, NSImageView, NSScreen, NSWindow, NSWindowOrderingMode,
    NSWindowStyleMask,
};
#[allow(deprecated)]
use objc2_core_graphics::{CGWindowImageOption, CGWindowListCreateImage, CGWindowListOption};
use objc2_foundation::{NSDictionary, NSRect, NSString};
use serde_json::{Value, json};
use std::{cell::RefCell, env, process, rc::Rc};

#[path = "../src/settings/page.rs"]
mod page;

fn option(args: &[String], name: &str) -> Option<String> {
    args.iter()
        .position(|a| a == name)
        .and_then(|i| args.get(i + 1))
        .cloned()
}

fn main() {
    let mtm = MainThreadMarker::new().expect("main thread");
    let args: Vec<String> = env::args().skip(1).collect();
    let page_id = args.first().cloned().unwrap_or_else(|| "general".into());
    let out = args
        .get(1)
        .cloned()
        .unwrap_or_else(|| "/tmp/settings-window.png".into());
    let dark = args.iter().any(|a| a == "--dark");
    let (width, height) = option(&args, "--size")
        .and_then(|s| {
            s.split_once('x')
                .map(|(w, h)| (w.parse().ok(), h.parse().ok()))
        })
        .and_then(|(w, h)| Some((w?, h?)))
        .unwrap_or((1000.0, 700.0));
    let wait: f64 = option(&args, "--wait")
        .and_then(|w| w.parse().ok())
        .unwrap_or(2.0);
    let behind = option(&args, "--behind").unwrap_or_else(|| {
        concat!(env!("CARGO_MANIFEST_DIR"), "/../../docs/images/hero.jpg").into()
    });

    app::run_accessory(mtm, move || {
        let slot: Rc<RefCell<Option<Rc<WebWindow>>>> = Rc::default();
        let on_message = {
            let slot = Rc::clone(&slot);
            move |text: String| {
                let Some(window) = slot.borrow().clone() else {
                    return;
                };
                let message: Value = serde_json::from_str(&text).unwrap_or(Value::Null);
                match message["t"].as_str() {
                    Some("ready") => {
                        let state = json!({ "t": "state", "state": page::fixture() });
                        window.host.post(&state.to_string());
                        window
                            .host
                            .eval(&format!("setTimeout(() => go('{page_id}'), 50)"));
                    }
                    Some("icons") => window
                        .host
                        .post(&json!({ "t": "icons", "icons": {} }).to_string()),
                    Some("js_error") => println!("PAGE ERROR: {text}"),
                    _ => {}
                }
            }
        };
        let spec = WindowSpec {
            title: "Figghost",
            width,
            height,
            min_width: 720.0,
            min_height: 480.0,
        };
        let window = WebWindow::new(mtm, &spec, &page::html(), on_message, || process::exit(0));
        window.set_dark(Some(dark));
        window.show(mtm);
        *slot.borrow_mut() = Some(Rc::clone(&window));
        // The picture behind, a little larger than the window, in a borderless window of this process: a capture of
        // other apps' windows would need Screen Recording.
        let number = window.window_number();
        let ns_window = NSApplication::sharedApplication(mtm)
            .windows()
            .iter()
            .find(|w| w.windowNumber() == number)
            .expect("the settings window");
        let frame = ns_window.frame();
        let around = NSRect::new(
            objc2_foundation::NSPoint::new(frame.origin.x - 60.0, frame.origin.y - 60.0),
            objc2_foundation::NSSize::new(frame.size.width + 120.0, frame.size.height + 120.0),
        );
        // SAFETY: designated initializers and plain setters on the main thread.
        let backdrop = unsafe {
            let backdrop = NSWindow::initWithContentRect_styleMask_backing_defer(
                mtm.alloc(),
                around,
                NSWindowStyleMask::Borderless,
                NSBackingStoreType::Buffered,
                false,
            );
            backdrop.setReleasedWhenClosed(false);
            let picture = NSImageView::initWithFrame(mtm.alloc(), around);
            picture.setImageScaling(NSImageScaling::ScaleAxesIndependently);
            picture.setImage(
                NSImage::initWithContentsOfFile(mtm.alloc(), &NSString::from_str(&behind))
                    .as_deref(),
            );
            backdrop.setContentView(Some(&picture));
            backdrop.orderWindow_relativeTo(NSWindowOrderingMode::Below, number);
            backdrop
        };

        let shot = Rc::clone(&window);
        app::after(wait, move || {
            let number = shot.window_number();
            let window = NSApplication::sharedApplication(mtm)
                .windows()
                .iter()
                .find(|w| w.windowNumber() == number);
            // The window and what is under it, as the screen shows them, in points from the top left of the main
            // screen.
            let screen = NSScreen::screens(mtm)
                .firstObject()
                .map_or(0.0, |s| s.frame().size.height);
            let mut frame = window.as_ref().map(|w| w.frame()).unwrap_or_default();
            frame.origin.y = screen - frame.origin.y - frame.size.height;
            #[allow(deprecated)]
            let image = CGWindowListCreateImage(
                frame,
                CGWindowListOption::OptionOnScreenBelowWindow
                    | CGWindowListOption::OptionIncludingWindow,
                number as u32,
                CGWindowImageOption::BestResolution,
            );
            let png = image.and_then(|image| {
                let rep = NSBitmapImageRep::initWithCGImage(mtm.alloc(), &image);
                // SAFETY: an empty properties dictionary is valid for PNG output.
                unsafe {
                    rep.representationUsingType_properties(
                        NSBitmapImageFileType::PNG,
                        &NSDictionary::new(),
                    )
                }
            });
            match png {
                Some(png) => {
                    std::fs::write(&out, png.to_vec()).expect("write png");
                    println!("captured window {number} -> {out}");
                }
                None => println!("the capture failed"),
            }
            let _ = &backdrop;
            let panes = window.and_then(|w| w.contentView()).map_or(0, |root| {
                root.subviews()
                    .iter()
                    .filter(|v| v.downcast_ref::<NSGlassEffectView>().is_some())
                    .count()
            });
            println!("glass panes under the page: {panes}");
            process::exit(i32::from(panes == 0));
        });
        window
    });
}
