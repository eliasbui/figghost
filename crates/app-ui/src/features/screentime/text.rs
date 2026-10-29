//! The words of the Screen Time tab, in each language.

use crate::i18n::Language;

/// The groups an app can be put into, by what it declares: the code, the glyph, and the name in each language.
const GROUPS: [(&str, &str, &str, &str); 21] = [
    ("business", "briefcase", "Business", "Kinh doanh"),
    (
        "developer-tools",
        "hammer",
        "Developer tools",
        "Công cụ lập trình",
    ),
    ("education", "graduationcap", "Education", "Giáo dục"),
    ("entertainment", "play.tv", "Entertainment", "Giải trí"),
    (
        "finance",
        "chart.line.uptrend.xyaxis",
        "Finance",
        "Tài chính",
    ),
    ("games", "gamecontroller", "Games", "Trò chơi"),
    (
        "graphics-design",
        "paintpalette",
        "Graphics and design",
        "Đồ họa",
    ),
    (
        "healthcare-fitness",
        "heart",
        "Health and fitness",
        "Sức khỏe",
    ),
    ("lifestyle", "leaf", "Lifestyle", "Đời sống"),
    ("medical", "cross.case", "Medical", "Y tế"),
    ("music", "music.note", "Music", "Âm nhạc"),
    ("news", "newspaper", "News", "Tin tức"),
    ("photography", "camera", "Photography", "Nhiếp ảnh"),
    (
        "productivity",
        "checkmark.circle",
        "Productivity",
        "Năng suất",
    ),
    ("reference", "book", "Reference", "Tra cứu"),
    (
        "social-networking",
        "bubble.left.and.bubble.right",
        "Social networking",
        "Mạng xã hội",
    ),
    ("sports", "sportscourt", "Sports", "Thể thao"),
    ("travel", "airplane", "Travel", "Du lịch"),
    (
        "utilities",
        "wrench.and.screwdriver",
        "Utilities",
        "Tiện ích",
    ),
    ("video", "video", "Video", "Video"),
    ("weather", "cloud.sun", "Weather", "Thời tiết"),
];

/// The glyph for a group code; a question mark for one that is not known.
/// The glyph of a row of the list by site.
pub const SITE_SYMBOL: &str = "globe";

pub fn group_symbol(code: &str) -> &'static str {
    GROUPS
        .iter()
        .find(|g| g.0 == code)
        .map_or("questionmark.circle", |g| g.1)
}

/// Every glyph the tab can draw, for the test that checks the system knows them.
#[cfg(test)]
pub fn all_group_symbols() -> Vec<&'static str> {
    GROUPS
        .iter()
        .map(|g| g.1)
        .chain(["questionmark.circle", SITE_SYMBOL])
        .collect()
}

impl Language {
    pub fn screentime_tab(self) -> &'static str {
        match self {
            Language::English => "Screen Time",
            Language::Vietnamese => "Thời gian dùng",
        }
    }

    /// "3 h 42 min", "42 min", "less than a minute"; nothing at all is "0 min".
    pub fn screentime_duration(self, seconds: u64) -> String {
        let (hours, minutes) = super::logic::hours_minutes(seconds);
        match (self, hours, minutes) {
            (Language::English, 0, 0) if seconds > 0 => "<1 min".into(),
            (Language::Vietnamese, 0, 0) if seconds > 0 => "<1 phút".into(),
            (Language::English, 0, m) => format!("{m} min"),
            (Language::Vietnamese, 0, m) => format!("{m} phút"),
            (Language::English, h, m) => format!("{h} h {m} min"),
            (Language::Vietnamese, h, m) => format!("{h} giờ {m} phút"),
        }
    }

    pub fn screentime_group(self, code: &str) -> &'static str {
        match GROUPS.iter().find(|g| g.0 == code) {
            Some(group) => match self {
                Language::English => group.2,
                Language::Vietnamese => group.3,
            },
            None => match self {
                Language::English => "Other",
                Language::Vietnamese => "Khác",
            },
        }
    }

    pub fn screentime_today(self) -> &'static str {
        match self {
            Language::English => "Today",
            Language::Vietnamese => "Hôm nay",
        }
    }

    pub fn screentime_week(self) -> &'static str {
        match self {
            Language::English => "7 days",
            Language::Vietnamese => "7 ngày",
        }
    }

    pub fn screentime_week_caption(self) -> &'static str {
        match self {
            Language::English => "Last 7 days",
            Language::Vietnamese => "7 ngày qua",
        }
    }

    pub fn screentime_apps(self) -> &'static str {
        match self {
            Language::English => "Apps",
            Language::Vietnamese => "Ứng dụng",
        }
    }

    pub fn screentime_groups(self) -> &'static str {
        match self {
            Language::English => "Groups",
            Language::Vietnamese => "Nhóm",
        }
    }

    pub fn screentime_sites(self) -> &'static str {
        match self {
            Language::English => "Sites",
            Language::Vietnamese => "Trang web",
        }
    }

    pub fn screentime_sites_empty(self) -> &'static str {
        match self {
            Language::English => {
                "No site yet. It counts the site of the front tab of Safari or a Chromium browser."
            }
            Language::Vietnamese => {
                "Chưa có trang nào. Nó tính trang của tab đang mở trong Safari hoặc trình duyệt Chromium."
            }
        }
    }

    pub fn screentime_empty(self) -> &'static str {
        match self {
            Language::English => {
                "Nothing yet. It counts the app in front while you are at the Mac."
            }
            Language::Vietnamese => {
                "Chưa có gì. Nó tính thời gian ứng dụng đang ở phía trước khi bạn dùng máy."
            }
        }
    }

    pub fn screentime_off_title(self) -> &'static str {
        match self {
            Language::English => "The Screen Time tab is off",
            Language::Vietnamese => "Tab Thời gian dùng đang tắt",
        }
    }

    pub fn screentime_off_hint(self) -> &'static str {
        match self {
            Language::English => "Turn it on in Settings, under Notch tabs, Screen Time.",
            Language::Vietnamese => "Bật trong Cài đặt, mục Tab của notch, Thời gian dùng.",
        }
    }
}
