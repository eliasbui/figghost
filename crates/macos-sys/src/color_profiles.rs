//! The colour profiles of the displays, as System Settings offers them under Displays, Colour profile: the display's
//! own profile and the RGB display profiles macOS comes with. Choosing one sets it for the display the way System
//! Settings does, for this user, and the picture changes at once; choosing the display's own takes the choice away.

use core_foundation::{
    base::{CFType, CFTypeRef, TCFType},
    boolean::CFBoolean,
    dictionary::{CFDictionary, CFDictionaryRef},
    string::{CFString, CFStringRef},
    url::{CFURL, CFURLRef},
    uuid::{CFUUID, CFUUIDRef},
};
use objc2::MainThreadMarker;
use objc2_app_kit::NSScreen;
use objc2_foundation::{NSNumber, NSString};
use std::{
    ffi::c_void,
    fs::{self, File},
    io::Read,
    path::{Path, PathBuf},
    ptr,
};

/// Where macOS keeps the profiles it comes with.
const SYSTEM_PROFILES: &str = "/System/Library/ColorSync/Profiles";

#[link(name = "ColorSync", kind = "framework")]
unsafe extern "C" {
    static kColorSyncDisplayDeviceClass: CFStringRef;
    static kColorSyncDeviceClass: CFStringRef;
    static kColorSyncDeviceID: CFStringRef;
    static kColorSyncDeviceDefaultProfileID: CFStringRef;
    static kColorSyncDeviceProfileURL: CFStringRef;
    static kColorSyncDeviceProfileIsFactory: CFStringRef;
    static kColorSyncDeviceProfileIsDefault: CFStringRef;
    static kColorSyncDeviceProfileIsCurrent: CFStringRef;
    fn ColorSyncProfileCreateWithURL(url: CFURLRef, error: *mut CFTypeRef) -> CFTypeRef;
    fn ColorSyncProfileCopyDescriptionString(profile: CFTypeRef) -> CFStringRef;
    fn ColorSyncIterateDeviceProfiles(
        callback: extern "C" fn(CFDictionaryRef, *mut c_void) -> bool,
        user_info: *mut c_void,
    );
    fn ColorSyncDeviceSetCustomProfiles(
        device_class: CFStringRef,
        device_id: CFUUIDRef,
        profile_info: CFDictionaryRef,
    ) -> bool;
    fn CGDisplayCreateUUIDFromDisplayID(display: u32) -> CFUUIDRef;
}

#[link(name = "CoreFoundation", kind = "framework")]
unsafe extern "C" {
    static kCFNull: CFTypeRef;
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Profile {
    /// The name the profile gives itself, such as "Display P3".
    pub name: String,
    pub path: PathBuf,
}

/// A display the system shows, with the profiles it can take.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScreenProfiles {
    /// The `CGDirectDisplayID`.
    pub display: u32,
    pub name: String,
    /// The display's own profile first, then the ones macOS comes with; one chosen from elsewhere is last.
    pub profiles: Vec<Profile>,
    /// The one in use, by its place in `profiles`.
    pub current: Option<usize>,
    /// The number of profiles at the start of `profiles` that are the display's own: choosing one of them takes the
    /// custom choice away instead of setting it.
    pub own: usize,
}

/// Whether the 128-byte header of an ICC file is that of an RGB display profile, the kind a display can take.
pub fn is_display_rgb(header: &[u8]) -> bool {
    header.len() >= 20 && &header[12..16] == b"mntr" && &header[16..20] == b"RGB "
}

fn header_of(path: &Path) -> Option<[u8; 20]> {
    let mut header = [0u8; 20];
    File::open(path).ok()?.read_exact(&mut header).ok()?;
    Some(header)
}

