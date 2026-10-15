//! Records three seconds of an area of the main display into a temporary MP4 with the key display
//! included, then checks the file: its length and picture size, that the key display shows in the
//! video while a key is up and is gone once it fades, that a GIF made from it loops with the expected
//! frames, and that a recording of a missing window reports an error instead of starting. Needs Screen
//! Recording and Accessibility access for the terminal.
//!
//!     cargo run -p macos-sys --example record_check

use macos_sys::{
    gif,
    record_ui::KeyDisplay,
    recorder::{self, RecordOptions, Recording, Target},
    screen_capture::{self, Frozen, Pixels, Shot},
};
use objc2::{MainThreadMarker, rc::Retained};
use objc2_app_kit::NSApplication;
use objc2_av_foundation::{AVAssetImageGenerator, AVMediaTypeVideo, AVURLAsset};
use objc2_core_foundation::CFRetained;
use objc2_core_media::CMTime;
use objc2_foundation::{NSDate, NSPoint, NSRect, NSRunLoop, NSSize, NSString, NSURL};
use std::{cell::RefCell, env, fs, path::Path, path::PathBuf, ptr::NonNull, rc::Rc, time::Instant};

type Slot<T> = Rc<RefCell<Option<T>>>;

fn spin_until(seconds: f64, done: impl Fn() -> bool) {
    let start = Instant::now();
    while !done() && start.elapsed().as_secs_f64() < seconds {
        NSRunLoop::mainRunLoop().runUntilDate(&NSDate::dateWithTimeIntervalSinceNow(0.05));
    }
}

fn options(include_windows: Vec<u32>) -> RecordOptions {
    RecordOptions {
        fps: 30,
        system_audio: false,
        microphone: false,
        clicks: true,
        cursor: true,
        include_windows,
    }
}

/// The video's frame at `seconds`, as pixels.
fn frame_at(video: &Path, seconds: f64) -> Option<Pixels> {
    let url = NSURL::fileURLWithPath(&NSString::from_str(&video.to_string_lossy()));
    // SAFETY: plain AVFoundation reads of a local file.
    unsafe {
        let asset = AVURLAsset::URLAssetWithURL_options(&url, None);
        let generator = AVAssetImageGenerator::assetImageGeneratorWithAsset(&asset);
        let exact = CMTime::new(0, 600);
        generator.setRequestedTimeToleranceBefore(exact);
        generator.setRequestedTimeToleranceAfter(exact);
        #[allow(deprecated)]
        let image = generator
            .copyCGImageAtTime_actualTime_error(
                CMTime::new((seconds * 600.0) as i64, 600),
                std::ptr::null_mut(),
            )
            .ok()?;
        let image = CFRetained::retain(NonNull::from(&*image));
        let shot = Shot::from_cg(image, 1.0);
        if env::var_os("KEEP_FRAMES").is_some()
            && let Some(png) = shot.png()
        {
            let _ = fs::write(format!("/tmp/figghost-record-{seconds}.png"), png);
        }
        shot.pixels()
    }
}

/// Mean difference per channel between two frames inside `x0..x1`, `y0..y1` (pixels, top-left origin).
fn difference(a: &Pixels, b: &Pixels, (x0, x1): (usize, usize), (y0, y1): (usize, usize)) -> f64 {
    let mut sum = 0u64;
    for y in y0..y1 {
        for x in x0..x1 {
            for c in 0..3 {
                let i = y * a.stride + x * 4 + c;
                let j = y * b.stride + x * 4 + c;
                sum += u64::from(a.data[i].abs_diff(b.data[j]));
            }
        }
    }
    sum as f64 / ((x1 - x0) * (y1 - y0) * 3) as f64
}

