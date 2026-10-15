//! Screenshots through ScreenCaptureKit: every display frozen at once for the selection overlay, one
//! window on its own with its shadow, and the screen recording permission. This app's own windows (the
//! notch, the volume indicator) never appear in a capture.

use crate::reply::{self, Ticket};
use block2::RcBlock;
use objc2::{AnyThread, MainThreadMarker, rc::Retained};
use objc2_app_kit::{NSBitmapImageFileType, NSBitmapImageRep, NSImage, NSScreen};
use objc2_core_foundation::{CFRetained, CGPoint, CGRect, CGSize};
use objc2_core_graphics::{CGImage, CGPreflightScreenCaptureAccess, CGRequestScreenCaptureAccess};
use objc2_foundation::{NSArray, NSDictionary, NSError, NSPoint, NSRect, NSSize};
use objc2_screen_capture_kit::{
    SCCaptureResolutionType, SCContentFilter, SCRunningApplication, SCScreenshotManager,
    SCShareableContent, SCStreamConfiguration, SCWindow,
};
use std::{
    ptr::NonNull,
    sync::{Arc, Mutex, PoisonError},
};

/// Smallest window, in points, offered when picking a window: anything smaller is a status item or a
/// floating control.
const MIN_WINDOW: f64 = 40.0;

/// Whether this app may record the screen. With `prompt`, macOS asks the user the first time and opens
/// System Settings after that.
pub fn screen_access(prompt: bool) -> bool {
    if CGPreflightScreenCaptureAccess() {
        return true;
    }
    prompt && CGRequestScreenCaptureAccess()
}

/// The folder macOS saves its own screenshots to, when the user changed it.
pub fn system_screenshot_folder() -> Option<std::path::PathBuf> {
    let defaults = objc2_foundation::NSUserDefaults::standardUserDefaults();
    let domain = defaults.persistentDomainForName(&objc2_foundation::NSString::from_str(
        "com.apple.screencapture",
    ))?;
    let location = domain.objectForKey(&objc2_foundation::NSString::from_str("location"))?;
    let location = location.downcast::<objc2_foundation::NSString>().ok()?;
    let path = location.stringByExpandingTildeInPath().to_string();
    (!path.is_empty()).then(|| std::path::PathBuf::from(path))
}

/// Where the pointer is, in AppKit screen coordinates.
pub fn pointer() -> NSPoint {
    objc2_app_kit::NSEvent::mouseLocation()
}

/// The local date and time now: year, month, day, hour, minute, second.
pub fn local_time() -> (i32, u32, u32, u32, u32, u32) {
    use objc2_foundation::{NSCalendar, NSCalendarUnit, NSDate};
    let parts = NSCalendar::currentCalendar().components_fromDate(
        NSCalendarUnit::Year
            | NSCalendarUnit::Month
            | NSCalendarUnit::Day
            | NSCalendarUnit::Hour
            | NSCalendarUnit::Minute
            | NSCalendarUnit::Second,
        &NSDate::now(),
    );
    let u = |v: isize| u32::try_from(v).unwrap_or(0);
    (
        i32::try_from(parts.year()).unwrap_or(1970),
        u(parts.month()),
        u(parts.day()),
        u(parts.hour()),
        u(parts.minute()),
        u(parts.second()),
    )
}

/// A captured image and how many pixels it has per point.
#[derive(Debug, Clone)]
pub struct Shot {
    image: CFRetained<CGImage>,
    scale: f64,
}

impl Shot {
    pub fn width(&self) -> usize {
        CGImage::width(Some(&self.image))
    }

    pub fn height(&self) -> usize {
        CGImage::height(Some(&self.image))
    }

    pub fn scale(&self) -> f64 {
        self.scale
    }

    pub(crate) fn cg(&self) -> &CGImage {
        &self.image
    }

    /// A picture made elsewhere (a drawing, an edited capture), with its pixels per point.
    pub fn from_cg(image: CFRetained<CGImage>, scale: f64) -> Self {
        Self { image, scale }
    }

