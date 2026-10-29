//! The words of the System tab: the names of its cards, and how a state is said.

use crate::i18n::Language;
use macos_sys::sysinfo::{Pressure, Thermal};

impl Language {
    pub fn system_tab(self) -> &'static str {
        match self {
            Language::English => "System",
            Language::Vietnamese => "Hệ thống",
        }
    }

    pub fn system_cpu(self) -> &'static str {
        "CPU"
    }

    /// The short names of the figures in the menu bar, in the order of `logic::Figure`.
    pub fn system_bar_labels(self) -> [&'static str; 6] {
        match self {
            Language::English => ["CPU", "Mem", "Net", "GPU", "Temp", "Disk"],
            Language::Vietnamese => ["CPU", "RAM", "Mạng", "GPU", "Nhiệt", "Ổ đĩa"],
        }
    }

    pub fn system_efficiency(self) -> &'static str {
        match self {
            Language::English => "Efficiency",
            Language::Vietnamese => "Tiết kiệm",
        }
    }

    pub fn system_performance(self) -> &'static str {
        match self {
            Language::English => "Performance",
            Language::Vietnamese => "Hiệu năng",
        }
    }

    pub fn system_memory(self) -> &'static str {
        match self {
            Language::English => "Memory",
            Language::Vietnamese => "Bộ nhớ",
        }
    }

    pub fn system_apps(self, amount: &str) -> String {
        match self {
            Language::English => format!("Apps {amount}"),
            Language::Vietnamese => format!("Ứng dụng {amount}"),
        }
    }

    pub fn system_wired(self, amount: &str) -> String {
        match self {
            Language::English => format!("Wired {amount}"),
            Language::Vietnamese => format!("Hệ thống {amount}"),
        }
    }

    pub fn system_compressed(self, amount: &str) -> String {
        match self {
            Language::English => format!("Compressed {amount}"),
            Language::Vietnamese => format!("Nén {amount}"),
        }
    }

    pub fn system_swap(self) -> &'static str {
        "Swap"
    }

    pub fn system_pressure(self, pressure: Pressure) -> &'static str {
        match (self, pressure) {
            (Language::English, Pressure::Normal) => "Normal",
            (Language::English, Pressure::Warning) => "Warning",
            (Language::English, Pressure::Critical) => "Critical",
            (Language::Vietnamese, Pressure::Normal) => "Bình thường",
            (Language::Vietnamese, Pressure::Warning) => "Cảnh báo",
            (Language::Vietnamese, Pressure::Critical) => "Nghiêm trọng",
        }
    }

    pub fn system_network(self) -> &'static str {
        match self {
            Language::English => "Network",
            Language::Vietnamese => "Mạng",
        }
    }

    pub fn system_gpu(self) -> &'static str {
        "GPU"
    }

    pub fn system_disk(self) -> &'static str {
        match self {
            Language::English => "Startup disk",
            Language::Vietnamese => "Ổ khởi động",
        }
    }

    pub fn system_disk_free(self, total: &str) -> String {
        match self {
            Language::English => format!("free of {total}"),
            Language::Vietnamese => format!("còn trống trên {total}"),
        }
    }

    pub fn system_thermal(self) -> &'static str {
        match self {
            Language::English => "Thermal",
            Language::Vietnamese => "Nhiệt",
        }
    }

    pub fn system_thermal_state(self, state: Thermal) -> &'static str {
        match (self, state) {
            (Language::English, Thermal::Nominal) => "Nominal",
            (Language::English, Thermal::Fair) => "Warm",
            (Language::English, Thermal::Serious) => "Hot",
            (Language::English, Thermal::Critical) => "Critical",
            (Language::Vietnamese, Thermal::Nominal) => "Bình thường",
            (Language::Vietnamese, Thermal::Fair) => "Ấm",
            (Language::Vietnamese, Thermal::Serious) => "Nóng",
            (Language::Vietnamese, Thermal::Critical) => "Quá nóng",
        }
    }

    pub fn system_fan(self) -> &'static str {
        match self {
            Language::English => "Fan",
            Language::Vietnamese => "Quạt",
        }
    }

    /// Under the speed of the fastest fan: how many there are and the fastest that one can turn.
    pub fn system_fan_detail(self, count: usize, max: &str) -> String {
        match (self, count) {
            (Language::English, 0 | 1) => format!("of {max}"),
            (Language::English, n) => format!("{n} fans, of {max}"),
            (Language::Vietnamese, 0 | 1) => format!("tối đa {max}"),
            (Language::Vietnamese, n) => format!("{n} quạt, tối đa {max}"),
        }
    }

    pub fn system_battery(self) -> &'static str {
        match self {
            Language::English => "Battery health",
            Language::Vietnamese => "Sức khỏe pin",
        }
    }

    pub fn system_cycles(self, cycles: u32) -> String {
        match self {
            Language::English => format!("{cycles} cycles"),
            Language::Vietnamese => format!("{cycles} lần sạc"),
        }
    }

    /// Shown in place of the figures while the feature is off.
    pub fn system_off_title(self) -> &'static str {
        match self {
            Language::English => "The System tab is off",
            Language::Vietnamese => "Tab Hệ thống đang tắt",
        }
    }

    pub fn system_off_hint(self) -> &'static str {
        match self {
            Language::English => "Turn it on in Settings, under Notch tabs, System.",
            Language::Vietnamese => "Bật trong Cài đặt, mục Tab của notch, Hệ thống.",
        }
    }

    /// Shown when the feature is on but every card has been switched off.
    pub fn system_empty_title(self) -> &'static str {
        match self {
            Language::English => "Nothing to show",
            Language::Vietnamese => "Không có gì để hiển thị",
        }
    }

    pub fn system_empty_hint(self) -> &'static str {
        match self {
            Language::English => {
                "Choose what this tab shows in Settings, under Notch tabs, System."
            }
            Language::Vietnamese => {
                "Chọn nội dung của tab trong Cài đặt, mục Tab của notch, Hệ thống."
            }
        }
    }
}
