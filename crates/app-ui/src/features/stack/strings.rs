//! The words this feature shows outside the settings page, in both languages.

use crate::i18n::Language;

impl Language {
    /// Marks a row in the list whose text is hidden.
    pub fn sensitive_item(self) -> &'static str {
        match self {
            Language::English => "Sensitive",
            Language::Vietnamese => "Nhạy cảm",
        }
    }

    /// Title of the menu that moves an item to a board.
    pub fn board_menu_title(self) -> &'static str {
        match self {
            Language::English => "Move to board",
            Language::Vietnamese => "Chuyển vào bảng ghim",
        }
    }

    /// The menu choice that takes an item out of its board.
    pub fn no_board(self) -> &'static str {
        match self {
            Language::English => "No board",
            Language::Vietnamese => "Không có bảng",
        }
    }

    /// What the pill says while a copy stack runs.
    pub fn stack_pill(self, queued: usize) -> String {
        match (self, queued) {
            (Language::English, 0) => "Copy Stack: nothing yet".into(),
            (Language::English, n) => format!("Copy Stack: {n} to paste"),
            (Language::Vietnamese, 0) => "Copy Stack: chưa có mục nào".into(),
            (Language::Vietnamese, n) => format!("Copy Stack: {n} mục chờ dán"),
        }
    }

    /// Menu bar item that ends a running stack.
    pub fn stack_end_menu(self, queued: usize) -> String {
        match self {
            Language::English => format!("End Copy Stack ({queued} queued)"),
            Language::Vietnamese => format!("Kết thúc Copy Stack ({queued} mục chờ)"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_string_exists_in_both_languages() {
        for language in Language::ALL {
            let all = [
                language.sensitive_item().to_owned(),
                language.board_menu_title().to_owned(),
                language.no_board().to_owned(),
                language.stack_pill(0),
                language.stack_pill(1),
                language.stack_pill(12),
                language.stack_end_menu(0),
                language.stack_end_menu(3),
            ];
            for text in all {
                assert!(!text.trim().is_empty(), "{language:?}");
            }
        }
        let (en, vi) = (Language::English, Language::Vietnamese);
        assert_ne!(en.sensitive_item(), vi.sensitive_item());
        assert_ne!(en.board_menu_title(), vi.board_menu_title());
        assert_ne!(en.no_board(), vi.no_board());
        assert_ne!(en.stack_pill(2), vi.stack_pill(2));
        assert!(vi.stack_pill(2).contains('2') && en.stack_pill(2).contains('2'));
        assert!(en.stack_end_menu(4).contains('4') && vi.stack_end_menu(4).contains('4'));
    }

    #[test]
    fn the_pill_keeps_the_count_visible() {
        for language in Language::ALL {
            assert_ne!(language.stack_pill(1), language.stack_pill(2));
            assert_ne!(language.stack_pill(0), language.stack_pill(1));
        }
    }
}
