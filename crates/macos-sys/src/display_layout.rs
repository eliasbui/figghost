//! Where the displays are and what each one does, as System Settings arranges them under Displays: the place of each
//! display in the desktop, which one is the main display (the one at the origin, with the menu bar and the Dock), and
//! which ones mirror another. A change goes through one display configuration and is kept for this user, as System
//! Settings keeps it.

use objc2::MainThreadMarker;
use objc2_app_kit::NSScreen;
use objc2_core_graphics::{
    CGBeginDisplayConfiguration, CGCancelDisplayConfiguration, CGCompleteDisplayConfiguration,
    CGConfigureDisplayMirrorOfDisplay, CGConfigureDisplayOrigin, CGConfigureOption,
    CGDisplayBounds, CGDisplayConfigRef, CGDisplayIsBuiltin, CGDisplayIsMain,
    CGDisplayMirrorsDisplay, CGError, CGGetOnlineDisplayList,
};
use objc2_foundation::{NSNumber, NSString};
use std::{cell::RefCell, collections::HashMap, ptr};

/// More displays than a Mac can drive.
const MAX_DISPLAYS: usize = 16;

/// One display that is connected.
#[derive(Debug, Clone, PartialEq)]
pub struct Placed {
    /// The `CGDirectDisplayID`.
    pub id: u32,
    /// The name the system shows; empty when it has none to give.
    pub name: String,
    /// Where it is in the desktop, in points, y downwards from the top-left of the main display: x, y, width, height.
    pub frame: (f64, f64, f64, f64),
    pub main: bool,
    pub built_in: bool,
    /// The display whose picture this one shows, while it mirrors another.
    pub mirror_of: Option<u32>,
}

thread_local! {
    /// The names seen so far: a display that mirrors another has no screen of its own to give its name.
    static NAMES: RefCell<HashMap<u32, String>> = RefCell::new(HashMap::new());
}

/// The names of the screens the system shows, by display.
fn screen_names(mtm: MainThreadMarker) -> HashMap<u32, String> {
    NSScreen::screens(mtm)
        .iter()
        .filter_map(|screen| {
            let number = screen
                .deviceDescription()
                .objectForKey(&NSString::from_str("NSScreenNumber"))?
                .downcast::<NSNumber>()
                .ok()?
                .unsignedIntValue();
            Some((number, screen.localizedName().to_string()))
        })
        .collect()
}

/// Every connected display, in the system's order.
pub fn displays(mtm: MainThreadMarker) -> Vec<Placed> {
    let mut ids = [0u32; MAX_DISPLAYS];
    let mut count = 0u32;
    // SAFETY: the list has room for `MAX_DISPLAYS` ids and the count is a valid pointer.
    let status =
        unsafe { CGGetOnlineDisplayList(MAX_DISPLAYS as u32, ids.as_mut_ptr(), &mut count) };
    if status != CGError::Success {
        return Vec::new();
    }
    let shown = screen_names(mtm);
    NAMES.with(|names| {
        let mut names = names.borrow_mut();
        for (id, name) in &shown {
            names.insert(*id, name.clone());
        }
        ids[..(count as usize).min(MAX_DISPLAYS)]
            .iter()
            .map(|&id| {
                let bounds = CGDisplayBounds(id);
                let mirror = CGDisplayMirrorsDisplay(id);
                Placed {
                    id,
                    name: names.get(&id).cloned().unwrap_or_default(),
                    frame: (
                        bounds.origin.x,
                        bounds.origin.y,
                        bounds.size.width,
                        bounds.size.height,
                    ),
                    main: CGDisplayIsMain(id),
                    built_in: CGDisplayIsBuiltin(id),
                    mirror_of: (mirror != 0).then_some(mirror),
                }
            })
            .collect()
    })
}

/// Runs one display configuration: `change` configures it, and it is completed for good, or cancelled when a step
/// fails. Returns the error the system gave, if any.
fn configure(change: impl FnOnce(CGDisplayConfigRef) -> CGError) -> Result<(), i32> {
    let mut config: CGDisplayConfigRef = ptr::null_mut();
    // SAFETY: `config` is a valid pointer to receive the configuration.
    let begun = unsafe { CGBeginDisplayConfiguration(&mut config) };
    if begun != CGError::Success || config.is_null() {
        return Err(begun.0);
    }
    let changed = change(config);
    if changed != CGError::Success {
        // SAFETY: the configuration begun above, not completed.
        unsafe { CGCancelDisplayConfiguration(config) };
        return Err(changed.0);
    }
    // SAFETY: the configuration begun above; completing it releases it.
    let done = unsafe { CGCompleteDisplayConfiguration(config, CGConfigureOption::Permanently) };
    if done == CGError::Success {
        Ok(())
    } else {
        Err(done.0)
    }
}

/// Puts the displays at these origins, in points of the desktop: the display at (0, 0) is the main display. The system
/// moves a display that would overlap another or float apart from the rest to the nearest place that touches them.
pub fn arrange(origins: &[(u32, i32, i32)]) -> Result<(), i32> {
    configure(|config| {
        for &(id, x, y) in origins {
            // SAFETY: a configuration being built; an id that is not a display is refused with an error.
            let status = unsafe { CGConfigureDisplayOrigin(config, id, x, y) };
            if status != CGError::Success {
                return status;
            }
        }
        CGError::Success
    })
}

/// Makes `display` show the picture of `of`, or show its own again with `None`.
pub fn mirror(display: u32, of: Option<u32>) -> Result<(), i32> {
    configure(|config| {
        // SAFETY: a configuration being built; 0 (kCGNullDirectDisplay) ends the mirroring.
        unsafe { CGConfigureDisplayMirrorOfDisplay(config, display, of.unwrap_or(0)) }
    })
}
