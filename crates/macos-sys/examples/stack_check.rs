//! Checks the list surface behind boards, hidden rows and the copy stack pill, in process and without showing
//! anything: key presses are synthetic NSEvents delivered to a panel placed outside every screen, and the
//! board menu is built and its items are performed without opening it.
//! `cargo run -p macos-sys --example stack_check`

use macos_sys::{
    MainThreadMarker,
    button::TouchButton,
    notch::EXPANDED_SIZE,
    panel::FigPanel,
    permissions,
    stack::{
        input::{ModifierWatch, PasteTap},
        pill::Pill,
    },
    ui::{Activate, BoardChip, Callbacks, ClipUi, Extras, ListStyle, Mark, Row, handle_key},
};
use objc2::Message;
use objc2_app_kit::{
    NSApplication, NSApplicationActivationPolicy, NSEvent, NSEventModifierFlags, NSEventType,
};
use objc2_foundation::{NSPoint, NSRect, NSString};
use std::{cell::RefCell, collections::HashMap, process::exit, rc::Rc};

type Log = Rc<RefCell<Vec<String>>>;

fn row(id: i64, title: &str, pinned: bool) -> Row {
    Row {
        id,
        title: title.into(),
        subtitle: String::new(),
        pinned,
        pin_key: pinned.then_some('b'),
        mono: false,
        thumb: None,
    }
}

fn key_event(
    panel: &FigPanel,
    code: u16,
    flags: NSEventModifierFlags,
) -> objc2::rc::Retained<NSEvent> {
    NSEvent::keyEventWithType_location_modifierFlags_timestamp_windowNumber_context_characters_charactersIgnoringModifiers_isARepeat_keyCode(
        NSEventType::KeyDown,
        NSPoint::ZERO,
        flags,
        1.0,
        panel.windowNumber(),
        None,
        &NSString::from_str(""),
        &NSString::from_str(""),
        false,
        code,
    )
    .expect("a synthetic key event")
}

