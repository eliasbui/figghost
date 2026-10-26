//! The Weather tab of the notch: the weather now, the next hours and the next days of a city, one at a time when
//! several are chosen. It is off until the person turns it on and chooses a city, because it is the one tab that
//! talks to the internet.
//!
//! Forecasts come from Open-Meteo, which needs no account or key, through the system's `curl` as the other features
//! that go online do (see `core_smart::net`). What leaves the Mac is the coordinates of the city on screen, or the
//! name typed when searching for one; never the Mac's own location, which is not asked for. The tab asks only
//! while it is showing, only for the city it shows, and not again until that city's last answer is ten minutes old.

use super::AttachEnv;
use crate::{
    history::{ago, now_ms},
    i18n::{self, Language},
    settings::model::Fault,
    trace,
};
use core_smart::net;
use logic::{AIR_URL, DAYS, FORECAST_URL, Forecast, GEOCODING_URL, HOURS, MAX_CITIES, Place};
use macos_sys::{
    app,
    weather_ui::{
        Advice, AirCard, Current, Day, Hour, Hue, Insight, PAGE_SIZE, Pager, Tile,
        WeatherCallbacks, WeatherDisplay, WeatherView,
    },
};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::{
    cell::{Cell, RefCell},
    rc::{Rc, Weak},
};

pub mod logic;
pub mod settings;
mod text;

/// The two systems of units the forecast can be asked for.
pub const UNITS: [&str; 2] = ["metric", "imperial"];
/// A request is given up after this many seconds.
const FETCH_SECONDS: u64 = 15;
/// An answer younger than this is not asked for again.
const FRESH_MS: i64 = 10 * 60 * 1000;
/// While the tab stays open it asks again this often, and redraws how old the answer is at the tick.
const REFRESH_MS: i64 = 15 * 60 * 1000;
const TICK_SECONDS: f64 = 30.0;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    pub enabled: bool,
    /// "metric" (degrees Celsius, kilometres an hour) or "imperial" (degrees Fahrenheit, miles an hour).
    pub units: String,
    /// Rain, wind and thunder played softly behind the tab while it is open. Off until chosen, as a sound nobody asked
    /// for is a surprise.
    pub sound: bool,
    /// The cities chosen from searches, in the order the tab flips through them. A city of the wrong shape is dropped,
    /// not an unreadable file.
    #[serde(deserialize_with = "lenient_places")]
    pub places: Vec<Place>,
    /// The one city of a file written before there could be several. Read, never written: `sanitized` makes it the
    /// first of `places`.
    #[serde(deserialize_with = "lenient_place", skip_serializing)]
    pub place: Option<Place>,
}

fn lenient_place<'de, D: serde::Deserializer<'de>>(
    deserializer: D,
) -> Result<Option<Place>, D::Error> {
    let value = Value::deserialize(deserializer)?;
    Ok(serde_json::from_value(value).ok())
}

fn lenient_places<'de, D: serde::Deserializer<'de>>(
    deserializer: D,
) -> Result<Vec<Place>, D::Error> {
    let value = Value::deserialize(deserializer)?;
    Ok(value
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|item| serde_json::from_value(item.clone()).ok())
        .collect())
}

impl Default for Config {
    fn default() -> Self {
        Self {
            enabled: false,
            units: UNITS[0].into(),
            sound: false,
            places: Vec::new(),
            place: None,
        }
    }
}

impl Config {
    /// A config file can say anything: unknown units are metric, and a city that is not real, comes twice or is one too
    /// many is dropped. The city of an older file comes first.
    pub fn sanitized(mut self) -> Self {
        if !UNITS.contains(&self.units.as_str()) {
            self.units = UNITS[0].into();
        }
        self.places = logic::tidy_places(self.place.take().into_iter().chain(self.places));
        self
    }

    fn imperial(&self) -> bool {
        self.units == "imperial"
    }
}

/// A button of the settings page on the city at `index`: `remove` takes it out, `up` moves it one place earlier.
fn edit_places(places: &mut Vec<Place>, verb: &str, index: usize) -> Result<(), Fault> {
    if index >= places.len() {
        return Err(Fault::new("invalid_value", "a city of the list"));
    }
    match verb {
        "remove" => drop(places.remove(index)),
        // The first city is the one the tab opens on.
        "up" if index > 0 => places.swap(index, index - 1),
        _ => return Err(Fault::new("invalid_value", "a city that can move up")),
    }
    Ok(())
}

/// Adds a city found by a search at the end of the list.
fn add_place(places: &mut Vec<Place>, place: Place) -> Result<(), Fault> {
    if places.iter().any(|p| p.same_spot(&place)) {
        return Err(Fault::new("weather_twice", place.label()));
    }
    if places.len() >= MAX_CITIES {
        return Err(Fault::new("weather_full", MAX_CITIES.to_string()));
    }
    places.push(place);
    Ok(())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Problem {
    Offline,
    Service,
}

impl Problem {
    fn code(self) -> &'static str {
        match self {
            Problem::Offline => "offline",
            Problem::Service => "service",
        }
    }
}

