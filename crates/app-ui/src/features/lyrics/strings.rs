//! The words of the lyrics area on the media tab, in both languages. The words of the Settings page are in
//! `web/features/lyrics.js`.

use crate::i18n::Language;

/// Why there are no lyrics to show.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Notice {
    /// Fetching is off and nothing was kept for this track.
    Off,
    Fetching,
    Missing,
    Failed,
    Instrumental,
}

impl Language {
    /// A short title and one sentence of explanation.
    pub fn lyrics_notice(self, notice: Notice) -> (&'static str, &'static str) {
        match (notice, self) {
            (Notice::Off, Language::English) => (
                "Online lyrics are off",
                "Turn on Fetch lyrics online in Settings, on the Notch page, to see lyrics for this track.",
            ),
            (Notice::Off, Language::Vietnamese) => (
                "Lời bài hát trực tuyến đang tắt",
                "Bật Tải lời bài hát trực tuyến trong Cài đặt, ở trang Notch, để xem lời của bài hát này.",
            ),
            (Notice::Fetching, Language::English) => {
                ("Fetching lyrics...", "Asking lrclib.net for this track.")
            }
            (Notice::Fetching, Language::Vietnamese) => (
                "Đang tải lời bài hát...",
                "Đang hỏi lrclib.net về bài hát này.",
            ),
            (Notice::Missing, Language::English) => (
                "No lyrics for this track",
                "lrclib.net has none for it yet. It is asked again in a few days.",
            ),
            (Notice::Missing, Language::Vietnamese) => (
                "Không có lời cho bài hát này",
                "lrclib.net chưa có lời cho bài này. Sẽ hỏi lại sau vài ngày.",
            ),
            (Notice::Failed, Language::English) => (
                "Lyrics could not be loaded",
                "lrclib.net did not answer. It is tried again in a little while.",
            ),
            (Notice::Failed, Language::Vietnamese) => (
                "Không tải được lời bài hát",
                "lrclib.net không phản hồi. Sẽ thử lại sau ít phút.",
            ),
            (Notice::Instrumental, Language::English) => {
                ("Instrumental", "This track has no lyrics.")
            }
            (Notice::Instrumental, Language::Vietnamese) => {
                ("Nhạc không lời", "Bản nhạc này không có lời.")
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_notice_has_words_in_every_language() {
        for language in Language::ALL {
            for notice in [
                Notice::Off,
                Notice::Fetching,
                Notice::Missing,
                Notice::Failed,
                Notice::Instrumental,
            ] {
                let (title, detail) = language.lyrics_notice(notice);
                assert!(!title.trim().is_empty() && !detail.trim().is_empty());
                assert!(title.is_ascii() || language != Language::English);
            }
        }
        // Vietnamese is written with its diacritics, not stripped to ASCII.
        assert!(!Language::Vietnamese.lyrics_notice(Notice::Off).0.is_ascii());
    }
}
