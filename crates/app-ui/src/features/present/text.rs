//! The words of the Present tab, in each language.

use super::{Action, Kind, Shown};
use crate::i18n::Language;

impl Language {
    pub fn present_tab(self) -> &'static str {
        match self {
            Language::English => "Present",
            Language::Vietnamese => "Trình chiếu",
        }
    }

    pub fn present_source_label(self) -> &'static str {
        match self {
            Language::English => "What to present",
            Language::Vietnamese => "Trình chiếu gì",
        }
    }

    pub fn present_stage_label(self) -> &'static str {
        match self {
            Language::English => "Where the audience looks",
            Language::Vietnamese => "Trình chiếu lên",
        }
    }

    pub fn present_kind(self, kind: Kind) -> &'static str {
        match (self, kind) {
            (Language::English, Kind::Display) => "Display",
            (Language::English, Kind::Area) => "Area",
            (Language::English, Kind::Window) => "Window",
            (Language::English, Kind::App) => "App",
            (Language::Vietnamese, Kind::Display) => "Màn hình",
            (Language::Vietnamese, Kind::Area) => "Vùng",
            (Language::Vietnamese, Kind::Window) => "Cửa sổ",
            (Language::Vietnamese, Kind::App) => "Ứng dụng",
        }
    }

    /// The stage that is a window of its own, which a video call can share.
    pub fn present_own_window(self) -> &'static str {
        match self {
            Language::English => "Its own window",
            Language::Vietnamese => "Cửa sổ riêng",
        }
    }

    pub fn present_window_title(self) -> &'static str {
        match self {
            Language::English => "Figghost presentation",
            Language::Vietnamese => "Figghost trình chiếu",
        }
    }

    pub fn present_start(self) -> &'static str {
        match self {
            Language::English => "Start presenting",
            Language::Vietnamese => "Bắt đầu trình chiếu",
        }
    }

    /// Beside the button: what a press does for this kind, and the key that starts and stops from anywhere.
    pub fn present_hint(self, kind: Kind, key: &str) -> String {
        let how = match (self, kind) {
            (Language::English, Kind::Display) => {
                "Shows a whole display. With more than two displays, a click chooses which."
            }
            (Language::English, Kind::Area) => {
                "Drag over the part of the screen to show: the audience sees only that part, live."
            }
            (Language::English, Kind::Window) => {
                "Click a window: it is shown on its own, even while other windows cover it."
            }
            (Language::English, Kind::App) => {
                "Click a window of the app: all its windows on that display are shown, and nothing else."
            }
            (Language::Vietnamese, Kind::Display) => {
                "Chiếu cả một màn hình. Nếu có hơn hai màn hình, bấm vào màn hình muốn chiếu."
            }
            (Language::Vietnamese, Kind::Area) => {
                "Kéo chọn phần màn hình muốn chiếu: khán giả chỉ thấy đúng phần đó, cập nhật liên tục."
            }
            (Language::Vietnamese, Kind::Window) => {
                "Bấm vào một cửa sổ: chỉ cửa sổ đó được chiếu, kể cả khi bị cửa sổ khác che."
            }
            (Language::Vietnamese, Kind::App) => {
                "Bấm vào một cửa sổ của ứng dụng: mọi cửa sổ của ứng dụng đó trên màn hình ấy được chiếu, ngoài ra không có gì khác."
            }
        };
        if key.is_empty() {
            return how.to_owned();
        }
        match self {
            Language::English => format!("{how} {key} starts and stops from any app."),
            Language::Vietnamese => {
                format!("{how} Bấm {key} để bắt đầu hoặc dừng từ bất kỳ ứng dụng nào.")
            }
        }
    }

    /// What the audience sees now, at the top of the tab.
    pub fn present_live_title(self, shown: Shown) -> &'static str {
        match (self, shown) {
            (Language::English, Shown::Live) => "Presenting",
            (Language::English, Shown::Held) => "Picture held",
            (Language::English, Shown::Black) => "Black screen",
            (Language::English, Shown::White) => "White screen",
            (Language::Vietnamese, Shown::Live) => "Đang trình chiếu",
            (Language::Vietnamese, Shown::Held) => "Đang đứng hình",
            (Language::Vietnamese, Shown::Black) => "Đang để màn hình đen",
            (Language::Vietnamese, Shown::White) => "Đang để màn hình trắng",
        }
    }

    pub fn present_what(self, source: &str, stage: &str) -> String {
        match self {
            Language::English => format!("{source} on {stage}"),
            Language::Vietnamese => format!("{source} lên {stage}"),
        }
    }

    pub fn present_area_of(self, display: &str) -> String {
        match self {
            Language::English => format!("An area of {display}"),
            Language::Vietnamese => format!("Một vùng của {display}"),
        }
    }

    pub fn present_stop(self) -> &'static str {
        match self {
            Language::English => "Stop",
            Language::Vietnamese => "Dừng",
        }
    }

    /// The name of a switch on the tab, and of a key in the line of keys.
    pub fn present_action(self, action: Action) -> &'static str {
        match (self, action) {
            (Language::English, Action::Start) => "start or stop",
            (Language::English, Action::Black) => "Black",
            (Language::English, Action::White) => "White",
            (Language::English, Action::Freeze) => "Hold",
            (Language::English, Action::ZoomIn) => "zoom in",
            (Language::English, Action::ZoomOut) => "zoom out",
            (Language::English, Action::ZoomReset) => "whole picture",
            (Language::English, Action::Auto) => "Auto zoom",
            (Language::English, Action::Spotlight) => "Spotlight",
            (Language::English, Action::Ring) => "Pointer ring",
            (Language::Vietnamese, Action::Start) => "bắt đầu hoặc dừng",
            (Language::Vietnamese, Action::Black) => "Màn đen",
            (Language::Vietnamese, Action::White) => "Màn trắng",
            (Language::Vietnamese, Action::Freeze) => "Đứng hình",
            (Language::Vietnamese, Action::ZoomIn) => "phóng to",
            (Language::Vietnamese, Action::ZoomOut) => "thu nhỏ",
            (Language::Vietnamese, Action::ZoomReset) => "toàn cảnh",
            (Language::Vietnamese, Action::Auto) => "Tự phóng to",
            (Language::Vietnamese, Action::Spotlight) => "Đèn chiếu",
            (Language::Vietnamese, Action::Ring) => "Vòng con trỏ",
        }
    }

    pub fn present_zoom_names(self) -> [&'static str; 2] {
        match self {
            Language::English => ["Zoom out", "Zoom in"],
            Language::Vietnamese => ["Thu nhỏ", "Phóng to"],
        }
    }

    pub fn present_needs_access(self) -> &'static str {
        match self {
            Language::English => {
                "Presenting needs Screen Recording: allow Figghost in System Settings, then start again."
            }
            Language::Vietnamese => {
                "Trình chiếu cần quyền Ghi màn hình: hãy cho phép Figghost trong Cài đặt hệ thống rồi bắt đầu lại."
            }
        }
    }

    pub fn present_same_display(self) -> &'static str {
        match self {
            Language::English => {
                "That is the display the audience looks at. Choose another one, or present in its own window."
            }
            Language::Vietnamese => {
                "Đó chính là màn hình khán giả đang xem. Hãy chọn nơi khác, hoặc chiếu vào cửa sổ riêng."
            }
        }
    }

    pub fn present_nothing_else(self) -> &'static str {
        match self {
            Language::English => {
                "There is no other display to show. Present in its own window, or choose an area, a window or an app."
            }
            Language::Vietnamese => {
                "Không còn màn hình nào khác để chiếu. Hãy chiếu vào cửa sổ riêng, hoặc chọn một vùng, cửa sổ hay ứng dụng."
            }
        }
    }

    pub fn present_failed(self, why: &str) -> String {
        match self {
            Language::English => format!("Cannot present: {why}"),
            Language::Vietnamese => format!("Không trình chiếu được: {why}"),
        }
    }

    pub fn present_ended(self, why: &str) -> String {
        match self {
            Language::English => format!("The presentation stopped: {why}"),
            Language::Vietnamese => format!("Buổi trình chiếu đã dừng: {why}"),
        }
    }

    pub fn present_display_gone(self) -> &'static str {
        match self {
            Language::English => "The display the audience looked at was disconnected.",
            Language::Vietnamese => "Màn hình khán giả đang xem đã bị ngắt.",
        }
    }

    pub fn present_off_title(self) -> &'static str {
        match self {
            Language::English => "The Present tab is off",
            Language::Vietnamese => "Tab Trình chiếu đang tắt",
        }
    }

    pub fn present_off_hint(self) -> &'static str {
        match self {
            Language::English => "Turn it on in Settings, under Notch tabs, Present.",
            Language::Vietnamese => "Bật trong Cài đặt, mục Tab của notch, Trình chiếu.",
        }
    }

    pub fn present_menu_start(self) -> &'static str {
        match self {
            Language::English => "Start presenting",
            Language::Vietnamese => "Bắt đầu trình chiếu",
        }
    }

    pub fn present_menu_stop(self, clock: &str) -> String {
        match self {
            Language::English => format!("Stop presenting ({clock})"),
            Language::Vietnamese => format!("Dừng trình chiếu ({clock})"),
        }
    }
}
