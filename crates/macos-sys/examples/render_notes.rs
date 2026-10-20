//! Draws the Notes tab of the open notch into PNGs with sample notes, in both languages, without showing anything,
//! and checks that typing reaches the app and that the app's own text does not: run it with
//! FIGGHOST_NOTCH_OFFSCREEN=1.
//! `FIGGHOST_NOTCH_OFFSCREEN=1 cargo run -p macos-sys --example render_notes -- /tmp/notes`

use macos_sys::{
    MainThreadMarker, app,
    notch::{Notch, NotchGeometry},
    notes_ui::{NoteRow, NotesCallbacks, NotesDisplay, NotesView, PAGE_SIZE},
    ui::AppCallbacks,
};
use objc2::{msg_send, rc::Retained, runtime::AnyObject};
use objc2_app_kit::{NSScrollView, NSTextView, NSView};
use objc2_foundation::NSString;
use std::{cell::RefCell, env, fs, path::PathBuf, process, rc::Rc};

struct Words {
    caption: &'static str,
    placeholder: &'static str,
    footer: &'static str,
    new_note: &'static str,
    delete: &'static str,
    off_title: &'static str,
    off_hint: &'static str,
    notes: [(&'static str, &'static str); 7],
    text: &'static str,
}

fn words(vi: bool) -> Words {
    if vi {
        Words {
            caption: "Ghi chú",
            placeholder: "Bắt đầu gõ...",
            footer: "176 ký tự",
            new_note: "Ghi chú mới",
            delete: "Xóa ghi chú",
            off_title: "Tab Ghi chú đang tắt",
            off_hint: "Bật trong Cài đặt, mục Tab của notch, Ghi chú.",
            notes: [
                (
                    "Kế hoạch chuyến đi Đà Lạt",
                    "vừa xong  đặt vé xe, thuê homestay",
                ),
                (
                    "Việc cần làm tuần này",
                    "12 phút trước  gửi báo giá, họp 3 giờ",
                ),
                (
                    "Ý tưởng cho ứng dụng",
                    "2 giờ trước  một con ma biết đọc pin",
                ),
                ("Mật khẩu wifi nhà bạn", "hôm qua"),
                ("Danh sách mua sắm", "2 ngày trước  sữa, trứng, cà phê"),
                ("Câu hỏi cho buổi họp", "5 ngày trước  ngân sách quý tới"),
                ("Bài hát hay", "1 tháng trước"),
            ],
            text: "Kế hoạch chuyến đi Đà Lạt\n\nThứ bảy: đặt vé xe giường nằm, thuê homestay gần hồ Xuân Hương.\nChủ nhật: cà phê sáng, chợ đêm, về lúc 5 giờ chiều.",
        }
    } else {
        Words {
            caption: "Notes",
            placeholder: "Start typing...",
            footer: "176 characters",
            new_note: "New note",
            delete: "Delete note",
            off_title: "The Notes tab is off",
            off_hint: "Turn it on in Settings, under Notch tabs, Notes.",
            notes: [
                (
                    "Trip plan for Lisbon",
                    "just now  book the train, find a hotel",
                ),
                ("This week", "12 minutes ago  send the quote, 3 pm meeting"),
                ("App ideas", "2 hours ago  a ghost that reads the battery"),
                ("Wifi password at Sam's", "yesterday"),
                ("Shopping list", "2 days ago  milk, eggs, coffee"),
                (
                    "Questions for the meeting",
                    "5 days ago  budget next quarter",
                ),
                ("Songs worth keeping", "1 month ago"),
            ],
            text: "Trip plan for Lisbon\n\nSaturday: book the sleeper train, find a hotel near the old town.\nSunday: coffee, the flea market, back by five.",
        }
    }
}

fn scene(vi: bool, selected: usize, many: bool) -> NotesDisplay {
    let w = words(vi);
    let count = if many { 7 } else { 3 };
    NotesDisplay {
        caption: w.caption.into(),
        rows: w.notes[..count]
            .iter()
            .enumerate()
            .map(|(i, (title, detail))| NoteRow {
                title: (*title).into(),
                detail: (*detail).into(),
                selected: i == selected,
            })
            .collect(),
        footer: w.footer.into(),
        placeholder: w.placeholder.into(),
        new_label: w.new_note.into(),
        delete_label: w.delete.into(),
        note: None,
    }
}

