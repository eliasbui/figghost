//! Per-app audio through Core Audio process taps (macOS 14.2 and later): which processes play sound,
//! the output devices, private taps that capture (and mute) some processes or everything bound for a
//! device, and routes that play a tap on an output device through a render callback.
//!
//! Everything created here is private to this process. If the app quits or crashes, the system removes
//! the taps and aggregate devices, and the tapped apps play normally again.

use block2::RcBlock;
use core_foundation::{
    array::CFArray,
    base::{CFType, TCFType},
    dictionary::CFDictionary,
    number::CFNumber,
    string::{CFString, CFStringRef},
};
use dispatch2::DispatchQueue;
use objc2::{
    msg_send,
    rc::{Allocated, Retained},
    runtime::{AnyClass, AnyObject},
};
use objc2_foundation::{NSArray, NSNumber, NSString, NSUUID};
use std::{
    cell::UnsafeCell,
    ffi::c_void,
    fmt,
    mem::size_of,
    panic::{AssertUnwindSafe, catch_unwind},
    ptr,
    sync::{
        OnceLock,
        atomic::{AtomicU32, Ordering},
    },
};

pub type ObjectId = u32;

/// Every aggregate device this app creates has a UID with this prefix, so device lists can leave them out.
const AGGREGATE_UID_PREFIX: &str = "vn.figghost.route.";

const fn code(text: &[u8; 4]) -> u32 {
    u32::from_be_bytes(*text)
}

const SYSTEM_OBJECT: ObjectId = 1;
const SCOPE_GLOBAL: u32 = code(b"glob");
const SCOPE_INPUT: u32 = code(b"inpt");
const SCOPE_OUTPUT: u32 = code(b"outp");
const ELEMENT_MAIN: u32 = 0;

const HARDWARE_DEVICES: u32 = code(b"dev#");
const HARDWARE_DEFAULT_OUTPUT: u32 = code(b"dOut");
const HARDWARE_PROCESSES: u32 = code(b"prs#");
const HARDWARE_PID_TO_PROCESS: u32 = code(b"id2p");
const HARDWARE_TAPS: u32 = code(b"tps#");
const OBJECT_NAME: u32 = code(b"lnam");
const DEVICE_UID: u32 = code(b"uid ");
const DEVICE_STREAMS: u32 = code(b"stm#");
const DEVICE_TRANSPORT: u32 = code(b"tran");
const DEVICE_NOMINAL_RATE: u32 = code(b"nsrt");
const DEVICE_STREAM_CONFIGURATION: u32 = code(b"slay");
const DEVICE_VIRTUAL_MAIN_VOLUME: u32 = code(b"vmvc");
const PROCESS_PID: u32 = code(b"ppid");
const PROCESS_BUNDLE_ID: u32 = code(b"pbid");
const PROCESS_RUNNING_OUTPUT: u32 = code(b"piro");
const TAP_DESCRIPTION: u32 = code(b"tdsc");
const TAP_FORMAT: u32 = code(b"tfmt");

/// Selectors a caller may listen to with [`Listener`].
pub mod selector {
    use super::code;
    /// System object: the list of processes connected to the audio system.
    pub const PROCESSES: u32 = code(b"prs#");
    /// System object: the list of devices.
    pub const DEVICES: u32 = code(b"dev#");
    /// System object: the default output device.
    pub const DEFAULT_OUTPUT: u32 = code(b"dOut");
    /// Process object: whether it is playing audio now.
    pub const RUNNING_OUTPUT: u32 = code(b"piro");
    /// Device: its sample rate.
    pub const NOMINAL_RATE: u32 = code(b"nsrt");
}

pub const SYSTEM: ObjectId = SYSTEM_OBJECT;

/// How a device is connected, from `kAudioDevicePropertyTransportType`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Transport {
    BuiltIn,
    Usb,
    Bluetooth,
    Hdmi,
    DisplayPort,
    AirPlay,
    Thunderbolt,
    Virtual,
    Aggregate,
    Other(u32),
}

