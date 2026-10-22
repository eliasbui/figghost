//! The brightness and contrast of external displays, set in the display itself over DDC/CI, the way its own
//! buttons set them. Apple silicon reaches the display's I2C bus through the `IOAVService` of each external
//! port, which IOKit exports but does not document; MonitorControl, BetterDisplay and m1ddc use the same calls.
//!
//! Every call waits on the display (a few tens of milliseconds), so none of this may run on the main thread or
//! inside an event tap. A display behind a hub, a DisplayLink adapter or some HDMI ports never answers; it is
//! simply not listed. An Intel Mac has no such service and lists nothing.

use core_foundation::{
    base::{CFRelease, CFType, CFTypeRef, TCFType},
    dictionary::CFDictionary,
    string::{CFString, CFStringRef},
};
use std::{
    ffi::{c_char, c_void},
    ptr,
    thread::sleep,
    time::Duration,
};

#[link(name = "IOKit", kind = "framework")]
unsafe extern "C" {
    fn IORegistryCreateIterator(
        main_port: u32,
        plane: *const c_char,
        options: u32,
        it: *mut u32,
    ) -> i32;
    fn IOIteratorNext(iterator: u32) -> u32;
    fn IOObjectRelease(object: u32) -> i32;
    fn IOObjectConformsTo(object: u32, class: *const c_char) -> u8;
    fn IORegistryEntryGetRegistryEntryID(entry: u32, id: *mut u64) -> i32;
    fn IORegistryEntryCreateCFProperty(
        entry: u32,
        key: CFStringRef,
        allocator: *const c_void,
        options: u32,
    ) -> CFTypeRef;
    fn IOAVServiceCreateWithService(allocator: *const c_void, service: u32) -> *const c_void;
    fn IOAVServiceReadI2C(
        service: *const c_void,
        chip: u32,
        offset: u32,
        buffer: *mut u8,
        size: u32,
    ) -> i32;
    fn IOAVServiceWriteI2C(
        service: *const c_void,
        chip: u32,
        offset: u32,
        buffer: *const u8,
        size: u32,
    ) -> i32;
}

/// A setting of the display, by its VCP code.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Control {
    Brightness,
    Contrast,
}

impl Control {
    const fn code(self) -> u8 {
        match self {
            Control::Brightness => 0x10,
            Control::Contrast => 0x12,
        }
    }
}

/// Where a setting stands and the most it goes to, as the display reports them.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Level {
    pub current: u16,
    pub max: u16,
}

impl Level {
    pub fn fraction(self) -> f64 {
        f64::from(self.current.min(self.max)) / f64::from(self.max.max(1))
    }

    /// The value that stands for a fraction, 0 to 1, of this setting's range.
    pub fn at(self, fraction: f64) -> u16 {
        (fraction.clamp(0.0, 1.0) * f64::from(self.max)).round() as u16
    }
}

// -- the messages ---------------------------------------------------------------------------------------------

/// The display listens at this I2C address; the host writes as this source.
const CHIP: u32 = 0x37;
const HOST: u8 = 0x51;
/// The address the display's checksum starts from (0x6E is the display, written to by the host).
const DISPLAY: u8 = 0x6E;
/// A reply's checksum starts from the virtual host address 0x50.
const REPLY_SEED: u8 = 0x50;
const REPLY_SIZE: usize = 11;

fn checksum(seed: u8, bytes: &[u8]) -> u8 {
    bytes.iter().fold(seed, |sum, b| sum ^ b)
}

/// Asks for the value of a setting: length, "get VCP feature", the code, checksum.
pub fn get_message(control: Control) -> [u8; 4] {
    let mut m = [0x82, 0x01, control.code(), 0];
    m[3] = checksum(DISPLAY ^ HOST, &m[..3]);
    m
}

/// Sets a setting: length, "set VCP feature", the code, the value high byte first, checksum.
pub fn set_message(control: Control, value: u16) -> [u8; 6] {
    let [high, low] = value.to_be_bytes();
    let mut m = [0x84, 0x03, control.code(), high, low, 0];
    m[5] = checksum(DISPLAY ^ HOST, &m[..5]);
    m
}

/// Reads the display's answer to `get_message`: it must be whole, for the same setting, and say it has one.
pub fn read_reply(reply: &[u8; REPLY_SIZE], control: Control) -> Option<Level> {
    let whole = checksum(REPLY_SEED, &reply[..REPLY_SIZE - 1]) == reply[REPLY_SIZE - 1];
    let ours = reply[1] == 0x88 && reply[2] == 0x02 && reply[3] == 0 && reply[4] == control.code();
    let max = u16::from_be_bytes([reply[6], reply[7]]);
    (whole && ours && max > 0).then(|| Level {
        current: u16::from_be_bytes([reply[8], reply[9]]),
        max,
    })
}