/// The text view inside the page, found the way a person would not: by walking the views.
fn find_editor(view: &NSView) -> Option<Retained<NSTextView>> {
    for sub in view.subviews().iter() {
        if let Some(scroll) = sub.downcast_ref::<NSScrollView>()
            && let Some(document) = scroll.documentView()
            && let Ok(text) = document.downcast::<NSTextView>()
        {
            return Some(text);
        }
    }
    None
}

fn main() {
    let mtm = MainThreadMarker::new().expect("main thread");
    let dir = PathBuf::from(env::args().nth(1).unwrap_or_else(|| "/tmp/notes".into()));
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
        let (new, select, delete, edit, step) = (
            Rc::clone(&log),
            Rc::clone(&log),
            Rc::clone(&log),
            Rc::clone(&log),
            Rc::clone(&log),
        );
        let view = NotesView::new(
            mtm,
            notch.page_area(PAGE_SIZE.width, PAGE_SIZE.height),
            NotesCallbacks {
                on_new: Box::new(move || new.borrow_mut().push("new".into())),
                on_select: Box::new(move |i| select.borrow_mut().push(format!("select {i}"))),
                on_delete: Box::new(move || delete.borrow_mut().push("delete".into())),
                on_edit: Box::new(move |text| edit.borrow_mut().push(format!("edit {text:?}"))),
                on_step: Box::new(move |d| step.borrow_mut().push(format!("step {d}"))),
            },
        );
        let index = notch.page_count();
        let page = view.page("Notes".into(), || {}, || {});
        let root: Retained<NSView> = page.root.clone();
        notch.add_page(page);
        let (shot, shown) = (Rc::clone(&notch), Rc::clone(&view));
        app::after(0.4, move || {
            let editor = find_editor(&root).expect("the text view is in the page");
            let check = |ok: bool, what: &str| {
                println!("{}  {what}", if ok { "ok    " } else { "FAILED" });
                if !ok {
                    process::exit(1);
                }
            };

            // The app's own text is not an edit; a person's typing is, and carries the whole text.
            let english = words(false);
            shown.show(&scene(false, 0, true));
            shown.set_text(english.text);
            check(
                log.borrow().is_empty(),
                "setting the text from the app calls nothing back",
            );
            check(
                shown.text() == english.text,
                "the editor holds what was set",
            );
            // SAFETY: insertText: takes a string and is what typing sends to a text view.
            let _: () =
                unsafe { msg_send![&*editor, insertText: &*NSString::from_str("!") as &AnyObject] };
            let typed = log.borrow().clone();
            check(
                typed.len() == 1 && typed[0].starts_with("edit \"") && typed[0].contains('!'),
                &format!("typing calls back once with the text: {typed:?}"),
            );
            log.borrow_mut().clear();
            shown.set_text(english.text);

            let mut scenes: Vec<(&str, NotesDisplay, &str, bool)> = vec![
                ("notes_en", scene(false, 0, true), english.text, false),
                (
                    "few_en",
                    scene(false, 1, false),
                    "This week\n\nsend the quote",
                    false,
                ),
                (
                    "empty_en",
                    NotesDisplay {
                        rows: Vec::new(),
                        ..scene(false, 0, false)
                    },
                    "",
                    false,
                ),
                ("notes_vi", scene(true, 2, true), words(true).text, true),
            ];
            for vi in [false, true] {
                let w = words(vi);
                scenes.push((
                    if vi { "off_vi" } else { "off_en" },
                    NotesDisplay {
                        note: Some((w.off_title.into(), w.off_hint.into())),
                        ..NotesDisplay::default()
                    },
                    "",
                    vi,
                ));
            }
            for (name, display, text, _) in scenes {
                shown.show(&display);
                shown.set_text(text);
                let png = shot.snapshot_png(index).expect("the page");
                let path = dir.join(format!("{name}.png"));
                fs::write(&path, png).expect("write png");
                println!("wrote {}", path.display());
            }
            check(
                log.borrow().is_empty(),
                "drawing the scenes called nothing back",
            );
            process::exit(0);
        });
        (notch, view)
    });
}
