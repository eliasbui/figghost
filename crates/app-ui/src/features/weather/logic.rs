//! What the forecast service answers, turned into what the Weather tab shows: the places a city name can mean, the
//! weather now, the next hours and the next days. No network, no clock and no drawing, so the tests feed it the
//! real answers of the service kept in `testdata`.

use macos_sys::weather_ui::{Atmosphere, Glyph, Hue, Verdict};
use serde::{Deserialize, Serialize};
use serde_json::Value;

pub const FORECAST_URL: &str = "https://api.open-meteo.com/v1/forecast";
pub const GEOCODING_URL: &str = "https://geocoding-api.open-meteo.com/v1/search";
pub const AIR_URL: &str = "https://air-quality-api.open-meteo.com/v1/air-quality";
/// How many hours and days the tab shows.
pub const HOURS: usize = 6;
pub const DAYS: usize = 5;
/// How many places a search offers.
pub const MAX_PLACES: usize = 5;
/// The longest city name that is searched for, in characters.
pub const MAX_QUERY: usize = 60;
/// How many cities the tab flips through.
pub const MAX_CITIES: usize = 8;

/// A place on Earth, as the service names it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Place {
    pub name: String,
    #[serde(default)]
    pub region: String,
    #[serde(default)]
    pub country: String,
    pub latitude: f64,
    pub longitude: f64,
}

fn tidy(text: &str) -> String {
    let clean: String = text
        .chars()
        .filter(|c| !crate::features::is_hidden_char(*c))
        .collect();
    clean.trim().chars().take(80).collect()
}

impl Place {
    /// A place read from a file or an answer can say anything: it keeps only a name and coordinates that are real.
    pub fn sanitized(self) -> Option<Self> {
        let place = Self {
            name: tidy(&self.name),
            region: tidy(&self.region),
            country: tidy(&self.country),
            ..self
        };
        let real = place.latitude.is_finite()
            && place.longitude.is_finite()
            && (-90.0..=90.0).contains(&place.latitude)
            && (-180.0..=180.0).contains(&place.longitude);
        (real && !place.name.is_empty()).then_some(place)
    }

    /// Whether two places get the same forecast, which is asked for coordinates to four decimals.
    pub fn same_spot(&self, other: &Place) -> bool {
        let spot = |p: &Place| format!("{:.4},{:.4}", p.latitude, p.longitude);
        spot(self) == spot(other)
    }

    /// "Springfield, Illinois, United States"; the region is left out when it only repeats the name.
    pub fn label(&self) -> String {
        let mut parts = vec![self.name.as_str()];
        if !self.region.is_empty() && self.region != self.name {
            parts.push(&self.region);
        }
        if !self.country.is_empty() {
            parts.push(&self.country);
        }
        parts.join(", ")
    }
}

/// The cities a file or the page gives: real places only, each once, no more than `MAX_CITIES`, in their order.
pub fn tidy_places(places: impl IntoIterator<Item = Place>) -> Vec<Place> {
    let mut out: Vec<Place> = Vec::new();
    for place in places.into_iter().filter_map(Place::sanitized) {
        if out.len() < MAX_CITIES && !out.iter().any(|p| p.same_spot(&place)) {
            out.push(place);
        }
    }
    out
}

/// A letter only Vietnamese writes: đ, ă, ơ, ư, a vowel with a tone mark and a hat or a hook (the block U+1EA0 to
/// U+1EF9), or the hook and the dot below of a name typed in decomposed form.
fn vietnamese_letter(c: char) -> bool {
    matches!(
        c,
        'đ' | 'Đ' | 'ă' | 'Ă' | 'ơ' | 'Ơ' | 'ư' | 'Ư' | '\u{1EA0}'
            ..='\u{1EF9}' | '\u{0309}' | '\u{0323}'
    )
}

/// What a city name is searched with. The service matches a name only against the names it knows in the language asked
/// for, so a Vietnamese name searched in English finds nothing ("Hải Phòng") or a village in place of the capital
/// ("Hà Nội"); a name with Vietnamese letters is therefore always asked in Vietnamese.
pub fn places_query(name: &str, language_code: &str) -> Vec<(&'static str, String)> {
    let language = if name.chars().any(vietnamese_letter) {
        "vi"
    } else {
        language_code
    };
    vec![
        ("name", name.to_owned()),
        ("count", MAX_PLACES.to_string()),
        ("language", language.to_owned()),
        ("format", "json".to_owned()),
    ]
}

/// A city name from the settings page: something to search for, or nothing.
pub fn query_of(text: &str) -> Option<String> {
    let clean: String = text
        .chars()
        .map(|c| {
            if crate::features::is_hidden_char(c) {
                ' '
            } else {
                c
            }
        })
        .collect();
    let clean = clean.split_whitespace().collect::<Vec<_>>().join(" ");
    let length = clean.chars().count();
    (2..=MAX_QUERY).contains(&length).then_some(clean)
}

/// The places in the answer to a search; none when the service found nothing.
pub fn parse_places(body: &str) -> Result<Vec<Place>, String> {
    let answer: Value = serde_json::from_str(body).map_err(|e| e.to_string())?;
    let Some(list) = answer.get("results") else {
        return Ok(Vec::new());
    };
    let list = list.as_array().ok_or("results is not a list")?;
    let text = |item: &Value, key: &str| item[key].as_str().unwrap_or_default().to_owned();
    Ok(list
        .iter()
        .filter_map(|item| {
            Place {
                name: text(item, "name"),
                region: text(item, "admin1"),
                country: text(item, "country"),
                latitude: item["latitude"].as_f64()?,
                longitude: item["longitude"].as_f64()?,
            }
            .sanitized()
        })
        .take(MAX_PLACES)
        .collect())
}

