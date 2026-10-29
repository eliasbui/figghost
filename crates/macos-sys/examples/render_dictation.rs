//! Draws the content of the Dictation panel into PNGs in each of its stages, without the glass behind it (the window
//! server draws the glass, and only on a screen) and without showing anything.
//! `cargo run -p macos-sys --example render_dictation -- /tmp/dictation`

use macos_sys::{
    MainThreadMarker, app,
    dictation_ui::{DictationDisplay, DictationPanel, Stage},
};
use std::{env, fs, path::PathBuf, process};

fn main() {
    let mtm = MainThreadMarker::new().expect("main thread");
    let dir = PathBuf::from(
        env::args()
            .nth(1)
            .unwrap_or_else(|| "/tmp/dictation".into()),
    );
    fs::create_dir_all(&dir).expect("output directory");
    app::run_accessory(mtm, move || {
        let panel = DictationPanel::new(mtm, |_| false);
        let vi = |stage, symbol, text: &str| DictationDisplay {
            stage,
            symbol,
            text: text.into(),
            placeholder: "Đang nghe...".into(),
            language: "Tiếng Việt".into(),
            hint: "Return để chèn, Esc để hủy, mũi tên để đổi ngôn ngữ".into(),
        };
        let voice: Vec<f32> = (0..36)
            .map(|i| (0.5 + 0.45 * ((i as f32) * 0.7).sin()).max(0.0))
            .collect();
        let scenes = vec![
            ("vi-waiting", vi(Stage::Listening, "mic.fill", ""), vec![0.02; 36]),
            (
                "vi-words",
                vi(Stage::Listening, "mic.fill", "Xin chào, hôm nay mình muốn đặt lịch họp vào chiều thứ sáu"),
                voice.clone(),
            ),
            (
                "vi-long",
                vi(
                    Stage::Listening,
                    "mic.fill",
                    "Đây là một đoạn rất dài để xem bảng chỉ giữ phần cuối của câu, khi người nói không dừng lại mà cứ tiếp tục nói thêm nhiều ý nữa cho tới hết",
                ),
                voice.clone(),
            ),
            (
                "vi-finishing",
                vi(Stage::Finishing, "waveform", "Xin chào, hôm nay mình muốn đặt lịch họp"),
                vec![0.1; 36],
            ),
            (
                "en-words",
                DictationDisplay {
                    stage: Stage::Listening,
                    symbol: "mic.fill",
                    text: "Hello, I would like to book a meeting for Friday afternoon".into(),
                    placeholder: "Listening...".into(),
                    language: "English".into(),
                    hint: "Return to insert, Esc to cancel, arrows to change language".into(),
                },
                voice,
            ),
            (
                "vi-copied",
                DictationDisplay {
                    stage: Stage::Note,
                    symbol: "doc.on.clipboard",
                    text: "Không có ô nhập nào đang nhận bàn phím, nên chữ đã được chép vào clipboard. Nhấn Command+V để dán.".into(),
                    ..DictationDisplay::default()
                },
                Vec::new(),
            ),
            (
                "en-no-microphone",
                DictationDisplay {
                    stage: Stage::Note,
                    symbol: "mic.slash.fill",
                    text: "Figghost may not use the microphone. Allow it in System Settings, Privacy and Security, Microphone.".into(),
                    ..DictationDisplay::default()
                },
                Vec::new(),
            ),
        ];
        for (name, display, levels) in scenes {
            let png = panel.png(&display, &levels).expect("the panel draws");
            let path = dir.join(format!("{name}.png"));
            fs::write(&path, png).expect("write png");
            println!("wrote {}", path.display());
        }
        process::exit(0);
    });
}
