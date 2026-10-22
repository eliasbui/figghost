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
const AX_VALUE_RECT: i32 = 3;
const AX_VALUE_RANGE: i32 = 4;

#[repr(C)]
#[derive(Clone, Copy, Default)]
struct Pair {
    first: f64,
    second: f64,
}

/// The layout of CFRange: where the range starts and how long it is, in UTF-16 units.
#[repr(C)]
#[derive(Clone, Copy, Default)]
struct Span {
    location: isize,
    length: isize,
}

/// The layout of CGRect.
#[repr(C)]
#[derive(Clone, Copy, Default)]
struct Quad {
    x: f64,
    y: f64,
    w: f64,
    h: f64,
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
    fn AXUIElementCopyParameterizedAttributeValue(
        element: AXUIElementRef,
        attribute: CFStringRef,
        parameter: CFTypeRef,
        value: *mut CFTypeRef,
    ) -> i32;
    fn AXUIElementGetPid(element: AXUIElementRef, pid: *mut i32) -> i32;
    fn AXUIElementPerformAction(element: AXUIElementRef, action: CFStringRef) -> i32;
    // Not documented, but every window manager uses it: the number the window server gave the window.
    fn _AXUIElementGetWindow(element: AXUIElementRef, window: *mut u32) -> i32;
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

    /// The number the window server gives the window this element is, which is how the system's list of windows names
    /// it. `None` for an element that is not a window.
    pub fn window_id(&self) -> Option<u32> {
        let mut id = 0;
        // SAFETY: `self.0` is a live element and `id` is a valid out pointer.
        (unsafe { _AXUIElementGetWindow(self.0, &mut id) } == 0 && id != 0).then_some(id)
    }

    /// Asks the element to do `action`, such as `AXRaise`. Whether the app then does it is up to the app.
    pub fn perform(&self, action: &str) -> bool {
        // SAFETY: element and action are live; the action name is a valid CFString.
        unsafe { AXUIElementPerformAction(self.0, name(action).as_concrete_TypeRef()) == 0 }
    }

    /// Sets a boolean attribute such as `AXFrontmost`.
    pub fn set_flag(&self, attribute: &str, on: bool) -> bool {
        let value = CFBoolean::from(on);
        // SAFETY: element and value are live; the attribute name is a valid CFString.
        unsafe {
            AXUIElementSetAttributeValue(
                self.0,
                name(attribute).as_concrete_TypeRef(),
                value.as_concrete_TypeRef().cast(),
            ) == 0
        }
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

    /// A range attribute such as `AXSelectedTextRange`, as (start, length) in UTF-16 units.
    pub fn range(&self, attribute: &str) -> Option<(usize, usize)> {
        let value = Element::own(self.copy(attribute)?)?;
        let mut out = Span::default();
        // SAFETY: `out` matches the layout of CFRange.
        let ok =
            unsafe { AXValueGetValue(value.0, AX_VALUE_RANGE, ptr::from_mut(&mut out).cast()) };
        if !ok {
            return None;
        }
        Some((
            usize::try_from(out.location).ok()?,
            usize::try_from(out.length).ok()?,
        ))
    }

    pub fn set_range(&self, attribute: &str, (start, length): (usize, usize)) -> bool {
        let span = Span {
            location: start as isize,
            length: length as isize,
        };
        // SAFETY: `span` matches the layout of CFRange and outlives the call.
        let Some(value) =
            Element::own(unsafe { AXValueCreate(AX_VALUE_RANGE, ptr::from_ref(&span).cast()) })
        else {
            return false;
        };
        // SAFETY: element and value are live; the attribute name is a valid CFString.
        unsafe {
            AXUIElementSetAttributeValue(self.0, name(attribute).as_concrete_TypeRef(), value.0)
                == 0
        }
    }

    /// Sets a string attribute such as `AXValue`. Whether the app then shows it is up to the app.
    pub fn set_string(&self, attribute: &str, text: &str) -> bool {
        let value = CFString::new(text);
        // SAFETY: element and value are live; the attribute name is a valid CFString.
        unsafe {
            AXUIElementSetAttributeValue(
                self.0,
                name(attribute).as_concrete_TypeRef(),
                value.as_concrete_TypeRef().cast(),
            ) == 0
        }
    }

    pub fn point(&self, attribute: &str) -> Option<(f64, f64)> {
        self.pair(attribute, AX_VALUE_POINT)
    }

    /// The rect a parameterized attribute such as `AXBoundsForRange` gives for `parameter` (a value read from
    /// this element, such as its `AXSelectedTextRange`), as (x, y, width, height) in the coordinates of
    /// `AXPosition`.
    pub fn rect_for(&self, attribute: &str, parameter: &Element) -> Option<(f64, f64, f64, f64)> {
        let mut value: CFTypeRef = ptr::null();
        // SAFETY: element and parameter are live, the name is a valid CFString and `value` a valid out pointer.
        let status = unsafe {
            AXUIElementCopyParameterizedAttributeValue(
                self.0,
                name(attribute).as_concrete_TypeRef(),
                parameter.0,
                &mut value,
            )
        };
        let value = if status == 0 {
            Element::own(value)?
        } else {
            return None;
        };
        let mut out = Quad::default();
        // SAFETY: `out` matches the layout of CGRect.
        let ok = unsafe { AXValueGetValue(value.0, AX_VALUE_RECT, ptr::from_mut(&mut out).cast()) };
        ok.then_some((out.x, out.y, out.w, out.h))
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
