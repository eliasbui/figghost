//! Global hotkeys through Carbon's RegisterEventHotKey: no Accessibility or Input Monitoring access needed.

use objc2::MainThreadMarker;
use std::{
    cell::RefCell,
    collections::HashMap,
    ffi::c_void,
    mem::size_of,
    panic::{AssertUnwindSafe, catch_unwind},
    ptr,
    rc::Rc,
};

/// Carbon modifier bits for `Hotkey::register`.
pub mod modifier {
    pub const COMMAND: u32 = 1 << 8;
    pub const SHIFT: u32 = 1 << 9;
    pub const OPTION: u32 = 1 << 11;
    pub const CONTROL: u32 = 1 << 12;
}

const EVENT_CLASS_KEYBOARD: u32 = u32::from_be_bytes(*b"keyb");
const EVENT_HOTKEY_PRESSED: u32 = 5;
const PARAM_DIRECT_OBJECT: u32 = u32::from_be_bytes(*b"----");
const TYPE_HOTKEY_ID: u32 = u32::from_be_bytes(*b"hkid");
const SIGNATURE: u32 = u32::from_be_bytes(*b"FGhs");

#[repr(C)]
struct EventTypeSpec {
    event_class: u32,
    event_kind: u32,
}

#[repr(C)]
#[derive(Clone, Copy)]
struct EventHotKeyId {
    signature: u32,
    id: u32,
}

type Handler = unsafe extern "C" fn(*mut c_void, *mut c_void, *mut c_void) -> i32;

#[link(name = "Carbon", kind = "framework")]
unsafe extern "C" {
    fn GetApplicationEventTarget() -> *mut c_void;
    fn InstallEventHandler(
        target: *mut c_void,
        handler: Handler,
        num_types: u64,
        types: *const EventTypeSpec,
        user_data: *mut c_void,
        out_ref: *mut *mut c_void,
    ) -> i32;
    fn RegisterEventHotKey(
        key_code: u32,
        modifiers: u32,
        id: EventHotKeyId,
        target: *mut c_void,
        options: u32,
        out_ref: *mut *mut c_void,
    ) -> i32;
    fn UnregisterEventHotKey(hotkey: *mut c_void) -> i32;
    fn GetEventParameter(
        event: *mut c_void,
        name: u32,
        desired_type: u32,
        actual_type: *mut u32,
        buffer_size: u64,
        actual_size: *mut u64,
        out_data: *mut c_void,
    ) -> i32;
}

thread_local! {
    static CALLBACKS: RefCell<HashMap<u32, Rc<dyn Fn()>>> = RefCell::new(HashMap::new());
    static NEXT_ID: RefCell<u32> = const { RefCell::new(1) };
    static HANDLER_INSTALLED: RefCell<bool> = const { RefCell::new(false) };
}

unsafe extern "C" fn on_hotkey(_next: *mut c_void, event: *mut c_void, _user: *mut c_void) -> i32 {
    let mut id = EventHotKeyId {
        signature: 0,
        id: 0,
    };
    // SAFETY: `event` is the live event Carbon handed to the handler and `id` matches the requested type.
    let status = unsafe {
        GetEventParameter(
            event,
            PARAM_DIRECT_OBJECT,
            TYPE_HOTKEY_ID,
            ptr::null_mut(),
            size_of::<EventHotKeyId>() as u64,
            ptr::null_mut(),
            ptr::from_mut(&mut id).cast(),
        )
    };
    if status == 0 && id.signature == SIGNATURE {
        // Clone out of the map first: the callback may register or drop hotkeys.
        let callback = CALLBACKS.with(|c| c.borrow().get(&id.id).cloned());
        if let Some(callback) = callback {
            // Unwinding through Carbon frames is undefined behaviour.
            let _ = catch_unwind(AssertUnwindSafe(|| callback()));
        }
    }
    0
}

/// A registered global hotkey; unregistered on drop.
pub struct Hotkey {
    reference: *mut c_void,
    id: u32,
}

impl Hotkey {
    /// `key_code` is a virtual key code (kVK_*); `modifiers` is a combination of `modifier::*`.
    /// The callback runs on the main thread. Fails with the OSStatus when the combination is taken.
    pub fn register(
        _mtm: MainThreadMarker,
        key_code: u32,
        modifiers: u32,
        on_press: impl Fn() + 'static,
    ) -> Result<Self, i32> {
        if !HANDLER_INSTALLED.with(|h| *h.borrow()) {
            let spec = EventTypeSpec {
                event_class: EVENT_CLASS_KEYBOARD,
                event_kind: EVENT_HOTKEY_PRESSED,
            };
            // SAFETY: `spec` outlives the call; the handler is a plain function that lives for the process.
            let status = unsafe {
                InstallEventHandler(
                    GetApplicationEventTarget(),
                    on_hotkey,
                    1,
                    &spec,
                    ptr::null_mut(),
                    ptr::null_mut(),
                )
            };
            if status != 0 {
                return Err(status);
            }
            HANDLER_INSTALLED.with(|h| *h.borrow_mut() = true);
        }
        let id = NEXT_ID.with(|n| {
            let mut n = n.borrow_mut();
            *n += 1;
            *n - 1
        });
        let mut reference = ptr::null_mut();
        // SAFETY: arguments are plain values and `reference` is a valid out pointer.
        let status = unsafe {
            RegisterEventHotKey(
                key_code,
                modifiers,
                EventHotKeyId {
                    signature: SIGNATURE,
                    id,
                },
                GetApplicationEventTarget(),
                0,
                &mut reference,
            )
        };
        if status != 0 {
            return Err(status);
        }
        CALLBACKS.with(|c| c.borrow_mut().insert(id, Rc::new(on_press)));
        Ok(Self { reference, id })
    }
}

impl Drop for Hotkey {
    fn drop(&mut self) {
        // SAFETY: `reference` came from RegisterEventHotKey and is released exactly once.
        unsafe { UnregisterEventHotKey(self.reference) };
        CALLBACKS.with(|c| c.borrow_mut().remove(&self.id));
    }
}
