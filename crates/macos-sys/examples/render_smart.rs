//! Draws the Smart Select window into PNGs without showing it, and checks its keys and clicks with events
//! delivered in the process. `cargo run -p macos-sys --example render_smart -- /tmp/smart`

use macos_sys::{
    MainThreadMarker,
    smart_ui::{Body, Content, SmartPopup, SmartRow},
    style::Tone,
};
use objc2_app_kit::{
    NSApplication, NSApplicationActivationPolicy, NSEvent, NSEventModifierFlags, NSEventType,
};
use objc2_foundation::{NSPoint, NSString};
use std::{cell::RefCell, env, fs, path::Path, rc::Rc};

fn row(symbol: &'static str, tone: Tone, title: &str, detail: &str) -> SmartRow {
    SmartRow {
        symbol,
        tone,
        title: title.into(),
        detail: detail.into(),
    }
}

fn save(popup: &SmartPopup, dir: &Path, name: &str) {
    let png = popup.snapshot_png().expect("png");
    let path = dir.join(name);
    fs::write(&path, png).expect("write png");
    println!("wrote {}", path.display());
}

/// A key press as the window would receive it from the system.
fn key(
    code: u16,
    modifiers: NSEventModifierFlags,
    characters: &str,
) -> objc2::rc::Retained<NSEvent> {
    NSEvent::keyEventWithType_location_modifierFlags_timestamp_windowNumber_context_characters_charactersIgnoringModifiers_isARepeat_keyCode(
        NSEventType::KeyDown,
        NSPoint::ZERO,
        modifiers,
        0.0,
        0,
        None,
        &NSString::from_str(characters),
        &NSString::from_str(characters),
        false,
        code,
    )
    .expect("a key event")
}

fn main() {
    let mtm = MainThreadMarker::new().expect("main thread");
    let dir = env::args().nth(1).unwrap_or_else(|| "/tmp/smart".into());
    let dir = Path::new(&dir);
    fs::create_dir_all(dir).expect("output directory");
    NSApplication::sharedApplication(mtm)
        .setActivationPolicy(NSApplicationActivationPolicy::Accessory);

    let copied: Rc<RefCell<Vec<String>>> = Rc::default();
    let popup = {
        let copied = Rc::clone(&copied);
        SmartPopup::new(mtm, move |text| copied.borrow_mut().push(text))
    };

    popup.update(Content {
        header: "5 km".into(),
        body: Body::Rows(vec![
            row("ruler", Tone::Blue, "3.107 mi", "Length"),
            row("ruler", Tone::Blue, "16,404 ft", "Length"),
        ]),
        footer: "Return copies, Esc closes".into(),
    });
    save(&popup, dir, "units.png");

    popup.update(Content {
        header: "100.00 USD".into(),
        body: Body::Rows(vec![
            row(
                "dollarsign.circle",
                Tone::Green,
                "89.44 EUR",
                "1 USD = 0.8944 EUR",
            ),
            row(
                "dollarsign.circle",
                Tone::Green,
                "2,589,020 VND",
                "1 USD = 25,890 VND",
            ),
        ]),
        footer: "Rates of 15 Oct. Return copies, Esc closes".into(),
    });
    save(&popup, dir, "money.png");

    popup.update(Content {
        header: "3:00 PM PST".into(),
        body: Body::Rows(vec![
            row("clock", Tone::Orange, "6:00 AM", "Local time, tomorrow"),
            row("clock", Tone::Orange, "11:00 PM", "UTC"),
            row("clock", Tone::Orange, "8:00 AM", "Tokyo, tomorrow"),
            row("clock", Tone::Orange, "7:00 PM", "New York"),
            row("clock", Tone::Orange, "12:00 AM", "London, tomorrow"),
        ]),
        footer: "Return copies, Esc closes".into(),
    });
    save(&popup, dir, "times.png");

    popup.update(Content {
        header: "Vietnamese".into(),
        body: Body::Text {
            text: "Good morning. The meeting starts at nine and we still need to prepare the slides for the review, so please come early if you can.".into(),
            muted: false,
        },
        footer: "Return copies, Esc closes".into(),
    });
    save(&popup, dir, "translation.png");

    popup.update(Content {
        header: "Hello there".into(),
        body: Body::Text {
            text: "Turn on the internet in Settings to translate this text.".into(),
            muted: true,
        },
        footer: "Esc closes".into(),
    });
    save(&popup, dir, "hint.png");

    // Return on a list copies the highlighted row.
    popup.update(Content {
        header: "5 km".into(),
        body: Body::Rows(vec![
            row("ruler", Tone::Blue, "3.107 mi", "Length"),
            row("ruler", Tone::Blue, "16,404 ft", "Length"),
        ]),
        footer: String::new(),
    });
    popup.activate_selected();
    assert_eq!(copied.borrow().as_slice(), ["3.107 mi"]);

    // The arrow keys move the highlight and Return copies what is highlighted, through the window's own key
    // handling, as the keyboard would deliver them.
    popup.update(Content {
        header: "5 km".into(),
        body: Body::Rows(vec![
            row("ruler", Tone::Blue, "3.107 mi", "Length"),
            row("ruler", Tone::Blue, "16,404 ft", "Length"),
            row("ruler", Tone::Blue, "1 x", "Length"),
        ]),
        footer: String::new(),
    });
    let none = NSEventModifierFlags::empty();
    for code in [125, 125, 125, 126, 36] {
        popup.send_event(&key(code, none, ""));
    }
    assert_eq!(
        copied.borrow().as_slice(),
        ["3.107 mi", "16,404 ft"],
        "down, down, down (stops at the end), up, return"
    );
    // Command and a digit copy that row.
    popup.send_event(&key(18, NSEventModifierFlags::Command, "1"));
    assert_eq!(copied.borrow().last().map(String::as_str), Some("3.107 mi"));

    // A message is not an answer and cannot be copied.
    popup.update(Content {
        header: "x".into(),
        body: Body::Text {
            text: "Nothing here".into(),
            muted: true,
        },
        footer: String::new(),
    });
    let before = copied.borrow().len();
    popup.activate_selected();
    popup.send_event(&key(36, none, ""));
    assert_eq!(copied.borrow().len(), before);
    println!("smart ok");
}
