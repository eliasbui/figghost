//! Checks the EventKit wrapper without asking for anything: prints the permission states, whether this build
//! could ask, and what an unauthorised store returns. It stops before reading when access is already given,
//! so it never looks at real calendars.
//! `cargo run -p macos-sys --example eventkit_check`

use macos_sys::{
    MainThreadMarker, app,
    eventkit::{self, Access, Kind, Store},
};
use std::process;

fn main() {
    let mtm = MainThreadMarker::new().expect("main thread");
    app::run_accessory(mtm, move || {
        let (events, reminders) = (
            eventkit::access(Kind::Events),
            eventkit::access(Kind::Reminders),
        );
        println!("events access: {}", events.code());
        println!("reminders access: {}", reminders.code());
        println!(
            "can prompt: events {} reminders {}",
            eventkit::can_prompt(Kind::Events),
            eventkit::can_prompt(Kind::Reminders)
        );
        let now = eventkit::now();
        println!("offset now: {} s", eventkit::utc_offset(now));
        if events == Access::Granted || reminders == Access::Granted {
            println!("access is already given to this process: stopping before any read");
            process::exit(0);
        }
        let store = Store::new();
        let _watch = store.observe_changes(|| println!("store changed"));
        println!("calendars: {}", store.calendars(Kind::Events).len());
        println!("lists: {}", store.calendars(Kind::Reminders).len());
        println!("events: {}", store.events(now, now + 86_400, &[]).len());
        // Without a usage text in the Info.plist the request is refused, never made.
        match store.request(Kind::Events, |_| println!("unexpected answer")) {
            Ok(()) => println!("request: made"),
            Err(e) => println!("request: refused ({e})"),
        }
        store.reminders(now + 86_400, &[], |list| {
            println!("reminders: {}", list.len());
            process::exit(0);
        });
        app::after(5.0, || {
            println!("no answer from the reminders fetch");
            process::exit(0);
        });
        store
    });
}
