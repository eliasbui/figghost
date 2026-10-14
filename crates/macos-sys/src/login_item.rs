//! Start at login, through the app's own bundle (macOS 13 and later).

use objc2_service_management::{SMAppService, SMAppServiceStatus};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LoginItem {
    Enabled,
    Disabled,
    /// Registered, but the user still has to allow it in System Settings.
    NeedsApproval,
    /// Not running from an app bundle, so there is nothing to register.
    Unavailable,
}

pub fn status() -> LoginItem {
    // SAFETY: plain Objective-C calls with no arguments.
    let status = unsafe { SMAppService::mainAppService().status() };
    if status == SMAppServiceStatus::Enabled {
        LoginItem::Enabled
    } else if status == SMAppServiceStatus::RequiresApproval {
        LoginItem::NeedsApproval
    } else if status == SMAppServiceStatus::NotFound {
        LoginItem::Unavailable
    } else {
        LoginItem::Disabled
    }
}

/// Registers or unregisters the app; the error text is the system's own description.
pub fn set_enabled(enabled: bool) -> Result<(), String> {
    // SAFETY: plain Objective-C calls; the binding handles the error out parameter.
    let result = unsafe {
        let service = SMAppService::mainAppService();
        if enabled {
            service.registerAndReturnError()
        } else {
            service.unregisterAndReturnError()
        }
    };
    result.map_err(|e| e.localizedDescription().to_string())
}

pub fn open_settings() {
    // SAFETY: a class method with no arguments.
    unsafe { SMAppService::openSystemSettingsLoginItems() };
}