// -- the displays ---------------------------------------------------------------------------------------------

/// The pauses DDC/CI asks for: between two writes, and before the answer to a question can be read.
const WRITE_GAP: Duration = Duration::from_millis(10);
const ANSWER_WAIT: Duration = Duration::from_millis(50);
/// Some displays drop a message now and then, so a message is written twice and a question asked up to this often.
const TRIES: usize = 4;

/// An external display reached through its port's `IOAVService`.
pub struct Monitor {
    service: *const c_void,
    /// The registry's number for the port, which stays the same while the display stays plugged in.
    pub id: u64,
    /// The name the display gives itself, or empty when it gives none.
    pub name: String,
}

// SAFETY: the service is a CoreFoundation object, which may be used from any thread; the caller keeps the calls to
// one display on one thread at a time, which `&self` on a value that is only moved, never shared, ensures.
unsafe impl Send for Monitor {}

impl Drop for Monitor {
    fn drop(&mut self) {
        // SAFETY: created by IOAVServiceCreateWithService and released once.
        unsafe { CFRelease(self.service.cast()) };
    }
}

impl Monitor {
    fn write(&self, message: &[u8]) -> bool {
        let mut written = false;
        for _ in 0..2 {
            sleep(WRITE_GAP);
            // SAFETY: a live service and a buffer of the size given.
            written |= unsafe {
                IOAVServiceWriteI2C(
                    self.service,
                    CHIP,
                    u32::from(HOST),
                    message.as_ptr(),
                    message.len() as u32,
                )
            } == 0;
        }
        written
    }

    /// Asks the display where a setting stands. `None` when it does not answer or has no such setting.
    pub fn read(&self, control: Control) -> Option<Level> {
        let question = get_message(control);
        (0..TRIES).find_map(|_| {
            if !self.write(&question) {
                return None;
            }
            sleep(ANSWER_WAIT);
            let mut reply = [0u8; REPLY_SIZE];
            // SAFETY: a live service and a buffer of the size given.
            let status = unsafe {
                IOAVServiceReadI2C(
                    self.service,
                    CHIP,
                    u32::from(HOST),
                    reply.as_mut_ptr(),
                    REPLY_SIZE as u32,
                )
            };
            (status == 0).then(|| read_reply(&reply, control)).flatten()
        })
    }

    /// Sets a setting. The display does not confirm a set, so `true` only says the message went out.
    pub fn write_level(&self, control: Control, value: u16) -> bool {
        self.write(&set_message(control, value))
    }
}

fn property(entry: u32, key: &'static str) -> Option<CFType> {
    let key = CFString::from_static_string(key);
    // SAFETY: the create rule: the value is owned by the wrapper made below.
    let value = unsafe {
        IORegistryEntryCreateCFProperty(entry, key.as_concrete_TypeRef(), ptr::null(), 0)
    };
    // SAFETY: a non-null result of a Create function is owned by the caller.
    (!value.is_null()).then(|| unsafe { CFType::wrap_under_create_rule(value) })
}

/// The product name in a framebuffer's display attributes.
fn product_name(entry: u32) -> String {
    let find = |dictionary: &CFType, key: &'static str| -> Option<CFType> {
        let dictionary = dictionary.downcast::<CFDictionary>()?;
        // SAFETY: the registry's dictionaries are keyed by strings.
        let dictionary: CFDictionary<CFString, CFType> =
            unsafe { CFDictionary::wrap_under_get_rule(dictionary.as_concrete_TypeRef()) };
        dictionary
            .find(CFString::from_static_string(key))
            .map(|v| v.clone())
    };
    property(entry, "DisplayAttributes")
        .and_then(|attributes| find(&attributes, "ProductAttributes"))
        .and_then(|product| find(&product, "ProductName"))
        .and_then(|name| name.downcast::<CFString>())
        .map(|name| name.to_string().trim().to_owned())
        .unwrap_or_default()
}

