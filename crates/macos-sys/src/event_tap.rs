//! Active CGEventTap hosted on its own run-loop thread.

use core_foundation::{
    base::TCFType,
    mach_port::{CFMachPort, CFMachPortInvalidate, CFMachPortRef},
    runloop::{CFRunLoop, kCFRunLoopCommonModes},
};
use std::{
    ffi::c_void,
    io,
    marker::PhantomData,
    panic::{AssertUnwindSafe, catch_unwind},
    ptr,
    sync::{
        atomic::{AtomicPtr, Ordering},
        mpsc,
    },
    thread::{self, JoinHandle},
    time::Duration,
};

pub const SCROLL_WHEEL: u32 = 22;

/// Equivalent of `CGEventMaskBit`.
pub const fn mask(event_type: u32) -> u64 {
    1 << event_type
}

/// `CGEventField` ids used by the scroll engine (CGEventTypes.h).
pub mod field {
    /// The process that posted the event; 0 for the hardware.
    pub const SOURCE_UNIX_PROCESS_ID: u32 = 41;
    pub const SOURCE_USER_DATA: u32 = 42;
    pub const SCROLL_DELTA_AXIS_1: u32 = 11;
    pub const SCROLL_DELTA_AXIS_2: u32 = 12;
    pub const SCROLL_FIXED_DELTA_AXIS_1: u32 = 93;
    pub const SCROLL_FIXED_DELTA_AXIS_2: u32 = 94;
    pub const SCROLL_IS_CONTINUOUS: u32 = 88;
    pub const SCROLL_POINT_DELTA_AXIS_1: u32 = 96;
    pub const SCROLL_POINT_DELTA_AXIS_2: u32 = 97;
    pub const SCROLL_PHASE: u32 = 99;
    pub const SCROLL_MOMENTUM_PHASE: u32 = 123;
}

const HID_EVENT_TAP: u32 = 0;
const SESSION_EVENT_TAP: u32 = 1;
const HEAD_INSERT: u32 = 0;
const OPTION_DEFAULT: u32 = 0;
const DISABLED_BY_TIMEOUT: u32 = 0xFFFF_FFFE;
const DISABLED_BY_USER_INPUT: u32 = 0xFFFF_FFFF;

type CGEventRef = *mut c_void;
type TapCallback = unsafe extern "C" fn(*mut c_void, u32, CGEventRef, *mut c_void) -> CGEventRef;

#[link(name = "CoreGraphics", kind = "framework")]
unsafe extern "C" {
    fn CGEventTapCreate(
        tap: u32,
        place: u32,
        options: u32,
        events_of_interest: u64,
        callback: TapCallback,
        user_info: *mut c_void,
    ) -> CFMachPortRef;
    fn CGEventTapEnable(tap: CFMachPortRef, enable: bool);
    fn CGEventGetIntegerValueField(event: CGEventRef, field: u32) -> i64;
    fn CGEventGetDoubleValueField(event: CGEventRef, field: u32) -> f64;
    fn CGEventSetIntegerValueField(event: CGEventRef, field: u32, value: i64);
    fn CGEventSetDoubleValueField(event: CGEventRef, field: u32, value: f64);
    fn CGEventGetFlags(event: CGEventRef) -> u64;
}

/// What the tap does with the event after the handler has seen it.
pub enum Verdict {
    Pass,
    Drop,
}

/// View of the event being delivered; valid only inside the handler call.
pub struct EventView<'a> {
    raw: CGEventRef,
    _life: PhantomData<&'a ()>,
}

