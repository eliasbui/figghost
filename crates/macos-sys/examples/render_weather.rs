//! Draws the Weather tab of the open notch into PNGs with sample figures, in both languages, without showing
//! anything: run it with FIGGHOST_NOTCH_OFFSCREEN=1.
//! `FIGGHOST_NOTCH_OFFSCREEN=1 cargo run -p macos-sys --example render_weather -- /tmp/weather`

use macos_sys::{
    MainThreadMarker, app,
    notch::{Notch, NotchGeometry},
    ui::AppCallbacks,
    weather_ui::{
        Advice, AirCard, Atmosphere, Current, Day, Glyph, Hour, Hue, Insight, PAGE_SIZE, Pager,
        Tile, Verdict, WeatherCallbacks, WeatherDisplay, WeatherView,
    },
};
use std::{env, fs, path::PathBuf, process, rc::Rc};

const SUN: Glyph = Glyph {
    symbol: "sun.max.fill",
    hue: Hue::Sun,
};
const MOON: Glyph = Glyph {
    symbol: "moon.stars.fill",
    hue: Hue::Moon,
};
const PARTLY: Glyph = Glyph {
    symbol: "cloud.sun.fill",
    hue: Hue::Cloud,
};
const CLOUD: Glyph = Glyph {
    symbol: "cloud.fill",
    hue: Hue::Cloud,
};
const RAIN: Glyph = Glyph {
    symbol: "cloud.rain.fill",
    hue: Hue::Rain,
};
const HEAVY: Glyph = Glyph {
    symbol: "cloud.heavyrain.fill",
    hue: Hue::Rain,
};
const STORM: Glyph = Glyph {
    symbol: "cloud.bolt.rain.fill",
    hue: Hue::Storm,
};
const SNOW: Glyph = Glyph {
    symbol: "cloud.snow.fill",
    hue: Hue::Snow,
};
const FOG: Glyph = Glyph {
    symbol: "cloud.fog.fill",
    hue: Hue::Fog,
};

fn scene(vi: bool, rainy: bool) -> WeatherDisplay {
    let (hours, days, feels, humidity, wind) = if vi {
        (
            ["14h", "15h", "16h", "17h", "18h", "19h"],
            ["Thứ Ba", "Thứ Tư", "Thứ Năm", "Thứ Sáu", "Thứ Bảy"],
            ("Cảm giác như", "Độ ẩm", "Gió"),
            "Cập nhật 3 phút trước",
            "Dữ liệu: Open-Meteo",
        )
    } else {
        (
            ["2 PM", "3 PM", "4 PM", "5 PM", "6 PM", "7 PM"],
            ["Tue", "Wed", "Thu", "Fri", "Sat"],
            ("Feels like", "Humidity", "Wind"),
            "Updated 3 minutes ago",
            "Data by Open-Meteo",
        )
    };
    let glyphs = if rainy {
        [RAIN, HEAVY, STORM, RAIN, CLOUD, MOON]
    } else {
        [SUN, SUN, PARTLY, PARTLY, CLOUD, MOON]
    };
    let temps = ["31°", "32°", "31°", "29°", "27°", "26°"];
    let rain = ["", "", "20%", "45%", "60%", "10%"];
    let day_glyphs = if rainy {
        [RAIN, STORM, HEAVY, CLOUD, FOG]
    } else {
        [CLOUD, SUN, PARTLY, SNOW, SUN]
    };
    WeatherDisplay {
        place: if vi {
            "Hà Nội, Việt Nam".into()
        } else {
            "Hanoi, Vietnam".into()
        },
        status: humidity.into(),
        credit: wind.into(),
        current: Some(Current {
            glyph: if rainy { RAIN } else { PARTLY },
            temp: "31°".into(),
            condition: if vi {
                if rainy {
                    "Mưa vừa".into()
                } else {
                    "Có mây rải rác".into()
                }
            } else if rainy {
                "Moderate rain".into()
            } else {
                "Partly cloudy".into()
            },
            facts: vec![
                (feels.0.into(), "36°".into()),
                (feels.1.into(), "64%".into()),
                (feels.2.into(), "6 km/h".into()),
                (
                    if vi {
                        "Bình minh".into()
                    } else {
                        "Sunrise".into()
                    },
                    if vi { "05:54".into() } else { "5:54 AM".into() },
                ),
                (
                    if vi {
                        "Hoàng hôn".into()
                    } else {
                        "Sunset".into()
                    },
                    if vi { "17:28".into() } else { "5:28 PM".into() },
                ),
            ],
        }),
        hours: (0..6)
            .map(|i| Hour {
                label: hours[i].into(),
                glyph: glyphs[i],
                temp: temps[i].into(),
                rain: rain[i].into(),
            })
            .collect(),
        days: (0..5)
            .map(|i| Day {
                label: days[i].into(),
                glyph: day_glyphs[i],
                high: ["32°", "31°", "28°", "-2°", "33°"][i].into(),
                low: ["25°", "24°", "23°", "-8°", "26°"][i].into(),
            })
            .collect(),
        note: None,
        action: None,
        pager: None,
        fx: None,
        sound: false,
        insight: Some(insight(vi)),
        views: if vi {
            ("Dự báo".into(), "Chi tiết".into())
        } else {
            ("Forecast".into(), "Details".into())
        },
    }
}

