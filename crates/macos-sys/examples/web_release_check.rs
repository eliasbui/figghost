//! Opens and drops the real glass window several times and checks that the web view is released each time and
//! that no WebKit helper process of this run outlives the last window. Nothing is shown on screen.
//! WebKit keeps its GPU helper for about a minute after the last page, so a run takes about 70 seconds.
//! Exits with status 1 on a failure. `cargo run -p macos-sys --example web_release_check`

use macos_sys::{
    MainThreadMarker, app,
    webview::{WebWindow, WindowSpec},
};
use objc2::rc::Weak;
use std::{
    cell::RefCell,
    collections::BTreeSet,
    process::{self, Command},
    rc::Rc,
    time::Instant,
};

const CYCLES: usize = 4;
/// How long WebKit may keep its helpers after the last window; `WEB_RELEASE_WAIT` overrides it.
const HELPERS_MAY_TAKE_SECONDS: u32 = 90;

struct State {
    baseline: BTreeSet<u32>,
    failures: RefCell<Vec<String>>,
}

fn helpers() -> BTreeSet<u32> {
    let out = Command::new("pgrep")
        .args(["-f", r"com\.apple\.WebKit\.(WebContent|GPU|Networking)"])
        .output()
        .expect("pgrep runs");
    String::from_utf8_lossy(&out.stdout)
        .lines()
        .filter_map(|line| line.trim().parse().ok())
        .collect()
}

/// What a helper process is and how much memory it holds, for the report.
fn describe(pid: u32) -> String {
    let text = |args: &[&str]| {
        let out = Command::new(args[0])
            .args(&args[1..])
            .output()
            .expect("tool runs");
        String::from_utf8_lossy(&out.stdout).trim().to_owned()
    };
    let name = text(&["ps", "-o", "comm=", "-p", &pid.to_string()]);
    let footprint = text(&[
        "sh",
        "-c",
        &format!(
            "footprint -p {pid} 2>/dev/null | grep -m1 phys_footprint: | awk '{{print $2, $3}}'"
        ),
    ]);
    format!(
        "{pid} {} ({footprint})",
        name.rsplit('/').next().unwrap_or(&name)
    )
}

fn cycle(mtm: MainThreadMarker, state: Rc<State>, n: usize) {
    if n == CYCLES {
        return wait_for_helpers(state, Instant::now(), 0);
    }
    let window = WebWindow::new(
        mtm,
        &WindowSpec {
            title: "release check",
            width: 900.0,
            height: 600.0,
            min_width: 600.0,
            min_height: 400.0,
        },
        "<!doctype html><body>release check</body>",
        |_| {},
        || {},
    );
    let view = Weak::from_retained(&window.host.view);
    let started = helpers().difference(&state.baseline).count();
    app::after(1.0, move || {
        let running = helpers().difference(&state.baseline).count();
        println!(
            "cycle {}: helpers of this run before it, then with the page loaded: {started}, {running}",
            n + 1
        );
        drop(window);
        app::after(1.5, move || {
            let released = view.load().is_none();
            println!(
                "cycle {}: web view released after the window was dropped: {released}",
                n + 1
            );
            if !released {
                state
                    .failures
                    .borrow_mut()
                    .push(format!("cycle {} kept the web view alive", n + 1));
            }
            cycle(mtm, state, n + 1);
        });
    });
}

fn wait_for_helpers(state: Rc<State>, since: Instant, waited: u32) {
    let limit = std::env::var("WEB_RELEASE_WAIT")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(HELPERS_MAY_TAKE_SECONDS);
    let current = helpers();
    let remaining: Vec<u32> = current.difference(&state.baseline).copied().collect();
    let left = remaining.len();
    if waited % 5 == 0 {
        let names: Vec<String> = remaining.iter().map(|p| describe(*p)).collect();
        println!("  {waited:>2} s after the last window: {left} helper(s) {names:?}");
    }
    if left == 0 {
        println!(
            "no WebKit helper of this run is left, {:.0} s after the last window was dropped",
            since.elapsed().as_secs_f64()
        );
    } else if waited >= limit {
        println!("{left} WebKit helper(s) of this run still alive after {waited} s");
        state
            .failures
            .borrow_mut()
            .push("WebKit helpers outlive the last window".to_owned());
    } else {
        return app::after(1.0, move || wait_for_helpers(state, since, waited + 1));
    }
    let failures = state.failures.borrow();
    for f in failures.iter() {
        println!("FAIL: {f}");
    }
    println!(
        "{}",
        if failures.is_empty() {
            "release ok"
        } else {
            "release broken"
        }
    );
    process::exit(i32::from(!failures.is_empty()));
}

fn main() {
    let mtm = MainThreadMarker::new().expect("main thread");
    app::run_accessory(mtm, move || {
        let state = Rc::new(State {
            baseline: helpers(),
            failures: RefCell::new(Vec::new()),
        });
        cycle(mtm, Rc::clone(&state), 0);
        state
    });
}
