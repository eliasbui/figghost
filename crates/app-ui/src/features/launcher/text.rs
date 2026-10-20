//! The words of the Launcher tab, in each language.

use crate::i18n::Language;

impl Language {
    pub fn launcher_tab(self) -> &'static str {
        match self {
            Language::English => "Launcher",
            Language::Vietnamese => "Mở nhanh",
        }
    }

    pub fn launcher_off_title(self) -> &'static str {
        match self {
            Language::English => "The Launcher tab is off",
            Language::Vietnamese => "Tab Mở nhanh đang tắt",
        }
    }

    pub fn launcher_off_hint(self) -> &'static str {
        match self {
            Language::English => "Turn it on in Settings, under Notch tabs, Launcher.",
            Language::Vietnamese => "Bật trong Cài đặt, mục Tab của notch, Mở nhanh.",
        }
    }

    pub fn launcher_empty_title(self) -> &'static str {
        match self {
            Language::English => "Nothing pinned yet",
            Language::Vietnamese => "Chưa ghim gì cả",
        }
    }

    pub fn launcher_empty_hint(self) -> &'static str {
        match self {
            Language::English => {
                "Add apps, folders and Shortcuts in Settings, under Notch tabs, Launcher."
            }
            Language::Vietnamese => {
                "Thêm ứng dụng, thư mục và Shortcuts trong Cài đặt, mục Tab của notch, Mở nhanh."
            }
        }
    }

    pub fn launcher_pick_folder(self) -> &'static str {
        match self {
            Language::English => "Choose a folder to pin to the notch",
            Language::Vietnamese => "Chọn một thư mục để ghim lên notch",
        }
    }
}
