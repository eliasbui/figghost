//! The words the utilities put on the notch and in the menu bar panel.

use crate::i18n::Language;

impl Language {
    pub fn util_awake_menu(self) -> &'static str {
        match self {
            Language::English => "Keep this Mac awake",
            Language::Vietnamese => "Giữ máy luôn thức",
        }
    }

    /// The length of one choice of the submenu; 0 is until the person stops it.
    pub fn util_awake_choice(self, minutes: u32) -> String {
        match (self, minutes) {
            (Language::English, 0) => "Until I stop".into(),
            (Language::Vietnamese, 0) => "Đến khi tôi tắt".into(),
            (Language::English, 60) => "1 hour".into(),
            (Language::Vietnamese, 60) => "1 giờ".into(),
            (Language::English, m) if m % 60 == 0 => format!("{} hours", m / 60),
            (Language::Vietnamese, m) if m % 60 == 0 => format!("{} giờ", m / 60),
            (Language::English, m) => format!("{m} minutes"),
            (Language::Vietnamese, m) => format!("{m} phút"),
        }
    }

    pub fn util_snooze_menu(self) -> &'static str {
        match self {
            Language::English => "Snooze the notch",
            Language::Vietnamese => "Tạm tắt notch",
        }
    }

    /// The length of one choice of the submenu; 0 is until the person wakes it.
    pub fn util_snooze_choice(self, minutes: u32) -> String {
        match (self, minutes) {
            (Language::English, 0) => "Until I wake it".into(),
            (Language::Vietnamese, 0) => "Đến khi tôi bật lại".into(),
            (Language::English, 60) => "1 hour".into(),
            (Language::Vietnamese, 60) => "1 giờ".into(),
            (Language::English, m) if m % 60 == 0 => format!("{} hours", m / 60),
            (Language::Vietnamese, m) if m % 60 == 0 => format!("{} giờ", m / 60),
            (Language::English, m) => format!("{m} minutes"),
            (Language::Vietnamese, m) => format!("{m} phút"),
        }
    }

    pub fn util_snooze_wake(self, left: &str) -> String {
        match self {
            Language::English => format!("Wake the notch ({left} left)"),
            Language::Vietnamese => format!("Bật lại notch (còn {left})"),
        }
    }

    pub fn util_snooze_wake_open(self) -> &'static str {
        match self {
            Language::English => "Wake the notch",
            Language::Vietnamese => "Bật lại notch",
        }
    }

    pub fn util_snooze_on(self) -> &'static str {
        match self {
            Language::English => "Notch snoozed",
            Language::Vietnamese => "Đã tạm tắt notch",
        }
    }

    pub fn util_snooze_for(self, minutes: u32) -> String {
        match (self, minutes) {
            (Language::English, 0) => "It will not open when the pointer reaches it".into(),
            (Language::Vietnamese, 0) => "Sẽ không mở khi con trỏ chạm vào".into(),
            (Language::English, m) => format!(
                "It will not open on hover for {}",
                self.util_snooze_choice(m)
            ),
            (Language::Vietnamese, m) => format!(
                "Sẽ không mở khi rê chuột trong {}",
                self.util_snooze_choice(m)
            ),
        }
    }

    pub fn util_snooze_off(self) -> &'static str {
        match self {
            Language::English => "Notch is back",
            Language::Vietnamese => "Notch đã bật lại",
        }
    }

    pub fn util_awake_stop(self, left: &str) -> String {
        match self {
            Language::English => format!("Stop keeping awake ({left} left)"),
            Language::Vietnamese => format!("Dừng giữ thức (còn {left})"),
        }
    }

    pub fn util_awake_stop_open(self) -> &'static str {
        match self {
            Language::English => "Stop keeping awake",
            Language::Vietnamese => "Dừng giữ thức",
        }
    }

    pub fn util_awake_on(self) -> &'static str {
        match self {
            Language::English => "Keeping this Mac awake",
            Language::Vietnamese => "Đang giữ máy thức",
        }
    }

    pub fn util_awake_for(self, minutes: u32) -> String {
        if minutes == 0 {
            match self {
                Language::English => "Until you stop it".into(),
                Language::Vietnamese => "Đến khi bạn tắt".into(),
            }
        } else {
            match self {
                Language::English => format!("For {}", self.util_awake_choice(minutes)),
                Language::Vietnamese => format!("Trong {}", self.util_awake_choice(minutes)),
            }
        }
    }

    pub fn util_awake_off(self) -> &'static str {
        match self {
            Language::English => "This Mac may sleep again",
            Language::Vietnamese => "Máy có thể ngủ trở lại",
        }
    }

    pub fn util_mic_menu(self) -> &'static str {
        match self {
            Language::English => "Mute microphone",
            Language::Vietnamese => "Tắt tiếng micro",
        }
    }

    pub fn util_mic_muted(self) -> &'static str {
        match self {
            Language::English => "Microphone muted",
            Language::Vietnamese => "Đã tắt tiếng micro",
        }
    }

    pub fn util_mic_live(self) -> &'static str {
        match self {
            Language::English => "Microphone on",
            Language::Vietnamese => "Micro đang bật",
        }
    }

    pub fn util_mic_cannot(self) -> &'static str {
        match self {
            Language::English => "This microphone cannot be muted",
            Language::Vietnamese => "Micro này không tắt tiếng được",
        }
    }

    pub fn util_mic_hint(self) -> &'static str {
        match self {
            Language::English => "Every app hears nothing",
            Language::Vietnamese => "Mọi ứng dụng đều không nghe thấy",
        }
    }

    pub fn util_lock_menu(self) -> &'static str {
        match self {
            Language::English => "Lock keyboard to clean it",
            Language::Vietnamese => "Khóa bàn phím để lau",
        }
    }

    pub fn util_lock_unlock_menu(self) -> &'static str {
        match self {
            Language::English => "Unlock keyboard",
            Language::Vietnamese => "Mở khóa bàn phím",
        }
    }

    pub fn util_lock_title(self) -> &'static str {
        match self {
            Language::English => "Keyboard locked",
            Language::Vietnamese => "Bàn phím đã khóa",
        }
    }

    pub fn util_lock_hint(self) -> &'static str {
        match self {
            Language::English => "Command and Escape unlock it",
            Language::Vietnamese => "Nhấn Command và Escape để mở",
        }
    }

    pub fn util_lock_done(self) -> &'static str {
        match self {
            Language::English => "Keyboard unlocked",
            Language::Vietnamese => "Đã mở khóa bàn phím",
        }
    }

    pub fn util_lock_needs_access(self) -> &'static str {
        match self {
            Language::English => "Allow Accessibility to lock the keyboard",
            Language::Vietnamese => "Hãy cho phép Trợ năng để khóa bàn phím",
        }
    }
}