    /// A rendered bitmap (a drawing, an edited capture) as a shot.
    pub fn from_bitmap(rep: &NSBitmapImageRep, scale: f64) -> Option<Shot> {
        let image = rep.CGImage()?;
        // SAFETY: the pointer comes from a live CGImage; retaining it keeps it alive as a CF object.
        let image = unsafe { CFRetained::retain(NonNull::from(&*image)) };
        Some(Self { image, scale })
    }

    /// The pixels `x, y, w, h` (origin top-left).
    pub fn crop(&self, x: usize, y: usize, w: usize, h: usize) -> Option<Shot> {
        let rect = CGRect::new(
            CGPoint::new(x as f64, y as f64),
            CGSize::new(w as f64, h as f64),
        );
        CGImage::with_image_in_rect(Some(&self.image), rect).map(|image| Shot {
            image,
            scale: self.scale,
        })
    }

    /// Size in points, as the image appears on screen.
    pub fn point_size(&self) -> NSSize {
        NSSize::new(
            self.width() as f64 / self.scale,
            self.height() as f64 / self.scale,
        )
    }

    /// PNG data, marked with its point size so other apps show it at the size it was on screen.
    pub fn png(&self) -> Option<Vec<u8>> {
        let rep = NSBitmapImageRep::initWithCGImage(NSBitmapImageRep::alloc(), &self.image);
        rep.setSize(self.point_size());
        // SAFETY: an empty properties dictionary is valid for PNG output.
        unsafe {
            rep.representationUsingType_properties(NSBitmapImageFileType::PNG, &NSDictionary::new())
        }
        .map(|data| data.to_vec())
    }

    pub fn ns_image(&self) -> Retained<NSImage> {
        NSImage::initWithCGImage_size(NSImage::alloc(), &self.image, self.point_size())
    }
}

/// One display as it was when the capture started.
#[derive(Debug, Clone)]
pub struct DisplayShot {
    pub id: u32,
    /// Where the display is, in AppKit screen coordinates (points, origin bottom-left of the main screen).
    pub frame: NSRect,
    pub shot: Shot,
}

/// A window that can be picked, in AppKit screen coordinates.
#[derive(Debug, Clone, PartialEq)]
pub struct WindowInfo {
    pub id: u32,
    pub frame: NSRect,
    pub title: String,
    pub app: String,
}

/// Every display frozen at the same moment, the windows on them from front to back, and what is needed
/// to capture one of those windows on its own.
pub struct Frozen {
    pub displays: Vec<DisplayShot>,
    pub windows: Vec<WindowInfo>,
    content: Sendable<Retained<SCShareableContent>>,
}

/// ScreenCaptureKit hands its content descriptions to completion handlers on its own queues; they are
/// immutable snapshots, safe to move to the main thread.
struct Sendable<T>(T);

// SAFETY: see above; only SCShareableContent snapshots are wrapped.
unsafe impl<T> Send for Sendable<T> {}

struct RawDisplay {
    id: u32,
    frame: CGRect,
    shot: Option<Shot>,
}

struct RawWindow {
    id: u32,
    frame: CGRect,
    title: String,
    app: String,
}

struct Raw {
    displays: Vec<RawDisplay>,
    windows: Vec<RawWindow>,
    content: Sendable<Retained<SCShareableContent>>,
}

/// Converts a rectangle measured from the top-left of the main display (Core Graphics) to one measured
/// from its bottom-left (AppKit).
fn appkit_rect(rect: CGRect, main_height: f64) -> NSRect {
    NSRect::new(
        NSPoint::new(
            rect.origin.x,
            main_height - (rect.origin.y + rect.size.height),
        ),
        rect.size,
    )
}

fn main_screen_height() -> f64 {
    let mtm = MainThreadMarker::new().expect("main thread");
    NSScreen::screens(mtm)
        .firstObject()
        .map_or(0.0, |s| s.frame().size.height)
}

fn error_text(error: *mut NSError) -> String {
    // SAFETY: the completion handler passes a valid NSError or null.
    unsafe { error.as_ref() }.map_or_else(
        || "unknown error".to_owned(),
        |e| e.localizedDescription().to_string(),
    )
}

/// This app's own processes as ScreenCaptureKit lists them, to leave out of every capture.
fn own_applications(content: &SCShareableContent) -> Retained<NSArray<SCRunningApplication>> {
    let pid = std::process::id() as i32;
    // SAFETY: plain property reads on a live content snapshot.
    let apps: Vec<Retained<SCRunningApplication>> = unsafe {
        content
            .applications()
            .iter()
            .filter(|app| app.processID() == pid)
            .collect()
    };
    NSArray::from_retained_slice(&apps)
}

