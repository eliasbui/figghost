//! The windows on screen, from the front to the back.

use core_capture::{Display, Rect, WindowBox, flip_y, primary_height};
use objc2::runtime::AnyObject;
use objc2_core_graphics::{CGWindowListCopyWindowInfo, CGWindowListOption};
use objc2_foundation::{NSArray, NSDictionary, NSNumber, NSString};

/// Windows smaller than this on a side are not worth offering (shadows, tool tips, status item buttons).
const MIN_SIDE: f64 = 24.0;

fn number(dict: &NSDictionary<NSString, AnyObject>, key: &str) -> Option<f64> {
    dict.objectForKey(&NSString::from_str(key))?
        .downcast_ref::<NSNumber>()
        .map(NSNumber::doubleValue)
}

fn text(dict: &NSDictionary<NSString, AnyObject>, key: &str) -> String {
    dict.objectForKey(&NSString::from_str(key))
        .and_then(|v| v.downcast_ref::<NSString>().map(ToString::to_string))
        .unwrap_or_default()
}

/// The ordinary windows (layer 0, visible, on a display) of every app, front to back, with frames in AppKit's space.
/// Names need the Screen Recording permission; without it the titles are empty and the list still works.
pub fn list(displays: &[Display]) -> Vec<WindowBox> {
    let Some(info) = CGWindowListCopyWindowInfo(
        CGWindowListOption::OptionOnScreenOnly | CGWindowListOption::ExcludeDesktopElements,
        0,
    ) else {
        return Vec::new();
    };
    let height = primary_height(displays);
    // SAFETY: a CFArray of CFDictionary is toll-free bridged to NSArray of NSDictionary, and the array is
    // alive for as long as `info`.
    let array: &NSArray<NSDictionary<NSString, AnyObject>> =
        unsafe { &*std::ptr::from_ref(&*info).cast() };
    array
        .iter()
        .filter_map(|dict| window_of(&dict, height))
        // Apps keep windows outside every display (a strip above the screen, for one): nobody can click them.
        .filter(|w| {
            displays
                .iter()
                .any(|d| w.frame.intersection(&d.frame).is_some())
        })
        .collect()
}

fn window_of(dict: &NSDictionary<NSString, AnyObject>, primary_height: f64) -> Option<WindowBox> {
    if number(dict, "kCGWindowLayer")? != 0.0
        || number(dict, "kCGWindowAlpha").unwrap_or(1.0) < 0.05
    {
        return None;
    }
    let id = number(dict, "kCGWindowNumber")? as u32;
    let value = dict.objectForKey(&NSString::from_str("kCGWindowBounds"))?;
    let bounds = value.downcast_ref::<NSDictionary>()?;
    // SAFETY: the keys of a window's bounds are strings; the key type is only a promise to the compiler.
    let bounds: &NSDictionary<NSString, AnyObject> = unsafe { &*std::ptr::from_ref(bounds).cast() };
    let frame = Rect::new(
        number(bounds, "X")?,
        number(bounds, "Y")?,
        number(bounds, "Width")?,
        number(bounds, "Height")?,
    );
    if frame.w < MIN_SIDE || frame.h < MIN_SIDE {
        return None;
    }
    Some(WindowBox {
        id,
        // The bounds are measured down from the top of the primary display.
        frame: flip_y(frame, primary_height),
        owner: text(dict, "kCGWindowOwnerName"),
        title: text(dict, "kCGWindowName"),
    })
}