impl Transport {
    fn from_code(value: u32) -> Self {
        match value {
            v if v == code(b"bltn") => Transport::BuiltIn,
            v if v == code(b"usb ") => Transport::Usb,
            v if v == code(b"blue") || v == code(b"blea") => Transport::Bluetooth,
            v if v == code(b"hdmi") => Transport::Hdmi,
            v if v == code(b"dprt") => Transport::DisplayPort,
            v if v == code(b"airp") => Transport::AirPlay,
            v if v == code(b"thun") => Transport::Thunderbolt,
            v if v == code(b"virt") => Transport::Virtual,
            v if v == code(b"grup") => Transport::Aggregate,
            other => Transport::Other(other),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct OutputDevice {
    pub id: ObjectId,
    pub uid: String,
    pub name: String,
    pub transport: Transport,
    /// False for devices whose volume the system cannot change (HDMI and DisplayPort speakers, mostly).
    pub hardware_volume: bool,
}

/// A process connected to the audio system.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AudioProcess {
    pub object: ObjectId,
    pub pid: i32,
    pub bundle_id: Option<String>,
    /// Playing audio right now.
    pub playing: bool,
}

/// A failed Core Audio call: what was being done and the OSStatus it returned.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Error {
    pub action: &'static str,
    pub status: i32,
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let bytes = self.status.to_be_bytes();
        if bytes.iter().all(|b| b.is_ascii_graphic() || *b == b' ') {
            write!(
                f,
                "{} failed ('{}')",
                self.action,
                String::from_utf8_lossy(&bytes)
            )
        } else {
            write!(f, "{} failed ({})", self.action, self.status)
        }
    }
}

impl std::error::Error for Error {}

fn check(status: i32, action: &'static str) -> Result<(), Error> {
    if status == 0 {
        Ok(())
    } else {
        Err(Error { action, status })
    }
}

#[repr(C)]
#[derive(Clone, Copy)]
struct Address {
    selector: u32,
    scope: u32,
    element: u32,
}

const fn address(selector: u32, scope: u32) -> Address {
    Address {
        selector,
        scope,
        element: ELEMENT_MAIN,
    }
}

#[repr(C)]
struct AudioBuffer {
    channels: u32,
    byte_size: u32,
    data: *mut c_void,
}

/// `AudioBufferList`: a count followed by that many buffers (declared with one, as in C).
#[repr(C)]
struct AudioBufferList {
    count: u32,
    buffers: [AudioBuffer; 1],
}

#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
struct StreamDescription {
    sample_rate: f64,
    format_id: u32,
    format_flags: u32,
    bytes_per_packet: u32,
    frames_per_packet: u32,
    bytes_per_frame: u32,
    channels_per_frame: u32,
    bits_per_channel: u32,
    reserved: u32,
}

type IoProc = unsafe extern "C" fn(
    ObjectId,
    *const c_void,
    *const AudioBufferList,
    *const c_void,
    *mut AudioBufferList,
    *const c_void,
    *mut c_void,
) -> i32;
type IoProcId = *mut c_void;
type ListenerBlock = block2::DynBlock<dyn Fn(u32, *const c_void)>;

#[link(name = "CoreAudio", kind = "framework")]
unsafe extern "C" {
    fn AudioObjectGetPropertyDataSize(
        object: ObjectId,
        address: *const Address,
        qualifier_size: u32,
        qualifier: *const c_void,
        size: *mut u32,
    ) -> i32;
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
    fn AudioObjectAddPropertyListenerBlock(
        object: ObjectId,
        address: *const Address,
        queue: *mut c_void,
        listener: *mut ListenerBlock,
    ) -> i32;
    fn AudioObjectRemovePropertyListenerBlock(
        object: ObjectId,
        address: *const Address,
        queue: *mut c_void,
        listener: *mut ListenerBlock,
    ) -> i32;
    fn AudioHardwareCreateAggregateDevice(description: *const c_void, out: *mut ObjectId) -> i32;
    fn AudioHardwareDestroyAggregateDevice(device: ObjectId) -> i32;
    fn AudioDeviceCreateIOProcID(
        device: ObjectId,
        proc_: IoProc,
        client: *mut c_void,
        out: *mut IoProcId,
    ) -> i32;
    fn AudioDeviceDestroyIOProcID(device: ObjectId, proc_id: IoProcId) -> i32;
    fn AudioDeviceStart(device: ObjectId, proc_id: IoProcId) -> i32;
    fn AudioDeviceStop(device: ObjectId, proc_id: IoProcId) -> i32;
}