fn retain_image(image: *mut CGImage) -> Option<CFRetained<CGImage>> {
    // SAFETY: the completion handler's image is valid for the call; retaining keeps it alive after.
    NonNull::new(image).map(|ptr| unsafe { CFRetained::retain(ptr) })
}

/// Captures every display, without this app's windows, and lists the windows on screen; `done` runs on
/// the main thread. Takes about a tenth of a second.
pub fn freeze(done: impl FnOnce(Result<Frozen, String>) + 'static) {
    let ticket = reply::expect(move |result: Result<Raw, String>| {
        done(result.map(|raw| {
            let main_height = main_screen_height();
            Frozen {
                displays: raw
                    .displays
                    .into_iter()
                    .filter_map(|d| {
                        Some(DisplayShot {
                            id: d.id,
                            frame: appkit_rect(d.frame, main_height),
                            shot: d.shot?,
                        })
                    })
                    .collect(),
                windows: raw
                    .windows
                    .into_iter()
                    .map(|w| WindowInfo {
                        id: w.id,
                        frame: appkit_rect(w.frame, main_height),
                        title: w.title,
                        app: w.app,
                    })
                    .collect(),
                content: raw.content,
            }
        }))
    });
    let ticket = Mutex::new(Some(ticket));
    let handler = RcBlock::new(
        move |content: *mut SCShareableContent, error: *mut NSError| {
            let Some(ticket) = ticket.lock().unwrap_or_else(PoisonError::into_inner).take() else {
                return;
            };
            // SAFETY: a non-null content pointer is a live snapshot for the duration of the call.
            match NonNull::new(content).map(|c| unsafe { Retained::retain(c.as_ptr()) }) {
                Some(Some(content)) => capture_displays(content, ticket),
                _ => ticket.send(Err(error_text(error))),
            }
        },
    );
    // SAFETY: the handler only touches Send data (the ticket) and is retained by ScreenCaptureKit.
    unsafe {
        SCShareableContent::getShareableContentExcludingDesktopWindows_onScreenWindowsOnly_completionHandler(
            false, true, &handler,
        );
    }
}

fn capture_displays(content: Retained<SCShareableContent>, ticket: Ticket<Result<Raw, String>>) {
    let own = own_applications(&content);
    let pid = std::process::id() as i32;
    // SAFETY: plain property reads on a live content snapshot.
    let (displays, windows) = unsafe {
        let displays: Vec<_> = content.displays().iter().collect();
        let windows: Vec<RawWindow> = content
            .windows()
            .iter()
            .filter(|w| {
                let frame = w.frame();
                w.isOnScreen()
                    && w.windowLayer() == 0
                    && frame.size.width >= MIN_WINDOW
                    && frame.size.height >= MIN_WINDOW
                    && w.owningApplication().is_none_or(|a| a.processID() != pid)
            })
            .map(|w| RawWindow {
                id: w.windowID(),
                frame: w.frame(),
                title: w.title().map(|t| t.to_string()).unwrap_or_default(),
                app: w
                    .owningApplication()
                    .map(|a| a.applicationName().to_string())
                    .unwrap_or_default(),
            })
            .collect();
        (displays, windows)
    };
    if displays.is_empty() {
        ticket.send(Err("no display to capture".into()));
        return;
    }
    let slots: Vec<RawDisplay> = displays
        .iter()
        // SAFETY: plain property reads.
        .map(|d| unsafe {
            RawDisplay {
                id: d.displayID(),
                frame: d.frame(),
                shot: None,
            }
        })
        .collect();
    let shared = Arc::new(Mutex::new((
        slots,
        displays.len(),
        Some(ticket),
        Some(windows),
        Some(Sendable(Retained::clone(&content))),
    )));
    for (index, display) in displays.iter().enumerate() {
        // SAFETY: a filter over a live display, leaving out this app; plain setters on a new configuration.
        let (filter, config, scale) = unsafe {
            let filter = SCContentFilter::initWithDisplay_excludingApplications_exceptingWindows(
                SCContentFilter::alloc(),
                display,
                &own,
                &NSArray::<SCWindow>::new(),
            );
            let scale = f64::from(filter.pointPixelScale()).max(1.0);
            let config = SCStreamConfiguration::new();
            config.setWidth((display.width() as f64 * scale).round() as usize);
            config.setHeight((display.height() as f64 * scale).round() as usize);
            config.setShowsCursor(false);
            config.setCaptureResolution(SCCaptureResolutionType::Best);
            (filter, config, scale)
        };
        let shared = Arc::clone(&shared);
        let block = RcBlock::new(move |image: *mut CGImage, error: *mut NSError| {
            let mut state = shared.lock().unwrap_or_else(PoisonError::into_inner);
            let (slots, remaining, ticket, windows, content) = &mut *state;
            match retain_image(image) {
                Some(image) => slots[index].shot = Some(Shot { image, scale }),
                None => eprintln!("capture: display {index}: {}", error_text(error)),
            }
            *remaining -= 1;
            if *remaining > 0 {
                return;
            }
            let (Some(ticket), Some(windows), Some(content)) =
                (ticket.take(), windows.take(), content.take())
            else {
                return;
            };
            let displays: Vec<RawDisplay> = std::mem::take(slots);
            if displays.iter().all(|d| d.shot.is_none()) {
                ticket.send(Err("no display could be captured".into()));
            } else {
                ticket.send(Ok(Raw {
                    displays,
                    windows,
                    content,
                }));
            }
        });
        // SAFETY: the filter and configuration outlive the call; the block only touches Send data.
        unsafe {
            SCScreenshotManager::captureImageWithFilter_configuration_completionHandler(
                &filter,
                &config,
                Some(&block),
            );
        }
    }
}

