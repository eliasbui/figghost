//! The words this feature puts on screen outside the settings page: the arrow's tooltip and the menu entry.

use crate::i18n::Language;

impl Language {
    pub fn menubar_show(self) -> &'static str {
        match self {
            Language::English => "Show hidden menu bar icons",
            Language::Vietnamese => "Hiện các biểu tượng thanh menu đã ẩn",
        }
    }

    pub fn menubar_hide(self) -> &'static str {
        match self {
            Language::English => "Hide menu bar icons",
            Language::Vietnamese => "Ẩn các biểu tượng thanh menu",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_string_exists_in_both_languages() {
        for language in Language::ALL {
            for text in [language.menubar_show(), language.menubar_hide()] {
                assert!(!text.trim().is_empty(), "{language:?}");
            }
        }
        assert_ne!(
            Language::English.menubar_show(),
            Language::Vietnamese.menubar_show()
        );
    }
}
