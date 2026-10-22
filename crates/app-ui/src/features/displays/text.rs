//! The words of the Displays tab.

use crate::i18n::Language;

impl Language {
    pub fn displays_tab(self) -> &'static str {
        match self {
            Language::English => "Displays",
            Language::Vietnamese => "Màn hình",
        }
    }

    pub fn displays_brightness(self) -> &'static str {
        match self {
            Language::English => "Brightness",
            Language::Vietnamese => "Độ sáng",
        }
    }

    pub fn displays_contrast(self) -> &'static str {
        match self {
            Language::English => "Contrast",
            Language::Vietnamese => "Độ tương phản",
        }
    }

    /// A display that gives no name of its own.
    pub fn displays_unnamed(self, number: usize) -> String {
        match self {
            Language::English => format!("Display {number}"),
            Language::Vietnamese => format!("Màn hình {number}"),
        }
    }

    pub fn displays_looking(self) -> &'static str {
        match self {
            Language::English => "Looking for displays",
            Language::Vietnamese => "Đang tìm màn hình",
        }
    }

    pub fn displays_none_title(self) -> &'static str {
        match self {
            Language::English => "No display to set",
            Language::Vietnamese => "Không có màn hình để chỉnh",
        }
    }

    pub fn displays_none_hint(self) -> &'static str {
        match self {
            Language::English => {
                "No external display answers over DDC/CI. Turn DDC/CI on in the display's own menu, or connect it directly instead of through a hub or adapter."
            }
            Language::Vietnamese => {
                "Không màn hình rời nào trả lời qua DDC/CI. Hãy bật DDC/CI trong menu của màn hình, hoặc cắm thẳng vào máy thay vì qua hub hay bộ chuyển."
            }
        }
    }

    /// Shown in place of the displays while the feature is off.
    pub fn displays_off_title(self) -> &'static str {
        match self {
            Language::English => "The Displays tab is off",
            Language::Vietnamese => "Tab Màn hình đang tắt",
        }
    }

    pub fn displays_off_hint(self) -> &'static str {
        match self {
            Language::English => "Turn it on in Settings, under Notch tabs, Displays.",
            Language::Vietnamese => "Bật trong Cài đặt, mục Tab của notch, Màn hình.",
        }
    }
}
