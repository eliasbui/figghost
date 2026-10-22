//! The words of the menu bar entry, in both languages.

use crate::i18n::Language;

impl Language {
    pub fn mole_menu(self) -> &'static str {
        match self {
            Language::English => "Clean up this Mac",
            Language::Vietnamese => "Dọn dẹp máy Mac",
        }
    }
}
