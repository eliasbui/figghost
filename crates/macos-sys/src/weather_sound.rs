//! The sound behind the Weather tab: rain, wind and a clap of thunder, matched to the sky that is drawn. The sounds
//! are short loops made by synthesis and kept in the app; `mix` says how loud each one is, and `Audio` plays them.

use crate::weather_fx::Atmosphere;
use objc2::rc::Retained;
use objc2_app_kit::NSSound;
use objc2_foundation::NSData;

const RAIN_LIGHT: &[u8] = include_bytes!("../assets/weather/rain_light.wav");
const RAIN_HEAVY: &[u8] = include_bytes!("../assets/weather/rain_heavy.wav");
const WIND: &[u8] = include_bytes!("../assets/weather/wind.wav");
const WIND_STRONG: &[u8] = include_bytes!("../assets/weather/wind_strong.wav");
const THUNDER: &[u8] = include_bytes!("../assets/weather/thunder.wav");

/// How loud each loop is, from 0 to 1. Everything stays quiet: it is a background to the tab, not a recording.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Mix {
    pub rain_light: f64,
    pub rain_heavy: f64,
    pub wind: f64,
    pub wind_strong: f64,
}

const MASTER: f64 = 0.55;

pub fn mix(atm: &Atmosphere) -> Mix {
    let mut mix = Mix::default();
    if atm.rain > 0.02 {
        // A drizzle and a downpour are two recordings, crossfaded as the rain grows.
        let level = (0.25 + 0.55 * atm.rain) * MASTER;
        let heavy = ((atm.rain - 0.3) / 0.4).clamp(0.0, 1.0);
        mix.rain_light = level * (1.0 - heavy);
        mix.rain_heavy = level * heavy;
    }
    if atm.wind > 0.1 {
        let level = (0.12 + 0.6 * atm.wind) * MASTER;
        let strong = ((atm.wind - 0.5) / 0.3).clamp(0.0, 1.0);
        mix.wind = level * (1.0 - strong);
        mix.wind_strong = level * strong;
    }
    mix
}

fn load(bytes: &[u8]) -> Option<Retained<NSSound>> {
    use objc2::AnyThread;
    NSSound::initWithData(NSSound::alloc(), &NSData::with_bytes(bytes))
}

/// One loop that is made when first wanted and played at the volume it is given.
#[derive(Default)]
struct Loop(Option<Retained<NSSound>>);

impl Loop {
    fn set(&mut self, bytes: &[u8], volume: f64) {
        if volume < 0.01 {
            if let Some(sound) = &self.0 {
                sound.stop();
            }
            return;
        }
        if self.0.is_none() {
            self.0 = load(bytes);
            if let Some(sound) = &self.0 {
                sound.setLoops(true);
            }
        }
        if let Some(sound) = &self.0 {
            sound.setVolume(volume as f32);
            if !sound.isPlaying() {
                sound.play();
            }
        }
    }
}

#[derive(Default)]
pub struct Audio {
    rain_light: Loop,
    rain_heavy: Loop,
    wind: Loop,
    wind_strong: Loop,
    thunder: Option<Retained<NSSound>>,
}

impl Audio {
    /// Plays the sky's sounds, or silences them all with `None`.
    pub fn set(&mut self, mix: Option<Mix>) {
        let mix = mix.unwrap_or_default();
        self.rain_light.set(RAIN_LIGHT, mix.rain_light);
        self.rain_heavy.set(RAIN_HEAVY, mix.rain_heavy);
        self.wind.set(WIND, mix.wind);
        self.wind_strong.set(WIND_STRONG, mix.wind_strong);
    }

    /// A clap of thunder, once.
    pub fn thunder(&mut self) {
        if self.thunder.is_none() {
            self.thunder = load(THUNDER);
        }
        if let Some(sound) = &self.thunder {
            sound.stop();
            sound.setVolume(0.7);
            sound.play();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_dry_calm_sky_is_silent_and_a_storm_is_loud_but_never_too_loud() {
        assert_eq!(mix(&Atmosphere::default()), Mix::default());
        let storm = mix(&Atmosphere {
            rain: 1.0,
            wind: 1.0,
            ..Atmosphere::default()
        });
        assert!(storm.rain_heavy > 0.0 && storm.wind_strong > 0.0);
        assert!(storm.rain_light == 0.0 && storm.wind == 0.0);
        for v in [storm.rain_heavy, storm.wind_strong] {
            assert!(v <= MASTER && v > 0.2);
        }
    }

    #[test]
    fn a_drizzle_is_the_light_loop_and_the_loops_crossfade_between() {
        let drizzle = mix(&Atmosphere {
            rain: 0.15,
            ..Atmosphere::default()
        });
        assert!(drizzle.rain_light > 0.0 && drizzle.rain_heavy == 0.0);
        let between = mix(&Atmosphere {
            rain: 0.5,
            ..Atmosphere::default()
        });
        assert!(between.rain_light > 0.0 && between.rain_heavy > 0.0);
        let breeze = mix(&Atmosphere {
            wind: 0.3,
            ..Atmosphere::default()
        });
        assert!(breeze.wind > 0.0 && breeze.wind_strong == 0.0);
    }
}
