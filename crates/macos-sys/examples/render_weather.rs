//! Draws the Weather tab of the open notch into PNGs with sample figures, in both languages, without showing
//! anything: run it with FIGGHOST_NOTCH_OFFSCREEN=1.
//! `FIGGHOST_NOTCH_OFFSCREEN=1 cargo run -p macos-sys --example render_weather -- /tmp/weather`

use macos_sys::{
    MainThreadMarker, app,
    notch::{Notch, NotchGeometry},
    ui::AppCallbacks,
    weather_ui::{Current, Day, Glyph, Hour, Hue, PAGE_SIZE, WeatherDisplay, WeatherView},
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
    }
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
        let view = WeatherView::new(mtm, notch.page_area(PAGE_SIZE.width, PAGE_SIZE.height));
        let index = notch.page_count();
        notch.add_page(view.page("Weather".into(), || {}, || {}));
        let (shot, shown) = (Rc::clone(&notch), Rc::clone(&view));
        app::after(0.4, move || {
            let mut scenes = vec![
                ("sunny_en", scene(false, false)),
                ("rainy_en", scene(false, true)),
                ("sunny_vi", scene(true, false)),
            ];
            for (name, title, hint) in [
                (
                    "city_en",
                    "Choose a city",
                    "Search for one in Settings, under Notch tabs, Weather.",
                ),
                (
                    "offline_vi",
                    "Không có kết nối",
                    "Chưa lấy được thời tiết. Sẽ thử lại khi bạn mở lại tab.",
                ),
            ] {
                scenes.push((
                    name,
                    WeatherDisplay {
                        note: Some((title.into(), hint.into())),
                        ..WeatherDisplay::default()
                    },
                ));
            }
            for (name, display) in scenes {
                shown.show(display);
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
