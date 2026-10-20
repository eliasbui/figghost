//! The words the islands put on the notch, and the entries they add to the menu bar panel. An island is a title
//! and one short line under it.

use crate::i18n::Language;

impl Language {
    pub fn island_caps_title(self) -> &'static str {
        "Caps Lock"
    }

    pub fn island_caps_state(self, on: bool) -> &'static str {
        match (self, on) {
            (Language::English, true) => "On",
            (Language::English, false) => "Off",
            (Language::Vietnamese, true) => "Đang bật",
            (Language::Vietnamese, false) => "Đã tắt",
        }
    }

    pub fn island_plugged(self) -> &'static str {
        match self {
            Language::English => "Power adapter connected",
            Language::Vietnamese => "Đã cắm sạc",
        }
    }

    pub fn island_unplugged(self) -> &'static str {
        match self {
            Language::English => "Running on battery",
            Language::Vietnamese => "Đang dùng pin",
        }
    }

    pub fn island_full(self) -> &'static str {
        match self {
            Language::English => "Battery full",
            Language::Vietnamese => "Pin đã đầy",
        }
    }

    pub fn island_full_hint(self, percent: u8) -> String {
        match self {
            Language::English => format!("Charged to {percent}%"),
            Language::Vietnamese => format!("Đã sạc tới {percent}%"),
        }
    }

    pub fn island_low(self) -> &'static str {
        match self {
            Language::English => "Battery low",
            Language::Vietnamese => "Pin yếu",
        }
    }

    pub fn island_low_hint(self, percent: u8) -> String {
        match self {
            Language::English => format!("{percent}% left, plug in soon"),
            Language::Vietnamese => format!("Còn {percent}%, hãy cắm sạc sớm"),
        }
    }

    pub fn island_percent(self, percent: u8) -> String {
        format!("{percent}%")
    }

    pub fn island_drive_connected(self) -> &'static str {
        match self {
            Language::English => "Connected",
            Language::Vietnamese => "Đã kết nối",
        }
    }

    pub fn island_drive_removed(self) -> &'static str {
        match self {
            Language::English => "Removed",
            Language::Vietnamese => "Đã tháo",
        }
    }

    pub fn island_drive_ejected(self) -> &'static str {
        match self {
            Language::English => "Safe to unplug",
            Language::Vietnamese => "Có thể rút ra",
        }
    }

    pub fn island_download_done(self, size: &str) -> String {
        match self {
            Language::English => format!("Download finished, {size}"),
            Language::Vietnamese => format!("Đã tải xong, {size}"),
        }
    }

    pub fn island_download_more(self, extra: usize) -> String {
        match self {
            Language::English => format!("and {extra} more in Downloads"),
            Language::Vietnamese => format!("và {extra} tệp khác trong Downloads"),
        }
    }

    pub fn island_camera(self) -> &'static str {
        match self {
            Language::English => "Camera in use",
            Language::Vietnamese => "Camera đang được dùng",
        }
    }

    pub fn island_mic(self) -> &'static str {
        match self {
            Language::English => "Microphone in use",
            Language::Vietnamese => "Micro đang được dùng",
        }
    }

    pub fn island_both(self) -> &'static str {
        match self {
            Language::English => "Camera and microphone in use",
            Language::Vietnamese => "Camera và micro đang được dùng",
        }
    }

    pub fn island_sample_drive(self) -> &'static str {
        match self {
            Language::English => "Backup",
            Language::Vietnamese => "Sao lưu",
        }
    }

    /// The entry of the menu bar panel that ejects a drive.
    pub fn island_eject(self, name: &str) -> String {
        match self {
            Language::English => format!("Eject {name}"),
            Language::Vietnamese => format!("Tháo {name}"),
        }
    }

    pub fn island_eject_failed(self) -> &'static str {
        match self {
            Language::English => "The drive is in use and could not be ejected",
            Language::Vietnamese => "Ổ đang được dùng nên chưa tháo được",
        }
    }
}
