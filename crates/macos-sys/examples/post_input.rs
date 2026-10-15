//! Posts synthetic input at the HID level, for end-to-end checks. Coordinates are global, measured from
//! the top-left of the main screen. Warning: whatever is under the pointer receives the clicks.
//!
//!     post_input key <keycode> [cmd] [shift] [opt] [ctrl]
//!     post_input move <x> <y>
//!     post_input click <x> <y>
//!     post_input drag <x1> <y1> <x2> <y2>
//!     post_input scroll <x> <y> <pixels>     (negative pixels scroll the content down; the pointer stays)

use objc2_core_foundation::CGPoint;
use objc2_core_graphics::{
    CGEvent, CGEventFlags, CGEventTapLocation, CGEventType, CGMouseButton, CGScrollEventUnit,
};
use std::{env, thread, time::Duration};

fn pause() {
    thread::sleep(Duration::from_millis(30));
}

fn mouse(kind: CGEventType, x: f64, y: f64) {
    let event = CGEvent::new_mouse_event(None, kind, CGPoint::new(x, y), CGMouseButton::Left);
    CGEvent::post(CGEventTapLocation::HIDEventTap, event.as_deref());
    pause();
}

fn main() {
    let args: Vec<String> = env::args().skip(1).collect();
    let num = |i: usize| -> f64 { args.get(i).and_then(|a| a.parse().ok()).expect("number") };
    match args.first().map(String::as_str) {
        Some("key") => {
            let code: u16 = args.get(1).and_then(|a| a.parse().ok()).expect("keycode");
            let mut flags = CGEventFlags::empty();
            for m in &args[2..] {
                flags |= match m.as_str() {
                    "cmd" => CGEventFlags::MaskCommand,
                    "shift" => CGEventFlags::MaskShift,
                    "opt" => CGEventFlags::MaskAlternate,
                    "ctrl" => CGEventFlags::MaskControl,
                    other => panic!("unknown modifier {other}"),
                };
            }
            // Press the modifier keys first, as a person would; some shortcut handlers check them.
            let modifier_codes: Vec<u16> = args[2..]
                .iter()
                .map(|m| match m.as_str() {
                    "cmd" => 55,
                    "shift" => 56,
                    "opt" => 58,
                    _ => 59,
                })
                .collect();
            let mut held = CGEventFlags::empty();
            for (m, &mc) in args[2..].iter().zip(&modifier_codes) {
                held |= match m.as_str() {
                    "cmd" => CGEventFlags::MaskCommand,
                    "shift" => CGEventFlags::MaskShift,
                    "opt" => CGEventFlags::MaskAlternate,
                    _ => CGEventFlags::MaskControl,
                };
                let event = CGEvent::new_keyboard_event(None, mc, true);
                CGEvent::set_flags(event.as_deref(), held);
                CGEvent::post(CGEventTapLocation::HIDEventTap, event.as_deref());
                pause();
            }
            for down in [true, false] {
                let event = CGEvent::new_keyboard_event(None, code, down);
                CGEvent::set_flags(event.as_deref(), flags);
                CGEvent::post(CGEventTapLocation::HIDEventTap, event.as_deref());
                pause();
            }
            for &mc in modifier_codes.iter().rev() {
                let event = CGEvent::new_keyboard_event(None, mc, false);
                CGEvent::set_flags(event.as_deref(), CGEventFlags::empty());
                CGEvent::post(CGEventTapLocation::HIDEventTap, event.as_deref());
                pause();
            }
        }
        Some("move") => mouse(CGEventType::MouseMoved, num(1), num(2)),
        Some("scroll") => {
            // Four steps, as a wheel sends them; the point travels in the events.
            let step = (num(3) / 4.0).round() as i32;
            for _ in 0..4 {
                let event =
                    CGEvent::new_scroll_wheel_event2(None, CGScrollEventUnit::Pixel, 1, step, 0, 0);
                CGEvent::set_location(event.as_deref(), CGPoint::new(num(1), num(2)));
                CGEvent::post(CGEventTapLocation::HIDEventTap, event.as_deref());
                pause();
            }
        }
        Some("click") => {
            mouse(CGEventType::MouseMoved, num(1), num(2));
            mouse(CGEventType::LeftMouseDown, num(1), num(2));
            mouse(CGEventType::LeftMouseUp, num(1), num(2));
        }
        Some("drag") => {
            let (x1, y1, x2, y2) = (num(1), num(2), num(3), num(4));
            mouse(CGEventType::MouseMoved, x1, y1);
            mouse(CGEventType::LeftMouseDown, x1, y1);
            for step in 1..=12 {
                let t = f64::from(step) / 12.0;
                mouse(
                    CGEventType::LeftMouseDragged,
                    x1 + (x2 - x1) * t,
                    y1 + (y2 - y1) * t,
                );
            }
            mouse(CGEventType::LeftMouseUp, x2, y2);
        }
        _ => panic!("usage: post_input key|move|click|drag ..."),
    }
}
