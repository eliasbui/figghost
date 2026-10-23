//! What the Mac is doing, read from the kernel and the I/O registry for the System tab: the processor core by core,
//! the memory and the swap, the network, the GPU, the startup disk, the thermal state and the health of the
//! battery. Every reading is a cheap call that can be repeated once a second, and none of them needs a permission.
//! The ones that are counters (processor ticks, network bytes) only mean something as the difference of two
//! readings, which is for the caller to take.

use core_foundation::{
    base::{CFType, CFTypeRef, TCFType},
    dictionary::{CFDictionary, CFDictionaryRef},
    number::CFNumber,
    string::{CFString, CFStringRef},
};
use objc2_foundation::{
    NSArray, NSProcessInfo, NSProcessInfoThermalState, NSString, NSURL,
    NSURLVolumeAvailableCapacityForImportantUsageKey, NSURLVolumeTotalCapacityKey,
};
use std::{
    ffi::{CStr, CString, c_char},
    mem::{size_of, zeroed},
    ptr,
};

#[link(name = "IOKit", kind = "framework")]
unsafe extern "C" {
    fn IOServiceMatching(name: *const c_char) -> CFDictionaryRef;
    fn IOServiceGetMatchingServices(
        main_port: u32,
        matching: CFDictionaryRef,
        existing: *mut u32,
    ) -> i32;
    fn IOIteratorNext(iterator: u32) -> u32;
    fn IORegistryEntryCreateCFProperty(
        entry: u32,
        key: CFStringRef,
        allocator: *const std::ffi::c_void,
        options: u32,
    ) -> CFTypeRef;
    fn IOObjectRelease(object: u32) -> i32;
}

unsafe extern "C" {
    fn mach_host_self() -> libc::mach_port_t;
    fn mach_timebase_info(info: *mut Timebase) -> i32;
    /// The port of this task, which `mach_task_self()` is a macro for. The `libc` crate marks its own declarations
    /// of both as deprecated in favour of a crate this one has no other use for.
    static mach_task_self_: libc::mach_port_t;
}

// -- processor ------------------------------------------------------------------------------------------------

/// The ticks each core has spent in each state since boot: user, system, idle, nice. They wrap around, so only the
/// difference of two readings means anything. `None` when the kernel does not answer.
pub fn cpu_ticks() -> Option<Vec<[u32; 4]>> {
    let mut cores: libc::natural_t = 0;
    let mut info: libc::processor_info_array_t = ptr::null_mut();
    let mut length: libc::mach_msg_type_number_t = 0;
    // SAFETY: the three out values are valid. On success the kernel hands over an array of `length` integers in
    // this task, which is released below.
    let status = unsafe {
        libc::host_processor_info(
            mach_host_self(),
            libc::PROCESSOR_CPU_LOAD_INFO,
            &mut cores,
            &mut info,
            &mut length,
        )
    };
    if status != 0 || info.is_null() {
        return None;
    }
    let states = libc::CPU_STATE_MAX as usize;
    let ticks = (0..cores as usize)
        .map(|core| {
            let mut ticks = [0u32; 4];
            for (state, slot) in ticks.iter_mut().enumerate().take(states) {
                // SAFETY: the array has CPU_STATE_MAX integers for each of the `cores` cores.
                *slot = unsafe { *info.add(core * states + state) } as u32;
            }
            ticks
        })
        .collect();
    // SAFETY: the array was allocated for this task by the call above and is released exactly once.
    unsafe {
        libc::vm_deallocate(
            mach_task_self_,
            info as libc::vm_address_t,
            length as libc::vm_size_t * size_of::<libc::integer_t>(),
        );
    }
    Some(ticks)
}

fn sysctl<T>(name: &str) -> Option<T> {
    let name = CString::new(name).ok()?;
    // SAFETY: every `T` used here is a plain integer or C struct that the kernel fills completely.
    let mut value: T = unsafe { zeroed() };
    let mut size = size_of::<T>();
    // SAFETY: `value` outlives the call and `size` is exactly its size in bytes.
    let status = unsafe {
        libc::sysctlbyname(
            name.as_ptr(),
            (&raw mut value).cast(),
            &mut size,
            ptr::null_mut(),
            0,
        )
    };
    (status == 0 && size == size_of::<T>()).then_some(value)
}

