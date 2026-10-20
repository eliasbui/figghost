//! The Weather tab of the notch: the weather now, the next hours and the next days of one city. It is off until the
//! person turns it on and chooses a city, because it is the one tab that talks to the internet.
//!
//! Forecasts come from Open-Meteo, which needs no account or key, through the system's `curl` as the other features
//! that go online do (see `core_smart::net`). What leaves the Mac is the coordinates of the chosen city, or the
//! name typed when searching for one; never the Mac's own location, which is not asked for. The tab asks only
//! while it is showing, and not again until the last answer is ten minutes old.

use super::AttachEnv;
use crate::{
    history::{ago, now_ms},
    i18n::{self, Language},
    settings::model::Fault,
    trace,
};
use core_smart::net;
use logic::{DAYS, FORECAST_URL, Forecast, GEOCODING_URL, HOURS, Place};
use macos_sys::{
    app,
    weather_ui::{Current, Day, Hour, PAGE_SIZE, WeatherDisplay, WeatherView},
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
    /// The city chosen from a search. A place of the wrong shape is none, not an unreadable file.
    #[serde(deserialize_with = "lenient_place")]
    pub place: Option<Place>,
}

fn lenient_place<'de, D: serde::Deserializer<'de>>(
    deserializer: D,
) -> Result<Option<Place>, D::Error> {
    let value = Value::deserialize(deserializer)?;
    Ok(serde_json::from_value(value).ok())
}

impl Default for Config {
    fn default() -> Self {
        Self {
            enabled: false,
            units: UNITS[0].into(),
            place: None,
        }
    }
}

impl Config {
    /// A config file can say anything: unknown units are metric, and a place that is not real is none.
    pub fn sanitized(mut self) -> Self {
        if !UNITS.contains(&self.units.as_str()) {
            self.units = UNITS[0].into();
        }
        self.place = self.place.and_then(Place::sanitized);
        self
    }

    fn imperial(&self) -> bool {
        self.units == "imperial"
    }

