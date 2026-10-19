//! Runs the real update popup in WebKit without showing a window.
//!
//!   update_preview selftest [news]
//!       runs the page's own checks; exits with status 1 on a failure
//!   update_preview snap <available|installing|ready|blocked|news> <out.png> [--dark] [--lang vi] [--few] [--slide N]
//!       renders one state to a PNG; --few shows a short list of changes instead of a long one

use macos_sys::{MainThreadMarker, app, webview::HiddenStage, widgets::image_png};
use serde_json::{Value, json};
use std::{cell::RefCell, env, fs, process, rc::Rc};

#[path = "../src/update/page.rs"]
mod page;

const WIDTH: f64 = 460.0;
const HEIGHT: f64 = 540.0;
/// The window of the news popup.
const NEWS_SIZE: (f64, f64) = (520.0, 580.0);
const APP_ICON: &[u8] = include_bytes!("../assets/Figghost.icns");

fn base64(bytes: &[u8]) -> String {
    const TABLE: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        let n = chunk
            .iter()
            .enumerate()
            .fold(0u32, |n, (i, &b)| n | u32::from(b) << (16 - 8 * i));
        for i in 0..4 {
            if i <= chunk.len() {
                out.push(char::from(TABLE[(n >> (18 - 6 * i) & 63) as usize]));
            } else {
                out.push('=');
            }
        }
    }
    out
}

fn option(args: &[String], name: &str) -> Option<String> {
    args.iter()
        .position(|a| a == name)
        .and_then(|i| args.get(i + 1))
        .cloned()
}

/// What the app would send the popup, with changes like the ones a real release lists.
fn sample(state: &str, lang: &str, few: bool) -> Value {
    if state == "news" {
        let changes = if few {
            vec!["Add a thing: it does this", "Fix another"]
        } else {
            vec![
                "New app icon: a friendlier ghost on a deep blue glass tile",
                "Sidebar with names: every page shows its name beside its icon",
                "Notch size styles: pick Compact, Standard or Wide, or set each size yourself",
                "Cleaner Drag wheel page",
                "What's new after an update: slides when there is a lot",
            ]
        };
        return json!({ "t": "show", "lang": lang, "version": "0.8.0", "changes": changes });
    }
    let mut changes = vec![
        "Add a Mole page for health, clean, optimize, purge, uninstall, disk usage and history",
        "Check for updates downloads and installs the new version; restart Figghost to use it",
        "Ask once a day whether to update, and say what is new in the version",
    ];
    if !few {
        changes.extend([
            "Update automatically, as an option on the About page",
            "Keep the clipboard list in step with the notch while a recording is running",
            "Fix the volume indicator staying on screen after the last key press of a long hold, which only showed when the Mac was busy with something else",
            "Smooth the opening of the notch on screens without a hardware notch",
            "Show the next meeting in the closed notch five minutes before it starts",
            "Read lyrics from the file next to the track before asking the network",
            "Translate Vietnamese text on the clipboard without sending it anywhere",
        ]);
    }
    json!({
        "t": "show",
        "lang": lang,
        "state": state,
        "version": "0.6.0",
        "current": "0.5.0",
        "changes": changes,
        "stage": "downloading",
        "percent": 42,
        "detail": "cannot write to /Applications: Permission denied",
    })
}

/// Runs the page's own checks. A page that has not finished loading has no checks to run yet, so a missing
/// answer is asked for again for a while before it counts.
fn run_selftest(stage: Rc<HiddenStage>, attempt: u32) {
    let again = Rc::clone(&stage);
    stage
        .host
        .eval_string("JSON.stringify(window.__selftest())", move |result| {
            if result.is_none() && attempt < 30 {
                app::after(0.5, move || run_selftest(again, attempt + 1));
                return;
            }
            let failures: Vec<String> = result
                .and_then(|r| serde_json::from_str(&r).ok())
                .unwrap_or_else(|| vec!["the self-test did not return".into()]);
            for failure in &failures {
                println!("FAIL: {failure}");
            }
            println!(
                "{}",
                if failures.is_empty() {
                    "update popup ok"
                } else {
                    "update popup broken"
                }
            );
            process::exit(i32::from(!failures.is_empty()));
        });
}

fn main() {
    let mtm = MainThreadMarker::new().expect("main thread");
    let args: Vec<String> = env::args().skip(1).collect();
    let mode = args.first().cloned().unwrap_or_default();
    let news = args.get(1).is_some_and(|a| a == "news");
    let (width, height) = if news { NEWS_SIZE } else { (WIDTH, HEIGHT) };
    let slide: u32 = option(&args, "--slide").and_then(|n| n.parse().ok()).unwrap_or(0);
    if mode != "selftest" && mode != "snap" {
        eprintln!(
            "usage: update_preview selftest | snap <state> <out.png> [--dark] [--lang vi] [--few]"
        );
        process::exit(2);
    }
    let snapping = mode == "snap";
    let dark = args.iter().any(|a| a == "--dark");
    let few = args.iter().any(|a| a == "--few");
    let lang = option(&args, "--lang").unwrap_or_else(|| "en".into());
    let state = args.get(1).cloned().unwrap_or_else(|| "available".into());
    let out = args
        .get(2)
        .cloned()
        .unwrap_or_else(|| "/tmp/update.png".into());
    let icon = image_png(APP_ICON, 64.0, 2.0)
        .map(|png| format!("data:image/png;base64,{}", base64(&png)))
        .unwrap_or_default();
    let html = if news {
        page::news_html(&icon)
    } else {
        page::html(&icon)
    };

    app::run_accessory(mtm, move || {
        let slot: Rc<RefCell<Option<Rc<HiddenStage>>>> = Rc::default();
        let on_message = {
            let (slot, shown) = (Rc::clone(&slot), sample(&state, &lang, few));
            move |text: String| {
                let Some(stage) = slot.borrow().clone() else {
                    return;
                };
                let message: Value = serde_json::from_str(&text).unwrap_or(Value::Null);
                match message["t"].as_str() {
                    Some("ready") if snapping => stage.host.post(&shown.to_string()),
                    Some("js_error") => println!("PAGE ERROR: {text}"),
                    _ => {}
                }
            }
        };
        let stage = Rc::new(HiddenStage::new(
            mtm,
            (width, height),
            dark,
            &html,
            on_message,
        ));
        *slot.borrow_mut() = Some(Rc::clone(&stage));

        if !snapping {
            let stage = Rc::clone(&stage);
            app::after(1.0, move || run_selftest(stage, 0));
        } else {
            // The window of the app is glass; the page itself is transparent, so give the picture a ground,
            // once the page has loaded.
            let ground = if dark { "#2b2b30" } else { "#ececf1" };
            let paint = Rc::clone(&stage);
            app::after(0.8, move || {
                paint.host.eval(&format!(
                    "document.documentElement.style.background = '{ground}'"
                ));
            });
            // Slides: step forward to the one asked for, with the slide's own motion given time to end.
            let step = Rc::clone(&stage);
            app::after(0.9, move || {
                for _ in 0..slide {
                    step.host.eval("document.getElementById('next').click()");
                }
            });
            let shot = Rc::clone(&stage);
            app::after(2.2, move || {
                shot.host.snapshot_png(mtm, width, move |png| {
                    let Some(png) = png else {
                        println!("the snapshot failed");
                        process::exit(1);
                    };
                    fs::write(&out, &png).expect("write png");
                    println!("wrote {out} ({} bytes)", png.len());
                    process::exit(0);
                });
            });
        }
        stage
    });
}