/// How many cores are the slow, frugal kind and how many the fast kind, on a chip that has two kinds (Apple
/// silicon). The slow ones come first in the list of cores. `None` on a chip with one kind.
pub fn core_kinds() -> Option<(usize, usize)> {
    let fast: i32 = sysctl("hw.perflevel0.logicalcpu")?;
    let slow: i32 = sysctl("hw.perflevel1.logicalcpu")?;
    (fast > 0 && slow > 0).then_some((slow as usize, fast as usize))
}

// -- memory ---------------------------------------------------------------------------------------------------

/// How the memory is used, in bytes, the way Activity Monitor adds it up.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Memory {
    pub total: u64,
    /// What apps hold, not counting what can be given back at once.
    pub app: u64,
    /// What the system keeps in place.
    pub wired: u64,
    pub compressed: u64,
}

impl Memory {
    pub fn used(&self) -> u64 {
        self.app + self.wired + self.compressed
    }
}

/// How hard the system is pressed for memory.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Pressure {
    Normal,
    Warning,
    Critical,
}

pub fn memory() -> Option<Memory> {
    let total: u64 = sysctl("hw.memsize")?;
    let page: u64 = sysctl::<u32>("vm.pagesize")
        .map(u64::from)
        .or_else(|| sysctl::<u64>("hw.pagesize"))
        .filter(|p| *p > 0)?;
    // SAFETY: a plain C struct that the call fills.
    let mut stats: libc::vm_statistics64 = unsafe { zeroed() };
    let mut count = libc::HOST_VM_INFO64_COUNT;
    // SAFETY: `stats` has room for `count` integers, which is what the call may write.
    let status = unsafe {
        libc::host_statistics64(
            mach_host_self(),
            libc::HOST_VM_INFO64,
            (&raw mut stats).cast(),
            &mut count,
        )
    };
    if status != 0 {
        return None;
    }
    let pages = |n: u32| u64::from(n) * page;
    Some(Memory {
        total,
        app: pages(
            stats
                .internal_page_count
                .saturating_sub(stats.purgeable_count),
        ),
        wired: pages(stats.wire_count),
        compressed: pages(stats.compressor_page_count),
    })
}

pub fn memory_pressure() -> Pressure {
    match sysctl::<i32>("kern.memorystatus_vm_pressure_level") {
        Some(4) => Pressure::Critical,
        Some(2) => Pressure::Warning,
        _ => Pressure::Normal,
    }
}

/// The swap file's size and use, in bytes.
pub fn swap() -> Option<(u64, u64)> {
    let usage: libc::xsw_usage = sysctl("vm.swapusage")?;
    Some((usage.xsu_total, usage.xsu_used))
}

// -- processes ------------------------------------------------------------------------------------------------

/// How many nanoseconds a tick of the machine's clock is, as a fraction.
#[repr(C)]
struct Timebase {
    numer: u32,
    denom: u32,
}

/// One running process: the processor time it has used so far, in nanoseconds, and the memory it holds, in bytes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Process {
    pub pid: i32,
    pub name: String,
    pub cpu_ns: u64,
    pub memory: u64,
}

