//! Checks the web view bridge end to end in a fully transparent window: messages in both directions,
//! cancelled navigation, script error reports and snapshots. Exits with status 1 on a failure.
//! `cargo run -p macos-sys --example web_probe -- /tmp/probe.png`

use macos_sys::{MainThreadMarker, app, webview::WebHost};
use objc2::rc::Retained;
use objc2_app_kit::{NSBackingStoreType, NSWindow, NSWindowStyleMask};
use objc2_foundation::{NSPoint, NSRect, NSSize};
use std::{cell::RefCell, env, fs, process, rc::Rc};

const PAGE: &str = r#"<!doctype html><html><body style="margin:0;background:linear-gradient(135deg,#3b82f6,#a855f7);color:white;font:20px -apple-system">
<div id="out" style="padding:24px">waiting</div>
<script>
window.__fig = { recv: function (m) {
  document.getElementById('out').textContent = 'got:' + m.t + ':' + m.v;
  window.fig.send({ t: 'echo', v: m.v });
}};
window.fig.send({ t: 'ready' });
setTimeout(function () { throw new Error('boom-inline'); }, 300);
</script></body></html>"#;

fn main() {
    let mtm = MainThreadMarker::new().expect("main thread");
    let out = env::args()
        .nth(1)
        .unwrap_or_else(|| "/tmp/probe.png".into());
    app::run_accessory(mtm, move || {
        let frame = NSRect::new(NSPoint::ZERO, NSSize::new(480.0, 160.0));
        // SAFETY: designated initializer with valid arguments on the main thread.
        let window = unsafe {
            NSWindow::initWithContentRect_styleMask_backing_defer(
                mtm.alloc(),
                frame,
                NSWindowStyleMask::Borderless,
                NSBackingStoreType::Buffered,
                false,
            )
        };
        // SAFETY: false is the only value compatible with a window kept alive by `Retained`.
        unsafe { window.setReleasedWhenClosed(false) };
        window.setAlphaValue(0.0);
        window.setIgnoresMouseEvents(true);

        let seen: Rc<RefCell<Vec<String>>> = Rc::default();
        let host: Rc<RefCell<Option<Rc<WebHost>>>> = Rc::default();
        let on_message = {
            let (seen, host) = (Rc::clone(&seen), Rc::clone(&host));
            move |text: String| {
                seen.borrow_mut().push(text.clone());
                if text.contains(r#""t":"ready""#)
                    && let Some(host) = host.borrow().as_ref()
                {
                    host.post(r#"{"t":"hello","v":42}"#);
                }
            }
        };
        let web = Rc::new(WebHost::new(mtm, frame, PAGE, on_message, |_| {}, |_| {}));
        *host.borrow_mut() = Some(Rc::clone(&web));
        window.setContentView(Some(&web.view));
        window.orderFrontRegardless();

        let stage = Rc::new(window);
        app::after(1.0, {
            let web = Rc::clone(&web);
            move || {
                // A navigation away must be cancelled.
                web.eval("location.href = 'https://example.com/'");
            }
        });
        app::after(2.2, {
            let (web, seen, out) = (Rc::clone(&web), Rc::clone(&seen), out.clone());
            move || {
                let messages = seen.borrow().clone();
                println!("messages from the page: {messages:?}");
                let mut failures = Vec::new();
                let has = |needle: &str| messages.iter().any(|m| m.contains(needle));
                if !has(r#""t":"ready""#) {
                    failures.push("the page never said ready");
                }
                if !has(r#""t":"echo","v":42"#) {
                    failures.push("the reply from Rust never reached the page and came back");
                }
                if !has("boom-inline") {
                    failures.push("the script error was not reported");
                }
                let failures = Rc::new(RefCell::new(failures));
                let web2 = Rc::clone(&web);
                let failures2 = Rc::clone(&failures);
                web.eval_string(
                    "document.getElementById('out').textContent + ' | ' + location.href",
                    move |text| {
                        println!("page state: {text:?}");
                        let text = text.unwrap_or_default();
                        if !text.starts_with("got:hello:42") {
                            failures2
                                .borrow_mut()
                                .push("the page did not show the message from Rust");
                        }
                        if text.contains("example.com") {
                            failures2
                                .borrow_mut()
                                .push("navigation to the web was not cancelled");
                        }
                        web2.snapshot_png(mtm, 480.0, move |png| {
                            match png {
                                Some(bytes) if bytes.len() > 2000 => {
                                    fs::write(&out, &bytes).expect("write png");
                                    println!("snapshot written: {out} ({} bytes)", bytes.len());
                                }
                                other => {
                                    println!("snapshot problem: {:?}", other.map(|b| b.len()));
                                    failures.borrow_mut().push("the snapshot was empty");
                                }
                            }
                            let failures = failures.borrow();
                            for f in failures.iter() {
                                println!("FAIL: {f}");
                            }
                            println!(
                                "{}",
                                if failures.is_empty() {
                                    "web bridge ok"
                                } else {
                                    "web bridge broken"
                                }
                            );
                            process::exit(i32::from(!failures.is_empty()));
                        });
                    },
                );
            }
        });
        (stage, web)
    });
    let _: Option<Retained<NSWindow>> = None;
}
