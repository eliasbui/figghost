//! Draws the sky of the Weather tab on the GPU into PNGs for every kind of weather, without showing anything.
//! `cargo run -p macos-sys --example render_sky -- /tmp/sky`

use macos_sys::weather_fx::{Atmosphere, render_image};
use std::{env, fs, path::PathBuf, process::Command};

fn base(day: bool, progress: f64) -> Atmosphere {
    Atmosphere {
        day,
        progress,
        wind_dir: 1.0,
        leaf: (0.95, 0.56, 0.16),
        ..Atmosphere::default()
    }
}

fn main() {
    let dir = PathBuf::from(env::args().nth(1).unwrap_or_else(|| "/tmp/sky".into()));
    fs::create_dir_all(&dir).expect("output directory");
    let scenes = [
        (
            "noon",
            Atmosphere {
                cloud: 0.25,
                ..base(true, 0.5)
            },
        ),
        (
            "morning",
            Atmosphere {
                cloud: 0.4,
                ..base(true, 0.12)
            },
        ),
        (
            "sunset",
            Atmosphere {
                cloud: 0.45,
                ..base(true, 0.94)
            },
        ),
        ("night", base(false, 0.4)),
        (
            "overcast",
            Atmosphere {
                cloud: 0.9,
                ..base(true, 0.5)
            },
        ),
        (
            "drizzle",
            Atmosphere {
                cloud: 0.85,
                rain: 0.15,
                ..base(true, 0.4)
            },
        ),
        (
            "downpour",
            Atmosphere {
                cloud: 1.0,
                rain: 0.95,
                thunder: true,
                wind: 0.5,
                ..base(true, 0.4)
            },
        ),
        (
            "snow",
            Atmosphere {
                cloud: 0.9,
                snow: 0.8,
                cold: 0.8,
                ..base(true, 0.4)
            },
        ),
        (
            "cold",
            Atmosphere {
                cloud: 0.15,
                cold: 0.9,
                ..base(false, 0.3)
            },
        ),
        (
            "wind",
            Atmosphere {
                cloud: 0.5,
                wind: 0.5,
                ..base(true, 0.6)
            },
        ),
        (
            "gale",
            Atmosphere {
                cloud: 0.7,
                wind: 1.0,
                ..base(true, 0.6)
            },
        ),
        (
            "haze",
            Atmosphere {
                haze: 0.85,
                cloud: 0.2,
                ..base(true, 0.3)
            },
        ),
        (
            "fog",
            Atmosphere {
                fog: 0.8,
                cloud: 0.8,
                ..base(true, 0.3)
            },
        ),
    ];
    for (name, atm) in scenes {
        let (w, h, scale) = (580usize, 250usize, 2usize);
        let Some(bgra) = render_image(atm, 14.0, w as f64, h as f64, scale as f64) else {
            eprintln!("no GPU");
            return;
        };
        let mut ppm = format!("P6\n{} {}\n255\n", w * scale, h * scale).into_bytes();
        for px in bgra.chunks_exact(4) {
            // Premultiplied over black, which is what the notch is.
            ppm.extend_from_slice(&[px[2], px[1], px[0]]);
        }
        let file = dir.join(format!("{name}.ppm"));
        fs::write(&file, ppm).expect("write");
        let png = dir.join(format!("{name}.png"));
        Command::new("sips")
            .args(["-s", "format", "png"])
            .arg(&file)
            .arg("--out")
            .arg(&png)
            .output()
            .expect("sips");
        fs::remove_file(&file).ok();
        println!("wrote {}", png.display());
    }
}
