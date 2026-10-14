use core_foundation::{
    base::TCFType,
    boolean::CFBoolean,
    dictionary::{CFDictionary, CFDictionaryRef},
    string::CFString,
};

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