fn insight(vi: bool) -> Insight {
    let t = |en: &str, vn: &str| if vi { vn.to_owned() } else { en.to_owned() };
    let verdicts = [
        (Verdict::Good, t("Fine", "Thoải mái")),
        (Verdict::Caution, t("Briefly", "Mở ngắn thôi")),
        (Verdict::Caution, t("Advised", "Nên đeo")),
        (Verdict::Avoid, t("Stay in", "Ở trong nhà")),
    ];
    let activities = [
        t("Exercise outdoors", "Tập thể dục ngoài trời"),
        t("Open windows", "Mở cửa sổ"),
        t("Mask outdoors", "Khẩu trang khi ra ngoài"),
        t("Children, elderly, asthma", "Trẻ em, người già, hen suyễn"),
    ];
    Insight {
        air: Some(AirCard {
            title: t("Air quality (AQI)", "Chất lượng không khí (AQI)"),
            value: "132".into(),
            level: t("Unhealthy for some", "Hại người nhạy cảm"),
            position: 132.0 / 300.0,
            rgb: (1.0, 0.62, 0.22),
            detail: t(
                "PM2.5 48 µg/m³, 3.2x the WHO limit",
                "PM2.5 48 µg/m³, gấp 3.2 lần ngưỡng WHO",
            ),
            advice: activities
                .into_iter()
                .zip(verdicts)
                .map(|(activity, (verdict, word))| Advice {
                    activity,
                    verdict,
                    word,
                })
                .collect(),
        }),
        air_missing: String::new(),
        tiles: vec![
            Tile {
                title: t("Season", "Mùa"),
                value: t("Autumn", "Mùa thu"),
                note: t("Mild and drier, falling leaves", "Dịu mát, ít mưa, lá rụng"),
                hue: Hue::Leaf,
            },
            Tile {
                title: t("Rain and sun", "Mưa nắng"),
                value: t("Heavy rain", "Mưa to"),
                note: t(
                    "Big drops, thunder and lightning, 9.2 mm/h",
                    "Hạt lớn, có sấm sét, 9.2 mm/h",
                ),
                hue: Hue::Storm,
            },
            Tile {
                title: t("Wind", "Gió"),
                value: t("Gale, 58 km/h", "Gió rất mạnh, 58 km/h"),
                note: t(
                    "Southwest, gusts 71 km/h. Leaves swirl up",
                    "Tây Nam, giật 71 km/h. Lá cuộn xoáy, cành có thể gãy",
                ),
                hue: Hue::Cloud,
            },
            Tile {
                title: t("Feels like", "Cảm giác"),
                value: t("Cold, 9°", "Lạnh, 9°"),
                note: t(
                    "Wear a warm jacket and cover your neck",
                    "Mặc áo khoác ấm, che cổ",
                ),
                hue: Hue::Snow,
            },
        ],
    }
}

fn sky(day: bool, progress: f64) -> Atmosphere {
    Atmosphere {
        day,
        progress,
        leaf: (0.95, 0.56, 0.16),
        wind_dir: 1.0,
        ..Atmosphere::default()
    }
}

fn pager(position: usize, count: usize, vi: bool) -> Option<Pager> {
    let (previous, next) = if vi {
        ("Thành phố trước", "Thành phố sau")
    } else {
        ("Previous city", "Next city")
    };
    Some(Pager {
        position,
        count,
        previous: previous.into(),
        next: next.into(),
    })
}

