//! Draws the calendar page of the open notch into PNGs with sample content, in both languages, without
//! showing anything on screen: run it with FIGGHOST_NOTCH_OFFSCREEN=1.
//! `FIGGHOST_NOTCH_OFFSCREEN=1 cargo run -p macos-sys --example render_calendar -- /tmp/calendar`

use core_calendar::format::Lang;
use macos_sys::{
    MainThreadMarker, app,
    calendar_ui::{
        AlertData, CalendarCallbacks, CalendarDisplay, CalendarView, DayData, EventData, NoteData,
        ReminderData,
    },
    notch::{Notch, NotchGeometry},
    panel::Key,
    style::Tone,
    ui::AppCallbacks,
};
use objc2::{msg_send, rc::Retained, runtime::AnyObject};
use objc2_app_kit::NSView;
use objc2_foundation::NSString;
use std::{cell::RefCell, env, fs, path::PathBuf, process, rc::Rc};

struct Words {
    today: &'static str,
    tomorrow: &'static str,
    date_today: &'static str,
    date_tomorrow: &'static str,
    soon: &'static str,
    in5: &'static str,
    now: &'static str,
    in3h: &'static str,
    all_day: &'static str,
    reminders: &'static str,
    overdue: &'static str,
    done: &'static str,
    dismiss: &'static str,
    more: &'static str,
    join: &'static str,
    clock: [&'static str; 6],
}

fn words(lang: Lang) -> Words {
    match lang {
        Lang::En => Words {
            today: "Today",
            tomorrow: "Tomorrow",
            date_today: "Thu, Oct 15",
            date_tomorrow: "Fri, Oct 16",
            soon: "Starting soon",
            in5: "in 5 min",
            now: "now",
            in3h: "in 3 h",
            all_day: "All day",
            reminders: "Reminders",
            overdue: "Overdue 9:00 AM",
            done: "Complete",
            dismiss: "Dismiss",
            more: "+{n} more",
            join: "Join",
            clock: [
                "9:30 AM", "10:00 AM", "10:30 AM", "11:15 AM", "2:00 PM", "3:30 PM",
            ],
        },
        Lang::Vi => Words {
            today: "Hôm nay",
            tomorrow: "Ngày mai",
            date_today: "Thứ Năm, 15 tháng 10",
            date_tomorrow: "Thứ Sáu, 16 tháng 10",
            soon: "Sắp bắt đầu",
            in5: "sau 5 phút",
            now: "đang diễn ra",
            in3h: "sau 3 giờ",
            all_day: "Cả ngày",
            reminders: "Lời nhắc",
            overdue: "Quá hạn 09:00",
            done: "Hoàn thành",
            dismiss: "Bỏ qua",
            more: "+{n} mục nữa",
            join: "Tham gia",
            clock: ["09:30", "10:00", "10:30", "11:15", "14:00", "15:30"],
        },
    }
}

#[allow(clippy::too_many_arguments)]
fn event(
    key: &str,
    color: u32,
    time: &str,
    until: &str,
    title: &str,
    detail: &str,
    tag: &str,
    tone: Option<Tone>,
    join: Option<&str>,
) -> EventData {
    EventData {
        key: key.into(),
        color,
        time: time.into(),
        until: until.into(),
        title: title.into(),
        detail: detail.into(),
        tag: tag.into(),
        tag_tone: tone,
        join: join.map(Into::into),
    }
}

fn events(lang: Lang) -> Vec<DayData> {
    let w = words(lang);
    let (zoom, meet) = (w.join.to_owned(), w.join.to_owned());
    vec![
        DayData {
            heading: w.today.into(),
            date: w.date_today.into(),
            events: vec![
                event(
                    "h",
                    0xff9f0a,
                    w.all_day,
                    "",
                    "Company holiday",
                    "Calendar: Holidays",
                    "",
                    None,
                    None,
                ),
                event(
                    "a",
                    0x3478f6,
                    w.clock[0],
                    w.clock[1],
                    "Design review",
                    "Room 4, Building B",
                    w.now,
                    Some(Tone::Green),
                    None,
                ),
                event(
                    "b",
                    0x30d158,
                    w.clock[2],
                    w.clock[3],
                    "Weekly sync with the platform team",
                    "https://acme.zoom.us/j/81234567890",
                    w.in5,
                    Some(Tone::Blue),
                    Some(&zoom),
                ),
                event(
                    "c",
                    0xbf5af2,
                    w.clock[4],
                    w.clock[5],
                    "Nguyễn Văn Đức: phỏng vấn ứng viên",
                    "Work",
                    w.in3h,
                    None,
                    Some(&meet),
                ),
            ],
        },
        DayData {
            heading: w.tomorrow.into(),
            date: w.date_tomorrow.into(),
            events: vec![event(
                "d", 0xff453a, w.clock[1], w.clock[2], "Dentist", "Home", "", None, None,
            )],
        },
    ]
}

fn reminders(lang: Lang) -> Vec<ReminderData> {
    let w = words(lang);
    vec![
        ReminderData {
            key: "r1".into(),
            color: 0xff9f0a,
            title: "Send the invoice to the accountant".into(),
            when: w.overdue.into(),
            overdue: true,
            done: w.done.into(),
        },
        ReminderData {
            key: "r2".into(),
            color: 0x64d2ff,
            title: "Buy a birthday present".into(),
            when: w.clock[4].into(),
            overdue: false,
            done: w.done.into(),
        },
    ]
}

fn alert(lang: Lang, with_link: bool) -> AlertData {
    let w = words(lang);
    AlertData {
        key: "b".into(),
        heading: w.soon.into(),
        when: w.in5.into(),
        title: "Weekly sync with the platform team".into(),
        detail: format!("{} - {}  -  Room 4, Building B", w.clock[2], w.clock[3]),
        join: with_link.then(|| format!("{} Zoom", w.join)),
        dismiss: w.dismiss.into(),
    }
}

fn scenes(lang: Lang) -> Vec<(&'static str, CalendarDisplay)> {
    let w = words(lang);
    let base = |alerts: Vec<AlertData>, days: Vec<DayData>, reminders: Vec<ReminderData>| {
        CalendarDisplay {
            alerts,
            more_alerts: String::new(),
            days,
            reminders_heading: w.reminders.into(),
            reminders,
            more: w.more.into(),
            note: None,
        }
    };
    let (en, hint_off, hint_none) = match lang {
        Lang::En => (
            [
                (
                    "Calendar access is off",
                    "Allow Figghost to read your calendar, then today's events show up here.",
                ),
                (
                    "Nothing planned",
                    "No more events or reminders for today and tomorrow.",
                ),
            ],
            "Allow access",
            "",
        ),
        Lang::Vi => (
            [
                (
                    "Chưa cho phép truy cập lịch",
                    "Cho phép Figghost đọc lịch, các sự kiện hôm nay sẽ hiện ở đây.",
                ),
                (
                    "Không có gì trong lịch",
                    "Không còn sự kiện hay lời nhắc nào hôm nay và ngày mai.",
                ),
            ],
            "Cho phép truy cập",
            "",
        ),
    };
    let _ = hint_none;
    let mut denied = base(vec![], vec![], vec![]);
    denied.note = Some(NoteData {
        symbol: "calendar.badge.exclamationmark",
        title: en[0].0.into(),
        hint: en[0].1.into(),
        action: Some(hint_off.into()),
    });
    let mut empty = base(vec![], vec![], vec![]);
    empty.note = Some(NoteData {
        symbol: "calendar",
        title: en[1].0.into(),
        hint: en[1].1.into(),
        action: None,
    });
    let mut crowded = base(vec![], events(lang), reminders(lang));
    for day in &mut crowded.days {
        let more = day.events.clone();
        day.events.extend(more);
    }
    let mut stacked = base(vec![alert(lang, true)], events(lang), reminders(lang));
    stacked.more_alerts = match lang {
        Lang::En => "1 more alert".into(),
        Lang::Vi => "Còn 1 lời báo nữa".into(),
    };
    vec![
        ("events", base(vec![], events(lang), vec![])),
        (
            "events_reminders",
            base(vec![], events(lang)[..1].to_vec(), reminders(lang)),
        ),
        ("alert_join", stacked),
        (
            "alert_no_link",
            base(vec![alert(lang, false)], events(lang)[..1].to_vec(), vec![]),
        ),
        ("crowded", crowded),
        ("denied", denied),
        ("empty", empty),
    ]
}

/// Every button under `view`, with its accessibility label.
fn buttons(view: &NSView, out: &mut Vec<(String, Retained<NSView>)>) {
    for sub in view.subviews().iter() {
        // SAFETY: the label is a plain property of any view; buttons answer with their title.
        let label: Option<Retained<NSString>> = unsafe { msg_send![&*sub, accessibilityLabel] };
        if sub.class().name().to_str() == Ok("FigTouchButton") {
            out.push((
                label.map(|l| l.to_string()).unwrap_or_default(),
                sub.clone(),
            ));
        }
        buttons(&sub, out);
    }
}

/// Presses the button with this label the way assistive technology does.
fn press(view: &CalendarView, label: &str, nth: usize) -> bool {
    let mut all = Vec::new();
    buttons(&view.root, &mut all);
    let Some((_, button)) = all.iter().filter(|(l, _)| l == label).nth(nth) else {
        return false;
    };
    // SAFETY: the buttons implement accessibilityPerformPress.
    let done: bool = unsafe { msg_send![&**button as &AnyObject, accessibilityPerformPress] };
    done
}

/// What the callbacks received when buttons and keys are used; stops the process when something differs.
fn behave(view: &CalendarView, log: &RefCell<Vec<String>>, mtm: MainThreadMarker) {
    let take = || std::mem::take(&mut *log.borrow_mut());
    let w = words(Lang::En);
    let scene = |name: &str| {
        scenes(Lang::En)
            .into_iter()
            .find(|(n, _)| *n == name)
            .expect("scene")
            .1
    };
    let expect = |what: &str, got: Vec<String>, want: &[&str]| {
        let ok = got == want;
        println!("{} {what}: {got:?}", if ok { "ok  " } else { "FAIL" });
        if !ok {
            process::exit(1);
        }
    };

    view.show(mtm, &scene("alert_join"));
    assert!(press(view, "Join Zoom", 0));
    assert!(press(view, "Dismiss", 0));
    assert!(press(view, w.join, 0));
    assert!(press(view, w.join, 1));
    assert!(!press(view, "No such button", 0));
    expect(
        "alert and row buttons",
        take(),
        &["join:b", "dismiss:b", "join:b", "join:c"],
    );
    assert!(
        view.handle_key(Key::Enter) && view.handle_key(Key::Delete) && !view.handle_key(Key::Up)
    );
    expect("keys with a link", take(), &["join:b", "dismiss:b"]);

    view.show(mtm, &scene("alert_no_link"));
    assert!(
        !view.handle_key(Key::Enter),
        "Return does nothing without a link"
    );
    assert!(view.handle_key(Key::Delete));
    expect("keys without a link", take(), &["dismiss:b"]);

    view.show(mtm, &scene("events_reminders"));
    assert!(
        !view.handle_key(Key::Enter) && !view.handle_key(Key::Delete),
        "no alert, no key"
    );
    assert!(press(view, w.done, 1));
    expect("reminder button", take(), &["complete:r2"]);

    view.show(mtm, &scene("denied"));
    assert!(press(view, "Allow access", 0));
    expect("the note's button", take(), &["action"]);
}

fn main() {
    let mtm = MainThreadMarker::new().expect("main thread");
    let dir = PathBuf::from(env::args().nth(1).unwrap_or_else(|| "/tmp/calendar".into()));
    fs::create_dir_all(&dir).expect("output directory");
    app::run_accessory(mtm, move || {
        let geometry = NotchGeometry::detect(mtm).expect("a screen");
        let noop = AppCallbacks {
            on_query: Box::new(|_, _| {}),
            on_activate: Box::new(|_, _, _| {}),
            on_pin: Box::new(|_, _| {}),
            on_delete: Box::new(|_, _| {}),
        };
        let notch = Rc::new(Notch::new(mtm, geometry, noop, |view| {
            view.set_rows(Vec::new(), "")
        }));
        let log: Rc<RefCell<Vec<String>>> = Rc::default();
        let (join, dismiss, complete, action) =
            (log.clone(), log.clone(), log.clone(), log.clone());
        let view = CalendarView::new(
            mtm,
            notch.page_frame(),
            "Return: join    Opt+Delete: dismiss",
            CalendarCallbacks {
                on_join: Box::new(move |k| join.borrow_mut().push(format!("join:{k}"))),
                on_dismiss: Box::new(move |k| dismiss.borrow_mut().push(format!("dismiss:{k}"))),
                on_complete: Box::new(move |k| complete.borrow_mut().push(format!("complete:{k}"))),
                on_action: Box::new(move || action.borrow_mut().push("action".into())),
            },
        );
        notch.add_page(view.page("Calendar", || {}, || {}));

        let (shot, shown) = (Rc::clone(&notch), Rc::clone(&view));
        app::after(0.4, move || {
            behave(&shown, &log, mtm);
            for (lang, suffix) in [(Lang::En, "en"), (Lang::Vi, "vi")] {
                for (name, display) in scenes(lang) {
                    shown.show(mtm, &display);
                    let png = shot.snapshot_png(1).expect("page 1");
                    let path = dir.join(format!("{name}_{suffix}.png"));
                    fs::write(&path, png).expect("write png");
                    println!("wrote {}", path.display());
                }
            }
            process::exit(0);
        });
        (notch, view)
    });
}
