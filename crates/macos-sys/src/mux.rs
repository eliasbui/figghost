//! Puts the picture of one file and the sound of another into a single mp4 without encoding either again. A site that
//! offers the two apart (YouTube does) leaves a video-only mp4 and an m4a; this is what makes them one file when no ffmpeg
//! is there to do it. Both calls block, so they belong on a thread of their own.

use block2::RcBlock;
use objc2::rc::{Retained, autoreleasepool};
use objc2_av_foundation::{
    AVAssetExportPresetPassthrough, AVAssetExportSession, AVAssetExportSessionStatus,
    AVFileTypeAppleM4A, AVFileTypeMPEG4, AVMediaType, AVMediaTypeAudio, AVMediaTypeVideo,
    AVMutableComposition, AVURLAsset,
};
use objc2_core_media::{CMTime, CMTimeRange, kCMPersistentTrackID_Invalid, kCMTimeZero};
use objc2_foundation::{NSString, NSURL};
use std::{
    fs::File,
    io::{Read, Seek, SeekFrom},
    path::Path,
    sync::mpsc,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Kind {
    Video,
    Audio,
}

impl Kind {
    fn media_type(self) -> &'static AVMediaType {
        // SAFETY: framework constants, valid for the whole process.
        unsafe {
            match self {
                Kind::Video => AVMediaTypeVideo,
                Kind::Audio => AVMediaTypeAudio,
            }
        }
        .expect("AVFoundation defines its media types")
    }

    fn name(self) -> &'static str {
        match self {
            Kind::Video => "picture",
            Kind::Audio => "sound",
        }
    }

    /// The `handler_type` of the `hdlr` box of a track of this kind.
    fn handler(self) -> &'static [u8; 4] {
        match self {
            Kind::Video => b"vide",
            Kind::Audio => b"soun",
        }
    }
}

/// A duration that a file states about itself, in ticks of `timescale` per second.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Declared {
    ticks: i64,
    timescale: i32,
}

impl Declared {
    fn seconds(self) -> f64 {
        self.ticks as f64 / f64::from(self.timescale)
    }
}

/// The boxes laid one after the other in `data`: their type and what is inside. Stops at the first one whose size does not
/// fit in what is left, so that a cut or damaged file ends the walk instead of breaking it.
fn boxes(mut data: &[u8]) -> impl Iterator<Item = (&[u8], &[u8])> {
    std::iter::from_fn(move || {
        let size = u32::from_be_bytes(data.get(..4)?.try_into().ok()?);
        let kind = data.get(4..8)?;
        let (header, size) = match size {
            0 => (8, data.len()),
            1 => (
                16,
                usize::try_from(u64::from_be_bytes(data.get(8..16)?.try_into().ok()?)).ok()?,
            ),
            n => (8, n as usize),
        };
        let inside = data.get(header..size)?;
        data = &data[size..];
        Some((kind, inside))
    })
}

/// The box of type `kind` in `data`, the first if there are several.
fn find<'a>(data: &'a [u8], kind: &[u8; 4]) -> Option<&'a [u8]> {
    boxes(data)
        .find(|(k, _)| k == kind)
        .map(|(_, inside)| inside)
}

/// The timescale and duration of an `mvhd` or `mdhd` box, which lay them out alike. A duration of nothing, or of every bit
/// set, which is how a file says it does not know, is no duration.
fn time_header(inside: &[u8]) -> Option<Declared> {
    let (scale, ticks) = match *inside.first()? {
        0 => (
            inside.get(12..16)?,
            u64::from(u32::from_be_bytes(inside.get(16..20)?.try_into().ok()?)),
        ),
        1 => (
            inside.get(20..24)?,
            u64::from_be_bytes(inside.get(24..32)?.try_into().ok()?),
        ),
        _ => return None,
    };
    let unknown = if inside[0] == 0 {
        u64::from(u32::MAX)
    } else {
        u64::MAX
    };
    let timescale = i32::from_be_bytes(scale.try_into().ok()?);
    (timescale > 0 && ticks != 0 && ticks != unknown).then_some(Declared {
        ticks: i64::try_from(ticks).ok()?,
        timescale,
    })
}