type CreateTapFn = unsafe extern "C" fn(*mut AnyObject, *mut ObjectId) -> i32;
type DestroyTapFn = unsafe extern "C" fn(ObjectId) -> i32;
type ResponsibleFn = unsafe extern "C" fn(i32) -> i32;

struct TapFunctions {
    create: CreateTapFn,
    destroy: DestroyTapFn,
}

fn symbol(name: &std::ffi::CStr) -> Option<*mut c_void> {
    // SAFETY: dlsym with RTLD_DEFAULT only looks a name up; a missing symbol returns null.
    let found = unsafe { libc::dlsym(libc::RTLD_DEFAULT, name.as_ptr()) };
    (!found.is_null()).then_some(found)
}

/// Looked up at run time so the app still launches on systems older than macOS 14.2.
fn tap_functions() -> Option<&'static TapFunctions> {
    static FUNCTIONS: OnceLock<Option<TapFunctions>> = OnceLock::new();
    FUNCTIONS
        .get_or_init(|| {
            let create = symbol(c"AudioHardwareCreateProcessTap")?;
            let destroy = symbol(c"AudioHardwareDestroyProcessTap")?;
            AnyClass::get(c"CATapDescription")?;
            // SAFETY: both symbols are the documented CoreAudio functions with these signatures.
            Some(unsafe {
                TapFunctions {
                    create: std::mem::transmute::<*mut c_void, CreateTapFn>(create),
                    destroy: std::mem::transmute::<*mut c_void, DestroyTapFn>(destroy),
                }
            })
        })
        .as_ref()
}

/// Whether this system can tap processes (macOS 14.2 and later).
pub fn taps_supported() -> bool {
    tap_functions().is_some()
}

fn get<T: Copy + Default>(object: ObjectId, address: Address) -> Option<T> {
    let mut value = T::default();
    let mut size = size_of::<T>() as u32;
    // SAFETY: `value` outlives the call and `size` is exactly its size in bytes.
    let status = unsafe {
        AudioObjectGetPropertyData(
            object,
            &address,
            0,
            ptr::null(),
            &mut size,
            (&raw mut value).cast(),
        )
    };
    (status == 0 && size as usize == size_of::<T>()).then_some(value)
}

fn get_with_qualifier<T: Copy + Default, Q>(
    object: ObjectId,
    address: Address,
    qualifier: &Q,
) -> Option<T> {
    let mut value = T::default();
    let mut size = size_of::<T>() as u32;
    // SAFETY: `value` and `qualifier` outlive the call and the sizes passed are exactly theirs.
    let status = unsafe {
        AudioObjectGetPropertyData(
            object,
            &address,
            size_of::<Q>() as u32,
            ptr::from_ref(qualifier).cast(),
            &mut size,
            (&raw mut value).cast(),
        )
    };
    (status == 0).then_some(value)
}

fn get_ids(object: ObjectId, address: Address) -> Vec<ObjectId> {
    let mut size = 0u32;
    // SAFETY: `size` is a valid out pointer.
    if unsafe { AudioObjectGetPropertyDataSize(object, &address, 0, ptr::null(), &mut size) } != 0 {
        return Vec::new();
    }
    let mut ids = vec![0 as ObjectId; size as usize / size_of::<ObjectId>()];
    // SAFETY: the buffer holds `size` bytes, as the call above reported.
    let status = unsafe {
        AudioObjectGetPropertyData(
            object,
            &address,
            0,
            ptr::null(),
            &mut size,
            ids.as_mut_ptr().cast(),
        )
    };
    if status != 0 {
        return Vec::new();
    }
    ids.truncate(size as usize / size_of::<ObjectId>());
    ids
}

fn get_string(object: ObjectId, address: Address) -> Option<String> {
    let mut value: CFStringRef = ptr::null();
    let mut size = size_of::<CFStringRef>() as u32;
    // SAFETY: the property is a CFString returned with a +1 reference, which `wrap_under_create_rule` takes.
    unsafe {
        let status = AudioObjectGetPropertyData(
            object,
            &address,
            0,
            ptr::null(),
            &mut size,
            (&raw mut value).cast(),
        );
        if status != 0 || value.is_null() {
            return None;
        }
        Some(CFString::wrap_under_create_rule(value).to_string())
    }
}

