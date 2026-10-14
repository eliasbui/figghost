/// Marks the calling thread latency-critical so input handling and animation frames are not queued
/// behind background work.
pub fn interactive() {
    // SAFETY: plain libc call affecting only the current thread.
    unsafe {
        libc::pthread_set_qos_class_self_np(libc::qos_class_t::QOS_CLASS_USER_INTERACTIVE, 0)
    };
}
