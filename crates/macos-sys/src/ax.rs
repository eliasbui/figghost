//! Accessibility API helpers: owned UI elements and typed attribute access.

use core_foundation::{
    base::{CFGetTypeID, CFTypeRef, TCFType},
    boolean::CFBoolean,
    string::{CFString, CFStringRef},
};
use std::{ffi::c_void, ptr};

type AXUIElementRef = *const c_void;
type AXValueRef = *const c_void;

const AX_VALUE_POINT: i32 = 1;
const AX_VALUE_SIZE: i32 = 2;

#[repr(C)]
#[derive(Clone, Copy, Default)]
struct Pair {
    first: f64,
    second: f64,
}

#[link(name = "ApplicationServices", kind = "framework")]
unsafe extern "C" {
    fn AXUIElementCreateSystemWide() -> AXUIElementRef;
    fn AXUIElementCreateApplication(pid: i32) -> AXUIElementRef;
    fn AXUIElementCopyAttributeValue(
        element: AXUIElementRef,
        attribute: CFStringRef,
        value: *mut CFTypeRef,
    ) -> i32;
    fn AXUIElementSetAttributeValue(
        element: AXUIElementRef,
        attribute: CFStringRef,
        value: CFTypeRef,
    ) -> i32;
    fn AXUIElementGetPid(element: AXUIElementRef, pid: *mut i32) -> i32;
    fn AXUIElementCopyElementAtPosition(
        application: AXUIElementRef,
        x: f32,
        y: f32,
        element: *mut AXUIElementRef,
    ) -> i32;
    fn AXUIElementSetMessagingTimeout(element: AXUIElementRef, seconds: f32) -> i32;
    fn AXValueCreate(kind: i32, value: *const c_void) -> AXValueRef;
    fn AXValueGetValue(value: AXValueRef, kind: i32, out: *mut c_void) -> bool;
}

#[link(name = "CoreFoundation", kind = "framework")]
unsafe extern "C" {
    fn CFRelease(cf: *const c_void);
    fn CFRetain(cf: *const c_void) -> *const c_void;
    fn CFArrayGetCount(array: *const c_void) -> isize;
    fn CFArrayGetValueAtIndex(array: *const c_void, index: isize) -> *const c_void;
}

/// An owned Accessibility object (an element, or a value read from one); released on drop.
pub struct Element(CFTypeRef);

impl Drop for Element {
    fn drop(&mut self) {
        // SAFETY: the pointer is a +1 reference owned by this wrapper.
        unsafe { CFRelease(self.0) };
    }
}

fn name(attribute: &str) -> CFString {
    CFString::new(attribute)
}

impl Element {
    fn own(raw: CFTypeRef) -> Option<Self> {
        (!raw.is_null()).then_some(Self(raw))
    }

    /// Another owner of the same object.
    pub fn duplicate(&self) -> Element {
        // SAFETY: `self.0` is live; the +1 from the retain is owned by the new wrapper.
        Element(unsafe { CFRetain(self.0) })
    }

    pub fn system_wide() -> Option<Self> {
        // SAFETY: returns a new reference or null.
        Self::own(unsafe { AXUIElementCreateSystemWide() })
    }

    pub fn application(pid: i32) -> Option<Self> {
        // SAFETY: returns a new reference or null.
        Self::own(unsafe { AXUIElementCreateApplication(pid) })
    }

    /// The deepest element at a point of the screen, in the same coordinates as `AXPosition`. `timeout` (seconds)
    /// applies to every Accessibility request this process makes from now on, because the system-wide element
    /// is where the system keeps it.
    pub fn at_position(x: f64, y: f64, timeout: f32) -> Option<Element> {
        let system = Element::system_wide()?;
        system.set_timeout(timeout);
        let mut found: AXUIElementRef = ptr::null();
        // SAFETY: `system` is live and `found` is a valid out pointer; on success it holds a +1 reference.
        let status =
            unsafe { AXUIElementCopyElementAtPosition(system.0, x as f32, y as f32, &mut found) };
        if status == 0 {
            Element::own(found)
        } else {
            None
        }
    }

    /// How long a request to the app that owns this element may take before it fails. The default is
    /// seconds, which would freeze this app's main thread behind a hung one. On the system-wide element
    /// it applies to every request this process makes.
    pub fn set_timeout(&self, seconds: f32) {
        // SAFETY: `self.0` is a live element.
        unsafe { AXUIElementSetMessagingTimeout(self.0, seconds) };
    }

