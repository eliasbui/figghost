//! Checks that the popup leaves Return and the arrow keys to the input method while it is composing
//! (Telex and VNI keep marked text until a word is committed), and handles them again afterwards.
//! Runs entirely in this process: no window is shown, no key focus is taken, no input source is changed.
//! `cargo run -p macos-sys --example check_composition`

use macos_sys::{
    MainThreadMarker,
    ui::{AppCallbacks, Popup, Row},
};
use objc2::{msg_send, rc::Retained, runtime::AnyObject};
use objc2_app_kit::{
    NSApplication, NSApplicationActivationPolicy, NSEvent, NSEventModifierFlags, NSEventType,
    NSTextView,
};
use objc2_foundation::{NSNotFound, NSPoint, NSRange, NSString};
use std::{cell::Cell, process, rc::Rc};

fn key_down(popup: &Popup, code: u16, characters: &str) {
    let chars = NSString::from_str(characters);
    let event = NSEvent::keyEventWithType_location_modifierFlags_timestamp_windowNumber_context_characters_charactersIgnoringModifiers_isARepeat_keyCode(
        NSEventType::KeyDown,
        NSPoint::ZERO,
        NSEventModifierFlags::empty(),
        0.0,
        popup.panel.windowNumber(),
        None,
        &chars,
        &chars,
        false,
        code,
    )
    .expect("key event");
    popup.panel.sendEvent(&event);
}

fn field_editor(popup: &Popup) -> Retained<NSTextView> {
    let search: &AnyObject = &popup.ui.view.search;
    // SAFETY: the object is the search field whose editor was requested.
    let editor = unsafe { popup.panel.fieldEditor_forObject(false, Some(search)) }
        .expect("the search field is being edited");
    editor
        .downcast::<NSTextView>()
        .expect("field editor is a text view")
}

fn start_composing(popup: &Popup) -> Retained<NSTextView> {
    popup.ui.focus_search();
    let editor = field_editor(popup);
    let marked = NSString::from_str("tie");
    // SAFETY: the standard NSTextInputClient call an input method makes while composing.
    unsafe {
        let _: () = msg_send![
            &*editor,
            setMarkedText: &*marked,
            selectedRange: NSRange::new(3, 0),
            replacementRange: NSRange::new(NSNotFound as usize, 0)
        ];
    }
    editor
}

fn has_marked_text(editor: &NSTextView) -> bool {
    // SAFETY: NSTextView implements NSTextInputClient.
    unsafe { msg_send![editor, hasMarkedText] }
}

fn expect(ok: bool, what: &str) {
    if ok {
        println!("  ok: {what}");
    } else {
        println!("  FAIL: {what}");
        process::exit(1);
    }
}

fn main() {
    let mtm = MainThreadMarker::new().expect("main thread");
    NSApplication::sharedApplication(mtm)
        .setActivationPolicy(NSApplicationActivationPolicy::Accessory);

    let chosen = Rc::new(Cell::new(0u32));
    let on_activate = {
        let chosen = Rc::clone(&chosen);
        Box::new(move |_: &_, _, _| chosen.set(chosen.get() + 1))
    };
    let popup = Popup::new(
        mtm,
        AppCallbacks {
            on_query: Box::new(|_, _| {}),
            on_activate,
            on_pin: Box::new(|_, _| {}),
            on_delete: Box::new(|_, _| {}),
        },
    );
    let row = |id| Row {
        id,
        title: format!("item {id}"),
        subtitle: String::new(),
        pinned: false,
        pin_key: None,
        mono: false,
        thumb: None,
    };
    popup.ui.view.set_rows(vec![row(1), row(2)], "");

    println!("while the input method has marked text:");
    let editor = start_composing(&popup);
    expect(
        has_marked_text(&editor),
        "the field editor reports marked text",
    );
    key_down(&popup, 125, "\u{f701}");
    expect(
        popup.ui.view.list.selected_index() == 0,
        "Down arrow is left to the input method",
    );
    key_down(&popup, 36, "\r");
    expect(
        chosen.get() == 0,
        "Return commits the composition instead of choosing an item",
    );

    println!("after the composition is committed:");
    let editor = start_composing(&popup);
    // SAFETY: the standard NSTextInputClient call an input method makes when it commits.
    unsafe {
        let _: () = msg_send![&*editor, unmarkText];
    }
    expect(!has_marked_text(&editor), "no marked text remains");
    key_down(&popup, 125, "\u{f701}");
    expect(
        popup.ui.view.list.selected_index() == 1,
        "Down arrow moves the selection",
    );
    key_down(&popup, 36, "\r");
    expect(chosen.get() == 1, "Return chooses the highlighted item");

    println!("composition guard: pass");
}