fn settable(object: ObjectId, address: Address) -> bool {
    let mut flag = 0u8;
    // SAFETY: `flag` is a valid byte for the Boolean result.
    let status = unsafe { AudioObjectIsPropertySettable(object, &address, &mut flag) };
    status == 0 && flag != 0
}

fn stream_count(device: ObjectId, scope: u32) -> usize {
    get_ids(device, address(DEVICE_STREAMS, scope)).len()
}

/// Channels of each stream in a scope, from the device's stream configuration.
fn stream_channels(device: ObjectId, scope: u32) -> Vec<u32> {
    let address = address(DEVICE_STREAM_CONFIGURATION, scope);
    let mut size = 0u32;
    // SAFETY: `size` is a valid out pointer.
    if unsafe { AudioObjectGetPropertyDataSize(device, &address, 0, ptr::null(), &mut size) } != 0 {
        return Vec::new();
    }
    // u64 storage keeps the buffer list's pointer field aligned.
    let mut storage = vec![0u64; (size as usize).div_ceil(8)];
    // SAFETY: the buffer holds at least `size` bytes, as the call above reported.
    let status = unsafe {
        AudioObjectGetPropertyData(
            device,
            &address,
            0,
            ptr::null(),
            &mut size,
            storage.as_mut_ptr().cast(),
        )
    };
    if status != 0 {
        return Vec::new();
    }
    let list = storage.as_ptr().cast::<AudioBufferList>();
    // SAFETY: the HAL filled a valid AudioBufferList of `count` buffers into `storage`.
    unsafe {
        let count = (*list).count as usize;
        let first = (&raw const (*list).buffers).cast::<AudioBuffer>();
        (0..count).map(|i| (*first.add(i)).channels).collect()
    }
}

fn devices() -> Vec<ObjectId> {
    get_ids(SYSTEM_OBJECT, address(HARDWARE_DEVICES, SCOPE_GLOBAL))
}

fn describe_output(id: ObjectId) -> Option<OutputDevice> {
    if stream_count(id, SCOPE_OUTPUT) == 0 {
        return None;
    }
    let uid = get_string(id, address(DEVICE_UID, SCOPE_GLOBAL))?;
    if uid.starts_with(AGGREGATE_UID_PREFIX) {
        return None;
    }
    Some(OutputDevice {
        id,
        name: get_string(id, address(OBJECT_NAME, SCOPE_GLOBAL)).unwrap_or_else(|| uid.clone()),
        uid,
        transport: Transport::from_code(
            get::<u32>(id, address(DEVICE_TRANSPORT, SCOPE_GLOBAL)).unwrap_or(0),
        ),
        hardware_volume: settable(id, address(DEVICE_VIRTUAL_MAIN_VOLUME, SCOPE_OUTPUT)),
    })
}

/// Every device that can play audio, without the routes this app made.
pub fn output_devices() -> Vec<OutputDevice> {
    devices().into_iter().filter_map(describe_output).collect()
}

/// How many aggregate devices this process made, and how many process taps it can see, exist now: a
/// check that dropping routes and taps leaves nothing behind.
pub fn routes_and_taps() -> (usize, usize) {
    let routes = devices()
        .into_iter()
        .filter(|&id| {
            get_string(id, address(DEVICE_UID, SCOPE_GLOBAL))
                .is_some_and(|uid| uid.starts_with(AGGREGATE_UID_PREFIX))
        })
        .count();
    let taps = get_ids(SYSTEM_OBJECT, address(HARDWARE_TAPS, SCOPE_GLOBAL)).len();
    (routes, taps)
}

pub fn default_output() -> Option<OutputDevice> {
    let id = get::<ObjectId>(
        SYSTEM_OBJECT,
        address(HARDWARE_DEFAULT_OUTPUT, SCOPE_GLOBAL),
    )?;
    describe_output(id)
}

/// Every process connected to the audio system, including ones that are silent now.
pub fn processes() -> Vec<AudioProcess> {
    get_ids(SYSTEM_OBJECT, address(HARDWARE_PROCESSES, SCOPE_GLOBAL))
        .into_iter()
        .filter_map(|object| {
            let pid = get::<i32>(object, address(PROCESS_PID, SCOPE_GLOBAL))?;
            Some(AudioProcess {
                object,
                pid,
                bundle_id: get_string(object, address(PROCESS_BUNDLE_ID, SCOPE_GLOBAL))
                    .filter(|b| !b.is_empty()),
                playing: is_playing(object),
            })
        })
        .collect()
}