/// The name a profile gives itself, in the user's language when it has one.
fn description(path: &Path) -> Option<String> {
    let url = CFURL::from_path(path, false)?;
    // SAFETY: a valid URL; a null error pointer is allowed. The profile is released once.
    unsafe {
        let profile = ColorSyncProfileCreateWithURL(url.as_concrete_TypeRef(), ptr::null_mut());
        if profile.is_null() {
            return None;
        }
        let profile = CFType::wrap_under_create_rule(profile);
        let name = ColorSyncProfileCopyDescriptionString(profile.as_CFTypeRef());
        (!name.is_null()).then(|| CFString::wrap_under_create_rule(name).to_string())
    }
}

/// The RGB display profiles macOS comes with, by name.
pub fn system_profiles() -> Vec<Profile> {
    let Ok(entries) = fs::read_dir(SYSTEM_PROFILES) else {
        return Vec::new();
    };
    let mut profiles: Vec<Profile> = entries
        .flatten()
        .map(|entry| entry.path())
        .filter(|path| header_of(path).is_some_and(|h| is_display_rgb(&h)))
        .filter_map(|path| {
            Some(Profile {
                name: description(&path)?,
                path,
            })
        })
        .collect();
    profiles.sort_by_key(|p| p.name.to_lowercase());
    profiles
}

/// A profile ColorSync has for a display: its own (factory) one, or one chosen for it.
struct DeviceProfile {
    device: CFUUID,
    path: PathBuf,
    factory: bool,
    default: bool,
    current: bool,
}

extern "C" fn collect(info: CFDictionaryRef, found: *mut c_void) -> bool {
    // SAFETY: ColorSync passes a valid dictionary for the length of the call, and `found` is the vector
    // `device_profiles` handed over, alive for the whole iteration.
    let (info, found) = unsafe {
        (
            CFDictionary::<CFString, CFType>::wrap_under_get_rule(info),
            &mut *found.cast::<Vec<DeviceProfile>>(),
        )
    };
    // SAFETY: the keys are constants of the framework.
    let key = |k: CFStringRef| {
        info.find(unsafe { CFString::wrap_under_get_rule(k) })
            .map(|v| v.clone())
    };
    let flag = |k| {
        key(k)
            .and_then(|v| v.downcast::<CFBoolean>())
            .is_some_and(bool::from)
    };
    // SAFETY: as above.
    let display = unsafe { CFString::wrap_under_get_rule(kColorSyncDisplayDeviceClass) };
    if key(unsafe { kColorSyncDeviceClass }).and_then(|v| v.downcast::<CFString>()) != Some(display)
    {
        return true;
    }
    let device = key(unsafe { kColorSyncDeviceID }).and_then(|v| v.downcast::<CFUUID>());
    let path = key(unsafe { kColorSyncDeviceProfileURL })
        .and_then(|v| v.downcast::<CFURL>())
        .and_then(|url| url.to_path());
    if let (Some(device), Some(path)) = (device, path) {
        found.push(DeviceProfile {
            device,
            path,
            factory: flag(unsafe { kColorSyncDeviceProfileIsFactory }),
            default: flag(unsafe { kColorSyncDeviceProfileIsDefault }),
            current: flag(unsafe { kColorSyncDeviceProfileIsCurrent }),
        });
    }
    true
}

fn device_profiles() -> Vec<DeviceProfile> {
    let mut found: Vec<DeviceProfile> = Vec::new();
    // SAFETY: `collect` only uses the pointer during this call.
    unsafe { ColorSyncIterateDeviceProfiles(collect, (&raw mut found).cast()) };
    found
}

fn display_uuid(display: u32) -> Option<CFUUID> {
    // SAFETY: any number may be asked about; a display that does not exist gives null.
    let uuid = unsafe { CGDisplayCreateUUIDFromDisplayID(display) };
    // SAFETY: a CFUUID made for the caller.
    (!uuid.is_null()).then(|| unsafe { CFUUID::wrap_under_create_rule(uuid) })
}

