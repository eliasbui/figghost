//! Renders the music widget's page in WebKit without showing a window, over a backdrop, to a PNG.
//!
//!   widget_preview <out.png> <card|cover|bar|vinyl|minimal> [--width 340] [--height 132] [--idle]
//!
//! The glass is drawn by the window server, so the backdrop stands in for the desktop behind it.

use macos_sys::{
    MainThreadMarker, app,
    webview::HiddenStage,
    widgets::{base64, image_png},
};
use serde_json::{Value, json};
use std::{cell::RefCell, env, fs, process, rc::Rc};

const PAGE: &str = include_str!("../web/widget.html");
const COVER: &[u8] = include_bytes!("../assets/mascots/ufo.png");

fn main() {
    let mtm = MainThreadMarker::new().expect("main thread");
    let args: Vec<String> = env::args().skip(1).collect();
    let out = args
        .first()
        .cloned()
        .unwrap_or_else(|| "/tmp/widget.png".into());
    let style = args.get(1).cloned().unwrap_or_else(|| "card".into());
    let value = |name: &str| {
        args.iter()
            .position(|a| a == name)
            .and_then(|i| args.get(i + 1))
            .and_then(|v| v.parse::<f64>().ok())
    };
    let (dw, dh) = match style.as_str() {
        "cover" => (240.0, 240.0),
        "bar" => (380.0, 64.0),
        "vinyl" => (240.0, 310.0),
        "minimal" => (280.0, 84.0),
        _ => (340.0, 132.0),
    };
    let (w, h) = (
        value("--width").unwrap_or(dw),
        value("--height").unwrap_or(dh),
    );
    let idle = args.iter().any(|a| a == "--idle");
    let art = image_png(COVER, 300.0, 2.0)
        .map(|png| format!("data:image/png;base64,{}", base64(&png)))
        .unwrap_or_default();
    app::run_accessory(mtm, move || {
        let slot: Rc<RefCell<Option<Rc<HiddenStage>>>> = Rc::default();
        let (style_for, art_for) = (style.clone(), art.clone());
        let on_message = {
            let slot = Rc::clone(&slot);
            move |text: String| {
                let Some(stage) = slot.borrow().clone() else {
                    return;
                };
                let message: Value = serde_json::from_str(&text).unwrap_or(Value::Null);
                match message["t"].as_str() {
                    Some("ready") => {
                        let track = (!idle).then(|| json!({
                            "title": "Midnight City", "artist": "M83", "album": "Hurry Up, We're Dreaming",
                            "app": "Music", "playing": true, "pos": 96000, "dur": 243000,
                        }));
                        stage.host.post(
                            &json!({
                                "t": "state", "style": style_for, "ambient": true,
                                "radius": if style_for == "bar" { h / 2.0 } else { 26.0 },
                                "labels": {}, "track": track, "art": art_for,
                            })
                            .to_string(),
                        );
                    }
                    Some("js_error") => println!("PAGE ERROR: {text}"),
                    _ => {}
                }
            }
        };
        let stage = Rc::new(HiddenStage::new(mtm, (w, h), true, PAGE, on_message));
        *slot.borrow_mut() = Some(Rc::clone(&stage));
        let shot = Rc::clone(&stage);
        app::after(1.0, move || {
            shot.host.eval("document.documentElement.style.background = 'linear-gradient(135deg,#2b3a67,#6b3f7a)'");
        });
        let shot = Rc::clone(&stage);
        app::after(2.0, move || {
            shot.host.snapshot_png(mtm, w * 2.0, move |png| {
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
