//! Everything this feature says outside the settings page, in both languages. Each string is a method with
//! an exhaustive `match`, so a new language lists what is still missing.

use crate::i18n::Language;
use macos_sys::annotate::Strings as EditorStrings;

impl Language {
    pub fn capture_menu_region(self) -> &'static str {
        match self {
            Language::English => "Capture region",
            Language::Vietnamese => "Chụp vùng chọn",
        }
    }

    pub fn capture_menu_window(self) -> &'static str {
        match self {
            Language::English => "Capture window",
            Language::Vietnamese => "Chụp cửa sổ",
        }
    }

    pub fn capture_menu_screen(self) -> &'static str {
        match self {
            Language::English => "Capture screen",
            Language::Vietnamese => "Chụp toàn màn hình",
        }
    }

    pub fn capture_menu_text(self) -> &'static str {
        match self {
            Language::English => "Capture text",
            Language::Vietnamese => "Chụp văn bản",
        }
    }

    pub fn capture_hint_region(self) -> &'static str {
        match self {
            Language::English => {
                "Drag to select. Hold Space to move. Return confirms, Escape cancels."
            }
            Language::Vietnamese => {
                "Kéo để chọn vùng. Giữ Space để di chuyển. Return xác nhận, Escape hủy."
            }
        }
    }

    pub fn capture_hint_window(self) -> &'static str {
        match self {
            Language::English => "Click a window to capture it. Escape cancels.",
            Language::Vietnamese => "Bấm vào một cửa sổ để chụp. Escape để hủy.",
        }
    }

    /// The file name without its extension; `{date}` and `{time}` are filled in.
    pub fn capture_file_template(self) -> &'static str {
        match self {
            Language::English => "Figghost {date} at {time}",
            Language::Vietnamese => "Figghost {date} lúc {time}",
        }
    }

    pub fn capture_folder_prompt(self) -> &'static str {
        match self {
            Language::English => "Choose the folder where screenshots are saved",
            Language::Vietnamese => "Chọn thư mục để lưu ảnh chụp màn hình",
        }
    }

    pub fn capture_folder_button(self) -> &'static str {
        match self {
            Language::English => "Choose",
            Language::Vietnamese => "Chọn",
        }
    }

    pub fn capture_copied_and_saved(self) -> &'static str {
        match self {
            Language::English => "Screenshot copied and saved",
            Language::Vietnamese => "Đã sao chép và lưu ảnh chụp",
        }
    }

    pub fn capture_copied(self) -> &'static str {
        match self {
            Language::English => "Screenshot copied",
            Language::Vietnamese => "Đã sao chép ảnh chụp",
        }
    }

    pub fn capture_saved(self) -> &'static str {
        match self {
            Language::English => "Screenshot saved",
            Language::Vietnamese => "Đã lưu ảnh chụp",
        }
    }

    pub fn capture_kept_nothing(self) -> &'static str {
        match self {
            Language::English => "Screenshot taken",
            Language::Vietnamese => "Đã chụp",
        }
    }

    pub fn capture_kept_nothing_detail(self) -> &'static str {
        match self {
            Language::English => {
                "Copying and saving are both off in Settings, so nothing was kept."
            }
            Language::Vietnamese => {
                "Sao chép và lưu đều đang tắt trong Cài đặt nên ảnh không được giữ lại."
            }
        }
    }

    pub fn capture_copied_not_saved(self) -> &'static str {
        match self {
            Language::English => "Screenshot copied, not saved",
            Language::Vietnamese => "Đã sao chép, chưa lưu được",
        }
    }

    pub fn capture_save_failed(self) -> &'static str {
        match self {
            Language::English => "Could not save the screenshot",
            Language::Vietnamese => "Không lưu được ảnh chụp",
        }
    }

    pub fn capture_text_copied(self) -> &'static str {
        match self {
            Language::English => "Text copied",
            Language::Vietnamese => "Đã sao chép văn bản",
        }
    }

    pub fn capture_text_none(self) -> &'static str {
        match self {
            Language::English => "No text found",
            Language::Vietnamese => "Không tìm thấy văn bản",
        }
    }

    pub fn capture_text_none_detail(self) -> &'static str {
        match self {
            Language::English => "Nothing in that area could be read as text.",
            Language::Vietnamese => "Không đọc được chữ nào trong vùng đã chọn.",
        }
    }

    pub fn capture_text_failed(self) -> &'static str {
        match self {
            Language::English => "Text could not be read",
            Language::Vietnamese => "Không đọc được văn bản",
        }
    }

    pub fn capture_permission_needed(self) -> &'static str {
        match self {
            Language::English => "Screen Recording is off",
            Language::Vietnamese => "Quyền Ghi màn hình đang tắt",
        }
    }

    pub fn capture_permission_detail(self) -> &'static str {
        match self {
            Language::English => {
                "Allow Figghost in System Settings, Privacy and Security, Screen Recording."
            }
            Language::Vietnamese => {
                "Hãy cho phép Figghost trong Cài đặt hệ thống, Quyền riêng tư và bảo mật, Ghi màn hình."
            }
        }
    }

    pub fn capture_nothing_to_repeat(self) -> &'static str {
        match self {
            Language::English => "No region to repeat",
            Language::Vietnamese => "Chưa có vùng nào để chụp lại",
        }
    }

    pub fn capture_nothing_to_repeat_detail(self) -> &'static str {
        match self {
            Language::English => "Capture a region first, on this display.",
            Language::Vietnamese => "Hãy chụp một vùng trước, trên màn hình này.",
        }
    }

    pub fn capture_failed(self) -> &'static str {
        match self {
            Language::English => "Capture failed",
            Language::Vietnamese => "Chụp không thành công",
        }
    }

    pub fn capture_unavailable(self) -> &'static str {
        match self {
            Language::English => "Screen capture needs macOS 14 or later",
            Language::Vietnamese => "Chụp màn hình cần macOS 14 trở lên",
        }
    }

    /// The labels of the editor window.
    pub fn capture_editor_strings(self) -> EditorStrings {
        let s = |text: &str| text.to_owned();
        match self {
            Language::English => EditorStrings {
                window_title: s("Edit screenshot"),
                select: s("Select"),
                arrow: s("Arrow"),
                line: s("Line"),
                rectangle: s("Rectangle"),
                ellipse: s("Ellipse"),
                text: s("Text"),
                pen: s("Pen"),
                highlighter: s("Highlighter"),
                blur: s("Blur"),
                crop: s("Crop"),
                undo: s("Undo"),
                redo: s("Redo"),
                delete: s("Delete"),
                copy: s("Copy"),
                save: s("Save"),
                done: s("Done"),
                colour: s("Colour"),
                thickness: s("Thickness"),
                fill: s("Fill"),
                text_placeholder: s("Type here"),
                saved: s("Saved"),
                copied: s("Copied"),
                save_failed: s("Could not save"),
                copy_failed: s("Could not copy"),
            },
            Language::Vietnamese => EditorStrings {
                window_title: s("Chỉnh sửa ảnh chụp"),
                select: s("Chọn"),
                arrow: s("Mũi tên"),
                line: s("Đường thẳng"),
                rectangle: s("Hình chữ nhật"),
                ellipse: s("Hình elip"),
                text: s("Chữ"),
                pen: s("Bút"),
                highlighter: s("Bút nhấn"),
                blur: s("Làm mờ"),
                crop: s("Cắt"),
                undo: s("Hoàn tác"),
                redo: s("Làm lại"),
                delete: s("Xóa"),
                copy: s("Sao chép"),
                save: s("Lưu"),
                done: s("Xong"),
                colour: s("Màu"),
                thickness: s("Độ dày"),
                fill: s("Tô màu"),
                text_placeholder: s("Nhập chữ ở đây"),
                saved: s("Đã lưu"),
                copied: s("Đã sao chép"),
                save_failed: s("Không lưu được"),
                copy_failed: s("Không sao chép được"),
            },
        }
    }

    /// Language codes in order of preference for reading text from a capture.
    pub fn capture_text_languages(self) -> Vec<String> {
        let codes: &[&str] = match self {
            Language::English => &["en-US"],
            Language::Vietnamese => &["vi-VT", "en-US"],
        };
        codes.iter().map(|c| (*c).to_owned()).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_string_exists_in_both_languages_and_none_is_empty() {
        for language in Language::ALL {
            let all = [
                language.capture_menu_region(),
                language.capture_menu_window(),
                language.capture_menu_screen(),
                language.capture_menu_text(),
                language.capture_hint_region(),
                language.capture_hint_window(),
                language.capture_file_template(),
                language.capture_folder_prompt(),
                language.capture_folder_button(),
                language.capture_copied_and_saved(),
                language.capture_copied(),
                language.capture_saved(),
                language.capture_kept_nothing(),
                language.capture_kept_nothing_detail(),
                language.capture_copied_not_saved(),
                language.capture_save_failed(),
                language.capture_text_copied(),
                language.capture_text_none(),
                language.capture_text_none_detail(),
                language.capture_text_failed(),
                language.capture_permission_needed(),
                language.capture_permission_detail(),
                language.capture_nothing_to_repeat(),
                language.capture_nothing_to_repeat_detail(),
                language.capture_failed(),
                language.capture_unavailable(),
            ];
            for text in all {
                assert!(!text.trim().is_empty(), "{language:?} has an empty string");
            }
            let editor = language.capture_editor_strings();
            let labels = [
                &editor.window_title,
                &editor.select,
                &editor.arrow,
                &editor.line,
                &editor.rectangle,
                &editor.ellipse,
                &editor.text,
                &editor.pen,
                &editor.highlighter,
                &editor.blur,
                &editor.crop,
                &editor.undo,
                &editor.redo,
                &editor.delete,
                &editor.copy,
                &editor.save,
                &editor.done,
                &editor.colour,
                &editor.thickness,
                &editor.fill,
                &editor.text_placeholder,
                &editor.saved,
                &editor.copied,
                &editor.save_failed,
                &editor.copy_failed,
            ];
            assert!(labels.iter().all(|l| !l.trim().is_empty()));
            assert!(!language.capture_text_languages().is_empty());
        }
    }

    #[test]
    fn the_file_template_has_the_date_and_the_time() {
        for language in Language::ALL {
            let template = language.capture_file_template();
            assert!(template.contains("{date}") && template.contains("{time}"));
        }
    }

    #[test]
    fn the_two_languages_differ() {
        let (en, vi) = (Language::English, Language::Vietnamese);
        assert_ne!(en.capture_menu_region(), vi.capture_menu_region());
        assert_ne!(
            en.capture_permission_detail(),
            vi.capture_permission_detail()
        );
        assert_ne!(
            en.capture_editor_strings().undo,
            vi.capture_editor_strings().undo
        );
    }
}