fn main() {
    let mtm = MainThreadMarker::new().expect("main thread");
    let dir = PathBuf::from(env::args().nth(1).unwrap_or_else(|| "/tmp/weather".into()));
    fs::create_dir_all(&dir).expect("output directory");
    app::run_accessory(mtm, move || {
        let geometry = NotchGeometry::detect(mtm).expect("a screen");
        let noop = AppCallbacks {
            on_query: Box::new(|_, _| {}),
            on_activate: Box::new(|_, _, _| {}),
            on_pin: Box::new(|_, _| {}),
            on_delete: Box::new(|_, _| {}),
        };
        let notch = Rc::new(Notch::new(mtm, geometry, noop, |view| {
            view.set_rows(Vec::new(), "")
        }));
        let callbacks = WeatherCallbacks {
            on_flip: Box::new(|_| {}),
            on_action: Box::new(|| {}),
        };
        let view = WeatherView::new(
            mtm,
            notch.page_area(PAGE_SIZE.width, PAGE_SIZE.height),
            callbacks,
        );
        let index = notch.page_count();
        notch.add_page(view.page("Weather".into(), || {}, || {}));
        let (shot, shown) = (Rc::clone(&notch), Rc::clone(&view));
        app::after(0.4, move || {
            let with = |fx: Atmosphere, detail: bool| {
                let mut display = scene(false, fx.rain > 0.0);
                display.fx = Some(fx);
                if !detail {
                    display.insight = None;
                }
                display
            };
            let mut scenes = vec![
                (
                    "fx_noon",
                    with(
                        Atmosphere {
                            cloud: 0.2,
                            ..sky(true, 0.5)
                        },
                        true,
                    ),
                ),
                (
                    "fx_sunset",
                    with(
                        Atmosphere {
                            cloud: 0.35,
                            ..sky(true, 0.93)
                        },
                        true,
                    ),
                ),
                ("fx_night", with(sky(false, 0.4), true)),
                (
                    "fx_drizzle",
                    with(
                        Atmosphere {
                            cloud: 0.8,
                            rain: 0.15,
                            ..sky(true, 0.4)
                        },
                        true,
                    ),
                ),
                (
                    "fx_downpour",
                    with(
                        Atmosphere {
                            cloud: 1.0,
                            rain: 0.95,
                            thunder: true,
                            wind: 0.5,
                            ..sky(true, 0.4)
                        },
                        true,
                    ),
                ),
                (
                    "fx_snow",
                    with(
                        Atmosphere {
                            cloud: 0.9,
                            snow: 0.8,
                            cold: 0.9,
                            ..sky(true, 0.4)
                        },
                        true,
                    ),
                ),
                (
                    "fx_cold",
                    with(
                        Atmosphere {
                            cloud: 0.2,
                            cold: 0.8,
                            ..sky(false, 0.3)
                        },
                        true,
                    ),
                ),
                (
                    "fx_wind",
                    with(
                        Atmosphere {
                            cloud: 0.5,
                            wind: 0.5,
                            ..sky(true, 0.6)
                        },
                        true,
                    ),
                ),
                (
                    "fx_gale",
                    with(
                        Atmosphere {
                            cloud: 0.7,
                            wind: 1.0,
                            ..sky(true, 0.6)
                        },
                        true,
                    ),
                ),
                (
                    "fx_haze",
                    with(
                        Atmosphere {
                            haze: 0.8,
                            ..sky(true, 0.3)
                        },
                        true,
                    ),
                ),
                (
                    "fx_fog",
                    with(
                        Atmosphere {
                            fog: 0.8,
                            cloud: 0.8,
                            ..sky(true, 0.3)
                        },
                        true,
                    ),
                ),
                ("sunny_en", scene(false, false)),
                ("rainy_en", scene(false, true)),
                ("sunny_vi", scene(true, false)),
                (
                    "cities_en",
                    WeatherDisplay {
                        pager: pager(2, 3, false),
                        ..scene(false, true)
                    },
                ),
                (
                    "cities_vi",
                    WeatherDisplay {
                        pager: pager(1, 8, true),
                        ..scene(true, false)
                    },
                ),
            ];
            for (name, title, hint, action, place, flip) in [
                (
                    "city_en",
                    "Choose a city",
                    "Search for one in Settings, under Notch tabs, Weather.",
                    Some("Open Settings"),
                    "",
                    None,
                ),
                (
                    "city_vi",
                    "Chọn một thành phố",
                    "Tìm trong Cài đặt, mục Tab của notch, Thời tiết.",
                    Some("Mở Cài đặt"),
                    "",
                    None,
                ),
                (
                    "offline_vi",
                    "Không có kết nối",
                    "Chưa lấy được thời tiết. Sẽ thử lại khi bạn mở lại tab.",
                    None,
                    "",
                    None,
                ),
                (
                    "loading_cities_en",
                    "Getting the weather",
                    "One moment.",
                    None,
                    "Hue, Vietnam",
                    pager(3, 3, false),
                ),
            ] {
                scenes.push((
                    name,
                    WeatherDisplay {
                        place: place.into(),
                        note: Some((title.into(), hint.into())),
                        action: action.map(Into::into),
                        pager: flip,
                        ..WeatherDisplay::default()
                    },
                ));
            }
            for (name, display) in scenes {
                shown.show(display.clone());
                // The details view, for the scenes that have it.
                if name.starts_with("fx_") {
                    shown.show_details(true);
                    let png = shot.snapshot_png(index).expect("the page");
                    fs::write(dir.join(format!("{name}_details.png")), png).expect("write png");
                    shown.show_details(false);
                }
                let png = shot.snapshot_png(index).expect("the page");
                let path = dir.join(format!("{name}.png"));
                fs::write(&path, png).expect("write png");
                println!("wrote {}", path.display());
            }
            process::exit(0);
        });
        (notch, view)
    });
}