/// The displays the system shows, in its order, each with the profiles it can take and the one it uses.
pub fn screens(mtm: MainThreadMarker) -> Vec<ScreenProfiles> {
    let system = system_profiles();
    let devices = device_profiles();
    NSScreen::screens(mtm)
        .iter()
        .filter_map(|screen| {
            let number = screen
                .deviceDescription()
                .objectForKey(&NSString::from_str("NSScreenNumber"))?
                .downcast::<NSNumber>()
                .ok()?
                .unsignedIntValue();
            let uuid = display_uuid(number)?;
            let mine: Vec<&DeviceProfile> = devices.iter().filter(|d| d.device == uuid).collect();
            let name = screen.localizedName().to_string();
            Some(choices(number, name, &mine, &system))
        })
        .collect()
}

/// The profiles of one display: its own one first, then the system's, then the one in use when it is neither.
fn choices(
    display: u32,
    name: String,
    mine: &[&DeviceProfile],
    system: &[Profile],
) -> ScreenProfiles {
    let own = mine
        .iter()
        .find(|d| d.factory && d.default)
        .or_else(|| mine.iter().find(|d| d.factory))
        .map(|d| Profile {
            name: description(&d.path).unwrap_or_else(|| name.clone()),
            path: d.path.clone(),
        });
    let mut profiles: Vec<Profile> = own.into_iter().collect();
    let own = profiles.len();
    for profile in system {
        if profiles.iter().all(|p| p.path != profile.path) {
            profiles.push(profile.clone());
        }
    }
    let in_use = mine.iter().find(|d| d.current).map(|d| d.path.clone());
    let current = in_use.map(|path| match profiles.iter().position(|p| p.path == path) {
        Some(at) => at,
        None => {
            profiles.push(Profile {
                name: description(&path).unwrap_or_else(|| path.display().to_string()),
                path,
            });
            profiles.len() - 1
        }
    });
    ScreenProfiles {
        display,
        name,
        profiles,
        current,
        own,
    }
}

/// Sets the profile of a display, for this user, as System Settings does; `None` goes back to the display's own.
/// False when ColorSync refuses or the display is gone.
pub fn set(display: u32, profile: Option<&Path>) -> bool {
    let Some(uuid) = display_uuid(display) else {
        return false;
    };
    let value = match profile {
        Some(path) => match CFURL::from_path(path, false) {
            Some(url) => url.as_CFType(),
            None => return false,
        },
        // SAFETY: the framework's null constant, which takes a custom choice away.
        None => unsafe { CFType::wrap_under_get_rule(kCFNull) },
    };
    // SAFETY: the framework's constants, a CFUUID and a dictionary that live through the call.
    unsafe {
        let key = CFString::wrap_under_get_rule(kColorSyncDeviceDefaultProfileID);
        let info = CFDictionary::from_CFType_pairs(&[(key, value)]);
        ColorSyncDeviceSetCustomProfiles(
            kColorSyncDisplayDeviceClass,
            uuid.as_concrete_TypeRef(),
            info.as_concrete_TypeRef(),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn header(class: &[u8; 4], space: &[u8; 4]) -> Vec<u8> {
        let mut header = vec![0u8; 128];
        header[12..16].copy_from_slice(class);
        header[16..20].copy_from_slice(space);
        header
    }

    #[test]
    fn only_rgb_display_profiles_are_offered() {
        assert!(is_display_rgb(&header(b"mntr", b"RGB ")));
        assert!(
            !is_display_rgb(&header(b"mntr", b"GRAY")),
            "a grey display profile"
        );
        assert!(
            !is_display_rgb(&header(b"abst", b"Lab ")),
            "an effect such as Sepia Tone"
        );
        assert!(!is_display_rgb(&header(b"prtr", b"CMYK")), "a printer's");
        assert!(!is_display_rgb(&[0u8; 12]), "a file too short to be one");
    }

    #[test]
    fn the_profiles_macos_comes_with_are_found_by_name() {
        let names: Vec<String> = system_profiles().into_iter().map(|p| p.name).collect();
        assert!(names.iter().any(|n| n.contains("sRGB")), "{names:?}");
        assert!(names.iter().any(|n| n == "Display P3"), "{names:?}");
        assert!(
            !names
                .iter()
                .any(|n| n.contains("Gray") || n.contains("CMYK")),
            "{names:?}"
        );
    }
}