/// The processes this user may read; those of the system and of other users are left out, since the kernel does
/// not tell their figures to an app that is not root. Their processor time only means something as the difference
/// of two readings.
pub fn processes() -> Vec<Process> {
    let mut base = Timebase { numer: 0, denom: 0 };
    // SAFETY: `base` is a valid out value.
    if unsafe { mach_timebase_info(&mut base) } != 0 || base.denom == 0 {
        return Vec::new();
    }
    // SAFETY: a null buffer asks only for the number of processes.
    let count = unsafe { libc::proc_listallpids(ptr::null_mut(), 0) };
    if count <= 0 {
        return Vec::new();
    }
    // Room for some that start between the two calls.
    let mut pids = vec![0i32; count as usize + 64];
    // SAFETY: the buffer holds `pids.len()` integers and its size in bytes is what is passed.
    let n = unsafe {
        libc::proc_listallpids(
            pids.as_mut_ptr().cast(),
            (pids.len() * size_of::<i32>()) as i32,
        )
    };
    pids.truncate(n.max(0) as usize);
    let mut out = Vec::with_capacity(pids.len());
    for pid in pids.into_iter().filter(|&p| p > 0) {
        // SAFETY: a plain C struct that the call fills.
        let mut info: libc::proc_taskinfo = unsafe { zeroed() };
        let size = size_of::<libc::proc_taskinfo>() as i32;
        // SAFETY: `info` has room for `size` bytes, which is the most the call writes.
        let got = unsafe {
            libc::proc_pidinfo(pid, libc::PROC_PIDTASKINFO, 0, (&raw mut info).cast(), size)
        };
        if got != size {
            continue;
        }
        // The file name of the program, since the short name the kernel keeps is cut at 32 bytes.
        let mut path = [0u8; libc::PROC_PIDPATHINFO_MAXSIZE as usize];
        // SAFETY: the buffer is as long as is passed, and the call writes the path into it.
        let len = unsafe { libc::proc_pidpath(pid, path.as_mut_ptr().cast(), path.len() as u32) };
        if len <= 0 {
            continue;
        }
        let path = String::from_utf8_lossy(&path[..len as usize]);
        let name = path.rsplit('/').next().unwrap_or(&path).to_owned();
        // The times are in ticks of the machine's clock, which on Apple silicon are not nanoseconds.
        let ticks = u128::from(info.pti_total_user + info.pti_total_system);
        out.push(Process {
            pid,
            name,
            cpu_ns: (ticks * u128::from(base.numer) / u128::from(base.denom)) as u64,
            memory: info.pti_resident_size,
        });
    }
    out
}

// -- network --------------------------------------------------------------------------------------------------

/// The bytes one network interface has received and sent. The counters are 32 bits wide, so they wrap.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Interface {
    pub name: String,
    pub received: u32,
    pub sent: u32,
}

/// The counters of each network interface that carries what the Mac really sends and receives: the Wi-Fi and
/// wired adapters (`en0`, `en5`, the Thunderbolt bridge's own). The tunnels (`utun`) repeat what the adapter
/// already counted, and `awdl` and `llw` are the peer to peer links of AirDrop and the like.
pub fn interfaces() -> Vec<Interface> {
    let mut list: *mut libc::ifaddrs = ptr::null_mut();
    // SAFETY: `list` is a valid out value; the list is released below.
    if unsafe { libc::getifaddrs(&mut list) } != 0 {
        return Vec::new();
    }
    let mut found = Vec::new();
    let mut cursor = list;
    while !cursor.is_null() {
        // SAFETY: each entry of the list is valid until freeifaddrs.
        let entry = unsafe { &*cursor };
        cursor = entry.ifa_next;
        let flags = entry.ifa_flags;
        let up = flags & libc::IFF_UP as u32 != 0 && flags & libc::IFF_LOOPBACK as u32 == 0;
        // SAFETY: the name is a NUL terminated string.
        let name = unsafe { CStr::from_ptr(entry.ifa_name) }
            .to_string_lossy()
            .into_owned();
        if !up || !name.starts_with("en") || entry.ifa_addr.is_null() || entry.ifa_data.is_null() {
            continue;
        }
        // SAFETY: the address of an entry is valid, and its family says what ifa_data holds.
        if i32::from(unsafe { (*entry.ifa_addr).sa_family }) != libc::AF_LINK {
            continue;
        }
        // SAFETY: the data of a link level entry is an `if_data`.
        let data = unsafe { &*entry.ifa_data.cast::<libc::if_data>() };
        found.push(Interface {
            name,
            received: data.ifi_ibytes,
            sent: data.ifi_obytes,
        });
    }
    // SAFETY: `list` came from getifaddrs and is released once.
    unsafe { libc::freeifaddrs(list) };
    found
}

// -- the startup disk -----------------------------------------------------------------------------------------

