//! A second process that offers one file as a draggable tile, standing in for Finder when checking drops
//! onto the notch. Prints `TILE x y` (global coordinates, origin top-left) once it is on screen.
//! `drag_source <file> <seconds>`

use macos_sys::{
    MainThreadMarker, app,
    panel::FigPanel,
    shelf_ui::{ShelfCallbacks, ShelfView},
};
use objc2_app_kit::{NSBox, NSBoxType, NSColor, NSScreen};
use objc2_foundation::{NSPoint, NSRect, NSSize};
use std::{env, process};

fn main() {
    let mtm = MainThreadMarker::new().expect("main thread");
    let mut args = env::args().skip(1);
    let file = args.next().expect("usage: drag_source <file> <seconds>");
    let seconds: f64 = args.next().and_then(|s| s.parse().ok()).unwrap_or(15.0);
    app::run_accessory(mtm, move || {
        let size = NSSize::new(420.0, 260.0);
        let view = ShelfView::new(
            mtm,
            NSRect::new(NSPoint::ZERO, size),
            ShelfCallbacks {
                on_drop: Box::new(|_| {}),
                on_remove: Box::new(|_| {}),
                on_open: Box::new(|_| {}),
            },
        );
        view.set_paths(vec![file]);
        let panel = FigPanel::new(mtm, NSRect::new(NSPoint::new(120.0, 420.0), size));
        // An opaque body, or the window server would not treat the panel as a drag source worth hit-testing.
        let body = NSBox::new(mtm);
        body.setBoxType(NSBoxType::Custom);
        body.setBorderWidth(0.0);
        body.setFillColor(&NSColor::blackColor());
        body.setContentViewMargins(NSSize::ZERO);
        body.setFrame(NSRect::new(NSPoint::ZERO, size));
        body.setContentView(Some(&view.root));
        panel.setContentView(Some(&body));
        panel.orderFrontRegardless();

        let reporter = std::rc::Rc::clone(&view);
        app::after(0.8, move || {
            let primary = NSScreen::screens(mtm)
                .firstObject()
                .expect("a screen")
                .frame()
                .size
                .height;
            let tile = reporter.tile_center_on_screen(0).expect("tile on screen");
            println!("TILE {} {}", tile.x, primary - tile.y);
        });
        app::after(seconds, || process::exit(0));
        (panel, view)
    });
}
