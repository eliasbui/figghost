//! The heat of the processor and the speed of the fans, for the System tab. Both are read without a permission
//! and without root: the temperatures from the sensors the system publishes as HID services (Apple silicon), the
//! fans from the System Management Controller, which any process may read but only root may write. This module
//! only reads.
//!
//! Neither is a public interface. The calls are the ones Activity Monitor's peers (Stats, iStat Menus, Macs Fan
//! Control) have used for years; a Mac where they answer nothing simply shows neither figure.

use core_foundation::{
    array::{CFArray, CFArrayRef},
    base::{CFRelease, CFType, CFTypeRef, TCFType},
    dictionary::{CFDictionary, CFDictionaryRef},
    number::CFNumber,
    string::{CFString, CFStringRef},
};
use std::{
    cell::RefCell,
    ffi::{c_char, c_void},
    mem::size_of,
    ptr,
};

#[link(name = "IOKit", kind = "framework")]
unsafe extern "C" {
    fn IOServiceMatching(name: *const c_char) -> CFDictionaryRef;
    fn IOServiceGetMatchingService(main_port: u32, matching: CFDictionaryRef) -> u32;
    fn IOServiceOpen(service: u32, task: u32, kind: u32, connection: *mut u32) -> i32;
    fn IOObjectRelease(object: u32) -> i32;
    fn IOConnectCallStructMethod(
        connection: u32,
        selector: u32,
        input: *const c_void,
        input_size: usize,
        output: *mut c_void,
        output_size: *mut usize,
    ) -> i32;
    fn IOHIDEventSystemClientCreate(allocator: *const c_void) -> *const c_void;
    fn IOHIDEventSystemClientSetMatching(client: *const c_void, matching: CFDictionaryRef) -> i32;
    fn IOHIDEventSystemClientCopyServices(client: *const c_void) -> CFArrayRef;
    fn IOHIDServiceClientCopyProperty(service: *const c_void, key: CFStringRef) -> CFTypeRef;
    fn IOHIDServiceClientCopyEvent(
        service: *const c_void,
        kind: i64,
        options: i32,
        timestamp: i64,
    ) -> *const c_void;
    fn IOHIDEventGetFloatValue(event: *const c_void, field: i32) -> f64;
}

unsafe extern "C" {
    static mach_task_self_: u32;
}

// -- the processor's heat -------------------------------------------------------------------------------------

/// The page and usage of the temperature sensors among the HID services, and the event that carries a reading.
const SENSOR_PAGE: i32 = 0xff00;
const SENSOR_USAGE: i32 = 5;
const TEMPERATURE_EVENT: i64 = 15;

/// The sensors on the processor's die are named "PMU tdie1", "PMU tdie2" and so on; "PMU2" is the second power
/// controller, which sits beside the processor and runs far cooler.
fn on_the_die(name: &str) -> bool {
    name.strip_prefix("PMU tdie")
        .is_some_and(|n| !n.is_empty() && n.bytes().all(|b| b.is_ascii_digit()))
}

/// The mean of the readings from the die, in degrees Celsius. A sensor that reads nothing or something no chip
/// survives is left out.
pub fn die_mean<'a>(readings: impl IntoIterator<Item = (&'a str, f64)>) -> Option<f64> {
    let (sum, count) = readings
        .into_iter()
        .filter(|(name, celsius)| on_the_die(name) && (1.0..150.0).contains(celsius))
        .fold((0.0, 0u32), |(sum, count), (_, c)| (sum + c, count + 1));
    (count > 0).then(|| sum / f64::from(count))
}

struct HidClient(*const c_void);

impl Drop for HidClient {
    fn drop(&mut self) {
        // SAFETY: the client came from a Create function and is released once.
        unsafe { CFRelease(self.0.cast()) };
    }
}

thread_local! {
    /// Making the client walks the HID services, so one is kept for the thread that reads.
    static CLIENT: RefCell<Option<HidClient>> = const { RefCell::new(None) };
}

fn hid_client() -> Option<*const c_void> {
    CLIENT.with(|slot| {
        let mut slot = slot.borrow_mut();
        if slot.is_none() {
            // SAFETY: the default allocator; a null result is checked.
            let client = unsafe { IOHIDEventSystemClientCreate(ptr::null()) };
            if client.is_null() {
                return None;
            }
            let matching = CFDictionary::from_CFType_pairs(&[
                (
                    CFString::from_static_string("PrimaryUsagePage").as_CFType(),
                    CFNumber::from(SENSOR_PAGE).as_CFType(),
                ),
                (
                    CFString::from_static_string("PrimaryUsage").as_CFType(),
                    CFNumber::from(SENSOR_USAGE).as_CFType(),
                ),
            ]);
            // SAFETY: a live client and a dictionary the call copies.
            unsafe { IOHIDEventSystemClientSetMatching(client, matching.as_concrete_TypeRef()) };
            *slot = Some(HidClient(client));
        }
        slot.as_ref().map(|c| c.0)
    })
}

