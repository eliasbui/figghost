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
            Language::English => "Choose a folder to pin to the Launcher",
            Language::Vietnamese => "Chọn một thư mục để ghim vào Mở nhanh",
        }
    }

    pub fn launcher_pick_file(self) -> &'static str {
        match self {
            Language::English => "Choose a file to pin to the Launcher",
            Language::Vietnamese => "Chọn một tệp để ghim vào Mở nhanh",
        }
    }

    /// The name of a group that was not given one.
    pub fn launcher_group(self, number: usize) -> String {
        match self {
            Language::English => format!("Group {number}"),
            Language::Vietnamese => format!("Nhóm {number}"),
        }
    }

    pub fn launcher_open(self) -> &'static str {
        match self {
            Language::English => "Open",
            Language::Vietnamese => "Mở",
        }
    }

    pub fn launcher_open_in_finder(self) -> &'static str {
        match self {
            Language::English => "Open in Finder",
            Language::Vietnamese => "Mở trong Finder",
        }
    }

    pub fn launcher_reveal(self) -> &'static str {
        match self {
            Language::English => "Show in Finder",
            Language::Vietnamese => "Hiện trong Finder",
        }
    }

    pub fn launcher_folder_empty(self) -> &'static str {
        match self {
            Language::English => "The folder is empty",
            Language::Vietnamese => "Thư mục trống",
        }
    }

    pub fn launcher_folder_gone(self) -> &'static str {
        match self {
            Language::English => "The folder is not there any more",
            Language::Vietnamese => "Thư mục không còn ở đó",
        }
    }

    pub fn launcher_folder_more(self) -> &'static str {
        match self {
            Language::English => "See the rest in Finder",
            Language::Vietnamese => "Xem phần còn lại trong Finder",
        }
    }
}
