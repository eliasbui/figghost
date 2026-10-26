use crate::{
    eventkit::{self, Access, Kind},
    screen_capture, workspace,
};
use core_foundation::{
    base::TCFType,
    boolean::CFBoolean,
    dictionary::{CFDictionary, CFDictionaryRef},
    string::CFString,
};
use objc2_av_foundation::{
    AVAuthorizationStatus, AVCaptureDevice, AVMediaTypeAudio, AVMediaTypeVideo,
};

pub const ACCESSIBILITY_PANE: &str =
    "x-apple.systempreferences:com.apple.preference.security?Privacy_Accessibility";
pub const SCREEN_RECORDING_PANE: &str =
    "x-apple.systempreferences:com.apple.preference.security?Privacy_ScreenCapture";
pub const MICROPHONE_PANE: &str =
    "x-apple.systempreferences:com.apple.preference.security?Privacy_Microphone";
pub const CAMERA_PANE: &str =
    "x-apple.systempreferences:com.apple.preference.security?Privacy_Camera";
pub const CALENDARS_PANE: &str =
    "x-apple.systempreferences:com.apple.preference.security?Privacy_Calendars";
pub const REMINDERS_PANE: &str =
    "x-apple.systempreferences:com.apple.preference.security?Privacy_Reminders";
pub const BLUETOOTH_PANE: &str =
    "x-apple.systempreferences:com.apple.preference.security?Privacy_Bluetooth";
pub const AUTOMATION_PANE: &str =
    "x-apple.systempreferences:com.apple.preference.security?Privacy_Automation";

#[link(name = "ApplicationServices", kind = "framework")]
unsafe extern "C" {
    fn AXIsProcessTrustedWithOptions(options: CFDictionaryRef) -> u8;
}

/// Whether this process may use Accessibility-gated APIs (active event taps, synthesized keystrokes).
/// With `prompt` set, macOS shows its own dialog that links to System Settings when access is missing.
pub fn accessibility_trusted(prompt: bool) -> bool {
    let options = CFDictionary::from_CFType_pairs(&[(
        CFString::from_static_string("AXTrustedCheckOptionPrompt"),
        CFBoolean::from(prompt),
    )]);
    // SAFETY: `options` is a valid CFDictionary that outlives the call.
    unsafe { AXIsProcessTrustedWithOptions(options.as_concrete_TypeRef()) != 0 }
}

/// Whether this app may record the screen.
pub fn screen_recording_trusted() -> bool {
    screen_capture::screen_access(false)
}

/// Asks macOS to show the screen recording permission prompt.
pub fn screen_recording_request() -> bool {
    screen_capture::screen_access(true)
}

/// Whether microphone access is granted.
pub fn microphone_trusted() -> bool {
    // SAFETY: AVMediaTypeAudio is a process-lifetime constant.
    let Some(audio) = (unsafe { AVMediaTypeAudio }) else {
        return false;
    };
    // SAFETY: class method taking valid media type.
    let status = unsafe { AVCaptureDevice::authorizationStatusForMediaType(audio) };
    status == AVAuthorizationStatus::Authorized
}

/// Prompts for microphone access if not determined.
pub fn microphone_request() {
    if let Some(audio) = unsafe { AVMediaTypeAudio } {
        let block = block2::RcBlock::new(|_: objc2::runtime::Bool| {});
        unsafe { AVCaptureDevice::requestAccessForMediaType_completionHandler(audio, &block) };
    }
}

/// Whether camera access is granted.
pub fn camera_trusted() -> bool {
    // SAFETY: AVMediaTypeVideo is a process-lifetime constant.
    let Some(video) = (unsafe { AVMediaTypeVideo }) else {
        return false;
    };
    // SAFETY: class method taking valid media type.
    let status = unsafe { AVCaptureDevice::authorizationStatusForMediaType(video) };
    status == AVAuthorizationStatus::Authorized
}

/// Prompts for camera access if not determined.
pub fn camera_request() {
    if let Some(video) = unsafe { AVMediaTypeVideo } {
        let block = block2::RcBlock::new(|_: objc2::runtime::Bool| {});
        unsafe { AVCaptureDevice::requestAccessForMediaType_completionHandler(video, &block) };
    }
}

/// Whether calendar access is granted.
pub fn calendar_trusted() -> bool {
    eventkit::access(Kind::Events) == Access::Granted
}

/// Prompts for calendar access.
pub fn calendar_request(done: impl FnOnce(bool) + 'static) {
    let store = eventkit::Store::new();
    let _ = store.request(Kind::Events, done);
}

/// Whether reminders access is granted.
pub fn reminders_trusted() -> bool {
    eventkit::access(Kind::Reminders) == Access::Granted
}

/// Prompts for reminders access.
pub fn reminders_request(done: impl FnOnce(bool) + 'static) {
    let store = eventkit::Store::new();
    let _ = store.request(Kind::Reminders, done);
}

/// Opens the specified privacy pane in System Settings.
pub fn open_pane(pane: &str) {
    workspace::open_url(pane);
}
