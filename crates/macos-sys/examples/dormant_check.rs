//! Checks what a dormant page is: its tab leaves the strip and comes back where it was, what the person chose in
//! `Style::pages` is left alone, `open_page` does not open it, an open notch that is on it moves to the first tab, and
//! the strip is never left empty. The notch is placed outside every screen. Exits with status 1 on a failure.
//! `cargo run -p macos-sys --example dormant_check`

use macos_sys::{
    MainThreadMarker, app,
    notch::{Notch, NotchGeometry, Page, Style},
    ui::AppCallbacks,
};
use objc2_app_kit::NSView;
use objc2_foundation::{NSPoint, NSRect, NSSize};
use std::{process, rc::Rc};

const IDS: [&str; 4] = ["one", "two", "three", "four"];

fn page(mtm: MainThreadMarker, notch: &Notch, id: &'static str, width: f64) -> Page {
    Page {
        id,
        title: id.into(),
        symbol: "circle.fill",
        root: NSView::initWithFrame(mtm.alloc(), notch.page_area(width, 100.0)),
        on_show: Box::new(|| {}),
        on_hide: Box::new(|| {}),
        on_key: Box::new(|_| false),
        accepts_files: false,
        size: None,
    }
}

fn check(ok: bool, what: &str) {
    println!("{}  {what}", if ok { "ok    " } else { "FAILED" });
    if !ok {
        process::exit(1);
    }
}

fn main() {
    let mtm = MainThreadMarker::new().expect("main thread");
    app::run_accessory(mtm, move || {
        let geometry = NotchGeometry {
            screen: NSRect::new(NSPoint::new(-30_000.0, 0.0), NSSize::new(1_920.0, 1_080.0)),
            notch: NSSize::new(200.0, 24.0),
            hardware: false,
        };
        let noop = AppCallbacks {
            on_query: Box::new(|_, _| {}),
            on_activate: Box::new(|_, _, _| {}),
            on_pin: Box::new(|_, _| {}),
            on_delete: Box::new(|_, _| {}),
        };
        let notch = Rc::new(Notch::new(mtm, geometry, noop, |_| {}));
        for id in IDS {
            // "two" is the widest page.
            let width = if id == "two" { 700.0 } else { 500.0 };
            notch.add_page(page(mtm, &notch, id, width));
        }
        notch.configure(&Style::default());
        let steps = Rc::clone(&notch);
        app::after(0.2, move || {
            let all = ["clipboard", "one", "two", "three", "four"];
            check(
                steps.tab_ids() == all,
                "every page has its tab to begin with",
            );
            steps.set_page_dormant("two", true);
            check(
                steps.tab_ids() == ["clipboard", "one", "three", "four"],
                "a dormant page leaves the strip",
            );
            steps.set_page_dormant("two", true);
            check(
                steps.tab_ids().len() == 4,
                "making it dormant twice changes nothing",
            );
            steps.open_page(2);
            check(!steps.is_expanded(), "a dormant page is not opened");
            steps.set_page_dormant("two", false);
            check(
                steps.tab_ids() == all,
                "waking it puts the tab back where it was",
            );
            steps.set_page_dormant("nothing", true);
            check(
                steps.tab_ids() == all,
                "an id that is no page changes nothing",
            );
            steps.set_page_dormant("nothing", false);

            // The person's own choice of tabs and order is left alone, and dormancy only takes tabs out of it.
            steps.configure(&Style {
                pages: ["four", "two", "one"].map(String::from).to_vec(),
                ..Style::default()
            });
            check(
                steps.tab_ids() == ["four", "two", "one"],
                "the chosen tabs are shown in the chosen order",
            );
            steps.set_page_dormant("two", true);
            check(
                steps.tab_ids() == ["four", "one"],
                "a dormant page leaves the chosen strip too",
            );
            steps.set_page_dormant("two", false);
            check(
                steps.tab_ids() == ["four", "two", "one"],
                "and returns to its place in it",
            );

            // An open notch that is on the page when it goes moves to the first tab and stays open.
            steps.configure(&Style::default());
            steps.open_page(3);
            check(steps.is_expanded(), "the notch opens on a page");
            steps.set_page_dormant("three", true);
            check(
                steps.is_expanded(),
                "an open notch stays open when its page goes dormant",
            );
            check(!steps.tab_ids().contains(&"three"), "and its tab is gone");
            steps.set_page_dormant("three", false);
            steps.close();

            // The open shape is as wide as the widest page that shows, so it narrows when that page goes dormant.
            steps.open_page(2);
            let wide = steps.open_size().width;
            check(
                wide >= 700.0,
                "the open shape is as wide as the widest page",
            );
            steps.set_page_dormant("two", true);
            check(
                steps.open_size().width < wide,
                "the open shape narrows when the widest page goes dormant",
            );
            steps.set_page_dormant("two", false);
            steps.close();

            // The strip is never left empty.
            for id in IDS {
                steps.set_page_dormant(id, true);
            }
            check(
                steps.tab_ids() == ["clipboard"],
                "with every other page dormant the clipboard is left",
            );
            steps.set_page_dormant("clipboard", true);
            check(
                steps.tab_ids().len() == 1,
                "even with every page dormant one tab is left",
            );
            process::exit(0);
        });
        notch
    });
}