/// The room on the startup disk, in bytes: what can be used, counting what the system would free on demand, and
/// the whole size.
pub fn disk() -> Option<(u64, u64)> {
    let url = NSURL::fileURLWithPath(&NSString::from_str("/"));
    // SAFETY: the keys are process-lifetime constants.
    let keys = unsafe {
        NSArray::from_slice(&[
            NSURLVolumeAvailableCapacityForImportantUsageKey,
            NSURLVolumeTotalCapacityKey,
        ])
    };
    let values = url.resourceValuesForKeys_error(&keys).ok()?;
    let number = |key: &NSString| -> Option<u64> {
        let value = values.objectForKey(key)?;
        // SAFETY: both values are NSNumbers holding a size.
        let n: i64 = unsafe { objc2::msg_send![&*value, longLongValue] };
        u64::try_from(n).ok()
    };
    // SAFETY: the keys are process-lifetime constants.
    let (free, total) = unsafe {
        (
            number(NSURLVolumeAvailableCapacityForImportantUsageKey)?,
            number(NSURLVolumeTotalCapacityKey)?,
        )
    };
    (total > 0).then_some((free.min(total), total))
}

// -- the graphics processor, the battery and the heat ---------------------------------------------------------

type Registry = CFDictionary<CFString, CFType>;

/// The matching services of a class of the I/O registry, each given to `each` and released after it.
fn services(class: &CStr, mut each: impl FnMut(u32)) {
    // SAFETY: IOServiceMatching returns a dictionary that IOServiceGetMatchingServices consumes.
    let matching = unsafe { IOServiceMatching(class.as_ptr()) };
    if matching.is_null() {
        return;
    }
    let mut iterator = 0u32;
    // SAFETY: `iterator` is a valid out value; port 0 is the default main port.
    if unsafe { IOServiceGetMatchingServices(0, matching, &mut iterator) } != 0 {
        return;
    }
    loop {
        // SAFETY: `iterator` is the one just made.
        let service = unsafe { IOIteratorNext(iterator) };
        if service == 0 {
            break;
        }
        each(service);
        // SAFETY: `service` was returned by IOIteratorNext and is released once.
        unsafe { IOObjectRelease(service) };
    }
    // SAFETY: as above, for the iterator.
    unsafe { IOObjectRelease(iterator) };
}

fn property(service: u32, key: &'static str) -> Option<CFType> {
    let key = CFString::from_static_string(key);
    // SAFETY: the create rule: the value is owned by the wrapper made below.
    let value = unsafe {
        IORegistryEntryCreateCFProperty(service, key.as_concrete_TypeRef(), ptr::null(), 0)
    };
    // SAFETY: a non-null result of a Create function is owned by the caller.
    (!value.is_null()).then(|| unsafe { CFType::wrap_under_create_rule(value) })
}

fn number_in(dictionary: &Registry, key: &'static str) -> Option<i64> {
    dictionary
        .find(CFString::from_static_string(key))?
        .downcast::<CFNumber>()?
        .to_i64()
}

/// How busy the busiest graphics processor is, 0 to 100, from what its driver reports about itself.
pub fn gpu_percent() -> Option<f32> {
    let mut busiest: Option<i64> = None;
    services(c"IOAccelerator", |service| {
        let Some(stats) = property(service, "PerformanceStatistics") else {
            return;
        };
        let Some(dictionary) = stats.downcast::<CFDictionary>() else {
            return;
        };
        // SAFETY: the statistics are a dictionary of strings to values, as the driver publishes them.
        let dictionary: Registry =
            unsafe { CFDictionary::wrap_under_get_rule(dictionary.as_concrete_TypeRef()) };
        if let Some(value) = number_in(&dictionary, "Device Utilization %") {
            busiest = Some(busiest.map_or(value, |b| b.max(value)));
        }
    });
    busiest.map(|v| v.clamp(0, 100) as f32)
}

/// The battery's wear: how much of its design capacity it still holds, in percent, and the number of charge
/// cycles it has been through.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Wear {
    pub health: u8,
    pub cycles: u32,
}

/// From the figures the battery reports. Apple silicon reports the capacity in milliamp hours as the raw
/// maximum; an older Mac reports it as the maximum. Either is held against the design capacity.
pub fn wear_of(raw_max: Option<i64>, max: Option<i64>, design: i64, cycles: i64) -> Option<Wear> {
    if design <= 0 {
        return None;
    }
    let capacity = raw_max.filter(|c| *c > 100).or(max.filter(|c| *c > 100))?;
    let health = (capacity.saturating_mul(100) + design / 2) / design;
    Some(Wear {
        health: u8::try_from(health.clamp(0, 100)).unwrap_or(100),
        cycles: u32::try_from(cycles.max(0)).unwrap_or(0),
    })
}

