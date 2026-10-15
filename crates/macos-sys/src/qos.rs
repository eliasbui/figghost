/// Marks the calling thread latency-critical so input handling and animation frames are not queued
/// behind background work.
pub fn interactive() {
    // SAFETY: plain libc call affecting only the current thread.
    unsafe {
        libc::pthread_set_qos_class_self_np(libc::qos_class_t::QOS_CLASS_USER_INTERACTIVE, 0)
    };
}

/// Marks the calling thread as long-running work nobody waits on, such as reading large logs, so it yields to
/// everything the user is doing and runs on the efficiency cores.
pub fn utility() {
    // SAFETY: plain libc call affecting only the current thread.
    unsafe { libc::pthread_set_qos_class_self_np(libc::qos_class_t::QOS_CLASS_UTILITY, 0) };
}
