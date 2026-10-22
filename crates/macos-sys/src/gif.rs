//! Turns a recording into an animated GIF on a worker thread: frames read with AVFoundation at a lower
//! rate and size, written with ImageIO to loop forever.

use crate::reply;
use objc2::rc::{Retained, autoreleasepool};
use objc2_av_foundation::{AVAssetImageGenerator, AVURLAsset};
use objc2_core_foundation::{CFDictionary, CFString, CFURL, CGSize};
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
const TIMESCALE: i32 = 600;

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
        let fps = fps.clamp(1.0, 30.0).min(MAX_FRAMES as f64 / duration);
        let generator = AVAssetImageGenerator::assetImageGeneratorWithAsset(&asset);
        generator.setAppliesPreferredTrackTransform(true);
        generator.setMaximumSize(CGSize::new(max_width, max_width * 4.0));
        let tolerance = CMTime::new((f64::from(TIMESCALE) / (2.0 * fps)) as i64, TIMESCALE);
        generator.setRequestedTimeToleranceBefore(tolerance);
        generator.setRequestedTimeToleranceAfter(tolerance);
        let frames = ((duration * fps).ceil() as usize).clamp(1, MAX_FRAMES);

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
        for index in 0..frames {
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
