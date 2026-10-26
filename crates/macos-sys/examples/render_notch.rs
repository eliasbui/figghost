//! Draws every page of the open notch into PNGs, with sample content, without showing anything on screen.
//! `cargo run -p macos-sys --example render_notch -- /tmp/notch [--hardware]`
//!
//! `--hardware` draws them as on a screen with a camera cutout (185 by 32, as `render_closed` uses), where the
//! header sits beside the cutout when it fits there.
//!
//! On the agents page it also checks that a click on a choice that spans a redraw (another session's event
//! arriving between the press and the release) still lands, and that the row of a waiting session is a button.

use macos_sys::{
    MainThreadMarker, NSPoint, NSRect, NSSize,
    agents_ui::{
        AgentsCallbacks, AgentsDisplay, AgentsView, ApprovalCardData, ChoiceData, QuestionData,
        SessionData, TokensData, UsageData,
    },
    app,
    button::TouchButton,
    focus_ui::{FocusCallbacks, FocusDisplay, FocusView},
    media_ui::{MediaCallbacks, MediaView},
    notch::{Notch, NotchGeometry},
    now_playing::{Sample, Track},
    panel::FigPanel,
    shelf_ui::{ShelfCallbacks, ShelfView},
    style::Tone,
    ui::{AppCallbacks, Row},
};
use objc2::{msg_send, rc::Retained};
use objc2_app_kit::{NSEvent, NSEventModifierFlags, NSEventType, NSTextField, NSView};
use objc2_foundation::NSString;
use std::{cell::RefCell, env, fs, path::PathBuf, process, rc::Rc, time::Instant};

/// The button under `view` that assistive tools call `name`.
fn find_button(view: &NSView, name: &str) -> Option<Retained<TouchButton>> {
    for sub in view.subviews().iter() {
        if let Ok(button) = sub.clone().downcast::<TouchButton>() {
            // SAFETY: accessibilityLabel takes no arguments and returns an optional string.
            let label: Option<Retained<NSString>> =
                unsafe { msg_send![&*button, accessibilityLabel] };
            if label.is_some_and(|l| l.to_string() == name) {
                return Some(button);
            }
        }
        if let Some(found) = find_button(&sub, name) {
            return Some(found);
        }
    }
    None
}

/// The first text field under `view`.
fn find_field(view: &NSView) -> Option<Retained<NSTextField>> {
    for sub in view.subviews().iter() {
        if let Ok(field) = sub.clone().downcast::<NSTextField>() {
            return Some(field);
        }
        if let Some(found) = find_field(&sub) {
            return Some(found);
        }
    }
    None
}

/// A press or a release of the left button in the middle of `button`, given to its window.
fn mouse(button: &TouchButton, kind: NSEventType) {
    let bounds = button.bounds();
    let middle = NSPoint::new(bounds.size.width / 2.0, bounds.size.height / 2.0);
    let at = button.convertPoint_toView(middle, None);
    let window = button.window().map_or(0, |w| w.windowNumber());
    let event = NSEvent::mouseEventWithType_location_modifierFlags_timestamp_windowNumber_context_eventNumber_clickCount_pressure(
        kind, at, NSEventModifierFlags::empty(), 0.0, window, None, 0, 1, 1.0,
    )
    .expect("mouse event");
    // Through the window, which picks the view that gets it, as for a real click.
    if let Some(window) = button.window() {
        window.sendEvent(&event);
    }
}

const ROOT: &str = env!("CARGO_MANIFEST_DIR");

fn sample_rows() -> Vec<Row> {
    let row = |id, title: &str, subtitle: &str, pinned, mono, thumb| Row {
        id,
        title: title.into(),
        subtitle: subtitle.into(),
        pinned,
        pin_key: pinned.then_some('b'),
        mono,
        thumb,
    };
    vec![
        row(
            1,
            "Meeting notes: ship the notch shelf on Friday",
            "Notes, just now",
            true,
            false,
            None,
        ),
        row(
            2,
            "https://example.com/docs/getting-started",
            "Safari, 2 minutes ago",
            false,
            false,
            None,
        ),
        row(
            3,
            "fn main() { let total = items.iter().sum::<i32>(); }",
            "Zed, 5 minutes ago",
            false,
            true,
            None,
        ),
        row(
            4,
            "Image 512 x 512",
            "Screenshot, 1 hour ago",
            false,
            false,
            Some(PathBuf::from(format!(
                "{ROOT}/../app-ui/assets/Figghost.icns"
            ))),
        ),
        row(
            5,
            "report-final.pdf and 2 other files",
            "Finder, yesterday",
            false,
            false,
            None,
        ),
        row(
            6,
            "Nguyễn Văn Đức - 0901 234 567",
            "Contacts, 2 days ago",
            false,
            false,
            None,
        ),
    ]
}

