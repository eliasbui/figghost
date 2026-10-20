//! Whether an app is using a camera right now, from CoreMediaIO: the same fact the green dot of the menu bar
//! shows. It says nothing about which app.

use std::{ffi::c_void, mem::size_of, ptr};

type ObjectId = u32;

#[repr(C)]
struct Address {
    selector: u32,
    scope: u32,
    element: u32,
}

const SYSTEM_OBJECT: ObjectId = 1;
const DEVICES: u32 = u32::from_be_bytes(*b"dev#");
const RUNNING_SOMEWHERE: u32 = u32::from_be_bytes(*b"gone");
const SCOPE_GLOBAL: u32 = u32::from_be_bytes(*b"glob");
const ELEMENT_MAIN: u32 = 0;

#[link(name = "CoreMediaIO", kind = "framework")]
unsafe extern "C" {
    fn CMIOObjectGetPropertyDataSize(
        object: ObjectId,
        address: *const Address,
        qualifier_size: u32,
        qualifier: *const c_void,
        size: *mut u32,
    ) -> i32;
    fn CMIOObjectGetPropertyData(
        object: ObjectId,
        address: *const Address,
        qualifier_size: u32,
        qualifier: *const c_void,
        size: u32,
        used: *mut u32,
        data: *mut c_void,
    ) -> i32;
}

const fn global(selector: u32) -> Address {
    Address {
        selector,
        scope: SCOPE_GLOBAL,
        element: ELEMENT_MAIN,
    }
}

fn devices() -> Vec<ObjectId> {
    let address = global(DEVICES);
    let mut size = 0u32;
    // SAFETY: `size` is a valid out value for the size of the list.
    let status = unsafe {
        CMIOObjectGetPropertyDataSize(SYSTEM_OBJECT, &address, 0, ptr::null(), &mut size)
    };
    if status != 0 || size == 0 {
        return Vec::new();
    }
    let mut ids = vec![0 as ObjectId; size as usize / size_of::<ObjectId>()];
    let mut used = 0u32;
    // SAFETY: `ids` has room for exactly `size` bytes, which is what the call may write.
    let status = unsafe {
        CMIOObjectGetPropertyData(
            SYSTEM_OBJECT,
            &address,
            0,
            ptr::null(),
            size,
            &mut used,
            ids.as_mut_ptr().cast(),
        )
    };
    if status != 0 {
        return Vec::new();
    }
    ids.truncate(used as usize / size_of::<ObjectId>());
    ids
}

fn running(device: ObjectId) -> bool {
    let address = global(RUNNING_SOMEWHERE);
    let mut value = 0u32;
    let mut used = 0u32;
    // SAFETY: `value` is a four byte out value, as the size passed says.
    let status = unsafe {
        CMIOObjectGetPropertyData(
            device,
            &address,
            0,
            ptr::null(),
            size_of::<u32>() as u32,
            &mut used,
            (&raw mut value).cast(),
        )
    };
    status == 0 && value != 0
}

/// True while any camera, built in or plugged in, is being read by an app.
pub fn in_use() -> bool {
    devices().into_iter().any(running)
}

#[cfg(test)]
mod tests {
    #[test]
    fn asking_never_fails_whether_or_not_there_is_a_camera() {
        let _ = super::in_use();
    }
}
