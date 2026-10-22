//! Turns a recording into an animated GIF on a worker thread: frames read with AVFoundation at a lower
//! rate and size, written with ImageIO to loop forever.

use crate::reply;
use objc2::rc::{Retained, autoreleasepool};
use objc2_av_foundation::{AVAssetImageGenerator, AVURLAsset};
use objc2_core_foundation::{CFDictionary, CFString, CFURL, CGSize};
use objc2_core_graphics::CGImage;
use objc2_core_media::CMTime;
use objc2_foundation::{NSDictionary, NSNumber, NSString, NSURL};
use objc2_image_io::{
    CGImageDestination, kCGImagePropertyGIFDelayTime, kCGImagePropertyGIFDictionary,
    kCGImagePropertyGIFLoopCount,
};
use std::path::{Path, PathBuf};

const GIF_TYPE: &str = "com.compuserve.gif";
/// At most this many frames: a longer recording gets fewer frames per second, never a cut.
const MAX_FRAMES: usize = 900;
/// Decoded frames may use about this much memory in all: ImageIO can hold every frame until `finalize`, so
/// a tall video would otherwise take gigabytes. A big frame gets fewer frames per second, never a cut.
const FRAME_BUDGET_BYTES: usize = 512 * 1024 * 1024;
const TIMESCALE: i32 = 600;
/// GIF frame delays are in hundredths of a second; 8 of them make exactly 12.5 frames per second.
pub const GIF_FPS: f64 = 12.5;
/// The widest a GIF is made, in pixels.
pub const GIF_MAX_WIDTH: f64 = 960.0;

/// Converts `video` into `gif` (`fps` frames per second, at most `max_width` pixels wide); `done` runs
/// on the main thread with the GIF's path.
pub fn video_to_gif(
    video: &Path,
    gif: &Path,
    max_width: f64,
    fps: f64,
    done: impl FnOnce(Result<PathBuf, String>) + 'static,
) {
    let ticket = reply::expect(done);
    let (video, gif) = (video.to_owned(), gif.to_owned());
    std::thread::spawn(move || {
        ticket.send(write_gif(&video, &gif, max_width, fps).map(|()| gif));
    });
}

/// Like `video_to_gif`, for a caller that is already on a worker thread and waits for the file.
pub fn write_gif(video: &Path, gif: &Path, max_width: f64, fps: f64) -> Result<(), String> {
    autoreleasepool(|_| convert(video, gif, max_width, fps))
}

fn file_url(path: &Path) -> Retained<NSURL> {
    NSURL::fileURLWithPath(&NSString::from_str(&path.to_string_lossy()))
}

/// `{ GIF dictionary: { key: value } }`, as ImageIO wants frame and file properties.
fn gif_properties(
    key: &CFString,
    value: f64,
) -> Retained<NSDictionary<NSString, NSDictionary<NSString, NSNumber>>> {
    // SAFETY: CFString and NSString are toll-free bridged; the constants live for the whole process.
    let (gif_key, key) = unsafe {
        (
            &*(kCGImagePropertyGIFDictionary as *const CFString as *const NSString),
            &*(key as *const CFString as *const NSString),
        )
    };
    let inner = NSDictionary::from_retained_objects(&[key], &[NSNumber::new_f64(value)]);
    NSDictionary::from_retained_objects(&[gif_key], &[inner])
}

fn as_cf<T>(dictionary: &NSDictionary<NSString, T>) -> &CFDictionary {
    // SAFETY: NSDictionary and CFDictionary are toll-free bridged.
    unsafe { &*(dictionary as *const NSDictionary<NSString, T> as *const CFDictionary) }
}

/// The frame rate and frame count for a recording of `duration` seconds: `fps` as asked, lowered so that
/// neither `max_frames` nor `budget` bytes of `frame_bytes` each are exceeded; always at least one frame.
fn plan(
    duration: f64,
    fps: f64,
    frame_bytes: usize,
    budget: usize,
    max_frames: usize,
) -> (f64, usize) {
    let cap = (budget / frame_bytes.max(1)).clamp(1, max_frames);
    let fps = fps.clamp(1.0, 30.0).min(cap as f64 / duration);
    (fps, ((duration * fps).ceil() as usize).clamp(1, cap))
}

