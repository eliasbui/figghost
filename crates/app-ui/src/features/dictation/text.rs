//! The words of Dictation's panel, in each language.

use crate::i18n::Language;

impl Language {
    pub fn dictation_listening(self) -> &'static str {
        match self {
            Language::English => "Listening...",
            Language::Vietnamese => "Đang nghe...",
        }
    }

    pub fn dictation_finishing(self) -> &'static str {
        match self {
            Language::English => "Finishing...",
            Language::Vietnamese => "Đang hoàn tất...",
        }
    }

    pub fn dictation_hint(self) -> &'static str {
        match self {
            Language::English => "Return to insert, Esc to cancel, arrows to change language",
            Language::Vietnamese => "Return để chèn, Esc để hủy, mũi tên để đổi ngôn ngữ",
        }
    }

    /// The name of a language that can be spoken, for the chip: `vi` or `en`.
    pub fn dictation_language(self, spoken: &str) -> &'static str {
        match (self, spoken) {
            (Language::English, "vi") => "Vietnamese",
            (Language::English, _) => "English",
            (Language::Vietnamese, "vi") => "Tiếng Việt",
            (Language::Vietnamese, _) => "Tiếng Anh",
        }
    }

    pub fn dictation_copied(self) -> &'static str {
        match self {
            Language::English => {
                "No text box had the keyboard, so the words are on the clipboard. Press Command+V to paste them."
            }
            Language::Vietnamese => {
                "Không có ô nhập nào đang nhận bàn phím, nên chữ đã được chép vào clipboard. Nhấn Command+V để dán."
            }
        }
    }

    pub fn dictation_nothing(self) -> &'static str {
        match self {
            Language::English => "Nothing was heard. Check the microphone and try again.",
            Language::Vietnamese => "Không nghe thấy gì. Hãy kiểm tra micrô rồi thử lại.",
        }
    }

    pub fn dictation_asking(self) -> &'static str {
        match self {
            Language::English => {
                "Allow Figghost in the dialog of macOS, then press the shortcut again."
            }
            Language::Vietnamese => {
                "Hãy cho phép Figghost trong hộp thoại của macOS, rồi nhấn lại phím tắt."
            }
        }
    }

    pub fn dictation_no_speech_access(self) -> &'static str {
        match self {
            Language::English => {
                "Figghost may not recognize speech. Allow it in System Settings, Privacy and Security, Speech Recognition."
            }
            Language::Vietnamese => {
                "Figghost chưa được phép nhận dạng giọng nói. Hãy bật trong Cài đặt hệ thống, Quyền riêng tư và bảo mật, Nhận dạng giọng nói."
            }
        }
    }

    pub fn dictation_no_microphone(self) -> &'static str {
        match self {
            Language::English => {
                "Figghost may not use the microphone. Allow it in System Settings, Privacy and Security, Microphone."
            }
            Language::Vietnamese => {
                "Figghost chưa được dùng micrô. Hãy bật trong Cài đặt hệ thống, Quyền riêng tư và bảo mật, Micrô."
            }
        }
    }

    pub fn dictation_not_on_device(self) -> &'static str {
        match self {
            Language::English => {
                "This Mac cannot recognize this language by itself yet. Turn off Only on this Mac in Settings to use Apple's servers."
            }
            Language::Vietnamese => {
                "Máy này chưa tự nhận dạng được ngôn ngữ này. Tắt tùy chọn Chỉ trên máy này trong Cài đặt để dùng máy chủ của Apple."
            }
        }
    }

    pub fn dictation_cannot_start(self) -> &'static str {
        match self {
            Language::English => {
                "The microphone could not be started. Check that one is connected."
            }
            Language::Vietnamese => "Không mở được micrô. Hãy kiểm tra micrô đã được cắm.",
        }
    }
}
