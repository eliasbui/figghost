//! Playing music in the closed notch: the cover on the left and bars on the right, for whichever app plays,
//! browsers included. The bars follow the sound the Mac actually plays when the meter can hear it (it needs the
//! System Audio Recording permission, which macOS asks for the first time); otherwise, or with the "animated"
//! setting, they only move.

use crate::{config::NotchConfig, media::MediaFeature};
use macos_sys::{
    app,
    audio_meter::{self, Meter},
    notch::{Activity, Ear, Levels, Notch, artwork_tint},
    now_playing::Sample,
    style::Tone,
};
use std::{
    cell::{Cell, RefCell},
    rc::{Rc, Weak},
    time::Instant,
};

/// The slot of this activity in the closed notch.
const SLOT: &str = "media";
/// A pause shorter than this keeps the activity, so skipping a track does not make the notch jump.
const PAUSE_GRACE_SECONDS: f64 = 3.0;
/// Bars for a cover with no colour of its own.
const PLAIN_BARS: [f64; 3] = [0.98, 0.98, 1.0];
/// One band per bar of the closed notch.
const BANDS: usize = 4;

pub struct LiveMedia {
    notch: Weak<Notch>,
    media: Weak<MediaFeature>,
    on: Cell<bool>,
    /// The bars follow the sound, rather than only moving.
    listen: Cell<bool>,
    /// The cover's id and the colour taken from it, so a cover is looked at once.
    tint: RefCell<Option<(u64, [f64; 3])>>,
    /// Bumped by every change, so a pause that ended does not take the activity away later.
    serial: Cell<u64>,
    /// Listens to what the Mac plays, only while music plays: a running meter keeps the Mac and a Bluetooth output
    /// awake. Shared with the bars, which read it while they are drawn.
    meter: Rc<RefCell<Option<Meter>>>,
    /// The meter could not be started; it is not tried again until the settings change.
    meter_failed: Cell<bool>,
    started: Instant,
    me: Weak<LiveMedia>,
}

impl LiveMedia {
    pub fn attach(notch: &Rc<Notch>, media: &Rc<MediaFeature>, config: &NotchConfig) -> Rc<Self> {
        let live = Rc::new_cyclic(|me| Self {
            notch: Rc::downgrade(notch),
            media: Rc::downgrade(media),
            on: Cell::new(config.live_media),
            listen: Cell::new(config.waveform == "live"),
            tint: RefCell::new(None),
            serial: Cell::new(0),
            meter: Rc::default(),
            meter_failed: Cell::new(false),
            started: Instant::now(),
            me: me.clone(),
        });
        let weak = Rc::downgrade(&live);
        media.watch(move |sample| {
            if let Some(live) = weak.upgrade() {
                live.update(sample);
            }
        });
        live.update(media.current().as_ref());
        live
    }

    /// Applies the notch settings that concern it.
    pub fn configure(&self, config: &NotchConfig) {
        self.on.set(config.live_media);
        self.listen.set(config.waveform == "live");
        self.meter_failed.set(false);
        if !self.listen.get() {
            self.meter.borrow_mut().take();
        }
        let current = self.media.upgrade().and_then(|m| m.current());
        self.update(current.as_ref());
    }

    fn update(&self, sample: Option<&Sample>) {
        let Some(notch) = self.notch.upgrade() else {
            return;
        };
        let serial = self.serial.get() + 1;
        self.serial.set(serial);
        let playing = sample.filter(|s| s.track.playing);
        let Some(sample) = playing.filter(|_| self.on.get()) else {
            if !self.on.get() || sample.is_none() {
                notch.set_activity(SLOT, None);
                self.meter.borrow_mut().take();
                return;
            }
            // Paused: wait a moment before the notch closes in, in case it plays again.
            let weak = self.me.clone();
            app::after(PAUSE_GRACE_SECONDS, move || {
                if let Some(live) = weak.upgrade()
                    && live.serial.get() == serial
                    && let Some(notch) = live.notch.upgrade()
                {
                    notch.set_activity(SLOT, None);
                    live.meter.borrow_mut().take();
                }
            });
            return;
        };
        self.listen_if_wanted();
        let track = &sample.track;
        let left = match &track.artwork {
            Some(art) => Ear::Artwork {
                id: art.id,
                bytes: art.bytes.clone(),
            },
            None => Ear::Symbol {
                name: "music.note",
                tone: Tone::Purple,
                pulse: false,
            },
        };
        let color = self.tint_for(sample);
        let page = notch.page_ids().iter().position(|(id, _)| *id == "media");
        notch.set_activity(
            SLOT,
            Some(Activity {
                left,
                right: Ear::Bars {
                    levels: self.levels(),
                    color,
                },
                page,
                priority: 20,
            }),
        );
    }

    /// The bars' colour: taken from the cover, or plain.
    fn tint_for(&self, sample: &Sample) -> [f64; 3] {
        let Some(art) = &sample.track.artwork else {
            return PLAIN_BARS;
        };
        if let Some((id, color)) = *self.tint.borrow()
            && id == art.id
        {
            return color;
        }
        let color = artwork_tint(&art.bytes).unwrap_or(PLAIN_BARS);
        *self.tint.borrow_mut() = Some((art.id, color));
        color
    }

    /// Starts the meter while music plays, when the settings want the real sound.
    fn listen_if_wanted(&self) {
        if !self.listen.get() || self.meter_failed.get() || self.meter.borrow().is_some() {
            return;
        }
        match audio_meter::start(BANDS) {
            Ok(meter) => *self.meter.borrow_mut() = Some(meter),
            Err(e) => {
                eprintln!("live media: the bars cannot follow the sound ({e}); they only move");
                self.meter_failed.set(true);
            }
        }
    }

    /// What the bars show: the sound when the meter hears it, else movement. Silence from a meter that runs is
    /// either a quiet passage or a missing permission (the tap then delivers nothing), and the bars keep moving in
    /// both cases rather than lying flat while music plays.
    fn levels(&self) -> Levels {
        let (started, meter) = (self.started, Rc::clone(&self.meter));
        Rc::new(move |out: &mut [f32]| match meter.borrow().as_ref() {
            Some(meter) if meter.heard() => meter.levels(out),
            _ => moving(started.elapsed().as_secs_f32(), out),
        })
    }
}

/// Bars that move by themselves, like music, each at its own pace: for when the real sound cannot be heard.
fn moving(t: f32, out: &mut [f32]) {
    for (i, level) in out.iter_mut().enumerate() {
        let i = i as f32;
        let a = (t * (5.1 + i * 1.3) + i * 1.9).sin();
        let b = (t * (2.3 + i * 0.7) + i * 0.6).sin();
        *level = (0.55 + 0.3 * a + 0.15 * b).clamp(0.15, 1.0);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn moving_bars_stay_in_range_differ_and_change() {
        let (mut a, mut b) = ([0.0; 4], [0.0; 4]);
        moving(1.0, &mut a);
        moving(1.2, &mut b);
        assert!(a.iter().chain(&b).all(|v| (0.15..=1.0).contains(v)));
        assert_ne!(a[0], a[1], "each bar keeps its own pace");
        assert_ne!(a, b, "the bars move");
    }
}
