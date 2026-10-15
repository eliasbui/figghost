//! Draws known Vietnamese and English text into a picture and checks that recognition reads it back.
//!
//!     cargo run -p macos-sys --example ocr_check

use macos_sys::{ocr, screen_capture::Shot};
use objc2::{AnyThread, MainThreadMarker};
use objc2_app_kit::{
    NSApplication, NSBitmapImageRep, NSColor, NSDeviceRGBColorSpace, NSFont, NSGraphicsContext,
    NSStringDrawing,
};
use objc2_foundation::{NSDate, NSPoint, NSRect, NSRunLoop, NSSize, NSString};
use std::{cell::RefCell, rc::Rc, time::Instant};

const LINES: [&str; 2] = ["Xin chào thế giới, chụp màn hình", "Hello world 2026"];

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
    objc2_app_kit::NSBezierPath::fillRect(NSRect::new(
        NSPoint::ZERO,
        NSSize::new(width as f64, height as f64),
    ));
    let font = NSFont::systemFontOfSize(56.0);
    let attrs = objc2_foundation::NSDictionary::from_retained_objects(
        &[unsafe { objc2_app_kit::NSFontAttributeName }],
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

    let started = Instant::now();
    let answer: Rc<RefCell<Option<Result<ocr::Recognized, String>>>> = Rc::default();
    let slot = Rc::clone(&answer);
    ocr::recognize(&shot, move |r| *slot.borrow_mut() = Some(r));
    while answer.borrow().is_none() && started.elapsed().as_secs() < 20 {
        NSRunLoop::mainRunLoop().runUntilDate(&NSDate::dateWithTimeIntervalSinceNow(0.05));
    }
    let result = answer
        .borrow_mut()
        .take()
        .expect("no answer")
        .expect("recognition failed");
    println!("recognized in {:?}: {:?}", started.elapsed(), result.lines);
    let ok = LINES
        .iter()
        .all(|line| result.lines.iter().any(|l| l == line));
    println!("{}", if ok { "OK" } else { "FAIL: lines differ" });
}