impl EventView<'_> {
    pub fn int(&self, field: u32) -> i64 {
        // SAFETY: `raw` is the live event handed to the callback; the lifetime bounds the view to that call.
        unsafe { CGEventGetIntegerValueField(self.raw, field) }
    }

    pub fn float(&self, field: u32) -> f64 {
        // SAFETY: as in `int`.
        unsafe { CGEventGetDoubleValueField(self.raw, field) }
    }

    /// The media or volume key this event carries, if it is a system-defined key event.
    pub fn system_key(&self) -> Option<crate::system_keys::KeyPress> {
        // SAFETY: `raw` is the live event handed to the callback; NSEvent copies what it needs from it.
        let cg = unsafe { &*self.raw.cast::<objc2_core_graphics::CGEvent>() };
        let event = objc2_app_kit::NSEvent::eventWithCGEvent(cg)?;
        crate::system_keys::from_ns_event(&event)
    }

    pub fn flags(&self) -> u64 {
        // SAFETY: as in `int`.
        unsafe { CGEventGetFlags(self.raw) }
    }

    /// Edits the event in place; the modified event is what the rest of the system receives on `Verdict::Pass`.
    pub fn set_int(&self, field: u32, value: i64) {
        // SAFETY: as in `int`.
        unsafe { CGEventSetIntegerValueField(self.raw, field, value) }
    }

    pub fn set_float(&self, field: u32, value: f64) {
        // SAFETY: as in `int`.
        unsafe { CGEventSetDoubleValueField(self.raw, field, value) }
    }
}