/// How hot the processor's die is, in degrees Celsius. `None` on a Mac that publishes no such sensors (an Intel
/// one).
pub fn processor_celsius() -> Option<f64> {
    let client = hid_client()?;
    // SAFETY: a live client; the array follows the create rule.
    let services = unsafe { IOHIDEventSystemClientCopyServices(client) };
    if services.is_null() {
        return None;
    }
    // SAFETY: a non-null array returned by a Copy function is owned here.
    let services: CFArray<*const c_void> = unsafe { CFArray::wrap_under_create_rule(services) };
    let product = CFString::from_static_string("Product");
    let mut readings = Vec::new();
    for service in services.iter() {
        let service = *service;
        // SAFETY: a service of the array, alive while the array is; the property follows the create rule.
        let name =
            unsafe { IOHIDServiceClientCopyProperty(service, product.as_concrete_TypeRef()) };
        if name.is_null() {
            continue;
        }
        // SAFETY: a non-null result of a Copy function is owned by the wrapper.
        let Some(name) = unsafe { CFType::wrap_under_create_rule(name) }.downcast::<CFString>()
        else {
            continue;
        };
        let name = name.to_string();
        if !on_the_die(&name) {
            continue;
        }
        // SAFETY: as above; the event follows the create rule and is released after its value is read.
        let event = unsafe { IOHIDServiceClientCopyEvent(service, TEMPERATURE_EVENT, 0, 0) };
        if event.is_null() {
            continue;
        }
        // SAFETY: a live temperature event; its field is the event type shifted into the high half.
        let celsius = unsafe { IOHIDEventGetFloatValue(event, (TEMPERATURE_EVENT as i32) << 16) };
        // SAFETY: owned since the Copy above, released once.
        unsafe { CFRelease(event.cast()) };
        readings.push((name, celsius));
    }
    die_mean(readings.iter().map(|(n, c)| (n.as_str(), *c)))
}

// -- the fans ---------------------------------------------------------------------------------------------

/// The message the SMC's user client takes and gives back, laid out as its driver expects (80 bytes).
#[repr(C)]
#[derive(Default, Clone, Copy)]
struct SmcMessage {
    key: u32,
    /// The version (six bytes) and the padding after it.
    version: [u8; 8],
    limits: [u32; 4],
    size: u32,
    kind: u32,
    attributes: u8,
    padding: [u8; 3],
    result: u8,
    status: u8,
    command: u8,
    index: u32,
    bytes: [u8; 32],
}

const SMC_CALL: u32 = 2;
const READ_BYTES: u8 = 5;
const READ_INFO: u8 = 9;

fn four(code: &[u8; 4]) -> u32 {
    u32::from_be_bytes(*code)
}

/// A value of the SMC as a number, from its type and bytes: Apple silicon gives a float in the machine's order, an
/// Intel Mac a fixed point number with two fraction bits, most significant byte first.
pub fn smc_number(kind: [u8; 4], bytes: &[u8]) -> Option<f64> {
    match &kind {
        b"flt " => Some(f64::from(f32::from_le_bytes(
            bytes.get(..4)?.try_into().ok()?,
        ))),
        b"fpe2" => Some(f64::from(u16::from_be_bytes(bytes.get(..2)?.try_into().ok()?)) / 4.0),
        b"ui8 " => bytes.first().map(|b| f64::from(*b)),
        _ => None,
    }
    .filter(|n| n.is_finite())
}

struct Smc(u32);

impl Drop for Smc {
    fn drop(&mut self) {
        // SAFETY: the connection was opened by IOServiceOpen and is closed once (a connection is a port).
        unsafe { IOObjectRelease(self.0) };
    }
}

impl Smc {
    fn open() -> Option<Self> {
        // SAFETY: IOServiceMatching returns a dictionary that the call consumes; port 0 is the default main port.
        let service =
            unsafe { IOServiceGetMatchingService(0, IOServiceMatching(c"AppleSMC".as_ptr())) };
        if service == 0 {
            return None;
        }
        let mut connection = 0;
        // SAFETY: a live service and a valid out value; the service is released after the open either way.
        let status = unsafe { IOServiceOpen(service, mach_task_self_, 0, &mut connection) };
        // SAFETY: as above.
        unsafe { IOObjectRelease(service) };
        (status == 0 && connection != 0).then_some(Self(connection))
    }

