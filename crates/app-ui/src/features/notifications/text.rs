//! The words of the Notifications tab, in each language.

use crate::i18n::Language;

impl Language {
    pub fn notifications_tab(self) -> &'static str {
        match self {
            Language::English => "Notifications",
            Language::Vietnamese => "Thông báo",
        }
    }

    pub fn notifications_count(self, n: usize) -> String {
        match (self, n) {
            (Language::English, 1) => "1 notification".to_owned(),
            (Language::English, n) => format!("{n} notifications"),
            (Language::Vietnamese, n) => format!("{n} thông báo"),
        }
    }

    /// How long ago a notification came, at the end of its row.
    pub fn notifications_ago(self, seconds: f64) -> String {
        let minutes = (seconds.max(0.0) / 60.0).floor() as u64;
        let (hours, days) = (minutes / 60, minutes / 1440);
        match self {
            Language::English => match (minutes, hours, days) {
                (0, ..) => "just now".to_owned(),
                (m, 0, _) => format!("{m} min ago"),
                (_, h, 0) => format!("{h} h ago"),
                (.., 1) => "yesterday".to_owned(),
                (.., d) => format!("{d} days ago"),
            },
            Language::Vietnamese => match (minutes, hours, days) {
                (0, ..) => "vừa xong".to_owned(),
                (m, 0, _) => format!("{m} phút trước"),
                (_, h, 0) => format!("{h} giờ trước"),
                (.., 1) => "hôm qua".to_owned(),
                (.., d) => format!("{d} ngày trước"),
            },
        }
    }

    /// In place of the words, when the person keeps them off the notch.
    pub fn notifications_hidden(self) -> &'static str {
        match self {
            Language::English => "New notification",
            Language::Vietnamese => "Thông báo mới",
        }
    }

    pub fn notifications_clear(self) -> &'static str {
        match self {
            Language::English => "Clear the list",
            Language::Vietnamese => "Xóa danh sách",
        }
    }

    pub fn notifications_open_settings(self) -> &'static str {
        match self {
            Language::English => "Open System Settings",
            Language::Vietnamese => "Mở Cài đặt hệ thống",
        }
    }

    pub fn notifications_off_title(self) -> &'static str {
        match self {
            Language::English => "The Notifications tab is off",
            Language::Vietnamese => "Tab Thông báo đang tắt",
        }
    }

    pub fn notifications_off_hint(self) -> &'static str {
        match self {
            Language::English => "Turn it on in Settings, under Notch & Island, Notifications.",
            Language::Vietnamese => "Bật trong Cài đặt, mục Notch & Đảo động, Thông báo.",
        }
    }

    pub fn notifications_access_title(self) -> &'static str {
        match self {
            Language::English => "Figghost cannot read notifications yet",
            Language::Vietnamese => "Figghost chưa đọc được thông báo",
        }
    }

    pub fn notifications_access_hint(self) -> &'static str {
        match self {
            Language::English => {
                "Turn Figghost on in System Settings, Privacy & Security, Full Disk Access, then quit and open it again. The button at the top right opens the list."
            }
            Language::Vietnamese => {
                "Bật Figghost trong Cài đặt hệ thống, Quyền riêng tư & Bảo mật, Truy cập toàn bộ ổ đĩa, rồi thoát và mở lại app. Nút ở góc trên bên phải mở đúng danh sách đó."
            }
        }
    }

    pub fn notifications_empty_title(self) -> &'static str {
        match self {
            Language::English => "No notifications yet",
            Language::Vietnamese => "Chưa có thông báo nào",
        }
    }

    pub fn notifications_empty_hint(self) -> &'static str {
        match self {
            Language::English => "New ones show here as they arrive.",
            Language::Vietnamese => "Thông báo mới sẽ hiện ở đây ngay khi tới.",
        }
    }
}
