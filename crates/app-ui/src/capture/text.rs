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

    pub fn capture_hint_scroll(self) -> &'static str {
        match self {
            Language::English => {
                "Drag over the content to capture; it will scroll down by itself. Esc: cancel."
            }
            Language::Vietnamese => {
                "Kéo chọn vùng nội dung cần chụp; nội dung sẽ tự cuộn xuống. Esc: hủy."
            }
        }
    }

    pub fn capture_scroll_done(self) -> &'static str {
        match self {
            Language::English => "Done",
            Language::Vietnamese => "Xong",
        }
    }

    pub fn capture_scroll_cancel(self) -> &'static str {
        match self {
            Language::English => "Cancel",
            Language::Vietnamese => "Hủy",
        }
    }

    pub fn capture_scroll_status(self, rows: usize) -> String {
        match self {
            Language::English => format!("{rows} px"),
            Language::Vietnamese => format!("{rows} px"),
        }
    }

    pub fn menu_capture_scroll(self) -> &'static str {
        match self {
            Language::English => "Scrolling Capture",
            Language::Vietnamese => "Chụp cuộn",
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

    pub fn capture_open(self) -> &'static str {
        match self {
            Language::English => "Open",
            Language::Vietnamese => "Mở",
        }
    }

    pub fn capture_file_copied(self) -> &'static str {
        match self {
            Language::English => "File copied",
            Language::Vietnamese => "Đã sao chép tệp",
        }
    }

    pub fn record_hint(self) -> &'static str {
        match self {
            Language::English => {
                "Drag the area to record. Click a window to record it. Space: pick windows. Return: whole screen. Esc: cancel."
            }
            Language::Vietnamese => {
                "Kéo chọn vùng cần quay. Bấm vào cửa sổ để quay cửa sổ đó. Space: chọn cửa sổ. Enter: cả màn hình. Esc: hủy."
            }
        }
    }

    pub fn record_stop(self) -> &'static str {
        match self {
            Language::English => "Stop",
            Language::Vietnamese => "Dừng",
        }
    }

    pub fn record_starting(self) -> &'static str {
        match self {
            Language::English => "Starting...",
            Language::Vietnamese => "Đang bắt đầu...",
        }
    }

    pub fn record_status(self, seconds: u64) -> String {
        let time = format!("{}:{:02}", seconds / 60, seconds % 60);
        match self {
            Language::English => format!("Recording {time}"),
            Language::Vietnamese => format!("Đang quay {time}"),
        }
    }

    pub fn record_saving(self) -> &'static str {
        match self {
            Language::English => "Saving...",
            Language::Vietnamese => "Đang lưu...",
        }
    }

    pub fn record_making_gif(self) -> &'static str {
        match self {
            Language::English => "Making the GIF...",
            Language::Vietnamese => "Đang tạo GIF...",
        }
    }

    pub fn record_failed(self, detail: &str) -> String {
        match self {
            Language::English => format!("Recording failed: {detail}"),
            Language::Vietnamese => format!("Không quay được: {detail}"),
        }
    }

    pub fn capture_needs_newer_macos(self) -> &'static str {
        match self {
            Language::English => "Screenshots need macOS 14 or later",
            Language::Vietnamese => "Chụp màn hình cần macOS 14 trở lên",
        }
    }

    pub fn record_needs_newer_macos(self) -> &'static str {
        match self {
            Language::English => "Screen recording needs macOS 15 or later",
            Language::Vietnamese => "Quay màn hình cần macOS 15 trở lên",
        }
    }

    pub fn record_no_camera(self) -> &'static str {
        match self {
            Language::English => "No camera found; recording without it",
            Language::Vietnamese => "Không tìm thấy camera; quay không kèm camera",
        }
    }

    pub fn record_camera_denied(self) -> &'static str {
        match self {
            Language::English => {
                "Camera access is off: allow Figghost in System Settings, Privacy & Security, Camera"
            }
            Language::Vietnamese => {
                "Chưa có quyền Camera: cho phép Figghost trong Cài đặt hệ thống, Quyền riêng tư và bảo mật, Camera"
            }
        }
    }

    pub fn record_keys_need_access(self) -> &'static str {
        match self {
            Language::English => "Showing keystrokes needs Accessibility access",
            Language::Vietnamese => "Hiện phím bấm cần quyền Trợ năng",
        }
    }

    pub fn menu_record_screen(self) -> &'static str {
        match self {
            Language::English => "Record Screen",
            Language::Vietnamese => "Quay màn hình",
        }
    }

    pub fn menu_record_gif(self) -> &'static str {
        match self {
            Language::English => "Record GIF",
            Language::Vietnamese => "Quay GIF",
        }
    }

    pub fn menu_record_stop(self) -> &'static str {
        match self {
            Language::English => "Stop Recording",
            Language::Vietnamese => "Dừng quay",
        }
    }

    pub fn menu_record_options(self) -> &'static str {
        match self {
            Language::English => "Recording Options",
            Language::Vietnamese => "Tùy chọn quay",
        }
    }

    pub fn menu_record_system_audio(self) -> &'static str {
        match self {
            Language::English => "Record Computer Audio",
            Language::Vietnamese => "Thu âm thanh máy tính",
        }
    }

    pub fn menu_record_microphone(self) -> &'static str {
        match self {
            Language::English => "Record Microphone",
            Language::Vietnamese => "Thu micrô",
        }
    }

    pub fn menu_record_camera(self) -> &'static str {
        match self {
            Language::English => "Show Camera",
            Language::Vietnamese => "Hiện camera",
        }
    }

    pub fn menu_record_clicks(self) -> &'static str {
        match self {
            Language::English => "Show Clicks",
            Language::Vietnamese => "Hiện cú nhấp chuột",
        }
    }

    pub fn menu_record_keys(self) -> &'static str {
        match self {
            Language::English => "Show Keystrokes",
            Language::Vietnamese => "Hiện phím tắt đã bấm",
        }
    }

    pub fn editor_title(self) -> &'static str {
        match self {
            Language::English => "Edit Capture",
            Language::Vietnamese => "Sửa ảnh chụp",
        }
    }

    pub fn editor_select(self) -> &'static str {
        match self {
            Language::English => "Select",
            Language::Vietnamese => "Chọn",
        }
    }

    pub fn editor_arrow(self) -> &'static str {
        match self {
            Language::English => "Arrow",
            Language::Vietnamese => "Mũi tên",
        }
    }

    pub fn editor_line(self) -> &'static str {
        match self {
            Language::English => "Line",
            Language::Vietnamese => "Đường",
        }
    }

    pub fn editor_frame(self) -> &'static str {
        match self {
            Language::English => "Frame",
            Language::Vietnamese => "Khung",
        }
    }

    pub fn editor_ellipse(self) -> &'static str {
        match self {
            Language::English => "Ellipse",
            Language::Vietnamese => "Elip",
        }
    }

    pub fn editor_highlight(self) -> &'static str {
        match self {
            Language::English => "Highlight",
            Language::Vietnamese => "Tô sáng",
        }
    }

    pub fn editor_text(self) -> &'static str {
        match self {
            Language::English => "Text",
            Language::Vietnamese => "Chữ",
        }
    }

    pub fn editor_counter(self) -> &'static str {
        match self {
            Language::English => "Number",
            Language::Vietnamese => "Số",
        }
    }

    pub fn editor_pixelate(self) -> &'static str {
        match self {
            Language::English => "Pixelate",
            Language::Vietnamese => "Che",
        }
    }

    pub fn editor_crop(self) -> &'static str {
        match self {
            Language::English => "Crop",
            Language::Vietnamese => "Cắt",
        }
    }

    pub fn editor_undo(self) -> &'static str {
        match self {
            Language::English => "Undo",
            Language::Vietnamese => "Hoàn tác",
        }
    }

    pub fn editor_redo(self) -> &'static str {
        match self {
            Language::English => "Redo",
            Language::Vietnamese => "Làm lại",
        }
    }

    pub fn editor_background(self) -> &'static str {
        match self {
            Language::English => "Background",
            Language::Vietnamese => "Nền",
        }
    }

    pub fn editor_done(self) -> &'static str {
        match self {
            Language::English => "Done",
            Language::Vietnamese => "Xong",
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