/// What is known of one city in one system of units: its last forecast and when it came, a request on its way, and
/// why the last one failed. Kept per city and units, so that one for another city or other units is never shown.
struct Slot {
    place: Place,
    imperial: bool,
    fetched: Option<(Forecast, i64)>,
    busy: bool,
    problem: Option<Problem>,
}

impl Slot {
    fn new(place: Place, imperial: bool) -> Self {
        Self {
            place,
            imperial,
            fetched: None,
            busy: false,
            problem: None,
        }
    }

    fn is(&self, place: &Place, imperial: bool) -> bool {
        self.imperial == imperial && self.place.same_spot(place)
    }
}

#[derive(Default)]
struct Search {
    busy: bool,
    found: Vec<Place>,
    problem: Option<Problem>,
}

thread_local! {
    /// Answers arrive on another thread and hop to the main one, which finds the feature here.
    static FEATURE: RefCell<Weak<Feature>> = const { RefCell::new(Weak::new()) };
}

fn with_feature(f: impl FnOnce(&Feature)) {
    FEATURE.with(|slot| {
        if let Some(feature) = slot.borrow().upgrade() {
            f(&feature);
        }
    });
}

pub struct Feature {
    view: Rc<WeatherView>,
    config: RefCell<Config>,
    slots: RefCell<Vec<Slot>>,
    /// The city on screen, kept while the app runs; the first one when it is gone from the list.
    shown: RefCell<Option<Place>>,
    search: RefCell<Search>,
    search_generation: Cell<u64>,
    ticker: RefCell<Option<app::Timer>>,
    showing: Cell<bool>,
    me: Weak<Feature>,
}

/// Adds the tab. `None` without a notch, which is where the tab lives.
pub fn attach(env: &AttachEnv) -> Option<Rc<Feature>> {
    let notch = env.notch?;
    let callbacks = WeatherCallbacks {
        on_flip: Box::new(|step| with_feature(|f| f.flip(step))),
        on_action: Box::new(|| crate::settings::show_page("weather")),
    };
    let view = WeatherView::new(
        env.mtm,
        notch.page_area(PAGE_SIZE.width, PAGE_SIZE.height),
        callbacks,
    );
    let feature = Rc::new_cyclic(|me| Feature {
        view: Rc::clone(&view),
        config: RefCell::new(env.config.weather.clone().sanitized()),
        slots: RefCell::default(),
        shown: RefCell::default(),
        search: RefCell::default(),
        search_generation: Cell::new(0),
        ticker: RefCell::default(),
        showing: Cell::new(false),
        me: me.clone(),
    });
    FEATURE.with(|slot| *slot.borrow_mut() = Rc::downgrade(&feature));
    let (shown, hidden) = (Rc::downgrade(&feature), Rc::downgrade(&feature));
    notch.add_page(view.page(
        i18n::current().weather_tab().to_owned(),
        move || {
            if let Some(f) = shown.upgrade() {
                f.showing.set(true);
                f.begin();
            }
        },
        move || {
            if let Some(f) = hidden.upgrade() {
                f.showing.set(false);
                f.ticker.borrow_mut().take();
            }
        },
    ));
    Some(feature)
}

impl Feature {
    /// Applies the settings after a change. What is kept of a city that left the list is dropped.
    pub fn configure(&self, config: &Config) -> Result<(), Fault> {
        *self.config.borrow_mut() = config.clone().sanitized();
        let config = self.config.borrow();
        self.slots
            .borrow_mut()
            .retain(|s| config.places.iter().any(|p| p.same_spot(&s.place)));
        drop(config);
        if self.showing.get() {
            self.begin();
        }
        Ok(())
    }

    /// Facts only the running app knows, for the settings page: the search and how it went.
    pub fn runtime(&self) -> Value {
        let search = self.search.borrow();
        json!({
            "searching": search.busy,
            "found": search.found.iter().map(|p| json!({ "label": p.label() })).collect::<Vec<_>>(),
            "problem": search.problem.map_or("", Problem::code),
        })
    }

    /// The buttons of the settings page: `weather_search:<city>` looks for places, `weather_add:<n>` adds the n-th of
    /// the last search to the cities, `weather_up:<n>` moves the n-th city one place earlier and `weather_remove:<n>`
    /// takes it out. `None` when the name is not this feature's.
    pub fn action(&self, name: &str, config: &mut Config) -> Option<Result<(), Fault>> {
        if let Some(query) = name.strip_prefix("weather_search:") {
            return Some(match logic::query_of(query) {
                Some(query) => {
                    self.search_places(query);
                    Ok(())
                }
                None => Err(Fault::new("invalid_value", "the name of a city")),
            });
        }
        let (verb, index) = name.strip_prefix("weather_")?.split_once(':')?;
        let index = index.parse::<usize>().ok();
        let result = match verb {
            "add" => {
                let place = index.and_then(|i| self.search.borrow().found.get(i).cloned());
                match place {
                    Some(place) => add_place(&mut config.places, place),
                    None => Err(Fault::new("invalid_value", "a place from the last search")),
                }
            }
            "up" | "remove" => match index {
                Some(index) => edit_places(&mut config.places, verb, index),
                None => Err(Fault::new("invalid_value", "a city of the list")),
            },
            _ => return None,
        };
        if result.is_ok() {
            let _ = self.configure(config);
        }
        Some(result)
    }

