//! The words of the Pomodoro tab, in each language.

use super::logic::Stage;
use crate::i18n::Language;

impl Language {
    pub fn pomodoro_tab(self) -> &'static str {
        match self {
            Language::English => "Pomodoro",
            Language::Vietnamese => "Pomodoro",
        }
    }

    pub fn pomodoro_stage(self, stage: Stage) -> &'static str {
        match (self, stage) {
            (Language::English, Stage::Work) => "Focus",
            (Language::English, Stage::ShortBreak) => "Short break",
            (Language::English, Stage::LongBreak) => "Long break",
            (Language::Vietnamese, Stage::Work) => "Tập trung",
            (Language::Vietnamese, Stage::ShortBreak) => "Nghỉ ngắn",
            (Language::Vietnamese, Stage::LongBreak) => "Nghỉ dài",
        }
    }

    pub fn pomodoro_running(self) -> &'static str {
        match self {
            Language::English => "Running",
            Language::Vietnamese => "Đang chạy",
        }
    }

    pub fn pomodoro_paused(self) -> &'static str {
        match self {
            Language::English => "Paused",
            Language::Vietnamese => "Tạm dừng",
        }
    }

    pub fn pomodoro_ready(self) -> &'static str {
        match self {
            Language::English => "Ready",
            Language::Vietnamese => "Sẵn sàng",
        }
    }

    /// "Session 2 of 4": the work session that is on or comes next.
    pub fn pomodoro_session(self, number: u32, rounds: u32) -> String {
        match self {
            Language::English => format!("Session {number} of {rounds}"),
            Language::Vietnamese => format!("Phiên {number} trên {rounds}"),
        }
    }

    pub fn pomodoro_today(self) -> &'static str {
        match self {
            Language::English => "Today",
            Language::Vietnamese => "Hôm nay",
        }
    }

    /// "3 sessions, 75 min"
    pub fn pomodoro_done(self, sessions: u32, minutes: u32) -> String {
        match (self, sessions) {
            (Language::English, 0) => "No sessions yet".into(),
            (Language::English, 1) => format!("1 session, {minutes} min"),
            (Language::English, n) => format!("{n} sessions, {minutes} min"),
            (Language::Vietnamese, 0) => "Chưa có phiên nào".into(),
            (Language::Vietnamese, n) => format!("{n} phiên, {minutes} phút"),
        }
    }

    pub fn pomodoro_streak(self) -> &'static str {
        match self {
            Language::English => "Streak",
            Language::Vietnamese => "Chuỗi ngày",
        }
    }

    pub fn pomodoro_days(self, days: u32) -> String {
        match (self, days) {
            (Language::English, 0) => "None yet".into(),
            (Language::English, 1) => "1 day".into(),
            (Language::English, n) => format!("{n} days"),
            (Language::Vietnamese, 0) => "Chưa có".into(),
            (Language::Vietnamese, n) => format!("{n} ngày"),
        }
    }

    /// The initial of a day of the week, 0 for Sunday, for the bars of the week.
    pub fn pomodoro_weekday(self, day: usize) -> &'static str {
        const EN: [&str; 7] = ["S", "M", "T", "W", "T", "F", "S"];
        const VI: [&str; 7] = ["CN", "T2", "T3", "T4", "T5", "T6", "T7"];
        match self {
            Language::English => EN[day % 7],
            Language::Vietnamese => VI[day % 7],
        }
    }

    pub fn pomodoro_start(self) -> &'static str {
        match self {
            Language::English => "Start",
            Language::Vietnamese => "Bắt đầu",
        }
    }

    pub fn pomodoro_pause(self) -> &'static str {
        match self {
            Language::English => "Pause",
            Language::Vietnamese => "Tạm dừng",
        }
    }

    pub fn pomodoro_resume(self) -> &'static str {
        match self {
            Language::English => "Resume",
            Language::Vietnamese => "Tiếp tục",
        }
    }

    pub fn pomodoro_skip(self) -> &'static str {
        match self {
            Language::English => "Skip to the next stage",
            Language::Vietnamese => "Chuyển sang giai đoạn kế",
        }
    }

    pub fn pomodoro_reset(self) -> &'static str {
        match self {
            Language::English => "Start over from the first session",
            Language::Vietnamese => "Bắt đầu lại từ phiên đầu",
        }
    }

    pub fn pomodoro_off_title(self) -> &'static str {
        match self {
            Language::English => "The Pomodoro tab is off",
            Language::Vietnamese => "Tab Pomodoro đang tắt",
        }
    }

    pub fn pomodoro_off_hint(self) -> &'static str {
        match self {
            Language::English => "Turn it on in Settings, under Notch tabs, Pomodoro.",
            Language::Vietnamese => "Bật trong Cài đặt, mục Tab của notch, Pomodoro.",
        }
    }

    pub fn pomodoro_menu_start(self) -> &'static str {
        match self {
            Language::English => "Start the Pomodoro",
            Language::Vietnamese => "Bắt đầu Pomodoro",
        }
    }

    pub fn pomodoro_menu_pause(self, left: &str) -> String {
        match self {
            Language::English => format!("Pause the Pomodoro ({left} left)"),
            Language::Vietnamese => format!("Tạm dừng Pomodoro (còn {left})"),
        }
    }

    pub fn pomodoro_menu_resume(self) -> &'static str {
        match self {
            Language::English => "Resume the Pomodoro",
            Language::Vietnamese => "Tiếp tục Pomodoro",
        }
    }
}
