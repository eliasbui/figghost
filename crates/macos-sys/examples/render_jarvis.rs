//! Draws the content of the Jarvis panel into PNGs in each of its states, without the glass behind it (the window
//! server draws the glass, and only on a screen) and without showing anything.
//! `cargo run -p macos-sys --example render_jarvis -- /tmp/jarvis`

use macos_sys::{
    MainThreadMarker, app,
    jarvis_ui::{JarvisDisplay, JarvisPanel, Line, Stage},
};
use std::{env, fs, path::PathBuf, process};

fn line(mine: bool, text: &str) -> Line {
    Line { mine, text: text.into() }
}

fn main() {
    let mtm = MainThreadMarker::new().expect("main thread");
    let dir = PathBuf::from(env::args().nth(1).unwrap_or_else(|| "/tmp/jarvis".into()));
    fs::create_dir_all(&dir).expect("output directory");
    app::run_accessory(mtm, move || {
        let panel = JarvisPanel::new(mtm, Some(include_bytes!("../../app-ui/assets/jarvis/jarvis.png")), |_| false);
        let base = |stage, symbol, status: &str, lines: Vec<Line>| JarvisDisplay {
            stage,
            symbol,
            name: "Jarvis".into(),
            beta: "BETA".into(),
            status: status.into(),
            lines,
            placeholder: "Hỏi bất cứ điều gì. Esc dừng câu trả lời, rồi đóng.".into(),
            greeting: "Tôi giúp được gì cho bạn? Gõ câu hỏi ở dưới rồi nhấn Return.".into(),
        };
        let talk = vec![
            line(true, "Hôm nay thứ mấy và làm sao đổi múi giờ trong lịch?"),
            line(false, "Hôm nay là thứ sáu. Trong Lịch, mở Cài đặt, chọn Nâng cao rồi bật Hỗ trợ múi giờ; sau đó mỗi sự kiện có một ô múi giờ riêng."),
            line(true, "Cảm ơn"),
        ];
        let mut long = talk.clone();
        for i in 0..6 {
            long.push(line(i % 2 == 0, "Một dòng nữa để xem những câu cũ nhất nhường chỗ cho câu mới nhất khi cuộc trò chuyện dài ra."));
        }
        let scenes = vec![
            ("idle-empty", base(Stage::Idle, "sparkles", "Sẵn sàng", Vec::new())),
            ("idle-talk", base(Stage::Idle, "sparkles", "Sẵn sàng", talk.clone())),
            ("listening", {
                let mut d = base(Stage::Listening, "mic.fill", "Đang nghe...", talk.clone());
                d.placeholder = "Đang nghe...".into();
                d
            }),
            ("speaking", base(Stage::Speaking, "sparkles", "Đang trả lời...", talk.clone())),
            ("thinking", base(Stage::Thinking, "ellipsis", "Đang nghĩ...", {
                let mut t = talk.clone();
                t.push(line(false, "..."));
                t
            })),
            ("long", base(Stage::Speaking, "sparkles", "Đang trả lời...", long)),
            ("error", base(Stage::Error, "exclamationmark.triangle.fill", "", {
                let mut t = talk;
                t.push(line(false, "Chưa có khóa cho Anthropic. Thêm khóa trong Cài đặt, ở trang Jarvis."));
                t
            })),
        ];
        // A voice that has been loud for a moment, for the ring of the listening character.
        for level in [0.2, 0.8, 0.9, 0.6] {
            panel.level(level);
        }
        for (name, display) in scenes {
            let png = panel.png(&display).expect("the panel draws");
            let path = dir.join(format!("{name}.png"));
            fs::write(&path, png).expect("write png");
            println!("wrote {}", path.display());
        }
        process::exit(0);
    });
}