    /// The number in the list of the city on screen.
    fn index(&self) -> usize {
        let config = self.config.borrow();
        self.shown
            .borrow()
            .as_ref()
            .and_then(|shown| config.places.iter().position(|p| p.same_spot(shown)))
            .unwrap_or(0)
    }

    fn current(&self) -> Option<Place> {
        self.config.borrow().places.get(self.index()).cloned()
    }

    /// Shows the previous (-1) or the next (1) city, round the list.
    fn flip(&self, step: isize) {
        let count = self.config.borrow().places.len();
        if count < 2 {
            return;
        }
        let next = (self.index() as isize + step).rem_euclid(count as isize) as usize;
        *self.shown.borrow_mut() = self.config.borrow().places.get(next).cloned();
        if self.showing.get() {
            self.begin();
        } else {
            self.render();
        }
    }

    /// The tab opened or another city shows: show what there is, ask for a fresh forecast if the one at hand is old,
    /// and keep it fresh while the tab stays open.
    fn begin(&self) {
        self.ticker.borrow_mut().take();
        self.render();
        if !self.config.borrow().enabled || self.current().is_none() {
            trace::log("weather: nothing to fetch");
            return;
        }
        self.refresh_if_old(FRESH_MS);
        let weak = self.me.clone();
        *self.ticker.borrow_mut() = Some(app::every(TICK_SECONDS, move || {
            if let Some(f) = weak.upgrade() {
                f.refresh_if_old(REFRESH_MS);
                f.render();
            }
        }));
    }

    /// Asks for the forecast of the city on screen when its last one is older than `limit` and none is on its way.
    fn refresh_if_old(&self, limit: i64) {
        let Some(place) = self.current() else {
            return;
        };
        let imperial = self.config.borrow().imperial();
        let old = self
            .slots
            .borrow()
            .iter()
            .find(|s| s.is(&place, imperial))
            .is_none_or(|s| {
                !s.busy
                    && s.fetched
                        .as_ref()
                        .is_none_or(|(_, at)| now_ms() - at >= limit)
            });
        if old {
            self.fetch(place, imperial);
        }
    }

    fn fetch(&self, place: Place, imperial: bool) {
        {
            let mut slots = self.slots.borrow_mut();
            let slot = match slots.iter().position(|s| s.is(&place, imperial)) {
                Some(i) => &mut slots[i],
                None => {
                    slots.push(Slot::new(place.clone(), imperial));
                    slots.last_mut().expect("just pushed")
                }
            };
            slot.busy = true;
        }
        trace::log("weather: asking for the forecast");
        self.render();
        std::thread::spawn(move || {
            let query = logic::forecast_query(&place, imperial);
            let result = match net::get(FORECAST_URL, &query, FETCH_SECONDS) {
                Ok(body) => logic::parse_forecast(&body)
                    .map_err(|_| Problem::Service)
                    .map(|mut forecast| {
                        // The air is a second service; the forecast is shown without it when it does not answer.
                        forecast.air = net::get(AIR_URL, &logic::air_query(&place), FETCH_SECONDS)
                            .ok()
                            .and_then(|body| logic::parse_air(&body).ok());
                        forecast
                    }),
                Err(net::NetError::Status | net::NetError::TooLarge | net::NetError::Failed(_)) => {
                    Err(Problem::Service)
                }
                Err(_) => Err(Problem::Offline),
            };
            app::on_main(move || {
                with_feature(|f| f.arrived(&place, imperial, result));
            });
        });
    }

    fn arrived(&self, place: &Place, imperial: bool, result: Result<Forecast, Problem>) {
        {
            let mut slots = self.slots.borrow_mut();
            // A city taken out of the list meanwhile has lost its slot, and the answer is dropped with it.
            let Some(slot) = slots.iter_mut().find(|s| s.is(place, imperial)) else {
                return;
            };
            slot.busy = false;
            match result {
                Ok(forecast) => {
                    trace::log("weather: forecast arrived");
                    slot.fetched = Some((forecast, now_ms()));
                    slot.problem = None;
                }
                Err(problem) => {
                    trace::log(&format!("weather: no forecast ({})", problem.code()));
                    slot.problem = Some(problem);
                }
            }
        }
        self.render();
    }

