//! Draws the Notifications tab of the open notch into PNGs (a list with the icons of apps that come with macOS, no
//! access, nothing yet, off) and presses a row and the button beside the heading to check that the app is told which.
//! Then draws a closed pill with music playing and its date and time, the same pill when a notification comes, which
//! takes the music's place and the date and time's, and the pill again once the notification has gone. Nothing shows
//! on screen; run it with FIGGHOST_NOTCH_OFFSCREEN=1.
//! `FIGGHOST_NOTCH_OFFSCREEN=1 cargo run -p macos-sys --example render_notifications -- /tmp/notifications`

use macos_sys::{
    MainThreadMarker, app,
    notch::{Activity, Detail, Ear, Notch, NotchGeometry, Style, URGENT},
    notifications_ui::{
        NoticeRow, NotificationsCallbacks, NotificationsDisplay, NotificationsView, PAGE_SIZE,
    },
    style::Tone,
    ui::AppCallbacks,
    workspace,
};
use objc2_foundation::{NSPoint, NSRect, NSSize};
use std::{
    cell::{Cell, RefCell},
    env, fs,
    path::PathBuf,
    process,
    rc::Rc,
    sync::Arc,
};

fn row(title: &str, detail: &str, app: &str) -> NoticeRow {
    NoticeRow {
        title: title.into(),
        detail: detail.into(),
        app: Some(PathBuf::from(app)),
    }
}

fn list(vi: bool) -> NotificationsDisplay {
    let rows = if vi {
        vec![
            row(
                "Lan",
                "Tin nhắn, vừa xong: Đi ăn trưa không?",
                "/System/Applications/Messages.app",
            ),
            row(
                "Họp nhóm lúc 15:00",
                "Lịch, 4 phút trước: Phòng 2, mang theo laptop",
                "/System/Applications/Calendar.app",
            ),
            row(
                "Báo cáo tháng 10",
                "Mail, 1 giờ trước: Anh xem giúp em bản cuối nhé",
                "/System/Applications/Mail.app",
            ),
            row(
                "Ghi chú",
                "Ghi chú, hôm qua: Mua sữa",
                "/System/Applications/Notes.app",
            ),
        ]
    } else {
        vec![
            row(
                "Lan",
                "Messages, just now: Lunch?",
                "/System/Applications/Messages.app",
            ),
            row(
                "Team meeting at 3 PM",
                "Calendar, 4 min ago: Room 2, bring a laptop",
                "/System/Applications/Calendar.app",
            ),
            row(
                "October report",
                "Mail, 1 h ago: Could you look at the last draft?",
                "/System/Applications/Mail.app",
            ),
        ]
    };
    NotificationsDisplay {
        heading: if vi { "Thông báo" } else { "Notifications" }.into(),
        status: if vi {
            format!("{} thông báo", rows.len())
        } else {
            format!("{} notifications", rows.len())
        },
        rows,
        button: (
            "trash",
            if vi {
                "Xóa danh sách"
            } else {
                "Clear the list"
            }
            .into(),
        ),
        note: None,
    }
}

