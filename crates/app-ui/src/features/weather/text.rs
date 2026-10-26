//! The words of the Weather tab, in each language.

use super::logic::{AirLevel, Feel, RainLevel, Season, Sky, UvLevel, WindLevel};
use crate::i18n::Language;
use macos_sys::weather_ui::Verdict;

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

    /// The button under the note of a tab that has nothing to show until something is chosen in the settings.
    pub fn weather_open_settings(self) -> &'static str {
        match self {
            Language::English => "Open Settings",
            Language::Vietnamese => "Mở Cài đặt",
        }
    }

    pub fn weather_previous(self) -> &'static str {
        match self {
            Language::English => "Previous city",
            Language::Vietnamese => "Thành phố trước",
        }
    }

    pub fn weather_next(self) -> &'static str {
        match self {
            Language::English => "Next city",
            Language::Vietnamese => "Thành phố sau",
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

    pub fn weather_details(self) -> &'static str {
        match self {
            Language::English => "Details",
            Language::Vietnamese => "Chi tiết",
        }
    }

    pub fn weather_forecast(self) -> &'static str {
        match self {
            Language::English => "Forecast",
            Language::Vietnamese => "Dự báo",
        }
    }

    pub fn weather_air_title(self) -> &'static str {
        match self {
            Language::English => "Air quality (AQI)",
            Language::Vietnamese => "Chất lượng không khí (AQI)",
        }
    }

    pub fn weather_air_missing(self) -> &'static str {
        match self {
            Language::English => "No air reading for this place",
            Language::Vietnamese => "Chưa có số đo không khí ở đây",
        }
    }

    pub fn weather_air_level(self, level: AirLevel) -> &'static str {
        match (self, level) {
            (Language::English, AirLevel::Good) => "Good",
            (Language::English, AirLevel::Moderate) => "Moderate",
            (Language::English, AirLevel::Sensitive) => "Unhealthy for some",
            (Language::English, AirLevel::Unhealthy) => "Unhealthy",
            (Language::English, AirLevel::VeryUnhealthy) => "Very unhealthy",
            (Language::English, AirLevel::Hazardous) => "Hazardous",
            (Language::Vietnamese, AirLevel::Good) => "Tốt",
            (Language::Vietnamese, AirLevel::Moderate) => "Trung bình",
            (Language::Vietnamese, AirLevel::Sensitive) => "Hại người nhạy cảm",
            (Language::Vietnamese, AirLevel::Unhealthy) => "Có hại",
            (Language::Vietnamese, AirLevel::VeryUnhealthy) => "Rất có hại",
            (Language::Vietnamese, AirLevel::Hazardous) => "Nguy hiểm",
        }
    }

    /// "PM2.5 21 µg/m³, 1.4x the WHO limit"
    pub fn weather_air_detail(self, pm25: f64, ratio: f64) -> String {
        match self {
            Language::English => format!("PM2.5 {pm25:.0} µg/m³, {ratio:.1}x the WHO limit"),
            Language::Vietnamese => {
                format!("PM2.5 {pm25:.0} µg/m³, gấp {ratio:.1} lần ngưỡng WHO")
            }
        }
    }

    /// The four things the air is judged for, in the order of `logic::air_advice`.
    pub fn weather_air_activity(self, which: usize) -> &'static str {
        const EN: [&str; 4] = [
            "Exercise outdoors",
            "Open windows",
            "Mask outdoors",
            "Children, elderly, asthma",
        ];
        const VI: [&str; 4] = [
            "Tập thể dục ngoài trời",
            "Mở cửa sổ",
            "Khẩu trang khi ra ngoài",
            "Trẻ em, người già, hen suyễn",
        ];
        match self {
            Language::English => EN[which % 4],
            Language::Vietnamese => VI[which % 4],
        }
    }

    pub fn weather_air_word(self, which: usize, verdict: Verdict) -> &'static str {
        const EN: [[&str; 3]; 4] = [
            ["Fine", "Take it easy", "Avoid"],
            ["Fine", "Briefly", "Keep shut"],
            ["Not needed", "Advised", "N95 needed"],
            ["Normal", "Limit time out", "Stay in"],
        ];
        const VI: [[&str; 3]; 4] = [
            ["Thoải mái", "Giảm cường độ", "Nên tránh"],
            ["Thoải mái", "Mở ngắn thôi", "Đóng cửa"],
            ["Không cần", "Nên đeo", "Cần loại N95"],
            ["Bình thường", "Hạn chế ra ngoài", "Ở trong nhà"],
        ];
        let column = match verdict {
            Verdict::Good => 0,
            Verdict::Caution => 1,
            Verdict::Avoid => 2,
        };
        match self {
            Language::English => EN[which % 4][column],
            Language::Vietnamese => VI[which % 4][column],
        }
    }

    pub fn weather_season_title(self) -> &'static str {
        match self {
            Language::English => "Season",
            Language::Vietnamese => "Mùa",
        }
    }

    pub fn weather_season(self, season: Season) -> &'static str {
        match (self, season) {
            (Language::English, Season::Spring) => "Spring",
            (Language::English, Season::Summer) => "Summer",
            (Language::English, Season::Autumn) => "Autumn",
            (Language::English, Season::Winter) => "Winter",
            (Language::English, Season::Wet) => "Rainy season",
            (Language::English, Season::Dry) => "Dry season",
            (Language::Vietnamese, Season::Spring) => "Mùa xuân",
            (Language::Vietnamese, Season::Summer) => "Mùa hè",
            (Language::Vietnamese, Season::Autumn) => "Mùa thu",
            (Language::Vietnamese, Season::Winter) => "Mùa đông",
            (Language::Vietnamese, Season::Wet) => "Mùa mưa",
            (Language::Vietnamese, Season::Dry) => "Mùa khô",
        }
    }

    pub fn weather_season_note(self, season: Season) -> &'static str {
        match (self, season) {
            (Language::English, Season::Spring) => "Warming up, drizzle and changeable days",
            (Language::English, Season::Summer) => "Hot, with storms likely in the afternoon",
            (Language::English, Season::Autumn) => "Mild and drier, falling leaves",
            (Language::English, Season::Winter) => "Cold, with cold snaps possible",
            (Language::English, Season::Wet) => "Frequent rain, often in the afternoon",
            (Language::English, Season::Dry) => "Little rain and plenty of sun",
            (Language::Vietnamese, Season::Spring) => "Ấm dần, mưa phùn, thời tiết hay đổi",
            (Language::Vietnamese, Season::Summer) => "Nóng, chiều dễ có mưa dông",
            (Language::Vietnamese, Season::Autumn) => "Dịu mát, ít mưa, lá rụng",
            (Language::Vietnamese, Season::Winter) => "Lạnh, có thể có đợt rét đậm",
            (Language::Vietnamese, Season::Wet) => "Mưa nhiều, thường vào chiều tối",
            (Language::Vietnamese, Season::Dry) => "Ít mưa, nắng nhiều",
        }
    }

    pub fn weather_sky_title(self) -> &'static str {
        match self {
            Language::English => "Rain and sun",
            Language::Vietnamese => "Mưa nắng",
        }
    }

    pub fn weather_rain_level(self, level: RainLevel) -> &'static str {
        match (self, level) {
            (Language::English, RainLevel::Dry) => "No rain",
            (Language::English, RainLevel::Drizzle) => "Drizzle",
            (Language::English, RainLevel::Light) => "Light rain",
            (Language::English, RainLevel::Moderate) => "Moderate rain",
            (Language::English, RainLevel::Heavy) => "Heavy rain",
            (Language::Vietnamese, RainLevel::Dry) => "Không mưa",
            (Language::Vietnamese, RainLevel::Drizzle) => "Mưa phùn",
            (Language::Vietnamese, RainLevel::Light) => "Mưa nhỏ",
            (Language::Vietnamese, RainLevel::Moderate) => "Mưa vừa",
            (Language::Vietnamese, RainLevel::Heavy) => "Mưa to",
        }
    }

    /// The size of the drops and whether there is lightning, with how much falls: "Fine drops, no thunder, 0.4 mm/h".
    pub fn weather_rain_note(self, level: RainLevel, thunder: bool, mm: f64) -> String {
        let drops = match (self, level) {
            (Language::English, RainLevel::Drizzle | RainLevel::Light) => "Small drops",
            (Language::English, RainLevel::Moderate) => "Medium drops",
            (Language::English, _) => "Big drops",
            (Language::Vietnamese, RainLevel::Drizzle | RainLevel::Light) => "Hạt nhỏ",
            (Language::Vietnamese, RainLevel::Moderate) => "Hạt vừa",
            (Language::Vietnamese, _) => "Hạt lớn",
        };
        let storm = match (self, thunder) {
            (Language::English, true) => "thunder and lightning",
            (Language::English, false) => "no thunder",
            (Language::Vietnamese, true) => "có sấm sét",
            (Language::Vietnamese, false) => "không sấm sét",
        };
        format!("{drops}, {storm}, {mm:.1} mm/h")
    }

    pub fn weather_snowing(self) -> &'static str {
        match self {
            Language::English => "Snowing",
            Language::Vietnamese => "Đang có tuyết",
        }
    }

    pub fn weather_uv_level(self, level: UvLevel) -> &'static str {
        match (self, level) {
            (Language::English, UvLevel::Low) => "low",
            (Language::English, UvLevel::Moderate) => "moderate",
            (Language::English, UvLevel::High) => "high",
            (Language::English, UvLevel::VeryHigh) => "very high",
            (Language::English, UvLevel::Extreme) => "extreme",
            (Language::Vietnamese, UvLevel::Low) => "thấp",
            (Language::Vietnamese, UvLevel::Moderate) => "vừa",
            (Language::Vietnamese, UvLevel::High) => "cao",
            (Language::Vietnamese, UvLevel::VeryHigh) => "rất cao",
            (Language::Vietnamese, UvLevel::Extreme) => "cực cao",
        }
    }

    /// "UV 8, very high"
    pub fn weather_uv(self, uv: f64, level: &str) -> String {
        match self {
            Language::English => format!("UV {uv:.0}, {level}"),
            Language::Vietnamese => format!("UV {uv:.0}, {level}"),
        }
    }

    /// What the sun is doing and what to do about it: "The sun is 62% of the way across. Wear sunscreen."
    pub fn weather_sun_note(self, percent: f64, level: UvLevel) -> String {
        let advice = match (self, level) {
            (Language::English, UvLevel::Low) => "No protection needed",
            (Language::English, UvLevel::Moderate) => "Sunglasses are enough",
            (Language::English, UvLevel::High | UvLevel::VeryHigh) => "Sunscreen and a hat",
            (Language::English, UvLevel::Extreme) => "Stay in shade at midday",
            (Language::Vietnamese, UvLevel::Low) => "Không cần che chắn",
            (Language::Vietnamese, UvLevel::Moderate) => "Đeo kính râm là đủ",
            (Language::Vietnamese, UvLevel::High | UvLevel::VeryHigh) => "Bôi chống nắng, đội nón",
            (Language::Vietnamese, UvLevel::Extreme) => "Tránh nắng giữa trưa",
        };
        match self {
            Language::English => format!("Sun {percent:.0}% across the sky. {advice}"),
            Language::Vietnamese => format!("Mặt trời qua {percent:.0}% quãng ngày. {advice}"),
        }
    }

    /// The chance of rain in the hours ahead, said only when the service gave one.
    pub fn weather_chance(self, chance: Option<f64>) -> String {
        match (self, chance) {
            (Language::English, Some(c)) => format!("Chance of rain in the next hours: {c:.0}%"),
            (Language::Vietnamese, Some(c)) => format!("Khả năng mưa vài giờ tới: {c:.0}%"),
            (_, None) => String::new(),
        }
    }

    /// "Southwest, gusts 38 km/h. Leaves blown loose"
    pub fn weather_wind_note(
        self,
        point: Option<usize>,
        gust: Option<f64>,
        unit: &str,
        effect: &str,
    ) -> String {
        let mut note = String::new();
        if let Some(point) = point {
            note.push_str(self.weather_compass(point));
        }
        if let Some(gust) = gust {
            if !note.is_empty() {
                note.push_str(", ");
            }
            note.push_str(&match self {
                Language::English => format!("gusts {gust:.0} {unit}"),
                Language::Vietnamese => format!("giật {gust:.0} {unit}"),
            });
        }
        if !note.is_empty() {
            note.push_str(". ");
        }
        note.push_str(effect);
        note
    }

    pub fn weather_wind_title(self) -> &'static str {
        match self {
            Language::English => "Wind",
            Language::Vietnamese => "Gió",
        }
    }

    pub fn weather_wind_level(self, level: WindLevel) -> &'static str {
        match (self, level) {
            (Language::English, WindLevel::Calm) => "Calm",
            (Language::English, WindLevel::Light) => "Light breeze",
            (Language::English, WindLevel::Moderate) => "Fresh breeze",
            (Language::English, WindLevel::Strong) => "Strong wind",
            (Language::English, WindLevel::Gale) => "Gale",
            (Language::English, WindLevel::Storm) => "Storm force",
            (Language::Vietnamese, WindLevel::Calm) => "Lặng gió",
            (Language::Vietnamese, WindLevel::Light) => "Gió nhẹ",
            (Language::Vietnamese, WindLevel::Moderate) => "Gió vừa",
            (Language::Vietnamese, WindLevel::Strong) => "Gió mạnh",
            (Language::Vietnamese, WindLevel::Gale) => "Gió rất mạnh",
            (Language::Vietnamese, WindLevel::Storm) => "Gió bão",
        }
    }

    /// What the wind does to the leaves and the trees.
    pub fn weather_wind_effect(self, level: WindLevel) -> &'static str {
        match (self, level) {
            (Language::English, WindLevel::Calm) => "Leaves hang still",
            (Language::English, WindLevel::Light) => "Leaves rustle a little",
            (Language::English, WindLevel::Moderate) => "Leaves and small twigs sway",
            (Language::English, WindLevel::Strong) => "Leaves blown loose, whole trees sway",
            (Language::English, WindLevel::Gale) => "Leaves swirl up, branches may break",
            (Language::English, WindLevel::Storm) => "Dangerous, stay indoors",
            (Language::Vietnamese, WindLevel::Calm) => "Lá cây đứng yên",
            (Language::Vietnamese, WindLevel::Light) => "Lá khẽ xào xạc",
            (Language::Vietnamese, WindLevel::Moderate) => "Lá và cành nhỏ đung đưa",
            (Language::Vietnamese, WindLevel::Strong) => "Lá bay lả tả, cây rung lắc",
            (Language::Vietnamese, WindLevel::Gale) => "Lá cuộn xoáy, cành có thể gãy",
            (Language::Vietnamese, WindLevel::Storm) => "Nguy hiểm, nên ở trong nhà",
        }
    }

    /// The compass point the wind blows from, 0 for north and clockwise.
    pub fn weather_compass(self, point: usize) -> &'static str {
        const EN: [&str; 8] = [
            "North",
            "Northeast",
            "East",
            "Southeast",
            "South",
            "Southwest",
            "West",
            "Northwest",
        ];
        const VI: [&str; 8] = [
            "Bắc",
            "Đông Bắc",
            "Đông",
            "Đông Nam",
            "Nam",
            "Tây Nam",
            "Tây",
            "Tây Bắc",
        ];
        match self {
            Language::English => EN[point % 8],
            Language::Vietnamese => VI[point % 8],
        }
    }

    pub fn weather_feel_title(self) -> &'static str {
        match self {
            Language::English => "Feels like",
            Language::Vietnamese => "Cảm giác",
        }
    }

    pub fn weather_feel(self, feel: Feel) -> &'static str {
        match (self, feel) {
            (Language::English, Feel::Freezing) => "Freezing",
            (Language::English, Feel::Cold) => "Cold",
            (Language::English, Feel::Cool) => "Cool",
            (Language::English, Feel::Mild) => "Comfortable",
            (Language::English, Feel::Warm) => "Warm",
            (Language::English, Feel::Hot) => "Hot",
            (Language::English, Feel::Scorching) => "Scorching",
            (Language::Vietnamese, Feel::Freezing) => "Rét buốt",
            (Language::Vietnamese, Feel::Cold) => "Lạnh",
            (Language::Vietnamese, Feel::Cool) => "Se lạnh",
            (Language::Vietnamese, Feel::Mild) => "Dễ chịu",
            (Language::Vietnamese, Feel::Warm) => "Ấm nóng",
            (Language::Vietnamese, Feel::Hot) => "Nóng gắt",
            (Language::Vietnamese, Feel::Scorching) => "Nóng cực độ",
        }
    }

    pub fn weather_feel_note(self, feel: Feel) -> &'static str {
        match (self, feel) {
            (Language::English, Feel::Freezing) => "Thick coat, scarf and gloves; frost forms",
            (Language::English, Feel::Cold) => "Wear a warm jacket and cover your neck",
            (Language::English, Feel::Cool) => "A light jacket is enough",
            (Language::English, Feel::Mild) => "Comfortable for any activity",
            (Language::English, Feel::Warm) => "Light clothes, drink enough water",
            (Language::English, Feel::Hot) => "Avoid the sun 11:00 to 15:00, drink water",
            (Language::English, Feel::Scorching) => "Risk of heatstroke, stay cool indoors",
            (Language::Vietnamese, Feel::Freezing) => "Áo dày, khăn, găng tay; có thể có sương giá",
            (Language::Vietnamese, Feel::Cold) => "Mặc áo khoác ấm, che cổ",
            (Language::Vietnamese, Feel::Cool) => "Một chiếc áo khoác mỏng là đủ",
            (Language::Vietnamese, Feel::Mild) => "Thoải mái cho mọi hoạt động",
            (Language::Vietnamese, Feel::Warm) => "Mặc đồ thoáng, uống đủ nước",
            (Language::Vietnamese, Feel::Hot) => "Tránh nắng 11h đến 15h, uống nhiều nước",
            (Language::Vietnamese, Feel::Scorching) => "Nguy cơ sốc nhiệt, ở nơi mát",
        }
    }
}