/// What a forecast is asked for. The service converts the units, so none is converted here.
pub fn forecast_query(place: &Place, imperial: bool) -> Vec<(&'static str, String)> {
    let list = |names: &[&str]| names.join(",");
    vec![
        ("latitude", format!("{:.4}", place.latitude)),
        ("longitude", format!("{:.4}", place.longitude)),
        (
            "current",
            list(&[
                "temperature_2m",
                "apparent_temperature",
                "relative_humidity_2m",
                "is_day",
                "weather_code",
                "wind_speed_10m",
                "wind_gusts_10m",
                "wind_direction_10m",
                "precipitation",
                "cloud_cover",
                "uv_index",
            ]),
        ),
        (
            "hourly",
            list(&[
                "temperature_2m",
                "weather_code",
                "precipitation_probability",
                "is_day",
            ]),
        ),
        (
            "daily",
            list(&[
                "weather_code",
                "temperature_2m_max",
                "temperature_2m_min",
                "sunrise",
                "sunset",
            ]),
        ),
        ("timezone", "auto".to_owned()),
        ("forecast_days", (DAYS + 1).to_string()),
        (
            "temperature_unit",
            if imperial { "fahrenheit" } else { "celsius" }.to_owned(),
        ),
        (
            "wind_speed_unit",
            if imperial { "mph" } else { "kmh" }.to_owned(),
        ),
    ]
}