/// What the `moov` box says of the length of the first track of `kind`: its `mdhd`, else the `mvhd` of the whole movie.
/// A fragmented mp4 (YouTube serves them) holds its samples in `moof` boxes that AVFoundation counts twice over, so this
/// is the number to trust.
fn declared_in(moov: &[u8], kind: Kind) -> Option<Declared> {
    let own = boxes(moov)
        .filter(|(k, _)| *k == b"trak")
        .filter_map(|(_, trak)| find(trak, b"mdia"))
        .find(|mdia| {
            find(mdia, b"hdlr").and_then(|h| h.get(8..12)) == Some(kind.handler().as_slice())
        })
        .and_then(|mdia| time_header(find(mdia, b"mdhd")?));
    own.or_else(|| time_header(find(moov, b"mvhd")?))
}

/// The inside of the top-level `moov` box of the file, walking over the boxes before it (a `mdat` of any size) with seeks.
fn read_moov(path: &Path) -> Option<Vec<u8>> {
    const MOST: u64 = 64 << 20;
    let mut file = File::open(path).ok()?;
    let length = file.metadata().ok()?.len();
    let mut at = 0;
    while at + 8 <= length {
        file.seek(SeekFrom::Start(at)).ok()?;
        let mut head = [0; 16];
        file.read_exact(&mut head[..8]).ok()?;
        let (header, size) = match u32::from_be_bytes(head[..4].try_into().ok()?) {
            0 => (8, length - at),
            1 => {
                file.read_exact(&mut head[8..]).ok()?;
                (16, u64::from_be_bytes(head[8..].try_into().ok()?))
            }
            n => (8, u64::from(n)),
        };
        if size < header || at.checked_add(size)? > length {
            return None;
        }
        if &head[4..8] == b"moov" {
            if size - header > MOST {
                return None;
            }
            let mut inside = vec![0; usize::try_from(size - header).ok()?];
            file.read_exact(&mut inside).ok()?;
            return Some(inside);
        }
        at += size;
    }
    None
}

/// The length the file states for its first track of `kind`, `None` when it states none or cannot be read.
fn declared_duration(path: &Path, kind: Kind) -> Option<Declared> {
    declared_in(&read_moov(path)?, kind)
}

/// What a media file holds.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Tracks {
    pub video: usize,
    pub audio: usize,
    pub seconds: f64,
}

fn asset(path: &Path) -> Retained<AVURLAsset> {
    let url = NSURL::fileURLWithPath(&NSString::from_str(&path.to_string_lossy()));
    // SAFETY: a plain AVFoundation constructor with a file URL.
    unsafe { AVURLAsset::URLAssetWithURL_options(&url, None) }
}

/// The tracks of the file, `None` when AVFoundation cannot make anything of it.
pub fn tracks(path: &Path) -> Option<Tracks> {
    autoreleasepool(|_| {
        let asset = asset(path);
        // SAFETY: plain property reads of a local file.
        #[allow(deprecated)]
        unsafe {
            let seconds = asset.duration().seconds();
            (seconds.is_finite() && seconds > 0.0).then(|| Tracks {
                video: asset.tracksWithMediaType(Kind::Video.media_type()).len(),
                audio: asset.tracksWithMediaType(Kind::Audio.media_type()).len(),
                seconds,
            })
        }
    })
}

/// Writes `video` and `audio` into `out`, an mp4 that must not exist yet. The picture is the first video track of `video`;
/// the sound is the first audio track of `video` when it has one, else of `audio`.
pub fn join(video: &Path, audio: &Path, out: &Path) -> Result<(), String> {
    let own_sound = tracks(video).is_some_and(|t| t.audio > 0);
    let sound = if own_sound { video } else { audio };
    export(&[(video, Kind::Video), (sound, Kind::Audio)], out)
}

/// Writes the sound of `sound` alone into `out`, an m4a that must not exist yet. A fragmented m4a, which is what a site
/// serves, becomes an ordinary one that every player counts the right length for.
pub fn rewrite_sound(sound: &Path, out: &Path) -> Result<(), String> {
    export(&[(sound, Kind::Audio)], out)
}

