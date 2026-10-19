//! Localization. English is the default; Vietnamese is the second language. Every user-visible string is a
//! method with an exhaustive `match`, so adding a language to `Language` makes the compiler list every
//! string that still needs translating.

use core_agents::tracker::State;
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

    pub fn shelf_menu_zip(self) -> &'static str {
        match self {
            Language::English => "Compress to ZIP",
            Language::Vietnamese => "Nén thành ZIP",
        }
    }

    pub fn shelf_menu_convert(self, format: &str) -> String {
        match self {
            Language::English => format!("Convert to {format}"),
            Language::Vietnamese => format!("Chuyển sang {format}"),
        }
    }

    pub fn shelf_menu_half(self) -> &'static str {
        match self {
            Language::English => "Resize to half",
            Language::Vietnamese => "Đổi kích thước còn một nửa",
        }
    }

    pub fn shelf_menu_quarter(self) -> &'static str {
        match self {
            Language::English => "Resize to a quarter",
            Language::Vietnamese => "Đổi kích thước còn một phần tư",
        }
    }

    pub fn shelf_menu_copy_text(self) -> &'static str {
        match self {
            Language::English => "Copy text from the image",
            Language::Vietnamese => "Sao chép chữ trong ảnh",
        }
    }

    pub fn shelf_text_copied(self, lines: usize) -> String {
        match self {
            Language::English if lines == 1 => "Copied 1 line of text".to_owned(),
            Language::English => format!("Copied {lines} lines of text"),
            Language::Vietnamese => format!("Đã sao chép {lines} dòng chữ"),
        }
    }

    pub fn shelf_no_text(self) -> &'static str {
        match self {
            Language::English => "No text found in the image",
            Language::Vietnamese => "Không tìm thấy chữ trong ảnh",
        }
    }

    pub fn shelf_image_failed(self, detail: &str) -> String {
        match self {
            Language::English => format!("Cannot read the image: {detail}"),
            Language::Vietnamese => format!("Không đọc được ảnh: {detail}"),
        }
    }

    pub fn shelf_menu_reveal(self) -> &'static str {
        match self {
            Language::English => "Show in Finder",
            Language::Vietnamese => "Hiện trong Finder",
        }
    }

    pub fn shelf_menu_remove(self) -> &'static str {
        match self {
            Language::English => "Remove from shelf",
            Language::Vietnamese => "Bỏ khỏi khay",
        }
    }

    pub fn agents_wants(self, agent: &str, tool: &str) -> String {
        match self {
            Language::English => format!("{agent} wants to use {tool}"),
            Language::Vietnamese => format!("{agent} muốn dùng {tool}"),
        }
    }

    pub fn agents_asks(self, agent: &str) -> String {
        match self {
            Language::English => format!("{agent} is asking for permission"),
            Language::Vietnamese => format!("{agent} đang xin quyền"),
        }
    }

    pub fn agents_allow(self) -> &'static str {
        match self {
            Language::English => "Allow",
            Language::Vietnamese => "Cho phép",
        }
    }

    pub fn agents_deny(self) -> &'static str {
        match self {
            Language::English => "Deny",
            Language::Vietnamese => "Từ chối",
        }
    }

    /// What the agent is told when the user denies a command from the notch.
    pub fn agents_denied_message(self) -> &'static str {
        match self {
            Language::English => "The user denied this from the notch.",
            Language::Vietnamese => "Người dùng đã từ chối yêu cầu này từ notch.",
        }
    }

    pub fn agents_more_waiting(self, n: usize) -> String {
        match self {
            Language::English => format!("{n} more waiting"),
            Language::Vietnamese => format!("Còn {n} yêu cầu đang chờ"),
        }
    }

    pub fn agents_sessions(self) -> &'static str {
        match self {
            Language::English => "Sessions",
            Language::Vietnamese => "Phiên làm việc",
        }
    }

    pub fn agents_usage(self) -> &'static str {
        match self {
            Language::English => "Usage limits",
            Language::Vietnamese => "Hạn mức sử dụng",
        }
    }

    /// The refresh button of the usage section, with its key.
    pub fn agents_refresh(self) -> &'static str {
        match self {
            Language::English => "Refresh usage limits (Cmd+R)",
            Language::Vietnamese => "Làm mới hạn mức sử dụng (Cmd+R)",
        }
    }

    /// After a refresh, beside the heading: "updated just now".
    pub fn agents_updated(self, age: &str) -> String {
        match self {
            Language::English => format!("updated {age}"),
            Language::Vietnamese => format!("cập nhật {age}"),
        }
    }

    pub fn agents_refresh_failed(self) -> &'static str {
        match self {
            Language::English => "Codex limits unavailable",
            Language::Vietnamese => "Không lấy được hạn mức Codex",
        }
    }

    pub fn agents_tokens(self) -> &'static str {
        match self {
            Language::English => "Tokens and cost at API prices",
            Language::Vietnamese => "Token và chi phí theo giá API",
        }
    }

    /// A token count in a few characters: 950, 12.4K, 1.24M.
    pub fn tokens_short(self, n: u64) -> String {
        let (value, unit) = match n {
            0..1_000 => return n.to_string(),
            1_000..1_000_000 => (n as f64 / 1_000.0, "K"),
            1_000_000..1_000_000_000 => (n as f64 / 1_000_000.0, "M"),
            _ => (n as f64 / 1_000_000_000.0, "B"),
        };
        let text = if value < 10.0 {
            format!("{value:.2}")
        } else if value < 100.0 {
            format!("{value:.1}")
        } else {
            format!("{value:.0}")
        };
        let text = match self {
            Language::English => text,
            Language::Vietnamese => text.replace('.', ","),
        };
        format!("{text}{unit}")
    }

    /// An amount in US dollars: $3.45, $1,204.
    pub fn dollars(self, amount: f64) -> String {
        let cents = (amount * 100.0).round() as i64;
        let (whole, part) = (cents / 100, cents % 100);
        let digits = whole.to_string();
        let mut grouped = String::new();
        for (i, c) in digits.chars().enumerate() {
            if i > 0 && (digits.len() - i) % 3 == 0 {
                grouped.push(match self {
                    Language::English => ',',
                    Language::Vietnamese => '.',
                });
            }
            grouped.push(c);
        }
        match self {
            Language::English if whole >= 1_000 => format!("${grouped}"),
            Language::English => format!("${grouped}.{part:02}"),
            Language::Vietnamese if whole >= 1_000 => format!("{grouped} US$"),
            Language::Vietnamese => format!("{grouped},{part:02} US$"),
        }
    }

    /// "1.24M tokens  $3.45" for what an agent used today; the cost only when it is known.
    pub fn tokens_today(self, tokens: u64, cost: Option<f64>) -> String {
        let count = match self {
            Language::English => format!("{} tokens", self.tokens_short(tokens)),
            Language::Vietnamese => format!("{} token", self.tokens_short(tokens)),
        };
        match cost {
            Some(cost) => format!("{count}  {}", self.dollars(cost)),
            None => count,
        }
    }

    /// "Week $21.30", or the tokens of the week when there is no price.
    pub fn tokens_week(self, tokens: u64, cost: Option<f64>) -> String {
        let amount = cost.map_or_else(|| self.tokens_short(tokens), |c| self.dollars(c));
        match self {
            Language::English => format!("Week {amount}"),
            Language::Vietnamese => format!("Tuần {amount}"),
        }
    }

    pub fn agents_state(self, state: State) -> &'static str {
        match (self, state) {
            (Language::English, State::Waiting) => "Waiting for you",
            (Language::English, State::Working) => "Working",
            (Language::English, State::Failed) => "Failed",
            (Language::English, State::Done) => "Done",
            (Language::English, State::Idle) => "Idle",
            (Language::Vietnamese, State::Waiting) => "Đang chờ bạn",
            (Language::Vietnamese, State::Working) => "Đang làm",
            (Language::Vietnamese, State::Failed) => "Lỗi",
            (Language::Vietnamese, State::Done) => "Xong",
            (Language::Vietnamese, State::Idle) => "Đang rảnh",
        }
    }

    pub fn agents_empty_title(self) -> &'static str {
        match self {
            Language::English => "No agent is running",
            Language::Vietnamese => "Chưa có tác tử nào đang chạy",
        }
    }

    pub fn agents_empty_hint(self) -> &'static str {
        match self {
            Language::English => {
                "Start Claude Code or Codex in a terminal. Turn on the hooks in Settings to see it here."
            }
            Language::Vietnamese => {
                "Chạy Claude Code hoặc Codex trong terminal. Bật hook trong Cài đặt để thấy chúng ở đây."
            }
        }
    }

    /// The length of a usage window: "5 h", "7 d".
    pub fn agents_window(self, minutes: u64) -> String {
        if minutes <= 2880 {
            let hours = (minutes as f64 / 60.0).round() as u64;
            match self {
                Language::English => format!("{hours} h"),
                Language::Vietnamese => format!("{hours} giờ"),
            }
        } else {
            let days = (minutes as f64 / 1440.0).round() as u64;
            match self {
                Language::English => format!("{days} d"),
                Language::Vietnamese => format!("{days} ngày"),
            }
        }
    }

    /// A time left, in its two biggest units: "2 h 10 min", "4 d".
    pub fn short_duration(self, secs: i64) -> String {
        let secs = secs.max(0);
        let (days, hours, minutes) = (secs / 86_400, (secs % 86_400) / 3600, (secs % 3600) / 60);
        match self {
            Language::English => match (days, hours) {
                (d, _) if d >= 2 => format!("{d} d"),
                (1, h) => format!("1 d {h} h"),
                (_, h) if h > 0 => format!("{h} h {minutes} min"),
                _ => format!("{} min", minutes.max(1)),
            },
            Language::Vietnamese => match (days, hours) {
                (d, _) if d >= 2 => format!("{d} ngày"),
                (1, h) => format!("1 ngày {h} giờ"),
                (_, h) if h > 0 => format!("{h} giờ {minutes} phút"),
                _ => format!("{} phút", minutes.max(1)),
            },
        }
    }

    pub fn agents_resets_in(self, secs: i64) -> String {
        match self {
            Language::English => format!("resets in {}", self.short_duration(secs)),
            Language::Vietnamese => format!("đặt lại sau {}", self.short_duration(secs)),
        }
    }

    pub fn agents_reset_already(self) -> &'static str {
        match self {
            Language::English => "has reset",
            Language::Vietnamese => "đã đặt lại",
        }
    }

    /// For numbers read a while ago: "as of 6 days ago".
    pub fn agents_as_of(self, age: &str) -> String {
        match self {
            Language::English => format!("as of {age}"),
            Language::Vietnamese => format!("số liệu {age}"),
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
                search_placeholder_short: "Tìm kiếm".into(),
                clipboard_hint: "Enter: dán    Cmd+1-9: dán dòng    Opt+P: ghim    Opt+Xoá: xoá    Esc: đóng".into(),
                clipboard_hint_strip: "Enter: dán    Trái và Phải: duyệt    Opt+P: ghim    Opt+Xoá: xoá    Esc: đóng".into(),
                chip_all: "Tất cả".into(),
                chip_pinned: "Đã ghim".into(),
                chip_links: "Liên kết".into(),
                chip_code: "Mã".into(),
                chip_images: "Ảnh".into(),
                chip_files: "Tệp".into(),
                chip_text: "Văn bản".into(),
                shelf_hint:
                    "Kéo tệp vào để giữ tạm    Nhấp đúp: mở    Kéo ra: dùng    Xoá: bỏ khỏi khay"
                        .into(),
                shelf_empty: "Kéo tệp vào đây để giữ tạm".into(),
                agents_tab: "Tác tử".into(),
                agents_hint: "Return: cho phép    Opt+Xoá: từ chối    Không trả lời thì tác tử hỏi ngay trong terminal."
                    .into(),
                media_tab: "Media".into(),
                media_idle: "Không có bài nào đang phát".into(),
                media_previous: "Trước".into(),
                media_play_pause: "Phát / Tạm dừng".into(),
                media_next: "Sau".into(),
                media_position: "Vị trí phát".into(),
                media_hint: "Điều khiển ứng dụng đang phát, kể cả trình duyệt.".into(),
                media_playing: "Đang phát".into(),
                media_paused: "Đã tạm dừng".into(),
                focus_tab: "Tập trung".into(),
                focus_start: "Bắt đầu".into(),
                focus_pause: "Tạm dừng".into(),
                focus_resume: "Tiếp tục".into(),
                focus_reset: "Đặt lại".into(),
                focus_ready: "Chọn thời lượng rồi bắt đầu".into(),
                focus_running: "Giữ nhịp nào".into(),
                focus_paused: "Đã tạm dừng".into(),
                focus_done: "Hết giờ".into(),
                focus_hint: "Return: bắt đầu hoặc tạm dừng    Notch tự mở khi hết giờ.".into(),
                focus_minutes: "phút".into(),
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

    #[test]
    fn token_counts_and_dollars_read_well_in_both_languages() {
        let (en, vi) = (Language::English, Language::Vietnamese);
        assert_eq!(en.tokens_short(950), "950");
        assert_eq!(en.tokens_short(12_400), "12.4K");
        assert_eq!(en.tokens_short(1_240_000), "1.24M");
        assert_eq!(vi.tokens_short(1_240_000), "1,24M");
        assert_eq!(en.dollars(3.454), "$3.45");
        assert_eq!(en.dollars(1_204.4), "$1,204");
        assert_eq!(vi.dollars(3.45), "3,45 US$");
        assert_eq!(vi.dollars(1_204.0), "1.204 US$");
        assert_eq!(
            en.tokens_today(1_240_000, Some(3.45)),
            "1.24M tokens  $3.45"
        );
        assert_eq!(vi.tokens_today(380_000, None), "380K token");
        assert_eq!(en.tokens_week(2_100_000, None), "Week 2.10M");
        assert_eq!(vi.tokens_week(0, Some(21.3)), "Tuần 21,30 US$");
    }

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
                language.agents_wants("Claude Code", "Bash"),
                language.agents_asks("Codex"),
                language.agents_allow().to_owned(),
                language.agents_deny().to_owned(),
                language.agents_denied_message().to_owned(),
                language.agents_more_waiting(2),
                language.agents_sessions().to_owned(),
                language.agents_usage().to_owned(),
                language.agents_refresh().to_owned(),
                language.agents_updated("just now"),
                language.agents_refresh_failed().to_owned(),
                language.agents_state(State::Waiting).to_owned(),
                language.agents_state(State::Working).to_owned(),
                language.agents_state(State::Failed).to_owned(),
                language.agents_state(State::Done).to_owned(),
                language.agents_state(State::Idle).to_owned(),
                language.agents_empty_title().to_owned(),
                language.agents_empty_hint().to_owned(),
                language.agents_window(300),
                language.agents_window(10_080),
                language.short_duration(7_800),
                language.agents_resets_in(2_280),
                language.agents_reset_already().to_owned(),
                language.agents_as_of("2 days ago"),
                language.shelf_menu_zip().to_owned(),
                language.shelf_menu_convert("PNG"),
                language.shelf_menu_half().to_owned(),
                language.shelf_menu_quarter().to_owned(),
                language.shelf_menu_copy_text().to_owned(),
                language.shelf_text_copied(1),
                language.shelf_text_copied(3),
                language.shelf_no_text().to_owned(),
                language.shelf_image_failed("the image cannot be read"),
                language.shelf_menu_reveal().to_owned(),
                language.shelf_menu_remove().to_owned(),
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
                &labels.search_placeholder_short,
                &labels.clipboard_hint,
                &labels.clipboard_hint_strip,
                &labels.chip_all,
                &labels.chip_pinned,
                &labels.chip_links,
                &labels.chip_code,
                &labels.chip_images,
                &labels.chip_files,
                &labels.chip_text,
                &labels.shelf_hint,
                &labels.shelf_empty,
                &labels.agents_tab,
                &labels.agents_hint,
                &labels.media_tab,
                &labels.media_idle,
                &labels.media_previous,
                &labels.media_play_pause,
                &labels.media_next,
                &labels.media_position,
                &labels.media_hint,
                &labels.media_playing,
                &labels.media_paused,
                &labels.focus_tab,
                &labels.focus_start,
                &labels.focus_pause,
                &labels.focus_resume,
                &labels.focus_reset,
                &labels.focus_ready,
                &labels.focus_running,
                &labels.focus_paused,
                &labels.focus_done,
                &labels.focus_hint,
                &labels.focus_minutes,
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
        assert_ne!(
            en.labels().clipboard_hint_strip,
            vi.labels().clipboard_hint_strip
        );
        assert_ne!(en.labels().shelf_tab, vi.labels().shelf_tab);
        assert_ne!(en.labels().media_idle, vi.labels().media_idle);
        assert_ne!(en.labels().media_play_pause, vi.labels().media_play_pause);
        assert_ne!(en.labels().chip_all, vi.labels().chip_all);
        assert_ne!(en.labels().chip_pinned, vi.labels().chip_pinned);
    }

    #[test]
    fn durations_use_their_two_biggest_units_in_both_languages() {
        let (en, vi) = (Language::English, Language::Vietnamese);
        assert_eq!(en.short_duration(7_800), "2 h 10 min");
        assert_eq!(en.short_duration(2_280), "38 min");
        assert_eq!(en.short_duration(30), "1 min");
        assert_eq!(en.short_duration(4 * 86_400), "4 d");
        assert_eq!(en.short_duration(86_400 + 2 * 3600), "1 d 2 h");
        assert_eq!(vi.short_duration(7_800), "2 giờ 10 phút");
        assert_eq!(vi.short_duration(4 * 86_400), "4 ngày");
        assert_eq!(en.agents_window(299), "5 h");
        assert_eq!(en.agents_window(10_079), "7 d");
        assert_eq!(en.agents_window(43_200), "30 d");
        assert_eq!(vi.agents_window(300), "5 giờ");
    }
}
