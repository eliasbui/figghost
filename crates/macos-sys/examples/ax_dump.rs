//! Prints the accessibility tree of one process: its windows with their position, and what is inside them.
//! `cargo run -p macos-sys --example ax_dump -- <pid>`. Reads only; the terminal needs Accessibility access.

use macos_sys::ax::Element;
use std::env;

fn show(element: &Element, depth: usize) {
    let text = |name: &str| element.string(name).unwrap_or_default();
    let at = element
        .point("AXPosition")
        .map(|(x, y)| format!(" at ({x:.0}, {y:.0})"))
        .unwrap_or_default();
    println!(
        "{}{} {:?} label={:?} value={:?}{at}",
        "  ".repeat(depth),
        text("AXRole"),
        text("AXTitle"),
        text("AXDescription"),
        text("AXValue"),
    );
    if depth < 8 {
        for child in element.elements("AXChildren") {
            show(&child, depth + 1);
        }
    }
}

fn main() {
    let pid: i32 = env::args()
        .nth(1)
        .and_then(|p| p.parse().ok())
        .expect("usage: ax_dump <pid>");
    let Some(app) = Element::application(pid) else {
        println!("no such process");
        return;
    };
    let windows = app.elements("AXWindows");
    println!("{} windows", windows.len());
    for window in &windows {
        show(window, 0);
    }
}
