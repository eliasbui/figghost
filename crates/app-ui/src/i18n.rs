//! Localization. English is the default; Vietnamese is the second language. Every user-visible string is a
//! method with an exhaustive `match`, so adding a language to `Language` makes the compiler list every
//! string that still needs translating.

use macos_sys::labels::Labels;
use std::sync::OnceLock;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Language {
    #[default]
    English,
    Vietnamese,
}

impl Language {
    pub const ALL: [Language; 2] = [Language::English, Language::Vietnamese];

    /// ISO 639-1 code, as written in the config file.
    pub const fn code(self) -> &'static str {
        match self {
            Language::English => "en",
            Language::Vietnamese => "vi",
        }
    }

    /// From a language tag such as `vi`, `vi-VN` or `en_US`; only the language part counts.
    pub fn from_tag(tag: &str) -> Option<Self> {
        let primary = tag.split(['-', '_']).next()?.trim().to_ascii_lowercase();
        Self::ALL.into_iter().find(|l| l.code() == primary)
    }

    /// The `language` setting wins unless it is `auto` (or unknown); then the first system language we
    /// have a translation for; then English.
    pub fn resolve(setting: &str, system: &[String]) -> Self {
        let setting = setting.trim();
        if !setting.eq_ignore_ascii_case("auto")
            && let Some(language) = Self::from_tag(setting)
        {
            return language;
        }
        system
            .iter()
            .find_map(|tag| Self::from_tag(tag))
            .unwrap_or_default()
    }

    pub fn just_now(self) -> &'static str {
        match self {
            Language::English => "just now",
            Language::Vietnamese => "vừa xong",
        }
    }

    pub fn minutes_ago(self, n: i64) -> String {
        match self {
            Language::English => plural(n, "minute", "minutes", " ago"),
            Language::Vietnamese => format!("{n} phút trước"),
        }
    }

    pub fn hours_ago(self, n: i64) -> String {
        match self {
            Language::English => plural(n, "hour", "hours", " ago"),
            Language::Vietnamese => format!("{n} giờ trước"),
        }
    }

    pub fn days_ago(self, n: i64) -> String {
        match self {
            Language::English => plural(n, "day", "days", " ago"),
            Language::Vietnamese => format!("{n} ngày trước"),
        }
    }

    pub fn months_ago(self, n: i64) -> String {
        match self {
            Language::English => plural(n, "month", "months", " ago"),
            Language::Vietnamese => format!("{n} tháng trước"),
        }
    }

    pub fn history_empty(self) -> &'static str {
        match self {
            Language::English => "Nothing in the clipboard history yet",
            Language::Vietnamese => "Chưa có gì trong lịch sử clipboard",
        }
    }

    pub fn no_matches(self) -> &'static str {
        match self {
            Language::English => "No matches",
            Language::Vietnamese => "Không tìm thấy kết quả",
        }
    }

    pub fn pinned(self) -> &'static str {
        match self {
            Language::English => "Pinned",
            Language::Vietnamese => "Đã ghim",
        }
    }

    pub fn image_title(self, width: u32, height: u32) -> String {
        match self {
            Language::English => format!("Image {width}x{height}"),
            Language::Vietnamese => format!("Ảnh {width}x{height}"),
        }
    }

    pub fn files_title(self, first: &str, others: usize) -> String {
        match self {
            Language::English if others == 1 => format!("{first} and 1 other file"),
            Language::English => format!("{first} and {others} other files"),
            Language::Vietnamese => format!("{first} và {others} tệp khác"),
        }
    }

    pub fn menu_accessibility(self) -> &'static str {
        match self {
            Language::English => "Accessibility access is off: click to grant",
            Language::Vietnamese => "Chưa cấp quyền Accessibility: bấm để cấp",
        }
    }

    pub fn menu_smooth(self) -> &'static str {
        match self {
            Language::English => "Smooth scrolling",
            Language::Vietnamese => "Cuộn mượt",
        }
    }

    pub fn menu_settings(self) -> &'static str {
        match self {
            Language::English => "Settings...",
            Language::Vietnamese => "Cài đặt...",
        }
    }

    pub fn menu_volume_hud(self) -> &'static str {
        match self {
            Language::English => "Volume indicator in the notch",
            Language::Vietnamese => "Hiển thị âm lượng ở notch",
        }
    }

    pub fn pick_app_prompt(self) -> &'static str {
        match self {
            Language::English => "Choose an app",
            Language::Vietnamese => "Chọn một ứng dụng",
        }
    }

    pub fn menu_reverse(self) -> &'static str {
        match self {
            Language::English => "Reverse mouse wheel direction",
            Language::Vietnamese => "Đảo hướng cuộn của chuột",
        }
    }

    pub fn menu_smooth_for(self, app: &str) -> String {
        match self {
            Language::English => format!("Smooth scrolling for {app}"),
            Language::Vietnamese => format!("Cuộn mượt cho {app}"),
        }
    }

    pub fn menu_smooth_current(self) -> &'static str {
        match self {
            Language::English => "Smooth scrolling for the current app",
            Language::Vietnamese => "Cuộn mượt cho ứng dụng hiện tại",
        }
    }

    pub fn menu_open_history(self) -> &'static str {
        match self {
            Language::English => "Open clipboard history",
            Language::Vietnamese => "Mở lịch sử clipboard",
        }
    }

    pub fn menu_clear_history(self) -> &'static str {
        match self {
            Language::English => "Clear history (keep pinned)",
            Language::Vietnamese => "Xoá lịch sử (giữ mục đã ghim)",
        }
    }

    pub fn menu_login(self) -> &'static str {
        match self {
            Language::English => "Start at login",
            Language::Vietnamese => "Khởi động cùng máy",
        }
    }

    pub fn menu_login_needs_approval(self) -> &'static str {
        match self {
            Language::English => "Start at login (allow it in System Settings)",
            Language::Vietnamese => "Khởi động cùng máy (cần cho phép trong Cài đặt)",
        }
    }

    pub fn menu_login_unavailable(self) -> &'static str {
        match self {
            Language::English => "Start at login (only when run from Figghost.app)",
            Language::Vietnamese => "Khởi động cùng máy (chỉ khi chạy từ Figghost.app)",
        }
    }

    pub fn menu_login_error(self, error: &str) -> String {
        match self {
            Language::English => format!("Start at login (could not enable: {error})"),
            Language::Vietnamese => format!("Khởi động cùng máy (không bật được: {error})"),
        }
    }

    pub fn menu_open_config(self) -> &'static str {
        match self {
            Language::English => "Open settings folder",
            Language::Vietnamese => "Mở thư mục cấu hình",
        }
    }

    pub fn menu_quit(self) -> &'static str {
        match self {
            Language::English => "Quit",
            Language::Vietnamese => "Thoát",
        }
    }

    /// The text the UI layer draws itself.
    pub fn labels(self) -> Labels {
        match self {
            Language::English => Labels::default(),
            Language::Vietnamese => Labels {
                clipboard_tab: "Clipboard".into(),
                shelf_tab: "Khay".into(),
                search_placeholder: "Tìm kiếm  (@image @link @code @file @pin)".into(),
                clipboard_hint: "Enter: dán    Opt+P: ghim    Opt+Xoá: xoá    Esc: đóng".into(),
                shelf_hint:
                    "Kéo tệp vào để giữ tạm    Nhấp đúp: mở    Kéo ra: dùng    Xoá: bỏ khỏi khay"
                        .into(),
                shelf_empty: "Kéo tệp vào đây để giữ tạm".into(),
                media_tab: "Media".into(),
                media_idle: "Không có bài nào đang phát".into(),
                media_previous: "Trước".into(),
                media_play_pause: "Phát / Tạm dừng".into(),
                media_next: "Sau".into(),
                media_hint: "Điều khiển ứng dụng đang phát. Thông tin bài hát: Music và Spotify."
                    .into(),
                media_playing: "Đang phát".into(),
                media_paused: "Đã tạm dừng".into(),
                menu_about: "Giới thiệu".into(),
                menu_settings: "Cài đặt...".into(),
                menu_hide: "Ẩn".into(),
                menu_hide_others: "Ẩn các ứng dụng khác".into(),
                menu_show_all: "Hiện tất cả".into(),
                menu_quit: "Thoát".into(),
                menu_edit: "Chỉnh sửa".into(),
                menu_undo: "Hoàn tác".into(),
                menu_cut: "Cắt".into(),
                menu_copy: "Sao chép".into(),
                menu_paste: "Dán".into(),
                menu_select_all: "Chọn tất cả".into(),
                menu_window: "Cửa sổ".into(),
                menu_minimize: "Thu nhỏ".into(),
                menu_zoom: "Phóng to".into(),
                menu_close: "Đóng".into(),
            },
        }
    }
}

