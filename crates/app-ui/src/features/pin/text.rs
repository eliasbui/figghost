//! The words this feature puts on screen outside the settings page: the name of its switch in the menu bar panel
//! and the notices that say why a window was not pinned. A notice is one short line.

use crate::i18n::Language;

impl Language {
    pub fn pin_menu(self) -> &'static str {
        match self {
            Language::English => "Keep window on top",
            Language::Vietnamese => "Giữ cửa sổ trên cùng",
        }
    }

    /// Stands for a pinned window that has no name, on the settings page.
    pub fn pin_unnamed(self) -> &'static str {
        match self {
            Language::English => "A window",
            Language::Vietnamese => "Một cửa sổ",
        }
    }

    pub fn pin_needs_accessibility(self) -> &'static str {
        match self {
            Language::English => "Allow Accessibility in System Settings to keep a window on top.",
            Language::Vietnamese => {
                "Hãy cho phép Trợ năng trong Cài đặt hệ thống để giữ cửa sổ trên cùng."
            }
        }
    }

    pub fn pin_needs_screen_recording(self) -> &'static str {
        match self {
            Language::English => {
                "Allow Screen Recording in System Settings to keep a window on top."
            }
            Language::Vietnamese => {
                "Hãy cho phép Ghi màn hình trong Cài đặt hệ thống để giữ cửa sổ trên cùng."
            }
        }
    }

    pub fn pin_needs_newer_macos(self) -> &'static str {
        match self {
            Language::English => "Keeping a window on top needs macOS 14 or later.",
            Language::Vietnamese => "Giữ cửa sổ trên cùng cần macOS 14 trở lên.",
        }
    }

    pub fn pin_no_window(self) -> &'static str {
        match self {
            Language::English => "There is no window in front to keep on top.",
            Language::Vietnamese => "Không có cửa sổ nào ở phía trước để giữ trên cùng.",
        }
    }

    pub fn pin_own_window(self) -> &'static str {
        match self {
            Language::English => "A window of Figghost cannot be kept on top.",
            Language::Vietnamese => "Không giữ được cửa sổ của Figghost trên cùng.",
        }
    }

    pub fn pin_full_screen(self) -> &'static str {
        match self {
            Language::English => "A full-screen window already covers everything.",
            Language::Vietnamese => "Cửa sổ toàn màn hình vốn đã che mọi thứ.",
        }
    }

    pub fn pin_cannot(self) -> &'static str {
        match self {
            Language::English => "This window cannot be kept on top.",
            Language::Vietnamese => "Không giữ được cửa sổ này trên cùng.",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_notice_is_one_short_line_in_both_languages() {
        for language in Language::ALL {
            for text in [
                language.pin_menu(),
                language.pin_unnamed(),
                language.pin_needs_accessibility(),
                language.pin_needs_screen_recording(),
                language.pin_needs_newer_macos(),
                language.pin_no_window(),
                language.pin_own_window(),
                language.pin_full_screen(),
                language.pin_cannot(),
            ] {
                // A notice is drawn on one line of a fixed width, so a long one would be cut.
                assert!(
                    !text.is_empty() && text.chars().count() <= 80 && !text.contains('\n'),
                    "{language:?}: {text}"
                );
            }
        }
    }
}