    fn ready(&self) -> bool {
        self.enabled && self.place.is_some()
    }
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

/// A forecast and what it was asked for, so that one for another city or other units is never shown.
struct Fetched {
    forecast: Forecast,
    at: i64,
    place: Place,
    imperial: bool,
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
    fetched: RefCell<Option<Fetched>>,
    problem: Cell<Option<Problem>>,
    busy: Cell<bool>,
    /// Bumped by every request; an answer that finds another number is for a question that is no longer asked.
    generation: Cell<u64>,
    search: RefCell<Search>,
    search_generation: Cell<u64>,
    ticker: RefCell<Option<app::Timer>>,
    showing: Cell<bool>,
    me: Weak<Feature>,
}

/// Adds the tab. `None` without a notch, which is where the tab lives.
pub fn attach(env: &AttachEnv) -> Option<Rc<Feature>> {
    let notch = env.notch?;
    let view = WeatherView::new(env.mtm, notch.page_area(PAGE_SIZE.width, PAGE_SIZE.height));
    let feature = Rc::new_cyclic(|me| Feature {
        view: Rc::clone(&view),
        config: RefCell::new(env.config.weather.clone().sanitized()),
        fetched: RefCell::default(),
        problem: Cell::new(None),
        busy: Cell::new(false),
        generation: Cell::new(0),
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
    /// Applies the settings after a change. A forecast for another city or other units is dropped.
    pub fn configure(&self, config: &Config) -> Result<(), Fault> {
        *self.config.borrow_mut() = config.clone().sanitized();
        self.drop_stale();
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

    /// The buttons of the settings page: `weather_search:<city>` looks for places, `weather_pick:<n>` takes the
    /// n-th of the last search. `None` when the name is not this feature's.
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
        let index = name.strip_prefix("weather_pick:")?;
        let place = index
            .parse::<usize>()
            .ok()
            .and_then(|i| self.search.borrow().found.get(i).cloned());
        Some(match place {
            Some(place) => {
                config.place = Some(place);
                *self.config.borrow_mut() = config.clone().sanitized();
                self.drop_stale();
                if self.showing.get() {
                    self.begin();
                }
                Ok(())
            }
            None => Err(Fault::new("invalid_value", "a place from the last search")),
        })
    }

    /// Forgets a forecast that is not for the city and units now chosen.
    fn drop_stale(&self) {
        let config = self.config.borrow();
        let keep = self.fetched.borrow().as_ref().is_some_and(|f| {
            Some(&f.place) == config.place.as_ref() && f.imperial == config.imperial()
        });
        if !keep {
            self.fetched.borrow_mut().take();
            self.problem.set(None);
            // An answer still on its way is for the old question.
            self.generation.set(self.generation.get() + 1);
            self.busy.set(false);
        }
    }

    /// The tab opened: show what there is, ask for a fresh forecast if the one at hand is old, and keep it fresh
    /// while the tab stays open.
    fn begin(&self) {
        self.ticker.borrow_mut().take();
        self.render();
        if !self.config.borrow().ready() {
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

    fn refresh_if_old(&self, limit: i64) {
        let old = self
            .fetched
            .borrow()
            .as_ref()
            .is_none_or(|f| now_ms() - f.at >= limit);
        if old && !self.busy.get() {
            self.fetch();
        }
    }

    fn fetch(&self) {
        let (place, imperial) = {
            let config = self.config.borrow();
            let Some(place) = config.place.clone() else {
                return;
            };
            (place, config.imperial())
        };
        let generation = self.generation.get() + 1;
        self.generation.set(generation);
        self.busy.set(true);
        trace::log("weather: asking for the forecast");
        self.render();
        std::thread::spawn(move || {
            let query = logic::forecast_query(&place, imperial);
            let result = match net::get(FORECAST_URL, &query, FETCH_SECONDS) {
                Ok(body) => logic::parse_forecast(&body).map_err(|_| Problem::Service),
                Err(net::NetError::Status | net::NetError::TooLarge | net::NetError::Failed(_)) => {
                    Err(Problem::Service)
                }
                Err(_) => Err(Problem::Offline),
            };
            app::on_main(move || {
                with_feature(|f| f.arrived(generation, place, imperial, result));
            });
        });
    }

    fn arrived(
        &self,
        generation: u64,
        place: Place,
        imperial: bool,
        result: Result<Forecast, Problem>,
    ) {
        if generation != self.generation.get() {
            return;
        }
        self.busy.set(false);
        match result {
            Ok(forecast) => {
                trace::log("weather: forecast arrived");
                *self.fetched.borrow_mut() = Some(Fetched {
                    forecast,
                    at: now_ms(),
                    place,
                    imperial,
                });
                self.problem.set(None);
            }
            Err(problem) => {
                trace::log(&format!("weather: no forecast ({})", problem.code()));
                self.problem.set(Some(problem));
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
        let language = i18n::current();
        self.view.show(display(
            language,
            &config,
            self.fetched.borrow().as_ref(),
            self.problem.get(),
            self.busy.get(),
            now_ms(),
        ));
    }
}

fn note(title: &str, hint: &str) -> WeatherDisplay {
    WeatherDisplay {
        note: Some((title.to_owned(), hint.to_owned())),
        ..WeatherDisplay::default()
    }
}

/// What the tab shows for the state it is in. Pure, so the tests drive it without a screen or a network.
fn display(
    language: Language,
    config: &Config,
    fetched: Option<&Fetched>,
    problem: Option<Problem>,
    busy: bool,
    now: i64,
) -> WeatherDisplay {
    if !config.enabled {
        return note(language.weather_off_title(), language.weather_off_hint());
    }
    let Some(place) = &config.place else {
        return note(language.weather_city_title(), language.weather_city_hint());
    };
    let Some(fetched) = fetched.filter(|f| &f.place == place && f.imperial == config.imperial())
    else {
        return match problem {
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
    };
    let forecast = &fetched.forecast;
    let imperial = config.imperial();
    let age = ago(language, now, fetched.at);
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
    }
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
            place: Some(hanoi()),
        }
    }

    fn fetched(at: i64, imperial: bool) -> Fetched {
        Fetched {
            forecast: logic::parse_forecast(FORECAST).unwrap(),
            at,
            place: hanoi(),
            imperial,
        }
    }

    #[test]
    fn the_feature_is_off_by_default_and_old_files_load() {
        let config: Config = serde_json::from_str("{}").unwrap();
        assert_eq!(config, Config::default());
        assert!(!config.enabled && config.place.is_none() && config.units == "metric");
        let partial: Config = serde_json::from_str(
            r#"{ "enabled": true, "units": "imperial", "place": { "name": "Hue", "latitude": 16.46, "longitude": 107.59 }, "future_key": 1 }"#,
        )
        .unwrap();
        assert!(partial.enabled && partial.imperial());
        assert_eq!(partial.place.unwrap().name, "Hue");
    }

    #[test]
    fn a_hostile_file_is_brought_into_range() {
        let config: Config = serde_json::from_str(
            r#"{ "units": "kelvin", "place": { "name": "Nowhere", "latitude": 400, "longitude": 0 } }"#,
        )
        .unwrap();
        let config = config.sanitized();
        assert_eq!(config.units, "metric");
        assert!(config.place.is_none());
        // A place of the wrong shape must not make the whole file unreadable, which would reset every setting.
        for wrong in [
            r#"{ "place": "Hanoi" }"#,
            r#"{ "place": 5 }"#,
            r#"{ "place": { "name": 3 } }"#,
            r#"{ "place": [] }"#,
            r#"{ "place": null }"#,
        ] {
            let config: Config = serde_json::from_str(wrong).expect(wrong);
            assert!(config.place.is_none(), "{wrong}");
        }
    }

    #[test]
    fn the_tab_says_why_it_has_nothing_to_show() {
        let off = display(Language::English, &Config::default(), None, None, false, 0);
        assert_eq!(off.note.as_ref().unwrap().0, "The Weather tab is off");
        let no_city = Config {
            enabled: true,
            ..Config::default()
        };
        let city = display(Language::English, &no_city, None, None, false, 0);
        assert_eq!(city.note.as_ref().unwrap().0, "Choose a city");
        let loading = display(Language::English, &ready(false), None, None, true, 0);
        assert_eq!(loading.note.as_ref().unwrap().0, "Getting the weather");
        let offline = display(
            Language::Vietnamese,
            &ready(false),
            None,
            Some(Problem::Offline),
            false,
            0,
        );
        assert_eq!(offline.note.as_ref().unwrap().0, "Không có kết nối");
        let service = display(
            Language::English,
            &ready(false),
            None,
            Some(Problem::Service),
            false,
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
                Some(&fetched(at, false)),
                None,
                false,
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
            Some(&fetched(at, false)),
            None,
            false,
            at,
        );
        assert_eq!(english.hours[0].label, "11 AM");
        assert_eq!(english.days[0].label, "Today");
        assert_eq!(english.days[1].label, "Wed");
        assert_eq!(english.current.as_ref().unwrap().facts[3].1, "5:54 AM");
        let vietnamese = display(
            Language::Vietnamese,
            &ready(false),
            Some(&fetched(at, false)),
            None,
            false,
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
            Some(&fetched(at, true)),
            None,
            false,
            at,
        );
        assert_eq!(imperial.current.as_ref().unwrap().facts[2].1, "4 mph");
        // Units changed: the forecast at hand is in the old ones, so it waits for a new one.
        let mismatch = display(
            Language::English,
            &ready(true),
            Some(&fetched(at, false)),
            None,
            true,
            at,
        );
        assert_eq!(mismatch.note.as_ref().unwrap().0, "Getting the weather");
        // Another city: the same.
        let mut other = fetched(at, false);
        other.place.name = "Hue".into();
        let elsewhere = display(
            Language::English,
            &ready(false),
            Some(&other),
            None,
            true,
            at,
        );
        assert!(elsewhere.note.is_some());
    }

    #[test]
    fn an_old_forecast_stays_up_when_the_new_one_cannot_be_fetched() {
        let at = 0;
        let now = 25 * 60_000;
        let stale = display(
            Language::English,
            &ready(false),
            Some(&fetched(at, false)),
            Some(Problem::Offline),
            false,
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
            Some(&fetched(at, false)),
            None,
            true,
            now,
        );
        assert_eq!(updating.status, "Updating...");
    }
}