fn convert(video: &Path, gif: &Path, max_width: f64, fps: f64) -> Result<(), String> {
    // SAFETY: AVFoundation and ImageIO calls with live objects and the documented argument types; the
    // generator and destination are used on this thread only.
    unsafe {
        let asset = AVURLAsset::URLAssetWithURL_options(&file_url(video), None);
        #[allow(deprecated)]
        let duration = asset.duration().seconds();
        if !duration.is_finite() || duration <= 0.0 {
            return Err("the recording is empty".into());
        }
        let generator = AVAssetImageGenerator::assetImageGeneratorWithAsset(&asset);
        generator.setAppliesPreferredTrackTransform(true);
        generator.setMaximumSize(CGSize::new(max_width, max_width * 4.0));
        // The first frame sizes the budget; every frame has its size (`maximum_size` fits them alike).
        #[allow(deprecated)]
        let first = generator
            .copyCGImageAtTime_actualTime_error(CMTime::new(0, TIMESCALE), std::ptr::null_mut())
            .map_err(|e| e.localizedDescription().to_string())?;
        let frame_bytes = CGImage::bytes_per_row(Some(&first)) * CGImage::height(Some(&first));
        let (fps, frames) = plan(duration, fps, frame_bytes, FRAME_BUDGET_BYTES, MAX_FRAMES);
        let tolerance = CMTime::new((f64::from(TIMESCALE) / (2.0 * fps)) as i64, TIMESCALE);
        generator.setRequestedTimeToleranceBefore(tolerance);
        generator.setRequestedTimeToleranceAfter(tolerance);

        let url = file_url(gif);
        let cf_url = &*(Retained::as_ptr(&url) as *const CFURL);
        let gif_type = CFString::from_str(GIF_TYPE);
        let destination = CGImageDestination::with_url(cf_url, &gif_type, frames, None)
            .ok_or("cannot create the GIF file")?;
        destination.set_properties(Some(as_cf(&gif_properties(
            kCGImagePropertyGIFLoopCount,
            0.0,
        ))));
        let frame = gif_properties(kCGImagePropertyGIFDelayTime, 1.0 / fps);
        destination.add_image(&first, Some(as_cf(&frame)));
        for index in 1..frames {
            let at = CMTime::new(
                (index as f64 / fps * f64::from(TIMESCALE)) as i64,
                TIMESCALE,
            );
            #[allow(deprecated)]
            let image = generator
                .copyCGImageAtTime_actualTime_error(at, std::ptr::null_mut())
                .map_err(|e| e.localizedDescription().to_string())?;
            destination.add_image(&image, Some(as_cf(&frame)));
        }
        if !destination.finalize() {
            return Err("cannot write the GIF file".into());
        }
    }
    mark_version_89a(gif).map_err(|e| e.to_string())
}

/// ImageIO can label an animated GIF "GIF87a", a version without the frame delays and looping the file
/// uses; strict decoders then show one frame. The header is corrected to "GIF89a".
fn mark_version_89a(path: &Path) -> std::io::Result<()> {
    use std::io::{Read, Seek, SeekFrom, Write};
    let mut file = std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .open(path)?;
    let mut header = [0u8; 6];
    file.read_exact(&mut header)?;
    if &header == b"GIF87a" {
        file.seek(SeekFrom::Start(3))?;
        file.write_all(b"89a")?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    const BUDGET: usize = 512 * 1024 * 1024;

    #[test]
    fn a_short_video_keeps_its_rate() {
        assert_eq!(plan(10.0, 12.5, 4_000_000, BUDGET, 900), (12.5, 125));
    }

    #[test]
    fn a_long_video_is_held_to_max_frames() {
        let (fps, frames) = plan(300.0, 12.5, 100_000, BUDGET, 900);
        assert_eq!((fps, frames), (3.0, 900));
    }

    #[test]
    fn a_big_frame_is_held_to_the_budget() {
        // 960x1707 at 4 bytes a pixel: about 6.5 MB, so 81 frames fit.
        let (fps, frames) = plan(300.0, 12.5, 960 * 4 * 1707, BUDGET, 900);
        assert_eq!(frames, 81);
        assert!(fps < 0.3);
    }

    #[test]
    fn there_is_always_one_frame() {
        assert_eq!(plan(5.0, 12.5, usize::MAX, BUDGET, 900).1, 1);
        assert_eq!(plan(0.01, 12.5, 4_000_000, BUDGET, 900).1, 1);
    }
}
