#![forbid(unsafe_code)]
//! Lyrics for the song that is playing: the LRC format, the LRCLIB client, the cache on disk, and the
//! service that ties them together without ever blocking the interface.

pub mod cache;
pub mod lrc;
pub mod lrclib;
pub mod service;