fn plural(n: i64, one: &str, many: &str, suffix: &str) -> String {
    format!("{n} {}{suffix}", if n == 1 { one } else { many })
}

static CURRENT: OnceLock<Language> = OnceLock::new();

/// Fixes the language for the process; only the first call has an effect.
pub fn set(language: Language) {
    let _ = CURRENT.set(language);
}

pub fn current() -> Language {
    *CURRENT.get_or_init(Language::default)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tags(list: &[&str]) -> Vec<String> {
        list.iter().map(|s| (*s).to_owned()).collect()
    }

    #[test]
    fn auto_follows_the_first_system_language_we_translate() {
        assert_eq!(
            Language::resolve("auto", &tags(&["vi-VN", "en-US"])),
            Language::Vietnamese
        );
        assert_eq!(
            Language::resolve("auto", &tags(&["fr-FR", "vi"])),
            Language::Vietnamese
        );
        assert_eq!(
            Language::resolve("auto", &tags(&["en-GB", "vi"])),
            Language::English
        );
        assert_eq!(
            Language::resolve("auto", &tags(&["fr-FR", "de-DE"])),
            Language::English
        );
        assert_eq!(Language::resolve("auto", &[]), Language::English);
    }

    #[test]
    fn an_explicit_setting_beats_the_system_language() {
        assert_eq!(
            Language::resolve("en", &tags(&["vi-VN"])),
            Language::English
        );
        assert_eq!(
            Language::resolve("vi", &tags(&["en-US"])),
            Language::Vietnamese
        );
        assert_eq!(
            Language::resolve(" VI_vn ", &tags(&["en-US"])),
            Language::Vietnamese
        );
        // An unknown code is ignored rather than breaking the app: it behaves like "auto".
        assert_eq!(
            Language::resolve("klingon", &tags(&["vi-VN"])),
            Language::Vietnamese
        );
        assert_eq!(Language::resolve("", &tags(&["en-US"])), Language::English);
    }

    #[test]
    fn language_codes_round_trip() {
        for language in Language::ALL {
            assert_eq!(Language::from_tag(language.code()), Some(language));
        }
        assert_eq!(Language::from_tag("zh-Hans-CN"), None);
    }

    #[test]
    fn english_plurals() {
        let en = Language::English;
        assert_eq!(en.minutes_ago(1), "1 minute ago");
        assert_eq!(en.minutes_ago(5), "5 minutes ago");
        assert_eq!(en.hours_ago(1), "1 hour ago");
        assert_eq!(en.days_ago(2), "2 days ago");
        assert_eq!(en.months_ago(1), "1 month ago");
        assert_eq!(en.files_title("a.pdf", 1), "a.pdf and 1 other file");
        assert_eq!(en.files_title("a.pdf", 3), "a.pdf and 3 other files");
    }

    #[test]
    fn every_string_exists_and_has_no_unfilled_placeholder_in_every_language() {
        for language in Language::ALL {
            let strings = [
                language.just_now().to_owned(),
                language.minutes_ago(5),
                language.hours_ago(2),
                language.days_ago(3),
                language.months_ago(4),
                language.history_empty().to_owned(),
                language.no_matches().to_owned(),
                language.pinned().to_owned(),
                language.image_title(10, 20),
                language.files_title("a.txt", 2),
                language.menu_accessibility().to_owned(),
                language.menu_smooth().to_owned(),
                language.menu_reverse().to_owned(),
                language.menu_volume_hud().to_owned(),
                language.menu_settings().to_owned(),
                language.pick_app_prompt().to_owned(),
                language.menu_smooth_for("Safari"),
                language.menu_smooth_current().to_owned(),
                language.menu_open_history().to_owned(),
                language.menu_clear_history().to_owned(),
                language.menu_login().to_owned(),
                language.menu_login_needs_approval().to_owned(),
                language.menu_login_unavailable().to_owned(),
                language.menu_login_error("denied"),
                language.menu_open_config().to_owned(),
                language.menu_quit().to_owned(),
            ];
            for text in strings {
                assert!(!text.trim().is_empty(), "{language:?}: empty string");
                assert!(
                    !text.contains(['{', '}']),
                    "{language:?}: unfilled placeholder in '{text}'"
                );
            }
            let labels = language.labels();
            for text in [
                &labels.clipboard_tab,
                &labels.shelf_tab,
                &labels.search_placeholder,
                &labels.clipboard_hint,
                &labels.shelf_hint,
                &labels.shelf_empty,
                &labels.media_tab,
                &labels.media_idle,
                &labels.media_previous,
                &labels.media_play_pause,
                &labels.media_next,
                &labels.media_hint,
                &labels.media_playing,
                &labels.media_paused,
                &labels.menu_about,
                &labels.menu_settings,
                &labels.menu_hide,
                &labels.menu_hide_others,
                &labels.menu_show_all,
                &labels.menu_quit,
                &labels.menu_edit,
                &labels.menu_undo,
                &labels.menu_cut,
                &labels.menu_copy,
                &labels.menu_paste,
                &labels.menu_select_all,
                &labels.menu_window,
                &labels.menu_minimize,
                &labels.menu_zoom,
                &labels.menu_close,
            ] {
                assert!(!text.trim().is_empty(), "{language:?}: empty label");
            }
        }
    }

    #[test]
    fn translations_are_not_copies_of_english() {
        let (en, vi) = (Language::English, Language::Vietnamese);
        assert_ne!(en.menu_quit(), vi.menu_quit());
        assert_ne!(en.pinned(), vi.pinned());
        assert_ne!(en.labels().shelf_hint, vi.labels().shelf_hint);
        assert_ne!(en.labels().clipboard_hint, vi.labels().clipboard_hint);
        assert_ne!(en.labels().shelf_tab, vi.labels().shelf_tab);
        assert_ne!(en.labels().media_idle, vi.labels().media_idle);
        assert_ne!(en.labels().media_play_pause, vi.labels().media_play_pause);
    }
}