fn main() {
    let mtm = MainThreadMarker::new().expect("main thread");
    let _app = NSApplication::sharedApplication(mtm);
    println!("recording supported: {}", recorder::recording_supported());
    let path = env::temp_dir().join(format!("figghost-record-check-{}.mp4", std::process::id()));
    let frozen: Slot<Frozen> = Rc::default();
    let slot = Rc::clone(&frozen);
    screen_capture::freeze(move |r| *slot.borrow_mut() = r.ok());
    spin_until(10.0, || frozen.borrow().is_some());
    let frozen = frozen.borrow_mut().take().expect("freeze");
    let display = frozen.displays[0].frame;
    let scale = frozen.displays[0].shot.scale();

    let missing: Slot<Result<Recording, String>> = Rc::default();
    let slot = Rc::clone(&missing);
    Recording::start(
        &frozen,
        Target::Window(u32::MAX),
        &path,
        &options(Vec::new()),
        move |r| *slot.borrow_mut() = Some(r),
        |_| {},
    );
    spin_until(10.0, || missing.borrow().is_some());
    let missing_ok = matches!(missing.borrow().as_ref(), Some(Err(_)));
    println!(
        "missing window reported: {:?}",
        missing.borrow().as_ref().map(|r| r.as_ref().err())
    );

    let area = NSRect::new(NSPoint::new(100.0, 100.0), NSSize::new(600.0, 400.0));
    let on_screen = NSRect::new(
        NSPoint::new(
            display.origin.x + area.origin.x,
            display.origin.y + area.origin.y,
        ),
        area.size,
    );
    let keys = KeyDisplay::show(mtm, on_screen).expect("key display (Accessibility access)");
    let recording: Slot<Result<Recording, String>> = Rc::default();
    let finished: Slot<Result<PathBuf, String>> = Rc::default();
    let (r, f) = (Rc::clone(&recording), Rc::clone(&finished));
    Recording::start(
        &frozen,
        Target::Area {
            display: 0,
            rect: area,
        },
        &path,
        &options(vec![keys.window_number()]),
        move |result| *r.borrow_mut() = Some(result),
        move |result| *f.borrow_mut() = Some(result),
    );
    spin_until(10.0, || recording.borrow().is_some());
    let recording = match recording.borrow_mut().take() {
        Some(Ok(recording)) => recording,
        other => {
            println!("FAIL: could not start: {:?}", other.map(|r| r.err()));
            return;
        }
    };
    let started = Instant::now();
    spin_until(0.5, || false);
    keys.press("Shift + Cmd + 4");
    spin_until(2.5, || false);
    println!("recorded {:.2} s so far", recording.duration());
    recording.stop();
    spin_until(10.0, || finished.borrow().is_some());
    keys.close();
    println!("stopped after {:?}", started.elapsed());
    let result = finished.borrow_mut().take();
    let Some(Ok(file)) = result else {
        println!("FAIL: {result:?}");
        return;
    };
    let size = fs::metadata(&file).map(|m| m.len()).unwrap_or(0);
    let url = NSURL::fileURLWithPath(&NSString::from_str(&file.to_string_lossy()));
    // SAFETY: plain reads of a local movie file's properties.
    let (seconds, natural) = unsafe {
        let asset: Retained<AVURLAsset> = AVURLAsset::URLAssetWithURL_options(&url, None);
        #[allow(deprecated)]
        let seconds = asset.duration().seconds();
        #[allow(deprecated)]
        let tracks = asset.tracksWithMediaType(AVMediaTypeVideo.unwrap());
        (seconds, tracks.firstObject().map(|t| t.naturalSize()))
    };
    println!("file {} bytes, {seconds:.2} s, video {natural:?}", size);

    // The key box sits 48 points above the bottom of the area, centred, 44 points tall.
    let px = |points: f64| (points * scale) as usize;
    let key_box = (
        (px(260.0), px(340.0)),
        (px(400.0 - 48.0 - 38.0), px(400.0 - 48.0 - 6.0)),
    );
    let corner = ((px(10.0), px(90.0)), (px(10.0), px(40.0)));
    let keys_ok = match (frame_at(&file, 1.2), frame_at(&file, 2.8)) {
        (Some(shown), Some(gone)) => {
            let at_key = difference(&shown, &gone, key_box.0, key_box.1);
            let elsewhere = difference(&shown, &gone, corner.0, corner.1);
            println!("key display: {at_key:.1} change where it shows, {elsewhere:.1} elsewhere");
            at_key > 20.0 && elsewhere < 5.0
        }
        _ => {
            println!("cannot read frames back");
            false
        }
    };

    let poster = recorder::poster(&file, scale);
    println!(
        "poster {:?}",
        poster.as_ref().map(|p| (p.width(), p.height()))
    );

    let gif_path = file.with_extension("gif");
    let converted: Slot<Result<PathBuf, String>> = Rc::default();
    let slot = Rc::clone(&converted);
    let started = Instant::now();
    gif::video_to_gif(&file, &gif_path, 480.0, 12.0, move |r| {
        *slot.borrow_mut() = Some(r)
    });
    spin_until(60.0, || converted.borrow().is_some());
    let gif_ok = match converted.borrow_mut().take() {
        Some(Ok(path)) => {
            let bytes = fs::read(&path).unwrap_or_default();
            // Each frame has a graphic control extension, which starts 0x21 0xF9.
            let frames = bytes.windows(2).filter(|w| w == &[0x21, 0xF9]).count();
            println!(
                "gif {} bytes, {frames} frames, made in {:?}",
                bytes.len(),
                started.elapsed()
            );
            if env::var_os("KEEP_GIF").is_some() {
                println!("kept {}", path.display());
            } else {
                let _ = fs::remove_file(&path);
            }
            bytes.starts_with(b"GIF89a") && (30..=40).contains(&frames)
        }
        other => {
            println!("gif failed: {other:?}");
            false
        }
    };
    let _ = fs::remove_file(&file);
    let ok = missing_ok
        && keys_ok
        && gif_ok
        && poster.is_some()
        && (2.5..4.5).contains(&seconds)
        && natural.is_some_and(|n| n.width == 600.0 * scale);
    println!("{}", if ok { "OK" } else { "FAIL" });
}