fn note(
    symbol: &'static str,
    title: &str,
    hint: &str,
    button: (&'static str, &str),
) -> NotificationsDisplay {
    NotificationsDisplay {
        heading: "Thông báo".into(),
        button: (button.0, button.1.into()),
        note: Some((symbol, title.into(), hint.into())),
        ..NotificationsDisplay::default()
    }
}

fn artwork(bundle: &str, id: u64) -> Ear {
    workspace::app_icon_png(bundle, 64.0).map_or(Ear::Empty, |png| Ear::Artwork {
        id,
        bytes: Arc::new(png),
    })
}

fn main() {
    let dir = PathBuf::from(
        env::args()
            .nth(1)
            .unwrap_or_else(|| "/tmp/notifications".into()),
    );
    fs::create_dir_all(&dir).expect("output folder");
    let mtm = MainThreadMarker::new().expect("main thread");
    app::run_accessory(mtm, move || {
        // A pill, as on a screen without a camera cutout, far outside every screen.
        let geometry = NotchGeometry {
            screen: NSRect::new(NSPoint::new(-30_000.0, 0.0), NSSize::new(1_920.0, 1_080.0)),
            notch: NSSize::new(185.0, 32.0),
            hardware: false,
        };
        let noop = AppCallbacks {
            on_query: Box::new(|_, _| {}),
            on_activate: Box::new(|_, _, _| {}),
            on_pin: Box::new(|_, _| {}),
            on_delete: Box::new(|_, _| {}),
        };
        let notch = Rc::new(Notch::new(mtm, geometry, noop, |view| {
            view.set_rows(Vec::new(), "")
        }));
        notch.configure(&Style::default());
        let log: Rc<RefCell<Vec<String>>> = Rc::default();
        let (opened, pressed) = (Rc::clone(&log), Rc::clone(&log));
        let view = NotificationsView::new(
            mtm,
            notch.page_area(PAGE_SIZE.width, PAGE_SIZE.height),
            NotificationsCallbacks {
                on_open: Box::new(move |i| opened.borrow_mut().push(format!("open {i}"))),
                on_button: Box::new(move || pressed.borrow_mut().push("button".into())),
            },
        );
        let index = notch.page_count();
        notch.add_page(view.page(
            "notifications",
            "bell.badge.fill",
            "Notifications".into(),
            || {},
            || {},
        ));
        let dir = Rc::new(dir);
        let write = {
            let dir = Rc::clone(&dir);
            move |name: &str, png: Option<Vec<u8>>| {
                let path = dir.join(format!("{name}.png"));
                fs::write(&path, png.expect("a picture")).expect("write png");
                println!("wrote {}", path.display());
            }
        };

        // The tab: a row and the button say which they are; a note has no rows to press.
        view.show(&list(false));
        assert!(view.press(Some(1)) && view.press(None));
        assert!(!view.press(Some(7)), "no such row");
        assert_eq!(*log.borrow(), ["open 1", "button"]);
        println!("ok      a row and the button tell the app which they are");
        for (name, display) in [
            ("tab_en", list(false)),
            ("tab_vi", list(true)),
            (
                "locked_vi",
                note(
                    "lock.fill",
                    "Figghost chưa đọc được thông báo",
                    "Bật Figghost trong Cài đặt hệ thống, Quyền riêng tư & Bảo mật, Truy cập toàn bộ ổ đĩa, rồi thoát và mở lại app. Nút ở góc trên bên phải mở đúng danh sách đó.",
                    ("lock.open.fill", "Mở Cài đặt hệ thống"),
                ),
            ),
            (
                "empty_vi",
                note(
                    "bell",
                    "Chưa có thông báo nào",
                    "Thông báo mới sẽ hiện ở đây ngay khi tới.",
                    ("", ""),
                ),
            ),
            (
                "off_en",
                note(
                    "bell.slash",
                    "The Notifications tab is off",
                    "Turn it on in Settings, under Notch & Island, Notifications.",
                    ("", ""),
                ),
            ),
        ] {
            view.show(&display);
            write(name, notch.snapshot_png(index));
        }
        assert!(!view.press(Some(0)), "a note has no rows");

        // The closed pill: music with the date and time, then a notification over both, then the music again.
        let music = Activity {
            left: artwork("com.apple.Music", 1),
            right: Ear::Symbol {
                name: "waveform",
                tone: Tone::Purple,
                pulse: false,
            },
            detail: None,
            page: None,
            priority: 20,
        };
        let notification = Activity {
            left: artwork("com.apple.MobileSMS", 2),
            right: Ear::Empty,
            detail: Some(Detail {
                title: "Lan".into(),
                subtitle: "Đi ăn trưa không?".into(),
            }),
            page: None,
            priority: URGENT,
        };
        let steps: Vec<(&str, &str, Option<Activity>)> = vec![
            ("closed_music", "music", Some(music)),
            ("closed_notification", "notifications", Some(notification)),
            ("closed_after", "notifications", None),
        ];
        let step = Cell::new(0_usize);
        let timer = app::every(0.7, move || {
            let n = step.get();
            step.set(n + 1);
            // Two ticks each: the first changes what is live, the second takes the picture once the shape has settled.
            let Some((name, slot, activity)) = steps.get(n / 2) else {
                process::exit(0);
            };
            if n % 2 == 0 {
                notch.set_activity(slot, activity.clone());
            } else {
                write(name, notch.snapshot_closed_png());
            }
        });
        std::mem::forget(timer);
    });
}