/// Takes the first track of each kind named from each file, in one file: an m4a when `out` ends so, else an mp4.
fn export(parts: &[(&Path, Kind)], out: &Path) -> Result<(), String> {
    if out.exists() {
        return Err("the file to write is already there".into());
    }
    autoreleasepool(|_| {
        // SAFETY: AVFoundation calls with live objects and the documented argument types; the assets are kept until the
        // export has ended, since the composition only refers to them.
        unsafe {
            let composition = AVMutableComposition::composition();
            let mut assets = Vec::new();
            for &(path, kind) in parts {
                let source = asset(path);
                #[allow(deprecated)]
                let found = source.tracksWithMediaType(kind.media_type());
                let track = found.iter().next().ok_or_else(|| {
                    format!(
                        "{} has no {}",
                        path.file_name().unwrap_or_default().to_string_lossy(),
                        kind.name()
                    )
                })?;
                let slot = composition
                    .addMutableTrackWithMediaType_preferredTrackID(
                        kind.media_type(),
                        kCMPersistentTrackID_Invalid,
                    )
                    .ok_or("cannot add a track")?;
                // Not the whole range AVFoundation reports: it takes a fragmented mp4 for twice as long as it says it
                // is, and a composition made of that would hold the last picture still for the extra half.
                let mut range = track.timeRange();
                if let Some(said) = declared_duration(path, kind)
                    && said.seconds() < range.duration.seconds()
                {
                    range = CMTimeRange {
                        start: range.start,
                        duration: CMTime::new(said.ticks, said.timescale),
                    };
                }
                slot.insertTimeRange_ofTrack_atTime_error(range, &track, kCMTimeZero)
                    .map_err(|e| e.localizedDescription().to_string())?;
                if kind == Kind::Video {
                    slot.setPreferredTransform(track.preferredTransform());
                }
                assets.push(source);
            }
            let session = AVAssetExportSession::exportSessionWithAsset_presetName(
                &composition,
                AVAssetExportPresetPassthrough,
            )
            .ok_or("these streams cannot be written without encoding them again")?;
            let m4a = out.extension().is_some_and(|e| e == "m4a");
            session.setOutputFileType(if m4a {
                AVFileTypeAppleM4A
            } else {
                AVFileTypeMPEG4
            });
            session.setOutputURL(Some(&NSURL::fileURLWithPath(&NSString::from_str(
                &out.to_string_lossy(),
            ))));
            let (done, wait) = mpsc::channel();
            let handler = RcBlock::new(move || {
                let _ = done.send(());
            });
            session.exportAsynchronouslyWithCompletionHandler(&handler);
            wait.recv().map_err(|_| "the export never reported")?;
            drop(assets);
            if session.status() == AVAssetExportSessionStatus::Completed {
                Ok(())
            } else {
                let _ = std::fs::remove_file(out);
                Err(session.error().map_or_else(
                    || "the export did not finish".to_owned(),
                    |e| e.localizedDescription().to_string(),
                ))
            }
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    struct Dir(std::path::PathBuf);

    impl Dir {
        fn new(name: &str) -> Self {
            let path =
                std::env::temp_dir().join(format!("figghost-mux-{name}-{}", std::process::id()));
            let _ = fs::remove_dir_all(&path);
            fs::create_dir_all(&path).unwrap();
            Self(path)
        }
    }

    impl Drop for Dir {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    fn mp4_box(kind: &[u8; 4], inside: &[u8]) -> Vec<u8> {
        let mut out = (inside.len() as u32 + 8).to_be_bytes().to_vec();
        out.extend(kind);
        out.extend(inside);
        out
    }

    /// An `mvhd` or `mdhd` of the given version.
    fn time_box(kind: &[u8; 4], version: u8, timescale: u32, ticks: u64) -> Vec<u8> {
        let mut inside = vec![version, 0, 0, 0];
        let dates = if version == 0 { 8 } else { 16 };
        inside.extend(vec![0; dates]);
        inside.extend(timescale.to_be_bytes());
        if version == 0 {
            inside.extend((ticks as u32).to_be_bytes());
        } else {
            inside.extend(ticks.to_be_bytes());
        }
        inside.extend([0; 20]);
        mp4_box(kind, &inside)
    }

    fn trak(handler: &[u8; 4], mdhd: Vec<u8>) -> Vec<u8> {
        let mut hdlr = vec![0; 8];
        hdlr.extend(handler);
        hdlr.extend([0; 13]);
        let mdia = [mdhd, mp4_box(b"hdlr", &hdlr)].concat();
        mp4_box(b"trak", &mp4_box(b"mdia", &mdia))
    }

    fn moov(parts: &[Vec<u8>]) -> Vec<u8> {
        mp4_box(b"moov", &parts.concat())
    }

    fn declared(moov: &[u8], kind: Kind) -> Option<(i64, i32)> {
        declared_in(&moov[8..], kind).map(|d| (d.ticks, d.timescale))
    }

    #[test]
    fn the_duration_a_track_declares_is_read_from_its_mdhd_of_either_version() {
        // The numbers of the fragmented picture and sound of YouTube's first video.
        let moov = moov(&[
            time_box(b"mvhd", 0, 1000, 19_000),
            mp4_box(b"mvex", &[0; 32]),
            trak(b"vide", time_box(b"mdhd", 0, 15360, 290_816)),
            trak(b"soun", time_box(b"mdhd", 1, 44100, 840_704)),
        ]);
        assert_eq!(declared(&moov, Kind::Video), Some((290_816, 15360)));
        assert_eq!(declared(&moov, Kind::Audio), Some((840_704, 44100)));
        let said = declared_in(&moov[8..], Kind::Video).unwrap();
        assert!(
            (said.seconds() - 18.933).abs() < 0.001,
            "{}",
            said.seconds()
        );
        // The track of the kind asked for, not the first one.
        let sound_only = super::tests::moov(&[trak(b"soun", time_box(b"mdhd", 0, 44100, 44100))]);
        assert_eq!(declared(&sound_only, Kind::Audio), Some((44100, 44100)));
    }

    #[test]
    fn a_track_without_a_duration_falls_back_to_the_movie_and_then_to_nothing() {
        let blank = trak(b"vide", time_box(b"mdhd", 0, 15360, 0));
        let with_movie = moov(&[time_box(b"mvhd", 0, 1000, 5000), blank.clone()]);
        assert_eq!(declared(&with_movie, Kind::Video), Some((5000, 1000)));
        assert_eq!(declared(&with_movie, Kind::Audio), Some((5000, 1000)));
        let nothing = moov(&[time_box(b"mvhd", 0, 1000, 0), blank]);
        assert_eq!(declared(&nothing, Kind::Video), None);
        // A duration of every bit set is a file that does not know, and a timescale of nothing cannot divide.
        let unknown = moov(&[trak(b"vide", time_box(b"mdhd", 1, 600, u64::MAX))]);
        assert_eq!(declared(&unknown, Kind::Video), None);
        let unknown = moov(&[trak(b"vide", time_box(b"mdhd", 0, 600, u32::MAX.into()))]);
        assert_eq!(declared(&unknown, Kind::Video), None);
        let no_scale = moov(&[trak(b"vide", time_box(b"mdhd", 0, 0, 100))]);
        assert_eq!(declared(&no_scale, Kind::Video), None);
        assert_eq!(declared(&moov(&[]), Kind::Video), None);
    }

    #[test]
    fn boxes_that_are_cut_or_wrong_in_size_give_nothing_and_never_panic() {
        let good = moov(&[
            time_box(b"mvhd", 0, 1000, 5000),
            trak(b"vide", time_box(b"mdhd", 0, 15360, 290_816)),
        ]);
        // Every cut of the box, and every size it could state for itself, ends in a value or in nothing.
        for cut in 0..good.len() {
            let _ = declared_in(good[..cut].get(8..).unwrap_or_default(), Kind::Video);
        }
        for size in [0_u32, 1, 4, 7, 8, 9, 100, u32::MAX] {
            let mut bent = good.clone();
            bent[..4].copy_from_slice(&size.to_be_bytes());
            let _ = declared_in(&bent[8..], Kind::Video);
            let mut bent = good.clone();
            bent[8..12].copy_from_slice(&size.to_be_bytes());
            let _ = declared_in(&bent[8..], Kind::Video);
        }
        // A box that says it is longer than what is left stops the walk before it.
        let mut long = time_box(b"mvhd", 0, 1000, 5000);
        long[..4].copy_from_slice(&1000_u32.to_be_bytes());
        assert_eq!(declared_in(&long, Kind::Video), None);
        // A box that is smaller than its own header does too.
        let mut small = time_box(b"mvhd", 0, 1000, 5000);
        small[..4].copy_from_slice(&4_u32.to_be_bytes());
        assert_eq!(declared_in(&small, Kind::Video), None);
        // A size of nothing reaches to the end, and 1 takes the size from the eight bytes after the type.
        let mut to_end = time_box(b"mvhd", 0, 1000, 5000);
        to_end[..4].copy_from_slice(&0_u32.to_be_bytes());
        assert_eq!(
            declared_in(&to_end, Kind::Video).map(|d| d.ticks),
            Some(5000)
        );
        let plain = time_box(b"mvhd", 0, 1000, 5000);
        let mut wide = 1_u32.to_be_bytes().to_vec();
        wide.extend(b"mvhd");
        wide.extend((plain.len() as u64 + 8).to_be_bytes());
        wide.extend(&plain[8..]);
        assert_eq!(declared_in(&wide, Kind::Video).map(|d| d.ticks), Some(5000));
        // A header of a version nobody wrote is not read.
        let mut odd = time_box(b"mvhd", 0, 1000, 5000);
        odd[8] = 7;
        assert_eq!(declared_in(&odd, Kind::Video), None);
        assert_eq!(declared_in(&[], Kind::Video), None);
        assert_eq!(declared_in(&[0, 0, 0], Kind::Video), None);
    }

    #[test]
    fn the_moov_of_a_file_is_found_after_a_big_mdat_and_a_file_without_one_gives_nothing() {
        let dir = Dir::new("moov");
        let ftyp = mp4_box(b"ftyp", b"isom\0\0\0\0isom");
        let moov = moov(&[
            time_box(b"mvhd", 0, 1000, 7000),
            trak(b"soun", time_box(b"mdhd", 0, 44100, 308_700)),
        ]);
        let mdat = mp4_box(b"mdat", &vec![1; 300_000]);
        let path = dir.0.join("a.m4a");
        fs::write(&path, [ftyp.clone(), mdat.clone(), moov.clone()].concat()).unwrap();
        let said = declared_duration(&path, Kind::Audio).unwrap();
        assert_eq!((said.ticks, said.timescale), (308_700, 44100));
        assert!((said.seconds() - 7.0).abs() < 1e-9);

        // Cut inside the moov, cut inside the mdat before it, and not an mp4 at all.
        let cut = dir.0.join("cut.m4a");
        fs::write(
            &cut,
            [ftyp.clone(), mdat.clone(), moov[..moov.len() - 5].to_vec()].concat(),
        )
        .unwrap();
        assert!(declared_duration(&cut, Kind::Audio).is_none());
        fs::write(&cut, [ftyp.clone(), mdat[..1000].to_vec()].concat()).unwrap();
        assert!(declared_duration(&cut, Kind::Audio).is_none());
        fs::write(&cut, b"not an mp4 file at all, only text").unwrap();
        assert!(declared_duration(&cut, Kind::Audio).is_none());
        fs::write(&cut, b"").unwrap();
        assert!(declared_duration(&cut, Kind::Audio).is_none());
        assert!(declared_duration(&dir.0.join("missing"), Kind::Audio).is_none());
    }

    #[test]
    fn a_picture_only_mp4_and_an_m4a_become_one_mp4_with_a_picture_and_a_sound() {
        let dir = Dir::new("join");
        let source = dir.0.join("sound.mov");
        fs::write(&source, include_bytes!("../testdata/sound.mov")).unwrap();
        let whole = tracks(&source).expect("the sample opens");
        assert_eq!((whole.video, whole.audio), (1, 1));

        // The two halves a site would serve apart, cut out of the sample.
        let (picture, sound) = (dir.0.join("v.f133.mp4"), dir.0.join("v.f140.m4a"));
        export(&[(&source, Kind::Video)], &picture).unwrap();
        export(&[(&source, Kind::Audio)], &sound).unwrap();
        let half = tracks(&picture).unwrap();
        assert_eq!((half.video, half.audio), (1, 0), "no sound in the picture");
        let half = tracks(&sound).unwrap();
        assert_eq!((half.video, half.audio), (0, 1), "no picture in the sound");

        let joined = dir.0.join("v.mp4");
        join(&picture, &sound, &joined).unwrap();
        let got = tracks(&joined).expect("the result opens");
        assert_eq!((got.video, got.audio), (1, 1));
        assert!(
            (got.seconds - whole.seconds).abs() < 0.1,
            "{} against {}",
            got.seconds,
            whole.seconds
        );

        // It does not write over a file, and a part without what is wanted from it is an error that names it.
        assert!(join(&picture, &sound, &joined).is_err());
        let wrong = dir.0.join("w.mp4");
        let error = join(&picture, &picture, &wrong).unwrap_err();
        assert!(error.contains("v.f133.mp4 has no sound"), "{error}");
        assert!(!wrong.exists(), "nothing half written is left");
        assert!(join(&dir.0.join("missing.mp4"), &sound, &wrong).is_err());
        assert!(tracks(&dir.0.join("missing.mp4")).is_none());
    }
}