type Handler = Box<dyn FnMut(u32, &EventView<'_>) -> Verdict + Send>;

struct Ctx {
    tap: AtomicPtr<c_void>,
    handler: Handler,
}

#[derive(Debug)]
pub enum TapError {
    /// No tap or no run-loop source came back: almost always missing Accessibility access.
    Unavailable,
    Thread(io::Error),
}

struct RunLoopHandle(CFRunLoop);

// SAFETY: CFRunLoopStop is documented as callable from any thread; the handle is used for nothing else.
unsafe impl Send for RunLoopHandle {}

pub struct EventTap {
    run_loop: RunLoopHandle,
    thread: Option<JoinHandle<()>>,
}

impl EventTap {
    /// Installs the tap on a dedicated thread. The handler runs inside the system input path:
    /// a slow handler delays every event on the machine and gets the tap disabled by macOS.
    pub fn spawn(
        event_mask: u64,
        handler: impl FnMut(u32, &EventView<'_>) -> Verdict + Send + 'static,
    ) -> Result<Self, TapError> {
        Self::spawn_at(SESSION_EVENT_TAP, event_mask, Box::new(handler))
    }

    /// The same where the events of the hardware enter the system, ahead of every tap of the session. Events that
    /// apps post (this one's included) never pass through it, and the next key cannot overtake one that the handler
    /// holds back while it posts others.
    pub fn spawn_hid(
        event_mask: u64,
        handler: impl FnMut(u32, &EventView<'_>) -> Verdict + Send + 'static,
    ) -> Result<Self, TapError> {
        Self::spawn_at(HID_EVENT_TAP, event_mask, Box::new(handler))
    }

    fn spawn_at(location: u32, event_mask: u64, handler: Handler) -> Result<Self, TapError> {
        let (ready_tx, ready_rx) = mpsc::channel();
        let thread = thread::Builder::new()
            .name("fig-event-tap".into())
            .spawn(move || run_tap_thread(location, event_mask, handler, ready_tx))
            .map_err(TapError::Thread)?;
        match ready_rx.recv() {
            Ok(Ok(run_loop)) => Ok(Self {
                run_loop,
                thread: Some(thread),
            }),
            Ok(Err(e)) => {
                let _ = thread.join();
                Err(e)
            }
            Err(_) => {
                let _ = thread.join();
                Err(TapError::Thread(io::Error::other(
                    "tap thread exited before reporting",
                )))
            }
        }
    }
}

impl Drop for EventTap {
    fn drop(&mut self) {
        if let Some(thread) = self.thread.take() {
            // CFRunLoopStop is a no-op until the loop is actually running, so a drop right after
            // spawn would otherwise hang the join. Keep poking until the thread has exited.
            while !thread.is_finished() {
                self.run_loop.0.stop();
                thread::sleep(Duration::from_millis(1));
            }
            let _ = thread.join();
        }
    }
}

fn run_tap_thread(
    location: u32,
    event_mask: u64,
    handler: Handler,
    ready: mpsc::Sender<Result<RunLoopHandle, TapError>>,
) {
    crate::qos::interactive();

    let ctx = Box::into_raw(Box::new(Ctx {
        tap: AtomicPtr::new(ptr::null_mut()),
        handler,
    }));
    // SAFETY: `ctx` stays allocated until the run loop below has stopped.
    let port = unsafe {
        CGEventTapCreate(
            location,
            HEAD_INSERT,
            OPTION_DEFAULT,
            event_mask,
            trampoline,
            ctx.cast(),
        )
    };
    if port.is_null() {
        // SAFETY: the tap was never created, so nothing else holds `ctx`.
        drop(unsafe { Box::from_raw(ctx) });
        let _ = ready.send(Err(TapError::Unavailable));
        return;
    }
    // SAFETY: `ctx` is valid; the callback reads this pointer only to re-arm the tap.
    unsafe { (*ctx).tap.store(port.cast(), Ordering::Release) };
    // SAFETY: CGEventTapCreate follows the create rule, so we own one reference.
    let port = unsafe { CFMachPort::wrap_under_create_rule(port) };

    let Ok(source) = port.create_runloop_source(0) else {
        // SAFETY: the tap is not attached to any run loop yet, so it cannot call back.
        drop(unsafe { Box::from_raw(ctx) });
        let _ = ready.send(Err(TapError::Unavailable));
        return;
    };
    let run_loop = CFRunLoop::get_current();
    // SAFETY: kCFRunLoopCommonModes is a process-lifetime constant.
    run_loop.add_source(&source, unsafe { kCFRunLoopCommonModes });
    // SAFETY: `port` is a valid tap.
    unsafe { CGEventTapEnable(port.as_concrete_TypeRef(), true) };
    let _ = ready.send(Ok(RunLoopHandle(run_loop)));

    CFRunLoop::run_current();

    // SAFETY: `port` is still a valid mach port; invalidating detaches the tap for good.
    unsafe { CFMachPortInvalidate(port.as_concrete_TypeRef()) };
    // SAFETY: the run loop has stopped and the port is invalidated, so no callback can run.
    drop(unsafe { Box::from_raw(ctx) });
}

unsafe extern "C" fn trampoline(
    _proxy: *mut c_void,
    kind: u32,
    event: CGEventRef,
    info: *mut c_void,
) -> CGEventRef {
    // SAFETY: `info` is the Ctx leaked by `run_tap_thread`; callbacks arrive only on that thread.
    let ctx = unsafe { &mut *info.cast::<Ctx>() };
    if matches!(kind, DISABLED_BY_TIMEOUT | DISABLED_BY_USER_INPUT) {
        // macOS unplugs taps that stall; re-arm instead of staying dead.
        // SAFETY: the stored pointer is the live tap set up before the run loop started.
        unsafe { CGEventTapEnable(ctx.tap.load(Ordering::Acquire).cast(), true) };
        return event;
    }
    let view = EventView {
        raw: event,
        _life: PhantomData,
    };
    match catch_unwind(AssertUnwindSafe(|| (ctx.handler)(kind, &view))) {
        Ok(Verdict::Drop) => ptr::null_mut(),
        // A panic must never swallow the user's input or unwind through CoreGraphics frames.
        _ => event,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn drop_right_after_spawn_does_not_hang() {
        // Without Accessibility access there is no tap to stop and the test is a no-op.
        for _ in 0..2000 {
            if let Ok(tap) = EventTap::spawn(mask(SCROLL_WHEEL), |_, _| Verdict::Pass) {
                drop(tap);
            }
        }
    }
}
