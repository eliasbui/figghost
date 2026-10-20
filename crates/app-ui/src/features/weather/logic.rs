//! What the forecast service answers, turned into what the Weather tab shows: the places a city name can mean, the
//! weather now, the next hours and the next days. No network, no clock and no drawing, so the tests feed it the
//! real answers of the service kept in `testdata`.

use macos_sys::weather_ui::{Glyph, Hue};
use serde::{Deserialize, Serialize};
use serde_json::Value;

pub const FORECAST_URL: &str = "https://api.open-meteo.com/v1/forecast";
pub const GEOCODING_URL: &str = "https://geocoding-api.open-meteo.com/v1/search";
/// How many hours and days the tab shows.
pub const HOURS: usize = 6;
pub const DAYS: usize = 5;
/// How many places a search offers.
pub const MAX_PLACES: usize = 5;
/// The longest city name that is searched for, in characters.
pub const MAX_QUERY: usize = 60;

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
    let clean: String = text.chars().filter(|c| !c.is_control()).collect();
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

/// What a city name is searched with.
pub fn places_query(name: &str, language_code: &str) -> Vec<(&'static str, String)> {
    vec![
        ("name", name.to_owned()),
        ("count", MAX_PLACES.to_string()),
        ("language", language_code.to_owned()),
        ("format", "json".to_owned()),
    ]
}

/// A city name from the settings page: something to search for, or nothing.
pub fn query_of(text: &str) -> Option<String> {
    let clean: String = text
        .chars()
        .map(|c| if c.is_control() { ' ' } else { c })
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
    let now = Now {
        time: current["time"].as_str().ok_or("no time")?.to_owned(),
        temp: number(current, "temperature_2m")?,
        feels: number(current, "apparent_temperature")?,
        humidity: number(current, "relative_humidity_2m")?,
        wind: number(current, "wind_speed_10m")?,
        code: number(current, "weather_code")? as i64,
        day: number(current, "is_day")? >= 1.0,
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
    Ok(Forecast { now, hours, days })
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
    let valid = clock.len() >= 5
        && clock.as_bytes()[2] == b':'
        && clock[..2].bytes().all(|b| b.is_ascii_digit())
        && clock[3..5].bytes().all(|b| b.is_ascii_digit());
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
}
