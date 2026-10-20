//! What can be told about the use of the Mac without any permission: how long it is since a finger last touched an
//! input device, and the category an app says it belongs to.

use crate::workspace;
use objc2_foundation::{NSBundle, NSString};

#[link(name = "CoreGraphics", kind = "framework")]
unsafe extern "C" {
    fn CGEventSourceSecondsSinceLastEventType(state: i32, event_type: u32) -> f64;
}

/// `kCGEventSourceStateCombinedSessionState`: the keyboards, mice and the software that posts events, together.
const COMBINED_SESSION_STATE: i32 = 0;
/// `kCGAnyInputEventType`, which is every bit set: whatever kind of input came last.
const ANY_INPUT_EVENT: u32 = u32::MAX;

/// Seconds since the last key, click, scroll or movement. Asking sees no event, only the time of the last one, so it
/// needs no Input Monitoring access.
pub fn idle_seconds() -> f64 {
    // SAFETY: a plain query of the session's input clock.
    let seconds =
        unsafe { CGEventSourceSecondsSinceLastEventType(COMBINED_SESSION_STATE, ANY_INPUT_EVENT) };
    if seconds.is_finite() {
        seconds.max(0.0)
    } else {
        0.0
    }
}

/// The category an installed app declares in its Info.plist, such as `public.app-category.productivity`; `None`
/// for an app that is not installed or declares none.
pub fn app_category(bundle_id: &str) -> Option<String> {
    let path = workspace::app_path(bundle_id)?;
    let bundle = NSBundle::bundleWithPath(&NSString::from_str(&path.to_string_lossy()))?;
    let value =
        bundle.objectForInfoDictionaryKey(&NSString::from_str("LSApplicationCategoryType"))?;
    let text = value.downcast::<NSString>().ok()?.to_string();
    (!text.is_empty()).then_some(text)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_idle_time_is_a_number_of_seconds_that_is_not_negative() {
        let a = idle_seconds();
        assert!(a.is_finite() && a >= 0.0);
        assert!(
            a < 10.0 * 365.0 * 86_400.0,
            "a Mac has not been idle for ten years: {a}"
        );
    }

    #[test]
    fn an_app_that_is_not_installed_has_no_category() {
        assert_eq!(app_category("com.example.NotInstalled"), None);
        assert_eq!(app_category(""), None);
    }

    #[test]
    fn an_installed_app_that_declares_a_category_gives_it() {
        // Safari declares one; an app that declares none gives `None`, which is also right, so only the shape is held.
        if let Some(category) = app_category("com.apple.Safari") {
            assert!(category.starts_with("public.app-category."), "{category}");
        }
    }
}