#[derive(Debug, Clone, PartialEq)]
pub struct Now {
    /// The local time at the place, "2026-10-20T11:30".
    pub time: String,
    pub temp: f64,
    pub feels: f64,
    pub humidity: f64,
    pub wind: f64,
    pub code: i64,
    pub day: bool,
    /// The strongest gust, in the units asked for; the wind itself when the service does not say.
    pub gust: f64,
    /// Where the wind blows from, in degrees.
    pub direction: Option<f64>,
    /// The rain falling now, in millimetres an hour.
    pub precip: f64,
    /// How much of the sky is cloud, in percent, when the service says.
    pub cloud: Option<f64>,
    pub uv: f64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Hour {
    /// "2026-10-20T14:00", local to the place.
    pub time: String,
    pub temp: f64,
    pub code: i64,
    /// The chance of rain, in percent, when the service says.
    pub rain: Option<f64>,
    pub day: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Day {
    /// "2026-10-20"
    pub date: String,
    pub code: i64,
    pub high: f64,
    pub low: f64,
    pub sunrise: String,
    pub sunset: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Forecast {
    pub now: Now,
    pub hours: Vec<Hour>,
    pub days: Vec<Day>,
    /// The air at the place, which comes from another service and may be missing without the forecast being so.
    pub air: Option<Air>,
}

/// The numbers of a list in the answer; a gap in the service's data stays a gap.
fn numbers(block: &Value, key: &str) -> Option<Vec<Option<f64>>> {
    block
        .get(key)?
        .as_array()
        .map(|list| list.iter().map(Value::as_f64).collect())
}

fn strings(block: &Value, key: &str) -> Option<Vec<String>> {
    block.get(key)?.as_array().map(|list| {
        list.iter()
            .map(|v| v.as_str().unwrap_or_default().to_owned())
            .collect()
    })
}

fn number(block: &Value, key: &str) -> Result<f64, String> {
    block
        .get(key)
        .and_then(Value::as_f64)
        .filter(|n| n.is_finite())
        .ok_or_else(|| format!("no {key}"))
}

/// The forecast in an answer; an answer that lacks what the tab shows is an error, a gap in a list is not.
pub fn parse_forecast(body: &str) -> Result<Forecast, String> {
    let answer: Value = serde_json::from_str(body).map_err(|e| e.to_string())?;
    let current = answer.get("current").ok_or("no current weather")?;
    let wind = number(current, "wind_speed_10m")?;
    let optional = |key: &str| {
        current
            .get(key)
            .and_then(Value::as_f64)
            .filter(|n| n.is_finite())
    };
    let now = Now {
        time: current["time"].as_str().ok_or("no time")?.to_owned(),
        temp: number(current, "temperature_2m")?,
        feels: number(current, "apparent_temperature")?,
        humidity: number(current, "relative_humidity_2m")?,
        wind,
        code: number(current, "weather_code")? as i64,
        day: number(current, "is_day")? >= 1.0,
        gust: optional("wind_gusts_10m").unwrap_or(wind).max(wind),
        direction: optional("wind_direction_10m"),
        precip: optional("precipitation").unwrap_or(0.0).max(0.0),
        cloud: optional("cloud_cover").map(|c| c.clamp(0.0, 100.0)),
        uv: optional("uv_index").unwrap_or(0.0).max(0.0),
    };

    let hourly = answer.get("hourly").ok_or("no hourly forecast")?;
    let times = strings(hourly, "time").ok_or("no hourly times")?;
    let temps = numbers(hourly, "temperature_2m").ok_or("no hourly temperatures")?;
    let codes = numbers(hourly, "weather_code").ok_or("no hourly codes")?;
    let rains = numbers(hourly, "precipitation_probability");
    let days = numbers(hourly, "is_day");
    let hours = times
        .iter()
        .enumerate()
        .filter_map(|(i, time)| {
            Some(Hour {
                time: time.clone(),
                temp: (*temps.get(i)?)?,
                code: (*codes.get(i)?)? as i64,
                rain: rains.as_ref().and_then(|r| r.get(i).copied().flatten()),
                day: days
                    .as_ref()
                    .and_then(|d| d.get(i).copied().flatten())
                    .is_none_or(|d| d >= 1.0),
            })
        })
        .collect();

    let daily = answer.get("daily").ok_or("no daily forecast")?;
    let dates = strings(daily, "time").ok_or("no daily dates")?;
    let codes = numbers(daily, "weather_code").ok_or("no daily codes")?;
    let highs = numbers(daily, "temperature_2m_max").ok_or("no daily highs")?;
    let lows = numbers(daily, "temperature_2m_min").ok_or("no daily lows")?;
    let sunrises = strings(daily, "sunrise").unwrap_or_default();
    let sunsets = strings(daily, "sunset").unwrap_or_default();
    let days = dates
        .iter()
        .enumerate()
        .filter_map(|(i, date)| {
            Some(Day {
                date: date.clone(),
                code: (*codes.get(i)?)? as i64,
                high: (*highs.get(i)?)?,
                low: (*lows.get(i)?)?,
                sunrise: sunrises.get(i).cloned().unwrap_or_default(),
                sunset: sunsets.get(i).cloned().unwrap_or_default(),
            })
        })
        .collect();
    Ok(Forecast {
        now,
        hours,
        days,
        air: None,
    })
}

impl Forecast {
    /// The hours from the one now on, as many as `count`.
    pub fn upcoming_hours(&self, count: usize) -> Vec<&Hour> {
        // Times are "YYYY-MM-DDTHH:MM", so what sorts later in text is later in time.
        let this_hour = self.now.time.get(..13).unwrap_or(&self.now.time);
        self.hours
            .iter()
            .filter(|h| h.time.as_str() >= this_hour)
            .take(count)
            .collect()
    }

    /// Today and the days after it, as many as `count`.
    pub fn upcoming_days(&self, count: usize) -> Vec<&Day> {
        let today = self.now.time.get(..10).unwrap_or(&self.now.time);
        self.days
            .iter()
            .filter(|d| d.date.as_str() >= today)
            .take(count)
            .collect()
    }

    /// Today's sunrise and sunset, as "05:54" and "17:28".
    pub fn sun(&self) -> Option<(String, String)> {
        let today = self.upcoming_days(1).into_iter().next()?;
        let (rise, set) = (clock_of(&today.sunrise)?, clock_of(&today.sunset)?);
        Some((rise, set))
    }
}

/// "05:54" from "2026-10-20T05:54".
pub fn clock_of(time: &str) -> Option<String> {
    let clock = time.split('T').nth(1)?;
    let bytes = clock.as_bytes();
    // Checked a byte at a time: text from the network may have a multi-byte character where a digit should be, and
    // cutting a string inside one panics.
    let valid = bytes.len() >= 5
        && bytes[2] == b':'
        && [0, 1, 3, 4].iter().all(|&i| bytes[i].is_ascii_digit());
    // All five are ASCII, so the cut is on a character boundary.
    valid.then(|| clock[..5].to_owned())
}

/// The hour of "2026-10-20T14:00", 0 to 23.
pub fn hour_of(time: &str) -> Option<u32> {
    clock_of(time)?[..2].parse().ok().filter(|h| *h < 24)
}

/// The day of the week, 0 for Sunday, of "2026-10-20".
pub fn weekday_of(date: &str) -> Option<usize> {
    let mut parts = date.get(..10)?.split('-');
    let year: i64 = parts.next()?.parse().ok()?;
    let month: i64 = parts.next()?.parse().ok()?;
    let day: i64 = parts.next()?.parse().ok()?;
    if !(1..=12).contains(&month) || !(1..=31).contains(&day) {
        return None;
    }
    // Days since 1970-01-01 of a civil date; 1970-01-01 was a Thursday.
    let y = if month <= 2 { year - 1 } else { year };
    let era = y.div_euclid(400);
    let yoe = y - era * 400;
    let doy = (153 * (month + if month > 2 { -3 } else { 9 }) + 2) / 5 + day - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    let days = era * 146_097 + doe - 719_468;
    Some((days + 4).rem_euclid(7) as usize)
}

/// How the weather looks, from the service's WMO code.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Sky {
    Clear,
    PartlyCloudy,
    Overcast,
    Fog,
    Drizzle,
    Rain,
    HeavyRain,
    Snow,
    Thunderstorm,
}

pub fn sky(code: i64) -> Sky {
    match code {
        0 | 1 => Sky::Clear,
        2 => Sky::PartlyCloudy,
        45 | 48 => Sky::Fog,
        51..=57 => Sky::Drizzle,
        61 | 63 | 66 | 80 | 81 => Sky::Rain,
        65 | 67 | 82 => Sky::HeavyRain,
        71..=77 | 85 | 86 => Sky::Snow,
        95 | 96 | 99 => Sky::Thunderstorm,
        // 3, and anything the service adds later, is told as cloud.
        _ => Sky::Overcast,
    }
}

/// The glyph of a sky, by day or by night.
pub fn glyph(sky: Sky, day: bool) -> Glyph {
    let (symbol, hue) = match (sky, day) {
        (Sky::Clear, true) => ("sun.max.fill", Hue::Sun),
        (Sky::Clear, false) => ("moon.stars.fill", Hue::Moon),
        (Sky::PartlyCloudy, true) => ("cloud.sun.fill", Hue::Cloud),
        (Sky::PartlyCloudy, false) => ("cloud.moon.fill", Hue::Cloud),
        (Sky::Overcast, _) => ("cloud.fill", Hue::Cloud),
        (Sky::Fog, _) => ("cloud.fog.fill", Hue::Fog),
        (Sky::Drizzle, _) => ("cloud.drizzle.fill", Hue::Rain),
        (Sky::Rain, _) => ("cloud.rain.fill", Hue::Rain),
        (Sky::HeavyRain, _) => ("cloud.heavyrain.fill", Hue::Rain),
        (Sky::Snow, _) => ("cloud.snow.fill", Hue::Snow),
        (Sky::Thunderstorm, _) => ("cloud.bolt.rain.fill", Hue::Storm),
    };
    Glyph { symbol, hue }
}

/// "31°", rounded, with no "-0".
pub fn temp_text(degrees: f64) -> String {
    let rounded = degrees.round();
    format!("{}°", if rounded == 0.0 { 0.0 } else { rounded })
}

/// The chance of rain as "40%"; empty while it is slight, which is not worth a line.
pub fn rain_text(percent: Option<f64>) -> String {
    match percent {
        Some(p) if p >= 20.0 => format!("{:.0}%", (p / 5.0).round() * 5.0),
        _ => String::new(),
    }
}

/// What is asked of the air quality service: the index of the United States, which is the one with a scale of
/// advice everyone knows, and the fine dust it is made of.
pub fn air_query(place: &Place) -> Vec<(&'static str, String)> {
    vec![
        ("latitude", format!("{:.4}", place.latitude)),
        ("longitude", format!("{:.4}", place.longitude)),
        ("current", "us_aqi,pm2_5,pm10".to_owned()),
        ("timezone", "auto".to_owned()),
    ]
}

#[derive(Debug, Clone, PartialEq)]
pub struct Air {
    /// The United States Air Quality Index, 0 up.
    pub aqi: f64,
    /// Fine dust and coarse dust, in micrograms a cubic metre.
    pub pm25: Option<f64>,
    pub pm10: Option<f64>,
}

pub fn parse_air(body: &str) -> Result<Air, String> {
    let answer: Value = serde_json::from_str(body).map_err(|e| e.to_string())?;
    let current = answer.get("current").ok_or("no current air")?;
    let value = |key: &str| {
        current
            .get(key)
            .and_then(Value::as_f64)
            .filter(|n| n.is_finite() && *n >= 0.0)
    };
    Ok(Air {
        aqi: value("us_aqi").ok_or("no air index")?,
        pm25: value("pm2_5"),
        pm10: value("pm10"),
    })
}

/// The bands of the index, as the United States names them.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AirLevel {
    Good,
    Moderate,
    Sensitive,
    Unhealthy,
    VeryUnhealthy,
    Hazardous,
}

pub fn air_level(aqi: f64) -> AirLevel {
    match aqi {
        a if a <= 50.0 => AirLevel::Good,
        a if a <= 100.0 => AirLevel::Moderate,
        a if a <= 150.0 => AirLevel::Sensitive,
        a if a <= 200.0 => AirLevel::Unhealthy,
        a if a <= 300.0 => AirLevel::VeryUnhealthy,
        _ => AirLevel::Hazardous,
    }
}

pub fn air_rgb(level: AirLevel) -> (f64, f64, f64) {
    match level {
        AirLevel::Good => (0.35, 0.86, 0.50),
        AirLevel::Moderate => (0.98, 0.85, 0.25),
        AirLevel::Sensitive => (1.0, 0.62, 0.22),
        AirLevel::Unhealthy => (1.0, 0.40, 0.36),
        AirLevel::VeryUnhealthy | AirLevel::Hazardous => (0.72, 0.42, 0.90),
    }
}

/// Where the index stands on the bar, which ends at 300.
pub fn air_position(aqi: f64) -> f64 {
    (aqi / 300.0).clamp(0.0, 1.0)
}

/// How the air suits, in this order: exercise outdoors, open windows, going without a mask, and the people who feel
/// it first (children, the old, those with asthma or a heart condition).
pub fn air_advice(level: AirLevel) -> [Verdict; 4] {
    use Verdict::{Avoid, Caution, Good};
    match level {
        AirLevel::Good => [Good; 4],
        AirLevel::Moderate => [Good, Good, Good, Caution],
        AirLevel::Sensitive => [Caution, Caution, Caution, Avoid],
        AirLevel::Unhealthy => [Avoid, Avoid, Caution, Avoid],
        AirLevel::VeryUnhealthy | AirLevel::Hazardous => [Avoid; 4],
    }
}

/// The World Health Organization's limit for the day, in micrograms a cubic metre of fine dust.
const WHO_PM25: f64 = 15.0;

/// How many times the limit the fine dust is.
pub fn who_ratio(pm25: f64) -> f64 {
    pm25 / WHO_PM25
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WindLevel {
    Calm,
    Light,
    Moderate,
    Strong,
    Gale,
    Storm,
}

/// The wind by its speed in kilometres an hour.
pub fn wind_level(kmh: f64) -> WindLevel {
    match kmh {
        v if v < 2.0 => WindLevel::Calm,
        v if v < 12.0 => WindLevel::Light,
        v if v < 29.0 => WindLevel::Moderate,
        v if v < 50.0 => WindLevel::Strong,
        v if v < 75.0 => WindLevel::Gale,
        _ => WindLevel::Storm,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum RainLevel {
    Dry,
    Drizzle,
    Light,
    Moderate,
    Heavy,
}

impl RainLevel {
    /// How hard it falls on the screen, from 0 to 1.
    pub fn intensity(self) -> f64 {
        match self {
            RainLevel::Dry => 0.0,
            RainLevel::Drizzle => 0.15,
            RainLevel::Light => 0.35,
            RainLevel::Moderate => 0.62,
            RainLevel::Heavy => 0.95,
        }
    }
}

/// The rain now: what the code says it looks like, or what the gauge says it measures, whichever is the harder.
pub fn rain_level(code: i64, mm_an_hour: f64) -> RainLevel {
    let by_code = match sky(code) {
        Sky::Drizzle => RainLevel::Drizzle,
        Sky::Rain => RainLevel::Moderate,
        Sky::HeavyRain => RainLevel::Heavy,
        Sky::Thunderstorm => RainLevel::Moderate,
        _ => RainLevel::Dry,
    };
    let by_gauge = match mm_an_hour {
        m if m < 0.05 => RainLevel::Dry,
        m if m < 0.5 => RainLevel::Drizzle,
        m if m < 2.5 => RainLevel::Light,
        m if m < 7.6 => RainLevel::Moderate,
        _ => RainLevel::Heavy,
    };
    by_code.max(by_gauge)
}

pub fn is_thunder(code: i64) -> bool {
    matches!(code, 95 | 96 | 99)
}

/// How hard it snows, from 0 to 1.
pub fn snow_intensity(code: i64) -> f64 {
    match code {
        71 | 77 => 0.35,
        73 | 85 => 0.6,
        75 | 86 => 1.0,
        _ => 0.0,
    }
}

/// What the air feels like on the skin, from the apparent temperature in degrees Celsius.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Feel {
    Freezing,
    Cold,
    Cool,
    Mild,
    Warm,
    Hot,
    Scorching,
}

pub fn feel(celsius: f64) -> Feel {
    match celsius {
        c if c <= 5.0 => Feel::Freezing,
        c if c <= 12.0 => Feel::Cold,
        c if c <= 18.0 => Feel::Cool,
        c if c <= 27.0 => Feel::Mild,
        c if c <= 32.0 => Feel::Warm,
        c if c <= 37.0 => Feel::Hot,
        _ => Feel::Scorching,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UvLevel {
    Low,
    Moderate,
    High,
    VeryHigh,
    Extreme,
}

pub fn uv_level(uv: f64) -> UvLevel {
    match uv {
        u if u < 3.0 => UvLevel::Low,
        u if u < 6.0 => UvLevel::Moderate,
        u if u < 8.0 => UvLevel::High,
        u if u < 11.0 => UvLevel::VeryHigh,
        _ => UvLevel::Extreme,
    }
}

/// The eight points of the compass, from the degrees the wind blows from; 0 is north.
pub fn compass(degrees: f64) -> usize {
    (((degrees.rem_euclid(360.0) + 22.5) / 45.0) as usize) % 8
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Season {
    Spring,
    Summer,
    Autumn,
    Winter,
    /// The tropics have two seasons, not four: the one with the rains and the one without.
    Wet,
    Dry,
}

/// The season of a place in a month (1 to 12). Away from the tropics there are four, turned round in the south; in
/// them, the rains come in the summer half of the year of their side of the equator. A guide, not a calendar of the
/// local weather service.
pub fn season(latitude: f64, month: u32) -> Season {
    let month = month.clamp(1, 12);
    if latitude.abs() < 20.0 {
        let northern_summer = (5..=10).contains(&month);
        return if northern_summer == (latitude >= 0.0) {
            Season::Wet
        } else {
            Season::Dry
        };
    }
    let month = if latitude < 0.0 {
        (month + 5) % 12 + 1
    } else {
        month
    };
    match month {
        3..=5 => Season::Spring,
        6..=8 => Season::Summer,
        9..=11 => Season::Autumn,
        _ => Season::Winter,
    }
}

fn leaf_colour(season: Season) -> (f64, f64, f64) {
    match season {
        Season::Spring => (1.0, 0.72, 0.80),
        Season::Summer | Season::Wet => (0.40, 0.78, 0.32),
        Season::Autumn => (0.95, 0.56, 0.16),
        Season::Winter => (0.62, 0.46, 0.30),
        Season::Dry => (0.80, 0.68, 0.30),
    }
}

/// "05:54" or "2026-10-20T05:54" as minutes after midnight.
fn minutes(time: &str) -> Option<f64> {
    let clock = time.rsplit('T').next()?;
    let (hour, minute) = (clock.get(..2)?, clock.get(3..5)?);
    Some(f64::from(
        hour.parse::<u32>().ok()? * 60 + minute.parse::<u32>().ok()?,
    ))
}

/// How far through the daylight (by day) or the night (by night) it is, from 0 to 1.
pub fn sun_progress(now: &str, rise: &str, set: &str, day: bool) -> f64 {
    let Some(at) = minutes(now) else {
        return 0.5;
    };
    let (rise, set) = match (minutes(rise), minutes(set)) {
        (Some(r), Some(s)) if s > r => (r, s),
        _ => (360.0, 1080.0),
    };
    if day {
        ((at - rise) / (set - rise)).clamp(0.0, 1.0)
    } else {
        let since = if at >= set {
            at - set
        } else {
            at + 1440.0 - set
        };
        (since / (1440.0 - (set - rise))).clamp(0.0, 1.0)
    }
}

pub fn to_kmh(speed: f64, imperial: bool) -> f64 {
    if imperial { speed * 1.609_344 } else { speed }
}

pub fn to_celsius(degrees: f64, imperial: bool) -> f64 {
    if imperial {
        (degrees - 32.0) / 1.8
    } else {
        degrees
    }
}

impl Forecast {
    /// The sky behind the tab, from the weather now.
    pub fn atmosphere(&self, place: &Place, imperial: bool) -> Atmosphere {
        let now = &self.now;
        let by_sky = match sky(now.code) {
            Sky::Clear => 0.1,
            Sky::PartlyCloudy => 0.45,
            Sky::Overcast => 0.9,
            _ => 1.0,
        };
        let day = self.upcoming_days(1).into_iter().next();
        let (rise, set) = day.map_or(("", ""), |d| (d.sunrise.as_str(), d.sunset.as_str()));
        let wind = to_kmh(now.wind, imperial);
        let gust = to_kmh(now.gust, imperial);
        let month = now.time.get(5..7).and_then(|m| m.parse().ok()).unwrap_or(6);
        let travel = now.direction.map_or(1.0, |from| -from.to_radians().sin());
        let rain = rain_level(now.code, now.precip);
        Atmosphere {
            day: now.day,
            progress: sun_progress(&now.time, rise, set, now.day),
            cloud: now.cloud.map_or(by_sky, |c| c / 100.0),
            rain: rain.intensity(),
            thunder: is_thunder(now.code),
            snow: snow_intensity(now.code),
            fog: if sky(now.code) == Sky::Fog { 0.8 } else { 0.0 },
            cold: ((16.0 - to_celsius(now.feels, imperial)) / 22.0).clamp(0.0, 1.0),
            // A fresh breeze starts to show at 8 km/h and a storm fills the screen at 60.
            wind: ((wind * 0.65 + gust * 0.35 - 8.0) / 52.0).clamp(0.0, 1.0),
            wind_dir: if travel < 0.0 { -1.0 } else { 1.0 },
            haze: self
                .air
                .as_ref()
                .map_or(0.0, |a| ((a.aqi - 50.0) / 200.0).clamp(0.0, 1.0)),
            leaf: leaf_colour(season(place.latitude, month)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const FORECAST: &str = include_str!("testdata/forecast.json");
    const GEOCODING: &str = include_str!("testdata/geocoding.json");

    #[test]
    fn the_real_answer_to_a_search_is_read_into_places() {
        let places = parse_places(GEOCODING).unwrap();
        assert_eq!(places.len(), 5);
        assert_eq!(places[0].name, "Hanoi");
        assert_eq!(places[0].country, "Vietnam");
        assert_eq!(
            places[0].label(),
            "Hanoi, Vietnam",
            "a region that repeats the name is left out"
        );
        assert!((places[0].latitude - 21.0245).abs() < 1e-9);
        assert_eq!(
            places[2].label(),
            "Hanoi, Southern Highlands Province, Papua New Guinea"
        );
        assert_eq!(places[3].name, "Bạch Mai");
    }

    #[test]
    fn a_search_that_finds_nothing_is_not_an_error_and_nonsense_is() {
        assert_eq!(
            parse_places(r#"{"generationtime_ms":0.5}"#).unwrap(),
            Vec::new()
        );
        assert_eq!(parse_places(r#"{"results":[]}"#).unwrap(), Vec::new());
        assert!(parse_places("not json").is_err());
        assert!(parse_places(r#"{"results":"x"}"#).is_err());
    }

    #[test]
    fn a_place_with_nonsense_coordinates_or_no_name_is_dropped() {
        let body = r#"{"results":[
            {"name":"Nowhere","latitude":95.0,"longitude":10.0},
            {"name":"Lost","latitude":10.0,"longitude":-181.0},
            {"name":"","latitude":10.0,"longitude":10.0},
            {"name":"No coordinates"},
            {"name":"Bad\u0007Bell","latitude":1.5,"longitude":2.5,"country":"  Fiji  "}
        ]}"#;
        let places = parse_places(body).unwrap();
        assert_eq!(places.len(), 1);
        assert_eq!(places[0].name, "BadBell");
        assert_eq!(places[0].country, "Fiji");
    }

    #[test]
    fn a_name_is_held_to_what_can_be_searched_for() {
        assert_eq!(query_of("  Hà   Nội "), Some("Hà Nội".into()));
        assert_eq!(query_of("Hanoi\n"), Some("Hanoi".into()));
        assert_eq!(query_of("a"), None);
        assert_eq!(query_of(""), None);
        assert_eq!(query_of(" \t\n "), None);
        assert!(query_of(&"x".repeat(MAX_QUERY)).is_some());
        assert_eq!(query_of(&"x".repeat(MAX_QUERY + 1)), None);
        assert_eq!(query_of("Bad\u{0}Name"), Some("Bad Name".into()));
    }

    #[test]
    fn the_questions_carry_what_the_service_needs() {
        let search = places_query("Hà Nội", "vi");
        assert!(
            search.contains(&("name", "Hà Nội".into()))
                && search.contains(&("language", "vi".into()))
        );
        assert!(places_query("Hanoi", "en").contains(&("language", "en".into())));
        // In English the service finds no "Hải Phòng" at all and takes "Hà Nội" for a village; asked in Vietnamese it
        // finds both cities. A name of other languages keeps the app's language, so its labels stay in English.
        let nfd = "Ha\u{300} No\u{302}\u{323}i";
        for vietnamese in [
            "Hà Nội",
            "Hải Phòng",
            "Đà Nẵng",
            "Thành phố Hồ Chí Minh",
            "Huế",
            nfd,
        ] {
            let search = places_query(vietnamese, "en");
            assert!(search.contains(&("language", "vi".into())), "{vietnamese}");
        }
        for other in ["Kraków", "São Paulo", "Zürich", "Rhône"] {
            assert!(
                places_query(other, "en").contains(&("language", "en".into())),
                "{other}"
            );
        }
        let place = Place {
            name: "Hanoi".into(),
            region: String::new(),
            country: String::new(),
            latitude: 21.024_512,
            longitude: -105.841_17,
        };
        let metric = forecast_query(&place, false);
        assert!(metric.contains(&("latitude", "21.0245".into())));
        assert!(metric.contains(&("longitude", "-105.8412".into())));
        assert!(metric.contains(&("temperature_unit", "celsius".into())));
        assert!(metric.contains(&("wind_speed_unit", "kmh".into())));
        let imperial = forecast_query(&place, true);
        assert!(imperial.contains(&("temperature_unit", "fahrenheit".into())));
        assert!(imperial.contains(&("wind_speed_unit", "mph".into())));
        assert!(metric.contains(&("forecast_days", "6".into())));
        assert!(metric.contains(&("timezone", "auto".into())));
    }

    #[test]
    fn the_real_forecast_is_read_into_now_hours_and_days() {
        let forecast = parse_forecast(FORECAST).unwrap();
        assert_eq!(forecast.now.time, "2026-10-20T11:30");
        assert_eq!(forecast.now.code, 1);
        assert!(forecast.now.day);
        assert!((forecast.now.temp - 87.2).abs() < 1e-9);
        assert!((forecast.now.feels - 96.8).abs() < 1e-9);
        assert_eq!(forecast.now.humidity, 64.0);
        assert_eq!(forecast.hours.len(), 48);
        assert!(!forecast.hours[0].day, "midnight is night");
        assert_eq!(forecast.hours[0].rain, Some(1.0));
        assert_eq!(forecast.days.len(), 6);
        assert_eq!(forecast.days[0].date, "2026-10-20");
        assert_eq!(forecast.days[0].sunrise, "2026-10-20T05:54");
    }

    #[test]
    fn the_tab_shows_the_hours_from_now_on_and_the_days_from_today() {
        let forecast = parse_forecast(FORECAST).unwrap();
        let hours = forecast.upcoming_hours(HOURS);
        assert_eq!(hours.len(), HOURS);
        // It is 11:30, so the hour that has begun is the first: 11:00, then 12:00 on.
        assert_eq!(hours[0].time, "2026-10-20T11:00");
        assert_eq!(hours[5].time, "2026-10-20T16:00");
        let days = forecast.upcoming_days(DAYS);
        assert_eq!(days.len(), DAYS);
        assert_eq!(days[0].date, "2026-10-20");
        assert_eq!(days[4].date, "2026-10-24");
        assert_eq!(forecast.sun(), Some(("05:54".into(), "17:28".into())));
    }

    #[test]
    fn near_midnight_the_hours_run_into_the_next_day_and_the_list_may_be_short() {
        let mut forecast = parse_forecast(FORECAST).unwrap();
        forecast.now.time = "2026-10-20T22:45".into();
        let hours = forecast.upcoming_hours(4);
        assert_eq!(
            hours.iter().map(|h| h.time.as_str()).collect::<Vec<_>>(),
            [
                "2026-10-20T22:00",
                "2026-10-20T23:00",
                "2026-10-21T00:00",
                "2026-10-21T01:00"
            ]
        );
        forecast.now.time = "2030-01-01T00:00".into();
        assert!(
            forecast.upcoming_hours(4).is_empty(),
            "a forecast that is all in the past shows nothing"
        );
        assert!(forecast.sun().is_none());
    }

    #[test]
    fn a_gap_in_a_list_drops_that_entry_and_a_missing_block_is_an_error() {
        let gap = r#"{
          "current":{"time":"2026-10-20T11:30","temperature_2m":30.0,"apparent_temperature":33.0,"relative_humidity_2m":60,"is_day":1,"weather_code":2,"wind_speed_10m":5.0},
          "hourly":{"time":["2026-10-20T11:00","2026-10-20T12:00","2026-10-20T13:00"],"temperature_2m":[30.0,null,31.0],"weather_code":[2,2,null]},
          "daily":{"time":["2026-10-20"],"weather_code":[2],"temperature_2m_max":[32.0],"temperature_2m_min":[null]}
        }"#;
        let forecast = parse_forecast(gap).unwrap();
        assert_eq!(
            forecast.hours.len(),
            1,
            "only the entry with all it needs stays"
        );
        assert!(
            forecast.hours[0].day,
            "an unknown day or night is read as day"
        );
        assert_eq!(forecast.hours[0].rain, None);
        assert!(forecast.days.is_empty());
        for broken in [
            "{}",
            "not json",
            r#"{"current":{"time":"x"}}"#,
            r#"{"current":{"time":"x","temperature_2m":1,"apparent_temperature":1,"relative_humidity_2m":1,"is_day":1,"weather_code":1,"wind_speed_10m":1}}"#,
        ] {
            assert!(parse_forecast(broken).is_err(), "{broken}");
        }
    }

    #[test]
    fn a_time_gives_its_clock_and_hour_and_a_date_its_weekday() {
        assert_eq!(clock_of("2026-10-20T05:54"), Some("05:54".into()));
        assert_eq!(clock_of("2026-10-20T05:54:12"), Some("05:54".into()));
        assert_eq!(clock_of("2026-10-20"), None);
        assert_eq!(clock_of("2026-10-20Tab:cd"), None);
        // A character of more than one byte where a digit should be is not a time, and does not panic.
        for odd in [
            "2026-10-20T12:3é",
            "2026-10-20Té2:30",
            "2026-10-20T1é:30",
            "2026-10-20T12é30",
            "2026-10-20T12:é0",
            "2026-10-20T１２:30",
            "2026-10-20T",
        ] {
            assert_eq!(clock_of(odd), None, "{odd}");
            assert_eq!(hour_of(odd), None, "{odd}");
        }
        assert_eq!(hour_of("2026-10-20T14:00"), Some(14));
        assert_eq!(hour_of("2026-10-20T24:00"), None);
        // 2026-10-20 is a Tuesday, 1970-01-01 a Thursday, 2000-02-29 a Tuesday, 2024-03-10 a Sunday.
        assert_eq!(weekday_of("2026-10-20"), Some(2));
        assert_eq!(weekday_of("1970-01-01"), Some(4));
        assert_eq!(weekday_of("2000-02-29"), Some(2));
        assert_eq!(weekday_of("2024-03-10"), Some(0));
        assert_eq!(weekday_of("2026-13-01"), None);
        assert_eq!(weekday_of("nonsense"), None);
    }

    #[test]
    fn every_code_of_the_service_has_a_sky_and_every_sky_a_glyph_the_system_knows() {
        for code in [
            0, 1, 2, 3, 45, 48, 51, 53, 55, 56, 57, 61, 63, 65, 66, 67, 71, 73, 75, 77, 80, 81, 82,
            85, 86, 95, 96, 99, 1234, -1,
        ] {
            for day in [true, false] {
                let glyph = glyph(sky(code), day);
                assert!(
                    macos_sys::widgets::symbol_png(glyph.symbol, 16.0, 1.0).is_some(),
                    "{} is not a symbol of this system (code {code})",
                    glyph.symbol
                );
            }
        }
        assert_eq!(sky(0), Sky::Clear);
        assert_eq!(sky(3), Sky::Overcast);
        assert_eq!(sky(65), Sky::HeavyRain);
        assert_eq!(sky(96), Sky::Thunderstorm);
        assert_ne!(
            glyph(Sky::Clear, true).symbol,
            glyph(Sky::Clear, false).symbol
        );
    }

    #[test]
    fn figures_are_said_whole_and_a_slight_chance_of_rain_is_not_said() {
        assert_eq!(temp_text(30.7), "31°");
        assert_eq!(temp_text(-0.4), "0°");
        assert_eq!(temp_text(-2.5), "-3°");
        assert_eq!(temp_text(0.0), "0°");
        assert_eq!(rain_text(Some(19.0)), "");
        assert_eq!(rain_text(Some(20.0)), "20%");
        assert_eq!(rain_text(Some(43.0)), "45%");
        assert_eq!(rain_text(Some(100.0)), "100%");
        assert_eq!(rain_text(None), "");
    }

    #[test]
    fn a_place_from_a_file_is_held_to_what_is_real() {
        let place = |lat: f64, lon: f64, name: &str| Place {
            name: name.into(),
            region: "x".repeat(300),
            country: String::new(),
            latitude: lat,
            longitude: lon,
        };
        assert!(place(10.0, 20.0, "Ok").sanitized().is_some());
        assert_eq!(
            place(10.0, 20.0, "Ok")
                .sanitized()
                .unwrap()
                .region
                .chars()
                .count(),
            80
        );
        assert!(place(f64::NAN, 20.0, "Ok").sanitized().is_none());
        assert!(place(10.0, f64::INFINITY, "Ok").sanitized().is_none());
        assert!(place(90.1, 0.0, "Ok").sanitized().is_none());
        assert!(place(0.0, 0.0, " \n ").sanitized().is_none());
        assert!(place(-90.0, 180.0, "Edge").sanitized().is_some());
    }

    #[test]
    fn a_list_of_cities_keeps_real_places_once_each_and_no_more_than_the_cap() {
        let at = |name: &str, lat: f64| Place {
            name: name.into(),
            region: String::new(),
            country: String::new(),
            latitude: lat,
            longitude: 100.0,
        };
        let mut many: Vec<Place> = (0..12)
            .map(|i| at(&format!("C{i}"), f64::from(i)))
            .collect();
        many.insert(1, at("C0 again", 0.000_01));
        many.insert(2, at("Nowhere", 91.0));
        let list = tidy_places(many);
        assert_eq!(list.len(), MAX_CITIES);
        assert_eq!(
            list[0].name, "C0",
            "the first of two at the same spot stays"
        );
        assert_eq!(
            list[1].name, "C1",
            "a duplicate and a place that is not real are dropped"
        );
        assert!(
            at("a", 1.0).same_spot(&at("b", 1.000_04)) && !at("a", 1.0).same_spot(&at("b", 1.0002))
        );
    }

    #[test]
    fn the_air_answer_is_read_and_graded_with_advice_that_gets_stricter() {
        let air =
            parse_air(r#"{ "current": { "us_aqi": 132, "pm2_5": 48.2, "pm10": 70.0 } }"#).unwrap();
        assert_eq!((air.aqi, air.pm25), (132.0, Some(48.2)));
        let bare = parse_air(r#"{ "current": { "us_aqi": 20 } }"#).unwrap();
        assert!(bare.pm25.is_none() && bare.pm10.is_none());
        assert!(parse_air(r#"{ "current": { "pm2_5": 5 } }"#).is_err());
        assert!(parse_air(r#"{ "current": { "us_aqi": -3 } }"#).is_err());
        assert!(parse_air("not json").is_err());
        let levels: Vec<AirLevel> = [0.0, 50.0, 51.0, 101.0, 151.0, 201.0, 301.0]
            .map(air_level)
            .into();
        assert_eq!(
            levels,
            [
                AirLevel::Good,
                AirLevel::Good,
                AirLevel::Moderate,
                AirLevel::Sensitive,
                AirLevel::Unhealthy,
                AirLevel::VeryUnhealthy,
                AirLevel::Hazardous
            ]
        );
        // The worse the air, the fewer things it suits.
        let rank = |v: Verdict| v as u8;
        let mut last = [0u8; 4];
        for level in [
            AirLevel::Good,
            AirLevel::Moderate,
            AirLevel::Sensitive,
            AirLevel::Unhealthy,
            AirLevel::Hazardous,
        ] {
            let now = air_advice(level).map(rank);
            assert!(now.iter().zip(last).all(|(n, l)| *n >= l), "{level:?}");
            last = now;
        }
        assert_eq!(air_position(900.0), 1.0);
        assert!((who_ratio(30.0) - 2.0).abs() < 1e-9);
    }

    #[test]
    fn rain_wind_cold_and_sun_are_graded_by_what_they_do() {
        assert_eq!(rain_level(0, 0.0), RainLevel::Dry);
        assert_eq!(rain_level(51, 0.0), RainLevel::Drizzle);
        assert_eq!(rain_level(63, 0.0), RainLevel::Moderate);
        assert_eq!(
            rain_level(80, 12.0),
            RainLevel::Heavy,
            "the gauge may say more than the code"
        );
        assert_eq!(
            rain_level(65, 0.2),
            RainLevel::Heavy,
            "and the code more than the gauge"
        );
        assert!(RainLevel::Drizzle.intensity() < RainLevel::Heavy.intensity());
        assert!(is_thunder(95) && is_thunder(99) && !is_thunder(65));
        assert!(snow_intensity(75) > snow_intensity(71) && snow_intensity(61) == 0.0);
        assert_eq!(wind_level(1.0), WindLevel::Calm);
        assert_eq!(wind_level(20.0), WindLevel::Moderate);
        assert_eq!(wind_level(60.0), WindLevel::Gale);
        assert_eq!(feel(-3.0), Feel::Freezing);
        assert_eq!(feel(24.0), Feel::Mild);
        assert_eq!(feel(41.0), Feel::Scorching);
        assert_eq!(uv_level(2.0), UvLevel::Low);
        assert_eq!(uv_level(8.0), UvLevel::VeryHigh);
        assert_eq!(
            [0.0, 20.0, 46.0, 180.0, 270.0, 359.0].map(compass),
            [0, 0, 1, 4, 6, 0]
        );
    }

    #[test]
    fn the_season_follows_the_month_the_hemisphere_and_the_tropics() {
        // Hanoi in October is autumn; the same month in Sydney is spring.
        assert_eq!(season(21.0, 10), Season::Autumn);
        assert_eq!(season(-33.9, 10), Season::Spring);
        assert_eq!(season(21.0, 1), Season::Winter);
        assert_eq!(season(-33.9, 1), Season::Summer);
        // Ho Chi Minh City has the rains from May to October; Jakarta, south of the equator, the other half.
        assert_eq!(season(10.8, 7), Season::Wet);
        assert_eq!(season(10.8, 1), Season::Dry);
        assert_eq!(season(-6.2, 1), Season::Wet);
        assert_eq!(season(-6.2, 7), Season::Dry);
        for month in 1..=12 {
            let _ = season(45.0, month);
            let _ = season(-45.0, month);
        }
    }

    #[test]
    fn the_sun_goes_over_the_day_and_the_moon_over_the_night() {
        let at =
            |now: &str, day: bool| sun_progress(now, "2026-10-20T05:54", "2026-10-20T17:28", day);
        assert_eq!(at("2026-10-20T05:54", true), 0.0);
        assert!((at("2026-10-20T11:41", true) - 0.5).abs() < 0.01);
        assert_eq!(at("2026-10-20T17:28", true), 1.0);
        assert!(at("2026-10-20T23:40", false) > 0.4 && at("2026-10-20T23:40", false) < 0.6);
        assert!(
            at("2026-10-20T02:00", false) > 0.5,
            "after midnight the night is more than half gone"
        );
        // No sunrise to go by: the day is taken as six to six, and a nonsense time as noon.
        assert!((sun_progress("2026-10-20T12:00", "", "", true) - 0.5).abs() < 1e-9);
        assert_eq!(sun_progress("garbage", "", "", true), 0.5);
    }

    #[test]
    fn the_sky_behind_the_tab_follows_the_forecast() {
        let place = Place {
            name: "Hanoi".into(),
            region: String::new(),
            country: String::new(),
            latitude: 21.02,
            longitude: 105.84,
        };
        let mut forecast = parse_forecast(include_str!("testdata/forecast.json")).unwrap();
        let clear = forecast.atmosphere(&place, true);
        assert!(clear.day && clear.rain == 0.0 && !clear.thunder && clear.cold == 0.0);
        assert!(clear.progress > 0.2 && clear.progress < 0.8);
        assert!(clear.wind == 0.0, "4 mph is not a wind worth drawing");
        forecast.now.code = 95;
        forecast.now.precip = 12.0;
        forecast.now.wind = 40.0;
        forecast.now.gust = 70.0;
        forecast.now.direction = Some(270.0);
        forecast.air = Some(Air {
            aqi: 250.0,
            pm25: Some(120.0),
            pm10: None,
        });
        let storm = forecast.atmosphere(&place, false);
        assert!(storm.thunder && storm.rain > 0.9);
        assert!(
            storm.wind > 0.6 && storm.wind_dir > 0.0,
            "a west wind blows to the right"
        );
        assert_eq!(storm.haze, 1.0);
        forecast.now.direction = Some(90.0);
        forecast.now.feels = 14.0;
        let cold = forecast.atmosphere(&place, true);
        assert!(
            cold.wind_dir < 0.0 && cold.cold > 0.9,
            "14 degrees Fahrenheit is far below freezing"
        );
    }
}
