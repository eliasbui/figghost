//! The words of the calendar page of the notch, in each language. Dates and times come from
//! `core_calendar::format`; these are the labels around them.

use crate::i18n::Language;
use core_calendar::format::Lang;

impl Language {
    pub fn calendar_lang(self) -> Lang {
        match self {
            Language::English => Lang::En,
            Language::Vietnamese => Lang::Vi,
        }
    }

    pub fn calendar_tab(self) -> &'static str {
        match self {
            Language::English => "Calendar",
            Language::Vietnamese => "Lịch",
        }
    }

    pub fn calendar_hint(self) -> &'static str {
        match self {
            Language::English => "Return: join    Opt+Delete: dismiss",
            Language::Vietnamese => "Return: tham gia    Opt+Delete: bỏ qua",
        }
    }

    pub fn calendar_starting_soon(self) -> &'static str {
        match self {
            Language::English => "Starting soon",
            Language::Vietnamese => "Sắp bắt đầu",
        }
    }

    pub fn calendar_started(self) -> &'static str {
        match self {
            Language::English => "Meeting started",
            Language::Vietnamese => "Cuộc họp đã bắt đầu",
        }
    }

    pub fn calendar_dismiss(self) -> &'static str {
        match self {
            Language::English => "Dismiss",
            Language::Vietnamese => "Bỏ qua",
        }
    }

    pub fn calendar_join(self) -> &'static str {
        match self {
            Language::English => "Join",
            Language::Vietnamese => "Tham gia",
        }
    }

    pub fn calendar_join_service(self, service: &str) -> String {
        match self {
            Language::English => format!("Join {service}"),
            Language::Vietnamese => format!("Tham gia {service}"),
        }
    }

    pub fn calendar_complete(self) -> &'static str {
        match self {
            Language::English => "Complete",
            Language::Vietnamese => "Hoàn thành",
        }
    }

    pub fn calendar_reminders(self) -> &'static str {
        match self {
            Language::English => "Reminders",
            Language::Vietnamese => "Lời nhắc",
        }
    }

    /// `{n}` stands for how many rows did not fit.
    pub fn calendar_more(self) -> &'static str {
        match self {
            Language::English => "+{n} more",
            Language::Vietnamese => "+{n} mục nữa",
        }
    }

    pub fn calendar_more_alerts(self, n: usize) -> String {
        match self {
            Language::English if n == 1 => "1 more alert".to_owned(),
            Language::English => format!("{n} more alerts"),
            Language::Vietnamese => format!("Còn {n} lời báo nữa"),
        }
    }

    pub fn calendar_untitled(self) -> &'static str {
        match self {
            Language::English => "Untitled",
            Language::Vietnamese => "Không có tiêu đề",
        }
    }

    pub fn calendar_off_title(self) -> &'static str {
        match self {
            Language::English => "Calendar is off",
            Language::Vietnamese => "Lịch đang tắt",
        }
    }

    pub fn calendar_off_hint(self) -> &'static str {
        match self {
            Language::English => "Turn it on in Settings, under Calendar.",
            Language::Vietnamese => "Hãy bật trong Cài đặt, mục Lịch.",
        }
    }

    pub fn calendar_ask_title(self) -> &'static str {
        match self {
            Language::English => "Allow access to your calendar",
            Language::Vietnamese => "Cho phép truy cập lịch",
        }
    }

    pub fn calendar_ask_hint(self) -> &'static str {
        match self {
            Language::English => {
                "Figghost reads the events and reminders on this Mac and shows them here. Nothing leaves it."
            }
            Language::Vietnamese => {
                "Figghost đọc sự kiện và lời nhắc trên máy này và hiển thị ở đây. Không có gì được gửi đi."
            }
        }
    }

    pub fn calendar_ask_action(self) -> &'static str {
        match self {
            Language::English => "Allow access",
            Language::Vietnamese => "Cho phép truy cập",
        }
    }

    pub fn calendar_denied_title(self) -> &'static str {
        match self {
            Language::English => "Calendar access is off",
            Language::Vietnamese => "Chưa có quyền truy cập lịch",
        }
    }

    pub fn calendar_denied_hint(self) -> &'static str {
        match self {
            Language::English => {
                "Allow Figghost in System Settings, under Privacy & Security, Calendars."
            }
            Language::Vietnamese => {
                "Hãy cho phép Figghost trong Cài đặt hệ thống, mục Quyền riêng tư và bảo mật, Lịch."
            }
        }
    }

    pub fn calendar_write_only_hint(self) -> &'static str {
        match self {
            Language::English => {
                "Figghost may only add events. It needs full access to show them. Change it in System Settings."
            }
            Language::Vietnamese => {
                "Figghost chỉ được thêm sự kiện. Cần quyền truy cập đầy đủ để hiển thị. Hãy đổi trong Cài đặt hệ thống."
            }
        }
    }

    pub fn calendar_open_settings(self) -> &'static str {
        match self {
            Language::English => "Open System Settings",
            Language::Vietnamese => "Mở Cài đặt hệ thống",
        }
    }

    pub fn calendar_restricted_title(self) -> &'static str {
        match self {
            Language::English => "Calendar access is restricted",
            Language::Vietnamese => "Quyền truy cập lịch bị hạn chế",
        }
    }

    pub fn calendar_restricted_hint(self) -> &'static str {
        match self {
            Language::English => "A profile or Screen Time on this Mac does not allow it.",
            Language::Vietnamese => "Một hồ sơ hoặc Thời gian sử dụng trên máy này không cho phép.",
        }
    }

    pub fn calendar_no_prompt_hint(self) -> &'static str {
        match self {
            Language::English => {
                "This build has no calendar permission text, so macOS cannot ask. Use the packaged app."
            }
            Language::Vietnamese => {
                "Bản này không có nội dung xin quyền lịch nên macOS không thể hỏi. Hãy dùng ứng dụng đã đóng gói."
            }
        }
    }

    pub fn calendar_empty_title(self) -> &'static str {
        match self {
            Language::English => "Nothing planned",
            Language::Vietnamese => "Không có lịch",
        }
    }

    pub fn calendar_empty_hint(self) -> &'static str {
        match self {
            Language::English => "No events or reminders for today and tomorrow.",
            Language::Vietnamese => "Không có sự kiện hay lời nhắc nào cho hôm nay và ngày mai.",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn no_string_is_empty_in_either_language() {
        for language in Language::ALL {
            let all = [
                language.calendar_tab(),
                language.calendar_hint(),
                language.calendar_starting_soon(),
                language.calendar_started(),
                language.calendar_dismiss(),
                language.calendar_join(),
                language.calendar_complete(),
                language.calendar_reminders(),
                language.calendar_more(),
                language.calendar_untitled(),
                language.calendar_off_title(),
                language.calendar_off_hint(),
                language.calendar_ask_title(),
                language.calendar_ask_hint(),
                language.calendar_ask_action(),
                language.calendar_denied_title(),
                language.calendar_denied_hint(),
                language.calendar_write_only_hint(),
                language.calendar_open_settings(),
                language.calendar_restricted_title(),
                language.calendar_restricted_hint(),
                language.calendar_no_prompt_hint(),
                language.calendar_empty_title(),
                language.calendar_empty_hint(),
            ];
            for text in all {
                assert!(!text.trim().is_empty(), "{language:?}");
            }
            assert!(language.calendar_more().contains("{n}"));
            assert!(language.calendar_join_service("Zoom").contains("Zoom"));
            assert!(language.calendar_more_alerts(3).contains('3'));
        }
    }

    #[test]
    fn the_two_languages_differ() {
        assert_ne!(
            Language::English.calendar_tab(),
            Language::Vietnamese.calendar_tab()
        );
        assert_ne!(
            Language::English.calendar_ask_hint(),
            Language::Vietnamese.calendar_ask_hint()
        );
    }
}
