//! The words of the Outlook tab and its card, in each language.

use crate::i18n::Language;

impl Language {
    pub fn outlook_tab(self) -> &'static str {
        "Outlook"
    }

    /// How many mails and reminders the tab lists.
    pub fn outlook_count(self, mails: usize, events: usize) -> String {
        let mails = match (self, mails) {
            (Language::English, 1) => "1 mail".to_owned(),
            (Language::English, n) => format!("{n} mails"),
            (Language::Vietnamese, n) => format!("{n} thư"),
        };
        match (self, events) {
            (_, 0) => mails,
            (Language::English, 1) => format!("{mails}, 1 reminder"),
            (Language::English, n) => format!("{mails}, {n} reminders"),
            (Language::Vietnamese, n) => format!("{mails}, {n} lịch"),
        }
    }

    /// What a row is: a mail or a reminder of the calendar.
    pub fn outlook_kind(self, event: bool) -> &'static str {
        match (self, event) {
            (Language::English, false) => "Mail",
            (Language::English, true) => "Calendar",
            (Language::Vietnamese, false) => "Thư",
            (Language::Vietnamese, true) => "Lịch",
        }
    }

    /// The second line on the closed notch when what a mail says is kept off it.
    pub fn outlook_hidden(self, event: bool) -> &'static str {
        match (self, event) {
            (Language::English, false) => "New mail",
            (Language::English, true) => "Calendar reminder",
            (Language::Vietnamese, false) => "Thư mới",
            (Language::Vietnamese, true) => "Nhắc lịch",
        }
    }

    pub fn outlook_open(self) -> &'static str {
        match self {
            Language::English => "Open in Outlook",
            Language::Vietnamese => "Mở trong Outlook",
        }
    }

    pub fn outlook_close(self) -> &'static str {
        match self {
            Language::English => "Close",
            Language::Vietnamese => "Đóng",
        }
    }

    pub fn outlook_off_title(self) -> &'static str {
        match self {
            Language::English => "Outlook is off",
            Language::Vietnamese => "Outlook đang tắt",
        }
    }

    pub fn outlook_off_hint(self) -> &'static str {
        match self {
            Language::English => "Turn it on in Settings, Outlook.",
            Language::Vietnamese => "Bật trong Cài đặt, mục Outlook.",
        }
    }

    pub fn outlook_empty_title(self) -> &'static str {
        match self {
            Language::English => "No new mail",
            Language::Vietnamese => "Chưa có thư mới",
        }
    }

    pub fn outlook_empty_hint(self) -> &'static str {
        match self {
            Language::English => {
                "New mail and reminders of the Outlook app show here, while it runs with its notifications on."
            }
            Language::Vietnamese => {
                "Thư mới và nhắc lịch của app Outlook hiện ở đây, khi Outlook đang chạy và bật thông báo."
            }
        }
    }
}
