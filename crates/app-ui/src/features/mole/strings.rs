//! The words of the menu bar entries, in both languages.

use crate::i18n::Language;

impl Language {
    pub fn mole_menu(self) -> [&'static str; 6] {
        match self {
            Language::English => [
                "Mole",
                "Preview clean",
                "Preview optimize",
                "Preview project leftovers",
                "Check health",
                "Open Mole",
            ],
            Language::Vietnamese => [
                "Mole",
                "Xem trước dọn dẹp",
                "Xem trước tối ưu",
                "Xem trước thư mục build cũ",
                "Kiểm tra sức khỏe",
                "Mở Mole",
            ],
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_entry_is_named_in_both_languages() {
        let (en, vi) = (
            Language::English.mole_menu(),
            Language::Vietnamese.mole_menu(),
        );
        for (a, b) in en.iter().zip(&vi) {
            assert!(!a.is_empty() && !b.is_empty());
        }
        assert_ne!(en[1], vi[1]);
    }
}
