//! The words of the video download feature that the app itself says, in each language. The page has its own.

use crate::i18n::Language;

impl Language {
    pub fn download_menu(self) -> &'static str {
        match self {
            Language::English => "Download video from copied link",
            Language::Vietnamese => "Tải video từ liên kết đã sao chép",
        }
    }

    /// On the closed notch, in the place of the percentage, while the picture and the sound are made into one file.
    pub fn download_joining(self) -> &'static str {
        match self {
            Language::English => "Joining",
            Language::Vietnamese => "Đang ghép",
        }
    }

    pub fn download_pick_folder(self) -> &'static str {
        match self {
            Language::English => "Choose the folder for downloaded videos",
            Language::Vietnamese => "Chọn thư mục lưu video đã tải",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_word_is_there_in_both_languages() {
        for language in Language::ALL {
            assert!(!language.download_menu().is_empty());
            assert!(!language.download_joining().is_empty());
            assert!(language.download_joining().chars().count() <= 14);
            assert!(!language.download_pick_folder().is_empty());
        }
    }
}