pub fn is_playing(process: ObjectId) -> bool {
    get::<u32>(process, address(PROCESS_RUNNING_OUTPUT, SCOPE_GLOBAL)).is_some_and(|v| v != 0)
}

/// The process object of a pid, once that process has used audio.
pub fn process_object(pid: i32) -> Option<ObjectId> {
    get_with_qualifier::<ObjectId, i32>(
        SYSTEM_OBJECT,
        address(HARDWARE_PID_TO_PROCESS, SCOPE_GLOBAL),
        &pid,
    )
    .filter(|&object| object != 0)
}

/// The process macOS holds responsible for `pid`: the app itself for a browser's audio helper or a
/// WebKit media process, `pid` for an ordinary app.
pub fn responsible_pid(pid: i32) -> Option<i32> {
    static FUNCTION: OnceLock<Option<ResponsibleFn>> = OnceLock::new();
    let function = (*FUNCTION.get_or_init(|| {
        // SAFETY: the symbol, when present, has this signature (libquarantine's responsibility API).
        symbol(c"responsibility_get_pid_responsible_for_pid")
            .map(|f| unsafe { std::mem::transmute::<*mut c_void, ResponsibleFn>(f) })
    }))?;
    // SAFETY: a plain query on a pid; an unknown pid returns -1.
    let responsible = unsafe { function(pid) };
    (responsible > 0).then_some(responsible)
}

/// What a tap captures.
#[derive(Debug, Clone, Copy)]
pub enum Source<'a> {
    /// Everything these processes play, on any device, mixed to stereo.
    Processes(&'a [ObjectId]),
    /// Everything played on this device's first stream, except by these processes, in that stream's format.
    Device {
        uid: &'a str,
        exclude: &'a [ObjectId],
    },
}

/// What the tapped processes still send to their own device.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Muting {
    /// They keep playing as before; the tap only listens.
    Unmuted,
    /// They are silent while the tap exists.
    Muted,
    /// They are silent while a route reads the tap, and play normally while it is stopped.
    MutedWhileRead,
}

fn number_array(ids: &[ObjectId]) -> Retained<NSArray<NSNumber>> {
    let numbers: Vec<Retained<NSNumber>> = ids.iter().map(|&id| NSNumber::new_u32(id)).collect();
    NSArray::from_retained_slice(&numbers)
}

/// A private process tap; destroyed when dropped.
pub struct Tap {
    id: ObjectId,
    uid: String,
    description: Retained<AnyObject>,
}

impl Tap {
    pub fn new(source: Source<'_>, muting: Muting, name: &str) -> Result<Self, Error> {
        let unsupported = Error {
            action: "process taps (macOS 14.2 or later)",
            status: -1,
        };
        let functions = tap_functions().ok_or(unsupported)?;
        let class = AnyClass::get(c"CATapDescription").ok_or(unsupported)?;
        // SAFETY: CATapDescription's initializers take an NSArray of NSNumber process objects (and a device
        // UID and stream index); the setters take the documented property types.
        let description = unsafe {
            let allocated: Allocated<AnyObject> = msg_send![class, alloc];
            let description: Option<Retained<AnyObject>> = match source {
                Source::Processes(ids) => {
                    msg_send![allocated, initStereoMixdownOfProcesses: &*number_array(ids)]
                }
                Source::Device { uid, exclude } => msg_send![
                    allocated,
                    initExcludingProcesses: &*number_array(exclude),
                    andDeviceUID: &*NSString::from_str(uid),
                    withStream: 0isize
                ],
            };
            let description = description.ok_or(Error {
                action: "describing the tap",
                status: -1,
            })?;
            let behavior: isize = match muting {
                Muting::Unmuted => 0,
                Muting::Muted => 1,
                Muting::MutedWhileRead => 2,
            };
            let _: () = msg_send![&*description, setMuteBehavior: behavior];
            let _: () = msg_send![&*description, setPrivate: true];
            let _: () = msg_send![&*description, setName: &*NSString::from_str(name)];
            let _: () = msg_send![&*description, setUUID: &*NSUUID::new()];
            description
        };
        // SAFETY: as above.
        let uid = unsafe {
            let uuid: Retained<NSUUID> = msg_send![&*description, UUID];
            uuid.UUIDString().to_string()
        };
        let mut id = 0;
        // SAFETY: the description is a live CATapDescription and `id` a valid out pointer.
        check(
            unsafe { (functions.create)(Retained::as_ptr(&description).cast_mut(), &mut id) },
            "creating the process tap",
        )?;
        Ok(Self {
            id,
            uid,
            description,
        })
    }

