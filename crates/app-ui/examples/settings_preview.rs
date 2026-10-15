//! Runs the real settings page in WebKit without showing a window.
//!
//!   settings_preview selftest
//!       runs the page's own checks against sample data; exits with status 1 on a failure
//!   settings_preview snap <page> <out.png> [--dark] [--lang vi] [--size 1000x700] [--backdrop light|dark|color]
//!                         [--eval "script"] [--patch '{"mouse":{"smooth":false}}'] [--wait seconds]
//!       renders one page to a PNG; pages: general scroll clipboard notch tiling about
//!
//! The sample data is `web/fixture.json` plus `web/features/*.fixture.json`; a unit test keeps it in step with
//! the real state.

use macos_sys::{MainThreadMarker, app, webview::HiddenStage};
use serde_json::{Value, json};
use std::{cell::RefCell, env, fs, process, rc::Rc};

#[path = "../src/settings/page.rs"]
mod page;

fn merge(into: &mut Value, patch: &Value) {
    match (into, patch) {
        (Value::Object(a), Value::Object(b)) => {
            for (key, value) in b {
                merge(a.entry(key.clone()).or_insert(Value::Null), value);
            }
        }
        (slot, value) => *slot = value.clone(),
    }
}

fn option(args: &[String], name: &str) -> Option<String> {
    args.iter()
        .position(|a| a == name)
        .and_then(|i| args.get(i + 1))
        .cloned()
}

fn backdrop_css(kind: &str, dark: bool) -> &'static str {
    match (kind, dark) {
        ("color", _) => "linear-gradient(135deg,#ff9a8b,#ff6a88 30%,#7f5af0 65%,#2cb1ff)",
        ("dark", _) | (_, true) => "linear-gradient(135deg,#0b1020,#27215a 55%,#4a1d6e)",
        _ => "linear-gradient(135deg,#b9cdfb,#e9d8ff 50%,#ffd9e4)",
    }
}

/// Runs the page's own checks. A page that has not finished loading has no checks to run yet (on a busy
/// machine that takes longer than a second), so a missing answer is asked for again for a while before it counts.
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
                    "settings page ok"
                } else {
                    "settings page broken"
                }
            );
            process::exit(i32::from(!failures.is_empty()));
        });
}

fn main() {
    let mtm = MainThreadMarker::new().expect("main thread");
    let args: Vec<String> = env::args().skip(1).collect();
    let mode = args.first().cloned().unwrap_or_default();
    if mode != "selftest" && mode != "snap" {
        eprintln!("usage: settings_preview selftest | snap <page> <out.png> [options]");
        process::exit(2);
    }
    let dark = args.iter().any(|a| a == "--dark");
    let (width, height) = option(&args, "--size")
        .and_then(|s| {
            s.split_once('x')
                .map(|(w, h)| (w.parse().ok(), h.parse().ok()))
        })
        .and_then(|(w, h)| Some((w?, h?)))
        .unwrap_or((1000.0, 700.0));
    let mut state: Value = page::fixture();
    if let Some(patch) = option(&args, "--patch") {
        merge(
            &mut state,
            &serde_json::from_str(&patch).expect("--patch is JSON"),
        );
    }
    if let Some(lang) = option(&args, "--lang") {
        state["general"]["language"] = json!(lang);
    }
    let log = args.iter().any(|a| a == "--log");

    app::run_accessory(mtm, move || {
        let slot: Rc<RefCell<Option<Rc<HiddenStage>>>> = Rc::default();
        let on_message = {
            let (slot, state) = (Rc::clone(&slot), state.clone());
            move |text: String| {
                let Some(stage) = slot.borrow().clone() else {
                    return;
                };
                let message: Value = serde_json::from_str(&text).unwrap_or(Value::Null);
                if log {
                    eprintln!("page -> app: {text}");
                }
                match message["t"].as_str() {
                    Some("ready") => stage
                        .host
                        .post(&json!({ "t": "state", "state": state }).to_string()),
                    Some("curve") => {
                        let duration = message["duration_ms"].as_f64().unwrap_or(360.0);
                        let points: Vec<Value> = core_mouse::glide_profile(duration, 64)
                            .into_iter()
                            .map(|p| json!([p.t_ms, p.covered, p.speed]))
                            .collect();
                        stage.host.post(
                            &json!({ "t": "curve", "duration_ms": duration, "points": points })
                                .to_string(),
                        );
                    }
                    Some("icons") => stage
                        .host
                        .post(&json!({ "t": "icons", "icons": {} }).to_string()),
                    Some("running") => stage.host.post(
                        &json!({ "t": "running", "purpose": message["purpose"], "apps": [
                            { "id": "com.apple.Safari", "name": "Safari" },
                            { "id": "com.apple.Terminal", "name": "Terminal" },
                        ] })
                        .to_string(),
                    ),
                    Some("js_error") => {
                        println!("PAGE ERROR: {text}");
                    }
                    _ => {}
                }
            }
        };
        let stage = Rc::new(HiddenStage::new(
            mtm,
            (width, height),
            dark,
            &page::html(),
            on_message,
        ));
        *slot.borrow_mut() = Some(Rc::clone(&stage));

        if mode == "selftest" {
            let stage = Rc::clone(&stage);
            app::after(1.0, move || run_selftest(stage, 0));
        } else {
            let page_id = args.get(1).cloned().unwrap_or_else(|| "general".into());
            let out = args
                .get(2)
                .cloned()
                .unwrap_or_else(|| "/tmp/settings.png".into());
            let backdrop = option(&args, "--backdrop");
            let script = option(&args, "--eval");
            let stage_nav = Rc::clone(&stage);
            app::after(0.8, move || {
                if let Some(kind) = &backdrop {
                    stage_nav.host.eval(&format!(
                        "document.documentElement.style.background = '{}'",
                        backdrop_css(kind, dark)
                    ));
                }
                stage_nav.host.eval(&format!("go('{page_id}')"));
                if let Some(script) = &script {
                    stage_nav.host.eval(script);
                }
            });
            let stage_shot = Rc::clone(&stage);
            let wait: f64 = option(&args, "--wait")
                .and_then(|w| w.parse().ok())
                .unwrap_or(2.5);
            app::after(0.8 + wait, move || {
                stage_shot.host.snapshot_png(mtm, width, move |png| {
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
