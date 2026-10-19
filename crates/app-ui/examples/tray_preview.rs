//! Renders the menu bar panel in WebKit without showing a window, over a backdrop, to a PNG.
//!
//!   tray_preview <out.png> [--dark] [--height 640]
//!
//! The entries are the ones the real menu has on a machine with every feature on.

use macos_sys::{MainThreadMarker, app, webview::HiddenStage};
use serde_json::{Value, json};
use std::{cell::RefCell, env, fs, process, rc::Rc};

const PAGE: &str = include_str!("../web/tray.html");

fn items() -> Value {
    let entries = [
        ("Settings", "gearshape", None, true),
        ("Allow Accessibility", "hand.raised", None, false),
        ("Smooth scrolling", "computermouse", Some(true), true),
        ("Reverse direction", "arrow.up.arrow.down", Some(false), true),
        ("Smooth in Safari", "macwindow", Some(true), true),
        ("Volume indicator", "speaker.wave.2", Some(true), true),
        ("Open clipboard history", "doc.on.clipboard", None, true),
        ("Clear history", "trash", None, true),
        ("Launch at login", "power", Some(true), true),
        ("Open config folder", "folder", None, true),
        ("Copy text from screen", "text.magnifyingglass", None, true),
        ("Capture area", "rectangle.dashed", None, true),
        ("Capture window", "macwindow", None, true),
        ("Capture screen", "display", None, true),
        ("Capture in 5 seconds", "timer", None, true),
        ("Capture text", "text.viewfinder", None, true),
        ("Scrolling capture", "arrow.up.and.down.text.horizontal", None, true),
        ("Record screen", "record.circle", None, true),
        ("Record GIF", "photo.stack", None, true),
        ("Colour picker", "eyedropper", None, true),
        ("Copy Stack", "square.stack.3d.up", None, true),
        ("Quit Figghost", "xmark.rectangle", None, true),
    ];
    Value::Array(
        entries
            .iter()
            .enumerate()
            .map(|(id, (title, symbol, checked, visible))| {
                json!({ "id": id, "title": title, "symbol": symbol, "checked": checked, "visible": visible })
            })
            .collect(),
    )
}

fn main() {
    let mtm = MainThreadMarker::new().expect("main thread");
    let args: Vec<String> = env::args().skip(1).collect();
    let out = args.first().cloned().unwrap_or_else(|| "/tmp/tray.png".into());
    let dark = args.iter().any(|a| a == "--dark");
    let height: f64 = args
        .iter()
        .position(|a| a == "--height")
        .and_then(|i| args.get(i + 1))
        .and_then(|h| h.parse().ok())
        .unwrap_or(700.0);
    app::run_accessory(mtm, move || {
        let slot: Rc<RefCell<Option<Rc<HiddenStage>>>> = Rc::default();
        let on_message = {
            let slot = Rc::clone(&slot);
            move |text: String| {
                let Some(stage) = slot.borrow().clone() else {
                    return;
                };
                let message: Value = serde_json::from_str(&text).unwrap_or(Value::Null);
                match message["t"].as_str() {
                    Some("ready") => stage
                        .host
                        .post(&json!({ "t": "state", "items": items() }).to_string()),
                    Some("js_error") => println!("PAGE ERROR: {text}"),
                    _ => {}
                }
            }
        };
        let stage = Rc::new(HiddenStage::new(mtm, (348.0, height), dark, PAGE, on_message));
        *slot.borrow_mut() = Some(Rc::clone(&stage));
        let backdrop = if dark {
            "linear-gradient(135deg,#1d2233,#2e2a4a)"
        } else {
            "linear-gradient(135deg,#cfd6e6,#e8d9f2)"
        };
        let shot = Rc::clone(&stage);
        app::after(1.0, move || {
            shot.host.eval(&format!("document.documentElement.style.background = '{backdrop}'"));
        });
        let shot = Rc::clone(&stage);
        app::after(2.0, move || {
            shot.host.snapshot_png(mtm, 348.0, move |png| {
                let Some(png) = png else {
                    println!("the snapshot failed");
                    process::exit(1);
                };
                fs::write(&out, &png).expect("write png");
                println!("wrote {out}");
                process::exit(0);
            });
        });
        stage
    });
}