    fn call(&self, message: &SmcMessage) -> Option<SmcMessage> {
        let mut answer = SmcMessage::default();
        let mut size = size_of::<SmcMessage>();
        // SAFETY: both structures are the size given and live for the call.
        let status = unsafe {
            IOConnectCallStructMethod(
                self.0,
                SMC_CALL,
                ptr::from_ref(message).cast(),
                size_of::<SmcMessage>(),
                ptr::from_mut(&mut answer).cast(),
                &mut size,
            )
        };
        (status == 0 && answer.result == 0).then_some(answer)
    }

    fn number(&self, key: &[u8; 4]) -> Option<f64> {
        let info = self.call(&SmcMessage {
            key: four(key),
            command: READ_INFO,
            ..SmcMessage::default()
        })?;
        let value = self.call(&SmcMessage {
            key: four(key),
            command: READ_BYTES,
            size: info.size,
            ..SmcMessage::default()
        })?;
        let size = usize::try_from(info.size).ok()?.min(value.bytes.len());
        smc_number(info.kind.to_be_bytes(), &value.bytes[..size])
    }
}

thread_local! {
    static SMC: RefCell<Option<Smc>> = const { RefCell::new(None) };
}

/// One fan: how fast it turns now and the range it turns in, in revolutions a minute.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Fan {
    pub rpm: f64,
    pub min: f64,
    pub max: f64,
}

/// The fans of this Mac, none for one that has none (a MacBook Air) or whose controller does not answer.
pub fn fans() -> Vec<Fan> {
    SMC.with(|slot| {
        let mut slot = slot.borrow_mut();
        if slot.is_none() {
            *slot = Smc::open();
        }
        let Some(smc) = slot.as_ref() else {
            return Vec::new();
        };
        let count = smc.number(b"FNum").unwrap_or(0.0).clamp(0.0, 10.0) as u8;
        (0..count)
            .filter_map(|i| {
                let read = |a: u8, b: u8| smc.number(&[b'F', b'0' + i, a, b]);
                Some(Fan {
                    rpm: read(b'A', b'c')?.max(0.0),
                    min: read(b'M', b'n').unwrap_or(0.0).max(0.0),
                    max: read(b'M', b'x')?.max(1.0),
                })
            })
            .collect()
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_message_is_the_size_the_driver_takes() {
        use std::mem::offset_of;
        assert_eq!(size_of::<SmcMessage>(), 80);
        assert_eq!(offset_of!(SmcMessage, limits), 12);
        assert_eq!(offset_of!(SmcMessage, size), 28);
        assert_eq!(offset_of!(SmcMessage, result), 40);
        assert_eq!(offset_of!(SmcMessage, command), 42);
        assert_eq!(offset_of!(SmcMessage, index), 44);
        assert_eq!(offset_of!(SmcMessage, bytes), 48);
    }

    #[test]
    fn only_the_sensors_on_the_die_count_and_nonsense_is_left_out() {
        let readings = [
            ("PMU tdie1", 60.0),
            ("PMU tdie12", 70.0),
            ("PMU2 tdie1", 40.0),
            ("PMU tdev1", 90.0),
            ("PMU tdie", 99.0),
            ("PMU tdie3", 0.0),
            ("PMU tdie4", f64::NAN),
            ("NAND CH0 temp", 50.0),
        ];
        assert_eq!(die_mean(readings), Some(65.0));
        assert_eq!(die_mean([("PMU2 tdie1", 40.0)]), None);
    }

    #[test]
    fn smc_numbers_are_read_in_each_type() {
        // 1432 rpm as Apple silicon gives it, and 2000 rpm as an Intel Mac gives it.
        assert_eq!(smc_number(*b"flt ", &[0, 0, 179, 68]), Some(1432.0));
        assert_eq!(smc_number(*b"fpe2", &[0x1f, 0x40]), Some(2000.0));
        assert_eq!(smc_number(*b"ui8 ", &[2]), Some(2.0));
        assert_eq!(smc_number(*b"flt ", &[0, 0]), None, "too short");
        assert_eq!(
            smc_number(*b"flt ", &[0, 0, 0xc0, 0x7f]),
            None,
            "not a number"
        );
        assert_eq!(
            smc_number(*b"si16", &[0, 1]),
            None,
            "a type that is not read"
        );
    }

    #[test]
    fn this_mac_answers_with_figures_that_make_sense() {
        if let Some(celsius) = processor_celsius() {
            assert!((1.0..150.0).contains(&celsius), "{celsius}");
        }
        for fan in fans() {
            assert!(fan.min <= fan.max && fan.max < 20_000.0, "{fan:?}");
            assert!(fan.rpm >= 0.0 && fan.rpm < 20_000.0, "{fan:?}");
        }
    }
}
