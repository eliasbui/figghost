//! Renders the menu bar panel in WebKit without showing a window, over a backdrop, to a PNG.
//!
//!   tray_preview <out.png> [--dark] [--height 420] [--access] [--recording] [--hover N]
//!
//! The entries are the ones the real menu has on a machine with every feature on, as the panel is given them:
//! the ones kept for the menu alone are not among them. `--access` shows the entry that asks for Accessibility,
//! `--recording` the one that stops a recording, `--hover N` names the Nth tool as the pointer would.

use macos_sys::{
    MainThreadMarker, app,
    webview::HiddenStage,
    widgets::{base64, image_png, symbol_data_url},
};
use serde_json::{Value, json};
use std::{cell::RefCell, env, fs, process, rc::Rc};

const PAGE: &str = include_str!("../web/tray.html");
const APP_ICON: &[u8] = include_bytes!("../assets/Figghost.icns");
/// The width `TRAY_WIDTH` gives the panel in macos-sys/src/webview.rs.
const WIDTH: f64 = 276.0;

fn items(access: bool, recording: bool) -> Value {
    let entries = [
        ("Settings...", "gearshape", None, true),
        (
            "Accessibility access is off: click to grant",
            "hand.raised",
            None,
            access,
        ),
        ("Smooth scrolling", "computermouse", Some(true), true),
        (
            "Reverse mouse wheel direction",
            "arrow.up.arrow.down",
            Some(false),
            true,
        ),
        ("Smooth scrolling for Safari", "macwindow", Some(true), true),
        (
            "Volume indicator in the notch",
            "speaker.wave.2",
            Some(true),
            true,
        ),
        ("Keep window on top", "pin", Some(false), true),
        ("Open clipboard history", "doc.on.clipboard", None, true),
        ("Clear history (keep pinned)", "trash", None, true),
        ("Start at login", "power", Some(true), true),
        ("Open settings folder", "folder", None, true),
        ("Smart Select", "text.magnifyingglass", None, true),
        ("Stop Recording", "stop.circle", None, recording),
        ("Capture Area", "rectangle.dashed", None, true),
        ("Capture Window", "macwindow", None, true),
        ("Capture Screen", "display", None, true),
        ("Capture Screen in 5 Seconds", "timer", None, true),
        ("Copy Text from Screen", "text.viewfinder", None, true),
        (
            "Scrolling Capture",
            "arrow.up.and.down.text.horizontal",
            None,
            true,
        ),
        ("Record Screen", "record.circle", None, !recording),
        ("Record GIF", "photo.stack", None, !recording),
        ("Stop Recording", "stop.circle", None, recording),
        ("Open Captures Folder", "folder", None, true),
        ("Pick a colour", "eyedropper", None, true),
        (
            "End Copy Stack (3 queued)",
            "square.stack.3d.up",
            None,
            false,
        ),
        ("Open Mole", "sparkles", None, true),
        (
            "Check for Updates...",
            "arrow.triangle.2.circlepath",
            None,
            true,
        ),
        ("Quit", "xmark.rectangle", None, true),
    ];
    Value::Array(
        entries
            .iter()
            .enumerate()
            .map(|(id, (title, symbol, checked, visible))| {
                // The icon is what status_item.rs sends with each entry.
                json!({ "id": id, "title": title, "symbol": symbol, "icon": symbol_data_url(symbol, 18.0, 2.0), "checked": checked, "visible": visible })
            })
            .collect(),
    )
}

fn main() {
    let mtm = MainThreadMarker::new().expect("main thread");
    let args: Vec<String> = env::args().skip(1).collect();
    let out = args
        .first()
        .cloned()
        .unwrap_or_else(|| "/tmp/tray.png".into());
    let dark = args.iter().any(|a| a == "--dark");
    let value = |name: &str| {
        args.iter()
            .position(|a| a == name)
            .and_then(|i| args.get(i + 1))
    };
    let height: f64 = value("--height")
        .and_then(|h| h.parse().ok())
        .unwrap_or(460.0);
    let hover: Option<usize> = value("--hover").and_then(|n| n.parse().ok());
    let (access, recording) = (
        args.iter().any(|a| a == "--access"),
        args.iter().any(|a| a == "--recording"),
    );
    let icon = image_png(APP_ICON, 28.0, 2.0)
        .map(|png| format!("data:image/png;base64,{}", base64(&png)))
        .unwrap_or_default();
    let page = PAGE.replace("/*__ICON__*/", &icon);
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
                    Some("ready") => stage.host.post(
                        &json!({ "t": "state", "items": items(access, recording) }).to_string(),
                    ),
                    Some("js_error") => println!("PAGE ERROR: {text}"),
                    _ => {}
                }
            }
        };
        let stage = Rc::new(HiddenStage::new(
            mtm,
            (WIDTH, height),
            dark,
            &page,
            on_message,
        ));
        *slot.borrow_mut() = Some(Rc::clone(&stage));
        let backdrop = if dark {
            "linear-gradient(135deg,#1d2233,#2e2a4a)"
        } else {
            "linear-gradient(135deg,#cfd6e6,#e8d9f2)"
        };
        let shot = Rc::clone(&stage);
        app::after(1.0, move || {
            shot.host.eval(&format!(
                "document.documentElement.style.background = '{backdrop}'"
            ));
            if let Some(n) = hover {
                shot.host.eval(&format!(
                    "document.querySelectorAll('.tool')[{n}].dispatchEvent(new PointerEvent('pointerenter'))"
                ));
            }
        });
        let shot = Rc::clone(&stage);
        app::after(2.0, move || {
            shot.host.snapshot_png(mtm, WIDTH, move |png| {
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
