//! The words of the Day progress tab, in each language.

use crate::i18n::Language;

impl Language {
    pub fn dayprogress_tab(self) -> &'static str {
        match self {
            Language::English => "Day progress",
            Language::Vietnamese => "Tiến độ ngày",
        }
    }

    pub fn dayprogress_caption(self) -> &'static str {
        match self {
            Language::English => "of the day has gone",
            Language::Vietnamese => "của ngày đã trôi qua",
        }
    }

    /// "8 h 40 min left", "25 min left", "Day done".
    pub fn dayprogress_left(self, hours: i64, minutes: i64) -> String {
        match (self, hours, minutes) {
            (Language::English, 0, 0) => "Day done".into(),
            (Language::Vietnamese, 0, 0) => "Hết ngày".into(),
            (Language::English, 0, m) => format!("{m} min left"),
            (Language::Vietnamese, 0, m) => format!("còn {m} phút"),
            (Language::English, h, 0) => format!("{h} h left"),
            (Language::Vietnamese, h, 0) => format!("còn {h} giờ"),
            (Language::English, h, m) => format!("{h} h {m} min left"),
            (Language::Vietnamese, h, m) => format!("còn {h} giờ {m} phút"),
        }
    }

    /// "7 AM", "1 PM", "12 AM" in English; "7h", "13h" in Vietnamese.
    pub fn dayprogress_hour(self, hour: u32) -> String {
        match self {
            Language::English => {
                let twelve = if hour % 12 == 0 { 12 } else { hour % 12 };
                format!("{twelve} {}", if hour % 24 < 12 { "AM" } else { "PM" })
            }
            Language::Vietnamese => format!("{hour}h"),
        }
    }

    /// "now, until 4 PM"
    pub fn dayprogress_now_until(self, clock: &str) -> String {
        match self {
            Language::English => format!("now, until {clock}"),
            Language::Vietnamese => format!("đang diễn ra, đến {clock}"),
        }
    }

    pub fn dayprogress_nothing_more(self) -> &'static str {
        match self {
            Language::English => "Nothing more today",
            Language::Vietnamese => "Hôm nay hết việc rồi",
        }
    }

    /// "4 events to go, 2 reminders left"; each part is left out when it is none, and so is the whole line.
    pub fn dayprogress_summary(self, events: usize, reminders: usize) -> String {
        let events_text = match (self, events) {
            (_, 0) => None,
            (Language::English, 1) => Some("1 event to go".to_owned()),
            (Language::English, n) => Some(format!("{n} events to go")),
            (Language::Vietnamese, n) => Some(format!("{n} sự kiện nữa")),
        };
        let reminders_text = match (self, reminders) {
            (_, 0) => None,
            (Language::English, 1) => Some("1 reminder left".to_owned()),
            (Language::English, n) => Some(format!("{n} reminders left")),
            (Language::Vietnamese, n) => Some(format!("{n} nhắc việc còn lại")),
        };
        [events_text, reminders_text]
            .into_iter()
            .flatten()
            .collect::<Vec<_>>()
            .join(", ")
    }

    pub fn dayprogress_off_title(self) -> &'static str {
        match self {
            Language::English => "The Day progress tab is off",
            Language::Vietnamese => "Tab Tiến độ ngày đang tắt",
        }
    }

    pub fn dayprogress_off_hint(self) -> &'static str {
        match self {
            Language::English => "Turn it on in Settings, under Notch tabs, Day progress.",
            Language::Vietnamese => "Bật trong Cài đặt, mục Tab của notch, Tiến độ ngày.",
        }
    }

    pub fn dayprogress_access_title(self) -> &'static str {
        match self {
            Language::English => "The calendar is not allowed",
            Language::Vietnamese => "Chưa được phép đọc lịch",
        }
    }

    pub fn dayprogress_access_hint(self) -> &'static str {
        match self {
            Language::English => "Allow it in Settings, under Notch tabs, Calendar.",
            Language::Vietnamese => "Hãy cho phép trong Cài đặt, mục Tab của notch, Lịch.",
        }
    }
}
