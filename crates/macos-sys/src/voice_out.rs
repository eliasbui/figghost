//! The voice that reads an answer aloud: the speech synthesizer of macOS, with the voice of the language the text is
//! in. Sentences are queued, so one begins as soon as it is written while the rest are still on their way.

use objc2::rc::Retained;
use objc2_avf_audio::{AVSpeechBoundary, AVSpeechSynthesisVoice, AVSpeechSynthesizer, AVSpeechUtterance};
use objc2_foundation::NSString;

pub struct Speaker {
    synth: Retained<AVSpeechSynthesizer>,
}

impl Default for Speaker {
    fn default() -> Self {
        Self::new()
    }
}

impl Speaker {
    pub fn new() -> Self {
        // SAFETY: a plain initializer, used on the main thread.
        Self { synth: unsafe { AVSpeechSynthesizer::new() } }
    }

    /// Queues `text` to be said in `language` (a locale such as `vi-VN`); the voice is the system's own for it, or
    /// the default one when this Mac has none.
    pub fn say(&self, text: &str, language: &str) {
        let text = text.trim();
        if text.is_empty() {
            return;
        }
        // SAFETY: plain constructors and setters on live objects, on the main thread.
        unsafe {
            let utterance = AVSpeechUtterance::speechUtteranceWithString(&NSString::from_str(text));
            if let Some(voice) = AVSpeechSynthesisVoice::voiceWithLanguage(Some(&NSString::from_str(language))) {
                utterance.setVoice(Some(&voice));
            }
            self.synth.speakUtterance(&utterance);
        }
    }

    pub fn is_speaking(&self) -> bool {
        // SAFETY: a plain getter.
        unsafe { self.synth.isSpeaking() }
    }

    /// Stops at once and forgets what was queued.
    pub fn stop(&self) {
        // SAFETY: a plain call on the live synthesizer.
        unsafe { self.synth.stopSpeakingAtBoundary(AVSpeechBoundary::Immediate) };
    }
}

/// `vi-VN` for a text with a letter only Vietnamese writes (not the plain accents French and Spanish share), `en-US` for any other.
pub fn language_of(text: &str) -> &'static str {
    const ONLY_VIETNAMESE: &str = "ăâđêôơưảãạấầẩẫậắằẳẵặẻẽẹếềểễệỉĩịỏõọốồổỗộớờởỡợủũụứừửữựỳỷỹỵ";
    let vietnamese = text
        .chars()
        .flat_map(char::to_lowercase)
        .any(|c| ONLY_VIETNAMESE.contains(c));
    if vietnamese { "vi-VN" } else { "en-US" }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_language_of_a_text_is_told_by_its_letters() {
        assert_eq!(language_of("Xin chào, bạn khỏe không?"), "vi-VN");
        assert_eq!(language_of("ĐÂY LÀ CHỮ HOA"), "vi-VN");
        assert_eq!(language_of("Hello, how are you?"), "en-US");
        assert_eq!(language_of("café"), "en-US", "an accent of another language is not Vietnamese");
        assert_eq!(language_of(""), "en-US");
    }
}
