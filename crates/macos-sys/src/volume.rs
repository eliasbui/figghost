//! Output volume and mute of the default output device, through CoreAudio. Every call is safe from any
//! thread, so the event tap can use them directly.

use std::{ffi::c_void, mem::size_of, ptr};

type ObjectId = u32;

#[repr(C)]
struct Address {
    selector: u32,
    scope: u32,
    element: u32,
}

const SYSTEM_OBJECT: ObjectId = 1;
const DEFAULT_OUTPUT_DEVICE: u32 = u32::from_be_bytes(*b"dOut");
const SCOPE_GLOBAL: u32 = u32::from_be_bytes(*b"glob");
const SCOPE_OUTPUT: u32 = u32::from_be_bytes(*b"outp");
const ELEMENT_MAIN: u32 = 0;
/// The device's overall volume, whether it has one control or one per channel.
const VIRTUAL_MAIN_VOLUME: u32 = u32::from_be_bytes(*b"vmvc");
const MUTE: u32 = u32::from_be_bytes(*b"mute");

#[link(name = "CoreAudio", kind = "framework")]
unsafe extern "C" {
    fn AudioObjectGetPropertyData(
        object: ObjectId,
        address: *const Address,
        qualifier_size: u32,
        qualifier: *const c_void,
        size: *mut u32,
        data: *mut c_void,
    ) -> i32;
    fn AudioObjectSetPropertyData(
        object: ObjectId,
        address: *const Address,
        qualifier_size: u32,
        qualifier: *const c_void,
        size: u32,
        data: *const c_void,
    ) -> i32;
    fn AudioObjectIsPropertySettable(
        object: ObjectId,
        address: *const Address,
        settable: *mut u8,
    ) -> i32;
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Volume {
    /// 0.0 to 1.0.
    pub level: f32,
    pub muted: bool,
}

const fn output(selector: u32) -> Address {
    Address {
        selector,
        scope: SCOPE_OUTPUT,
        element: ELEMENT_MAIN,
    }
}

fn get<T: Copy + Default>(object: ObjectId, address: &Address) -> Option<T> {
    let mut value = T::default();
    let mut size = size_of::<T>() as u32;
    // SAFETY: `value` outlives the call and `size` is exactly its size in bytes.
    let status = unsafe {
        AudioObjectGetPropertyData(
            object,
            address,
            0,
            ptr::null(),
            &mut size,
            (&raw mut value).cast(),
        )
    };
    (status == 0).then_some(value)
}

fn set<T: Copy>(object: ObjectId, address: &Address, value: T) -> bool {
    // SAFETY: `value` outlives the call and the size passed is exactly its size in bytes.
    let status = unsafe {
        AudioObjectSetPropertyData(
            object,
            address,
            0,
            ptr::null(),
            size_of::<T>() as u32,
            (&raw const value).cast(),
        )
    };
    status == 0
}

fn settable(object: ObjectId, address: &Address) -> bool {
    let mut flag = 0u8;
    // SAFETY: `flag` is a valid byte for the Boolean result.
    let status = unsafe { AudioObjectIsPropertySettable(object, address, &mut flag) };
    status == 0 && flag != 0
}

fn default_device() -> Option<ObjectId> {
    let address = Address {
        selector: DEFAULT_OUTPUT_DEVICE,
        scope: SCOPE_GLOBAL,
        element: ELEMENT_MAIN,
    };
    get::<u32>(SYSTEM_OBJECT, &address).filter(|&device| device != 0)
}

/// The current state, or `None` when the output device has no volume this app may change (HDMI and
/// DisplayPort speakers, for one). Callers leave the key to the system in that case.
pub fn read() -> Option<Volume> {
    let device = default_device()?;
    let volume = output(VIRTUAL_MAIN_VOLUME);
    if !settable(device, &volume) {
        return None;
    }
    let level = get::<f32>(device, &volume)?.clamp(0.0, 1.0);
    let muted = get::<u32>(device, &output(MUTE)).is_some_and(|m| m != 0);
    Some(Volume { level, muted })
}

pub fn set_level(level: f32) -> bool {
    default_device()
        .is_some_and(|device| set(device, &output(VIRTUAL_MAIN_VOLUME), level.clamp(0.0, 1.0)))
}

/// False when the device has no mute control.
pub fn set_muted(muted: bool) -> bool {
    let address = output(MUTE);
    default_device()
        .is_some_and(|device| settable(device, &address) && set(device, &address, u32::from(muted)))
}

/// Where the level goes when a volume key is pressed: to the next mark in that direction on a scale of
/// 16 (64 for the fine step, Shift+Option), the way macOS moves it.
pub fn step_level(level: f32, up: bool, fine: bool) -> f32 {
    const EDGE: f32 = 1e-3;
    let marks = if fine { 64.0 } else { 16.0 };
    let at = level.clamp(0.0, 1.0) * marks;
    let target = if up {
        (at + EDGE).floor() + 1.0
    } else {
        (at - EDGE).ceil() - 1.0
    };
    (target / marks).clamp(0.0, 1.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn close(a: f32, b: f32) -> bool {
        (a - b).abs() < 1e-6
    }

    #[test]
    fn steps_land_on_the_next_mark_in_the_direction_pressed() {
        assert!(close(step_level(0.5, true, false), 0.5625));
        assert!(close(step_level(0.5, false, false), 0.4375));
        // Between marks, the first press only reaches the nearest mark in that direction.
        assert!(close(step_level(0.54, true, false), 0.5625));
        assert!(close(step_level(0.54, false, false), 0.5));
    }

    #[test]
    fn steps_stop_at_the_ends() {
        assert!(close(step_level(1.0, true, false), 1.0));
        assert!(close(step_level(0.0, false, false), 0.0));
        assert!(close(step_level(0.0, true, false), 0.0625));
        assert!(close(step_level(0.97, true, false), 1.0));
        assert!(close(step_level(0.03, false, false), 0.0));
    }

    #[test]
    fn fine_steps_are_a_quarter_of_the_normal_ones() {
        assert!(close(step_level(0.5, true, true), 0.515625));
        assert!(close(step_level(0.5, false, true), 0.484375));
    }

    #[test]
    fn sixteen_presses_cross_the_whole_range_without_drifting() {
        let mut level = 0.0;
        for _ in 0..16 {
            level = step_level(level, true, false);
        }
        assert!(close(level, 1.0));
        for _ in 0..16 {
            level = step_level(level, false, false);
        }
        assert!(close(level, 0.0));
    }

    #[test]
    fn out_of_range_levels_are_clamped() {
        assert!(close(step_level(-0.3, true, false), 0.0625));
        assert!(close(step_level(1.7, false, false), 0.9375));
    }
}
