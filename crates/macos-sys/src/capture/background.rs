//! Work off the main thread whose answer comes back to it.

use dispatch2::DispatchQueue;

/// A value that is only ever touched on the main thread, so it may travel inside a closure that must be
/// `Send`: the closures that carry it run on the main queue.
pub(crate) struct MainOnly<T>(T);

// SAFETY: a `MainOnly` is only created on the main thread and only unwrapped inside a block that runs on the
// main queue, so the value is never used from two threads. Moving the wrapper in between does not touch it.
unsafe impl<T> Send for MainOnly<T> {}

impl<T> MainOnly<T> {
    pub(crate) fn new(value: T) -> Self {
        Self(value)
    }

    /// A method rather than a field access, so a closure captures the whole wrapper (and with it the `Send`
    /// promise) and not the field.
    pub(crate) fn into_inner(self) -> T {
        self.0
    }
}

/// Runs `done` on the main thread soon, from any thread.
pub(crate) fn to_main<T: Send + 'static>(value: T, done: MainOnly<impl FnOnce(T) + 'static>) {
    DispatchQueue::main().exec_async(move || done.into_inner()(value));
}

/// Runs `work` on a new thread and hands its result to `done` on the main thread.
pub fn spawn<T: Send + 'static>(
    work: impl FnOnce() -> T + Send + 'static,
    done: impl FnOnce(T) + 'static,
) {
    let done = MainOnly::new(done);
    std::thread::spawn(move || to_main(work(), done));
}