/// The ports of the external displays, each with the name of its display. The registry is walked in order: each
/// display's framebuffer comes before the port that leads to it, which is how the port gets the display's name. A
/// port whose display does not answer is listed as well; `Monitor::read` tells.
pub fn monitors() -> Vec<Monitor> {
    let mut iterator = 0;
    // SAFETY: a valid out value; port 0 is the default main port and 1 walks the plane recursively.
    if unsafe { IORegistryCreateIterator(0, c"IOService".as_ptr(), 1, &mut iterator) } != 0 {
        return Vec::new();
    }
    let mut found = Vec::new();
    let mut name = String::new();
    loop {
        // SAFETY: the iterator just made.
        let entry = unsafe { IOIteratorNext(iterator) };
        if entry == 0 {
            break;
        }
        // SAFETY: a live entry and class names that are C strings.
        if unsafe { IOObjectConformsTo(entry, c"IOMobileFramebuffer".as_ptr()) } != 0 {
            name = product_name(entry);
        } else if unsafe { IOObjectConformsTo(entry, c"DCPAVServiceProxy".as_ptr()) } != 0
            && property(entry, "Location")
                .and_then(|l| l.downcast::<CFString>())
                .is_some_and(|l| l == "External")
        {
            let mut id = 0;
            // SAFETY: a live entry and a valid out value; a null service is checked.
            let service = unsafe {
                IORegistryEntryGetRegistryEntryID(entry, &mut id);
                IOAVServiceCreateWithService(ptr::null(), entry)
            };
            if !service.is_null() {
                found.push(Monitor {
                    service,
                    id,
                    name: name.clone(),
                });
            }
        }
        // SAFETY: returned by IOIteratorNext and released once.
        unsafe { IOObjectRelease(entry) };
    }
    // SAFETY: as above, for the iterator.
    unsafe { IOObjectRelease(iterator) };
    found
}

#[link(name = "CoreGraphics", kind = "framework")]
unsafe extern "C" {
    fn CGGetActiveDisplayList(max: u32, displays: *mut u32, count: *mut u32) -> i32;
    fn CGDisplayIsBuiltin(display: u32) -> u32;
}

/// Whether a built-in screen is lit, which the system's own brightness keys are for. Cheap enough for an event tap.
pub fn built_in_screen_on() -> bool {
    let mut displays = [0u32; 16];
    let mut count = 0;
    // SAFETY: the buffer holds as many displays as it says.
    if unsafe { CGGetActiveDisplayList(16, displays.as_mut_ptr(), &mut count) } != 0 {
        return false;
    }
    displays[..(count as usize).min(16)]
        .iter()
        // SAFETY: a display the system just listed.
        .any(|d| unsafe { CGDisplayIsBuiltin(*d) } != 0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_messages_carry_their_checksums() {
        assert_eq!(get_message(Control::Brightness), [0x82, 0x01, 0x10, 0xac]);
        assert_eq!(get_message(Control::Contrast), [0x82, 0x01, 0x12, 0xae]);
        assert_eq!(
            set_message(Control::Brightness, 80),
            [0x84, 0x03, 0x10, 0x00, 0x50, 0xf8]
        );
        assert_eq!(set_message(Control::Brightness, 0x0102)[3..5], [0x01, 0x02]);
    }

    #[test]
    fn a_whole_answer_for_the_setting_asked_is_read() {
        // What a Lenovo S24i-30 answers about its brightness (100 of 100) and contrast (75 of 100).
        let brightness = [
            0x6e, 0x88, 0x02, 0x00, 0x10, 0x00, 0x00, 0x64, 0x00, 0x64, 0xa4,
        ];
        let contrast = [
            0x6e, 0x88, 0x02, 0x00, 0x12, 0x00, 0x00, 0x64, 0x00, 0x4b, 0x89,
        ];
        assert_eq!(
            read_reply(&brightness, Control::Brightness),
            Some(Level {
                current: 100,
                max: 100
            })
        );
        assert_eq!(
            read_reply(&contrast, Control::Contrast).unwrap().current,
            75
        );
        assert_eq!(
            read_reply(&brightness, Control::Contrast),
            None,
            "another setting"
        );
        let mut torn = brightness;
        torn[9] = 0x63;
        assert_eq!(
            read_reply(&torn, Control::Brightness),
            None,
            "a bad checksum"
        );
        let mut unsupported = brightness;
        unsupported[3] = 0x01;
        unsupported[10] = checksum(REPLY_SEED, &unsupported[..10]);
        assert_eq!(read_reply(&unsupported, Control::Brightness), None);
        assert_eq!(read_reply(&[0; 11], Control::Brightness), None, "silence");
    }

    #[test]
    fn a_level_is_a_fraction_of_its_range_and_back() {
        let level = Level {
            current: 75,
            max: 100,
        };
        assert_eq!(level.fraction(), 0.75);
        assert_eq!(level.at(0.333), 33);
        assert_eq!(level.at(1.5), 100);
        assert_eq!(level.at(-1.0), 0);
        let odd = Level { current: 9, max: 0 };
        assert_eq!(odd.fraction(), 0.0, "never a division by zero");
    }
}