    fn search_places(&self, query: String) {
        let generation = self.search_generation.get() + 1;
        self.search_generation.set(generation);
        *self.search.borrow_mut() = Search {
            busy: true,
            ..Search::default()
        };
        trace::log("weather: searching for a city");
        let code = i18n::current().code();
        std::thread::spawn(move || {
            let result = match net::get(
                GEOCODING_URL,
                &logic::places_query(&query, code),
                FETCH_SECONDS,
            ) {
                Ok(body) => logic::parse_places(&body).map_err(|_| Problem::Service),
                Err(net::NetError::Status | net::NetError::TooLarge | net::NetError::Failed(_)) => {
                    Err(Problem::Service)
                }
                Err(_) => Err(Problem::Offline),
            };
            app::on_main(move || with_feature(|f| f.searched(generation, result)));
        });
    }

    fn searched(&self, generation: u64, result: Result<Vec<Place>, Problem>) {
        if generation != self.search_generation.get() {
            return;
        }
        let mut search = self.search.borrow_mut();
        search.busy = false;
        match result {
            Ok(found) => {
                trace::log(&format!("weather: {} places found", found.len()));
                search.found = found;
                search.problem = None;
            }
            Err(problem) => {
                search.found.clear();
                search.problem = Some(problem);
            }
        }
    }

    fn render(&self) {
        let config = self.config.borrow().clone();
        let index = self.index();
        let slots = self.slots.borrow();
        let slot = config
            .places
            .get(index)
            .and_then(|p| slots.iter().find(|s| s.is(p, config.imperial())));
        self.view
            .show(display(i18n::current(), &config, index, slot, now_ms()));
    }
}

fn note(title: &str, hint: &str) -> WeatherDisplay {
    WeatherDisplay {
        note: Some((title.to_owned(), hint.to_owned())),
        ..WeatherDisplay::default()
    }
}

/// What the tab shows for the state it is in. Pure, so the tests drive it without a screen or a network.
/// `index` is the number of the city on screen and `slot` what is known of it in the units chosen.
fn display(
    language: Language,
    config: &Config,
    index: usize,
    slot: Option<&Slot>,
    now: i64,
) -> WeatherDisplay {
    // The way to fix it is in the settings, so the note opens them.
    let settings = |title: &str, hint: &str| WeatherDisplay {
        action: Some(language.weather_open_settings().to_owned()),
        ..note(title, hint)
    };
    if !config.enabled {
        return settings(language.weather_off_title(), language.weather_off_hint());
    }
    let Some(place) = config.places.get(index) else {
        return settings(language.weather_city_title(), language.weather_city_hint());
    };
    let pager = (config.places.len() > 1).then(|| Pager {
        position: index + 1,
        count: config.places.len(),
        previous: language.weather_previous().to_owned(),
        next: language.weather_next().to_owned(),
    });
    let slot = slot.filter(|s| s.is(place, config.imperial()));
    let (busy, problem) = slot.map_or((false, None), |s| (s.busy, s.problem));
    let Some((forecast, at)) = slot.and_then(|s| s.fetched.as_ref()) else {
        let shown = match problem {
            Some(Problem::Offline) => note(
                language.weather_offline_title(),
                language.weather_offline_hint(),
            ),
            Some(Problem::Service) => note(
                language.weather_service_title(),
                language.weather_service_hint(),
            ),
            None => note(
                language.weather_loading_title(),
                language.weather_loading_hint(),
            ),
        };
        return WeatherDisplay {
            place: place.label(),
            pager,
            ..shown
        };
    };
    let imperial = config.imperial();
    let age = ago(language, now, *at);
    let status = if busy {
        language.weather_updating().to_owned()
    } else if problem.is_some() {
        language.weather_stale(&age)
    } else {
        language.weather_updated(&age)
    };
    let sky = logic::sky(forecast.now.code);
    let mut facts = vec![
        (
            language.weather_feels().to_owned(),
            logic::temp_text(forecast.now.feels),
        ),
        (
            language.weather_humidity().to_owned(),
            format!("{:.0}%", forecast.now.humidity),
        ),
        (
            language.weather_wind().to_owned(),
            format!(
                "{:.0} {}",
                forecast.now.wind,
                if imperial { "mph" } else { "km/h" }
            ),
        ),
    ];
    if let Some((rise, set)) = forecast.sun() {
        facts.push((
            language.weather_sunrise().to_owned(),
            language.weather_clock(&rise),
        ));
        facts.push((
            language.weather_sunset().to_owned(),
            language.weather_clock(&set),
        ));
    }
    let hours = forecast
        .upcoming_hours(HOURS)
        .into_iter()
        .map(|h| Hour {
            label: logic::hour_of(&h.time)
                .map_or_else(String::new, |hour| language.weather_hour(hour)),
            glyph: logic::glyph(logic::sky(h.code), h.day),
            temp: logic::temp_text(h.temp),
            rain: logic::rain_text(h.rain),
        })
        .collect();
    let days = forecast
        .upcoming_days(DAYS)
        .into_iter()
        .enumerate()
        .map(|(i, d)| Day {
            label: if i == 0 {
                language.weather_today().to_owned()
            } else {
                logic::weekday_of(&d.date)
                    .map_or_else(String::new, |w| language.weather_weekday(w).to_owned())
            },
            glyph: logic::glyph(logic::sky(d.code), true),
            high: logic::temp_text(d.high),
            low: logic::temp_text(d.low),
        })
        .collect();
    WeatherDisplay {
        place: place.label(),
        status,
        credit: language.weather_credit().to_owned(),
        current: Some(Current {
            glyph: logic::glyph(sky, forecast.now.day),
            temp: logic::temp_text(forecast.now.temp),
            condition: language.weather_sky(sky).to_owned(),
            facts,
        }),
        hours,
        days,
        note: None,
        action: None,
        pager,
        fx: Some(forecast.atmosphere(place, imperial)),
        sound: config.sound,
        insight: Some(insight(language, forecast, place, imperial)),
        views: (
            language.weather_forecast().to_owned(),
            language.weather_details().to_owned(),
        ),
    }
}