    pub fn id(&self) -> ObjectId {
        self.id
    }

    pub fn uid(&self) -> &str {
        &self.uid
    }

    /// Replaces the processes of the tap in place: for a device tap, the ones it leaves out. The route
    /// reading it keeps running.
    pub fn set_processes(&self, ids: &[ObjectId]) -> Result<(), Error> {
        // SAFETY: `processes` takes an NSArray of NSNumber process objects.
        let _: () = unsafe { msg_send![&*self.description, setProcesses: &*number_array(ids)] };
        let pointer = Retained::as_ptr(&self.description);
        // SAFETY: the property holds a CATapDescription pointer; the description outlives the call.
        check(
            unsafe {
                AudioObjectSetPropertyData(
                    self.id,
                    &address(TAP_DESCRIPTION, SCOPE_GLOBAL),
                    0,
                    ptr::null(),
                    size_of::<*const AnyObject>() as u32,
                    (&raw const pointer).cast(),
                )
            },
            "updating the process tap",
        )
    }

    /// Sample rate and channel count of what the tap delivers.
    pub fn format(&self) -> Option<(f64, u32)> {
        get::<StreamDescription>(self.id, address(TAP_FORMAT, SCOPE_GLOBAL))
            .map(|f| (f.sample_rate, f.channels_per_frame))
    }
}

impl Drop for Tap {
    fn drop(&mut self) {
        if let Some(functions) = tap_functions() {
            // SAFETY: `id` came from AudioHardwareCreateProcessTap and is destroyed once.
            unsafe { (functions.destroy)(self.id) };
        }
    }
}

/// The shape of a route's audio, known once its aggregate device exists.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RouteFormat {
    pub sample_rate: f64,
    pub input_channels: usize,
    pub output_channels: usize,
}

/// Called on the device's real-time IO thread once per cycle, with the tap's interleaved frames and the
/// device's (zeroed) interleaved output, and the channel counts of each. It must not allocate, lock,
/// block or panic; a panic plays silence for that cycle.
pub type Render = Box<dyn FnMut(&[f32], usize, &mut [f32], usize) + Send>;

struct RouteContext {
    render: UnsafeCell<Render>,
    /// Which input buffer carries the tap: the aggregate lists its devices' input streams first.
    tap_buffer: usize,
    /// Highest output sample since the last `take_peak`, as f32 bits.
    peak: AtomicU32,
}

/// # Safety
/// `list` must be a valid AudioBufferList whose buffers hold interleaved f32 samples.
unsafe fn buffer<'a>(list: *const AudioBufferList, index: usize) -> Option<(&'a mut [f32], usize)> {
    // SAFETY: guaranteed by the caller; the index is checked against the count.
    unsafe {
        if list.is_null() || index >= (*list).count as usize {
            return None;
        }
        let entry = &*(&raw const (*list).buffers)
            .cast::<AudioBuffer>()
            .add(index);
        if entry.data.is_null() || entry.channels == 0 {
            return None;
        }
        let samples = entry.byte_size as usize / size_of::<f32>();
        Some((
            std::slice::from_raw_parts_mut(entry.data.cast::<f32>(), samples),
            entry.channels as usize,
        ))
    }
}

unsafe extern "C" fn io_proc(
    _device: ObjectId,
    _now: *const c_void,
    input: *const AudioBufferList,
    _input_time: *const c_void,
    output: *mut AudioBufferList,
    _output_time: *const c_void,
    client: *mut c_void,
) -> i32 {
    // SAFETY: `client` is the RouteContext the route keeps alive until its IOProc is destroyed; only this
    // thread touches `render`.
    let context = unsafe { &*client.cast::<RouteContext>() };
    // SAFETY: the HAL passes valid float buffer lists for the cycle.
    unsafe {
        let count = if output.is_null() {
            0
        } else {
            (*output).count as usize
        };
        for index in 0..count {
            if let Some((samples, _)) = buffer(output, index) {
                samples.fill(0.0);
            }
        }
        let (Some((tap, tap_channels)), Some((out, out_channels))) =
            (buffer(input, context.tap_buffer), buffer(output, 0))
        else {
            return 0;
        };
        let render = &mut *context.render.get();
        let rendered = catch_unwind(AssertUnwindSafe(|| {
            render(tap, tap_channels, out, out_channels);
        }));
        if rendered.is_err() {
            out.fill(0.0);
            return 0;
        }
        let peak = out.iter().fold(0.0f32, |m, x| m.max(x.abs()));
        context.peak.fetch_max(peak.to_bits(), Ordering::Relaxed);
    }
    0
}

