//! The words of the Video tab.

use crate::i18n::Language;

impl Language {
    pub fn video_tab(self) -> &'static str {
        match self {
            Language::English => "Video",
            Language::Vietnamese => "Video",
        }
    }

    pub fn video_no_url(self) -> (&'static str, &'static str) {
        match self {
            Language::English => (
                "No stream chosen",
                "Paste the address of a stream or video under Notch tabs, Video.",
            ),
            Language::Vietnamese => (
                "Chưa chọn luồng video",
                "Dán địa chỉ luồng hoặc video ở mục Tab của notch, Video.",
            ),
        }
    }

    pub fn video_failed(self) -> (&'static str, &'static str) {
        match self {
            Language::English => (
                "The video cannot be played",
                "Check the address and the connection. macOS plays HLS (.m3u8), MP4 and MOV over https.",
            ),
            Language::Vietnamese => (
                "Không phát được video",
                "Kiểm tra địa chỉ và kết nối. macOS phát được HLS (.m3u8), MP4 và MOV qua https.",
            ),
        }
    }
}