fn main() {
    let mtm = MainThreadMarker::new().expect("main thread");
    NSApplication::sharedApplication(mtm)
        .setActivationPolicy(NSApplicationActivationPolicy::Accessory);
    let mut failures: Vec<String> = Vec::new();
    let mut check = |ok: bool, what: &str| {
        if !ok {
            failures.push(what.to_owned());
        }
    };

    let log: Log = Rc::default();
    let record = |tag: &'static str, log: &Log| {
        let log = Rc::clone(log);
        move |id: i64| log.borrow_mut().push(format!("{tag} {id}"))
    };
    let (on_activate, on_pin, on_delete) = (
        record("activate", &log),
        record("pin", &log),
        record("delete", &log),
    );
    let callbacks = Callbacks {
        on_query: Box::new(|_, _| {}),
        on_activate: Box::new(move |_, id, mode| {
            on_activate(id);
            let _ = mode == Activate::Normal;
        }),
        on_pin: Box::new(move |_, id| on_pin(id)),
        on_delete: Box::new(move |_, id| on_delete(id)),
        on_close: Box::new(|_| {}),
    };
    let size = EXPANDED_SIZE;
    let ui = ClipUi::with_style(mtm, size, 0.0, callbacks, ListStyle::Cards);
    let callbacks = Rc::clone(&ui.callbacks);

    // A panel outside every screen carries the surface, so the real key path (decode, then handler) runs.
    let panel = FigPanel::new(mtm, NSRect::new(NSPoint::new(-30_000.0, 0.0), size));
    panel.setContentView(Some(&ui.view.root));
    {
        let (view, callbacks) = (ui.view.clone(), Rc::clone(&callbacks));
        panel.set_key_handler(move |key| handle_key(&view, &callbacks, key));
    }

    // Chips: the filter chips alone, then with boards.
    let buttons = |ui: &ClipUi| -> Vec<NSRect> {
        ui.view
            .root
            .subviews()
            .iter()
            .filter(|v| v.downcast_ref::<TouchButton>().is_some())
            .map(|v| v.frame())
            .collect()
    };
    let scroll_height = |ui: &ClipUi| {
        ui.view
            .list
            .enclosingScrollView()
            .map_or(0.0, |s| s.frame().size.height)
    };
    let before = buttons(&ui);
    let list_before = scroll_height(&ui);
    check(before.len() == 7, "seven filter chips");
    let boards = vec![
        BoardChip {
            id: 1,
            name: "Work".into(),
            filter: "@work".into(),
        },
        BoardChip {
            id: 2,
            name: "Snippets".into(),
            filter: "@snippets".into(),
        },
        BoardChip {
            id: 3,
            name: "A rather long board name".into(),
            filter: "@a_rather_long_board_name".into(),
        },
    ];
    ui.view.set_boards(boards.clone());
    let after = buttons(&ui);
    check(after.len() == 10, "three board chips are added");
    let filter_row_y = before[0].origin.y;
    check(
        after[7..].iter().all(|f| f.origin.y < filter_row_y),
        "board chips sit below the filter chips",
    );
    check(
        after
            .iter()
            .all(|f| f.origin.x >= 0.0 && f.origin.x + f.size.width <= size.width),
        "every chip is inside the surface",
    );
    check(
        after.windows(2).all(|w| {
            let (a, b) = (&w[0], &w[1]);
            a.origin.y != b.origin.y || a.origin.x + a.size.width <= b.origin.x
        }),
        "chips on one row do not overlap",
    );
    check(
        scroll_height(&ui) < list_before,
        "the list gives room to the board row",
    );
    let lowest = after.iter().map(|f| f.origin.y).fold(f64::MAX, f64::min);
    check(lowest >= 24.0 + 6.0, "the board row stays above the footer");
    // The same boards again change nothing; no boards takes the row away.
    ui.view.set_boards(boards.clone());
    check(
        buttons(&ui).len() == 10,
        "the same boards are not added twice",
    );
    ui.view.set_boards(Vec::new());
    check(
        buttons(&ui).len() == 7,
        "removing the boards removes their chips",
    );
    check(
        (scroll_height(&ui) - list_before).abs() < 0.5,
        "the list takes its room back",
    );
    ui.view.set_boards(boards);

    // Rows: a filed item, a hidden secret, a plain one.
    let rows = vec![
        row(1, "filed", true),
        row(2, "", false),
        row(3, "plain", false),
    ];
    let marks = HashMap::from([
        (
            1,
            Mark {
                board: Some(1),
                ..Mark::default()
            },
        ),
        (
            2,
            Mark {
                sensitive: true,
                hidden: true,
                board: None,
            },
        ),
    ]);
    ui.view.set_rows_marked(rows, marks, "");
    check(
        ui.view.list.mark_of(2).hidden && ui.view.list.mark_of(2).sensitive,
        "marks are kept by row id",
    );
    check(
        ui.view.list.mark_of(3) == Mark::default(),
        "an unmarked row has the default mark",
    );

    // Option+B files the selected item in the next board, then the next, then takes it out.
    let moves: Log = Rc::default();
    let reveals: Log = Rc::default();
    let (moved, revealed) = (Rc::clone(&moves), Rc::clone(&reveals));
    ui.view.set_extras(Extras {
        on_reveal: Box::new(move |view, id| {
            revealed.borrow_mut().push(format!("reveal {id}"));
            view.list.reveal(id, "shown text");
        }),
        on_board: Box::new(move |_, id, board| {
            moved.borrow_mut().push(format!("board {id} {board:?}"))
        }),
        menu_title: "Move to board".into(),
        no_board_title: "No board".into(),
    });
    let option_b = NSEventModifierFlags::Option;
    ui.view.list.select_id(3);
    panel.sendEvent(&key_event(&panel, 11, option_b));
    check(
        moves.borrow().last().map(String::as_str) == Some("board 3 Some(1)"),
        "Option+B files an unfiled item in the first board",
    );
    ui.view.list.select_id(1);
    panel.sendEvent(&key_event(&panel, 11, option_b));
    check(
        moves.borrow().last().map(String::as_str) == Some("board 1 Some(2)"),
        "Option+B moves a filed item to the next board",
    );
    let marks = HashMap::from([(
        1,
        Mark {
            board: Some(3),
            ..Mark::default()
        },
    )]);
    ui.view
        .set_rows_marked(vec![row(1, "filed", true)], marks, "");
    ui.view.list.select_id(1);
    panel.sendEvent(&key_event(&panel, 11, option_b));
    check(
        moves.borrow().last().map(String::as_str) == Some("board 1 None"),
        "Option+B on the last board takes the item out of it",
    );
    // Command+Option+B is not ours.
    let count = moves.borrow().len();
    panel.sendEvent(&key_event(
        &panel,
        11,
        option_b | NSEventModifierFlags::Command,
    ));
    check(
        moves.borrow().len() == count,
        "Command+Option+B does nothing here",
    );

    // The board menu offers every board and no board, ticks the current one, and performs the choice.
    let (menu, _targets) = ui.view.board_menu(1).expect("a menu while boards exist");
    let titles: Vec<String> = menu
        .itemArray()
        .iter()
        .map(|i| i.title().to_string())
        .collect();
    check(
        titles
            == [
                "Move to board",
                "No board",
                "Work",
                "Snippets",
                "A rather long board name",
            ],
        "the menu lists the header, no board and each board",
    );
    let ticked: Vec<bool> = menu.itemArray().iter().map(|i| i.state() != 0).collect();
    check(
        ticked == [false, false, false, false, true],
        "the current board is ticked",
    );
    moves.borrow_mut().clear();
    menu.performActionForItemAtIndex(3);
    check(
        moves.borrow().as_slice() == ["board 1 Some(2)"],
        "choosing a board in the menu files the item",
    );
    menu.performActionForItemAtIndex(1);
    check(
        moves.borrow().last().map(String::as_str) == Some("board 1 None"),
        "choosing no board takes it out",
    );
    ui.view.set_boards(Vec::new());
    check(
        ui.view.board_menu(1).is_none(),
        "there is no menu without boards",
    );

    // A hidden row: the first activation reveals it, only the second pastes.
    let rows = vec![row(2, "", false), row(3, "plain", false)];
    let marks = HashMap::from([(
        2,
        Mark {
            sensitive: true,
            hidden: true,
            board: None,
        },
    )]);
    ui.view.set_rows_marked(rows, marks, "");
    log.borrow_mut().clear();
    ui.view.list.select_id(2);
    panel.sendEvent(&key_event(&panel, 36, NSEventModifierFlags::empty()));
    check(
        reveals.borrow().as_slice() == ["reveal 2"],
        "Return on a hidden row reveals it",
    );
    check(log.borrow().is_empty(), "revealing pastes nothing");
    check(
        !ui.view.list.mark_of(2).hidden,
        "the row is shown after it is revealed",
    );
    panel.sendEvent(&key_event(&panel, 36, NSEventModifierFlags::empty()));
    check(
        log.borrow().as_slice() == ["activate 2"],
        "Return on a shown row pastes it",
    );
    check(
        reveals.borrow().len() == 1,
        "a shown row is not revealed again",
    );
    // The pin letter of a hidden pinned row is held back the same way.
    let rows = vec![row(5, "", true)];
    let marks = HashMap::from([(
        5,
        Mark {
            sensitive: true,
            hidden: true,
            board: None,
        },
    )]);
    ui.view.set_rows_marked(rows, marks, "");
    log.borrow_mut().clear();
    panel.sendEvent(&key_event(&panel, 11, NSEventModifierFlags::Command));
    check(
        log.borrow().is_empty(),
        "Command and the pin letter does not paste a hidden item",
    );
    check(
        reveals.borrow().last().map(String::as_str) == Some("reveal 5"),
        "it reveals it instead",
    );
    // Rebuilding the rows hides it again.
    let rows = vec![row(5, "", true)];
    let marks = HashMap::from([(
        5,
        Mark {
            sensitive: true,
            hidden: true,
            board: None,
        },
    )]);
    ui.view.set_rows_marked(rows, marks, "");
    check(ui.view.list.mark_of(5).hidden, "new rows start hidden");
    // Ordinary items are untouched by all this.
    let rows = vec![row(3, "plain", false)];
    ui.view.set_rows(rows, "");
    log.borrow_mut().clear();
    panel.sendEvent(&key_event(&panel, 36, NSEventModifierFlags::empty()));
    check(
        log.borrow().as_slice() == ["activate 3"],
        "a plain row pastes at once",
    );
    panel.sendEvent(&key_event(&panel, 35, option_b));
    check(
        log.borrow().last().map(String::as_str) == Some("pin 3"),
        "Option+P still pins",
    );
    panel.sendEvent(&key_event(&panel, 51, option_b));
    check(
        log.borrow().last().map(String::as_str) == Some("delete 3"),
        "Option+Delete still deletes",
    );

    // The pill never shows itself, wraps its text and stays outside the screen when asked to.
    let pill = Pill::new(mtm);
    pill.set_text("Copy Stack: 3 to paste");
    let (narrow, wide) = (pill.frame().size.width, {
        pill.set_text("Copy Stack: 12 mục chờ dán, còn nữa");
        pill.frame().size.width
    });
    check(wide > narrow, "the pill grows with its text");
    check(
        !pill.is_visible(),
        "a pill that was not shown is not visible",
    );
    check(
        pill.snapshot_png().is_some_and(|png| png.len() > 200),
        "the pill draws",
    );
    let _ = ui.view.root.retain();

    // The keyboard watchers: with Accessibility access the paste tap and the modifier monitor are made and
    // dropped on the real run loop; without it the tap is refused, which is how the app notices.
    let trusted = permissions::accessibility_trusted(false);
    let tap = PasteTap::start(|| {});
    check(
        tap.is_some() == trusted,
        "the paste tap exists exactly when Accessibility access does",
    );
    let watch = ModifierWatch::start(|_, _| {});
    check(watch.is_some(), "the modifier monitor can be installed");
    // Let the run loop turn a few times with both installed, then remove them.
    objc2_foundation::NSRunLoop::mainRunLoop()
        .runUntilDate(&objc2_foundation::NSDate::dateWithTimeIntervalSinceNow(0.3));
    drop(tap);
    drop(watch);
    println!("accessibility trusted: {trusted}");

    if failures.is_empty() {
        println!("stack_check ok");
    } else {
        for failure in &failures {
            println!("FAIL: {failure}");
        }
        println!("stack_check failed");
        exit(1);
    }
}