pub fn battery_wear() -> Option<Wear> {
    let mut wear = None;
    services(c"AppleSmartBattery", |service| {
        let number = |key| {
            property(service, key)
                .and_then(|v| v.downcast::<CFNumber>())
                .and_then(|n| n.to_i64())
        };
        if let Some(design) = number("DesignCapacity") {
            wear = wear_of(
                number("AppleRawMaxCapacity"),
                number("MaxCapacity"),
                design,
                number("CycleCount").unwrap_or(0),
            );
        }
    });
    wear
}

/// How hot the Mac says it is running.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Thermal {
    Nominal,
    Fair,
    Serious,
    Critical,
}

pub fn thermal() -> Thermal {
    match NSProcessInfo::processInfo().thermalState() {
        NSProcessInfoThermalState::Fair => Thermal::Fair,
        NSProcessInfoThermalState::Serious => Thermal::Serious,
        NSProcessInfoThermalState::Critical => Thermal::Critical,
        _ => Thermal::Nominal,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn this_process_is_listed_with_its_memory() {
        let me = std::process::id() as i32;
        let list = processes();
        let own = list
            .iter()
            .find(|p| p.pid == me)
            .expect("this process can read itself");
        assert!(own.memory > 0 && !own.name.is_empty());
    }

    #[test]
    fn the_processor_has_cores_that_each_have_four_counters() {
        let ticks = cpu_ticks().expect("the kernel answers");
        assert!(!ticks.is_empty());
        assert!(ticks.iter().all(|core| core.iter().any(|t| *t > 0)));
    }

    #[test]
    fn the_counters_grow_and_never_all_stand_still() {
        let before = cpu_ticks().unwrap();
        // A little work, so that some core has something to count.
        let mut sum = 0u64;
        for i in 0..20_000_000u64 {
            sum = sum.wrapping_add(i ^ (sum >> 3));
        }
        assert!(sum > 0);
        let after = cpu_ticks().unwrap();
        let total = |t: &[[u32; 4]]| t.iter().flatten().map(|v| u64::from(*v)).sum::<u64>();
        assert!(total(&after) >= total(&before));
    }

    #[test]
    fn the_memory_adds_up_and_fits_in_the_machine() {
        let memory = memory().expect("the kernel answers");
        assert!(memory.total >= 1 << 30, "a Mac has a gigabyte at least");
        assert!(memory.used() > 0 && memory.used() <= memory.total * 2);
    }

    #[test]
    fn the_swap_and_the_pressure_can_be_read() {
        if let Some((total, used)) = swap() {
            assert!(used <= total || total == 0);
        }
        let _ = memory_pressure();
    }

    #[test]
    fn the_network_lists_only_the_adapters_and_never_the_loopback_or_a_tunnel() {
        for interface in interfaces() {
            assert!(interface.name.starts_with("en"), "{}", interface.name);
        }
    }

    #[test]
    fn the_startup_disk_has_a_size_and_room() {
        let (free, total) = disk().expect("the startup disk");
        assert!(total > 1 << 30 && free <= total);
    }

    #[test]
    fn the_graphics_processor_reports_a_percentage_when_it_reports_at_all() {
        if let Some(percent) = gpu_percent() {
            assert!((0.0..=100.0).contains(&percent));
        }
        let _ = battery_wear();
        let _ = thermal();
        let _ = core_kinds();
    }

    #[test]
    fn a_battery_that_holds_most_of_its_design_is_healthy() {
        // Apple silicon: the raw maximum is in milliamp hours.
        let wear = wear_of(Some(5000), Some(100), 5500, 212).unwrap();
        assert_eq!((wear.health, wear.cycles), (91, 212));
        // An older Mac: the maximum is in milliamp hours as well.
        let old = wear_of(None, Some(4000), 5000, 640).unwrap();
        assert_eq!(old.health, 80);
    }

    #[test]
    fn a_percentage_in_place_of_a_capacity_is_not_taken_for_one() {
        assert!(wear_of(None, Some(100), 5500, 10).is_none());
        assert!(wear_of(Some(5000), None, 0, 10).is_none());
        let full = wear_of(Some(6000), None, 5500, 5).unwrap();
        assert_eq!(
            full.health, 100,
            "a new battery can hold more than its design"
        );
    }
}