/// The second view of the tab: the air and what suits it, the season, the rain and sun, the wind and the feel.
fn insight(language: Language, forecast: &Forecast, place: &Place, imperial: bool) -> Insight {
    let now = &forecast.now;
    let air = forecast.air.as_ref().map(|air| {
        let level = logic::air_level(air.aqi);
        AirCard {
            title: language.weather_air_title().to_owned(),
            value: format!("{:.0}", air.aqi),
            level: language.weather_air_level(level).to_owned(),
            position: logic::air_position(air.aqi),
            rgb: logic::air_rgb(level),
            detail: air
                .pm25
                .map(|pm| language.weather_air_detail(pm, logic::who_ratio(pm)))
                .unwrap_or_default(),
            advice: advice_list(language, level),
        }
    });
    let month = now.time.get(5..7).and_then(|m| m.parse().ok()).unwrap_or(6);
    let season = logic::season(place.latitude, month);
    let sky = logic::sky(now.code);
    let rain = logic::rain_level(now.code, now.precip);
    let thunder = logic::is_thunder(now.code);
    let chance = forecast
        .upcoming_hours(HOURS)
        .into_iter()
        .filter_map(|h| h.rain)
        .reduce(f64::max);
    let sky_tile = if logic::snow_intensity(now.code) > 0.0 {
        (
            language.weather_snowing().to_owned(),
            language.weather_chance(chance),
            Hue::Snow,
        )
    } else if rain != logic::RainLevel::Dry {
        (
            language.weather_rain_level(rain).to_owned(),
            language.weather_rain_note(rain, thunder, now.precip),
            if thunder { Hue::Storm } else { Hue::Rain },
        )
    } else if now.day {
        let level = logic::uv_level(now.uv);
        let across = logic::sun_progress(
            &now.time,
            forecast.upcoming_days(1).first().map_or("", |d| &d.sunrise),
            forecast.upcoming_days(1).first().map_or("", |d| &d.sunset),
            true,
        );
        (
            language.weather_uv(now.uv, language.weather_uv_level(level)),
            language.weather_sun_note(across * 100.0, level),
            Hue::Sun,
        )
    } else {
        (
            language.weather_sky(sky).to_owned(),
            language.weather_chance(chance),
            Hue::Moon,
        )
    };
    let unit = if imperial { "mph" } else { "km/h" };
    let wind_level = logic::wind_level(logic::to_kmh(now.wind, imperial));
    let gust = (now.gust > now.wind + 3.0).then_some(now.gust);
    let feels = logic::feel(logic::to_celsius(now.feels, imperial));
    let tiles = vec![
        Tile {
            title: language.weather_season_title().to_owned(),
            value: language.weather_season(season).to_owned(),
            note: language.weather_season_note(season).to_owned(),
            hue: Hue::Leaf,
        },
        Tile {
            title: language.weather_sky_title().to_owned(),
            value: sky_tile.0,
            note: sky_tile.1,
            hue: sky_tile.2,
        },
        Tile {
            title: language.weather_wind_title().to_owned(),
            value: format!(
                "{}, {:.0} {unit}",
                language.weather_wind_level(wind_level),
                now.wind
            ),
            note: language.weather_wind_note(
                now.direction.map(logic::compass),
                gust,
                unit,
                language.weather_wind_effect(wind_level),
            ),
            hue: Hue::Cloud,
        },
        Tile {
            title: language.weather_feel_title().to_owned(),
            value: format!(
                "{}, {}",
                language.weather_feel(feels),
                logic::temp_text(now.feels)
            ),
            note: language.weather_feel_note(feels).to_owned(),
            hue: match feels {
                logic::Feel::Freezing | logic::Feel::Cold | logic::Feel::Cool => Hue::Snow,
                logic::Feel::Mild => Hue::Leaf,
                _ => Hue::Sun,
            },
        },
    ];
    Insight {
        air,
        air_missing: language.weather_air_missing().to_owned(),
        tiles,
    }
}

