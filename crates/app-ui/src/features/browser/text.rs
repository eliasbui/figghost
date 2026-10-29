//! The words the browser bridge says on the notch.

use crate::i18n::Language;

impl Language {
    pub fn browser_download_failed(self) -> &'static str {
        match self {
            Language::English => "The browser's video was not downloaded",
            Language::Vietnamese => "Không tải được video từ trình duyệt",
        }
    }

    pub fn browser_why_off(self) -> &'static str {
        match self {
            Language::English => "Turn on Video download in Settings",
            Language::Vietnamese => "Bật Tải video trong Cài đặt",
        }
    }

    pub fn browser_why_missing(self) -> &'static str {
        match self {
            Language::English => "Install yt-dlp first",
            Language::Vietnamese => "Cần cài yt-dlp trước",
        }
    }

    pub fn browser_why_url(self) -> &'static str {
        match self {
            Language::English => "That address is not a video page",
            Language::Vietnamese => "Địa chỉ đó không phải trang video",
        }
    }
}