/// A tap played through an output device: a private aggregate device with the device as output and the
/// tap as input, and an IOProc calling the render callback.
pub struct Route {
    aggregate: ObjectId,
    proc_id: IoProcId,
    context: *mut RouteContext,
    format: RouteFormat,
    running: bool,
    tap: Tap,
}

impl Route {
    /// Builds the route, stopped. `render` is made once the format is known.
    pub fn new(
        tap: Tap,
        output_uid: &str,
        name: &str,
        render: impl FnOnce(RouteFormat) -> Render,
    ) -> Result<Self, Error> {
        let uid = format!("{AGGREGATE_UID_PREFIX}{}", NSUUID::new().UUIDString());
        let key = |k: &str| CFString::new(k);
        let sub_device =
            CFDictionary::from_CFType_pairs(&[(key("uid"), CFString::new(output_uid))]);
        let sub_tap = CFDictionary::from_CFType_pairs(&[
            (key("uid"), CFString::new(tap.uid()).as_CFType()),
            (key("drift"), CFNumber::from(1).as_CFType()),
        ]);
        let description: CFDictionary<CFString, CFType> = CFDictionary::from_CFType_pairs(&[
            (key("uid"), CFString::new(&uid).as_CFType()),
            (key("name"), CFString::new(name).as_CFType()),
            (key("master"), CFString::new(output_uid).as_CFType()),
            (key("private"), CFNumber::from(1).as_CFType()),
            (key("stacked"), CFNumber::from(0).as_CFType()),
            (
                key("subdevices"),
                CFArray::from_CFTypes(&[sub_device]).as_CFType(),
            ),
            (key("taps"), CFArray::from_CFTypes(&[sub_tap]).as_CFType()),
        ]);
        let mut aggregate = 0;
        // SAFETY: `description` is a valid CFDictionary for the duration of the call.
        check(
            unsafe {
                AudioHardwareCreateAggregateDevice(
                    description.as_concrete_TypeRef().cast(),
                    &mut aggregate,
                )
            },
            "creating the aggregate device",
        )?;
        let inputs = stream_channels(aggregate, SCOPE_INPUT);
        let outputs = stream_channels(aggregate, SCOPE_OUTPUT);
        let (Some(&input_channels), Some(&output_channels)) = (inputs.last(), outputs.first())
        else {
            // SAFETY: the aggregate was just created by this process.
            unsafe { AudioHardwareDestroyAggregateDevice(aggregate) };
            return Err(Error {
                action: "reading the aggregate device's streams",
                status: -1,
            });
        };
        let format = RouteFormat {
            sample_rate: get::<f64>(aggregate, address(DEVICE_NOMINAL_RATE, SCOPE_GLOBAL))
                .unwrap_or(48_000.0),
            input_channels: input_channels as usize,
            output_channels: output_channels as usize,
        };
        let context = Box::into_raw(Box::new(RouteContext {
            render: UnsafeCell::new(render(format)),
            tap_buffer: inputs.len() - 1,
            peak: AtomicU32::new(0),
        }));
        let mut proc_id: IoProcId = ptr::null_mut();
        // SAFETY: `context` stays allocated until the IOProc is destroyed in `drop`.
        let created = check(
            unsafe { AudioDeviceCreateIOProcID(aggregate, io_proc, context.cast(), &mut proc_id) },
            "creating the IO callback",
        );
        if let Err(e) = created {
            // SAFETY: no IOProc exists, so nothing else holds `context`; the aggregate is ours.
            unsafe {
                drop(Box::from_raw(context));
                AudioHardwareDestroyAggregateDevice(aggregate);
            }
            return Err(e);
        }
        Ok(Self {
            aggregate,
            proc_id,
            context,
            format,
            running: false,
            tap,
        })
    }

