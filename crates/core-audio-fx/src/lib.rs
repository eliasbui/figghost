#![forbid(unsafe_code)]
//! Audio effects for the per-app mixer: a smoothed volume, a 10-band equalizer, dynamics (Magic Boost
//! and Night mode), stereo width and balance, spatial 3D for headphones, and a peak limiter that keeps a boost past 100% from
//! clipping.
//!
//! [`Params`] is shared between the interface and the audio thread through atomics only. A
//! [`Processor`] belongs to one audio stream and never allocates, locks or blocks while it processes,
//! so it can run on the real-time IO thread of an audio device.

mod biquad;
mod dynamics;
mod eq;
mod params;
mod processor;
mod spatial;

pub use biquad::Coefficients;
pub use eq::{BAND_HZ, BANDS, MAX_BAND_DB, Preset};
pub use params::{MAX_GAIN, MAX_ORBIT, Params};
pub use processor::Processor;