fn main() {
    let mtm = MainThreadMarker::new().expect("main thread");
    let dir = PathBuf::from(env::args().nth(1).unwrap_or_else(|| "/tmp/notch".into()));
    let hardware = env::args().any(|a| a == "--hardware");
    fs::create_dir_all(&dir).expect("output directory");
    app::run_accessory(mtm, move || {
        let mut geometry = NotchGeometry::detect(mtm).expect("a screen");
        if hardware {
            geometry.notch = NSSize::new(185.0, 32.0);
            geometry.hardware = true;
        }
        let noop = AppCallbacks {
            on_query: Box::new(|_, _| {}),
            on_activate: Box::new(|_, _, _| {}),
            on_pin: Box::new(|_, _| {}),
            on_delete: Box::new(|_, _| {}),
        };
        let rows = sample_rows();
        let notch = Rc::new(Notch::new(mtm, geometry, noop, move |view| {
            view.set_rows(rows.clone(), "Nothing copied yet");
        }));

        let shelf = ShelfView::new(
            mtm,
            notch.page_area(660.0, 150.0),
            ShelfCallbacks {
                on_drop: Box::new(|_| {}),
                on_remove: Box::new(|_| {}),
                on_open: Box::new(|_| {}),
                actions_for: Box::new(|_| Vec::new()),
                on_action: Box::new(|_, _| {}),
            },
        );
        shelf.set_paths(
            [
                "README.md",
                "Cargo.toml",
                "../app-ui/assets/Figghost.icns",
                "src",
                "examples",
            ]
            .iter()
            .map(|p| format!("{ROOT}/{p}"))
            .collect(),
        );
        notch.add_page(shelf.page(|| {}));

        let media = MediaView::new(
            mtm,
            notch.page_frame(),
            MediaCallbacks {
                on_previous: Box::new(|| {}),
                on_play_pause: Box::new(|| {}),
                on_next: Box::new(|| {}),
                on_seek: Box::new(|_| {}),
            },
        );
        media.show(Some(&Sample {
            track: Track {
                app: "com.apple.Music".into(),
                app_name: "Music".into(),
                title: "Blue in Green".into(),
                artist: "Miles Davis".into(),
                album: "Kind of Blue".into(),
                playing: true,
                rate: 1.0,
                position_ms: 0,
                duration_ms: 0,
                artwork: None,
            },
            at: Instant::now(),
        }));
        notch.add_page(media.page(|| {}, || {}));

        let focus = FocusView::new(
            mtm,
            notch.page_frame(),
            &[15, 25, 45, 60],
            FocusCallbacks {
                on_toggle: Box::new(|| {}),
                on_reset: Box::new(|| {}),
                on_preset: Box::new(|_| {}),
                on_adjust: Box::new(|_| {}),
            },
        );
        focus.show(&FocusDisplay {
            clock: "17:42",
            progress: 0.29,
            running: true,
            paused: false,
            finished: false,
            minutes: 25,
        });
        notch.add_page(focus.page(|| {}, || {}));

        let agents = AgentsView::new(
            mtm,
            notch.page_frame(),
            AgentsCallbacks {
                on_decide: Box::new(|_, _| {}),
                on_pick: Box::new(|_, _| {}),
                on_step: Box::new(|_, _, _| {}),
                on_refresh: Box::new(|| {}),
                on_session: Box::new(|_| {}),
            },
        );
        // The same page in a panel outside every screen, where clicks are delivered by its window.
        let pressed: Rc<RefCell<Vec<String>>> = Rc::default();
        let (picks, steps, chosen) = (
            Rc::clone(&pressed),
            Rc::clone(&pressed),
            Rc::clone(&pressed),
        );
        let clicked = AgentsView::new(
            mtm,
            notch.page_frame(),
            AgentsCallbacks {
                on_decide: Box::new(|_, _| {}),
                on_pick: Box::new(move |id, n| picks.borrow_mut().push(format!("pick {id} {n}"))),
                on_step: Box::new(move |id, step, typed| {
                    steps.borrow_mut().push(format!("{step:?} {id} {typed}"));
                }),
                on_refresh: Box::new(|| {}),
                on_session: Box::new(move |id| chosen.borrow_mut().push(format!("session {id}"))),
            },
        );
        let panel = FigPanel::new(
            mtm,
            NSRect::new(NSPoint::new(-30_000.0, 0.0), notch.page_frame().size),
        );
        panel.setContentView(Some(&clicked.root));
        panel.orderFrontRegardless();
        notch.add_page(agents.page(|| {}, || {}));

        let sessions = vec![
            SessionData {
                tone: Tone::Orange,
                agent: "Claude Code".into(),
                project: "shop".into(),
                state: "Waiting for you".into(),
                age: "just now".into(),
                approval: Some(1),
            },
            SessionData {
                tone: Tone::Blue,
                agent: "Codex".into(),
                project: "figghost".into(),
                state: "Working".into(),
                age: "2 min".into(),
                approval: None,
            },
            SessionData {
                tone: Tone::Green,
                agent: "Claude Code".into(),
                project: "api-gateway".into(),
                state: "Done".into(),
                age: "14 min".into(),
                approval: None,
            },
        ];
        let usage = vec![
            UsageData {
                label: "Claude 5 h".into(),
                fraction: Some(0.62),
                value: "62%".into(),
                reset: "resets in 2 h 10 min".into(),
            },
            UsageData {
                label: "Claude 7 d".into(),
                fraction: Some(0.31),
                value: "31%".into(),
                reset: "resets in 4 d".into(),
            },
            UsageData {
                label: "Codex 5 h".into(),
                fraction: Some(0.94),
                value: "94%".into(),
                reset: "resets in 38 min".into(),
            },
        ];
        let approval = ApprovalCardData {
            id: 1,
            heading: "Claude Code wants to use Bash".into(),
            command:
                "rm -rf node_modules && npm ci && npm run build -- --mode production --sourcemap"
                    .into(),
            detail: "shop  -  just now".into(),
            allow: "Allow".into(),
            deny: "Deny".into(),
            choices: Vec::new(),
            question: None,
        };
        let display = |approval: Option<ApprovalCardData>,
                       more: &str,
                       sessions: Vec<SessionData>,
                       usage: Vec<UsageData>| {
            AgentsDisplay {
                approval,
                more_waiting: more.into(),
                sessions_heading: "Sessions".into(),
                sessions,
                usage_heading: "Usage limits".into(),
                usage,
                refresh_label: Some("Refresh usage limits (Cmd+R)".into()),
                refreshing: false,
                tokens_heading: "Tokens and cost, at API prices".into(),
                tokens: vec![
                    TokensData {
                        label: "Claude".into(),
                        today: "1.24M tokens  $3.45".into(),
                        week: "Week $21.30".into(),
                    },
                    TokensData {
                        label: "Codex".into(),
                        today: "380K tokens".into(),
                        week: "Week 2.1M tokens".into(),
                    },
                ],
                empty_title: "No agent is running".into(),
                empty_hint: "Start Claude Code or Codex in a terminal. Turn on the hooks in Settings to see it here.".into(),
            }
        };

        let shot = Rc::clone(&notch);
        let view = Rc::clone(&agents);
        app::after(0.4, move || {
            let save = |name: &str, index: usize| match shot.snapshot_png(index) {
                Some(png) => {
                    let path = dir.join(format!("{name}.png"));
                    fs::write(&path, png).expect("write png");
                    println!("wrote {}", path.display());
                }
                None => println!("no page {index}"),
            };
            save("clipboard", 0);
            save("shelf", 1);
            save("media", 2);
            save("focus", 3);
            view.show(
                mtm,
                &display(
                    Some(approval.clone()),
                    "1 more waiting",
                    sessions.clone(),
                    usage.clone(),
                ),
            );
            save("agents_approval", 4);
            let question = ApprovalCardData {
                id: 7,
                heading: "Claude Code - Database 2/3".into(),
                command: "Which database should the shop use?".into(),
                detail: "shop  -  just now".into(),
                allow: "Next".into(),
                deny: "Answer in terminal".into(),
                choices: [
                    "Postgres",
                    "SQLite",
                    "A managed MySQL cluster with read replicas in two regions and nightly backups (Recommended)",
                ]
                .map(|label| ChoiceData {
                    label: label.into(),
                    selected: label == "SQLite",
                })
                .to_vec(),
                question: Some(QuestionData {
                    back: Some("Back".into()),
                    can_back: true,
                    cancel: Some("Cancel".into()),
                    other: Some("Type your own answer".into()),
                    typed: String::new(),
                    single: true,
                    last: false,
                }),
            };
            view.show(
                mtm,
                &display(Some(question.clone()), "", sessions.clone(), usage.clone()),
            );
            save("agents_question", 4);
            clicked.show(
                mtm,
                &display(Some(question.clone()), "", sessions.clone(), usage.clone()),
            );
            let _keep = &panel;
            let choice = find_button(&clicked.root, "Postgres").expect("a button per choice");
            mouse(&choice, NSEventType::LeftMouseDown);
            mouse(&choice, NSEventType::LeftMouseUp);
            assert_eq!(pressed.borrow().as_slice(), ["pick 7 0"], "a click lands");
            pressed.borrow_mut().clear();
            mouse(&choice, NSEventType::LeftMouseDown);
            // Another session reports in the middle of the click, and a minute has gone by on the card.
            let mut busier = sessions.clone();
            busier[1].state = "Done".into();
            busier[1].tone = Tone::Green;
            clicked.show(
                mtm,
                &display(
                    Some(ApprovalCardData {
                        detail: "shop  -  1 minute ago".into(),
                        ..question.clone()
                    }),
                    "",
                    busier,
                    usage.clone(),
                ),
            );
            mouse(&choice, NSEventType::LeftMouseUp);
            assert_eq!(
                pressed.borrow().as_slice(),
                ["pick 7 0"],
                "the click that spanned the redraw landed"
            );
            pressed.borrow_mut().clear();
            clicked.show(mtm, &display(None, "", sessions.clone(), usage.clone()));
            let row = find_button(&clicked.root, "Claude Code shop")
                .expect("the waiting session is a button");
            // SAFETY: an accessibility press takes no arguments.
            let _: bool = unsafe { msg_send![&*row, accessibilityPerformPress] };
            assert_eq!(pressed.borrow().as_slice(), ["session 1"]);
            assert!(
                find_button(&clicked.root, "Codex figghost").is_none(),
                "a session that waits for nothing is a plain line"
            );
            clicked.show(
                mtm,
                &display(Some(question.clone()), "", sessions.clone(), usage.clone()),
            );
            let field = find_field(&clicked.root).expect("a field for an answer of one's own");
            field.setStringValue(&NSString::from_str("  MariaDB  "));
            let next = find_button(&clicked.root, "Next").expect("a next button");
            mouse(&next, NSEventType::LeftMouseDown);
            mouse(&next, NSEventType::LeftMouseUp);
            let cancel = find_button(&clicked.root, "Cancel").expect("a cancel button");
            mouse(&cancel, NSEventType::LeftMouseDown);
            mouse(&cancel, NSEventType::LeftMouseUp);
            assert_eq!(
                pressed.borrow().as_slice(),
                ["session 1", "Next 7 MariaDB", "Cancel 7 MariaDB"],
                "next and cancel carry what was typed"
            );
            println!(
                "ok: a click across a redraw lands, a waiting session's row is pressable, next sends what was typed"
            );
            view.show(mtm, &display(None, "", sessions.clone(), usage.clone()));
            save("agents_overview", 4);
            let mut busy = display(None, "", sessions.clone(), usage.clone());
            busy.refreshing = true;
            busy.usage_heading = "Usage limits  -  Codex limits unavailable".into();
            view.show(mtm, &busy);
            save("agents_refreshing", 4);
            view.show(mtm, &display(None, "", sessions.clone(), Vec::new()));
            save("agents_no_usage", 4);
            let mut empty = display(None, "", Vec::new(), Vec::new());
            empty.refresh_label = None;
            view.show(mtm, &empty);
            save("agents_empty", 4);
            process::exit(0);
        });
        (notch, shelf, media, focus, agents)
    });
}
