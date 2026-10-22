//! The words of Vietnamese typing in the menu bar panel and on the notch.

use crate::i18n::Language;

impl Language {
    pub fn typing_menu(self) -> &'static str {
        match self {
            Language::English => "Vietnamese typing",
            Language::Vietnamese => "Gõ tiếng Việt",
        }
    }

    /// The notice on the notch when the language of typing changes.
    pub fn typing_now(self, vietnamese: bool) -> &'static str {
        match (self, vietnamese) {
            (Language::English, true) => "Vietnamese",
            (Language::Vietnamese, true) => "Tiếng Việt",
            (Language::English, false) => "English",
            (Language::Vietnamese, false) => "Tiếng Anh",
        }
    }

    pub fn typing_now_detail(self, vietnamese: bool, method: &str) -> String {
        match (self, vietnamese) {
            (Language::English, true) => format!("Typing with {method}"),
            (Language::Vietnamese, true) => format!("Gõ kiểu {method}"),
            (Language::English, false) => "Keys go on as they are".into(),
            (Language::Vietnamese, false) => "Phím được giữ nguyên".into(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_word_is_there_in_both_languages() {
        for language in Language::ALL {
            assert!(!language.typing_menu().is_empty());
            for vietnamese in [true, false] {
                assert!(!language.typing_now(vietnamese).is_empty());
                assert!(!language.typing_now_detail(vietnamese, "Telex").is_empty());
            }
        }
        assert_eq!(
            Language::Vietnamese.typing_now_detail(true, "VNI"),
            "Gõ kiểu VNI"
        );
    }
}
