//! Draws known Vietnamese and English text into a picture and checks that recognition reads it back, both
//! from the picture in memory and from a PNG on disk, which is how the shelf reads an image it holds. It
//! also prints the languages the app asks for against the ones this system's Vision knows.
//!
//!     cargo run -p macos-sys --example ocr_check

use macos_sys::{ocr, screen_capture::Shot};
use objc2::{AnyThread, MainThreadMarker};
use objc2_app_kit::{
    NSApplication, NSBezierPath, NSBitmapImageFileType, NSBitmapImageRep, NSColor,
    NSDeviceRGBColorSpace, NSFont, NSFontAttributeName, NSGraphicsContext, NSStringDrawing,
};
use objc2_foundation::{NSDate, NSDictionary, NSPoint, NSRect, NSRunLoop, NSSize, NSString};
use objc2_vision::VNRecognizeTextRequest;
use std::{cell::RefCell, rc::Rc, time::Instant};

const LINES: [&str; 2] = ["Xin chào thế giới, chụp màn hình", "Hello world 2026"];
/// A recognition that takes longer than this is counted as one that never came back. The first one on a
/// cold system loads Vision's model, which took over twenty seconds here; later ones take 50 to 110 ms.
const WAIT: f64 = 60.0;

fn main() {
    let mtm = MainThreadMarker::new().expect("main thread");
    let _app = NSApplication::sharedApplication(mtm);
    let (width, height) = (1400isize, 320isize);
    // SAFETY: a plain RGBA bitmap with valid dimensions.
    let rep = unsafe {
        NSBitmapImageRep::initWithBitmapDataPlanes_pixelsWide_pixelsHigh_bitsPerSample_samplesPerPixel_hasAlpha_isPlanar_colorSpaceName_bytesPerRow_bitsPerPixel(
            NSBitmapImageRep::alloc(), std::ptr::null_mut(), width, height, 8, 4, true, false,
            NSDeviceRGBColorSpace, 0, 0,
        )
    }
    .expect("bitmap");
    let context = NSGraphicsContext::graphicsContextWithBitmapImageRep(&rep).expect("context");
    NSGraphicsContext::saveGraphicsState_class();
    NSGraphicsContext::setCurrentContext(Some(&context));
    NSColor::whiteColor().setFill();
    NSBezierPath::fillRect(NSRect::new(
        NSPoint::ZERO,
        NSSize::new(width as f64, height as f64),
    ));
    let font = NSFont::systemFontOfSize(56.0);
    let attrs = NSDictionary::from_retained_objects(
        &[unsafe { NSFontAttributeName }],
        &[objc2::rc::Retained::into_super(
            objc2::rc::Retained::into_super(font),
        )],
    );
    for (i, line) in LINES.iter().enumerate() {
        // SAFETY: drawing into the current bitmap context with a valid attribute dictionary.
        unsafe {
            NSString::from_str(line).drawAtPoint_withAttributes(
                NSPoint::new(40.0, 200.0 - i as f64 * 120.0),
                Some(&attrs),
            )
        };
    }
    NSGraphicsContext::restoreGraphicsState_class();
    let shot = Shot::from_bitmap(&rep, 1.0).expect("cg image");

    // What this system's Vision knows, against the languages the app asks for. A wanted language that is
    // not listed is dropped by `ocr`, which then leans on the automatic detection.
    let request = VNRecognizeTextRequest::new();
    // SAFETY: a plain query on a new request.
    let supported: Vec<String> = unsafe { request.supportedRecognitionLanguagesAndReturnError() }
        .map(|list| list.iter().map(|l| l.to_string()).collect())
        .unwrap_or_default();
    for wanted in ocr::LANGUAGES {
        println!(
            "language {wanted}: {}",
            if supported.iter().any(|s| s == wanted) {
                "known to this system"
            } else {
                "NOT known to this system"
            }
        );
    }

    let mut ok = check("the picture in memory", &shot);

    // The same picture in a file on disk, which is the route the shelf takes.
    let png = unsafe {
        rep.representationUsingType_properties(NSBitmapImageFileType::PNG, &NSDictionary::new())
    }
    .expect("the picture cannot be encoded");
    let path = std::env::temp_dir().join("figghost-ocr-check.png");
    std::fs::write(&path, png.to_vec()).expect("the file cannot be written");
    match Shot::from_file(&path) {
        Ok(from_file) => ok &= check("the same picture in a file", &from_file),
        Err(e) => {
            println!("FAIL: the file cannot be read: {e}");
            ok = false;
        }
    }
    let _ = std::fs::remove_file(&path);
    println!("{}", if ok { "OK" } else { "FAIL: lines differ" });
}

/// Recognises `what` and checks it holds every drawn line; answers whether it did.
fn check(what: &str, shot: &Shot) -> bool {
    let started = Instant::now();
    let answer: Rc<RefCell<Option<Result<ocr::Recognized, String>>>> = Rc::default();
    let slot = Rc::clone(&answer);
    ocr::recognize(shot, move |r| *slot.borrow_mut() = Some(r));
    while answer.borrow().is_none() && started.elapsed().as_secs_f64() < WAIT {
        NSRunLoop::mainRunLoop().runUntilDate(&NSDate::dateWithTimeIntervalSinceNow(0.05));
    }
    let Some(result) = answer.borrow_mut().take() else {
        println!("FAIL: {what}: no answer after {WAIT} seconds");
        return false;
    };
    match result {
        Ok(result) => {
            println!(
                "recognized in {what} in {:?}: {:?}",
                started.elapsed(),
                result.lines
            );
            LINES
                .iter()
                .all(|line| result.lines.iter().any(|l| l == line))
        }
        Err(e) => {
            println!("FAIL: {what}: {e}");
            false
        }
    }
}
