//! The words of Jarvis's panel, in each language.

use crate::i18n::Language;
use core_jarvis::Failure;

impl Language {
    pub fn jarvis_status_ready(self) -> &'static str {
        match self {
            Language::English => "Ready",
            Language::Vietnamese => "Sẵn sàng",
        }
    }

    pub fn jarvis_status_thinking(self) -> &'static str {
        match self {
            Language::English => "Thinking...",
            Language::Vietnamese => "Đang nghĩ...",
        }
    }

    pub fn jarvis_status_answering(self) -> &'static str {
        match self {
            Language::English => "Answering...",
            Language::Vietnamese => "Đang trả lời...",
        }
    }

    pub fn jarvis_beta(self) -> &'static str {
        "BETA"
    }

    pub fn jarvis_greeting(self) -> &'static str {
        match self {
            Language::English => "What can I do for you? Type your question below and press Return.",
            Language::Vietnamese => "Tôi giúp được gì cho bạn? Gõ câu hỏi ở dưới rồi nhấn Return.",
        }
    }

    pub fn jarvis_placeholder(self) -> &'static str {
        match self {
            Language::English => "Ask anything. Esc stops the answer, then closes.",
            Language::Vietnamese => "Hỏi bất cứ điều gì. Esc dừng câu trả lời, rồi đóng.",
        }
    }

    pub fn jarvis_thinking_line(self) -> &'static str {
        "..."
    }

    pub fn jarvis_no_key(self, provider: &str) -> String {
        match self {
            Language::English => format!("There is no key for {provider} yet. Add one in Settings, on the Jarvis page."),
            Language::Vietnamese => format!("Chưa có khóa cho {provider}. Thêm khóa trong Cài đặt, ở trang Jarvis."),
        }
    }

    pub fn jarvis_stopped(self) -> &'static str {
        match self {
            Language::English => "Stopped.",
            Language::Vietnamese => "Đã dừng.",
        }
    }

    pub fn jarvis_failure(self, failure: &Failure) -> String {
        match (self, failure) {
            (Language::English, Failure::NoCurl) => "This Mac has no curl, so Jarvis cannot reach the internet.".into(),
            (Language::Vietnamese, Failure::NoCurl) => "Máy này không có curl nên Jarvis không ra mạng được.".into(),
            (Language::English, Failure::Unreachable) => "Could not reach the service. Check the connection and try again.".into(),
            (Language::Vietnamese, Failure::Unreachable) => "Không kết nối được với dịch vụ. Kiểm tra mạng rồi thử lại.".into(),
            (Language::English, Failure::Service(why)) => format!("The service said: {why}"),
            (Language::Vietnamese, Failure::Service(why)) => format!("Dịch vụ trả lời: {why}"),
            (Language::English, Failure::Empty) => "The service answered with nothing.".into(),
            (Language::Vietnamese, Failure::Empty) => "Dịch vụ không trả lời gì.".into(),
            (_, Failure::Cancelled) => self.jarvis_stopped().into(),
        }
    }

    pub fn jarvis_status_listening(self) -> &'static str {
        match self {
            Language::English => "Listening...",
            Language::Vietnamese => "Đang nghe...",
        }
    }

    /// What Jarvis says when the panel opens.
    pub fn jarvis_ask(self) -> &'static str {
        match self {
            Language::English => "What can I do for you?",
            Language::Vietnamese => "Bạn muốn tôi giúp gì?",
        }
    }

    pub fn jarvis_asking(self) -> &'static str {
        match self {
            Language::English => "Allow Speech Recognition and the microphone in the dialogs, then press the microphone button again.",
            Language::Vietnamese => "Hãy cho phép Nhận dạng giọng nói và micrô trong các hộp thoại, rồi nhấn lại nút micrô.",
        }
    }

    pub fn jarvis_no_speech_access(self) -> &'static str {
        match self {
            Language::English => "Figghost may not use Speech Recognition. Allow it in System Settings, Privacy and Security, or type your question.",
            Language::Vietnamese => "Figghost chưa được dùng Nhận dạng giọng nói. Cho phép trong Cài đặt hệ thống, Quyền riêng tư và bảo mật, hoặc gõ câu hỏi.",
        }
    }

    pub fn jarvis_no_microphone(self) -> &'static str {
        match self {
            Language::English => "Figghost may not use the microphone. Allow it in System Settings, Privacy and Security, or type your question.",
            Language::Vietnamese => "Figghost chưa được dùng micrô. Cho phép trong Cài đặt hệ thống, Quyền riêng tư và bảo mật, hoặc gõ câu hỏi.",
        }
    }

    pub fn jarvis_not_on_device(self) -> &'static str {
        match self {
            Language::English => "This Mac cannot recognize that language by itself. Allow Apple's servers in Settings, on the Jarvis page, or type your question.",
            Language::Vietnamese => "Máy này chưa tự nhận dạng được ngôn ngữ đó. Cho phép máy chủ của Apple trong Cài đặt, ở trang Jarvis, hoặc gõ câu hỏi.",
        }
    }

    pub fn jarvis_cannot_listen(self) -> &'static str {
        match self {
            Language::English => "The microphone could not be opened. Type your question instead.",
            Language::Vietnamese => "Không mở được micrô. Hãy gõ câu hỏi.",
        }
    }

    /// Why Jarvis cannot ask through the ChatGPT plan yet.
    pub fn jarvis_codex(self, why: core_jarvis::codex::SignIn) -> &'static str {
        use core_jarvis::codex::SignIn;
        match (self, why) {
            (Language::English, SignIn::Missing) => {
                "Codex is not signed in on this Mac. Run `codex login` in a terminal, or sign in to Codex in Orca and switch to that account, then ask again."
            }
            (Language::Vietnamese, SignIn::Missing) => {
                "Codex chưa đăng nhập trên máy này. Chạy `codex login` trong terminal, hoặc đăng nhập Codex trong Orca rồi chuyển sang tài khoản đó, sau đó hỏi lại."
            }
            (Language::English, SignIn::Expired) => {
                "The Codex sign-in has run out. Use Codex once (for example `codex` in a terminal), which renews it, then ask again."
            }
            (Language::Vietnamese, SignIn::Expired) => {
                "Phiên đăng nhập Codex đã hết hạn. Dùng Codex một lần (ví dụ chạy `codex` trong terminal) để nó tự làm mới, rồi hỏi lại."
            }
            (Language::English, SignIn::Unreadable) => "The Codex sign-in file could not be read. Sign in to Codex again.",
            (Language::Vietnamese, SignIn::Unreadable) => "Không đọc được tệp đăng nhập của Codex. Hãy đăng nhập lại Codex.",
        }
    }
}
