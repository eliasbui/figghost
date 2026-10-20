//! The words of the Pull requests tab, in each language.

use super::logic::Host;
use crate::i18n::Language;

impl Language {
    pub fn prs_tab(self) -> &'static str {
        match self {
            Language::English => "Pull requests",
            Language::Vietnamese => "Yêu cầu duyệt",
        }
    }

    pub fn prs_heading(self, count: usize) -> String {
        match (self, count) {
            (Language::English, 1) => "1 request waits for your review".into(),
            (Language::English, n) => format!("{n} requests wait for your review"),
            (Language::Vietnamese, n) => format!("{n} yêu cầu đang chờ bạn duyệt"),
        }
    }

    pub fn prs_host(self, host: Host) -> &'static str {
        match host {
            Host::GitHub => "GitHub",
            Host::GitLab => "GitLab",
        }
    }

    pub fn prs_draft(self) -> &'static str {
        match self {
            Language::English => "Draft",
            Language::Vietnamese => "Bản nháp",
        }
    }

    pub fn prs_updated(self, ago: &str) -> String {
        match self {
            Language::English => format!("Updated {ago}"),
            Language::Vietnamese => format!("Cập nhật {ago}"),
        }
    }

    pub fn prs_checking(self) -> &'static str {
        match self {
            Language::English => "Looking...",
            Language::Vietnamese => "Đang xem...",
        }
    }

    /// What is said next to the status when one of two sources could not be read.
    pub fn prs_source_problem(self, host: Host) -> String {
        let name = self.prs_host(host);
        match self {
            Language::English => format!("{name} could not be read"),
            Language::Vietnamese => format!("Không đọc được {name}"),
        }
    }

    pub fn prs_refresh(self) -> &'static str {
        match self {
            Language::English => "Look again",
            Language::Vietnamese => "Xem lại",
        }
    }

    pub fn prs_off_title(self) -> &'static str {
        match self {
            Language::English => "The Pull requests tab is off",
            Language::Vietnamese => "Tab Yêu cầu duyệt đang tắt",
        }
    }

    pub fn prs_off_hint(self) -> &'static str {
        match self {
            Language::English => "Turn it on in Settings, under Notch tabs, Pull requests.",
            Language::Vietnamese => "Bật trong Cài đặt, mục Tab của notch, Yêu cầu duyệt.",
        }
    }

    pub fn prs_nothing_title(self) -> &'static str {
        match self {
            Language::English => "Nothing waits for your review",
            Language::Vietnamese => "Không có gì đang chờ bạn duyệt",
        }
    }

    pub fn prs_nothing_hint(self) -> &'static str {
        match self {
            Language::English => {
                "A new request shows up here, and a dot on the tab's icon says so."
            }
            Language::Vietnamese => {
                "Yêu cầu mới sẽ hiện ở đây, và một chấm trên biểu tượng của tab báo cho bạn biết."
            }
        }
    }

    pub fn prs_loading_title(self) -> &'static str {
        match self {
            Language::English => "Looking for review requests",
            Language::Vietnamese => "Đang tìm yêu cầu duyệt",
        }
    }

    pub fn prs_loading_hint(self) -> &'static str {
        match self {
            Language::English => "One moment.",
            Language::Vietnamese => "Chờ một chút.",
        }
    }

    pub fn prs_no_source_title(self) -> &'static str {
        match self {
            Language::English => "No place to look",
            Language::Vietnamese => "Chưa chọn nơi để xem",
        }
    }

    pub fn prs_no_source_hint(self) -> &'static str {
        match self {
            Language::English => {
                "Choose GitHub, GitLab or both in Settings, under Notch tabs, Pull requests."
            }
            Language::Vietnamese => {
                "Chọn GitHub, GitLab hoặc cả hai trong Cài đặt, mục Tab của notch, Yêu cầu duyệt."
            }
        }
    }

    pub fn prs_missing_title(self) -> &'static str {
        match self {
            Language::English => "No command-line tool found",
            Language::Vietnamese => "Không thấy công cụ dòng lệnh",
        }
    }

    pub fn prs_missing_hint(self) -> &'static str {
        match self {
            Language::English => "Install gh for GitHub or glab for GitLab, and sign in with it.",
            Language::Vietnamese => {
                "Cài gh cho GitHub hoặc glab cho GitLab, rồi đăng nhập bằng nó."
            }
        }
    }

    pub fn prs_failed_title(self) -> &'static str {
        match self {
            Language::English => "Could not read your review requests",
            Language::Vietnamese => "Không đọc được yêu cầu duyệt của bạn",
        }
    }

    pub fn prs_failed_hint(self) -> &'static str {
        match self {
            Language::English => {
                "Is gh or glab signed in? Run gh auth login or glab auth login in a terminal."
            }
            Language::Vietnamese => {
                "gh hoặc glab đã đăng nhập chưa? Chạy gh auth login hoặc glab auth login trong terminal."
            }
        }
    }

    pub fn prs_slow_title(self) -> &'static str {
        match self {
            Language::English => "No answer in time",
            Language::Vietnamese => "Không trả lời kịp",
        }
    }

    pub fn prs_slow_hint(self) -> &'static str {
        match self {
            Language::English => "GitHub or GitLab did not answer. Press Look again in a moment.",
            Language::Vietnamese => {
                "GitHub hoặc GitLab chưa trả lời. Hãy bấm Xem lại sau giây lát."
            }
        }
    }
}
