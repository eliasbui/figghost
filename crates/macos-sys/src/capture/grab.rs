//! The picture itself, through ScreenCaptureKit (`SCScreenshotManager`, macOS 14 and later): a display or a
//! region of it at native pixels, or one window alone. The answer comes back on the main thread.
//!
//! The region is cut from the picture of the whole display, so that the crop is the tested function in
//! `image.rs` and not a rounding rule of the system.

use super::{background::MainOnly, image};
use block2::RcBlock;
use core_capture::{Grab, Shot};
use dispatch2::DispatchQueue;
use objc2::{AnyThread, rc::Retained, runtime::AnyClass};
use objc2_core_graphics::CGImage;
use objc2_foundation::{NSArray, NSError};
use objc2_screen_capture_kit::{
    SCCaptureResolutionType, SCContentFilter, SCScreenshotManager, SCShareableContent,
    SCStreamConfiguration, SCWindow,
};
use std::cell::Cell;

type Done = Box<dyn FnOnce(Result<Shot, String>)>;

/// Whether this system has the screenshot API (macOS 14 and later).
pub fn available() -> bool {
    AnyClass::get(c"SCScreenshotManager").is_some()
}

/// Hands the answer to `done` on the main thread.
fn finish(done: MainOnly<Done>, result: Result<Shot, String>) {
    DispatchQueue::main().exec_async(move || done.into_inner()(result));
}

fn describe(error: *mut NSError) -> String {
    // SAFETY: the pointer is null or an error that is alive for the call.
    match unsafe { error.as_ref() } {
        Some(error) => error.localizedDescription().to_string(),
        None => "the system returned no image".to_owned(),
    }
}

/// Takes the picture. `exclude` are the window numbers of this app's own overlay and confirmation, which
/// must not be in it. `done` runs on the main thread, also when the answer is an error.
pub fn grab(what: Grab, exclude: Vec<u32>, done: Done) {
    let done = MainOnly::new(done);
    if !available() {
        return finish(
            done,
            Err("this version of macOS cannot capture the screen".into()),
        );
    }
    let job = Cell::new(Some((what, exclude, done)));
    let block = RcBlock::new(
        move |content: *mut SCShareableContent, error: *mut NSError| {
            let Some((what, exclude, done)) = job.take() else {
                return;
            };
            // SAFETY: the pointer is null or a live object for the duration of the call.
            match unsafe { content.as_ref() } {
                Some(content) => start(content, what, &exclude, done),
                None => finish(done, Err(describe(error))),
            }
        },
    );
    // SAFETY: the block lives until the call returns and the system copies it if it needs it longer.
    unsafe { SCShareableContent::getShareableContentWithCompletionHandler(&block) };
}

fn start(content: &SCShareableContent, what: Grab, exclude: &[u32], done: MainOnly<Done>) {
    // SAFETY: plain getters on live objects, called with the content the system just delivered.
    let result = unsafe { prepare(content, &what, exclude) };
    match result {
        Ok((filter, config, region, scale)) => capture(&filter, &config, region, scale, done),
        Err(message) => finish(done, Err(message)),
    }
}

type Prepared = (
    Retained<SCContentFilter>,
    Retained<SCStreamConfiguration>,
    Option<(core_capture::Rect, core_capture::Display)>,
    f64,
);

/// # Safety
/// `content` must be alive. Every call is a documented getter or initializer of ScreenCaptureKit.
unsafe fn prepare(
    content: &SCShareableContent,
    what: &Grab,
    exclude: &[u32],
) -> Result<Prepared, String> {
    // SAFETY: see the function's contract.
    unsafe {
        let config = SCStreamConfiguration::new();
        config.setShowsCursor(false);
        config.setCaptureResolution(SCCaptureResolutionType::Best);
        match what {
            Grab::Region { display, .. } | Grab::Screen(display) => {
                let sc_display = content
                    .displays()
                    .iter()
                    .find(|d| d.displayID() == display.id)
                    .ok_or("the display is no longer there")?;
                let ours: Vec<Retained<SCWindow>> = content
                    .windows()
                    .iter()
                    .filter(|w| exclude.contains(&w.windowID()))
                    .collect();
                let filter = SCContentFilter::initWithDisplay_excludingWindows(
                    SCContentFilter::alloc(),
                    &sc_display,
                    &NSArray::from_retained_slice(&ours),
                );
                config.setWidth((display.frame.w * display.scale).round() as usize);
                config.setHeight((display.frame.h * display.scale).round() as usize);
                let region = match what {
                    Grab::Region { rect, .. } => Some((*rect, *display)),
                    _ => None,
                };
                Ok((filter, config, region, display.scale))
            }
            Grab::Window { id, shadow } => {
                let window = content
                    .windows()
                    .iter()
                    .find(|w| w.windowID() == *id)
                    .ok_or("the window is gone")?;
                let filter = SCContentFilter::initWithDesktopIndependentWindow(
                    SCContentFilter::alloc(),
                    &window,
                );
                let info = SCShareableContent::infoForFilter(&filter);
                let (rect, scale) = (info.contentRect(), f64::from(info.pointPixelScale()));
                config.setWidth((rect.size.width * scale).ceil() as usize);
                config.setHeight((rect.size.height * scale).ceil() as usize);
                config.setIgnoreShadowsSingleWindow(!shadow);
                Ok((filter, config, None, scale))
            }
        }
    }
}

fn capture(
    filter: &SCContentFilter,
    config: &SCStreamConfiguration,
    region: Option<(core_capture::Rect, core_capture::Display)>,
    scale: f64,
    done: MainOnly<Done>,
) {
    let job = Cell::new(Some((region, done)));
    let block = RcBlock::new(move |picture: *mut CGImage, error: *mut NSError| {
        let Some((region, done)) = job.take() else {
            return;
        };
        // SAFETY: the pointer is null or an image that is alive for the duration of the call.
        let result = match unsafe { picture.as_ref() } {
            Some(picture) => image::shot(picture, region.as_ref().map(|(r, d)| (*r, d)), scale),
            None => Err(describe(error)),
        };
        finish(done, result);
    });
    // SAFETY: the block lives until the call returns and the system copies it if it needs it longer.
    unsafe {
        SCScreenshotManager::captureImageWithFilter_configuration_completionHandler(
            filter,
            config,
            Some(&block),
        );
    }
}