    fn copy(&self, attribute: &str) -> Option<CFTypeRef> {
        let mut value: CFTypeRef = ptr::null();
        // SAFETY: `self.0` is a live element and `value` is a valid out pointer.
        let status = unsafe {
            AXUIElementCopyAttributeValue(self.0, name(attribute).as_concrete_TypeRef(), &mut value)
        };
        (status == 0 && !value.is_null()).then_some(value)
    }

    /// An attribute whose value is another element or an opaque value.
    pub fn attribute(&self, attribute: &str) -> Option<Element> {
        self.copy(attribute).and_then(Element::own)
    }

    /// The elements of an array-valued attribute such as `AXWindows`.
    pub fn elements(&self, attribute: &str) -> Vec<Element> {
        let Some(array) = self.copy(attribute) else {
            return Vec::new();
        };
        // SAFETY: `array` is an owned CFArray; each retained item outlives the array release below.
        let items = unsafe {
            (0..CFArrayGetCount(array))
                .filter_map(|i| Element::own(CFRetain(CFArrayGetValueAtIndex(array, i))))
                .collect()
        };
        // SAFETY: balances the +1 from the copy above.
        unsafe { CFRelease(array) };
        items
    }

    pub fn pid(&self) -> Option<i32> {
        let mut pid = 0;
        // SAFETY: `self.0` is a live element and `pid` is a valid out pointer.
        (unsafe { AXUIElementGetPid(self.0, &mut pid) } == 0).then_some(pid)
    }

    pub fn flag(&self, attribute: &str) -> Option<bool> {
        let value = Element::own(self.copy(attribute)?)?;
        // SAFETY: only reading the type id of a live object.
        (unsafe { CFGetTypeID(value.0) } == CFBoolean::type_id())
            // SAFETY: the type was checked above; the get rule leaves ownership with `value`.
            .then(|| bool::from(unsafe { CFBoolean::wrap_under_get_rule(value.0.cast()) }))
    }

    /// A string attribute such as `AXSelectedText`.
    pub fn string(&self, attribute: &str) -> Option<String> {
        let value = Element::own(self.copy(attribute)?)?;
        // SAFETY: only reading the type id of a live object.
        (unsafe { CFGetTypeID(value.0) } == CFString::type_id())
            // SAFETY: the type was checked above; the get rule leaves ownership with `value`.
            .then(|| unsafe { CFString::wrap_under_get_rule(value.0.cast()) }.to_string())
    }

    fn pair(&self, attribute: &str, kind: i32) -> Option<(f64, f64)> {
        let value = Element::own(self.copy(attribute)?)?;
        let mut out = Pair::default();
        // SAFETY: `out` matches the layout of CGPoint and CGSize.
        let ok = unsafe { AXValueGetValue(value.0, kind, ptr::from_mut(&mut out).cast()) };
        ok.then_some((out.first, out.second))
    }

    fn set_pair(&self, attribute: &str, kind: i32, (first, second): (f64, f64)) -> bool {
        let pair = Pair { first, second };
        // SAFETY: `pair` matches the layout of CGPoint and CGSize and outlives the call.
        let Some(value) = Element::own(unsafe { AXValueCreate(kind, ptr::from_ref(&pair).cast()) })
        else {
            return false;
        };
        // SAFETY: element and value are live; the attribute name is a valid CFString.
        unsafe {
            AXUIElementSetAttributeValue(self.0, name(attribute).as_concrete_TypeRef(), value.0)
                == 0
        }
    }

    pub fn point(&self, attribute: &str) -> Option<(f64, f64)> {
        self.pair(attribute, AX_VALUE_POINT)
    }

    pub fn size(&self, attribute: &str) -> Option<(f64, f64)> {
        self.pair(attribute, AX_VALUE_SIZE)
    }

    pub fn set_point(&self, attribute: &str, point: (f64, f64)) -> bool {
        self.set_pair(attribute, AX_VALUE_POINT, point)
    }

    pub fn set_size(&self, attribute: &str, size: (f64, f64)) -> bool {
        self.set_pair(attribute, AX_VALUE_SIZE, size)
    }
}

/// Process that owns the UI element with keyboard focus, or `None` when nothing is focused or the
/// query fails. While the popup is key this is our own process; once the previous app has taken the
/// keyboard back it is that app.
pub fn focused_pid() -> Option<i32> {
    Element::system_wide()?
        .attribute("AXFocusedUIElement")?
        .pid()
}
