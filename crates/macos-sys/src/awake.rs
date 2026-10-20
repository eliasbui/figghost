//! Keeping this Mac awake: a power assertion of IOKit, the thing `caffeinate` makes. It lasts as long as the
//! value does, and the system lets it go by itself if the app quits.

use core_foundation::{
    base::TCFType,
    string::{CFString, CFStringRef},
};

#[link(name = "IOKit", kind = "framework")]
unsafe extern "C" {
    fn IOPMAssertionCreateWithName(
        assertion_type: CFStringRef,
        level: u32,
        name: CFStringRef,
        id: *mut u32,
    ) -> i32;
    fn IOPMAssertionRelease(id: u32) -> i32;
}

/// `kIOPMAssertionLevelOn`.
const LEVEL_ON: u32 = 255;

pub struct Awake {
    ids: Vec<u32>,
}

impl Awake {
    /// Stops the Mac from sleeping on its own while this value lives; with `display`, the screen stays lit too.
    /// `None` when the system refused.
    pub fn start(display: bool, reason: &str) -> Option<Self> {
        let name = CFString::new(reason);
        let kinds: &[&'static str] = if display {
            &["PreventUserIdleDisplaySleep", "PreventUserIdleSystemSleep"]
        } else {
            &["PreventUserIdleSystemSleep"]
        };
        let mut ids = Vec::new();
        for kind in kinds {
            let kind = CFString::from_static_string(kind);
            let mut id = 0u32;
            // SAFETY: both strings outlive the call, which copies them, and `id` is a valid out value.
            let status = unsafe {
                IOPMAssertionCreateWithName(
                    kind.as_concrete_TypeRef(),
                    LEVEL_ON,
                    name.as_concrete_TypeRef(),
                    &mut id,
                )
            };
            if status == 0 {
                ids.push(id);
            }
        }
        (!ids.is_empty()).then_some(Self { ids })
    }
}

impl Drop for Awake {
    fn drop(&mut self) {
        for id in &self.ids {
            // SAFETY: each id was returned by IOPMAssertionCreateWithName and is released once.
            unsafe { IOPMAssertionRelease(*id) };
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_assertion_can_be_taken_and_let_go() {
        let awake = Awake::start(true, "Figghost test");
        assert!(awake.is_some(), "the system should grant the assertion");
        drop(awake);
    }
}