/// Captures one window of a frozen screen on its own, as it looks now: its whole content even where
/// other windows cover it, with its shadow on a transparent background.
pub fn capture_window(frozen: &Frozen, id: u32, done: impl FnOnce(Result<Shot, String>) + 'static) {
    // SAFETY: plain property reads on the retained snapshot.
    let window = unsafe {
        frozen
            .content
            .0
            .windows()
            .iter()
            .find(|w| w.windowID() == id)
    };
    let Some(window) = window else {
        done(Err("the window is gone".into()));
        return;
    };
    let ticket = reply::expect(done);
    let ticket = Mutex::new(Some(ticket));
    // SAFETY: a filter for one live window; plain setters on a new configuration.
    let (filter, config, scale) = unsafe {
        let filter =
            SCContentFilter::initWithDesktopIndependentWindow(SCContentFilter::alloc(), &window);
        let scale = f64::from(filter.pointPixelScale()).max(1.0);
        let rect = filter.contentRect();
        let config = SCStreamConfiguration::new();
        config.setWidth((rect.size.width * scale).round() as usize);
        config.setHeight((rect.size.height * scale).round() as usize);
        config.setShowsCursor(false);
        config.setIgnoreShadowsSingleWindow(false);
        config.setShouldBeOpaque(false);
        config.setCaptureResolution(SCCaptureResolutionType::Best);
        (filter, config, scale)
    };
    let block = RcBlock::new(move |image: *mut CGImage, error: *mut NSError| {
        if let Some(ticket) = ticket.lock().unwrap_or_else(PoisonError::into_inner).take() {
            ticket.send(
                retain_image(image)
                    .map(|image| Shot { image, scale })
                    .ok_or_else(|| error_text(error)),
            );
        }
    });
    // SAFETY: the filter and configuration outlive the call; the block only touches Send data.
    unsafe {
        SCScreenshotManager::captureImageWithFilter_configuration_completionHandler(
            &filter,
            &config,
            Some(&block),
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn core_graphics_rects_flip_to_appkit() {
        // A 1080-point-high main display; a window 100 points below its top edge.
        let cg = CGRect::new(CGPoint::new(50.0, 100.0), CGSize::new(300.0, 200.0));
        let ns = appkit_rect(cg, 1080.0);
        assert_eq!((ns.origin.x, ns.origin.y), (50.0, 780.0));
        assert_eq!((ns.size.width, ns.size.height), (300.0, 200.0));
    }
}
