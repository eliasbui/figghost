//! Every string of this feature that the Rust side shows, in both languages.

use crate::i18n::Language;

impl Language {
    /// The keys of the loupe, in one short line.
    pub fn vision_hint(self) -> &'static str {
        match self {
            Language::English => "Tab format   Click copy   Esc exit",
            Language::Vietnamese => "Tab đổi dạng   Nhấp để chép   Esc thoát",
        }
    }

    /// Replaces the colour in the loupe while Screen Recording is not allowed.
    pub fn vision_no_access(self) -> &'static str {
        match self {
            Language::English => "Allow Screen Recording in System Settings.",
            Language::Vietnamese => "Hãy cho phép Ghi màn hình trong Cài đặt hệ thống.",
        }
    }

    /// Shown instead of the loupe on a system without the screenshots' frozen picture (before macOS 14).
    pub fn vision_needs_newer_macos(self) -> &'static str {
        match self {
            Language::English => "The colour picker needs macOS 14 or later",
            Language::Vietnamese => "Chọn màu cần macOS 14 trở lên",
        }
    }

    pub fn vision_menu_pick(self) -> &'static str {
        match self {
            Language::English => "Pick a colour",
            Language::Vietnamese => "Chọn màu",
        }
    }

    /// The shelf's right-click item for an image.
    pub fn vision_remove_background(self) -> &'static str {
        match self {
            Language::English => "Remove background",
            Language::Vietnamese => "Xóa nền",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn no_string_is_empty_in_any_language() {
        for language in Language::ALL {
            for text in [
                language.vision_hint(),
                language.vision_no_access(),
                language.vision_needs_newer_macos(),
                language.vision_menu_pick(),
                language.vision_remove_background(),
            ] {
                assert!(!text.trim().is_empty(), "{language:?}");
            }
        }
    }

    #[test]
    fn vietnamese_is_written_with_its_diacritics() {
        let l = Language::Vietnamese;
        for text in [
            l.vision_hint(),
            l.vision_no_access(),
            l.vision_needs_newer_macos(),
            l.vision_menu_pick(),
            l.vision_remove_background(),
        ] {
            assert!(!text.is_ascii(), "{text}");
        }
        assert_ne!(l.vision_hint(), Language::English.vision_hint());
    }
}