/// What the air at this level suits, one line each.
fn advice_list(language: Language, level: logic::AirLevel) -> Vec<Advice> {
    logic::air_advice(level)
        .into_iter()
        .enumerate()
        .map(|(i, verdict)| Advice {
            activity: language.weather_air_activity(i).to_owned(),
            verdict,
            word: language.weather_air_word(i, verdict).to_owned(),
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    const FORECAST: &str = include_str!("testdata/forecast.json");

    fn hanoi() -> Place {
        Place {
            name: "Hanoi".into(),
            region: "Hanoi".into(),
            country: "Vietnam".into(),
            latitude: 21.0245,
            longitude: 105.84117,
        }
    }

    fn ready(imperial: bool) -> Config {
        Config {
            enabled: true,
            units: if imperial { "imperial" } else { "metric" }.into(),
            sound: false,
            places: vec![hanoi()],
            place: None,
        }
    }

    fn fetched(at: i64, imperial: bool) -> Slot {
        Slot {
            fetched: Some((logic::parse_forecast(FORECAST).unwrap(), at)),
            ..Slot::new(hanoi(), imperial)
        }
    }

    fn failed(problem: Option<Problem>, busy: bool) -> Slot {
        Slot {
            problem,
            busy,
            ..Slot::new(hanoi(), false)
        }
    }

    #[test]
    fn the_feature_is_off_by_default_and_old_files_load() {
        let config: Config = serde_json::from_str("{}").unwrap();
        assert_eq!(config, Config::default());
        assert!(!config.enabled && config.places.is_empty() && config.units == "metric");
        let partial: Config = serde_json::from_str(
            r#"{ "enabled": true, "units": "imperial", "place": { "name": "Hue", "latitude": 16.46, "longitude": 107.59 }, "future_key": 1 }"#,
        )
        .unwrap();
        assert!(partial.enabled && partial.imperial());
        // A file of the one-city days: its city becomes the first of the list, and is written back only there.
        let partial = partial.sanitized();
        assert_eq!(partial.places.len(), 1);
        assert_eq!(partial.places[0].name, "Hue");
        let written = serde_json::to_value(&partial).unwrap();
        assert!(written.get("place").is_none() && written["places"][0]["name"] == "Hue");
        let both: Config = serde_json::from_str(
            r#"{ "place": { "name": "Hue", "latitude": 16.46, "longitude": 107.59 },
                 "places": [{ "name": "Hanoi", "latitude": 21.02, "longitude": 105.84 }, { "name": "Hue again", "latitude": 16.46, "longitude": 107.59 }] }"#,
        )
        .unwrap();
        let names: Vec<String> = both
            .sanitized()
            .places
            .into_iter()
            .map(|p| p.name)
            .collect();
        assert_eq!(names, ["Hue", "Hanoi"], "the old city first, and once");
    }

    #[test]
    fn a_hostile_file_is_brought_into_range() {
        let config: Config = serde_json::from_str(
            r#"{ "units": "kelvin", "place": { "name": "Nowhere", "latitude": 400, "longitude": 0 } }"#,
        )
        .unwrap();
        let config = config.sanitized();
        assert_eq!(config.units, "metric");
        assert!(config.place.is_none() && config.places.is_empty());
        // A city of the wrong shape must not make the whole file unreadable, which would reset every setting.
        for wrong in [
            r#"{ "place": "Hanoi" }"#,
            r#"{ "place": 5 }"#,
            r#"{ "place": { "name": 3 } }"#,
            r#"{ "place": [] }"#,
            r#"{ "place": null }"#,
            r#"{ "places": "Hanoi" }"#,
            r#"{ "places": { "name": "Hue" } }"#,
            r#"{ "places": null }"#,
            r#"{ "places": [5, "x", { "name": 3 }, { "name": "Far", "latitude": 0, "longitude": 999 }] }"#,
        ] {
            let config: Config = serde_json::from_str(wrong).expect(wrong);
            assert!(config.sanitized().places.is_empty(), "{wrong}");
        }
        let mixed: Config = serde_json::from_str(
            r#"{ "places": [7, { "name": "Hue", "latitude": 16.46, "longitude": 107.59 }] }"#,
        )
        .unwrap();
        assert_eq!(mixed.places.len(), 1, "a bad city leaves the good ones");
    }

    #[test]
    fn cities_are_added_once_up_to_the_cap_moved_up_and_removed() {
        let at = |name: &str, latitude: f64| Place {
            name: name.into(),
            latitude,
            ..hanoi()
        };
        let mut places = Vec::new();
        for i in 0..MAX_CITIES {
            assert!(add_place(&mut places, at("C", i as f64)).is_ok());
        }
        assert_eq!(
            add_place(&mut places, at("More", 50.0)).unwrap_err().code,
            "weather_full"
        );
        assert_eq!(
            add_place(&mut places, at("Again", 3.0)).unwrap_err().code,
            "weather_twice",
            "a city already there is refused before the cap is"
        );
        assert!(
            edit_places(&mut places, "up", 0).is_err(),
            "the first cannot move up"
        );
        assert!(edit_places(&mut places, "up", 2).is_ok());
        assert_eq!((places[1].latitude, places[2].latitude), (2.0, 1.0));
        assert!(edit_places(&mut places, "remove", 0).is_ok());
        assert_eq!(places.len(), MAX_CITIES - 1);
        assert!(edit_places(&mut places, "remove", 99).is_err());
        assert!(edit_places(&mut places, "sideways", 1).is_err());
    }

    #[test]
    fn the_tab_says_why_it_has_nothing_to_show() {
        let off = display(Language::English, &Config::default(), 0, None, 0);
        assert_eq!(off.note.as_ref().unwrap().0, "The Weather tab is off");
        let no_city = Config {
            enabled: true,
            ..Config::default()
        };
        // With no city the tab has nothing to fetch, and its button opens the page where a city is added.
        for (language, button) in [
            (Language::English, "Open Settings"),
            (Language::Vietnamese, "Mở Cài đặt"),
        ] {
            let city = display(language, &no_city, 0, None, 0);
            assert!(city.note.is_some() && city.pager.is_none());
            assert_eq!(city.action.as_deref(), Some(button));
        }
        let city = display(Language::English, &no_city, 0, None, 0);
        assert_eq!(city.note.as_ref().unwrap().0, "Choose a city");
        let loading = display(
            Language::English,
            &ready(false),
            0,
            Some(&failed(None, true)),
            0,
        );
        assert_eq!(loading.note.as_ref().unwrap().0, "Getting the weather");
        assert!(
            loading.action.is_none(),
            "nothing to change in the settings while it loads"
        );
        let offline = display(
            Language::Vietnamese,
            &ready(false),
            0,
            Some(&failed(Some(Problem::Offline), false)),
            0,
        );
        assert_eq!(offline.note.as_ref().unwrap().0, "Không có kết nối");
        let service = display(
            Language::English,
            &ready(false),
            0,
            Some(&failed(Some(Problem::Service), false)),
            0,
        );
        assert_eq!(
            service.note.as_ref().unwrap().0,
            "No answer from the weather service"
        );
    }

    #[test]
    fn a_forecast_is_shown_whole_in_both_languages() {
        let at = 1_000_000;
        for language in [Language::English, Language::Vietnamese] {
            let shown = display(
                language,
                &ready(false),
                0,
                Some(&fetched(at, false)),
                at + 3 * 60_000,
            );
            assert!(shown.note.is_none());
            assert_eq!(shown.place, "Hanoi, Vietnam");
            assert_eq!(shown.hours.len(), HOURS);
            assert_eq!(shown.days.len(), DAYS);
            let current = shown.current.as_ref().unwrap();
            assert_eq!(current.temp, "87°");
            assert_eq!(
                current.facts.len(),
                5,
                "feels, humidity, wind, sunrise and sunset"
            );
            assert_eq!(current.facts[2].1, "4 km/h");
            assert!(shown.status.contains('3'), "{}", shown.status);
            assert!(shown.credit.contains("Open-Meteo"));
            assert!(
                shown
                    .hours
                    .iter()
                    .all(|h| !h.label.is_empty() && !h.temp.is_empty())
            );
            assert!(shown.days.iter().all(|d| !d.label.is_empty()));
        }
        let english = display(
            Language::English,
            &ready(false),
            0,
            Some(&fetched(at, false)),
            at,
        );
        assert_eq!(english.hours[0].label, "11 AM");
        assert_eq!(english.days[0].label, "Today");
        assert_eq!(english.days[1].label, "Wed");
        assert_eq!(english.current.as_ref().unwrap().facts[3].1, "5:54 AM");
        let vietnamese = display(
            Language::Vietnamese,
            &ready(false),
            0,
            Some(&fetched(at, false)),
            at,
        );
        assert_eq!(vietnamese.hours[0].label, "11h");
        assert_eq!(vietnamese.days[1].label, "T4");
        assert_eq!(vietnamese.current.as_ref().unwrap().facts[3].1, "05:54");
    }

    #[test]
    fn the_wind_is_said_in_the_units_chosen_and_a_forecast_in_other_units_is_not_shown() {
        let at = 5;
        let imperial = display(
            Language::English,
            &ready(true),
            0,
            Some(&fetched(at, true)),
            at,
        );
        assert_eq!(imperial.current.as_ref().unwrap().facts[2].1, "4 mph");
        // Units changed: the forecast at hand is in the old ones, so it waits for a new one.
        let mismatch = display(
            Language::English,
            &ready(true),
            0,
            Some(&Slot {
                busy: true,
                ..fetched(at, false)
            }),
            at,
        );
        assert_eq!(mismatch.note.as_ref().unwrap().0, "Getting the weather");
        // Another city: the same.
        let mut other = fetched(at, false);
        other.place.latitude = 16.46;
        let elsewhere = display(Language::English, &ready(false), 0, Some(&other), at);
        assert!(elsewhere.note.is_some());
    }

    #[test]
    fn an_old_forecast_stays_up_when_the_new_one_cannot_be_fetched() {
        let at = 0;
        let now = 25 * 60_000;
        let stale = display(
            Language::English,
            &ready(false),
            0,
            Some(&Slot {
                problem: Some(Problem::Offline),
                ..fetched(at, false)
            }),
            now,
        );
        assert!(stale.note.is_none());
        assert!(
            stale.status.starts_with("Offline, from 25 minutes ago"),
            "{}",
            stale.status
        );
        let updating = display(
            Language::English,
            &ready(false),
            0,
            Some(&Slot {
                busy: true,
                ..fetched(at, false)
            }),
            now,
        );
        assert_eq!(updating.status, "Updating...");
    }

    #[test]
    fn several_cities_show_one_at_a_time_with_where_it_is_in_the_list() {
        let at = 7;
        let hue = Place {
            name: "Hue".into(),
            region: String::new(),
            latitude: 16.46,
            longitude: 107.59,
            ..hanoi()
        };
        let config = Config {
            places: vec![hanoi(), hue.clone()],
            ..ready(false)
        };
        let first = display(Language::English, &config, 0, Some(&fetched(at, false)), at);
        let pager = first.pager.as_ref().unwrap();
        assert_eq!((pager.position, pager.count), (1, 2));
        assert_eq!(
            (pager.previous.as_str(), pager.next.as_str()),
            ("Previous city", "Next city")
        );
        assert!(first.note.is_none());
        // The second city has no forecast yet: it says so under its own name, and can still be flipped past.
        let second = display(Language::Vietnamese, &config, 1, None, at);
        assert_eq!(second.place, "Hue, Vietnam");
        assert_eq!(second.note.as_ref().unwrap().0, "Đang lấy thời tiết");
        assert_eq!(second.pager.as_ref().unwrap().position, 2);
        assert_eq!(second.pager.as_ref().unwrap().previous, "Thành phố trước");
        // The forecast of the first city is never shown for the second.
        let wrong = display(Language::English, &config, 1, Some(&fetched(at, false)), at);
        assert!(wrong.note.is_some() && wrong.current.is_none());
        let one = display(
            Language::English,
            &ready(false),
            0,
            Some(&fetched(at, false)),
            at,
        );
        assert!(one.pager.is_none(), "one city needs no flipping");
    }

    #[test]
    fn the_details_say_the_season_the_rain_the_wind_and_what_the_air_suits_in_both_languages() {
        let mut forecast = logic::parse_forecast(FORECAST).unwrap();
        forecast.now.code = 95;
        forecast.now.precip = 9.0;
        forecast.now.wind = 38.0;
        forecast.now.gust = 61.0;
        forecast.now.direction = Some(225.0);
        forecast.air = Some(logic::Air {
            aqi: 165.0,
            pm25: Some(60.0),
            pm10: None,
        });
        let place = hanoi();
        let english = insight(Language::English, &forecast, &place, true);
        let air = english.air.as_ref().unwrap();
        assert_eq!(air.value, "165");
        assert_eq!(air.level, "Unhealthy");
        assert!(air.detail.contains("4.0x"), "{}", air.detail);
        assert_eq!(air.advice.len(), 4);
        assert_eq!(air.advice[0].verdict, macos_sys::weather_ui::Verdict::Avoid);
        assert_eq!(english.tiles[0].value, "Autumn");
        assert_eq!(english.tiles[1].value, "Heavy rain");
        assert!(
            english.tiles[1].note.contains("thunder"),
            "{}",
            english.tiles[1].note
        );
        assert!(
            english.tiles[2].note.starts_with("Southwest, gusts 61 mph"),
            "{}",
            english.tiles[2].note
        );
        let vietnamese = insight(Language::Vietnamese, &forecast, &place, false);
        assert_eq!(vietnamese.tiles[0].value, "Mùa thu");
        assert!(vietnamese.tiles[1].note.contains("sấm sét"));
        assert_eq!(vietnamese.air.as_ref().unwrap().level, "Có hại");
        // No air reading: the tiles stay and the air card says so.
        forecast.air = None;
        let without = insight(Language::English, &forecast, &place, true);
        assert!(
            without.air.is_none() && without.tiles.len() == 4 && !without.air_missing.is_empty()
        );
        // A clear day tells the sun and the UV, and a clear night the sky.
        forecast.now.code = 0;
        forecast.now.precip = 0.0;
        forecast.now.uv = 9.0;
        let sunny = insight(Language::English, &forecast, &place, true);
        assert_eq!(sunny.tiles[1].value, "UV 9, very high");
        forecast.now.day = false;
        let night = insight(Language::English, &forecast, &place, true);
        assert_eq!(night.tiles[1].value, "Clear");
        // The display carries both views and the sky once a forecast is in.
        let shown = display(
            Language::English,
            &ready(false),
            0,
            Some(&fetched(1, false)),
            1,
        );
        assert!(shown.fx.is_some() && shown.insight.is_some());
        assert_eq!(shown.views, ("Forecast".to_owned(), "Details".to_owned()));
        let note = display(Language::English, &Config::default(), 0, None, 0);
        assert!(note.fx.is_none() && note.insight.is_none());
    }
}
