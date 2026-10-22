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
use objc2_core_media::{kCMPersistentTrackID_Invalid, kCMTimeZero};
use objc2_foundation::{NSString, NSURL};
use std::{path::Path, sync::mpsc};

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
                slot.insertTimeRange_ofTrack_atTime_error(track.timeRange(), &track, kCMTimeZero)
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
        fn new() -> Self {
            let path = std::env::temp_dir().join(format!("figghost-mux-{}", std::process::id()));
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

    #[test]
    fn a_picture_only_mp4_and_an_m4a_become_one_mp4_with_a_picture_and_a_sound() {
        let dir = Dir::new();
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
