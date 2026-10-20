//! The words of the Weather tab, in each language.

use super::logic::Sky;
use crate::i18n::Language;

impl Language {
    pub fn weather_tab(self) -> &'static str {
        match self {
            Language::English => "Weather",
            Language::Vietnamese => "Thời tiết",
        }
    }

    pub fn weather_sky(self, sky: Sky) -> &'static str {
        match (self, sky) {
            (Language::English, Sky::Clear) => "Clear",
            (Language::English, Sky::PartlyCloudy) => "Partly cloudy",
            (Language::English, Sky::Overcast) => "Cloudy",
            (Language::English, Sky::Fog) => "Fog",
            (Language::English, Sky::Drizzle) => "Drizzle",
            (Language::English, Sky::Rain) => "Rain",
            (Language::English, Sky::HeavyRain) => "Heavy rain",
            (Language::English, Sky::Snow) => "Snow",
            (Language::English, Sky::Thunderstorm) => "Thunderstorm",
            (Language::Vietnamese, Sky::Clear) => "Trời quang",
            (Language::Vietnamese, Sky::PartlyCloudy) => "Có mây rải rác",
            (Language::Vietnamese, Sky::Overcast) => "Nhiều mây",
            (Language::Vietnamese, Sky::Fog) => "Sương mù",
            (Language::Vietnamese, Sky::Drizzle) => "Mưa phùn",
            (Language::Vietnamese, Sky::Rain) => "Mưa",
            (Language::Vietnamese, Sky::HeavyRain) => "Mưa to",
            (Language::Vietnamese, Sky::Snow) => "Tuyết",
            (Language::Vietnamese, Sky::Thunderstorm) => "Dông",
        }
    }

    pub fn weather_feels(self) -> &'static str {
        match self {
            Language::English => "Feels like",
            Language::Vietnamese => "Cảm giác như",
        }
    }

    pub fn weather_humidity(self) -> &'static str {
        match self {
            Language::English => "Humidity",
            Language::Vietnamese => "Độ ẩm",
        }
    }

    pub fn weather_wind(self) -> &'static str {
        match self {
            Language::English => "Wind",
            Language::Vietnamese => "Gió",
        }
    }

    pub fn weather_sunrise(self) -> &'static str {
        match self {
            Language::English => "Sunrise",
            Language::Vietnamese => "Bình minh",
        }
    }

    pub fn weather_sunset(self) -> &'static str {
        match self {
            Language::English => "Sunset",
            Language::Vietnamese => "Hoàng hôn",
        }
    }

    pub fn weather_today(self) -> &'static str {
        match self {
            Language::English => "Today",
            Language::Vietnamese => "Hôm nay",
        }
    }

    /// The short name of a day of the week, 0 for Sunday.
    pub fn weather_weekday(self, day: usize) -> &'static str {
        const EN: [&str; 7] = ["Sun", "Mon", "Tue", "Wed", "Thu", "Fri", "Sat"];
        const VI: [&str; 7] = ["CN", "T2", "T3", "T4", "T5", "T6", "T7"];
        match self {
            Language::English => EN[day % 7],
            Language::Vietnamese => VI[day % 7],
        }
    }

    /// "2 PM" in English, "14h" in Vietnamese.
    pub fn weather_hour(self, hour: u32) -> String {
        match self {
            Language::English => {
                let twelve = if hour % 12 == 0 { 12 } else { hour % 12 };
                format!("{twelve} {}", if hour < 12 { "AM" } else { "PM" })
            }
            Language::Vietnamese => format!("{hour}h"),
        }
    }

    /// "5:54 AM" in English, "05:54" in Vietnamese, from "05:54".
    pub fn weather_clock(self, clock: &str) -> String {
        let (Some(hour), Some(minute)) = (
            clock.get(..2).and_then(|h| h.parse::<u32>().ok()),
            clock.get(3..5),
        ) else {
            return clock.to_owned();
        };
        match self {
            Language::English => {
                let twelve = if hour % 12 == 0 { 12 } else { hour % 12 };
                format!("{twelve}:{minute} {}", if hour < 12 { "AM" } else { "PM" })
            }
            Language::Vietnamese => clock.to_owned(),
        }
    }

    pub fn weather_updated(self, ago: &str) -> String {
        match self {
            Language::English => format!("Updated {ago}"),
            Language::Vietnamese => format!("Cập nhật {ago}"),
        }
    }

    pub fn weather_updating(self) -> &'static str {
        match self {
            Language::English => "Updating...",
            Language::Vietnamese => "Đang cập nhật...",
        }
    }

    /// The forecast on show is old because the new one could not be fetched.
    pub fn weather_stale(self, ago: &str) -> String {
        match self {
            Language::English => format!("Offline, from {ago}"),
            Language::Vietnamese => format!("Mất kết nối, từ {ago}"),
        }
    }

    pub fn weather_credit(self) -> &'static str {
        match self {
            Language::English => "Data by Open-Meteo",
            Language::Vietnamese => "Dữ liệu: Open-Meteo",
        }
    }

    pub fn weather_off_title(self) -> &'static str {
        match self {
            Language::English => "The Weather tab is off",
            Language::Vietnamese => "Tab Thời tiết đang tắt",
        }
    }

    pub fn weather_off_hint(self) -> &'static str {
        match self {
            Language::English => "Turn it on in Settings, under Notch tabs, Weather.",
            Language::Vietnamese => "Bật trong Cài đặt, mục Tab của notch, Thời tiết.",
        }
    }

    pub fn weather_city_title(self) -> &'static str {
        match self {
            Language::English => "Choose a city",
            Language::Vietnamese => "Chọn một thành phố",
        }
    }

    pub fn weather_city_hint(self) -> &'static str {
        match self {
            Language::English => "Search for one in Settings, under Notch tabs, Weather.",
            Language::Vietnamese => "Tìm trong Cài đặt, mục Tab của notch, Thời tiết.",
        }
    }

    pub fn weather_loading_title(self) -> &'static str {
        match self {
            Language::English => "Getting the weather",
            Language::Vietnamese => "Đang lấy thời tiết",
        }
    }

    pub fn weather_loading_hint(self) -> &'static str {
        match self {
            Language::English => "One moment.",
            Language::Vietnamese => "Chờ một chút.",
        }
    }

    pub fn weather_offline_title(self) -> &'static str {
        match self {
            Language::English => "No connection",
            Language::Vietnamese => "Không có kết nối",
        }
    }

    pub fn weather_offline_hint(self) -> &'static str {
        match self {
            Language::English => {
                "The weather could not be fetched. It tries again when the tab opens."
            }
            Language::Vietnamese => "Chưa lấy được thời tiết. Sẽ thử lại khi bạn mở lại tab.",
        }
    }

    pub fn weather_service_title(self) -> &'static str {
        match self {
            Language::English => "No answer from the weather service",
            Language::Vietnamese => "Dịch vụ thời tiết không trả lời",
        }
    }

    pub fn weather_service_hint(self) -> &'static str {
        match self {
            Language::English => "Try again in a little while.",
            Language::Vietnamese => "Hãy thử lại sau ít phút.",
        }
    }
}
