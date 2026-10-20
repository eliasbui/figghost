//! Checks what snoozing the notch does and does not do: an open notch closes when it is snoozed, and what opens it
//! by itself (a coding agent that asks, an alert) and a click still open it, so nothing that waits for the person
//! is lost. The pointer's own hover cannot be driven from here, so that it does not open the notch is a read of the
//! code and not a check. The notch is placed outside every screen. Exits with status 1 on a failure.
//! `cargo run -p macos-sys --example snooze_check`

use macos_sys::{
    MainThreadMarker, app,
    notch::{Notch, NotchGeometry, Style},
    ui::AppCallbacks,
};
use objc2_foundation::{NSPoint, NSRect, NSSize};
use std::{process, rc::Rc};

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
        notch.configure(&Style::default());
        let steps = Rc::clone(&notch);
        app::after(0.2, move || {
            check(!steps.is_snoozed(), "a notch is not snoozed to begin with");
            steps.open();
            check(steps.is_expanded(), "the notch opens");
            steps.set_snoozed(true);
            check(steps.is_snoozed(), "it can be snoozed");
            check(
                !steps.is_expanded(),
                "an open notch closes when it is snoozed",
            );
            steps.open_page(0);
            check(
                steps.is_expanded(),
                "a snoozed notch still opens for what opens it by itself",
            );
            steps.close();
            steps.open_focused();
            check(
                steps.is_expanded(),
                "a snoozed notch still opens for a click",
            );
            steps.set_snoozed(false);
            check(!steps.is_snoozed(), "it can be woken");
            check(
                steps.is_expanded(),
                "waking it does not close an open notch",
            );
            process::exit(0);
        });
        notch
    });
}
