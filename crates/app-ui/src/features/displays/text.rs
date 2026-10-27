//! The words of the Displays tab.

use crate::i18n::Language;

impl Language {
    pub fn displays_tab(self) -> &'static str {
        match self {
            Language::English => "Displays",
            Language::Vietnamese => "Màn hình",
        }
    }

    pub fn displays_brightness(self) -> &'static str {
        match self {
            Language::English => "Brightness",
            Language::Vietnamese => "Độ sáng",
        }
    }

    pub fn displays_contrast(self) -> &'static str {
        match self {
            Language::English => "Contrast",
            Language::Vietnamese => "Độ tương phản",
        }
    }

    /// The menu of a display's colour profiles, for VoiceOver.
    pub fn displays_profile(self) -> &'static str {
        match self {
            Language::English => "Color profile",
            Language::Vietnamese => "Cấu hình màu",
        }
    }

    /// A display that gives no name of its own.
    pub fn displays_unnamed(self, number: usize) -> String {
        match self {
            Language::English => format!("Display {number}"),
            Language::Vietnamese => format!("Màn hình {number}"),
        }
    }

    pub fn displays_looking(self) -> &'static str {
        match self {
            Language::English => "Looking for displays",
            Language::Vietnamese => "Đang tìm màn hình",
        }
    }

    pub fn displays_none_title(self) -> &'static str {
        match self {
            Language::English => "No display to set",
            Language::Vietnamese => "Không có màn hình để chỉnh",
        }
    }

    pub fn displays_none_hint(self) -> &'static str {
        match self {
            Language::English => {
                "No external display answers over DDC/CI. Turn DDC/CI on in the display's own menu, or connect it directly instead of through a hub or adapter."
            }
            Language::Vietnamese => {
                "Không màn hình rời nào trả lời qua DDC/CI. Hãy bật DDC/CI trong menu của màn hình, hoặc cắm thẳng vào máy thay vì qua hub hay bộ chuyển."
            }
        }
    }

    /// The two views of the tab.
    pub fn displays_mode_adjust(self) -> &'static str {
        match self {
            Language::English => "Brightness & color",
            Language::Vietnamese => "Độ sáng & màu",
        }
    }

    pub fn displays_mode_arrange(self) -> &'static str {
        match self {
            Language::English => "Arrange",
            Language::Vietnamese => "Sắp xếp",
        }
    }

    /// The name of the menu of what the chosen display is used as, and its choices, as System Settings words them.
    pub fn displays_use_label(self) -> &'static str {
        match self {
            Language::English => "Use as",
            Language::Vietnamese => "Dùng làm",
        }
    }

    pub fn displays_use_main(self) -> &'static str {
        match self {
            Language::English => "Main display",
            Language::Vietnamese => "Màn hình chính",
        }
    }

    pub fn displays_use_extended(self) -> &'static str {
        match self {
            Language::English => "Extended display",
            Language::Vietnamese => "Màn hình mở rộng",
        }
    }

    pub fn displays_use_mirror(self, of: &str) -> String {
        match self {
            Language::English => format!("Mirror for {of}"),
            Language::Vietnamese => format!("Phản chiếu cho {of}"),
        }
    }

    /// What the chosen display is, under its name: its size in points, and whether it is built in, the main one, or
    /// shows another's picture.
    pub fn displays_detail(
        self,
        (width, height): (f64, f64),
        built_in: bool,
        main: bool,
        mirror_of: Option<&str>,
    ) -> String {
        let size = format!("{width:.0} x {height:.0}");
        let mut parts = vec![size];
        match self {
            Language::English => {
                if built_in {
                    parts.push("built in".into());
                }
                if main {
                    parts.push("main display".into());
                }
                if let Some(of) = mirror_of {
                    parts.push(format!("shows {of}"));
                }
            }
            Language::Vietnamese => {
                if built_in {
                    parts.push("liền máy".into());
                }
                if main {
                    parts.push("màn hình chính".into());
                }
                if let Some(of) = mirror_of {
                    parts.push(format!("đang hiện hình của {of}"));
                }
            }
        }
        parts.join(", ")
    }

    pub fn displays_arrange_hint(self, mirrored: bool) -> &'static str {
        match (self, mirrored) {
            (Language::English, false) => {
                "Drag a display to move it. Drag the white bar onto another display to make that one the main display."
            }
            (Language::English, true) => {
                "Drag a display to move it, or the white bar to choose the main display. Click a stack again for the display that mirrors it."
            }
            (Language::Vietnamese, false) => {
                "Kéo một màn hình để đổi chỗ. Kéo thanh trắng sang màn hình khác để đặt nó làm màn hình chính."
            }
            (Language::Vietnamese, true) => {
                "Kéo màn hình để đổi chỗ, kéo thanh trắng để chọn màn hình chính. Bấm lại vào một chồng để chọn màn hình đang phản chiếu."
            }
        }
    }

    /// Shown in place of the displays while the feature is off.
    pub fn displays_off_title(self) -> &'static str {
        match self {
            Language::English => "The Displays tab is off",
            Language::Vietnamese => "Tab Màn hình đang tắt",
        }
    }

    pub fn displays_off_hint(self) -> &'static str {
        match self {
            Language::English => "Turn it on in Settings, under Notch tabs, Displays.",
            Language::Vietnamese => "Bật trong Cài đặt, mục Tab của notch, Màn hình.",
        }
    }
}
