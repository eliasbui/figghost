//! The words of the Updates tab, in each language.

use super::logic::Source;
use crate::i18n::Language;

impl Language {
    pub fn updates_tab(self) -> &'static str {
        match self {
            Language::English => "Updates",
            Language::Vietnamese => "Cập nhật",
        }
    }

    pub fn updates_heading(self, count: usize) -> String {
        match (self, count) {
            (Language::English, 1) => "1 update".into(),
            (Language::English, n) => format!("{n} updates"),
            (Language::Vietnamese, n) => format!("{n} bản cập nhật"),
        }
    }

    pub fn updates_source(self, source: Source) -> &'static str {
        match (self, source) {
            (_, Source::Formula) => "Homebrew",
            (_, Source::Cask) => "Homebrew cask",
            (_, Source::Npm) => "npm",
            (_, Source::AppStore) => "App Store",
            (_, Source::Apple) => "Apple",
            (Language::English, Source::Agent) => "Coding agent",
            (Language::Vietnamese, Source::Agent) => "Agent lập trình",
        }
    }

    pub fn updates_refresh(self) -> &'static str {
        match self {
            Language::English => "Check again",
            Language::Vietnamese => "Kiểm tra lại",
        }
    }

    pub fn updates_chosen(self, count: usize) -> String {
        match self {
            Language::English => format!("Update {count}"),
            Language::Vietnamese => format!("Cập nhật {count}"),
        }
    }

    pub fn updates_all(self) -> &'static str {
        match self {
            Language::English => "Update all",
            Language::Vietnamese => "Cập nhật tất cả",
        }
    }

    pub fn updates_checked(self, ago: &str) -> String {
        match self {
            Language::English => format!("Checked {ago}"),
            Language::Vietnamese => format!("Kiểm tra {ago}"),
        }
    }

    pub fn updates_looking(self) -> &'static str {
        match self {
            Language::English => "Checking...",
            Language::Vietnamese => "Đang kiểm tra...",
        }
    }

    pub fn updates_updating(self) -> &'static str {
        match self {
            Language::English => "Updating, this can take a few minutes...",
            Language::Vietnamese => "Đang cập nhật, có thể mất vài phút...",
        }
    }

    pub fn updates_result(self, done: usize, failed: usize) -> String {
        match (self, failed) {
            (Language::English, 0) => format!("Updated {done}"),
            (Language::English, f) => format!("Updated {done}, {f} failed"),
            (Language::Vietnamese, 0) => format!("Đã cập nhật {done}"),
            (Language::Vietnamese, f) => format!("Đã cập nhật {done}, {f} lỗi"),
        }
    }

    pub fn updates_source_failed(self, source: Source) -> String {
        let name = self.updates_source(source);
        match self {
            Language::English => format!("{name} did not answer"),
            Language::Vietnamese => format!("{name} không trả lời"),
        }
    }

    pub fn updates_off_title(self) -> &'static str {
        match self {
            Language::English => "Updates are off",
            Language::Vietnamese => "Đang tắt kiểm tra cập nhật",
        }
    }

    pub fn updates_off_hint(self) -> &'static str {
        match self {
            Language::English => "Turn it on in Settings, under Updates.",
            Language::Vietnamese => "Bật trong Cài đặt, mục Cập nhật.",
        }
    }

    pub fn updates_loading_title(self) -> &'static str {
        match self {
            Language::English => "Looking for updates",
            Language::Vietnamese => "Đang tìm bản cập nhật",
        }
    }

    pub fn updates_loading_hint(self) -> &'static str {
        match self {
            Language::English => {
                "Homebrew, npm, the App Store, macOS and the coding agents are asked; it takes a minute."
            }
            Language::Vietnamese => {
                "Đang hỏi Homebrew, npm, App Store, macOS và các agent lập trình; mất khoảng một phút."
            }
        }
    }

    pub fn updates_nothing_title(self) -> &'static str {
        match self {
            Language::English => "Everything is up to date",
            Language::Vietnamese => "Mọi thứ đã là bản mới nhất",
        }
    }

    pub fn updates_nothing_hint(self) -> &'static str {
        match self {
            Language::English => "Figghost looks again every few hours.",
            Language::Vietnamese => "Figghost sẽ kiểm tra lại sau vài giờ.",
        }
    }
}
