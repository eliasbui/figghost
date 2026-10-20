//! The words of the Notes tab, in each language.

use crate::i18n::Language;

impl Language {
    pub fn notes_tab(self) -> &'static str {
        match self {
            Language::English => "Notes",
            Language::Vietnamese => "Ghi chú",
        }
    }

    pub fn notes_placeholder(self) -> &'static str {
        match self {
            Language::English => "Start typing...",
            Language::Vietnamese => "Bắt đầu gõ...",
        }
    }

    pub fn notes_new(self) -> &'static str {
        match self {
            Language::English => "New note",
            Language::Vietnamese => "Ghi chú mới",
        }
    }

    pub fn notes_delete(self) -> &'static str {
        match self {
            Language::English => "Move note to the Trash",
            Language::Vietnamese => "Chuyển ghi chú vào Thùng rác",
        }
    }

    /// What the line under the editor says about how much is written.
    pub fn notes_count(self, characters: usize) -> String {
        match (self, characters) {
            (Language::English, 1) => "1 character".into(),
            (Language::English, n) => format!("{n} characters"),
            (Language::Vietnamese, n) => format!("{n} ký tự"),
        }
    }

    pub fn notes_off_title(self) -> &'static str {
        match self {
            Language::English => "The Notes tab is off",
            Language::Vietnamese => "Tab Ghi chú đang tắt",
        }
    }

    pub fn notes_off_hint(self) -> &'static str {
        match self {
            Language::English => "Turn it on in Settings, under Notch tabs, Notes.",
            Language::Vietnamese => "Bật trong Cài đặt, mục Tab của notch, Ghi chú.",
        }
    }
}