    pub fn format(&self) -> RouteFormat {
        self.format
    }

    pub fn tap(&self) -> &Tap {
        &self.tap
    }

    pub fn is_running(&self) -> bool {
        self.running
    }

    pub fn start(&mut self) -> Result<(), Error> {
        if !self.running {
            // SAFETY: the aggregate and IOProc belong to this route.
            check(
                unsafe { AudioDeviceStart(self.aggregate, self.proc_id) },
                "starting the route",
            )?;
            self.running = true;
        }
        Ok(())
    }

    /// Returns once the IO callback has run for the last time.
    pub fn stop(&mut self) {
        if self.running {
            // SAFETY: as in `start`.
            unsafe { AudioDeviceStop(self.aggregate, self.proc_id) };
            self.running = false;
        }
    }

    /// Highest output level since the last call, 0 to 1, for a level meter.
    pub fn take_peak(&self) -> f32 {
        // SAFETY: `context` lives as long as the route; `peak` is an atomic.
        let peak = unsafe { &(*self.context).peak };
        f32::from_bits(peak.swap(0, Ordering::Relaxed))
    }
}

impl Drop for Route {
    fn drop(&mut self) {
        self.stop();
        // SAFETY: the IOProc is stopped, so destroying it and then freeing its context is safe; the
        // aggregate was created by this route. The tap is destroyed after, when the field drops.
        unsafe {
            AudioDeviceDestroyIOProcID(self.aggregate, self.proc_id);
            drop(Box::from_raw(self.context));
            AudioHardwareDestroyAggregateDevice(self.aggregate);
        }
    }
}

/// A property listener whose callback runs on the main thread; removed when dropped.
pub struct Listener {
    object: ObjectId,
    address: Address,
    block: RcBlock<dyn Fn(u32, *const c_void)>,
}

impl Listener {
    pub fn new(object: ObjectId, selector: u32, on_change: impl Fn() + 'static) -> Option<Self> {
        let address = address(selector, SCOPE_GLOBAL);
        let block = RcBlock::new(move |_count: u32, _addresses: *const c_void| on_change());
        // SAFETY: the block is invoked only on the main queue given here, the thread that made it.
        let status = unsafe {
            AudioObjectAddPropertyListenerBlock(
                object,
                &address,
                main_queue(),
                RcBlock::as_ptr(&block),
            )
        };
        (status == 0).then_some(Self {
            object,
            address,
            block,
        })
    }
}

impl Drop for Listener {
    fn drop(&mut self) {
        // SAFETY: the same object, address, queue and block that were registered.
        unsafe {
            AudioObjectRemovePropertyListenerBlock(
                self.object,
                &self.address,
                main_queue(),
                RcBlock::as_ptr(&self.block),
            )
        };
    }
}

fn main_queue() -> *mut c_void {
    let queue: &DispatchQueue = DispatchQueue::main();
    ptr::from_ref(queue).cast_mut().cast()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn errors_show_four_character_codes_as_text() {
        let e = Error {
            action: "creating the process tap",
            status: i32::from_be_bytes(*b"nope"),
        };
        assert_eq!(e.to_string(), "creating the process tap failed ('nope')");
        let e = Error {
            action: "starting the route",
            status: -50,
        };
        assert_eq!(e.to_string(), "starting the route failed (-50)");
    }

    #[test]
    fn transports_decode() {
        assert_eq!(Transport::from_code(code(b"hdmi")), Transport::Hdmi);
        assert_eq!(Transport::from_code(code(b"blea")), Transport::Bluetooth);
        assert_eq!(Transport::from_code(7), Transport::Other(7));
    }

    #[test]
    fn this_process_is_its_own_responsible_process_or_has_one() {
        let me = std::process::id() as i32;
        assert!(responsible_pid(me).is_some());
        assert_eq!(responsible_pid(-5), None);
    }

    #[test]
    fn our_own_routes_are_never_listed_as_outputs() {
        // Reading the device list needs no permission; any listed device must be a real one.
        for device in output_devices() {
            assert!(!device.uid.starts_with(AGGREGATE_UID_PREFIX));
            assert!(!device.name.is_empty());
        }
    }
}
