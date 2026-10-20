//! The words of the Devices tab, in each language.

use crate::i18n::Language;

impl Language {
    pub fn devices_tab(self) -> &'static str {
        match self {
            Language::English => "Devices",
            Language::Vietnamese => "Thiết bị",
        }
    }

    pub fn devices_low(self) -> &'static str {
        match self {
            Language::English => "Low",
            Language::Vietnamese => "Pin yếu",
        }
    }

    /// Under the card of a device that says nothing about its charge.
    pub fn devices_silent(self) -> &'static str {
        match self {
            Language::English => "Does not report a charge",
            Language::Vietnamese => "Không báo mức pin",
        }
    }

    /// The charging case of a pair of earbuds.
    pub fn devices_case(self) -> &'static str {
        match self {
            Language::English => "Case",
            Language::Vietnamese => "Hộp",
        }
    }

    /// "Updated just now", with how many devices there are that the page has no room for.
    pub fn devices_footer(self, ago: &str, hidden: usize) -> String {
        let updated = match self {
            Language::English => format!("Updated {ago}"),
            Language::Vietnamese => format!("Cập nhật {ago}"),
        };
        match (self, hidden) {
            (_, 0) => updated,
            (Language::English, n) => format!("{updated}. {n} more not shown."),
            (Language::Vietnamese, n) => format!("{updated}. {n} thiết bị nữa không hiện."),
        }
    }

    pub fn devices_none_title(self) -> &'static str {
        match self {
            Language::English => "No Bluetooth device is connected",
            Language::Vietnamese => "Chưa có thiết bị Bluetooth nào được kết nối",
        }
    }

    pub fn devices_none_hint(self) -> &'static str {
        match self {
            Language::English => {
                "Connect a mouse, keyboard or headphones and its charge shows here."
            }
            Language::Vietnamese => {
                "Kết nối chuột, bàn phím hoặc tai nghe, mức pin của nó sẽ hiện ở đây."
            }
        }
    }

    pub fn devices_looking_title(self) -> &'static str {
        match self {
            Language::English => "Looking at the devices...",
            Language::Vietnamese => "Đang xem các thiết bị...",
        }
    }

    pub fn devices_problem_title(self) -> &'static str {
        match self {
            Language::English => "The Bluetooth devices could not be read",
            Language::Vietnamese => "Không đọc được thiết bị Bluetooth",
        }
    }

    pub fn devices_problem_hint(self) -> &'static str {
        match self {
            Language::English => "macOS did not answer in time. It is tried again in a moment.",
            Language::Vietnamese => "macOS chưa trả lời kịp. Sẽ thử lại sau một lát.",
        }
    }
}
