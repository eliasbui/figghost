//! Draws the Pull requests tab of the open notch into PNGs with sample requests, in both languages, without showing
//! anything (run it with FIGGHOST_NOTCH_OFFSCREEN=1), and checks that a press on a row says which row it was.
//! `FIGGHOST_NOTCH_OFFSCREEN=1 cargo run -p macos-sys --example render_prs -- /tmp/prs`

use macos_sys::{
    MainThreadMarker, app,
    notch::{Notch, NotchGeometry},
    prs_ui::{PAGE_SIZE, PrRow, PrsCallbacks, PrsDisplay, PrsView},
    ui::AppCallbacks,
};
use objc2::{msg_send, rc::Retained};
use objc2_app_kit::{NSScrollView, NSView};
use std::{cell::RefCell, env, fs, path::PathBuf, process, rc::Rc};

fn rows(vi: bool, count: usize) -> Vec<PrRow> {
    let ago = |en: &'static str, vi_text: &'static str| if vi { vi_text } else { en };
    let all = [
        (
            "Refreshable tokens: build-gated debug overrides for testing token expiry",
            format!(
                "cli/cli #14752  ·  babakks  ·  {}",
                ago("2 hours ago", "2 giờ trước")
            ),
        ),
        (
            "Fix the progress bar of the release job so that it stops at a hundred percent",
            format!(
                "{}  ·  gitlab-org/cli #4155  ·  kkahula  ·  {}",
                if vi { "Bản nháp" } else { "Draft" },
                ago("5 hours ago", "5 giờ trước")
            ),
        ),
        (
            "Add a link to the migration guide in the error message",
            format!(
                "figghost/app #212  ·  thai  ·  {}",
                ago("yesterday", "hôm qua")
            ),
        ),
        (
            "Bump the Rust toolchain to 1.99",
            format!(
                "figghost/app #209  ·  bot-renovate  ·  {}",
                ago("2 days ago", "2 ngày trước")
            ),
        ),
        (
            "Make the notch close when a pinned tile is pressed",
            format!(
                "figghost/app #207  ·  sam  ·  {}",
                ago("3 days ago", "3 ngày trước")
            ),
        ),
        (
            "Document the new configuration keys",
            format!(
                "figghost/app #204  ·  lee  ·  {}",
                ago("4 days ago", "4 ngày trước")
            ),
        ),
    ];
    all.iter()
        .take(count)
        .map(|(title, detail)| PrRow {
            title: (*title).into(),
            detail: detail.clone(),
        })
        .collect()
}

fn scene(vi: bool, count: usize) -> PrsDisplay {
    PrsDisplay {
        heading: if vi {
            format!("{count} yêu cầu đang chờ bạn duyệt")
        } else {
            format!("{count} requests wait for your review")
        },
        status: if vi {
            "Cập nhật 3 phút trước".into()
        } else {
            "Updated 3 minutes ago".into()
        },
        rows: rows(vi, count),
        refresh_label: if vi {
            "Xem lại".into()
        } else {
            "Look again".into()
        },
        busy: false,
        note: None,
    }
}

fn main() {
    let mtm = MainThreadMarker::new().expect("main thread");
    let dir = PathBuf::from(env::args().nth(1).unwrap_or_else(|| "/tmp/prs".into()));
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
        let (open, refresh) = (Rc::clone(&log), Rc::clone(&log));
        let view = PrsView::new(
            mtm,
            notch.page_area(PAGE_SIZE.width, PAGE_SIZE.height),
            PrsCallbacks {
                on_open: Box::new(move |i| open.borrow_mut().push(format!("open {i}"))),
                on_refresh: Box::new(move || refresh.borrow_mut().push("refresh".into())),
            },
        );
        let index = notch.page_count();
        let page = view.page("Pull requests".into(), || {}, || {});
        let root: Retained<NSView> = page.root.clone();
        notch.add_page(page);
        let (shot, shown) = (Rc::clone(&notch), Rc::clone(&view));
        app::after(0.4, move || {
            // A press on a row, and on the button that looks again, tells the app which it was.
            shown.show(&scene(false, 6));
            let mut presses = 0;
            for sub in root.subviews().iter() {
                if let Some(scroll) = sub.downcast_ref::<NSScrollView>() {
                    let list = scroll.documentView().expect("the list");
                    let rows: Vec<Retained<NSView>> =
                        list.subviews().iter().filter(|v| !v.isHidden()).collect();
                    assert_eq!(rows.len(), 6, "a row for each request");
                    // SAFETY: accessibilityPerformPress takes no arguments and returns whether it pressed.
                    let pressed: bool = unsafe { msg_send![&*rows[3], accessibilityPerformPress] };
                    assert!(pressed);
                    presses += 1;
                } else if !sub.isHidden() {
                    // SAFETY: as above.
                    let pressed: bool = unsafe { msg_send![&*sub, accessibilityPerformPress] };
                    assert!(pressed, "the refresh button took the press");
                    presses += 1;
                }
            }
            assert_eq!(presses, 2);
            assert_eq!(*log.borrow(), ["refresh", "open 3"]);
            println!(
                "ok      a press on a row and on the refresh button tells the app which it was"
            );

            let scenes = vec![
                ("list_en", scene(false, 6)),
                ("list_vi", scene(true, 4)),
                ("one_en", PrsDisplay { heading: "1 request waits for your review".into(), rows: rows(false, 1), ..scene(false, 1) }),
                (
                    "nothing_en",
                    PrsDisplay {
                        status: "Updated 1 minute ago".into(),
                        refresh_label: "Look again".into(),
                        note: Some(("checkmark.circle", "Nothing waits for your review".into(), "A new request shows up here, and a dot on the tab's icon says so.".into())),
                        ..PrsDisplay::default()
                    },
                ),
                (
                    "signed_out_en",
                    PrsDisplay {
                        refresh_label: "Look again".into(),
                        note: Some(("arrow.triangle.pull", "Could not read your review requests".into(), "Is gh or glab signed in? Run gh auth login or glab auth login in a terminal.".into())),
                        ..PrsDisplay::default()
                    },
                ),
            ];
            for (name, display) in scenes {
                shown.show(&display);
                let png = shot.snapshot_png(index).expect("the page");
                let path = dir.join(format!("{name}.png"));
                fs::write(&path, png).expect("write png");
                println!("wrote {}", path.display());
            }
            process::exit(0);
        });
        (notch, view)
    });
}
