//! Strings of screen capture, in the same exhaustive-match style as `i18n.rs`.

use crate::i18n::Language;

impl Language {
    pub fn capture_hint_any(self) -> &'static str {
        match self {
            Language::English => {
                "Drag to select. Click a window to take it. Space: pick windows. Return: whole screen. Esc: cancel."
            }
            Language::Vietnamese => {
                "Kéo để chọn vùng. Bấm vào cửa sổ để chụp cửa sổ đó. Space: chọn cửa sổ. Enter: cả màn hình. Esc: hủy."
            }
        }
    }

    pub fn capture_hint_window(self) -> &'static str {
        match self {
            Language::English => {
                "Click the window to take. Space: drag an area instead. Esc: cancel."
            }
            Language::Vietnamese => "Bấm vào cửa sổ cần chụp. Space: kéo chọn vùng. Esc: hủy.",
        }
    }

    pub fn capture_hint_text(self) -> &'static str {
        match self {
            Language::English => "Drag around the text to copy. Esc: cancel.",
            Language::Vietnamese => "Kéo quanh đoạn chữ cần sao chép. Esc: hủy.",
        }
    }

    pub fn capture_copy(self) -> &'static str {
        match self {
            Language::English => "Copy",
            Language::Vietnamese => "Sao chép",
        }
    }

    pub fn capture_save(self) -> &'static str {
        match self {
            Language::English => "Save...",
            Language::Vietnamese => "Lưu...",
        }
    }

    pub fn capture_edit(self) -> &'static str {
        match self {
            Language::English => "Edit",
            Language::Vietnamese => "Sửa",
        }
    }

    pub fn capture_pin(self) -> &'static str {
        match self {
            Language::English => "Pin",
            Language::Vietnamese => "Ghim",
        }
    }

    pub fn capture_close(self) -> &'static str {
        match self {
            Language::English => "Close",
            Language::Vietnamese => "Đóng",
        }
    }

    pub fn capture_copied(self) -> &'static str {
        match self {
            Language::English => "Picture copied",
            Language::Vietnamese => "Đã sao chép ảnh",
        }
    }

    pub fn capture_saved(self, name: &str) -> String {
        match self {
            Language::English => format!("Saved {name}"),
            Language::Vietnamese => format!("Đã lưu {name}"),
        }
    }

    pub fn capture_needs_permission(self) -> &'static str {
        match self {
            Language::English => {
                "Screen Recording access is needed: allow Figghost in System Settings, then try again."
            }
            Language::Vietnamese => {
                "Cần quyền Ghi màn hình: cho phép Figghost trong Cài đặt hệ thống rồi thử lại."
            }
        }
    }

    pub fn capture_failed(self, detail: &str) -> String {
        match self {
            Language::English => format!("Capture failed: {detail}"),
            Language::Vietnamese => format!("Không chụp được: {detail}"),
        }
    }

    pub fn capture_write_failed(self, detail: &str) -> String {
        match self {
            Language::English => format!("Cannot save the capture: {detail}"),
            Language::Vietnamese => format!("Không lưu được ảnh chụp: {detail}"),
        }
    }

    pub fn capture_text_copied(self, lines: usize) -> String {
        match self {
            Language::English if lines == 1 => "Copied 1 line of text".to_owned(),
            Language::English => format!("Copied {lines} lines of text"),
            Language::Vietnamese => format!("Đã sao chép {lines} dòng chữ"),
        }
    }

    pub fn capture_no_text(self) -> &'static str {
        match self {
            Language::English => "No text found in the selected area",
            Language::Vietnamese => "Không tìm thấy chữ trong vùng đã chọn",
        }
    }

    pub fn capture_countdown(self, seconds: u32) -> String {
        match self {
            Language::English => format!("Capturing in {seconds}"),
            Language::Vietnamese => format!("Chụp sau {seconds}"),
        }
    }

    pub fn menu_capture_area(self) -> &'static str {
        match self {
            Language::English => "Capture Area",
            Language::Vietnamese => "Chụp vùng",
        }
    }

    pub fn menu_capture_window(self) -> &'static str {
        match self {
            Language::English => "Capture Window",
            Language::Vietnamese => "Chụp cửa sổ",
        }
    }

    pub fn menu_capture_screen(self) -> &'static str {
        match self {
            Language::English => "Capture Screen",
            Language::Vietnamese => "Chụp toàn màn hình",
        }
    }

    pub fn menu_capture_timer(self, seconds: u32) -> String {
        match self {
            Language::English => format!("Capture Screen in {seconds} Seconds"),
            Language::Vietnamese => format!("Chụp toàn màn hình sau {seconds} giây"),
        }
    }

    pub fn menu_capture_text(self) -> &'static str {
        match self {
            Language::English => "Copy Text from Screen",
            Language::Vietnamese => "Sao chép chữ trên màn hình",
        }
    }

    pub fn menu_capture_folder(self) -> &'static str {
        match self {
            Language::English => "Open Captures Folder",
            Language::Vietnamese => "Mở thư mục ảnh chụp",
        }
    }
}
