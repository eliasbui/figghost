//! Strings of the sound page, in the same exhaustive-match style as `i18n.rs`.

use crate::i18n::Language;
use core_audio_fx::Preset;

impl Language {
    pub fn audio_tab(self) -> &'static str {
        match self {
            Language::English => "Sound",
            Language::Vietnamese => "Âm thanh",
        }
    }

    pub fn audio_hint(self) -> &'static str {
        match self {
            Language::English => {
                "Right-click an app for its own EQ. Slider middle is 100%, right end 400%."
            }
            Language::Vietnamese => {
                "Chuột phải vào tên app để chọn EQ riêng. Giữa thanh là 100%, cuối thanh 400%."
            }
        }
    }

    pub fn audio_enabled(self) -> &'static str {
        match self {
            Language::English => "Mixer on",
            Language::Vietnamese => "Bật bộ trộn",
        }
    }

    pub fn audio_device_volume(self) -> &'static str {
        match self {
            Language::English => "Software volume",
            Language::Vietnamese => "Âm lượng phần mềm",
        }
    }

    pub fn audio_boost(self) -> &'static str {
        match self {
            Language::English => "Lift quiet",
            Language::Vietnamese => "Nâng âm nhỏ",
        }
    }

    pub fn audio_night(self) -> &'static str {
        match self {
            Language::English => "Night mode",
            Language::Vietnamese => "Chế độ đêm",
        }
    }

    pub fn audio_width(self) -> &'static str {
        match self {
            Language::English => "Stereo width",
            Language::Vietnamese => "Độ rộng stereo",
        }
    }

    pub fn audio_mute(self) -> &'static str {
        match self {
            Language::English => "Mute",
            Language::Vietnamese => "Tắt",
        }
    }

    pub fn audio_app_eq(self) -> &'static str {
        match self {
            Language::English => "EQ for this app",
            Language::Vietnamese => "EQ riêng cho app này",
        }
    }

    pub fn audio_default_output(self) -> &'static str {
        match self {
            Language::English => "Default",
            Language::Vietnamese => "Mặc định",
        }
    }

    pub fn audio_output(self, device: &str, software: bool) -> String {
        match (self, software) {
            (Language::English, false) => format!("Output: {device}"),
            (Language::English, true) => format!("Output: {device} (no hardware volume)"),
            (Language::Vietnamese, false) => format!("Đầu ra: {device}"),
            (Language::Vietnamese, true) => {
                format!("Đầu ra: {device} (không có âm lượng phần cứng)")
            }
        }
    }

    pub fn audio_no_output(self) -> &'static str {
        match self {
            Language::English => "No output device",
            Language::Vietnamese => "Không có thiết bị phát",
        }
    }

    pub fn audio_eq_off(self) -> &'static str {
        match self {
            Language::English => "EQ off",
            Language::Vietnamese => "Tắt EQ",
        }
    }

    pub fn audio_eq_custom(self) -> &'static str {
        match self {
            Language::English => "Custom (config file)",
            Language::Vietnamese => "Tùy chỉnh (tệp cấu hình)",
        }
    }

    pub fn audio_preset(self, preset: Preset) -> &'static str {
        match (self, preset) {
            (Language::English, Preset::Flat) => "Flat",
            (Language::English, Preset::BassBoost) => "Bass boost",
            (Language::English, Preset::BassReducer) => "Bass reducer",
            (Language::English, Preset::TrebleBoost) => "Treble boost",
            (Language::English, Preset::TrebleReducer) => "Treble reducer",
            (Language::English, Preset::Vocal) => "Vocal",
            (Language::English, Preset::Loudness) => "Loudness",
            (Language::English, Preset::Rock) => "Rock",
            (Language::English, Preset::Pop) => "Pop",
            (Language::English, Preset::Electronic) => "Electronic",
            (Language::English, Preset::HipHop) => "Hip-hop",
            (Language::English, Preset::Jazz) => "Jazz",
            (Language::English, Preset::Classical) => "Classical",
            (Language::English, Preset::Acoustic) => "Acoustic",
            (Language::English, Preset::Spoken) => "Spoken word",
            (Language::English, Preset::Movie) => "Movie",
            (Language::English, Preset::Gaming) => "Gaming",
            (Language::Vietnamese, Preset::Flat) => "Phẳng",
            (Language::Vietnamese, Preset::BassBoost) => "Tăng bass",
            (Language::Vietnamese, Preset::BassReducer) => "Giảm bass",
            (Language::Vietnamese, Preset::TrebleBoost) => "Tăng treble",
            (Language::Vietnamese, Preset::TrebleReducer) => "Giảm treble",
            (Language::Vietnamese, Preset::Vocal) => "Giọng hát",
            (Language::Vietnamese, Preset::Loudness) => "Loudness",
            (Language::Vietnamese, Preset::Rock) => "Rock",
            (Language::Vietnamese, Preset::Pop) => "Pop",
            (Language::Vietnamese, Preset::Electronic) => "Điện tử",
            (Language::Vietnamese, Preset::HipHop) => "Hip-hop",
            (Language::Vietnamese, Preset::Jazz) => "Jazz",
            (Language::Vietnamese, Preset::Classical) => "Cổ điển",
            (Language::Vietnamese, Preset::Acoustic) => "Mộc",
            (Language::Vietnamese, Preset::Spoken) => "Lời nói, podcast",
            (Language::Vietnamese, Preset::Movie) => "Phim",
            (Language::Vietnamese, Preset::Gaming) => "Chơi game",
        }
    }

    pub fn audio_unsupported(self) -> &'static str {
        match self {
            Language::English => "Per-app audio needs macOS 14.2 or later.",
            Language::Vietnamese => "Âm thanh theo từng app cần macOS 14.2 trở lên.",
        }
    }

    pub fn audio_nothing_playing(self) -> &'static str {
        match self {
            Language::English => "No app has played sound yet.",
            Language::Vietnamese => "Chưa có app nào phát âm thanh.",
        }
    }

    pub fn audio_disabled(self) -> &'static str {
        match self {
            Language::English => "The mixer is off: every app plays as usual.",
            Language::Vietnamese => "Bộ trộn đang tắt: mọi app phát như bình thường.",
        }
    }

    pub fn audio_problem(self, detail: &str) -> String {
        match self {
            Language::English => format!("Audio problem: {detail}"),
            Language::Vietnamese => format!("Lỗi âm thanh: {detail}"),
        }
    }
}
